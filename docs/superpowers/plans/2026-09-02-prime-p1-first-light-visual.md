# Prime P1 First Light Visual Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the mechanically proven P1 construction Shell with a physically accepted Prime-owned dark glass desktop, left rail, Orb, quick controls, window depth, real compositor backdrop blur, motion, and production typography while preserving truthful Core contracts and `SHELL_READY` semantics.

**Architecture:** Keep policy/content/interaction in `prime-shell` and scene-dependent effects in `prime-compositor`. Prime Shell renders transparent/tinted ARGB layer surfaces using a focused visual module tree and frame-callback motion; Prime Compositor identifies the existing Prime layer namespaces and inserts compositor-owned backdrop/shadow/focus elements into the bottom-up GLES render chain before the Shell surfaces are drawn. The proven commit `786018fd38a066e30144df869b9a8b2a2701381a` remains immutable; all work occurs on `design/p1-first-light-visual`.

**Tech Stack:** Rust 1.97.1, Smithay 0.7.0, smithay-client-toolkit 0.20.0, Wayland layer-shell/XDG, GLES2, `fontdue 0.9.4`, `fontdb 0.24.0`, Fedora 44 `google-noto-sans-fonts`, systemd/bootc proof harness, KRATOS Intel UHD 630 at 1920x1080@60 Hz.

**Spec:** `docs/superpowers/specs/2026-09-02-prime-p1-first-light-visual-design.md`

## Global Constraints

- Parent mechanical authority remains `786018fd38a066e30144df869b9a8b2a2701381a`; never rewrite or force-update that commit.
- Work only in `/home/kratos/prime-p1-visual-20260902` on `design/p1-first-light-visual`.
- Exact Rust toolchain remains `1.97.1`.
- Smithay remains exactly `0.7.0`; smithay-client-toolkit remains exactly `0.20.0`.
- No GNOME/KDE/GTK/COSMIC desktop shell or toolkit is introduced.
- Normal Shell color authority is near-black/navy + cyan + violet; semantic green/amber/red remain status-only.
- Real backdrop blur is compositor-owned. A tinted translucent fallback may preserve function but does not satisfy KRATOS owner visual acceptance.
- Existing Core application admission, status/storage projection, launch request, and double-confirmed power contracts remain unchanged.
- Existing readiness semantics remain truthful: `SHELL_READY` is earned only after the persistent background + rail baseline retires in a mapped DRM frame.
- Final Prime logo/mark is deferred; provisional text identity only.
- P1 physical visual target is KRATOS HP 290 G4, Intel UHD 630, HDMI-A-1, 1920x1080@60 Hz.
- No full-screen CPU blur and no continuous idle redraw.

---

## File Structure

### Prime Shell

- `crates/prime-shell/src/visual/mod.rs` — public visual entry points and shared render context.
- `crates/prime-shell/src/visual/theme.rs` — brand palette, spacing, radii, material/content tokens.
- `crates/prime-shell/src/visual/primitives.rs` — premultiplied-alpha blending, gradients, rounded geometry, strokes, circles and icon primitives.
- `crates/prime-shell/src/visual/text.rs` — system-font discovery, `fontdue` rasterization and glyph cache.
- `crates/prime-shell/src/visual/background.rs` — startup/desktop field.
- `crates/prime-shell/src/visual/rail.rs` — vertical rail paint and hit geometry.
- `crates/prime-shell/src/visual/orb.rs` — Orb launcher paint and application-row hit geometry.
- `crates/prime-shell/src/visual/quick_controls.rs` — truthful system cards and power-action hit geometry.
- `crates/prime-shell/src/motion.rs` — transition state, easing and Wayland frame-callback scheduling.
- `crates/prime-shell/src/main.rs` — surface lifecycle, Core data, keyboard/pointer actions and animation dispatch only.

### Prime Compositor

- `crates/prime-compositor/src/effects.rs` — Prime material detection, backdrop capture texture, blur shader, tint/edge/shadow element state.
- `crates/prime-compositor/src/frame.rs` — build ordered Prime render elements and retain existing frame/readiness lifecycle.
- `crates/prime-compositor/src/input.rs` — expose focus change to visual-depth state without changing input authority.
- `crates/prime-compositor/src/protocols.rs` — track focused XDG window and request re-render on map/focus changes only.
- `crates/prime-compositor/src/main.rs` — own `EffectsState` and readiness limitation if GPU effects fail.

### Image / proof

- `Cargo.toml`, `Cargo.lock`, `crates/prime-shell/Cargo.toml` — locked font dependencies.
- `image/Containerfile` — Fedora Noto Sans runtime package and link/package assertions.
- `tools/prove-p1-local.sh` — only if stronger visual-effect evidence can be added without weakening existing mechanical proof; otherwise leave canonical proof untouched and add a separate visual proof helper.
- `tools/prove-p1-visual-host.sh` — host-side bounded visual/mechanical checks before the manual KRATOS owner review.

---

### Task 1: Split the Shell renderer and establish Prime material primitives

**Files:**
- Replace: `crates/prime-shell/src/visual.rs`
- Create: `crates/prime-shell/src/visual/mod.rs`
- Create: `crates/prime-shell/src/visual/theme.rs`
- Create: `crates/prime-shell/src/visual/primitives.rs`
- Modify: `crates/prime-shell/src/main.rs`

**Interfaces:**
- Produces: `visual::RenderContext`, `theme::Theme`, `primitives::Canvas`, `primitives::Rect`, `primitives::Argb`.
- Preserves temporarily: `visual::paint_background`, `paint_rail`, `paint_orb`, `paint_quick_controls`, `orb_row_at`, `quick_power_action_at` so later tasks can migrate one surface at a time.

- [ ] **Step 1: Write failing primitive tests**

Add tests that prove premultiplied alpha composition and rounded-corner transparency:

```rust
#[test]
fn alpha_blend_preserves_opaque_destination() {
    let dst = Argb::from_u32(0xff050818);
    let src = Argb::from_u32(0x8022d3ee);
    let mixed = src.over(dst);
    assert_eq!(mixed.a, 255);
    assert!(mixed.b > dst.b);
    assert!(mixed.g > dst.g);
}

#[test]
fn rounded_rect_leaves_corner_outside_radius_untouched() {
    let mut bytes = vec![0u8; 32 * 32 * 4];
    let mut canvas = Canvas::new(&mut bytes, 32, 32).unwrap();
    canvas.fill_rounded_rect(Rect::new(0, 0, 32, 32), 10, Argb::from_u32(0xcc0f172a));
    assert_eq!(canvas.pixel(0, 0).unwrap().a, 0);
    assert!(canvas.pixel(16, 16).unwrap().a > 0);
}
```

- [ ] **Step 2: Run the focused tests and prove they fail before implementation**

Run:

```text
cargo test -p prime-shell visual::primitives --locked
```

Expected: compile failure because `Argb`, `Canvas`, and `Rect` do not exist yet.

- [ ] **Step 3: Implement the visual module split and primitives**

Implement:

```rust
pub struct Argb { pub a: u8, pub r: u8, pub g: u8, pub b: u8 }
pub struct Rect { pub x: i32, pub y: i32, pub width: u32, pub height: u32 }
pub struct Canvas<'a> { bytes: &'a mut [u8], width: u32, height: u32 }
```

Required primitives: `fill_rect`, `fill_rounded_rect`, `stroke_rounded_rect`, `vertical_gradient`, `radial_glow`, `circle`, `line`, `pixel`, `blend_pixel`. Preserve little-endian ARGB8888 buffer output expected by the existing `wl_shm::Format::Argb8888` path.

Define `Theme::prime_dark()` with exact core tokens:

```rust
base_0 = #05050d
base_1 = #050818
base_2 = #071021
panel = #0f172a
cyan = #22d3ee
cyan_alt = #06b6d4
violet = #8b5cf6
violet_alt = #a855f7
text = #f8fafc
muted = #94a3b8
```

Do not add animation or new geometry in this task.

- [ ] **Step 4: Run Shell tests and formatting**

Run:

```text
cargo test -p prime-shell --locked
cargo fmt --all -- --check
```

Expected: all existing power/hit tests plus new primitive tests PASS.

- [ ] **Step 5: Commit**

```text
git add crates/prime-shell/src/visual crates/prime-shell/src/visual.rs crates/prime-shell/src/main.rs
git commit -m "refactor(p1): establish prime visual material primitives"
```

---

### Task 2: Add anti-aliased production typography and Prime-owned geometric icons

**Files:**
- Modify: `Cargo.toml`
- Modify: `Cargo.lock`
- Modify: `crates/prime-shell/Cargo.toml`
- Create: `crates/prime-shell/src/visual/text.rs`
- Modify: `crates/prime-shell/src/visual/primitives.rs`
- Modify: `crates/prime-shell/src/visual/mod.rs`
- Modify later runtime package in Task 7: `image/Containerfile`

**Interfaces:**
- Produces: `TextSystem::load_system() -> Result<TextSystem, TextError>`.
- Produces: `TextSystem::measure(text, TextStyle) -> TextMetrics` and `TextSystem::draw(canvas, origin, text, TextStyle)`.
- Produces: `Icon` enum and `draw_icon(canvas, rect, Icon, color)`.

- [ ] **Step 1: Pin font dependencies**

Add workspace dependencies:

```toml
fontdb = "=0.24.0"
fontdue = "=0.9.4"
```

Add both via `.workspace = true` to `prime-shell`.

- [ ] **Step 2: Write failing typography-selection and alpha-raster tests**

Test family preference as a pure function so CI does not depend on installed desktop fonts:

```rust
#[test]
fn family_preference_is_noto_then_dejavu() {
    assert_eq!(preferred_families(), ["Noto Sans", "DejaVu Sans"]);
}

#[test]
fn glyph_coverage_blends_alpha_without_box_background() {
    let coverage = 128u8;
    let color = Argb::from_u32(0xfff8fafc);
    assert_eq!(coverage_color(color, coverage).a, 128);
}
```

- [ ] **Step 3: Implement `TextSystem`**

Load system fonts with `fontdb::Database::load_system_fonts()`. Query `Noto Sans` first, then `DejaVu Sans`. Copy the selected face bytes into owned memory using `Database::with_face_data`; build `fontdue::Font`. Cache rasterized glyphs by `(char, px_size, style_weight)` in a bounded `HashMap` owned by `TextSystem`.

Use fontdue coverage as alpha and blend through `Canvas::blend_pixel`. Provide regular and semibold visual styles; if the selected face database exposes only one weight, use regular rather than synthetically distorting glyphs.

On total font-load failure, return `TextError::NoUsableFont`; do not silently fall back to the old 5x7 font in the accepted Shell.

- [ ] **Step 4: Implement geometric system icons**

Create an `Icon` enum for `Orb`, `Applications`, `Status`, `Network`, `Audio`, `Storage`, `Health`, `Restart`, `Power`, `Search`, `Chevron`, and `Blocked`. Render from circles/lines/rounded geometry so icons are deterministic and do not depend on emoji/font glyphs.

- [ ] **Step 5: Run tests and locked build**

```text
cargo test -p prime-shell --locked
cargo build --release --locked -p prime-shell
```

Expected: PASS; normal Shell code can construct `TextSystem` and all icon primitives compile without a desktop toolkit dependency.

- [ ] **Step 6: Commit**

```text
git add Cargo.toml Cargo.lock crates/prime-shell
git commit -m "feat(p1): add production shell typography and icons"
```

---

### Task 3: Build the settled desktop and vertical glass rail geometry

**Files:**
- Create: `crates/prime-shell/src/visual/background.rs`
- Create: `crates/prime-shell/src/visual/rail.rs`
- Modify: `crates/prime-shell/src/visual/mod.rs`
- Modify: `crates/prime-shell/src/main.rs`

**Interfaces:**
- Produces: `RailLayout::for_output(width, height) -> RailLayout`.
- Produces: `RailAction::{Orb, Status}` and `RailLayout::hit(x, y) -> Option<RailAction>`.
- Keeps persistent namespaces exactly `prime.shell.background` and `prime.shell.rail` so the existing readiness contract remains valid.

- [ ] **Step 1: Write failing rail geometry tests**

Use a 1920x1080 target and assert a floating left rail, not a full-width top strip:

```rust
#[test]
fn kratos_1080p_rail_is_vertical_and_floating() {
    let rail = RailLayout::for_output(1920, 1080);
    assert!(rail.bounds.width <= 96);
    assert!(rail.bounds.height > 400);
    assert!(rail.bounds.x >= 12);
    assert!(rail.bounds.y >= 40);
}

#[test]
fn rail_hit_targets_map_orb_and_status() {
    let rail = RailLayout::for_output(1920, 1080);
    assert_eq!(rail.hit(rail.orb.center_x(), rail.orb.center_y()), Some(RailAction::Orb));
    assert_eq!(rail.hit(rail.status.center_x(), rail.status.center_y()), Some(RailAction::Status));
}
```

- [ ] **Step 2: Change the persistent rail layer-shell contract**

Replace `RAIL_HEIGHT`/full-width top anchoring with a fixed-width floating vertical surface anchored `LEFT`, using margins via `LayerSurface::set_margin` in SCTK 0.20.0. Keep exclusive zone `0` so application windows can exist behind/next to the floating rail; P1 window placement remains compositor-owned. The approved thin top status strip is drawn as part of the non-interactive background composition at P1 rather than preserving the old engineering rail.

Background remains full output, non-interactive, exclusive zone `-1`.

- [ ] **Step 3: Implement the desktop field**

Paint the background from `#05050d`/`#050818` with the owner-approved visible violet-to-blue-to-cyan aurora/light-ribbon field crossing the desktop body, plus subtle geometric traces. Color must exist throughout the wallpaper/material field rather than only borders. Remove the permanent centered `PRIME` word from settled background painting. Paint the approved hairline top status strip as subordinate desktop chrome with only truthful available status. Keep a separate startup-progress parameter so Task 4 can animate provisional identity before settled state.

- [ ] **Step 4: Implement rail paint**

Paint only Shell-owned tint/content: transparent ARGB outside the rounded rail body, dark navy translucent fill, subtle inner highlight, cyan/violet active indicator, geometric Orb and Status icons. The compositor blur in Task 5 will provide the actual backdrop material.

Remove visible construction copy `O ORB`, `Q STATUS`, and top diagnostic border.

- [ ] **Step 5: Replace pointer rail hit testing**

Replace `RAIL_TRIGGER_WIDTH` and horizontal edge tests with `RailLayout::hit(event.position.0, event.position.1)`. Keyboard shortcuts `o` and `q` may remain for construction accessibility but are no longer displayed as UI labels.

- [ ] **Step 6: Run tests/build and commit**

```text
cargo test -p prime-shell --locked
cargo build --release --locked -p prime-shell
git add crates/prime-shell
git commit -m "feat(p1): build prime desktop and vertical rail"
```

---

### Task 4: Rebuild Orb and quick controls as animated Prime glass surfaces

**Files:**
- Create: `crates/prime-shell/src/visual/orb.rs`
- Create: `crates/prime-shell/src/visual/quick_controls.rs`
- Create: `crates/prime-shell/src/motion.rs`
- Modify: `crates/prime-shell/src/visual/mod.rs`
- Modify: `crates/prime-shell/src/main.rs`

**Interfaces:**
- Produces: `Transition { progress: f32, direction: TransitionDirection, started_at: Instant, duration: Duration }`.
- Produces: `MotionState` for startup, Orb and quick-controls transitions.
- Produces: `OrbLayout::row_at(x, y, count)` and `QuickControlsLayout::power_action_at(x, y)`.
- Existing Core data and `PowerConfirmation` behavior remain unchanged.

- [ ] **Step 1: Write failing easing/state tests**

```rust
#[test]
fn ease_out_cubic_is_monotonic_and_bounded() {
    assert_eq!(ease_out_cubic(0.0), 0.0);
    assert_eq!(ease_out_cubic(1.0), 1.0);
    assert!(ease_out_cubic(0.5) > 0.5);
}

#[test]
fn closing_transition_reaches_closed_state() {
    let t = Transition::closing(Duration::from_millis(200), Instant::now());
    assert_eq!(t.sample_at(t.started_at + Duration::from_millis(200)), 0.0);
}
```

- [ ] **Step 2: Implement frame-callback animation scheduling**

After drawing any animating layer, request a Wayland frame callback:

```rust
layer.wl_surface().frame(queue_handle, layer.wl_surface().clone());
```

Implement `CompositorHandler::frame` to advance only the animation associated with the callback surface, redraw it, and request another callback until settled. Do not use busy loops or sleeps.

Startup transition begins only after persistent background and rail are configured. Orb/quick durations use the spec ranges (nominal 200 ms); startup uses nominal 500 ms.

- [ ] **Step 3: Replace Orb diagnostic table with launcher composition**

Use a floating left-side overlay adjacent to the rail, approximately 430x520 at 1080p. Render search/selection header, application rows/cards with icon + display name, subtle readiness indicator, selected cyan/violet focus treatment, and a concise message footer.

Preserve truthful behavior:
- `core.applications()` remains the source;
- blocked app stays non-launchable;
- `activate_selected_application()` remains the only launch request path;
- Core unavailable becomes a readable glass-panel state, not a panic.

- [ ] **Step 4: Rebuild quick controls into truthful cards**

Render concise sections from the existing `quick_lines` projection while retaining current Core API calls. At P1, parse only already-produced display strings into grouped visual rows; do not invent mutation APIs. Keep restart/power-off double-confirmation and visually separate armed vs ready vs unavailable state.

- [ ] **Step 5: Replace hit geometry and transient close lifecycle**

Use `OrbLayout::row_at(x, y, count)` and `QuickControlsLayout::power_action_at(x, y)`. Closing an animated transient must keep the Wayland surface alive until its close transition reaches 0, then drop it and clear keyboard focus/pending power.

- [ ] **Step 6: Run tests/build and commit**

```text
cargo test -p prime-shell --locked
cargo clippy --locked -p prime-shell --all-targets -- -D warnings
cargo build --release --locked -p prime-shell
git add crates/prime-shell
git commit -m "feat(p1): build animated orb and quick controls"
```

---

### Task 5: Implement compositor-owned real backdrop blur and Prime material effects

**Files:**
- Create: `crates/prime-compositor/src/effects.rs`
- Modify: `crates/prime-compositor/src/main.rs`
- Modify: `crates/prime-compositor/src/frame.rs`
- Modify: `crates/prime-compositor/src/shell.rs`

**Interfaces:**
- Produces: `MaterialKind::{Rail, Orb, QuickControls}` selected only from exact Prime layer namespaces.
- Produces: `EffectsState::new(&mut GlesRenderer, output_size) -> Result<EffectsState, EffectsError>`.
- Produces: `GlassBackdropElement` implementing `Element` and `RenderElement<GlesRenderer>`.
- `Runtime` owns `effects: EffectsState` and a readiness limitation string `Prime glass effects are in fallback mode` only when initialization/draw fails.

- [ ] **Step 1: Write failing material/geometry tests**

Tests are pure and do not require GL:

```rust
#[test]
fn only_prime_transient_namespaces_request_glass() {
    assert_eq!(material_for_namespace("prime.shell.rail"), Some(MaterialKind::Rail));
    assert_eq!(material_for_namespace("prime.shell.orb"), Some(MaterialKind::Orb));
    assert_eq!(material_for_namespace("prime.shell.quick-controls"), Some(MaterialKind::QuickControls));
    assert_eq!(material_for_namespace("random.client"), None);
}

#[test]
fn blur_capture_is_clamped_to_output() {
    let capture = expanded_capture(Rectangle::new((-10, 20).into(), (200, 100).into()), 24, (1920,1080).into());
    assert_eq!(capture.loc.x, 0);
    assert!(capture.size.w <= 224);
}
```

- [ ] **Step 2: Compile the blur texture shader at compositor startup**

Use Smithay 0.7.0 `GlesRenderer::compile_custom_texture_shader`. The shader must include `//_DEFINES` and sample `tex` using a bounded 9/13-tap separable-looking 2D kernel driven by `texel_size` and `blur_radius` uniforms. It also applies material tint and saturation reduction in the same pass so P1 avoids unnecessary full-resolution passes.

The implementation may use one captured texture per active glass geometry and update it only on damaged frames. Do not allocate GL textures each frame.

- [ ] **Step 3: Implement framebuffer-region capture inside `GlassBackdropElement::draw`**

Smithay draws render elements bottom-up (`elements_to_render.iter().rev()`), so insert the backdrop element immediately below its matching Shell surface in the top-to-bottom element list. When `draw` executes, lower scene content has already been rendered into the active framebuffer.

Use `GlesFrame::with_context` to update a preallocated GL texture with `glCopyTexSubImage2D` for the bounded backdrop rectangle, accounting for GLES bottom-left framebuffer coordinates on the unrotated P1 output. Wrap the owned raw texture as a `GlesTexture` during effects initialization, not per draw.

Then draw the captured texture into the material geometry with `GlesFrame::render_texture_from_to(..., Some(&blur_program), uniforms)` followed by a low-alpha cyan/violet edge/tint element. Restore any raw GL binding/state touched by `with_context` before returning.

- [ ] **Step 4: Insert glass elements into the Prime frame list**

In `frame::try_queue`, continue obtaining normal `space_render_elements`. Build a `PrimeRenderElement` enum with Smithay's `render_elements!` macro so it can contain both the existing space elements and `GlassBackdropElement`.

For each mapped layer in `layer_map_for_output` whose namespace maps to a `MaterialKind`, insert exactly one glass backdrop immediately after that layer's normal surface element in the top-to-bottom vector. Do not blur the full desktop background layer.

Preserve `persistent_baseline_renderable()` and existing shell/frame readiness checks against the actual Wayland surfaces.

- [ ] **Step 5: Add truthful fallback state**

If shader compilation/capture fails, log the concrete GLES error, add `Prime glass effects are in fallback mode` to readiness limitations, and render the Shell's translucent tint without the backdrop element. Do not crash Core/Shell solely because optional visual effect initialization failed; do not allow fallback state to be recorded as owner visual PASS.

- [ ] **Step 6: Run unit/static proof and release build**

```text
cargo test -p prime-compositor --locked
cargo clippy --locked -p prime-compositor --all-targets -- -D warnings
cargo build --release --locked -p prime-compositor
```

Expected: PASS, no `unsafe` outside the tightly bounded GL capture/texture-ownership section required by Smithay/GLES interop.

- [ ] **Step 7: Commit**

```text
git add crates/prime-compositor
git commit -m "feat(p1): add compositor-native prime glass effects"
```

---

### Task 6: Add focused-window depth and preserve XDG/input correctness

**Files:**
- Modify: `crates/prime-compositor/src/input.rs`
- Modify: `crates/prime-compositor/src/protocols.rs`
- Modify: `crates/prime-compositor/src/frame.rs`
- Modify: `crates/prime-compositor/src/effects.rs`

**Interfaces:**
- Produces: `Runtime::focused_window_surface: Option<WlSurface>` or equivalent stable identity.
- Produces compositor elements for `WindowShadow` and `WindowFocusGlow` that do not modify client pixels.

- [ ] **Step 1: Write failing focus-state tests around a pure helper**

Extract a pure focus transition helper:

```rust
#[test]
fn focus_transition_changes_only_when_surface_changes() {
    assert_eq!(focus_changed(None, Some(7)), true);
    assert_eq!(focus_changed(Some(7), Some(7)), false);
    assert_eq!(focus_changed(Some(7), Some(8)), true);
}
```

Use a small test identity rather than constructing Wayland resources in unit tests.

- [ ] **Step 2: Track XDG focus on pointer press**

When `keyboard_focus_under` resolves an XDG root, update compositor visual focus identity at the same point keyboard focus changes. Layer-shell focus (Orb/quick controls) must not become the focused XDG window.

Request a new frame only when focus identity actually changes.

- [ ] **Step 3: Add shadow/focus elements behind XDG windows**

For each mapped XDG window, create a soft shadow render element behind the content bounds. Focused window receives a restrained cyan/violet halo/edge; unfocused windows retain shadow but reduced highlight. Use shader/pixel elements and damage only the expanded window geometry.

Do not crop or recolor client content. Do not add a stock titlebar. Do not alter XDG configure semantics.

- [ ] **Step 4: Re-run physical-protocol unit path**

Run all compositor tests and the existing disposable XDG toplevel/popup client when the host visual session is opened in Task 8. For now run:

```text
cargo test -p prime-compositor --locked
cargo clippy --locked -p prime-compositor --all-targets -- -D warnings
```

- [ ] **Step 5: Commit**

```text
git add crates/prime-compositor
git commit -m "feat(p1): add prime window depth and focus hierarchy"
```

---

### Task 7: Integrate fonts/effects into the bootable image and add host visual proof tooling

**Files:**
- Modify: `image/Containerfile`
- Create: `tools/prove-p1-visual-host.sh`
- Modify: docs only if exact runtime package evidence requires recording.

**Interfaces:**
- Image guarantees Fedora 44 `google-noto-sans-fonts-20251201-2.fc44` is present.
- Host proof emits machine-readable PASS markers but never substitutes for owner visual acceptance.

- [ ] **Step 1: Extend image package assertions**

Install `google-noto-sans-fonts-20251201-2.fc44` in the rootfs package step and assert exact RPM ownership/version with the same strict style already used for Mesa/libseat/systemd packages.

Do not install GTK, Qt, GNOME Shell, KDE Plasma, a display manager, or a second compositor.

- [ ] **Step 2: Add a font smoke check to the image build**

After copying `prime-shell`, invoke a non-graphical Shell font probe mode added minimally to `prime-shell`:

```text
/usr/libexec/prime/prime-shell --probe-font
```

It must print a stable marker such as:

```text
PRIME_SHELL_FONT=Noto Sans
```

and exit 0 without requiring a Wayland connection. Keep normal `prime-shell` behavior unchanged.

- [ ] **Step 3: Add `tools/prove-p1-visual-host.sh`**

The helper must be read-only with respect to repo/system state except its own `/var/tmp/prime-p1-visual-host-*` evidence directory. It runs exact toolchain checks, fmt, clippy, tests, locked release builds, verifies the reference namespaces remain present, and verifies no forbidden desktop toolkit/package dependency entered `Cargo.lock` or image packages.

Required markers:

```text
P1_VISUAL_HOST_STATIC=PASS
P1_VISUAL_HOST_BUILD=PASS
P1_VISUAL_HOST_NO_BORROWED_DESKTOP=PASS
```

Do not claim physical glass PASS from this helper.

- [ ] **Step 4: Run host proof**

```text
bash tools/prove-p1-visual-host.sh
```

Expected: all three markers PASS.

- [ ] **Step 5: Commit**

```text
git add image/Containerfile crates/prime-shell tools/prove-p1-visual-host.sh Cargo.toml Cargo.lock
git commit -m "build(p1): integrate first-light visual runtime"
```

---

### Task 8: Full regression, canonical image proof, and physical KRATOS visual gate

**Files:**
- No source changes unless a proven failure is found.
- Evidence: unique `/var/tmp/prime-p1-visual-proof-*` directory.
- Update documentation only after evidence is complete.

**Interfaces:**
- Consumes the completed branch.
- Produces mechanical evidence, physical effect evidence, and owner acceptance boundary.

- [ ] **Step 1: Freeze candidate state before proof**

Record:

```text
git status --short
git rev-parse HEAD
git branch --show-current
```

Require clean worktree. Commit/push any intended source change before proof. Do not prove a dirty tree.

- [ ] **Step 2: Run full static/unit verification**

```text
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo build --release --locked -p prime-compositor -p prime-shell
```

Expected: PASS.

- [ ] **Step 3: Run the existing canonical P1 local proof unchanged first**

Launch `tools/prove-p1-local.sh` through the already-proven user-systemd + sudo execution lane with a unique `PRIME_P1_WORK_ROOT`. Require:

```text
P1_LOCAL_PROOF=PASS
```

and persisted `HEALTH_PROVING`, `SHELL_READY`, mapped frame retirement, clean QCOW2/UEFI evidence. If the visual implementation invalidates mechanical proof, stop and fix the regression before any visual acceptance attempt.

- [ ] **Step 4: Open the physical KRATOS Prime graphics window**

Preserve normal desktop recovery path first. Stop GDM only for the bounded proof window, move to the Prime tty/logind session, launch the exact branch `prime-compositor`, then `prime-shell`. Keep Oracle workstation connectivity alive.

Verify readiness JSON reaches:

```json
{
  "direct_tty_backend": true,
  "drm_access_ready": true,
  "renderer_ready": true,
  "outputs_ready": true,
  "frame_loop_ready": true,
  "shell_ready": true
}
```

and **does not** contain `Prime glass effects are in fallback mode`.

- [ ] **Step 5: Re-run physical XDG and input evidence**

Run the disposable XDG toplevel+popup proof against the live Prime compositor and require:

```text
XDG_TOPLEVEL_INITIAL_CONFIGURE=PASS
XDG_POPUP_INITIAL_CONFIGURE=PASS
PRIME_P1_PHYSICAL_XDG_CLIENT=PASS
```

Exercise keyboard/pointer Orb and quick-controls paths and verify input counters advance. Re-run the malformed-client isolation probe and prove the offending client is rejected without killing compositor/Shell.

- [ ] **Step 6: Capture performance/effect evidence**

During Orb open/close and quick-controls animation, record compositor process CPU, frame/readiness counters, and absence of effect fallback/errors. Confirm idle desktop does not continuously increment queued/submitted frames when no client/content changes require redraw.

The physical target is perceptually smooth 60 Hz on UHD 630; any repeated missed-frame or input-starvation behavior blocks owner review.

- [ ] **Step 7: Owner visual review on the monitor**

Leave Prime live on KRATOS and request owner review against `/home/kratos/Downloads/ui should be something like this .mp4`.

Owner explicitly judges: startup field, THETECHGUY dark/cyan/violet palette, glass quality, left rail, Orb, quick controls, window depth, typography, spacing, motion, responsiveness, and absence of construction/debug UI or stock-distro identity.

Do **not** infer approval from screenshots or mechanical readiness. Owner acceptance must be explicit.

- [ ] **Step 8: Restore KRATOS desktop after review**

Stop temporary Prime Shell/compositor units and restore GDM/XFCE. Verify `gdm.service` active and Xorg/XFCE present so the workstation is not left at a TTY or stale framebuffer.

- [ ] **Step 9: Freeze and push accepted visual candidate**

After owner acceptance and clean regression evidence:

```text
git status --short
git log -1 --oneline
git push origin design/p1-first-light-visual
```

Record the exact accepted SHA and evidence paths. Do not mark generation `KNOWN_GOOD` unless the separate Prime generation/promotion authority requires and proves that transition.

---

## Plan Self-Review

- Spec coverage: desktop/startup, THETECHGUY palette, left rail, Orb, quick controls, real compositor glass, typography/icons, window depth, motion, truth/fallback, Fedora image integration, UHD 630 performance, mechanical regression and owner acceptance are each mapped to a task.
- Dependency boundary: only `fontdb` and `fontdue` are added to Shell; no desktop UI toolkit is introduced. Fedora Noto Sans package is explicit and versioned.
- Type consistency: `Theme`, `Argb`, `Canvas`, `Rect`, `TextSystem`, `RailLayout`, `MotionState`, `MaterialKind`, `EffectsState`, and `GlassBackdropElement` are introduced before downstream use.
- Mechanical authority: persistent namespaces and `SHELL_READY` proof rules remain intact; canonical proof is rerun unchanged before owner review.
- Completeness scan: every implementation step is explicit and actionable.
