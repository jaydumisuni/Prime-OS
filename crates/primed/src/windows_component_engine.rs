use crate::windows_state;
use prime_contracts::{
    valid_sha256_label, WindowsComponentManifest, WindowsComponentReference,
    WindowsVerificationProbe,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const WINDOWS_COMPONENT_STATE_SCHEMA: &str = "prime.windows-component-state.v1";

#[derive(Debug, Error)]
pub enum WindowsComponentEngineError {
    #[error("WINDOWS_COMPONENT_STATE_IO: {0}")]
    Io(#[from] io::Error),
    #[error("WINDOWS_COMPONENT_STATE_JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("WINDOWS_COMPONENT_PROBE_PATH_UNSAFE: {0}")]
    UnsafeProbePath(String),
    #[error("WINDOWS_COMPONENT_PROBE_UNSUPPORTED: {0}")]
    ProbeUnsupported(&'static str),
    #[error("WINDOWS_COMPONENT_PROBE_INVALID: {0}")]
    InvalidProbe(&'static str),
    #[error("WINDOWS_COMPONENT_STATE_INVALID: {0}")]
    MarkerInvalid(&'static str),
    #[error(transparent)]
    State(#[from] windows_state::WindowsStateError),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WindowsProbeResult {
    pub kind: String,
    pub target: String,
    pub passed: bool,
    pub observed: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct WindowsComponentStateMarker {
    schema: String,
    component_id: String,
    component_revision: u64,
    component_digest: String,
    donor_fingerprint: String,
    installed_at: String,
    verification: Vec<WindowsProbeResult>,
}

pub fn all_probes_pass(results: &[WindowsProbeResult]) -> bool {
    !results.is_empty() && results.iter().all(|result| result.passed)
}

pub fn verify_component_state(
    prefix: &Path,
    manifest: &WindowsComponentManifest,
) -> Result<Vec<WindowsProbeResult>, WindowsComponentEngineError> {
    validate_prefix(prefix)?;
    let mut results = Vec::with_capacity(manifest.verification.len());
    for probe in &manifest.verification {
        let result = match probe {
            WindowsVerificationProbe::FileExists { path } => {
                let target = safe_probe_path(prefix, path)?;
                let exists = match fs::symlink_metadata(&target) {
                    Ok(metadata) => {
                        if metadata.file_type().is_symlink() {
                            return Err(WindowsComponentEngineError::UnsafeProbePath(path.clone()));
                        }
                        true
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => false,
                    Err(error) => return Err(error.into()),
                };
                WindowsProbeResult {
                    kind: "FILE_EXISTS".to_owned(),
                    target: path.clone(),
                    passed: exists,
                    observed: exists.then(|| "present".to_owned()),
                }
            }
            WindowsVerificationProbe::FileSha256 { path, sha256 } => {
                if !valid_sha256_label(sha256) {
                    return Err(WindowsComponentEngineError::InvalidProbe(
                        "FILE_SHA256 expected digest is not canonical SHA-256",
                    ));
                }
                let target = safe_probe_path(prefix, path)?;
                let observed = match fs::symlink_metadata(&target) {
                    Ok(metadata) => {
                        if metadata.file_type().is_symlink() {
                            return Err(WindowsComponentEngineError::UnsafeProbePath(path.clone()));
                        }
                        if !metadata.is_file() {
                            None
                        } else {
                            Some(sha256_file(&target)?)
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::NotFound => None,
                    Err(error) => return Err(error.into()),
                };
                WindowsProbeResult {
                    kind: "FILE_SHA256".to_owned(),
                    target: path.clone(),
                    passed: observed.as_deref() == Some(sha256.as_str()),
                    observed,
                }
            }
            WindowsVerificationProbe::RegistryValueEquals { .. } => {
                return Err(WindowsComponentEngineError::ProbeUnsupported(
                    "REGISTRY_VALUE_EQUALS requires the W2 donor query binding",
                ));
            }
        };
        results.push(result);
    }
    Ok(results)
}

pub fn component_marker_path(state_root: &Path, manifest: &WindowsComponentManifest) -> PathBuf {
    state_root
        .join("components")
        .join(&manifest.component_id)
        .join("installed.json")
}

pub fn write_component_marker(
    state_root: &Path,
    manifest: &WindowsComponentManifest,
    donor_fingerprint: &str,
    installed_at: &str,
    verification: &[WindowsProbeResult],
) -> Result<String, WindowsComponentEngineError> {
    if !all_probes_pass(verification) {
        return Err(WindowsComponentEngineError::MarkerInvalid(
            "cannot seal component marker with failed verification",
        ));
    }
    validate_manifest_identity(manifest)?;
    windows_state::ensure_private_directory(state_root)?;
    let components = state_root.join("components");
    windows_state::ensure_private_directory(&components)?;
    let component_root = components.join(&manifest.component_id);
    windows_state::ensure_private_directory(&component_root)?;

    let marker = WindowsComponentStateMarker {
        schema: WINDOWS_COMPONENT_STATE_SCHEMA.to_owned(),
        component_id: manifest.component_id.clone(),
        component_revision: manifest.revision,
        component_digest: manifest.digest.clone(),
        donor_fingerprint: donor_fingerprint.to_owned(),
        installed_at: installed_at.to_owned(),
        verification: verification.to_vec(),
    };
    let bytes = serde_json::to_vec_pretty(&marker)?;
    let digest = sha256_labelled(&bytes);
    let path = component_marker_path(state_root, manifest);
    if let Ok(metadata) = fs::symlink_metadata(&path) {
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(WindowsComponentEngineError::MarkerInvalid(
                "component marker path is not a regular non-symlink file",
            ));
        }
    }
    let temp = component_root.join(format!(".installed.{}.tmp", Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(&bytes)?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    fs::rename(&temp, &path)?;
    File::open(&component_root)?.sync_all()?;
    Ok(digest)
}

pub fn component_marker_matches(
    state_root: &Path,
    manifest: &WindowsComponentManifest,
    donor_fingerprint: &str,
) -> Result<bool, WindowsComponentEngineError> {
    validate_manifest_identity(manifest)?;
    let path = component_marker_path(state_root, manifest);
    let metadata = match fs::symlink_metadata(&path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error.into()),
    };
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(WindowsComponentEngineError::MarkerInvalid(
            "component marker path is not a regular non-symlink file",
        ));
    }
    let marker: WindowsComponentStateMarker = serde_json::from_slice(&fs::read(path)?)?;
    Ok(marker.schema == WINDOWS_COMPONENT_STATE_SCHEMA
        && marker.component_id == manifest.component_id
        && marker.component_revision == manifest.revision
        && marker.component_digest == manifest.digest
        && marker.donor_fingerprint == donor_fingerprint
        && all_probes_pass(&marker.verification))
}

fn validate_manifest_identity(
    manifest: &WindowsComponentManifest,
) -> Result<(), WindowsComponentEngineError> {
    let reference = format!(
        "windows-component:{}@{}#{}",
        manifest.component_id, manifest.revision, manifest.digest
    );
    WindowsComponentReference::parse(&reference).map_err(|_| {
        WindowsComponentEngineError::MarkerInvalid("component identity is not canonical")
    })?;
    Ok(())
}

fn validate_prefix(prefix: &Path) -> Result<(), WindowsComponentEngineError> {
    let metadata = fs::symlink_metadata(prefix)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(WindowsComponentEngineError::UnsafeProbePath(
            prefix.display().to_string(),
        ));
    }
    Ok(())
}

fn safe_probe_path(prefix: &Path, raw: &str) -> Result<PathBuf, WindowsComponentEngineError> {
    if raw.is_empty() {
        return Err(WindowsComponentEngineError::UnsafeProbePath(raw.to_owned()));
    }
    let relative = Path::new(raw);
    if relative.is_absolute() {
        return Err(WindowsComponentEngineError::UnsafeProbePath(raw.to_owned()));
    }
    let mut names = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(name) => names.push(name.to_os_string()),
            _ => return Err(WindowsComponentEngineError::UnsafeProbePath(raw.to_owned())),
        }
    }
    if names.is_empty() {
        return Err(WindowsComponentEngineError::UnsafeProbePath(raw.to_owned()));
    }

    let canonical_prefix = fs::canonicalize(prefix)?;
    let mut target = canonical_prefix.clone();
    for name in names {
        target.push(name);
        match fs::symlink_metadata(&target) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(WindowsComponentEngineError::UnsafeProbePath(raw.to_owned()));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    if !target.starts_with(&canonical_prefix) {
        return Err(WindowsComponentEngineError::UnsafeProbePath(raw.to_owned()));
    }
    Ok(target)
}

fn sha256_file(path: &Path) -> Result<String, WindowsComponentEngineError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(encoded)
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
