use prime_contracts::{
    ArtifactFormat, ExecInspection, ExecutionBackend, RuntimeFamily, EXEC_INSPECTION_SCHEMA,
};
use sha2::{Digest, Sha256};
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::os::unix::fs::{FileExt, MetadataExt, PermissionsExt};
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManagedPeInspection {
    pub cli_header_rva: u32,
    pub metadata_rva: u32,
    pub metadata_size: u32,
    pub flags: u32,
    pub entry_point_token: u32,
}

#[derive(Debug, Error)]
pub enum ManagedPeError {
    #[error("managed PE inspection I/O failed: {0}")]
    Io(#[from] io::Error),
    #[error("managed PE artifact path is a symbolic link")]
    Symlink,
    #[error("managed PE artifact is not a regular file")]
    NotRegularFile,
    #[error("managed PE artifact changed while it was being inspected")]
    ChangedDuringInspection,
    #[error("malformed managed PE: {0}")]
    Malformed(&'static str),
}

#[derive(Debug, Clone, Copy)]
struct PeSection {
    virtual_size: u32,
    virtual_address: u32,
    raw_size: u32,
    raw_offset: u32,
}

pub fn inspect_managed_pe(path: &Path) -> Result<Option<ManagedPeInspection>, ManagedPeError> {
    let before_path = fs::symlink_metadata(path)?;
    if before_path.file_type().is_symlink() {
        return Err(ManagedPeError::Symlink);
    }
    if !before_path.file_type().is_file() {
        return Err(ManagedPeError::NotRegularFile);
    }
    let stamp = FileStamp::from(&before_path);
    let file = File::open(path)?;
    let opened = file.metadata()?;
    if FileStamp::from(&opened) != stamp {
        return Err(ManagedPeError::ChangedDuringInspection);
    }

    let mut dos = [0_u8; 64];
    read_exact_checked(&file, stamp.length, 0, &mut dos)?;
    if &dos[0..2] != b"MZ" {
        return Ok(None);
    }
    let pe_offset = u32::from_le_bytes(dos[0x3c..0x40].try_into().unwrap()) as u64;
    let mut coff = [0_u8; 24];
    read_exact_checked(&file, stamp.length, pe_offset, &mut coff)?;
    if &coff[0..4] != b"PE\0\0" {
        return Err(ManagedPeError::Malformed("PE signature is invalid"));
    }
    let section_count = u16::from_le_bytes([coff[6], coff[7]]) as usize;
    if section_count == 0 || section_count > 96 {
        return Err(ManagedPeError::Malformed(
            "PE section count is out of bounds",
        ));
    }
    let optional_size = u16::from_le_bytes([coff[20], coff[21]]) as usize;
    if optional_size < 2 || optional_size > 4096 {
        return Err(ManagedPeError::Malformed(
            "PE optional header size is out of bounds",
        ));
    }
    let optional_offset = pe_offset.checked_add(24).ok_or(ManagedPeError::Malformed(
        "PE optional header offset overflow",
    ))?;
    let mut optional = vec![0_u8; optional_size];
    read_exact_checked(&file, stamp.length, optional_offset, &mut optional)?;
    let magic = u16::from_le_bytes([optional[0], optional[1]]);
    let directory_base = match magic {
        0x10b => 96usize,
        0x20b => 112usize,
        _ => {
            return Err(ManagedPeError::Malformed(
                "PE optional header magic is unsupported",
            ))
        }
    };
    let cli_dir = directory_base + 14 * 8;
    if cli_dir + 8 > optional.len() {
        return Err(ManagedPeError::Malformed(
            "PE optional header omits CLR data directory",
        ));
    }
    let cli_rva = u32::from_le_bytes(optional[cli_dir..cli_dir + 4].try_into().unwrap());
    let cli_size = u32::from_le_bytes(optional[cli_dir + 4..cli_dir + 8].try_into().unwrap());
    if cli_rva == 0 && cli_size == 0 {
        verify_unchanged(path, &file, &stamp)?;
        return Ok(None);
    }
    if cli_rva == 0 || cli_size < 0x48 {
        return Err(ManagedPeError::Malformed(
            "CLR data directory is incomplete",
        ));
    }

    let section_table_offset =
        optional_offset
            .checked_add(optional_size as u64)
            .ok_or(ManagedPeError::Malformed(
                "PE section table offset overflow",
            ))?;
    let mut section_bytes = vec![0_u8; section_count * 40];
    read_exact_checked(
        &file,
        stamp.length,
        section_table_offset,
        &mut section_bytes,
    )?;
    let mut sections = Vec::with_capacity(section_count);
    for section in section_bytes.chunks_exact(40) {
        sections.push(PeSection {
            virtual_size: u32::from_le_bytes(section[8..12].try_into().unwrap()),
            virtual_address: u32::from_le_bytes(section[12..16].try_into().unwrap()),
            raw_size: u32::from_le_bytes(section[16..20].try_into().unwrap()),
            raw_offset: u32::from_le_bytes(section[20..24].try_into().unwrap()),
        });
    }

    let cli_offset = map_pe_rva(cli_rva, 24, &sections).ok_or(ManagedPeError::Malformed(
        "CLR header RVA is not mapped by a PE section",
    ))?;
    let mut cli = [0_u8; 24];
    read_exact_checked(&file, stamp.length, cli_offset, &mut cli)?;
    let cli_cb = u32::from_le_bytes(cli[0..4].try_into().unwrap());
    if cli_cb < 0x48 || cli_cb > cli_size {
        return Err(ManagedPeError::Malformed("CLR header size is invalid"));
    }
    let metadata_rva = u32::from_le_bytes(cli[8..12].try_into().unwrap());
    let metadata_size = u32::from_le_bytes(cli[12..16].try_into().unwrap());
    if metadata_rva == 0 || metadata_size < 4 {
        return Err(ManagedPeError::Malformed(
            "CLR metadata directory is incomplete",
        ));
    }
    let metadata_offset = map_pe_rva(metadata_rva, 4, &sections).ok_or(
        ManagedPeError::Malformed("CLR metadata RVA is not mapped by a PE section"),
    )?;
    let mut signature = [0_u8; 4];
    read_exact_checked(&file, stamp.length, metadata_offset, &mut signature)?;
    if &signature != b"BSJB" {
        return Err(ManagedPeError::Malformed(
            "CLR metadata signature is not BSJB",
        ));
    }
    let result = ManagedPeInspection {
        cli_header_rva: cli_rva,
        metadata_rva,
        metadata_size,
        flags: u32::from_le_bytes(cli[16..20].try_into().unwrap()),
        entry_point_token: u32::from_le_bytes(cli[20..24].try_into().unwrap()),
    };
    verify_unchanged(path, &file, &stamp)?;
    Ok(Some(result))
}

fn read_exact_checked(
    file: &File,
    file_len: u64,
    offset: u64,
    buffer: &mut [u8],
) -> Result<(), ManagedPeError> {
    let end = offset
        .checked_add(buffer.len() as u64)
        .ok_or(ManagedPeError::Malformed("managed PE read offset overflow"))?;
    if end > file_len {
        return Err(ManagedPeError::Malformed(
            "managed PE structure extends beyond file",
        ));
    }
    file.read_exact_at(buffer, offset)?;
    Ok(())
}

fn map_pe_rva(rva: u32, required: u32, sections: &[PeSection]) -> Option<u64> {
    for section in sections {
        let span = section.virtual_size.max(section.raw_size);
        let Some(relative) = rva.checked_sub(section.virtual_address) else {
            continue;
        };
        if relative >= span {
            continue;
        }
        let raw_end = relative.checked_add(required)?;
        if raw_end > section.raw_size {
            continue;
        }
        return Some(section.raw_offset.checked_add(relative)? as u64);
    }
    None
}

fn verify_unchanged(path: &Path, file: &File, stamp: &FileStamp) -> Result<(), ManagedPeError> {
    let after_open = file.metadata()?;
    let after_path = fs::symlink_metadata(path)?;
    if after_path.file_type().is_symlink()
        || !after_path.file_type().is_file()
        || FileStamp::from(&after_open) != *stamp
        || FileStamp::from(&after_path) != *stamp
    {
        return Err(ManagedPeError::ChangedDuringInspection);
    }
    Ok(())
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

    fn managed_pe64() -> Vec<u8> {
        let mut bytes = vec![0_u8; 0x600];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x80_u32.to_le_bytes());
        let pe = 0x80usize;
        bytes[pe..pe + 4].copy_from_slice(b"PE\0\0");
        bytes[pe + 4..pe + 6].copy_from_slice(&0x8664_u16.to_le_bytes());
        bytes[pe + 6..pe + 8].copy_from_slice(&1_u16.to_le_bytes());
        bytes[pe + 20..pe + 22].copy_from_slice(&0x00f0_u16.to_le_bytes());
        let opt = pe + 24;
        bytes[opt..opt + 2].copy_from_slice(&0x20b_u16.to_le_bytes());
        bytes[opt + 108..opt + 112].copy_from_slice(&16_u32.to_le_bytes());
        let cli_dir = opt + 112 + 14 * 8;
        bytes[cli_dir..cli_dir + 4].copy_from_slice(&0x2000_u32.to_le_bytes());
        bytes[cli_dir + 4..cli_dir + 8].copy_from_slice(&0x48_u32.to_le_bytes());
        let section = opt + 0xf0;
        bytes[section..section + 5].copy_from_slice(b".text");
        bytes[section + 8..section + 12].copy_from_slice(&0x400_u32.to_le_bytes());
        bytes[section + 12..section + 16].copy_from_slice(&0x2000_u32.to_le_bytes());
        bytes[section + 16..section + 20].copy_from_slice(&0x400_u32.to_le_bytes());
        bytes[section + 20..section + 24].copy_from_slice(&0x200_u32.to_le_bytes());
        let cli = 0x200usize;
        bytes[cli..cli + 4].copy_from_slice(&0x48_u32.to_le_bytes());
        bytes[cli + 4..cli + 6].copy_from_slice(&2_u16.to_le_bytes());
        bytes[cli + 6..cli + 8].copy_from_slice(&5_u16.to_le_bytes());
        bytes[cli + 8..cli + 12].copy_from_slice(&0x2100_u32.to_le_bytes());
        bytes[cli + 12..cli + 16].copy_from_slice(&0x40_u32.to_le_bytes());
        bytes[cli + 16..cli + 20].copy_from_slice(&1_u32.to_le_bytes());
        bytes[cli + 20..cli + 24].copy_from_slice(&0x0600_0001_u32.to_le_bytes());
        bytes[0x300..0x304].copy_from_slice(b"BSJB");
        bytes
    }

    #[test]
    fn clr_inspection_distinguishes_native_and_managed_pe() {
        let dir = tempfile::tempdir().expect("tempdir");
        let managed = dir.path().join("managed.exe");
        fs::write(&managed, managed_pe64()).expect("managed fixture");
        let info = inspect_managed_pe(&managed)
            .expect("inspect managed PE")
            .expect("CLR metadata");
        assert_eq!(info.cli_header_rva, 0x2000);
        assert_eq!(info.metadata_rva, 0x2100);
        assert_eq!(info.metadata_size, 0x40);
        assert_eq!(info.flags, 1);
        assert_eq!(info.entry_point_token, 0x0600_0001);

        let native = dir.path().join("native.exe");
        let mut native_bytes = managed_pe64();
        let opt = 0x80 + 24;
        let cli_dir = opt + 112 + 14 * 8;
        native_bytes[cli_dir..cli_dir + 8].fill(0);
        fs::write(&native, native_bytes).expect("native fixture");
        assert_eq!(
            inspect_managed_pe(&native).expect("inspect native PE"),
            None
        );
    }

    #[test]
    fn clr_inspection_rejects_unmapped_cli_rva_and_bad_metadata_signature() {
        let dir = tempfile::tempdir().expect("tempdir");
        let bad_rva = dir.path().join("bad-rva.exe");
        let mut bad_rva_bytes = managed_pe64();
        let opt = 0x80 + 24;
        let cli_dir = opt + 112 + 14 * 8;
        bad_rva_bytes[cli_dir..cli_dir + 4].copy_from_slice(&0x9000_u32.to_le_bytes());
        fs::write(&bad_rva, bad_rva_bytes).expect("bad RVA fixture");
        assert!(matches!(
            inspect_managed_pe(&bad_rva),
            Err(ManagedPeError::Malformed(
                "CLR header RVA is not mapped by a PE section"
            ))
        ));

        let bad_meta = dir.path().join("bad-meta.exe");
        let mut bad_meta_bytes = managed_pe64();
        bad_meta_bytes[0x300..0x304].copy_from_slice(b"NOPE");
        fs::write(&bad_meta, bad_meta_bytes).expect("bad metadata fixture");
        assert!(matches!(
            inspect_managed_pe(&bad_meta),
            Err(ManagedPeError::Malformed(
                "CLR metadata signature is not BSJB"
            ))
        ));
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
