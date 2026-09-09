use crate::{exec, launcher, registry, windows_personality, CoreState};
use prime_contracts::{
    ApplicationEntry, ApplicationProfile, ApplicationsProjection, ArtifactFormat, ExecutionBackend,
    LaunchEvidence, MechanicalCompatibilityState, NativeLaunchRequest, RuntimeFamily,
    ShellLaunchRequest, APPLICATIONS_PROJECTION_SCHEMA, CAPABILITY_INTERFACE,
    NATIVE_LAUNCH_REQUEST_SCHEMA, SHELL_LAUNCH_REQUEST_SCHEMA,
};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum ShellApiError {
    #[error("Shell application registry I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Registry(#[from] registry::RegistryError),
    #[error(transparent)]
    Exec(#[from] exec::ExecError),
    #[error(transparent)]
    Launch(#[from] launcher::LaunchError),
    #[error(transparent)]
    Windows(#[from] windows_personality::WindowsPersonalityError),
    #[error("invalid Shell launch request: {0}")]
    InvalidRequest(&'static str),
    #[error("selected Application Profile state is invalid: {0}")]
    InvalidSelected(String),
    #[error("selected application artifact is unavailable: {0}")]
    ArtifactUnavailable(&'static str),
}

pub fn applications_projection(
    state: &CoreState,
    interface_version: &str,
) -> Result<ApplicationsProjection, ShellApiError> {
    let mut profiles = list_selected_profiles(&state.state_dir)?;
    profiles.sort_by(|left, right| {
        left.display_name
            .to_lowercase()
            .cmp(&right.display_name.to_lowercase())
            .then_with(|| {
                left.application_id
                    .as_bytes()
                    .cmp(right.application_id.as_bytes())
            })
    });

    let applications = profiles
        .into_iter()
        .map(|profile| application_entry(state, profile))
        .collect();

    Ok(ApplicationsProjection {
        schema: APPLICATIONS_PROJECTION_SCHEMA.to_owned(),
        interface: CAPABILITY_INTERFACE.to_owned(),
        interface_version: interface_version.to_owned(),
        host_id: state.host.host_id,
        generation_id: state.generation.generation_id.clone(),
        applications,
        limitations: vec![
            "P1 lists selected Host-local Application Profiles only".to_owned(),
            "Final Prime Exec admission is revalidated at launch time".to_owned(),
        ],
    })
}

pub fn launch_selected(
    state: &CoreState,
    request: &ShellLaunchRequest,
) -> Result<LaunchEvidence, ShellApiError> {
    if request.schema != SHELL_LAUNCH_REQUEST_SCHEMA {
        return Err(ShellApiError::InvalidRequest(
            "unexpected Shell launch request schema",
        ));
    }

    let profile =
        load_selected_profile_including_revoked(&state.state_dir, request.application_id)?;
    if profile.revoked {
        return Err(ShellApiError::InvalidRequest(
            "selected Application Profile is revoked",
        ));
    }
    let artifact_path = artifact_path(&state.state_dir, &profile)?;
    let inspection = exec::inspect(&artifact_path, &state.host.host_arch)?;
    if inspection.artifact_identity != profile.artifact.identity {
        return Err(ShellApiError::ArtifactUnavailable(
            "content-addressed artifact identity does not match the selected profile",
        ));
    }

    match (&profile.execution_backend, &profile.artifact.runtime_family) {
        (ExecutionBackend::Native, RuntimeFamily::NativeLinux) => launcher::launch_native(
            &state.state_dir,
            &state.systemd_run,
            &state.host,
            &state.generation,
            &NativeLaunchRequest {
                schema: NATIVE_LAUNCH_REQUEST_SCHEMA.to_owned(),
                application_id: request.application_id,
                artifact_path: artifact_path.display().to_string(),
            },
        )
        .map(LaunchEvidence::Native)
        .map_err(Into::into),
        (ExecutionBackend::Personality, RuntimeFamily::Windows) => {
            windows_personality::launch_windows(
                &state.state_dir,
                &state.windows_provider_dir,
                &state.systemd_run,
                &state.host,
                &state.generation,
                request.application_id,
                &artifact_path,
            )
            .map(LaunchEvidence::Windows)
            .map_err(Into::into)
        }
        _ => Err(ShellApiError::InvalidRequest(
            "selected Application Profile backend/runtime is not launchable",
        )),
    }

}

fn application_entry(state: &CoreState, profile: ApplicationProfile) -> ApplicationEntry {
    let mut limitations = profile_limitations(
        &profile,
        &state.host.host_arch,
        &state.windows_provider_dir,
    );
    match artifact_path(&state.state_dir, &profile) {
        Ok(path) => match exec::inspect(&path, &state.host.host_arch) {
            Ok(inspection) if inspection.artifact_identity == profile.artifact.identity => {}
            Ok(_) => limitations.push(
                "Content-addressed artifact identity does not match the selected profile"
                    .to_owned(),
            ),
            Err(error) => limitations.push(format!("Stored artifact inspection failed: {error}")),
        },
        Err(error) => limitations.push(error.to_string()),
    }
    limitations.sort();
    limitations.dedup();

    ApplicationEntry {
        application_id: profile.application_id,
        display_name: profile.display_name,
        profile_revision: profile.profile_revision,
        profile_digest: profile.profile_digest,
        execution_backend: profile.execution_backend,
        compatibility: profile.compatibility,
        launch_ready: limitations.is_empty(),
        limitations,
    }
}

fn profile_limitations(
    profile: &ApplicationProfile,
    host_arch: &str,
    windows_provider_dir: &Path,
) -> Vec<String> {
    let mut limitations = Vec::new();
    if profile.revoked {
        limitations.push(
            profile
                .revocation_reason
                .clone()
                .unwrap_or_else(|| "Selected Application Profile is revoked".to_owned()),
        );
    }

    match (&profile.execution_backend, &profile.artifact.runtime_family) {
        (ExecutionBackend::Native, RuntimeFamily::NativeLinux) => {
            if profile.artifact.format != ArtifactFormat::Elf {
                limitations.push("P1 native launch requires an ELF artifact".to_owned());
            }
            match profile.artifact.workload_arch.as_deref() {
                Some(arch) if arch == host_arch => {}
                Some(_) => limitations
                    .push("Application workload architecture does not match this Host".to_owned()),
                None => limitations.push("Application workload architecture is unresolved".to_owned()),
            }
        }
        (ExecutionBackend::Personality, RuntimeFamily::Windows) => {
            if !matches!(profile.artifact.format, ArtifactFormat::Pe32 | ArtifactFormat::Pe32Plus) {
                limitations.push("Windows Personality requires a PE32/PE32+ artifact".to_owned());
            }
            if !matches!(host_arch, "x86_64" | "amd64") {
                limitations.push("Windows Personality W1 requires an x86_64 Prime Host".to_owned());
            }
            match profile.artifact.workload_arch.as_deref() {
                Some(arch @ ("x86" | "x86_64")) if matches!(host_arch, "x86_64" | "amd64") => {
                    if let Err(error) = windows_personality::load_provider(
                        windows_provider_dir,
                        &profile.artifact.format,
                        arch,
                    ) {
                        limitations.push(error.to_string());
                    }
                }
                Some(_) => limitations.push(
                    "Windows Personality W1 supports x86/x86_64 workloads only".to_owned(),
                ),
                None => limitations.push("Application workload architecture is unresolved".to_owned()),
            }
        }
        _ => limitations.push(
            "Selected Application Profile backend/runtime is not supported by this Prime generation"
                .to_owned(),
        ),
    }

    if !profile.dependencies.is_empty() {
        limitations.push("W1 dependency admission is not implemented".to_owned());
    }
    if !profile.permissions.is_empty() {
        limitations.push("W1 application permission mediation is not implemented".to_owned());
    }
    if matches!(
        profile.compatibility.state,
        MechanicalCompatibilityState::Unknown
            | MechanicalCompatibilityState::Broken
            | MechanicalCompatibilityState::Unsupported
            | MechanicalCompatibilityState::RequiresVm
            | MechanicalCompatibilityState::RequiresRemoteProvider
    ) {
        limitations.push("Mechanical compatibility state does not permit a launch attempt".to_owned());
    }
    limitations
}

fn list_selected_profiles(root: &Path) -> Result<Vec<ApplicationProfile>, ShellApiError> {
    let applications = root.join("applications");
    if !applications.exists() {
        return Ok(Vec::new());
    }

    let mut profiles = Vec::new();
    for entry in fs::read_dir(applications)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Ok(application_id) = Uuid::parse_str(&name) else {
            continue;
        };
        let selected = entry.path().join("selected");
        if !selected.is_file() {
            continue;
        }
        profiles.push(load_selected_profile_including_revoked(
            root,
            application_id,
        )?);
    }
    Ok(profiles)
}

fn load_selected_profile_including_revoked(
    root: &Path,
    application_id: Uuid,
) -> Result<ApplicationProfile, ShellApiError> {
    let selected = root
        .join("applications")
        .join(application_id.to_string())
        .join("selected");
    let raw = fs::read_to_string(&selected)?;
    let revision = raw.trim().parse::<u64>().map_err(|_| {
        ShellApiError::InvalidSelected(format!("{} is not a revision number", selected.display()))
    })?;
    if revision == 0 {
        return Err(ShellApiError::InvalidSelected(format!(
            "{} selects revision zero",
            selected.display()
        )));
    }
    registry::load_profile_revision(root, application_id, revision).map_err(Into::into)
}

fn artifact_path(root: &Path, profile: &ApplicationProfile) -> Result<PathBuf, ShellApiError> {
    let Some(hex) = profile.artifact.identity.strip_prefix("sha256:") else {
        return Err(ShellApiError::ArtifactUnavailable(
            "profile artifact identity is not SHA-256",
        ));
    };
    if hex.len() != 64
        || !hex
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(ShellApiError::ArtifactUnavailable(
            "profile artifact SHA-256 is not canonical lowercase hex",
        ));
    }
    let path = root.join("artifacts/sha256").join(hex);
    let metadata = fs::symlink_metadata(&path).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ShellApiError::ArtifactUnavailable("content-addressed artifact is not staged")
        } else {
            ShellApiError::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(ShellApiError::ArtifactUnavailable(
            "content-addressed artifact is not a regular file",
        ));
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;


    #[test]
    fn windows_profile_is_launch_ready_only_when_compatible_provider_exists() {
        use prime_contracts::{WindowsProviderManifest, WINDOWS_PROVIDER_MANIFEST_SCHEMA};
        use std::os::unix::fs::PermissionsExt;

        let providers = tempfile::tempdir().expect("providers");
        let adapter = providers.path().join("adapter");
        fs::write(&adapter, b"adapter").expect("write adapter");
        fs::set_permissions(&adapter, fs::Permissions::from_mode(0o755)).expect("chmod");
        let manifest = WindowsProviderManifest {
            schema: WINDOWS_PROVIDER_MANIFEST_SCHEMA.to_owned(),
            provider_id: "prime.windows.compat".to_owned(),
            provider_revision: 1,
            adapter_path: adapter.display().to_string(),
            formats: vec![ArtifactFormat::Pe32Plus],
            workload_arches: vec!["x86_64".to_owned()],
            limitations: vec![],
        };
        fs::write(
            providers.path().join("provider.json"),
            serde_json::to_vec(&manifest).expect("serialize"),
        ).expect("manifest");
        let profile = ApplicationProfile {
            schema: "prime.application-profile.v1".to_owned(),
            application_id: Uuid::now_v7(),
            profile_revision: 1,
            profile_digest: "sha256:test".to_owned(),
            display_name: "Windows Fixture".to_owned(),
            artifact: prime_contracts::ApplicationArtifact {
                identity: format!("sha256:{}", "0".repeat(64)),
                format: ArtifactFormat::Pe32Plus,
                runtime_family: RuntimeFamily::Windows,
                workload_arch: Some("x86_64".to_owned()),
            },
            execution_backend: ExecutionBackend::Personality,
            dependencies: vec![],
            workload_policy: prime_contracts::PolicyReference {
                policy_id: Uuid::now_v7(),
                policy_revision: 1,
                policy_digest: "sha256:test".to_owned(),
            },
            permissions: vec![],
            compatibility: prime_contracts::CompatibilityRecord {
                state: MechanicalCompatibilityState::Recognized,
                evidence_refs: vec![],
            },
            revoked: false,
            revocation_reason: None,
            created_at: "2026-09-10T00:00:00Z".to_owned(),
        };

        assert!(profile_limitations(&profile, "x86_64", providers.path()).is_empty());
        assert!(profile_limitations(&profile, "x86_64", &providers.path().join("missing"))
            .iter()
            .any(|value| value.contains("WINDOWS_PERSONALITY_UNAVAILABLE")));
    }


    #[test]
    fn shell_launch_dispatches_windows_profile_to_windows_personality() {
        use prime_contracts::*;
        use sha2::{Digest, Sha256};
        use std::os::unix::fs::PermissionsExt;
        use std::sync::{Arc, RwLock};

        let root = tempfile::tempdir().expect("state");
        let providers = tempfile::tempdir().expect("providers");
        let adapter = providers.path().join("adapter");
        fs::write(&adapter, b"adapter").expect("adapter");
        fs::set_permissions(&adapter, fs::Permissions::from_mode(0o755)).expect("chmod");
        let provider = WindowsProviderManifest {
            schema: WINDOWS_PROVIDER_MANIFEST_SCHEMA.to_owned(),
            provider_id: "prime.windows.compat".to_owned(),
            provider_revision: 1,
            adapter_path: adapter.display().to_string(),
            formats: vec![ArtifactFormat::Pe32Plus],
            workload_arches: vec!["x86_64".to_owned()],
            limitations: vec![],
        };
        fs::write(
            providers.path().join("provider.json"),
            serde_json::to_vec(&provider).expect("provider json"),
        ).expect("provider manifest");

        let mut pe = vec![0_u8; 128];
        pe[0..2].copy_from_slice(b"MZ");
        pe[0x3c..0x40].copy_from_slice(&0x40_u32.to_le_bytes());
        pe[0x40..0x44].copy_from_slice(b"PE\0\0");
        pe[0x44..0x46].copy_from_slice(&0x8664_u16.to_le_bytes());
        pe[0x58..0x5a].copy_from_slice(&0x20b_u16.to_le_bytes());
        let digest = format!("{:x}", Sha256::digest(&pe));
        let identity = format!("sha256:{digest}");
        let artifact_dir = root.path().join("artifacts/sha256");
        fs::create_dir_all(&artifact_dir).expect("artifact dir");
        fs::write(artifact_dir.join(&digest), &pe).expect("artifact");

        let policy = registry::seal_policy(WorkloadPolicy {
            schema: WORKLOAD_POLICY_SCHEMA.to_owned(),
            policy_id: Uuid::now_v7(),
            revision: 1,
            digest: String::new(),
            class: PolicyClass::ForeignRuntime,
            cpu: CpuPolicy { weight: 100, quota_percent: None },
            memory: MemoryPolicy { max_bytes: Some(256 * 1024 * 1024), swap_max_bytes: Some(0) },
            gpu: GpuPolicy { mode: GpuMode::Deny },
            storage: StoragePolicy { quota_bytes: None, io_weight: 100 },
            process: ProcessPolicy { max_processes: Some(32), max_runtime_seconds: Some(30) },
            network: NetworkPolicy { mode: NetworkMode::Offline, destinations: vec![] },
            filesystem: FilesystemPolicy::default(),
            devices: DevicePolicy::default(),
            secrets: SecretPolicy::default(),
            background: BackgroundPolicy { allowed: false },
            evidence: EvidencePolicy { required: true, classes: vec!["exit".to_owned()] },
        }).expect("seal policy");
        registry::store_policy_revision(root.path(), &policy).expect("store policy");
        registry::select_policy_revision(root.path(), policy.policy_id, 1).expect("select policy");

        let application_id = Uuid::now_v7();
        let profile = registry::seal_profile(ApplicationProfile {
            schema: APPLICATION_PROFILE_SCHEMA.to_owned(),
            application_id,
            profile_revision: 1,
            profile_digest: String::new(),
            display_name: "Windows Fixture".to_owned(),
            artifact: ApplicationArtifact {
                identity,
                format: ArtifactFormat::Pe32Plus,
                runtime_family: RuntimeFamily::Windows,
                workload_arch: Some("x86_64".to_owned()),
            },
            execution_backend: ExecutionBackend::Personality,
            dependencies: vec![],
            workload_policy: PolicyReference {
                policy_id: policy.policy_id,
                policy_revision: 1,
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
        }).expect("seal profile");
        registry::store_profile_revision(root.path(), &profile).expect("store profile");
        registry::select_profile_revision(root.path(), application_id, 1).expect("select profile");

        let host = HostIdentity {
            schema: HOST_IDENTITY_SCHEMA.to_owned(),
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
        };
        let generation = GenerationRecord {
            schema: GENERATION_SCHEMA.to_owned(),
            generation_id: "fixture-generation".to_owned(),
            image_digest: format!("sha256:{}", "1".repeat(64)),
            channel: ReleaseChannel::Lab,
            created_at: "2026-09-10T00:00:00Z".to_owned(),
            source_revision: "fixture".to_owned(),
            state: GenerationState::KnownGood,
            boot_attempts_remaining: None,
            evidence_refs: vec![],
        };
        let storage = StorageInventory {
            schema: STORAGE_INVENTORY_SCHEMA.to_owned(),
            observed_at: "2026-09-10T00:00:00Z".to_owned(),
            mount_namespace_source: "fixture".to_owned(),
            mounts: vec![],
            local_physical_totals: StorageTotals::default(),
            root_mount_id: None,
            generation_accounting: StorageGenerationAccounting {
                current_generation_id: generation.generation_id.clone(),
                current_generation_bytes: None,
                previous_known_good_bytes: None,
                recovery_generation_bytes: None,
                staged_generation_bytes: None,
                limitations: vec![],
            },
            reserve: StorageReserveVisibility {
                policy_configured: false,
                protected_rollback_recovery_bytes: None,
                limitations: vec![],
            },
            pressure: StoragePressure {
                state: StoragePressureState::Unknown,
                available_bytes: None,
                low_threshold_bytes: None,
                critical_threshold_bytes: None,
                limitations: vec![],
            },
            limitations: vec![],
        };
        let state = CoreState {
            host,
            generation,
            hardware: Arc::new(HardwareGraph {
                schema: HARDWARE_GRAPH_SCHEMA.to_owned(),
                revision: 1,
                topology_digest: "sha256:fixture".to_owned(),
                observed_at: "2026-09-10T00:00:00Z".to_owned(),
                inventory: HardwareInventory::default(),
                limitations: vec![],
            }),
            storage: Arc::new(RwLock::new(storage)),
            capabilities: Arc::new(vec![]),
            health_limitations: Arc::new(vec![]),
            state_dir: Arc::new(root.path().to_path_buf()),
            systemd_run: Arc::new(PathBuf::from("/usr/bin/true")),
            windows_provider_dir: Arc::new(providers.path().to_path_buf()),
            storage_mountinfo: Arc::new(PathBuf::from("/nonexistent")),
            storage_policy_file: Arc::new(PathBuf::from("/nonexistent")),
            system_root: Arc::new(PathBuf::from("/")),
            started_at: "2026-09-10T00:00:00Z".to_owned(),
        };

        let evidence = launch_selected(
            &state,
            &ShellLaunchRequest {
                schema: SHELL_LAUNCH_REQUEST_SCHEMA.to_owned(),
                application_id,
            },
        ).expect("Windows Shell launch");
        assert!(matches!(evidence, LaunchEvidence::Windows(_)));
    }

    #[test]
    fn profile_limitations_reject_unknown_compatibility() {
        let profile = ApplicationProfile {
            schema: "prime.application-profile.v1".to_owned(),
            application_id: Uuid::now_v7(),
            profile_revision: 1,
            profile_digest: "sha256:test".to_owned(),
            display_name: "Fixture".to_owned(),
            artifact: prime_contracts::ApplicationArtifact {
                identity: format!("sha256:{}", "0".repeat(64)),
                format: ArtifactFormat::Elf,
                runtime_family: RuntimeFamily::NativeLinux,
                workload_arch: Some("x86_64".to_owned()),
            },
            execution_backend: ExecutionBackend::Native,
            dependencies: Vec::new(),
            workload_policy: prime_contracts::PolicyReference {
                policy_id: Uuid::now_v7(),
                policy_revision: 1,
                policy_digest: "sha256:test".to_owned(),
            },
            permissions: Vec::new(),
            compatibility: prime_contracts::CompatibilityRecord {
                state: MechanicalCompatibilityState::Unknown,
                evidence_refs: Vec::new(),
            },
            revoked: false,
            revocation_reason: None,
            created_at: "2026-08-20T00:00:00Z".to_owned(),
        };
        assert_eq!(profile_limitations(&profile, "x86_64", Path::new("/missing")).len(), 1);
    }
}
