use crate::{exec, launcher, registry, CoreState};
use prime_contracts::{
    ApplicationEntry, ApplicationProfile, ApplicationsProjection, ArtifactFormat, ExecutionBackend,
    MechanicalCompatibilityState, NativeLaunchEvidence, NativeLaunchRequest, RuntimeFamily,
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
    Launch(#[from] launcher::LaunchError),
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
) -> Result<NativeLaunchEvidence, ShellApiError> {
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

    launcher::launch_native(
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
    .map_err(Into::into)
}

fn application_entry(state: &CoreState, profile: ApplicationProfile) -> ApplicationEntry {
    let mut limitations = profile_limitations(&profile, &state.host.host_arch);
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

fn profile_limitations(profile: &ApplicationProfile, host_arch: &str) -> Vec<String> {
    let mut limitations = Vec::new();
    if profile.revoked {
        limitations.push(
            profile
                .revocation_reason
                .clone()
                .unwrap_or_else(|| "Selected Application Profile is revoked".to_owned()),
        );
    }
    if profile.execution_backend != ExecutionBackend::Native {
        limitations.push("P1 Orb launch supports NATIVE execution profiles only".to_owned());
    }
    if profile.artifact.format != ArtifactFormat::Elf {
        limitations.push("P1 native launch requires an ELF artifact".to_owned());
    }
    if profile.artifact.runtime_family != RuntimeFamily::NativeLinux {
        limitations.push("P1 native launch requires NATIVE_LINUX runtime family".to_owned());
    }
    match profile.artifact.workload_arch.as_deref() {
        Some(arch) if arch == host_arch => {}
        Some(_) => limitations
            .push("Application workload architecture does not match this Host".to_owned()),
        None => limitations.push("Application workload architecture is unresolved".to_owned()),
    }
    if !profile.dependencies.is_empty() {
        limitations.push("P1 dependency admission is not implemented".to_owned());
    }
    if !profile.permissions.is_empty() {
        limitations.push("P1 application permission mediation is not implemented".to_owned());
    }
    if matches!(
        profile.compatibility.state,
        MechanicalCompatibilityState::Unknown
            | MechanicalCompatibilityState::Broken
            | MechanicalCompatibilityState::Unsupported
            | MechanicalCompatibilityState::RequiresVm
            | MechanicalCompatibilityState::RequiresRemoteProvider
    ) {
        limitations
            .push("Mechanical compatibility state does not permit a native attempt".to_owned());
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
        assert_eq!(profile_limitations(&profile, "x86_64").len(), 1);
    }
}
