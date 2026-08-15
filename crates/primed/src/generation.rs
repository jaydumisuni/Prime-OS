use prime_contracts::{
    GenerationRecord, GenerationSeed, GenerationState, GENERATION_SCHEMA, GENERATION_SEED_SCHEMA,
};
use serde::Deserialize;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use std::process::{Command, Stdio};
use thiserror::Error;
use uuid::Uuid;

const MAX_BOOTC_STATUS_BYTES: u64 = 1024 * 1024;
const BOOTC_STATUS_EVIDENCE: &str = "bootc.status.v1";

#[derive(Debug, Error)]
pub enum GenerationError {
    #[error("generation I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("generation JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("generation schema is {found}, expected {expected}")]
    Schema {
        found: String,
        expected: &'static str,
    },
    #[error("generation seed ID is empty")]
    GenerationId,
    #[error("generation source revision is empty")]
    SourceRevision,
    #[error("generation created_at is empty")]
    CreatedAt,
    #[error("generation seed base image digest must be canonical sha256")]
    BaseImageDigest,
    #[error("generation image digest must be canonical sha256")]
    ImageDigest,
    #[error("generation seed boot_attempt_limit must be greater than zero")]
    BootAttemptLimit,
    #[error("bootc status command exited unsuccessfully with code {0:?}")]
    BootcStatusCommand(Option<i32>),
    #[error("bootc status output exceeded the P1 limit")]
    BootcStatusTooLarge,
    #[error("bootc status has no booted deployment")]
    BootedDeploymentMissing,
    #[error("bootc status booted deployment has no image identity")]
    BootedImageMissing,
    #[error("bootc reports the booted deployment as incompatible")]
    BootedDeploymentIncompatible,
    #[error("bootc reported image architecture {reported}, but Prime Host architecture is {host}")]
    ArchitectureMismatch { reported: String, host: String },
    #[error("persisted generation state is incompatible with the booted generation: {0}")]
    PersistedMismatch(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootedImageIdentity {
    pub image_digest: String,
    pub architecture: String,
}

#[derive(Debug, Deserialize)]
struct BootcHost {
    status: BootcHostStatus,
}

#[derive(Debug, Deserialize)]
struct BootcHostStatus {
    booted: Option<BootcBootEntry>,
}

#[derive(Debug, Deserialize)]
struct BootcBootEntry {
    image: Option<BootcImageStatus>,
    #[serde(default)]
    incompatible: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootcImageStatus {
    image_digest: String,
    architecture: String,
}

pub fn load_seed(path: &Path) -> Result<GenerationSeed, GenerationError> {
    let bytes = fs::read(path)?;
    let seed: GenerationSeed = serde_json::from_slice(&bytes)?;
    validate_seed(&seed)?;
    Ok(seed)
}

pub fn load_or_bind(
    seed_path: &Path,
    bootc_path: &Path,
    state_dir: &Path,
    host_arch: &str,
) -> Result<GenerationRecord, GenerationError> {
    let seed = load_seed(seed_path)?;
    let booted = read_bootc_status(bootc_path, host_arch)?;
    bind_seed(state_dir, &seed, &booted)
}

pub fn parse_bootc_status(
    bytes: &[u8],
    host_arch: &str,
) -> Result<BootedImageIdentity, GenerationError> {
    let host: BootcHost = serde_json::from_slice(bytes)?;
    let booted = host
        .status
        .booted
        .ok_or(GenerationError::BootedDeploymentMissing)?;
    if booted.incompatible {
        return Err(GenerationError::BootedDeploymentIncompatible);
    }
    let image = booted.image.ok_or(GenerationError::BootedImageMissing)?;
    if !canonical_sha256(&image.image_digest) {
        return Err(GenerationError::ImageDigest);
    }
    if normalize_arch(&image.architecture) != normalize_arch(host_arch) {
        return Err(GenerationError::ArchitectureMismatch {
            reported: image.architecture,
            host: host_arch.to_owned(),
        });
    }
    Ok(BootedImageIdentity {
        image_digest: image.image_digest,
        architecture: normalize_arch(host_arch).to_owned(),
    })
}

fn read_bootc_status(
    bootc_path: &Path,
    host_arch: &str,
) -> Result<BootedImageIdentity, GenerationError> {
    let mut child = Command::new(bootc_path)
        .args(["status", "--format=json", "--format-version=1"])
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::BrokenPipe,
            "bootc status stdout pipe was not available",
        )
    })?;
    let mut limited = stdout.take(MAX_BOOTC_STATUS_BYTES + 1);
    let mut bytes = Vec::new();
    limited.read_to_end(&mut bytes)?;
    let status = child.wait()?;
    if !status.success() {
        return Err(GenerationError::BootcStatusCommand(status.code()));
    }
    if bytes.len() as u64 > MAX_BOOTC_STATUS_BYTES {
        return Err(GenerationError::BootcStatusTooLarge);
    }
    parse_bootc_status(&bytes, host_arch)
}

fn bind_seed(
    state_dir: &Path,
    seed: &GenerationSeed,
    booted: &BootedImageIdentity,
) -> Result<GenerationRecord, GenerationError> {
    validate_seed(seed)?;
    if !canonical_sha256(&booted.image_digest) {
        return Err(GenerationError::ImageDigest);
    }

    let generations_dir = state_dir.join("generations");
    let current_path = generations_dir.join("current.json");
    match fs::read(&current_path) {
        Ok(bytes) => {
            let persisted: GenerationRecord = serde_json::from_slice(&bytes)?;
            validate_record(&persisted)?;
            validate_persisted_binding(&persisted, seed, booted)?;
            return Ok(persisted);
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }

    let record = GenerationRecord {
        schema: GENERATION_SCHEMA.to_owned(),
        generation_id: seed.generation_id.clone(),
        image_digest: booted.image_digest.clone(),
        channel: seed.channel.clone(),
        created_at: seed.created_at.clone(),
        source_revision: seed.source_revision.clone(),
        state: GenerationState::BootTry,
        boot_attempts_remaining: Some(seed.boot_attempt_limit),
        evidence_refs: vec![BOOTC_STATUS_EVIDENCE.to_owned()],
    };
    fs::create_dir_all(&generations_dir)?;
    fs::set_permissions(&generations_dir, fs::Permissions::from_mode(0o700))?;
    write_atomic_json(&current_path, &record, 0o600)?;
    Ok(record)
}

fn validate_seed(seed: &GenerationSeed) -> Result<(), GenerationError> {
    if seed.schema != GENERATION_SEED_SCHEMA {
        return Err(GenerationError::Schema {
            found: seed.schema.clone(),
            expected: GENERATION_SEED_SCHEMA,
        });
    }
    if seed.generation_id.trim().is_empty() {
        return Err(GenerationError::GenerationId);
    }
    if seed.source_revision.trim().is_empty() {
        return Err(GenerationError::SourceRevision);
    }
    if seed.created_at.trim().is_empty() {
        return Err(GenerationError::CreatedAt);
    }
    if !canonical_sha256(&seed.base_image_digest) {
        return Err(GenerationError::BaseImageDigest);
    }
    if seed.boot_attempt_limit == 0 {
        return Err(GenerationError::BootAttemptLimit);
    }
    Ok(())
}

fn validate_record(record: &GenerationRecord) -> Result<(), GenerationError> {
    if record.schema != GENERATION_SCHEMA {
        return Err(GenerationError::Schema {
            found: record.schema.clone(),
            expected: GENERATION_SCHEMA,
        });
    }
    if record.generation_id.trim().is_empty() {
        return Err(GenerationError::GenerationId);
    }
    if !canonical_sha256(&record.image_digest) {
        return Err(GenerationError::ImageDigest);
    }
    if record.source_revision.trim().is_empty() {
        return Err(GenerationError::SourceRevision);
    }
    if record.created_at.trim().is_empty() {
        return Err(GenerationError::CreatedAt);
    }
    Ok(())
}

fn validate_persisted_binding(
    persisted: &GenerationRecord,
    seed: &GenerationSeed,
    booted: &BootedImageIdentity,
) -> Result<(), GenerationError> {
    if persisted.generation_id != seed.generation_id {
        return Err(GenerationError::PersistedMismatch("generation ID differs"));
    }
    if persisted.image_digest != booted.image_digest {
        return Err(GenerationError::PersistedMismatch("booted image digest differs"));
    }
    if persisted.channel != seed.channel {
        return Err(GenerationError::PersistedMismatch("release channel differs"));
    }
    if persisted.created_at != seed.created_at {
        return Err(GenerationError::PersistedMismatch("created_at differs"));
    }
    if persisted.source_revision != seed.source_revision {
        return Err(GenerationError::PersistedMismatch("source revision differs"));
    }
    Ok(())
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

fn normalize_arch(value: &str) -> &str {
    match value.trim().to_ascii_lowercase().as_str() {
        "amd64" | "x86_64" => "x86_64",
        "arm64" | "aarch64" => "aarch64",
        "ppc64le" => "ppc64le",
        "s390x" => "s390x",
        _ => "unknown",
    }
}

fn write_atomic_json<T: serde::Serialize>(
    path: &Path,
    value: &T,
    mode: u32,
) -> Result<(), GenerationError> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "generation state path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;
    let temp_path = parent.join(format!(".generation.{}.tmp", Uuid::now_v7()));
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(&temp_path)?;
    temp.write_all(&serde_json::to_vec_pretty(value)?)?;
    temp.write_all(b"\n")?;
    temp.sync_all()?;
    fs::set_permissions(&temp_path, fs::Permissions::from_mode(mode))?;
    fs::rename(&temp_path, path)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::ReleaseChannel;

    fn digest(fill: char) -> String {
        format!("sha256:{}", fill.to_string().repeat(64))
    }

    fn seed() -> GenerationSeed {
        GenerationSeed {
            schema: GENERATION_SEED_SCHEMA.to_owned(),
            generation_id: "prime-gen-test".to_owned(),
            channel: ReleaseChannel::Lab,
            created_at: "2026-08-15T08:00:00Z".to_owned(),
            source_revision: "abcdef123456".to_owned(),
            base_image_digest: digest('a'),
            boot_attempt_limit: 3,
        }
    }

    fn bootc_json(image_digest: &str, architecture: &str, incompatible: bool) -> Vec<u8> {
        serde_json::json!({
            "apiVersion": "org.containers.bootc/v1",
            "kind": "BootcHost",
            "status": {
                "booted": {
                    "image": {
                        "image": {
                            "image": "example.invalid/prime:testing",
                            "transport": "registry"
                        },
                        "version": null,
                        "timestamp": null,
                        "imageDigest": image_digest,
                        "architecture": architecture
                    },
                    "cachedUpdate": null,
                    "incompatible": incompatible,
                    "pinned": false,
                    "softRebootCapable": false,
                    "downloadOnly": false,
                    "store": null,
                    "ostree": null,
                    "composefs": null
                },
                "staged": null,
                "rollback": null,
                "rollbackQueued": false,
                "type": "bootcHost",
                "usrOverlay": null,
                "readOnly": false
            }
        })
        .to_string()
        .into_bytes()
    }

    #[test]
    fn bootc_v1_status_binds_exact_booted_digest() {
        let expected = digest('b');
        let parsed = parse_bootc_status(&bootc_json(&expected, "amd64", false), "x86_64")
            .expect("valid bootc status");
        assert_eq!(parsed.image_digest, expected);
        assert_eq!(parsed.architecture, "x86_64");
    }

    #[test]
    fn missing_booted_deployment_is_rejected() {
        let bytes = br#"{"status":{"booted":null}}"#;
        assert!(matches!(
            parse_bootc_status(bytes, "x86_64"),
            Err(GenerationError::BootedDeploymentMissing)
        ));
    }

    #[test]
    fn missing_booted_image_is_rejected() {
        let bytes = br#"{"status":{"booted":{"image":null,"incompatible":false}}}"#;
        assert!(matches!(
            parse_bootc_status(bytes, "x86_64"),
            Err(GenerationError::BootedImageMissing)
        ));
    }

    #[test]
    fn incompatible_booted_deployment_is_rejected() {
        assert!(matches!(
            parse_bootc_status(&bootc_json(&digest('b'), "amd64", true), "x86_64"),
            Err(GenerationError::BootedDeploymentIncompatible)
        ));
    }

    #[test]
    fn booted_architecture_must_match_host() {
        assert!(matches!(
            parse_bootc_status(&bootc_json(&digest('b'), "arm64", false), "x86_64"),
            Err(GenerationError::ArchitectureMismatch { .. })
        ));
    }

    #[test]
    fn seed_requires_pinned_base_digest_and_positive_attempt_limit() {
        let mut invalid = seed();
        invalid.base_image_digest = "fedora:44".to_owned();
        assert!(matches!(
            validate_seed(&invalid),
            Err(GenerationError::BaseImageDigest)
        ));
        let mut invalid = seed();
        invalid.boot_attempt_limit = 0;
        assert!(matches!(
            validate_seed(&invalid),
            Err(GenerationError::BootAttemptLimit)
        ));
    }

    #[test]
    fn first_binding_starts_boot_try_and_persists() {
        let dir = tempfile::tempdir().expect("tempdir");
        let seed = seed();
        let booted = BootedImageIdentity {
            image_digest: digest('b'),
            architecture: "x86_64".to_owned(),
        };
        let record = bind_seed(dir.path(), &seed, &booted).expect("bind");
        assert_eq!(record.state, GenerationState::BootTry);
        assert_eq!(record.boot_attempts_remaining, Some(3));
        assert_eq!(record.image_digest, booted.image_digest);
        assert!(dir.path().join("generations/current.json").is_file());
    }

    #[test]
    fn persisted_state_is_reused_when_binding_matches() {
        let dir = tempfile::tempdir().expect("tempdir");
        let seed = seed();
        let booted = BootedImageIdentity {
            image_digest: digest('b'),
            architecture: "x86_64".to_owned(),
        };
        let mut persisted = bind_seed(dir.path(), &seed, &booted).expect("bind");
        persisted.state = GenerationState::HealthProving;
        persisted.boot_attempts_remaining = Some(2);
        write_atomic_json(
            &dir.path().join("generations/current.json"),
            &persisted,
            0o600,
        )
        .expect("persist state transition");
        let loaded = bind_seed(dir.path(), &seed, &booted).expect("reuse");
        assert_eq!(loaded.state, GenerationState::HealthProving);
        assert_eq!(loaded.boot_attempts_remaining, Some(2));
    }

    #[test]
    fn persisted_conflicting_image_digest_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let seed = seed();
        let booted = BootedImageIdentity {
            image_digest: digest('b'),
            architecture: "x86_64".to_owned(),
        };
        bind_seed(dir.path(), &seed, &booted).expect("bind");
        let changed = BootedImageIdentity {
            image_digest: digest('c'),
            architecture: "x86_64".to_owned(),
        };
        assert!(matches!(
            bind_seed(dir.path(), &seed, &changed),
            Err(GenerationError::PersistedMismatch(_))
        ));
    }
}
