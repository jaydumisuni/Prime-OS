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
const ORB_NAMESPACE: &str = "prime.shell.orb";

const CONSTRUCTION_RAIL_HEIGHT: u32 = 48;
const CONSTRUCTION_ORB_SIZE: u32 = 72;

const BACKGROUND_ARGB: u32 = 0xff11141a;
const RAIL_ARGB: u32 = 0xff1b2028;
const ORB_ARGB: u32 = 0xff2b303a;

fn main() -> Result<(), Box<dyn Error>> {
    if std::env::args().any(|arg| arg == "--help" || arg == "-h") {
        println!("prime-shell — Prime P1 native Shell construction host");
        println!("Usage: prime-shell");
        println!("Persistent construction surfaces: background, rail, orb");
        return Ok(());
    }

    let connection = Connection::connect_to_env()?;
    let (globals, mut event_queue) = registry_queue_init(&connection)?;
    let queue_handle = event_queue.handle();

    let compositor = CompositorState::bind(&globals, &queue_handle)?;
    let layer_shell = LayerShell::bind(&globals, &queue_handle)?;
    let shm = Shm::bind(&globals, &queue_handle)?;
    let pool = SlotPool::new(4, &shm)?;

    let background = create_surface(
        &compositor,
        &layer_shell,
        &queue_handle,
        SurfaceSpec {
            name: "background",
            namespace: BACKGROUND_NAMESPACE,
            layer: Layer::Background,
            anchor: Anchor::TOP | Anchor::BOTTOM | Anchor::LEFT | Anchor::RIGHT,
            keyboard: KeyboardInteractivity::None,
            exclusive_zone: -1,
            requested_width: 0,
            requested_height: 0,
            color: BACKGROUND_ARGB,
        },
    );
    let rail = create_surface(
        &compositor,
        &layer_shell,
        &queue_handle,
        SurfaceSpec {
            name: "rail",
            namespace: RAIL_NAMESPACE,
            layer: Layer::Top,
            anchor: Anchor::TOP | Anchor::LEFT | Anchor::RIGHT,
            keyboard: KeyboardInteractivity::None,
            exclusive_zone: CONSTRUCTION_RAIL_HEIGHT as i32,
            requested_width: 0,
            requested_height: CONSTRUCTION_RAIL_HEIGHT,
            color: RAIL_ARGB,
        },
    );
    let orb = create_surface(
        &compositor,
        &layer_shell,
        &queue_handle,
        SurfaceSpec {
            name: "orb",
            namespace: ORB_NAMESPACE,
            layer: Layer::Overlay,
            anchor: Anchor::BOTTOM,
            keyboard: KeyboardInteractivity::OnDemand,
            exclusive_zone: 0,
            requested_width: CONSTRUCTION_ORB_SIZE,
            requested_height: CONSTRUCTION_ORB_SIZE,
            color: ORB_ARGB,
        },
    );

    let mut shell = PrimeShell {
        registry_state: RegistryState::new(&globals),
        output_state: OutputState::new(&globals, &queue_handle),
        shm,
        pool,
        surfaces: vec![background, rail, orb],
        exit: false,
        persistent_set_reported: false,
    };

    while !shell.exit {
        event_queue.blocking_dispatch(&mut shell)?;
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct SurfaceSpec {
    name: &'static str,
    namespace: &'static str,
    layer: Layer,
    anchor: Anchor,
    keyboard: KeyboardInteractivity,
    exclusive_zone: i32,
    requested_width: u32,
    requested_height: u32,
    color: u32,
}

struct SurfaceState {
    spec: SurfaceSpec,
    layer: LayerSurface,
    width: u32,
    height: u32,
    configured: bool,
}

fn create_surface(
    compositor: &CompositorState,
    layer_shell: &LayerShell,
    queue_handle: &QueueHandle<PrimeShell>,
    spec: SurfaceSpec,
) -> SurfaceState {
    let surface = compositor.create_surface(queue_handle);
    let layer = layer_shell.create_layer_surface(
        queue_handle,
        surface,
        spec.layer,
        Some(spec.namespace),
        None,
    );
    layer.set_anchor(spec.anchor);
    layer.set_keyboard_interactivity(spec.keyboard);
    layer.set_exclusive_zone(spec.exclusive_zone);
    layer.set_size(spec.requested_width, spec.requested_height);
    layer.commit();

    SurfaceState {
        spec,
        layer,
        width: spec.requested_width,
        height: spec.requested_height,
        configured: false,
    }
}

struct PrimeShell {
    registry_state: RegistryState,
    output_state: OutputState,
    shm: Shm,
    pool: SlotPool,
    surfaces: Vec<SurfaceState>,
    exit: bool,
    persistent_set_reported: bool,
}

impl PrimeShell {
    fn surface_index(&self, layer: &LayerSurface) -> Option<usize> {
        self.surfaces
            .iter()
            .position(|surface| surface.layer.wl_surface() == layer.wl_surface())
    }

    fn configured_size(
        surface: &SurfaceState,
        configure: LayerSurfaceConfigure,
    ) -> Option<(u32, u32)> {
        let width = NonZeroU32::new(configure.new_size.0)
            .map(NonZeroU32::get)
            .or_else(|| NonZeroU32::new(surface.spec.requested_width).map(NonZeroU32::get));
        let height = NonZeroU32::new(configure.new_size.1)
            .map(NonZeroU32::get)
            .or_else(|| NonZeroU32::new(surface.spec.requested_height).map(NonZeroU32::get));
        width.zip(height)
    }

    fn draw_surface(&mut self, index: usize) -> Result<(), Box<dyn Error>> {
        let (pool, surfaces) = (&mut self.pool, &self.surfaces);
        let surface = &surfaces[index];
        let width = i32::try_from(surface.width)?;
        let height = i32::try_from(surface.height)?;
        let stride = width
            .checked_mul(4)
            .ok_or_else(|| io::Error::other("Prime Shell surface stride overflow"))?;
        let (buffer, canvas) =
            pool.create_buffer(width, height, stride, wl_shm::Format::Argb8888)?;
        let pixel = surface.spec.color.to_le_bytes();
        for bytes in canvas.chunks_exact_mut(4) {
            bytes.copy_from_slice(&pixel);
        }

        surface.layer.wl_surface().damage_buffer(0, 0, width, height);
        buffer.attach_to(surface.layer.wl_surface())?;
        surface.layer.commit();
        Ok(())
    }

    fn report_persistent_set_once(&mut self) {
        if !self.persistent_set_reported && self.surfaces.iter().all(|surface| surface.configured) {
            self.persistent_set_reported = true;
            eprintln!(
                "PRIME_SHELL_PERSISTENT_SURFACES_CONFIGURED=background,rail,orb;readiness_unearned"
            );
        }
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
        if let Some(index) = self.surface_index(layer) {
            eprintln!(
                "prime-shell required persistent surface closed: {}",
                self.surfaces[index].spec.name
            );
        } else {
            eprintln!("prime-shell received close for an unknown layer surface");
        }
        self.exit = true;
    }

    fn configure(
        &mut self,
        _connection: &Connection,
        _queue_handle: &QueueHandle<Self>,
        layer: &LayerSurface,
        configure: LayerSurfaceConfigure,
        _serial: u32,
    ) {
        let Some(index) = self.surface_index(layer) else {
            eprintln!("prime-shell received configure for an unknown layer surface");
            self.exit = true;
            return;
        };

        let Some((width, height)) = Self::configured_size(&self.surfaces[index], configure) else {
            eprintln!(
                "prime-shell received no usable dimensions for required {} surface",
                self.surfaces[index].spec.name
            );
            self.exit = true;
            return;
        };

        self.surfaces[index].width = width;
        self.surfaces[index].height = height;
        if let Err(error) = self.draw_surface(index) {
            eprintln!(
                "prime-shell could not draw required {} surface: {error}",
                self.surfaces[index].spec.name
            );
            self.exit = true;
            return;
        }
        self.surfaces[index].configured = true;
        self.report_persistent_set_once();
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
