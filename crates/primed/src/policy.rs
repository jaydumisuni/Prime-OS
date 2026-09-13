use crate::registry::{verify_policy, RegistryError};
use prime_contracts::{GpuMode, NetworkMode, PolicyClass, WorkloadPolicy};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemdProperty {
    pub name: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SystemdEnforcementPlan {
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

pub fn compile_systemd(
    policy: &WorkloadPolicy,
) -> Result<SystemdEnforcementPlan, PolicyCompileError> {
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
    if !policy.devices.other.is_empty() {
        return Err(PolicyCompileError::Unsupported("non-USB device allowlists"));
    }
    validate_usb_allowlist(&policy.devices.usb)?;
    if !policy.secrets.grants.is_empty() {
        return Err(PolicyCompileError::Unsupported("secret grants"));
    }
    if matches!(policy.gpu.mode, GpuMode::Exclusive) {
        return Err(PolicyCompileError::Unsupported("exclusive GPU ownership"));
    }
    let restricted_class = matches!(
        policy.class,
        PolicyClass::UserApp | PolicyClass::Build | PolicyClass::ForeignRuntime
    );
    if restricted_class && !matches!(policy.gpu.mode, GpuMode::Deny) {
        return Err(PolicyCompileError::Unsupported(
            "shared/inherited GPU or device access for non-core workloads",
        ));
    }
    if policy.process.max_processes.is_some_and(|value| value == 0) {
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

    let mut properties = baseline_properties(restricted_class);
    if restricted_class {
        property(&mut properties, "DynamicUser", "yes");
        property(&mut properties, "RemoveIPC", "yes");
        property(&mut properties, "UMask", "0077");
    }
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
        GpuMode::Deny if policy.devices.usb.is_empty() => {
            property(&mut properties, "PrivateDevices", "yes")
        }
        GpuMode::Deny => {
            property(&mut properties, "DevicePolicy", "closed");
            for device in &policy.devices.usb {
                property(&mut properties, "DeviceAllow", format!("{device} rw"));
            }
        }
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

    Ok(SystemdEnforcementPlan {
        properties,
        background_allowed: policy.background.allowed,
        evidence_required: policy.evidence.required,
    })
}

fn validate_usb_allowlist(devices: &[String]) -> Result<(), PolicyCompileError> {
    let mut seen = HashSet::with_capacity(devices.len());
    for device in devices {
        let Some(rest) = device.strip_prefix("/dev/bus/usb/") else {
            return Err(PolicyCompileError::Invalid(
                "USB device path must be /dev/bus/usb/BBB/DDD",
            ));
        };
        let Some((bus, address)) = rest.split_once('/') else {
            return Err(PolicyCompileError::Invalid(
                "USB device path must be /dev/bus/usb/BBB/DDD",
            ));
        };
        if bus.len() != 3
            || address.len() != 3
            || !bus.bytes().all(|byte| byte.is_ascii_digit())
            || !address.bytes().all(|byte| byte.is_ascii_digit())
            || bus == "000"
            || address == "000"
            || address.contains('/')
        {
            return Err(PolicyCompileError::Invalid(
                "USB device path must be /dev/bus/usb/BBB/DDD",
            ));
        }
        if !seen.insert(device.as_str()) {
            return Err(PolicyCompileError::Invalid(
                "USB device allowlist contains duplicates",
            ));
        }
    }
    Ok(())
}

pub type NativeEnforcementPlan = SystemdEnforcementPlan;

pub fn compile_native(
    policy: &WorkloadPolicy,
) -> Result<NativeEnforcementPlan, PolicyCompileError> {
    compile_systemd(policy)
}

fn baseline_properties(restricted_class: bool) -> Vec<SystemdProperty> {
    let mut properties = Vec::new();
    property(&mut properties, "NoNewPrivileges", "yes");
    property(&mut properties, "PrivateTmp", "yes");
    property(&mut properties, "ProtectKernelTunables", "yes");
    property(&mut properties, "ProtectKernelModules", "yes");
    property(&mut properties, "ProtectControlGroups", "yes");
    property(&mut properties, "RestrictSUIDSGID", "yes");
    property(&mut properties, "LockPersonality", "yes");
    property(&mut properties, "KillMode", "control-group");
    if restricted_class {
        property(&mut properties, "ProtectSystem", "strict");
        property(&mut properties, "ProtectHome", "yes");
    }
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
            gpu: GpuPolicy {
                mode: GpuMode::Deny,
            },
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
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "ProtectSystem" && item.value == "strict"));
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "ProtectHome" && item.value == "yes"));
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "DynamicUser" && item.value == "yes"));
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "RemoveIPC" && item.value == "yes"));
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "UMask" && item.value == "0077"));
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

    #[test]
    fn non_core_shared_gpu_fails_closed_until_device_mediation_exists() {
        let mut value = policy();
        value.gpu.mode = GpuMode::Shared;
        value = seal_policy(value).expect("reseal");
        assert!(matches!(
            compile_native(&value),
            Err(PolicyCompileError::Unsupported(_))
        ));
    }
    #[test]
    fn usb_allowlist_compiles_to_closed_device_policy() {
        let mut value = policy();
        value.devices.usb = vec!["/dev/bus/usb/001/018".to_owned()];
        value = seal_policy(value).expect("reseal");
        let plan = compile_native(&value).expect("USB allowlist should compile");
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "DevicePolicy" && item.value == "closed"));
        assert!(plan
            .properties
            .iter()
            .any(|item| item.name == "DeviceAllow" && item.value == "/dev/bus/usb/001/018 rw"));
        assert!(!plan
            .properties
            .iter()
            .any(|item| item.name == "PrivateDevices" && item.value == "yes"));
    }

    #[test]
    fn usb_allowlist_rejects_non_usb_device_paths() {
        let mut value = policy();
        value.devices.usb = vec!["/dev/sda".to_owned()];
        value = seal_policy(value).expect("reseal");
        assert!(matches!(
            compile_native(&value),
            Err(PolicyCompileError::Invalid(_))
        ));
    }

    #[test]
    fn usb_allowlist_rejects_non_adjacent_duplicates() {
        let mut value = policy();
        value.devices.usb = vec![
            "/dev/bus/usb/001/018".to_owned(),
            "/dev/bus/usb/002/003".to_owned(),
            "/dev/bus/usb/001/018".to_owned(),
        ];
        value = seal_policy(value).expect("reseal");
        assert!(matches!(
            compile_native(&value),
            Err(PolicyCompileError::Invalid(_))
        ));
    }

    #[test]
    fn usb_allowlist_rejects_malformed_paths() {
        for path in [
            "/dev/bus/usb/1/018",
            "/dev/bus/usb/001/18",
            "/dev/bus/usb/000/018",
            "/dev/bus/usb/001/000",
            "/dev/bus/usb/001/018/extra",
            "/dev/bus/usb/abc/018",
        ] {
            let mut value = policy();
            value.devices.usb = vec![path.to_owned()];
            value = seal_policy(value).expect("reseal");
            assert!(
                matches!(compile_native(&value), Err(PolicyCompileError::Invalid(_))),
                "accepted malformed path: {path}"
            );
        }
    }

    #[test]
    fn generic_device_allowlist_remains_fail_closed() {
        let mut value = policy();
        value.devices.other = vec!["/dev/ttyUSB0".to_owned()];
        value = seal_policy(value).expect("reseal");
        assert!(matches!(
            compile_native(&value),
            Err(PolicyCompileError::Unsupported(_))
        ));
    }
}
