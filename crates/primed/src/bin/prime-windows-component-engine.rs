use prime_contracts::{WindowsComponentManifest, WindowsInstallerKind};
use primed::{windows_component_engine, windows_components, windows_state, windows_wine};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::env;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ComponentRequest {
    manifest: PathBuf,
    application_id: Uuid,
    transaction_id: Uuid,
    runtime_dir: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CommandSpec {
    program: PathBuf,
    args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeCommand {
    program: PathBuf,
    args: Vec<String>,
    env: BTreeMap<String, String>,
    capture_stdout: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct RuntimeOutput {
    exit_code: Option<i32>,
    stdout: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TransactionResult {
    Installed,
    AlreadySatisfied,
}

#[derive(Debug, Error)]
enum ComponentEngineError {
    #[error("invalid Prime Windows component-engine ABI: {0}")]
    InvalidArgs(&'static str),
    #[error("invalid application UUID")]
    InvalidApplicationId,
    #[error("invalid transaction UUID")]
    InvalidTransactionId,
    #[error("component runtime directory is not bound to transaction")]
    RuntimeBinding,
    #[error("component manifest path is invalid: {0}")]
    ManifestPath(&'static str),
    #[error("component installer recipe is invalid: {0}")]
    Recipe(&'static str),
    #[error("WINDOWS_COMPONENT_ARTIFACT_PATH_INVALID: {0}")]
    ArtifactPath(&'static str),
    #[error("WINDOWS_COMPONENT_ARTIFACT_MISMATCH")]
    ArtifactIdentityMismatch,
    #[error("WINDOWS_COMPONENT_REGISTRY_QUERY_INVALID: {0}")]
    RegistryQuery(&'static str),
    #[error("WINDOWS_INSTALLER_FAILED: exit code {0:?}")]
    InstallerFailed(Option<i32>),
    #[error("WINDOWS_COMPONENT_VERIFY_FAILED")]
    VerificationFailed,
    #[error("WINDOWS_DONOR_WAIT_FAILED: exit code {0:?}")]
    DonorWaitFailed(Option<i32>),
    #[error(transparent)]
    ComponentState(#[from] windows_component_engine::WindowsComponentEngineError),
    #[error(transparent)]
    WindowsState(#[from] windows_state::WindowsStateError),
    #[error(transparent)]
    Registry(#[from] windows_components::WindowsComponentRegistryError),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] io::Error),
}

fn main() {
    if let Err(error) = run() {
        eprintln!("prime-windows-component-engine: {error}");
        process::exit(64);
    }
}

fn run() -> Result<(), ComponentEngineError> {
    let request = parse_args(env::args().skip(1))?;
    validate_runtime_binding(&request)?;
    validate_manifest_path(&request.manifest)?;
    let manifest: WindowsComponentManifest = serde_json::from_slice(&fs::read(&request.manifest)?)?;
    windows_components::verify_component(&manifest)?;
    prepare_ephemeral_runtime(&request.runtime_dir)?;

    let state_root = windows_state::application_state_root(request.application_id);
    let prefix = windows_state::application_prefix(request.application_id);
    windows_state::ensure_private_directory(&state_root)?;
    windows_state::ensure_private_directory(&prefix)?;
    let _compatibility_lock = windows_state::acquire_compatibility_lock(&state_root)?;

    ensure_prefix_initialized(
        request.application_id,
        &request.runtime_dir,
        &prefix,
        &mut run_runtime_command,
    )?;

    let installed_at = primed::identity::now_rfc3339().map_err(|_| {
        ComponentEngineError::Recipe("system clock could not produce RFC3339 timestamp")
    })?;
    let _result = execute_component_transaction_at(
        &state_root,
        &prefix,
        request.application_id,
        &request.runtime_dir,
        &manifest,
        &installed_at,
        &mut run_runtime_command,
    )?;
    Ok(())
}

fn parse_args<I>(args: I) -> Result<ComponentRequest, ComponentEngineError>
where
    I: IntoIterator<Item = String>,
{
    let mut manifest = None;
    let mut application_id = None;
    let mut transaction_id = None;
    let mut runtime_dir = None;
    let mut iter = args.into_iter();
    while let Some(flag) = iter.next() {
        let value = iter.next().ok_or(ComponentEngineError::InvalidArgs(
            "flag is missing its value",
        ))?;
        match flag.as_str() {
            "--manifest" if manifest.is_none() => manifest = Some(PathBuf::from(value)),
            "--application-id" if application_id.is_none() => {
                application_id = Some(
                    Uuid::parse_str(&value)
                        .map_err(|_| ComponentEngineError::InvalidApplicationId)?,
                )
            }
            "--transaction-id" if transaction_id.is_none() => {
                transaction_id = Some(
                    Uuid::parse_str(&value)
                        .map_err(|_| ComponentEngineError::InvalidTransactionId)?,
                )
            }
            "--runtime-dir" if runtime_dir.is_none() => runtime_dir = Some(PathBuf::from(value)),
            "--manifest" | "--application-id" | "--transaction-id" | "--runtime-dir" => {
                return Err(ComponentEngineError::InvalidArgs("duplicate ABI flag"));
            }
            _ => return Err(ComponentEngineError::InvalidArgs("unknown ABI flag")),
        }
    }
    Ok(ComponentRequest {
        manifest: manifest.ok_or(ComponentEngineError::InvalidArgs("--manifest is required"))?,
        application_id: application_id.ok_or(ComponentEngineError::InvalidArgs(
            "--application-id is required",
        ))?,
        transaction_id: transaction_id.ok_or(ComponentEngineError::InvalidArgs(
            "--transaction-id is required",
        ))?,
        runtime_dir: runtime_dir.ok_or(ComponentEngineError::InvalidArgs(
            "--runtime-dir is required",
        ))?,
    })
}

fn validate_runtime_binding(request: &ComponentRequest) -> Result<(), ComponentEngineError> {
    let expected = PathBuf::from(format!(
        "/run/prime-win-component-{}",
        request.transaction_id.to_string().replace('-', "")
    ));
    if request.runtime_dir != expected {
        return Err(ComponentEngineError::RuntimeBinding);
    }
    Ok(())
}

fn validate_manifest_path(path: &Path) -> Result<(), ComponentEngineError> {
    if !path.is_absolute() {
        return Err(ComponentEngineError::ManifestPath("path is not absolute"));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ComponentEngineError::ManifestPath("path cannot be inspected"))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(ComponentEngineError::ManifestPath(
            "path is not a regular non-symlink file",
        ));
    }
    Ok(())
}

fn installer_command(
    manifest: &WindowsComponentManifest,
) -> Result<Option<CommandSpec>, ComponentEngineError> {
    match manifest.installer_kind {
        WindowsInstallerKind::Builtin => Ok(None),
        WindowsInstallerKind::Msi => {
            let artifact = manifest
                .artifact_path
                .as_deref()
                .ok_or(ComponentEngineError::Recipe("MSI artifact path is missing"))?;
            let mut args = vec!["msiexec".to_owned(), "/i".to_owned(), artifact.to_owned()];
            args.extend(manifest.installer_args.iter().cloned());
            Ok(Some(CommandSpec {
                program: PathBuf::from(windows_wine::WINDOWS_WINE_BINARY),
                args,
            }))
        }
        WindowsInstallerKind::Exe => {
            let artifact = manifest
                .artifact_path
                .as_deref()
                .ok_or(ComponentEngineError::Recipe("EXE artifact path is missing"))?;
            let mut args = vec![artifact.to_owned()];
            args.extend(manifest.installer_args.iter().cloned());
            Ok(Some(CommandSpec {
                program: PathBuf::from(windows_wine::WINDOWS_WINE_BINARY),
                args,
            }))
        }
    }
}

fn prepare_ephemeral_runtime(runtime_dir: &Path) -> Result<(), ComponentEngineError> {
    let metadata =
        fs::symlink_metadata(runtime_dir).map_err(|_| ComponentEngineError::RuntimeBinding)?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_dir() {
        return Err(ComponentEngineError::RuntimeBinding);
    }
    for name in ["home", "cache", "config"] {
        windows_state::ensure_private_directory(&runtime_dir.join(name))?;
    }
    Ok(())
}

fn ensure_prefix_initialized<F>(
    application_id: Uuid,
    runtime_dir: &Path,
    prefix: &Path,
    runner: &mut F,
) -> Result<(), ComponentEngineError>
where
    F: FnMut(&RuntimeCommand) -> Result<RuntimeOutput, ComponentEngineError>,
{
    if windows_wine::prefix_marker_matches(prefix)? {
        return Ok(());
    }
    for (stage, spec) in ["wineboot", "wineserver-wait"].into_iter().zip(
        windows_wine::prefix_initialization_commands(application_id, runtime_dir, None),
    ) {
        let runtime = RuntimeCommand {
            program: spec.program,
            args: spec.args,
            env: spec.env,
            capture_stdout: false,
        };
        let output = runner(&runtime)?;
        if output.exit_code != Some(0) {
            return Err(ComponentEngineError::Recipe(match stage {
                "wineboot" => "Wine prefix initialization failed",
                _ => "Wine server did not quiesce after initialization",
            }));
        }
    }
    windows_wine::write_prefix_marker(prefix)?;
    Ok(())
}

fn execute_component_transaction_at<F>(
    state_root: &Path,
    prefix: &Path,
    application_id: Uuid,
    runtime_dir: &Path,
    manifest: &WindowsComponentManifest,
    installed_at: &str,
    runner: &mut F,
) -> Result<TransactionResult, ComponentEngineError>
where
    F: FnMut(&RuntimeCommand) -> Result<RuntimeOutput, ComponentEngineError>,
{
    let environment = windows_wine::donor_environment(application_id, runtime_dir, None);
    let initial = verify_with_registry(prefix, manifest, &environment, runner)?;
    if windows_component_engine::all_probes_pass(&initial) {
        if !windows_component_engine::component_marker_matches(
            state_root,
            manifest,
            windows_wine::WINDOWS_WINE_DONOR_FINGERPRINT,
        )? {
            windows_component_engine::write_component_marker(
                state_root,
                manifest,
                windows_wine::WINDOWS_WINE_DONOR_FINGERPRINT,
                installed_at,
                &initial,
            )?;
        }
        return Ok(TransactionResult::AlreadySatisfied);
    }

    let Some(installer) = installer_command(manifest)? else {
        return Err(ComponentEngineError::VerificationFailed);
    };
    validate_installer_artifact(manifest)?;
    let installer_runtime = RuntimeCommand {
        program: installer.program,
        args: installer.args,
        env: environment.clone(),
        capture_stdout: false,
    };
    let installed = runner(&installer_runtime)?;
    if !installer_exit_accepted(manifest, installed.exit_code) {
        return Err(ComponentEngineError::InstallerFailed(installed.exit_code));
    }

    let wait = RuntimeCommand {
        program: PathBuf::from(windows_wine::WINDOWS_WINESERVER_BINARY),
        args: vec!["-w".to_owned()],
        env: environment.clone(),
        capture_stdout: false,
    };
    let waited = runner(&wait)?;
    if waited.exit_code != Some(0) {
        return Err(ComponentEngineError::DonorWaitFailed(waited.exit_code));
    }

    let verified = verify_with_registry(prefix, manifest, &environment, runner)?;
    if !windows_component_engine::all_probes_pass(&verified) {
        return Err(ComponentEngineError::VerificationFailed);
    }
    windows_component_engine::write_component_marker(
        state_root,
        manifest,
        windows_wine::WINDOWS_WINE_DONOR_FINGERPRINT,
        installed_at,
        &verified,
    )?;
    Ok(TransactionResult::Installed)
}

fn verify_with_registry<F>(
    prefix: &Path,
    manifest: &WindowsComponentManifest,
    environment: &BTreeMap<String, String>,
    runner: &mut F,
) -> Result<Vec<windows_component_engine::WindowsProbeResult>, ComponentEngineError>
where
    F: FnMut(&RuntimeCommand) -> Result<RuntimeOutput, ComponentEngineError>,
{
    let mut query =
        |hive: &str,
         key: &str,
         name: &str|
         -> Result<Option<String>, windows_component_engine::WindowsComponentEngineError> {
            let spec = registry_query_command(hive, key, name).map_err(|error| {
                windows_component_engine::WindowsComponentEngineError::InvalidProbe(match error {
                    ComponentEngineError::RegistryQuery(message) => message,
                    _ => "registry query construction failed",
                })
            })?;
            let command = RuntimeCommand {
                program: spec.program,
                args: spec.args,
                env: environment.clone(),
                capture_stdout: true,
            };
            let output = runner(&command).map_err(|_| {
                windows_component_engine::WindowsComponentEngineError::ProbeUnsupported(
                    "registry query execution failed",
                )
            })?;
            if output.exit_code != Some(0) {
                return Ok(None);
            }
            Ok(parse_registry_value(&output.stdout, name))
        };
    Ok(
        windows_component_engine::verify_component_state_with_registry(
            prefix, manifest, &mut query,
        )?,
    )
}

fn run_runtime_command(spec: &RuntimeCommand) -> Result<RuntimeOutput, ComponentEngineError> {
    let mut command = Command::new(&spec.program);
    command.args(&spec.args).env_clear();
    for (name, value) in &spec.env {
        command.env(name, value);
    }
    if spec.capture_stdout {
        command.stderr(Stdio::null());
        let output = command.output()?;
        return Ok(RuntimeOutput {
            exit_code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        });
    }
    let status = command.status()?;
    Ok(RuntimeOutput {
        exit_code: status.code(),
        stdout: String::new(),
    })
}

fn validate_installer_artifact(
    manifest: &WindowsComponentManifest,
) -> Result<(), ComponentEngineError> {
    if matches!(manifest.installer_kind, WindowsInstallerKind::Builtin) {
        return Ok(());
    }
    let path = Path::new(
        manifest
            .artifact_path
            .as_deref()
            .ok_or(ComponentEngineError::Recipe(
                "installer artifact path is missing",
            ))?,
    );
    if !path.is_absolute() {
        return Err(ComponentEngineError::ArtifactPath("path is not absolute"));
    }
    let metadata = fs::symlink_metadata(path)
        .map_err(|_| ComponentEngineError::ArtifactPath("path cannot be inspected"))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        return Err(ComponentEngineError::ArtifactPath(
            "path is not a regular non-symlink file",
        ));
    }
    let expected = manifest
        .artifact_identity
        .as_deref()
        .ok_or(ComponentEngineError::Recipe(
            "installer artifact identity is missing",
        ))?;
    if sha256_file(path)? != expected {
        return Err(ComponentEngineError::ArtifactIdentityMismatch);
    }
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, ComponentEngineError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(7 + digest.len() * 2);
    encoded.push_str("sha256:");
    for byte in digest {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    Ok(encoded)
}

fn registry_query_command(
    hive: &str,
    key: &str,
    name: &str,
) -> Result<CommandSpec, ComponentEngineError> {
    if !matches!(hive, "HKLM" | "HKCU" | "HKCR" | "HKU" | "HKCC") {
        return Err(ComponentEngineError::RegistryQuery(
            "registry hive is not allowed",
        ));
    }
    if key.is_empty()
        || name.is_empty()
        || key.contains(['\0', '\n', '\r'])
        || name.contains(['\0', '\n', '\r'])
    {
        return Err(ComponentEngineError::RegistryQuery(
            "registry key/value name is invalid",
        ));
    }
    Ok(CommandSpec {
        program: PathBuf::from(windows_wine::WINDOWS_WINE_BINARY),
        args: vec![
            "reg".to_owned(),
            "query".to_owned(),
            format!("{hive}\\{key}"),
            "/v".to_owned(),
            name.to_owned(),
        ],
    })
}

fn parse_registry_value(stdout: &str, name: &str) -> Option<String> {
    for line in stdout.lines() {
        let parts = line.split_whitespace().collect::<Vec<_>>();
        if parts.len() >= 3 && parts[0] == name && parts[1].starts_with("REG_") {
            return Some(parts[2..].join(" "));
        }
    }
    None
}

fn installer_exit_accepted(manifest: &WindowsComponentManifest, exit_code: Option<i32>) -> bool {
    exit_code
        .map(|code| manifest.accepted_exit_codes.contains(&code))
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::{
        WindowsComponentKind, WindowsComponentManifest, WindowsInstallerKind,
        WindowsVerificationProbe, WINDOWS_COMPONENT_SCHEMA,
    };
    use std::path::PathBuf;
    use uuid::Uuid;

    fn manifest(
        kind: WindowsInstallerKind,
        artifact: Option<&str>,
        args: &[&str],
    ) -> WindowsComponentManifest {
        WindowsComponentManifest {
            schema: WINDOWS_COMPONENT_SCHEMA.to_owned(),
            component_id: "runtime.fixture".to_owned(),
            revision: 1,
            digest: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
                .to_owned(),
            display_name: "Fixture".to_owned(),
            kind: WindowsComponentKind::Runtime,
            workload_arches: vec!["x86_64".to_owned()],
            depends_on: vec![],
            installer_kind: kind,
            artifact_identity: artifact.map(|_| {
                "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_owned()
            }),
            artifact_path: artifact.map(str::to_owned),
            installer_args: args.iter().map(|value| (*value).to_owned()).collect(),
            accepted_exit_codes: if artifact.is_some() {
                vec![0, 3010]
            } else {
                vec![]
            },
            restart_compatibility_environment: false,
            verification: vec![WindowsVerificationProbe::FileExists {
                path: "drive_c/proof.txt".to_owned(),
            }],
            limitations: vec![],
        }
    }

    #[test]
    fn component_engine_abi_is_exact_and_runtime_bound_to_transaction() {
        let app = Uuid::now_v7();
        let transaction = Uuid::now_v7();
        let runtime = format!(
            "/run/prime-win-component-{}",
            transaction.to_string().replace('-', "")
        );
        let parsed = parse_args(vec![
            "--manifest".to_owned(),
            "/usr/lib/prime/windows-components/runtime.fixture/revisions/00000000000000000001.json"
                .to_owned(),
            "--application-id".to_owned(),
            app.to_string(),
            "--transaction-id".to_owned(),
            transaction.to_string(),
            "--runtime-dir".to_owned(),
            runtime.clone(),
        ])
        .unwrap();
        assert_eq!(parsed.application_id, app);
        assert_eq!(parsed.transaction_id, transaction);
        assert_eq!(parsed.runtime_dir, PathBuf::from(runtime));
        assert!(validate_runtime_binding(&parsed).is_ok());
    }

    #[test]
    fn component_engine_abi_rejects_unknown_and_duplicate_flags() {
        assert!(parse_args(vec!["--backend".to_owned(), "wine".to_owned()]).is_err());
        let app = Uuid::now_v7();
        let tx = Uuid::now_v7();
        assert!(parse_args(vec![
            "--manifest".to_owned(),
            "/tmp/a.json".to_owned(),
            "--manifest".to_owned(),
            "/tmp/b.json".to_owned(),
            "--application-id".to_owned(),
            app.to_string(),
            "--transaction-id".to_owned(),
            tx.to_string(),
            "--runtime-dir".to_owned(),
            "/run/nope".to_owned(),
        ])
        .is_err());
    }

    #[test]
    fn installer_artifact_requires_exact_regular_file_sha256() {
        use std::os::unix::fs::symlink;
        let dir = tempfile::tempdir().unwrap();
        let artifact = dir.path().join("setup.exe");
        std::fs::write(&artifact, b"prime-w2-installer").unwrap();
        let expected = sha256_file(&artifact).unwrap();
        let mut m = manifest(
            WindowsInstallerKind::Exe,
            Some(artifact.to_str().unwrap()),
            &["/quiet"],
        );
        m.artifact_identity = Some(expected);
        validate_installer_artifact(&m).unwrap();

        m.artifact_identity = Some(format!("sha256:{}", "b".repeat(64)));
        assert!(matches!(
            validate_installer_artifact(&m),
            Err(ComponentEngineError::ArtifactIdentityMismatch)
        ));

        let link = dir.path().join("link.exe");
        symlink(&artifact, &link).unwrap();
        m.artifact_path = Some(link.display().to_string());
        m.artifact_identity = Some(sha256_file(&artifact).unwrap());
        assert!(matches!(
            validate_installer_artifact(&m),
            Err(ComponentEngineError::ArtifactPath(_))
        ));
    }

    #[test]
    fn registry_query_is_fixed_and_parses_exact_reg_sz_value() {
        let spec = registry_query_command("HKLM", r"Software\PrimeW2", "Installed").unwrap();
        assert_eq!(spec.program, PathBuf::from("/usr/bin/wine"));
        assert_eq!(
            spec.args,
            vec!["reg", "query", r"HKLM\Software\PrimeW2", "/v", "Installed"]
        );
        assert!(registry_query_command("HKEY_BAD", "Software", "Value").is_err());

        let stdout =
            "\r\nHKEY_LOCAL_MACHINE\\Software\\PrimeW2\r\n    Installed    REG_SZ    1\r\n";
        assert_eq!(
            parse_registry_value(stdout, "Installed").as_deref(),
            Some("1")
        );
        assert_eq!(parse_registry_value(stdout, "Missing"), None);
    }

    #[test]
    fn installer_exit_code_must_be_manifest_accepted() {
        let m = manifest(
            WindowsInstallerKind::Exe,
            Some("/trusted/setup.exe"),
            &["/quiet"],
        );
        assert!(installer_exit_accepted(&m, Some(0)));
        assert!(installer_exit_accepted(&m, Some(3010)));
        assert!(!installer_exit_accepted(&m, Some(1)));
        assert!(!installer_exit_accepted(&m, None));
    }

    #[test]
    fn transaction_installs_only_when_needed_and_seals_after_verified_state() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        let state = dir.path().join("state");
        std::fs::create_dir(&prefix).unwrap();
        std::fs::create_dir(prefix.join("drive_c")).unwrap();
        std::fs::create_dir(&state).unwrap();

        let artifact = dir.path().join("setup.exe");
        std::fs::write(&artifact, b"fixture-installer").unwrap();
        let mut m = manifest(
            WindowsInstallerKind::Exe,
            Some(artifact.to_str().unwrap()),
            &["/quiet"],
        );
        m.artifact_identity = Some(sha256_file(&artifact).unwrap());
        m.verification = vec![WindowsVerificationProbe::FileExists {
            path: "drive_c/installed.txt".to_owned(),
        }];
        m = windows_components::seal_component(m).unwrap();

        let calls = std::cell::RefCell::new(Vec::<Vec<String>>::new());
        let mut runner = |spec: &RuntimeCommand| -> Result<RuntimeOutput, ComponentEngineError> {
            calls.borrow_mut().push(spec.args.clone());
            if spec.args.first().map(String::as_str) == Some(artifact.to_str().unwrap()) {
                std::fs::write(prefix.join("drive_c/installed.txt"), b"ok").unwrap();
                return Ok(RuntimeOutput {
                    exit_code: Some(0),
                    stdout: String::new(),
                });
            }
            if spec.program == Path::new(windows_wine::WINDOWS_WINESERVER_BINARY) {
                return Ok(RuntimeOutput {
                    exit_code: Some(0),
                    stdout: String::new(),
                });
            }
            panic!("unexpected command: {spec:?}");
        };

        let result = execute_component_transaction_at(
            &state,
            &prefix,
            Uuid::nil(),
            Path::new("/run/prime-win-component-test"),
            &m,
            "2026-09-10T12:00:00Z",
            &mut runner,
        )
        .unwrap();
        assert_eq!(result, TransactionResult::Installed);
        assert_eq!(calls.borrow().len(), 2, "installer + wineserver wait only");
        assert!(primed::windows_component_engine::component_marker_matches(
            &state,
            &m,
            windows_wine::WINDOWS_WINE_DONOR_FINGERPRINT,
        )
        .unwrap());

        calls.borrow_mut().clear();
        let again = execute_component_transaction_at(
            &state,
            &prefix,
            Uuid::nil(),
            Path::new("/run/prime-win-component-test2"),
            &m,
            "2026-09-10T12:01:00Z",
            &mut runner,
        )
        .unwrap();
        assert_eq!(again, TransactionResult::AlreadySatisfied);
        assert!(
            calls.borrow().is_empty(),
            "verified component must not reinstall"
        );
    }

    #[test]
    fn successful_installer_exit_does_not_count_when_probe_fails() {
        let dir = tempfile::tempdir().unwrap();
        let prefix = dir.path().join("prefix");
        let state = dir.path().join("state");
        std::fs::create_dir(&prefix).unwrap();
        std::fs::create_dir(prefix.join("drive_c")).unwrap();
        std::fs::create_dir(&state).unwrap();
        let artifact = dir.path().join("setup.exe");
        std::fs::write(&artifact, b"fixture-installer").unwrap();
        let mut m = manifest(
            WindowsInstallerKind::Exe,
            Some(artifact.to_str().unwrap()),
            &[],
        );
        m.artifact_identity = Some(sha256_file(&artifact).unwrap());
        m.verification = vec![WindowsVerificationProbe::FileExists {
            path: "drive_c/never-created.txt".to_owned(),
        }];
        m = windows_components::seal_component(m).unwrap();
        let mut runner = |_spec: &RuntimeCommand| {
            Ok(RuntimeOutput {
                exit_code: Some(0),
                stdout: String::new(),
            })
        };
        assert!(matches!(
            execute_component_transaction_at(
                &state,
                &prefix,
                Uuid::nil(),
                Path::new("/run/prime-win-component-test"),
                &m,
                "2026-09-10T12:00:00Z",
                &mut runner,
            ),
            Err(ComponentEngineError::VerificationFailed)
        ));
        assert!(!primed::windows_component_engine::component_marker_matches(
            &state,
            &m,
            windows_wine::WINDOWS_WINE_DONOR_FINGERPRINT,
        )
        .unwrap());
    }

    #[test]
    fn runtime_preparation_uses_shared_application_state_and_private_ephemeral_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let runtime = dir.path().join("runtime");
        std::fs::create_dir(&runtime).unwrap();
        prepare_ephemeral_runtime(&runtime).unwrap();
        for name in ["home", "cache", "config"] {
            let path = runtime.join(name);
            assert!(path.is_dir());
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
                0o700
            );
        }
        assert_eq!(
            windows_state::application_prefix(Uuid::nil()),
            PathBuf::from("/var/lib/prime-win-app-00000000000000000000000000000000/wine-prefix")
        );
    }

    #[test]
    fn installer_commands_are_fixed_direct_argv() {
        let msi = installer_command(&manifest(
            WindowsInstallerKind::Msi,
            Some("/trusted/setup.msi"),
            &["/qn", "/norestart"],
        ))
        .unwrap()
        .unwrap();
        assert_eq!(msi.program, PathBuf::from("/usr/bin/wine"));
        assert_eq!(
            msi.args,
            vec!["msiexec", "/i", "/trusted/setup.msi", "/qn", "/norestart"]
        );

        let exe = installer_command(&manifest(
            WindowsInstallerKind::Exe,
            Some("/trusted/setup.exe"),
            &["/quiet"],
        ))
        .unwrap()
        .unwrap();
        assert_eq!(exe.program, PathBuf::from("/usr/bin/wine"));
        assert_eq!(exe.args, vec!["/trusted/setup.exe", "/quiet"]);

        assert!(
            installer_command(&manifest(WindowsInstallerKind::Builtin, None, &[]))
                .unwrap()
                .is_none()
        );
        for spec in [msi, exe] {
            assert!(!spec
                .args
                .iter()
                .any(|arg| matches!(arg.as_str(), "sh" | "bash" | "-c")));
        }
    }
}
