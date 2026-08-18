use serde::Serialize;
use smithay::{
    backend::{
        drm::DrmNode,
        libinput::{LibinputInputBackend, LibinputSessionInterface},
        session::{
            libseat::{LibSeatSession, LibSeatSessionNotifier},
            Event as SessionEvent, Session,
        },
        udev::{all_gpus, primary_gpu, UdevBackend, UdevEvent},
    },
    reexports::{
        calloop::{generic::Generic, EventLoop, Interest, Mode, PostAction},
        input::Libinput,
        rustix::fs::OFlags,
        wayland_server::{
            backend::{ClientData, ClientId, DisconnectReason},
            Display, DisplayHandle,
        },
    },
    wayland::socket::ListeningSocketSource,
};
use std::{
    env,
    error::Error,
    ffi::OsString,
    fs,
    io,
    path::{Path, PathBuf},
    process,
    sync::Arc,
    time::Duration,
};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

const READINESS_SCHEMA: &str = "prime.compositor-readiness.v1";
const DEFAULT_READINESS_PATH: &str = "/run/prime/compositor/readiness.json";

#[derive(Debug, Serialize)]
struct Readiness {
    schema: &'static str,
    observed_at: String,
    phase: &'static str,
    direct_tty_backend: bool,
    seat_name: String,
    wayland_socket: String,
    primary_gpu: String,
    gpu_count: usize,
    udev_device_count: usize,
    drm_access_ready: bool,
    libinput_bound: bool,
    session_active: bool,
    wayland_listener_ready: bool,
    wayland_protocols_ready: bool,
    renderer_ready: bool,
    outputs_ready: bool,
    shell_ready: bool,
    clients_accepted: u64,
    input_events_seen: u64,
    last_udev_event: Option<String>,
    limitations: Vec<String>,
}

struct Runtime {
    display_handle: DisplayHandle,
    _session: LibSeatSession,
    readiness_path: PathBuf,
    readiness: Readiness,
}

impl Runtime {
    fn persist(&mut self) -> Result<(), Box<dyn Error>> {
        self.readiness.observed_at = now_rfc3339()?;
        write_json_atomic(&self.readiness_path, &self.readiness)?;
        Ok(())
    }

    fn persist_best_effort(&mut self) {
        if let Err(error) = self.persist() {
            eprintln!("prime-compositor could not persist readiness: {error}");
        }
    }
}

#[derive(Default)]
struct PrimeClientState;

impl ClientData for PrimeClientState {
    fn initialized(&self, _client_id: ClientId) {}

    fn disconnected(&self, _client_id: ClientId, _reason: DisconnectReason) {}
}

fn main() -> Result<(), Box<dyn Error>> {
    let probe_only = parse_args()?;
    let readiness_path = env::var_os("PRIME_COMPOSITOR_READINESS")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_READINESS_PATH));

    let mut event_loop: EventLoop<Runtime> = EventLoop::try_new()?;
    let display: Display<Runtime> = Display::new()?;
    let display_handle = display.handle();

    let (mut session, notifier) = LibSeatSession::new()?;
    let seat_name = session.seat();
    let session_active = session.is_active();

    let gpu_paths = all_gpus(&seat_name)?;
    if gpu_paths.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no DRM GPU is available on seat {seat_name}"),
        )
        .into());
    }
    let primary_gpu_path = primary_gpu(&seat_name)?.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("no primary DRM GPU is available on seat {seat_name}"),
        )
    })?;
    let _primary_gpu_node = DrmNode::from_path(&primary_gpu_path)?;

    let drm_fd = session.open(
        &primary_gpu_path,
        OFlags::RDWR | OFlags::CLOEXEC | OFlags::NOCTTY | OFlags::NONBLOCK,
    )?;
    session.close(drm_fd)?;

    let udev_backend = UdevBackend::new(&seat_name)?;
    let udev_device_count = udev_backend.device_list().count();
    if udev_device_count == 0 {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("udev reported no DRM device on seat {seat_name}"),
        )
        .into());
    }

    let mut libinput_context =
        Libinput::new_with_udev::<LibinputSessionInterface<LibSeatSession>>(session.clone().into());
    libinput_context
        .udev_assign_seat(&seat_name)
        .map_err(|()| io::Error::other(format!("libinput rejected seat {seat_name}")))?;
    let libinput_backend = LibinputInputBackend::new(libinput_context.clone());

    let listening_socket = ListeningSocketSource::new_auto()?;
    let socket_name: OsString = listening_socket.socket_name().to_os_string();

    let loop_handle = event_loop.handle();
    loop_handle.insert_source(listening_socket, |client_stream, _, runtime| {
        match runtime
            .display_handle
            .insert_client(client_stream, Arc::new(PrimeClientState))
        {
            Ok(_) => {
                runtime.readiness.clients_accepted =
                    runtime.readiness.clients_accepted.saturating_add(1);
            }
            Err(error) => {
                eprintln!("prime-compositor rejected Wayland client: {error}");
            }
        }
    })?;

    loop_handle.insert_source(
        Generic::new(display, Interest::READ, Mode::Level),
        |_, display, runtime| {
            // SAFETY: calloop owns the Display source for the lifetime of this event source,
            // matching Smithay's v0.7.0 Smallvil listener pattern.
            unsafe {
                if let Err(error) = display.get_mut().dispatch_clients(runtime) {
                    eprintln!("prime-compositor Wayland dispatch failed: {error}");
                }
            }
            Ok(PostAction::Continue)
        },
    )?;

    loop_handle.insert_source(libinput_backend, |_, _, runtime| {
        runtime.readiness.input_events_seen = runtime.readiness.input_events_seen.saturating_add(1);
    })?;

    install_session_notifier(&loop_handle, notifier, libinput_context)?;

    loop_handle.insert_source(udev_backend, |event, _, runtime| {
        match event {
            UdevEvent::Added { device_id, path } => {
                runtime.readiness.udev_device_count =
                    runtime.readiness.udev_device_count.saturating_add(1);
                runtime.readiness.last_udev_event =
                    Some(format!("ADDED:{device_id}:{}", path.display()));
            }
            UdevEvent::Changed { device_id } => {
                runtime.readiness.last_udev_event = Some(format!("CHANGED:{device_id}"));
            }
            UdevEvent::Removed { device_id } => {
                runtime.readiness.udev_device_count =
                    runtime.readiness.udev_device_count.saturating_sub(1);
                runtime.readiness.last_udev_event = Some(format!("REMOVED:{device_id}"));
            }
        }
        runtime.persist_best_effort();
    })?;

    let mut runtime = Runtime {
        display_handle,
        _session: session,
        readiness_path,
        readiness: Readiness {
            schema: READINESS_SCHEMA,
            observed_at: now_rfc3339()?,
            phase: "BACKEND_PREFLIGHT",
            direct_tty_backend: true,
            seat_name,
            wayland_socket: socket_name.to_string_lossy().into_owned(),
            primary_gpu: primary_gpu_path.display().to_string(),
            gpu_count: gpu_paths.len(),
            udev_device_count,
            drm_access_ready: true,
            libinput_bound: true,
            session_active,
            wayland_listener_ready: true,
            wayland_protocols_ready: false,
            renderer_ready: false,
            outputs_ready: false,
            shell_ready: false,
            clients_accepted: 0,
            input_events_seen: 0,
            last_udev_event: None,
            limitations: vec![
                "Wayland compositor protocol globals are not initialized yet".to_owned(),
                "GBM/EGL/GLES renderer is not initialized yet".to_owned(),
                "DRM outputs are not configured yet".to_owned(),
                "Prime Shell is not started yet".to_owned(),
            ],
        },
    };
    runtime.persist()?;

    if probe_only {
        println!("{}", serde_json::to_string_pretty(&runtime.readiness)?);
        return Ok(());
    }

    loop {
        event_loop.dispatch(Some(Duration::from_millis(16)), &mut runtime)?;
        runtime.display_handle.flush_clients()?;
    }
}

fn install_session_notifier(
    loop_handle: &smithay::reexports::calloop::LoopHandle<'_, Runtime>,
    notifier: LibSeatSessionNotifier,
    mut libinput_context: Libinput,
) -> Result<(), smithay::reexports::calloop::InsertError<LibSeatSessionNotifier>> {
    loop_handle.insert_source(notifier, move |event, _, runtime| {
        match event {
            SessionEvent::PauseSession => {
                libinput_context.suspend();
                runtime.readiness.session_active = false;
            }
            SessionEvent::ActivateSession => {
                if libinput_context.resume().is_err() {
                    eprintln!("prime-compositor could not resume libinput");
                    runtime.readiness.session_active = false;
                } else {
                    runtime.readiness.session_active = true;
                }
            }
        }
        runtime.persist_best_effort();
    })?;
    Ok(())
}

fn parse_args() -> Result<bool, Box<dyn Error>> {
    let mut probe_only = false;
    for arg in env::args().skip(1) {
        match arg.as_str() {
            "--probe" => probe_only = true,
            "--help" | "-h" => {
                println!("Usage: prime-compositor [--probe]");
                process::exit(0);
            }
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("unknown argument: {arg}"),
                )
                .into());
            }
        }
    }
    Ok(probe_only)
}

fn now_rfc3339() -> Result<String, time::error::Format> {
    OffsetDateTime::now_utc().format(&Rfc3339)
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T) -> Result<(), Box<dyn Error>> {
    let parent = path.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "compositor readiness path has no parent directory",
        )
    })?;
    fs::create_dir_all(parent)?;
    let temp = parent.join(format!(".readiness.{}.tmp", process::id()));
    let bytes = serde_json::to_vec_pretty(value)?;
    fs::write(&temp, bytes)?;
    fs::rename(temp, path)?;
    Ok(())
}
