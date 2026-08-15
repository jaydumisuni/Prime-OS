use crate::{
    ArtifactFormat, ExecutionBackend, MechanicalCompatibilityState, PolicyReference, RuntimeFamily,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplicationArtifact {
    pub identity: String,
    pub format: ArtifactFormat,
    pub runtime_family: RuntimeFamily,
    pub workload_arch: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CompatibilityRecord {
    pub state: MechanicalCompatibilityState,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplicationProfile {
    pub schema: String,
    pub application_id: Uuid,
    pub profile_revision: u64,
    pub profile_digest: String,
    pub display_name: String,
    pub artifact: ApplicationArtifact,
    pub execution_backend: ExecutionBackend,
    #[serde(default)]
    pub dependencies: Vec<String>,
    pub workload_policy: PolicyReference,
    #[serde(default)]
    pub permissions: Vec<String>,
    pub compatibility: CompatibilityRecord,
    pub revoked: bool,
    pub revocation_reason: Option<String>,
    pub created_at: String,
}
