# Prime OS Mobile MOB-1 — ARM64 Linux early-init candidate

**Status:** cross-compiled, ELF/archive-verified; **NOT booted**. No physical device, QEMU, Rust Prime Core or Samsung image was used. This is a throwaway Linux **pre-Core diagnostic /init**, not Prime Core, a mobile ROM, a production init system, or Android app compatibility.

## Why this exists

The Prime Mobile roadmap requires a separate ARM64 virtual First Light before physical phone changes. With KRATOS unavailable, the workspace has Clang 17 + LLD but neither `qemu-system-aarch64`, an ARM64 Linux `Image`, nor a Rust toolchain. This bounded fixture proves ARM64 cross-compilation and a boot-stage payload can be prepared **now**; execution is a later test with the existing TTG Virtual Hardware Lab's pinned QEMU facility.

## Contents

- `src/early_init.c` — independently reviewed freestanding AArch64 Linux PID1 fixture. Uses direct Linux syscalls to print actual kernel-reported architecture, PID1 state, `/proc/meminfo` physical-memory total and Device Tree model if available. Unknowns stay unknown. Mounts only `/proc` and `/sys` inside a future guest. No host filesystem writes, USB, networking, firmware, bootloader or device access.
- `build.py` — builds a true statically linked AArch64 ELF (machine=183, valid executable PT_LOAD entry, no PT_INTERP / PT_DYNAMIC) using Clang/LLD, then constructs reproducible cpio `newc` initramfs and `gzip` timestamp zero. Verifies ELF and marker integrity. No installation/download or emulation.
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

## What a later Prime Core port still requires

1. Recover exact `prime-contracts`, `prime-hardware`, `primed`, service/generation/security and mobile-port authorities before cross-building production Rust crates with an approved AArch64 Rust target and proper Linux sysroot.
2. Verify core service launch and `prime.capability.v1` version negotiation in an actual arm64 Linux guest (not merely the `/init` marker). Build source digest ≠ runtime proof.
3. Adapt generic ARM64 board discovery to DT/ACPI and vendor firmware where justified; preserve unknown/driver-missing truth.
4. Physically verify vendor kernel/HAL, modem/telephony, touch, display, USB-C, thermal, battery, radio, update and recovery separately on owner-authorized hardware.

**Engineering classification:** new isolated MOB-1 preparation + mechanical static verification; **no change** to Prime Core, PHI, frozen contracts, Desktop P1 or physical device. Never market this as a mobile OS boot.
