# Prime-on-Ubuntu — KRATOS preview integration

Date: 2026-10-26. Status: candidate, not shipped or Prime First Light acceptance.

Recovered host: Ubuntu 26.04.1 LTS, XFCE on X11, GDM autologin. Prime Shell is a
Wayland layer-shell client, requiring a supporting compositor. Ubuntu's native
desktop and boot/recovery are retained unchanged.

An isolated *nested* labwc Wayland compositor runs as a window inside the existing
Ubuntu/XFCE session. It starts the existing Prime Shell UI and connects it to
a bounded Ubuntu application bridge through a private user runtime Unix socket.
The bridge only dispatches eight enumerated Ubuntu desktop application IDs.
It does not impersonate a real Prime Host: host UUID is NIL, generation identity
is UBUNTU_PREVIEW_NOT_PRIME, and compatibility stays RECOGNIZED (not FUNCTIONAL).
The power route is forbidden; unavailable Prime Core controls fail closed.

Run tests: python3 tools/prime-on-ubuntu/test_bridge.py
Run preview: python3 tools/prime-on-ubuntu/session.py
Exit preview: Super+Escape inside labwc or close the nested compositor window.
Ubuntu XFCE and Oracle remain in place. Do not edit GDM or enable autostart yet.

Future gates: build/check current Prime Shell, live compositor/layer-shell proof,
actual host-app launch proof, input/window management, resilience after crash,
then separate opt-in login-session adoption. This is not a bootable Prime ISO,
native Prime Core authority, generation update, or KNOWN_GOOD promotion.

## 26 October KRATOS live checkpoint

- Existing source branch: integration/prime-on-ubuntu-20261026. Main remains protected.
- Ubuntu 26.04.1 LTS remains XFCE X11 (loginctl session 2); GDM autologin unchanged.
- Host compositor dependency: Ubuntu package labwc 0.9.3-1 installed. The nested
  X11-backed Wayland preview runs with WLR_BACKENDS=x11, not as a new OS login.
- Rebuilt Prime Shell from the integration branch. The prior framebuffer-length
  panic is addressed by bounded cached-frame copying. Three launcher-geometry
  regressions bound the UI at KRATOS resolution; release and tests passed.
- Python bridge 5/5 tests passed. Prime Shell 67/67 tests passed after the Ubuntu dispatch-label
  correction, and the final release binary compiled successfully.
- Live Prime Shell persisted as a Wayland layer-shell client. A dedicated
  app-session D-Bus bus prevents GTK application dispatch from reusing Ubuntu
  XFCE's existing Thunar daemon.
- Live /v1/applications reported 8 explicit Ubuntu application entries. A POST
  for Files returned DISPATCHED_TO_UBUNTU; a separate Thunar process was
  observed with DISPLAY=:1, WAYLAND_DISPLAY=wayland-0 and private D-Bus.
  This proves routing and process launch, not every GUI action or Prime Exec.
- The nested compositor's startup log includes an Xwayland :0 bind warning;
  nested Xwayland operates on :1. This is not a verified warning-free release.
- The preview is intentionally not set as GDM's default or a replacement for
  Oracle/Ubuntu. User-level launcher may be installed via
  python3 tools/prime-on-ubuntu/install-launcher.py.
- Escape: Super+Escape inside labwc, or terminate the nested labwc process.
  XFCE remains active; all preview bridge processes must exit and socket
  cleanup must be checked. Installer/boot/recovery and native Prime acceptance
  remain separate future proof gates.

## Owner-directed Prime launcher concept — 26 October 2026 (design freeze candidate; not implemented)

This section captures the owner's follow-up to the saved `menu/Prime_OS_Apps_Menu_Standalone.html` and the Ubuntu-hosted mockup. **Do not replace or silently modify the frozen standalone menu artifact.** The generated mockup is a discussion artifact, not approved Prime branding or proof.

### Exact Prime identity — correction to mockup

- The generated mockup incorrectly substituted a large angular/A-shaped triangular glyph for the Prime OS emblem. **Do not use that glyph.**
- Canonical Prime mark is already tracked in `crates/prime-shell/assets/branding/prime-logo-master-128.png` and scaled `prime-logo-*.rgba` variants: the violet/cyan hood-shaped symbol with an illuminated circular center/eye. Preserve its exact silhouette and color.
- Default wallpaper `crates/prime-shell/assets/wallpapers/Prime_OS_Wallpaper_03.png` has the corresponding central logo composition. The default-wallpaper menu arranges application icons **around the logo already present in the wallpaper** at its existing visible scale; do not duplicate or replace the wallpaper logo. On any other wallpaper the self-contained floating menu may display the same authentic Prime mark.
- The left taskbar Prime menu button is labeled **Prime OS** beneath the icon. Never put "Prime OS" under the former bottom "More apps" position.

### Bounded visible apps and two-sided folding carousel

- The app ring has **at most 11 visible application positions**. The owner chooses/reorders which applications take priority; this limit never constrains the total installed-app catalogue.
- Retain the saved **icons surrounding a central Prime symbol** layout, not the currently implemented generic card grid. Retain the inward-folding, nested portal visual language of the original menu.
- Two stacks at the lower **left and right** are both **the navigation controls and visible ends of one circular stream** of installed apps. Do not add a "More apps" label, arrow, or separate expansion panel in the bottom center.
- **Right stack activation:** the next hidden app unfolds/inserts from the right, visible positions shift as one sequence, and one app at the far left retracts/folds into the opposite stack. **Left stack activation:** the inverse sequence (one arrives from the left, one leaves on the right). Once the carousel has more than 11 apps, there must still be exactly 11 visible after a completed step. If fewer than 11 are available, show only the available apps.
- Wheel/trackpad and keyboard navigation may drive the same single-step sequence. Animation must convey arrival, translation across the app arc, and retreat toward the opposite stack. No duplicate or disappearing catalogue entries.
- A compact search input is integrated into the center-logo area; clicking it **or typing as soon as the menu is open** begins live search across accessible apps and files (with folders/settings where connected). Search results use Prime styling, not Windows artwork.
- The outer icon arrangement should preserve the visual clarity of the referenced Insync-style app presentation, while retaining Prime's two-sided inward-folding overflow.

### Adjacent agreed desktop behaviors (separate implementation boundaries)

- The taskbar/rail tracks actual running windows with up to five visible running-app entries and independent overflow navigation; pinned apps (at least three supported) are distinguished from running entries.
- Desktop shortcuts can be placed and rearranged independently of the eleven-menu-icon selection.
- Quick Controls are accessed via the taskbar's lower system-control area.
- Menus and Quick Controls dismiss on outside click, Escape, another transient opening, or switching to an app; never require clicking only the original toggle/collapse button.
- Ubuntu-backed Settings/app launches must produce/focus real windows, not merely report a dispatch as completion.

### Acceptance gate

This is a **recorded owner design direction**, not implemented UI or permission to replace the Ubuntu default desktop. Before implementation: reproduce the corrected Prime mark in an owner-reviewed mockup; then implement and test the two-stack bidirectional movement, exactly bounded visible slots, wallpaper-aware presentation, full search, app/window focus, and outside-dismiss behavior against the existing Prime Shell/Compositor contracts. Preserve XFCE, Oracle, and rollback. Do not mark this or native Prime release `KNOWN_GOOD` without independent proof.\n

## 26 October 2026 — Native preview UI update (draft, not qualified)

The implementation advances the existing Prime-on-Ubuntu PR rather than replacing Ubuntu.
The initial generic line-art app renderer was visually rejected by the owner. The
corrective preview now renders the **actual installed app artwork** from the
Ubuntu GTK icon theme. A user-only bridge materializes PNG icons under
`$XDG_RUNTIME_DIR/prime-on-ubuntu/icons`, and the native Rust Shell decodes
and composites the corresponding per-application artwork (rather than
drawing generic line icons). The exact repository Prime logo is retained
on the rail. When system-03 is selected, the wallpaper's existing Prime
logo remains in place without creating a second one.

Implementation and validation at this checkpoint:

- 79 real Ubuntu desktop apps indexed; 79/79 icons materialized on KRATOS.
  The Ubuntu-preview desktop entry is excluded from its own catalogue.
- Eleven visible app slots; two-sided circular navigation; colorful incoming
  app icons shown on both folding-stack fronts. Right-click a visible app to
  toggle preferred front-of-menu placement, stored in user config
  `prime/menu.json`. No native Prime Exec status is inferred.
- Compact typing-first app search and a bounded filename-only local search
  in user directories. A discovered file may be dispatched with Ubuntu's
  `gio open`; this is not certification of opening all formats.
- Launcher outside-click / Escape dismissals; distinct Ubuntu-preview
  Quick Controls backed by real read-only host status. No power mutation
  is enabled.
- Native Rust test suite and bridge tests pass at this checkpoint, and the
  native release builds on KRATOS.
- Live compositor capture confirmed full-color installed app icons and
  colored stack-front images at 1366 x 718 on the isolated nested desktop:
  `/home/kratos/.oracle/proof/prime-color-icon-acceptance-20261026/live-prime-color-icons-and-stacks.png`.
  This is a visual check, not owner acceptance of the overall UI.

**Still open:** Running-app taskbar and overflow are not wired to the preview
rail; actual window focus is not yet accepted; desktop placement and
Prime-native Machine/Bin icons remain pending; full search coverage, arc
animation, small-screen layout, and Ubuntu Quick Controls outside-click
recovery need systematic interaction proof. Some browser/compositor checks
still demonstrate dispatch rather than a complete application workflow.
No new default desktop, native boot identity, installer, or ISO admission.

## Prime mockup acceptance worktree (2026-10-26) — NOT ACCEPTED

Branch ui/prime-mockup-acceptance-20261026 is an isolated visual QA worktree based on the existing Prime-on-Ubuntu branch. The owner reference is Prime OS Neon Space Desktop UI(1).png (1672x941). No native Prime, boot, GDM default, Ubuntu replacement or ISO change is authorized.

Implementation: 18 cropped Prime-only system/Quick Controls PNG icon assets, embedded reference-art renderer, branded desktop shortcuts (Machine, Projects, Bin), named 11-slot app orbit with duplicate-name priority suppression and matching lower-arc positions, two incoming app stacks, installed XDG app icons retained for unrelated apps, real Ubuntu-only shortcut dispatches, disabled/fail-closed unfinished system apps, six Quick Controls visual tiles, isolated runtime-name preview, full-height rail first-paint and zero-height SHM correction.

Evidence: Rust 76/76, bridge 13/13 and release build passed. Live app orbit screenshot is stored at /home/kratos/.oracle/proof/prime-workspace-menu-pass2-20261026/menu-corrected-order.png. The desktop screenshot is under prime-visual-workspace-v2-20261026. The screenshot-derived tiles have baked glows and require clean high-resolution re-authoring before pixel acceptance.

Blocked: current system-03 waves are not the cosmic mockup wallpaper; central mark alignment differs. Quick Controls has not passed graphical or behavioral acceptance; a capture failed/turned black and the second nested Wayland session exited with a broken pipe. Carousel movement timing, window focus, real app launching, full taskbar overflow, desktop shortcut usability, click-away, search and multiple resolutions require complete interactive proof.

ACCEPTANCE STATUS: FAILED / OPEN. Keep existing Prime preview, Ubuntu XFCE, recovery and rollback unchanged. No merge and no KRATOS default-desktop promotion until owner visual signoff and system-function proofs.

### Independent Quick Controls raster check

An offline Rust/PNG painter test named
offline_mockup_quick_controls_reference_proof now renders the Ubuntu-preview
panel at 455x424 independently of the nested compositor. It verifies the card
raster and exports reproducible evidence under
~/.oracle/proof/prime-quick-offline-20261026/prime-quick-controls-offline.png.
Six screenshot-derived control icons were re-trimmed to discard baked-in text;
upper three and lower three use distinct crop coordinates.
Visual inspection confirms six icons/labels and two sliders, all unclipped.
This is **only an offline painter proof**, NOT live network, Bluetooth,
performance, brightness or power integrations; the compositor's earlier
black capture/exit and outside-click behavior remain live acceptance gates.
