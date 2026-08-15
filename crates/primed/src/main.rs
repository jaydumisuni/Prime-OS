use primed::{generation, hardware, identity, server, storage, CoreState};
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let state_dir = env::var_os("PRIME_STATE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/var/lib/prime"));
    let generation_seed_file = env::var_os("PRIME_GENERATION_SEED_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/lib/prime/generation-seed.json"));
    let socket_path = env::var_os("PRIME_CORE_SOCKET")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/run/prime/core.sock"));
    let systemd_run = env::var_os("PRIME_SYSTEMD_RUN")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/sbin/systemd-run"));
    let bootc = PathBuf::from("/usr/sbin/bootc");
    let storage_mountinfo = PathBuf::from("/proc/self/mountinfo");
    let storage_policy_file = env::var_os("PRIME_STORAGE_POLICY_FILE")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("/usr/lib/prime/storage-reserve-policy.json"));

    let identity_path = state_dir.join("identity/host.json");
    let mut host = identity::load_or_create(&identity_path)?;
    let generation =
        generation::load_or_bind(&generation_seed_file, &bootc, &state_dir, &host.host_arch)?;
    let observed_at = identity::now_rfc3339()?;
    let probe = prime_hardware::probe(Path::new("/"), &host.host_arch)?;
    host = identity::reconcile_fingerprint(&identity_path, host, &probe.fingerprint, &observed_at)?;
    let hardware = hardware::load_or_update(
        &state_dir.join("hardware/current.json"),
        probe,
        observed_at.clone(),
    )?;

    let mut storage_inventory = storage::observe(
        &storage_mountinfo,
        &storage_policy_file,
        observed_at.clone(),
        generation.generation_id.clone(),
    );
    match storage::persist_snapshot(
        &state_dir,
        host.host_id,
        &generation.generation_id,
        &storage_inventory,
    ) {
        Ok(report) => {
            if report.previous_cache_corrupt {
                eprintln!("primed rebuilt a corrupt non-authoritative storage cache");
            }
        }
        Err(error) => storage_inventory
            .limitations
            .push(format!("storage state persistence failed: {error}")),
    }

    let state = CoreState::new(
        host,
        generation,
        hardware,
        storage_inventory,
        state_dir,
        systemd_run,
        storage_mountinfo,
        storage_policy_file,
        observed_at,
    );

    let storage_pressure = state
        .storage
        .read()
        .map(|storage| format!("{:?}", storage.pressure.state))
        .unwrap_or_else(|_| "LOCK_POISONED".to_owned());
    eprintln!(
        "primed: host={} generation={} hardware_revision={} storage_pressure={} socket={}",
        state.host.host_id,
        state.generation.generation_id,
        state.hardware.revision,
        storage_pressure,
        socket_path.display()
    );

    server::run(&socket_path, state).await?;
    Ok(())
}
