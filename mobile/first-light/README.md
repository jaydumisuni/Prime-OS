# Prime OS Mobile MOB-1 — ARM64 Linux early-init candidate

**Status:** cross-compiled, ELF/archive-verified; **NOT booted**. No physical device, QEMU, Rust Prime Core or Samsung image was used. This is a throwaway Linux **pre-Core diagnostic /init**, not Prime Core, a mobile ROM, a production init system, or Android app compatibility.

## Why this exists

The Prime Mobile roadmap requires a separate ARM64 virtual First Light before physical phone changes. With KRATOS unavailable, the workspace has Clang 17 + LLD but neither `qemu-system-aarch64`, an ARM64 Linux `Image`, nor a Rust toolchain. This bounded fixture proves ARM64 cross-compilation and a boot-stage payload can be prepared **now**; execution is a later test with the existing TTG Virtual Hardware Lab's pinned QEMU facility.

## Contents

- `src/early_init.c` — independently reviewed freestanding AArch64 Linux PID1 fixture. Uses direct Linux syscalls to print actual kernel-reported architecture, PID1 state, `/proc/meminfo` physical-memory total and Device Tree model if available. Unknowns stay unknown. Mounts only `/proc` and `/sys` inside a future guest. No host filesystem writes, USB, networking, firmware, bootloader or device access.
- `build.py` — builds a true statically linked AArch64 ELF (machine=183, valid executable PT_LOAD entry, no PT_INTERP / PT_DYNAMIC) using Clang/LLD, then constructs reproducible cpio `newc` initramfs and `gzip` timestamp zero. The archive explicitly provides `/dev/console` (character 5:1) and `/dev/null` (character 1:3), with a fail-closed cpio verifier; otherwise guest PID 1 may have no usable stdout/stderr for proof. Verifies ELF and marker integrity. No installation/download or emulation.
- `verify_serial.py` — bounded parser for a *future captured* QEMU guest UART log. Requires correct exact marker order, arm64 kernel machine, PID1, proc mount and nonzero memory. Produces only `GUEST_EARLY_INIT_OBSERVED_ONLY`; never `SHELL_READY`, `KNOWN_GOOD`, a phone acceptance or flash authority.
- `tests/test_first_light.py` — deterministic source/build/replay and hostile input checks.

## Build locally (Clang/LLD only)

```bash
python3 mobile/first-light/build.py --out .workspace/mobile-mob1-first-light
python3 -m unittest discover -s mobile/first-light/tests -v
file .workspace/mobile-mob1-first-light/init-aarch64
readelf -h .workspace/mobile-mob1-first-light/init-aarch64
```

The output `initramfs.cpio.gz` is a candidate generic ARM64 Linux initramfs. **It is not a flashable phone image.** The receipt deliberately says `arm64_guest_boot_observed=false` and `prime_core_compiled=false`.

## Candidate PHI read-only memory evidence (2026-11-15)

The same cross-compiled AArch64 `/init` now emits kernel-reported `MemAvailable`, `SwapTotal`, `SwapFree`, PSI memory `some avg10`, and zswap enablement, each as an optional `PRIME_MOB1_V0|...` UART marker. Missing data is `UNKNOWN`, not zero. The serial parser verifies numeric consistency and rejects markers embedded in unrelated messages or outside the init stage window. It never claims actual PHI memory optimization, boot attestation, Core/Shell readiness or physical device support.

See [the versioned source-and-proof receipt](../../docs/PRIME_MOBILE_MOB1_PHI_GUEST_PROBE_2026-11-15.md). Run `python3 -m unittest discover -s mobile/first-light/tests -q`; all **49** host tests pass. The AArch64 ELF and gzip initramfs digests have changed relative to the historical MOB-1 console correction. **Rebuild and re-pin before any guest execution.** No guest boot has occurred in this workspace.

## Future QEMU guest proof — existing lab owns the emulator

Recover and use the [Virtual Hardware Lab G0.7 ARM64 QEMU path](https://github.com/jaydumisuni/TTG-Virtual-Hardware-Lab/blob/main/docs/G07_QEMU_GENERIC_ARM_EXECUTION.md), including its pinned emulator and isolated workspace. **This stage additionally needs an independently reviewed ARM64 Linux kernel Image** with initramfs support; the G0.7 bare-metal UART firmware is *not* a Linux kernel and cannot directly execute `/init`.

An illustrative, intentionally **non-executed** command once the kernel/emulator are available:

```bash
qemu-system-aarch64 -M virt -cpu cortex-a57 -m 512 \
  -nographic -no-reboot -net none -monitor none \
  -kernel /reviewed/path/to/arm64/Image \
  -initrd .workspace/mobile-mob1-first-light/initramfs.cpio.gz \
  -append 'console=ttyAMA0 rdinit=/init loglevel=4'
```

Use a bounded child process, no actual phone/USB passthrough, and preserve emulator/kernel/image digests and original guest output in an evidence receipt. Pass the raw serial log to `verify_serial.py` for preliminary classification. Simply feeding fabricated log lines to the parser is **not** a real boot; only a captured, authenticated process observation can count toward guest First Light. The user must not flash the generated cpio/ELF into Samsung boot, recovery or vendor partitions.

## Pinned QEMU guest-proof adapter (new, offline-tested preflight)

`run_qemu_guest.py` is a thin adapter around the **existing TTG Virtual Hardware
Lab's QEMU**, not another emulator. It verifies exactly pinned SHA-256 digests
for the reviewed local `qemu-system-aarch64` ELF, raw ARM64 Linux `Image` and
this corrected initramfs. It inspects the ARM64 Linux Image magic and checks
that the initramfs contains the real console device. No package install or
runtime network is performed.

The tool defaults to **admission only**, performing no guest execution. Only
`--run` launches a subprocess with `virt`, 512 MiB, PL011 UART, no network,
USB passthrough or drive, and a 3–90 second bounded timeout. It writes raw
guest UART, QEMU stderr and a candidate receipt to the specified output path.
A complete guest early-init marker sequence is still **not Prime Core
readiness**, a phone port or flash approval.

```bash
python3 mobile/first-light/run_qemu_guest.py \
  --qemu /REVIEWED/qemu-system-aarch64 \
  --qemu-sha256 <verified-emulator-sha256> \
  --kernel /REVIEWED/arm64/Image \
  --kernel-sha256 <verified-kernel-sha256> \
  --initramfs .workspace/mobile-mob1-first-light/initramfs.cpio.gz \
  --initramfs-sha256 9ecb64769e08a54ca290a3d2f08ea629c0b077b1278cb749f1e515c0056f9f28 \
  --output .workspace/mobile-mob1-guest-proof
# Add --run only after reviewing the pinned emulator and kernel provenance.
```

The provided checksum is for this correction's gzip and will change if the
payload changes. Without an actual trusted emulator/kernel, the command
cannot qualify a guest and must fail closed. The script does not sandbox
arbitrary untrusted guests; only reviewed test images are within scope.

## What a later Prime Core port still requires

1. Recover exact `prime-contracts`, `prime-hardware`, `primed`, service/generation/security and mobile-port authorities before cross-building production Rust crates with an approved AArch64 Rust target and proper Linux sysroot.
2. Verify core service launch and `prime.capability.v1` version negotiation in an actual arm64 Linux guest (not merely the `/init` marker). Build source digest ≠ runtime proof.
3. Adapt generic ARM64 board discovery to DT/ACPI and vendor firmware where justified; preserve unknown/driver-missing truth.
4. Physically verify vendor kernel/HAL, modem/telephony, touch, display, USB-C, thermal, battery, radio, update and recovery separately on owner-authorized hardware.

**Engineering classification:** new isolated MOB-1 preparation + mechanical static verification; **no change** to Prime Core, PHI, frozen contracts, Desktop P1 or physical device. Never market this as a mobile OS boot.
