use smithay::{
    backend::{
        allocator::{
            format::FormatSet,
            gbm::{GbmAllocator, GbmBufferFlags, GbmDevice},
            Fourcc,
        },
        drm::{
            exporter::gbm::GbmFramebufferExporter,
            output::{DrmOutput, DrmOutputManager, DrmOutputRenderElements},
            DrmDevice, DrmDeviceFd, DrmDeviceNotifier,
        },
        renderer::{element::solid::SolidColorRenderElement, gles::GlesRenderer},
    },
    output::{Mode as WlMode, Output, PhysicalProperties},
    reexports::drm::control::{connector, crtc, Device as ControlDevice, Mode, ModeTypeFlags},
};
use std::{error::Error, io};

const PRIME_SCANOUT_FORMATS: &[Fourcc] = &[Fourcc::Abgr8888, Fourcc::Argb8888];

type PrimeAllocator = GbmAllocator<DrmDeviceFd>;
type PrimeExporter = GbmFramebufferExporter<DrmDeviceFd>;
pub type PrimeDrmOutput = DrmOutput<PrimeAllocator, PrimeExporter, (), DrmDeviceFd>;
pub type PrimeDrmOutputManager = DrmOutputManager<PrimeAllocator, PrimeExporter, (), DrmDeviceFd>;

#[derive(Debug)]
pub struct OutputSelection {
    pub connector: connector::Info,
    pub crtc: crtc::Handle,
    pub mode: Mode,
}

pub struct OutputBackend {
    pub output: Output,
    pub manager: PrimeDrmOutputManager,
    pub drm_output: PrimeDrmOutput,
    pub notifier: DrmDeviceNotifier,
    pub connector_name: String,
    pub mode: WlMode,
}

pub fn select_primary_output(device: &DrmDevice) -> Result<OutputSelection, Box<dyn Error>> {
    let resources = device.resource_handles()?;
    let mut connector_handles = resources.connectors().to_vec();
    connector_handles.sort_by_key(|handle| u32::from(*handle));

    for connector_handle in connector_handles {
        let connector = device.get_connector(connector_handle, false)?;
        if connector.state() != connector::State::Connected || connector.modes().is_empty() {
            continue;
        }

        let mode = connector
            .modes()
            .iter()
            .copied()
            .find(|mode| mode.mode_type().contains(ModeTypeFlags::PREFERRED))
            .unwrap_or(connector.modes()[0]);

        let current_encoder = connector.current_encoder();
        let mut encoder_handles = connector.encoders().to_vec();
        encoder_handles.sort_by_key(|handle| u32::from(*handle));
        encoder_handles.dedup();
        if let Some(current) = current_encoder {
            encoder_handles.retain(|handle| *handle != current);
            encoder_handles.insert(0, current);
        }

        for encoder_handle in encoder_handles {
            let encoder = device.get_encoder(encoder_handle)?;
            let mut possible_crtcs = resources.filter_crtcs(encoder.possible_crtcs());
            possible_crtcs.sort_by_key(|handle| u32::from(*handle));

            if let Some(current) = encoder.crtc() {
                if possible_crtcs.contains(&current) {
                    return Ok(OutputSelection {
                        connector,
                        crtc: current,
                        mode,
                    });
                }
            }

            if let Some(crtc) = possible_crtcs.first().copied() {
                return Ok(OutputSelection {
                    connector,
                    crtc,
                    mode,
                });
            }
        }
    }

    Err(io::Error::new(
        io::ErrorKind::NotFound,
        "no connected DRM connector has a compatible CRTC and mode",
    )
    .into())
}

pub fn initialize_primary_output(
    drm_fd: DrmDeviceFd,
    gbm: GbmDevice<DrmDeviceFd>,
    renderer: &mut GlesRenderer,
) -> Result<OutputBackend, Box<dyn Error>> {
    let (drm, notifier) = DrmDevice::new(drm_fd, true)?;
    let selection = select_primary_output(&drm)?;
    let connector_name = format!(
        "{}-{}",
        selection.connector.interface().as_str(),
        selection.connector.interface_id()
    );
    let wl_mode = WlMode::from(selection.mode);
    let (physical_width, physical_height) = selection.connector.size().unwrap_or((0, 0));
    let output = Output::new(
        connector_name.clone(),
        PhysicalProperties {
            size: (physical_width as i32, physical_height as i32).into(),
            subpixel: selection.connector.subpixel().into(),
            make: "Unknown".to_owned(),
            model: "Unknown".to_owned(),
        },
    );
    output.set_preferred(wl_mode);
    output.change_current_state(Some(wl_mode), None, None, Some((0, 0).into()));

    let allocator = GbmAllocator::new(
        gbm.clone(),
        GbmBufferFlags::RENDERING | GbmBufferFlags::SCANOUT,
    );
    // Client direct-scanout remains disabled until the Wayland protocol/dmabuf slice is proven.
    let exporter = GbmFramebufferExporter::new(gbm.clone(), None);
    let renderer_formats = renderer
        .egl_context()
        .dmabuf_render_formats()
        .iter()
        .copied()
        .collect::<FormatSet>();
    if renderer_formats.indexset().is_empty() {
        return Err(io::Error::other("EGL renderer reports no renderable dmabuf formats").into());
    }

    let mut manager = DrmOutputManager::new(
        drm,
        allocator,
        exporter,
        Some(gbm),
        PRIME_SCANOUT_FORMATS.iter().copied(),
        renderer_formats,
    );
    let planes = manager.device().planes(&selection.crtc)?;
    let render_elements: DrmOutputRenderElements<GlesRenderer, SolidColorRenderElement> =
        DrmOutputRenderElements::default();
    let drm_output = manager.initialize_output(
        selection.crtc,
        selection.mode,
        &[selection.connector.handle()],
        &output,
        Some(planes),
        renderer,
        &render_elements,
    )?;

    Ok(OutputBackend {
        output,
        manager,
        drm_output,
        notifier,
        connector_name,
        mode: wl_mode,
    })
}
