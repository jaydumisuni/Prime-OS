use prime_contracts::{
    ApplicationProfile, WorkloadPolicy, APPLICATION_PROFILE_SCHEMA, WORKLOAD_POLICY_SCHEMA,
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum RegistryError {
    #[error("registry I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("registry JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("record schema {found} does not match expected {expected}")]
    Schema {
        found: String,
        expected: &'static str,
    },
    #[error("record identifier does not match its registry path")]
    IdentityMismatch,
    #[error("record revision does not match its registry path")]
    RevisionMismatch,
    #[error("record revision must start at 1")]
    InvalidRevision,
    #[error("record digest does not match its typed contents")]
    DigestMismatch,
    #[error("revision already exists")]
    RevisionExists,
    #[error("selected revision is missing or invalid")]
    InvalidSelected,
    #[error("revoked application profile cannot be selected")]
    Revoked,
}

pub fn seal_policy(mut policy: WorkloadPolicy) -> Result<WorkloadPolicy, RegistryError> {
    policy.digest.clear();
    policy.digest = policy_digest(&policy)?;
    Ok(policy)
}

pub fn seal_profile(mut profile: ApplicationProfile) -> Result<ApplicationProfile, RegistryError> {
    profile.profile_digest.clear();
    profile.profile_digest = profile_digest(&profile)?;
    Ok(profile)
}

pub fn verify_policy(policy: &WorkloadPolicy) -> Result<(), RegistryError> {
    if policy.schema != WORKLOAD_POLICY_SCHEMA {
        return Err(RegistryError::Schema {
            found: policy.schema.clone(),
            expected: WORKLOAD_POLICY_SCHEMA,
        });
    }
    if policy.revision == 0 {
        return Err(RegistryError::InvalidRevision);
    }
    if policy.digest != policy_digest(policy)? {
        return Err(RegistryError::DigestMismatch);
    }
    Ok(())
}

pub fn verify_profile(profile: &ApplicationProfile) -> Result<(), RegistryError> {
    if profile.schema != APPLICATION_PROFILE_SCHEMA {
        return Err(RegistryError::Schema {
            found: profile.schema.clone(),
            expected: APPLICATION_PROFILE_SCHEMA,
        });
    }
    if profile.profile_revision == 0 {
        return Err(RegistryError::InvalidRevision);
    }
    if profile.profile_digest != profile_digest(profile)? {
        return Err(RegistryError::DigestMismatch);
    }
    Ok(())
}

pub fn store_policy_revision(root: &Path, policy: &WorkloadPolicy) -> Result<(), RegistryError> {
    verify_policy(policy)?;
    let path = policy_revision_path(root, policy.policy_id, policy.revision);
    create_revision(&path, &serde_json::to_vec_pretty(policy)?)
}

pub fn load_policy_revision(
    root: &Path,
    policy_id: Uuid,
    revision: u64,
) -> Result<WorkloadPolicy, RegistryError> {
    let policy: WorkloadPolicy =
        serde_json::from_slice(&fs::read(policy_revision_path(root, policy_id, revision))?)?;
    if policy.policy_id != policy_id {
        return Err(RegistryError::IdentityMismatch);
    }
    if policy.revision != revision {
        return Err(RegistryError::RevisionMismatch);
    }
    verify_policy(&policy)?;
    Ok(policy)
}

pub fn select_policy_revision(
    root: &Path,
    policy_id: Uuid,
    revision: u64,
) -> Result<(), RegistryError> {
    load_policy_revision(root, policy_id, revision)?;
    write_selected(&policy_root(root, policy_id).join("selected"), revision)
}

pub fn load_selected_policy(root: &Path, policy_id: Uuid) -> Result<WorkloadPolicy, RegistryError> {
    let revision = read_selected(&policy_root(root, policy_id).join("selected"))?;
    load_policy_revision(root, policy_id, revision)
}

pub fn store_profile_revision(
    root: &Path,
    profile: &ApplicationProfile,
) -> Result<(), RegistryError> {
    verify_profile(profile)?;
    let path = profile_revision_path(root, profile.application_id, profile.profile_revision);
    create_revision(&path, &serde_json::to_vec_pretty(profile)?)
}

pub fn load_profile_revision(
    root: &Path,
    application_id: Uuid,
    revision: u64,
) -> Result<ApplicationProfile, RegistryError> {
    let profile: ApplicationProfile = serde_json::from_slice(&fs::read(profile_revision_path(
        root,
        application_id,
        revision,
    ))?)?;
    if profile.application_id != application_id {
        return Err(RegistryError::IdentityMismatch);
    }
    if profile.profile_revision != revision {
        return Err(RegistryError::RevisionMismatch);
    }
    verify_profile(&profile)?;
    Ok(profile)
}

pub fn select_profile_revision(
    root: &Path,
    application_id: Uuid,
    revision: u64,
) -> Result<(), RegistryError> {
    let profile = load_profile_revision(root, application_id, revision)?;
    if profile.revoked {
        return Err(RegistryError::Revoked);
    }
    write_selected(
        &profile_root(root, application_id).join("selected"),
        revision,
    )
}

pub fn load_selected_profile(
    root: &Path,
    application_id: Uuid,
) -> Result<ApplicationProfile, RegistryError> {
    let revision = read_selected(&profile_root(root, application_id).join("selected"))?;
    let profile = load_profile_revision(root, application_id, revision)?;
    if profile.revoked {
        return Err(RegistryError::Revoked);
    }
    Ok(profile)
}

fn policy_digest(policy: &WorkloadPolicy) -> Result<String, RegistryError> {
    let mut canonical = policy.clone();
    canonical.digest.clear();
    digest_json(&canonical)
}

fn profile_digest(profile: &ApplicationProfile) -> Result<String, RegistryError> {
    let mut canonical = profile.clone();
    canonical.profile_digest.clear();
    digest_json(&canonical)
}

fn digest_json<T: Serialize>(value: &T) -> Result<String, RegistryError> {
    let bytes = serde_json::to_vec(value)?;
    Ok(sha256_labelled(&bytes))
}

fn policy_root(root: &Path, id: Uuid) -> PathBuf {
    root.join("policies").join(id.to_string())
}

fn profile_root(root: &Path, id: Uuid) -> PathBuf {
    root.join("applications").join(id.to_string())
}

fn policy_revision_path(root: &Path, id: Uuid, revision: u64) -> PathBuf {
    policy_root(root, id)
        .join("revisions")
        .join(format!("{revision:020}.json"))
}

fn profile_revision_path(root: &Path, id: Uuid, revision: u64) -> PathBuf {
    profile_root(root, id)
        .join("revisions")
        .join(format!("{revision:020}.json"))
}

fn create_revision(path: &Path, bytes: &[u8]) -> Result<(), RegistryError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "revision path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let temp = parent.join(format!(".revision.{}.tmp", Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    match fs::hard_link(&temp, path) {
        Ok(()) => {
            fs::remove_file(&temp)?;
            File::open(parent)?.sync_all()?;
            Ok(())
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(&temp)?;
            Err(RegistryError::RevisionExists)
        }
        Err(error) => {
            let _ = fs::remove_file(&temp);
            Err(error.into())
        }
    }
}

fn write_selected(path: &Path, revision: u64) -> Result<(), RegistryError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "selected path has no parent")
    })?;
    fs::create_dir_all(parent)?;
    fs::set_permissions(parent, fs::Permissions::from_mode(0o700))?;
    let temp = parent.join(format!(".selected.{}.tmp", Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    writeln!(file, "{revision}")?;
    file.sync_all()?;
    if let Err(error) = fs::rename(&temp, path) {
        let _ = fs::remove_file(&temp);
        return Err(error.into());
    }
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn read_selected(path: &Path) -> Result<u64, RegistryError> {
    let value = fs::read_to_string(path)?;
    let revision = value
        .trim()
        .parse::<u64>()
        .map_err(|_| RegistryError::InvalidSelected)?;
    if revision == 0 {
        return Err(RegistryError::InvalidSelected);
    }
    Ok(revision)
}

fn sha256_labelled(bytes: &[u8]) -> String {
    let digest = Sha256::digest(bytes);
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::*;

    fn policy(id: Uuid) -> WorkloadPolicy {
        seal_policy(WorkloadPolicy {
            schema: WORKLOAD_POLICY_SCHEMA.to_owned(),
            policy_id: id,
            revision: 1,
            digest: String::new(),
            class: PolicyClass::UserApp,
            cpu: CpuPolicy {
                weight: 100,
                quota_percent: None,
            },
            memory: MemoryPolicy {
                max_bytes: Some(512 * 1024 * 1024),
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
                max_processes: Some(64),
                max_runtime_seconds: Some(60),
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
                classes: Vec::new(),
            },
        })
        .expect("seal policy")
    }

    fn profile(id: Uuid, policy: &WorkloadPolicy) -> ApplicationProfile {
        seal_profile(ApplicationProfile {
            schema: APPLICATION_PROFILE_SCHEMA.to_owned(),
            application_id: id,
            profile_revision: 1,
            profile_digest: String::new(),
            display_name: "Fixture".to_owned(),
            artifact: ApplicationArtifact {
                identity: "sha256:artifact".to_owned(),
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
        .expect("seal profile")
    }

    #[test]
    fn policy_revision_is_append_only_and_selected_by_exact_revision() {
        let dir = tempfile::tempdir().expect("tempdir");
        let id = Uuid::now_v7();
        let policy = policy(id);
        store_policy_revision(dir.path(), &policy).expect("store");
        assert!(matches!(
            store_policy_revision(dir.path(), &policy),
            Err(RegistryError::RevisionExists)
        ));
        select_policy_revision(dir.path(), id, 1).expect("select");
        assert_eq!(load_selected_policy(dir.path(), id).expect("load"), policy);
    }

    #[test]
    fn tampered_policy_fails_digest_validation() {
        let dir = tempfile::tempdir().expect("tempdir");
        let id = Uuid::now_v7();
        let policy = policy(id);
        store_policy_revision(dir.path(), &policy).expect("store");
        let path = policy_revision_path(dir.path(), id, 1);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("read")).expect("json");
        value["cpu"]["weight"] = serde_json::json!(999);
        fs::write(path, serde_json::to_vec_pretty(&value).expect("serialize")).expect("tamper");
        assert!(matches!(
            load_policy_revision(dir.path(), id, 1),
            Err(RegistryError::DigestMismatch)
        ));
    }

    #[test]
    fn revoked_profile_cannot_be_selected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let policy = policy(Uuid::now_v7());
        let mut profile = profile(Uuid::now_v7(), &policy);
        profile.revoked = true;
        profile.revocation_reason = Some("fixture revocation".to_owned());
        profile = seal_profile(profile).expect("reseal");
        store_profile_revision(dir.path(), &profile).expect("store");
        assert!(matches!(
            select_profile_revision(dir.path(), profile.application_id, 1),
            Err(RegistryError::Revoked)
        ));
    }

    #[test]
    fn selected_profile_revalidates_digest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let policy = policy(Uuid::now_v7());
        let profile = profile(Uuid::now_v7(), &policy);
        store_profile_revision(dir.path(), &profile).expect("store");
        select_profile_revision(dir.path(), profile.application_id, 1).expect("select");
        assert_eq!(
            load_selected_profile(dir.path(), profile.application_id).expect("load"),
            profile
        );
    }
}
