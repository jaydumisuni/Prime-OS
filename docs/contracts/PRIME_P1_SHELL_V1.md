# Prime P1 Shell v1

Status: **CONSTRUCTION DRAFT — NOT PRODUCT AUTHORITY**

Depends on: `PRIME_P1_FRAME_LOOP_V1.md` acceptance and promotion.

## Purpose

Define the minimum Prime-owned user experience required for P1 First Light without expanding into P2 component delivery, Windows/Android personalities, Origins/Ptah integration, or a generic desktop-environment replacement.

This contract refines the already-frozen Prime Shell requirements in `PRIME_OS_MASTER_PLAN.md`, `PRIME_OS_ROADMAP.md`, `PRIME_P1_SHELL_COMPOSITOR_V1.md`, and `ADR-0001-P1-SYSTEM-FOUNDATION.md`. Those documents remain authoritative if this draft conflicts with them.

## Ownership

Prime Shell is a Prime-owned user-space client of the Prime compositor and Prime Core.

P1 implementation structure:

- `prime-compositor` remains Wayland/window/system-surface authority;
- `prime-shell-host` is the lightweight Rust native Shell host;
- `shell-ui/` contains React + TypeScript rich Shell surfaces;
- rich surfaces use the system WebKit stack rather than Electron or a duplicated Chromium runtime;
- `primed` remains machine/Core authority and is consumed through `/run/prime/core.sock`;
- Shell does not move privileged display, workload, network, audio, power, update, or recovery authority out of their Prime owners.

## Required P1 surfaces

The first accepted Shell must provide unmistakable Prime identity and at minimum:

1. Prime background/startup surface;
2. Prime system rail/status surface;
3. Prime Orb/launcher baseline;
4. quick-controls surface with truthful network/audio/power/settings state;
5. visible windowing path for ordinary XDG application surfaces;
6. shutdown/restart entry points routed to Prime-owned authority;
7. recovery entry that remains available independently if Shell fails.

The visual gate remains owner-accepted and includes Prime glass/depth language, smooth core transitions, no obvious Fedora/GNOME/KDE/COSMIC identity, and reference-video comparison.

## Wayland roles

P1 Shell uses the compositor's existing Wayland protocols; it does not create a second compositor.

Privileged Shell surfaces use WLR layer-shell namespaces owned by Prime. The initial namespace family is reserved as:

- `prime.shell.background`
- `prime.shell.rail`
- `prime.shell.orb`
- `prime.shell.quick-controls`

Expected layer intent:

- background: `Background`, output-filling and non-exclusive;
- rail: `Top`, reserving its visible workspace edge while persistent;
- Orb/launcher: `Overlay`, persistent baseline entry surface;
- quick controls: `Overlay`, ephemeral and mapped only while open.

Exact final geometry, styling, motion, margins and visual dimensions remain Shell policy and owner visual-acceptance work. Construction constants used to prove protocol mechanics are not final visual authority.

WLR configure dimensions follow the protocol literally: a zero configure dimension means the client chooses that dimension. A fixed-size construction surface may therefore fall back to its requested construction dimension. A surface that deliberately delegated a dimension to the compositor must fail closed if neither compositor nor an already-earned size provides a truthful dimension.

## Persistent and ephemeral readiness surfaces

The persistent P1 Shell readiness set is:

- `prime.shell.background`;
- `prime.shell.rail`;
- `prime.shell.orb`.

All three must be live, configured, mapped and represented in an accepted retired Shell frame before `shell_ready=true` can be earned.

`prime.shell.quick-controls` is intentionally ephemeral. It does **not** need to remain mapped while closed. Before Shell readiness can become product authority, however, the accepted Shell candidate must have a proven quick-controls construction/open/close path and must not treat a closed quick-controls surface as Shell failure.

## Readiness

Process start is not Shell readiness.

`prime-compositor` may set `shell_ready=true` only after all of the following are true in one live compositor process:

1. `frame_loop_ready=true` has already been earned;
2. a live Wayland client owns the reserved persistent `prime.shell.*` surface set;
3. background, rail and Orb have been configured and mapped to the retained P1 output;
4. the accepted candidate has a proven quick-controls construction/open/close path, even if quick controls are currently closed;
5. a queued DRM frame containing the persistent mapped Prime Shell surfaces has retired successfully on the matching selected-CRTC vblank;
6. the Shell client remains alive after that retirement.

A startup process, socket connection, configure event, mapped object without a renderable buffer, generic non-Shell XDG client, or construction-only log marker must not set `shell_ready=true`.

If the Shell client exits, a required persistent surface disappears, Wayland protocol dispatch fails, the output/frame authority is invalidated, or session graphics revalidation is required, `shell_ready` must fail closed until re-earned.

## Shell host / UI boundary

The Rust host owns:

- Wayland connection and lifecycle;
- privileged layer-shell role creation;
- system WebKit view hosting;
- Core socket transport and version negotiation;
- capability/readiness handoff to the UI;
- input/activation plumbing;
- fail-closed handling when required Prime authority is unavailable.

React + TypeScript owns presentation and ordinary UI state for:

- rail rendering;
- Orb/launcher presentation;
- quick-control presentation;
- settings/navigation surfaces;
- motion/glass/depth presentation.

JavaScript must not directly acquire machine privileges. Mutations cross a typed Rust/Core boundary and are authorized by the owning Prime subsystem.

## Core data truth

P1 Shell displays only state that Prime currently owns and can report truthfully.

The existing `prime.system.status` capability may provide read-only observed network link, audio-device, power and thermal truth. Where a mutation backend has not yet earned authority, the corresponding control is visibly unavailable rather than simulated.

No UI state may imply NetworkManager/PipeWire/power mutation authority merely because the control is visually present.

## Failure semantics

Shell failure must not prevent:

- Prime Core operation;
- compositor failure reporting;
- recovery entry;
- system generation rollback/recovery;
- machine shutdown through a recovery/console path.

The Shell service may restart under systemd policy, but restart loops must not convert failure into readiness.

## Explicitly unearned by this contract

P1 Shell v1 does not claim:

- Prime Store/component delivery;
- Windows or Android personality UI;
- Origins/Hunter/Ptah integration;
- multi-output Shell policy;
- full accessibility implementation;
- touch/tablet/gesture completion;
- presentation-time/VRR policy;
- final geometry or styling from construction constants;
- owner visual acceptance before the owner performs it.

## Construction acceptance sequence

`FRAME accepted → Shell host/UI construction → static/build proof → compositor/Shell readiness integration proof → physical first-frame proof on the P1 host → owner visual gate → selective promotion`.
