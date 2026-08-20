use serde::{Deserialize, Serialize};

pub const SYSTEM_POWER_REQUEST_SCHEMA: &str = "prime.system-power-request.v1";
pub const SYSTEM_POWER_EVIDENCE_SCHEMA: &str = "prime.system-power-evidence.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SystemPowerAction {
    Reboot,
    PowerOff,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemPowerRequest {
    pub schema: String,
    pub action: SystemPowerAction,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemPowerEvidence {
    pub schema: String,
    pub action: SystemPowerAction,
    pub accepted: bool,
    pub executor: String,
}
