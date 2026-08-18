# Prime OS — Implementation Roadmap

**Authority:** derived from `docs/PRIME_OS_MASTER_PLAN.md` and accepted supplements  
**Planning baseline:** accepted for handoff  
**Implementation:** P1 First Light is active in draft PR #1 on `build/p1-first-light`; future roadmap work must not silently expand the frozen P1 scope.

This file is the fast operational roadmap. The Master Plan remains canonical when this summary is ambiguous. Narrow accepted supplements, including `docs/PRIME_STORAGE_INTELLIGENCE.md` and `docs/PRIME_APPLE_FILESYSTEMS.md`, govern their specific subsystems where they add detail without contradicting the Master Plan.

Prime system-generation updates and optional component delivery are separate mechanisms. Prime's image-owned base is updated through the generation/update architecture; independently installable applications, Providers, runtimes, toolchains and optional capabilities are delivered through the Prime component/package architecture and, later, the Prime Store.

---

## P0 — Complete the load

**Purpose:** resolve the architecture before product implementation.

P0 must produce/recover the concrete contract and ADR set required by the Master Plan, including:

- Prime Host Identity v1 and self-only Host authority.
- Host hardware-change, migration, re-enrollment, lineage, rebind, and supersession rules.
- Prime Host → Origins Node projection contract.
- Future Prime Host → Ptah mapping boundary.
- Prime Exec model.
- Prime Application Profile v1.
- Profile revision pinning, schema migration, and revocation rules.
- Prime Capability Interface v1.
- Interface version negotiation, zero-overlap failure behavior, and deprecation rules.
- Prime Workload Policy v1.
- Network/resource/filesystem/device/secret policy.
- Driver trust tiers and Developer Mode driver policy.
- Storage/generation model.
- **Prime Storage Intelligence architecture:** Storage Inventory, Storage Index schema, generic scanner contract, filesystem-adapter contract, metric/confidence semantics, Change Engine, cleanup safety states, storage event boundary, and proof fixtures.
- **Filesystem strategy:** ext4, Btrfs, XFS, NTFS, APFS, HFS+/HFS, exFAT/FAT, Apple disk-image containers, generic Linux/VFS fallback, and rules for virtual/network filesystems.
- **Apple storage contract:** APFS container/volume space sharing, snapshots, clones/shared allocation, encryption/FileVault secret boundary, sealed/system-volume awareness, HFS+ resource-fork/xattr/Finder metadata preservation, Time Machine awareness, disk-image layering, and read-only-first safety policy.
- **WinDirStat donor disposition:** study/adapt/reference-oracle only by default; native Rust Prime implementation; no direct GPLv2 C++ incorporation into Prime Core without a separate intentional licence decision.
- **APFS donor disposition:** Apple specifications plus multiple independent read-only implementations as references; APFS write support is not ordinary Prime functionality until a dedicated corruption/recovery proof earns it.
- Update, rollback, generation-retention, workload-quiescence, and recovery architecture.
- Hardware graph and driver architecture.
- Build/image architecture.
- Init/service model.
- Component/package model, including the permanent boundary between image-owned Prime base components and independently installable optional components.
- Prime Store boundary: Store is a user-facing discovery/catalog/install/update surface over the component/package mechanism, not a replacement for Prime system-generation updates.
- Release Target contract and Provider model.
- Prime Shell and Prime Orb specifications.
- Reference-video design study.
- Performance gates, proof matrix, research-disposition matrix, and implementation handoff.
- Donor matrix covering Linux/kernel, Atomic/update systems, Wine/ReactOS, Android/AOSP/Waydroid, FEX/Box64, QEMU/KVM, VLC/libVLC, WinDirStat, Apple APFS/HFS references, containers, Origins/Hunter/Ptah, and relevant TTG systems.

P0 may run experiments to choose architecture. Those experiments are not Prime product implementation.

**Exit:** complete authority → independent review → correction → missing-scope review → contradiction review → implementation-readiness review → owner acceptance → freeze.

---

## P1 — First Light

**Goal:** produce the first unmistakably Prime bootable system.

Required:

- UEFI boot.
- Linux kernel foundation.
- Prime product identity.
- Prime generation identity.
- Prime Host identity.
- Prime Core.
- Hardware graph.
- Storage separation.
- Minimal Prime Storage Intelligence foundation:
  - block-device/filesystem/mount inventory;
  - total/free/available/reserved accounting;
  - Prime generation/rollback/recovery storage accounting;
  - update-space preflight;
  - storage-pressure event/reporting foundation;
  - basic Prime Storage UI;
  - safe recognition of APFS, HFS+, and HFS media;
  - honest encrypted/locked/read-only/unsupported state reporting for Apple media;
  - no destructive automatic mounting of foreign Apple filesystems.
- USB, Ethernet, input, audio baseline.
- Intel graphics on the HP proof Host.
- Prime Exec foundation.
- Application Profile registry.
- Capability Interface v1.
- Workload Policy v1.
- Prime Shell.
- Prime Orb baseline.
- Network/audio/power/settings surfaces.
- Shutdown/restart.
- Recovery entry.
- Update-aware generation layout.

### P1 visual gate

First Light is not complete merely because a GUI starts. It must provide Prime startup identity, Prime Shell, system rail, Prime Orb/launcher, functional windowing, quick controls, smooth core transitions, Prime glass/depth language, no obvious stock-distro identity, reference-video comparison, and owner visual acceptance.

**Not required:** Windows Personality, Android Personality, Ptah, Prime Store/component-delivery implementation, full updater proof, full WinDirStat-equivalent storage analyzer, APFS write support, Darwin local compatibility, or iOS local compatibility.

P1 keeps the Prime base image-owned. Optional SDKs, runtimes, applications and Providers remain separately activatable future capabilities; P1 does not add a live base-mutation package path merely to prepare for the Store.

---

## P1.5 — Survival

**Goal:** prove Prime can evolve safely without reinstalling or destroying work.

Prove:

`A installed → B built → B downloaded → B verified → B staged → B booted → B health proven → B retained`

Also prove:

- corrupt update handling;
- network interruption;
- power loss during download;
- power loss during staging;
- candidate boot failure;
- candidate health failure;
- late regression;
- driver/display regression;
- manual rollback;
- automatic rollback;
- user/project-state preservation;
- profile-schema compatibility after rollback;
- Capability Interface incompatibility after rollback;
- active-workload enumeration, classification, and quiescence;
- non-resumable work becomes `INTERRUPTED`, never fake success;
- recovery remains available if Prime Shell fails;
- update staging refuses safely when it would consume protected rollback/recovery reserve;
- low-space update/recovery behavior;
- storage accounting remains consistent across generation changes;
- attached Apple inspection/recovery media remain unchanged/read-only through update and rollback tests unless a later separately proven write path is deliberately under test.

P1.5 proves **Prime system-generation update and rollback**, not the later optional-component lifecycle. Component install/update/remove/rollback is introduced separately in P2.

---

## P2 — Development Body

**Goal:** make Prime capable of building real software and establish the optional-component delivery foundation.

Add capability-managed tooling for:

- Git and terminal workflows;
- Rust;
- Python;
- C/C++;
- LLVM/Clang and GCC;
- Node/TypeScript;
- JVM tooling;
- .NET tooling;
- Android build tooling;
- containers;
- isolated build environments;
- cross-compilers;
- developer SDK facility;
- signing facility;
- packaging framework;
- release-provider framework.

### Prime component/package foundation

Implement the Prime-owned mechanism used to add optional capabilities without rebuilding or mutating the running image-owned Prime base:

- versioned Prime Component Manifest/package contract;
- component identities, versions, architecture/runtime requirements and dependency declarations;
- component classes for applications, Providers, runtimes, toolchains/SDKs and optional Prime capability packs;
- publisher identity, cryptographic signature and immutable package-digest verification;
- local/offline package installation as a first-class path;
- install, update, remove and rollback transactions with truthful failure/recovery states;
- dependency resolution and compatibility checks against Prime generation, Application Profile schemas and Capability Interface versions;
- explicit persistent-data/configuration ownership so removing or updating a component does not silently destroy retained user/project state;
- registration/unregistration of Application Profiles, services and exposed Prime capabilities through their existing Prime authorities;
- Workload Policy and permission binding for installed workloads/services;
- catalog/source abstraction so later Store, private/internal repositories and local media can use the same component engine;
- evidence/audit records for package verification and lifecycle changes.

**Immutable-base rule:** ordinary component installation must not mutate image-owned Prime `/usr` or bypass the Prime generation model. If a requested feature requires kernel, boot, Prime Core, image-owned driver or other base-system changes, the component layer hands that requirement to the Prime generation/update mechanism instead of patching the live base.

Build the main native Rust Prime Storage Intelligence body:

- generic Linux/VFS scanner;
- Storage Index;
- incremental Change Engine;
- hardlink/sparse/extent accounting;
- duplicate/hash candidate engine;
- build/cache ownership accounting;
- cleanup planning;
- resource-aware scan scheduling;
- ext4/Btrfs/XFS enrichment required for truthful local-storage reporting;
- read-only Apple storage workflows: APFS container/volume discovery, safe read-only provider/extraction path, HFS+ read-only integration, Apple disk-image inspection, Apple metadata preservation, and authorized encryption/unlock workflow where technically supported.

**Permanent rule:** build capability does not imply local execution capability.

Prime may build APKs before Android Personality and Windows artifacts before Windows Personality.

---

## P3 — Origins Factory

**Goal:** turn Prime into the main Hunter/THETECHGUY development workstation and make ecosystem capabilities easy to add and maintain.

Integrate:

- Origins;
- `originsd`;
- Hunter;
- AgentOps;
- CodeOps;
- Sergeant;
- Ptah where ready, without moving Ptah semantics into Prime;
- Oracle where ready;
- Builder where ready;
- Lumi DM where ready;
- Lumi Browser later when ready;
- Grid-Knight where ready;
- repositories;
- process/terminal work;
- Prime Capability Interface;
- Prime Application Profiles;
- Prime Host → Origins Node projection;
- Prime Storage Intelligence projections for repository/project/mission/build/cache usage without transferring ownership of the Host index to Origins;
- Apple-storage projections for developer/repair/recovery missions, while Prime retains Host-local storage truth and access policy.

### Prime Store

Add the user-facing **Prime Store** over the P2 component/package mechanism:

- discover and inspect available applications, Providers, runtimes, toolchains and optional capability packs;
- install, update, remove and roll back independently delivered components;
- show dependency, permission, resource, compatibility, publisher/signature and channel information before mutation;
- expose available updates without conflating component updates with Prime OS generation updates;
- support approved first-party, private/internal and later external catalog sources through the same component contract;
- keep local/offline installation available when the Store UI or network is unavailable;
- use the component engine for ecosystem delivery so systems such as Ptah, Origins, Hunter, Oracle, Builder, Lumi DM, CodeOps, Sergeant and Grid-Knight can evolve independently of the Prime base where their architecture permits it.

The Store is a **client of the Prime component/package authority**, not the authority itself. CLI/API/offline installation must remain possible without the Store UI.

After P3, later Prime engineering should normally happen through Origins.

---

## System generation updates vs component updates

These lifecycles remain permanently distinct even if Prime Shell later presents them in one Updates surface.

### Prime system-generation update

Used for image-owned Prime OS changes such as kernel, boot/recovery material, Prime Core and other base `/usr` content.

`DISCOVERED → DOWNLOADED → VERIFIED → STAGED → BOOT_TRY → HEALTH_PROVING → KNOWN_GOOD`

System updates use the Prime generation/update architecture and retain previous-known-good/recovery generations.

### Prime component update

Used for independently installed optional applications, Providers, runtimes, toolchains/SDKs and capability packs whose lifecycle does not require mutation of the image-owned base.

Component updates use the Prime component/package mechanism and its own verify/install/update/remove/rollback transaction evidence.

A Store request that requires a base-system change must hand off to the system-generation updater. The Store cannot weaken, bypass or silently mutate the Prime generation boundary.

---

## P4A — Windows Personality

Implement progressively:

- W0 — PE recognition / profiles.
- W1 — portable/simple Win32.
- W2 — installers/common runtimes.
- W3 — .NET applications.
- W4 — DirectX/GPU acceleration.
- W5 — COM.
- W6 — supported Windows services.
- W7 — USB/device integration.
- W8 — VM fallback.
- W9 — real TTG workload certification.

Initial runtime target: x86_64 Prime Host → x86/x86_64 Windows workloads.

Real THETECHGUY Windows applications are compatibility fixtures.

Useful Windows support may ship before complete Windows compatibility.

NTFS-mounted storage uses the generic Prime scanner initially. A native Rust NTFS/MFT accelerator is a later research lane only under conditions proven safe; WinDirStat remains an NTFS behavioral reference oracle.

---

## P4B — Android Personality

Implement Android support progressively using AOSP/ART/Waydroid/Binder/Mesa/Linux isolation concepts where appropriate.

Desired experience:

`APK → install → Prime application entry → launch`

Real TTG APKs become compatibility fixtures.

P4A and P4B may proceed in parallel when efficient and non-conflicting.

---

## P5 — Cross-architecture execution

Add translation only where actual workloads justify it.

Potential needs:

- x86 on ARM;
- ARM on x86 where useful;
- translated Windows workloads;
- foreign-architecture containers;
- native-library forwarding.

Prime contracts carry both `host_arch` and `workload_arch` from day one.

---

## P6 — Darwin local compatibility

Research/local-execution phase.

Allowed disposition:

`ADOPT / ADAPT / DEFER / REMOTE-OFFICIAL / NOT-VIABLE / REJECT`

Local Darwin compatibility does not block macOS development capability through suitable Providers, and is separate from Prime's earlier read-only Apple-filesystem support.

---

## P7 — iOS local compatibility

Initial focus:

- IPA understanding;
- architecture inspection;
- entitlements;
- signing metadata;
- framework analysis;
- development integration.

Local arbitrary IPA execution remains evidence-dependent and is not required for iOS development workflows through suitable Providers.

---

## P8 — Distributed Prime deployment

Multiple independent Prime Hosts may participate in the wider system.

Each Prime Host owns only its local identity, hardware graph, generations, health, capabilities, and Host-local storage truth. Prime does not own a global Host registry.

Origins may aggregate Prime Hosts as Origins Nodes. Ptah may later enroll suitable Prime Hosts as Ptah Nodes/Providers.

Non-Prime Windows/Apple/cloud/specialist machines remain Providers rather than fake Prime Hosts.

---

# Cross-phase gates

## Resource gate

Measure idle RAM, idle CPU, background process count, boot time, Prime Shell ready time, runtime activation latency, runtime shutdown latency, storage pressure, scanner CPU/I/O impact, foreign-filesystem helper cost, and power draw where measurable.

## Storage-truth gate

Prime must distinguish logical, allocated, shared/exclusive (when provable), reserved, and unknown/metadata storage instead of presenting guessed physical ownership as exact. Cleanup must respect `PROTECTED / RECLAIMABLE / REVIEW / UNKNOWN` classifications.

For APFS specifically, Prime must distinguish container capacity, per-volume logical usage, shared container space, snapshot-retained storage, clone/shared allocation when provable, locked/encrypted state, and unknown/metadata usage. It must not sum APFS volume capacities as though they were independent physical disks.

## Security gate

All runtime backends use Prime Workload Policy. No separate weak VM/container/personality path is allowed. Storage/file events may be consumed by Grid-Knight later, but Prime Storage Intelligence does not make malware judgments. Foreign Apple media remains read-only by default until a write backend has separately earned trust.

Store/component delivery does not create a weaker execution path: installed workloads and services remain subject to Prime authorization, Workload Policy, signature/integrity checks and the existing machine authority boundaries.

## Review gate

Prime owns mechanical evidence. Sergeant performs independent engineering review where required. Owner acceptance remains necessary for subjective/product decisions such as Prime Shell visual quality.

## Architecture drift rule

If implementation exposes a genuine contradiction:

`stop → preserve evidence → return to planning authority → amend deliberately → freeze correction → resume`

No silent redesign.

---

# Current implementation mission

The active implementation workstream is draft PR #1, `build/p1-first-light`:

> **Continue P1 — First Light from the frozen Prime authority and P1 contracts.**

The Store/component additions above are future P2/P3 scope and must not expand or destabilize the current P1 First Light implementation. Do not begin Windows, Android, Ptah integration, Store implementation, the full Storage Intelligence analyzer, APFS write support, or distributed execution before the required earlier phases are proven.
