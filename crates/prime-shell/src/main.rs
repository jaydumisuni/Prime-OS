use std::{error::Error, io, num::NonZeroU32};

use smithay_client_toolkit::{
    compositor::{CompositorHandler, CompositorState},
    delegate_compositor, delegate_layer, delegate_output, delegate_registry, delegate_shm,
    output::{OutputHandler, OutputState},
    registry::{ProvidesRegistryState, RegistryState},
    registry_handlers,
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
    protocol::{wl_output, wl_shm, wl_surface},
    Connection, QueueHandle,
};

const BACKGROUND_NAMESPACE: &str = "prime.shell.background";
const RAIL_NAMESPACE: &str = "prime.shell.rail";
const RAIL_HEIGHT: u32 = 44;
const PRIME_BACKGROUND_ARGB: u32 = 0xff11141a;
const PRIME_RAIL_ARGB: u32 = 0xff1a1e27;

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!("prime-shell — Prime P1 native Shell host construction baseline");
        println!("Usage: prime-shell");
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
    rail.set_keyboard_interactivity(KeyboardInteractivity::None);
    rail.set_exclusive_zone(i32::try_from(RAIL_HEIGHT)?);
    rail.set_size(0, RAIL_HEIGHT);
    rail.commit();

    let mut shell = PrimeShell {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &queue_handle),
        shm,
        pool,
        background,
        rail,
        background_readiness_redraw_pending: false,
        rail_readiness_redraw_pending: false,
        exit: false,
    };

    while !shell.exit {
        event_queue.blocking_dispatch(&mut shell)?;
    }

    Ok(())
}

struct PrimeShell {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    background: LayerSurface,
    rail: LayerSurface,
    background_readiness_redraw_pending: bool,
    rail_readiness_redraw_pending: bool,
    exit: bool,
}

impl PrimeShell {
    fn draw_layer(
        pool: &mut SlotPool,
        layer: &LayerSurface,
        queue_handle: &QueueHandle<Self>,
        width: u32,
        height: u32,
        color: u32,
        role: &str,
    ) -> Result<(), Box<dyn Error>> {
        let width = i32::try_from(width)?;
        let height = i32::try_from(height)?;
        let stride = width
            .checked_mul(4)
            .ok_or_else(|| io::Error::other(format!("Prime Shell {role} stride overflow")))?;
        let (buffer, canvas) = pool.create_buffer(width, height, stride, wl_shm::Format::Argb8888)?;
        let pixel = color.to_le_bytes();
        for bytes in canvas.chunks_exact_mut(4) {
            bytes.copy_from_slice(&pixel);
        }

        layer.wl_surface().damage_buffer(0, 0, width, height);
        buffer.attach_to(layer.wl_surface())?;
        layer
            .wl_surface()
            .frame(queue_handle, layer.wl_surface());
        layer.commit();
        Ok(())
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
        surface: &wl_surface::WlSurface,
        _time: u32,
    ) {
        let pending = if surface == self.background.wl_surface() {
            &mut self.background_readiness_redraw_pending
        } else if surface == self.rail.wl_surface() {
            &mut self.rail_readiness_redraw_pending
        } else {
            return;
        };

        if *pending {
            *pending = false;
            surface.damage_buffer(0, 0, i32::MAX, i32::MAX);
            surface.commit();
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
        _layer: &LayerSurface,
    ) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let (role, color, redraw_pending) = if layer.wl_surface() == self.background.wl_surface() {
            (
                "background",
                PRIME_BACKGROUND_ARGB,
                &mut self.background_readiness_redraw_pending,
            )
        } else if layer.wl_surface() == self.rail.wl_surface() {
            (
                "rail",
                PRIME_RAIL_ARGB,
                &mut self.rail_readiness_redraw_pending,
            )
        } else {
            return;
        };

        let Some(width) = NonZeroU32::new(configure.new_size.0) else {
            eprintln!("prime-shell received zero {role} width; refusing to map an ambiguous surface");
            self.exit = true;
            return;
        };
        let Some(height) = NonZeroU32::new(configure.new_size.1) else {
            eprintln!("prime-shell received zero {role} height; refusing to map an ambiguous surface");
            self.exit = true;
            return;
        };

        *redraw_pending = true;
        if let Err(error) = Self::draw_layer(
            &mut self.pool,
            layer,
            queue_handle,
            width.get(),
            height.get(),
            color,
            role,
        ) {
            *redraw_pending = false;
            eprintln!("prime-shell could not draw the {role} surface: {error}");
            self.exit = true;
        }
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
delegate_layer!(PrimeShell);
delegate_registry!(PrimeShell);

impl ProvidesRegistryState for PrimeShell {
    fn registry(&mut self) -> &mut RegistryState {
        &mut self.registry_state
    }

    registry_handlers![OutputState];
}
