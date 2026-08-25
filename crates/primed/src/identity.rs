use prime_contracts::{
    FingerprintConfidence, HardwareFingerprint, HostIdentity, HOST_IDENTITY_SCHEMA,
};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use thiserror::Error;
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum IdentityError {
    #[error("identity I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("identity JSON is invalid: {0}")]
    Json(#[from] serde_json::Error),
    #[error("identity schema is {found}, expected {expected}")]
    Schema {
        found: String,
        expected: &'static str,
    },
    #[error("could not format identity timestamp: {0}")]
    Time(#[from] time::error::Format),
    #[error("enrolled hardware fingerprint does not match the observed machine")]
    HardwareMismatch,
}

pub fn now_rfc3339() -> Result<String, IdentityError> {
    Ok(OffsetDateTime::now_utc().format(&Rfc3339)?)
}

pub fn load_or_create(path: &Path) -> Result<HostIdentity, IdentityError> {
    if path.exists() {
        return load(path);
    }

    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "identity path has no parent")
    })?;
    fs::create_dir_all(parent)?;

    let identity = HostIdentity {
        schema: HOST_IDENTITY_SCHEMA.to_owned(),
        host_id: Uuid::now_v7(),
        lineage_id: Uuid::now_v7(),
        created_at: now_rfc3339()?,
        host_arch: std::env::consts::ARCH.to_owned(),
        hardware_fingerprint: HardwareFingerprint {
            algorithm: "sha256".to_owned(),
            digest: None,
            confidence: FingerprintConfidence::Unprobed,
            observed_at: None,
        },
        rebind_revision: 0,
        supersedes_host_id: None,
    };

    let bytes = serde_json::to_vec_pretty(&identity)?;
    let temp_path = parent.join(format!(".host.json.{}.tmp", Uuid::now_v7()));
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp_path)?;
    temp.write_all(&bytes)?;
    temp.write_all(b"\n")?;
    temp.sync_all()?;

    match fs::hard_link(&temp_path, path) {
        Ok(()) => {
            fs::remove_file(&temp_path)?;
            File::open(parent)?.sync_all()?;
            Ok(identity)
        }
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
            fs::remove_file(&temp_path)?;
            load(path)
        }
        Err(error) => {
            let _ = fs::remove_file(&temp_path);
            Err(error.into())
        }
    }
}

pub fn load(path: &Path) -> Result<HostIdentity, IdentityError> {
    let bytes = fs::read(path)?;
    let identity: HostIdentity = serde_json::from_slice(&bytes)?;
    if identity.schema != HOST_IDENTITY_SCHEMA {
        return Err(IdentityError::Schema {
            found: identity.schema,
            expected: HOST_IDENTITY_SCHEMA,
        });
    }
    Ok(identity)
}

pub fn reconcile_fingerprint(
    path: &Path,
    mut identity: HostIdentity,
    candidate: &HardwareFingerprint,
    observed_at: &str,
) -> Result<HostIdentity, IdentityError> {
    if !matches!(
        &candidate.confidence,
        FingerprintConfidence::High | FingerprintConfidence::Medium
    ) {
        return Ok(identity);
    }
    let Some(candidate_digest) = candidate.digest.as_ref() else {
        return Ok(identity);
    };

    match identity.hardware_fingerprint.digest.as_ref() {
        None => {
            identity.hardware_fingerprint = HardwareFingerprint {
                algorithm: candidate.algorithm.clone(),
                digest: Some(candidate_digest.clone()),
                confidence: candidate.confidence.clone(),
                observed_at: Some(observed_at.to_owned()),
            };
            replace_atomic(path, &serde_json::to_vec_pretty(&identity)?)?;
            Ok(identity)
        }
        Some(enrolled) if enrolled == candidate_digest => Ok(identity),
        Some(_) => Err(IdentityError::HardwareMismatch),
    }
}

fn replace_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(io::ErrorKind::InvalidInput, "identity path has no parent")
    })?;
    let temp_path = parent.join(format!(".host.json.{}.tmp", Uuid::now_v7()));
    let mut temp = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp_path)?;
    temp.write_all(bytes)?;
    temp.write_all(b"\n")?;
    temp.sync_all()?;
    if let Err(error) = fs::rename(&temp_path, path) {
        let _ = fs::remove_file(&temp_path);
        return Err(error);
    }
    File::open(parent)?.sync_all()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_is_stable_across_reloads() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("identity/host.json");

        let first = load_or_create(&path).expect("first enrollment");
        let second = load_or_create(&path).expect("reload");

        assert_eq!(first.host_id, second.host_id);
        assert_eq!(first.lineage_id, second.lineage_id);
        assert_eq!(first.schema, HOST_IDENTITY_SCHEMA);
    }

    #[test]
    fn corrupt_existing_identity_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("identity/host.json");
        fs::create_dir_all(path.parent().expect("parent")).expect("mkdir");
        fs::write(&path, b"not-json").expect("write corrupt identity");

        assert!(load_or_create(&path).is_err());
    }

    #[test]
    fn high_confidence_fingerprint_enrolls_then_mismatch_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("identity/host.json");
        let identity = load_or_create(&path).expect("identity");
        let candidate = HardwareFingerprint {
            algorithm: "sha256".to_owned(),
            digest: Some("sha256:machine-a".to_owned()),
            confidence: FingerprintConfidence::High,
            observed_at: None,
        };
        let enrolled = reconcile_fingerprint(&path, identity, &candidate, "t1").expect("enroll");
        assert_eq!(
            enrolled.hardware_fingerprint.digest.as_deref(),
            Some("sha256:machine-a")
        );

        let mismatch = HardwareFingerprint {
            digest: Some("sha256:machine-b".to_owned()),
            ..candidate
        };
        assert!(matches!(
            reconcile_fingerprint(&path, enrolled, &mismatch, "t2"),
            Err(IdentityError::HardwareMismatch)
        ));
        assert_eq!(
            load(&path)
                .expect("identity remains")
                .hardware_fingerprint
                .digest
                .as_deref(),
            Some("sha256:machine-a")
        );
    }
}
