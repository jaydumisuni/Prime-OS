# Prime P1 Wayland Protocols v1

Status: **FROZEN P1 PROTOCOL CONTRACT — CLIENT/PHYSICAL PROOF REQUIRED**

Authority: `docs/contracts/PRIME_P1_SHELL_COMPOSITOR_V1.md` and `docs/contracts/PRIME_COMPOSITOR_READINESS_V1.md`

## Purpose

Prime already earns a direct Linux graphics body through `OUTPUTS_READY`: libseat/session ownership, DRM/GBM/EGL/GLES rendering state, one real KMS output, a Wayland display/socket and calloop ownership.

That is intentionally not enough to advertise `wayland_protocols_ready=true`.

This contract defines the smallest server-side Wayland protocol responsibility required before `prime-shell` or an ordinary native Wayland application can establish the surface roles Prime P1 actually intends to support.

The protocol milestone is:

```text
OUTPUTS_READY
      ↓
WAYLAND_PROTOCOLS_READY
```

It is a protocol/global/dispatch milestone, not a rendering, input, Shell or visual-acceptance milestone.

## Required P1 protocol globals

`WAYLAND_PROTOCOLS_READY` may be published only after the compositor owns and retains all of the following Smithay v0.7.0 server state on the same `DisplayHandle` used by the accepted Wayland listener:

1. `CompositorState` for the core `wl_compositor`/subsurface surface lifecycle;
2. `ShmState` so P1 clients have a baseline shared-memory buffer path without requiring dmabuf first;
3. `OutputManagerState::new_with_xdg_output`;
4. a published Wayland global for the already-selected physical Smithay `Output` retained by `OUTPUTS_READY`;
5. `XdgShellState` for ordinary P1 application toplevel and popup roles;
6. `WlrLayerShellState` for Prime Shell desktop/background/rail/Orb layer surfaces.

No new Smithay feature is required. These facilities are already part of Prime's locked `wayland_frontend`/`desktop` graph.

## Internal Smithay seat state is not a public seat claim

Compiler evidence from Smithay v0.7.0 establishes that `delegate_xdg_shell!(Runtime)` requires `Runtime: SeatHandler` even when P1 does not publish a Wayland seat global.

Prime therefore retains one internal `SeatState<Runtime>` and implements `SeatHandler` with `WlSurface` focus target types solely to satisfy Smithay's XDG dispatch type contract.

This internal state must **not** be confused with a public input responsibility. This phase deliberately does not:

- call `SeatState::new_seat`;
- register `delegate_seat!`;
- create a `wl_seat` global;
- add keyboard, pointer or touch capabilities;
- route libinput events into Wayland seat focus/input delivery.

Those are later input responsibilities. The presence of `SeatState<Runtime>` alone does not permit Prime to claim a public Wayland seat or input readiness.

## Client compositor state

Every accepted client must carry Smithay `CompositorClientState` in its `ClientData`.

`CompositorHandler::client_compositor_state` must recover that exact state from the accepted client rather than inventing a parallel surface/client registry.

## Required handler/delegate responsibility

Before readiness is published, the compositor must implement and register the Smithay dispatch/delegate surface required by the globals above:

- `SeatHandler` with internal `SeatState<Runtime>` only;
- `CompositorHandler` and `BufferHandler`;
- `ShmHandler`;
- `OutputHandler`;
- `XdgShellHandler`;
- `WlrLayerShellHandler`;
- `delegate_compositor!`;
- `delegate_shm!`;
- `delegate_output!`;
- `delegate_xdg_shell!`;
- `delegate_layer_shell!`.

`delegate_seat!` is deliberately absent in this phase.

The Wayland `Display` remains owned by its calloop source. Client dispatch and flush remain inside that source callback.

## XDG application surface responsibility

The first protocol phase must accept ordinary XDG application roles without claiming full window management.

Minimum server behavior:

- retain a Smithay `Space<Window>`;
- create a `Window` for each new `ToplevelSurface`;
- map the new window into that internal Space at deterministic initial coordinates;
- retain a `PopupManager`;
- track new XDG popups;
- on surface commit, call Smithay's commit-buffer handler;
- deliver the initial XDG toplevel configure when it has not yet been sent;
- commit tracked popup state and deliver the initial popup configure;
- accept required XDG reposition/grab requests without inventing interactive move/resize behavior that P1 has not earned yet.

Interactive move, interactive resize, focus policy, stacking policy and real output rendering are later responsibilities.

## Prime Shell layer-surface responsibility

`prime-shell` is a separate compositor client. P1 therefore uses Smithay's existing WLR layer-shell protocol instead of moving Prime Shell UI into compositor authority.

The first protocol phase must:

- create a `WlrLayerShellState` global;
- accept new layer surfaces;
- retain the Smithay `LayerSurface` handles required for later rendering/layout ownership;
- send an initial configure to every accepted new layer surface.

This is enough to establish the compositor/client role contract for Prime desktop/background/system-rail/Orb surfaces.

It does **not** mean layer geometry, exclusive zones, keyboard interactivity, stacking or rendering are complete.

## Output publication

The selected physical `Output` already retained by `OUTPUTS_READY` must be published to Wayland clients only after `OutputManagerState` is initialized.

The published output must reflect the same selected connector/mode object that backs the retained DRM output. Prime must not fabricate a second virtual output merely to satisfy protocol initialization.

Publishing the output global does not weaken the current one-output P1 policy and does not earn hotplug reconciliation.

## Readiness transition

After all required state/global/delegate objects exist and the listener/display event sources are registered, Prime may publish:

```text
phase=WAYLAND_PROTOCOLS_READY
wayland_protocols_ready=true
outputs_ready=true
renderer_ready=true
drm_access_ready=true
shell_ready=false
```

The protocol state, including internal `SeatState<Runtime>`, must remain retained for the compositor process lifetime.

## Wayland protocol fail-closed rule

A fatal error from Wayland display dispatch/flush after `WAYLAND_PROTOCOLS_READY` invalidates the protocol claim.

Prime must at least:

- set `wayland_protocols_ready=false`;
- transition to `WAYLAND_PROTOCOL_ERROR`;
- persist an explicit Wayland protocol/dispatch limitation;
- not restore `wayland_protocols_ready=true` without explicit protocol revalidation or compositor restart through the full protocol initialization path.

A malformed or disconnected individual client must not be confused with a compositor-wide protocol failure when Wayland-server reports it as a normal client-local condition.

## Relationship to graphics invalidation

`WAYLAND_PROTOCOLS_READY` describes server globals/dispatch, not healthy scanout.

Therefore a later libseat pause, DRM notifier error or unreconciled DRM topology event may invalidate `drm_access_ready`, `renderer_ready` and/or `outputs_ready` while the protocol globals still mechanically exist.

Prime must not infer visual usability from `wayland_protocols_ready=true` when `outputs_ready=false`.

## Explicit P1 non-claims at this phase

`WAYLAND_PROTOCOLS_READY` does **not** claim:

- a public `wl_seat` global;
- keyboard delivery to Wayland clients;
- pointer delivery to Wayland clients;
- touch/tablet input;
- clipboard/data-device support;
- interactive window move/resize;
- focus or stacking correctness;
- mapped client surface rendering to the KMS output;
- frame scheduling/page-flip lifecycle;
- dmabuf client import/direct scanout;
- presentation-time/fractional-scale/viewporter/activation protocols;
- XDG decoration negotiation;
- layer-shell exclusive-zone/layout correctness;
- Prime Shell process existence;
- Prime Shell first frame;
- system rail or Orb behavior;
- owner visual acceptance;
- HP/Kratos physical client acceptance.

Those responsibilities must be earned separately rather than hidden behind this boolean.

## Construction proof gate

Before the protocol implementation may move into the product branch:

- build from the exact clean P1 product parent while the current Kratos proof SHA remains immutable;
- use exact Smithay `0.7.0` / release commit `a166cf4` APIs already locked by Prime;
- make no dependency/feature expansion unless evidence forces one;
- Rust `1.97.1` rustfmt must pass;
- Clippy must pass with `-D warnings`;
- the real `prime-compositor` release binary must build on the exact locked Fedora 44 substrate;
- dynamic-link closure and `prime-compositor --help` must remain valid;
- construction-only workflows/helpers must not be selectively promoted.

The accepted hosted construction candidate must also prove that adding the compiler-required internal `SeatState`/`SeatHandler` plumbing does not require dependency or Smithay feature expansion.

## Live/client proof gate

Hosted construction proves server implementation/buildability only.

Before the protocol phase can count toward P1 Host acceptance, the physical Kratos lane must additionally prove at least:

- a client can connect to the selected Wayland socket;
- the published physical output is visible to the client;
- an XDG toplevel can be created and receive initial configure;
- an XDG popup can complete initial configure when exercised;
- a WLR layer-shell client can create a layer surface and receive initial configure;
- the compositor remains truthful if the client disconnects or a protocol error occurs.

Rendering those surfaces, publishing a Wayland seat, routing input and running Prime Shell are subsequent gates.
