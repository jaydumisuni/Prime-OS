use smithay::{
    backend::drm::DrmDevice,
    reexports::drm::control::{connector, crtc, Device as ControlDevice, Mode, ModeTypeFlags},
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
