use crate::{
    ArtifactFormat, ExecutionBackend, MechanicalCompatibilityState, PolicyReference, RuntimeFamily,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const APPLICATIONS_PROJECTION_SCHEMA: &str = "prime.applications.v1";
pub const SHELL_LAUNCH_REQUEST_SCHEMA: &str = "prime.shell-launch-request.v1";

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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplicationEntry {
    pub application_id: Uuid,
    pub display_name: String,
    pub profile_revision: u64,
    pub profile_digest: String,
    pub execution_backend: ExecutionBackend,
    pub compatibility: CompatibilityRecord,
    pub launch_ready: bool,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApplicationsProjection {
    pub schema: String,
    pub interface: String,
    pub interface_version: String,
    pub host_id: Uuid,
    pub generation_id: String,
    #[serde(default)]
    pub applications: Vec<ApplicationEntry>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ShellLaunchRequest {
    pub schema: String,
    pub application_id: Uuid,
}
