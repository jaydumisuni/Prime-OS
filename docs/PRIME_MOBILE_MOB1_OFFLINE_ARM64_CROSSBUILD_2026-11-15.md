# Prime OS Mobile MOB-1 — Offline ARM64 Cross-Build Checkpoint

**Date:** 2026-11-15. **Classification:** NEW isolated source + real AArch64 cross-compilation + static verification. **Not ARM64 instruction execution, Linux boot, Prime Core port, usable mobile shell, ROM, physical Samsung support, qualified simulation generation, or owner acceptance.**

## Previous authority recovered

- `docs/PRIME_OS_MASTER_PLAN.md`: Prime Linux foundation; one local Core authority; frozen Host ID and Capability Interface; real machine-vs-model truth.
- `docs/PRIME_MOBILE_ROADMAP_2026-11-15.md`: independent Mobile MOB-0–MOB-9 proof; M-series does not replace Desktop P1.
- `docs/PRIME_HARDWARE_INTELLIGENCE_PROPOSAL_2026-11-15.md`: PHI shared Core/Mobile/Desktop resource observability. No new memory manager.
- `docs/contracts/PRIME_HARDWARE_GRAPH_V1.md`, `PRIME_CAPABILITY_INTERFACE_V1.md`: normalized kernel discovery and separate acceptance; generic devices are not the Desktop P1 proof fixture.
- Prime draft [PR #223](https://github.com/jaydumisuni/Prime-OS/pull/223): isolated MOB-0/PHI evidence/board tooling, 34 host tests; separate unmerged candidate.
- TTG-Simulation draft [PR #1](https://github.com/jaydumisuni/TTG-Simulation/pull/1): 19 D-XQT v6/WASM synthetic decision worlds; not an emulator.
- TTG Virtual Hardware Lab draft [PR #3](https://github.com/jaydumisuni/TTG-Virtual-Hardware-Lab/pull/3): synthetic boot/flash fault recovery; not an ARM Linux guest.
- [TTG VLab G0.7](https://github.com/jaydumisuni/TTG-Virtual-Hardware-Lab/blob/main/docs/G07_QEMU_GENERIC_ARM_EXECUTION.md): existing *actual* ARM64 QEMU generic virt guest path on KRATOS; donor expected and not replicated here.

## Present workspace and implementation

In the isolated ChatGPT workspace, observed Clang 17.0.0 with AArch64 cross target and LLD 17.0.0. `cargo`, `rustc`, `qemu-system-aarch64`, an approved ARM64 Linux `Image` and the Samsung handset were unavailable. Network package retrieval was also unavailable. Therefore no execution or Android/Prime Core bring-up was attempted or reported.

Added `mobile/first-light` to an isolated Prime-OS **draft** branch:

1. `src/early_init.c`: self-contained freestanding Linux AArch64 `/init` using guest kernel syscalls (`mount`, `openat`, `read`, `write`, `uname`, `getpid`, `nanosleep`). Reports `INIT_ENTERED`, actual guest kernel machine/PID1, proc/sys mount availability, MemTotal and kernel-projected Device Tree model where readable. Fail-closed `UNKNOWN` values. Final stage always `EVIDENCE_ONLY_CORE_NOT_STARTED`, **never** `SHELL_READY`.
2. `build.py`: cross-compiles a real statically linked AArch64 ELF; checks ELF class, machine 183, executable entry/segments, no interpreter/dynamic load and required markers. Builds deterministic `newc` cpio with `/init`, `/proc`, `/sys`, `/dev` and gzip timestamp zero. No package/download, QEMU or device operation.
3. `verify_serial.py`: conservative future-guest-UART classifier, rejecting malformed/duplicate/out-of-order markers, false PID1 or wrong architecture and missing memory. Even on a valid trace it can only say `GUEST_EARLY_INIT_OBSERVED_ONLY` and has no Core/phone/flash authorization.
4. `tests/test_first_light.py`: **17 positive/negative unit tests** (independent cpio parser, wrong architecture/entry, missing marker, reproducibility, serial adversarial cases).
5. `README.md`: exact commands, division of authority and non-executed future QEMU example, requiring separately reviewed ARM64 Linux kernel Image.

## Exact GitHub-source reproduction receipt

All five source/test/README files were fetched back from the GitHub draft branch, compared **byte-for-byte** with the authored source, and built/tested in a separate clean reconstruction. Results:

- `python3 -m unittest discover -s mobile/first-light/tests -v`: **17/17 PASS** on reconstructed GitHub source.
- Independently repeated Clang+LLD compilations in separate output directories were byte-identical.
- ELF: **AArch64, ELF64 little-endian, ET_EXEC, static, 2 PT_LOAD segments, entry 0x21039c, 4616 bytes**; no unresolved external symbols, runtime libraries or dynamic interpreter.
- ELF SHA256: `ea11e6b1c442e587eda841a054e043b93836e775d740e706bef238b4e9925445`.
- `initramfs.cpio` SHA256: `a4013edae383fa9ed0034994f49d6ace9799cef74ecee7f054a37f6417efb603`.
- `initramfs.cpio.gz` SHA256: `e46c7d3381a9a4e7b4ea25bf80a447ca2fc0fd390065b05812a43ce780ae7b85`.
- Program-source `early_init.c` SHA256: `eaab08076ff7d8f54adf56dbafb6be6f7e35979a5a0e8170b99fb31517cc43c1`.
- GitHub-source status: same compiled artifact digests as the original source tree, with all source files byte-matching.
- ARM64 guest CPU instructions executed: **NO**. Linux `/init` executed: **NO**. Guest UART captured: **NO**. Rust Prime Core built for ARM64: **NO**. Physical phone modified: **NO**.

An executable architectural fixture is not proof of a completed operating system. The generated cpio is a **generic virtual ARM64 Linux initramfs**, not a kernel, Android system image, TWRP payload or flashable Samsung image. Do not pass the guest bytes to a live phone.

## First genuine Linux-guest proof gate (future, not executed)

Use the **existing** TTG VLab pinned ARM64 QEMU executable plus a legally usable, reviewed ARM64 Linux kernel Image with `CONFIG_BLK_DEV_INITRD`, `CONFIG_DEVTMPFS`, proc/sysfs and PL011 UART console support; explicitly bind emulator/kernel/initramfs hashes and capture bounded process/serial evidence. The example command is in `mobile/first-light/README.md`. QEMU `virt` is not MediaTek/Exynos/Qualcomm Samsung silicon.

A real execution receipt must independently show PID1, guest kernel machine `aarch64`, proc mount and memory reading. Only then may the guest early-init fixture be marked *observed*. To move beyond this fixture, cross-build and launch **existing Rust `prime-contracts`, `prime-hardware`, `primed`** with a qualified AArch64 Rust toolchain and Linux guest system environment and prove `prime.capability.v1` negotiation, recovery, Workload Policy and shell lifecycle. The user-approved lab phone stays untouched until a physically proven recovery route and explicit action authorization exist.

## Engineering handoff

- Contribution: **New** MOB-1 cross-build fixture/source, **Verification** of ARM64 ELF static characteristics, deterministic cpio packaging and negative trace classifier, **No Change** to actual Prime Core, PHI, Frozen P1, D-XQT engine, TTG VLab or physical handset.
- Freeze: the source/receipt can be reviewed, but **MOB-1 ARM64 guest First Light is NOT frozen/accepted**.
- Owner's rule: Understand → Build → Review → Freeze → Prove → Submit/Ship. Keep independent review and exact live world evidence separate.
- When KRATOS reconnects, first recover the current branch PRs and pinned guest environment. Do not rerun expensive research unnecessarily; do not use an unverified kernel or flash the a14m merely because a synthetic prototype exists.
