# Prime OS — AI / Engineer Handoff

This file is the recovery entry point for any future chat, AI agent, engineer, or reviewer continuing Prime OS.

## Read first

1. `README.md`
2. `docs/PRIME_OS_MASTER_PLAN.md` — canonical product and architecture authority.
3. `docs/PRIME_STORAGE_INTELLIGENCE.md` — accepted storage-intelligence supplement and filesystem strategy.
4. `docs/PRIME_APPLE_FILESYSTEMS.md` — accepted Apple-storage supplement covering APFS, HFS+/HFS, encryption, snapshots/clones, Time Machine awareness, and disk-image handling.
5. `docs/PRIME_OS_ROADMAP.md` — operational phase sequence.
6. `docs/donors/WINDIRSTAT.md` — WinDirStat donor/licence/reference-oracle decision.
7. `docs/donors/APFS.md` — APFS/Apple storage donor/reference decision.
8. `planning/state.json` — machine-readable current state.

If any summary conflicts with the Master Plan, the Master Plan wins unless a later explicitly accepted supplement/amendment supersedes the narrower topic.

---

## Current state

- Product name: **Prime OS**.
- Prime is an integration-first development and execution operating system built on the Linux kernel foundation but not intended to be merely a themed distribution.
- Prime's implementation repository was intentionally kept free of product implementation while the product/architecture plan was being reviewed.
- The consolidated planning baseline is stored in this repository.
- Prime Storage Intelligence has an accepted planning supplement derived from WinDirStat and Linux/VFS filesystem research.
- Apple storage has an accepted planning supplement: APFS is a first-class foreign filesystem target, HFS+/HFS and Apple disk-image formats are explicit compatibility classes, and APFS is read-only-first until a dedicated write-safety proof earns more.
- Product implementation has **not** been performed in this planning workstream.
- The implementation workstream must recover the repository authority before coding.

## Permanent boundaries

- **Prime OS** = body / operating system / machine authority.
- **Prime Host** = one local physical or virtual machine running Prime. Prime Host authority is self-only; Prime has no global Host registry.
- **Prime Exec** = executable/runtime/backend determination.
- **Prime Workload Policy** = machine-level resource/security enforcement for every backend.
- **Prime Storage Intelligence** = Host-local mechanical storage inventory/index/accounting/change/cleanup-safety truth.
- **Origins Factory** = mission workspace and composition of capabilities.
- **Hunter** = intelligence/reasoning.
- **AgentOps** = semantic operation lifecycle.
- **Ptah** = neutral mechanical Workspace/execution substrate; Prime does not implement Ptah.
- **CodeOps** = engineering.
- **Sergeant** = independent engineering review, not CodeOps and not a per-application runtime certification daemon.
- **Grid-Knight** = cybersecurity interpretation/response/remediation; it may later consume Prime storage/file events but Prime Storage Intelligence does not become antivirus.
- **X-Ray** = specialist evidence.
- **Oracle** = authorized browser/OS control.
- **Lumi** = downloads/transfers.

## Prime Host identity rule

`Prime Host ID ≠ Origins Node ID ≠ Ptah Node ID`

Mappings are explicit:

`Prime Host → Prime Capability Interface → Origins adapter → Origins Node projection`

Later:

`Prime Host → Prime Capability Interface → Ptah adapter → Ptah Node/Providers/Facilities`

Moving Prime storage to materially different hardware does not silently preserve Host identity. Fresh hardware discovery and migration/re-enrollment are required.

## Core architecture contracts

Before implementation relies on them, recover/finalize the P0 hard contracts listed in the Master Plan and accepted supplements:

- Prime Host Identity v1.
- Prime Host capability/health model.
- Prime Exec model.
- Prime Application Profile v1.
- Profile revision pinning/schema migration/revocation rules.
- Prime Capability Interface v1.
- Interface negotiation/zero-overlap/deprecation rules.
- Prime Workload Policy v1.
- Network/resource/filesystem/device/secret policy.
- Driver trust tiers and Developer Mode driver policy.
- Storage/generation model.
- Prime Storage Intelligence contracts: Storage Inventory, Storage Index, generic scanner, filesystem adapter, metric/confidence semantics, Change Engine, cleanup safety states, event boundary, and proof fixtures.
- Filesystem strategy for ext4, Btrfs, XFS, NTFS, APFS, HFS+/HFS, exFAT/FAT, Apple disk images, generic Linux/VFS fallback, and virtual/network mounts.
- WinDirStat donor boundary: `STUDY / ADAPT / REFERENCE ORACLE`; native Rust Prime implementation; no direct GPLv2 C++ incorporation into Prime Core by default.
- APFS donor boundary: Apple specifications plus independent read-only implementations as references; APFS is read-only-first; `linux-apfs-rw` experimental writes are research only and are not ordinary Prime functionality.
- Apple-storage semantic contract: APFS container/volume space sharing, snapshots, clones/shared allocation, FileVault/encryption secret boundary, sealed/system-volume awareness, HFS+ metadata/resource-fork preservation, Time Machine awareness, and disk-image layering.
- Update/rollback/recovery architecture.
- Workload quiescence.
- Hardware graph/driver architecture.
- Build/image, init/service, and component/package architecture.
- Release Target and Provider model.
- Prime Shell / Prime Orb specification.
- Reference-video design study.
- Performance/proof/research-disposition matrices.

Do not silently choose technologies where the Master Plan or supplement says P0 must make an architecture decision.

---

## Storage Intelligence rule

Prime Storage Intelligence is **not** "WinDirStat for Linux" and does not port WinDirStat's C++ application into Prime Core.

The accepted route is:

`WinDirStat → study capability/architecture/UX → specify Prime equivalent → native Rust implementation → Prime Storage Intelligence`

Live mounted Linux-supported filesystems use kernel-mediated Linux/VFS truth as the default path. Prime uses a generic scanner and filesystem-specific enrichment for ext4/Btrfs/XFS/NTFS where correctness or meaningful additional semantics justify it.

Apple storage is explicit rather than hidden under generic adapters:

`APFS / HFS+ / HFS / Apple disk images → safe adapter/provider → normalized Prime Storage Intelligence truth`

APFS is read-only-first. Prime must not promote experimental APFS writes as normal support. Safe modification may use a proven future Prime backend or an appropriate macOS/Apple Provider.

Prime must distinguish logical, allocated, shared/exclusive when provable, reserved, and unknown/metadata usage rather than manufacturing exact physical ownership. This is especially important for Btrfs and APFS snapshots/clones/shared extents.

Cleanup uses `PROTECTED / RECLAIMABLE / REVIEW / UNKNOWN` safety classes.

---

## First implementation mission

The first bounded implementation mission is:

> **P1 — First Light: produce the first unmistakably Prime bootable system according to the frozen authority.**

P1 includes Prime identity/generation/Host identity, Prime Core, hardware graph, storage separation, base HP hardware support, Prime Exec foundation, Application Profile registry, Capability Interface v1, Workload Policy v1, Prime Shell, Prime Orb baseline, recovery entry, update-aware generation layout, and the minimum Prime Storage Intelligence foundation for capacity/reserve/generation accounting and update-space preflight.

For Apple media, P1 should safely recognize APFS/HFS+/HFS and report encrypted/locked/read-only/unsupported status honestly. It does not need APFS write support.

P1 does **not** include complete Windows Personality, Android Personality, Ptah, the full WinDirStat-equivalent storage analyzer, Darwin/iOS local compatibility, or full update/rollback proof.

The next phase, P1.5 Survival, proves real update/rollback and failure recovery.

---

## Planned sequence

`P0 Complete Load → P1 First Light → P1.5 Survival → P2 Development Body → P3 Origins Factory → P4A Windows Personality / P4B Android Personality → P5 Cross-Architecture → P6 Darwin Local Compatibility → P7 iOS Local Compatibility → P8 Distributed Prime Deployment`

Ptah runtime testing occurs from the Ptah workstream when Ptah reaches its own authorized test candidate and Prime is ready to host it.

---

## Implementation discipline

Do not infer completion from a successful build.

Each relevant phase requires positive, negative, failure, recovery, resource, security, hardware, compatibility, storage-truth, and rollback evidence as applicable. Sergeant reviews engineering changes when required. Owner acceptance remains required for subjective product decisions, especially visual quality.

If implementation exposes a real architectural contradiction:

`STOP → preserve evidence → amend planning authority → review → freeze correction → resume`

Do not patch architecture silently in implementation code.

## UI quality rule

Prime must look and behave like Prime from First Light. The supplied reference video is a design/interaction donor and quality bar. Do not boot into an obviously stock GNOME/KDE/Fedora/Ubuntu experience and call it complete.

## Resource rule

**Support broadly; activate narrowly.** Optional runtimes, VMs, SDKs, workers, models, compatibility services, storage hashes, deep filesystem scans, and foreign-filesystem helpers should not consume resources merely because Prime supports them.

## Developer-platform rule

`build ≠ execute ≠ sign ≠ publish`

Prime may build for a target before it can locally execute that target. Official/vendor environments and stores are optional Providers used only where required or intentionally selected.

## Recovery rule

A future chat or engineer should not need this original conversation to continue Prime. Recover the repository documents first and treat them as the durable source of truth.
