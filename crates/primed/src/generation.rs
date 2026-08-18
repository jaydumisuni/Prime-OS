use prime_contracts::{
    GenerationHealthReport, GenerationRecord, GenerationSeed, GenerationState, HealthStatus,
    GENERATION_HEALTH_SCHEMA, GENERATION_SCHEMA, GENERATION_SEED_SCHEMA,
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
const BOOTC_API_VERSION: &str = "org.containers.bootc/v1";
const BOOTC_KIND: &str = "BootcHost";

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
    #[error("bootc status API identity is {api_version}/{kind}, expected org.containers.bootc/v1/BootcHost")]
    BootcStatusApi { api_version: String, kind: String },
    #[error("bootc status has no booted deployment")]
    BootedDeploymentMissing,
    #[error("bootc status booted deployment has no image identity")]
    BootedImageMissing,
    #[error("bootc reports the booted deployment as incompatible")]
    BootedDeploymentIncompatible,
    #[error("bootc reports a /usr overlay; Prime cannot attest the image-owned generation seed")]
    UsrOverlayPresent,
    #[error("bootc reported unsupported image architecture {0}")]
    UnsupportedReportedArchitecture(String),
    #[error("Prime Host architecture {0} is not supported by generation binding")]
    UnsupportedHostArchitecture(String),
    #[error("bootc reported image architecture {reported}, but Prime Host architecture is {host}")]
    ArchitectureMismatch { reported: String, host: String },
    #[error("persisted generation state is incompatible with the booted generation: {0}")]
    PersistedMismatch(&'static str),
    #[error("generation state transition {from:?} -> {to:?} is not allowed")]
    InvalidStateTransition {
        from: GenerationState,
        to: GenerationState,
    },
    #[error("generation transition evidence reference is empty")]
    EvidenceReference,
    #[error("generation health report is incompatible with the current generation: {0}")]
    HealthBindingMismatch(&'static str),
    #[error("generation health report is incomplete")]
    HealthIncomplete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootedImageIdentity {
    pub image_digest: String,
    pub architecture: String,
}

#[derive(Debug, Deserialize)]
struct BootcHost {
    #[serde(rename = "apiVersion")]
    api_version: String,
    kind: String,
    status: BootcHostStatus,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BootcHostStatus {
    booted: Option<BootcBootEntry>,
    usr_overlay: Option<serde_json::Value>,
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

pub fn begin_health_proving(
    state_dir: &Path,
    current: &GenerationRecord,
    evidence_ref: &str,
) -> Result<GenerationRecord, GenerationError> {
    validate_record(current)?;
    if evidence_ref.trim().is_empty() {
        return Err(GenerationError::EvidenceReference);
    }

    match &current.state {
        GenerationState::HealthProving | GenerationState::KnownGood => Ok(current.clone()),
        GenerationState::BootTry => {
            let mut updated = current.clone();
            updated.state = GenerationState::HealthProving;
            if !updated.evidence_refs.iter().any(|item| item == evidence_ref) {
                updated.evidence_refs.push(evidence_ref.to_owned());
            }
            persist_current(state_dir, &updated)?;
            Ok(updated)
        }
        _ => Err(GenerationError::InvalidStateTransition {
            from: current.state.clone(),
            to: GenerationState::HealthProving,
        }),
    }
}

pub fn promote_known_good(
    state_dir: &Path,
    current: &GenerationRecord,
    report: &GenerationHealthReport,
) -> Result<GenerationRecord, GenerationError> {
    validate_record(current)?;
    validate_health_report(current, report)?;

    if current.state == GenerationState::KnownGood {
        return Ok(current.clone());
    }
    if current.state != GenerationState::HealthProving {
        return Err(GenerationError::InvalidStateTransition {
            from: current.state.clone(),
            to: GenerationState::KnownGood,
        });
    }
    if !report.all_required_ready() {
        return Err(GenerationError::HealthIncomplete);
    }

    let evidence_id = Uuid::now_v7();
    let evidence_dir = state_dir.join("evidence/generation-health");
    fs::create_dir_all(&evidence_dir)?;
    fs::set_permissions(&evidence_dir, fs::Permissions::from_mode(0o700))?;
    let evidence_path = evidence_dir.join(format!("{evidence_id}.json"));
    write_new_json(&evidence_path, report, 0o600)?;
    File::open(&evidence_dir)?.sync_all()?;

    let mut updated = current.clone();
    updated.state = GenerationState::KnownGood;
    updated.boot_attempts_remaining = None;
    updated
        .evidence_refs
        .push(format!("{GENERATION_HEALTH_SCHEMA}:{evidence_id}"));
    persist_current(state_dir, &updated)?;
    Ok(updated)
}

pub fn health_status(record: &GenerationRecord) -> HealthStatus {
    match &record.state {
        GenerationState::KnownGood => HealthStatus::Healthy,
        GenerationState::Rejected => HealthStatus::Failed,
        GenerationState::RolledBack | GenerationState::Recovery => HealthStatus::Degraded,
        GenerationState::Staged | GenerationState::BootTry | GenerationState::HealthProving => {
            HealthStatus::Unknown
        }
    }
}

pub fn health_limitations(record: &GenerationRecord) -> Vec<String> {
    match &record.state {
        GenerationState::KnownGood => Vec::new(),
        GenerationState::Staged => vec![
            "Current generation is STAGED and has not entered boot health proof".to_owned(),
        ],
        GenerationState::BootTry => vec![
            "Current generation is BOOT_TRY and has not entered P1 health proving".to_owned(),
        ],
        GenerationState::HealthProving => vec![
            "Current generation is HEALTH_PROVING and has not earned KNOWN_GOOD".to_owned(),
        ],
        GenerationState::Rejected => vec!["Current generation is REJECTED".to_owned()],
        GenerationState::RolledBack => vec!["Current generation is ROLLED_BACK".to_owned()],
        GenerationState::Recovery => vec!["Current generation is in RECOVERY state".to_owned()],
    }
}

pub fn parse_bootc_status(
    bytes: &[u8],
    host_arch: &str,
) -> Result<BootedImageIdentity, GenerationError> {
    let host: BootcHost = serde_json::from_slice(bytes)?;
    if host.api_version != BOOTC_API_VERSION || host.kind != BOOTC_KIND {
        return Err(GenerationError::BootcStatusApi {
            api_version: host.api_version,
            kind: host.kind,
        });
    }
    if host.status.usr_overlay.is_some() {
        return Err(GenerationError::UsrOverlayPresent);
    }
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
    let reported_arch = normalize_arch(&image.architecture).ok_or_else(|| {
        GenerationError::UnsupportedReportedArchitecture(image.architecture.clone())
    })?;
    let host_normalized = normalize_arch(host_arch)
        .ok_or_else(|| GenerationError::UnsupportedHostArchitecture(host_arch.to_owned()))?;
    if reported_arch != host_normalized {
        return Err(GenerationError::ArchitectureMismatch {
            reported: image.architecture,
            host: host_arch.to_owned(),
        });
    }
    Ok(BootedImageIdentity {
        image_digest: image.image_digest,
        architecture: host_normalized.to_owned(),
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
    persist_current(state_dir, &record)?;
    Ok(record)
}

fn persist_current(
    state_dir: &Path,
    record: &GenerationRecord,
) -> Result<(), GenerationError> {
    write_atomic_json(
        &state_dir.join("generations/current.json"),
        record,
        0o600,
    )
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

fn validate_health_report(
    current: &GenerationRecord,
    report: &GenerationHealthReport,
) -> Result<(), GenerationError> {
    if report.schema != GENERATION_HEALTH_SCHEMA {
        return Err(GenerationError::Schema {
            found: report.schema.clone(),
            expected: GENERATION_HEALTH_SCHEMA,
        });
    }
    if report.generation_id != current.generation_id {
        return Err(GenerationError::HealthBindingMismatch(
            "generation ID differs",
        ));
    }
    if report.image_digest != current.image_digest {
        return Err(GenerationError::HealthBindingMismatch(
            "image digest differs",
        ));
    }
    if report.observed_at.trim().is_empty() {
        return Err(GenerationError::HealthBindingMismatch(
            "observed_at is empty",
        ));
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
        return Err(GenerationError::PersistedMismatch(
            "booted image digest differs",
        ));
    }
    if persisted.channel != seed.channel {
        return Err(GenerationError::PersistedMismatch(
            "release channel differs",
        ));
    }
    if persisted.created_at != seed.created_at {
        return Err(GenerationError::PersistedMismatch("created_at differs"));
    }
    if persisted.source_revision != seed.source_revision {
        return Err(GenerationError::PersistedMismatch(
            "source revision differs",
        ));
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

fn normalize_arch(value: &str) -> Option<&'static str> {
    match value.trim().to_ascii_lowercase().as_str() {
        "amd64" | "x86_64" => Some("x86_64"),
        "arm64" | "aarch64" => Some("aarch64"),
        "ppc64le" => Some("ppc64le"),
        "s390x" => Some("s390x"),
        _ => None,
    }
}

fn write_new_json<T: serde::Serialize>(
    path: &Path,
    value: &T,
    mode: u32,
) -> Result<(), GenerationError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(mode)
        .open(path)?;
    file.write_all(&serde_json::to_vec_pretty(value)?)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::set_permissions(path, fs::Permissions::from_mode(mode))?;
    Ok(())
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

    fn health_report(record: &GenerationRecord) -> GenerationHealthReport {
        GenerationHealthReport {
            schema: GENERATION_HEALTH_SCHEMA.to_owned(),
            generation_id: record.generation_id.clone(),
            image_digest: record.image_digest.clone(),
            observed_at: "2026-08-18T21:00:00Z".to_owned(),
            core_interface_ready: true,
            host_identity_ready: true,
            hardware_baseline_ready: true,
            shell_ready: true,
            recovery_ready: true,
            limitations: Vec::new(),
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
        let bytes = br#"{"apiVersion":"org.containers.bootc/v1","kind":"BootcHost","status":{"booted":null,"usrOverlay":null}}"#;
        assert!(matches!(
            parse_bootc_status(bytes, "x86_64"),
            Err(GenerationError::BootedDeploymentMissing)
        ));
    }

    #[test]
    fn missing_booted_image_is_rejected() {
        let bytes = br#"{"apiVersion":"org.containers.bootc/v1","kind":"BootcHost","status":{"booted":{"image":null,"incompatible":false},"usrOverlay":null}}"#;
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
    fn usr_overlay_is_rejected() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&bootc_json(&digest('b'), "amd64", false)).expect("json");
        value["status"]["usrOverlay"] = serde_json::json!({
            "accessMode": "readWrite",
            "persistence": "persistent"
        });
        assert!(matches!(
            parse_bootc_status(value.to_string().as_bytes(), "x86_64"),
            Err(GenerationError::UsrOverlayPresent)
        ));
    }

    #[test]
    fn bootc_api_identity_must_match_v1() {
        let mut value: serde_json::Value =
            serde_json::from_slice(&bootc_json(&digest('b'), "amd64", false)).expect("json");
        value["apiVersion"] = serde_json::json!("org.containers.bootc/v2");
        assert!(matches!(
            parse_bootc_status(value.to_string().as_bytes(), "x86_64"),
            Err(GenerationError::BootcStatusApi { .. })
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
    fn unknown_architecture_is_rejected_not_normalized_together() {
        assert!(matches!(
            parse_bootc_status(&bootc_json(&digest('b'), "mystery", false), "mystery"),
            Err(GenerationError::UnsupportedReportedArchitecture(_))
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
        assert_eq!(health_status(&record), HealthStatus::Unknown);
        assert!(!health_limitations(&record).is_empty());
    }

    #[test]
    fn boot_try_enters_health_proving_without_becoming_known_good() {
        let dir = tempfile::tempdir().expect("tempdir");
        let booted = BootedImageIdentity {
            image_digest: digest('b'),
            architecture: "x86_64".to_owned(),
        };
        let record = bind_seed(dir.path(), &seed(), &booted).expect("bind");
        let proving = begin_health_proving(dir.path(), &record, "prime.core.socket.bound.v1")
            .expect("health proving");
        assert_eq!(proving.state, GenerationState::HealthProving);
        assert_eq!(proving.boot_attempts_remaining, Some(3));
        assert!(proving
            .evidence_refs
            .iter()
            .any(|item| item == "prime.core.socket.bound.v1"));
        assert_eq!(health_status(&proving), HealthStatus::Unknown);
    }

    #[test]
    fn incomplete_health_report_cannot_promote_known_good() {
        let dir = tempfile::tempdir().expect("tempdir");
        let booted = BootedImageIdentity {
            image_digest: digest('b'),
            architecture: "x86_64".to_owned(),
        };
        let record = bind_seed(dir.path(), &seed(), &booted).expect("bind");
        let proving = begin_health_proving(dir.path(), &record, "prime.core.socket.bound.v1")
            .expect("health proving");
        let mut report = health_report(&proving);
        report.shell_ready = false;
        report
            .limitations
            .push("Prime Shell is not ready".to_owned());
        assert!(matches!(
            promote_known_good(dir.path(), &proving, &report),
            Err(GenerationError::HealthIncomplete)
        ));
        let persisted: GenerationRecord = serde_json::from_slice(
            &fs::read(dir.path().join("generations/current.json")).expect("current"),
        )
        .expect("record");
        assert_eq!(persisted.state, GenerationState::HealthProving);
    }

    #[test]
    fn complete_health_report_promotes_exact_generation_and_persists_evidence() {
        let dir = tempfile::tempdir().expect("tempdir");
        let booted = BootedImageIdentity {
            image_digest: digest('b'),
            architecture: "x86_64".to_owned(),
        };
        let record = bind_seed(dir.path(), &seed(), &booted).expect("bind");
        let proving = begin_health_proving(dir.path(), &record, "prime.core.socket.bound.v1")
            .expect("health proving");
        let report = health_report(&proving);
        let known_good = promote_known_good(dir.path(), &proving, &report).expect("known good");
        assert_eq!(known_good.state, GenerationState::KnownGood);
        assert_eq!(known_good.boot_attempts_remaining, None);
        assert_eq!(health_status(&known_good), HealthStatus::Healthy);
        assert!(health_limitations(&known_good).is_empty());
        assert_eq!(
            fs::read_dir(dir.path().join("evidence/generation-health"))
                .expect("evidence dir")
                .count(),
            1
        );
    }

    #[test]
    fn health_report_cannot_promote_a_different_image() {
        let dir = tempfile::tempdir().expect("tempdir");
        let booted = BootedImageIdentity {
            image_digest: digest('b'),
            architecture: "x86_64".to_owned(),
        };
        let record = bind_seed(dir.path(), &seed(), &booted).expect("bind");
        let proving = begin_health_proving(dir.path(), &record, "prime.core.socket.bound.v1")
            .expect("health proving");
        let mut report = health_report(&proving);
        report.image_digest = digest('c');
        assert!(matches!(
            promote_known_good(dir.path(), &proving, &report),
            Err(GenerationError::HealthBindingMismatch(_))
        ));
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
