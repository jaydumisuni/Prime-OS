use smithay::{
    backend::drm::DrmDevice,
    reexports::drm::control::{
        connector, crtc, Device as ControlDevice, Mode, ModeTypeFlags,
    },
};
use std::{error::Error, io};

#[derive(Debug)]
pub struct OutputSelection {
    pub connector: connector::Info,
    pub crtc: crtc::Handle,
    pub mode: Mode,
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

        let mut encoder_handles = Vec::with_capacity(connector.encoders().len() + 1);
        if let Some(current) = connector.current_encoder() {
            encoder_handles.push(current);
        }
        for handle in connector.encoders() {
            if !encoder_handles.contains(handle) {
                encoder_handles.push(*handle);
            }
        }
        encoder_handles.sort_by_key(|handle| u32::from(*handle));

        for encoder_handle in encoder_handles {
            let encoder = device.get_encoder(encoder_handle)?;
            let mut possible_crtcs = resources
                .filter_crtcs(encoder.possible_crtcs())
                .collect::<Vec<_>>();
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
