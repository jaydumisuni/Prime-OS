use std::{error::Error, num::NonZeroU32};

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
const PRIME_BACKGROUND_ARGB: u32 = 0xff11141a;

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

    let surface = compositor.create_surface(&queue_handle);
    let layer = layer_shell.create_layer_surface(
        &queue_handle,
        surface,
        Layer::Background,
        Some(BACKGROUND_NAMESPACE),
        None,
    );
    layer.set_anchor(Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT);
    layer.set_keyboard_interactivity(KeyboardInteractivity::None);
    layer.set_size(0, 0);
    layer.commit();

    let mut shell = PrimeShell {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &queue_handle),
        shm,
        pool: SlotPool::new(4, &shm)?,
        layer,
        width: 1,
        height: 1,
        first_configure: true,
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
    layer: LayerSurface,
    width: u32,
    height: u32,
    first_configure: bool,
    exit: bool,
}

impl PrimeShell {
    fn draw_background(&mut self) -> Result<(), Box<dyn Error>> {
        let width = i32::try_from(self.width)?;
        let height = i32::try_from(self.height)?;
        let stride = width.checked_mul(4).ok_or("Prime Shell background stride overflow")?;
        let (buffer, canvas) =
            self.pool.create_buffer(width, height, stride, wl_shm::Format::Argb8888)?;
        let pixel = PRIME_BACKGROUND_ARGB.to_le_bytes();
        for bytes in canvas.chunks_exact_mut(4) {
            bytes.copy_from_slice(&pixel);
        }

        self.layer.wl_surface().damage_buffer(0, 0, width, height);
        buffer.attach_to(self.layer.wl_surface())?;
        self.layer.commit();
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
        _layer: &LayerSurface,
    ) {
        self.exit = true;
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        _layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        self.width = NonZeroU32::new(configure.new_size.0).map_or(1, NonZeroU32::get);
        self.height = NonZeroU32::new(configure.new_size.1).map_or(1, NonZeroU32::get);
        if self.first_configure {
            self.first_configure = false;
            if let Err(error) = self.draw_background() {
                eprintln!("prime-shell could not draw the background surface: {error}");
                self.exit = true;
            }
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
