use crate::{exec, identity, launcher, policy, registry};
use prime_contracts::{
    ApplicationProfile, ArtifactFormat, ExecutionBackend, MechanicalCompatibilityState, PolicyClass,
    RuntimeFamily, WindowsProviderManifest, WINDOWS_PROVIDER_MANIFEST_SCHEMA,
};
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use uuid::Uuid;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowsPersonalityError {
    #[error("WINDOWS_PERSONALITY_UNAVAILABLE: {0}")]
    ProviderUnavailable(String),
    #[error("WINDOWS_PROVIDER_INVALID: {0}")]
    InvalidProvider(String),
    #[error("WINDOWS_WORKLOAD_UNSUPPORTED: no compatible provider for {format:?}/{arch}")]
    NoCompatibleProvider { format: ArtifactFormat, arch: String },
    #[error("WINDOWS_PROFILE_MISMATCH: {0}")]
    ProfileMismatch(&'static str),
    #[error("WINDOWS_ARTIFACT_MISMATCH: {0}")]
    ArtifactMismatch(&'static str),
    #[error("WINDOWS_WORKLOAD_ARCH_UNSUPPORTED: W1 requires an x86_64 Prime Host, got {0}")]
    UnsupportedHostArchitecture(String),
    #[error("selected profile policy reference does not match the stored policy")]
    PolicyReferenceMismatch,
    #[error(transparent)]
    Registry(#[from] registry::RegistryError),
    #[error(transparent)]
    Exec(#[from] exec::ExecError),
    #[error(transparent)]
    Policy(#[from] policy::PolicyCompileError),
    #[error(transparent)]
    Identity(#[from] identity::IdentityError),
    #[error("Windows artifact staging failed: {0}")]
    ArtifactStage(String),
    #[error("Windows provider registry I/O failed: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedWindowsProvider {
    pub manifest: WindowsProviderManifest,
    pub manifest_path: PathBuf,
    pub adapter_path: PathBuf,
}

pub fn load_provider(
    provider_dir: &Path,
    format: &ArtifactFormat,
    arch: &str,
) -> Result<ValidatedWindowsProvider, WindowsPersonalityError> {
    if !provider_dir.is_dir() {
        return Err(WindowsPersonalityError::ProviderUnavailable(format!(
            "trusted provider directory is unavailable: {}",
            provider_dir.display()
        )));
    }

    let mut providers = Vec::new();
    let mut saw_manifest = false;
    for entry in fs::read_dir(provider_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        saw_manifest = true;
        if !entry.file_type()?.is_file() {
            return Err(WindowsPersonalityError::InvalidProvider(format!(
                "provider manifest is not a regular file: {}",
                path.display()
            )));
        }
        providers.push(validate_manifest(&path)?);
    }

    if !saw_manifest {
        return Err(WindowsPersonalityError::ProviderUnavailable(format!(
            "no provider manifests found in {}",
            provider_dir.display()
        )));
    }

    providers.retain(|provider| {
        provider.manifest.formats.contains(format)
            && provider.manifest.workload_arches.iter().any(|value| value == arch)
    });
    if providers.is_empty() {
        return Err(WindowsPersonalityError::NoCompatibleProvider {
            format: format.clone(),
            arch: arch.to_owned(),
        });
    }

    providers.sort_by(|left, right| {
        left.manifest
            .provider_id
            .cmp(&right.manifest.provider_id)
            .then_with(|| right.manifest.provider_revision.cmp(&left.manifest.provider_revision))
            .then_with(|| compare_paths(&left.manifest_path, &right.manifest_path))
    });
    Ok(providers.remove(0))
}

fn validate_manifest(path: &Path) -> Result<ValidatedWindowsProvider, WindowsPersonalityError> {
    let raw = fs::read(path)?;
    let manifest: WindowsProviderManifest = serde_json::from_slice(&raw).map_err(|error| {
        WindowsPersonalityError::InvalidProvider(format!(
            "{} is not valid provider JSON: {error}",
            path.display()
        ))
    })?;
    if manifest.schema != WINDOWS_PROVIDER_MANIFEST_SCHEMA {
        return Err(WindowsPersonalityError::InvalidProvider(format!(
            "{} has unsupported schema {}",
            path.display(), manifest.schema
        )));
    }
    if manifest.provider_id.trim().is_empty() {
        return Err(WindowsPersonalityError::InvalidProvider(format!(
            "{} has an empty provider_id",
            path.display()
        )));
    }
    if manifest.provider_revision == 0 {
        return Err(WindowsPersonalityError::InvalidProvider(format!(
            "{} has provider revision zero",
            path.display()
        )));
    }
    if manifest.formats.is_empty() || manifest.workload_arches.is_empty() {
        return Err(WindowsPersonalityError::InvalidProvider(format!(
            "{} declares no supported formats or architectures",
            path.display()
        )));
    }

    let adapter_path = PathBuf::from(&manifest.adapter_path);
    if !adapter_path.is_absolute() {
        return Err(WindowsPersonalityError::InvalidProvider(format!(
            "{} adapter_path is not absolute",
            path.display()
        )));
    }
    let metadata = fs::symlink_metadata(&adapter_path).map_err(|error| {
        WindowsPersonalityError::InvalidProvider(format!(
            "adapter {} cannot be inspected: {error}",
            adapter_path.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(WindowsPersonalityError::InvalidProvider(format!(
            "adapter is not a regular non-symlink file: {}",
            adapter_path.display()
        )));
    }
    if metadata.permissions().mode() & 0o111 == 0 {
        return Err(WindowsPersonalityError::InvalidProvider(format!(
            "adapter is not executable: {}",
            adapter_path.display()
        )));
    }
    let canonical_adapter = fs::canonicalize(&adapter_path).map_err(|error| {
        WindowsPersonalityError::InvalidProvider(format!(
            "adapter {} cannot be canonicalized: {error}",
            adapter_path.display()
        ))
    })?;

    Ok(ValidatedWindowsProvider {
        manifest,
        manifest_path: path.to_path_buf(),
        adapter_path: canonical_adapter,
    })
}

fn compare_paths(left: &Path, right: &Path) -> Ordering {
    left.as_os_str().as_encoded_bytes().cmp(right.as_os_str().as_encoded_bytes())
}


#[derive(Debug, Clone)]
pub struct PreparedWindowsLaunch {
    pub launch_id: Uuid,
    pub profile: ApplicationProfile,
    pub policy_id: Uuid,
    pub policy_revision: u64,
    pub policy_digest: String,
    pub provider: ValidatedWindowsProvider,
    pub staged_artifact_path: PathBuf,
    pub unit_name: String,
    pub runtime_directory_name: String,
    pub requested_at: String,
    pub plan: policy::SystemdEnforcementPlan,
}

pub fn prepare_windows_launch(
    state_dir: &Path,
    provider_dir: &Path,
    application_id: Uuid,
    candidate: &Path,
    host_arch: &str,
) -> Result<PreparedWindowsLaunch, WindowsPersonalityError> {
    if !matches!(host_arch, "x86_64" | "amd64") {
        return Err(WindowsPersonalityError::UnsupportedHostArchitecture(
            host_arch.to_owned(),
        ));
    }
    if !candidate.is_absolute() {
        return Err(WindowsPersonalityError::ArtifactMismatch(
            "artifact path must be absolute",
        ));
    }

    let profile = registry::load_selected_profile(state_dir, application_id)?;
    validate_windows_profile(&profile)?;
    let workload_arch = profile
        .artifact
        .workload_arch
        .as_deref()
        .ok_or(WindowsPersonalityError::ProfileMismatch(
            "Windows workload architecture is unresolved",
        ))?;
    if !matches!(workload_arch, "x86" | "x86_64") {
        return Err(WindowsPersonalityError::ProfileMismatch(
            "W1 supports x86/x86_64 Windows workloads only",
        ));
    }

    let policy_ref = &profile.workload_policy;
    let workload_policy = registry::load_policy_revision(
        state_dir,
        policy_ref.policy_id,
        policy_ref.policy_revision,
    )?;
    if workload_policy.digest != policy_ref.policy_digest {
        return Err(WindowsPersonalityError::PolicyReferenceMismatch);
    }
    if workload_policy.class != PolicyClass::ForeignRuntime {
        return Err(WindowsPersonalityError::ProfileMismatch(
            "W1 Windows Personality requires FOREIGN_RUNTIME policy class",
        ));
    }
    let plan = policy::compile_systemd(&workload_policy)?;

    let candidate_inspection = exec::inspect(candidate, host_arch)?;
    verify_inspection_matches_profile(&candidate_inspection, &profile)?;

    let provider = load_provider(provider_dir, &profile.artifact.format, workload_arch)?;
    let staged_artifact_path = launcher::stage_artifact(
        state_dir,
        candidate,
        &profile.artifact.identity,
        host_arch,
    )
    .map_err(|error| WindowsPersonalityError::ArtifactStage(error.to_string()))?;
    let staged_inspection = exec::inspect(&staged_artifact_path, host_arch)?;
    verify_inspection_matches_profile(&staged_inspection, &profile)?;

    let launch_id = Uuid::now_v7();
    let compact_app_id = profile.application_id.to_string().replace('-', "");
    let compact_launch_id = launch_id.to_string().replace('-', "");
    let unit_name = format!(
        "prime-win-{compact_app_id}-r{}-{compact_launch_id}.service",
        profile.profile_revision
    );
    let runtime_directory_name = format!("prime-win-{compact_launch_id}");

    Ok(PreparedWindowsLaunch {
        launch_id,
        profile,
        policy_id: workload_policy.policy_id,
        policy_revision: workload_policy.revision,
        policy_digest: workload_policy.digest,
        provider,
        staged_artifact_path,
        unit_name,
        runtime_directory_name,
        requested_at: identity::now_rfc3339()?,
        plan,
    })
}

fn validate_windows_profile(profile: &ApplicationProfile) -> Result<(), WindowsPersonalityError> {
    if profile.execution_backend != ExecutionBackend::Personality {
        return Err(WindowsPersonalityError::ProfileMismatch(
            "execution_backend is not PERSONALITY",
        ));
    }
    if profile.artifact.runtime_family != RuntimeFamily::Windows {
        return Err(WindowsPersonalityError::ProfileMismatch(
            "runtime family is not WINDOWS",
        ));
    }
    if !matches!(profile.artifact.format, ArtifactFormat::Pe32 | ArtifactFormat::Pe32Plus) {
        return Err(WindowsPersonalityError::ProfileMismatch(
            "artifact format is not PE32/PE32+",
        ));
    }
    if !profile.dependencies.is_empty() {
        return Err(WindowsPersonalityError::ProfileMismatch(
            "W1 dependency admission is not implemented",
        ));
    }
    if !profile.permissions.is_empty() {
        return Err(WindowsPersonalityError::ProfileMismatch(
            "W1 application permission mediation is not implemented",
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
        return Err(WindowsPersonalityError::ProfileMismatch(
            "mechanical compatibility state does not permit a W1 personality attempt",
        ));
    }
    Ok(())
}

fn verify_inspection_matches_profile(
    inspection: &prime_contracts::ExecInspection,
    profile: &ApplicationProfile,
) -> Result<(), WindowsPersonalityError> {
    if inspection.artifact_identity != profile.artifact.identity {
        return Err(WindowsPersonalityError::ArtifactMismatch(
            "SHA-256 identity differs",
        ));
    }
    if inspection.format != profile.artifact.format {
        return Err(WindowsPersonalityError::ArtifactMismatch("format differs"));
    }
    if inspection.runtime_family != RuntimeFamily::Windows
        || inspection.runtime_family != profile.artifact.runtime_family
    {
        return Err(WindowsPersonalityError::ArtifactMismatch(
            "runtime family differs",
        ));
    }
    if inspection.workload_arch != profile.artifact.workload_arch {
        return Err(WindowsPersonalityError::ArtifactMismatch(
            "workload architecture differs",
        ));
    }
    if inspection.native_compatible {
        return Err(WindowsPersonalityError::ArtifactMismatch(
            "Windows PE must not be marked native-compatible",
        ));
    }
    Ok(())
}
