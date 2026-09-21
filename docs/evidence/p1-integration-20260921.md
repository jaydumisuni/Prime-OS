# P1 Integrated Candidate — Canonical Local / UEFI Proof

Date: 2026-09-22
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

The integrated candidate was replayed through the existing canonical `tools/prove-p1-local.sh` using the audited Oracle privileged broker for root-only Podman/NBD/AppArmor operations. Rust/Cargo execution remained delegated to the `kratos` user.

Exact proof source:

- source revision: `086ee97f4bef57f78b725af9b220e98faf9f87a1`
- proof result: **PASS**
- proof report SHA-256: `4fe99fab39b5878a610643f5411f5896c31b7271ed53fe6bc2711923a4be9296`
- QCOW2 SHA-256: `d2240fb54921bf65aad1a7f003216164e71c75831da6f8db761a62006c94f667`
- generation id: `p1-first-light-086ee97f4bef`
- generation state: `HEALTH_PROVING`
- canonical/final/normal-UKI/recovery-UKI Composefs digests: identical
- UEFI: OVMF
- persisted Prime Host identity: present
- mechanical Shell readiness: **true**
- `P1_LOCAL_PROOF=PASS`

Durable evidence:

- `docs/evidence/p1-integration-086ee97-local-proof.json`
- `docs/evidence/p1-integration-086ee97-broker-journal.txt`

The first privileged launch failed only on root Git safe-directory protection. The second failed only because an isolated Cargo target directory violated the canonical script's expected `target/release` path. Neither changed Prime source. The third audited run passed end-to-end.

## What this proof does not promote

This exact integrated SHA has not earned new physical-KRATOS owner-visual/recovery acceptance and has not been promoted to `KNOWN_GOOD`. The report remains:

- `known_good_proven=false`
- `owner_visual_acceptance=false`
- `physical_kratos_boot_proven=false`
- `recovery_boot_proven=false`

Earlier physical/UI/recovery evidence remains historical evidence for its exact source revisions and is not silently transferred to `086ee97`.

## Next proof gate

The integrated source and canonical local/UEFI gates are closed. The remaining P1 truth boundary is bounded exact-integrated-SHA physical acceptance/recovery before any P1-completion or `KNOWN_GOOD` claim.
