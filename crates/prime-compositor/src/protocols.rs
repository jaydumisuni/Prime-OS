use crate::{PrimeClientState, Runtime};
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_layer_shell, delegate_output, delegate_seat, delegate_shm,
    delegate_xdg_shell,
    desktop::{
        layer_map_for_output, LayerSurface as DesktopLayerSurface, PopupKind, PopupManager, Space,
        Window, WindowSurfaceType,
    },
    input::{
        keyboard::{KeyboardHandle, XkbConfig},
        pointer::PointerHandle,
        SeatHandler, SeatState,
    },
    reexports::wayland_server::{
        backend::GlobalId,
        protocol::{wl_buffer, wl_output::WlOutput, wl_surface::WlSurface},
        Client,
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_parent, is_sync_subsurface, with_states, CompositorClientState, CompositorHandler,
            CompositorState,
        },
        output::{OutputHandler, OutputManagerState},
        shell::{
            wlr_layer::{
                Layer, LayerSurface as WlrLayerSurface, LayerSurfaceData, WlrLayerShellHandler,
                WlrLayerShellState,
            },
            xdg::{PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState},
        },
        shm::{ShmHandler, ShmState},
    },
};

pub(crate) struct ProtocolState {
    pub(crate) compositor_state: CompositorState,
    pub(crate) shm_state: ShmState,
    pub(crate) xdg_shell_state: XdgShellState,
    pub(crate) layer_shell_state: WlrLayerShellState,
    pub(crate) seat_state: SeatState<Runtime>,
    pub(crate) keyboard: KeyboardHandle<Runtime>,
    pub(crate) pointer: PointerHandle<Runtime>,
    pub(crate) space: Space<Window>,
    pub(crate) popups: PopupManager,
    _output_manager_state: OutputManagerState,
    _output_global: GlobalId,
}

impl ProtocolState {
    pub(crate) fn new(
        display_handle: &smithay::reexports::wayland_server::DisplayHandle,
        output: &smithay::output::Output,
        seat_name: &str,
    ) -> Result<Self, smithay::input::keyboard::Error> {
        let compositor_state = CompositorState::new::<Runtime>(display_handle);
        let shm_state = ShmState::new::<Runtime>(display_handle, vec![]);
        let output_manager_state =
            OutputManagerState::new_with_xdg_output::<Runtime>(display_handle);
        let output_global = output.create_global::<Runtime>(display_handle);
        let xdg_shell_state = XdgShellState::new::<Runtime>(display_handle);
        let layer_shell_state = WlrLayerShellState::new::<Runtime>(display_handle);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(display_handle, seat_name.to_owned());
        let keyboard = seat.add_keyboard(XkbConfig::default(), 200, 25)?;
        let pointer = seat.add_pointer();

        let mut space = Space::default();
        space.map_output(output, (0, 0));

        Ok(Self {
            compositor_state,
            shm_state,
            xdg_shell_state,
            layer_shell_state,
            seat_state,
            keyboard,
            pointer,
            space,
            popups: PopupManager::default(),
            _output_manager_state: output_manager_state,
            _output_global: output_global,
        })
    }
}

impl SeatHandler for Runtime {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.protocols.seat_state
    }
}

impl CompositorHandler for Runtime {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.protocols.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client
            .get_data::<PrimeClientState>()
            .expect("accepted Wayland client is missing PrimeClientState")
            .compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        on_commit_buffer_handler::<Self>(surface);

        if !is_sync_subsurface(surface) {
            let mut root = surface.clone();
            while let Some(parent) = get_parent(&root) {
                root = parent;
            }

            if let Some(window) = self.protocols.space.elements().find(|window| {
                window
                    .toplevel()
                    .is_some_and(|toplevel| toplevel.wl_surface() == &root)
            }) {
                window.on_commit();
            }
        }

        handle_xdg_commit(&mut self.protocols.popups, &self.protocols.space, surface);
        handle_layer_commit(&self._output, surface);
        if crate::shell::persistent_layer_for_surface(&self._output, surface) {
            self.invalidate_shell_readiness(crate::shell::SHELL_NOT_PROVEN_LIMITATION);
        }
        self.request_frame();
    }
}

impl BufferHandler for Runtime {
    fn buffer_destroyed(&mut self, _buffer: &wl_buffer::WlBuffer) {}
}

impl ShmHandler for Runtime {
    fn shm_state(&self) -> &ShmState {
        &self.protocols.shm_state
    }
}

impl OutputHandler for Runtime {}

impl XdgShellHandler for Runtime {
    fn xdg_shell_state(&mut self) -> &mut XdgShellState {
        &mut self.protocols.xdg_shell_state
    }

    fn new_toplevel(&mut self, surface: ToplevelSurface) {
        let window = Window::new_wayland_window(surface);
        self.protocols.space.map_element(window, (0, 0), false);
        self.request_frame();
    }

    fn new_popup(&mut self, surface: PopupSurface, _positioner: PositionerState) {
        if let Err(error) = self.protocols.popups.track_popup(PopupKind::Xdg(surface)) {
            eprintln!("prime-compositor could not track XDG popup: {error}");
        }
    }

    fn grab(
        &mut self,
        _surface: PopupSurface,
        _seat: smithay::reexports::wayland_server::protocol::wl_seat::WlSeat,
        _serial: smithay::utils::Serial,
    ) {
    }

    fn reposition_request(
        &mut self,
        surface: PopupSurface,
        positioner: PositionerState,
        token: u32,
    ) {
        surface.with_pending_state(|state| {
            state.geometry = positioner.get_geometry();
            state.positioner = positioner;
        });
        surface.send_repositioned(token);
    }
}

impl WlrLayerShellHandler for Runtime {
    fn shell_state(&mut self) -> &mut WlrLayerShellState {
        &mut self.protocols.layer_shell_state
    }

    fn new_layer_surface(
        &mut self,
        surface: WlrLayerSurface,
        output: Option<WlOutput>,
        _layer: Layer,
        namespace: String,
    ) {
        let target = output
            .as_ref()
            .and_then(smithay::output::Output::from_resource)
            .unwrap_or_else(|| self._output.clone());
        if target != self._output {
            eprintln!("prime-compositor rejected layer surface for unsupported output");
            self.invalidate_frame_loop("FRAME_MAPPING_ERROR", crate::frame::FRAME_ERROR_LIMITATION);
            return;
        }

        let desktop_surface = DesktopLayerSurface::new(surface, namespace);
        let map_result = {
            let mut map = layer_map_for_output(&self._output);
            map.map_layer(&desktop_surface)
        };
        if let Err(error) = map_result {
            eprintln!("prime-compositor could not map layer surface: {error}");
            self.invalidate_frame_loop("FRAME_MAPPING_ERROR", crate::frame::FRAME_ERROR_LIMITATION);
        }
    }

    fn layer_destroyed(&mut self, surface: WlrLayerSurface) {
        let mut map = layer_map_for_output(&self._output);
        let mapped = map
            .layers()
            .find(|layer| layer.wl_surface() == surface.wl_surface())
            .cloned();
        if let Some(layer) = mapped {
            let persistent_shell = crate::shell::is_persistent_namespace(layer.namespace());
            map.unmap_layer(&layer);
            drop(map);
            if persistent_shell {
                self.invalidate_shell_readiness(crate::shell::SHELL_NOT_PROVEN_LIMITATION);
            }
            self.request_frame();
        }
    }
}

fn handle_xdg_commit(popups: &mut PopupManager, space: &Space<Window>, surface: &WlSurface) {
    if let Some(toplevel) = space.elements().find_map(|window| {
        window
            .toplevel()
            .filter(|toplevel| toplevel.wl_surface() == surface)
            .cloned()
    }) {
        if !toplevel.is_initial_configure_sent() {
            toplevel.send_configure();
        }
    }

    popups.commit(surface);
    if let Some(PopupKind::Xdg(popup)) = popups.find_popup(surface) {
        if !popup.is_initial_configure_sent() {
            if let Err(error) = popup.send_configure() {
                eprintln!("prime-compositor could not send initial XDG popup configure: {error}");
            }
        }
    }
}

fn handle_layer_commit(output: &smithay::output::Output, surface: &WlSurface) {
    let mut map = layer_map_for_output(output);
    if map
        .layer_for_surface(surface, WindowSurfaceType::ALL)
        .is_none()
    {
        return;
    }

    let is_layer_root = map
        .layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
        .is_some();
    let initial_configure_sent = is_layer_root.then(|| {
        with_states(surface, |states| {
            states
                .data_map
                .get::<LayerSurfaceData>()
                .expect("mapped WLR layer root is missing LayerSurfaceData")
                .lock()
                .unwrap()
                .initial_configure_sent
        })
    });

    map.arrange();

    if initial_configure_sent == Some(false) {
        map.layer_for_surface(surface, WindowSurfaceType::TOPLEVEL)
            .expect("mapped WLR layer root disappeared during arrangement")
            .layer_surface()
            .send_configure();
    }
}

delegate_compositor!(Runtime);
delegate_seat!(Runtime);
delegate_shm!(Runtime);
delegate_output!(Runtime);
delegate_xdg_shell!(Runtime);
delegate_layer_shell!(Runtime);
