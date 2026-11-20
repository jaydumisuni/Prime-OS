# Prime Mobile MOB-1 — ARM64 initramfs console-node correction

**Date:** 2026-11-15  
**Status:** Corrected and locally verified cross-build candidate. **NOT A GUEST BOOT, PHONE PORT, OR PRIME CORE STARTUP.**
**Owning candidate:** [Prime-OS draft PR #224](https://github.com/jaydumisuni/Prime-OS/pull/224), branch `work/mobile-mob1-arm64-initramfs-20261115`. No production code or accepted frozen Prime contract changed.

## Finding: the original initramfs did not explicitly provide a Linux console

The original `mobile/first-light/build.py` generated `/dev`, `/proc`, `/sys`, `/init`, and `TRAILER!!!`, but no `/dev/console` character device. The kernel's opening of `/dev/console` before launching PID1 can fail if a usable console node is absent; then standard input/output/error may be closed and the pre-Core executable's `write(1,...)` proof markers can never be observed. Some kernel configurations supply default nodes, but a reproducible standalone initramfs should not rely on that incidental fallback. This is a pre-boot reliability defect, not evidence of a physical boot failure.

Linux authoritative documentation for `newc` archives explicitly demonstrates a `/dev/console` node using character major 5/minor 1:
https://www.kernel.org/doc/html/latest/filesystems/ramfs-rootfs-initramfs.html

Linux kernel discussion of failed initial console opening:
https://lore.kernel.org/all/20260406090815.1495095-1-safinaskar@gmail.com/

## Source correction

- `newc_entry(...)` now admits explicit bounded `rdevmajor` and `rdevminor` numbers in the correct fields of a `070701` cpio record, with no `mknod`, privilege escalation, host mutation or shell command.
- `initramfs()` now packages `dev/console` as a character-device entry (major **5**, minor **1**, mode `020600`) and `dev/null` (major **1**, minor **3**, mode `020666`) between the `dev` directory and the other entries.
- `inspect_initramfs()` fails closed for absent/unexpected/duplicate/truncated cpio records, invalid initial ELF payload, malformed device type/major/minor, trailing bytes or missing executable mode on `/init`.
- The build calls `inspect_initramfs()` before issuing a receipt, and records `initramfs_structure`.
- Host-side tests verify the nodes and their exact major/minor numbers, reject a missing `dev/console`, reject the wrong console major, and reject extra archive bytes. Existing static ELF and serial marker checks are retained.

## Observed in this isolated ChatGPT container

Environment: x86-64 Linux, Clang 17 and LLD 17. `qemu-system-aarch64`, Rust/cargo, and an approved Linux AArch64 `Image` were not locally available; outbound DNS to GitHub and Debian package hosts failed, so this session did **not** download an emulator or boot a guest.

The original saved ZIP was unpacked and independently tested before modification:
- Original **17/17** local unit tests PASS.
- Original ELF SHA-256 `ea11e6b1c442e587eda841a054e043b93836e775d740e706bef238b4e9925445`.
- Original `initramfs.cpio` SHA-256 `a4013edae383fa9ed0034994f49d6ace9799cef74ecee7f054a37f6417efb603`.
- Original `initramfs.cpio.gz` SHA-256 `e46c7d3381a9a4e7b4ea25bf80a447ca2fc0fd390065b05812a43ce780ae7b85`.

After the correction, the same Clang/LLD produced:
- **21/21** host-side tests PASS, including four new negative/positive archive-node cases.
- ARM64 ELF: **4616 bytes**, SHA-256 **`ea11e6b1c442e587eda841a054e043b93836e775d740e706bef238b4e9925445`**. **Unchanged** AArch64 executable bytes.
- Corrected `initramfs.cpio`: **5448 bytes**, SHA-256 **`646f0ad4150dc1be7cdc6ff44e4ea141e5950876bbf3a2b6da0de8c75f66f018`**.
- Corrected `initramfs.cpio.gz`: **2364 bytes**, SHA-256 **`9ecb64769e08a54ca290a3d2f08ea629c0b077b1278cb749f1e515c0056f9f28`**.
- Two independent corrected builds produced identical SHA-256 checksums.
- The exact three updated files were read back from GitHub, and their Git blob SHA-1s matched the independently computed local `git hash-object` values:

| File | Exact committed Git blob |
| --- | --- |
| `mobile/first-light/build.py` | `5afda1def3643775cecb10d5684203fcbf009ba5` |
| `mobile/first-light/tests/test_first_light.py` | `3cc5ebed58db1983392ffe3492db953f8c15dc9d` |
| `mobile/first-light/README.md` | `d6d301dd8e43c5cd721cf7db88f8429e974bfdd5` |

Independent format cross-check: the system GNU `cpio -itv` (not the Python archive validator) parsed the corrected archive and explicitly displayed `dev/console` as `crw------- 5,1`, `dev/null` as `crw-rw-rw- 1,3`, executable `/init` 4616 bytes, and the expected directories. `file` independently identified an SVR4 newc cpio; `readelf -h` independently identified ELF64 AArch64, ET_EXEC, entry 0x21039c. This is archive-format corroboration, **not** guest execution.

All associated source files remain in the same PR #224 branch. The original `docs/PRIME_MOBILE_MOB1_OFFLINE_ARM64_CROSSBUILD_2026-11-15.md` is retained as a **historical** build receipt; its original archive hashes describe the earlier, incomplete initramfs and are superseded for new builds by this correction.

## Bounded QEMU guest-proof adapter added — 2026-11-15

The same MOB-1 draft branch also includes `mobile/first-light/run_qemu_guest.py` and `mobile/first-light/tests/test_qemu_admission.py`. This is a small host adapter to **borrow the existing Virtual Hardware Lab's QEMU**, not a new emulator or separate authority. It requires caller-supplied external SHA256 pins for the actual emulator, ARM64 Linux kernel and corrected initramfs, validates executable/image magic and exact cpio device nodes, then prepares an allowlisted no-network/no-USB/no-host-disk command. Without explicit `--run` it executes nothing. With `--run` it bounds subprocess lifetime (3–90 seconds), captures guest UART and stderr, and uses the existing `verify_serial.evaluate()` to return at most `GUEST_EARLY_INIT_OBSERVED_ONLY`. It always reports Prime Core/Shell/device/flash acceptance as FALSE.

Kernel header validation uses the documented ARM64 raw `Image` magic `0x644d5241` at byte offset 56; this checks format, **not kernel provenance**: https://www.kernel.org/doc/html/latest/arch/arm64/booting.html . The QEMU executable and kernel must still be independently sourced and reviewed; a caller-provided checksum does not establish trust. The runner has no malicious-guest sandbox guarantee.

The runner is **host-side implementation only**: synthetic negative fixtures cover mismatched hashes, wrong QEMU/kernel formats, missing emulator, bad gzip, oversized gzip expansion, missing console in the older archive, unsafe timeout and forbidden QEMU device flags. The expanded suite is **33/33 PASS** from local source. Three new/updated GitHub files were read back and independently Git-blob matched to their locally tested bytes:
- `mobile/first-light/run_qemu_guest.py`: `3e61f7474838af222c23fe895666d91354aa0bb8`;
- `mobile/first-light/tests/test_qemu_admission.py`: `aeccd74359642feaa98c28d7c58210790624bf0b`;
- `mobile/first-light/README.md`: `c8bd2133b4836b3e846f6a03d1ab425515ddeba5`.

**No actual QEMU guest was launched here.** Neither `qemu-system-aarch64` nor a trusted ARM64 Linux `Image` is in this workspace, and direct package downloads failed due blocked DNS. The runner is a reproducible acquisition/boot handoff, not the missing execution proof.

## What this does and does not prove

**Proved locally:** reproducible genuine AArch64 executable compilation; correct and checked `newc` device nodes; build integrity and hostile-archive rejection; no host mutation.

**Not proved:** actual ARM64 instruction execution, booting Linux inside QEMU, a logged real PID1, Prime Rust Core or Shell readiness, verified physical device identity, Samsung kernel/DT/driver support, modem/telephony, PHI power/swap tuning or safe firmware flash.

**Engineering next gate:** reuse TTG-Virtual-Hardware-Lab's already documented pinned `qemu-system-aarch64` (G0.7), obtain a reviewed AArch64 Linux kernel `Image`, boot this exact corrected initramfs in a bounded, no-network, no-USB-pass-through guest and preserve the emulator/kernel/initramfs digests with captured stdout and `verify_serial.py` output. Only then promote the narrow **guest early-init observed** state. An ARM64 Rust cross-build and Prime Core capability handshake remain separate later gates.

**Contribution classification:** **Correction + Verification**. No new PR, simulator, firmware or separate Core was created. The corrected file set is still a draft candidate pending independent review.
