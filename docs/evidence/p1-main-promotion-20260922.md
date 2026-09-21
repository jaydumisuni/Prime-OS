# P1 Main Promotion Record — 2026-09-22

## Scope

`main` promotion integrates the P1 final-UI lineage, canonical P1 proof lineage, and current recovery/planning authority. It does **not** declare P1 phase completion and does **not** promote a generation to `KNOWN_GOOD`.

## Bound identities

- previous `main`: `d20d0a1337d2d5890ef3ac8384fde55d3bf482d9`
- executable integrated proof source: `086ee97f4bef57f78b725af9b220e98faf9f87a1`
- proof/status evidence child before final recovery wording: `8822d58`
- canonical local/UEFI proof: `P1_LOCAL_PROOF=PASS`
- proof report SHA-256: `4fe99fab39b5878a610643f5411f5896c31b7271ed53fe6bc2711923a4be9296`
- QCOW2 SHA-256: `d2240fb54921bf65aad1a7f003216164e71c75831da6f8db761a62006c94f667`

## Review / prove

- Rust source proof: 140 PASS, 0 FAIL.
- workspace clippy and formatting: PASS.
- canonical sealed image / OVMF proof at `086ee97`: PASS.
- Sergeant full `origin/main..candidate` changed-file review: 156 files, `APPROVE`, confidence 0.88, zero required actions.
- Sergeant final-proof: PASS, zero blockers.
- `git push --dry-run origin HEAD:main`: fast-forward accepted.

## Explicit non-claims

The promotion does not transfer historical physical evidence onto the integrated SHA. Exact-integrated-SHA physical KRATOS acceptance, owner visual acceptance, recovery-boot acceptance, and `KNOWN_GOOD` remain pending. W8 remains `HEALTHY_WAITING` on an authorized digest-bound Windows guest.
