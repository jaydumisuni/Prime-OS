# Prime Compositor Readiness v1

Status: **FROZEN P1 READINESS CONTRACT — IMPLEMENTATION PROOF REQUIRED**

Schema identifier: `prime.compositor-readiness.v1`

Authority: `docs/contracts/PRIME_P1_SHELL_COMPOSITOR_V1.md`

## Purpose

Prime must distinguish compositor process existence from actual graphical responsibility. This contract defines the first machine-readable readiness record for `prime-compositor` so Prime Core, proof tooling and later Prime Shell integration can reason about what the compositor has mechanically earned.

The first implementation phase is deliberately named:

```text
BACKEND_PREFLIGHT
```

It proves direct-TTY backend prerequisites only. It does **not** claim a functioning Wayland desktop, renderer, configured display output, or Prime Shell.

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
  "phase": "BACKEND_PREFLIGHT",
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
  "renderer_ready": false,
  "outputs_ready": false,
  "shell_ready": false,
  "clients_accepted": 0,
  "input_events_seen": 0,
  "last_udev_event": null,
  "limitations": []
}
```

The concrete values above are illustrative; only the field semantics are normative.

## BACKEND_PREFLIGHT requirements

`phase=BACKEND_PREFLIGHT` may be persisted only after all of the following initialization work succeeds:

1. Smithay `LibSeatSession` is created successfully;
2. the active seat name is recovered from that session;
3. at least one DRM GPU is enumerated for the same seat;
4. a primary DRM GPU path is selected;
5. the selected path is accepted as a Smithay `DrmNode`;
6. the selected DRM node is actually opened through `Session::open` on the libseat session using the nonblocking/CLOEXEC flags used by Smithay's direct-Udev reference path, then closed successfully;
7. `UdevBackend` is created for the same seat and its initial DRM device snapshot is non-empty;
8. libinput is created through `LibinputSessionInterface<LibSeatSession>` and assigned to the same seat;
9. a Wayland listening socket is created;
10. the Wayland socket, display source, libinput source, libseat notifier and udev monitor are registered with calloop;
11. the readiness record can be serialized and atomically persisted.

Failure of any required step is startup failure, not degraded success.

## Field semantics

### `direct_tty_backend`

`true` means the implementation is using the Smithay direct Linux session/Udev/DRM/libinput path rather than the nested winit/X11 construction backends. It does not mean scanout is configured.

### `seat_name`

The seat returned by the active libseat session. The same value must be used for GPU enumeration, UdevBackend and libinput seat assignment.

### `primary_gpu`

The selected DRM device path for the current seat. This is runtime evidence only and is not Prime Host identity.

### `gpu_count`

The number of GPU device paths returned by Smithay `all_gpus` for the active seat during startup.

P1 remains a single-GPU rendering target even if discovery observes more than one GPU. Multi-GPU rendering is not implied by this count.

### `udev_device_count`

Current count of DRM devices known to Smithay's `UdevBackend`.

The initial value comes from `device_list()`. Smithay documents that the event source emits only subsequent changes, so later `Added`/`Removed` events may update this count without double-counting the initial snapshot.

### `drm_access_ready`

May be `true` only after Prime successfully opens the selected DRM node through the active libseat `Session::open` path and closes the returned descriptor.

Merely seeing `/dev/dri/card*`, successfully calling `stat`, or constructing `DrmNode` is insufficient.

This field proves seat-mediated device access. It does **not** prove DRM master acquisition, modesetting, GBM allocation, EGL initialization, GLES rendering or scanout.

### `libinput_bound`

May be `true` only after libinput's udev context accepts the same seat through `udev_assign_seat`.

Input behavior is still proven separately by actual input events.

### `session_active`

Reflects the current libseat activation state known by the compositor. Pause/activate notifications update this field. On pause, libinput is suspended; on activation, Prime attempts to resume it.

### `wayland_listener_ready`

May be `true` once `ListeningSocketSource::new_auto()` succeeds and the listener is registered with calloop.

A listening socket alone is not a usable desktop protocol implementation.

### `wayland_protocols_ready`

Must remain `false` in BACKEND_PREFLIGHT. It may become `true` only after the required Wayland compositor/shell/input/output globals and dispatch state for the next P1 slice are initialized and proven.

### `renderer_ready`

Must remain `false` in BACKEND_PREFLIGHT. It may become `true` only after the selected P1 single-GPU GBM/EGL/GLES renderer initializes successfully on the exact active device/session path.

### `outputs_ready`

Must remain `false` in BACKEND_PREFLIGHT. It requires a later DRM connector/CRTC/output configuration slice and actual output proof.

### `shell_ready`

Owned by the Prime Shell integration gate. The compositor must never infer this merely from its own process health.

### `clients_accepted`

Monotonic count of Wayland client streams successfully accepted into the display since this compositor process started.

It is intentionally **not** named `connected_clients`: the current P1 callback does not maintain an exact live disconnect count, so that stronger claim would be false.

### `input_events_seen`

Monotonic count of libinput events delivered to the compositor process since startup. Event count alone does not imply full keyboard/pointer semantics.

### `last_udev_event`

Diagnostic description of the most recent DRM add/change/remove event processed after startup. It is not durable hardware identity.

### `limitations`

Explicit list of responsibilities not yet earned or currently degraded. During BACKEND_PREFLIGHT the record must state that Wayland protocol globals, renderer, DRM outputs and Prime Shell are not ready.

## Wayland display ownership

The Wayland `Display` object is owned by its calloop event source, matching Smithay v0.7.0's Smallvil pattern. Client dispatch and output flushing are performed through that owned `Display` inside the event callback.

`DisplayHandle` is retained only for handle-level operations such as inserting accepted clients; it is not treated as owning the display or as exposing `flush_clients()`.

## Udev event truth

Smithay v0.7.0 documents `UdevBackend::device_list()` as the initial snapshot and the inserted event source as subsequent changes only. Prime therefore seeds `udev_device_count` once and adjusts it only on later `Added` and `Removed` events.

## Probe mode

`prime-compositor --probe` executes the same backend initialization path through readiness persistence, prints the resulting JSON readiness object, and exits before entering the long-running dispatch loop.

Probe success means only BACKEND_PREFLIGHT success. In particular it does not set:

- `wayland_protocols_ready`;
- `renderer_ready`;
- `outputs_ready`;
- `shell_ready`.

This mode exists so Kratos can prove the least-privileged libseat/device/session boundary before Prime enables a permanent compositor service.

## Service boundary

P1 does not enable a permanent `prime-compositor` systemd service merely because this record can compile.

The least-privileged libseat backend/session model must first be proven on the HP 290 G4 / Kratos Host. Prime must not use root as an implementation shortcut if seatd/logind/libseat mediation can provide the required device access.

## P1 promotion gates

Before BACKEND_PREFLIGHT code may be promoted into the product candidate:

- exact Smithay graph remains locked;
- source passes Rust 1.97.1 rustfmt;
- Clippy passes with `-D warnings`;
- release build succeeds on the exact Fedora base lock;
- the exact runtime library owners are recovered from the produced binary;
- construction-only scripts/workflows do not leak into the product image;
- Kratos later proves `--probe` through the intended least-privileged session/device boundary.

Hosted construction proves buildability only. It is not physical graphics acceptance.

## Non-claims

BACKEND_PREFLIGHT does not claim:

- DRM master/modesetting success;
- GBM allocator success;
- EGL/GLES renderer success;
- a configured output;
- frame rendering or scanout;
- Wayland compositor protocol completeness;
- keyboard/pointer behavior completeness;
- Prime Shell or Orb;
- service credential acceptance;
- multi-GPU rendering;
- XWayland, X11, winit, Vulkan or Pixman support;
- HP/Kratos physical graphics acceptance.
