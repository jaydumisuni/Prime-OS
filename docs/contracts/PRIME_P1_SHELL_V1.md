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

Orb/launcher and quick controls remain required P1 capabilities, but they are transient overlays. They must have separately proven deterministic open/close and input behavior before the P1 Shell can become product authority, but `shell_ready` must not require them to remain mapped while closed.

A future Orb/quick-controls construction slice must not weaken the persistent background+rail readiness boundary.

## Configure semantics

WLR layer-shell configure dimensions are interpreted literally: a zero configure dimension means the client chooses that dimension.

A fixed construction dimension may fall back only to a dimension the client explicitly requested. A compositor-owned dimension with no truthful client-side fallback must fail closed rather than inventing geometry.

For the current baseline:

- background requests compositor-owned width and height, so both must be supplied before it draws;
- rail requests compositor-owned width and a provisional fixed height, so only the height may fall back to that requested construction value.

Every accepted configure redraws the affected static SHM surface. Process start, role creation, configure receipt, or a construction log marker does not earn compositor Shell readiness.

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
- Orb/launcher input lifecycle;
- quick-controls lifecycle;
- final geometry, styling, animation or glass/depth treatment;
- system WebKit rich-surface integration;
- systemd Shell service/restart policy;
- owner visual acceptance;
- multi-output Shell policy;
- reserved namespace authentication beyond the current same-client mechanical readiness boundary;
- accessibility/touch/tablet/gesture completion.

## Construction acceptance sequence

`FRAME authority → persistent background+rail mechanics → static/build proof → exact readiness integration proof → Orb + quick-controls mechanics → physical Shell-frame retirement on the P1 host → owner visual gate → selective product promotion`.
