use crate::registry::{verify_policy, RegistryError};
use prime_contracts::{GpuMode, NetworkMode, WorkloadPolicy};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemdProperty {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeEnforcementPlan {
    pub properties: Vec<SystemdProperty>,
    pub background_allowed: bool,
    pub evidence_required: bool,
}

#[derive(Debug, Error)]
pub enum PolicyCompileError {
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error("invalid policy value: {0}")]
    Invalid(&'static str),
    #[error("P1 native backend cannot yet enforce: {0}")]
    Unsupported(&'static str),
}

pub fn compile_native(policy: &WorkloadPolicy) -> Result<NativeEnforcementPlan, PolicyCompileError> {
    verify_policy(policy)?;
    if !(1..=10_000).contains(&policy.cpu.weight) {
        return Err(PolicyCompileError::Invalid("cpu.weight must be 1..=10000"));
    }
    if policy
        .cpu
        .quota_percent
        .is_some_and(|quota| !(1..=10_000).contains(&quota))
    {
        return Err(PolicyCompileError::Invalid(
            "cpu.quota_percent must be 1..=10000 when set",
        ));
    }
    if !(1..=10_000).contains(&policy.storage.io_weight) {
        return Err(PolicyCompileError::Invalid(
            "storage.io_weight must be 1..=10000",
        ));
    }
    if policy.storage.quota_bytes.is_some() {
        return Err(PolicyCompileError::Unsupported("storage quota"));
    }
    if !policy.filesystem.exposures.is_empty() {
        return Err(PolicyCompileError::Unsupported(
            "filesystem exposure/Landlock rules",
        ));
    }
    if !policy.devices.usb.is_empty() || !policy.devices.other.is_empty() {
        return Err(PolicyCompileError::Unsupported("device allowlists"));
    }
    if !policy.secrets.grants.is_empty() {
        return Err(PolicyCompileError::Unsupported("secret grants"));
    }
    if matches!(policy.gpu.mode, GpuMode::Exclusive) {
        return Err(PolicyCompileError::Unsupported("exclusive GPU ownership"));
    }
    if policy
        .process
        .max_processes
        .is_some_and(|value| value == 0)
    {
        return Err(PolicyCompileError::Invalid(
            "process.max_processes must be positive when set",
        ));
    }
    if policy
        .process
        .max_runtime_seconds
        .is_some_and(|value| value == 0)
    {
        return Err(PolicyCompileError::Invalid(
            "process.max_runtime_seconds must be positive when set",
        ));
    }
    if policy.memory.max_bytes.is_some_and(|value| value == 0) {
        return Err(PolicyCompileError::Invalid(
            "memory.max_bytes must be positive when set",
        ));
    }

    let mut properties = baseline_properties();
    property(&mut properties, "CPUWeight", policy.cpu.weight);
    if let Some(quota) = policy.cpu.quota_percent {
        property(&mut properties, "CPUQuota", format!("{quota}%"));
    }
    if let Some(limit) = policy.memory.max_bytes {
        property(&mut properties, "MemoryMax", limit);
    }
    if let Some(limit) = policy.memory.swap_max_bytes {
        property(&mut properties, "MemorySwapMax", limit);
    }
    property(&mut properties, "IOWeight", policy.storage.io_weight);
    if let Some(max) = policy.process.max_processes {
        property(&mut properties, "TasksMax", max);
    }
    if let Some(seconds) = policy.process.max_runtime_seconds {
        property(&mut properties, "RuntimeMaxSec", seconds);
    }

    match policy.gpu.mode {
        GpuMode::Deny => property(&mut properties, "PrivateDevices", "yes"),
        GpuMode::Shared | GpuMode::Inherit => {}
        GpuMode::Exclusive => unreachable!("exclusive GPU was rejected above"),
    }

    match policy.network.mode {
        NetworkMode::Offline => {
            if !policy.network.destinations.is_empty() {
                return Err(PolicyCompileError::Invalid(
                    "OFFLINE policy cannot carry destinations",
                ));
            }
            property(&mut properties, "PrivateNetwork", "yes");
        }
        NetworkMode::Unrestricted => {
            if !policy.network.destinations.is_empty() {
                return Err(PolicyCompileError::Invalid(
                    "UNRESTRICTED policy cannot carry destinations",
                ));
            }
        }
        NetworkMode::LanOnly => return Err(PolicyCompileError::Unsupported("LAN_ONLY network")),
        NetworkMode::OutboundInternet => {
            return Err(PolicyCompileError::Unsupported("OUTBOUND_INTERNET network"));
        }
        NetworkMode::DestinationRestricted => {
            return Err(PolicyCompileError::Unsupported(
                "DESTINATION_RESTRICTED network",
            ));
        }
        NetworkMode::LocalListener => {
            return Err(PolicyCompileError::Unsupported("LOCAL_LISTENER network"));
        }
        NetworkMode::InboundAllowed => {
            return Err(PolicyCompileError::Unsupported("INBOUND_ALLOWED network"));
        }
    }

    Ok(NativeEnforcementPlan {
        properties,
        background_allowed: policy.background.allowed,
        evidence_required: policy.evidence.required,
    })
}

fn baseline_properties() -> Vec<SystemdProperty> {
    let mut properties = Vec::new();
    property(&mut properties, "NoNewPrivileges", "yes");
    property(&mut properties, "PrivateTmp", "yes");
    property(&mut properties, "ProtectKernelTunables", "yes");
    property(&mut properties, "ProtectKernelModules", "yes");
    property(&mut properties, "ProtectControlGroups", "yes");
    property(&mut properties, "RestrictSUIDSGID", "yes");
    property(&mut properties, "LockPersonality", "yes");
    property(&mut properties, "KillMode", "control-group");
    properties
}

fn property<T: ToString>(properties: &mut Vec<SystemdProperty>, name: &str, value: T) {
    properties.push(SystemdProperty {
        name: name.to_owned(),
        value: value.to_string(),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::seal_policy;
    use prime_contracts::*;
    use uuid::Uuid;

    fn policy() -> WorkloadPolicy {
        seal_policy(WorkloadPolicy {
            schema: WORKLOAD_POLICY_SCHEMA.to_owned(),
            policy_id: Uuid::now_v7(),
            revision: 1,
            digest: String::new(),
            class: PolicyClass::UserApp,
            cpu: CpuPolicy {
                weight: 200,
                quota_percent: Some(50),
            },
            memory: MemoryPolicy {
                max_bytes: Some(256 * 1024 * 1024),
                swap_max_bytes: Some(0),
            },
            gpu: GpuPolicy { mode: GpuMode::Deny },
            storage: StoragePolicy {
                quota_bytes: None,
                io_weight: 100,
            },
            process: ProcessPolicy {
                max_processes: Some(32),
                max_runtime_seconds: Some(30),
            },
            network: NetworkPolicy {
                mode: NetworkMode::Offline,
                destinations: Vec::new(),
            },
            filesystem: FilesystemPolicy::default(),
            devices: DevicePolicy::default(),
            secrets: SecretPolicy::default(),
            background: BackgroundPolicy { allowed: false },
            evidence: EvidencePolicy {
                required: true,
                classes: vec!["exit".to_owned()],
            },
        })
        .expect("seal")
    }

    #[test]
    fn offline_policy_compiles_to_restrictive_plan() {
        let plan = compile_native(&policy()).expect("compile");
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "PrivateNetwork" && item.value == "yes"));
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "CPUQuota" && item.value == "50%"));
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "PrivateDevices" && item.value == "yes"));
    }

    #[test]
    fn destination_restricted_network_fails_closed_until_backend_exists() {
        let mut value = policy();
        value.network.mode = NetworkMode::DestinationRestricted;
        value.network.destinations = vec!["example.com:443".to_owned()];
        value = seal_policy(value).expect("reseal");
        assert!(matches!(
            compile_native(&value),
            Err(PolicyCompileError::Unsupported(_))
        ));
    }

    #[test]
    fn filesystem_exposure_fails_closed_until_landlock_backend_exists() {
        let mut value = policy();
        value.filesystem.exposures.push(FilesystemExposure {
            path: "/work".to_owned(),
            access: vec![FilesystemAccess::Read],
        });
        value = seal_policy(value).expect("reseal");
        assert!(matches!(
            compile_native(&value),
            Err(PolicyCompileError::Unsupported(_))
        ));
    }
}
