use prime_contracts::{
    ApplicationProfile, ArtifactFormat, ExecutionBackend, MechanicalCompatibilityState,
    RuntimeFamily,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::{self, Read};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

pub const WINDOWS_VM_GUEST_SCHEMA: &str = "prime.windows-vm-guest.v1";
pub const WINDOWS_VM_AGENT_SCHEMA: &str = "prime.windows-vm-agent.v1";
pub const WINDOWS_VM_RUNTIME_PROOF_SCHEMA: &str = "prime.windows-vm-runtime-proof.v1";
pub const WINDOWS_VM_RUNTIME_OBSERVATION_SCHEMA: &str = "prime.windows-vm-runtime-observation.v1";

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
    pub evidence_sha256: String,
    pub observation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VmRuntimeProofState {
    Pending,
    Pass,
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
    if hello.guest_sha256.len() != 64
        || !hello
            .guest_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
        || !hello
            .guest_sha256
            .eq_ignore_ascii_case(expected_guest_sha256)
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
    if !safe_component(application_id) || !safe_component(session_id) {
        return Err(WindowsVmError::InvalidPlan("invalid session identity"));
    }
    let runtime_dir = root.join(application_id).join(session_id);
    Ok(VmSessionPaths {
        overlay_image: runtime_dir.join("overlay.qcow2"),
        agent_socket: runtime_dir.join("agent.sock"),
        runtime_dir,
    })
}

pub fn prepare_runtime_proof_plan(
    guest: &ValidatedVmGuest,
    root: &Path,
    application_id: &str,
    session_id: &str,
) -> Result<VmRuntimeProofPlan, WindowsVmError> {
    let paths = vm_session_paths(root, application_id, session_id)?;
    let required_cases = vec![
        VmRuntimeAdversarialCase::ConcurrentSessionIsolation,
        VmRuntimeAdversarialCase::ForcedProcessTermination,
        VmRuntimeAdversarialCase::CorruptGuestAuthority,
        VmRuntimeAdversarialCase::DeniedUsbNode,
    ];
    let proof_id = runtime_proof_id(
        &guest.guest_id,
        guest.revision,
        &guest.base_sha256,
        application_id,
        session_id,
        &required_cases,
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
    if evidence.is_empty() {
        return Err(WindowsVmError::InvalidPlan(
            "runtime proof evidence is empty",
        ));
    }
    let evidence_sha256 = format!("{:x}", Sha256::digest(evidence));
    create_runtime_proof_observation(plan, case, passed, zero_residual_state, &evidence_sha256)
}

pub fn create_runtime_proof_observation(
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
    );
    if plan.proof_id != expected_proof_id || observations.len() != plan.required_cases.len() {
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
) -> String {
    let body = serde_json::json!({
        "schema": WINDOWS_VM_RUNTIME_PROOF_SCHEMA,
        "guest_id": guest_id,
        "guest_revision": guest_revision,
        "guest_sha256": guest_sha256,
        "application_id": application_id,
        "session_id": session_id,
        "required_cases": required_cases,
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
        MechanicalCompatibilityState::Unknown | MechanicalCompatibilityState::Broken
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
    if definition.guest_id.trim().is_empty() || definition.revision == 0 {
        return Err(WindowsVmError::InvalidGuest("missing guest identity"));
    }
    if !(512..=32768).contains(&definition.memory_mib) || !(1..=16).contains(&definition.vcpus) {
        return Err(WindowsVmError::InvalidGuest("resource bounds exceeded"));
    }
    if definition.base_sha256.len() != 64
        || !definition
            .base_sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(WindowsVmError::InvalidGuest("invalid sha256"));
    }
    let image = PathBuf::from(&definition.base_image);
    if !image.is_absolute() {
        return Err(WindowsVmError::InvalidGuest("base image is not absolute"));
    }
    let metadata = fs::symlink_metadata(&image)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(WindowsVmError::InvalidGuest(
            "base image is not a regular non-symlink file",
        ));
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
    if !actual.eq_ignore_ascii_case(&definition.base_sha256) {
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
    if !request.overlay_image.is_absolute() || !request.runtime_dir.is_absolute() {
        return Err(WindowsVmError::InvalidPlan(
            "runtime paths are not absolute",
        ));
    }
    let overlay_meta = fs::symlink_metadata(&request.overlay_image)?;
    if overlay_meta.file_type().is_symlink() || !overlay_meta.file_type().is_file() {
        return Err(WindowsVmError::InvalidPlan("overlay is not a regular file"));
    }
    let runtime_meta = fs::symlink_metadata(&request.runtime_dir)?;
    if runtime_meta.file_type().is_symlink() || !runtime_meta.file_type().is_dir() {
        return Err(WindowsVmError::InvalidPlan(
            "runtime directory is not a regular directory",
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
        program: request.qemu_binary.clone(),
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
