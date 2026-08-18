# Prime Compositor Readiness v1

Status: **FROZEN P1 READINESS CONTRACT — PHYSICAL PROOF REQUIRED**

Schema identifier: `prime.compositor-readiness.v1`

Authority: `docs/contracts/PRIME_P1_SHELL_COMPOSITOR_V1.md`

## Purpose

Prime must distinguish compositor process existence from actual graphical responsibility. This contract defines the machine-readable readiness record for `prime-compositor` so Prime Core, proof tooling and later Prime Shell integration can reason about what the compositor has mechanically earned.

P1 currently defines two earned initialization phases:

```text
BACKEND_PREFLIGHT
      ↓
RENDERER_READY
```

Neither phase claims a configured display output, frame scanout, complete Wayland desktop protocol state, Prime Shell, or owner visual acceptance.

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
  "phase": "BACKEND_PREFLIGHT|RENDERER_READY|SESSION_PAUSED|RENDERER_REVALIDATION_REQUIRED|SESSION_RESUME_FAILED",
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
  "wayland_protocols_ready": false,
  "renderer_ready": true,
  "outputs_ready": false,
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

Renderer initialization proves that the selected seat/device path can construct Prime's GBM/EGL/GLES rendering body. It does **not** prove connector modesetting, CRTC/plane configuration, swapchain/allocator behavior, frame rendering, scanout, protocol globals, or Shell output.

## Session pause/resume fail-closed rule

A renderer that was valid before libseat deactivation is not automatically declared valid after activation.

On `PauseSession`, Prime must at least:

- suspend libinput;
- set `session_active=false`;
- set `drm_access_ready=false`;
- set `renderer_ready=false`;
- transition the readiness phase to `SESSION_PAUSED`;
- record an explicit renderer/DRM revalidation limitation.

On `ActivateSession`, Prime may resume libinput. Until an explicit renderer/device revalidation mechanism is implemented and passes, Prime must **not** restore renderer truth optimistically. Successful input resume therefore transitions to:

```text
RENDERER_REVALIDATION_REQUIRED
```

with both `drm_access_ready=false` and `renderer_ready=false`.

A failed input resume transitions to:

```text
SESSION_RESUME_FAILED
```

and remains non-ready.

Restarting/reinitializing the compositor through the full renderer path is currently a valid way to earn `RENDERER_READY` again. A later hot revalidation slice may improve this, but must not weaken the fail-closed rule.

## Field semantics

### `phase`

Describes the strongest currently earned compositor initialization state, or an explicit invalidated session state. It is not a progress percentage.

### `direct_tty_backend`

`true` means the implementation is using the Smithay direct Linux session/Udev/DRM/libinput path rather than nested winit/X11 construction backends. It does not mean scanout is configured.

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

May be `true` only when the current readiness phase has earned seat-mediated access to the selected DRM node. BACKEND_PREFLIGHT proves an actual open/close through libseat. RENDERER_READY retains the opened device through `DrmDeviceFd` for renderer ownership.

Merely seeing `/dev/dri/card*`, successfully calling `stat`, or constructing `DrmNode` is insufficient.

The value must be invalidated on session pause and must not be restored on activation without explicit revalidation.

### `libinput_bound`

May be `true` only after libinput's udev context accepts the same seat through `udev_assign_seat`.

Input behavior is still proven separately by actual input events.

### `session_active`

Reflects the current libseat activation state known by the compositor. Pause/activate notifications update this field. On pause, libinput is suspended; on activation, Prime attempts to resume it.

### `wayland_listener_ready`

May be `true` once `ListeningSocketSource::new_auto()` succeeds and the listener is registered with calloop.

A listening socket alone is not a usable desktop protocol implementation.

### `wayland_protocols_ready`

Must remain `false` through BACKEND_PREFLIGHT and RENDERER_READY. It may become `true` only after the required Wayland compositor/shell/input/output globals and dispatch state are initialized and proven.

### `renderer_ready`

May be `true` only in a phase that has earned the selected P1 single-GPU GBM/EGL/GLES renderer on the exact current session/device path.

It is invalidated on session pause and is not automatically restored merely because libseat later reports activation.

### `outputs_ready`

Must remain `false` through RENDERER_READY. It requires a later DRM connector/CRTC/output configuration slice and actual output proof.

### `shell_ready`

Owned by the Prime Shell integration gate. The compositor must never infer this merely from its own process or renderer health.

### `clients_accepted`

Monotonic count of Wayland client streams successfully accepted into the display since this compositor process started.

It is intentionally **not** named `connected_clients`: the current P1 callback does not maintain an exact live disconnect count, so that stronger claim would be false.

### `input_events_seen`

Monotonic count of libinput events delivered to the compositor process since startup. Event count alone does not imply complete keyboard/pointer semantics.

### `last_udev_event`

Diagnostic description of the most recent DRM add/change/remove event processed after startup. It is not durable hardware identity.

### `limitations`

Explicit list of responsibilities not yet earned or currently degraded.

BACKEND_PREFLIGHT must report that protocol globals, renderer, DRM outputs and Prime Shell are not ready.

RENDERER_READY removes only the renderer limitation; it must continue to report protocol/output/Shell limitations.

Session invalidation adds the explicit limitation:

```text
Renderer and DRM access require revalidation after session activation
```

until renderer/device readiness is actually re-earned.

## Wayland display ownership

The Wayland `Display` object is owned by its calloop event source, matching Smithay v0.7.0's Smallvil pattern. Client dispatch and output flushing are performed through that owned `Display` inside the event callback.

`DisplayHandle` is retained only for handle-level operations such as inserting accepted clients; it is not treated as owning the display or as exposing `flush_clients()`.

## Udev event truth

Smithay v0.7.0 documents `UdevBackend::device_list()` as the initial snapshot and the inserted event source as subsequent changes only. Prime therefore seeds `udev_device_count` once and adjusts it only on later `Added` and `Removed` events.

## Runtime readiness-file safety

Readiness persistence uses a unique create-new temporary file in the target runtime directory, explicit permissions, file synchronization, atomic rename and parent-directory synchronization. A failed write removes its temporary file where possible.

The file remains a runtime projection. These durability mechanics do not convert it into persistent Prime authority.

## Probe mode

`prime-compositor --probe` executes the same current initialization path through readiness persistence, prints the resulting JSON readiness object, and exits before entering the long-running dispatch loop.

Once the renderer slice is product-side, probe success may earn `RENDERER_READY`; it still must not set:

- `wayland_protocols_ready`;
- `outputs_ready`;
- `shell_ready`.

This mode exists so Kratos can prove the least-privileged libseat/device/GBM/EGL/GLES boundary before Prime enables a permanent compositor service.

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
- construction-only scripts/workflows do not leak into product history;
- responsibility-specific fields fail closed when their underlying resource/session is invalidated.

Physical graphics acceptance additionally requires Kratos to execute `--probe` through the intended least-privileged session/device boundary.

Hosted construction proves buildability only. It is not physical graphics acceptance.

## Non-claims

`RENDERER_READY` does not claim:

- connector discovery acceptance;
- DRM master/modesetting/atomic commit success;
- GBM scanout allocator/swapchain success;
- a configured output;
- frame rendering or scanout;
- Wayland compositor protocol completeness;
- keyboard/pointer behavior completeness;
- Prime Shell or Orb;
- service credential acceptance;
- multi-GPU rendering;
- XWayland, X11, winit, Vulkan or Pixman support;
- HP/Kratos physical graphics acceptance.
