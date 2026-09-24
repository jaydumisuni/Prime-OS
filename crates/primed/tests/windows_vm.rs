use prime_contracts::{
    ApplicationArtifact, ApplicationProfile, ArtifactFormat, CompatibilityRecord, ExecutionBackend,
    MechanicalCompatibilityState, PolicyReference, RuntimeFamily, APPLICATION_PROFILE_SCHEMA,
};
use primed::windows_vm::{
    build_qemu_plan, validate_guest_definition, validate_vm_profile, VmGuestDefinition,
    VmPlanRequest,
};
use sha2::{Digest, Sha256};
use std::fs;
use tempfile::tempdir;
use uuid::Uuid;

fn digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn definition(image: &std::path::Path, bytes: &[u8]) -> VmGuestDefinition {
    VmGuestDefinition {
        schema: "prime.windows-vm-guest.v1".to_owned(),
        guest_id: "windows-fallback-test".to_owned(),
        revision: 1,
        base_image: image.display().to_string(),
        base_sha256: digest(bytes),
        memory_mib: 4096,
        vcpus: 2,
    }
}

#[test]
fn guest_definition_requires_exact_digest_bound_regular_image() {
    let dir = tempdir().unwrap();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-deterministic-guest";
    fs::write(&image, bytes).unwrap();

    let validated = validate_guest_definition(&definition(&image, bytes)).unwrap();
    assert_eq!(validated.base_image, image);

    fs::write(&image, b"tampered").unwrap();
    let error = validate_guest_definition(&definition(&image, bytes)).unwrap_err();
    assert!(error.to_string().contains("digest"));
}

#[test]
fn qemu_plan_is_kvm_only_network_closed_and_agent_scoped() {
    let dir = tempdir().unwrap();
    let overlay = dir.path().join("session.qcow2");
    fs::write(&overlay, b"overlay").unwrap();
    let runtime = dir.path().join("runtime");
    fs::create_dir(&runtime).unwrap();

    let plan = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime.clone(),
        memory_mib: 4096,
        vcpus: 2,
        usb_nodes: vec![],
    })
    .unwrap();

    assert_eq!(
        plan.program,
        std::path::PathBuf::from("/usr/bin/qemu-system-x86_64")
    );
    let joined = plan.args.join(" ");
    assert!(joined.contains("-accel kvm"));
    assert!(joined.contains("-net none"));
    assert!(joined.contains("-nodefaults"));
    assert!(joined.contains("-no-user-config"));
    assert!(joined.contains("name=org.thetechguy.prime.agent"));
    assert!(joined.contains(overlay.to_str().unwrap()));
    assert!(!joined.contains("-nic user"));
    assert!(!joined.contains("hostfwd"));
}

#[test]
fn qemu_plan_accepts_only_strict_w7_usb_nodes() {
    let dir = tempdir().unwrap();
    let overlay = dir.path().join("session.qcow2");
    fs::write(&overlay, b"overlay").unwrap();
    let runtime = dir.path().join("runtime");
    fs::create_dir(&runtime).unwrap();

    let allowed = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay.clone(),
        runtime_dir: runtime.clone(),
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec!["/dev/bus/usb/001/018".into()],
    })
    .unwrap();
    assert!(allowed
        .args
        .join(" ")
        .contains("usb-host,hostbus=1,hostaddr=18"));

    let denied = build_qemu_plan(&VmPlanRequest {
        qemu_binary: "/usr/bin/qemu-system-x86_64".into(),
        overlay_image: overlay,
        runtime_dir: runtime,
        memory_mib: 2048,
        vcpus: 2,
        usb_nodes: vec!["/dev/sda".into()],
    })
    .unwrap_err();
    assert!(denied.to_string().contains("USB"));
}

fn vm_profile(backend: ExecutionBackend) -> ApplicationProfile {
    ApplicationProfile {
        schema: APPLICATION_PROFILE_SCHEMA.to_owned(),
        application_id: Uuid::nil(),
        profile_revision: 1,
        profile_digest: "sha256:test".to_owned(),
        display_name: "W8 VM fixture".to_owned(),
        artifact: ApplicationArtifact {
            identity: "sha256:fixture".to_owned(),
            format: ArtifactFormat::Pe32Plus,
            runtime_family: RuntimeFamily::Windows,
            workload_arch: Some("x86_64".to_owned()),
        },
        execution_backend: backend,
        dependencies: vec![],
        workload_policy: PolicyReference {
            policy_id: Uuid::nil(),
            policy_revision: 1,
            policy_digest: "sha256:policy".to_owned(),
        },
        permissions: vec![],
        compatibility: CompatibilityRecord {
            state: MechanicalCompatibilityState::Recognized,
            evidence_refs: vec![],
        },
        revoked: false,
        revocation_reason: None,
        created_at: "2026-09-13T00:00:00Z".to_owned(),
    }
}

#[test]
fn vm_fallback_is_explicit_backend_authority_not_personality_retry() {
    validate_vm_profile(&vm_profile(ExecutionBackend::Vm)).unwrap();
    let error = validate_vm_profile(&vm_profile(ExecutionBackend::Personality)).unwrap_err();
    assert!(error.to_string().contains("VM"));
}

#[test]
fn guest_agent_handshake_is_versioned_arch_bound_and_digest_bound() {
    use primed::windows_vm::{
        validate_guest_agent_hello, GuestAgentHello, WINDOWS_VM_AGENT_SCHEMA,
    };

    let hello = GuestAgentHello {
        schema: WINDOWS_VM_AGENT_SCHEMA.to_owned(),
        protocol_revision: 1,
        guest_id: "windows-fallback-test".to_owned(),
        guest_revision: 7,
        guest_sha256: "a".repeat(64),
        workload_arch: "x86_64".to_owned(),
    };
    validate_guest_agent_hello(
        &hello,
        "windows-fallback-test",
        7,
        &"a".repeat(64),
        "x86_64",
    )
    .unwrap();

    let wrong_arch = GuestAgentHello {
        workload_arch: "x86".to_owned(),
        ..hello.clone()
    };
    assert!(validate_guest_agent_hello(
        &wrong_arch,
        "windows-fallback-test",
        7,
        &"a".repeat(64),
        "x86_64"
    )
    .is_err());

    let wrong_digest = GuestAgentHello {
        guest_sha256: "b".repeat(64),
        ..hello
    };
    assert!(validate_guest_agent_hello(
        &wrong_digest,
        "windows-fallback-test",
        7,
        &"a".repeat(64),
        "x86_64"
    )
    .is_err());
}

#[test]
fn vm_session_paths_are_application_scoped_and_reject_escape() {
    use primed::windows_vm::vm_session_paths;

    let root = tempdir().unwrap();
    let a = vm_session_paths(root.path(), "app-001", "session-a").unwrap();
    let b = vm_session_paths(root.path(), "app-002", "session-b").unwrap();
    assert_ne!(a.runtime_dir, b.runtime_dir);
    assert_ne!(a.overlay_image, b.overlay_image);
    assert!(a.agent_socket.starts_with(&a.runtime_dir));
    assert!(b.agent_socket.starts_with(&b.runtime_dir));

    assert!(vm_session_paths(root.path(), "../escape", "session-a").is_err());
    assert!(vm_session_paths(root.path(), "app-001", "/absolute").is_err());
}

#[test]
fn runtime_proof_plan_is_exact_guest_bound_and_requires_all_w8_cases() {
    use primed::windows_vm::{
        create_runtime_proof_observation, evaluate_runtime_proof, prepare_runtime_proof_plan,
        VmRuntimeAdversarialCase, VmRuntimeProofState,
    };

    let dir = tempdir().unwrap();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    fs::write(&image, bytes).unwrap();
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();

    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    assert_eq!(plan.proof_id.len(), 64);
    assert_eq!(plan.required_cases.len(), 4);
    assert!(plan
        .required_cases
        .contains(&VmRuntimeAdversarialCase::ConcurrentSessionIsolation));
    assert!(plan
        .required_cases
        .contains(&VmRuntimeAdversarialCase::ForcedProcessTermination));
    assert!(plan
        .required_cases
        .contains(&VmRuntimeAdversarialCase::CorruptGuestAuthority));
    assert!(plan
        .required_cases
        .contains(&VmRuntimeAdversarialCase::DeniedUsbNode));
    assert_eq!(
        evaluate_runtime_proof(&plan, &[]),
        VmRuntimeProofState::Pending
    );

    let observations: Vec<_> = plan
        .required_cases
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, case)| {
            create_runtime_proof_observation(
                &plan,
                case,
                true,
                true,
                &format!("{:064x}", index + 1),
            )
            .unwrap()
        })
        .collect();

    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pass
    );
}

#[test]
fn runtime_proof_cannot_pass_on_wrong_binding_duplicate_case_or_residual_state() {
    use primed::windows_vm::{
        create_runtime_proof_observation, evaluate_runtime_proof, prepare_runtime_proof_plan,
        VmRuntimeProofState,
    };

    let dir = tempdir().unwrap();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    fs::write(&image, bytes).unwrap();
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    let make = || {
        plan.required_cases
            .iter()
            .cloned()
            .enumerate()
            .map(|(index, case)| {
                create_runtime_proof_observation(
                    &plan,
                    case,
                    true,
                    true,
                    &format!("{:064x}", index + 1),
                )
                .unwrap()
            })
            .collect::<Vec<_>>()
    };

    let mut observations = make();
    observations[0].proof_id = "b".repeat(64);
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );

    let mut observations = make();
    observations[0].session_id = "session-b".to_owned();
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );

    let mut observations = make();
    observations[1].zero_residual_state = false;
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );

    let mut observations = make();
    observations[3].case = observations[2].case.clone();
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );

    let mut observations = make();
    observations[1] = create_runtime_proof_observation(
        &plan,
        observations[1].case.clone(),
        true,
        true,
        &observations[0].evidence_sha256,
    )
    .unwrap();
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );
}

#[test]
fn runtime_observation_identity_rejects_mutation_and_noncanonical_evidence_digest() {
    use primed::windows_vm::{
        create_runtime_proof_observation, evaluate_runtime_proof, prepare_runtime_proof_plan,
        VmRuntimeProofState,
    };

    let dir = tempdir().unwrap();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    fs::write(&image, bytes).unwrap();
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    assert!(create_runtime_proof_observation(
        &plan,
        plan.required_cases[0].clone(),
        true,
        true,
        "NOT-A-DIGEST",
    )
    .is_err());

    let mut observations: Vec<_> = plan
        .required_cases
        .iter()
        .cloned()
        .enumerate()
        .map(|(index, case)| {
            create_runtime_proof_observation(
                &plan,
                case,
                true,
                true,
                &format!("{:064x}", index + 1),
            )
            .unwrap()
        })
        .collect();

    observations[2].passed = false;
    assert_eq!(
        evaluate_runtime_proof(&plan, &observations),
        VmRuntimeProofState::Pending
    );
}

#[test]
fn runtime_proof_plan_identity_changes_with_guest_application_or_session_scope() {
    use primed::windows_vm::prepare_runtime_proof_plan;

    let dir = tempdir().unwrap();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    fs::write(&image, bytes).unwrap();
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();

    let a = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();
    let b = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-b").unwrap();
    let c = prepare_runtime_proof_plan(&guest, &root, "app-002", "session-a").unwrap();
    assert_ne!(a.proof_id, b.proof_id);
    assert_ne!(a.proof_id, c.proof_id);

    let mut other = guest.clone();
    other.base_sha256 = "b".repeat(64);
    let d = prepare_runtime_proof_plan(&other, &root, "app-001", "session-a").unwrap();
    assert_ne!(a.proof_id, d.proof_id);
}

#[test]
fn runtime_observation_hashes_actual_nonempty_evidence_bytes() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_evidence, prepare_runtime_proof_plan,
    };

    let dir = tempdir().unwrap();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    fs::write(&image, bytes).unwrap();
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();
    let evidence = b"forced-termination:zero-qemu-processes";

    let observation = create_runtime_proof_observation_from_evidence(
        &plan,
        plan.required_cases[1].clone(),
        true,
        true,
        evidence,
    )
    .unwrap();
    assert_eq!(observation.evidence_sha256, digest(evidence));
    assert!(create_runtime_proof_observation_from_evidence(
        &plan,
        plan.required_cases[1].clone(),
        true,
        true,
        b"",
    )
    .is_err());
}

#[test]
fn runtime_evidence_envelope_rejects_cross_case_and_cross_session_replay() {
    use primed::windows_vm::{
        create_runtime_proof_observation_from_envelope, prepare_runtime_proof_plan,
        VmRuntimeEvidenceEnvelope, WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA,
    };

    let dir = tempdir().unwrap();
    let image = dir.path().join("windows.qcow2");
    let bytes = b"prime-w8-runtime-proof-guest";
    fs::write(&image, bytes).unwrap();
    let guest = validate_guest_definition(&definition(&image, bytes)).unwrap();
    let root = dir.path().join("runtime");
    fs::create_dir(&root).unwrap();
    let plan = prepare_runtime_proof_plan(&guest, &root, "app-001", "session-a").unwrap();

    let envelope = VmRuntimeEvidenceEnvelope {
        schema: WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA.to_owned(),
        proof_id: plan.proof_id.clone(),
        case: plan.required_cases[0].clone(),
        guest_id: plan.guest_id.clone(),
        guest_revision: plan.guest_revision,
        guest_sha256: plan.guest_sha256.clone(),
        application_id: plan.application_id.clone(),
        session_id: plan.session_id.clone(),
        evidence: b"concurrent-session-isolation:clean".to_vec(),
    };
    create_runtime_proof_observation_from_envelope(&plan, &envelope, true, true).unwrap();

    let mut wrong_session = envelope.clone();
    wrong_session.session_id = "session-b".to_owned();
    assert!(
        create_runtime_proof_observation_from_envelope(&plan, &wrong_session, true, true).is_err()
    );

    let mut wrong_case = envelope;
    wrong_case.case = plan.required_cases[1].clone();
    wrong_case.proof_id = "b".repeat(64);
    assert!(
        create_runtime_proof_observation_from_envelope(&plan, &wrong_case, true, true).is_err()
    );
}
