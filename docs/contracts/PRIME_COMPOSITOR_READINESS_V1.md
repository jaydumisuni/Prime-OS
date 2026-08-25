# Prime Compositor Readiness v1

Status: **FROZEN P1 READINESS CONTRACT — PHYSICAL PROOF REQUIRED**

Schema identifier: `prime.compositor-readiness.v1`

Authority: `docs/contracts/PRIME_P1_SHELL_COMPOSITOR_V1.md`

## Purpose

Prime must distinguish compositor process existence from actual graphical responsibility. This contract defines the machine-readable readiness record for `prime-compositor` so Prime Core, proof tooling and later Prime Shell integration can reason about what the compositor has mechanically earned.

P1 defines four earned initialization phases:

```text
BACKEND_PREFLIGHT
      ↓
RENDERER_READY
      ↓
OUTPUTS_READY
      ↓
WAYLAND_PROTOCOLS_READY
```

`BACKEND_PREFLIGHT` and `RENDERER_READY` do not claim a configured display output. `OUTPUTS_READY` claims only the P1 minimum physical KMS responsibility: one selected connected output has a real Smithay DRM surface/compositor initialized through the retained single-GPU renderer path.

`WAYLAND_PROTOCOLS_READY` extends that body with the minimum server-side protocol/global roles required for ordinary XDG application surfaces and Prime Shell layer surfaces. It still does not claim public Wayland seat/input delivery, client-surface rendering, Prime Shell, multi-output policy, hotplug reconciliation, or owner visual acceptance.

## Default record location

The compositor writes its latest readiness snapshot to:

```text
/run/prime/compositor/readiness.json
```

Construction and proof tooling may override that path with:

```text
PRIME_COMPOSITOR_READINESS
```

The readiness file is a runtime projection, not durable machine authority. It must not replace Prime Host, generation, Hardware Graph or health evidence under `/var/lib/prime`.

## Schema

The current P1 record contains:

```json
{
  "schema": "prime.compositor-readiness.v1",
  "observed_at": "RFC3339",
  "phase": "BACKEND_PREFLIGHT|RENDERER_READY|OUTPUTS_READY|WAYLAND_PROTOCOLS_READY|WAYLAND_INPUT_READY|FRAME_LOOP_READY|FRAME_ERROR|FRAME_MAPPING_ERROR|WAYLAND_PROTOCOL_ERROR|OUTPUT_ERROR|OUTPUT_REVALIDATION_REQUIRED|SESSION_PAUSED|RENDERER_REVALIDATION_REQUIRED|SESSION_RESUME_FAILED",
  "direct_tty_backend": true,
  "seat_name": "seat0",
  "wayland_socket": "wayland-0",
  "primary_gpu": "/dev/dri/card0",
  "gpu_count": 1,
  "udev_device_count": 1,
  "drm_access_ready": true,
  "libinput_bound": true,
  "session_active": true,
  "wayland_listener_ready": true,
  "wayland_protocols_ready": true,
  "wayland_seat_ready": false,
  "keyboard_ready": false,
  "pointer_ready": false,
  "input_delivery_ready": false,
  "frame_loop_ready": false,
  "frame_in_flight": false,
  "frames_queued": 0,
  "frames_submitted": 0,
  "mapped_surface_frames_submitted": 0,
  "renderer_ready": true,
  "outputs_ready": true,
  "shell_ready": false,
  "clients_accepted": 0,
  "input_events_seen": 0,
  "last_udev_event": null,
  "limitations": []
}
```

The concrete values above are illustrative; only field/phase semantics are normative.

## BACKEND_PREFLIGHT requirements

`phase=BACKEND_PREFLIGHT` may be persisted only after all of the following initialization work succeeds:

1. Smithay `LibSeatSession` is created successfully;
2. the seat name is recovered from that session;
3. at least one DRM GPU is enumerated for the same seat;
4. a primary DRM GPU path is selected;
5. the selected path is accepted as a Smithay `DrmNode`;
6. the selected DRM node is actually opened through `Session::open` using the nonblocking/CLOEXEC flags from Smithay's direct-Udev reference path;
7. backend-preflight-only implementations close that descriptor after proving seat-mediated access;
8. `UdevBackend` is created for the same seat and its initial DRM-device snapshot is non-empty;
9. libinput is created through `LibinputSessionInterface<LibSeatSession>` and assigned to the same seat;
10. a Wayland listening socket is created;
11. the Wayland socket, display source, libinput source, libseat notifier and udev monitor are registered with calloop;
12. the readiness record can be serialized and atomically persisted.

Failure of any required step is startup failure, not degraded success.

`BACKEND_PREFLIGHT` must keep:

```text
wayland_protocols_ready=false
renderer_ready=false
outputs_ready=false
shell_ready=false
```

## RENDERER_READY requirements

`phase=RENDERER_READY` extends BACKEND_PREFLIGHT with the single-GPU P1 renderer path and may be persisted only after all of the following additional conditions hold:

1. the libseat session is active at initialization time;
2. the selected primary DRM node is opened through that same active libseat session;
3. the returned descriptor remains owned by the compositor through Smithay `DeviceFd` / `DrmDeviceFd` rather than being closed after the access probe;
4. one Smithay `GbmDevice` is created from that DRM device;
5. an `EGLDisplay` is created on that GBM device;
6. an `EGLContext` is created with Smithay `ContextPriority::High`;
7. one Smithay `GlesRenderer` is created from that EGL context and retained by the compositor runtime.

This is deliberately the single-GPU path required for the P1 Intel UHD 630 proof Host. The Smithay multi-GPU manager is not part of the P1 minimum renderer responsibility.

`RENDERER_READY` permits:

```text
drm_access_ready=true
renderer_ready=true
```

but still requires:

```text
wayland_protocols_ready=false
outputs_ready=false
shell_ready=false
```

Renderer initialization proves that the selected seat/device path can construct Prime's GBM/EGL/GLES rendering body. It does **not** prove connector modesetting, CRTC/plane configuration, swapchain/allocator behavior, scanout, protocol globals, or Shell output.

## OUTPUTS_READY requirements

`phase=OUTPUTS_READY` extends `RENDERER_READY` with the P1 minimum physical KMS output responsibility. It may be persisted only after all of the following additional conditions hold on the same retained active session/device path:

1. `DrmDevice::new` succeeds on the retained libseat-owned `DrmDeviceFd` with connector state initialized through Smithay rather than by direct unmanaged ioctls;
2. DRM resource handles are read from that `DrmDevice`;
3. Prime deterministically selects one connector whose state is `Connected` and which exposes at least one mode;
4. the connector's preferred DRM mode is selected when it carries `ModeTypeFlags::PREFERRED`, otherwise the connector's first reported mode is used;
5. the connector's current encoder is preferred when usable, with deterministic numeric encoder fallback;
6. encoder compatibility is validated through `encoder.possible_crtcs()` and `ResourceHandles::filter_crtcs(...)`;
7. the current encoder CRTC is preferred when it is in that compatible set, otherwise deterministic numeric CRTC fallback is used;
8. a `GbmAllocator` is created from the same GBM device with `RENDERING | SCANOUT` buffer flags;
9. the retained GLES/EGL context reports at least one renderable dmabuf format;
10. P1 scanout format candidates are restricted to the conservative 8-bit `Abgr8888` and `Argb8888` set for this phase;
11. `GbmFramebufferExporter` is created with no client import node, so client direct-scanout remains disabled until Prime's later Wayland/dmabuf responsibility is proven;
12. a Smithay `Output` mode source is created for the selected connector and mode without exposing a Wayland output global;
13. the selected CRTC's DRM plane set is recovered from the same `DrmDevice`;
14. `DrmOutputManager::initialize_output` succeeds for the selected connector, CRTC, mode, planes, allocator/exporter and retained GLES renderer;
15. the initialized `DrmOutputManager`, `DrmOutput`, Smithay `Output` and renderer remain owned by the compositor runtime rather than being dropped after initialization;
16. the `DrmDeviceNotifier` returned with the DRM device is registered with calloop before readiness is published;
17. the readiness record can be persisted with `outputs_ready=true` only after the above ownership and event-source registration are complete.

Smithay v0.7.0's `DrmOutputManager::initialize_output` creates the real DRM surface/compositor and internally submits a composited fallback frame to establish the primary-plane format/bandwidth state. Prime therefore does not treat connector discovery alone as output readiness.

The construction path uses `DrmOutputRenderElements::default()`, whose allocator-backed fallback is black. This earns the KMS/output body without claiming Prime Shell pixels or client-surface rendering.

`OUTPUTS_READY` permits:

```text
drm_access_ready=true
renderer_ready=true
outputs_ready=true
```

but still requires:

```text
wayland_protocols_ready=false
shell_ready=false
```

P1 earns exactly one selected physical output in this phase. Multi-output layout, mirrored/extended policy, connector hotplug reconciliation, client dmabuf direct-scanout and continuous frame scheduling are later responsibilities and must not be inferred from `outputs_ready=true`.

## WAYLAND_PROTOCOLS_READY requirements

`phase=WAYLAND_PROTOCOLS_READY` extends `OUTPUTS_READY` with the P1 minimum server-side Wayland role contract. It may be persisted only after the compositor has created, registered and retained all of the following on the same accepted `DisplayHandle`:

1. Smithay `CompositorState`;
2. per-client `CompositorClientState` stored in every accepted client's `ClientData`;
3. Smithay `ShmState` for baseline shared-memory buffers;
4. `OutputManagerState::new_with_xdg_output`;
5. a published Wayland global for the same physical Smithay `Output` already retained by `OUTPUTS_READY`;
6. Smithay `XdgShellState` for ordinary application toplevel/popup roles;
7. Smithay `WlrLayerShellState` for Prime Shell layer-surface roles;
8. an internal `SeatState<Runtime>` and `SeatHandler` implementation required by Smithay's XDG dispatch type contract;
9. compositor/buffer, SHM, output, XDG-shell and WLR-layer-shell handler/delegate registration;
10. a retained `Space<Window>` for XDG toplevel tracking;
11. a retained `PopupManager` for XDG popup tracking;
12. initial configure handling for newly committed XDG toplevels/popups;
13. initial configure handling for accepted WLR layer surfaces;
14. Wayland display dispatch/flush remains owned by the existing calloop display source;
15. the readiness record can be persisted with `wayland_protocols_ready=true` only after all required server state is retained by the runtime.

The internal `SeatState<Runtime>` exists because Smithay v0.7.0's `delegate_xdg_shell!` requires `Runtime: SeatHandler`. It does **not** earn a public seat/input claim. In this phase Prime deliberately does not call `SeatState::new_seat`, does not register `delegate_seat!`, does not publish `wl_seat`, does not add keyboard/pointer/touch capabilities and does not route libinput events into Wayland client input delivery.

`WAYLAND_PROTOCOLS_READY` permits:

```text
wayland_protocols_ready=true
drm_access_ready=true
renderer_ready=true
outputs_ready=true
```

but still requires:

```text
shell_ready=false
```

This phase proves server roles and dispatch, not mapped client rendering, focus/input, clipboard/data-device, frame scheduling, Prime Shell execution or owner visual acceptance.

## Wayland protocol fail-closed rule

A fatal Wayland display dispatch or flush error after `WAYLAND_PROTOCOLS_READY` invalidates the protocol claim.

Prime must at least:

- set `wayland_protocols_ready=false`;
- transition to `WAYLAND_PROTOCOL_ERROR`;
- persist an explicit Wayland protocol/dispatch limitation;
- not restore `wayland_protocols_ready=true` without explicit protocol revalidation or compositor restart through the complete protocol initialization path.

Prime treats this conservatively at P1. Normal client-local disconnection must not be promoted into a stronger success claim; a compositor-wide display callback error fails protocol readiness closed.

## DRM notifier fail-closed rule

Smithay's P1 DRM notifier exposes `DrmEvent::VBlank(crtc)` and `DrmEvent::Error(error)`.

Before Prime implements its queued frame/vblank lifecycle, observing `VBlank` does not itself change readiness and does not earn a stronger phase.

A `DrmEvent::Error` after `OUTPUTS_READY` invalidates output truth. Prime must at least:

- set `outputs_ready=false`;
- transition to `OUTPUT_ERROR`;
- persist an explicit output/DRM limitation;
- not restore `outputs_ready=true` without re-running the output responsibility or another future evidence-equivalent revalidation path.

A DRM notifier error does not permit Prime to continue advertising a healthy configured output merely because the process and GLES renderer still exist.

The retained Wayland protocol globals may still mechanically exist when graphics output becomes invalid. `wayland_protocols_ready=true` must therefore never be interpreted as visual usability when `outputs_ready=false`.

## DRM topology fail-closed rule

P1 does not yet implement connector hotplug or DRM-topology reconciliation. Therefore a post-startup `UdevEvent::Changed` or `UdevEvent::Removed` invalidates the selected output claim even when the process and GLES renderer remain alive.

Prime must at least:

- retain the diagnostic `last_udev_event` value;
- update `udev_device_count` for removals;
- set `outputs_ready=false`;
- transition to `OUTPUT_REVALIDATION_REQUIRED`;
- persist an explicit topology/output revalidation limitation;
- not restore `outputs_ready=true` until the output responsibility is rerun or a future evidence-equivalent hotplug reconciliation path succeeds.

`UdevEvent::Added` may update discovery count and diagnostics without invalidating an already selected output; it does not itself earn multi-output correctness or alter P1's single-output policy.

This deliberately over-invalidates when a changed/removed DRM device is not the selected P1 device. P1 prefers stale-truth prevention over optimistic multi-device inference until device-specific hotplug ownership is implemented.

Wayland server globals may remain mechanically retained after graphics topology invalidation. That does not restore or substitute for `outputs_ready`.

## Session pause/resume fail-closed rule

A renderer or output that was valid before libseat deactivation is not automatically declared valid after activation.

On `PauseSession`, Prime must at least:

- suspend libinput;
- pause the retained `DrmOutputManager` when `OUTPUTS_READY` has been earned;
- set `session_active=false`;
- set `drm_access_ready=false`;
- set `renderer_ready=false`;
- set `outputs_ready=false`;
- transition the readiness phase to `SESSION_PAUSED`;
- record an explicit renderer/DRM/output revalidation limitation.

The already-created Wayland server state may continue to exist and `wayland_protocols_ready` may remain true because this flag describes retained server protocol/global responsibility, not current scanout health. Consumers must require `outputs_ready=true` for visual usability.

On `ActivateSession`, Prime may resume libinput. Until an explicit renderer/device/output revalidation mechanism is implemented and passes, Prime must **not** restore renderer or output truth optimistically. Successful input resume therefore transitions to:

```text
RENDERER_REVALIDATION_REQUIRED
```

with `drm_access_ready=false`, `renderer_ready=false` and `outputs_ready=false`.

A failed input resume transitions to:

```text
SESSION_RESUME_FAILED
```

and remains non-ready.

Restarting/reinitializing the compositor through the full renderer/output path is currently a valid way to earn the graphics phases again. A later hot revalidation slice may improve this, but must not weaken the fail-closed rule.

## Field semantics

### `phase`

Describes the strongest currently earned compositor initialization state, or an explicit invalidated session/output/protocol state. It is not a progress percentage.

### `direct_tty_backend`

`true` means the implementation is using the Smithay direct Linux session/Udev/DRM/libinput path rather than nested winit/X11 construction backends. It does not by itself mean scanout or client protocols are configured.

### `seat_name`

The seat returned by the libseat session. The same value must be used for GPU enumeration, UdevBackend and libinput seat assignment.

### `primary_gpu`

The selected DRM device path for the current seat. This is runtime evidence only and is not Prime Host identity.

### `gpu_count`

The number of GPU device paths returned by Smithay `all_gpus` for the active seat during startup.

P1 remains a single-GPU rendering target even if discovery observes more than one GPU. Multi-GPU rendering is not implied by this count.

### `udev_device_count`

Current count of DRM devices known to Smithay's `UdevBackend`.

The initial value comes from `device_list()`. Smithay v0.7.0 emits only subsequent changes from the registered event source, so later `Added`/`Removed` events may update this count without double-counting the initial snapshot.

### `drm_access_ready`

May be `true` only when the current readiness phase has earned seat-mediated access to the selected DRM node. BACKEND_PREFLIGHT proves an actual open/close through libseat. RENDERER_READY and later graphics-capable phases retain the opened device through `DrmDeviceFd` for renderer/KMS ownership.

Merely seeing `/dev/dri/card*`, successfully calling `stat`, or constructing `DrmNode` is insufficient.

The value must be invalidated on session pause and must not be restored on activation without explicit revalidation.

### `libinput_bound`

May be `true` only after libinput's udev context accepts the same seat through `udev_assign_seat`.

Input behavior is still proven separately by actual input events. This field does not mean a Wayland `wl_seat` exists or that clients receive input.

### `session_active`

Reflects the current libseat activation state known by the compositor. Pause/activate notifications update this field. On pause, libinput is suspended; on activation, Prime attempts to resume it.

### `wayland_listener_ready`

May be `true` once `ListeningSocketSource::new_auto()` succeeds and the listener is registered with calloop.

A listening socket alone is not a usable desktop protocol implementation.

### `wayland_protocols_ready`

Must remain `false` through BACKEND_PREFLIGHT, RENDERER_READY and OUTPUTS_READY.

It may become `true` only after the server responsibilities frozen by `PRIME_P1_WAYLAND_PROTOCOLS_V1.md` are initialized and retained: compositor/client state, SHM, publication of the existing physical output, XDG application roles, WLR layer-shell roles, required dispatch/delegates, and the compiler-required internal `SeatState<Runtime>`/`SeatHandler` plumbing.

It explicitly does **not** mean a public `wl_seat`, keyboard/pointer/touch delivery, clipboard/data-device, client-surface rendering, frame scheduling or Prime Shell is ready.

A fatal display dispatch/flush error invalidates this field as `WAYLAND_PROTOCOL_ERROR`. Graphics/session invalidation may leave this field true while `outputs_ready=false`; consumers must not infer visual usability from protocol readiness alone.

### `wayland_seat_ready`

May become `true` only after `SeatState::new_wl_seat` has published exactly one Prime Wayland seat using the active libseat session's seat name and `delegate_seat!(Runtime)` is registered. It does not by itself imply any input capability; capabilities are separate fields below.

### `keyboard_ready`

May become `true` only after the public seat owns a live Smithay keyboard handle initialized with the P1 XKB/repeat baseline. It does not imply that a client is focused or that a key event has already occurred.

### `pointer_ready`

May become `true` only after the public seat owns a live Smithay pointer handle. P1's first pointer authority covers relative motion, button and axis delivery only; unsupported absolute/touch/tablet/gesture classes remain limitations.

### `input_delivery_ready`

May become `true` only at `WAYLAND_INPUT_READY` after the bounded keyboard/relative-pointer libinput classes are routed into the public Wayland seat. It must be false when libinput is suspended, resume fails, or Wayland display dispatch/flush fails.

### `frame_loop_ready`

May become `true` only after a non-empty render containing at least one actual mapped XDG/WLR render element is queued and retired by the matching selected-CRTC DRM vblank through `DrmOutput::frame_submitted()`.

### `frame_in_flight`

True only while one selected-CRTC frame queued by `DrmOutput::queue_frame(())` awaits retirement. Prime P1 permits at most one such frame.

### `frames_queued`, `frames_submitted`, `mapped_surface_frames_submitted`

Current-process monotonic frame evidence counters. `frames_submitted <= frames_queued`; `mapped_surface_frames_submitted <= frames_submitted`. The mapped-surface counter advances only when the retired frame's Smithay render-element list was non-empty.

A successful libinput resume may restore this field while graphics remain in `RENDERER_REVALIDATION_REQUIRED`, because input delivery and renderer/output validation are separate authorities.

### `renderer_ready`

May be `true` only in a phase that has earned the selected P1 single-GPU GBM/EGL/GLES renderer on the exact current session/device path.

It is invalidated on session pause and is not automatically restored merely because libseat later reports activation.

### `outputs_ready`

Must remain `false` through RENDERER_READY.

It may become `true` only in an earned output phase that has a retained Smithay DRM device/output-manager/output ownership chain for a real selected connector/CRTC/mode and has completed `DrmOutputManager::initialize_output` successfully.

It does not mean client surfaces, Shell pixels, multi-output policy or owner visual acceptance are ready.

It is invalidated by session pause, a DRM notifier error, or a post-startup DRM topology `Changed`/`Removed` event and must not be restored without explicit revalidation.

### `shell_ready`

Owned by the Prime Shell integration gate. The compositor must never infer this merely from its own process, renderer, KMS output or protocol-global health.

### `clients_accepted`

Monotonic count of Wayland client streams successfully accepted into the display since this compositor process started.

It is intentionally **not** named `connected_clients`: the current P1 callback does not maintain an exact live disconnect count, so that stronger claim would be false.

### `input_events_seen`

Monotonic count of libinput events delivered to the compositor process since startup. Event count alone does not imply Wayland keyboard/pointer/touch delivery or complete input semantics.

### `last_udev_event`

Diagnostic description of the most recent DRM add/change/remove event processed after startup. It is not durable hardware identity.

### `limitations`

Explicit list of responsibilities not yet earned or currently degraded.

BACKEND_PREFLIGHT must report that protocol globals, renderer, DRM outputs and Prime Shell are not ready.

RENDERER_READY removes only the renderer limitation; it must continue to report protocol/output/Shell limitations.

OUTPUTS_READY removes only the DRM-output limitation; it must continue to report protocol/Shell limitations and must not imply client surface or Prime Shell rendering.

WAYLAND_PROTOCOLS_READY removes the server protocol/global limitation. It must continue to report at least the unearned Wayland input-delivery, client-surface rendering and Prime Shell responsibilities.

Session invalidation adds an explicit graphics revalidation limitation until renderer/device/output readiness is actually re-earned. Output notifier failure adds an explicit DRM/output limitation until output responsibility is re-earned. DRM topology change/removal adds an explicit topology/output limitation until output responsibility is re-earned. Fatal Wayland display dispatch/flush failure adds an explicit protocol limitation until protocol responsibility is re-earned.

## Wayland display ownership

The Wayland `Display` object is owned by its calloop event source, matching Smithay v0.7.0's Smallvil pattern. Client dispatch and output flushing are performed through that owned `Display` inside the event callback.

`DisplayHandle` is retained only for handle-level operations such as inserting accepted clients and constructing protocol globals; it is not treated as owning the display or as exposing `flush_clients()`.

`OUTPUTS_READY` does not create or expose a Wayland output global. At `WAYLAND_PROTOCOLS_READY`, `OutputManagerState` is initialized and the same retained physical Smithay `Output` is published to clients. Prime must not fabricate a second virtual output merely to satisfy protocol readiness.

## WAYLAND_INPUT_READY requirements

`phase=WAYLAND_INPUT_READY` extends `WAYLAND_PROTOCOLS_READY` with the bounded P1 keyboard/pointer input authority. It may be persisted only after Prime has:

1. created one public `wl_seat` from the existing `SeatState<Runtime>` using the same seat name returned by the active libseat session;
2. registered Smithay `delegate_seat!(Runtime)` dispatch;
3. attached exactly keyboard and pointer capabilities to that seat;
4. initialized the keyboard with `XkbConfig::default()`, repeat delay `200 ms`, and repeat rate `25 Hz`;
5. retained the resulting keyboard and pointer handles for runtime delivery;
6. routed libinput keyboard events through `KeyboardHandle::input` with Smithay serial/time evidence;
7. routed relative pointer motion, pointer button and pointer axis events through the retained pointer handle;
8. changed keyboard focus on pointer press to the mapped XDG window root under the pointer when one exists;
9. clamped relative pointer movement to the current P1 output mode; and
10. kept touch, tablet, gestures, absolute-position pointer routing and layer-shell pointer hit-testing explicit limitations rather than inventing support.

At this phase the readiness record may report:

```text
wayland_protocols_ready=true
wayland_seat_ready=true
keyboard_ready=true
pointer_ready=true
input_delivery_ready=true
shell_ready=false
```

`input_delivery_ready=true` means the P1 keyboard/relative-pointer event classes above have live Wayland delivery paths. It does not mean every libinput event family or every future Prime Shell surface role is supported.

A libseat pause or failed libinput resume must set `input_delivery_ready=false`. A successful libinput resume may restore input delivery because the advertised seat/capability handles survive the pause, even while renderer/output truth remains false until the separate graphics revalidation mechanism runs. A fatal Wayland protocol dispatch/flush error also invalidates input delivery.

## Internal seat-state ownership

Smithay v0.7.0 requires `Runtime: SeatHandler` for XDG-shell delegate dispatch. Prime therefore retains `SeatState<Runtime>` as internal Smithay handler state.

That state is not a public Wayland seat. At `WAYLAND_PROTOCOLS_READY` Prime does not publish a seat. `WAYLAND_INPUT_READY` is the later phase that earns one `wl_seat`, `delegate_seat!`, keyboard/pointer capabilities and the bounded event-routing contract above.

## Udev event truth

Smithay v0.7.0 documents `UdevBackend::device_list()` as the initial snapshot and the inserted event source as subsequent changes only. Prime therefore seeds `udev_device_count` once and adjusts it only on later `Added` and `Removed` events.

The initial P1 output phase is not a hotplug reconciliation implementation. `Changed` or `Removed` therefore fail the output claim closed as `OUTPUT_REVALIDATION_REQUIRED`; Prime does not continue advertising `outputs_ready=true` from stale startup state. A later connector/device-specific hotplug policy may avoid over-invalidation, but it must explicitly reconcile the selected output before Prime can claim dynamic-output correctness.

## Runtime readiness-file safety

Readiness persistence uses a unique create-new temporary file in the target runtime directory, explicit permissions, file synchronization, atomic rename and parent-directory synchronization. A failed write removes its temporary file where possible.

The file remains a runtime projection. These durability mechanics do not convert it into persistent Prime authority.

## Probe mode

`prime-compositor --probe` executes the same current initialization path through readiness persistence, prints the resulting JSON readiness object, and exits before entering the long-running dispatch loop.

At OUTPUTS_READY, probe initialization performs the real KMS path. After the protocol phase is product-side, probe initialization may also construct and retain the server protocol globals long enough to report `WAYLAND_PROTOCOLS_READY`; it still does not prove a client connected, received globals/configures, rendered pixels or received input.

Because the graphics portion requires real KMS initialization, `--probe` is not a passive metadata check: on the physical proof Host it may reset/configure the selected connector and establish the black allocator-backed fallback frame before exiting. Proof procedures must treat that as an intentional graphics-device action.

Live client acceptance for the protocol phase requires a separate Kratos/client proof, as frozen by `PRIME_P1_WAYLAND_PROTOCOLS_V1.md`.

## Service boundary

P1 does not enable a permanent `prime-compositor` systemd service merely because this code compiles or the binary is packaged in the image.

The least-privileged libseat backend/session model must first be proven on the HP 290 G4 / Kratos Host. Prime must not use root as an implementation shortcut if seatd/logind/libseat mediation can provide the required device access.

## P1 promotion gates

Before a new readiness phase may be promoted into the product candidate:

- exact Smithay graph remains locked;
- source passes Rust 1.97.1 rustfmt;
- Clippy passes with `-D warnings`;
- release build succeeds on the exact Fedora base lock;
- runtime linkage remains valid;
- construction-only scripts/workflows/probe binaries do not leak into product history;
- responsibility-specific fields fail closed when their underlying resource/session is invalidated.

Physical graphics acceptance additionally requires Kratos to execute `--probe` through the intended least-privileged session/device boundary and mechanically observe the selected physical output behavior.

Protocol Host acceptance additionally requires a real client to prove the published output and required XDG/layer-shell initial configure paths. Hosted construction proves buildability only.

## Non-claims

Even at `WAYLAND_PROTOCOLS_READY`, P1 does not claim:

- a public Wayland `wl_seat`;
- keyboard/pointer/touch delivery to Wayland clients;
- clipboard/data-device support;
- client surface composition to the physical output;
- continuous frame scheduling or vblank lifecycle completeness;
- client dmabuf direct-scanout;
- interactive window move/resize, focus or stacking correctness;
- layer-shell layout/exclusive-zone correctness;
- Prime Shell or Orb pixels;
- multi-output layout or hotplug reconciliation;
- service credential acceptance;
- multi-GPU rendering;
- XWayland, X11, winit, Vulkan or Pixman support;
- owner visual acceptance;
- HP/Kratos physical protocol/graphics acceptance until that proof is actually executed.


## FRAME_LOOP_READY requirements

`phase=FRAME_LOOP_READY` is governed by `PRIME_P1_FRAME_LOOP_V1.md` and extends `WAYLAND_INPUT_READY` only after Prime has successfully retired at least one queued frame containing a mapped XDG window or WLR layer-shell surface on the selected CRTC.

At this phase:

```text
wayland_protocols_ready=true
wayland_seat_ready=true
keyboard_ready=true
pointer_ready=true
input_delivery_ready=true
renderer_ready=true
outputs_ready=true
frame_loop_ready=true
frame_in_flight=false|true
shell_ready=false
mapped_surface_frames_submitted>=1
```

`frame_in_flight=true` means one and only one selected-CRTC `queue_frame(())` is awaiting its matching DRM vblank. `frames_queued`, `frames_submitted`, and `mapped_surface_frames_submitted` are current-process monotonic evidence counters; `frames_submitted` may not exceed `frames_queued`.

An empty render is never queued. A startup/fallback frame without mapped client content does not earn this phase. `--probe` exits before the live page-flip lifecycle and therefore cannot prove `FRAME_LOOP_READY`.

Frame failure is fail-closed at `FRAME_ERROR`. Layer mapping failure is fail-closed at `FRAME_MAPPING_ERROR`. Session pause, output invalidation/topology change, or fatal Wayland dispatch failure also clears frame readiness and in-flight truth. None of these frame-specific failures implicitly falsifies an otherwise retained physical-output object unless the corresponding output/graphics gate separately fails.

`FRAME_LOOP_READY` still leaves `shell_ready=false`; Prime Shell process ownership and first-frame acceptance remain a separate responsibility.
