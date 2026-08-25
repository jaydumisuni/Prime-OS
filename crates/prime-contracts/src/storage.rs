use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const STORAGE_INVENTORY_SCHEMA: &str = "prime.storage-inventory.v1";
pub const STORAGE_RESERVE_POLICY_SCHEMA: &str = "prime.storage-reserve-policy.v1";
pub const STORAGE_PREFLIGHT_SCHEMA: &str = "prime.storage-preflight.v1";
pub const STORAGE_PRESSURE_SCHEMA: &str = "prime.storage-pressure.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StorageScope {
    LocalPhysical,
    Remote,
    Memory,
    Overlay,
    Virtual,
    Unknown,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageCapacity {
    pub source: String,
    pub fragment_size_bytes: u64,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub reserved_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageMount {
    pub mount_id: u64,
    pub parent_mount_id: u64,
    pub major_minor: String,
    pub root: String,
    pub mount_point: String,
    pub filesystem_type: String,
    pub mount_source: Option<String>,
    pub read_only: bool,
    pub scope: StorageScope,
    pub filesystem_key: String,
    pub capacity: Option<StorageCapacity>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageTotals {
    pub filesystem_count: u64,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub reserved_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageGenerationAccounting {
    pub current_generation_id: String,
    pub current_generation_bytes: Option<u64>,
    pub previous_known_good_bytes: Option<u64>,
    pub recovery_generation_bytes: Option<u64>,
    pub staged_generation_bytes: Option<u64>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageReserveVisibility {
    pub policy_configured: bool,
    pub protected_rollback_recovery_bytes: Option<u64>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoragePressureState {
    Unknown,
    Normal,
    Low,
    Critical,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoragePressure {
    pub state: StoragePressureState,
    pub available_bytes: Option<u64>,
    pub low_threshold_bytes: Option<u64>,
    pub critical_threshold_bytes: Option<u64>,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageInventory {
    pub schema: String,
    pub observed_at: String,
    pub mount_namespace_source: String,
    #[serde(default)]
    pub mounts: Vec<StorageMount>,
    pub local_physical_totals: StorageTotals,
    pub root_mount_id: Option<u64>,
    pub generation_accounting: StorageGenerationAccounting,
    pub reserve: StorageReserveVisibility,
    pub pressure: StoragePressure,
    #[serde(default)]
    pub limitations: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageProjection {
    pub interface: String,
    pub interface_version: String,
    pub host_id: Uuid,
    pub generation_id: String,
    pub inventory: StorageInventory,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StorageReservePolicy {
    pub schema: String,
    pub protected_rollback_recovery_bytes: u64,
    pub low_space_warning_bytes: u64,
    pub critical_space_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoragePreflightRequest {
    pub schema: String,
    pub required_staging_bytes: u64,
    pub target_mount_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum StoragePreflightReason {
    SpaceAvailableWithProtectedReserve,
    ProtectedReserveUnconfigured,
    TargetMountMissing,
    TargetMountNotLocalPhysical,
    TargetCapacityUnavailable,
    ArithmeticOverflow,
    ProtectedReserveWouldBeConsumed,
    InvalidRequestSchema,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoragePreflight {
    pub schema: String,
    pub target_mount_id: Option<u64>,
    pub required_staging_bytes: u64,
    pub available_bytes: Option<u64>,
    pub protected_rollback_recovery_bytes: Option<u64>,
    pub remaining_after_stage_bytes: Option<u64>,
    pub admitted: bool,
    pub reason: StoragePreflightReason,
    pub observed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoragePressureEvidence {
    pub schema: String,
    pub evidence_id: Uuid,
    pub host_id: Uuid,
    pub generation_id: String,
    pub previous_state: Option<StoragePressureState>,
    pub current_state: StoragePressureState,
    pub root_mount_id: Option<u64>,
    pub available_bytes: Option<u64>,
    pub observed_at: String,
}
