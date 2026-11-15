# Prime PHI ARM64 — Read-only Rust Hardware Observation Candidate

Date: 2026-11-15. Status: **unmerged, review-only source candidate**. This is not a proven ARM64 build, a bootable Prime Core, a supported Samsung phone, a PHI performance optimization or a frozen contract.

## Why this code exists

Prime's existing `prime-hardware` scanner (`crates/prime-hardware/src/lib.rs`, main) collects DMI/SMBIOS identifiers, x86 `/proc/cpuinfo` fields, and PCI devices. ARM64 phone kernels commonly expose Device Tree and platform-bus devices instead. The current P1 hardware acceptance is correctly specific to KRATOS and must remain unchanged. The existing Prime Mobile Python `board_readonly_probe.py` in draft PR #223 already provides a candidate read-only Device Tree probe; we reuse its candidate schema name, not invent a new Host Identity authority.

## Exact source boundary

New additive module: `crates/prime-hardware/src/arm64.rs`, exported by a single additive `pub mod arm64;` in the existing crate. `prime_hardware::probe()` is NOT replaced, and no production caller is changed. All existing public `prime.hardware-graph.v1`, Prime Host Identity and Workload Policy structures/acceptance remain unchanged. We do not update frozen P1 contracts or redefine driver trust.

The API `prime_hardware::arm64::observe(root, host_arch)` returns in-memory, source-based **unqualified observations**:

- Device Tree `model` and NUL-separated `compatible` strings, read from `/proc/device-tree` or `/sys/firmware/devicetree/base` when present; provenance path is retained.
- `/proc/sys/kernel/osrelease`, and `/sys/devices/system/cpu/{present,online}` sorted CPU number sets. If online is not a subset of present, an inconsistency limitation is recorded.
- `/sys/bus/platform/devices` kernel names and each actually existing driver symlink binding, with strict filesystem root confinement for fixtures; **bound ≠ functioning**. PCI may still be valid on other hardware and is not disabled.
- Explicit limitations for missing, conflicting, truncated or outside-root observations; bounded file reads (4 KiB), finite CPU indexes and device count.
- No serial, hardware key, IMEI, eFuse, network MAC, USB ID, phone owner data, private bootloader slot, or partition contents.

`candidate_json()` offers an **additive development projection** under `prime.mobile.board-observation.candidate.v0` for comparison with PR #223's Python prototype; it marks `host_arch_attested=false`, `source_is_hardware_attestation=false`, `any_device_functionality_proven=false` and `firmware_or_device_modified=false`. Caller-supplied architecture and timestamp are **not attestation**. It does not enroll or reconcile a hardware fingerprint.

This is source-based mechanical discovery only. Live iAM/COG proof freshness, D-XQT admission, PHI resource policy, vendor kernel qualification, GPU/NPU compute runtime availability and signed device-package review remain separate authorities.

## Review and verification plan

Embedded Rust unit tests in `arm64.rs` cover:

1. Ordinary ARM64 Device Tree + CPU sets + platform driver-symlink observation, retaining unqualified status.
2. Missing Device Tree and platform bus remains unknown.
3. Non-ARM64 caller does not silently claim an ARM64 board.
4. NUL termination, invalid UTF-8 and malformed compatible strings.
5. CPU-list overlap, unreasonable ranges and online-not-present mismatch.
6. Escaping symlinks in either board model or platform device/driver are blocked under a fixture root.
7. Source-length bounds and omission of private board serial fields.
8. Deterministic repeated source observation and optional candidate JSON projection.

**Local execution limitation:** this isolated workspace has no `rustc`, `cargo`, ARM64 Linux kernel `Image` or `qemu-system-aarch64`. Do not claim the Rust module passed Cargo tests or an ARM64 guest boot. Mechanical source review and exact GitHub blob readback can be performed here, followed by `cargo test -p prime-hardware --lib` and a target `aarch64-unknown-linux-gnu` cross-build when a reviewed Rust toolchain is available. Those commands are pending proof, not accepted exit gates.

## Existing Prime authorities to preserve

- `docs/contracts/PRIME_HARDWARE_GRAPH_V1.md` — **frozen** v1 kernel inventory; no new silently accepted fields.
- `crates/primed/src/hardware.rs` — KRATOS P1 x86_64/HP/i915 hardware acceptance must remain unchanged.
- `crates/primed/src/identity.rs` — HIGH/MEDIUM enrollment still requires meaningful identity evidence; Device Tree `model` alone cannot make a stable Host ID.
- `docs/PRIME_MOBILE_ROADMAP_2026-11-15.md` — MOB-0 identity/boot-chain/restore and MOB-1 guest execution gates remain separate.
- `docs/PRIME_HARDWARE_INTELLIGENCE_PROPOSAL_2026-11-15.md` — shared PHI must not become a competing scheduler or mandatory AI model.
- `docs/PRIME_MOBILE_ARM64_SHARED_CORE_GAP_AUDIT_2026-11-15.md` on draft PR #224 — source-level roadmap for later actual Core ARM64 start.
- Related draft PRs: Prime-OS #223 (PHI/MOB-0 Python observation), #224 (ARM64 early-init), TTG-Simulation #1 (D-XQT synthetic policy), Virtual-Hardware-Lab #3 (synthetic recovery).

## Next genuine runtime checks

1. Cargo compile/test the crate, including new module tests and existing x86 fixtures. Verify no existing graph bytes change on the same source fixtures.
2. Cross-build the shared crate(s) with reviewed AArch64 Rust target and Linux linker, using recorded artifact hashes.
3. Run on generic QEMU ARM64 Linux after independently verified kernel/emulator availability, then compare Rust observations to guest `proc`/`sysfs` evidence and Python candidate behavior.
4. Only after independent code review and contract-version decision should Core optionally consume additive ARM64 observations. A physical phone is a later, separate boot, driver, telephony, recovery and owner-authorization gate.

## Contribution classification

**New additive candidate Rust module + unexecuted test source + portability documentation.** Existing Core runtime, protected APIs, Desktop P1 and the physical lab phone remain unchanged. Source review does not substitute for compilation, runtime or hardware acceptance.
