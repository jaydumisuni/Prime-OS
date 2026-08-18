use crate::ReleaseChannel;
use serde::{Deserialize, Serialize};

pub const GENERATION_SEED_SCHEMA: &str = "prime.generation-seed.v1";
pub const GENERATION_HEALTH_SCHEMA: &str = "prime.generation-health.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationSeed {
    pub schema: String,
    pub generation_id: String,
    pub channel: ReleaseChannel,
    pub created_at: String,
    pub source_revision: String,
    pub base_image_digest: String,
    pub boot_attempt_limit: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationHealthReport {
    pub schema: String,
    pub generation_id: String,
    pub image_digest: String,
    pub observed_at: String,
    pub core_interface_ready: bool,
    pub host_identity_ready: bool,
    pub hardware_baseline_ready: bool,
    pub shell_ready: bool,
    pub recovery_ready: bool,
    #[serde(default)]
    pub limitations: Vec<String>,
}

impl GenerationHealthReport {
    pub fn all_required_ready(&self) -> bool {
        self.core_interface_ready
            && self.host_identity_ready
            && self.hardware_baseline_ready
            && self.shell_ready
            && self.recovery_ready
            && self.limitations.is_empty()
    }
}
