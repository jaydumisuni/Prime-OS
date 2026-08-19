# Prime P1 Wayland Input v1

Status: **P1 IMPLEMENTATION CONTRACT — CONSTRUCTION PROOF REQUIRED**

Authority: `docs/contracts/PRIME_P1_WAYLAND_PROTOCOLS_V1.md`, `docs/contracts/PRIME_COMPOSITOR_READINESS_V1.md`, and `docs/contracts/PRIME_P1_SHELL_COMPOSITOR_V1.md`.

## Purpose

This contract defines the first public Wayland input authority required after `WAYLAND_PROTOCOLS_READY`. It is deliberately narrower than complete desktop input. P1 first earns one keyboard/relative-pointer seat sufficient for ordinary XDG application interaction; later frame/layout work may extend hit-testing to fully arranged Prime Shell layer surfaces.

## Exact Smithay v0.7.0 basis

The implementation uses the release APIs already frozen for the compositor:

- `SeatState::new_wl_seat(display, name)` to publish one `wl_seat`;
- `delegate_seat!(Runtime)` for Wayland seat/keyboard/pointer/touch object dispatch;
- `Seat::add_keyboard(XkbConfig, repeat_delay, repeat_rate)`;
- `Seat::add_pointer()`;
- `KeyboardHandle::input` and `KeyboardHandle::set_focus`;
- `PointerHandle::motion`, `button`, `axis`, and `frame`;
- libinput `InputEvent::{Keyboard,PointerMotion,PointerButton,PointerAxis}` as the P1 routed event classes.

The presence of `delegate_seat!` does not itself earn touch authority. Wayland seat capabilities are derived from the handles actually attached to the seat; P1 attaches keyboard and pointer only.

## P1 seat contract

Prime creates exactly one compositor `wl_seat` using the seat name returned by the active libseat session (normally `seat0`). That is a Wayland protocol identity, not Prime Host identity and not a physical-device identity.

The P1 keyboard baseline uses:

```text
XkbConfig::default()
repeat_delay = 200 ms
repeat_rate  = 25 Hz
```

Those values follow Smithay v0.7.0's documented keyboard initialization example and remain an implementation baseline, not a permanent user-settings policy.

## Routed events

P1 routes:

- keyboard key press/release;
- relative pointer motion;
- pointer buttons;
- vertical/horizontal pointer axis including v120 data when present.

Every routed keyboard/pointer event uses Smithay's serial/time mechanisms rather than fabricated client timestamps.

P1 does not yet claim:

- touch;
- tablet tools;
- gestures;
- absolute-position pointer devices;
- pointer constraints/relative-pointer protocol extensions;
- clipboard/data-device;
- compositor shortcuts;
- cursor rendering;
- complete layer-shell hit-testing before the frame/layout responsibility maps those surfaces.

Those event classes may still increment generic observed-input diagnostics when delivered by libinput, but they are not advertised as supported Wayland delivery.

## Focus policy

At this baseline, pointer hit-testing uses Prime's existing `Space<Window>` and the mapped XDG window surface tree. On pointer press, keyboard focus moves to the XDG surface under the current pointer when one exists.

The later frame/layout responsibility may expand the same policy to arranged WLR layer-shell surfaces. Prime must not claim Prime Shell pointer interaction before that geometry exists.

## Session semantics

On libseat pause:

- suspend libinput;
- set `session_active=false`;
- set `input_delivery_ready=false`;
- retain the Wayland seat/capability objects without pretending events can still be delivered.

On activation, a successful libinput resume may restore `input_delivery_ready=true`. Renderer/output truth remains separately fail-closed until the graphics revalidation responsibility runs.

A failed libinput resume leaves input delivery false. A fatal Wayland protocol dispatch/flush failure also invalidates input delivery.

## Readiness gate

`WAYLAND_INPUT_READY` may be reported only when all of the following are true:

```text
wayland_listener_ready=true
wayland_protocols_ready=true
wayland_seat_ready=true
keyboard_ready=true
pointer_ready=true
input_delivery_ready=true
```

It still requires:

```text
shell_ready=false
```

until Prime Shell itself starts and earns the later first-frame/interaction gate.

## Proof obligations

Before selective promotion, the exact construction candidate must pass the pinned Fedora 44 / Rust 1.97.1 compositor gate with whole-workspace rustfmt, all-target Clippy `-D warnings`, locked release build, dynamic-link closure, and `prime-compositor --help`.

Physical HP/Kratos acceptance later requires actual keyboard and pointer event evidence through the public seat. Hosted compile evidence proves only the implementation/API boundary.
