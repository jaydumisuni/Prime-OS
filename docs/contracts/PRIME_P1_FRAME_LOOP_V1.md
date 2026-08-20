# Prime P1 Frame Loop v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Readiness phase: `FRAME_LOOP_READY`

## Purpose

Earn the first continuous Prime compositor scanout lifecycle after `WAYLAND_INPUT_READY` without widening into Prime Shell ownership, multi-output policy, client direct-scanout, touch/tablet/gesture input, or presentation timing.

## Required mechanism

Prime P1 FRAME must retain one physical Smithay output inside `Space<Window>` and use that same output for XDG windows and WLR layer-shell surfaces.

A frame request may render only through Smithay's `space_render_elements` over the retained Space/output. That path composes the mapped WLR upper layers, XDG window Space, and WLR lower layers in Smithay's defined order.

Prime may have at most one queued DRM frame in flight for the selected CRTC.

For a non-empty `DrmOutput::render_frame` result Prime must:

1. inspect `RenderFrameResult::needs_sync()`;
2. when explicit synchronization is required, wait on the public swapchain sync point before KMS submission;
3. drop the render result before another render attempt;
4. queue exactly one frame with `DrmOutput::queue_frame(())`;
5. wait for a DRM `VBlank` for the retained output's exact CRTC;
6. call `DrmOutput::frame_submitted()` exactly once for that queued frame;
7. only then free the in-flight slot; send Wayland frame callbacks only when no newer surface commit requested a follow-up frame while the retired frame was in flight.

An empty render result is not queued. Prime keeps the frame request pending and retries no sooner than approximately one 16 ms retrace interval.

## Mapped-surface readiness gate

`frame_loop_ready=true` and `phase=FRAME_LOOP_READY` are earned only after a queued frame whose Smithay render-element list contained at least one actual XDG or WLR layer-shell render element is retired successfully by the matching DRM vblank. A Space/layer object without a committed renderable buffer is not sufficient.

A compositor startup black/fallback frame without mapped client content must not earn this phase.

`prime-compositor --probe` therefore does **not** prove `FRAME_LOOP_READY`; the live event loop and a mapped surface are required.

## Readiness fields

- `frame_loop_ready`: matching-vblank lifecycle has successfully retired at least one mapped-surface frame.
- `frame_in_flight`: exactly one queued selected-CRTC frame is awaiting retirement when true.
- `frames_queued`: monotonic count of successfully queued frames in the current process.
- `frames_submitted`: monotonic count of matching-vblank `frame_submitted()` successes.
- `mapped_surface_frames_submitted`: monotonic subset of submitted frames that contained at least one mapped XDG/layer-shell surface.

`frames_submitted` must never exceed `frames_queued`. `frame_in_flight=true` must never coexist with another queue attempt.

## Surface mapping

The retained physical output is mapped to Space at logical `(0,0)` in the single-output P1 baseline.

XDG toplevels remain Space elements. WLR layer-shell surfaces must be wrapped as Smithay desktop layer surfaces and mapped through `layer_map_for_output` for the same physical output. Layer destruction must unmap the corresponding desktop layer surface.

Layer geometry is arranged on mapping and commits before rendering.

## Input seam completed by FRAME

Once WLR layer surfaces are mapped, pointer hit-testing is ordered:

1. Overlay layer;
2. Top layer;
3. XDG window Space;
4. Bottom layer;
5. Background layer.

Hit-testing uses Smithay surface input regions and includes popups/subsurfaces (`WindowSurfaceType::ALL`).

Pointer button keyboard focus may enter a layer surface only when Smithay reports `can_receive_keyboard_focus()`. XDG focus resolves to the root surface.

This completes the layer-shell hit-testing obligation explicitly deferred by `PRIME_P1_WAYLAND_INPUT_V1.md`; it does not add touch/tablet/gesture input.

## Failure semantics

A render/queue/retirement error sets `frame_loop_ready=false`, clears in-flight truth, records `FRAME_ERROR`, and requires restart or explicit revalidation.

Session pause, DRM/output invalidation, unreconciled topology change, or fatal Wayland protocol failure clears frame-loop readiness and in-flight truth. A later libinput-only resume cannot restore frame readiness.

An unrelated CRTC vblank must not retire the selected output's in-flight frame. If a newer commit arrives while a frame is in flight, Prime must retain that follow-up request and must not satisfy its frame callback from the older frame's vblank; callbacks are deferred until the newer queued frame retires.

## Explicitly unearned

`FRAME_LOOP_READY` does not mean:

- Prime Shell is running or has shown its first frame;
- `shell_ready=true`;
- touch/tablet/gesture input;
- absolute pointer routing;
- client dmabuf direct scanout;
- presentation-time feedback accuracy;
- multi-output layout/hotplug reconciliation;
- variable-refresh or frame pacing policy;
- owner visual acceptance.
