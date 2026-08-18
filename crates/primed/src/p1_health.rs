use crate::hardware;
use prime_contracts::{
    FingerprintConfidence, GenerationHealthReport, GenerationRecord, GenerationState, HardwareGraph,
    HostIdentity, GENERATION_HEALTH_SCHEMA, HOST_IDENTITY_SCHEMA,
};

pub fn build_report(
    generation: &GenerationRecord,
    host: &HostIdentity,
    hardware_graph: &HardwareGraph,
    core_interface_ready: bool,
    shell_ready: bool,
    recovery_ready: bool,
    observed_at: String,
) -> GenerationHealthReport {
    let host_limitations = host_identity_limitations(host, hardware_graph);
    let hardware_limitations = hardware::p1_baseline_limitations(hardware_graph);
    let host_identity_ready = host_limitations.is_empty();
    let hardware_baseline_ready = hardware_limitations.is_empty();

    let mut limitations = Vec::new();
    if generation.state != GenerationState::HealthProving {
        limitations.push(format!(
            "Current generation state is {:?}, expected HEALTH_PROVING",
            generation.state
        ));
    }
    if !core_interface_ready {
        limitations.push("Prime Core interface readiness is not proven".to_owned());
    }
    limitations.extend(host_limitations);
    limitations.extend(hardware_limitations);
    if !shell_ready {
        limitations.push("Prime Shell readiness is not proven".to_owned());
    }
    if !recovery_ready {
        limitations.push("Prime recovery readiness is not proven".to_owned());
    }
    if observed_at.trim().is_empty() {
        limitations.push("Health observation timestamp is empty".to_owned());
    }
    limitations.sort();
    limitations.dedup();

    GenerationHealthReport {
        schema: GENERATION_HEALTH_SCHEMA.to_owned(),
        generation_id: generation.generation_id.clone(),
        image_digest: generation.image_digest.clone(),
        observed_at,
        core_interface_ready,
        host_identity_ready,
        hardware_baseline_ready,
        shell_ready,
        recovery_ready,
        limitations,
    }
}

fn host_identity_limitations(host: &HostIdentity, hardware_graph: &HardwareGraph) -> Vec<String> {
    let mut limitations = Vec::new();
    if host.schema != HOST_IDENTITY_SCHEMA {
        limitations.push(format!(
            "Prime Host identity schema is {}, expected {HOST_IDENTITY_SCHEMA}",
            host.schema
        ));
    }
    if host.host_arch != hardware_graph.inventory.host_arch {
        limitations.push(format!(
            "Prime Host architecture {} differs from Hardware Graph architecture {}",
            host.host_arch, hardware_graph.inventory.host_arch
        ));
    }
    if host.hardware_fingerprint.algorithm != "sha256" {
        limitations.push(format!(
            "Prime Host fingerprint algorithm is {}, expected sha256",
            host.hardware_fingerprint.algorithm
        ));
    }
    if !matches!(
        &host.hardware_fingerprint.confidence,
        FingerprintConfidence::High | FingerprintConfidence::Medium
    ) {
        limitations.push(format!(
            "Prime Host fingerprint confidence is {:?}, expected HIGH or MEDIUM",
            host.hardware_fingerprint.confidence
        ));
    }
    match host.hardware_fingerprint.digest.as_deref() {
        Some(digest) if canonical_sha256(digest) => {}
        Some(_) => limitations.push("Prime Host fingerprint digest is not canonical sha256".to_owned()),
        None => limitations.push("Prime Host fingerprint digest is not enrolled".to_owned()),
    }
    if host.hardware_fingerprint.observed_at.is_none() {
        limitations.push("Prime Host fingerprint has no enrollment observation time".to_owned());
    }
    limitations.sort();
    limitations.dedup();
    limitations
}

fn canonical_sha256(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else {
        return false;
    };
    hex.len() == 64
        && hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::{
        FingerprintConfidence, GenerationState, HardwareFingerprint, HardwareInventory,
        ReleaseChannel, GENERATION_SCHEMA, HARDWARE_GRAPH_SCHEMA,
    };
    use uuid::Uuid;

    fn digest(fill: char) -> String {
        format!("sha256:{}", fill.to_string().repeat(64))
    }

    fn generation(state: GenerationState) -> GenerationRecord {
        GenerationRecord {
            schema: GENERATION_SCHEMA.to_owned(),
            generation_id: "p1-health-test".to_owned(),
            image_digest: digest('a'),
            channel: ReleaseChannel::Lab,
            created_at: "2026-08-18T21:00:00Z".to_owned(),
            source_revision: "abcdef".to_owned(),
            state,
            boot_attempts_remaining: Some(3),
            evidence_refs: vec!["prime.core.socket.bound.v1".to_owned()],
        }
    }

    fn host(confidence: FingerprintConfidence) -> HostIdentity {
        HostIdentity {
            schema: HOST_IDENTITY_SCHEMA.to_owned(),
            host_id: Uuid::now_v7(),
            lineage_id: Uuid::now_v7(),
            created_at: "2026-08-18T21:00:00Z".to_owned(),
            host_arch: "x86_64".to_owned(),
            hardware_fingerprint: HardwareFingerprint {
                algorithm: "sha256".to_owned(),
                digest: Some(digest('b')),
                confidence,
                observed_at: Some("2026-08-18T21:00:00Z".to_owned()),
            },
            rebind_revision: 0,
            supersedes_host_id: None,
        }
    }

    fn incomplete_hardware() -> HardwareGraph {
        HardwareGraph {
            schema: HARDWARE_GRAPH_SCHEMA.to_owned(),
            revision: 1,
            topology_digest: digest('c'),
            observed_at: "2026-08-18T21:00:00Z".to_owned(),
            inventory: HardwareInventory {
                host_arch: "x86_64".to_owned(),
                ..HardwareInventory::default()
            },
            limitations: Vec::new(),
        }
    }

    #[test]
    fn report_binds_exact_generation_and_keeps_unearned_gates_false() {
        let generation = generation(GenerationState::HealthProving);
        let report = build_report(
            &generation,
            &host(FingerprintConfidence::High),
            &incomplete_hardware(),
            true,
            false,
            false,
            "2026-08-18T21:01:00Z".to_owned(),
        );
        assert_eq!(report.generation_id, generation.generation_id);
        assert_eq!(report.image_digest, generation.image_digest);
        assert!(report.core_interface_ready);
        assert!(report.host_identity_ready);
        assert!(!report.hardware_baseline_ready);
        assert!(!report.shell_ready);
        assert!(!report.recovery_ready);
        assert!(!report.all_required_ready());
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("Prime Shell readiness is not proven")));
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("Prime recovery readiness is not proven")));
    }

    #[test]
    fn low_confidence_host_identity_cannot_become_health_ready() {
        let report = build_report(
            &generation(GenerationState::HealthProving),
            &host(FingerprintConfidence::Low),
            &incomplete_hardware(),
            true,
            false,
            false,
            "2026-08-18T21:01:00Z".to_owned(),
        );
        assert!(!report.host_identity_ready);
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("expected HIGH or MEDIUM")));
    }

    #[test]
    fn report_refuses_to_describe_boot_try_as_health_proving() {
        let report = build_report(
            &generation(GenerationState::BootTry),
            &host(FingerprintConfidence::High),
            &incomplete_hardware(),
            false,
            false,
            false,
            "2026-08-18T21:01:00Z".to_owned(),
        );
        assert!(report
            .limitations
            .iter()
            .any(|item| item.contains("expected HEALTH_PROVING")));
    }
}
