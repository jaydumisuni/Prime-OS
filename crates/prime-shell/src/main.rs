mod core_client;
mod visual;

use std::{error::Error, io, num::NonZeroU32};

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
const ORB_NAMESPACE: &str = "prime.shell.orb";
const QUICK_CONTROLS_NAMESPACE: &str = "prime.shell.quick-controls";

const RAIL_HEIGHT: u32 = 48;
const ORB_WIDTH: u32 = 360;
const ORB_HEIGHT: u32 = 420;
const QUICK_CONTROLS_WIDTH: u32 = 320;
const QUICK_CONTROLS_HEIGHT: u32 = 560;
const RAIL_TRIGGER_WIDTH: f64 = 96.0;

const BACKGROUND_ARGB: u32 = 0xff11141a;
const RAIL_ARGB: u32 = 0xff1b2028;

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!("prime-shell — Prime P1 Shell interaction construction host");
        println!("Usage: prime-shell");
        println!("Persistent baseline: background + rail");
        println!("Functional surfaces: Orb applications + truthful quick controls");
        return Ok(());
    }

    let connection = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init(&connection)?;
    let queue_handle = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &queue_handle)?;
    let layer_shell = LayerShell::bind(&globals, &queue_handle)?;
    let shm = Shm::bind(&globals, &queue_handle)?;
    let pool = SlotPool::new(4, &shm)?;

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
    rail.set_anchor(Anchor::TOP | Anchor::LEFT | Anchor::RIGHT);
    rail.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
    rail.set_exclusive_zone(i32::try_from(RAIL_HEIGHT)?);
    rail.set_size(0, RAIL_HEIGHT);
    rail.commit();

    let mut shell = PrimeShell {
        registry_state: RegistryState::new(&globals),
        seat_state: SeatState::new(&globals, &queue_handle),
        output_state: OutputState::new(&globals, &queue_handle),
        compositor,
        layer_shell,
        shm,
        pool,
        background,
        rail,
        rail_width: 0,
        background_configured: false,
        rail_configured: false,
        baseline_reported: false,
        orb: None,
        quick_controls: None,
        keyboard: None,
        pointer: None,
        keyboard_focus: None,
        core: core_client::CoreClient::from_env(),
        applications: Vec::new(),
        selected_application: 0,
        orb_message: None,
        quick_lines: Vec::new(),
        quick_power_ready: false,
        pending_power: None,
        quick_message: None,
        exit: false,
    };

    eprintln!("PRIME_SHELL_CORE_BRIDGE=typed_application_id;quick_controls=truthful_status;power=typed_double_confirm");

    while !shell.exit {
        event_queue.blocking_dispatch(&mut shell)?;
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ShellSurfaceKind {
    Rail,
    Orb,
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

struct TransientSurface {
    layer: LayerSurface,
    width: u32,
    height: u32,
}

struct PrimeShell {
    registry_state: RegistryState,
    seat_state: SeatState,
    output_state: OutputState,
    compositor: CompositorState,
    layer_shell: LayerShell,
    shm: Shm,
    pool: SlotPool,
    background: LayerSurface,
    rail: LayerSurface,
    rail_width: u32,
    background_configured: bool,
    rail_configured: bool,
    baseline_reported: bool,
    orb: Option<TransientSurface>,
    quick_controls: Option<TransientSurface>,
    keyboard: Option<wl_keyboard::WlKeyboard>,
    pointer: Option<wl_pointer::WlPointer>,
    keyboard_focus: Option<ShellSurfaceKind>,
    core: core_client::CoreClient,
    applications: Vec<prime_contracts::ApplicationEntry>,
    selected_application: usize,
    orb_message: Option<String>,
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

impl PrimeShell {
    fn report_baseline_once(&mut self) {
        if self.background_configured && self.rail_configured && !self.baseline_reported {
            self.baseline_reported = true;
            eprintln!(
                "PRIME_SHELL_PERSISTENT_BASELINE_CONFIGURED=background,rail;readiness_unearned"
            );
        }
    }

    fn surface_kind(&self, surface: &wl_surface::WlSurface) -> Option<ShellSurfaceKind> {
        if surface == self.rail.wl_surface() {
            Some(ShellSurfaceKind::Rail)
        } else if self
            .orb
            .as_ref()
            .is_some_and(|overlay| surface == overlay.layer.wl_surface())
        {
            Some(ShellSurfaceKind::Orb)
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
        layer.set_keyboard_interactivity(KeyboardInteractivity::OnDemand);
        layer.set_exclusive_zone(0);
        layer.set_size(width, height);
        layer.commit();
        TransientSurface {
            layer,
            width,
            height,
        }
    }

    fn toggle_orb(&mut self, queue_handle: &QueueHandle<Self>, source: InteractionSource) {
        if self.orb.is_some() {
            self.close_transient(ShellSurfaceKind::Orb, source);
            return;
        }
        match self.core.applications() {
            Ok(projection) => {
                self.applications = projection.applications;
                self.selected_application = self
                    .selected_application
                    .min(self.applications.len().saturating_sub(1));
                self.orb_message = if self.applications.is_empty() {
                    Some("NO ADMITTED APPLICATION PROFILES".to_owned())
                } else {
                    None
                };
            }
            Err(error) => {
                self.applications.clear();
                self.selected_application = 0;
                self.orb_message = Some(format!("CORE UNAVAILABLE: {error}"));
            }
        }
        self.orb = Some(self.create_overlay(
            queue_handle,
            ORB_NAMESPACE,
            Anchor::BOTTOM,
            ORB_WIDTH,
            ORB_HEIGHT,
        ));
        match source {
            InteractionSource::Pointer => eprintln!("PRIME_SHELL_ORB_OPEN=pointer"),
            InteractionSource::Keyboard => eprintln!("PRIME_SHELL_ORB_OPEN=keyboard"),
        }
    }

    fn toggle_quick_controls(
        &mut self,
        queue_handle: &QueueHandle<Self>,
        source: InteractionSource,
    ) {
        if self.quick_controls.is_some() {
            self.close_transient(ShellSurfaceKind::QuickControls, source);
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
            QUICK_CONTROLS_WIDTH,
            QUICK_CONTROLS_HEIGHT,
        ));
        match source {
            InteractionSource::Pointer => {
                eprintln!("PRIME_SHELL_QUICK_CONTROLS_OPEN=pointer")
            }
            InteractionSource::Keyboard => {
                eprintln!("PRIME_SHELL_QUICK_CONTROLS_OPEN=keyboard")
            }
        }
    }

    fn activate_selected_application(&mut self) {
        let Some(entry) = self.applications.get(self.selected_application).cloned() else {
            self.orb_message = Some("NO APPLICATION SELECTED".to_owned());
            return;
        };
        if !entry.launch_ready {
            self.orb_message = Some(if entry.limitations.is_empty() {
                "APPLICATION IS NOT LAUNCH READY".to_owned()
            } else {
                entry.limitations.join(" | ")
            });
            return;
        }
        self.orb_message = Some(match self.core.launch(entry.application_id) {
            Ok(()) => format!("LAUNCH ACCEPTED: {}", entry.display_name),
            Err(error) => format!("LAUNCH DENIED: {error}"),
        });
    }

    fn move_application_selection(&mut self, delta: isize) {
        if self.applications.is_empty() {
            self.selected_application = 0;
            return;
        }
        let last = self.applications.len() - 1;
        self.selected_application = if delta < 0 {
            self.selected_application
                .saturating_sub(delta.unsigned_abs())
        } else {
            self.selected_application
                .saturating_add(delta as usize)
                .min(last)
        };
    }

    fn redraw_orb(&mut self) {
        let Some(orb) = self.orb.as_ref() else {
            return;
        };
        let layer = orb.layer.clone();
        let width = orb.width;
        let height = orb.height;
        if let Err(error) =
            draw_visual_surface(&mut self.pool, &layer, width, height, |canvas, w, h| {
                visual::paint_orb(
                    canvas,
                    w,
                    h,
                    &self.applications,
                    self.selected_application,
                    self.orb_message.as_deref(),
                );
            })
        {
            eprintln!("prime-shell could not redraw Orb: {error}");
            self.exit = true;
        }
    }

    fn redraw_quick_controls(&mut self) {
        let Some(quick_controls) = self.quick_controls.as_ref() else {
            return;
        };
        let layer = quick_controls.layer.clone();
        let width = quick_controls.width;
        let height = quick_controls.height;
        if let Err(error) =
            draw_visual_surface(&mut self.pool, &layer, width, height, |canvas, w, h| {
                visual::paint_quick_controls(
                    canvas,
                    w,
                    h,
                    &self.quick_lines,
                    self.quick_power_ready,
                    self.pending_power,
                    self.quick_message.as_deref(),
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
                    "PRESS {} AGAIN TO CONFIRM {}",
                    match action {
                        SystemPowerAction::Reboot => "R",
                        SystemPowerAction::PowerOff => "P",
                    },
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

    fn close_transient(&mut self, kind: ShellSurfaceKind, source: InteractionSource) {
        let closed = match kind {
            ShellSurfaceKind::Orb => self.orb.take().is_some(),
            ShellSurfaceKind::QuickControls => self.quick_controls.take().is_some(),
            ShellSurfaceKind::Rail => false,
        };
        if closed {
            if kind == ShellSurfaceKind::QuickControls {
                self.pending_power = None;
                self.quick_message = None;
            }
            match source {
                InteractionSource::Pointer => eprintln!("PRIME_SHELL_TRANSIENT_CLOSE=pointer"),
                InteractionSource::Keyboard => eprintln!("PRIME_SHELL_TRANSIENT_CLOSE=keyboard"),
            }
            self.keyboard_focus = None;
        }
    }

    fn configure_transient(
        &mut self,
        layer: &LayerSurface,
        configure: &LayerSurfaceConfigure,
    ) -> bool {
        if let Some(orb) = self.orb.as_mut() {
            if layer.wl_surface() == orb.layer.wl_surface() {
                let width = NonZeroU32::new(configure.new_size.0)
                    .map(NonZeroU32::get)
                    .unwrap_or(orb.width);
                let height = NonZeroU32::new(configure.new_size.1)
                    .map(NonZeroU32::get)
                    .unwrap_or(orb.height);
                orb.width = width;
                orb.height = height;
                if let Err(error) = draw_visual_surface(
                    &mut self.pool,
                    &orb.layer,
                    width,
                    height,
                    |canvas, w, h| {
                        visual::paint_orb(
                            canvas,
                            w,
                            h,
                            &self.applications,
                            self.selected_application,
                            self.orb_message.as_deref(),
                        );
                    },
                ) {
                    eprintln!("prime-shell could not draw Orb overlay: {error}");
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
                if let Err(error) = draw_visual_surface(
                    &mut self.pool,
                    &quick_controls.layer,
                    width,
                    height,
                    |canvas, w, h| {
                        visual::paint_quick_controls(
                            canvas,
                            w,
                            h,
                            &self.quick_lines,
                            self.quick_power_ready,
                            self.pending_power,
                            self.quick_message.as_deref(),
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
        _queue_handle: &QueueHandle<Self>,
        _surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
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
            .orb
            .as_ref()
            .is_some_and(|overlay| layer.wl_surface() == overlay.layer.wl_surface())
        {
            self.orb.take();
            self.keyboard_focus = None;
            eprintln!("PRIME_SHELL_ORB_CLOSED=compositor");
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
        _queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        if self.configure_transient(layer, &configure) {
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
            if let Err(error) =
                draw_visual_surface(&mut self.pool, layer, width, height, |canvas, w, h| {
                    visual::paint_background(canvas, w, h, BACKGROUND_ARGB);
                })
            {
                eprintln!("prime-shell could not draw background: {error}");
                self.exit = true;
                return;
            }
            self.background_configured = true;
        } else if layer.wl_surface() == self.rail.wl_surface() {
            let height = NonZeroU32::new(configure.new_size.1)
                .map(NonZeroU32::get)
                .unwrap_or(RAIL_HEIGHT);
            if let Err(error) =
                draw_visual_surface(&mut self.pool, layer, width, height, |canvas, w, h| {
                    visual::paint_rail(canvas, w, h, RAIL_ARGB);
                })
            {
                eprintln!("prime-shell could not draw rail: {error}");
                self.exit = true;
                return;
            }
            self.rail_width = width;
            self.rail_configured = true;
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
            if matches!(self.keyboard_focus, Some(ShellSurfaceKind::Orb)) {
                self.close_transient(ShellSurfaceKind::Orb, InteractionSource::Keyboard);
            } else if matches!(self.keyboard_focus, Some(ShellSurfaceKind::QuickControls)) {
                self.close_transient(ShellSurfaceKind::QuickControls, InteractionSource::Keyboard);
            }
            return;
        }

        if self.keyboard_focus == Some(ShellSurfaceKind::Orb) {
            if event.keysym == Keysym::Up {
                self.move_application_selection(-1);
                self.redraw_orb();
                return;
            }
            if event.keysym == Keysym::Down {
                self.move_application_selection(1);
                self.redraw_orb();
                return;
            }
            if event.keysym == Keysym::Return {
                self.activate_selected_application();
                self.redraw_orb();
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
                self.redraw_quick_controls();
            }
            return;
        }
        if self.keyboard_focus == Some(ShellSurfaceKind::Rail) {
            match character.to_ascii_lowercase() {
                'o' => self.toggle_orb(queue_handle, InteractionSource::Keyboard),
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

            if &event.surface == self.rail.wl_surface() {
                if event.position.0 <= RAIL_TRIGGER_WIDTH {
                    self.toggle_orb(queue_handle, InteractionSource::Pointer);
                } else if self.rail_width > 0
                    && event.position.0 >= f64::from(self.rail_width) - RAIL_TRIGGER_WIDTH
                {
                    self.toggle_quick_controls(queue_handle, InteractionSource::Pointer);
                }
            } else if self
                .orb
                .as_ref()
                .is_some_and(|orb| &event.surface == orb.layer.wl_surface())
            {
                if let Some(index) = visual::orb_row_at(event.position.1, self.applications.len()) {
                    self.selected_application = index;
                    self.activate_selected_application();
                    self.redraw_orb();
                }
            } else {
                let action = self
                    .quick_controls
                    .as_ref()
                    .and_then(|quick_controls| {
                        (&event.surface == quick_controls.layer.wl_surface()).then(|| {
                            visual::quick_power_action_at(event.position.1, quick_controls.height)
                        })
                    })
                    .flatten();
                if let Some(action) = action {
                    self.stage_power_action(action);
                    self.redraw_quick_controls();
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
