# Prime OS P1 First Light Visual Body — Design

Date: 2026-09-02
Status: proposed visual authority for user review
Parent mechanical candidate: `786018fd38a066e30144df869b9a8b2a2701381a`
Workstream: `design/p1-first-light-visual`

## 1. Objective

Turn the mechanically proven P1 compositor/Shell scaffold into the first visually complete Prime OS body without changing the frozen mechanical proof SHA.

P1 visual acceptance is not a cosmetic pass. The resulting system must boot into a coherent Prime-owned desktop that visibly satisfies the First Light authority: startup identity, Prime Shell, system rail, Prime Orb/launcher, functional windowing, quick controls, smooth transitions, Prime glass/depth language, and no obvious Fedora/GNOME/KDE/COSMIC identity leakage.

The supplied reference video at `/home/kratos/Downloads/ui should be something like this .mp4` is the interaction/design donor and quality bar. Prime borrows its principles and motion language, not its branding or exact screen composition.

The final Prime logo/mark is explicitly deferred until the OS visual body boots and behaves correctly. A provisional textual `PRIME` identity may be used during this phase; logo work must not block or distort the desktop architecture.

## 2. Non-negotiable product direction

1. **Best practical P1 implementation now.** Do not knowingly build a temporary visual architecture that will need replacement merely to achieve the already-known First Light target.
2. **Native Prime identity.** Do not replace Prime Shell with GNOME, KDE, GTK desktop chrome, COSMIC, or another borrowed desktop body.
3. **Real glass.** Prime glass must include compositor-owned backdrop blur, tint, edge highlight, shadow/depth, and focus hierarchy on the KRATOS proof Host. Plain semi-transparent rectangles are a fallback only, not the accepted P1 target.
4. **Reference-led but original.** Preserve the donor's clean desktop, slim rail, floating launcher, layered depth, restrained motion, and uncluttered return state while expressing them through Prime's own geometry and brand palette.
5. **Truthful controls.** Network/audio/power/storage/application state continues to come from Prime Core and existing typed contracts. Unsupported actions remain visibly unavailable rather than simulated.
6. **Mechanical truth remains intact.** Existing `SHELL_READY`, frame-loop, client, input, generation, and local proof semantics are not weakened to make the UI appear complete.
7. **KRATOS is the P1 visual target.** The implementation must sustain the accepted experience at 1920x1080/60 Hz on the HP 290 G4 Intel UHD 630 path already proven physically.

## 3. Brand and color authority

Prime uses the established THETECHGUY visual family recovered from the current live website and brand authority, translated into an OS material system rather than copied as web CSS.

### Core field

- Near-black/navy foundation: `#05050d`, `#050818`, `#060817`, `#071021`, `#081225`.
- Deep panel tint: approximately `#0f172a` mixed over the scene at variable alpha.
- Primary text: `#f8fafc` / white at reduced intensity for secondary hierarchy.
- Muted text: `#94a3b8` / `#cbd5e1` depending hierarchy.

### Brand light

- Cyan: `#06b6d4`, `#22d3ee`.
- Violet: `#7c3aed`, `#8b5cf6`, `#a855f7`.
- Optional magenta energy accent: `#d946ef` / `#e879f9`, used sparingly.
- Blue bridge accent: `#3b82f6` / `#60a5fa`, used to connect cyan and violet rather than become a separate theme.

### Material rule

Cyan is the primary live/interaction light. Violet supplies depth and Prime/THETECHGUY identity. Magenta is rare emphasis, not a permanent neon border. The desktop remains predominantly dark so the color appears as light in glass, focus, and motion rather than as large saturated blocks.

Status colors such as green/amber/red remain semantic and do not replace the Prime cyan/violet accent system.

## 4. Desktop composition

### 4.1 Background

The settled desktop is a deep near-black/navy field with a restrained indigo-to-cyan aurora/depth gradient and very subtle texture/noise to prevent flat banding. It must remain visually quiet enough that application content and glass surfaces dominate.

The current giant centered `PRIME` desktop label is removed after startup. Prime identity is present through the system materials, rail, Orb, and startup sequence rather than a permanent watermark.

### 4.2 Startup transition

Normal boot reaches a centered provisional `PRIME` identity on the dark field. Once Shell/Core readiness is earned, the wordmark/light resolves into the desktop and rail with a short, controlled transition. This is a startup state, not a permanent desktop surface.

No final logo decision is made in this phase.

### 4.3 System rail

Replace the construction top debug bar with a slim floating vertical rail aligned near the left edge, following the donor's spatial idea while remaining Prime-specific.

The rail contains:

- Prime Orb entry point;
- running/admitted application affordances when useful;
- system/status entry;
- minimal workspace/system indicators required by P1.

The rail is a frosted material with a small corner radius, soft cyan/violet edge light, subtle inner highlight, and restrained shadow. Idle state is quiet. Hover/focus/active state increases luminosity and depth rather than changing into solid blocks.

The rail must never display engineering labels such as `O ORB`, `Q STATUS`, or readiness/debug state in the accepted product surface.

## 5. Prime Orb

Prime Orb is the central interaction surface and the main launcher.

Activation sources remain keyboard and pointer, with a later `Super` binding permitted by the existing authority. The initial P1 implementation preserves existing truthful application admission/launch logic and changes the presentation and interaction body around it.

### Visual form

The Orb entry is a circular/rounded luminous control on the rail. Activation expands into a floating glass launcher, anchored to the rail but visually separated from it through depth and motion.

The launcher includes, at P1 minimum:

- search/selection focus surface;
- admitted applications with clear launch-ready/blocked states;
- selected-item hierarchy;
- Prime/system destinations that are actually implemented;
- concise status/limitation messaging when Core data is unavailable.

The Orb is not a diagnostic table. Application status belongs in secondary visual treatment, not `READY/BLOCKED` columns dominating the launcher.

Keyboard selection and pointer hit targets remain mechanically testable.

## 6. Quick controls / system surface

Quick controls become a compact floating glass system panel instead of the current rectangular text dump.

P1 content includes existing truthful projections for:

- network state;
- audio state;
- power readiness/actions;
- storage pressure/capacity summary;
- hardware/system health summary where currently available;
- restart and power-off with the existing double-confirmation safety rule.

Unavailable operations are visually disabled with an explanation; the UI must not manufacture toggles that Core cannot perform.

Information is grouped into cards/rows with icon + concise label + state. Long evidence strings and engineering messages do not appear in the normal product surface.

## 7. Windowing visual hierarchy

The compositor already maps real XDG clients; P1 now gives them a Prime-owned spatial hierarchy.

### Required treatment

- focused window receives a restrained Prime edge/focus light;
- unfocused windows lose emphasis rather than becoming unreadable;
- compositor-owned soft shadow separates windows from the desktop;
- stacking/focus changes animate subtly;
- application content is not recolored or falsified;
- no stock-distro title bar may be introduced by Prime merely for decoration.

P1 may begin with rectangular application content while using compositor-owned shadow/focus material around it. Rounded content clipping is allowed only if implemented without breaking client correctness or forcing a fragile protocol shortcut.

## 8. Glass and depth architecture

### 8.1 Ownership

**Prime Compositor owns effects that depend on the scene behind a surface:**

- backdrop capture;
- blur;
- shadow;
- focus halo;
- depth ordering/effect composition.

**Prime Shell owns:**

- surface geometry;
- content;
- icons/text;
- material intent through its known Prime layer namespaces;
- interaction state;
- animation state.

This split is mandatory because a client cannot truthfully blur pixels it cannot see behind itself.

### 8.2 Backdrop blur

Use a GPU path on the existing GLES renderer. The intended P1 method is a downsampled multi-pass blur (dual-Kawase or equivalent separable approach) rather than a full-resolution Gaussian over the entire 1080p frame each refresh.

The compositor should:

1. render/capture the scene below the target Prime glass layer;
2. downsample the required backdrop region;
3. perform a bounded multi-pass blur;
4. composite the blurred region beneath the translucent Shell material;
5. add edge highlight/shadow according to the material profile.

Static/unchanged backdrop work should be reused where correctness permits. Dirty-region or region-bounded processing is preferred so UHD 630 is not forced to reblur the entire screen for a small panel animation.

### 8.3 Material profiles

At minimum define internal material profiles for:

- `rail` — strongest translucency, compact blur, quiet shadow;
- `orb` — deeper blur, stronger separation and violet/cyan edge response;
- `quick_controls` — similar to Orb but more neutral for readability;
- `window_focus` — shadow/focus treatment without pretending client content is glass.

If GPU blur initialization fails, functionality must remain available with a tinted translucent fallback and the limitation must be recorded. **However, fallback mode does not satisfy KRATOS P1 visual acceptance.**

## 9. Typography and iconography

The current built-in 5x7 bitmap font is retained only for recovery/debug/evidence contexts and removed from accepted normal Shell presentation.

P1 normal Shell uses anti-aliased scalable typography. Prefer a small, locked native Rust font rasterization dependency and a Fedora-owned open font package so Prime does not pull in a desktop toolkit merely for text.

Typography hierarchy:

- clean modern sans for all normal Shell content;
- medium/semibold weight for primary controls;
- regular weight for status/detail;
- monospace reserved for evidence/technical identifiers when intentionally exposed.

Icons should be Prime-owned geometric vector/raster primitives or a small reviewed open icon source incorporated into Prime's own visual language. Emoji and font-dependent glyph icons are not accepted as primary system controls.

## 10. Motion language

Motion is short, physical, and restrained.

Target interaction durations:

- hover/focus light: roughly 90–140 ms;
- rail/Orb/quick panel open-close: roughly 160–240 ms;
- window focus/stack depth transition: roughly 120–200 ms;
- startup identity-to-desktop: approximately 350–650 ms, governed by actual readiness rather than a fake timer.

Use ease-out for entry, ease-in for exit, and spring-like overshoot only where subtle enough not to read as a game UI.

Animations are driven by Wayland frame callbacks / compositor frame timing, not busy loops or arbitrary sleeps. Reduced-motion behavior should be structurally possible even if the P1 settings control lands later.

## 11. Rendering structure

The current monolithic `visual.rs` is sufficient for construction proof but not for the accepted body. The implementation should be decomposed into focused Prime Shell rendering units rather than growing one giant painter.

Proposed client-side boundaries:

- `visual/theme.rs` — palette, spacing, radii, material/content tokens;
- `visual/primitives.rs` — alpha blending, gradients, rounded geometry, vector/icon primitives;
- `visual/text.rs` — font loading/raster/cache/layout;
- `visual/background.rs` — desktop/startup composition;
- `visual/rail.rs` — rail paint + hit geometry;
- `visual/orb.rs` — launcher paint + hit geometry;
- `visual/quick_controls.rs` — system panel paint + hit geometry;
- `motion.rs` — transition state and frame-callback timing.

Proposed compositor-side boundary:

- a focused effects/material module responsible for backdrop blur, shadow and focus composition, without moving Shell data or application policy into the compositor.

Existing Core client, typed application/power contracts, protocol handling and readiness authority remain separate.

## 12. Performance target

P1 accepted visual state must remain responsive at the physical KRATOS baseline: 1920x1080 at 60 Hz on Intel UHD Graphics 630.

Acceptance target:

- normal desktop idle does not continuously redraw without cause;
- Orb/quick-panel animation should sustain perceptually smooth 60 Hz where the display path allows;
- blur/effects must not starve input or Wayland dispatch;
- no CPU software blur of full-screen frames;
- memory allocations for repeated animation frames should be bounded and reused where practical.

Performance regressions are engineering failures, not acceptable visual trade-offs.

## 13. Input and accessibility baseline

Existing physical keyboard/pointer delivery remains required.

- pointer hit regions match visual controls;
- keyboard navigation remains complete for Orb and quick controls;
- Escape closes transient surfaces;
- focus is visually obvious without relying only on color;
- normal text contrast remains readable through glass;
- critical states use icon/label/state in addition to semantic color.

## 14. Failure and truth behavior

- Core unavailable: Shell still renders, but Core-backed content states that it is unavailable.
- Blur/effect failure: degrade to readable translucent material, persist/report limitation, fail owner visual acceptance until corrected on KRATOS.
- Font failure: fail closed into a reviewed readable fallback, never invisible/garbled text.
- Client protocol error: offending client may fail without bringing down compositor/Shell; existing resilience obligation remains.
- Shell restart: compositor remains authoritative and Shell can reconnect/rebuild its persistent surfaces.

## 15. Proof and acceptance

### Construction proof

Before physical review:

- exact Rust 1.97.1 toolchain;
- `cargo fmt --all -- --check`;
- all-target Clippy `-D warnings` for affected crates;
- locked release build;
- runtime link closure;
- unit tests for material math, hit geometry, animation state, typography fallback, and effect invalidation;
- existing canonical mechanical proof must remain green or be deliberately superseded by stronger equivalent proof.

### Physical mechanical proof on KRATOS

Must re-prove at minimum:

- direct tty/logind/libseat path;
- UHD 630 + HDMI output;
- keyboard and pointer;
- real XDG client mapping;
- Prime Shell background/rail/Orb/quick-controls surface lifecycle;
- mapped frame retirement;
- `SHELL_READY` remains truthful;
- effect path active rather than fallback during visual acceptance.

### Owner visual acceptance

The owner compares the live KRATOS body against the supplied inspiration video and Prime authority. Acceptance requires the system to look like a coherent Prime desktop, not an engineering scaffold.

Review explicitly covers:

- startup identity;
- dark cyan/violet brand field;
- real glass/depth quality;
- left rail;
- Orb launcher behavior;
- quick controls;
- window focus/depth treatment;
- spacing and typography;
- motion quality;
- responsiveness;
- absence of stock-distro identity leakage.

Only owner acceptance closes the P1 visual gate.

## 16. Explicitly deferred

This visual workstream does **not** finalize:

- the final Prime logo/mark;
- Windows Personality;
- Android Personality;
- Ptah integration;
- Prime Store;
- full P1.5 update/survival behavior;
- later multi-display visual policy beyond not breaking the architecture for it;
- decorative features that do not contribute to First Light acceptance.

The logo is intentionally handled after the OS body boots and behaves correctly so branding work does not mask architectural/UI defects.

## 17. Definition of done

This workstream is done only when the mechanically proven Prime foundation has become a physically running, coherent First Light desktop on KRATOS with the real glass material path active, the donor's interaction principles translated into Prime's own identity, and owner visual acceptance recorded.
