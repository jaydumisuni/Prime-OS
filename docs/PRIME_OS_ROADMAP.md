# Prime OS — Implementation Roadmap

**Authority:** derived from `docs/PRIME_OS_MASTER_PLAN.md`  
**Planning baseline:** accepted for handoff  
**Implementation:** must occur in a fresh workstream after recovering repository authority

This file is the fast operational roadmap. The Master Plan remains canonical when this summary is ambiguous.

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
- Update, rollback, generation-retention, workload-quiescence, and recovery architecture.
- Hardware graph and driver architecture.
- Build/image architecture.
- Init/service model.
- Component/package model.
- Release Target contract and Provider model.
- Prime Shell and Prime Orb specifications.
- Reference-video design study.
- Performance gates, proof matrix, research-disposition matrix, and implementation handoff.
- Donor matrix covering Linux/kernel, Atomic/update systems, Wine/ReactOS, Android/AOSP/Waydroid, FEX/Box64, QEMU/KVM, VLC/libVLC, containers, Origins/Hunter/Ptah, and relevant TTG systems.

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

**Not required:** Windows Personality, Android Personality, Ptah, full updater proof, Darwin local compatibility, or iOS local compatibility.

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
- recovery remains available if Prime Shell fails.

---

## P2 — Development Body

**Goal:** make Prime capable of building real software.

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

**Permanent rule:** build capability does not imply local execution capability.

Prime may build APKs before Android Personality and Windows artifacts before Windows Personality.

---

## P3 — Origins Factory

**Goal:** turn Prime into the main Hunter/THETECHGUY development workstation.

Integrate:

- Origins;
- `originsd`;
- Hunter;
- AgentOps;
- CodeOps;
- Sergeant;
- Oracle where ready;
- Lumi where ready;
- repositories;
- process/terminal work;
- Prime Capability Interface;
- Prime Application Profiles;
- Prime Host → Origins Node projection.

After P3, later Prime engineering should normally happen through Origins.

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

Local Darwin compatibility does not block macOS development capability through suitable Providers.

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

Each Prime Host owns only its local identity, hardware graph, generations, health, and capabilities. Prime does not own a global Host registry.

Origins may aggregate Prime Hosts as Origins Nodes. Ptah may later enroll suitable Prime Hosts as Ptah Nodes/Providers.

Non-Prime Windows/Apple/cloud/specialist machines remain Providers rather than fake Prime Hosts.

---

# Cross-phase gates

## Resource gate

Measure idle RAM, idle CPU, background process count, boot time, Prime Shell ready time, runtime activation latency, runtime shutdown latency, storage pressure, and power draw where measurable.

## Security gate

All runtime backends use Prime Workload Policy. No separate weak VM/container/personality path is allowed.

## Review gate

Prime owns mechanical evidence. Sergeant performs independent engineering review where required. Owner acceptance remains necessary for subjective/product decisions such as Prime Shell visual quality.

## Architecture drift rule

If implementation exposes a genuine contradiction:

`stop → preserve evidence → return to planning authority → amend deliberately → freeze correction → resume`

No silent redesign.

---

# Next implementation mission

A fresh implementation workstream begins with:

> **Build P1 — First Light from the frozen Prime authority.**

Do not begin with Windows, Android, Ptah, or distributed execution before the required earlier phases are proven.
