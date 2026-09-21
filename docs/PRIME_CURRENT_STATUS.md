# Prime OS — Current Implementation Status

Updated: 2026-09-22

This file is the canonical fast recovery record for **what has actually moved in implementation**. It does not replace the Master Plan, accepted supplements, frozen contracts, proof artifacts, or branch-local benchmark evidence. It must distinguish branch implementation from merge/release truth.

## Repository truth

- Canonical repository: `jaydumisuni/Prime-OS`.
- `main` now carries the integrated P1 source/proof/recovery lineage; the architecture/planning baseline is preserved in that history.
- P4A Windows remains branch-isolated and is not implied to be merged into the P1 line.
- The integrated P1 candidate joins final-UI, canonical proof, and current recovery/planning histories at exact proven revision `086ee97f4bef57f78b725af9b220e98faf9f87a1`.
- A branch being published does not mean that phase is shipped, merged, or promoted to `KNOWN_GOOD`.

## P1 — First Light

P1 is **implemented deeply enough to have canonical VM boot proof, KRATOS physical graphics proof, owner visual acceptance, and later UI refinement**, but P1 phase completion is not declared.

### Integrated P1 candidate

- `integration/p1-first-light-candidate-20260921` joins `work/p1-final-ui`, `proof/p1-first-light-final`, and current recovery/planning authority without rewriting their histories.
- Exact integrated proof source: `086ee97f4bef57f78b725af9b220e98faf9f87a1`.
- Canonical `tools/prove-p1-local.sh`: **PASS** on KRATOS through the audited privileged broker.
- Proof binds sealed Composefs, normal/recovery UKIs, QCOW2 identity, OVMF boot, persisted Prime Host/generation state, and mechanical `SHELL_READY`.
- Integrated-SHA `KNOWN_GOOD`, physical KRATOS boot acceptance, owner visual acceptance, and recovery-boot acceptance remain unearned.

### Boot / proof lineage

- `proof/p1-first-light-final` → `0398835`: sealed First-Light proof lineage with KRATOS OVMF pflash correction.
- `adapter/p1-image-builder-mount-probe` → published implementation head `83c45c2`; functional image-builder head remains `786018f`, with the later commit only ignoring local worktrees.
- Canonical proof work established UEFI/OVMF boot, Prime Host identity, generation identity, normal/recovery UKI separation, Composefs digest binding, Shell readiness, and recovery boot behavior.
- `KNOWN_GOOD` promotion is **not earned** in the P1 proof records.

### Physical KRATOS / visual lineage

- `work/p1-visual-fidelity-kratos` → `a4ae2c2`: owner-accepted First-Light visual candidate.
- `design/p1-first-light-visual` → published remote head `77df519`: final-UI KRATOS checkpoint lineage; its merge-base with the older local design worktree is `ee783a8`.
- `work/p1-system-wallpapers` → `1c2685d`: Prime system wallpaper lane.
- `proof/p1-ui-finalization-20260904` → `b8cae37`: a divergent interactive-UI proof lane created from `1c2685d`; it is not an ancestor of the later final-UI lane and its evidence must not be silently transferred.
- `work/p1-final-ui` → `263d1b7`: latest committed P1 UI refinement lane; `b3f52b7` is the final-UI implementation checkpoint and `77df519` records its KRATOS evidence checkpoint.

Evidence already recorded on the P1 UI lineage proves canonical `P1_LOCAL_PROOF=PASS` at `a4ae2c2`, direct KRATOS Intel UHD 630 / DRM / Wayland operation, Shell readiness and mapped-frame retirement, owner visual acceptance, recovery boot, deep S3 suspend/resume, attached USB2 input/device-path proof, Wi-Fi operation/recovery, real ALSA hardware-stream acceptance, software DRM topology invalidation/revalidation, and bounded performance/thermal behavior.

Truth gaps remain explicit:

- no `KNOWN_GOOD` generation promotion;
- physical USB3/SuperSpeed data-path proof requires an actual SuperSpeed device;
- live Ethernet carrier/traffic requires a connected link;
- audible playback still requires physical/human confirmation;
- physical display cable hotplug / DP proof remains external;
- final UI evidence is at `b3f52b7`; later visual commits `db15fe0` and `263d1b7` do not manufacture a new physical-interaction proof.

The local `design/p1-first-light-visual` worktree is still based at `ee783a8` and contains additional uncommitted changes, while GitHub's branch is five commits ahead at `77df519`. Those local dirty bytes are **not** part of the published remote head. Do not force-push or reset that worktree as part of recovery.

## P4A — Windows Personality

Windows Personality development is a **parallel branch-isolated lane** descending from the P1 image-builder baseline. It is not merged with the later P1 final-UI branch and must not be described as a single linear release.

| Wave | Scope | Head | Evidence state |
|---|---|---:|---|
| W1 | portable/simple Win32 | `4fe487f` | frozen |
| W2 | installers/common runtimes | `4296dd6` | frozen |
| W3 | managed .NET applications | `8af61f4` | frozen |
| W4 | DirectX/GPU acceleration | `f99c7f8` | **FROZEN / SHIPPED HOST CERTIFICATION**; later exact-SHA certification closed the preserved benchmark pending gates |
| W5 | COM | `706e90a` | **FROZEN**; frozen predecessor authority consumed by W6 |
| W6 | supported Windows services | `0428413` | **FROZEN / SHIPPED HOST CERTIFICATION**; exact-SHA replay, Formula predicates, Sergeant approval and remote binding completed externally |
| W7 | USB/device mediation | `8115b4c` | **FROZEN**; frozen predecessor authority consumed by W8 |
| W8 | VM fallback | `c2cc9de` | **HEALTHY_WAITING / authorized_guest**; control rebind complete, awaiting authorized digest-bound Windows guest proof |
| W9 | real TTG workload certification | — | not started |

### Certification precedence note

The W4–W7 benchmark JSON files are preserved snapshots and were not rewritten after later exact-SHA certification. Downstream authority resolves the state: W5 consumes frozen W4 `f99c7f8`, W6 consumes frozen W5 `706e90a`, W7 consumes frozen W6 `0428413`, and W8 consumes frozen W7 `8115b4c`. Do not regress W4–W7 to freeze-pending from older benchmark placeholders.

### W8 blocker

KRATOS already proves the host-side VM donor closure required by W8: QEMU x86_64 is available, KVM is usable, VM selection is explicit through `ExecutionBackend::Vm`, guest authority is digest-bound and fails closed, QEMU plans are bounded/isolated/network-disabled by default, and the guest-agent handoff contract is implemented and regression-tested.

The first irreducible W8 blocker is **authorized Windows guest execution**. The later Cookpit control rebind places Prime Windows in `HEALTHY_WAITING` on `authorized_guest`, so no safe source/control work remains before that external proof. A fresh KRATOS search found no QCOW2/VHDX/VMDK/ISO candidate. Guest-dependent runtime/isolation/adversarial gates remain unearned, and W9 must not start until W8 closes.

## Prime Manager / Prime Terminal

Do not conflate Prime OS branch names with the separate products:

- `jaydumisuni/Prime-Manager` local repository remains at roadmap baseline `375ca54` from 2026-09-05.
- `jaydumisuni/Prime-Terminal` local repository remains at roadmap baseline `a58628a` from 2026-09-05.
- The Prime OS ref `work/p1-prime-manager` currently aliases the P1 UI commit lineage and is not evidence that the separate Prime Manager product advanced.

## Current recovery rule

Recover in this order when the question is "where is Prime now?":

1. `README.md`
2. `docs/PRIME_OS_MASTER_PLAN.md`
3. accepted supplements
4. **`docs/PRIME_CURRENT_STATUS.md`**
5. `docs/PRIME_OS_ROADMAP.md`
6. `docs/AI_HANDOFF.md`
7. `planning/state.json`
8. branch-local proof/evidence files for the exact lane being continued

Use the exact branch and evidence record for implementation claims. Do not infer merge, freeze, release, `KNOWN_GOOD`, or physical proof from the existence of later source code alone.
