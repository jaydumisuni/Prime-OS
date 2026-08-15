use primed::{generation, identity, server, CoreState};
use std::env;
use std::error::Error;
use std::path::PathBuf;

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

    let host = identity::load_or_create(&state_dir.join("identity/host.json"))?;
    let generation = generation::load(&generation_file)?;
    let observed_at = identity::now_rfc3339()?;
    let state = CoreState::new(host, generation, observed_at);

    eprintln!(
        "primed: host={} generation={} socket={}",
        state.host.host_id,
        state.generation.generation_id,
        socket_path.display()
    );

    server::run(&socket_path, state).await?;
    Ok(())
}
