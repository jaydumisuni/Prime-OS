# Prime P1 Final UI — KRATOS evidence checkpoint

Candidate: `b3f52b7c5bc7862236c0041a916fca20beabdf39` on `work/p1-final-ui`.

## Proven

- Full workspace tests: PASS; Prime Shell: 57/57 PASS.
- `cargo fmt --check`: PASS.
- Clippy workspace/all-targets with warnings denied: PASS.
- Optimized `prime-shell` + `prime-compositor` release build: PASS.
- Optimized `primed` release build: PASS.
- Current candidate `primed` is live in a disposable unprivileged namespace and serves Core on `/run/user/1000/prime-b3f52b7-core.sock`.
- Core Capability Interface negotiation is live; system status reports real KRATOS network/audio/thermal state and `power_mutation.ready=true`.
- Current candidate `prime-shell` is live against the physical compositor on `wayland-1`, using Prime 03 and Noto Sans.
- Fresh readiness re-earned `SHELL_READY` on seat0, `/dev/dri/card1`, HDMI-A-1, 1920x1080@60.
- The live compositor source has no source diff between `42cdd8f` and the final candidate. Readiness contains no glass-fallback limitation.

## Truth boundary

The root-owned physical compositor transient unit could not be restarted through Oracle after `primed` because the workstation RPC blocks `sudo` and system `systemctl restart` requires interactive authentication. Therefore the strict chronological `primed -> compositor -> Shell` relaunch is **not re-proven in this run**; the already-live, source-equivalent physical compositor was retained.

No permitted input-injection route is exposed through the current Oracle RPC, so fresh physical pointer/keyboard Home and Quick Controls activation markers were not re-earned. The final Shell runtime is live, its Core data paths are live, and the 57/57 regression suite covers the new Home/search/Quick Controls behavior, but this checkpoint does not manufacture a new physical interaction PASS.

`KNOWN_GOOD` remains unpromoted.
