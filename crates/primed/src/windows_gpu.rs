use sha2::{Digest, Sha256};
use std::fs;
use std::io;
use std::path::PathBuf;
use thiserror::Error;

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
