# PHI ARM64 observation CLI — isolated candidate, not Core readiness

**Date:** 2026-11-15  
**Target:** existing Prime-OS draft PR #225 (`work/phi-arm64-board-discovery-20261115`).  
**Status:** NEW source-level candidate; `rustc`/Cargo unavailable in the isolated workspace. **No Rust compilation or Rust unit test passed here.** No ARM64 Linux guest or handset executed.

## What is implemented

`crates/prime-hardware/src/bin/prime-phi-arm64-observe.rs` adds an optional Cargo binary target that consumes the **same library function** `prime_hardware::arm64::observe()` and its **same** candidate-only `candidate_json()` projection. It does not modify the existing `prime_hardware::probe()` code, hardware graph v1, Host Identity, the P1 Desktop acceptance evaluator, the Workload Policy or privileged service interfaces. It does not create a new PHI daemon, scheduling engine or iAM authority. This is a step toward collecting real ARM64 Linux kernel observations on generic QEMU and, after authorization, a physical test system.

The interface is intentionally two-mode and fail-closed:

```sh
# Only when the binary has been built and installed on a real Linux guest:
prime-phi-arm64-observe --live

# Reproducible synthetic read-only fixture, never a live device claim:
prime-phi-arm64-observe --fixture-root /path/to/fixture --arch aarch64
```

`--live` reads only fixed `/proc` and `/sys` paths exposed by the running Linux kernel and gets the build target arch from Rust's `std::env::consts::ARCH`. This is **not** cryptographic attestation. `--fixture-root` needs a directory other than `/` and makes its architecture a *caller claim*. It does not silently reinterpret a test directory as a live handset. The output has `capture_mode: LIVE_KERNEL_READ_ONLY | SYNTHETIC_FIXTURE`, `host_clock_unix_ms_unattested`, `collector_version`, and `live_device_qualification: false`, in addition to the existing candidate board fields and always-false `any_device_functionality_proven` / `firmware_or_device_modified` values.

Only stdout is used for JSON output, with stderr for bounded argument errors. There is no output file, device node, firmware, bootloader or USB interface mutation. The collector never triggers any Prime Core host identity enrollment or device certification. Its observation helper uses bounded reads and source-dependent `UNKNOWN` limitations (see the ARM64 Rust checkpoint).

## Source verification (not Cargo tests)

- Exact source and original contract recovered before implementation; current PR #225 at `bf1d5b89e8183f71ce0b4bd1c88bc4832da5cce5` before this new candidate.
- `src/bin/` is Cargo's standard location for additional binary targets. No changes to `Cargo.toml` are required for this conventional target layout.
- Six in-source Rust tests **authored**, covering mutually exclusive capture mode, refused duplicates, fixture architecture claims, ambiguous help/mode, refusal to name `/` a fixture, and synthetic output's never-qualified fields.
- Narrow host-side textual checks verified the new source has no explicit `std::process::Command` or `fs::write` and invokes the existing `arm64::observe()` path. These checks do **not** certify the Rust compiler, system call behavior, or absence of all possible side effects.
- Main ARM64 adapter remains untouched from source commit `bf1d5b89...`; its 19 Rust tests remain **authored but not executed**.

## Actual compile and acceptance gates — still required

On a reviewed Rust toolchain with cached dependencies and a target linker:

```sh
cargo test -p prime-hardware --all-targets
cargo build -p prime-hardware --release --bin prime-phi-arm64-observe
cargo build -p prime-hardware --target aarch64-unknown-linux-gnu --release --bin prime-phi-arm64-observe
```

`--all-targets` is important: `--lib` alone does not execute tests inside the new `src/bin/` collector. Inspect the ELF target architecture (machine 183), runtime dependencies, and SHA-256 of the produced binary. Then in the **verified** generic ARM64 Linux guest, capture stdout/stderr with exact binary/kernel/emulator hashes, compare direct `/proc`/`/sys` observations, verify architecture, time uncertainty and provenance, and confirm that neither output nor Prime Core readiness claims imply phone driver acceptance. Run x86-64 pre-existing hardware tests independently.

The workstation lacks `rustc`, Cargo, `gccrs` (GCC's `crab1` frontend), `qemu-system-aarch64` and direct package retrieval (DNS failures verified). Do not count this source as a successfully built Rust program. Do not merge PR #225 based only on source review.

## Current engineering contribution

**NEW:** reusable, explicitly scoped optional Rust CLI for the already implemented ARM64 observation adapter. **Already documented — No update required:** frozen Hardware Graph v1, Host Identity v1, Desktop P1, Core/Workload Policy, iAM/COG evidence trust, D-XQT simulation, and the prior Device Tree conflict and sysfs containment corrections.  
**Next first-party gate:** actual `cargo test -p prime-hardware --all-targets`, then cross-build and guest execution before any production Core integration.


## 2026-11-15 superseding executable proof — Rust, ARM64 link, and QEMU user-mode

**Historical correction:** The earlier statements in this document that Cargo/rustc were unavailable and that the 25 new Rust tests were unexecuted are now superseded by actual GitHub-hosted verification. KRATOS and the local ChatGPT container were not used to run Rust; the restricted CI job used an isolated Ubuntu 24.04 GitHub-hosted runner, repository token with read-only permissions, no devices and no deployment.

**Exact verified run:** https://github.com/jaydumisuni/Prime-OS/actions/runs/43725251730 at commit 0772c0fff7a9acb610185542a505f9706fcc07c1, full job **SUCCESS**, Rust/Cargo **1.97.1**.

- **Cargo host tests:** cargo test --locked -p prime-hardware --all-targets -- --nocapture: 23/23 library (19 new ARM64 plus four existing scanner tests) and 6/6 CLI tests, **29/29 PASS**.
- **Cross-target Rust check:** rustup target add aarch64-unknown-linux-gnu, then cargo check --locked -p prime-hardware --all-targets --target aarch64-unknown-linux-gnu: **PASS**.
- **Actual ARM64 link:** cargo build --locked --release -p prime-hardware --bin prime-phi-arm64-observe --target aarch64-unknown-linux-gnu: **PASS** after providing cross gcc and libc6-dev-arm64-cross in the ephemeral runner. An earlier linker attempt had correctly failed with missing Scrt1.o/crti.o/glibc, which was fixed at the build-environment layer.
- **Result:** ELF64 AArch64 dynamically linked PIE, 642736 bytes, requires ARM64 glibc loader /lib/ld-linux-aarch64.so.1. Binary SHA256 **5ef2f0418944ed82f92c38a44a84cfcb492b276ed9b15fda0db44ed13ccbe6ed**.
- **Actual ARM64 instructions:** QEMU **user-mode** 8.2.2 executed the real collector with --fixture-root and --arch aarch64; checked model/compatibles, present/online CPU sets, bound synthetic platform driver, and all hardware-support/attestation/firmware-modification flags false. Step emitted **QEMU_AARCH64_USER_MODE_FIXTURE_OBSERVATION_PASS (not guest boot)**.
- **Artifact:** https://github.com/jaydumisuni/Prime-OS/actions/runs/43725251730/artifacts/14166266188 (binary, SHA file, QEMU version/help, captured synthetic JSON). ZIP SHA256 **79d3142b0d1c057aa9443d18fd4878b03b3dc25b2f81695acf8a590bd0943c22**. Artifact was independently downloaded and validated with sha256sum, file, readelf and JSON assertions outside CI.

**Explicit non-proofs:** QEMU user-mode uses the host Linux kernel. No ARM64 *guest kernel* boot, no Prime Core/Shell or capability handshake, PHI policy actuation, live phone hardware, modem/GPU/telephony, signed image or firmware flash is demonstrated. The executable is a dynamically linked diagnostic tool, not a standalone ROM or initramfs. The candidate remains **draft and unmerged** for independent review and later guest/device proof.

**Contribution:** Update existing documentation; real Rust compilation, regression tests, ARM64 cross-link, and limited real ARM64 user-space execution verification. A subsequent documentation-only commit changes the branch head without changing the already proven Rust source; cite the exact successful run and source Git blobs when checking freshness.
