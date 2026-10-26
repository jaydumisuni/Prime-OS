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
