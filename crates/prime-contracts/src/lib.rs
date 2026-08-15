pub mod application;
pub mod exec;
pub mod generation;
pub mod policy;
pub mod storage;

pub use application::*;
pub use exec::*;
pub use generation::*;
pub use policy::*;
pub use storage::*;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

pub const HOST_IDENTITY_SCHEMA: &str = "prime.host-identity.v1";
pub const HARDWARE_GRAPH_SCHEMA: &str = "prime.hardware-graph.v1";
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

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardwareInventory {
    pub host_arch: String,
    pub system: SystemHardware,
    pub firmware: FirmwareHardware,
    pub cpu: CpuHardware,
    pub memory: MemoryHardware,
    #[serde(default)]
    pub pci_devices: Vec<PciHardware>,
    #[serde(default)]
    pub usb_devices: Vec<UsbHardware>,
    #[serde(default)]
    pub block_devices: Vec<BlockHardware>,
    #[serde(default)]
    pub network_interfaces: Vec<NetworkHardware>,
    #[serde(default)]
    pub display_connectors: Vec<DisplayConnector>,
    #[serde(default)]
    pub input_devices: Vec<InputHardware>,
    #[serde(default)]
    pub sound_cards: Vec<SoundHardware>,
    #[serde(default)]
    pub thermal_zones: Vec<ThermalHardware>,
    #[serde(default)]
    pub tpm_devices: Vec<TpmHardware>,
    pub virtualization: VirtualizationHardware,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SystemHardware {
    pub vendor: Option<String>,
    pub product_name: Option<String>,
    pub product_version: Option<String>,
    pub board_vendor: Option<String>,
    pub board_name: Option<String>,
    pub board_version: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct FirmwareHardware {
    pub uefi: bool,
    pub bios_vendor: Option<String>,
    pub bios_version: Option<String>,
    pub bios_date: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct CpuHardware {
    pub logical_cpus: u32,
    pub vendor: Option<String>,
    pub model: Option<String>,
    pub supports_vmx: bool,
    pub supports_svm: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryHardware {
    pub total_bytes: Option<u64>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct PciHardware {
    pub address: String,
    pub vendor_id: Option<String>,
    pub device_id: Option<String>,
    pub class_code: Option<String>,
    pub class_family: Option<String>,
    pub subsystem_vendor_id: Option<String>,
    pub subsystem_device_id: Option<String>,
    pub driver: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct UsbHardware {
    pub kernel_name: String,
    pub bus_number: Option<u32>,
    pub device_number: Option<u32>,
    pub vendor_id: String,
    pub product_id: String,
    pub device_class: Option<String>,
    pub device_subclass: Option<String>,
    pub device_protocol: Option<String>,
    pub manufacturer: Option<String>,
    pub product: Option<String>,
    pub speed_mbps: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BlockHardware {
    pub kernel_name: String,
    pub kind: String,
    pub size_bytes: Option<u64>,
    pub read_only: Option<bool>,
    pub removable: Option<bool>,
    pub rotational: Option<bool>,
    pub logical_block_size: Option<u64>,
    pub physical_block_size: Option<u64>,
    pub vendor: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct NetworkHardware {
    pub kernel_name: String,
    pub interface_type: Option<u32>,
    pub driver: Option<String>,
    pub wireless: bool,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct DisplayConnector {
    pub kernel_name: String,
    pub status: Option<String>,
    #[serde(default)]
    pub modes: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct InputHardware {
    pub kernel_name: String,
    pub name: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoundHardware {
    pub kernel_name: String,
    pub id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ThermalHardware {
    pub kernel_name: String,
    pub zone_type: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TpmHardware {
    pub kernel_name: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct VirtualizationHardware {
    pub cpu_vmx: bool,
    pub cpu_svm: bool,
    pub kvm_device_present: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardwareGraph {
    pub schema: String,
    pub revision: u64,
    pub topology_digest: String,
    pub observed_at: String,
    pub inventory: HardwareInventory,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HardwareProjection {
    pub interface: String,
    pub interface_version: String,
    pub host_id: Uuid,
    pub generation_id: String,
    pub graph: HardwareGraph,
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
