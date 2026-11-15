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

## 2026-11-15 candidate hardening — sysfs domain qualification

**Code correction on this same draft PR:** the original \`platform_devices()\` resolved a bus entry and considered it admissible whenever the target directory remained under the *observation root*. On live Linux the observation root can be \`/\`, making a syntactically forged bus-entry symlink to \`/tmp\` or another non-sysfs directory pass that insufficient check; similarly, a forged \`driver\` link to a directory under the observation root would be reported as "bound". The parent contract never authorizes promoting these path names to hardware/driver functionality.

The candidate now requires all of the following before reporting a bound platform driver:
- The platform-bus entry itself must be a **symlink**, not an arbitrary directory placed in a synthetic fixture.
- The resolved device path must lie under the canonical **\`/sys/devices\`** kernel device tree, not merely under a fixture root or \`/\`.
- The device's \`driver\` entry must itself be a **symlink**, not a plain directory.
- The resolved driver must lie under canonical **\`/sys/bus/platform/drivers\`**, not a forged arbitrary in-root directory.
- Missing kernel sysfs roots, invalid/unresolved entries and non-symlink entries produce explicit limitations. The original bounded 4 KiB reads, caller-architecture skepticism, private-ID omission, no Host ID enrollment and no writes are preserved.

**Regression coverage authored:** three new Rust unit tests target an in-root non-kernel "device" symlink, an in-root non-kernel "driver" symlink and ordinary directories posing as bus/driver symlinks. The existing outside-root negative test now expects \`PLATFORM_DEVICE_OUTSIDE_KERNEL_TREE\`, the more precise failure state. Rust source now contains **15 unit tests written; 0 executed**. Cargo/rustc and an AArch64 guest remain unavailable in this workspace.

**Executed narrow mechanical evidence, NOT Rust test results:** a temporary Python filesystem fixture established that an in-root \`tmp/forged\` directory passes the previous root-only path-prefix condition, but fails the new \`/sys/devices\` prefix condition; real \`/sys/devices/platform/real\` and \`/sys/bus/platform/drivers/real\` fixture directories pass their respective path-prefix conditions. This confirms the path-scope issue and intended boundary, **not** the compiled Rust branch behavior.

**Exact-file verification:** source SHA-256 \`b3ce547cfeec5a8cc1348e12af0a372c629c32a7e8fb786d354a798e67a23912\`, Git blob \`ecf0a7fc4a54b8a6ee4d60b44afdf52b952f7ecc\`; GitHub readback matches the locally edited file byte-for-byte. The existing \`prime_hardware::probe()\` path, Desktop P1 acceptance, and frozen Hardware Graph v1 remain untouched by this correction.

**Reviewer caveat:** bounded path resolution via \`canonicalize\` is appropriate for this read-only observational candidate but is **not** race-free openat2 containment or a security sandbox against a hostile privileged filesystem writer. A kernel sysfs binding and syntactically well-formed Device Tree content still cannot attest the board, firmware, driver functionality or handset support. Independent code review, actual Cargo tests, x86 regression and AArch64 guest proof remain mandatory. Do not merge or promote to Core readiness from this source review.

## 2026-11-15 Device Tree source-consistency correction — build proof pending

**Source defect confirmed by review:** the preceding `first_valid()` selected the first syntactically valid `model` or `compatible` value from two kernel Device Tree projections and ignored a second valid but conflicting value. On a normal live Linux system `/proc/device-tree` and `/sys/firmware/devicetree/base` normally describe the same tree. Silent preference would produce a misleading 'observed' board value in synthetic fixtures or degraded/misconfigured environments.

**Changed candidate:** an in-crate `consistent_valid<T: Eq>()` helper now compares every successfully parsed candidate source. When two valid values disagree, `kernel_model` / `kernel_compatible` and the corresponding source become `None`, with explicit `MODEL_SOURCES_CONFLICT` or `COMPATIBLE_SOURCES_CONFLICT` limitation. A single valid source or two agreeing sources remain usable as *unqualified* observations. Missing/unreadable paths are not silently called conflicting. The single-source kernel release observation still uses its existing first-valid path.

**Second gap:** the platform scanner previously made `/sys/bus/platform/drivers` an optional local variable without reporting that the directory itself was absent. It now records `PLATFORM_DRIVER_ROOT_UNAVAILABLE`, distinguishing unobservable driver-binding infrastructure from a genuinely unbound device. No driver binding is promoted to operational/physical support.

**New authored Rust unit tests:** contradictory valid model; contradictory valid compatible list; same values through both Device Tree projections; and missing driver directory with a visible platform device. Total `arm64.rs` unit tests now **19 authored, 0 executed**. The workspace has no `rustc` or `cargo`, no existing PR CI workflow runs, and outgoing DNS access is blocked. No compiler execution, Rust unit-test pass, AArch64 Rust artifact or guest boot is asserted. An independent Pygments token/delimiter structural check passed, but is NOT a Rust grammar/type checker. The four new tests and original 15 must all be run with `cargo test -p prime-hardware --lib` on a real Rust installation before merge.

**Exact GitHub readback:** `crates/prime-hardware/src/arm64.rs` SHA256 `e1526cbe83392e12e553b198782461a16afe1098ec948255fa9ac32726327169`, Git blob `5db7faf240b1769aba70222a489e60b144cd8c02`; remote content is identical to the locally edited file. Existing public signatures `observe()`, `candidate_json()` and the Desktop `prime_hardware::probe()` were preserved; no frozen contract, Host ID, scheduler or device firmware was changed.

**Disposition:** Correction + source verification only. Continue with a real Cargo test and AArch64 cross-build rather than accumulating unsupported acceptance claims. Keep this branch and PR draft.
