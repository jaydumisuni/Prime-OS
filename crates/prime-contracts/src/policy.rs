use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PolicyClass {
    #[serde(rename = "SYSTEM_CORE")]
    SystemCore,
    #[serde(rename = "SHELL")]
    Shell,
    #[serde(rename = "USER_APP")]
    UserApp,
    #[serde(rename = "BUILD")]
    Build,
    #[serde(rename = "FOREIGN_RUNTIME")]
    ForeignRuntime,
    #[serde(rename = "RECOVERY")]
    Recovery,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CpuPolicy {
    pub weight: u16,
    pub quota_percent: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPolicy {
    pub max_bytes: Option<u64>,
    pub swap_max_bytes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum GpuMode {
    #[serde(rename = "DENY")]
    Deny,
    #[serde(rename = "SHARED")]
    Shared,
    #[serde(rename = "EXCLUSIVE")]
    Exclusive,
    #[serde(rename = "INHERIT")]
    Inherit,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct GpuPolicy {
    pub mode: GpuMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoragePolicy {
    pub quota_bytes: Option<u64>,
    pub io_weight: u16,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProcessPolicy {
    pub max_processes: Option<u64>,
    pub max_runtime_seconds: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NetworkMode {
    #[serde(rename = "OFFLINE")]
    Offline,
    #[serde(rename = "LAN_ONLY")]
    LanOnly,
    #[serde(rename = "OUTBOUND_INTERNET")]
    OutboundInternet,
    #[serde(rename = "DESTINATION_RESTRICTED")]
    DestinationRestricted,
    #[serde(rename = "LOCAL_LISTENER")]
    LocalListener,
    #[serde(rename = "INBOUND_ALLOWED")]
    InboundAllowed,
    #[serde(rename = "UNRESTRICTED")]
    Unrestricted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkPolicy {
    pub mode: NetworkMode,
    #[serde(default)]
    pub destinations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum FilesystemAccess {
    #[serde(rename = "READ")]
    Read,
    #[serde(rename = "WRITE")]
    Write,
    #[serde(rename = "CREATE")]
    Create,
    #[serde(rename = "EXECUTE")]
    Execute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FilesystemExposure {
    pub path: String,
    #[serde(default)]
    pub access: Vec<FilesystemAccess>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct FilesystemPolicy {
    #[serde(default)]
    pub exposures: Vec<FilesystemExposure>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct DevicePolicy {
    #[serde(default)]
    pub usb: Vec<String>,
    #[serde(default)]
    pub other: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct SecretPolicy {
    #[serde(default)]
    pub grants: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackgroundPolicy {
    pub allowed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidencePolicy {
    pub required: bool,
    #[serde(default)]
    pub classes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkloadPolicy {
    pub schema: String,
    pub policy_id: Uuid,
    pub revision: u64,
    pub digest: String,
    pub class: PolicyClass,
    pub cpu: CpuPolicy,
    pub memory: MemoryPolicy,
    pub gpu: GpuPolicy,
    pub storage: StoragePolicy,
    pub process: ProcessPolicy,
    pub network: NetworkPolicy,
    pub filesystem: FilesystemPolicy,
    pub devices: DevicePolicy,
    pub secrets: SecretPolicy,
    pub background: BackgroundPolicy,
    pub evidence: EvidencePolicy,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyReference {
    pub policy_id: Uuid,
    pub policy_revision: u64,
    pub policy_digest: String,
}
