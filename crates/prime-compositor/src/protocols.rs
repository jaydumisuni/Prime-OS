use crate::{input, Runtime};
use smithay::{
    delegate_compositor, delegate_output, delegate_seat, delegate_shm, delegate_xdg_shell,
    desktop::{
        layer_map_for_output, PopupKind, PopupManager, Space, Window,
        {LayerSurface as DesktopLayerSurface, WindowSurfaceType},
    },
    input::{
        keyboard::KeyboardHandle,
        pointer::{CursorImageStatus, PointerHandle},
        Seat, SeatHandler, SeatState,
    },
    output::OutputHandler,
    reexports::wayland_server::{
        backend::GlobalId,
        protocol::{wl_buffer, wl_output::WlOutput, wl_seat, wl_surface::WlSurface},
        Client, DataInit, Dispatch, DisplayHandle, GlobalDispatch, New,
    },
    wayland::{
        buffer::BufferHandler,
        compositor::{
            add_blocker, add_pre_commit_hook, get_parent, is_sync_subsurface,
            with_states, CompositorClientState, CompositorHandler, CompositorState,
        },
        selection::{
            data_device::{DataDeviceHandler, DataDeviceState},
            SelectionHandler,
        },
        shell::{
            wlr_layer::{
                Layer, LayerSurface as WlrLayerSurface, WlrLayerShellHandler, WlrLayerShellState,
            },
            xdg::{
                PopupSurface, PositionerState, ToplevelSurface, XdgShellHandler, XdgShellState,
            },
        },
        shm::{ShmHandler, ShmState},
    },
};

pub(crate) struct ProtocolState {
    pub(crate) compositor_state: CompositorState,
    pub(crate) shm_state: ShmState,
    pub(crate) xdg_shell_state: XdgShellState,
    pub(crate) layer_shell_state: WlrLayerShellState,
    pub(crate) data_device_state: DataDeviceState,
    pub(crate) seat_state: SeatState<Runtime>,
    pub(crate) seat: Seat<Runtime>,
    pub(crate) keyboard: KeyboardHandle<Runtime>,
    pub(crate) pointer: PointerHandle<Runtime>,
    pub(crate) space: Space<Window>,
    pub(crate) popups: PopupManager,
}

impl ProtocolState {
    pub(crate) fn new(display: &DisplayHandle) -> Result<Self, Box<dyn std::error::Error>> {
        let compositor_state = CompositorState::new::<Runtime>(display);
        let shm_state = ShmState::new::<Runtime>(display, Vec::new());
        let xdg_shell_state = XdgShellState::new::<Runtime>(display);
        let layer_shell_state = WlrLayerShellState::new::<Runtime>(display);
        let data_device_state = DataDeviceState::new::<Runtime>(display);
        let mut seat_state = SeatState::new();
        let mut seat = seat_state.new_wl_seat(display, "prime-seat");
        let keyboard = seat.add_keyboard(Default::default(), 200, 25)?;
        let pointer = seat.add_pointer();

        Ok(Self {
            compositor_state,
            shm_state,
            xdg_shell_state,
            layer_shell_state,
            data_device_state,
            seat_state,
            seat,
            keyboard,
            pointer,
            space: Space::default(),
            popups: PopupManager::default(),
        })
    }
}

impl CompositorHandler for Runtime {
    fn compositor_state(&mut self) -> &mut CompositorState {
        &mut self.protocols.compositor_state
    }

    fn client_compositor_state<'a>(&self, client: &'a Client) -> &'a CompositorClientState {
        &client.get_data::<crate::PrimeClientState>().unwrap().compositor_state
    }

    fn commit(&mut self, surface: &WlSurface) {
        if is_sync_subsurface(surface) {
            return;
        }

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

        let persistent_shell = crate::shell::is_persistent_namespace(&namespace);
        let desktop_surface = DesktopLayerSurface::new(surface, namespace);
        let map_result = {
            let mut map = layer_map_for_output(&self._output);
            map.map_layer(&desktop_surface)
        };
        match map_result {
            Ok(()) => {
                if persistent_shell {
                    self.invalidate_shell_readiness(crate::shell::SHELL_NOT_PROVEN_LIMITATION);
                }
                self.request_frame();
            }
            Err(error) => {
                eprintln!("prime-compositor could not map layer surface: {error}");
                self.invalidate_frame_loop(
                    "FRAME_MAPPING_ERROR",
                    crate::frame::FRAME_ERROR_LIMITATION,
                );
            }
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

    if let Some(popup) = popups.find_popup(surface) {
        popup.with_pending_state(|state| {
            if !state.geometry.is_empty() {
                return;
            }
            state.geometry = state.positioner.get_geometry();
        });
        if !popup.is_initial_configure_sent() {
            popup.send_configure();
        }
    }
}

fn handle_layer_commit(output: &smithay::output::Output, surface: &WlSurface) {
    let mut map = layer_map_for_output(output);
    let Some(layer) = map
        .layers()
        .find(|layer| layer.wl_surface() == surface)
        .cloned()
    else {
        return;
    };

    if !layer.is_initial_configure_sent() {
        layer.send_configure();
    }
    map.arrange();
}

delegate_compositor!(Runtime);
delegate_shm!(Runtime);
delegate_output!(Runtime);
delegate_xdg_shell!(Runtime);
delegate_seat!(Runtime);

impl SeatHandler for Runtime {
    type KeyboardFocus = WlSurface;
    type PointerFocus = WlSurface;
    type TouchFocus = WlSurface;

    fn seat_state(&mut self) -> &mut SeatState<Self> {
        &mut self.protocols.seat_state
    }

    fn cursor_image(&mut self, _seat: &Seat<Self>, _image: CursorImageStatus) {}

    fn focus_changed(&mut self, _seat: &Seat<Self>, _focused: Option<&Self::KeyboardFocus>) {}
}

impl SelectionHandler for Runtime {
    type SelectionUserData = ();
}

impl DataDeviceHandler for Runtime {
    fn data_device_state(&self) -> &DataDeviceState {
        &self.protocols.data_device_state
    }
}

delegate_data_device!(Runtime);

smithay::delegate_primary_selection!(Runtime);

smithay::delegate_output!(Runtime);

impl GlobalDispatch<wl_output::WlOutput, GlobalId, Runtime> for Runtime {
    fn bind(
        state: &mut Runtime,
        handle: &DisplayHandle,
        client: &Client,
        resource: New<wl_output::WlOutput>,
        global_data: &GlobalId,
        data_init: &mut DataInit<'_, Runtime>,
    ) {
        smithay::wayland::output::OutputManagerState::bind(
            state,
            handle,
            client,
            resource,
            global_data,
            data_init,
        )
    }
}

impl Dispatch<wl_output::WlOutput, (), Runtime> for Runtime {
    fn request(
        state: &mut Runtime,
        client: &Client,
        resource: &wl_output::WlOutput,
        request: <wl_output::WlOutput as smithay::reexports::wayland_server::Resource>::Request,
        data: &(),
        handle: &DisplayHandle,
        data_init: &mut DataInit<'_, Runtime>,
    ) {
        smithay::wayland::output::OutputManagerState::request(
            state, client, resource, request, data, handle, data_init,
        )
    }
}
