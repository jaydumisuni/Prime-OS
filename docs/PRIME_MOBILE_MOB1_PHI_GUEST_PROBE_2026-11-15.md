# Prime Mobile MOB-1 — read-only PHI memory evidence in AArch64 early-init

Date: 2026-11-15. Status: **candidate, cross-compiled and host-test verified only**. Owning review lane: [Prime-OS draft PR #224](https://github.com/jaydumisuni/Prime-OS/pull/224). This extends the pre-Core Linux `/init` diagnostic fixture; it is NOT Prime Core, a second PHI daemon, a QEMU/Linux guest boot, or a Samsung image.

## Evidence recovered before editing

- The existing PR #224 and its build/console correction receipts, exact committed `early_init.c`, `verify_serial.py`, cpio builder and bounded QEMU adapter were recovered and verified.
- Generic PHI direction and frozen Prime hardware/capability/workload contracts remain authoritative. Existing MOB-0 / PHI Python probes in draft PR #223 remain separate. Do not merge these proofs or create another hardware authority.
- This isolated workspace has Clang 17/LLD but no QEMU binary, reviewed ARM64 Linux kernel Image, Rust/cargo or connected phone. DNS/HTTP package downloads failed. No actual ARM64 instructions executed.

## New guest observations and source limits

The **existing** static AArch64 Linux early `/init` uses only actual kernel-backed read-only paths (after mounting `/proc` and `/sys` inside the future guest). It now emits these additional bounded UART fields:

| UART field | Guest Linux source | Interpretation |
| --- | --- | --- |
| `mem_available_kib` | `/proc/meminfo` / `MemAvailable:` | Kernel estimated available RAM, distinct from physical MemTotal |
| `swap_total_kib` | `/proc/meminfo` / `SwapTotal:` | Kernel swap size, NOT physical RAM |
| `swap_free_kib` | `/proc/meminfo` / `SwapFree:` | Swap still free, NOT allocatable fast DRAM |
| `psi_memory_some_avg10` | `/proc/pressure/memory` / `some avg10=` | Kernel memory pressure signal if PSI compiled/enabled; not a benchmark |
| `zswap_state` | `/sys/module/zswap/parameters/enabled` | ENABLED / DISABLED / UNKNOWN; enablement does NOT prove zswap has been used or saved memory |

The existing `mem_total_kib` and `device_tree_model` fields are retained. No model runtime, Colibri, cloud inference, storage-backed swap changes, kernel tuning, writes, filesystem mounts beyond prior temporary `/proc` and `/sys`, hardware access or kernel module insertion were added.

Absent, unreadable or invalid kernel entries are printed `UNKNOWN`, rather than invented as `0`. The host serial parser treats optional PHI fields as `None` when unknown or missing; it **does not change** the frozen Prime Capability Interface schema or assert user-approved memory policy.

## Host verification and hostile inputs

`python3 -m unittest discover -s mobile/first-light/tests -q` completed: **49 / 49 PASS**, including prior 33 and 16 new PHI/source-order tests. New tests cover real AArch64 compiled marker availability, consistent physical/available RAM, swap free <= total, oversized/negative counters, missing PSI, invalid/nonfinite PSI, unrecognized zswap, duplicate fields, and serial marker injections before guest startup/after completion or embedded inside unrelated messages.

The serial parser is a **structural validator of untrusted raw text**, not a live iAM/COG runtime or hardware attestation. A synthetic test string is not boot evidence. Even a valid captured QEMU serial log can establish only `GUEST_EARLY_INIT_OBSERVED_ONLY` after independent QEMU/kernel/initramfs process and hash receipt, never `SHELL_READY`, PHI optimization success or approved phone support.

Independent Clang/LLD builds in two fresh directories gave byte-identical outputs; `readelf -h` identified `ELF64`, `ET_EXEC`, machine `AArch64`, entry `0x210484`. GNU `cpio -itv` independently displayed `dev/console` character 5:1, `dev/null` 1:3 and an executable `/init` (6424 bytes). New deterministic output digests:

- `init-aarch64`: SHA256 `f4aa2dca509bfee0ff5e76018e9250cc0270e0abb6c26f2cb9a41a16f100bc83`
- `initramfs.cpio`: SHA256 `01cd2d04dadb775fd9491b1900631ccdd3bc312ec3f1c4e7bc5f25460e1fa967`
- `initramfs.cpio.gz`: SHA256 `a495ed8af5d8e98bb418b8aacbf9a27f27ba26cd7670403dde314282e3930761`

The preceding console-correction hashes (ELF `ea11e6...`, archive `9ecb647...`) are historical and were **superseded by recompilation** after adding the five PHI observations. A binary with old digests is not the current candidate; the QEMU adapter requires these new pins.

## Reproduction

```bash
python3 -m unittest discover -s mobile/first-light/tests -q
python3 mobile/first-light/build.py --out .workspace/mobile/phi-mob1-a
python3 mobile/first-light/build.py --out .workspace/mobile/phi-mob1-b
sha256sum .workspace/mobile/phi-mob1-{a,b}/{init-aarch64,initramfs.cpio,initramfs.cpio.gz}
readelf -h .workspace/mobile/phi-mob1-a/init-aarch64
cpio -itv < .workspace/mobile/phi-mob1-a/initramfs.cpio
```

A later actual QEMU launch needs an independently reviewed AArch64 Linux kernel Image and independently sourced pinned emulator from the existing TTG Virtual Hardware Lab, not the D-XQT shadow event runtime. Run QEMU with no real device/network passthrough and enforce a bounded process/captured-output receipt.

## Engineering disposition

**New:** bounded read-only PHI observation fields in existing compiled AArch64 pre-Core Linux diagnostic. **Update existing:** host UART parser/test coverage and MOB-1 proof receipt. **Already documented — No update required** for Prime Hardware Graph v1, Workload Policy v1, iAM/COG design, the MOB-0 PHI prototype or the virtual hardware simulator; neither is duplicated. **Not achieved:** real Linux guest boot, Rust Core ARM64 crossbuild, PHI actuator/policy, Samsung hardware/drivers, release image, flash, daily-driver certification, independent review, or PR merge.
