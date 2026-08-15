use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const HOST_IDENTITY_SCHEMA: &str = "prime.host-identity.v1";
pub const GENERATION_SCHEMA: &str = "prime.generation.v1";
pub const CAPABILITY_INTERFACE: &str = "prime.capability.v1";
pub const CAPABILITY_INTERFACE_VERSION: &str = "1.0";
pub const APPLICATION_PROFILE_SCHEMA: &str = "prime.application-profile.v1";
pub const WORKLOAD_POLICY_SCHEMA: &str = "prime.workload-policy.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostIdentity {
    pub schema: String,
    pub host_id: Uuid,
    pub lineage_id: Uuid,
    pub created_at: String,
    pub host_arch: String,
    pub hardware_fingerprint: HardwareFingerprint,
    pub rebind_revision: u64,
    pub supersedes_host_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardwareFingerprint {
    pub algorithm: String,
    pub digest: Option<String>,
    pub confidence: FingerprintConfidence,
    pub observed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FingerprintConfidence {
    High,
    Medium,
    Low,
    Unprobed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GenerationRecord {
    pub schema: String,
    pub generation_id: String,
    pub image_digest: String,
    pub channel: ReleaseChannel,
    pub created_at: String,
    pub source_revision: String,
    pub state: GenerationState,
    pub boot_attempts_remaining: Option<u32>,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ReleaseChannel {
    Lab,
    Candidate,
    Stable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GenerationState {
    Staged,
    BootTry,
    HealthProving,
    KnownGood,
    Rejected,
    RolledBack,
    Recovery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostProjection {
    pub interface: String,
    pub interface_version: String,
    pub host: HostProjectionBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HostProjectionBody {
    pub host_id: Uuid,
    pub host_arch: String,
    pub generation_id: String,
    pub hardware_graph_revision: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VersionsProjection {
    pub interface: String,
    pub supported_versions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthProjection {
    pub interface: String,
    pub interface_version: String,
    pub host_id: Uuid,
    pub generation_id: String,
    pub status: HealthStatus,
    pub observed_at: String,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilitiesProjection {
    pub interface: String,
    pub interface_version: String,
    pub host_id: Uuid,
    pub generation_id: String,
    pub capabilities: Vec<CapabilityDescriptor>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Failed,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CapabilityDescriptor {
    pub capability_id: String,
    pub capability_version: String,
    pub family: String,
    pub provider: CapabilityProvider,
    pub availability: CapabilityAvailability,
    #[serde(default)]
    pub effects: Vec<String>,
    pub accepts: CapabilityAccepts,
    #[serde(default)]
    pub permissions: Vec<String>,
    pub resources: Value,
    #[serde(default)]
    pub hardware_requirements: Vec<String>,
    pub limits: Value,
    pub health: CapabilityHealth,
    #[serde(default)]
    pub limitations: Vec<String>,
    pub placement: CapabilityPlacement,
    #[serde(default)]
    pub expected_evidence: Vec<String>,
    pub rollback: CapabilityRollback,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityProvider {
    pub id: String,
    pub generation_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum CapabilityAvailability {
    Available,
    Degraded,
    Unavailable,
    Incompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CapabilityAccepts {
    #[serde(default)]
    pub formats: Vec<String>,
    #[serde(default)]
    pub runtime_families: Vec<String>,
    #[serde(default)]
    pub workload_arches: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityHealth {
    pub status: HealthStatus,
    pub observed_at: String,
    #[serde(default)]
    pub evidence_refs: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityPlacement {
    pub scope: String,
    pub host_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityRollback {
    pub supported: bool,
    pub mode: Option<String>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterfaceError {
    pub error: String,
    pub message: String,
    #[serde(default)]
    pub supported_versions: Vec<String>,
    #[serde(default)]
    pub requested_versions: Vec<String>,
}
