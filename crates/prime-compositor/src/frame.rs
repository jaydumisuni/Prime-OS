use crate::Runtime;
use smithay::{
    backend::{
        drm::compositor::{FrameFlags, PrimaryPlaneElement},
        renderer::{Color32F, Renderer},
    },
    desktop::space::space_render_elements,
    reexports::drm::control::crtc,
};

pub(crate) fn render_if_ready(runtime: &mut Runtime) {
    if !runtime.readiness.frame_loop_ready
        || !runtime.readiness.session_active
        || !runtime.readiness.renderer_ready
        || !runtime.readiness.outputs_ready
        || runtime.frame_in_flight
    {
        return;
    }

    runtime.protocols.space.refresh();
    let elements = match space_render_elements(
        &mut runtime._renderer,
        [&runtime.protocols.space],
        &runtime._output,
        1.0,
    ) {
        Ok(elements) => elements,
        Err(error) => {
            runtime.invalidate_frame(format!("space render-element preparation failed: {error}"));
            runtime.persist_best_effort();
            return;
        }
    };

    let frame = match runtime._drm_output.render_frame(
        &mut runtime._renderer,
        &elements,
        Color32F::BLACK,
        FrameFlags::DEFAULT,
    ) {
        Ok(frame) => frame,
        Err(error) => {
            runtime.invalidate_frame(format!("DRM frame render failed: {error}"));
            runtime.persist_best_effort();
            return;
        }
    };

    if frame.is_empty {
        return;
    }

    if frame.needs_sync() {
        if let PrimaryPlaneElement::Swapchain(primary) = &frame.primary_element {
            if let Err(error) = runtime._renderer.wait(&primary.sync) {
                runtime.invalidate_frame(format!("renderer/KMS synchronization failed: {error}"));
                runtime.persist_best_effort();
                return;
            }
        }
    }

    drop(frame);
    if let Err(error) = runtime._drm_output.queue_frame(()) {
        runtime.invalidate_frame(format!("DRM frame queue failed: {error}"));
        runtime.persist_best_effort();
        return;
    }

    runtime.frame_in_flight = true;
}

pub(crate) fn handle_vblank(runtime: &mut Runtime, crtc: crtc::Handle) {
    if crtc != runtime._drm_output.crtc() || !runtime.frame_in_flight {
        return;
    }

    if let Err(error) = runtime._drm_output.frame_submitted() {
        runtime.invalidate_frame(format!("DRM frame retirement failed: {error}"));
        runtime.persist_best_effort();
        return;
    }

    runtime.frame_in_flight = false;
    runtime
        .protocols
        .send_frame_callbacks(&runtime._output, runtime.frame_clock.elapsed());
}
