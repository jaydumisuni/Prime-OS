use primed::{generation, hardware, identity, server, CoreState};
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let state_dir = env::var_os("PRIME_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/prime"));
    let generation_file = env::var_os("PRIME_GENERATION_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/lib/prime/generation.json"));
    let socket_path = env::var_os("PRIME_CORE_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/prime/core.sock"));
    let systemd_run = env::var_os("PRIME_SYSTEMD_RUN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/bin/systemd-run"));

    let identity_path = state_dir.join("identity/host.json");
    let mut host = identity::load_or_create(&identity_path)?;
    let generation = generation::load(&generation_file)?;
    let observed_at = identity::now_rfc3339()?;
    let probe = prime_hardware::probe(Path::new("/"), &host.host_arch)?;
    host = identity::reconcile_fingerprint(&identity_path, host, &probe.fingerprint, &observed_at)?;
    let hardware = hardware::load_or_update(
        &state_dir.join("hardware/current.json"),
        probe,
        observed_at.clone(),
    )?;
    let state = CoreState::new(
        host,
        generation,
        hardware,
        state_dir,
        systemd_run,
        observed_at,
    );

    eprintln!(
        "primed: host={} generation={} hardware_revision={} socket={}",
        state.host.host_id,
        state.generation.generation_id,
        state.hardware.revision,
        socket_path.display()
    );

    server::run(&socket_path, state).await?;
    Ok(())
}
