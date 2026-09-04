mod core_client;
mod motion;
mod visual;

use std::{
    error::Error,
    io,
    num::NonZeroU32,
    os::fd::AsRawFd,
    time::{Duration, Instant},
};

use prime_contracts::SystemPowerAction;

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_keyboard, delegate_layer, delegate_output, delegate_pointer,
    delegate_registry, delegate_seat, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
    seat::{
        keyboard::{KeyEvent, KeyboardHandler, Keysym, Modifiers, RawModifiers},
        pointer::{PointerEvent, PointerEventKind, PointerHandler, BTN_LEFT},
        Capability, SeatHandler, SeatState,
    },
    shell::{
        wlr_layer::{
            Anchor, KeyboardInteractivity, Layer, LayerShell, LayerShellHandler, LayerSurface,
            LayerSurfaceConfigure,
        },
        WaylandSurface,
    },
    shm::{slot::SlotPool, Shm, ShmHandler},
};
use wayland_client::{
    globals::registry_queue_init,
    protocol::{wl_keyboard, wl_output, wl_pointer, wl_seat, wl_shm, wl_surface},
    Connection, QueueHandle,
};

const BACKGROUND_NAMESPACE: &str = "prime.shell.background";
const RAIL_NAMESPACE: &str = "prime.shell.rail";
const STATUS_NAMESPACE: &str = "prime.shell.status";
const PRIME_NAMESPACE: &str = "prime.shell.prime";
const QUICK_CONTROLS_NAMESPACE: &str = "prime.shell.quick-controls";

fn finish_wayland_read(
    result: Result<usize, wayland_client::backend::WaylandError>,
) -> Result<(), wayland_client::backend::WaylandError> {
    match result {
        Ok(_) => Ok(()),
        Err(wayland_client::backend::WaylandError::Io(error))
            if error.kind() == io::ErrorKind::WouldBlock =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--font-probe") {
        let text = visual::TextSystem::load_system()?;
        println!("PRIME_SHELL_FONT_PROBE={}", text.family_name());
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--help" || arg == "-h") {
        println!("prime-shell — Prime P1 Shell interaction construction host");
        println!("Usage: prime-shell [--font-probe]");
        println!("Persistent baseline: background + rail");
        println!("Functional surfaces: Prime launcher + truthful quick controls");
        return Ok(());
    }

    let connection = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init(&connection)?;
    let queue_handle = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &queue_handle)?;
    let layer_shell = LayerShell::bind(&globals, &queue_handle)?;
    let shm = Shm::bind(&globals, &queue_handle)?;
    let pool = SlotPool::new(4, &shm)?;

    let config_root = std::env::var_os("XDG_CONFIG_HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            std::env::var_os("HOME")
                .map(std::path::PathBuf::from)
                .map(|home| home.join(".config"))
        });
    let rail_config_path = config_root
        .as_ref()
        .map(|root| root.join("prime/rail.json"));
    let rail_configuration = rail_config_path
        .as_deref()
        .map(visual::RailConfiguration::load_from_path)
        .unwrap_or_default();
    if let Some(path) = rail_config_path.as_deref() {
        if !path.exists() {
            if let Err(error) = rail_configuration.save_to_path(path) {
                eprintln!("prime-shell could not persist default rail configuration: {error}");
            }
        }
    }
    let rail_actions = rail_configuration.actions().to_vec();
    let rail_height = visual::rail_height_for_items(rail_actions.len());

    let wallpaper_config_path = config_root
        .as_ref()
        .map(|root| root.join("prime/wallpaper.json"));
    let requested_wallpaper = wallpaper_config_path
        .as_deref()
        .map(visual::wallpaper::WallpaperSelection::load_from_path)
        .unwrap_or_default();
    if let Some(path) = wallpaper_config_path.as_deref() {
        if !path.exists() {
            if let Err(error) = requested_wallpaper.save_to_path(path) {
                eprintln!("prime-shell could not persist default wallpaper selection: {error}");
            }
        }
    }
    let (wallpaper_selection, system_wallpaper) = match requested_wallpaper {
        visual::wallpaper::WallpaperSelection::Animated => {
            eprintln!("PRIME_SHELL_WALLPAPER=animated-first-light");
            (visual::wallpaper::WallpaperSelection::Animated, None)
        }
        visual::wallpaper::WallpaperSelection::System(index) => {
            match visual::wallpaper::decode_system_wallpaper(index) {
                Ok(decoded) => {
                    let entry = &visual::wallpaper::system_wallpapers()[index];
                    eprintln!(
                        "PRIME_SHELL_WALLPAPER=system;id={};title={}",
                        entry.id, entry.title
                    );
                    (requested_wallpaper, Some(decoded))
                }
                Err(error) => {
                    eprintln!(
                        "prime-shell rejected selected system wallpaper: {error}; falling back to animated First Light"
                    );
                    (visual::wallpaper::WallpaperSelection::Animated, None)
                }
            }
        }
    };

    let background_surface = compositor.create_surface(&queue_handle);
    let background = layer_shell.create_layer_surface(
        &queue_handle,
        background_surface,
        Layer::Background,
        Some(BACKGROUND_NAMESPACE),
        None,
    );
    background.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    background.set_keyboard_interactivity(KeyboardInteractivity::None);
    background.set_exclusive_zone(-1);
    background.set_size(0, 0);
    background.commit();

    let rail_surface = compositor.create_surface(&queue_handle);
    let rail = layer_shell.create_layer_surface(
        &queue_handle,
        rail_surface,
        Layer::Top,
        Some(RAIL_NAMESPACE),
        None,
    );
    rail.set_anchor(Anchor::TOP | Anchor::LEFT);
    rail.set_margin(visual::RAIL_TOP_MARGIN, 0, 0, visual::RAIL_LEFT_MARGIN);
    rail.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    rail.set_exclusive_zone(0);
    rail.set_size(visual::RAIL_WIDTH, rail_height);
    rail.commit();

    let status_surface = compositor.create_surface(&queue_handle);
    let status_cluster = layer_shell.create_layer_surface(
        &queue_handle,
        status_surface,
        Layer::Top,
        Some(STATUS_NAMESPACE),
        None,
    );
    status_cluster.set_anchor(Anchor::TOP | Anchor::RIGHT);
    status_cluster.set_margin(
        visual::STATUS_CLUSTER_TOP_MARGIN,
        visual::STATUS_CLUSTER_RIGHT_MARGIN,
        0,
        0,
    );
    status_cluster.set_keyboard_interactivity(KeyboardInteractivity::None);
    status_cluster.set_exclusive_zone(0);
    status_cluster.set_size(visual::STATUS_CLUSTER_WIDTH, visual::STATUS_CLUSTER_HEIGHT);
    status_cluster.commit();

    let text = visual::TextSystem::load_system()?;
    eprintln!("PRIME_SHELL_FONT={}", text.family_name());

    let mut shell = PrimeShell {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &queue_handle),
        output_state: OutputState::new(&globals, &queue_handle),
        compositor,
        layer_shell,
        shm,
        pool,
        text,
        theme: visual::Theme::prime_dark(),
        wallpaper_selection,
        system_wallpaper,
        background,
        rail,
        status_cluster,
        rail_width: 0,
        rail_height,
        rail_actions,
        background_configured: false,
        background_width: 0,
        background_height: 0,
        background_static: Vec::new(),
        background_started: Instant::now(),
        background_last_render: Instant::now(),
        background_motion_frames: 0,
        rail_configured: false,
        status_configured: false,
        baseline_reported: false,
        prime_launcher: None,
        quick_controls: None,
        keyboard: None,
        pointer: None,
        keyboard_focus: None,
        core: core_client::CoreClient::from_env(),
        applications: Vec::new(),
        selected_application: 0,
        launcher_query: String::new(),
        prime_launcher_message: None,
        quick_lines: Vec::new(),
        quick_power_ready: false,
        pending_power: None,
        quick_message: None,
        exit: false,
    };

    eprintln!("PRIME_SHELL_CORE_BRIDGE=typed_application_id;quick_controls=truthful_status;power=typed_double_confirm");

    while !shell.exit {
        event_queue.flush()?;
        event_queue.dispatch_pending(&mut shell)?;

        if let Some(read_guard) = event_queue.prepare_read() {
            let fd = read_guard.connection_fd().as_raw_fd();
            let mut poll_fd = libc::pollfd {
                fd,
                events: libc::POLLIN | libc::POLLERR | libc::POLLHUP,
                revents: 0,
            };
            let ready = unsafe { libc::poll(&mut poll_fd, 1, 16) };
            if ready < 0 {
                let error = io::Error::last_os_error();
                if error.kind() != io::ErrorKind::Interrupted {
                    return Err(error.into());
                }
                drop(read_guard);
            } else if ready > 0 {
                finish_wayland_read(read_guard.read())?;
                event_queue.dispatch_pending(&mut shell)?;
            } else {
                drop(read_guard);
            }
        }

        shell.redraw_background_motion(Instant::now());
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShellSurfaceKind {
    Rail,
    PrimeLauncher,
    QuickControls,
}

#[derive(Clone, Copy)]
enum InteractionSource {
    Pointer,
    Keyboard,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PowerConfirmation {
    Arm(SystemPowerAction),
    Execute(SystemPowerAction),
}

fn power_confirmation(
    pending: Option<SystemPowerAction>,
    requested: SystemPowerAction,
) -> PowerConfirmation {
    if pending == Some(requested) {
        PowerConfirmation::Execute(requested)
    } else {
        PowerConfirmation::Arm(requested)
    }
}

fn power_action_name(action: SystemPowerAction) -> &'static str {
    match action {
        SystemPowerAction::Reboot => "RESTART",
        SystemPowerAction::PowerOff => "POWER OFF",
    }
}

fn launcher_query_matches(display_name: &str, query: &str) -> bool {
    let query = query.trim();
    query.is_empty() || display_name.to_lowercase().contains(&query.to_lowercase())
}

struct TransientSurface {
    layer: LayerSurface,
    width: u32,
    height: u32,
    transition: Option<motion::Transition>,
    progress: f32,
    closing: bool,
}

struct PrimeShell {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: SlotPool,
    text: visual::TextSystem,
    theme: visual::Theme,
    wallpaper_selection: visual::wallpaper::WallpaperSelection,
    system_wallpaper: Option<visual::wallpaper::DecodedWallpaper>,
    background: LayerSurface,
    rail: LayerSurface,
    status_cluster: LayerSurface,
    rail_width: u32,
    rail_height: u32,
    rail_actions: Vec<visual::RailAction>,
    background_configured: bool,
    background_width: u32,
    background_height: u32,
    background_static: Vec<u8>,
    background_started: Instant,
    background_last_render: Instant,
    background_motion_frames: u64,
    rail_configured: bool,
    status_configured: bool,
    baseline_reported: bool,
    prime_launcher: Option<TransientSurface>,
    quick_controls: Option<TransientSurface>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard_focus: Option<ShellSurfaceKind>,
    core: core_client::CoreClient,
    applications: Vec<prime_contracts::ApplicationEntry>,
    selected_application: usize,
    launcher_query: String,
    prime_launcher_message: Option<String>,
    quick_lines: Vec<String>,
    quick_power_ready: bool,
    pending_power: Option<SystemPowerAction>,
    quick_message: Option<String>,
    exit: bool,
}

fn draw_visual_surface<F>(
    pool: &mut SlotPool,
    layer: &LayerSurface,
    width: u32,
    height: u32,
    painter: F,
) -> Result<(), Box<dyn Error>>
where
    F: FnOnce(&mut [u8], u32, u32),
{
    let buffer_width = i32::try_from(width)?;
    let buffer_height = i32::try_from(height)?;
    let stride = buffer_width
        .checked_mul(4)
        .ok_or_else(|| io::Error::other("Prime Shell visual stride overflow"))?;
    let (buffer, canvas) = pool.create_buffer(
        buffer_width,
        buffer_height,
        stride,
        wl_shm::Format::Argb8888,
    )?;
    painter(canvas, width, height);
    layer
        .wl_surface()
        .damage_buffer(0, 0, buffer_width, buffer_height);
    buffer.attach_to(layer.wl_surface())?;
    layer.commit();
    Ok(())
}

fn persistent_baseline_ready(background: bool, rail: bool, status: bool) -> bool {
    background && rail && status
}

impl PrimeShell {
    fn report_baseline_once(&mut self) {
        if persistent_baseline_ready(
            self.background_configured,
            self.rail_configured,
            self.status_configured,
        ) && !self.baseline_reported
        {
            self.baseline_reported = true;
            eprintln!(
                "PRIME_SHELL_PERSISTENT_BASELINE_CONFIGURED=background,rail,status;readiness_unearned"
            );
        }
    }

    fn redraw_background_motion(&mut self, now: Instant) {
        if self.wallpaper_selection != visual::wallpaper::WallpaperSelection::Animated
            || !self.background_configured
            || self.background_width == 0
            || self.background_height == 0
            || self.background_static.is_empty()
        {
            return;
        }

        let layer = self.background.clone();
        // The main event loop wakes on a bounded 16 ms poll even when Wayland is
        // otherwise idle. Upload only when the wallpaper phase has advanced far
        // enough to avoid a busy redraw loop.
        if now.duration_since(self.background_last_render) < Duration::from_millis(30) {
            return;
        }

        let width = self.background_width;
        let height = self.background_height;
        let theme = self.theme;
        let phase = self.background_started.elapsed().as_secs_f32() * 0.22;
        let background_static = &self.background_static;
        if let Err(error) =
            draw_visual_surface(&mut self.pool, &layer, width, height, |bytes, w, h| {
                debug_assert_eq!(bytes.len(), background_static.len());
                bytes.copy_from_slice(background_static);
                let mut canvas = visual::Canvas::new(bytes, w, h)
                    .expect("Prime background buffer must match configured dimensions");
                visual::paint_background_motion(&mut canvas, &theme, phase);
            })
        {
            eprintln!("prime-shell could not animate background: {error}");
            self.exit = true;
            return;
        }
        self.background_last_render = now;
        self.background_motion_frames = self.background_motion_frames.saturating_add(1);
        if self.background_motion_frames.is_multiple_of(300) {
            eprintln!(
                "PRIME_SHELL_IDLE_MOTION=active;frames={};phase={phase:.3}",
                self.background_motion_frames
            );
        }
    }

    fn redraw_rail(&mut self) {
        if self.rail_width == 0 {
            return;
        }
        let layer = self.rail.clone();
        let width = self.rail_width;
        let height = self.rail_height;
        let theme = self.theme;
        let active = if self.prime_launcher.is_some() {
            Some(visual::RailAction::Prime)
        } else if self.quick_controls.is_some() {
            Some(visual::RailAction::Status)
        } else {
            None
        };
        let text = &mut self.text;
        let rail_actions = &self.rail_actions;
        if let Err(error) =
            draw_visual_surface(&mut self.pool, &layer, width, height, |bytes, w, h| {
                let mut canvas = visual::Canvas::new(bytes, w, h)
                    .expect("Prime rail buffer must match configured dimensions");
                visual::paint_rail_surface(&mut canvas, text, &theme, rail_actions, active);
            })
        {
            eprintln!("prime-shell could not redraw rail: {error}");
            self.exit = true;
        }
    }

    fn surface_kind(&self, surface: &wl_surface::WlSurface) -> Option<ShellSurfaceKind> {
        if surface == self.rail.wl_surface() {
            Some(ShellSurfaceKind::Rail)
        } else if self
            .prime_launcher
            .as_ref()
            .is_some_and(|overlay| surface == overlay.layer.wl_surface())
        {
            Some(ShellSurfaceKind::PrimeLauncher)
        } else if self
            .quick_controls
            .as_ref()
            .is_some_and(|overlay| surface == overlay.layer.wl_surface())
        {
            Some(ShellSurfaceKind::QuickControls)
        } else {
            None
        }
    }

    fn create_overlay(
        &self,
        queue_handle: &QueueHandle<Self>,
        namespace: &'static str,
        anchor: Anchor,
        margins: (i32, i32, i32, i32),
        width: u32,
        height: u32,
    ) -> TransientSurface {
        let surface = self.compositor.create_surface(queue_handle);
        let layer = self.layer_shell.create_layer_surface(
            queue_handle,
            surface,
            Layer::Overlay,
            Some(namespace),
            None,
        );
        layer.set_anchor(anchor);
        layer.set_margin(margins.0, margins.1, margins.2, margins.3);
        layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        layer.set_exclusive_zone(0);
        layer.set_size(width, height);
        layer.commit();
        TransientSurface {
            layer,
            width,
            height,
            transition: Some(motion::Transition::opening(
                Duration::from_millis(200),
                Instant::now(),
            )),
            progress: 0.0,
            closing: false,
        }
    }

    fn toggle_prime_launcher(
        &mut self,
        queue_handle: &QueueHandle<Self>,
        source: InteractionSource,
    ) {
        if self.prime_launcher.is_some() {
            self.close_transient(ShellSurfaceKind::PrimeLauncher, queue_handle, source);
            return;
        }
        self.pending_power = None;
        self.quick_message = None;
        self.launcher_query.clear();
        self.selected_application = 0;
        match self.core.applications() {
            Ok(projection) => {
                self.applications = projection.applications;
                self.selected_application = self
                    .selected_application
                    .min(self.applications.len().saturating_sub(1));
                self.prime_launcher_message = if self.applications.is_empty() {
                    Some("NO ADMITTED APPLICATION PROFILES".to_owned())
                } else {
                    None
                };
            }
            Err(error) => {
                self.applications.clear();
                self.selected_application = 0;
                self.prime_launcher_message = Some(format!("CORE UNAVAILABLE: {error}"));
            }
        }
        self.prime_launcher = Some(self.create_overlay(
            queue_handle,
            PRIME_NAMESPACE,
            Anchor::TOP | Anchor::LEFT,
            (
                visual::PRIME_LAUNCHER_TOP_MARGIN,
                0,
                0,
                visual::PRIME_LAUNCHER_LEFT_MARGIN,
            ),
            visual::PRIME_LAUNCHER_WIDTH,
            visual::PRIME_LAUNCHER_HEIGHT,
        ));
        self.redraw_rail();
        match source {
            InteractionSource::Pointer => eprintln!("PRIME_SHELL_PRIME_OPEN=pointer"),
            InteractionSource::Keyboard => eprintln!("PRIME_SHELL_PRIME_OPEN=keyboard"),
        }
    }

    fn toggle_quick_controls(
        &mut self,
        queue_handle: &QueueHandle<Self>,
        source: InteractionSource,
    ) {
        if self.quick_controls.is_some() {
            self.close_transient(ShellSurfaceKind::QuickControls, queue_handle, source);
            return;
        }
        self.pending_power = None;
        self.quick_message = None;
        match self.core.system_status() {
            Ok(status) => {
                self.quick_lines = status.lines;
                self.quick_power_ready = status.power_ready;
            }
            Err(error) => {
                self.quick_lines = vec![format!("CORE STATUS UNAVAILABLE: {error}")];
                self.quick_power_ready = false;
            }
        }
        match self.core.storage_status() {
            Ok(storage_lines) => self.quick_lines.extend(storage_lines),
            Err(error) => self
                .quick_lines
                .push(format!("STORAGE UNAVAILABLE: {error}")),
        }
        self.quick_controls = Some(self.create_overlay(
            queue_handle,
            QUICK_CONTROLS_NAMESPACE,
            Anchor::TOP | Anchor::RIGHT,
            (94, 28, 0, 0),
            visual::QUICK_WIDTH,
            visual::QUICK_HEIGHT,
        ));
        self.redraw_rail();
        match source {
            InteractionSource::Pointer => {
                eprintln!("PRIME_SHELL_QUICK_CONTROLS_OPEN=pointer")
            }
            InteractionSource::Keyboard => {
                eprintln!("PRIME_SHELL_QUICK_CONTROLS_OPEN=keyboard")
            }
        }
    }

    fn filtered_applications(&self) -> Vec<prime_contracts::ApplicationEntry> {
        self.applications
            .iter()
            .filter(|entry| launcher_query_matches(&entry.display_name, &self.launcher_query))
            .cloned()
            .collect()
    }

    fn activate_selected_application(&mut self) {
        let visible = self.filtered_applications();
        let Some(entry) = visible.get(self.selected_application).cloned() else {
            self.prime_launcher_message = Some("NO APPLICATION SELECTED".to_owned());
            return;
        };
        if !entry.launch_ready {
            self.prime_launcher_message = Some(if entry.limitations.is_empty() {
                "APPLICATION IS NOT LAUNCH READY".to_owned()
            } else {
                entry.limitations.join(" | ")
            });
            return;
        }
        self.prime_launcher_message = Some(match self.core.launch(entry.application_id) {
            Ok(()) => format!("LAUNCH ACCEPTED: {}", entry.display_name),
            Err(error) => format!("LAUNCH DENIED: {error}"),
        });
    }

    fn move_application_selection(&mut self, delta: isize) {
        let visible_count = self.filtered_applications().len();
        if visible_count == 0 {
            self.selected_application = 0;
            return;
        }
        let last = visible_count - 1;
        self.selected_application = if delta < 0 {
            self.selected_application
                .saturating_sub(delta.unsigned_abs())
        } else {
            self.selected_application
                .saturating_add(delta as usize)
                .min(last)
        };
    }

    fn redraw_prime_launcher(&mut self, queue_handle: &QueueHandle<Self>) {
        let Some(prime_launcher) = self.prime_launcher.as_ref() else {
            return;
        };
        let layer = prime_launcher.layer.clone();
        let width = prime_launcher.width;
        let height = prime_launcher.height;
        let progress = prime_launcher.progress;
        if prime_launcher.transition.is_some() {
            layer
                .wl_surface()
                .frame(queue_handle, layer.wl_surface().clone());
        }
        let applications = self.filtered_applications();
        let theme = self.theme;
        let text = &mut self.text;
        if let Err(error) =
            draw_visual_surface(&mut self.pool, &layer, width, height, |bytes, w, h| {
                let mut canvas = visual::Canvas::new(bytes, w, h)
                    .expect("Prime launcher buffer must match configured dimensions");
                visual::paint_prime_launcher_surface(
                    &mut canvas,
                    text,
                    &theme,
                    visual::PrimeLauncherView {
                        applications: &applications,
                        selected: self.selected_application,
                        query: &self.launcher_query,
                        message: self.prime_launcher_message.as_deref(),
                        progress,
                    },
                );
            })
        {
            eprintln!("prime-shell could not redraw Prime launcher: {error}");
            self.exit = true;
        }
    }

    fn redraw_quick_controls(&mut self, queue_handle: &QueueHandle<Self>) {
        let Some(quick_controls) = self.quick_controls.as_ref() else {
            return;
        };
        let layer = quick_controls.layer.clone();
        let width = quick_controls.width;
        let height = quick_controls.height;
        let progress = quick_controls.progress;
        if quick_controls.transition.is_some() {
            layer
                .wl_surface()
                .frame(queue_handle, layer.wl_surface().clone());
        }
        let theme = self.theme;
        let text = &mut self.text;
        if let Err(error) =
            draw_visual_surface(&mut self.pool, &layer, width, height, |bytes, w, h| {
                let mut canvas = visual::Canvas::new(bytes, w, h)
                    .expect("Prime quick-controls buffer must match configured dimensions");
                visual::paint_quick_controls_surface(
                    &mut canvas,
                    text,
                    &theme,
                    visual::QuickControlsView {
                        lines: &self.quick_lines,
                        power_ready: self.quick_power_ready,
                        pending_power: self.pending_power,
                        message: self.quick_message.as_deref(),
                        progress,
                    },
                );
            })
        {
            eprintln!("prime-shell could not redraw quick controls: {error}");
            self.exit = true;
        }
    }

    fn stage_power_action(&mut self, action: SystemPowerAction) {
        if !self.quick_power_ready {
            self.pending_power = None;
            self.quick_message = Some("POWER CONTROL UNAVAILABLE".to_owned());
            return;
        }

        match power_confirmation(self.pending_power, action) {
            PowerConfirmation::Arm(action) => {
                self.pending_power = Some(action);
                self.quick_message = Some(format!(
                    "SELECT {} AGAIN TO CONFIRM",
                    power_action_name(action)
                ));
            }
            PowerConfirmation::Execute(action) => {
                self.pending_power = None;
                self.quick_message = Some(match self.core.power(action) {
                    Ok(()) => format!("{} ACCEPTED BY PRIME CORE", power_action_name(action)),
                    Err(error) => format!("{} DENIED: {error}", power_action_name(action)),
                });
            }
        }
    }

    fn close_transient(
        &mut self,
        kind: ShellSurfaceKind,
        queue_handle: &QueueHandle<Self>,
        source: InteractionSource,
    ) {
        let transition = motion::Transition::closing(Duration::from_millis(180), Instant::now());
        let armed = match kind {
            ShellSurfaceKind::PrimeLauncher => {
                self.prime_launcher.as_mut().is_some_and(|surface| {
                    surface.closing = true;
                    surface.transition = Some(transition);
                    true
                })
            }
            ShellSurfaceKind::QuickControls => {
                self.quick_controls.as_mut().is_some_and(|surface| {
                    surface.closing = true;
                    surface.transition = Some(transition);
                    true
                })
            }
            ShellSurfaceKind::Rail => false,
        };
        if !armed {
            return;
        }
        if kind == ShellSurfaceKind::QuickControls {
            self.pending_power = None;
            self.quick_message = None;
            self.redraw_quick_controls(queue_handle);
        } else {
            self.pending_power = None;
            self.quick_message = None;
            self.redraw_prime_launcher(queue_handle);
        }
        match source {
            InteractionSource::Pointer => eprintln!("PRIME_SHELL_TRANSIENT_CLOSE=pointer"),
            InteractionSource::Keyboard => eprintln!("PRIME_SHELL_TRANSIENT_CLOSE=keyboard"),
        }
    }

    fn configure_transient(
        &mut self,
        queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: &LayerSurfaceConfigure,
    ) -> bool {
        let filtered_applications = self.filtered_applications();
        if let Some(prime_launcher) = self.prime_launcher.as_mut() {
            if layer.wl_surface() == prime_launcher.layer.wl_surface() {
                let width = NonZeroU32::new(configure.new_size.0)
                    .map(NonZeroU32::get)
                    .unwrap_or(prime_launcher.width);
                let height = NonZeroU32::new(configure.new_size.1)
                    .map(NonZeroU32::get)
                    .unwrap_or(prime_launcher.height);
                prime_launcher.width = width;
                prime_launcher.height = height;
                let progress = prime_launcher
                    .transition
                    .map_or(prime_launcher.progress, |transition| {
                        transition.sample_at(Instant::now())
                    });
                prime_launcher.progress = progress;
                if prime_launcher.transition.is_some() {
                    prime_launcher
                        .layer
                        .wl_surface()
                        .frame(queue_handle, prime_launcher.layer.wl_surface().clone());
                }
                let theme = self.theme;
                let text = &mut self.text;
                if let Err(error) = draw_visual_surface(
                    &mut self.pool,
                    &prime_launcher.layer,
                    width,
                    height,
                    |bytes, w, h| {
                        let mut canvas = visual::Canvas::new(bytes, w, h)
                            .expect("Prime launcher buffer must match configured dimensions");
                        visual::paint_prime_launcher_surface(
                            &mut canvas,
                            text,
                            &theme,
                            visual::PrimeLauncherView {
                                applications: &filtered_applications,
                                selected: self.selected_application,
                                query: &self.launcher_query,
                                message: self.prime_launcher_message.as_deref(),
                                progress,
                            },
                        );
                    },
                ) {
                    eprintln!("prime-shell could not draw Prime launcher overlay: {error}");
                    self.exit = true;
                }
                return true;
            }
        }

        if let Some(quick_controls) = self.quick_controls.as_mut() {
            if layer.wl_surface() == quick_controls.layer.wl_surface() {
                let width = NonZeroU32::new(configure.new_size.0)
                    .map(NonZeroU32::get)
                    .unwrap_or(quick_controls.width);
                let height = NonZeroU32::new(configure.new_size.1)
                    .map(NonZeroU32::get)
                    .unwrap_or(quick_controls.height);
                quick_controls.width = width;
                quick_controls.height = height;
                let progress = quick_controls
                    .transition
                    .map_or(quick_controls.progress, |transition| {
                        transition.sample_at(Instant::now())
                    });
                quick_controls.progress = progress;
                if quick_controls.transition.is_some() {
                    quick_controls
                        .layer
                        .wl_surface()
                        .frame(queue_handle, quick_controls.layer.wl_surface().clone());
                }
                let theme = self.theme;
                let text = &mut self.text;
                if let Err(error) = draw_visual_surface(
                    &mut self.pool,
                    &quick_controls.layer,
                    width,
                    height,
                    |bytes, w, h| {
                        let mut canvas = visual::Canvas::new(bytes, w, h)
                            .expect("Prime quick-controls buffer must match configured dimensions");
                        visual::paint_quick_controls_surface(
                            &mut canvas,
                            text,
                            &theme,
                            visual::QuickControlsView {
                                lines: &self.quick_lines,
                                power_ready: self.quick_power_ready,
                                pending_power: self.pending_power,
                                message: self.quick_message.as_deref(),
                                progress,
                            },
                        )
                    },
                ) {
                    eprintln!("prime-shell could not draw quick-controls overlay: {error}");
                    self.exit = true;
                }
                return true;
            }
        }

        false
    }
}

impl CompositorHandler for PrimeShell {
    fn scale_factor_changed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_factor: i32,
    ) {
    }

    fn transform_changed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _new_transform: wl_output::Transform,
    ) {
    }

    fn frame(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        let now = Instant::now();
        if surface == self.background.wl_surface() {
            self.redraw_background_motion(now);
            return;
        }

        if self
            .prime_launcher
            .as_ref()
            .is_some_and(|overlay| surface == overlay.layer.wl_surface())
        {
            let mut should_close = false;
            if let Some(overlay) = self.prime_launcher.as_mut() {
                if let Some(transition) = overlay.transition {
                    overlay.progress = transition.sample_at(now);
                    if transition.is_complete_at(now) {
                        overlay.progress = if overlay.closing { 0.0 } else { 1.0 };
                        overlay.transition = None;
                        should_close = overlay.closing;
                    }
                }
            }
            if should_close {
                self.prime_launcher.take();
                self.keyboard_focus = None;
                self.redraw_rail();
            } else {
                self.redraw_prime_launcher(queue_handle);
            }
            return;
        }

        if self
            .quick_controls
            .as_ref()
            .is_some_and(|overlay| surface == overlay.layer.wl_surface())
        {
            let mut should_close = false;
            if let Some(overlay) = self.quick_controls.as_mut() {
                if let Some(transition) = overlay.transition {
                    overlay.progress = transition.sample_at(now);
                    if transition.is_complete_at(now) {
                        overlay.progress = if overlay.closing { 0.0 } else { 1.0 };
                        overlay.transition = None;
                        should_close = overlay.closing;
                    }
                }
            }
            if should_close {
                self.quick_controls.take();
                self.pending_power = None;
                self.quick_message = None;
                self.keyboard_focus = None;
                self.redraw_rail();
            } else {
                self.redraw_quick_controls(queue_handle);
            }
        }
    }

    fn surface_enter(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }

    fn surface_leave(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _output: &wl_output::WlOutput,
    ) {
    }
}

impl OutputHandler for PrimeShell {
    fn output_state(&mut self) -> &mut OutputState {
        &mut self.output_state
    }

    fn new_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn update_output(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }

    fn output_destroyed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _output: wl_output::WlOutput,
    ) {
    }
}

impl LayerShellHandler for PrimeShell {
    fn closed(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
    ) {
        if layer.wl_surface() == self.background.wl_surface() {
            eprintln!("prime-shell persistent background surface closed");
            self.exit = true;
        } else if layer.wl_surface() == self.rail.wl_surface() {
            eprintln!("prime-shell persistent rail surface closed");
            self.exit = true;
        } else if self
            .prime_launcher
            .as_ref()
            .is_some_and(|overlay| layer.wl_surface() == overlay.layer.wl_surface())
        {
            self.prime_launcher.take();
            self.keyboard_focus = None;
            eprintln!("PRIME_SHELL_PRIME_CLOSED=compositor");
        } else if self
            .quick_controls
            .as_ref()
            .is_some_and(|overlay| layer.wl_surface() == overlay.layer.wl_surface())
        {
            self.quick_controls.take();
            self.keyboard_focus = None;
            eprintln!("PRIME_SHELL_QUICK_CONTROLS_CLOSED=compositor");
        }
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.configure_transient(queue_handle, layer, &configure) {
            return;
        }

        let Some(width) = NonZeroU32::new(configure.new_size.0).map(NonZeroU32::get) else {
            eprintln!("prime-shell received no truthful persistent-surface width");
            self.exit = true;
            return;
        };

        if layer.wl_surface() == self.background.wl_surface() {
            let Some(height) = NonZeroU32::new(configure.new_size.1).map(NonZeroU32::get) else {
                eprintln!("prime-shell received no truthful background height");
                self.exit = true;
                return;
            };
            let status = if self.core.system_status().is_ok() {
                visual::TopStatus::Online
            } else {
                visual::TopStatus::Limited
            };
            let theme = self.theme;
            let mut static_bytes = vec![0u8; width as usize * height as usize * 4];
            {
                let mut canvas = visual::Canvas::new(&mut static_bytes, width, height)
                    .expect("Prime background cache must match configured dimensions");
                if let Some(wallpaper) = self.system_wallpaper.as_ref() {
                    visual::wallpaper::paint_system_wallpaper(&mut canvas, wallpaper);
                } else {
                    visual::paint_background_base(&mut canvas, &theme);
                }
                visual::paint_top_status_strip(&mut canvas, &mut self.text, &theme, status);
            }

            if let Err(error) =
                draw_visual_surface(&mut self.pool, layer, width, height, |bytes, w, h| {
                    debug_assert_eq!(bytes.len(), static_bytes.len());
                    bytes.copy_from_slice(&static_bytes);
                    if self.wallpaper_selection == visual::wallpaper::WallpaperSelection::Animated {
                        let mut canvas = visual::Canvas::new(bytes, w, h)
                            .expect("Prime background buffer must match configured dimensions");
                        visual::paint_background_motion(&mut canvas, &theme, 0.0);
                    }
                })
            {
                eprintln!("prime-shell could not draw background: {error}");
                self.exit = true;
                return;
            }
            self.background_width = width;
            self.background_height = height;
            self.background_static = static_bytes;
            self.background_started = Instant::now();
            self.background_last_render = Instant::now();
            self.background_motion_frames = 0;
            self.background_configured = true;
        } else if layer.wl_surface() == self.rail.wl_surface() {
            let height = NonZeroU32::new(configure.new_size.1)
                .map(NonZeroU32::get)
                .unwrap_or(self.rail_height);
            let theme = self.theme;
            let active = if self.prime_launcher.is_some() {
                Some(visual::RailAction::Prime)
            } else if self.quick_controls.is_some() {
                Some(visual::RailAction::Status)
            } else {
                None
            };
            let text = &mut self.text;
            let rail_actions = &self.rail_actions;
            if let Err(error) =
                draw_visual_surface(&mut self.pool, layer, width, height, |bytes, w, h| {
                    let mut canvas = visual::Canvas::new(bytes, w, h)
                        .expect("Prime rail buffer must match configured dimensions");
                    visual::paint_rail_surface(&mut canvas, text, &theme, rail_actions, active);
                })
            {
                eprintln!("prime-shell could not draw rail: {error}");
                self.exit = true;
                return;
            }
            self.rail_width = width;
            self.rail_configured = true;
        } else if layer.wl_surface() == self.status_cluster.wl_surface() {
            let height = NonZeroU32::new(configure.new_size.1)
                .map(NonZeroU32::get)
                .unwrap_or(visual::STATUS_CLUSTER_HEIGHT);
            let status = if self.core.system_status().is_ok() {
                visual::TopStatus::Online
            } else {
                visual::TopStatus::Limited
            };
            let theme = self.theme;
            let text = &mut self.text;
            if let Err(error) =
                draw_visual_surface(&mut self.pool, layer, width, height, |bytes, w, h| {
                    let mut canvas = visual::Canvas::new(bytes, w, h)
                        .expect("Prime status buffer must match configured dimensions");
                    visual::paint_status_cluster(&mut canvas, text, &theme, status);
                })
            {
                eprintln!("prime-shell could not draw status cluster: {error}");
                self.exit = true;
                return;
            }
            self.status_configured = true;
        } else {
            eprintln!("prime-shell received configure for an unknown layer surface");
            self.exit = true;
            return;
        }

        self.report_baseline_once();
    }
}

impl SeatHandler for PrimeShell {
    fn seat_state(&mut self) -> &mut SeatState {
        &mut self.seat_state
    }

    fn new_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }

    fn new_capability(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard && self.keyboard.is_none() {
            match self.seat_state.get_keyboard(queue_handle, &seat, None) {
                Ok(keyboard) => self.keyboard = Some(keyboard),
                Err(error) => {
                    eprintln!("prime-shell could not acquire keyboard capability: {error}");
                    self.exit = true;
                }
            }
        }
        if capability == Capability::Pointer && self.pointer.is_none() {
            match self.seat_state.get_pointer(queue_handle, &seat) {
                Ok(pointer) => self.pointer = Some(pointer),
                Err(error) => {
                    eprintln!("prime-shell could not acquire pointer capability: {error}");
                    self.exit = true;
                }
            }
        }
    }

    fn remove_capability(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
        capability: Capability,
    ) {
        if capability == Capability::Keyboard {
            if let Some(keyboard) = self.keyboard.take() {
                keyboard.release();
            }
            self.keyboard_focus = None;
        }
        if capability == Capability::Pointer {
            if let Some(pointer) = self.pointer.take() {
                pointer.release();
            }
        }
    }

    fn remove_seat(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _seat: wl_seat::WlSeat,
    ) {
    }
}

impl KeyboardHandler for PrimeShell {
    fn enter(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
        _raw: &[u32],
        _keysyms: &[Keysym],
    ) {
        self.keyboard_focus = self.surface_kind(surface);
    }

    fn leave(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        surface: &wl_surface::WlSurface,
        _serial: u32,
    ) {
        if self.surface_kind(surface) == self.keyboard_focus {
            self.keyboard_focus = None;
        }
    }

    fn press_key(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        event: KeyEvent,
    ) {
        if event.keysym == Keysym::Escape {
            if matches!(self.keyboard_focus, Some(ShellSurfaceKind::PrimeLauncher)) {
                self.close_transient(
                    ShellSurfaceKind::PrimeLauncher,
                    queue_handle,
                    InteractionSource::Keyboard,
                );
            } else if matches!(self.keyboard_focus, Some(ShellSurfaceKind::QuickControls)) {
                self.close_transient(
                    ShellSurfaceKind::QuickControls,
                    queue_handle,
                    InteractionSource::Keyboard,
                );
            }
            return;
        }

        if self.keyboard_focus == Some(ShellSurfaceKind::PrimeLauncher) {
            if event.keysym == Keysym::BackSpace {
                self.launcher_query.pop();
                self.selected_application = 0;
                self.prime_launcher_message = None;
                self.redraw_prime_launcher(queue_handle);
                return;
            }
            if event.keysym == Keysym::Up {
                self.move_application_selection(-1);
                self.redraw_prime_launcher(queue_handle);
                return;
            }
            if event.keysym == Keysym::Down {
                self.move_application_selection(1);
                self.redraw_prime_launcher(queue_handle);
                return;
            }
            if event.keysym == Keysym::Return {
                self.activate_selected_application();
                self.redraw_prime_launcher(queue_handle);
                return;
            }
            if let Some(character) = event.keysym.key_char() {
                if !character.is_control() && self.launcher_query.chars().count() < 64 {
                    self.launcher_query.push(character);
                    self.selected_application = 0;
                    self.prime_launcher_message = None;
                    self.redraw_prime_launcher(queue_handle);
                }
                return;
            }
        }

        let Some(character) = event.keysym.key_char() else {
            return;
        };
        if self.keyboard_focus == Some(ShellSurfaceKind::QuickControls) {
            let action = match character.to_ascii_lowercase() {
                'r' => Some(SystemPowerAction::Reboot),
                'p' => Some(SystemPowerAction::PowerOff),
                _ => None,
            };
            if let Some(action) = action {
                self.stage_power_action(action);
                self.redraw_quick_controls(queue_handle);
            }
            return;
        }
        if self.keyboard_focus == Some(ShellSurfaceKind::Rail) {
            match character.to_ascii_lowercase() {
                'o' => self.toggle_prime_launcher(queue_handle, InteractionSource::Keyboard),
                'q' => self.toggle_quick_controls(queue_handle, InteractionSource::Keyboard),
                _ => {}
            }
        }
    }

    fn repeat_key(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn release_key(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _event: KeyEvent,
    ) {
    }

    fn update_modifiers(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _keyboard: &wl_keyboard::WlKeyboard,
        _serial: u32,
        _modifiers: Modifiers,
        _raw_modifiers: RawModifiers,
        _layout: u32,
    ) {
    }
}

impl PointerHandler for PrimeShell {
    fn pointer_frame(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        _pointer: &wl_pointer::WlPointer,
        events: &[PointerEvent],
    ) {
        for event in events {
            let PointerEventKind::Press { button, .. } = event.kind else {
                continue;
            };
            if button != BTN_LEFT {
                continue;
            }

            if &event.surface == self.status_cluster.wl_surface() {
                let layout = visual::StatusClusterLayout::for_surface(
                    visual::STATUS_CLUSTER_WIDTH,
                    visual::STATUS_CLUSTER_HEIGHT,
                );
                if layout.hit(event.position.0, event.position.1) {
                    self.toggle_quick_controls(queue_handle, InteractionSource::Pointer);
                }
            } else if &event.surface == self.rail.wl_surface() {
                let layout = visual::RailLayout::for_surface(
                    self.rail_width.max(visual::RAIL_WIDTH),
                    self.rail_height,
                    self.rail_actions.len(),
                );
                match layout.hit(event.position.0, event.position.1, &self.rail_actions) {
                    Some(visual::RailAction::Prime) => {
                        self.toggle_prime_launcher(queue_handle, InteractionSource::Pointer);
                    }
                    Some(
                        visual::RailAction::Status
                        | visual::RailAction::Network
                        | visual::RailAction::Audio
                        | visual::RailAction::Storage
                        | visual::RailAction::Health,
                    ) => {
                        self.toggle_quick_controls(queue_handle, InteractionSource::Pointer);
                    }
                    Some(visual::RailAction::Application(application_id)) => {
                        match self.core.applications() {
                            Ok(projection) => {
                                if let Some(entry) = projection.applications.into_iter().find(|entry| {
                                    entry.application_id.to_string() == application_id
                                }) {
                                    if entry.launch_ready {
                                        match self.core.launch(entry.application_id) {
                                            Ok(()) => eprintln!(
                                                "PRIME_SHELL_RAIL_APPLICATION_LAUNCH=accepted;application_id={application_id}"
                                            ),
                                            Err(error) => eprintln!(
                                                "PRIME_SHELL_RAIL_APPLICATION_LAUNCH=denied;application_id={application_id};error={error}"
                                            ),
                                        }
                                    } else {
                                        eprintln!(
                                            "PRIME_SHELL_RAIL_APPLICATION_LAUNCH=not_ready;application_id={application_id}"
                                        );
                                    }
                                } else {
                                    eprintln!(
                                        "PRIME_SHELL_RAIL_APPLICATION_LAUNCH=missing;application_id={application_id}"
                                    );
                                }
                            }
                            Err(error) => eprintln!(
                                "PRIME_SHELL_RAIL_APPLICATION_LAUNCH=core_unavailable;application_id={application_id};error={error}"
                            ),
                        }
                    }
                    None => {}
                }
            } else if self
                .prime_launcher
                .as_ref()
                .is_some_and(|prime_launcher| &event.surface == prime_launcher.layer.wl_surface())
            {
                let visible_count = self.filtered_applications().len();
                let application = self.prime_launcher.as_ref().and_then(|prime_launcher| {
                    let layout = visual::PrimeLauncherLayout::new(
                        prime_launcher.width,
                        prime_launcher.height,
                    );
                    layout.application_at(event.position.0, event.position.1, visible_count)
                });
                if let Some(index) = application {
                    self.selected_application = index;
                    self.activate_selected_application();
                    self.redraw_prime_launcher(queue_handle);
                }
            } else if self
                .quick_controls
                .as_ref()
                .is_some_and(|quick_controls| &event.surface == quick_controls.layer.wl_surface())
            {
                let layout = self
                    .quick_controls
                    .as_ref()
                    .map(|quick_controls| {
                        visual::QuickControlsLayout::new(
                            quick_controls.width,
                            quick_controls.height,
                        )
                    })
                    .expect("quick controls exists for matched surface");
                if layout.collapse_at(event.position.0, event.position.1) {
                    self.close_transient(
                        ShellSurfaceKind::QuickControls,
                        queue_handle,
                        InteractionSource::Pointer,
                    );
                    continue;
                }
                if let Some(action) = layout.power_action_at(event.position.0, event.position.1) {
                    self.stage_power_action(action);
                    self.redraw_quick_controls(queue_handle);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn launcher_search_is_case_insensitive_and_empty_query_matches_all() {
        assert!(launcher_query_matches("Files", "fi"));
        assert!(launcher_query_matches("Terminal", "MIN"));
        assert!(launcher_query_matches("Prime Settings", ""));
        assert!(!launcher_query_matches("Browser", "terminal"));
    }

    #[test]
    fn ease_out_cubic_is_monotonic_and_bounded() {
        assert_eq!(motion::ease_out_cubic(0.0), 0.0);
        assert_eq!(motion::ease_out_cubic(1.0), 1.0);
        assert!(motion::ease_out_cubic(0.5) > 0.5);
    }

    #[test]
    fn closing_transition_reaches_closed_state() {
        use std::time::{Duration, Instant};
        let now = Instant::now();
        let transition = motion::Transition::closing(Duration::from_millis(200), now);
        assert_eq!(transition.sample_at(now + Duration::from_millis(200)), 0.0);
    }

    #[test]
    fn persistent_baseline_requires_background_rail_and_status_cluster() {
        assert!(!persistent_baseline_ready(true, true, false));
        assert!(!persistent_baseline_ready(true, false, true));
        assert!(persistent_baseline_ready(true, true, true));
    }

    #[test]
    fn wayland_read_would_block_is_retryable() {
        let error =
            wayland_client::backend::WaylandError::Io(io::Error::from(io::ErrorKind::WouldBlock));
        assert!(finish_wayland_read(Err(error)).is_ok());
    }

    #[test]
    fn wayland_read_other_io_errors_remain_fatal() {
        let error = wayland_client::backend::WaylandError::Io(io::Error::other("socket failed"));
        assert!(finish_wayland_read(Err(error)).is_err());
    }

    #[test]
    fn power_confirmation_requires_same_action_twice() {
        assert_eq!(
            power_confirmation(None, SystemPowerAction::Reboot),
            PowerConfirmation::Arm(SystemPowerAction::Reboot)
        );
        assert_eq!(
            power_confirmation(Some(SystemPowerAction::Reboot), SystemPowerAction::Reboot),
            PowerConfirmation::Execute(SystemPowerAction::Reboot)
        );
        assert_eq!(
            power_confirmation(Some(SystemPowerAction::Reboot), SystemPowerAction::PowerOff),
            PowerConfirmation::Arm(SystemPowerAction::PowerOff)
        );
    }
}

impl ShmHandler for PrimeShell {
    fn shm_state(&mut self) -> &mut Shm {
        &mut self.shm
    }
}

delegate_compositor!(PrimeShell);
delegate_output!(PrimeShell);
delegate_shm!(PrimeShell);
delegate_seat!(PrimeShell);
delegate_keyboard!(PrimeShell);
delegate_pointer!(PrimeShell);
delegate_layer!(PrimeShell);
delegate_registry!(PrimeShell);

impl ProvidesRegistryState for PrimeShell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState, SeatState];
}
