use prime_contracts::{ArtifactFormat, WindowsProviderManifest, WINDOWS_PROVIDER_MANIFEST_SCHEMA};
use primed::windows_personality::{
    launch_windows_with_components, load_provider, prepare_windows_dependencies,
    windows_component_systemd_run_args, WindowsLaunchRuntime, WindowsPersonalityError,
};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn write_adapter(path: &Path, mode: u32) {
    fs::write(path, b"#!/bin/false\n").expect("write adapter");
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).expect("chmod adapter");
}

fn write_manifest(
    dir: &Path,
    file_name: &str,
    provider_id: &str,
    revision: u64,
    adapter_path: &Path,
    arches: &[&str],
) {
    let manifest = WindowsProviderManifest {
        schema: WINDOWS_PROVIDER_MANIFEST_SCHEMA.to_owned(),
        provider_id: provider_id.to_owned(),
        provider_revision: revision,
        adapter_path: adapter_path.display().to_string(),
        formats: vec![ArtifactFormat::Pe32, ArtifactFormat::Pe32Plus],
        workload_arches: arches.iter().map(|value| (*value).to_owned()).collect(),
        limitations: vec!["W1_PORTABLE_ONLY".to_owned()],
    };
    fs::write(
        dir.join(file_name),
        serde_json::to_vec_pretty(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");
}

#[test]
fn missing_provider_directory_is_explicitly_unavailable() {
    let dir = tempfile::tempdir().expect("tempdir");
    let missing = dir.path().join("missing");
    assert!(matches!(
        load_provider(&missing, &ArtifactFormat::Pe32Plus, "x86_64"),
        Err(WindowsPersonalityError::ProviderUnavailable(_))
    ));
}

#[test]
fn relative_adapter_path_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let manifest = WindowsProviderManifest {
        schema: WINDOWS_PROVIDER_MANIFEST_SCHEMA.to_owned(),
        provider_id: "prime.windows.relative".to_owned(),
        provider_revision: 1,
        adapter_path: "relative/provider".to_owned(),
        formats: vec![ArtifactFormat::Pe32Plus],
        workload_arches: vec!["x86_64".to_owned()],
        limitations: vec![],
    };
    fs::write(
        dir.path().join("relative.json"),
        serde_json::to_vec(&manifest).expect("serialize manifest"),
    )
    .expect("write manifest");

    assert!(matches!(
        load_provider(dir.path(), &ArtifactFormat::Pe32Plus, "x86_64"),
        Err(WindowsPersonalityError::InvalidProvider(_))
    ));
}

#[test]
fn non_executable_adapter_is_rejected() {
    let dir = tempfile::tempdir().expect("tempdir");
    let adapter = dir.path().join("adapter");
    write_adapter(&adapter, 0o644);
    write_manifest(
        dir.path(),
        "provider.json",
        "prime.windows.nonexec",
        1,
        &adapter,
        &["x86_64"],
    );

    assert!(matches!(
        load_provider(dir.path(), &ArtifactFormat::Pe32Plus, "x86_64"),
        Err(WindowsPersonalityError::InvalidProvider(_))
    ));
}

#[test]
fn unsupported_architecture_has_no_compatible_provider() {
    let dir = tempfile::tempdir().expect("tempdir");
    let adapter = dir.path().join("adapter");
    write_adapter(&adapter, 0o755);
    write_manifest(
        dir.path(),
        "provider.json",
        "prime.windows.x64-only",
        1,
        &adapter,
        &["x86_64"],
    );

    assert!(matches!(
        load_provider(dir.path(), &ArtifactFormat::Pe32, "arm"),
        Err(WindowsPersonalityError::NoCompatibleProvider { .. })
    ));
}

#[test]
fn selection_is_deterministic_and_prefers_latest_revision_within_provider() {
    let dir = tempfile::tempdir().expect("tempdir");
    let adapter_a1 = dir.path().join("adapter-a1");
    let adapter_a2 = dir.path().join("adapter-a2");
    let adapter_b = dir.path().join("adapter-b");
    for path in [&adapter_a1, &adapter_a2, &adapter_b] {
        write_adapter(path, 0o755);
    }
    write_manifest(
        dir.path(),
        "b.json",
        "prime.windows.b",
        9,
        &adapter_b,
        &["x86_64"],
    );
    write_manifest(
        dir.path(),
        "a1.json",
        "prime.windows.a",
        1,
        &adapter_a1,
        &["x86_64"],
    );
    write_manifest(
        dir.path(),
        "a2.json",
        "prime.windows.a",
        2,
        &adapter_a2,
        &["x86_64"],
    );

    let selected = load_provider(dir.path(), &ArtifactFormat::Pe32Plus, "x86_64")
        .expect("compatible provider");
    assert_eq!(selected.manifest.provider_id, "prime.windows.a");
    assert_eq!(selected.manifest.provider_revision, 2);
    assert_eq!(
        selected.adapter_path,
        fs::canonicalize(adapter_a2).expect("canonical adapter")
    );
}

use prime_contracts::{
    ApplicationArtifact, ApplicationProfile, BackgroundPolicy, CompatibilityRecord, CpuPolicy,
    DevicePolicy, EvidencePolicy, ExecutionBackend, FilesystemPolicy, GpuMode, GpuPolicy,
    MechanicalCompatibilityState, MemoryPolicy, NetworkMode, NetworkPolicy, PolicyClass,
    PolicyReference, ProcessPolicy, RuntimeFamily, SecretPolicy, StoragePolicy, WorkloadPolicy,
    APPLICATION_PROFILE_SCHEMA, WORKLOAD_POLICY_SCHEMA,
};
use primed::registry::{
    seal_policy, seal_profile, select_policy_revision, select_profile_revision,
    store_policy_revision, store_profile_revision,
};
use primed::windows_components::{seal_component, store_component_revision};
use primed::windows_personality::prepare_windows_launch;
use sha2::{Digest, Sha256};
use uuid::Uuid;

fn pe(machine: u16, optional_magic: u16) -> Vec<u8> {
    let mut bytes = vec![0_u8; 128];
    bytes[0..2].copy_from_slice(b"MZ");
    bytes[0x3c..0x40].copy_from_slice(&0x40_u32.to_le_bytes());
    bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
    bytes[0x44..0x46].copy_from_slice(&machine.to_le_bytes());
    bytes[0x58..0x5a].copy_from_slice(&optional_magic.to_le_bytes());
    bytes
}

fn pe64() -> Vec<u8> {
    pe(0x8664, 0x20b)
}

fn pe32() -> Vec<u8> {
    pe(0x014c, 0x10b)
}

fn labelled_sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn fixture_policy(root: &Path) -> WorkloadPolicy {
    let policy = seal_policy(WorkloadPolicy {
        schema: WORKLOAD_POLICY_SCHEMA.to_owned(),
        policy_id: Uuid::now_v7(),
        revision: 1,
        digest: String::new(),
        class: PolicyClass::ForeignRuntime,
        cpu: CpuPolicy {
            weight: 100,
            quota_percent: None,
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
            destinations: vec![],
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
    .expect("seal policy");
    store_policy_revision(root, &policy).expect("store policy");
    select_policy_revision(root, policy.policy_id, policy.revision).expect("select policy");
    policy
}

fn fixture_profile(
    root: &Path,
    policy: &WorkloadPolicy,
    identity: String,
    format: ArtifactFormat,
    runtime_family: RuntimeFamily,
    backend: ExecutionBackend,
    arch: Option<&str>,
) -> Uuid {
    let application_id = Uuid::now_v7();
    let profile = seal_profile(ApplicationProfile {
        schema: APPLICATION_PROFILE_SCHEMA.to_owned(),
        application_id,
        profile_revision: 1,
        profile_digest: String::new(),
        display_name: "Windows W1 Fixture".to_owned(),
        artifact: ApplicationArtifact {
            identity,
            format,
            runtime_family,
            workload_arch: arch.map(str::to_owned),
        },
        execution_backend: backend,
        dependencies: vec![],
        workload_policy: PolicyReference {
            policy_id: policy.policy_id,
            policy_revision: policy.revision,
            policy_digest: policy.digest.clone(),
        },
        permissions: vec![],
        compatibility: CompatibilityRecord {
            state: MechanicalCompatibilityState::Recognized,
            evidence_refs: vec![],
        },
        revoked: false,
        revocation_reason: None,
        created_at: "2026-09-10T00:00:00Z".to_owned(),
    })
    .expect("seal profile");
    store_profile_revision(root, &profile).expect("store profile");
    select_profile_revision(root, application_id, 1).expect("select profile");
    application_id
}

fn fixture_provider(dir: &Path, arches: &[&str]) {
    let adapter = dir.join("adapter");
    write_adapter(&adapter, 0o755);
    write_manifest(
        dir,
        "provider.json",
        "prime.windows.fixture",
        1,
        &adapter,
        arches,
    );
}

fn fixture_component(
    root: &Path,
    id: &str,
    arch: &str,
) -> prime_contracts::WindowsComponentManifest {
    use prime_contracts::{
        WindowsComponentKind, WindowsComponentManifest, WindowsInstallerKind,
        WindowsVerificationProbe, WINDOWS_COMPONENT_SCHEMA,
    };
    let sealed = seal_component(WindowsComponentManifest {
        schema: WINDOWS_COMPONENT_SCHEMA.to_owned(),
        component_id: id.to_owned(),
        revision: 1,
        digest: String::new(),
        display_name: id.to_owned(),
        kind: WindowsComponentKind::Runtime,
        workload_arches: vec![arch.to_owned()],
        depends_on: vec![],
        installer_kind: WindowsInstallerKind::Builtin,
        artifact_identity: None,
        artifact_path: None,
        installer_args: vec![],
        accepted_exit_codes: vec![],
        restart_compatibility_environment: false,
        verification: vec![WindowsVerificationProbe::FileExists {
            path: format!("drive_c/PrimeW2/{id}.txt"),
        }],
        limitations: vec![],
    })
    .expect("seal component");
    store_component_revision(root, &sealed).expect("store component");
    sealed
}

fn add_profile_dependencies(root: &Path, application_id: Uuid, dependencies: Vec<String>) {
    let mut profile =
        primed::registry::load_profile_revision(root, application_id, 1).expect("load profile");
    profile.profile_revision = 2;
    profile.profile_digest.clear();
    profile.dependencies = dependencies;
    let profile = seal_profile(profile).expect("reseal profile");
    store_profile_revision(root, &profile).expect("store dependent profile");
    select_profile_revision(root, application_id, 2).expect("select dependent profile");
}

fn component_reference(component: &prime_contracts::WindowsComponentManifest) -> String {
    format!(
        "windows-component:{}@{}#{}",
        component.component_id, component.revision, component.digest
    )
}

#[test]
fn dependency_bearing_windows_profile_prepares_exact_component_engine_transaction() {
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    let components = tempfile::tempdir().expect("components");
    fixture_provider(providers.path(), &["x86_64"]);
    let candidate = state.path().join("fixture.exe");
    let bytes = pe64();
    fs::write(&candidate, &bytes).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&bytes),
        ArtifactFormat::Pe32Plus,
        RuntimeFamily::Windows,
        ExecutionBackend::Personality,
        Some("x86_64"),
    );
    let component = fixture_component(components.path(), "runtime.fixture", "x86_64");
    add_profile_dependencies(
        state.path(),
        application_id,
        vec![component_reference(&component)],
    );

    let prepared = prepare_windows_launch(
        state.path(),
        providers.path(),
        application_id,
        &candidate,
        "x86_64",
    )
    .expect("dependency-bearing profile remains a valid Windows launch");
    let dependencies = prepare_windows_dependencies(components.path(), &prepared)
        .expect("resolve W2 dependency plan");
    assert_eq!(dependencies.components.len(), 1);
    assert_eq!(dependencies.components[0].manifest, component);

    let engine = Path::new("/usr/libexec/prime/prime-windows-component-engine");
    let args = windows_component_systemd_run_args(engine, &prepared, &dependencies.components[0]);
    assert!(args.iter().any(|arg| arg == "--pipe"));
    assert!(args.iter().any(|arg| arg == engine.to_str().unwrap()));
    assert!(args.windows(2).any(|pair| {
        pair[0] == "--manifest"
            && pair[1]
                == dependencies.components[0]
                    .manifest_path
                    .display()
                    .to_string()
    }));
    assert!(args
        .windows(2)
        .any(|pair| { pair[0] == "--application-id" && pair[1] == application_id.to_string() }));
    assert!(args
        .iter()
        .any(|arg| arg.starts_with("--property=StateDirectory=prime-win-app-")));
    assert!(!args.iter().any(|arg| arg.contains("prime-shell")));
    assert!(!args.iter().any(|arg| arg.contains("prime-display")));
}

#[test]
fn dependency_preparation_records_exact_outcome_before_windows_launch() {
    use prime_contracts::{PersonalityLaunchOutcome, WindowsComponentOutcome};
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    let components = tempfile::tempdir().expect("components");
    let tools = tempfile::tempdir().expect("tools");
    fixture_provider(providers.path(), &["x86_64"]);
    let candidate = state.path().join("fixture.exe");
    let bytes = pe64();
    fs::write(&candidate, &bytes).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&bytes),
        ArtifactFormat::Pe32Plus,
        RuntimeFamily::Windows,
        ExecutionBackend::Personality,
        Some("x86_64"),
    );
    let component = fixture_component(components.path(), "runtime.fixture", "x86_64");
    add_profile_dependencies(
        state.path(),
        application_id,
        vec![component_reference(&component)],
    );

    let engine = tools.path().join("prime-windows-component-engine");
    write_adapter(&engine, 0o755);
    let systemd = tools.path().join("systemd-run-fixture");
    fs::write(
        &systemd,
        format!(
            "#!/bin/sh\ncase \"$*\" in\n  *{}*) printf 'INSTALLED\\n' ;;\nesac\nexit 0\n",
            engine.display()
        ),
    )
    .expect("write systemd fixture");
    fs::set_permissions(&systemd, fs::Permissions::from_mode(0o755)).expect("chmod systemd");

    let host = fixture_host();
    let generation = fixture_generation();
    let runtime = WindowsLaunchRuntime {
        provider_dir: providers.path(),
        component_dir: components.path(),
        component_engine: &engine,
        systemd_run: &systemd,
    };
    let evidence = launch_windows_with_components(
        state.path(),
        &runtime,
        &host,
        &generation,
        application_id,
        &candidate,
    )
    .expect("dependency preparation then W1 launch");
    assert_eq!(evidence.outcome, PersonalityLaunchOutcome::ExitedSuccess);

    let evidence_root = state.path().join("evidence/windows-components");
    let transaction_dirs = fs::read_dir(&evidence_root)
        .expect("component evidence root")
        .collect::<Result<Vec<_>, _>>()
        .expect("component evidence entries");
    assert_eq!(transaction_dirs.len(), 1);
    let transaction = transaction_dirs[0].path();
    assert!(transaction.join("01-admitted.json").is_file());
    let completed: prime_contracts::WindowsComponentEvidence = serde_json::from_slice(
        &fs::read(transaction.join("02-completed.json")).expect("completed evidence"),
    )
    .expect("component evidence json");
    assert_eq!(completed.outcome, WindowsComponentOutcome::Installed);
    assert_eq!(completed.component_id, "runtime.fixture");
}

#[test]
fn prepare_admits_exact_x64_windows_personality_profile() {
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    fixture_provider(providers.path(), &["x86", "x86_64"]);
    let candidate = state.path().join("fixture.exe");
    let bytes = pe64();
    fs::write(&candidate, &bytes).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&bytes),
        ArtifactFormat::Pe32Plus,
        RuntimeFamily::Windows,
        ExecutionBackend::Personality,
        Some("x86_64"),
    );

    let prepared = prepare_windows_launch(
        state.path(),
        providers.path(),
        application_id,
        &candidate,
        "x86_64",
    )
    .expect("prepare Windows launch");
    assert_eq!(prepared.profile.application_id, application_id);
    assert_eq!(
        prepared.provider.manifest.provider_id,
        "prime.windows.fixture"
    );
    assert_eq!(
        prepared.profile.execution_backend,
        ExecutionBackend::Personality
    );
    assert_eq!(
        prepared.profile.artifact.runtime_family,
        RuntimeFamily::Windows
    );
    assert!(prepared
        .staged_artifact_path
        .starts_with(state.path().join("artifacts/sha256")));
}

#[test]
fn prepare_admits_x86_pe_on_x64_w1_host() {
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    fixture_provider(providers.path(), &["x86", "x86_64"]);
    let candidate = state.path().join("fixture32.exe");
    let bytes = pe32();
    fs::write(&candidate, &bytes).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&bytes),
        ArtifactFormat::Pe32,
        RuntimeFamily::Windows,
        ExecutionBackend::Personality,
        Some("x86"),
    );

    prepare_windows_launch(
        state.path(),
        providers.path(),
        application_id,
        &candidate,
        "x86_64",
    )
    .expect("prepare x86 Windows launch");
}

#[test]
fn native_profile_is_rejected_by_windows_personality() {
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    fixture_provider(providers.path(), &["x86_64"]);
    let candidate = state.path().join("fixture.exe");
    let bytes = pe64();
    fs::write(&candidate, &bytes).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&bytes),
        ArtifactFormat::Pe32Plus,
        RuntimeFamily::Windows,
        ExecutionBackend::Native,
        Some("x86_64"),
    );

    assert!(matches!(
        prepare_windows_launch(
            state.path(),
            providers.path(),
            application_id,
            &candidate,
            "x86_64"
        ),
        Err(WindowsPersonalityError::ProfileMismatch(_))
    ));
}

#[test]
fn mismatched_candidate_bytes_are_rejected_before_provider_execution() {
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    fixture_provider(providers.path(), &["x86_64"]);
    let candidate = state.path().join("fixture.exe");
    let expected = pe64();
    let mut actual = pe64();
    actual.push(0xaa);
    fs::write(&candidate, &actual).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&expected),
        ArtifactFormat::Pe32Plus,
        RuntimeFamily::Windows,
        ExecutionBackend::Personality,
        Some("x86_64"),
    );

    assert!(matches!(
        prepare_windows_launch(
            state.path(),
            providers.path(),
            application_id,
            &candidate,
            "x86_64"
        ),
        Err(WindowsPersonalityError::ArtifactMismatch(_))
    ));
}

#[test]
fn w1_rejects_non_x64_prime_host_before_launch() {
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    fixture_provider(providers.path(), &["x86_64"]);
    let candidate = state.path().join("fixture.exe");
    let bytes = pe64();
    fs::write(&candidate, &bytes).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&bytes),
        ArtifactFormat::Pe32Plus,
        RuntimeFamily::Windows,
        ExecutionBackend::Personality,
        Some("x86_64"),
    );

    assert!(matches!(
        prepare_windows_launch(
            state.path(),
            providers.path(),
            application_id,
            &candidate,
            "aarch64"
        ),
        Err(WindowsPersonalityError::UnsupportedHostArchitecture(_))
    ));
}

use prime_contracts::{
    FingerprintConfidence, GenerationRecord, GenerationState, HardwareFingerprint, HostIdentity,
    PersonalityLaunchOutcome, ReleaseChannel,
};
use primed::windows_personality::{launch_windows, provider_support, windows_systemd_run_args};

fn fixture_host() -> HostIdentity {
    HostIdentity {
        schema: "prime.host-identity.v1".to_owned(),
        host_id: Uuid::now_v7(),
        lineage_id: Uuid::now_v7(),
        created_at: "2026-09-10T00:00:00Z".to_owned(),
        host_arch: "x86_64".to_owned(),
        hardware_fingerprint: HardwareFingerprint {
            algorithm: "fixture".to_owned(),
            digest: None,
            confidence: FingerprintConfidence::Unprobed,
            observed_at: None,
        },
        rebind_revision: 0,
        supersedes_host_id: None,
    }
}

fn fixture_generation() -> GenerationRecord {
    GenerationRecord {
        schema: "prime.generation.v1".to_owned(),
        generation_id: "prime-w1-fixture".to_owned(),
        image_digest: format!("sha256:{}", "1".repeat(64)),
        channel: ReleaseChannel::Lab,
        created_at: "2026-09-10T00:00:00Z".to_owned(),
        source_revision: "fixture".to_owned(),
        state: GenerationState::KnownGood,
        boot_attempts_remaining: None,
        evidence_refs: vec![],
    }
}

fn prepared_fixture() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    primed::windows_personality::PreparedWindowsLaunch,
) {
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    fixture_provider(providers.path(), &["x86", "x86_64"]);
    let candidate = state.path().join("fixture.exe");
    let bytes = pe64();
    fs::write(&candidate, &bytes).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&bytes),
        ArtifactFormat::Pe32Plus,
        RuntimeFamily::Windows,
        ExecutionBackend::Personality,
        Some("x86_64"),
    );
    let prepared = prepare_windows_launch(
        state.path(),
        providers.path(),
        application_id,
        &candidate,
        "x86_64",
    )
    .expect("prepare");
    (state, providers, prepared)
}

#[test]
fn windows_systemd_argv_invokes_provider_directly_with_prime_abi() {
    let (_state, _providers, prepared) = prepared_fixture();
    let args = windows_systemd_run_args(&prepared);
    let adapter = prepared.provider.adapter_path.display().to_string();
    let runtime_path = format!("/run/{}", prepared.runtime_directory_name);

    assert!(args.iter().any(|arg| arg == &adapter));
    assert!(args.windows(2).any(|pair| pair[0] == "--artifact"
        && pair[1] == prepared.staged_artifact_path.display().to_string()));
    assert!(args.windows(2).any(|pair| pair[0] == "--application-id"
        && pair[1] == prepared.profile.application_id.to_string()));
    assert!(args
        .windows(2)
        .any(|pair| pair[0] == "--launch-id" && pair[1] == prepared.launch_id.to_string()));
    assert!(args
        .windows(2)
        .any(|pair| pair[0] == "--runtime-dir" && pair[1] == runtime_path));
    assert!(args.iter().any(|arg| arg
        == &format!(
            "--property=RuntimeDirectory={}",
            prepared.runtime_directory_name
        )));
    assert!(args
        .iter()
        .any(|arg| arg == "--property=RuntimeDirectoryMode=0700"));
    let app_state = format!(
        "prime-win-app-{}",
        prepared.profile.application_id.to_string().replace('-', "")
    );
    assert!(args
        .iter()
        .any(|arg| arg == &format!("--property=StateDirectory={app_state}")));
    assert!(args
        .iter()
        .any(|arg| arg == "--property=StateDirectoryMode=0700"));
    assert!(args
        .iter()
        .any(|arg| arg == "--property=PrivateNetwork=yes"));
    assert!(args.iter().any(|arg| arg == "--property=DynamicUser=yes"));
    assert!(args.iter().any(|arg| arg == "--property=RemoveIPC=yes"));
    assert!(args.iter().any(|arg| arg == "--property=UMask=0077"));
    assert!(args
        .iter()
        .any(|arg| arg == "--property=SupplementaryGroups=prime-display"));
    assert!(args
        .iter()
        .any(|arg| arg == "--setenv=XDG_RUNTIME_DIR=/run/prime-compositor"));
    assert!(!args
        .iter()
        .any(|arg| matches!(arg.as_str(), "sh" | "/bin/sh" | "bash" | "/bin/bash" | "-c")));
    assert!(!args.iter().any(|arg| arg.contains("prime-shell")));
}

#[test]
fn successful_windows_launch_records_personality_provider_evidence() {
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    fixture_provider(providers.path(), &["x86_64"]);
    let candidate = state.path().join("fixture.exe");
    let bytes = pe64();
    fs::write(&candidate, &bytes).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&bytes),
        ArtifactFormat::Pe32Plus,
        RuntimeFamily::Windows,
        ExecutionBackend::Personality,
        Some("x86_64"),
    );
    let host = fixture_host();
    let generation = fixture_generation();

    let evidence = launch_windows(
        state.path(),
        providers.path(),
        Path::new("/usr/bin/true"),
        &host,
        &generation,
        application_id,
        &candidate,
    )
    .expect("launch");

    assert_eq!(evidence.execution_backend, ExecutionBackend::Personality);
    assert_eq!(evidence.runtime_family, RuntimeFamily::Windows);
    assert_eq!(evidence.provider_id, "prime.windows.fixture");
    assert_eq!(evidence.provider_revision, 1);
    assert!(evidence
        .enforcement_properties
        .iter()
        .any(
            |property| property.name == "SupplementaryGroups" && property.value == "prime-display"
        ));
    assert_eq!(evidence.outcome, PersonalityLaunchOutcome::ExitedSuccess);
    assert_eq!(evidence.launcher_exit_code, Some(0));
    let evidence_dir = state
        .path()
        .join("evidence/launches")
        .join(evidence.launch_id.to_string());
    assert!(evidence_dir.join("01-admitted.json").is_file());
    assert!(evidence_dir.join("02-completed.json").is_file());
}

#[test]
fn provider_support_reports_validated_union_without_overclaiming() {
    let dir = tempfile::tempdir().expect("providers");
    let adapter_x64 = dir.path().join("adapter-x64");
    let adapter_x86 = dir.path().join("adapter-x86");
    write_adapter(&adapter_x64, 0o755);
    write_adapter(&adapter_x86, 0o755);
    write_manifest(
        dir.path(),
        "x64.json",
        "prime.windows.compat",
        2,
        &adapter_x64,
        &["x86_64"],
    );
    let manifest = WindowsProviderManifest {
        schema: WINDOWS_PROVIDER_MANIFEST_SCHEMA.to_owned(),
        provider_id: "prime.windows.compat32".to_owned(),
        provider_revision: 1,
        adapter_path: adapter_x86.display().to_string(),
        formats: vec![ArtifactFormat::Pe32],
        workload_arches: vec!["x86".to_owned()],
        limitations: vec!["W1_PORTABLE_ONLY".to_owned()],
    };
    fs::write(
        dir.path().join("x86.json"),
        serde_json::to_vec_pretty(&manifest).expect("serialize"),
    )
    .expect("write manifest");

    let support = provider_support(dir.path(), "x86_64").expect("support");
    assert_eq!(support.provider_count, 2);
    assert_eq!(
        support.formats,
        vec![ArtifactFormat::Pe32, ArtifactFormat::Pe32Plus]
    );
    assert_eq!(
        support.workload_arches,
        vec!["x86".to_owned(), "x86_64".to_owned()]
    );
}

#[test]
fn shipped_wine_mono_dependency_resolves_from_real_application_profile() {
    let state = tempfile::tempdir().expect("state");
    let providers = tempfile::tempdir().expect("providers");
    fixture_provider(providers.path(), &["x86_64"]);
    let candidate = state.path().join("fixture.exe");
    let bytes = pe64();
    fs::write(&candidate, &bytes).expect("write PE");
    let policy = fixture_policy(state.path());
    let application_id = fixture_profile(
        state.path(),
        &policy,
        labelled_sha256(&bytes),
        ArtifactFormat::Pe32Plus,
        RuntimeFamily::Windows,
        ExecutionBackend::Personality,
        Some("x86_64"),
    );
    let component_dir =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../image/windows-components");
    let mono =
        primed::windows_components::load_component_revision(&component_dir, "runtime.wine-mono", 1)
            .expect("load shipped Wine Mono component");
    add_profile_dependencies(
        state.path(),
        application_id,
        vec![component_reference(&mono)],
    );

    let prepared = prepare_windows_launch(
        state.path(),
        providers.path(),
        application_id,
        &candidate,
        "x86_64",
    )
    .expect("prepare dependency-bearing Windows launch");
    let dependencies = prepare_windows_dependencies(&component_dir, &prepared)
        .expect("resolve shipped Wine Mono dependency from profile");
    assert_eq!(dependencies.components.len(), 1);
    assert_eq!(dependencies.components[0].manifest, mono);
}
