# Prime P1 System Wallpapers + Physical Revalidation — 2026-09-04

Candidate: `42cdd8fb139cc5a8363264bcc356e2a15ce0c9ed` (parent `ee783a8c40ffb69a20a4ff06ee2b0ebae84b9f3f`).

## Result

- Eight approved Prime wallpapers are bundled as selectable system wallpapers.
- Animated First Light remains the default.
- All eight PNGs decode through the production Rust path; Prime Shell suite is **55/55 PASS**.
- Workspace tests, format, diff check, Clippy with warnings denied, and optimized Shell/compositor release build all pass.
- Live `system-01` selection loaded by catalog ID/title and re-earned `SHELL_READY`.
- Default `animated-first-light` was restored and idle motion resumed.
- Final live compositor phase: `SHELL_READY` on `seat0`, HDMI-A-1, 1920x1080/60, keyboard/pointer/input delivery ready.
- Lexar USB3 storage negotiated at 5 Gbit/s, mounted read-only, enumerated successfully, and sustained **173 MB/s** for a direct 256 MiB read from `sources/install.wim`.
- ASUS TUF H1 USB audio accepted a 48 kHz S16_LE stereo stream on both channels. Audibility remains external.

## External-only gaps

Ethernet still has no carrier, DP is disconnected, audible sound requires a human listener, and this unattended run did not include new physical keyboard/mouse movement.

Machine-readable evidence is in `prime-p1-system-wallpapers-physical-proof.json`; final readiness is in `readiness-final.json`.
