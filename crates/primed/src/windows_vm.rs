use prime_contracts::{
    ApplicationProfile, ArtifactFormat, ExecutionBackend, MechanicalCompatibilityState,
    RuntimeFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::ffi::CStr;
use std::fs;
use std::io::{self, Read, Write};
use std::os::fd::AsRawFd;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const WINDOWS_VM_GUEST_SCHEMA: &str = "prime.windows-vm-guest.v1";
pub const WINDOWS_VM_AGENT_SCHEMA: &str = "prime.windows-vm-agent.v1";
pub const WINDOWS_VM_RUNTIME_PROOF_SCHEMA: &str = "prime.windows-vm-runtime-proof.v1";
pub const WINDOWS_VM_RUNTIME_OBSERVATION_SCHEMA: &str = "prime.windows-vm-runtime-observation.v1";
pub const WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA: &str = "prime.windows-vm-runtime-evidence.v1";
pub const WINDOWS_VM_RUNTIME_EVIDENCE_MAX_BYTES: usize = 1024 * 1024;
/// Maximum JSON handshake payload admitted before decoding (8 KiB).
pub const WINDOWS_VM_AGENT_HELLO_MAX_BYTES: usize = 8 * 1024;
const WINDOWS_VM_MAX_USB_PASSTHROUGH_NODES: usize = 15;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GuestAgentHello {
    pub schema: String,
    pub protocol_revision: u64,
    pub guest_id: String,
    pub guest_revision: u64,
    pub guest_sha256: String,
    pub workload_arch: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmSessionPaths {
    pub runtime_dir: PathBuf,
    pub overlay_image: PathBuf,
    pub agent_socket: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct VmGuestDefinition {
    pub schema: String,
    pub guest_id: String,
    pub revision: u64,
    pub base_image: String,
    pub base_sha256: String,
    pub memory_mib: u32,
    pub vcpus: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedVmGuest {
    pub guest_id: String,
    pub revision: u64,
    pub base_image: PathBuf,
    pub base_sha256: String,
    pub memory_mib: u32,
    pub vcpus: u8,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmPlanRequest {
    pub qemu_binary: PathBuf,
    pub overlay_image: PathBuf,
    pub runtime_dir: PathBuf,
    pub memory_mib: u32,
    pub vcpus: u8,
    pub usb_nodes: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QemuPlan {
    pub program: PathBuf,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum VmRuntimeAdversarialCase {
    ConcurrentSessionIsolation,
    ForcedProcessTermination,
    CorruptGuestAuthority,
    DeniedUsbNode,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmRuntimeProofPlan {
    pub proof_id: String,
    pub guest_id: String,
    pub guest_revision: u64,
    pub guest_sha256: String,
    pub application_id: String,
    pub session_id: String,
    pub paths: VmSessionPaths,
    pub required_cases: Vec<VmRuntimeAdversarialCase>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmRuntimeProofObservation {
    pub proof_id: String,
    pub case: VmRuntimeAdversarialCase,
    pub guest_id: String,
    pub guest_revision: u64,
    pub guest_sha256: String,
    pub application_id: String,
    pub session_id: String,
    pub passed: bool,
    pub zero_residual_state: bool,
    pub evidence_schema: String,
    pub evidence_sha256: String,
    pub observation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmRuntimeProofState {
    Pending,
    Pass,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VmRuntimeEvidenceEnvelope {
    pub schema: String,
    pub proof_id: String,
    pub case: VmRuntimeAdversarialCase,
    pub guest_id: String,
    pub guest_revision: u64,
    pub guest_sha256: String,
    pub application_id: String,
    pub session_id: String,
    pub evidence: Vec<u8>,
}

#[derive(Debug, Error)]
pub enum WindowsVmError {
    #[error("invalid Windows VM guest definition: {0}")]
    InvalidGuest(&'static str),
    #[error("Windows VM guest image digest mismatch")]
    GuestDigestMismatch,
    #[error("invalid Windows VM profile: {0}")]
    InvalidProfile(&'static str),
    #[error("invalid Windows VM QEMU plan: {0}")]
    InvalidPlan(&'static str),
    #[error("invalid Windows VM USB node: {0}")]
    InvalidUsb(String),
    #[error(transparent)]
    Io(#[from] io::Error),
}

/// Parse a bounded guest-agent hello without granting guest-runtime authority.
///
/// A future socket reader must first bound the received frame, then use this
/// entry point. This is only source-level handshake admission: it does not
/// establish a live guest connection or certify Windows workload execution.
pub fn parse_guest_agent_hello_json(
    bytes: &[u8],
    expected_guest_id: &str,
    expected_guest_revision: u64,
    expected_guest_sha256: &str,
    expected_workload_arch: &str,
) -> Result<GuestAgentHello, WindowsVmError> {
    if bytes.len() > WINDOWS_VM_AGENT_HELLO_MAX_BYTES {
        return Err(WindowsVmError::InvalidGuest(
            "guest-agent hello exceeds JSON size limit",
        ));
    }
    // Caller-owned host expectations must be admitted before spending any
    // parsing work on an untrusted guest payload, even at the direct JSON API.
    validate_expected_guest_agent_authority(
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )?;
    let hello: GuestAgentHello = serde_json::from_slice(bytes)
        .map_err(|_| WindowsVmError::InvalidGuest("malformed guest-agent hello JSON"))?;
    validate_guest_agent_hello(
        &hello,
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )?;
    Ok(hello)
}

/// Construct deterministic, bounded hello JSON for a non-production guest
/// harness. This is only fixture construction: it does not attest a guest,
/// open a transport, or authorize Windows execution. The caller retains
/// ownership of the frame length and any transport framing.
pub fn encode_guest_agent_hello_json(
    hello: &GuestAgentHello,
    expected_guest_id: &str,
    expected_guest_revision: u64,
    expected_guest_sha256: &str,
    expected_workload_arch: &str,
) -> Result<Vec<u8>, WindowsVmError> {
    // Reuse the same fail-closed host and guest identity admission as the
    // decoder. A harness must not serialize an unauthorized guest identity.
    validate_guest_agent_hello(
        hello,
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )?;
    let encoded = serde_json::to_vec(hello)
        .map_err(|_| WindowsVmError::InvalidGuest("cannot encode guest-agent hello JSON"))?;
    if encoded.len() > WINDOWS_VM_AGENT_HELLO_MAX_BYTES {
        return Err(WindowsVmError::InvalidGuest(
            "guest-agent hello exceeds JSON size limit",
        ));
    }
    Ok(encoded)
}

/// Write exactly one validated, bounded JSON payload for a non-production
/// W8 harness. The caller owns the length delimiter and transport; this
/// function does not open a socket, start a guest, or grant runtime authority.
///
/// Invalid host or guest authority is rejected before any writer call.
/// Completion includes Write::flush, so a buffered transport failure cannot
/// be mistaken for a completed payload. This is not guest acknowledgement.
/// Any write or flush error invalidates the caller-owned stream/frame.
pub fn write_guest_agent_hello_payload<W: Write>(
    writer: &mut W,
    hello: &GuestAgentHello,
    expected_guest_id: &str,
    expected_guest_revision: u64,
    expected_guest_sha256: &str,
    expected_workload_arch: &str,
) -> Result<usize, WindowsVmError> {
    let payload = encode_guest_agent_hello_json(
        hello,
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )?;
    write_all_guest_agent_bounded(writer, &payload)?;
    flush_guest_agent_bounded(writer)?;
    Ok(payload.len())
}

// An interrupted buffered flush is not acknowledgement or a fatal stream error.
// Retry transient EINTR, but never allow a non-progressing transport to pin
// the Gate 6 host worker. Any terminal error invalidates the caller-owned frame.
fn flush_guest_agent_bounded<W: Write>(writer: &mut W) -> io::Result<()> {
    let mut interruptions = 0;
    loop {
        match writer.flush() {
            Ok(()) => return Ok(()),
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                interruptions += 1;
                if interruptions > WINDOWS_VM_AGENT_MAX_CONSECUTIVE_WRITE_INTERRUPTS {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
}

// Caller-owned expectations must be valid before consuming an untrusted
// transport frame. A failed host preflight must leave the stream untouched.
fn validate_expected_guest_agent_authority(
    expected_guest_id: &str,
    expected_guest_revision: u64,
    expected_guest_sha256: &str,
    expected_workload_arch: &str,
) -> Result<(), WindowsVmError> {
    if expected_guest_id.is_empty()
        || expected_guest_id.len() > 128
        || !expected_guest_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || expected_guest_revision == 0
    {
        return Err(WindowsVmError::InvalidGuest(
            "invalid expected guest identity",
        ));
    }
    // A zero-filled SHA-256 is a common unbound placeholder, not immutable
    // guest-image evidence. Do not let a matching synthetic hello turn that
    // sentinel into apparent host/guest authority at any framing entry point.
    if !canonical_sha256_hex(expected_guest_sha256)
        || expected_guest_sha256.bytes().all(|byte| byte == b'0')
    {
        return Err(WindowsVmError::InvalidGuest("guest-agent digest mismatch"));
    }
    if !matches!(expected_workload_arch, "x86" | "x86_64") {
        return Err(WindowsVmError::InvalidGuest(
            "guest-agent workload architecture mismatch",
        ));
    }
    Ok(())
}

// Write::write_all retries Interrupted indefinitely. A stalled guest-facing
// harness writer must not pin a host worker. Bound consecutive retries, but
// reset the budget after positive progress; any failure invalidates the frame.
const WINDOWS_VM_AGENT_MAX_CONSECUTIVE_WRITE_INTERRUPTS: usize = 32;

fn write_all_guest_agent_bounded<W: Write>(writer: &mut W, mut remaining: &[u8]) -> io::Result<()> {
    let mut consecutive_interrupts = 0;
    while !remaining.is_empty() {
        match writer.write(remaining) {
            Ok(0) => return Err(io::ErrorKind::WriteZero.into()),
            Ok(count) => {
                if count > remaining.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "guest-agent writer exceeded supplied length",
                    ));
                }
                remaining = &remaining[count..];
                consecutive_interrupts = 0;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                consecutive_interrupts += 1;
                if consecutive_interrupts > WINDOWS_VM_AGENT_MAX_CONSECUTIVE_WRITE_INTERRUPTS {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

// Bound consecutive EINTR retries on an untrusted guest-agent transport.
// read_exact retries Interrupted indefinitely; a peer that never yields bytes
// must not pin a host worker in a busy loop. Progress resets the retry budget.
const WINDOWS_VM_AGENT_MAX_CONSECUTIVE_INTERRUPTS: usize = 32;

fn read_exact_guest_agent_bounded<R: Read>(
    reader: &mut R,
    mut remaining: &mut [u8],
) -> io::Result<()> {
    let mut consecutive_interrupts = 0;
    while !remaining.is_empty() {
        match reader.read(remaining) {
            Ok(0) => return Err(io::ErrorKind::UnexpectedEof.into()),
            Ok(count) => {
                if count > remaining.len() {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "guest-agent reader exceeded requested length",
                    ));
                }
                let (_, tail) = std::mem::take(&mut remaining).split_at_mut(count);
                remaining = tail;
                consecutive_interrupts = 0;
            }
            Err(error) if error.kind() == io::ErrorKind::Interrupted => {
                consecutive_interrupts += 1;
                if consecutive_interrupts > WINDOWS_VM_AGENT_MAX_CONSECUTIVE_INTERRUPTS {
                    return Err(error);
                }
            }
            Err(error) => return Err(error),
        }
    }
    Ok(())
}

/// Admit one explicitly length-delimited hello frame from a caller-owned reader.
///
/// The transport must establish `frame_len` independently; this function
/// does not infer framing from EOF, start a socket, or grant guest execution.
/// Reject an oversized declaration before allocating or reading, and never
/// consume bytes from a following frame.
pub fn read_guest_agent_hello_frame<R: Read>(
    reader: &mut R,
    frame_len: usize,
    expected_guest_id: &str,
    expected_guest_revision: u64,
    expected_guest_sha256: &str,
    expected_workload_arch: &str,
) -> Result<GuestAgentHello, WindowsVmError> {
    if frame_len == 0 {
        return Err(WindowsVmError::InvalidGuest(
            "empty guest-agent hello frame",
        ));
    }
    if frame_len > WINDOWS_VM_AGENT_HELLO_MAX_BYTES {
        return Err(WindowsVmError::InvalidGuest(
            "guest-agent hello exceeds JSON size limit",
        ));
    }
    validate_expected_guest_agent_authority(
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )?;
    let mut frame = vec![0u8; frame_len];
    read_exact_guest_agent_bounded(reader, &mut frame).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            WindowsVmError::InvalidGuest("truncated guest-agent hello frame")
        } else {
            WindowsVmError::Io(error)
        }
    })?;
    parse_guest_agent_hello_json(
        &frame,
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )
}

/// Write one harness-only length-prefixed hello packet: a four-byte
/// big-endian payload length followed by bounded, validated JSON.
///
/// This framing is a deterministic host-side W8 Gate 6 fixture contract, not
/// a negotiated production guest protocol. It does not start a VM or attest
/// a Windows guest. A successful return includes Write::flush, so a buffered
/// writer cannot report packet completion before its underlying flush fails.
/// This is not a guest delivery acknowledgement. Any write or flush failure
/// invalidates the whole packet; the caller must discard the stream on error.
pub fn write_guest_agent_hello_packet<W: Write>(
    writer: &mut W,
    hello: &GuestAgentHello,
    expected_guest_id: &str,
    expected_guest_revision: u64,
    expected_guest_sha256: &str,
    expected_workload_arch: &str,
) -> Result<usize, WindowsVmError> {
    let payload = encode_guest_agent_hello_json(
        hello,
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )?;
    let length = u32::try_from(payload.len())
        .map_err(|_| WindowsVmError::InvalidGuest("guest-agent hello exceeds JSON size limit"))?;
    write_all_guest_agent_bounded(writer, &length.to_be_bytes())?;
    write_all_guest_agent_bounded(writer, &payload)?;
    flush_guest_agent_bounded(writer)?;
    Ok(4 + payload.len())
}

/// Read exactly one harness-only length-prefixed hello packet without
/// consuming bytes from a subsequent packet.
///
/// Validate caller-owned host authority before touching the stream. Reject
/// zero and oversized length declarations before allocation or payload read;
/// a truncated prefix or payload fails closed. This is not runtime guest proof.
pub fn read_guest_agent_hello_packet<R: Read>(
    reader: &mut R,
    expected_guest_id: &str,
    expected_guest_revision: u64,
    expected_guest_sha256: &str,
    expected_workload_arch: &str,
) -> Result<GuestAgentHello, WindowsVmError> {
    validate_expected_guest_agent_authority(
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )?;
    let mut prefix = [0u8; 4];
    read_exact_guest_agent_bounded(reader, &mut prefix).map_err(|error| {
        if error.kind() == io::ErrorKind::UnexpectedEof {
            WindowsVmError::InvalidGuest("truncated guest-agent hello length prefix")
        } else {
            WindowsVmError::Io(error)
        }
    })?;
    let length = u32::from_be_bytes(prefix) as usize;
    read_guest_agent_hello_frame(
        reader,
        length,
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )
}

pub fn validate_guest_agent_hello(
    hello: &GuestAgentHello,
    expected_guest_id: &str,
    expected_guest_revision: u64,
    expected_guest_sha256: &str,
    expected_workload_arch: &str,
) -> Result<(), WindowsVmError> {
    validate_expected_guest_agent_authority(
        expected_guest_id,
        expected_guest_revision,
        expected_guest_sha256,
        expected_workload_arch,
    )?;
    if hello.schema != WINDOWS_VM_AGENT_SCHEMA || hello.protocol_revision != 1 {
        return Err(WindowsVmError::InvalidGuest(
            "unsupported guest-agent protocol",
        ));
    }
    if hello.guest_id != expected_guest_id || hello.guest_revision != expected_guest_revision {
        return Err(WindowsVmError::InvalidGuest(
            "guest-agent identity mismatch",
        ));
    }
    if !canonical_sha256_hex(&hello.guest_sha256) || hello.guest_sha256 != expected_guest_sha256 {
        return Err(WindowsVmError::InvalidGuest("guest-agent digest mismatch"));
    }
    if !matches!(hello.workload_arch.as_str(), "x86" | "x86_64")
        || hello.workload_arch != expected_workload_arch
    {
        return Err(WindowsVmError::InvalidGuest(
            "guest-agent workload architecture mismatch",
        ));
    }
    Ok(())
}

fn runtime_path_has_unsafe_text(value: &str) -> bool {
    // QEMU option separators are parser authority. Bidirectional formatting
    // controls can also disguise the displayed path in a proof/review log.
    // Ordinary visible Unicode remains valid, subject to the AF_UNIX byte cap.
    let parser_delimiter = value
        .bytes()
        .any(|byte| byte == b',' || byte.is_ascii_control());
    let bidi_control = value.chars().any(|character| {
        matches!(
            character,
            '\u{061c}' | '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{206f}'
        )
    });
    // Default-ignorable joiners, variation selectors and zero-width marks
    // can create distinct runtime proof paths with indistinguishable display.
    // Reject them before proof-id construction or QEMU option construction.
    let invisible_control = value.chars().any(|character| {
        matches!(
            character,
            '\u{00ad}' | '\u{034f}' | '\u{115f}' | '\u{1160}' | '\u{17b4}'..='\u{17b5}'
                | '\u{180b}'..='\u{180f}'
                | '\u{200b}'..='\u{200d}' | '\u{2060}'..='\u{2064}' | '\u{3164}'
                | '\u{13430}'..='\u{1343f}' // Egyptian hieroglyph format controls alter proof rendering.
                | '\u{1bca0}'..='\u{1bca3}' // Shorthand format controls are invisible.
                | '\u{1d173}'..='\u{1d17a}' // Musical format controls hide in proof paths.
                | '\u{fe00}'..='\u{fe0f}' | '\u{feff}' | '\u{2800}' | '\u{ffa0}'
                | '\u{fff9}'..='\u{fffb}' // Interlinear annotation controls alter rendered proof text.
                | '\u{e0000}'..='\u{e007f}' // Default-ignorable Unicode language/tag controls.
                | '\u{e0100}'..='\u{e01ef}'
        )
    });
    // Unicode NEL, line separator and paragraph separator can split a
    // displayed proof/log entry even though none is an ASCII control byte.
    let unicode_line_break = value
        .chars()
        .any(|character| matches!(character, '\u{0085}' | '\u{2028}' | '\u{2029}'));
    // C1 control code points (including CSI/OSC-style terminal controls)
    // are not ASCII bytes but can alter proof/log rendering. Reject the
    // entire C1 range rather than relying on individual display aliases.
    let unicode_c1_control = value
        .chars()
        .any(|character| matches!(character, '\u{0080}'..='\u{009f}'));
    // Unicode space separators have no distinctive glyph in many proof
    // renderers. They can make different path bytes appear as the same
    // whitespace-delimited session identity. Preserve ordinary ASCII spaces
    // and visible Unicode, but deny these unbound spacing aliases.
    let unicode_space_alias = value.chars().any(|character| {
        matches!(
            character,
            '\u{00a0}' | '\u{1680}' | '\u{2000}'
                ..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
        )
    });
    // Unicode noncharacters are valid Rust scalar values but are reserved
    // from text interchange and have no stable visible proof identity. Deny
    // U+FDD0..FDEF and the final two code points of every Unicode plane.
    let unicode_noncharacter = value.chars().any(|character| {
        matches!(character, '\u{fdd0}'..='\u{fdef}') || (character as u32 & 0xffff) >= 0xfffe
    });
    parser_delimiter
        || unicode_noncharacter
        || bidi_control
        || invisible_control
        || unicode_line_break
        || unicode_c1_control
        || unicode_space_alias
}

fn validate_agent_socket_path(path: &Path) -> Result<(), WindowsVmError> {
    // Linux sockaddr_un.sun_path is 108 bytes including the trailing NUL.
    if path.as_os_str().as_bytes().len() >= 108 {
        return Err(WindowsVmError::InvalidPlan(
            "guest agent socket path exceeds AF_UNIX limit",
        ));
    }
    Ok(())
}

pub fn vm_session_paths(
    root: &Path,
    application_id: &str,
    session_id: &str,
) -> Result<VmSessionPaths, WindowsVmError> {
    fn safe_component(value: &str) -> bool {
        !value.is_empty()
            && value.len() <= 128
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_'))
    }

    if !root.is_absolute() {
        return Err(WindowsVmError::InvalidPlan("session root is not absolute"));
    }
    if root.parent().is_none() {
        return Err(WindowsVmError::InvalidPlan(
            "session root is filesystem root",
        ));
    }
    // The proof plan must never authorize a session root that QEMU would
    // later parse as extra -drive/-chardev options or cannot represent.
    // Reject these before producing a proof identity, not only at VM launch.
    if root.to_str().is_none_or(runtime_path_has_unsafe_text) {
        return Err(WindowsVmError::InvalidPlan(
            "session root contains QEMU option delimiters or invisible Unicode controls",
        ));
    }
    let root_text = root.as_os_str().to_string_lossy();
    if root_text.ends_with('/') || root_text.contains("//") {
        return Err(WindowsVmError::InvalidPlan("session root is not canonical"));
    }
    if root_text.split('/').any(|segment| segment == ".") {
        return Err(WindowsVmError::InvalidPlan(
            "session root contains current-directory traversal",
        ));
    }
    if root
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(WindowsVmError::InvalidPlan(
            "session root contains parent traversal",
        ));
    }
    let canonical_root = fs::canonicalize(root)
        .map_err(|_| WindowsVmError::InvalidPlan("session root must exist"))?;
    if canonical_root != root {
        return Err(WindowsVmError::InvalidPlan(
            "session root is a symlink alias",
        ));
    }
    let root_meta = fs::symlink_metadata(root)?;
    if !root_meta.file_type().is_dir() {
        return Err(WindowsVmError::InvalidPlan(
            "session root is not a directory",
        ));
    }
    // A foreign owner can replace a 0700 session root even when its mode
    // excludes group and other writes. Require custody before proof binding.
    let worker_uid = unsafe { libc::geteuid() };
    validate_runtime_owner(root_meta.uid(), worker_uid)?;
    // The session root itself must be private. An unrelated local principal
    // must not be able to substitute its application/session entries.
    if root_meta.permissions().mode() & 0o077 != 0 {
        return Err(WindowsVmError::InvalidPlan(
            "session root is shared-writable",
        ));
    }
    // A private root is not sufficient when a shared-writable parent can
    // rename it. Root-owned sticky temp directories protect foreign entries.
    for directory in root.ancestors().skip(1) {
        let parent = fs::symlink_metadata(directory)?;
        let mode = parent.permissions().mode();
        if !parent.file_type().is_dir() || !trusted_runtime_ancestor(parent.uid(), mode, worker_uid)
        {
            return Err(WindowsVmError::InvalidPlan(
                "session root ancestor is untrusted or invalid",
            ));
        }
    }
    if !safe_component(application_id) || !safe_component(session_id) {
        return Err(WindowsVmError::InvalidPlan("invalid session identity"));
    }
    let application_dir = root.join(application_id);
    let runtime_dir = application_dir.join(session_id);
    // An existing application/session directory must not redirect the VM
    // outside its scoped runtime root before a proof plan is bound to it.
    for path in [&application_dir, &runtime_dir] {
        match fs::symlink_metadata(path) {
            Ok(metadata) => {
                if !metadata.file_type().is_dir() || fs::canonicalize(path)? != *path {
                    return Err(WindowsVmError::InvalidPlan(
                        "session directory is not a canonical directory",
                    ));
                }
                validate_runtime_owner(metadata.uid(), worker_uid)?;
                if metadata.permissions().mode() & 0o077 != 0 {
                    return Err(WindowsVmError::InvalidPlan(
                        "session directory is shared-writable",
                    ));
                }
            }
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
    }
    let agent_socket = runtime_dir.join("agent.sock");
    validate_agent_socket_path(&agent_socket)?;
    Ok(VmSessionPaths {
        overlay_image: runtime_dir.join("overlay.qcow2"),
        agent_socket,
        runtime_dir,
    })
}

const MANDATORY_RUNTIME_PROOF_CASES: [VmRuntimeAdversarialCase; 4] = [
    VmRuntimeAdversarialCase::ConcurrentSessionIsolation,
    VmRuntimeAdversarialCase::ForcedProcessTermination,
    VmRuntimeAdversarialCase::CorruptGuestAuthority,
    VmRuntimeAdversarialCase::DeniedUsbNode,
];

pub fn prepare_runtime_proof_plan(
    guest: &ValidatedVmGuest,
    root: &Path,
    application_id: &str,
    session_id: &str,
) -> Result<VmRuntimeProofPlan, WindowsVmError> {
    // ValidatedVmGuest is public and may have been mutated after admission.
    // Revalidate its identity, resource bounds, and image before issuing proof authority.
    let base_image = guest
        .base_image
        .to_str()
        .ok_or(WindowsVmError::InvalidPlan(
            "runtime proof guest image path is not UTF-8",
        ))?;
    let revalidated = validate_guest_definition(&VmGuestDefinition {
        schema: WINDOWS_VM_GUEST_SCHEMA.to_owned(),
        guest_id: guest.guest_id.clone(),
        revision: guest.revision,
        base_image: base_image.to_owned(),
        base_sha256: guest.base_sha256.clone(),
        memory_mib: guest.memory_mib,
        vcpus: guest.vcpus,
    })?;
    if revalidated != *guest {
        return Err(WindowsVmError::InvalidPlan(
            "runtime proof guest changed after validation",
        ));
    }
    if !root.is_absolute() {
        return Err(WindowsVmError::InvalidPlan(
            "runtime proof root must be absolute",
        ));
    }
    let paths = vm_session_paths(root, application_id, session_id)?;
    // A fresh proof session must not inherit an existing overlay. Reusing an
    // untrusted overlay could carry residual guest state into a new proof.
    // symlink_metadata also rejects dangling aliases without following them.
    match fs::symlink_metadata(&paths.overlay_image) {
        Ok(_) => {
            return Err(WindowsVmError::InvalidPlan(
                "runtime proof overlay path is already occupied",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    // Proof preparation must not bless a preoccupied guest-agent endpoint.
    // Use symlink_metadata so even a dangling symlink is rejected.
    match fs::symlink_metadata(&paths.agent_socket) {
        Ok(_) => {
            return Err(WindowsVmError::InvalidPlan(
                "runtime proof guest agent socket path is already occupied",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let required_cases = MANDATORY_RUNTIME_PROOF_CASES.to_vec();
    let proof_id = runtime_proof_id(
        &guest.guest_id,
        guest.revision,
        &guest.base_sha256,
        application_id,
        session_id,
        &required_cases,
        &paths,
    );
    Ok(VmRuntimeProofPlan {
        proof_id,
        guest_id: guest.guest_id.clone(),
        guest_revision: guest.revision,
        guest_sha256: guest.base_sha256.clone(),
        application_id: application_id.to_owned(),
        session_id: session_id.to_owned(),
        paths,
        required_cases,
    })
}

pub fn create_runtime_proof_observation_from_evidence(
    plan: &VmRuntimeProofPlan,
    case: VmRuntimeAdversarialCase,
    passed: bool,
    zero_residual_state: bool,
    evidence: &[u8],
) -> Result<VmRuntimeProofObservation, WindowsVmError> {
    if evidence.len() > WINDOWS_VM_RUNTIME_EVIDENCE_MAX_BYTES {
        return Err(WindowsVmError::InvalidPlan(
            "runtime proof evidence exceeds size limit",
        ));
    }
    let envelope = VmRuntimeEvidenceEnvelope {
        schema: WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA.to_owned(),
        proof_id: plan.proof_id.clone(),
        case: case.clone(),
        guest_id: plan.guest_id.clone(),
        guest_revision: plan.guest_revision,
        guest_sha256: plan.guest_sha256.clone(),
        application_id: plan.application_id.clone(),
        session_id: plan.session_id.clone(),
        evidence: evidence.to_vec(),
    };
    create_runtime_proof_observation_from_envelope(plan, &envelope, passed, zero_residual_state)
}

pub fn create_runtime_proof_observation_from_envelope(
    plan: &VmRuntimeProofPlan,
    envelope: &VmRuntimeEvidenceEnvelope,
    passed: bool,
    zero_residual_state: bool,
) -> Result<VmRuntimeProofObservation, WindowsVmError> {
    if envelope.schema != WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA
        || envelope.proof_id != plan.proof_id
        || envelope.guest_id != plan.guest_id
        || envelope.guest_revision != plan.guest_revision
        || envelope.guest_sha256 != plan.guest_sha256
        || envelope.application_id != plan.application_id
        || envelope.session_id != plan.session_id
        || !plan.required_cases.contains(&envelope.case)
    {
        return Err(WindowsVmError::InvalidPlan(
            "runtime proof evidence envelope does not match plan",
        ));
    }
    if envelope.evidence.is_empty() {
        return Err(WindowsVmError::InvalidPlan(
            "runtime proof evidence is empty",
        ));
    }
    if envelope.evidence.len() > WINDOWS_VM_RUNTIME_EVIDENCE_MAX_BYTES {
        return Err(WindowsVmError::InvalidPlan(
            "runtime proof evidence exceeds size limit",
        ));
    }
    let evidence_sha256 = format!("{:x}", Sha256::digest(&envelope.evidence));
    create_runtime_proof_observation(
        plan,
        envelope.case.clone(),
        passed,
        zero_residual_state,
        &evidence_sha256,
    )
}

fn create_runtime_proof_observation(
    plan: &VmRuntimeProofPlan,
    case: VmRuntimeAdversarialCase,
    passed: bool,
    zero_residual_state: bool,
    evidence_sha256: &str,
) -> Result<VmRuntimeProofObservation, WindowsVmError> {
    if !canonical_sha256_hex(evidence_sha256) {
        return Err(WindowsVmError::InvalidPlan(
            "runtime proof evidence digest is not canonical SHA-256",
        ));
    }
    if !plan.required_cases.contains(&case) {
        return Err(WindowsVmError::InvalidPlan(
            "runtime proof case is not required by plan",
        ));
    }
    let mut observation = VmRuntimeProofObservation {
        proof_id: plan.proof_id.clone(),
        case,
        guest_id: plan.guest_id.clone(),
        guest_revision: plan.guest_revision,
        guest_sha256: plan.guest_sha256.clone(),
        application_id: plan.application_id.clone(),
        session_id: plan.session_id.clone(),
        passed,
        zero_residual_state,
        evidence_schema: WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA.to_owned(),
        evidence_sha256: evidence_sha256.to_owned(),
        observation_id: String::new(),
    };
    observation.observation_id = runtime_observation_id(&observation);
    Ok(observation)
}

pub fn evaluate_runtime_proof(
    plan: &VmRuntimeProofPlan,
    observations: &[VmRuntimeProofObservation],
) -> VmRuntimeProofState {
    let expected_proof_id = runtime_proof_id(
        &plan.guest_id,
        plan.guest_revision,
        &plan.guest_sha256,
        &plan.application_id,
        &plan.session_id,
        &plan.required_cases,
        &plan.paths,
    );
    // The plan digest is public and can be recomputed after path tampering.
    // Reconstruct the canonical application/session paths instead of trusting
    // self-consistent but unscoped overlay or agent socket paths.
    let Some(root) = plan.paths.runtime_dir.parent().and_then(Path::parent) else {
        return VmRuntimeProofState::Pending;
    };
    let Ok(expected_paths) = vm_session_paths(root, &plan.application_id, &plan.session_id) else {
        return VmRuntimeProofState::Pending;
    };
    if plan.paths != expected_paths {
        return VmRuntimeProofState::Pending;
    }

    if plan.required_cases.as_slice() != MANDATORY_RUNTIME_PROOF_CASES.as_slice()
        || plan.guest_id.is_empty()
        || plan.guest_id.len() > 128
        || !plan
            .guest_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || plan.guest_revision == 0
        || !canonical_sha256_hex(&plan.guest_sha256)
        // A public proof ID can be recomputed for an unbound placeholder.
        // Gate 6 rejects the zero digest at the hello boundary; Gate 8 must
        // independently reject it even when all observations are rehashed.
        || plan.guest_sha256.bytes().all(|byte| byte == b'0')
        || plan.proof_id != expected_proof_id
        || observations.len() != plan.required_cases.len()
    {
        return VmRuntimeProofState::Pending;
    }

    let mut evidence_digests = std::collections::HashSet::new();
    for observation in observations {
        if !evidence_digests.insert(observation.evidence_sha256.as_str()) {
            return VmRuntimeProofState::Pending;
        }
    }

    for required in &plan.required_cases {
        let matches: Vec<_> = observations
            .iter()
            .filter(|observation| observation.case == *required)
            .collect();
        if matches.len() != 1 {
            return VmRuntimeProofState::Pending;
        }
        let observation = matches[0];
        let expected_observation_id = runtime_observation_id(observation);
        if observation.proof_id != plan.proof_id
            || observation.observation_id != expected_observation_id
            || observation.evidence_schema != WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA
            || !canonical_sha256_hex(&observation.evidence_sha256)
            || observation.guest_id != plan.guest_id
            || observation.guest_revision != plan.guest_revision
            || observation.guest_sha256 != plan.guest_sha256
            || observation.application_id != plan.application_id
            || observation.session_id != plan.session_id
            || !observation.passed
            || !observation.zero_residual_state
        {
            return VmRuntimeProofState::Pending;
        }
    }

    VmRuntimeProofState::Pass
}

fn runtime_proof_id(
    guest_id: &str,
    guest_revision: u64,
    guest_sha256: &str,
    application_id: &str,
    session_id: &str,
    required_cases: &[VmRuntimeAdversarialCase],
    paths: &VmSessionPaths,
) -> String {
    let body = serde_json::json!({
        "schema": WINDOWS_VM_RUNTIME_PROOF_SCHEMA,
        "guest_id": guest_id,
        "guest_revision": guest_revision,
        "guest_sha256": guest_sha256,
        "application_id": application_id,
        "session_id": session_id,
        "required_cases": required_cases,
        "runtime_dir": paths.runtime_dir,
        "overlay_image": paths.overlay_image,
        "agent_socket": paths.agent_socket,
    });
    sha256_json(&body)
}

fn runtime_observation_id(observation: &VmRuntimeProofObservation) -> String {
    let body = serde_json::json!({
        "schema": WINDOWS_VM_RUNTIME_OBSERVATION_SCHEMA,
        "proof_id": observation.proof_id,
        "case": observation.case,
        "guest_id": observation.guest_id,
        "guest_revision": observation.guest_revision,
        "guest_sha256": observation.guest_sha256,
        "application_id": observation.application_id,
        "session_id": observation.session_id,
        "passed": observation.passed,
        "zero_residual_state": observation.zero_residual_state,
        "evidence_schema": observation.evidence_schema,
        "evidence_sha256": observation.evidence_sha256,
    });
    sha256_json(&body)
}

fn sha256_json(value: &serde_json::Value) -> String {
    let bytes = serde_json::to_vec(value).expect("runtime proof identity JSON is serializable");
    format!("{:x}", Sha256::digest(bytes))
}

fn canonical_sha256_hex(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

pub fn validate_vm_profile(profile: &ApplicationProfile) -> Result<(), WindowsVmError> {
    if profile.execution_backend != ExecutionBackend::Vm {
        return Err(WindowsVmError::InvalidProfile(
            "execution_backend is not VM",
        ));
    }
    if profile.artifact.runtime_family != RuntimeFamily::Windows {
        return Err(WindowsVmError::InvalidProfile(
            "runtime family is not WINDOWS",
        ));
    }
    if !matches!(
        profile.artifact.format,
        ArtifactFormat::Pe32 | ArtifactFormat::Pe32Plus
    ) {
        return Err(WindowsVmError::InvalidProfile(
            "artifact format is not PE32/PE32+",
        ));
    }
    if !matches!(
        profile.artifact.workload_arch.as_deref(),
        Some("x86" | "x86_64")
    ) {
        return Err(WindowsVmError::InvalidProfile(
            "workload architecture is not x86/x86_64",
        ));
    }
    // The format and architecture are one binding, not two independent
    // allowlists: a PE32+ x86 or PE32 x86_64 profile would authorize a
    // workload whose declared image format cannot match its execution lane.
    if !matches!(
        (
            &profile.artifact.format,
            profile.artifact.workload_arch.as_deref()
        ),
        (ArtifactFormat::Pe32, Some("x86")) | (ArtifactFormat::Pe32Plus, Some("x86_64"))
    ) {
        return Err(WindowsVmError::InvalidProfile(
            "PE format and workload architecture mismatch",
        ));
    }
    if matches!(
        profile.compatibility.state,
        MechanicalCompatibilityState::Unknown
            | MechanicalCompatibilityState::Broken
            | MechanicalCompatibilityState::Unsupported
            | MechanicalCompatibilityState::RequiresRemoteProvider
    ) {
        return Err(WindowsVmError::InvalidProfile(
            "compatibility state is not launchable",
        ));
    }
    if profile.revoked {
        return Err(WindowsVmError::InvalidProfile("profile is revoked"));
    }
    Ok(())
}

// A guest-image pathname can be swapped after its regular-file admission.
// Ordinary File::open would wait indefinitely on a replacement FIFO with no
// writer. Open without following symlinks or waiting on special files, then
// bind the descriptor to the admitted inode before hashing any guest bytes.
fn open_admitted_guest_image(
    image: &Path,
    admitted: &fs::Metadata,
) -> Result<fs::File, WindowsVmError> {
    use std::os::unix::fs::OpenOptionsExt;
    let opened = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(image)?;
    let actual = opened.metadata()?;
    if !actual.file_type().is_file()
        || actual.dev() != admitted.dev()
        || actual.ino() != admitted.ino()
        || actual.uid() != admitted.uid()
        || actual.gid() != admitted.gid()
        || actual.nlink() != admitted.nlink()
        || actual.len() != admitted.len()
        || actual.mode() != admitted.mode()
        || actual.mtime() != admitted.mtime()
        || actual.mtime_nsec() != admitted.mtime_nsec()
        || actual.ctime() != admitted.ctime()
        || actual.ctime_nsec() != admitted.ctime_nsec()
    {
        return Err(WindowsVmError::InvalidGuest(
            "base image changed before digest read",
        ));
    }
    Ok(opened)
}

// The digest must describe the same inode that was admitted and is still
// reachable at the declared guest-image pathname after the final read.
fn verify_guest_image_stable(
    image: &Path,
    opened: &fs::File,
    admitted: &fs::Metadata,
) -> Result<(), WindowsVmError> {
    let after_read = opened.metadata()?;
    let worker_uid = unsafe { libc::geteuid() };
    // A parent may be replaced with a symlink to the same inode while the
    // digest is being read. Recheck pathname and ancestor admission.
    if fs::canonicalize(image)? != image {
        return Err(WindowsVmError::InvalidGuest(
            "base image parent changed during digest verification",
        ));
    }
    for directory in image.ancestors().skip(1) {
        let parent = fs::symlink_metadata(directory)?;
        if !parent.file_type().is_dir() || parent.file_type().is_symlink() {
            return Err(WindowsVmError::InvalidGuest(
                "base image parent changed during digest verification",
            ));
        }
        let mode = parent.permissions().mode();
        if !trusted_runtime_ancestor(parent.uid(), mode, worker_uid) {
            return Err(WindowsVmError::InvalidGuest(
                "base image parent became untrusted during digest verification",
            ));
        }
    }
    let after_path = fs::symlink_metadata(image)?;
    let same_identity = |current: &fs::Metadata| {
        current.file_type().is_file()
            && current.dev() == admitted.dev()
            && current.ino() == admitted.ino()
            && current.uid() == admitted.uid()
            && current.gid() == admitted.gid()
            && current.nlink() == 1
            && current.len() == admitted.len()
            && current.mtime() == admitted.mtime()
            && current.mtime_nsec() == admitted.mtime_nsec()
            && current.ctime() == admitted.ctime()
            && current.ctime_nsec() == admitted.ctime_nsec()
            && current.mode() == admitted.mode()
    };
    if !same_identity(&after_read) || !same_identity(&after_path) {
        return Err(WindowsVmError::InvalidGuest(
            "base image changed during digest verification",
        ));
    }
    Ok(())
}

// Do not let a pathname swap from a regular overlay to a FIFO stall W8
// preflight while waiting for an attacker-controlled writer. A no-follow,
// nonblocking descriptor must still match the admitted regular inode before
// any header bytes are consumed. This is not QEMU launch-time authority.
fn open_admitted_overlay_header(
    overlay: &Path,
    admitted: &fs::Metadata,
) -> Result<fs::File, WindowsVmError> {
    use std::os::unix::fs::OpenOptionsExt;
    let opened = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(overlay)?;
    // Check the opened descriptor before reading; preserve the established
    // runtime-ancestor validation order in build_qemu_plan below.
    let actual = opened.metadata()?;
    if !actual.file_type().is_file()
        || actual.dev() != admitted.dev()
        || actual.ino() != admitted.ino()
        || actual.uid() != admitted.uid()
        || actual.gid() != admitted.gid()
        || actual.nlink() != admitted.nlink()
        || actual.len() != admitted.len()
        || actual.mode() != admitted.mode()
        || actual.mtime() != admitted.mtime()
        || actual.mtime_nsec() != admitted.mtime_nsec()
        || actual.ctime() != admitted.ctime()
        || actual.ctime_nsec() != admitted.ctime_nsec()
    {
        return Err(WindowsVmError::InvalidPlan(
            "overlay changed before header read",
        ));
    }
    Ok(opened)
}

// A qcow2 header read is not authority if the overlay pathname is swapped
// while it is being inspected. This is a preflight snapshot, not a guarantee
// about later QEMU execution (which needs its own admission boundary).
fn verify_overlay_stable(
    overlay: &Path,
    opened: &fs::File,
    admitted: &fs::Metadata,
) -> Result<(), WindowsVmError> {
    if fs::canonicalize(overlay)? != overlay {
        return Err(WindowsVmError::InvalidPlan(
            "overlay path changed during header verification",
        ));
    }
    let worker_uid = unsafe { libc::geteuid() };
    for directory in overlay.ancestors().skip(1) {
        let parent = fs::symlink_metadata(directory)?;
        if !parent.file_type().is_dir()
            || !trusted_runtime_ancestor(parent.uid(), parent.permissions().mode(), worker_uid)
        {
            return Err(WindowsVmError::InvalidPlan(
                "overlay parent changed during header verification",
            ));
        }
    }
    let after_open = opened.metadata()?;
    let after_path = fs::symlink_metadata(overlay)?;
    let unchanged = |current: &fs::Metadata| {
        current.file_type().is_file()
            && current.dev() == admitted.dev()
            && current.ino() == admitted.ino()
            && current.uid() == admitted.uid()
            && current.gid() == admitted.gid()
            && current.nlink() == 1
            && current.len() == admitted.len()
            && current.mtime() == admitted.mtime()
            && current.mtime_nsec() == admitted.mtime_nsec()
            && current.ctime() == admitted.ctime()
            && current.ctime_nsec() == admitted.ctime_nsec()
            && current.mode() == admitted.mode()
    };
    if !unchanged(&after_open) || !unchanged(&after_path) {
        return Err(WindowsVmError::InvalidPlan(
            "overlay changed during header verification",
        ));
    }
    Ok(())
}

pub fn validate_guest_definition(
    definition: &VmGuestDefinition,
) -> Result<ValidatedVmGuest, WindowsVmError> {
    if definition.schema != WINDOWS_VM_GUEST_SCHEMA {
        return Err(WindowsVmError::InvalidGuest("unsupported schema"));
    }
    if definition.guest_id.trim().is_empty()
        || definition.guest_id.trim() != definition.guest_id
        || definition.guest_id.len() > 128
        || !definition
            .guest_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
        || definition.revision == 0
    {
        return Err(WindowsVmError::InvalidGuest("invalid guest identity"));
    }
    if !(512..=32768).contains(&definition.memory_mib) || !(1..=16).contains(&definition.vcpus) {
        return Err(WindowsVmError::InvalidGuest("resource bounds exceeded"));
    }
    if !canonical_sha256_hex(&definition.base_sha256) {
        return Err(WindowsVmError::InvalidGuest("invalid canonical sha256"));
    }
    let image = PathBuf::from(&definition.base_image);
    let worker_uid = unsafe { libc::geteuid() };
    if !image.is_absolute() {
        return Err(WindowsVmError::InvalidGuest("base image is not absolute"));
    }
    // The immutable guest-image authority must obey the same pathname
    // visibility contract as runtime proof roots and QEMU plan paths.
    // A digest match alone cannot make a bidi/invisible alias auditable.
    if runtime_path_has_unsafe_text(&definition.base_image) {
        return Err(WindowsVmError::InvalidGuest(
            "base image path contains QEMU option delimiters or invisible Unicode controls",
        ));
    }
    if fs::canonicalize(&image)? != image {
        return Err(WindowsVmError::InvalidGuest(
            "base image path is not canonical",
        ));
    }
    // A private image inode is not sufficient if another principal can
    // replace its pathname through a writable ancestor. The root-owned,
    // sticky temporary directories (/tmp and /var/tmp) are exceptions:
    // their sticky bit prevents other users from renaming this user's files.
    for directory in image.ancestors().skip(1) {
        let parent = fs::symlink_metadata(directory)?;
        if !parent.file_type().is_dir() || parent.file_type().is_symlink() {
            return Err(WindowsVmError::InvalidGuest(
                "base image directory is not a real directory",
            ));
        }
        // Reject foreign-owned read-only parents before digest I/O, not
        // only in the post-read identity check. Their owner can chmod and
        // replace the image entry while the digest is being computed.
        if !trusted_runtime_ancestor(parent.uid(), parent.permissions().mode(), worker_uid) {
            return Err(WindowsVmError::InvalidGuest(
                "base image directory is untrusted",
            ));
        }
    }
    let metadata = fs::symlink_metadata(&image)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(WindowsVmError::InvalidGuest(
            "base image is not a regular non-symlink file",
        ));
    }
    // A foreign owner can chmod and rewrite its own read-only image after
    // digest admission; the worker or root must own the guest inode.
    if !trusted_guest_image_owner(metadata.uid(), worker_uid) {
        return Err(WindowsVmError::InvalidGuest(
            "base image has an untrusted owner",
        ));
    }
    // A hardlinked guest image can alias unrelated host data despite a
    // canonical path and matching digest; reject shared inode ownership.
    if metadata.nlink() != 1 {
        return Err(WindowsVmError::InvalidGuest(
            "base image has a hardlink alias",
        ));
    }
    // The authenticated base must not be writable by other local principals.
    // Otherwise its bytes may change between digest verification and VM use.
    if metadata.permissions().mode() & 0o022 != 0 {
        return Err(WindowsVmError::InvalidGuest(
            "base image is shared-writable",
        ));
    }
    // A digest can authenticate zero bytes, but zero bytes cannot form a
    // guest disk. Reject it before admitting the VM authority.
    if metadata.len() == 0 {
        return Err(WindowsVmError::InvalidGuest("base image is empty"));
    }
    let mut file = open_admitted_guest_image(&image, &metadata)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    verify_guest_image_stable(&image, &file, &metadata)?;
    let actual = format!("{:x}", hasher.finalize());
    if actual != definition.base_sha256 {
        return Err(WindowsVmError::GuestDigestMismatch);
    }
    Ok(ValidatedVmGuest {
        guest_id: definition.guest_id.clone(),
        revision: definition.revision,
        base_image: image,
        base_sha256: actual,
        memory_mib: definition.memory_mib,
        vcpus: definition.vcpus,
    })
}

// Runtime ancestors may be owned by the worker or by root. A foreign owner
// can replace descendants even when its directory is mode 0755; only root's
// sticky shared directories are an exception to the writable-parent rule.
fn trusted_guest_image_owner(owner_uid: u32, worker_uid: u32) -> bool {
    owner_uid == worker_uid || owner_uid == 0
}

fn trusted_runtime_ancestor(owner_uid: u32, mode: u32, worker_uid: u32) -> bool {
    trusted_qemu_executable_owner(owner_uid, worker_uid)
        && (mode & 0o022 == 0 || (mode & 0o1000 != 0 && owner_uid == 0))
}

// QEMU is an executable authority boundary: an unrelated owner can replace
// its own mode-0755 binary or parent directory after preflight.
fn trusted_qemu_executable_owner(owner_uid: u32, worker_uid: u32) -> bool {
    owner_uid == worker_uid || owner_uid == 0
}

// Linux file capabilities can grant process privileges without setuid/setgid
// mode bits. Probe the already-open descriptor, not a pathname that may be
// swapped between admission and launch. An unsupported/failed xattr query is
// not evidence that the executable has no capabilities.
fn reject_qemu_privilege_xattr(opened: &fs::File, name: &CStr) -> Result<(), WindowsVmError> {
    let size =
        unsafe { libc::fgetxattr(opened.as_raw_fd(), name.as_ptr(), std::ptr::null_mut(), 0) };
    if size >= 0 {
        return Err(WindowsVmError::InvalidPlan(
            "QEMU executable has privileged file capabilities",
        ));
    }
    let error = io::Error::last_os_error();
    if error.raw_os_error() == Some(libc::ENODATA) {
        Ok(())
    } else {
        Err(error.into())
    }
}

fn reject_qemu_file_capabilities(opened: &fs::File) -> Result<(), WindowsVmError> {
    let capability = c"security.capability";
    reject_qemu_privilege_xattr(opened, capability)
}

// A QEMU pathname admitted before plan construction must still refer to the
// same executable inode afterwards. This is a preflight snapshot only;
// the process launcher must separately enforce its own admission boundary.
fn verify_qemu_stable(
    qemu: &Path,
    opened: &fs::File,
    admitted: &fs::Metadata,
) -> Result<(), WindowsVmError> {
    if fs::canonicalize(qemu)? != qemu {
        return Err(WindowsVmError::InvalidPlan(
            "QEMU path changed during plan construction",
        ));
    }
    let worker_uid = unsafe { libc::geteuid() };
    for directory in qemu.ancestors().skip(1) {
        let parent = fs::symlink_metadata(directory)?;
        if !parent.file_type().is_dir()
            || !trusted_runtime_ancestor(parent.uid(), parent.permissions().mode(), worker_uid)
        {
            return Err(WindowsVmError::InvalidPlan(
                "QEMU ancestor changed during plan construction",
            ));
        }
    }
    let after_open = opened.metadata()?;
    let after_path = fs::symlink_metadata(qemu)?;
    let unchanged = |current: &fs::Metadata| {
        current.file_type().is_file()
            && current.dev() == admitted.dev()
            && current.ino() == admitted.ino()
            && current.uid() == admitted.uid()
            && current.gid() == admitted.gid()
            && current.nlink() == 1
            && current.len() == admitted.len()
            && current.mtime() == admitted.mtime()
            && current.mtime_nsec() == admitted.mtime_nsec()
            && current.ctime() == admitted.ctime()
            && current.ctime_nsec() == admitted.ctime_nsec()
            && current.mode() == admitted.mode()
    };
    if !unchanged(&after_open) || !unchanged(&after_path) {
        return Err(WindowsVmError::InvalidPlan(
            "QEMU executable changed during plan construction",
        ));
    }
    reject_qemu_file_capabilities(opened)?;
    Ok(())
}

// A private mode is not ownership: a different local principal can modify
// its own 0600 file or 0700 directory after this worker has inspected it.
fn validate_runtime_owner(owner_uid: u32, worker_uid: u32) -> Result<(), WindowsVmError> {
    if owner_uid != worker_uid {
        return Err(WindowsVmError::InvalidPlan(
            "VM runtime state has a foreign owner",
        ));
    }
    Ok(())
}

pub fn build_qemu_plan(request: &VmPlanRequest) -> Result<QemuPlan, WindowsVmError> {
    if !request.qemu_binary.is_absolute() {
        return Err(WindowsVmError::InvalidPlan("QEMU path is not absolute"));
    }
    let qemu = fs::canonicalize(&request.qemu_binary)?;
    // A canonical pathname is not proof that the executable remains regular:
    // a FIFO substituted before open would block forever without a writer.
    // Reject symlink replacement and open nonblocking before inspecting the
    // descriptor; the final identity check below still binds the pathname.
    use std::os::unix::fs::OpenOptionsExt;
    let qemu_file = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(&qemu)?;
    let qemu_meta = qemu_file.metadata()?;
    let worker_uid = unsafe { libc::geteuid() };
    if !trusted_qemu_executable_owner(qemu_meta.uid(), worker_uid) {
        return Err(WindowsVmError::InvalidPlan(
            "QEMU executable has an untrusted owner",
        ));
    }
    if !qemu_meta.is_file() || qemu_meta.permissions().mode() & 0o111 == 0 {
        return Err(WindowsVmError::InvalidPlan("QEMU is not executable"));
    }
    // A set-user-ID or set-group-ID executable can change the identity of
    // the eventual QEMU process even when its inode and directory are trusted.
    // W8 has no privilege-transition authority: reject these mode bits before
    // constructing a launchable plan. The final identity check binds mode.
    if qemu_meta.permissions().mode() & 0o6000 != 0 {
        return Err(WindowsVmError::InvalidPlan(
            "QEMU executable has set-id permission bits",
        ));
    }
    // A shared-writable QEMU executable is mutable by another local principal
    // after its path is canonicalized and before the VM process starts.
    if qemu_meta.permissions().mode() & 0o022 != 0 {
        return Err(WindowsVmError::InvalidPlan(
            "QEMU executable is shared-writable",
        ));
    }
    // Hardlink aliases can expose an otherwise private QEMU inode to
    // unrelated host paths after executable admission.
    if qemu_meta.nlink() != 1 {
        return Err(WindowsVmError::InvalidPlan(
            "QEMU executable has a hardlink alias",
        ));
    }
    reject_qemu_file_capabilities(&qemu_file)?;
    for directory in qemu.ancestors().skip(1) {
        let parent = fs::symlink_metadata(directory)?;
        let mode = parent.permissions().mode();
        if !parent.file_type().is_dir() || !trusted_runtime_ancestor(parent.uid(), mode, worker_uid)
        {
            return Err(WindowsVmError::InvalidPlan(
                "QEMU executable directory is untrusted or invalid",
            ));
        }
    }
    if !request.overlay_image.is_absolute() || !request.runtime_dir.is_absolute() {
        return Err(WindowsVmError::InvalidPlan(
            "runtime paths are not absolute",
        ));
    }
    // QEMU parses commas inside -drive and -chardev values as new options.
    // Refuse paths that could change the requested VM isolation contract.
    if [&request.overlay_image, &request.runtime_dir]
        .iter()
        .any(|path| path.to_str().is_none_or(runtime_path_has_unsafe_text))
    {
        return Err(WindowsVmError::InvalidPlan(
            "runtime path contains QEMU option delimiters or invisible Unicode controls",
        ));
    }
    // Query the effective identity that will own the session, not the
    // environment's spoofable UID text. No privilege or identity is changed.
    let overlay_meta = fs::symlink_metadata(&request.overlay_image)?;
    if overlay_meta.file_type().is_symlink() || !overlay_meta.file_type().is_file() {
        return Err(WindowsVmError::InvalidPlan("overlay is not a regular file"));
    }
    validate_runtime_owner(overlay_meta.uid(), worker_uid)?;
    // A zero-byte overlay cannot contain a usable qcow2 header.
    if overlay_meta.len() == 0 {
        return Err(WindowsVmError::InvalidPlan("overlay image is empty"));
    }
    // A hardlinked overlay may alias unrelated host data even when its
    // canonical path is confined to the selected VM runtime directory.
    if overlay_meta.nlink() != 1 {
        return Err(WindowsVmError::InvalidPlan("overlay has a hardlink alias"));
    }
    // A nonempty file is not necessarily a qcow2 overlay. Reject malformed
    // magic, unsupported versions and truncated fixed headers before QEMU
    // receives a disk path. This is a header preflight, not a substitute for
    // qemu-img integrity checks or authorized guest-runtime qualification.
    if overlay_meta.len() < 72 {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 overlay header is truncated",
        ));
    }
    let mut overlay_file = open_admitted_overlay_header(&request.overlay_image, &overlay_meta)?;
    let mut qcow2_prefix = [0u8; 104];
    // Both qcow2 versions carry the backing-file pointer in their fixed
    // 72-byte header. A referenced backing image would let QEMU open host
    // storage that this plan never admitted or digest-bound.
    overlay_file.read_exact(&mut qcow2_prefix[..72])?;
    let qcow2_version = u32::from_be_bytes(qcow2_prefix[4..8].try_into().unwrap());
    if &qcow2_prefix[..4] != b"QFI\xfb"
        || !matches!(qcow2_version, 2 | 3)
        || (qcow2_version == 3 && overlay_meta.len() < 104)
    {
        return Err(WindowsVmError::InvalidPlan("invalid qcow2 overlay header"));
    }
    let backing_offset = u64::from_be_bytes(qcow2_prefix[8..16].try_into().unwrap());
    let backing_size = u32::from_be_bytes(qcow2_prefix[16..20].try_into().unwrap());
    if backing_offset != 0 || backing_size != 0 {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 overlay references an unbound external backing file",
        ));
    }
    // qcow2 v2/v3 internal snapshots carry additional guest-state tables
    // that W8 does not select, digest-bind, or isolate per application.
    // Refuse both populated and contradictory snapshot metadata rather than
    // allowing QEMU to interpret unadmitted state during launch.
    let snapshot_count = u32::from_be_bytes(qcow2_prefix[60..64].try_into().unwrap());
    let snapshot_offset = u64::from_be_bytes(qcow2_prefix[64..72].try_into().unwrap());
    if snapshot_count != 0 || snapshot_offset != 0 {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 overlay has unbound internal snapshot metadata",
        ));
    }
    // The W8 header-only admission has no key/secret authority. Even a
    // structurally valid encrypted qcow2 image cannot be handed to QEMU.
    let crypt_method = u32::from_be_bytes(qcow2_prefix[32..36].try_into().unwrap());
    if crypt_method != 0 {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 overlay requires unsupported encryption authority",
        ));
    }
    if qcow2_version == 3 {
        overlay_file.read_exact(&mut qcow2_prefix[72..])?;
        // The qcow2 v3 refcount width is 1 << refcount_order bits. QEMU's
        // documented format bounds the order at 6 (64-bit refcounts).
        // Reject impossible metadata before emitting an executable VM plan.
        let refcount_order = u32::from_be_bytes(qcow2_prefix[96..100].try_into().unwrap());
        if refcount_order > 6 {
            return Err(WindowsVmError::InvalidPlan(
                "qcow2 v3 refcount order exceeds supported width",
            ));
        }
        // qcow2 incompatible feature bit 2 enables a separate data file.
        // Its pathname is not part of this VM plan's admitted authority.
        let incompatible = u64::from_be_bytes(qcow2_prefix[72..80].try_into().unwrap());
        if incompatible & (1 << 2) != 0 {
            return Err(WindowsVmError::InvalidPlan(
                "qcow2 overlay references an unbound external data file",
            ));
        }
        // This bounded host preflight does not qualify dirty/corrupt images,
        // extended L2, alternate compression or unknown incompatible bits.
        // Keep these out of QEMU until an explicit feature admission exists.
        if incompatible != 0 {
            return Err(WindowsVmError::InvalidPlan(
                "qcow2 overlay has unsupported incompatible features",
            ));
        }
        // Compatible features (bytes 80..88) may change refcount persistence,
        // while autoclear features (bytes 88..96) may carry unadmitted bitmap
        // or metadata state. W8 has not qualified either feature family for
        // its session overlay, even though QEMU can recognize some bits.
        // Do not infer guest-state authority from a recognized feature bit.
        let compatible = u64::from_be_bytes(qcow2_prefix[80..88].try_into().unwrap());
        let autoclear = u64::from_be_bytes(qcow2_prefix[88..96].try_into().unwrap());
        if compatible != 0 || autoclear != 0 {
            return Err(WindowsVmError::InvalidPlan(
                "qcow2 overlay has unadmitted compatible or autoclear features",
            ));
        }
        // qcow2 v3 declares its header length at byte 100. A zero, short or
        // beyond-EOF declaration cannot describe the admitted on-disk header.
        let header_length = u32::from_be_bytes(qcow2_prefix[100..104].try_into().unwrap()) as u64;
        // qcow2 v3 headers are 8-byte aligned so extensions begin at a
        // canonical boundary. A short, unaligned or beyond-EOF declaration
        // must not reach QEMU as an admitted overlay.
        if header_length < 104
            || !header_length.is_multiple_of(8)
            || header_length > overlay_meta.len()
        {
            return Err(WindowsVmError::InvalidPlan(
                "invalid qcow2 v3 declared header length",
            ));
        }
    }
    // Magic and version alone do not describe a usable qcow2 disk. QEMU's
    // supported cluster size is 512 bytes through 2 MiB (bits 9..=21), and
    // the virtual disk size must be positive and fit the signed size domain.
    // Keep this host-only geometry preflight separate from full image proof.
    let cluster_bits = u32::from_be_bytes(qcow2_prefix[20..24].try_into().unwrap());
    let virtual_size = u64::from_be_bytes(qcow2_prefix[24..32].try_into().unwrap());
    if !(9..=21).contains(&cluster_bits) || virtual_size == 0 || virtual_size > i64::MAX as u64 {
        return Err(WindowsVmError::InvalidPlan(
            "invalid qcow2 cluster geometry or virtual size",
        ));
    }
    // The qcow2 L1 and refcount table pointers in the fixed v2/v3 header
    // identify cluster-based metadata, not arbitrary byte positions. A
    // nonzero unaligned pointer would let QEMU interpret bytes outside the
    // declared table cluster. Zero remains a header-only fixture value;
    // full table presence and guest bootability require separate proof.
    let cluster_size = 1u64 << cluster_bits;
    for table_offset in [
        u64::from_be_bytes(qcow2_prefix[40..48].try_into().unwrap()),
        u64::from_be_bytes(qcow2_prefix[48..56].try_into().unwrap()),
    ] {
        if table_offset != 0 && table_offset % cluster_size != 0 {
            return Err(WindowsVmError::InvalidPlan(
                "qcow2 metadata table offset is not cluster-aligned",
            ));
        }
    }
    // qcow2's L1 and refcount table counts are meaningful only when their
    // corresponding metadata pointers exist. A nonzero count with a null
    // pointer would leave QEMU to interpret an absent table. This is only
    // fixed-header coherence; it does not prove table contents or guest boot.
    let l1_size = u32::from_be_bytes(qcow2_prefix[36..40].try_into().unwrap());
    let l1_offset = u64::from_be_bytes(qcow2_prefix[40..48].try_into().unwrap());
    let refcount_offset = u64::from_be_bytes(qcow2_prefix[48..56].try_into().unwrap());
    let refcount_clusters = u32::from_be_bytes(qcow2_prefix[56..60].try_into().unwrap());
    // A nonzero pointer with a zero count is equally contradictory: W8
    // cannot grant authority to metadata that the header declares absent.
    if (l1_size == 0) != (l1_offset == 0) || (refcount_clusters == 0) != (refcount_offset == 0) {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 metadata table count/offset mismatch",
        ));
    }
    // Each L1 entry addresses one L2 cluster of cluster_size / 8 entries,
    // and each ordinary L2 entry addresses one data cluster. A declared
    // nonempty L1 table must cover the advertised virtual disk, even in a
    // header-only preflight. Zero L1 entries remain an explicit fixture-only
    // allowance here; they do not prove a bootable or qualified guest image.
    // The bounded cluster geometry keeps this multiplication within u64.
    let l1_entry_coverage = cluster_size * (cluster_size / 8);
    let required_l1_entries = virtual_size.div_ceil(l1_entry_coverage);
    if l1_size != 0 && u64::from(l1_size) < required_l1_entries {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 L1 table does not cover declared virtual size",
        ));
    }
    // A table larger than the disk requires can force disproportionate
    // QEMU metadata allocation before W8 has qualified the guest image.
    // Permit cluster-rounded L1 capacity, but not arbitrary extra clusters
    // reserved for unadmitted future disk growth. Zero remains the explicit
    // header-only fixture exception above, not a guest-bootability claim.
    let l1_entries_per_cluster = cluster_size / 8;
    let max_l1_entries =
        required_l1_entries.div_ceil(l1_entries_per_cluster) * l1_entries_per_cluster;
    if l1_size != 0 && u64::from(l1_size) > max_l1_entries {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 L1 table exceeds cluster-rounded virtual disk coverage",
        ));
    }
    // Refcount table clusters also allocate QEMU metadata. The header
    // alone must not grant a tiny virtual disk a multi-gigabyte refcount
    // table. Conservatively cap table clusters to the number of clusters
    // in the declared virtual disk. This is a W8 admission budget, not a
    // general qcow2-format prohibition on separately qualified images.
    // The virtual size and cluster geometry were already bounded above.
    let max_refcount_clusters = virtual_size.div_ceil(cluster_size);
    if u64::from(refcount_clusters) > max_refcount_clusters {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 refcount table exceeds virtual disk cluster budget",
        ));
    }
    // A coherent pointer/count pair can still alias the other table's
    // clusters. The L1 count is in 8-byte entries, while the refcount count
    // is in whole clusters. Reject overlap and address-space wrap without
    // claiming that either table's on-disk contents are qualified.
    let l1_bytes = u64::from(l1_size) * 8;
    let l1_clusters = l1_bytes.div_ceil(cluster_size);
    let l1_span = l1_clusters * cluster_size;
    let refcount_span = u64::from(refcount_clusters) * cluster_size;
    let l1_end = l1_offset
        .checked_add(l1_span)
        .ok_or(WindowsVmError::InvalidPlan(
            "qcow2 metadata table span overflows address space",
        ))?;
    let refcount_end =
        refcount_offset
            .checked_add(refcount_span)
            .ok_or(WindowsVmError::InvalidPlan(
                "qcow2 metadata table span overflows address space",
            ))?;
    // QEMU's block I/O offsets and lengths are signed 64-bit values.
    // A checked u64 span can still lie outside that address domain; do
    // not pass an unrepresentable metadata extent to the VM backend.
    if (l1_size != 0 && l1_end > i64::MAX as u64)
        || (refcount_clusters != 0 && refcount_end > i64::MAX as u64)
    {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 metadata table span exceeds signed file-offset range",
        ));
    }
    if l1_size != 0
        && refcount_clusters != 0
        && l1_offset < refcount_end
        && refcount_offset < l1_end
    {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 L1 and refcount metadata tables overlap",
        ));
    }
    // QEMU stores the entire v3 header inside the first qcow2 cluster.
    // A length within the physical file can still spill into the next
    // cluster, which is not a valid header geometry. The shift is safe
    // because cluster_bits has already passed the bounded range check.
    if qcow2_version == 3 {
        let declared_header_length =
            u32::from_be_bytes(qcow2_prefix[100..104].try_into().unwrap()) as u64;
        if declared_header_length > (1u64 << cluster_bits) {
            return Err(WindowsVmError::InvalidPlan(
                "qcow2 v3 declared header exceeds first cluster",
            ));
        }
        // The fixed v3 header ends at byte 104. A longer declared header
        // introduces fields that W8 does not yet interpret or authorize.
        // Permit zero-filled future space, but reject nonzero extended fields
        // before handing the overlay to QEMU. The geometry bound above caps
        // this read to one qcow2 cluster (at most 2 MiB).
        let extra_header_len = (declared_header_length - 104) as usize;
        let mut extra_header = vec![0u8; extra_header_len];
        overlay_file.read_exact(&mut extra_header)?;
        if extra_header.iter().any(|byte| *byte != 0) {
            return Err(WindowsVmError::InvalidPlan(
                "qcow2 v3 extended header contains unadmitted fields",
            ));
        }
    }
    // qcow2 header extensions follow the v2 fixed header or the v3
    // declared header, rather than being included in v3 header_length.
    // An extension can therefore be interpreted by QEMU even when every
    // declared-header byte is zero. W8 has no authority for those fields.
    // Inspect only the present bytes of the first cluster (at most 2 MiB).
    let admitted_header_end = if qcow2_version == 3 {
        u32::from_be_bytes(qcow2_prefix[100..104].try_into().unwrap()) as u64
    } else {
        72
    };
    let first_cluster_end = (1u64 << cluster_bits).min(overlay_meta.len());
    let extension_len = (first_cluster_end - admitted_header_end) as usize;
    let mut extension_space = vec![0u8; extension_len];
    overlay_file.read_exact(&mut extension_space)?;
    if extension_space.iter().any(|byte| *byte != 0) {
        return Err(WindowsVmError::InvalidPlan(
            "qcow2 overlay contains unadmitted header extensions",
        ));
    }
    // A shared-writable overlay can be replaced between plan validation and
    // QEMU admission, defeating session-scoped guest state integrity.
    if overlay_meta.permissions().mode() & 0o077 != 0 {
        return Err(WindowsVmError::InvalidPlan("overlay is shared-writable"));
    }
    let runtime_meta = fs::symlink_metadata(&request.runtime_dir)?;
    if runtime_meta.file_type().is_symlink() || !runtime_meta.file_type().is_dir() {
        return Err(WindowsVmError::InvalidPlan(
            "runtime directory is not a regular directory",
        ));
    }
    validate_runtime_owner(runtime_meta.uid(), worker_uid)?;
    // Shared-writable session directories permit agent-socket or overlay
    // substitution by another local principal after the plan is issued.
    if runtime_meta.permissions().mode() & 0o077 != 0 {
        return Err(WindowsVmError::InvalidPlan(
            "runtime directory is shared-writable",
        ));
    }
    // A private session directory is insufficient when another principal can
    // rename it through a shared-writable ancestor. Root-owned sticky temp
    // directories retain their standard protected-entry exception.
    for directory in request.runtime_dir.ancestors().skip(1) {
        let parent = fs::symlink_metadata(directory)?;
        let mode = parent.permissions().mode();
        if !parent.file_type().is_dir() || !trusted_runtime_ancestor(parent.uid(), mode, worker_uid)
        {
            return Err(WindowsVmError::InvalidPlan(
                "runtime directory ancestor is untrusted or invalid",
            ));
        }
    }
    // The VM overlay must belong to this session, not an unrelated host file.
    // Canonical equality also rejects traversal and symlinked parent directories.
    let runtime = fs::canonicalize(&request.runtime_dir)?;
    let overlay = fs::canonicalize(&request.overlay_image)?;
    if runtime != request.runtime_dir
        || overlay != request.overlay_image
        || overlay != runtime.join("overlay.qcow2")
    {
        return Err(WindowsVmError::InvalidPlan(
            "overlay escapes the selected VM runtime directory",
        ));
    }
    // Rebind the validated header to its original inode and pathname before
    // any VM arguments are constructed.
    verify_overlay_stable(&request.overlay_image, &overlay_file, &overlay_meta)?;
    if !(512..=32768).contains(&request.memory_mib) || !(1..=16).contains(&request.vcpus) {
        return Err(WindowsVmError::InvalidPlan("resource bounds exceeded"));
    }

    // The single qemu-xhci controller has bounded port capacity. Do not
    // silently deduplicate an oversized caller allowlist into apparent authority.
    if request.usb_nodes.len() > WINDOWS_VM_MAX_USB_PASSTHROUGH_NODES {
        return Err(WindowsVmError::InvalidPlan(
            "USB passthrough allowlist exceeds controller capacity",
        ));
    }
    let mut usb = Vec::new();
    for node in &request.usb_nodes {
        usb.push(parse_usb_node(node)?);
    }
    usb.sort_unstable();
    // The W7 allowlist is explicit authority, not a bag of requested
    // devices. Silently deduplicating a repeated node conceals malformed
    // caller state and changes the approved request before QEMU planning.
    if usb.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(WindowsVmError::InvalidPlan(
            "duplicate USB passthrough node in allowlist",
        ));
    }

    let agent = request.runtime_dir.join("agent.sock");
    validate_agent_socket_path(&agent)?;
    // Reject an already-occupied agent socket path before handing it to QEMU.
    // The runtime directory still requires exclusive ownership against races.
    match fs::symlink_metadata(&agent) {
        Ok(_) => {
            return Err(WindowsVmError::InvalidPlan(
                "guest agent socket path is already occupied",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error.into()),
    }
    let mut args = vec![
        "-accel".to_owned(),
        "kvm".to_owned(),
        "-machine".to_owned(),
        "q35".to_owned(),
        "-nodefaults".to_owned(),
        "-no-user-config".to_owned(),
        "-display".to_owned(),
        "none".to_owned(),
        "-net".to_owned(),
        "none".to_owned(),
        "-m".to_owned(),
        request.memory_mib.to_string(),
        "-smp".to_owned(),
        request.vcpus.to_string(),
        "-drive".to_owned(),
        format!(
            "file={},if=virtio,format=qcow2,cache=none",
            request.overlay_image.display()
        ),
        "-chardev".to_owned(),
        format!(
            "socket,id=primeagent,path={},server=on,wait=off",
            agent.display()
        ),
        "-device".to_owned(),
        "virtio-serial-pci".to_owned(),
        "-device".to_owned(),
        "virtserialport,chardev=primeagent,name=org.thetechguy.prime.agent".to_owned(),
    ];
    if !usb.is_empty() {
        args.extend(["-device".to_owned(), "qemu-xhci".to_owned()]);
        for (bus, device) in usb {
            args.extend([
                "-device".to_owned(),
                format!("usb-host,hostbus={bus},hostaddr={device}"),
            ]);
        }
    }
    // Reject a replacement after argument construction as well; QEMU launch
    // must still independently revalidate this admission snapshot.
    verify_overlay_stable(&request.overlay_image, &overlay_file, &overlay_meta)?;
    verify_qemu_stable(&qemu, &qemu_file, &qemu_meta)?;
    Ok(QemuPlan {
        program: qemu,
        args,
    })
}

fn parse_usb_node(path: &Path) -> Result<(u16, u16), WindowsVmError> {
    let raw = path
        .to_str()
        .ok_or_else(|| WindowsVmError::InvalidUsb("non-UTF8 path".to_owned()))?;
    let rest = raw
        .strip_prefix("/dev/bus/usb/")
        .ok_or_else(|| WindowsVmError::InvalidUsb(raw.to_owned()))?;
    let (bus, device) = rest
        .split_once('/')
        .ok_or_else(|| WindowsVmError::InvalidUsb(raw.to_owned()))?;
    if bus.len() != 3
        || device.len() != 3
        || !bus.bytes().all(|b| b.is_ascii_digit())
        || !device.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(WindowsVmError::InvalidUsb(raw.to_owned()));
    }
    let bus = bus
        .parse::<u16>()
        .map_err(|_| WindowsVmError::InvalidUsb(raw.to_owned()))?;
    let device = device
        .parse::<u16>()
        .map_err(|_| WindowsVmError::InvalidUsb(raw.to_owned()))?;
    // Linux USB bus numbers are allocated in 1..=255; device addresses are
    // seven-bit (1..=127). Reject impossible QEMU hostbus/hostaddr pairs.
    if !(1..=255).contains(&bus) || !(1..=127).contains(&device) {
        return Err(WindowsVmError::InvalidUsb(raw.to_owned()));
    }
    Ok((bus, device))
}

#[cfg(test)]
mod vm_runtime_owner_tests {
    use super::*;

    #[test]
    fn guest_digest_open_rejects_fifo_and_symlink_swaps_without_waiting() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("guest.qcow2");
        fs::write(&image, b"admitted guest bytes").unwrap();
        let admitted = fs::symlink_metadata(&image).unwrap();
        fs::remove_file(&image).unwrap();

        let c_path = CString::new(image.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(c_path.as_ptr(), 0o600) }, 0);
        // No writer exists: a blocking File::open would hang this regression.
        let error = open_admitted_guest_image(&image, &admitted).unwrap_err();
        assert!(matches!(error, WindowsVmError::InvalidGuest(_)));

        fs::remove_file(&image).unwrap();
        let replacement = dir.path().join("replacement.qcow2");
        fs::write(&replacement, b"untrusted guest bytes").unwrap();
        std::os::unix::fs::symlink(&replacement, &image).unwrap();
        assert!(open_admitted_guest_image(&image, &admitted).is_err());
    }

    #[test]
    fn qemu_privilege_xattr_probe_rejects_present_descriptor_attribute() {
        use std::ffi::CString;

        let dir = tempfile::tempdir().unwrap();
        let executable = dir.path().join("qemu-test");
        fs::write(&executable, b"test executable").unwrap();
        let opened = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&executable)
            .unwrap();
        let attribute = CString::new("user.ttg_w8_capability_probe").unwrap();
        assert_eq!(
            unsafe {
                libc::fsetxattr(
                    opened.as_raw_fd(),
                    attribute.as_ptr(),
                    b"present".as_ptr().cast(),
                    7,
                    0,
                )
            },
            0,
            "disposable fixture must support user xattrs"
        );
        let error = reject_qemu_privilege_xattr(&opened, &attribute).unwrap_err();
        assert!(error.to_string().contains("file capabilities"), "{error}");
        assert_eq!(
            unsafe { libc::fremovexattr(opened.as_raw_fd(), attribute.as_ptr()) },
            0
        );
        assert!(reject_qemu_privilege_xattr(&opened, &attribute).is_ok());
    }

    #[test]
    fn foreign_owned_guest_image_inode_is_not_trusted_by_readonly_mode() {
        let worker_uid = 1000;
        assert!(trusted_guest_image_owner(worker_uid, worker_uid));
        assert!(trusted_guest_image_owner(0, worker_uid));
        for foreign_uid in [1001, 65534] {
            assert!(!trusted_guest_image_owner(foreign_uid, worker_uid));
            assert!(!trusted_runtime_ancestor(foreign_uid, 0o755, worker_uid));
        }
    }

    #[test]
    fn foreign_owned_qemu_executable_is_not_trusted_by_mode_alone() {
        let worker_uid = 1000;
        assert!(trusted_qemu_executable_owner(worker_uid, worker_uid));
        assert!(trusted_qemu_executable_owner(0, worker_uid));
        for foreign_uid in [1001, 65534] {
            assert!(!trusted_qemu_executable_owner(foreign_uid, worker_uid));
            assert!(!trusted_runtime_ancestor(foreign_uid, 0o755, worker_uid));
        }
    }

    #[test]
    fn foreign_owned_runtime_ancestor_is_not_trusted_by_private_mode_alone() {
        let worker_uid = 1000;
        assert!(trusted_runtime_ancestor(worker_uid, 0o755, worker_uid));
        assert!(trusted_runtime_ancestor(0, 0o755, worker_uid));
        assert!(trusted_runtime_ancestor(0, 0o1777, worker_uid));
        for foreign_uid in [1001, 65534] {
            assert!(!trusted_runtime_ancestor(foreign_uid, 0o755, worker_uid));
            assert!(!trusted_runtime_ancestor(foreign_uid, 0o700, worker_uid));
            assert!(!trusted_runtime_ancestor(foreign_uid, 0o1777, worker_uid));
        }
        assert!(!trusted_runtime_ancestor(0, 0o777, worker_uid));
    }

    #[test]
    fn foreign_owned_runtime_state_is_not_admitted_by_private_mode_alone() {
        // The adversarial owner can set 0700/0600 on its own inode; modes
        // alone therefore cannot establish this worker's session custody.
        let worker_uid = 1000;
        assert!(validate_runtime_owner(worker_uid, worker_uid).is_ok());
        for foreign_uid in [0, 1001, 65534] {
            assert!(
                validate_runtime_owner(foreign_uid, worker_uid).is_err(),
                "foreign owner {foreign_uid} was admitted"
            );
        }
    }
}

#[cfg(test)]
mod guest_image_identity_tests {
    use super::*;

    #[test]
    fn usb_node_rejects_impossible_device_addresses() {
        assert_eq!(
            parse_usb_node(Path::new("/dev/bus/usb/001/127")).unwrap(),
            (1, 127)
        );
        for invalid in [
            "/dev/bus/usb/001/000",
            "/dev/bus/usb/001/128",
            "/dev/bus/usb/001/255",
            "/dev/bus/usb/001/999",
        ] {
            assert!(parse_usb_node(Path::new(invalid)).is_err(), "{invalid}");
        }
    }

    #[test]
    fn usb_node_rejects_impossible_bus_numbers() {
        // Linux USB bus IDs are allocated in the 1..=255 range.
        assert_eq!(
            parse_usb_node(Path::new("/dev/bus/usb/255/001")).unwrap(),
            (255, 1)
        );
        for invalid in [
            "/dev/bus/usb/000/001",
            "/dev/bus/usb/256/001",
            "/dev/bus/usb/999/001",
        ] {
            assert!(parse_usb_node(Path::new(invalid)).is_err(), "{invalid}");
        }
    }

    use std::os::unix::fs::PermissionsExt;

    #[test]
    fn qemu_executable_path_replacement_is_not_admitted() {
        let dir = tempfile::tempdir().unwrap();
        let qemu = dir.path().join("qemu");
        let moved = dir.path().join("qemu-original");
        fs::write(&qemu, b"executable-fixture").unwrap();
        fs::set_permissions(&qemu, fs::Permissions::from_mode(0o700)).unwrap();
        let opened = fs::File::open(&qemu).unwrap();
        let admitted = opened.metadata().unwrap();
        assert!(verify_qemu_stable(&qemu, &opened, &admitted).is_ok());
        fs::rename(&qemu, &moved).unwrap();
        fs::write(&qemu, b"executable-fixture").unwrap();
        fs::set_permissions(&qemu, fs::Permissions::from_mode(0o700)).unwrap();
        assert!(verify_qemu_stable(&qemu, &opened, &admitted).is_err());
    }

    #[test]
    fn qemu_executable_in_place_change_is_not_admitted() {
        let dir = tempfile::tempdir().unwrap();
        let qemu = dir.path().join("qemu");
        fs::write(&qemu, b"executable-fixture").unwrap();
        fs::set_permissions(&qemu, fs::Permissions::from_mode(0o700)).unwrap();
        let opened = fs::File::open(&qemu).unwrap();
        let admitted = opened.metadata().unwrap();
        fs::write(&qemu, b"changed-executable-fixture").unwrap();
        assert!(verify_qemu_stable(&qemu, &opened, &admitted).is_err());
    }

    #[test]
    fn qemu_executable_parent_symlink_swap_is_not_admitted() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("bin");
        let relocated = dir.path().join("relocated");
        fs::create_dir(&parent).unwrap();
        let qemu = parent.join("qemu");
        fs::write(&qemu, b"executable-fixture").unwrap();
        fs::set_permissions(&qemu, fs::Permissions::from_mode(0o700)).unwrap();
        let opened = fs::File::open(&qemu).unwrap();
        let admitted = opened.metadata().unwrap();
        fs::rename(&parent, &relocated).unwrap();
        symlink(&relocated, &parent).unwrap();
        assert!(verify_qemu_stable(&qemu, &opened, &admitted).is_err());
    }

    #[test]
    fn overlay_header_open_rejects_fifo_swap_without_waiting_for_writer() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay.qcow2");
        fs::write(&overlay, b"QFI\xfb\0\0\0\x03").unwrap();
        let admitted = fs::symlink_metadata(&overlay).unwrap();
        fs::remove_file(&overlay).unwrap();
        let name = CString::new(overlay.as_os_str().as_bytes()).unwrap();
        assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
        // Opening a substituted FIFO with blocking read-only open would
        // hang waiting for an attacker-controlled writer.
        assert!(open_admitted_overlay_header(&overlay, &admitted).is_err());
    }

    #[test]
    fn overlay_header_path_replacement_is_not_admitted() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay.qcow2");
        let moved = dir.path().join("old.qcow2");
        fs::write(&overlay, b"QFI\xfb\0\0\0\x03").unwrap();
        let opened = fs::File::open(&overlay).unwrap();
        let admitted = opened.metadata().unwrap();
        assert!(verify_overlay_stable(&overlay, &opened, &admitted).is_ok());
        fs::rename(&overlay, &moved).unwrap();
        fs::write(&overlay, b"QFI\xfb\0\0\0\x03").unwrap();
        assert!(verify_overlay_stable(&overlay, &opened, &admitted).is_err());
    }

    #[test]
    fn overlay_header_in_place_mutation_is_not_admitted() {
        let dir = tempfile::tempdir().unwrap();
        let overlay = dir.path().join("overlay.qcow2");
        fs::write(&overlay, b"QFI\xfb\0\0\0\x03").unwrap();
        let opened = fs::File::open(&overlay).unwrap();
        let admitted = opened.metadata().unwrap();
        fs::write(&overlay, b"QFI\xfb\0\0\0\x02-new").unwrap();
        assert!(verify_overlay_stable(&overlay, &opened, &admitted).is_err());
    }

    #[test]
    fn image_path_replacement_is_not_admitted_after_open() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("guest.qcow2");
        let replaced = dir.path().join("original.qcow2");
        fs::write(&image, b"identical-guest-bytes").unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o600)).unwrap();
        let opened = fs::File::open(&image).unwrap();
        let admitted = opened.metadata().unwrap();
        fs::rename(&image, &replaced).unwrap();
        fs::write(&image, b"identical-guest-bytes").unwrap();
        fs::set_permissions(&image, fs::Permissions::from_mode(0o600)).unwrap();
        assert!(verify_guest_image_stable(&image, &opened, &admitted).is_err());
    }

    #[test]
    fn image_parent_symlink_swap_is_not_admitted_after_open() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("images");
        let relocated = dir.path().join("relocated");
        fs::create_dir(&parent).unwrap();
        let image = parent.join("guest.qcow2");
        fs::write(&image, b"identical-guest-bytes").unwrap();
        let opened = fs::File::open(&image).unwrap();
        let admitted = opened.metadata().unwrap();
        fs::rename(&parent, &relocated).unwrap();
        symlink(&relocated, &parent).unwrap();
        // The pathname still resolves to the original inode and bytes,
        // but its admitted real-directory ancestry has been replaced.
        assert!(verify_guest_image_stable(&image, &opened, &admitted).is_err());
    }

    #[test]
    fn image_in_place_change_is_not_admitted_after_open() {
        let dir = tempfile::tempdir().unwrap();
        let image = dir.path().join("guest.qcow2");
        fs::write(&image, b"original-guest").unwrap();
        let opened = fs::File::open(&image).unwrap();
        let admitted = opened.metadata().unwrap();
        fs::write(&image, b"changed-guest-with-new-length").unwrap();
        assert!(verify_guest_image_stable(&image, &opened, &admitted).is_err());
    }
}
