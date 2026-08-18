use prime_contracts::{GenerationRecord, HardwareGraph, HostIdentity, StorageInventory};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::env;
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

const RECOVERY_STATUS_SCHEMA: &str = "prime.recovery-status.v1";
const MAX_STATE_FILE_BYTES: u64 = 2 * 1024 * 1024;

#[derive(Debug, Serialize)]
struct RecoverySnapshot {
    schema: &'static str,
    observed_at: String,
    shell_independent: bool,
    host: Option<HostIdentity>,
    generation: Option<GenerationRecord>,
    hardware: Option<HardwareGraph>,
    storage: Option<StorageInventory>,
    limitations: Vec<String>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let state_dir = env::var_os("PRIME_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/prime"));

    let mut json_only = false;
    let mut once = false;
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--json" => json_only = true,
            "--once" => once = true,
            "--help" | "-h" => {
                println!("Usage: prime-recovery [--json|--once]");
                return Ok(());
            }
            _ => return Err(format!("unknown argument: {arg}").into()),
        }
    }
    if json_only && once {
        return Err("--json and --once are mutually exclusive".into());
    }

    let snapshot = load_snapshot(&state_dir);
    if json_only {
        println!("{}", serde_json::to_string_pretty(&snapshot)?);
        return Ok(());
    }
    if once {
        print_human(&snapshot);
        return Ok(());
    }

    interactive_loop(&state_dir)
}

fn interactive_loop(state_dir: &Path) -> Result<(), Box<dyn Error>> {
    loop {
        let snapshot = load_snapshot(state_dir);
        print!("\x1b[2J\x1b[H");
        print_human(&snapshot);
        println!();
        println!("Recovery controls:");
        println!("  [s] refresh persisted Prime state");
        println!("  [j] print recovery status as JSON");
        println!("  [r] reboot");
        println!("  [p] power off");
        println!();
        print!("prime-recovery> ");
        io::stdout().flush()?;

        let mut input = String::new();
        if io::stdin().read_line(&mut input)? == 0 {
            return Err("recovery console input closed".into());
        }
        match input.trim().to_ascii_lowercase().as_str() {
            "s" | "" => continue,
            "j" => {
                println!("{}", serde_json::to_string_pretty(&snapshot)?);
                wait_for_enter()?;
            }
            "r" => {
                run_systemctl("reboot")?;
                return Ok(());
            }
            "p" => {
                run_systemctl("poweroff")?;
                return Ok(());
            }
            _ => {
                println!("Unknown recovery command. No action taken.");
                wait_for_enter()?;
            }
        }
    }
}

fn wait_for_enter() -> io::Result<()> {
    print!("Press Enter to continue...");
    io::stdout().flush()?;
    let mut input = String::new();
    let _ = io::stdin().read_line(&mut input)?;
    Ok(())
}

fn run_systemctl(action: &str) -> Result<(), Box<dyn Error>> {
    let status = Command::new("/usr/bin/systemctl").arg(action).status()?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("systemctl {action} failed with status {status}").into())
    }
}

fn load_snapshot(state_dir: &Path) -> RecoverySnapshot {
    let mut limitations = Vec::new();
    let host = read_state_json(
        &state_dir.join("identity/host.json"),
        "Prime Host identity",
        &mut limitations,
    );
    let generation = read_state_json(
        &state_dir.join("generations/current.json"),
        "current Prime generation",
        &mut limitations,
    );
    let hardware = read_state_json(
        &state_dir.join("hardware/current.json"),
        "hardware graph",
        &mut limitations,
    );
    let storage = read_state_json(
        &state_dir.join("storage/current.json"),
        "storage inventory",
        &mut limitations,
    );

    RecoverySnapshot {
        schema: RECOVERY_STATUS_SCHEMA,
        observed_at: now_rfc3339(),
        shell_independent: true,
        host,
        generation,
        hardware,
        storage,
        limitations,
    }
}

fn read_state_json<T: DeserializeOwned>(
    path: &Path,
    label: &str,
    limitations: &mut Vec<String>,
) -> Option<T> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            limitations.push(format!("{label} is not persisted at {}", path.display()));
            return None;
        }
        Err(error) => {
            limitations.push(format!("{label} could not be opened at {}: {error}", path.display()));
            return None;
        }
    };

    let mut bytes = Vec::new();
    let mut limited = file.take(MAX_STATE_FILE_BYTES + 1);
    match limited.read_to_end(&mut bytes) {
        Ok(_) if bytes.len() as u64 <= MAX_STATE_FILE_BYTES => {}
        Ok(_) => {
            limitations.push(format!(
                "{label} at {} exceeds the {} byte recovery read limit",
                path.display(),
                MAX_STATE_FILE_BYTES
            ));
            return None;
        }
        Err(error) => {
            limitations.push(format!("{label} could not be read at {}: {error}", path.display()));
            return None;
        }
    }

    match serde_json::from_slice(&bytes) {
        Ok(value) => Some(value),
        Err(error) => {
            limitations.push(format!("{label} is invalid at {}: {error}", path.display()));
            None
        }
    }
}

fn print_human(snapshot: &RecoverySnapshot) {
    println!("Prime OS Recovery");
    println!("=================");
    println!("Observed: {}", snapshot.observed_at);
    println!("Prime Shell dependency: NONE");

    match &snapshot.host {
        Some(host) => println!("Host: {} ({})", host.host_id, host.host_arch),
        None => println!("Host: unavailable"),
    }
    match &snapshot.generation {
        Some(generation) => println!(
            "Generation: {} [{:?}] image={}",
            generation.generation_id, generation.state, generation.image_digest
        ),
        None => println!("Generation: unavailable"),
    }
    match &snapshot.hardware {
        Some(hardware) => println!(
            "Hardware graph: revision {} digest={}",
            hardware.revision, hardware.topology_digest
        ),
        None => println!("Hardware graph: unavailable"),
    }
    match &snapshot.storage {
        Some(storage) => println!(
            "Storage: pressure={:?}, reserve_configured={}, root_mount={:?}",
            storage.pressure.state, storage.reserve.policy_configured, storage.root_mount_id
        ),
        None => println!("Storage: unavailable"),
    }

    if snapshot.limitations.is_empty() {
        println!("State limitations: none");
    } else {
        println!("State limitations:");
        for limitation in &snapshot.limitations {
            println!("  - {limitation}");
        }
    }
}

fn now_rfc3339() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| "UNKNOWN".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use prime_contracts::{GenerationState, ReleaseChannel, GENERATION_SCHEMA};
    use std::fs;

    fn digest(fill: char) -> String {
        format!("sha256:{}", fill.to_string().repeat(64))
    }

    #[test]
    fn missing_state_is_reported_without_blocking_recovery() {
        let dir = tempfile::tempdir().expect("tempdir");
        let snapshot = load_snapshot(dir.path());
        assert!(snapshot.shell_independent);
        assert!(snapshot.host.is_none());
        assert!(snapshot.generation.is_none());
        assert!(snapshot.hardware.is_none());
        assert!(snapshot.storage.is_none());
        assert_eq!(snapshot.limitations.len(), 4);
    }

    #[test]
    fn valid_generation_is_recovered_even_when_other_state_is_missing() {
        let dir = tempfile::tempdir().expect("tempdir");
        let generation_dir = dir.path().join("generations");
        fs::create_dir_all(&generation_dir).expect("generation dir");
        let generation = GenerationRecord {
            schema: GENERATION_SCHEMA.to_owned(),
            generation_id: "p1-test".to_owned(),
            image_digest: digest('a'),
            channel: ReleaseChannel::Lab,
            created_at: "2026-08-18T00:00:00Z".to_owned(),
            source_revision: "abcdef".to_owned(),
            state: GenerationState::BootTry,
            boot_attempts_remaining: Some(3),
            evidence_refs: vec!["fixture".to_owned()],
        };
        fs::write(
            generation_dir.join("current.json"),
            serde_json::to_vec(&generation).expect("generation json"),
        )
        .expect("write generation");

        let snapshot = load_snapshot(dir.path());
        assert_eq!(
            snapshot
                .generation
                .as_ref()
                .map(|value| value.generation_id.as_str()),
            Some("p1-test")
        );
        assert_eq!(snapshot.limitations.len(), 3);
    }

    #[test]
    fn corrupt_state_is_limited_not_fatal() {
        let dir = tempfile::tempdir().expect("tempdir");
        let identity_dir = dir.path().join("identity");
        fs::create_dir_all(&identity_dir).expect("identity dir");
        fs::write(identity_dir.join("host.json"), b"not-json").expect("write corrupt state");

        let snapshot = load_snapshot(dir.path());
        assert!(snapshot.host.is_none());
        assert!(snapshot
            .limitations
            .iter()
            .any(|value| value.contains("Prime Host identity is invalid")));
    }
}
