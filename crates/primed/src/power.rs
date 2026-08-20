use prime_contracts::{
    SystemPowerAction, SystemPowerEvidence, SystemPowerRequest, SYSTEM_POWER_EVIDENCE_SCHEMA,
    SYSTEM_POWER_REQUEST_SCHEMA,
};
use std::{fmt, io, process::Command};

const SYSTEMCTL: &str = "/usr/bin/systemctl";

#[derive(Debug)]
pub enum PowerError {
    InvalidSchema,
    Spawn(io::Error),
    Rejected(String),
}

impl fmt::Display for PowerError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSchema => write!(formatter, "power request schema is not supported"),
            Self::Spawn(error) => write!(formatter, "system power executor could not start: {error}"),
            Self::Rejected(status) => write!(formatter, "system power executor rejected request: {status}"),
        }
    }
}

pub fn execute(request: &SystemPowerRequest) -> Result<SystemPowerEvidence, PowerError> {
    if request.schema != SYSTEM_POWER_REQUEST_SCHEMA {
        return Err(PowerError::InvalidSchema);
    }
    let action = systemctl_action(request.action);
    let status = Command::new(SYSTEMCTL)
        .args(["--no-block", action])
        .status()
        .map_err(PowerError::Spawn)?;
    if !status.success() {
        return Err(PowerError::Rejected(status.to_string()));
    }
    Ok(SystemPowerEvidence {
        schema: SYSTEM_POWER_EVIDENCE_SCHEMA.to_owned(),
        action: request.action,
        accepted: true,
        executor: "systemd.systemctl.v1".to_owned(),
    })
}

fn systemctl_action(action: SystemPowerAction) -> &'static str {
    match action {
        SystemPowerAction::Reboot => "reboot",
        SystemPowerAction::PowerOff => "poweroff",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reboot_maps_to_fixed_systemctl_action() {
        assert_eq!(systemctl_action(SystemPowerAction::Reboot), "reboot");
    }

    #[test]
    fn poweroff_maps_to_fixed_systemctl_action() {
        assert_eq!(systemctl_action(SystemPowerAction::PowerOff), "poweroff");
    }

    #[test]
    fn invalid_schema_is_rejected_before_execution() {
        let request = SystemPowerRequest {
            schema: "wrong".to_owned(),
            action: SystemPowerAction::Reboot,
        };
        assert!(matches!(execute(&request), Err(PowerError::InvalidSchema)));
    }
}
