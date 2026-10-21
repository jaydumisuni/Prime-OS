use prime_contracts::{
    ApplicationProfile, ArtifactFormat, ExecutionBackend, MechanicalCompatibilityState,
    RuntimeFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Component, Path, PathBuf};
use thiserror::Error;

pub const WINDOWS_VM_GUEST_SCHEMA: &str = "prime.windows-vm-guest.v1";
pub const WINDOWS_VM_AGENT_SCHEMA: &str = "prime.windows-vm-agent.v1";
pub const WINDOWS_VM_RUNTIME_PROOF_SCHEMA: &str = "prime.windows-vm-runtime-proof.v1";
pub const WINDOWS_VM_RUNTIME_OBSERVATION_SCHEMA: &str = "prime.windows-vm-runtime-observation.v1";
pub const WINDOWS_VM_RUNTIME_EVIDENCE_SCHEMA: &str = "prime.windows-vm-runtime-evidence.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

pub fn validate_guest_agent_hello(
    hello: &GuestAgentHello,
    expected_guest_id: &str,
    expected_guest_revision: u64,
    expected_guest_sha256: &str,
    expected_workload_arch: &str,
) -> Result<(), WindowsVmError> {
    // Expected identity is caller-supplied; matching an invalid expectation is not authority.
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
    if !canonical_sha256_hex(&hello.guest_sha256)
        || !canonical_sha256_hex(expected_guest_sha256)
        || hello.guest_sha256 != expected_guest_sha256
    {
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
    if root.to_str().is_none_or(|value| {
        value
            .bytes()
            .any(|byte| byte == b',' || byte.is_ascii_control())
    }) {
        return Err(WindowsVmError::InvalidPlan(
            "session root contains QEMU option delimiters",
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
        if !parent.file_type().is_dir()
            || (mode & 0o022 != 0 && !(mode & 0o1000 != 0 && parent.uid() == 0))
        {
            return Err(WindowsVmError::InvalidPlan(
                "session root ancestor is shared-writable or invalid",
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
    if !image.is_absolute() {
        return Err(WindowsVmError::InvalidGuest("base image is not absolute"));
    }
    // The guest definition is a later VM-preparation input. Do not admit
    // QEMU option separators or control characters into its path authority.
    if definition
        .base_image
        .bytes()
        .any(|byte| byte == b',' || byte.is_ascii_control())
    {
        return Err(WindowsVmError::InvalidGuest(
            "base image path contains QEMU option delimiters",
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
        let mode = parent.permissions().mode();
        if mode & 0o022 != 0 && !(mode & 0o1000 != 0 && parent.uid() == 0) {
            return Err(WindowsVmError::InvalidGuest(
                "base image directory is shared-writable",
            ));
        }
    }
    let metadata = fs::symlink_metadata(&image)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(WindowsVmError::InvalidGuest(
            "base image is not a regular non-symlink file",
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
    let mut file = fs::File::open(&image)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
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

pub fn build_qemu_plan(request: &VmPlanRequest) -> Result<QemuPlan, WindowsVmError> {
    if !request.qemu_binary.is_absolute() {
        return Err(WindowsVmError::InvalidPlan("QEMU path is not absolute"));
    }
    let qemu = fs::canonicalize(&request.qemu_binary)?;
    let qemu_meta = fs::metadata(&qemu)?;
    if !qemu_meta.is_file() || qemu_meta.permissions().mode() & 0o111 == 0 {
        return Err(WindowsVmError::InvalidPlan("QEMU is not executable"));
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
    for directory in qemu.ancestors().skip(1) {
        let parent = fs::symlink_metadata(directory)?;
        let mode = parent.permissions().mode();
        if !parent.file_type().is_dir()
            || (mode & 0o022 != 0 && !(mode & 0o1000 != 0 && parent.uid() == 0))
        {
            return Err(WindowsVmError::InvalidPlan(
                "QEMU executable directory is shared-writable or invalid",
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
        .any(|path| {
            path.to_str().is_none_or(|value| {
                value
                    .bytes()
                    .any(|byte| byte == b',' || byte.is_ascii_control())
            })
        })
    {
        return Err(WindowsVmError::InvalidPlan(
            "runtime path contains QEMU option delimiters",
        ));
    }
    let overlay_meta = fs::symlink_metadata(&request.overlay_image)?;
    if overlay_meta.file_type().is_symlink() || !overlay_meta.file_type().is_file() {
        return Err(WindowsVmError::InvalidPlan("overlay is not a regular file"));
    }
    // A zero-byte overlay cannot contain a usable qcow2 header.
    if overlay_meta.len() == 0 {
        return Err(WindowsVmError::InvalidPlan("overlay image is empty"));
    }
    // A hardlinked overlay may alias unrelated host data even when its
    // canonical path is confined to the selected VM runtime directory.
    if overlay_meta.nlink() != 1 {
        return Err(WindowsVmError::InvalidPlan("overlay has a hardlink alias"));
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
        if !parent.file_type().is_dir()
            || (mode & 0o022 != 0 && !(mode & 0o1000 != 0 && parent.uid() == 0))
        {
            return Err(WindowsVmError::InvalidPlan(
                "runtime directory ancestor is shared-writable or invalid",
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
    if !(512..=32768).contains(&request.memory_mib) || !(1..=16).contains(&request.vcpus) {
        return Err(WindowsVmError::InvalidPlan("resource bounds exceeded"));
    }

    let mut usb = Vec::new();
    for node in &request.usb_nodes {
        usb.push(parse_usb_node(node)?);
    }
    usb.sort_unstable();
    usb.dedup();

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
    if bus == 0 || device == 0 {
        return Err(WindowsVmError::InvalidUsb(raw.to_owned()));
    }
    Ok((bus, device))
}
