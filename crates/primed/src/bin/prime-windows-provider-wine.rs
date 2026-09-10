use prime_contracts::{ArtifactFormat, RuntimeFamily};
use primed::exec;
use serde_json::Value;
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use thiserror::Error;
use uuid::Uuid;

const DONOR_BINARY: &str = "/usr/bin/wine";
const COMPOSITOR_READINESS: &str = "/run/prime-compositor/readiness.json";
const COMPOSITOR_RUNTIME: &str = "/run/prime-compositor";
const COMPOSITOR_READINESS_SCHEMA: &str = "prime.compositor-readiness.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
struct ProviderRequest {
    artifact: PathBuf,
    application_id: Uuid,
    launch_id: Uuid,
    runtime_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DonorCommand {
    program: PathBuf,
    args: Vec<String>,
    env: BTreeMap<String, String>,
}

#[derive(Debug, Error)]
enum AdapterError {
    #[error("invalid Prime Windows provider ABI: {0}")]
    InvalidArgs(&'static str),
    #[error("invalid application UUID")]
    InvalidApplicationId,
    #[error("invalid launch UUID")]
    InvalidLaunchId,
    #[error("artifact path is invalid: {0}")]
    ArtifactPath(&'static str),
    #[error("artifact is not an admitted PE32/PE32+ x86/x86_64 Windows workload")]
    ArtifactNotPe,
    #[error("runtime directory is not bound to this launch")]
    RuntimeBinding,
    #[error("runtime directory is invalid: {0}")]
    RuntimePath(&'static str),
    #[error("packaged Windows compatibility donor is unavailable: {0}")]
    DonorUnavailable(String),
    #[error(transparent)]
    Exec(#[from] exec::ExecError),
    #[error(transparent)]
    Io(#[from] io::Error),
}

fn main() {
    if let Err(error) = run() {
        eprintln!("prime-windows-provider-w1: {error}");
        process::exit(64);
    }
}

fn run() -> Result<(), AdapterError> {
    let request = parse_args(env::args().skip(1))?;
    validate_request(&request)?;
    prepare_runtime_state(&request)?;
    validate_donor()?;
    let wayland_socket = fs::read(COMPOSITOR_READINESS)
        .ok()
        .and_then(|raw| parse_wayland_socket(&raw));
    let spec = donor_command(&request, wayland_socket.as_deref());

    let mut command = Command::new(&spec.program);
    command.args(&spec.args).env_clear();
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    let error = command.exec();
    Err(AdapterError::Io(error))
}

fn parse_args<I>(args: I) -> Result<ProviderRequest, AdapterError>
where
    I: IntoIterator<Item = String>,
{
    let mut artifact = None;
    let mut application_id = None;
    let mut launch_id = None;
    let mut runtime_dir = None;
    let mut iter = args.into_iter();

    while let Some(flag) = iter.next() {
        let value = iter
            .next()
            .ok_or(AdapterError::InvalidArgs("flag is missing its value"))?;
        match flag.as_str() {
            "--artifact" if artifact.is_none() => artifact = Some(PathBuf::from(value)),
            "--application-id" if application_id.is_none() => {
                application_id =
                    Some(Uuid::parse_str(&value).map_err(|_| AdapterError::InvalidApplicationId)?)
            }
            "--launch-id" if launch_id.is_none() => {
                launch_id =
                    Some(Uuid::parse_str(&value).map_err(|_| AdapterError::InvalidLaunchId)?)
            }
            "--runtime-dir" if runtime_dir.is_none() => runtime_dir = Some(PathBuf::from(value)),
            "--artifact" | "--application-id" | "--launch-id" | "--runtime-dir" => {
                return Err(AdapterError::InvalidArgs("duplicate ABI flag"));
            }
            _ => return Err(AdapterError::InvalidArgs("unknown ABI flag")),
        }
    }

    Ok(ProviderRequest {
        artifact: artifact.ok_or(AdapterError::InvalidArgs("--artifact is required"))?,
        application_id: application_id
            .ok_or(AdapterError::InvalidArgs("--application-id is required"))?,
        launch_id: launch_id.ok_or(AdapterError::InvalidArgs("--launch-id is required"))?,
        runtime_dir: runtime_dir.ok_or(AdapterError::InvalidArgs("--runtime-dir is required"))?,
    })
}

fn validate_request(request: &ProviderRequest) -> Result<(), AdapterError> {
    if !request.artifact.is_absolute() {
        return Err(AdapterError::ArtifactPath("path is not absolute"));
    }
    let metadata = fs::symlink_metadata(&request.artifact)
        .map_err(|_| AdapterError::ArtifactPath("path cannot be inspected"))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(AdapterError::ArtifactPath(
            "path is not a regular non-symlink file",
        ));
    }

    let inspection = exec::inspect(&request.artifact, "x86_64")?;
    if inspection.runtime_family != RuntimeFamily::Windows
        || !matches!(
            inspection.format,
            ArtifactFormat::Pe32 | ArtifactFormat::Pe32Plus
        )
        || !matches!(inspection.workload_arch.as_deref(), Some("x86" | "x86_64"))
    {
        return Err(AdapterError::ArtifactNotPe);
    }
    validate_runtime_binding(request)?;
    validate_runtime_directory(&request.runtime_dir)?;
    Ok(())
}

fn validate_runtime_binding(request: &ProviderRequest) -> Result<(), AdapterError> {
    if !request.runtime_dir.is_absolute() {
        return Err(AdapterError::RuntimeBinding);
    }
    let expected = PathBuf::from(format!(
        "/run/prime-win-{}",
        request.launch_id.to_string().replace('-', "")
    ));
    if request.runtime_dir != expected {
        return Err(AdapterError::RuntimeBinding);
    }
    Ok(())
}

fn validate_runtime_directory(path: &Path) -> Result<(), AdapterError> {
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| AdapterError::RuntimePath("path cannot be inspected"))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(AdapterError::RuntimePath(
            "path is not a regular non-symlink directory",
        ));
    }
    Ok(())
}

fn prepare_runtime_state(request: &ProviderRequest) -> Result<(), AdapterError> {
    for directory in [
        request.runtime_dir.join("wine-prefix"),
        request.runtime_dir.join("home"),
        request.runtime_dir.join("cache"),
        request.runtime_dir.join("config"),
    ] {
        fs::create_dir(&directory)?;
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

fn validate_donor() -> Result<(), AdapterError> {
    let resolved = fs::canonicalize(DONOR_BINARY).map_err(|error| {
        AdapterError::DonorUnavailable(format!("{DONOR_BINARY} cannot be resolved: {error}"))
    })?;
    let metadata = fs::metadata(&resolved).map_err(|error| {
        AdapterError::DonorUnavailable(format!(
            "{} cannot be inspected: {error}",
            resolved.display()
        ))
    })?;
    if !metadata.is_file() || metadata.permissions().mode() & 0o111 == 0 {
        return Err(AdapterError::DonorUnavailable(format!(
            "{} is not an executable regular file",
            resolved.display()
        )));
    }
    Ok(())
}

fn donor_command(request: &ProviderRequest, wayland_socket: Option<&str>) -> DonorCommand {
    let mut environment = BTreeMap::from([
        (
            "HOME".to_owned(),
            request.runtime_dir.join("home").display().to_string(),
        ),
        ("LANG".to_owned(), "C.UTF-8".to_owned()),
        ("PATH".to_owned(), "/usr/bin:/usr/sbin".to_owned()),
        ("WINEDEBUG".to_owned(), "-all".to_owned()),
        (
            "WINEPREFIX".to_owned(),
            request
                .runtime_dir
                .join("wine-prefix")
                .display()
                .to_string(),
        ),
        (
            "XDG_CACHE_HOME".to_owned(),
            request.runtime_dir.join("cache").display().to_string(),
        ),
        (
            "XDG_CONFIG_HOME".to_owned(),
            request.runtime_dir.join("config").display().to_string(),
        ),
    ]);
    if let Some(socket) = wayland_socket {
        environment.insert("XDG_RUNTIME_DIR".to_owned(), COMPOSITOR_RUNTIME.to_owned());
        environment.insert("WAYLAND_DISPLAY".to_owned(), socket.to_owned());
    }
    DonorCommand {
        program: PathBuf::from(DONOR_BINARY),
        args: vec![request.artifact.display().to_string()],
        env: environment,
    }
}

fn parse_wayland_socket(raw: &[u8]) -> Option<String> {
    let value: Value = serde_json::from_slice(raw).ok()?;
    if value.get("schema")?.as_str()? != COMPOSITOR_READINESS_SCHEMA {
        return None;
    }
    let socket = value.get("wayland_socket")?.as_str()?;
    if socket.is_empty()
        || socket.len() > 108
        || socket.contains('/')
        || socket.contains('\\')
        || socket.contains('\0')
        || !socket.starts_with("wayland-")
        || !socket
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        return None;
    }
    Some(socket.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::os::unix::fs::symlink;
    use std::path::PathBuf;
    use tempfile::tempdir;
    use uuid::Uuid;

    fn ids() -> (String, String) {
        (Uuid::now_v7().to_string(), Uuid::now_v7().to_string())
    }

    #[test]
    fn parse_requires_exact_prime_provider_abi() {
        let (app, launch) = ids();
        let args = vec![
            "--artifact".to_owned(),
            "/tmp/a.exe".to_owned(),
            "--application-id".to_owned(),
            app.clone(),
            "--launch-id".to_owned(),
            launch.clone(),
            "--runtime-dir".to_owned(),
            format!("/run/prime-win-{}", launch.replace('-', "")),
        ];
        let parsed = parse_args(args).expect("valid ABI");
        assert_eq!(parsed.application_id.to_string(), app);
        assert_eq!(parsed.launch_id.to_string(), launch);
    }

    #[test]
    fn parse_rejects_unknown_or_duplicate_flags() {
        let (app, launch) = ids();
        let duplicate = vec![
            "--artifact".to_owned(),
            "/tmp/a.exe".to_owned(),
            "--artifact".to_owned(),
            "/tmp/b.exe".to_owned(),
            "--application-id".to_owned(),
            app.clone(),
            "--launch-id".to_owned(),
            launch.clone(),
            "--runtime-dir".to_owned(),
            format!("/run/prime-win-{}", launch.replace('-', "")),
        ];
        assert!(parse_args(duplicate).is_err());

        let unknown = vec!["--backend".to_owned(), "wine".to_owned()];
        assert!(parse_args(unknown).is_err());
    }

    #[test]
    fn validate_rejects_relative_and_non_pe_artifact_paths() {
        let dir = tempdir().expect("tempdir");
        let runtime = dir.path().join("runtime");
        fs::create_dir(&runtime).expect("runtime");
        let artifact = dir.path().join("fixture.exe");
        fs::write(&artifact, b"not a pe").expect("artifact");
        let launch_id = Uuid::now_v7();
        let request = ProviderRequest {
            artifact: artifact.clone(),
            application_id: Uuid::now_v7(),
            launch_id,
            runtime_dir: runtime,
        };
        assert!(matches!(
            validate_request(&request),
            Err(AdapterError::ArtifactNotPe)
        ));

        let relative = ProviderRequest {
            artifact: PathBuf::from("fixture.exe"),
            ..request
        };
        assert!(matches!(
            validate_request(&relative),
            Err(AdapterError::ArtifactPath(_))
        ));
    }

    #[test]
    fn validate_rejects_symlink_artifact() {
        let dir = tempdir().expect("tempdir");
        let target = dir.path().join("real.exe");
        fs::write(&target, minimal_pe64()).expect("target");
        let link = dir.path().join("link.exe");
        symlink(&target, &link).expect("symlink");
        let launch_id = Uuid::now_v7();
        let runtime_dir = PathBuf::from(format!(
            "/run/prime-win-{}",
            launch_id.to_string().replace('-', "")
        ));
        let request = ProviderRequest {
            artifact: link,
            application_id: Uuid::now_v7(),
            launch_id,
            runtime_dir,
        };
        assert!(matches!(
            validate_request(&request),
            Err(AdapterError::ArtifactPath(_))
        ));
    }

    #[test]
    fn validate_requires_runtime_dir_bound_to_launch_id() {
        let dir = tempdir().expect("tempdir");
        let artifact = dir.path().join("fixture.exe");
        fs::write(&artifact, minimal_pe64()).expect("artifact");
        let launch_id = Uuid::now_v7();
        let wrong = ProviderRequest {
            artifact,
            application_id: Uuid::now_v7(),
            launch_id,
            runtime_dir: PathBuf::from("/run/prime-win-wrong"),
        };
        assert!(matches!(
            validate_runtime_binding(&wrong),
            Err(AdapterError::RuntimeBinding)
        ));
    }

    #[test]
    fn donor_command_is_fixed_direct_argv_with_isolated_state() {
        let runtime = PathBuf::from("/run/prime-win-001122");
        let request = ProviderRequest {
            artifact: PathBuf::from("/var/lib/prime/artifacts/sha256/deadbeef"),
            application_id: Uuid::nil(),
            launch_id: Uuid::nil(),
            runtime_dir: runtime.clone(),
        };
        let command = donor_command(&request, None);
        assert_eq!(command.program, PathBuf::from("/usr/bin/wine"));
        assert_eq!(command.args, vec![request.artifact.display().to_string()]);
        assert_eq!(
            command.env.get("WINEPREFIX"),
            Some(&runtime.join("wine-prefix").display().to_string())
        );
        assert_eq!(
            command.env.get("HOME"),
            Some(&runtime.join("home").display().to_string())
        );
        assert!(!command.env.contains_key("DISPLAY"));
        assert!(!command
            .args
            .iter()
            .any(|arg| matches!(arg.as_str(), "sh" | "bash" | "-c")));
    }

    #[test]
    fn compositor_readiness_accepts_only_safe_wayland_socket_name() {
        assert_eq!(
            parse_wayland_socket(
                br#"{"schema":"prime.compositor-readiness.v1","wayland_socket":"wayland-7"}"#
            )
            .as_deref(),
            Some("wayland-7")
        );
        assert!(parse_wayland_socket(
            br#"{"schema":"prime.compositor-readiness.v1","wayland_socket":"../core.sock"}"#
        )
        .is_none());
        assert!(
            parse_wayland_socket(br#"{"schema":"wrong","wayland_socket":"wayland-0"}"#).is_none()
        );
    }

    fn minimal_pe64() -> Vec<u8> {
        let mut bytes = vec![0_u8; 128];
        bytes[0..2].copy_from_slice(b"MZ");
        bytes[0x3c..0x40].copy_from_slice(&0x40_u32.to_le_bytes());
        bytes[0x40..0x44].copy_from_slice(b"PE\0\0");
        bytes[0x44..0x46].copy_from_slice(&0x8664_u16.to_le_bytes());
        bytes[0x58..0x5a].copy_from_slice(&0x20b_u16.to_le_bytes());
        bytes
    }
}
