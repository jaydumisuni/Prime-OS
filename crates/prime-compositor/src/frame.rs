use crate::Runtime;
use smithay::{
    backend::{
        drm::compositor::{FrameFlags, PrimaryPlaneElement},
        renderer::{Color32F, Renderer},
    },
    desktop::{layer_map_for_output, space::space_render_elements},
    reexports::drm::control::crtc,
};
use std::{
    error::Error,
    io,
    time::{Duration, Instant},
};

pub(crate) const FRAME_NOT_PROVEN_LIMITATION: &str =
    "Mapped-surface frame lifecycle is not proven until a queued frame retires on matching DRM vblank";
pub(crate) const FRAME_ERROR_LIMITATION: &str =
    "Mapped-surface frame lifecycle requires restart or explicit revalidation after frame failure";
pub(crate) const FRAME_REVALIDATION_LIMITATION: &str =
    "Mapped-surface frame lifecycle requires revalidation with graphics/output state";

pub(crate) struct FrameState {
    pub(crate) requested: bool,
    pub(crate) in_flight: bool,
    pub(crate) in_flight_had_mapped_surface: bool,
    pub(crate) in_flight_had_shell_baseline: bool,
    pub(crate) retry_at: Instant,
    pub(crate) clock_started: Instant,
}

impl FrameState {
    pub(crate) fn new() -> Self {
        let now = Instant::now();
        Self {
            requested: true,
            in_flight: false,
            in_flight_had_mapped_surface: false,
            in_flight_had_shell_baseline: false,
            retry_at: now,
            clock_started: now,
        }
    }

    pub(crate) fn reset(&mut self) {
        self.requested = false;
        self.in_flight = false;
        self.in_flight_had_mapped_surface = false;
        self.in_flight_had_shell_baseline = false;
        self.retry_at = Instant::now();
    }
}

pub(crate) fn request(runtime: &mut Runtime) {
    runtime.frame.requested = true;
}

pub(crate) fn try_queue(runtime: &mut Runtime) -> Result<(), Box<dyn Error>> {
    if !runtime.frame.requested
        || runtime.frame.in_flight
        || !runtime.readiness.session_active
        || !runtime.readiness.outputs_ready
        || !runtime.readiness.wayland_protocols_ready
    {
        return Ok(());
    }

    let now = Instant::now();
    if now < runtime.frame.retry_at {
        return Ok(());
    }

    runtime.protocols.space.refresh();
    layer_map_for_output(&runtime._output).arrange();
    let elements = space_render_elements(
        &mut runtime._renderer,
        [&runtime.protocols.space],
        &runtime._output,
        1.0,
    )?;
    let had_mapped_surface = !elements.is_empty();
    let had_shell_baseline = crate::shell::baseline_renderable(&runtime._output);
    let result = runtime._drm_output.render_frame(
        &mut runtime._renderer,
        &elements,
        Color32F::BLACK,
        FrameFlags::empty(),
    )?;

    let is_empty = result.is_empty;
    if result.needs_sync() {
        match &result.primary_element {
            PrimaryPlaneElement::Swapchain(element) => runtime._renderer.wait(&element.sync)?,
            PrimaryPlaneElement::Element(_) => {
                return Err(io::Error::other(
                    "DRM frame requested explicit synchronization without a swapchain sync point",
                )
                .into());
            }
        }
    }
    drop(result);

    if is_empty {
        runtime.frame.retry_at = now + Duration::from_millis(16);
        return Ok(());
    }

    runtime._drm_output.queue_frame(())?;
    runtime.frame.in_flight = true;
    runtime.frame.in_flight_had_mapped_surface = had_mapped_surface;
    runtime.frame.in_flight_had_shell_baseline = had_shell_baseline;
    runtime.frame.requested = false;
    runtime.readiness.frame_in_flight = true;
    runtime.readiness.frames_queued = runtime.readiness.frames_queued.saturating_add(1);
    Ok(())
}

pub(crate) fn handle_vblank(
    runtime: &mut Runtime,
    event_crtc: crtc::Handle,
) -> Result<bool, Box<dyn Error>> {
    if event_crtc != runtime._drm_output.crtc() || !runtime.frame.in_flight {
        return Ok(false);
    }

    runtime._drm_output.frame_submitted()?;
    let frame_loop_was_ready = runtime.readiness.frame_loop_ready;
    let followup_frame_requested = runtime.frame.requested;
    let submitted_had_mapped_surface = runtime.frame.in_flight_had_mapped_surface;
    let submitted_had_shell_baseline = runtime.frame.in_flight_had_shell_baseline;
    runtime.frame.in_flight = false;
    runtime.readiness.frame_in_flight = false;
    runtime.readiness.frames_submitted = runtime.readiness.frames_submitted.saturating_add(1);

    if !followup_frame_requested {
        let elapsed = runtime.frame.clock_started.elapsed();
        let output = runtime._output.clone();
        for window in runtime.protocols.space.elements() {
            window.send_frame(&output, elapsed, None, |_, _| Some(output.clone()));
        }
        let layer_map = layer_map_for_output(&output);
        for layer in layer_map.layers() {
            layer.send_frame(&output, elapsed, None, |_, _| Some(output.clone()));
        }
    }

    if submitted_had_mapped_surface {
        runtime.readiness.mapped_surface_frames_submitted = runtime
            .readiness
            .mapped_surface_frames_submitted
            .saturating_add(1);
        if !runtime.readiness.frame_loop_ready {
            runtime.readiness.frame_loop_ready = true;
            runtime.readiness.phase = "FRAME_LOOP_READY".to_owned();
            runtime.remove_limitation(FRAME_NOT_PROVEN_LIMITATION);
        }
    }

    let shell_baseline_still_renderable = crate::shell::baseline_renderable(&runtime._output);
    if frame_loop_was_ready && submitted_had_shell_baseline && shell_baseline_still_renderable {
        runtime.readiness.shell_frames_submitted =
            runtime.readiness.shell_frames_submitted.saturating_add(1);
        runtime.readiness.shell_ready = true;
        runtime.readiness.phase = "SHELL_READY".to_owned();
        runtime.remove_limitation(crate::shell::SHELL_NOT_PROVEN_LIMITATION);
    } else if !frame_loop_was_ready
        && runtime.readiness.frame_loop_ready
        && submitted_had_shell_baseline
        && shell_baseline_still_renderable
    {
        runtime.frame.requested = true;
    }

    runtime.frame.in_flight_had_mapped_surface = false;
    runtime.frame.in_flight_had_shell_baseline = false;
    runtime.revalidate_shell_presence();
    Ok(true)
}
