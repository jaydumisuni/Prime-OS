use smithay::{
    desktop::layer_map_for_output,
    output::Output,
    utils::IsAlive,
    wayland::shell::wlr_layer::Layer,
};

pub(crate) const BACKGROUND_NAMESPACE: &str = "prime.shell.background";
pub(crate) const RAIL_NAMESPACE: &str = "prime.shell.rail";
pub(crate) const SHELL_NOT_PROVEN_LIMITATION: &str =
    "Prime Shell persistent baseline is not proven until background and rail survive a DRM retirement after FRAME readiness";

pub(crate) fn baseline_renderable(output: &Output) -> bool {
    let map = layer_map_for_output(output);
    let mut background = false;
    let mut rail = false;

    for layer in map.layers().filter(|layer| layer.alive()) {
        let bbox = layer.bbox();
        let renderable = bbox.size.w > 0 && bbox.size.h > 0;
        if !renderable {
            continue;
        }

        match layer.namespace() {
            BACKGROUND_NAMESPACE if layer.layer() == Layer::Background => background = true,
            RAIL_NAMESPACE if layer.layer() == Layer::Top => rail = true,
            _ => {}
        }
    }

    background && rail
}
