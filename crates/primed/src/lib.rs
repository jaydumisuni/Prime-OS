pub mod generation;
pub mod identity;
pub mod server;

use prime_contracts::{
    CapabilityAccepts, CapabilityAvailability, CapabilityDescriptor, CapabilityHealth,
    CapabilityPlacement, CapabilityProvider, CapabilityRollback, GenerationRecord, HealthStatus,
    HostIdentity,
};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct CoreState {
    pub host: HostIdentity,
    pub generation: GenerationRecord,
    pub capabilities: Arc<Vec<CapabilityDescriptor>>,
    pub started_at: String,
}

impl CoreState {
    pub fn new(host: HostIdentity, generation: GenerationRecord, observed_at: String) -> Self {
        let provider = CapabilityProvider {
            id: "prime".to_owned(),
            generation_id: generation.generation_id.clone(),
        };
        let placement = CapabilityPlacement {
            scope: "HOST_LOCAL".to_owned(),
            host_id: host.host_id,
        };
        let health = CapabilityHealth {
            status: HealthStatus::Healthy,
            observed_at: observed_at.clone(),
            evidence_refs: Vec::new(),
        };

        let capabilities = vec![
            CapabilityDescriptor {
                capability_id: "prime.host.identity".to_owned(),
                capability_version: "1.0.0".to_owned(),
                family: "host".to_owned(),
                provider: provider.clone(),
                availability: CapabilityAvailability::Available,
                effects: Vec::new(),
                accepts: CapabilityAccepts::default(),
                permissions: vec!["prime.host.read".to_owned()],
                resources: json!({}),
                hardware_requirements: Vec::new(),
                limits: json!({}),
                health: health.clone(),
                limitations: vec!["Hardware fingerprint remains UNPROBED until the hardware graph slice lands".to_owned()],
                placement: placement.clone(),
                expected_evidence: vec!["prime.host-identity.v1".to_owned()],
                rollback: CapabilityRollback {
                    supported: false,
                    mode: None,
                    limitations: vec!["Host identity is not a generation rollback object".to_owned()],
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
                health: health.clone(),
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
                health,
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
            capabilities: Arc::new(capabilities),
            started_at: observed_at,
        }
    }
}
