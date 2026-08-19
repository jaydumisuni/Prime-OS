use crate::{PrimeClientState, Runtime};
use smithay::{
    backend::renderer::utils::on_commit_buffer_handler,
    delegate_compositor, delegate_layer_shell, delegate_output, delegate_shm, delegate_xdg_shell,
    desktop::{PopupKind, PopupManager, Space, Window},
    reexports::wayland_server::{
        backend::GlobalId,
        protocol::{wl_buffer, wl_output::WlOutput, wl_surface::WlSurface},
        Client,
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{
            get_parent, is_sync_subsurface, CompositorClientState, CompositorHandler,
            CompositorState,
        },
        output::{OutputHandler, OutputManagerState},
        shell::{
            wlr_layer::{Layer, LayerSurface, WlrLayerShellHandler, WlrLayerShellState},
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
    pub(crate) space: Space<Window>,
    pub(crate) popups: PopupManager,
    layer_surfaces: Vec<LayerSurface>,
    _output_manager_state: OutputManagerState,
    _output_global: GlobalId,
}

impl ProtocolState {
    pub(crate) fn new(
        display_handle: &smithay::reexports::wayland_server::DisplayHandle,
        output: &smithay::output::Output,
    ) -> Self {
        let compositor_state = CompositorState::new::<Runtime>(display_handle);
        let shm_state = ShmState::new::<Runtime>(display_handle, vec![]);
        let output_manager_state =
            OutputManagerState::new_with_xdg_output::<Runtime>(display_handle);
        let output_global = output.create_global::<Runtime>(display_handle);
        let xdg_shell_state = XdgShellState::new::<Runtime>(display_handle);
        let layer_shell_state = WlrLayerShellState::new::<Runtime>(display_handle);

        Self {
            compositor_state,
            shm_state,
            xdg_shell_state,
            layer_shell_state,
            space: Space::default(),
            popups: PopupManager::default(),
            layer_surfaces: Vec::new(),
            _output_manager_state: output_manager_state,
            _output_global: output_global,
        }
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
        surface: LayerSurface,
        _output: Option<WlOutput>,
        _layer: Layer,
        _namespace: String,
    ) {
        self.protocols.layer_surfaces.retain(LayerSurface::alive);
        surface.send_configure();
        self.protocols.layer_surfaces.push(surface);
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

delegate_compositor!(Runtime);
delegate_shm!(Runtime);
delegate_output!(Runtime);
delegate_xdg_shell!(Runtime);
delegate_layer_shell!(Runtime);
