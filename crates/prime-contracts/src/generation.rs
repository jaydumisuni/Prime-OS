use crate::ReleaseChannel;
use serde::{Deserialize, Serialize};

pub const GENERATION_SEED_SCHEMA: &str = "prime.generation-seed.v1";

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
