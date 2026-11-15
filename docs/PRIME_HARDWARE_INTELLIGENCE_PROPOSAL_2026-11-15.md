# Prime Hardware Intelligence (PHI) — shared Desktop / Mobile proposal

Date: 2026-11-15. Status: owner-selected NAME AND DIRECTION; design PROPOSAL, NOT a frozen Prime contract, working daemon, production release or evidence that any new device is supported.

## 0. Meaning and correction

PHI = PRIME HARDWARE INTELLIGENCE: generic hardware understanding, capability-aware resource adaptation, workload budgets, thermal/power and memory-tier visibility for Prime OS Desktop, Prime OS Mobile and later hardware. **Not Colibri rebranded.** Colibri is a donor for workload-specific memory placement and CPU/GPU selection. PHI must benefit ordinary users, games, browsers, developer toolchains, databases, mobile phone services and optional AI workloads.

PHI should be built as an additive orchestration/observation contract across existing Prime Core, Hardware Graph, Prime Exec, Capability Interface and Workload Policy, not a competing OS scheduler, independent privileged memory manager or mandatory AI model. Linux kernel retains real CPU scheduling, virtual memory, page reclamation, compression/swap, drivers, power governors and device isolation. Applications retain their own algorithms and hardware backend execution.

PHYSICS: RAM is data and state storage, not a replacement CPU/GPU. CPU, GPU or NPU must actually compute. Integrated GPU may share RAM, but usable compute requires driver/kernel/API/operations and contention proof. Swap can enlarge available address space, not fast physical RAM. zRAM uses compressed physical RAM, zswap caches swap pages compressed in RAM (normally before backing swap), disk swap uses much slower SSD/UFS/eMMC. Colibri expert streaming is **application-owned file I/O** with model-aware cache, not generic virtual memory or Linux swap. No infinite-RAM claim.

## 1. Reuse frozen source and authorities

- docs/PRIME_OS_MASTER_PLAN.md: Linux foundation, self-local Prime Host identity, driver trust, image generations, power/thermal authority, minimal footprint.
- docs/contracts/PRIME_HARDWARE_GRAPH_V1.md: actual kernel/firmware discovery, device/driver binding and missing-data limitations. Device detected does not mean fully functional.
- docs/contracts/PRIME_CAPABILITY_INTERFACE_V1.md: versioned, health-qualified capability metadata; typed local actions, separately authorized. Never introduce an unbounded root shell endpoint.
- docs/contracts/PRIME_WORKLOAD_POLICY_V1.md: CPU weights/quotas, memory and swap limits, GPU policy, IO quotas, network/device/secret/background permissions; same rule for every backend.
- docs/contracts/PRIME_SYSTEM_STATUS_V1.md and docs/contracts/PRIME_STORAGE_INVENTORY_V1.md: source-observed metrics; recover before field proposals.
- docs/PRIME_STORAGE_INTELLIGENCE.md: filesystem/storage accounting and safety truth, not model memory streaming.
- docs/PRIME_DESKTOP_CAPABILITY_NATIVE_CROSS_CHAT_RECOVERY_2026-11-15.md: wider design context.

Already documented — No update required for these foundations. Only approved, demonstrated missing semantics should be defined in a later additive contract and reviewed against them.

## 2. Proposed capability/metric classes

CPU: architecture, clusters/cores, ISA and load, cpufreq where exposed, CPU and PSI pressure, CPU quota, measured throughput/cost where qualified.
GPU: real DRM/driver/firmware and user-space compute backend availability, memory budget, data-copy vs shared-buffer support and compatible operations; GPU detected != model kernels on GPU.
NPU: actual driver, firmware, user-space runtime, supported operators and transfer costs; vendor marketing silicon is not accessible capability proof.
Physical RAM: total/available, reclaimable, current system and workload budgets, memory pressure, page fault rates.
Shared memory: iGPU/system RAM contention, drivers and DMA sharing where supported, synchronization and truthful zero-copy limitations.
Compressed memory: zRAM and/or zswap existence, size, real compression ratio, actual memory cost, writeback, faults, CPU overhead and failures.
Storage-backed swap: availability, encrypted backing, active swap pages, latency and bandwidth, write amplification and flash health, device-specific limits.
Application streaming: Colibri or another application owns model weight/dataset loading and cache; Prime exposes IO resource/power constraints, not model-internal semantics.
Power/thermal: battery, charger status, energy limits, real sensors, thermal throttling, safe sustained load.
Driver/device support: architecture, board/SoC ID, exact driver/firmware/module/HAL, successful capability-specific physical tests, degraded/unknown reasons.

Every exposed observation requires provenance, timestamp, version, capability availability/health and known unknowns. Supported, installed, driver-bound, operational and physically qualified are different states. Do not fabricate exact performance/power numbers.

## 3. Resource policy scenarios

Prime Desktop, small RAM plus NVMe: preserve interactive shell and essential services while an approved build or large application runs. Compare RAM-only, zRAM, zswap+NVMe swap, or direct swap only where qualified. Colibri may stream expert weights from disk; that is independent of swap. Heavy active swapping can make a desktop slower even though allocation succeeds.

Prime Mobile, shared RAM and battery: prioritize calls/radio, system UI, input, audio, notifications, thermal and battery. Evaluate bounded zRAM and *conditional* storage swap only with power, latency, flash wear and secure storage proof. Memory optimization must not cause missed calls, thermal shutdowns, blocked touch input or catastrophic storage wear.

Docked/powered phone: optional explicit host profile with background server, compiler or inference; charging and thermal policy still applies, safe suspension/recovery and networking/ingress must be proven. No always-on host service merely because RAM is large.

Third-party application: should be able to query a stable versioned Prime capability and request an eligible backend. Prime enforces admission/security/resource budgets; the application chooses its algorithm. PHI does not automatically translate arbitrary CPU code into GPU code.

## 4. Memory management qualification matrix

A. Physical RAM only: baseline.
B. zRAM compressed swap device inside RAM.
C. zswap compressed swap cache with explicitly supported and encrypted backing swap.
D. Direct disk-backed swap only where measured and justified.
E. Application-owned file streaming, for workloads designed for it.
F. Throttling, disabling optional background work or truthful OOM rejection where swap would ruin the user experience.

Do not enable every compression/swap layer blindly. zRAM and zswap can overlap or impose wasteful CPU cost; assess exact device and kernel config. Measurement includes PSI memory.pressure, memory.current and memory.swap.current, page faults, zRAM/zswap actual use/evictions, p95/p99 storage latency, bytes written, energy, shell input latency, call/audio continuity, thermal and workload completion/abort. Any profile thresholds must come from real measured evidence. No swap may expose plaintext secrets or compromise encrypted storage, suspend/resume, OTA/rollback or flash endurance.

## 5. Universal third-party hardware support

A driver/port developer should be able to support qualified new boards without rewriting Prime Core. Proposed device support description: exact vendor/model/board revision/SoC, CPU/GPU/NPU, boot firmware and partitions, Device Tree/ACPI, kernel config/module KMI, required firmware distribution constraints, trusted image signatures, userspace graphics/accelerator HAL where required, protected peripherals and resource/power constraints. Tests must be per function: boot, display/touch, input, Wi-Fi/Bluetooth, modem/calls/SMS/data, audio/microphone, cameras, GPS/sensors, USB-C host/device/PD, charging/standby/thermal, encrypted storage, safe update/recovery.

Use existing Prime T0 trusted upstream/T1 verified vendor/T2 or T3 controlled developer trust categories as defined in Master Plan; exact tiers must be read from authority. A kernel/boot/core-driver change travels via a signed generation update, never an unprivileged userspace package mutating the image-owned base. Initially device ports may live under a governed Prime-OS device directory; later separate maintainer repositories can be useful after the common manifest ABI, signatures and admission are frozen.

For an Android-origin handset, the Android Common Kernel/GKI plus vendor modules/firmware/HAL can differ substantially from generic Linux. A mainline path is preferred where actual hardware works; a vendor-kernel approach may be required for a first port, with explicit compatibility/security limits. Android HAL binaries do not automatically work in glibc userspace. A booting shell is not a complete phone OS. No blind bootloader unlock/flashing to prove an unreviewed concept.

## 6. Donor boundaries

Colibri: optional model runtime (CPU/Vulkan/CUDA/HIP and model-specific memory tiers); only consumes PHI resource/capability metadata if integrated. Neither Colibri nor its model weights are part of Prime Core.
D-XQT: optional deterministic source-language and WASM runtime for qualified app events; NOT Linux mm, security authority or flashing backend.
Prime Manager: optional deep workload/process/resource UI; observes PHI, requests typed approved actions.
Prime Terminal: optional deterministic CLI for queries; no generic root escape.
Oracle/MIBU/TTG-UNLOCK: optional authorized remote/device/client applications; device-specific protocols belong to qualified providers.
Other developers: equal access to Prime's standard API, no mandatory account, model, THETECHGUY tool or privileged agent.

## 7. Proposal implementation gates (PHI-0 to PHI-6)

PHI-0: recover all existing Prime contracts; produce exact gap/owner matrix before an ADR. No duplicate daemon.
PHI-1: read-only versioned resource/capability reporting, unknown/provenance honesty on desktop and ARM64 emulator; no automatic mutation.
PHI-2: bounded policy selection using existing Prime Workload Policy, security and power/thermal authority; no separate scheduler.
PHI-3: real Desktop memory-tier campaign across RAM/zRAM/zswap/storage streaming and pressure/latency/failure/recovery; measure benefit before defaults.
PHI-4: real Mobile thermal/telephony-survival/energy/flash-wear campaign for selected handset; no unqualified swap profile.
PHI-5: optional app-provider test with Colibri CPU and actual GPU/NPU only when physically qualified, plus non-AI apps; dormant apps consume no AI resources.
PHI-6: third-party device-port manifest, SDK validation, hardware trace/evidence and signed release candidate; independent review and owner freeze of any newly proposed contract.

## 8. Sources and recovery

- Prime OS Master Plan: https://github.com/jaydumisuni/Prime-OS/blob/main/docs/PRIME_OS_MASTER_PLAN.md
- Prime Hardware Graph: https://github.com/jaydumisuni/Prime-OS/blob/main/docs/contracts/PRIME_HARDWARE_GRAPH_V1.md
- Prime Capability Interface: https://github.com/jaydumisuni/Prime-OS/blob/main/docs/contracts/PRIME_CAPABILITY_INTERFACE_V1.md
- Prime Workload Policy: https://github.com/jaydumisuni/Prime-OS/blob/main/docs/contracts/PRIME_WORKLOAD_POLICY_V1.md
- Colibri: https://github.com/jaydumisuni/colibri ; docs/edge-runtime.md ; GPU_BACKENDS.md
- Linux cgroup v2: https://docs.kernel.org/admin-guide/cgroup-v2.html
- Linux zswap: https://github.com/torvalds/linux/blob/master/Documentation/admin-guide/mm/zswap.rst
- Android GKI vendor architecture: https://source.android.com/docs/core/architecture/kernel/generic-kernel-image
- Mainline mobile constraints: https://develop.kde.org/docs/plasma-mobile/porting/
- Separate Prime OS Mobile roadmap: docs/PRIME_MOBILE_ROADMAP_2026-11-15.md (create/verify separately).

Contribution type: NEW named shared design proposal and cross-project memory/compute synthesis. Not a production implementation, acceptance, frozen contract or PHI benchmark. Future chat must recover exact project heads and physical evidence before making claims.
