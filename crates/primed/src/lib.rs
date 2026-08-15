pub mod exec;
pub mod generation;
pub mod hardware;
pub mod identity;
pub mod policy;
pub mod registry;
pub mod server;

use prime_contracts::{
    CapabilityAccepts, CapabilityAvailability, CapabilityDescriptor, CapabilityHealth,
    CapabilityPlacement, CapabilityProvider, CapabilityRollback, FingerprintConfidence,
    GenerationRecord, HardwareGraph, HealthStatus, HostIdentity,
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct CoreState {
    pub host: HostIdentity,
    pub generation: GenerationRecord,
    pub hardware: Arc<HardwareGraph>,
    pub capabilities: Arc<Vec<CapabilityDescriptor>>,
    pub health_limitations: Arc<Vec<String>>,
    pub started_at: String,
}

impl CoreState {
    pub fn new(
        host: HostIdentity,
        generation: GenerationRecord,
        hardware: HardwareGraph,
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
        let mut health_limitations = hardware.limitations.clone();
        health_limitations.extend(fingerprint_limitations.clone());
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
            status: HealthStatus::Healthy,
            observed_at: observed_at.clone(),
            evidence_refs: vec!["prime.generation.v1".to_owned()],
        };
        let interface_health = CapabilityHealth {
            status: HealthStatus::Healthy,
            observed_at: observed_at.clone(),
            evidence_refs: vec!["prime.capability.v1".to_owned()],
        };

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
                capability_id: "prime.generation.current".to_owned(),
                capability_version: "1.0.0".to_owned(),
                family: "generation".to_owned(),
                provider: provider.clone(),
                availability: CapabilityAvailability::Available,
                effects: Vec::new(),
                accepts: CapabilityAccepts::default(),
                permissions: vec!["prime.generation.read".to_owned()],
                resources: json!({}),
                hardware_requirements: Vec::new(),
                limits: json!({}),
                health: generation_health,
                limitations: vec!["P1.5 owns exhaustive update/rollback proof".to_owned()],
                placement: placement.clone(),
                expected_evidence: vec!["prime.generation.v1".to_owned()],
                rollback: CapabilityRollback {
                    supported: true,
                    mode: Some("previous_known_good".to_owned()),
                    limitations: vec!["Activation remains Prime policy-controlled".to_owned()],
                },
            },
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
            capabilities: Arc::new(capabilities),
            health_limitations: Arc::new(health_limitations),
            started_at: observed_at,
        }
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
