use crate::Runtime;
use smithay::{
    backend::renderer::{
        element::{surface::WaylandSurfaceRenderElement, AsRenderElements},
        gles::GlesRenderer,
    },
    desktop::{layer_map_for_output, WindowSurfaceType},
    output::Output,
    reexports::wayland_server::{
        backend::ClientId, protocol::wl_surface::WlSurface, Resource,
    },
    utils::{IsAlive, Scale},
    wayland::shell::wlr_layer::Layer,
};

pub(crate) const BACKGROUND_NAMESPACE: &str = "prime.shell.background";
pub(crate) const RAIL_NAMESPACE: &str = "prime.shell.rail";
pub(crate) const SHELL_NOT_PROVEN_LIMITATION: &str =
    "Prime Shell persistent baseline has not retired in a selected-CRTC frame";
pub(crate) const SHELL_REVALIDATION_LIMITATION: &str =
    "Prime Shell readiness requires revalidation with frame/output/session state";

#[derive(Clone)]
pub(crate) struct ShellBaselineIdentity {
    background: WlSurface,
    rail: WlSurface,
}

impl Runtime {
    pub(crate) fn invalidate_shell_readiness(&mut self, limitation: &str) {
        if self.readiness.phase == "SHELL_READY" && self.readiness.frame_loop_ready {
            self.readiness.phase = "FRAME_LOOP_READY".to_owned();
        }
        self.readiness.shell_ready = false;
        self.frame.in_flight_shell_baseline = None;
        self.add_limitation(limitation);
    }
}

pub(crate) fn is_persistent_namespace(namespace: &str) -> bool {
    matches!(namespace, BACKGROUND_NAMESPACE | RAIL_NAMESPACE)
}

fn expected_layer(namespace: &str) -> Option<Layer> {
    match namespace {
        BACKGROUND_NAMESPACE => Some(Layer::Background),
        RAIL_NAMESPACE => Some(Layer::Top),
        _ => None,
    }
}

fn renderable_candidate(
    renderer: &mut GlesRenderer,
    output: &Output,
    layer: &smithay::desktop::LayerSurface,
) -> Option<(ClientId, WlSurface)> {
    if !layer.alive() || expected_layer(layer.namespace()) != Some(layer.layer()) {
        return None;
    }

    let map = layer_map_for_output(output);
    let geometry = map.layer_geometry(layer)?;
    let scale: Scale<f64> = output.current_scale().fractional_scale().into();
    let location = geometry.loc.to_physical_precise_round(scale);
    let elements = AsRenderElements::<GlesRenderer>::render_elements::<
        WaylandSurfaceRenderElement<GlesRenderer>,
    >(layer, renderer, location, scale, 1.0);
    if elements.is_empty() {
        return None;
    }

    let surface = layer.wl_surface().clone();
    let client = surface.client()?;
    Some((client.id(), surface))
}

pub(crate) fn persistent_baseline_renderable(
    renderer: &mut GlesRenderer,
    output: &Output,
) -> Option<ShellBaselineIdentity> {
    let map = layer_map_for_output(output);
    let mut backgrounds = Vec::new();
    let mut rails = Vec::new();

    for layer in map.layers() {
        let Some(candidate) = renderable_candidate(renderer, output, layer) else {
            continue;
        };
        match layer.namespace() {
            BACKGROUND_NAMESPACE => backgrounds.push(candidate),
            RAIL_NAMESPACE => rails.push(candidate),
            _ => {}
        }
    }

    if backgrounds.len() != 1 || rails.len() != 1 {
        return None;
    }

    let (background_client, background) = backgrounds.pop()?;
    let (rail_client, rail) = rails.pop()?;
    if background_client != rail_client {
        return None;
    }

    Some(ShellBaselineIdentity { background, rail })
}

pub(crate) fn persistent_baseline_identity_renderable(
    renderer: &mut GlesRenderer,
    output: &Output,
    identity: &ShellBaselineIdentity,
) -> bool {
    let map = layer_map_for_output(output);
    let mut background_renderable = false;
    let mut rail_renderable = false;

    for layer in map.layers() {
        if layer.wl_surface() == &identity.background
            && layer.namespace() == BACKGROUND_NAMESPACE
            && layer.layer() == Layer::Background
        {
            background_renderable = renderable_candidate(renderer, output, layer).is_some();
        } else if layer.wl_surface() == &identity.rail
            && layer.namespace() == RAIL_NAMESPACE
            && layer.layer() == Layer::Top
        {
            rail_renderable = renderable_candidate(renderer, output, layer).is_some();
        }
    }

    background_renderable && rail_renderable
}

pub(crate) fn persistent_layer_for_surface(output: &Output, surface: &WlSurface) -> bool {
    let map = layer_map_for_output(output);
    map.layer_for_surface(surface, WindowSurfaceType::ALL)
        .is_some_and(|layer| {
            is_persistent_namespace(layer.namespace())
                && expected_layer(layer.namespace()) == Some(layer.layer())
        })
}
