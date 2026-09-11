use sha2::{Digest, Sha256};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const PRIME_MANAGED_BRIDGE_MARKER: &str = ".prime-w3-managed-runtime";
pub const MANAGED_BRIDGE_FINGERPRINT: &str = "wine-mono-w3-v1+mingw64-win-iconv-0.0.10-4.fc44+mingw32-libgcc-16.1.1-1.fc44+mingw32-winpthreads-13.0.0-3.fc44";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedBridgeSpec {
    pub source: PathBuf,
    pub target_relative: PathBuf,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManagedBridgeOutcome {
    AlreadySatisfied,
    Repaired,
}

#[derive(Debug, Error)]
pub enum ManagedBridgeError {
    #[error("managed bridge source is unsafe: {0}")]
    UnsafeSource(String),
    #[error("managed bridge target is unsafe: {0}")]
    UnsafeTarget(String),
    #[error("managed bridge source SHA-256 mismatch: {0}")]
    SourceHashMismatch(String),
    #[error("managed bridge I/O failed: {0}")]
    Io(#[from] io::Error),
}

pub fn managed_bridge_specs() -> Vec<ManagedBridgeSpec> {
    vec![
        ManagedBridgeSpec {
            source: PathBuf::from("/usr/lib/prime/windows-managed-runtime/x64/iconv.dll"),
            target_relative: PathBuf::from("drive_c/windows/system32/iconv.dll"),
            sha256: "74e23093e962adceccf77d2b05af4f4c7d6deee4a524140a296609f849700fc2".to_owned(),
        },
        ManagedBridgeSpec {
            source: PathBuf::from("/usr/lib/prime/windows-managed-runtime/x86/libgcc_s_dw2-1.dll"),
            target_relative: PathBuf::from("drive_c/windows/syswow64/libgcc_s_dw2-1.dll"),
            sha256: "09caac62f690912492418d5ca86c3340f5a76e5de07e5d6b7fe057e032b779d1".to_owned(),
        },
        ManagedBridgeSpec {
            source: PathBuf::from("/usr/lib/prime/windows-managed-runtime/x86/libwinpthread-1.dll"),
            target_relative: PathBuf::from("drive_c/windows/syswow64/libwinpthread-1.dll"),
            sha256: "1a8f18e693e49581c2d11d02812cadbaffa1bf6184353fa69ef35ac2950b9d9e".to_owned(),
        },
    ]
}

pub fn ensure_managed_bridge(
    prefix: &Path,
    specs: &[ManagedBridgeSpec],
) -> Result<ManagedBridgeOutcome, ManagedBridgeError> {
    ensure_directory(prefix)?;
    let mut repaired = false;
    for spec in specs {
        validate_relative_target(&spec.target_relative)?;
        let bytes = read_verified_source(spec)?;
        let target = prefix.join(&spec.target_relative);
        let parent = target.parent().ok_or_else(|| {
            ManagedBridgeError::UnsafeTarget(spec.target_relative.display().to_string())
        })?;
        validate_directory_chain(prefix, parent)?;
        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                    return Err(ManagedBridgeError::UnsafeTarget(
                        target.display().to_string(),
                    ));
                }
                if sha256_hex(&fs::read(&target)?) == spec.sha256 {
                    continue;
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        atomic_replace(&target, &bytes)?;
        if sha256_hex(&fs::read(&target)?) != spec.sha256 {
            return Err(ManagedBridgeError::UnsafeTarget(format!(
                "{} failed post-write SHA-256 verification",
                target.display()
            )));
        }
        repaired = true;
    }
    write_bridge_marker(prefix)?;
    Ok(if repaired {
        ManagedBridgeOutcome::Repaired
    } else {
        ManagedBridgeOutcome::AlreadySatisfied
    })
}

pub fn bridge_marker_matches(prefix: &Path) -> Result<bool, ManagedBridgeError> {
    let marker = prefix.join(PRIME_MANAGED_BRIDGE_MARKER);
    match fs::symlink_metadata(&marker) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                return Err(ManagedBridgeError::UnsafeTarget(
                    marker.display().to_string(),
                ));
            }
            Ok(fs::read_to_string(marker)? == format!("{MANAGED_BRIDGE_FINGERPRINT}\n"))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn read_verified_source(spec: &ManagedBridgeSpec) -> Result<Vec<u8>, ManagedBridgeError> {
    let metadata = fs::symlink_metadata(&spec.source).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            ManagedBridgeError::UnsafeSource(spec.source.display().to_string())
        } else {
            ManagedBridgeError::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(ManagedBridgeError::UnsafeSource(
            spec.source.display().to_string(),
        ));
    }
    if metadata.len() > 64 * 1024 * 1024 {
        return Err(ManagedBridgeError::UnsafeSource(format!(
            "{} exceeds 64 MiB",
            spec.source.display()
        )));
    }
    let bytes = fs::read(&spec.source)?;
    if sha256_hex(&bytes) != spec.sha256 {
        return Err(ManagedBridgeError::SourceHashMismatch(
            spec.source.display().to_string(),
        ));
    }
    Ok(bytes)
}

fn validate_relative_target(path: &Path) -> Result<(), ManagedBridgeError> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(ManagedBridgeError::UnsafeTarget(path.display().to_string()));
    }
    Ok(())
}

fn ensure_directory(path: &Path) -> Result<(), ManagedBridgeError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(ManagedBridgeError::UnsafeTarget(path.display().to_string()));
    }
    Ok(())
}

fn validate_directory_chain(prefix: &Path, parent: &Path) -> Result<(), ManagedBridgeError> {
    let relative = parent
        .strip_prefix(prefix)
        .map_err(|_| ManagedBridgeError::UnsafeTarget(parent.display().to_string()))?;
    let mut current = prefix.to_path_buf();
    ensure_directory(&current)?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(ManagedBridgeError::UnsafeTarget(
                parent.display().to_string(),
            ));
        };
        current.push(name);
        ensure_directory(&current)?;
    }
    Ok(())
}

fn atomic_replace(target: &Path, bytes: &[u8]) -> Result<(), ManagedBridgeError> {
    let parent = target
        .parent()
        .ok_or_else(|| ManagedBridgeError::UnsafeTarget(target.display().to_string()))?;
    let temp = parent.join(format!(".prime-w3-{}.tmp", Uuid::now_v7()));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::set_permissions(&temp, fs::Permissions::from_mode(0o644))?;
    fs::rename(&temp, target)?;
    Ok(())
}

fn write_bridge_marker(prefix: &Path) -> Result<(), ManagedBridgeError> {
    let marker = prefix.join(PRIME_MANAGED_BRIDGE_MARKER);
    if bridge_marker_matches(prefix)? {
        return Ok(());
    }
    if let Ok(metadata) = fs::symlink_metadata(&marker) {
        if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
            return Err(ManagedBridgeError::UnsafeTarget(
                marker.display().to_string(),
            ));
        }
    }
    let temp = prefix.join(format!(
        "{}.{}.tmp",
        PRIME_MANAGED_BRIDGE_MARKER,
        Uuid::now_v7()
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&temp)?;
    file.write_all(format!("{MANAGED_BRIDGE_FINGERPRINT}\n").as_bytes())?;
    file.sync_all()?;
    fs::rename(&temp, &marker)?;
    fs::set_permissions(&marker, fs::Permissions::from_mode(0o600))?;
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;

    fn fixture_spec(source: std::path::PathBuf) -> ManagedBridgeSpec {
        ManagedBridgeSpec {
            source,
            target_relative: std::path::PathBuf::from("drive_c/windows/system32/iconv.dll"),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad".to_owned(),
        }
    }

    #[test]
    fn production_bridge_contract_pins_exact_sources_targets_and_hashes() {
        let specs = managed_bridge_specs();
        assert_eq!(specs.len(), 3);
        assert_eq!(
            specs[0].source,
            std::path::PathBuf::from("/usr/lib/prime/windows-managed-runtime/x64/iconv.dll")
        );
        assert_eq!(
            specs[0].target_relative,
            std::path::PathBuf::from("drive_c/windows/system32/iconv.dll")
        );
        assert_eq!(
            specs[0].sha256,
            "74e23093e962adceccf77d2b05af4f4c7d6deee4a524140a296609f849700fc2"
        );
        assert_eq!(
            specs[1].source,
            std::path::PathBuf::from(
                "/usr/lib/prime/windows-managed-runtime/x86/libgcc_s_dw2-1.dll"
            )
        );
        assert_eq!(
            specs[1].target_relative,
            std::path::PathBuf::from("drive_c/windows/syswow64/libgcc_s_dw2-1.dll")
        );
        assert_eq!(
            specs[1].sha256,
            "09caac62f690912492418d5ca86c3340f5a76e5de07e5d6b7fe057e032b779d1"
        );
        assert_eq!(
            specs[2].source,
            std::path::PathBuf::from(
                "/usr/lib/prime/windows-managed-runtime/x86/libwinpthread-1.dll"
            )
        );
        assert_eq!(
            specs[2].target_relative,
            std::path::PathBuf::from("drive_c/windows/syswow64/libwinpthread-1.dll")
        );
        assert_eq!(
            specs[2].sha256,
            "1a8f18e693e49581c2d11d02812cadbaffa1bf6184353fa69ef35ac2950b9d9e"
        );
        assert!(MANAGED_BRIDGE_FINGERPRINT.contains("mingw64-win-iconv-0.0.10-4.fc44"));
        assert!(MANAGED_BRIDGE_FINGERPRINT.contains("mingw32-libgcc-16.1.1-1.fc44"));
        assert!(MANAGED_BRIDGE_FINGERPRINT.contains("mingw32-winpthreads-13.0.0-3.fc44"));
    }

    #[test]
    fn bridge_repairs_corruption_and_marker_loss_does_not_rewrite_valid_target() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir_all(prefix.join("drive_c/windows/system32")).unwrap();
        let source = dir.path().join("source.dll");
        fs::write(&source, b"abc").unwrap();
        let spec = fixture_spec(source);
        assert_eq!(
            ensure_managed_bridge(&prefix, std::slice::from_ref(&spec)).unwrap(),
            ManagedBridgeOutcome::Repaired
        );
        let target = prefix.join(&spec.target_relative);
        assert_eq!(fs::read(&target).unwrap(), b"abc");
        assert!(bridge_marker_matches(&prefix).unwrap());
        let before = fs::metadata(&target).unwrap().modified().unwrap();
        fs::remove_file(prefix.join(PRIME_MANAGED_BRIDGE_MARKER)).unwrap();
        assert_eq!(
            ensure_managed_bridge(&prefix, std::slice::from_ref(&spec)).unwrap(),
            ManagedBridgeOutcome::AlreadySatisfied
        );
        assert_eq!(fs::metadata(&target).unwrap().modified().unwrap(), before);
        fs::write(&target, b"broken").unwrap();
        assert_eq!(
            ensure_managed_bridge(&prefix, &[spec]).unwrap(),
            ManagedBridgeOutcome::Repaired
        );
        assert_eq!(fs::read(&target).unwrap(), b"abc");
    }

    #[test]
    fn bridge_rejects_symlink_source_or_target() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir_all(prefix.join("drive_c/windows/system32")).unwrap();
        let real = dir.path().join("real.dll");
        fs::write(&real, b"abc").unwrap();
        let link = dir.path().join("link.dll");
        symlink(&real, &link).unwrap();
        assert!(matches!(
            ensure_managed_bridge(&prefix, &[fixture_spec(link)]),
            Err(ManagedBridgeError::UnsafeSource(_))
        ));

        let target = prefix.join("drive_c/windows/system32/iconv.dll");
        symlink(&real, &target).unwrap();
        assert!(matches!(
            ensure_managed_bridge(&prefix, &[fixture_spec(real)]),
            Err(ManagedBridgeError::UnsafeTarget(_))
        ));
    }
}
