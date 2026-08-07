# Prime OS — Master Plan, Architecture, and Implementation Roadmap

**Planning status:** OWNER-APPROVED BASELINE / READY FOR HANDOFF  
**Implementation status:** NOT STARTED IN THIS WORKSTREAM  
**Implementation rule:** implementation must begin in a fresh workstream that first recovers this repository authority.  
**Repository:** `jaydumisuni/Prime-OS`

This document is the canonical Prime OS planning authority produced before implementation. It exists so a future chat, AI, engineer, or team can recover the intended product and continue without reconstructing design decisions from conversation history.

Prime implementation must not silently redesign this architecture. If implementation discovers a genuine contradiction, preserve the evidence, return to planning authority, amend deliberately, then continue.

---

## 1. Product definition

Prime OS is a **lightweight, integration-first development and execution operating system**.

Its purpose is to let a developer remain in one operating environment while they:

`develop → build → run → test → debug → sign → package → verify → release`

software for multiple target platforms.

Prime is designed from its foundation to understand hardware, application/package formats, runtime families, CPU architectures, native and foreign-platform applications, containers, virtual machines, remote execution resources, specialized development environments, build systems, signing systems, packaging systems, and release Providers.

The intended user experience is `Open / Build / Run / Test / Release`, not manually launching Wine, Waydroid, QEMU, another operating system, or unrelated remote-desktop workflows.

Those mechanisms may exist underneath Prime; Prime integrates them into one operating-system experience.

## 2. Prime is not

Prime is not Fedora, Ubuntu, a Linux distribution with a custom theme, Windows, macOS, Android, Wine, a Wine frontend, Waydroid, a VM manager, Origins Factory, Hunter, Ptah, CodeOps, or Sergeant.

Prime may borrow proven engineering from any of these ecosystems where appropriate.

Prime uses the Linux kernel because it already provides mature CPU support, memory management, filesystems, networking, PCI/USB, graphics, device drivers, power management, virtualization, namespaces/cgroups, and security primitives.

Prime owns the operating-system integration, runtime architecture, shell, update/recovery model, machine capability layer, workload policy, and product experience built above that foundation.

## 3. Prime-built applications remain ordinary target-platform applications

Prime is primarily a development/build/test/execution environment. Software produced on Prime does not require Prime unless deliberately built as a Prime-native product.

Examples:

- Prime → Windows build → EXE/MSI/installer → normal Windows machine.
- Prime → Android build → APK → normal Android device.
- Prime → Apple build → signed Apple artifact → normal Apple device.

Prime is the developer's system, not a runtime dependency imposed on customers.

## 4. Core operating principle

> **Support broadly. Activate narrowly.**

Prime may support many runtime families while keeping only essential machinery alive when idle.

An idle Prime installation should keep the Linux kernel, Prime Core, display, storage, network, audio, security, hardware monitoring, and essential services. Windows/Android/Darwin runtimes, VMs, large SDKs, build workers, containers, AI models, and compatibility services activate only when required.

Low resource usage is a product requirement.

---

# Authority boundaries

## 5. Prime OS — machine authority

Prime owns boot, kernel integration, hardware discovery, drivers, graphics, audio, networking, storage, power, thermal behavior, Prime Core, Prime Exec, runtime personalities, process execution, virtualization machinery, container machinery, machine-level isolation, Prime Workload Policy, Prime Application Profiles, Prime Capability Interface, updates, rollback, recovery, Prime Shell, and machine security.

Prime answers:

> **What can this machine execute, and how can it execute it safely?**

## 6. Origins Factory — mission/workspace authority

Origins Factory is the portable mission/work environment.

Permanent distinction:

- **Prime Exec = executability.** It determines what a workload is, whether this Prime Host can execute it, which backend can execute it, and what machine policy applies.
- **Origins Capability Compiler = mission composition.** It determines what capabilities a mission needs, in what order, on which machine/provider, and what evidence/review path applies.

> **Prime determines how something can execute. Origins determines how already-available capabilities are composed into work.**

## 7. Hunter — intelligence

Hunter owns conversation, reasoning, context, planning, capability selection, model/provider routing, reconciliation, gap detection, and upgrade proposals.

Prime must remain a useful operating system without Hunter. Hunter becomes more capable when Prime supplies a consistent physical execution body.

## 8. Ptah — neutral mechanical workspace/execution substrate

Ptah owns higher-level mechanical concepts including Workspace, Activity, Attempt, Environment, Facility, Provider, Node, Object, Revision, View, Artifact, Grant, Lease, Fence, Receipt, Evidence, scheduling, and recovery.

Prime does not recreate those semantics.

Prime may expose machine capabilities such as Windows VM execution, CPU architecture, USB passthrough, or available memory. Ptah may later request an Environment using those capabilities for an Activity.

Prime owns machine capability. Ptah owns Ptah-managed mechanical use of that capability.

Prime does **not** implement Ptah. Ptah is tested on Prime when the Ptah workstream reaches its own authorized runtime/testing stage.

## 9. CodeOps and Sergeant

**CodeOps performs engineering.**

**Sergeant performs independent software-engineering review.** Its canonical engineering outcomes remain `PASS / NEEDS WORK / BLOCK`.

Sergeant is not a Prime runtime certification daemon. Prime owns mechanical compatibility observations. Sergeant reviews Prime engineering/release changes when engineering review is required.

## 10. Long-term stack

```text
USER
 │
 ▼
PRIME SHELL
 │
 ▼
ORIGINS FACTORY
 │
 ▼
HUNTER
 │
 ▼
AGENTOPS
 │
 ▼
CAPABILITY COMPILER
 │
 ├── Prime machine capabilities
 ├── Ptah mechanical environments
 ├── CodeOps
 ├── Oracle
 ├── Lumi
 ├── specialist Gateways
 └── other Providers
 │
 ▼
SERGEANT / XRAY / deterministic proof
```

Prime remains the operating system; Origins the mission workspace; Hunter the intelligence; Ptah the neutral mechanical execution substrate; CodeOps the engineering system; Sergeant the review system.

---

# Prime Core and implementation responsibilities

## 11. Prime Core

Prime Core is the smallest continuously available user-space system plane. Primary implementation language: **Rust**.

Prime Core owns Prime Host identity, Prime generation identity, hardware graph, Prime Exec, service lifecycle, Workload Policy, Application Profile registry, Capability Interface, resource accounting, system events, update controller, rollback controller, recovery, power/thermal coordination, and secure IPC.

Optional capabilities surround Prime Core rather than becoming permanent resident services.

## 12. Language responsibilities

### Rust

Preferred for Prime Core, Prime Exec, hardware inventory, driver orchestration, process supervision, resource enforcement, system APIs, update/recovery, runtime lifecycle, capability registry, and security-sensitive persistent services.

### Python

Preferred for Hunter, automation, AI/model work, device tooling, research, rapid capability development, build helpers, data processing, and CodeOps integrations. Python workers activate only when needed.

### React + TypeScript

Preferred for Prime's rich visual shell and system applications where suitable, likely through a lightweight native host backed by Rust system services. Exact compositor/window implementation is a P0 architecture decision. Avoid unnecessary Electron-style duplicated Chromium runtimes when a lighter path works.

---

# Prime Host

## 13. Prime Host definition

Prime does not use `Node` as its machine identity because Origins and Ptah already have meaningful Node concepts.

A **Prime Host** is one physical or virtual machine currently running Prime, known locally by Prime for hardware discovery, system generations, machine health, and local capability exposure.

## 14. Prime Host authority is self-only

A Prime Host authoritatively knows only about itself.

Prime Core locally owns the current Prime Host ID, hardware identity/fingerprint, Prime generations, local capability graph, local health, and local Host lineage.

Prime Core does **not** own a global registry of every Prime machine. Aggregation belongs to higher-level systems such as Origins and later Ptah.

## 15. Prime Host projections

Mappings are explicit:

`Prime Host → Prime Capability Interface → Origins adapter → Origins Node projection`

Later:

`Prime Host → Prime Capability Interface → Ptah adapter → Ptah Node / Providers / Facilities`

Identities remain distinct:

`Prime Host ID ≠ Origins Node ID ≠ Ptah Node ID`

## 16. Prime Host hardware migration

Storage identity is not Host identity.

Moving a Prime system disk from one physical machine to another triggers fresh hardware discovery and Host migration/re-enrollment. By default, a materially different machine becomes a new Prime Host identity. System/user/workspace data may migrate; machine identity does not silently migrate with the disk.

## 17. Hardware-change classifications

- **Ordinary hardware change:** RAM upgrade, GPU replacement, added SSD. Same Prime Host; hardware graph revision changes and capabilities are recalculated.
- **Material machine migration:** Prime storage moved to a different physical machine. Default new Prime Host identity.
- **Owner-approved rebind:** exceptional replacement such as motherboard change; explicit rebind with old identity, new hardware identity, reason, evidence, time, and supersession history retained.

---

# Prime Exec and workload control

## 18. Prime Exec

Prime Exec is a first-class OS subsystem. It identifies both package/binary format and runtime family.

Examples:

- ELF → native/Linux.
- PE32 / PE32+ → Windows.
- JAR / `.class` → JVM.
- APK / DEX → Android / ART.
- WASM → WebAssembly.
- Mach-O / `.app` → Darwin.
- IPA → iOS package/runtime family.

Prime Exec determines format, runtime family, host architecture, workload architecture, dependencies, required APIs, required hardware, security requirements, and available execution backend.

Recognition never means automatic support.

## 19. Prime execution backends

Prime may satisfy a workload through:

- `NATIVE`
- `PERSONALITY`
- `CONTAINER`
- `VM`
- `REMOTE_PROVIDER`
- `SPECIALIZED/OFFICIAL_PROVIDER`

Ordinary interaction should hide unnecessary backend-management complexity from the developer.

## 20. VM ownership

Prime owns KVM/QEMU integration, VM disks, virtual devices, VM networking, USB passthrough, boot/stop/suspend, and machine resource enforcement.

A VM is not inherently a Ptah Environment.

Prime-local application profiles may launch VMs directly. Ptah-managed VM environments use the same Prime VM capability while Ptah owns Activity/Attempt/Environment lifecycle.

There is no weaker Prime-local VM security path.

## 21. Prime Workload Policy v1

**P0 hard contract.**

Every workload passes through Prime Workload Policy regardless of backend.

Required controls include CPU quota/weight, memory limit, GPU policy, storage quota, process limits, I/O priority, runtime duration, USB/device access, filesystem exposure, network policy, secret access, background behavior, and logging/evidence requirements.

Ptah may impose stricter constraints later. Ptah cannot weaken Prime policy.

## 22. Network policy

Foreign workloads do not automatically receive unrestricted networking.

Prime supports policies including `OFFLINE`, `LAN_ONLY`, `OUTBOUND_INTERNET`, `DESTINATION_RESTRICTED`, `LOCAL_LISTENER`, `INBOUND_ALLOWED`, and `UNRESTRICTED`.

---

# Prime Application Profiles

## 23. Prime Application Profile v1

**P0 hard contract.** Minimal runtime registry lands in P1.

A profile records application identity, profile schema version, profile revision, binary/package format, runtime family, workload architecture, execution backend, dependencies, resource policy, network policy, device policy, mechanical compatibility state, and evidence references.

## 24. Profile revision pinning

A workload remains pinned to the exact profile revision used at launch. A newer profile revision applies only to new launches unless a critical security revocation explicitly suspends or terminates affected workloads according to revocation rules.

Prime never silently mutates policy beneath a running workload.

## 25. Profile schema versioning

Prime generation, Application Profile schema version, Application Profile revision, and Capability Interface version are independent version axes.

Profile schema migrations must be explicit, versioned, non-destructive, rollback-aware, and tested against retained generations. Rolling Prime back must not destroy newer profile data.

## 26. Mechanical compatibility truth

Prime owns mechanical application compatibility observations.

States may include:

`UNKNOWN / RECOGNIZED / INSTALLABLE / LAUNCHES / PARTIALLY_FUNCTIONAL / FUNCTIONAL / BROKEN / UNSUPPORTED / REQUIRES_VM / REQUIRES_REMOTE_PROVIDER`

`FUNCTIONAL` means Prime's defined compatibility checks passed and evidence exists. It does not require Sergeant approval per application.

Engineering facts remain separate. A runtime change may have `Sergeant: PASS` while an Application Profile independently records `FUNCTIONAL` mechanical compatibility.

---

# Prime Capability Interface

## 27. Prime Capability Interface v1

**P0 hard contract.**

Prime exposes Host capabilities through a stable versioned interface. It must express Host identity/architecture, capability identity/version, availability, supported workload formats/runtime families, resources, hardware features, limits, and health.

Capability families may include hardware, execution, runtime, driver, build, test, sign, package, release, network, virtualization, and container capability.

## 28. Interface version lifecycle

Use major/minor semantics:

- minor = additive/backward compatible.
- major = breaking semantic change.

Consumers negotiate the highest mutually supported contract.

Lifecycle:

`INTRODUCE → COEXIST → DEPRECATE → prove supported consumers migrated → RETIRE`

Prime must not silently remove a still-supported contract.

## 29. Negotiation failure

If no compatible contract exists, higher-level workspaces remain recoverable, the Prime provider reports `INCOMPATIBLE`, Prime-backed actions fail closed, required/supported versions are shown, and no fake compatibility is invented.

Normal rollback performs compatibility preflight and may block or require explicit owner/recovery override if it would remove the last compatible interface.

Emergency automatic rollback prioritizes machine recovery; higher-level adapter functionality may temporarily degrade while state remains preserved.

P1.5 must prove this case.

---

# Storage and hardware

## 30. Storage model

**P0 hard decision.**

Prime separates system generations, user/project data, Prime configuration, Application Profiles, Origins state, VM state, container state, build caches, scratch, recovery data, and logs/evidence.

System replacement cannot destroy project/user state.

## 31. Hardware graph

Prime first boot discovers CPU, architecture, RAM, PCI, USB, ACPI, SMBIOS, GPU, audio, storage, Ethernet, Wi-Fi, Bluetooth, virtualization, firmware, display topology, input devices, thermal sensors, battery, Secure Boot, and TPM where present.

## 32. First proof Host

Initial physical target: HP 290 G4 Microtower with Intel Core i7-10700, 8 cores / 16 threads, 8 GB DDR4 initially, Intel UHD Graphics 630, 1 TB Samsung NVMe, 500 GB Crucial SATA SSD, Realtek Gigabit Ethernet, and VT-x.

Prime must not become HP-specific.

## 33. Driver architecture

Prime treats driver support as first-class capability. Important families include Intel, AMD, NVIDIA, Realtek, Broadcom, Qualcomm, MediaTek, storage, audio, Wi-Fi, Bluetooth, USB, Thunderbolt, and specialist TTG hardware.

Only matching hardware support activates.

## 34. Driver trust tiers

Prime supports development hardware without globally disabling security:

- `T0` upstream/kernel trusted.
- `T1` verified vendor.
- `T2` Prime-reviewed / Prime-signed.
- `T3` developer-trusted local.
- `T4` untrusted / quarantined.

T3 supports prototype hardware such as ISP Box development. Developer-trusted driver records bind exact digest, owner/developer identity, hardware scope, Prime generation, trust decision, and audit record.

Stable/public Prime may disable T3 unless Developer Mode is explicitly enabled.

Promotion path:

`developer local → test → hardware proof → review → Prime-sign → candidate → stable`

## 35. Display/input proof classes

First Light tests single display, multi-display, display hotplug, resolution changes, DPI scaling, HDMI/DP audio, keyboard, mouse, USB hotplug, removable storage, suspend/resume, GPU reprobe, and thermal behavior.

Prime also designs for touch, trackpad gestures, eGPU, and Thunderbolt even if the first HP cannot prove all classes.

---

# Prime Shell and UX

## 36. Prime Shell

Prime must visibly be Prime from First Light. It must not boot into an obvious stock GNOME/KDE/Fedora/Ubuntu environment and call that complete.

Prime Shell owns startup experience, login, desktop, windowing, system rail, launcher, Prime Orb, notifications, quick controls, network, audio, power, hardware health, updates, application launch, and workspace surfaces.

Existing technologies may be implementation donors.

## 37. Visual reference

The supplied reference video is an explicit design donor and quality bar.

P0 must document motion language, glass/depth, workspace transitions, system/status rail, radial/circular controls, widget behavior, wallpaper/system integration, spacing, visual hierarchy, animations, and media interaction.

Prime borrows principles, not the exact design.

P1 requires owner visual acceptance.

## 38. Prime Orb

Prime's central interaction surface. A likely activation is `Super → Prime Orb`.

Possible content includes applications, search, system state, hardware health, running workloads, updates, Origins, Hunter when installed, Prime capabilities, and quick controls.

Prime Orb remains useful without Hunter.

---

# Developer platform and release model

## 39. Prime Development Platform

Prime eventually supports capability-managed toolchains for Rust, Python, C/C++, LLVM/Clang, GCC, Node/TypeScript, Java/Kotlin/JVM, .NET, Android SDK/NDK, Gradle, Git, containers, cross-compilers, signing, packaging, and release tooling.

Heavy SDKs remain optional capabilities.

## 40. Build ≠ execute ≠ sign ≠ publish

Permanent rule:

`can build ≠ can execute locally ≠ can sign ≠ can publish`

Prime may build an APK before Android Personality exists, or build a Windows executable before Windows Personality exists.

## 41. Specialized/official Providers

Vendor/platform-specific environments are optional Providers. Use them only where tooling, platform policy, signing, developer choice, or selected release channel requires them.

The developer should remain in Prime/Origins even when actual work happens locally, in a VM, on another machine, in cloud infrastructure, or in an official/vendor environment.

## 42. Release Providers

Prime does not equate release with an app store.

Possible Providers include THETECHGUY website, GitHub Releases, direct download, private customer deployment, enterprise/internal deployment, LAN/local deployment, Google Play, Microsoft Store, TestFlight/App Store, and others.

Stores are optional Providers, not Prime policy.

## 43. Release Target contract

**P0 hard contract.**

Contains at least target platform, artifact type, package format, signing policy, verification policy, release Provider, release channel, update feed, and rollback/revocation policy.

---

# Runtime personalities

## 44. Windows Personality

Prime treats Windows PE applications as first-class workloads.

Potential donors include Wine, ReactOS, DXVK, VKD3D, Mesa, Mono/.NET, and LLVM. Prime should reuse proven engineering rather than rewriting decades of compatibility work unnecessarily.

Stages:

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

Initial target: x86_64 Prime Host → x86/x86_64 Windows workload. Cross-architecture Windows execution comes later.

## 45. Android Personality

Prime recognizes APK, DEX, and Android native libraries as first-class workloads.

Potential donors include AOSP, ART, Waydroid, Binder, Mesa, and namespaces/cgroups.

Target UX: `APK → install → Prime application entry → launch`, not manually launching a separate Android desktop.

Android support is staged rather than all-or-nothing.

## 46. JVM runtime

JVM is its own runtime family: `JAR / .class → JVM`. JVM development/run capability may arrive in P2 independently of Android Personality.

## 47. Darwin/macOS

Prime recognizes Mach-O and `.app` bundles early. Local compatibility is later research. Potential donors include Darling, GNUstep, Darwin/Mach projects, and LLVM.

macOS development capability and local macOS application execution are separate requirements.

## 48. iOS / IPA

Prime understands IPA packages. Initial support includes recognition, architecture inspection, entitlements, signing metadata, framework analysis, and build/test preparation.

Prime does not promise arbitrary local IPA execution. Development can still occur through suitable Providers.

## 49. Research disposition

Every research-heavy capability concludes with one of:

`ADOPT / ADAPT / DEFER / REMOTE-OFFICIAL / NOT-VIABLE / REJECT`

No permanently ambiguous roadmap item.

## 50. Cross-architecture execution

OS ABI compatibility and CPU compatibility remain separate.

Example: x86_64 Windows workload on x86_64 Prime → Windows Personality. The same workload on ARM64 Prime → instruction translation → Windows Personality.

Potential donors include FEX, Box64, and QEMU. Contracts include `host_arch` and `workload_arch` from day one.

## 51. Containers

Potential donors include containerd, crun, youki, BuildKit, and Dagger.

Use cases include builds, tests, CodeOps, developer environments, temporary workers, untrusted workloads, and later Ptah environments.

---

# Prime system release and recovery

## 52. Prime release channels

Prime system releases use `LAB / CANDIDATE / STABLE`.

Flow:

`new capability → LAB → build → test → challenge → candidate → acceptance → stable`

Stable systems are not where architecture is casually discovered.

## 53. Update architecture

Prime is update-aware from First Light.

`generation A → download B → verify → stage → boot B → health proof → retain B or return A`

Potential donors include OSTree, bootc, RAUC, SWUpdate, A/B systems, and other image-based designs. P0 chooses the actual mechanism.

## 54. Generation model

Prime retains current generation, previous known-good generation, and recovery generation.

A successful first boot does not immediately erase rollback capability. A retention policy preserves previous-known-good state for delayed regressions.

## 55. Update failure matrix

P1.5 must prove corrupt update handling, network interruption, power loss during download, power loss during staging, candidate boot failure, candidate health failure, late regression, driver/display regression, manual rollback, automatic rollback, user/project state preservation, profile-schema compatibility after rollback, and Capability Interface incompatibility after rollback.

## 56. Active workload handling

Prime does not promise arbitrary live-process memory survival across system generations.

Before an intentional generation change, Prime enumerates active workloads, classifies them as `SAFE_TO_STOP / CHECKPOINTABLE / DURABLE_EXTERNAL_STATE / NON_RESUMABLE / CRITICAL`, and quiesces them.

Prime may flush output, save supported state, coordinate Origins/Ptah durable state, stop workloads cleanly, and block or request confirmation for critical work.

Unexpected termination becomes `INTERRUPTED`, not false completion.

Automatic boot-health rollback should occur before ordinary workloads start whenever possible.

## 57. Recovery

Prime recovery must work even if Prime Shell does not.

Required: known-good boot, recovery environment, filesystem inspection, driver rollback, network recovery, generation rollback, offline update/recovery media, and configuration recovery.

---

# Origins, Ptah, and distributed execution

## 58. Origins on Prime

Origins is integrated before major Windows/Android work.

Progression:

`Prime boots → Prime survives updates → Prime builds software → Origins runs on Prime → Hunter/CodeOps/Sergeant available → later Prime capabilities are built through Origins`

## 59. Prime Host → Origins Node projection

P3 implements:

`Prime Host → Prime Capability Interface → Origins Prime adapter → Origins Node projection`

Prime owns Host truth. Origins owns mission-level Node truth.

## 60. Future Prime Host → Ptah mapping

Later:

`Prime Host → Prime Capability Interface → Ptah adapter → Ptah Node / Providers / Facilities`

Prime remains unaware of Ptah Activities/Attempts unless Ptah calls it.

## 61. Distributed Prime deployment

Multiple independent Prime Hosts may exist. Each owns only its local identity, hardware graph, generations, health, and capabilities.

Prime itself does not maintain a global Host registry.

Origins may aggregate Prime Hosts as Origins Nodes. Ptah may later enroll appropriate Prime Hosts as Ptah Nodes/Providers.

Non-Prime Windows/Apple/cloud/specialist resources remain Providers rather than fake Prime Hosts.

## 62. Prime builds Prime

Long-term loop:

`Prime → Origins → Hunter/CodeOps → build next Prime LAB generation → VM/alternate generation/test Host → proof → Sergeant review → correction → candidate → owner acceptance → Prime upgrade`

Eventually Prime becomes its own main development Host.

## 63. Ptah testing

Prime does not implement Ptah.

When Ptah reaches an authorized runtime candidate:

`Ptah candidate → Prime Host → Prime Capability Interface → Ptah Activities/Environments → proof/evidence → Ptah workstream continues`

Prime should be ready before Ptah needs this stage.

---

# Donor lanes

## 64. VLC/media donor lane

VLC/libVLC should be studied for plugin architecture, modular capabilities, playback, streaming, recording, transcoding, hardware acceleration, filters, and cross-platform packaging.

Possible Prime media facilities include playback, screen recording, streaming, evidence preview, thumbnail generation, conversion, and remote worker display.

Code reuse must respect licensing boundaries; architecture can be borrowed without copying VLC wholesale.

---

# P0 planning phase

## 65. P0 — Complete the load

**No Prime product implementation.**

P0 resolves:

### Product boundaries
Prime, Origins, Hunter, AgentOps, Ptah, CodeOps, Sergeant, Oracle, Lumi, specialist systems.

### Execution
Native, Windows, Android, JVM, WASM, Darwin, iOS, containers, VMs, remote Providers, official/specialized Providers, cross-architecture.

### Hardware
Kernel, CPU, Intel/AMD/NVIDIA, storage, network, wireless, audio, input, display, USB, Thunderbolt/eGPU classes, power, thermal, virtualization, driver trust, Host portability.

### Development
Toolchains, SDKs, build systems, signing, packaging, release Providers.

### System
Build/image architecture, init/service architecture, component/package architecture, storage architecture, Prime Host identity, Prime Exec, Workload Policy, Application Profiles, Capability Interface, updates, rollback, recovery, security.

### UX
Reference-video study, Prime Shell, Prime Orb, compositor/rendering, interaction design, visual proof gates.

### Donors
Linux kernel ecosystem, Fedora Atomic/update ideas, Wine, ReactOS, Waydroid/AOSP, FEX/Box64, QEMU/KVM, VLC/libVLC, container systems, Origins, Hunter, Ptah donor research, and relevant TTG systems.

## 66. P0 hard exit

P0 cannot close until all of these are explicit and reviewed:

- Prime Master Plan.
- Prime Architecture.
- Implementation Roadmap.
- Donor Matrix.
- Prime Host Identity v1.
- Self-only Host authority rule.
- Hardware-change classification.
- Migration/re-enrollment semantics.
- Local Host lineage.
- Host rebind/supersession rules.
- Distributed-registry non-ownership rule.
- Prime Host capability/health model.
- Prime Host → Origins Node projection.
- Future Prime Host → Ptah mapping boundary.
- Prime Exec model.
- Prime Application Profile v1.
- Profile revision pinning.
- Profile schema migration rules.
- Profile revocation rules.
- Prime Capability Interface v1.
- Version negotiation.
- Zero-overlap failure behavior.
- Deprecation policy.
- Prime Workload Policy v1.
- Network policy.
- Resource policy.
- Filesystem/device policy.
- Secret policy.
- Driver trust tiers.
- Developer Mode driver policy.
- Storage/generation model.
- Update architecture.
- Rollback architecture.
- Generation-retention policy.
- Workload quiescence.
- Recovery architecture.
- Rollback/interface compatibility rules.
- Hardware graph.
- Driver architecture.
- Build/image architecture.
- Init/service model.
- Component/package model.
- Release Target contract.
- Provider model.
- Prime Shell specification.
- Prime Orb specification.
- Reference-video study.
- Performance gates.
- Proof matrix.
- Research disposition matrix.
- Implementation handoff.

P0 may use experiments to resolve design decisions. Those experiments do not become Prime product implementation.

## 67. P0 closure process

`recover requirements → study donors → complete architecture → independent review → correct → missing-scope review → contradiction review → implementation-readiness review → owner acceptance → FREEZE`

The review rounds represented in this plan closed the identified architecture overlaps and edge cases. The implementation workstream must still recover and honor the hard-exit contracts when turning the plan into concrete ADRs/specifications.

---

# Implementation roadmap

## 68. P1 — First Light

Goal: **Produce the first unmistakably Prime bootable system.**

Required:

- UEFI boot.
- Linux kernel foundation.
- Prime identity.
- Prime generation identity.
- Prime Host identity.
- Prime Core.
- Hardware graph.
- Storage separation.
- USB.
- Ethernet.
- Input.
- Audio baseline.
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

Not required yet: Windows Personality, Android Personality, Ptah, complete updater proof, Darwin local compatibility, or iOS local compatibility.

## 69. P1 visual gate

P1 is not complete merely because a GUI starts.

Required: Prime startup experience, Prime identity, Prime Shell, system rail, Prime Orb/launcher, functional windowing, quick controls, smooth transitions, Prime glass/depth language, no obvious borrowed-distro identity, reference-video comparison, and owner visual acceptance.

## 70. P1.5 — Survival

Goal: **Prove Prime can evolve safely without reinstalling or destroying work.**

Must prove A→B update, download/verification/staging/boot/health, and every update/recovery failure case including power interruption, corruption, late regression, driver regression, manual/automatic rollback, active workload quiescence, profile compatibility, Capability Interface rollback incompatibility, and user/project-state preservation.

## 71. P2 — Development Body

Goal: **Make Prime capable of building real software.**

Add Git, terminal, Rust, Python, C/C++, LLVM/Clang, GCC, Node/TypeScript, JVM tooling, .NET tooling, Android build tooling, containers, build environments, cross-compilers, developer SDK facility, signing facility, packaging framework, and release-provider framework.

**P2 build capability does not imply target runtime capability.**

## 72. P3 — Origins Factory

Goal: **Turn Prime into the main Hunter/THETECHGUY development workstation.**

Integrate Origins, originsd, Hunter, AgentOps, CodeOps, Sergeant, Oracle where ready, Lumi where ready, repositories, process/terminal work, Prime Capability Interface, Prime Application Profiles, and Prime Host → Origins Node projection.

After P3, later Prime engineering should normally happen through Origins.

## 73. P4A — Windows Personality

Implement W0–W9 progressively using real THETECHGUY Windows applications as compatibility fixtures. Useful Windows support may ship before complete Windows compatibility.

## 74. P4B — Android Personality

Implement Android support progressively using real TTG APKs as compatibility fixtures. P4A/P4B may proceed in parallel when efficient and non-conflicting.

## 75. P5 — Cross-architecture execution

Add translation where actual workloads justify it, including x86-on-ARM, ARM-on-x86 where useful, translated Windows workloads, foreign-architecture containers, and native-library forwarding.

## 76. P6 — Darwin local compatibility

Research/local-execution phase. Allowed disposition: `ADOPT / ADAPT / DEFER / REMOTE-OFFICIAL / NOT-VIABLE / REJECT`.

Local Darwin compatibility does not block macOS development capability.

## 77. P7 — iOS local compatibility

Initial focus: IPA understanding, architecture, entitlements, signing metadata, framework analysis, development integration. Local arbitrary IPA execution remains evidence-dependent.

## 78. P8 — Distributed Prime deployment

Multiple independent Prime Hosts may participate in the wider system. Prime itself remains self-local. Origins/Ptah provide aggregation where appropriate. Non-Prime Windows/Apple/cloud/specialist resources remain Providers rather than being falsely represented as Prime Hosts.

---

# Proof and handoff

## 79. Performance proof

Prime's lightweight claim must be measured. Track at minimum idle RAM, idle CPU, background process count, boot time, Prime Shell ready time, runtime activation latency, runtime shutdown latency, storage pressure, and power draw where measurable.

## 80. Proof principle

Every relevant phase requires positive proof, negative proof, failure proof, recovery proof, resource proof, security proof, hardware proof, compatibility proof, rollback proof where relevant, Sergeant engineering review where required, and owner acceptance where subjective/product judgment is required.

A build succeeding does not itself prove completion.

## 81. Implementation handoff

The planning workstream does not become the implementation workstream.

A fresh implementation workstream must recover this repository and begin with one bounded mission:

> **Build P1 First Light from the Prime authority.**

If implementation discovers a real architectural contradiction:

`stop → preserve evidence → return to planning authority → review amendment → freeze correction → resume`

No silent redesign.

## 82. End-state developer experience

Prime should eventually allow the developer to request a Windows, Android, Apple, Linux, web, firmware, or other target and have Prime select approved build/test/sign/package/release capabilities without forcing the developer to manually leave Prime/Origins.

## 83. Final private stack

Prime remains independently useful.

The private THETECHGUY environment may later combine:

`Prime OS + Origins Factory + Hunter + AgentOps + Ptah + CodeOps + Oracle + Lumi + Sergeant + X-Ray + TTG specialist Facilities`

Responsibilities remain distinct:

- Prime = body / operating system.
- Origins = mission workspace.
- Hunter = intelligence.
- AgentOps = semantic lifecycle.
- Ptah = neutral mechanical workspace/execution substrate.
- CodeOps = engineering.
- Sergeant = engineering review.
- X-Ray = specialist evidence.
- Oracle = authorized browser/OS control.
- Lumi = downloads/transfers.

No system absorbs another merely because integration would be convenient.

---

# Current handoff state

The product/architecture review has converged sufficiently to preserve this plan as the accepted baseline for future work.

**Do not start implementation by inventing technology choices that P0 marks as architecture decisions.** Recover the hard-exit items, create the concrete ADR/specification set from this plan, then execute the roadmap in order.

The next implementation mission is **P1 — First Light**, in a separate workstream.
