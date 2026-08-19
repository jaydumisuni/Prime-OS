pub mod exec;
pub mod generation;
pub mod hardware;
pub mod identity;
pub mod launcher;
pub mod p1_health;
pub mod policy;
pub mod registry;
pub mod server;
pub mod storage;
pub mod system_status;

use prime_contracts::{
    CapabilityAccepts, CapabilityAvailability, CapabilityDescriptor, CapabilityHealth,
    CapabilityPlacement, CapabilityProvider, CapabilityRollback, FingerprintConfidence,
    GenerationRecord, HardwareGraph, HealthStatus, HostIdentity, StorageInventory, StorageScope,
    NATIVE_LAUNCH_EVIDENCE_SCHEMA, STORAGE_INVENTORY_SCHEMA, STORAGE_PRESSURE_SCHEMA,
};
use serde_json::json;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct CoreState {
    pub host: HostIdentity,
    pub generation: GenerationRecord,
    pub hardware: Arc<HardwareGraph>,
    pub storage: Arc<RwLock<StorageInventory>>,
    pub capabilities: Arc<Vec<CapabilityDescriptor>>,
    pub health_limitations: Arc<Vec<String>>,
    pub state_dir: Arc<PathBuf>,
    pub systemd_run: Arc<PathBuf>,
    pub storage_mountinfo: Arc<PathBuf>,
    pub storage_policy_file: Arc<PathBuf>,
    pub system_root: Arc<PathBuf>,
    pub started_at: String,
}

impl CoreState {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        host: HostIdentity,
        generation: GenerationRecord,
        hardware: HardwareGraph,
        storage: StorageInventory,
        state_dir: PathBuf,
        systemd_run: PathBuf,
        storage_mountinfo: PathBuf,
        storage_policy_file: PathBuf,
        system_root: PathBuf,
        observed_at: String,
    ) -> Self {
        let provider = CapabilityProvider {
            id: "prime".to_owned(),
            generation_id: generation.generation_id.clone(),
        };
        let placement = CapabilityPlacement {
            scope: "HOST_LOCAL".to_owned(),
            host_id: host.host_id,
        };
        let fingerprint_limitations = if matches!(
            &host.hardware_fingerprint.confidence,
            FingerprintConfidence::Unprobed | FingerprintConfidence::Low
        ) {
            vec!["High/medium-confidence hardware fingerprint is not enrolled".to_owned()]
        } else {
            Vec::new()
        };
        let hardware_status = status_for(&hardware.limitations);
        let host_status = status_for(&fingerprint_limitations);
        let generation_limitations = crate::generation::health_limitations(&generation);

        let root_storage_usable = storage.root_mount_id.and_then(|root_id| {
            storage
                .mounts
                .iter()
                .find(|mount| mount.mount_id == root_id)
                .map(|mount| mount.scope == StorageScope::LocalPhysical && mount.capacity.is_some())
        }) == Some(true);
        let storage_core_limitations = if root_storage_usable {
            Vec::new()
        } else {
            vec!["Root local-physical storage capacity is unavailable".to_owned()]
        };

        let mut health_limitations = hardware.limitations.clone();
        health_limitations.extend(fingerprint_limitations.clone());
        health_limitations.extend(storage_core_limitations);
        health_limitations.extend(generation_limitations.clone());
        health_limitations.sort();
        health_limitations.dedup();

        let host_health = CapabilityHealth {
            status: host_status,
            observed_at: observed_at.clone(),
            evidence_refs: vec!["prime.host-identity.v1".to_owned()],
        };
        let hardware_health = CapabilityHealth {
            status: hardware_status,
            observed_at: hardware.observed_at.clone(),
            evidence_refs: vec!["prime.hardware-graph.v1".to_owned()],
        };
        let generation_health = CapabilityHealth {
            status: crate::generation::health_status(&generation),
            observed_at: observed_at.clone(),
            evidence_refs: generation.evidence_refs.clone(),
        };
        let interface_health = CapabilityHealth {
            status: HealthStatus::Healthy,
            observed_at: observed_at.clone(),
            evidence_refs: vec!["prime.capability.v1".to_owned()],
        };
        let systemd_run_present = systemd_run.is_file();
        let exec_limitations = if systemd_run_present {
            vec![
                "P1 mutation admission is Host-local UID 0 only".to_owned(),
                "Native runtime still requires physical Host proof".to_owned(),
                "Arguments, caller environment and unsupported policy semantics remain blocked"
                    .to_owned(),
            ]
        } else {
            vec![format!(
                "systemd-run frontend is unavailable at {}",
                systemd_run.display()
            )]
        };
        let exec_health = CapabilityHealth {
            status: if systemd_run_present {
                HealthStatus::Unknown
            } else {
                HealthStatus::Failed
            },
            observed_at: observed_at.clone(),
            evidence_refs: Vec::new(),
        };
        let artifact_store = state_dir.join("artifacts/sha256").display().to_string();

        let mut storage_limitations = storage.limitations.clone();
        storage_limitations.extend(storage.reserve.limitations.clone());
        storage_limitations.extend(storage.pressure.limitations.clone());
        storage_limitations.extend(storage.generation_accounting.limitations.clone());
        if let Some(root_id) = storage.root_mount_id {
            if let Some(root) = storage
                .mounts
                .iter()
                .find(|mount| mount.mount_id == root_id)
            {
                storage_limitations.extend(root.limitations.clone());
            }
        }
        storage_limitations.sort();
        storage_limitations.dedup();
        let storage_health = CapabilityHealth {
            status: if !root_storage_usable {
                HealthStatus::Failed
            } else if storage_limitations.is_empty() {
                HealthStatus::Healthy
            } else {
                HealthStatus::Degraded
            },
            observed_at: storage.observed_at.clone(),
            evidence_refs: vec![STORAGE_INVENTORY_SCHEMA.to_owned()],
        };
        let storage_availability = if !root_storage_usable {
            CapabilityAvailability::Unavailable
        } else if storage_limitations.is_empty() {
            CapabilityAvailability::Available
        } else {
            CapabilityAvailability::Degraded
        };

        let mut generation_capability_limitations = generation_limitations;
        generation_capability_limitations
            .push("P1.5 owns exhaustive update/rollback proof".to_owned());

        let capabilities = vec![
            CapabilityDescriptor {
                capability_id: "prime.host.identity".to_owned(),
                capability_version: "1.0.0".to_owned(),
                family: "host".to_owned(),
                provider: provider.clone(),
                availability: availability_for(&fingerprint_limitations),
                effects: Vec::new(),
                accepts: CapabilityAccepts::default(),
                permissions: vec!["prime.host.read".to_owned()],
                resources: json!({}),
                hardware_requirements: Vec::new(),
                limits: json!({}),
                health: host_health,
                limitations: fingerprint_limitations,
                placement: placement.clone(),
                expected_evidence: vec!["prime.host-identity.v1".to_owned()],
                rollback: CapabilityRollback {
                    supported: false,
                    mode: None,
                    limitations: vec![
                        "Host identity is not a generation rollback object".to_owned()
                    ],
                },
            },
            CapabilityDescriptor {
                capability_id: "prime.hardware.inventory".to_owned(),
                capability_version: "1.0.0".to_owned(),
                family: "hardware".to_owned(),
                provider: provider.clone(),
                availability: availability_for(&hardware.limitations),
                effects: Vec::new(),
                accepts: CapabilityAccepts::default(),
                permissions: vec!["prime.hardware.read".to_owned()],
                resources: json!({
                    "pci_devices": hardware.inventory.pci_devices.len(),
                    "usb_devices": hardware.inventory.usb_devices.len(),
                    "block_devices": hardware.inventory.block_devices.len(),
                    "network_interfaces": hardware.inventory.network_interfaces.len(),
                    "display_connectors": hardware.inventory.display_connectors.len(),
                }),
                hardware_requirements: Vec::new(),
                limits: json!({}),
                health: hardware_health,
                limitations: hardware.limitations.clone(),
                placement: placement.clone(),
                expected_evidence: vec!["prime.hardware-graph.v1".to_owned()],
                rollback: CapabilityRollback {
                    supported: false,
                    mode: None,
                    limitations: vec!["Hardware topology is observed, not rolled back".to_owned()],
                },
            },
            CapabilityDescriptor {
                capability_id: "prime.storage.inventory".to_owned(),
                capability_version: "1.0.0".to_owned(),
                family: "storage".to_owned(),
                provider: provider.clone(),
                availability: storage_availability,
                effects: Vec::new(),
                accepts: CapabilityAccepts::default(),
                permissions: vec![
                    "prime.storage.read".to_owned(),
                    "prime.storage.preflight".to_owned(),
                ],
                resources: json!({
                    "root_mount_id": storage.root_mount_id,
                    "local_physical_filesystems": storage.local_physical_totals.filesystem_count,
                    "local_physical_total_bytes": storage.local_physical_totals.total_bytes,
                    "local_physical_available_bytes": storage.local_physical_totals.available_bytes,
                    "rollback_recovery_reserve_configured": storage.reserve.policy_configured,
                }),
                hardware_requirements: Vec::new(),
                limits: json!({
                    "preflight_fresh_probe": true,
                    "preflight_requires_explicit_reserve": true,
                    "recursive_file_index": false,
                }),
                health: storage_health,
                limitations: storage_limitations,
                placement: placement.clone(),
                expected_evidence: vec![
                    STORAGE_INVENTORY_SCHEMA.to_owned(),
                    STORAGE_PRESSURE_SCHEMA.to_owned(),
                ],
                rollback: CapabilityRollback {
                    supported: false,
                    mode: None,
                    limitations: vec![
                        "Storage observations are current-state evidence, not generation rollback objects"
                            .to_owned(),
                    ],
                },
            },
            CapabilityDescriptor {
                capability_id: "prime.generation.current".to_owned(),
                capability_version: "1.0.0".to_owned(),
                family: "generation".to_owned(),
                provider: provider.clone(),
                availability: CapabilityAvailability::Available,
                effects: Vec::new(),
                accepts: CapabilityAccepts::default(),
                permissions: vec!["prime.generation.read".to_owned()],
                resources: json!({
                    "state": &generation.state,
                    "boot_attempts_remaining": generation.boot_attempts_remaining,
                }),
                hardware_requirements: Vec::new(),
                limits: json!({}),
                health: generation_health,
                limitations: generation_capability_limitations,
                placement: placement.clone(),
                expected_evidence: vec!["prime.generation.v1".to_owned()],
                rollback: CapabilityRollback {
                    supported: true,
                    mode: Some("previous_known_good".to_owned()),
                    limitations: vec!["Activation remains Prime policy-controlled".to_owned()],
                },
            },
            CapabilityDescriptor {
                capability_id: "prime.exec.native".to_owned(),
                capability_version: "1.0.0".to_owned(),
                family: "execution".to_owned(),
                provider: provider.clone(),
                availability: if systemd_run_present {
                    CapabilityAvailability::Available
                } else {
                    CapabilityAvailability::Unavailable
                },
                effects: vec!["process".to_owned()],
                accepts: CapabilityAccepts {
                    formats: vec!["ELF".to_owned()],
                    runtime_families: vec!["NATIVE_LINUX".to_owned()],
                    workload_arches: vec![host.host_arch.clone()],
                },
                permissions: vec!["prime.exec.native.launch".to_owned()],
                resources: json!({
                    "artifact_store": artifact_store,
                    "supervisor": "systemd transient service",
                }),
                hardware_requirements: Vec::new(),
                limits: json!({
                    "caller_uid": 0,
                    "arguments": false,
                    "caller_environment": false,
                    "interactive_terminal": false,
                }),
                health: exec_health,
                limitations: exec_limitations,
                placement: placement.clone(),
                expected_evidence: vec![NATIVE_LAUNCH_EVIDENCE_SCHEMA.to_owned()],
                rollback: CapabilityRollback {
                    supported: false,
                    mode: None,
                    limitations: vec![
                        "A completed process launch is not rolled back as a generation".to_owned(),
                    ],
                },
            },
            crate::system_status::capability_descriptor(
                &system_root,
                &hardware,
                provider.clone(),
                placement.clone(),
                observed_at.clone(),
            ),
            CapabilityDescriptor {
                capability_id: "prime.capability.interface".to_owned(),
                capability_version: "1.0.0".to_owned(),
                family: "system".to_owned(),
                provider,
                availability: CapabilityAvailability::Available,
                effects: Vec::new(),
                accepts: CapabilityAccepts::default(),
                permissions: vec!["prime.capability.read".to_owned()],
                resources: json!({"transport":"AF_UNIX","protocol":"HTTP/1.1+JSON"}),
                hardware_requirements: Vec::new(),
                limits: json!({"remote_tcp":false}),
                health: interface_health,
                limitations: vec!["Host-local transport only in P1".to_owned()],
                placement,
                expected_evidence: vec!["prime.capability.v1".to_owned()],
                rollback: CapabilityRollback {
                    supported: false,
                    mode: None,
                    limitations: Vec::new(),
                },
            },
        ];

        Self {
            host,
            generation,
            hardware: Arc::new(hardware),
            storage: Arc::new(RwLock::new(storage)),
            capabilities: Arc::new(capabilities),
            health_limitations: Arc::new(health_limitations),
            state_dir: Arc::new(state_dir),
            systemd_run: Arc::new(systemd_run),
            storage_mountinfo: Arc::new(storage_mountinfo),
            storage_policy_file: Arc::new(storage_policy_file),
            system_root: Arc::new(system_root),
            started_at: observed_at,
        }
    }

    pub fn capabilities_snapshot(&self) -> Vec<CapabilityDescriptor> {
        let observed_at =
            crate::identity::now_rfc3339().unwrap_or_else(|_| self.started_at.clone());
        let provider = CapabilityProvider {
            id: "prime".to_owned(),
            generation_id: self.generation.generation_id.clone(),
        };
        let placement = CapabilityPlacement {
            scope: "HOST_LOCAL".to_owned(),
            host_id: self.host.host_id,
        };
        let refreshed = crate::system_status::capability_descriptor(
            &self.system_root,
            &self.hardware,
            provider,
            placement,
            observed_at,
        );
        let mut capabilities = (*self.capabilities).clone();
        if let Some(existing) = capabilities
            .iter_mut()
            .find(|item| item.capability_id == crate::system_status::SYSTEM_STATUS_CAPABILITY)
        {
            *existing = refreshed;
        } else {
            capabilities.push(refreshed);
        }
        capabilities
    }

    pub fn begin_generation_health_proving(
        &mut self,
        evidence_ref: &str,
    ) -> Result<(), crate::generation::GenerationError> {
        let updated = crate::generation::begin_health_proving(
            &self.state_dir,
            &self.generation,
            evidence_ref,
        )?;
        self.apply_generation_record(updated);
        Ok(())
    }

    fn apply_generation_record(&mut self, generation: GenerationRecord) {
        let generation_limitations = crate::generation::health_limitations(&generation);
        let health_status = crate::generation::health_status(&generation);

        self.generation = generation.clone();

        let capabilities = Arc::make_mut(&mut self.capabilities);
        if let Some(capability) = capabilities
            .iter_mut()
            .find(|item| item.capability_id == "prime.generation.current")
        {
            capability.provider.generation_id = generation.generation_id.clone();
            capability.resources = json!({
                "state": &generation.state,
                "boot_attempts_remaining": generation.boot_attempts_remaining,
            });
            capability.health.status = health_status;
            capability.health.evidence_refs = generation.evidence_refs.clone();
            capability
                .limitations
                .retain(|item| !item.starts_with("Current generation is "));
            capability
                .limitations
                .extend(generation_limitations.iter().cloned());
            capability.limitations.sort();
            capability.limitations.dedup();
        }

        let limitations = Arc::make_mut(&mut self.health_limitations);
        limitations.retain(|item| !item.starts_with("Current generation is "));
        limitations.extend(generation_limitations);
        limitations.sort();
        limitations.dedup();
    }
}

fn status_for(limitations: &[String]) -> HealthStatus {
    if limitations.is_empty() {
        HealthStatus::Healthy
    } else {
        HealthStatus::Degraded
    }
}

fn availability_for(limitations: &[String]) -> CapabilityAvailability {
    if limitations.is_empty() {
        CapabilityAvailability::Available
    } else {
        CapabilityAvailability::Degraded
    }
}
