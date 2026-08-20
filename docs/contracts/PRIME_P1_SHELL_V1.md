# Prime P1 Shell v1

Status: **CONSTRUCTION DRAFT — NOT PRODUCT AUTHORITY**

Depends on the frozen Prime compositor/FRAME authority and current product head.

## Purpose

Define the minimum Prime-owned P1 Shell mechanics and readiness boundary without expanding into P2 component delivery, Windows/Android personalities, Origins/Ptah integration, or a generic desktop-environment replacement.

If this draft conflicts with `PRIME_OS_MASTER_PLAN.md`, `PRIME_OS_ROADMAP.md`, `PRIME_P1_SHELL_COMPOSITOR_V1.md`, or another already-frozen Prime contract, the frozen authority wins.

## Ownership

Prime Shell is a Prime-owned user-space Wayland client of `prime-compositor` and a consumer of Prime Core truth.

- `prime-compositor` retains display, mapping, focus, input-routing, frame and DRM retirement authority.
- `prime-shell` owns Shell-client lifecycle and privileged Prime layer-shell roles.
- `primed` remains machine/Core authority.
- Shell does not acquire privileged network, audio, power, update, recovery or workload authority merely because it presents controls.

## Required P1 capabilities

P1 requires:

1. Prime background/startup identity;
2. persistent system rail/status surface;
3. Prime Orb/launcher that opens and closes deterministically from accepted input;
4. quick-controls surface with truthful network/audio/power/settings state;
5. ordinary XDG windowing path;
6. shutdown/restart entry points routed to Prime-owned authority;
7. recovery entry independent of Shell health;
8. owner visual acceptance before Shell becomes product authority.

## Persistent readiness baseline

The persistent baseline is `prime.shell.background` plus `prime.shell.rail`.

Both surfaces must:

- be owned by the same live Wayland client;
- use their reserved namespaces and expected WLR layers;
- be uniquely identifiable on the retained P1 output;
- have renderable surface content at frame queue time;
- remain the exact same unique renderable surface identities when that queued frame retires on the matching selected-CRTC vblank.

A duplicate background or rail candidate makes baseline identity ambiguous and must not earn readiness, even if the duplicate has not attached renderable content yet.

`prime.shell.background` uses the WLR Background layer and does not reserve workspace area.

`prime.shell.rail` uses the WLR Top layer and may reserve its visible edge while persistent. Construction dimensions are provisional protocol-mechanics values, not final visual authority.

## Transient Shell capabilities

Orb/launcher and quick controls remain required P1 capabilities, but they are transient WLR Overlay surfaces. They do not participate in the persistent `shell_ready` identity and may be absent while closed.

The current interaction construction slice uses the existing Wayland seat authority rather than adding a second input stack:

- the persistent rail uses `KeyboardInteractivity::OnDemand` so the compositor may give it keyboard focus after accepted pointer interaction;
- a left-edge rail press opens `prime.shell.orb`;
- a right-edge rail press opens `prime.shell.quick-controls`;
- while the rail owns keyboard focus, `o` opens Orb and `q` opens quick controls;
- a focused Orb or quick-controls overlay closes on Escape;
- dropping the owned transient layer surface is the close transition, so the Wayland surface lifecycle is real rather than a hidden boolean.

The rail trigger widths, Orb dimensions, quick-controls dimensions, anchors and colors are provisional construction mechanics only. They are not final Prime visual authority.

Global keyboard accelerator and full launcher navigation remain unearned. This slice proves a keyboard-triggered open path only while the Shell rail already owns focus and a focused-overlay Escape close path. A later input/launcher integration slice must earn any global accelerator and complete keyboard navigation semantics without weakening compositor focus authority.

## Privileged action boundary

Transient UI existence does not grant mutation authority.

The current Prime Core launch seam is typed and privileged. Orb construction must therefore not use direct arbitrary process spawning and must not call the native-launch endpoint from an unprivileged UI shortcut. The Rust/Core authorization bridge is a separate milestone.

Until that bridge is earned:

- Orb activation is visibly/logically unavailable rather than faked;
- no application is launched directly from `prime-shell`;
- quick-controls mutations are unavailable;
- settings, network, audio, power, restart and shutdown mutations are not simulated;
- later rich UI may display only capability truth actually supplied by Prime Core.

## Configure semantics

WLR layer-shell configure dimensions are interpreted literally: a zero configure dimension means the client chooses that dimension.

A fixed construction dimension may fall back only to a dimension the client explicitly requested. A compositor-owned dimension with no truthful client-side fallback must fail closed rather than inventing geometry.

For the current baseline:

- background requests compositor-owned width and height, so both must be supplied before it draws;
- rail requests compositor-owned width and a provisional fixed height, so only the height may fall back to that requested construction value;
- transient Orb and quick-controls overlays request both construction dimensions, so zero configure dimensions may fall back to those explicit requests.

Every accepted configure redraws the affected static SHM construction surface. Process start, role creation, configure receipt, or a construction log marker does not earn compositor Shell readiness or visual acceptance.

## `shell_ready` invariant

`prime-compositor` may set `shell_ready=true` only when one queued DRM frame satisfies all of the following:

1. the existing FRAME lifecycle has earned `frame_loop_ready=true` on the matching selected CRTC;
2. exactly one mapped reserved `prime.shell.background` and exactly one mapped reserved `prime.shell.rail` exist on the retained output;
3. both are renderable, use their expected WLR layers, and are owned by the same live Wayland client;
4. their exact Wayland surface identities are frozen into the in-flight frame state at queue time;
5. when the matching vblank retires that frame, those exact identities are still the unique mapped reserved baseline and are still renderable in the expected layers;
6. no graphics/output/session/protocol/frame invalidation occurred that requires revalidation.

`SHELL_READY` is therefore a DRM-retirement claim, not a client-process or surface-mapping claim.

After `SHELL_READY` has been earned, an ordinary background/rail content replacement that leaves the same unique baseline live, correctly layered, same-client and renderable does not by itself revoke readiness. The compositor still queues the updated frame. This allows a live system rail to update without turning normal Shell presentation into a readiness failure.

Transient Orb/quick-controls creation or destruction does not revoke persistent Shell readiness unless it independently causes a compositor/frame/protocol failure.

## Fail-closed revalidation

Once `shell_ready=true`, it must be cleared immediately when any of the following occurs:

- a reserved persistent background/rail role is created or destroyed, changing baseline identity or uniqueness;
- a commit to the reserved persistent surface tree leaves the baseline missing, duplicate, wrong-layer, cross-client or non-renderable;
- frame-loop authority is invalidated;
- selected output authority is invalidated;
- renderer/session authority requires revalidation;
- compositor-wide Wayland protocol dispatch/flush authority is invalidated.

Creating a reserved persistent role also discards any in-flight Shell readiness proof token before that role has renderable content. A failed or invalid persistent commit likewise discards the token. Valid content replacement on the already-earned unique baseline does not needlessly discard readiness.

Whenever a persistent-Shell lifecycle event clears readiness, the persisted readiness artifact must be updated in the same callback so `/run/prime/compositor/readiness.json` cannot advertise stale `shell_ready=true` while in-memory authority is non-ready.

After invalidation, background+rail readiness is re-earned only by a later frame that passes the complete queue + matching-vblank identity test again.

## Explicitly unearned by this construction draft

This draft does not claim:

- live physical `SHELL_READY` proof on the P1 host;
- global Orb keyboard accelerator;
- full Orb application/profile inventory and keyboard navigation;
- admitted application launch through the typed privileged Prime Core bridge;
- truthful live quick-controls data/mutations;
- final geometry, styling, animation or glass/depth treatment;
- system WebKit rich-surface integration;
- systemd Shell service/restart policy;
- owner visual acceptance;
- multi-output Shell policy;
- reserved namespace authentication beyond the current same-client mechanical readiness boundary;
- accessibility/touch/tablet/gesture completion.

## Construction acceptance sequence

`FRAME authority → persistent background+rail mechanics → static/build proof → exact readiness integration proof → Orb + quick-controls mechanics → privileged Core bridge + truthful Shell data → physical Shell-frame proof on the P1 host → owner visual gate → selective product promotion`.
