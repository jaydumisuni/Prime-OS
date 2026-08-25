use crate::exec;
use crate::identity;
use crate::policy::{compile_native, NativeEnforcementPlan, PolicyCompileError, SystemdProperty};
use crate::registry::{self, RegistryError};
use prime_contracts::{
    ApplicationProfile, ArtifactFormat, ExecutionBackend, GenerationRecord, HostIdentity,
    LaunchEnforcementProperty, MechanicalCompatibilityState, NativeLaunchEvidence,
    NativeLaunchOutcome, NativeLaunchRequest, PolicyClass, RuntimeFamily,
    NATIVE_LAUNCH_EVIDENCE_SCHEMA, NATIVE_LAUNCH_REQUEST_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum LaunchError {
    #[error(transparent)]
    Registry(#[from] RegistryError),
    #[error(transparent)]
    Exec(#[from] exec::ExecError),
    #[error(transparent)]
    Policy(#[from] PolicyCompileError),
    #[error(transparent)]
    Identity(#[from] identity::IdentityError),
    #[error("native launch I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("invalid native launch request: {0}")]
    InvalidRequest(&'static str),
    #[error("selected profile is not admissible for P1 native launch: {0}")]
    UnsupportedProfile(&'static str),
    #[error("selected profile policy reference does not match the stored policy")]
    PolicyReferenceMismatch,
    #[error("candidate/staged artifact does not match the selected profile: {0}")]
    ArtifactMismatch(&'static str),
}

#[derive(Debug, Clone)]
pub struct PreparedNativeLaunch {
    pub launch_id: Uuid,
    pub profile: ApplicationProfile,
    pub policy_id: Uuid,
    pub policy_revision: u64,
    pub policy_digest: String,
    pub staged_artifact_path: PathBuf,
    pub unit_name: String,
    pub requested_at: String,
    pub plan: NativeEnforcementPlan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    device: u64,
    inode: u64,
    length: u64,
    mode: u32,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

impl From<&fs::Metadata> for FileStamp {
    fn from(metadata: &fs::Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            mode: metadata.mode(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        }
    }
}

pub fn prepare_native_launch(
    state_dir: &Path,
    request: &NativeLaunchRequest,
    host_arch: &str,
) -> Result<PreparedNativeLaunch, LaunchError> {
    if request.schema != NATIVE_LAUNCH_REQUEST_SCHEMA {
        return Err(LaunchError::InvalidRequest("unexpected request schema"));
    }
    let candidate = Path::new(&request.artifact_path);
    if !candidate.is_absolute() {
        return Err(LaunchError::InvalidRequest(
            "artifact_path must be absolute",
        ));
    }

    let profile = registry::load_selected_profile(state_dir, request.application_id)?;
    validate_profile_for_native(&profile)?;

    let policy_ref = &profile.workload_policy;
    let policy = registry::load_policy_revision(
        state_dir,
        policy_ref.policy_id,
        policy_ref.policy_revision,
    )?;
    if policy.digest != policy_ref.policy_digest {
        return Err(LaunchError::PolicyReferenceMismatch);
    }
    let mut plan = compile_native(&policy)?;
    if matches!(
        policy.class,
        PolicyClass::UserApp | PolicyClass::Build | PolicyClass::ForeignRuntime
    ) {
        plan.properties.push(SystemdProperty {
            name: "DynamicUser".to_owned(),
            value: "yes".to_owned(),
        });
        plan.properties.push(SystemdProperty {
            name: "RemoveIPC".to_owned(),
            value: "yes".to_owned(),
        });
        plan.properties.push(SystemdProperty {
            name: "UMask".to_owned(),
            value: "0077".to_owned(),
        });
    }

    let staged_artifact_path =
        stage_artifact(state_dir, candidate, &profile.artifact.identity, host_arch)?;
    let inspection = exec::inspect(&staged_artifact_path, host_arch)?;
    if inspection.artifact_identity != profile.artifact.identity {
        return Err(LaunchError::ArtifactMismatch("SHA-256 identity differs"));
    }
    if inspection.format != profile.artifact.format {
        return Err(LaunchError::ArtifactMismatch("format differs"));
    }
    if inspection.runtime_family != profile.artifact.runtime_family {
        return Err(LaunchError::ArtifactMismatch("runtime family differs"));
    }
    if inspection.workload_arch != profile.artifact.workload_arch {
        return Err(LaunchError::ArtifactMismatch(
            "workload architecture differs",
        ));
    }
    if !inspection.native_compatible {
        return Err(LaunchError::ArtifactMismatch(
            "staged ELF is not native-compatible with this Host",
        ));
    }

    let launch_id = Uuid::now_v7();
    let compact_app_id = profile.application_id.to_string().replace('-', "");
    let compact_launch_id = launch_id.to_string().replace('-', "");
    let unit_name = format!(
        "prime-app-{compact_app_id}-r{}-{compact_launch_id}.service",
        profile.profile_revision
    );

    Ok(PreparedNativeLaunch {
        launch_id,
        profile,
        policy_id: policy.policy_id,
        policy_revision: policy.revision,
        policy_digest: policy.digest,
        staged_artifact_path,
        unit_name,
        requested_at: identity::now_rfc3339()?,
        plan,
    })
}

pub fn launch_native(
    state_dir: &Path,
    systemd_run: &Path,
    host: &HostIdentity,
    generation: &GenerationRecord,
    request: &NativeLaunchRequest,
) -> Result<NativeLaunchEvidence, LaunchError> {
    let prepared = prepare_native_launch(state_dir, request, &host.host_arch)?;
    let admitted = evidence_for(
        &prepared,
        host,
        generation,
        NativeLaunchOutcome::Admitted,
        None,
        None,
    );
    store_evidence(state_dir, &admitted, 1, "admitted")?;

    let status = Command::new(systemd_run)
        .args(systemd_run_args(&prepared))
        .status();

    let (outcome, exit_code) = match status {
        Ok(status) if status.success() => (NativeLaunchOutcome::ExitedSuccess, status.code()),
        Ok(status) => (NativeLaunchOutcome::SystemdOrWorkloadFailure, status.code()),
        Err(_) => (NativeLaunchOutcome::LauncherFailure, None),
    };
    let completed_at = identity::now_rfc3339()?;
    let completed = evidence_for(
        &prepared,
        host,
        generation,
        outcome,
        exit_code,
        Some(completed_at),
    );
    store_evidence(state_dir, &completed, 2, "completed")?;
    Ok(completed)
}

pub fn systemd_run_args(prepared: &PreparedNativeLaunch) -> Vec<String> {
    let mut args = vec![
        "--system".to_owned(),
        format!("--unit={}", prepared.unit_name),
        "--service-type=exec".to_owned(),
        "--wait".to_owned(),
        "--collect".to_owned(),
        "--no-ask-password".to_owned(),
        "--quiet".to_owned(),
    ];
    for property in &prepared.plan.properties {
        args.push(format!("--property={}={}", property.name, property.value));
    }
    args.push(prepared.staged_artifact_path.display().to_string());
    args
}

fn validate_profile_for_native(profile: &ApplicationProfile) -> Result<(), LaunchError> {
    if profile.execution_backend != ExecutionBackend::Native {
        return Err(LaunchError::UnsupportedProfile(
            "execution_backend is not NATIVE",
        ));
    }
    if profile.artifact.format != ArtifactFormat::Elf {
        return Err(LaunchError::UnsupportedProfile(
            "artifact format is not ELF",
        ));
    }
    if profile.artifact.runtime_family != RuntimeFamily::NativeLinux {
        return Err(LaunchError::UnsupportedProfile(
            "runtime family is not NATIVE_LINUX",
        ));
    }
    if profile.artifact.workload_arch.is_none() {
        return Err(LaunchError::UnsupportedProfile(
            "native workload architecture is unresolved",
        ));
    }
    if !profile.dependencies.is_empty() {
        return Err(LaunchError::UnsupportedProfile(
            "P1 dependency admission is not implemented",
        ));
    }
    if !profile.permissions.is_empty() {
        return Err(LaunchError::UnsupportedProfile(
            "P1 application permission mediation is not implemented",
        ));
    }
    if matches!(
        profile.compatibility.state,
        MechanicalCompatibilityState::Unknown
            | MechanicalCompatibilityState::Broken
            | MechanicalCompatibilityState::Unsupported
            | MechanicalCompatibilityState::RequiresVm
            | MechanicalCompatibilityState::RequiresRemoteProvider
    ) {
        return Err(LaunchError::UnsupportedProfile(
            "mechanical compatibility state does not permit a native attempt",
        ));
    }
    Ok(())
}

fn stage_artifact(
    state_dir: &Path,
    source: &Path,
    expected_identity: &str,
    host_arch: &str,
) -> Result<PathBuf, LaunchError> {
    let digest_hex = digest_hex(expected_identity)?;
    let root = state_dir.join("artifacts/sha256");
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o755))?;
    let final_path = root.join(digest_hex);

    if final_path.exists() {
        let existing = exec::inspect(&final_path, host_arch)?;
        if existing.artifact_identity != expected_identity {
            return Err(LaunchError::ArtifactMismatch(
                "existing content-addressed object failed identity verification",
            ));
        }
        return Ok(final_path);
    }

    let source_path_metadata = fs::symlink_metadata(source)?;
    if source_path_metadata.file_type().is_symlink() {
        return Err(LaunchError::ArtifactMismatch(
            "candidate source is a symbolic link",
        ));
    }
    if !source_path_metadata.file_type().is_file() {
        return Err(LaunchError::ArtifactMismatch(
            "candidate source is not a regular file",
        ));
    }
    let source_stamp = FileStamp::from(&source_path_metadata);
    let mut source_file = File::open(source)?;
    if FileStamp::from(&source_file.metadata()?) != source_stamp {
        return Err(LaunchError::ArtifactMismatch(
            "candidate changed before staging began",
        ));
    }

    let temp_path = root.join(format!(".artifact.{}.tmp", Uuid::now_v7()));
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o500)
        .open(&temp_path)?;
    let mut hasher = Sha256::new();
    let mut copied = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = source_file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        temp.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        copied = copied
            .checked_add(read as u64)
            .ok_or(LaunchError::ArtifactMismatch("artifact size overflow"))?;
    }
    temp.sync_all()?;

    let after_open = source_file.metadata()?;
    let after_path = fs::symlink_metadata(source)?;
    if after_path.file_type().is_symlink()
        || !after_path.file_type().is_file()
        || FileStamp::from(&after_open) != source_stamp
        || FileStamp::from(&after_path) != source_stamp
        || copied != source_stamp.length
    {
        let _ = fs::remove_file(&temp_path);
        return Err(LaunchError::ArtifactMismatch(
            "candidate changed while being staged",
        ));
    }

    let copied_identity = sha256_labelled(&hasher.finalize());
    if copied_identity != expected_identity {
        let _ = fs::remove_file(&temp_path);
        return Err(LaunchError::ArtifactMismatch(
            "candidate bytes do not match the selected profile identity",
        ));
    }

    fs::set_permissions(&temp_path, fs::Permissions::from_mode(0o555))?;
    match fs::hard_link(&temp_path, &final_path) {
        Ok(()) => {
            fs::remove_file(&temp_path)?;
            File::open(&root)?.sync_all()?;
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(&temp_path)?;
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            return Err(error.into());
        }
    }

    let staged = exec::inspect(&final_path, host_arch)?;
    if staged.artifact_identity != expected_identity {
        return Err(LaunchError::ArtifactMismatch(
            "published content-addressed object failed identity verification",
        ));
    }
    Ok(final_path)
}

fn evidence_for(
    prepared: &PreparedNativeLaunch,
    host: &HostIdentity,
    generation: &GenerationRecord,
    outcome: NativeLaunchOutcome,
    launcher_exit_code: Option<i32>,
    completed_at: Option<String>,
) -> NativeLaunchEvidence {
    NativeLaunchEvidence {
        schema: NATIVE_LAUNCH_EVIDENCE_SCHEMA.to_owned(),
        launch_id: prepared.launch_id,
        host_id: host.host_id,
        generation_id: generation.generation_id.clone(),
        application_id: prepared.profile.application_id,
        profile_revision: prepared.profile.profile_revision,
        profile_digest: prepared.profile.profile_digest.clone(),
        policy_id: prepared.policy_id,
        policy_revision: prepared.policy_revision,
        policy_digest: prepared.policy_digest.clone(),
        artifact_identity: prepared.profile.artifact.identity.clone(),
        staged_artifact_path: prepared.staged_artifact_path.display().to_string(),
        unit_name: prepared.unit_name.clone(),
        requested_at: prepared.requested_at.clone(),
        completed_at,
        outcome,
        launcher_exit_code,
        enforcement_properties: prepared
            .plan
            .properties
            .iter()
            .map(|property| LaunchEnforcementProperty {
                name: property.name.clone(),
                value: property.value.clone(),
            })
            .collect(),
    }
}

fn store_evidence(
    state_dir: &Path,
    evidence: &NativeLaunchEvidence,
    sequence: u8,
    phase: &str,
) -> Result<(), LaunchError> {
    let root = state_dir
        .join("evidence/launches")
        .join(evidence.launch_id.to_string());
    fs::create_dir_all(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    let path = root.join(format!("{sequence:02}-{phase}.json"));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)?;
    let encoded = serde_json::to_vec_pretty(evidence).map_err(RegistryError::from)?;
    file.write_all(&encoded)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    File::open(&root)?.sync_all()?;
    Ok(())
}

fn digest_hex(identity: &str) -> Result<&str, LaunchError> {
    let Some(hex) = identity.strip_prefix("sha256:") else {
        return Err(LaunchError::ArtifactMismatch(
            "profile artifact identity is not SHA-256",
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(LaunchError::ArtifactMismatch(
            "profile artifact SHA-256 is not canonical lowercase hex",
        ));
    }
    Ok(hex)
}

fn sha256_labelled(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(7 + bytes.len() * 2);
    encoded.push_str("sha256:");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{
        seal_policy, seal_profile, select_policy_revision, select_profile_revision,
        store_policy_revision, store_profile_revision,
    };
    use prime_contracts::*;

    fn native_elf() -> Vec<u8> {
        let mut bytes = vec![0_u8; 64];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[18..20].copy_from_slice(&62_u16.to_le_bytes());
        bytes
    }

    fn identity(bytes: &[u8]) -> String {
        sha256_labelled(&Sha256::digest(bytes))
    }

    fn fixture_registry(root: &Path, artifact_identity: String) -> Uuid {
        let policy = seal_policy(WorkloadPolicy {
            schema: WORKLOAD_POLICY_SCHEMA.to_owned(),
            policy_id: Uuid::now_v7(),
            revision: 1,
            digest: String::new(),
            class: PolicyClass::UserApp,
            cpu: CpuPolicy {
                weight: 100,
                quota_percent: None,
            },
            memory: MemoryPolicy {
                max_bytes: Some(64 * 1024 * 1024),
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
                max_processes: Some(8),
                max_runtime_seconds: Some(10),
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
        .expect("seal policy");
        store_policy_revision(root, &policy).expect("store policy");
        select_policy_revision(root, policy.policy_id, 1).expect("select policy");

        let application_id = Uuid::now_v7();
        let profile = seal_profile(ApplicationProfile {
            schema: APPLICATION_PROFILE_SCHEMA.to_owned(),
            application_id,
            profile_revision: 1,
            profile_digest: String::new(),
            display_name: "Fixture".to_owned(),
            artifact: ApplicationArtifact {
                identity: artifact_identity,
                format: ArtifactFormat::Elf,
                runtime_family: RuntimeFamily::NativeLinux,
                workload_arch: Some("x86_64".to_owned()),
            },
            execution_backend: ExecutionBackend::Native,
            dependencies: Vec::new(),
            workload_policy: PolicyReference {
                policy_id: policy.policy_id,
                policy_revision: policy.revision,
                policy_digest: policy.digest.clone(),
            },
            permissions: Vec::new(),
            compatibility: CompatibilityRecord {
                state: MechanicalCompatibilityState::Recognized,
                evidence_refs: Vec::new(),
            },
            revoked: false,
            revocation_reason: None,
            created_at: "t1".to_owned(),
        })
        .expect("seal profile");
        store_profile_revision(root, &profile).expect("store profile");
        select_profile_revision(root, application_id, 1).expect("select profile");
        application_id
    }

    #[test]
    fn preparation_stages_exact_profile_artifact() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("candidate");
        let bytes = native_elf();
        fs::write(&source, &bytes).expect("write artifact");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).expect("chmod");
        let application_id = fixture_registry(dir.path(), identity(&bytes));
        let request = NativeLaunchRequest {
            schema: NATIVE_LAUNCH_REQUEST_SCHEMA.to_owned(),
            application_id,
            artifact_path: source.display().to_string(),
        };
        let prepared = prepare_native_launch(dir.path(), &request, "x86_64").expect("prepare");
        assert!(prepared
            .staged_artifact_path
            .starts_with(dir.path().join("artifacts/sha256")));
        assert_ne!(prepared.staged_artifact_path, source);
        assert!(prepared
            .plan
            .properties
            .iter()
            .any(|property| property.name == "DynamicUser" && property.value == "yes"));
    }

    #[test]
    fn wrong_artifact_identity_is_denied_before_systemd() {
        let dir = tempfile::tempdir().expect("tempdir");
        let source = dir.path().join("candidate");
        let bytes = native_elf();
        fs::write(&source, &bytes).expect("write artifact");
        fs::set_permissions(&source, fs::Permissions::from_mode(0o755)).expect("chmod");
        let application_id = fixture_registry(dir.path(), format!("sha256:{}", "0".repeat(64)));
        let request = NativeLaunchRequest {
            schema: NATIVE_LAUNCH_REQUEST_SCHEMA.to_owned(),
            application_id,
            artifact_path: source.display().to_string(),
        };
        assert!(matches!(
            prepare_native_launch(dir.path(), &request, "x86_64"),
            Err(LaunchError::ArtifactMismatch(_))
        ));
    }

    #[test]
    fn systemd_argv_never_uses_a_shell() {
        let prepared = PreparedNativeLaunch {
            launch_id: Uuid::now_v7(),
            profile: ApplicationProfile {
                schema: APPLICATION_PROFILE_SCHEMA.to_owned(),
                application_id: Uuid::now_v7(),
                profile_revision: 1,
                profile_digest: "sha256:p".to_owned(),
                display_name: "Fixture".to_owned(),
                artifact: ApplicationArtifact {
                    identity: "sha256:a".to_owned(),
                    format: ArtifactFormat::Elf,
                    runtime_family: RuntimeFamily::NativeLinux,
                    workload_arch: Some("x86_64".to_owned()),
                },
                execution_backend: ExecutionBackend::Native,
                dependencies: Vec::new(),
                workload_policy: PolicyReference {
                    policy_id: Uuid::now_v7(),
                    policy_revision: 1,
                    policy_digest: "sha256:w".to_owned(),
                },
                permissions: Vec::new(),
                compatibility: CompatibilityRecord {
                    state: MechanicalCompatibilityState::Recognized,
                    evidence_refs: Vec::new(),
                },
                revoked: false,
                revocation_reason: None,
                created_at: "t1".to_owned(),
            },
            policy_id: Uuid::now_v7(),
            policy_revision: 1,
            policy_digest: "sha256:w".to_owned(),
            staged_artifact_path: PathBuf::from("/var/lib/prime/artifacts/sha256/abc"),
            unit_name: "prime-app-fixture.service".to_owned(),
            requested_at: "t1".to_owned(),
            plan: NativeEnforcementPlan {
                properties: vec![SystemdProperty {
                    name: "PrivateNetwork".to_owned(),
                    value: "yes".to_owned(),
                }],
                background_allowed: false,
                evidence_required: true,
            },
        };
        let args = systemd_run_args(&prepared);
        assert_eq!(
            args.last().map(String::as_str),
            Some("/var/lib/prime/artifacts/sha256/abc")
        );
        assert!(!args
            .iter()
            .any(|arg| arg == "sh" || arg == "bash" || arg == "-c"));
        assert!(args.iter().any(|arg| arg == "--service-type=exec"));
        assert!(args.iter().any(|arg| arg == "--wait"));
        assert!(args.iter().any(|arg| arg == "--collect"));
    }
}
