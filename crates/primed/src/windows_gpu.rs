use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;
use uuid::Uuid;

pub const DXVK_FINGERPRINT: &str =
    "dxvk-w4-v1+upstream-2.7.1+d85ce7c79f57ecd765aaa1b9e7007cb875e6fde9f6d331df799bce73d513ce87";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxvkSourceSpec {
    pub source: PathBuf,
    pub sha256: &'static str,
}

#[derive(Debug, Error)]
pub enum DxvkSourceError {
    #[error("DXVK source is unsafe: {0}")]
    UnsafeSource(String),
    #[error("DXVK source SHA-256 mismatch: {0}")]
    HashMismatch(String),
    #[error("DXVK source I/O failed: {0}")]
    Io(#[from] io::Error),
}

pub fn read_verified_dxvk_source(spec: &DxvkSourceSpec) -> Result<Vec<u8>, DxvkSourceError> {
    let metadata = fs::symlink_metadata(&spec.source).map_err(|error| {
        if error.kind() == io::ErrorKind::NotFound {
            DxvkSourceError::UnsafeSource(spec.source.display().to_string())
        } else {
            DxvkSourceError::Io(error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(DxvkSourceError::UnsafeSource(
            spec.source.display().to_string(),
        ));
    }
    if metadata.len() > 64 * 1024 * 1024 {
        return Err(DxvkSourceError::UnsafeSource(format!(
            "{} exceeds 64 MiB",
            spec.source.display()
        )));
    }
    let bytes = fs::read(&spec.source)?;
    let digest = format!("{:x}", Sha256::digest(&bytes));
    if digest != spec.sha256 {
        return Err(DxvkSourceError::HashMismatch(
            spec.source.display().to_string(),
        ));
    }
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxvkArchitecture {
    X64,
    X86,
}

pub fn dxvk_projection_specs(architecture: DxvkArchitecture) -> Vec<DxvkProjectionSpec> {
    let sources = dxvk_source_specs();
    let (range, target_dir) = match architecture {
        DxvkArchitecture::X64 => (0..5, "drive_c/windows/system32"),
        DxvkArchitecture::X86 => (5..10, "drive_c/windows/syswow64"),
    };
    sources[range]
        .iter()
        .map(|source| DxvkProjectionSpec {
            source: source.source.clone(),
            target_relative: PathBuf::from(target_dir).join(
                source
                    .source
                    .file_name()
                    .expect("production DXVK source paths always include a DLL name"),
            ),
            sha256: source.sha256,
        })
        .collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DxvkProjectionSpec {
    pub source: PathBuf,
    pub target_relative: PathBuf,
    pub sha256: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DxvkProjectionOutcome {
    AlreadySatisfied,
    Repaired,
}

#[derive(Debug, Error)]
pub enum DxvkProjectionError {
    #[error("DXVK source is unsafe: {0}")]
    UnsafeSource(String),
    #[error("DXVK target is unsafe: {0}")]
    UnsafeTarget(String),
    #[error("DXVK source SHA-256 mismatch: {0}")]
    SourceHashMismatch(String),
    #[error("DXVK projection I/O failed: {0}")]
    Io(#[from] io::Error),
}

pub fn ensure_dxvk_projection(
    prefix: &Path,
    specs: &[DxvkProjectionSpec],
) -> Result<DxvkProjectionOutcome, DxvkProjectionError> {
    ensure_projection_directory(prefix)?;
    let mut repaired = false;
    for spec in specs {
        validate_projection_target(&spec.target_relative)?;
        let source = DxvkSourceSpec {
            source: spec.source.clone(),
            sha256: spec.sha256,
        };
        let bytes = read_verified_dxvk_source(&source).map_err(|error| match error {
            DxvkSourceError::UnsafeSource(path) => DxvkProjectionError::UnsafeSource(path),
            DxvkSourceError::HashMismatch(path) => DxvkProjectionError::SourceHashMismatch(path),
            DxvkSourceError::Io(error) => DxvkProjectionError::Io(error),
        })?;
        let target = prefix.join(&spec.target_relative);
        let parent = target
            .parent()
            .ok_or_else(|| DxvkProjectionError::UnsafeTarget(target.display().to_string()))?;
        validate_projection_directory_chain(prefix, parent)?;
        match fs::symlink_metadata(&target) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
                    return Err(DxvkProjectionError::UnsafeTarget(
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
        atomic_projection_replace(&target, &bytes)?;
        if sha256_hex(&fs::read(&target)?) != spec.sha256 {
            return Err(DxvkProjectionError::UnsafeTarget(format!(
                "{} failed post-write SHA-256 verification",
                target.display()
            )));
        }
        repaired = true;
    }
    Ok(if repaired {
        DxvkProjectionOutcome::Repaired
    } else {
        DxvkProjectionOutcome::AlreadySatisfied
    })
}

pub fn native_only_overrides(imports: &[String]) -> String {
    let mut names: Vec<String> = imports
        .iter()
        .filter_map(|name| {
            let normalized = name.to_ascii_lowercase();
            match normalized.as_str() {
                "d3d8.dll" | "d3d9.dll" | "d3d10core.dll" | "d3d11.dll" | "dxgi.dll" => {
                    Some(normalized.trim_end_matches(".dll").to_owned())
                }
                "d3d10.dll" | "d3d10_1.dll" => Some("d3d10core".to_owned()),
                _ => None,
            }
        })
        .collect();
    if imports.iter().any(|name| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "d3d10.dll" | "d3d10_1.dll" | "d3d10core.dll" | "d3d11.dll"
        )
    }) {
        names.push("dxgi".to_owned());
    }
    names.sort();
    names.dedup();
    if names.is_empty() {
        String::new()
    } else {
        format!("{}=n", names.join(","))
    }
}

fn validate_projection_target(path: &Path) -> Result<(), DxvkProjectionError> {
    if path.is_absolute()
        || path
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(DxvkProjectionError::UnsafeTarget(
            path.display().to_string(),
        ));
    }
    Ok(())
}

fn ensure_projection_directory(path: &Path) -> Result<(), DxvkProjectionError> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(DxvkProjectionError::UnsafeTarget(
            path.display().to_string(),
        ));
    }
    Ok(())
}

fn validate_projection_directory_chain(
    prefix: &Path,
    parent: &Path,
) -> Result<(), DxvkProjectionError> {
    let relative = parent
        .strip_prefix(prefix)
        .map_err(|_| DxvkProjectionError::UnsafeTarget(parent.display().to_string()))?;
    let mut current = prefix.to_path_buf();
    ensure_projection_directory(&current)?;
    for component in relative.components() {
        let Component::Normal(name) = component else {
            return Err(DxvkProjectionError::UnsafeTarget(
                parent.display().to_string(),
            ));
        };
        current.push(name);
        ensure_projection_directory(&current)?;
    }
    Ok(())
}

fn atomic_projection_replace(target: &Path, bytes: &[u8]) -> Result<(), DxvkProjectionError> {
    let parent = target
        .parent()
        .ok_or_else(|| DxvkProjectionError::UnsafeTarget(target.display().to_string()))?;
    let temp = parent.join(format!(".prime-w4-{}.tmp", Uuid::now_v7()));
    let mut file = fs::OpenOptions::new()
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

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

pub fn dxvk_source_specs() -> Vec<DxvkSourceSpec> {
    [
        (
            "x64/d3d8.dll",
            "253dcda5bd9791b36dbdbc99881f518c6b867a78c19448e664782a4fa306c2a5",
        ),
        (
            "x64/d3d9.dll",
            "2740233a94774fa64a1e07c6eaf94f67027a3c7594ff3b48f7a1c9801777e5a9",
        ),
        (
            "x64/d3d10core.dll",
            "3bf5fec5115649dfb6fed1613a4c3f9487c2f2aaf74c2786d9f9d7d21a2f1482",
        ),
        (
            "x64/d3d11.dll",
            "523da2cd765dd51d99fe1644b24f9ee7ee6247e4c98953bcc449ce84cd303405",
        ),
        (
            "x64/dxgi.dll",
            "ec02eb37620ff52361cb45376a4611fc4210d96e71d0363f1cc9807f151c01be",
        ),
        (
            "x86/d3d8.dll",
            "901d4f234cad5b9c6c9f0323aba123bfa8c096dcf6f7bfbe24d3ec3c3798d348",
        ),
        (
            "x86/d3d9.dll",
            "00ecd422b3b12e9d3b309785f3f19431405389d80eb4a37018ebc63e2d87d900",
        ),
        (
            "x86/d3d10core.dll",
            "9f305720d5be52a8329cebb34f3c2e26ca3decace0d57b1d618531673d11e9bc",
        ),
        (
            "x86/d3d11.dll",
            "7406e13d2694244499783d2e9fb1be285b6324890a348137fe5b5042c64b5913",
        ),
        (
            "x86/dxgi.dll",
            "53defc65dd2e53d05924a960c0891e15692ca8780ed0509a1a7efda71f4d631e",
        ),
    ]
    .into_iter()
    .map(|(relative, sha256)| DxvkSourceSpec {
        source: PathBuf::from("/usr/lib/prime/windows-gpu-runtime").join(relative),
        sha256,
    })
    .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_projection_maps_each_architecture_to_its_exact_prefix_directory() {
        let x64 = dxvk_projection_specs(DxvkArchitecture::X64);
        let x86 = dxvk_projection_specs(DxvkArchitecture::X86);
        assert_eq!(x64.len(), 5);
        assert_eq!(x86.len(), 5);
        assert!(x64
            .iter()
            .all(|spec| spec.target_relative.starts_with("drive_c/windows/system32")));
        assert!(x86
            .iter()
            .all(|spec| spec.target_relative.starts_with("drive_c/windows/syswow64")));
        assert_eq!(
            x64[3].target_relative,
            PathBuf::from("drive_c/windows/system32/d3d11.dll")
        );
        assert_eq!(
            x86[3].target_relative,
            PathBuf::from("drive_c/windows/syswow64/d3d11.dll")
        );
    }

    #[test]
    fn projection_rejects_symlink_target() {
        use std::fs;
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir_all(prefix.join("drive_c/windows/system32")).unwrap();
        let source = dir.path().join("d3d11.dll");
        fs::write(&source, b"abc").unwrap();
        let outside = dir.path().join("outside.dll");
        fs::write(&outside, b"abc").unwrap();
        symlink(&outside, prefix.join("drive_c/windows/system32/d3d11.dll")).unwrap();
        let spec = DxvkProjectionSpec {
            source,
            target_relative: PathBuf::from("drive_c/windows/system32/d3d11.dll"),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        assert!(matches!(
            ensure_dxvk_projection(&prefix, &[spec]),
            Err(DxvkProjectionError::UnsafeTarget(_))
        ));
    }

    #[test]
    fn x86_projection_uses_syswow64_and_same_repair_invariants() {
        use std::fs;
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir_all(prefix.join("drive_c/windows/syswow64")).unwrap();
        let source = dir.path().join("d3d9.dll");
        fs::write(&source, b"abc").unwrap();
        let spec = DxvkProjectionSpec {
            source,
            target_relative: PathBuf::from("drive_c/windows/syswow64/d3d9.dll"),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        assert_eq!(ensure_dxvk_projection(&prefix, std::slice::from_ref(&spec)).unwrap(), DxvkProjectionOutcome::Repaired);
        assert_eq!(fs::read(prefix.join(&spec.target_relative)).unwrap(), b"abc");
        assert_eq!(native_only_overrides(&["d3d9.dll".into()]), "d3d9=n");
    }

    #[test]
    fn x64_projection_repairs_corruption_and_builds_native_only_overrides() {
        use std::fs;

        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        fs::create_dir_all(prefix.join("drive_c/windows/system32")).unwrap();
        let source = dir.path().join("d3d11.dll");
        fs::write(&source, b"abc").unwrap();
        let spec = DxvkProjectionSpec {
            source: source.clone(),
            target_relative: PathBuf::from("drive_c/windows/system32/d3d11.dll"),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };

        assert_eq!(
            ensure_dxvk_projection(&prefix, std::slice::from_ref(&spec)).unwrap(),
            DxvkProjectionOutcome::Repaired
        );
        let target = prefix.join(&spec.target_relative);
        assert_eq!(fs::read(&target).unwrap(), b"abc");
        assert_eq!(
            native_only_overrides(&["d3d11.dll".into(), "dxgi.dll".into()]),
            "d3d11,dxgi=n"
        );

        fs::write(&target, b"broken").unwrap();
        assert_eq!(
            ensure_dxvk_projection(&prefix, &[spec]).unwrap(),
            DxvkProjectionOutcome::Repaired
        );
        assert_eq!(fs::read(&target).unwrap(), b"abc");
    }

    #[test]
    fn dxvk_source_verification_rejects_hash_mismatch_and_symlink() {
        use std::fs;
        use std::os::unix::fs::symlink;

        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("d3d11.dll");
        fs::write(&source, b"abc").unwrap();
        let good = DxvkSourceSpec {
            source: source.clone(),
            sha256: "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
        };
        assert_eq!(read_verified_dxvk_source(&good).unwrap(), b"abc");

        let bad = DxvkSourceSpec {
            source: source.clone(),
            sha256: "0000000000000000000000000000000000000000000000000000000000000000",
        };
        assert!(matches!(
            read_verified_dxvk_source(&bad),
            Err(DxvkSourceError::HashMismatch(_))
        ));

        let link = dir.path().join("link.dll");
        symlink(&source, &link).unwrap();
        let linked = DxvkSourceSpec {
            source: link,
            sha256: good.sha256,
        };
        assert!(matches!(
            read_verified_dxvk_source(&linked),
            Err(DxvkSourceError::UnsafeSource(_))
        ));
    }

    #[test]
    fn production_dxvk_contract_pins_exact_release_architectures_sources_and_hashes() {
        let specs = dxvk_source_specs();
        assert_eq!(DXVK_FINGERPRINT, "dxvk-w4-v1+upstream-2.7.1+d85ce7c79f57ecd765aaa1b9e7007cb875e6fde9f6d331df799bce73d513ce87");
        assert_eq!(specs.len(), 10);
        assert_eq!(
            specs[0].source,
            PathBuf::from("/usr/lib/prime/windows-gpu-runtime/x64/d3d8.dll")
        );
        assert_eq!(
            specs[0].sha256,
            "253dcda5bd9791b36dbdbc99881f518c6b867a78c19448e664782a4fa306c2a5"
        );
        assert_eq!(
            specs[4].source,
            PathBuf::from("/usr/lib/prime/windows-gpu-runtime/x64/dxgi.dll")
        );
        assert_eq!(
            specs[4].sha256,
            "ec02eb37620ff52361cb45376a4611fc4210d96e71d0363f1cc9807f151c01be"
        );
        assert_eq!(
            specs[5].source,
            PathBuf::from("/usr/lib/prime/windows-gpu-runtime/x86/d3d8.dll")
        );
        assert_eq!(
            specs[5].sha256,
            "901d4f234cad5b9c6c9f0323aba123bfa8c096dcf6f7bfbe24d3ec3c3798d348"
        );
        assert_eq!(
            specs[9].source,
            PathBuf::from("/usr/lib/prime/windows-gpu-runtime/x86/dxgi.dll")
        );
        assert_eq!(
            specs[9].sha256,
            "53defc65dd2e53d05924a960c0891e15692ca8780ed0509a1a7efda71f4d631e"
        );
        assert!(specs.iter().all(|spec| spec.sha256.len() == 64));
    }
}
