use prime_contracts::{ArtifactFormat, WindowsProviderManifest, WINDOWS_PROVIDER_MANIFEST_SCHEMA};
use std::cmp::Ordering;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum WindowsPersonalityError {
    #[error("WINDOWS_PERSONALITY_UNAVAILABLE: {0}")]
    ProviderUnavailable(String),
    #[error("WINDOWS_PROVIDER_INVALID: {0}")]
    InvalidProvider(String),
    #[error("WINDOWS_WORKLOAD_UNSUPPORTED: no compatible provider for {format:?}/{arch}")]
    NoCompatibleProvider { format: ArtifactFormat, arch: String },
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
