use prime_contracts::{
    ArtifactFormat, ExecInspection, ExecutionBackend, RuntimeFamily, EXEC_INSPECTION_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::Path;
use thiserror::Error;

const INSPECTION_PREFIX_LIMIT: usize = 1024 * 1024;

#[derive(Debug, Error)]
pub enum ExecError {
    #[error("artifact I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("artifact path is a symbolic link")]
    Symlink,
    #[error("artifact is not a regular file")]
    NotRegularFile,
    #[error("artifact changed while it was being inspected")]
    ChangedDuringInspection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FileStamp {
    device: u64,
    inode: u64,
    length: u64,
    mode: u32,
    mtime: i64,
    mtime_nsec: i64,
    ctime: i64,
    ctime_nsec: i64,
}

impl From<&Metadata> for FileStamp {
    fn from(metadata: &Metadata) -> Self {
        Self {
            device: metadata.dev(),
            inode: metadata.ino(),
            length: metadata.len(),
            mode: metadata.mode(),
            mtime: metadata.mtime(),
            mtime_nsec: metadata.mtime_nsec(),
            ctime: metadata.ctime(),
            ctime_nsec: metadata.ctime_nsec(),
        }
    }
}

pub fn inspect(path: &Path, host_arch: &str) -> Result<ExecInspection, ExecError> {
    let before_path = fs::symlink_metadata(path)?;
    if before_path.file_type().is_symlink() {
        return Err(ExecError::Symlink);
    }
    if !before_path.file_type().is_file() {
        return Err(ExecError::NotRegularFile);
    }
    let before_stamp = FileStamp::from(&before_path);

    let mut file = File::open(path)?;
    let opened = file.metadata()?;
    if !opened.file_type().is_file() || FileStamp::from(&opened) != before_stamp {
        return Err(ExecError::ChangedDuringInspection);
    }

    let mut hasher = Sha256::new();
    let mut prefix = Vec::with_capacity(64 * 1024);
    let mut buffer = [0_u8; 64 * 1024];
    let mut size_bytes = 0_u64;
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        size_bytes = size_bytes
            .checked_add(read as u64)
            .ok_or(ExecError::ChangedDuringInspection)?;
        hasher.update(&buffer[..read]);
        if prefix.len() < INSPECTION_PREFIX_LIMIT {
            let remaining = INSPECTION_PREFIX_LIMIT - prefix.len();
            let capture = remaining.min(read);
            prefix.extend_from_slice(&buffer[..capture]);
        }
    }

    let after_open = file.metadata()?;
    let after_path = fs::symlink_metadata(path)?;
    if after_path.file_type().is_symlink()
        || !after_path.file_type().is_file()
        || FileStamp::from(&after_open) != before_stamp
        || FileStamp::from(&after_path) != before_stamp
        || size_bytes != before_stamp.length
    {
        return Err(ExecError::ChangedDuringInspection);
    }

    let executable = before_path.permissions().mode() & 0o111 != 0;
    let classification = classify(&prefix, path, host_arch, executable);
    let artifact_identity = sha256_labelled(hasher.finalize().as_slice());

    Ok(ExecInspection {
        schema: EXEC_INSPECTION_SCHEMA.to_owned(),
        artifact_identity,
        size_bytes,
        format: classification.format,
        runtime_family: classification.runtime_family,
        workload_arch: classification.workload_arch,
        suggested_backend: classification.suggested_backend,
        native_compatible: classification.native_compatible,
        limitations: classification.limitations,
    })
}

#[derive(Debug)]
struct Classification {
    format: ArtifactFormat,
    runtime_family: RuntimeFamily,
    workload_arch: Option<String>,
    suggested_backend: Option<ExecutionBackend>,
    native_compatible: bool,
    limitations: Vec<String>,
}

fn classify(bytes: &[u8], path: &Path, host_arch: &str, executable: bool) -> Classification {
    if bytes.starts_with(b"\x7fELF") {
        return classify_elf(bytes, host_arch, executable);
    }
    if bytes.starts_with(b"MZ") {
        return classify_pe(bytes);
    }
    if bytes.starts_with(b"dex\n") {
        return foreign(ArtifactFormat::Dex, RuntimeFamily::Android, None);
    }
    if bytes.starts_with(b"\0asm") {
        return foreign(ArtifactFormat::Wasm, RuntimeFamily::Wasm, None);
    }
    if bytes.starts_with(b"PK\x03\x04") || bytes.starts_with(b"PK\x05\x06") {
        return classify_zip(path);
    }
    if bytes.starts_with(&[0xca, 0xfe, 0xba, 0xbe]) {
        if extension(path).as_deref() == Some("class") {
            return foreign(ArtifactFormat::Class, RuntimeFamily::Jvm, None);
        }
        let mut result = foreign(ArtifactFormat::MachO, RuntimeFamily::Darwin, None);
        result
            .limitations
            .push("MACHO_FAT_ARCHES_NOT_EXPANDED".to_owned());
        return result;
    }
    if is_thin_macho(bytes) {
        return classify_macho(bytes);
    }
    foreign(ArtifactFormat::Other, RuntimeFamily::Other, None)
}

fn classify_elf(bytes: &[u8], host_arch: &str, executable: bool) -> Classification {
    let mut limitations = Vec::new();
    let workload_arch = if bytes.len() >= 20 {
        let little_endian = bytes.get(5).copied() == Some(1);
        let machine = read_u16(&bytes[18..20], little_endian);
        machine.and_then(|machine| elf_arch(machine, bytes.get(4).copied()))
    } else {
        None
    };
    if workload_arch.is_none() {
        limitations.push("ELF_ARCHITECTURE_UNRESOLVED".to_owned());
    }
    if !executable {
        limitations.push("ELF_EXECUTE_PERMISSION_MISSING".to_owned());
    }
    let native_compatible = executable
        && workload_arch
            .as_deref()
            .is_some_and(|arch| arch_matches(host_arch, arch));
    if executable && workload_arch.is_some() && !native_compatible {
        limitations.push("ELF_HOST_ARCHITECTURE_MISMATCH".to_owned());
    }
    Classification {
        format: ArtifactFormat::Elf,
        runtime_family: RuntimeFamily::NativeLinux,
        workload_arch,
        suggested_backend: native_compatible.then_some(ExecutionBackend::Native),
        native_compatible,
        limitations,
    }
}

fn classify_pe(bytes: &[u8]) -> Classification {
    let mut limitations = Vec::new();
    let mut format = ArtifactFormat::Pe32;
    let mut workload_arch = None;
    if bytes.len() >= 64 {
        let offset =
            u32::from_le_bytes([bytes[0x3c], bytes[0x3d], bytes[0x3e], bytes[0x3f]]) as usize;
        if offset.checked_add(26).is_some_and(|end| end <= bytes.len())
            && bytes.get(offset..offset + 4) == Some(b"PE\0\0")
        {
            let machine = u16::from_le_bytes([bytes[offset + 4], bytes[offset + 5]]);
            workload_arch = pe_arch(machine);
            let optional_magic = u16::from_le_bytes([bytes[offset + 24], bytes[offset + 25]]);
            format = match optional_magic {
                0x20b => ArtifactFormat::Pe32Plus,
                0x10b => ArtifactFormat::Pe32,
                _ => {
                    limitations.push("PE_OPTIONAL_HEADER_MAGIC_UNRECOGNIZED".to_owned());
                    ArtifactFormat::Pe32
                }
            };
        } else {
            limitations.push("PE_HEADER_NOT_AVAILABLE_IN_INSPECTION_PREFIX".to_owned());
        }
    } else {
        limitations.push("PE_HEADER_TRUNCATED".to_owned());
    }
    if workload_arch.is_none() {
        limitations.push("PE_ARCHITECTURE_UNRESOLVED".to_owned());
    }
    Classification {
        format,
        runtime_family: RuntimeFamily::Windows,
        workload_arch,
        suggested_backend: None,
        native_compatible: false,
        limitations,
    }
}

fn classify_zip(path: &Path) -> Classification {
    match extension(path).as_deref() {
        Some("jar") => foreign(ArtifactFormat::Jar, RuntimeFamily::Jvm, None),
        Some("apk") => foreign(ArtifactFormat::Apk, RuntimeFamily::Android, None),
        Some("ipa") => foreign(ArtifactFormat::Ipa, RuntimeFamily::Ios, None),
        _ => foreign(ArtifactFormat::Other, RuntimeFamily::Other, None),
    }
}

fn classify_macho(bytes: &[u8]) -> Classification {
    let (little_endian, is_64) = match bytes.get(0..4) {
        Some([0xce, 0xfa, 0xed, 0xfe]) => (true, false),
        Some([0xcf, 0xfa, 0xed, 0xfe]) => (true, true),
        Some([0xfe, 0xed, 0xfa, 0xce]) => (false, false),
        Some([0xfe, 0xed, 0xfa, 0xcf]) => (false, true),
        _ => return foreign(ArtifactFormat::MachO, RuntimeFamily::Darwin, None),
    };
    let workload_arch = bytes
        .get(4..8)
        .and_then(|value| read_u32(value, little_endian))
        .and_then(|cpu| macho_arch(cpu, is_64));
    let mut result = foreign(ArtifactFormat::MachO, RuntimeFamily::Darwin, workload_arch);
    if result.workload_arch.is_none() {
        result
            .limitations
            .push("MACHO_ARCHITECTURE_UNRESOLVED".to_owned());
    }
    result
}

fn foreign(
    format: ArtifactFormat,
    runtime_family: RuntimeFamily,
    workload_arch: Option<String>,
) -> Classification {
    Classification {
        format,
        runtime_family,
        workload_arch,
        suggested_backend: None,
        native_compatible: false,
        limitations: Vec::new(),
    }
}

fn extension(path: &Path) -> Option<String> {
    path.extension()?
        .to_str()
        .map(|value| value.to_ascii_lowercase())
}

fn is_thin_macho(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(0..4),
        Some([0xce, 0xfa, 0xed, 0xfe])
            | Some([0xcf, 0xfa, 0xed, 0xfe])
            | Some([0xfe, 0xed, 0xfa, 0xce])
            | Some([0xfe, 0xed, 0xfa, 0xcf])
    )
}

fn read_u16(bytes: &[u8], little_endian: bool) -> Option<u16> {
    let bytes: [u8; 2] = bytes.try_into().ok()?;
    Some(if little_endian {
        u16::from_le_bytes(bytes)
    } else {
        u16::from_be_bytes(bytes)
    })
}

fn read_u32(bytes: &[u8], little_endian: bool) -> Option<u32> {
    let bytes: [u8; 4] = bytes.try_into().ok()?;
    Some(if little_endian {
        u32::from_le_bytes(bytes)
    } else {
        u32::from_be_bytes(bytes)
    })
}

fn elf_arch(machine: u16, class: Option<u8>) -> Option<String> {
    match machine {
        3 => Some("x86".to_owned()),
        40 => Some("arm".to_owned()),
        62 => Some("x86_64".to_owned()),
        183 => Some("aarch64".to_owned()),
        243 => Some(if class == Some(2) { "riscv64" } else { "riscv" }.to_owned()),
        _ => None,
    }
}

fn pe_arch(machine: u16) -> Option<String> {
    match machine {
        0x014c => Some("x86".to_owned()),
        0x01c4 => Some("arm".to_owned()),
        0x8664 => Some("x86_64".to_owned()),
        0xaa64 => Some("aarch64".to_owned()),
        _ => None,
    }
}

fn macho_arch(cpu: u32, is_64: bool) -> Option<String> {
    let base = cpu & 0x00ff_ffff;
    match (base, is_64 || cpu & 0x0100_0000 != 0) {
        (7, false) => Some("x86".to_owned()),
        (7, true) => Some("x86_64".to_owned()),
        (12, false) => Some("arm".to_owned()),
        (12, true) => Some("aarch64".to_owned()),
        _ => None,
    }
}

fn arch_matches(host_arch: &str, workload_arch: &str) -> bool {
    normalize_arch(host_arch) == normalize_arch(workload_arch)
}

fn normalize_arch(arch: &str) -> &str {
    match arch {
        "amd64" => "x86_64",
        "arm64" => "aarch64",
        "i386" | "i486" | "i586" | "i686" => "x86",
        other => other,
    }
}

fn sha256_labelled(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(7 + bytes.len() * 2);
    encoded.push_str("sha256:");
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::{symlink, PermissionsExt};

    fn elf(machine: u16) -> Vec<u8> {
        let mut bytes = vec![0_u8; 64];
        bytes[0..4].copy_from_slice(b"\x7fELF");
        bytes[4] = 2;
        bytes[5] = 1;
        bytes[18..20].copy_from_slice(&machine.to_le_bytes());
        bytes
    }

    #[test]
    fn matching_executable_elf_is_native_candidate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("app");
        fs::write(&path, elf(62)).expect("write elf");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        let result = inspect(&path, "x86_64").expect("inspect");
        assert_eq!(result.format, ArtifactFormat::Elf);
        assert_eq!(result.workload_arch.as_deref(), Some("x86_64"));
        assert_eq!(result.suggested_backend, Some(ExecutionBackend::Native));
        assert!(result.native_compatible);
    }

    #[test]
    fn elf_without_execute_permission_is_not_native_candidate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("app");
        fs::write(&path, elf(62)).expect("write elf");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
        let result = inspect(&path, "x86_64").expect("inspect");
        assert!(!result.native_compatible);
        assert!(result
            .limitations
            .contains(&"ELF_EXECUTE_PERMISSION_MISSING".to_owned()));
    }

    #[test]
    fn foreign_elf_architecture_is_not_silently_translated() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("app");
        fs::write(&path, elf(183)).expect("write elf");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).expect("chmod");
        let result = inspect(&path, "x86_64").expect("inspect");
        assert_eq!(result.workload_arch.as_deref(), Some("aarch64"));
        assert_eq!(result.suggested_backend, None);
        assert!(!result.native_compatible);
    }

    #[test]
    fn wasm_is_recognized_without_claiming_runtime_availability() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("module.wasm");
        fs::write(&path, b"\0asm\x01\0\0\0").expect("write wasm");
        let result = inspect(&path, "x86_64").expect("inspect");
        assert_eq!(result.format, ArtifactFormat::Wasm);
        assert_eq!(result.runtime_family, RuntimeFamily::Wasm);
        assert_eq!(result.suggested_backend, None);
    }

    #[test]
    fn symlink_input_is_rejected() {
        let dir = tempfile::tempdir().expect("tempdir");
        let target = dir.path().join("target");
        let link = dir.path().join("link");
        fs::write(&target, elf(62)).expect("write target");
        symlink(&target, &link).expect("symlink");
        assert!(matches!(inspect(&link, "x86_64"), Err(ExecError::Symlink)));
    }
}
