# P1 Integrated Candidate — Source Proof

Date: 2026-09-21
Branch: `integration/p1-first-light-candidate-20260921`
Integration base commit: `e3d2d4e`

## Integrated authority

This candidate joins three previously separate authorities without rewriting their history:

- P1 final UI lineage through `263d1b7`;
- canonical P1 First-Light proof lineage through `0398835`;
- current Prime recovery/planning authority through `d20d0a1`.

The canonical proof merge had one textual conflict in `tools/prove-p1-local.sh`. The final-UI side was retained because it had already integrated the Aug-25 recovery/OVMF proof path in `651a86f` and then added later proven XBOOTLDR/Discoverable-Root and boot-budget corrections (`e15e896`, `786018f`). No other implementation content differed in the proof merge.

## Source proof

Executed on KRATOS at this integrated candidate:

- `cargo fmt --all -- --check` — PASS.
- Core/recovery/contracts locked workspace tests — 73 PASS, 0 FAIL.
- `cargo test --locked -p prime-shell` — 62 PASS, 0 FAIL.
- `cargo test --locked -p prime-compositor` — 5 PASS, 0 FAIL.
- `cargo clippy --locked --workspace --all-targets -- -D warnings` — PASS.

Total directly executed Rust tests: **140 PASS, 0 FAIL**.

## Canonical image / UEFI proof status

`tools/prove-p1-local.sh` requires non-interactive root authority for rootful Podman, NBD, AppArmor profile handling, mounts and QCOW2 inspection.

The current Oracle workstation RPC rejects `sudo` before command execution with:

`PRIVILEGED_COMMAND_BLOCKED: sudo is not permitted through Oracle workstation RPC.`

Therefore the integrated candidate has **not** earned a new canonical image/UEFI proof, physical KRATOS proof, or `KNOWN_GOOD` promotion in this run. Existing earlier proof remains historical evidence only and is not silently transferred to the integrated candidate.

## Next proof gate

Run the existing canonical `tools/prove-p1-local.sh` through an authorized privileged execution surface, then perform the bounded latest-UI physical acceptance checks. Only after those pass may P1 completion or generation promotion be considered.
