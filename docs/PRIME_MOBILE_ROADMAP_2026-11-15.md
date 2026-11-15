# Prime OS Mobile — Independent Hardware/UX Roadmap on Shared Prime Core

**Date:** 2026-11-15
**Status:** Owner-directed STARTING ROADMAP / DRAFT FOR ARCHITECTURE REVIEW; NOT an approved kernel port, device flash, physical qualification, production build, freeze or product release.
**One-line decision:** ONE Prime OS repository and shared core; DISTINCT Desktop and Mobile release targets, roadmaps, images, hardware proofs and acceptance. **Do not create another Prime OS core/repository now.** Device-support packages may grow into independently maintained repositories later after a common trust and compatibility interface exists.
**Owning repository:** https://github.com/jaydumisuni/Prime-OS
**Shared PHI proposal:** docs/PRIME_HARDWARE_INTELLIGENCE_PROPOSAL_2026-11-15.md
**Desktop shared design recovery:** docs/PRIME_DESKTOP_CAPABILITY_NATIVE_CROSS_CHAT_RECOVERY_2026-11-15.md

## Executive pickup — what Prime OS Mobile actually is

Prime OS Mobile is the user's chosen **phone/mobile extension of the same clean, lightweight Prime OS**, not a rebranding of Nodie OS or a new THETECHGUY business application. It should be a full touch-first, mobile-device operating system built on a Linux-kernel foundation, with real telephony, modem, media, battery, sensors, graphics, mobile security, user accounts and updates where physically supported. It is NOT the existing Desktop P4B Android Personality. P4B means the ability to run Android workloads/apps on a Prime Host; Mobile means Prime itself running as the primary OS on a phone. An eventual Mobile Android-app provider may use P4B concepts after real arm64/HAL/runtimes proof, but the mobile release does not magically inherit a working Android app stack.

The owner wants an experience usable by ordinary people *and* developers: native touch Shell, normal phone functions, on-demand installation of toolchains/runtimes, optional cross-platform app support, optional secure remote client/workstation, browser or native USB host for supported phone repair, self-hosted web/API services when explicitly enabled, and optional local AI/Colibri **without making AI part of Prime Core**. The architecture is general-purpose: Prime as a platform where ANY developer may add signed qualified device support, drivers, toolchains, models, applications or hardware accelerators using stable interfaces.

The name **Prime Hardware Intelligence (PHI)** was accepted in discussion for shared hardware awareness and adaptive resource use. PHI belongs to both Desktop and Mobile, based on existing Prime Hardware Graph, Capability Interface, Workload Policy, power/thermal and Linux kernel mechanisms, not a separate privileged model or bloatware service. CPU/GPU/NPU compute and RAM/storage tiers must be understood honestly. zRAM, zswap, storage swap and model expert streaming are DIFFERENT tools, never marketed as unlimited physical RAM.

## Decision: monorepo/core, separately governed targets and releases

### Same source authorities

Shared Prime Host identity semantics, hardware graph, capability version negotiation, Prime Exec, Application Profiles, Workload Policy, storage/recovery/generation, machine security, driver-trust design, service/typed action contracts and additive PHI observations must remain ONE source of truth. No alternative Mobile Core architecture or incompatible second model of Host IDs, process authorization, app profiles or memory policies.

Prime Desktop's current P1/P4A work stays on its established lanes. Mobile receives a **distinct M-series roadmap** and build/release/proof records. An M-series pass does not certify Desktop P1, Windows Personality, KRATOS GPU or Desktop shell; a Desktop success does not prove a modem, touch screen or Mali GPU works on a phone. Each variant produces own hardware and release evidence.

### Different integration and user interface

Desktop: x86_64 KRATOS target first, UEFI/OVMF image/generation, desktop compositor/Shell, desktop power/graphics/audio/driver set.
Mobile: arm64 SoC and exact device boot chain; touch-first Shell, rotation, soft keyboard, gestures, notifications, calls/messages, permissions and lock screen; Android-origin kernel/vendor modules/HAL or mainline device drivers according to proven hardware; phone update/recovery packaging and power policy. Mobile first proof and each new board must identify exact hardware and boot-chain restrictions.
The shared Rust/core logic may be cross-compiled, but kernel images, boot partitions, initramfs, device tree, firmware, HAL, user-space drivers, system UI and release images differ as necessary. No literal reuse of desktop x86 boot binaries. No Linux/Android/macOS/iOS runtime inference from file extension alone.

### Release naming (design proposal, not frozen)

Two public product families:
- Prime OS Desktop: qualified x86_64 (later other) desktop images and stable/candidate/development channels.
- Prime OS Mobile: qualified arm64 **device-scoped** images, candidate/development/stable channel only after device proof, signed board/kernel/vendor packages and recovery manifest.
Release artifact examples (proposed only): prime-os-desktop-x86_64-<version>; prime-os-mobile-a14m-sm-a145p-<version>. These are NOT claims that such images now exist. Even two arm64 phones cannot safely share one generic flashable image when their boot partitions, DTBs, firmware and drivers differ.
Shared core semver/schema/ABI milestones must pin compatible interfaces; independent mobile release cadence. System-generation updates distinct from optional component packages (and Prime Store eventually), with rollback/rescue; never let ordinary optional app installation silently patch boot/kernel/Prime Core or protected vendor modules.

### Repository topology (future candidate, not source changes today)

- Existing crates/prime-* and existing contracts remain shared foundation.
- Proposed mobile-specific roots when implementation starts: mobile/ for platform integration, mobile/shell/ for touch UX, mobile/device-ports/<vendor>/<codename> for board manifests and scripts, mobile/tests/ for ARM64 emulation/physical proof, mobile/evidence/ for redacted hardware and release records. These are example names, not established package architecture.
- Proposed docs/PRIME_MOBILE_ROADMAP_2026-11-15.md = this roadmap. Shared PHI design at docs/PRIME_HARDWARE_INTELLIGENCE_PROPOSAL_2026-11-15.md.
- Later separate independently maintained device-support repo(s) may be justified by different maintainers/licenses/vendor artifacts and independent hardware review. Each device bundle must declare Prime compatibility, kernel/module ABI, signatures, board/firmware range, capabilities and rollback; Prime Core stays canonical.
- A separate Prime-Mobile *product code repository* is not presently justified; it would duplicate/shared contract drift before the first phone boots. The word 'Mobile' distinguishes an edition/release target and roadmap, not a second OS authority.

## Hardware and baseline: Samsung Galaxy A14 4G SM-A145P

**Provisional first physical experiment device from prior owner discussion:** Samsung Galaxy A14 4G, model SM-A145P, product/codename a14m, MediaTek Helio G80 MT6769 family, ARM64 Cortex-A75/A55, ARM Mali-G52 MC2. This is **not** Snapdragon; do NOT introduce Qualcomm/Turnip graphics assumptions for this board.

External identity pointers: https://devcheck.app/devices/samsung/a14m , https://github.com/forforksake/recovery_device_samsung_a14m , https://github.com/forforksake/android_kernel_samsung_a145p . These are background/donor leads, NOT verified boot support for Prime or a green light to flash. Device revision, current firmware build, actual partition layout, bootloader lock state, kernel/vendor interface, touch panel variant, baseband/modem, display/touch/audio/radio HAL and media services MUST be recovered from the physical owner's device before planning any mutation.

**No current physical acceptance evidence:** a14m boot to Prime, reliable recovery, usable DRM/display/touch, Mali GPU, modem telephony, SMS/cellular data, Wi-Fi/Bluetooth, camera, GPS, USB Host/MTP/ADB, charger, battery, suspend, thermal, storage crypto or OTA. Treat all as UNKNOWN/PENDING.
Do not infer support from a recovery-device-tree repository alone. A donor may prove its own recovery UI/USB work but does not prove complete upstream Linux or Prime operating-system feature parity.
The owner may use the phone as a daily driver. All initial work must be read-only investigation and emulator/secondary test context until the owner explicitly authorizes a device change with a tested restoration path. Do not alter partitions/bootloader, wipe data, downgrade firmware, disable verified boot, flash recovery or sacrifice calling functionality to 'just test'. Prefer lab/spare device if available; no unstated spare is assumed.

## Mobile kernel and hardware enablement strategy

The Linux foundation is shared in principle, but phone vendors often rely on Android Common Kernel / vendor kernels, non-mainline patches, proprietary firmware and Android HAL service stacks. Full mainline Linux on a given phone may be incomplete even when a generic ARM64 userspace runs. Public technical references:
- Android GKI: https://source.android.com/docs/core/architecture/kernel/generic-kernel-image
- Android modules: https://source.android.com/docs/core/architecture/kernel/modules
- vendor boot partitions: https://source.android.com/docs/core/architecture/partitions/vendor-boot-partitions
- KDE Plasma Mobile porting caveats: https://develop.kde.org/docs/plasma-mobile/porting/
- postmarketOS device support classifications: https://docs.postmarketos.org/pmaports/main/packaging/device-categorization.html

**M0 must choose by actual evidence** among:
A. Mainline kernel and sufficiently supported ARM64 board/DT/driver peripherals.
B. Vendor/Android kernel, device-specific module/DT/vendor boot/firmware integration plus necessary user-space compatibility surfaces where technically/legal possible.
C. Hybrid/transition route, migrating drivers to mainline or using a constrained compatible HAL adapter with explicit restrictions.
No automatic 'copy Android drivers and everything works' claim. Android Bionic-based HALs and Android service assumptions may require dedicated transport/compatibility and are not native glibc modules. An arbitrary OEM phone may have secure boot/AVB/Knox and flash policies requiring vendor/owner authorization; impossible/unsafe paths must fail closed.

Driver trust follows Prime Master Plan: upstream trusted, verified vendor, controlled third-party/developer trust tied to exact digest, owner, target scope, signature, capability and runtime evidence. Developers must be able to add device-specific functionality without rewriting generic Prime Core. Device Tree/ACPI is hardware description, NOT a missing-driver generator. Boot/firmware/kernel changes use signed generation/device-image update and recovery, not ordinary store app packaging.

## Mobile OS feature definition

### Always-needed device functionality, as physically applicable

- Power/charging, safe battery status and budget, wake/sleep/resume, thermal sensors and emergency shutdown.
- Touch/display/compositor, rotation/orientation, accessibility basics, on-screen keyboard, lock screen, notifications and permissions.
- Telephony/radio calls, SMS/MMS as supported, cellular data, SIM detection/eSIM only where physically supported, and emergency-calling/legal/regulatory constraints; never call a phone 'daily-driver ready' without meaningful network/call proof.
- Wi-Fi/Bluetooth, microphone/speaker/earpiece, headset/USB audio, camera where supported, sensors, location/GNSS and timekeeping.
- Device user data and secure storage, access control, encryption at rest, recovery/update partitions and owner rollback.
- USB-C role switching, data path, MTP/device and USB Host/OTG where real kernel/board support; power delivery/dock compatibility separately proven.

### Optional capabilities by installation and actual qualification

Developer tools, terminal, Prime Manager, D-XQT deterministic event runtime, Colibri inference engine and separate model files, Android app/personality provider, compatibility translators, web/API hosting, storage/developer container/VM provider if supported, device-repair tool/provider, Oracle remote client, hosted model adapters and future third-party components.

Normal phone use does NOT require Colibri, Hunter, an AI model, cloud subscription, vendor repair toolkit, Android compatibility personality, desktop VM, remote Athena/KRATOS, Node/Python workers or extra user-visible Store. Do not bring up heavyweight components at boot merely because supported.

## PHI on Desktop + Mobile: one mechanism, hardware-specific policies

**Prime Hardware Intelligence = owner-approved NAME, reviewed proposal.** Reuse Prime Hardware Graph and Capability Interface for read-only observations; Prime Workload Policy for actual resource enforcement; Linux for scheduler, memory management, zRAM/zswap/swap, cgroup/PSI and device driver APIs.

PHI should be able to report physical memory, compressed memory, storage-backed swap, page reclaim/pressure, I/O latency, GPU/NPU actual backend availability, shared memory, battery, charging and thermal state with provenance/unknown flags. It does not transparently turn RAM into GPU compute or transform storage into physical-speed RAM.
- Mobile: first protect interactive shell, calls, radios, audio, battery and thermal. zRAM may aid memory capacity; storage swap on UFS/eMMC requires write-endurance, encryption, energy and tail-latency proof, and may be deliberately off. No blind simultaneously active zRAM/zswap overcommit.
- Desktop: compare RAM-only, zRAM, zswap + qualified NVMe swap, direct swap and application-owned streaming on actual storage. Never describe allocated swap as equally fast RAM or inflate benchmark claims.
- Colibri donor: model-aware hot-expert cache in RAM/GPU, cold experts from disk, real CPU/Vulkan/CUDA/HIP where supported. The application owns model math/weight I/O. PHI exposes device capability/budgets and helps avoid GPU memory/thermal contention; no model weights resident by default.
- Other users: same PHI API for apps, compilers, media, games, browsers, web servers. No model needed to optimize mechanical resources.
- Developer-supplied board port may expose a qualified Vulkan/compute backend to PHI; detection of Mali-G52 alone is not a production Vulkan inference proof.

**PHI is shared**, so do not create a Mobile-only PHI fork. Its independent proposal and gates are in the companion PHI document. Avoid introducing a new privileged PHI background agent if the existing Prime Core/systems API can own the observations.

## Remote/workstation and repair scenarios, not Mobile first-boot blockers

A phone running qualified Prime Mobile might:
1. Perform local developer/server work under approved power/thermal/resource/background policy, with security and reachable network ingress where relevant.
2. Act as a field technician workstation: connect a customer device over Type-C/OTG to a supported native diagnostic/repair provider, or operate tools on an authorized remote ATHENA Windows/KRATOS Linux machine.
3. Open TTG-UNLOCK customer site or act as technician for remote sessions, where support/browser hardware API permits.
4. Run a chosen small local inference model through optional Colibri or use selected remote Cloudflare 70B/other model; D-XQT for bounded deterministic programs, never as system-level mandatory AI.
5. Install third-party apps/ports/toolchains that use standard Prime machine capabilities.

**Do not repeat previous browser error:** owner reports actually flashing a Samsung using https://samfw.com/Odinweb on 15 Nov 2026, and its browser page advertises desktop Chrome/Edge Web Serial. Browser-native WebUSB is another distinct supported transport for some protocols. Thus a native bridge is not ALWAYS required; nor does SamFW desktop Chromium prove mobile browser Web Serial/USB support or that all protected device modes are available. Remote technician consent/case and physical USB endpoints are independent security concerns. See https://docs.google.com/document/d/1iD1RYlgqclPHySmADfjX5jaaMANxCWD1mraXZZ4BV5Q/edit?usp=drivesdk .
Device repair/flash/unlock authorization remains the responsibility of a qualified, user-authorized device application, not Prime Core, PHI, D-XQT or inference models. Prime itself only exposes typed OS/hardware capabilities and enforces local policy.

## M-series roadmap: Prime Mobile independent from existing P0–P8

The M-series is a NEW Mobile edition lane. It neither renames nor replaces the existing Prime Desktop P0/P1/P1.5/P2/.../P4A/P4B milestones. Mobile milestones can run parallel after shared interfaces are ready, with each phase claiming only its own proof. No phase auto-marks Desktop source KNOWN_GOOD.

### MOB-0 — Program authority and device feasibility (FIRST)

**Goal:** prove there is an authorized, technically plausible, reversible route for Prime Mobile on chosen ARM64 proof hardware before selecting the kernel or changing a handset.

Tasks:
- Recover Prime core Master Plan, accepted supplements, capability/exec/workload/driver/security/storage contracts, this roadmap and PHI proposal; reconcile against local KRATOS worktrees before implementation.
- Confirm phone model/SoC/board variant and exact stock firmware/kernel/GKI/vbmeta/boot/vendor_boot/init_boot/recovery/super partition topology where applicable, OEM unlock status and restoration/relock implications, support tools and recovery images. Read-only first.
- Compare mainline versus vendor/Android kernel and optional user-space HAL bridge via device-specific source and actual driver evidence.
- Define minimal Mobile product acceptance matrix, protected daily-driver requirements, accessible developer mode and phone privacy/security, independent release-image boundaries.
- Define a device-port manifest candidate, exact build provenance, hardware acceptance fixtures, compatibility with existing Prime Host and Workload Policy, version negotiation and architecture split.
- Declare PHI/desktop shared versus mobile-specific rules; review ARM64 cross-compilation plan and mobile Shell technology.
Exit: reviewed architecture/feasibility record, identified kernel/boot constraints, read-only physical inventory, non-destructive recovery plan, explicit STOP for unsupported boot chain. **No flashing or user-data change**.

### MOB-1 — ARM64 emulator / virtual First Light

**Goal:** make the shared Prime Host mechanics operate on an ARM64 Linux-based virtual target before touching any device.

Tasks: cross-build/reuse selected Prime Rust crates; ARM64 machine identity and hardware graph honest for emulator; capability version negotiation; Workload Policy and service lifecycle; basic init/login, storage generation and recovery; touch-style display/input simulation; safe network and userspace service smoke; PHI read-only memory and CPU observations; no forced AI.
Exit: deterministic virtual boot/source review, actual shell readiness, failure/recovery, bounded resource and security proof, exact artifact/host identity; **not** a physical phone proof.

### MOB-2 — First physical device boot and safe recovery

**Goal:** one unmistakably Prime mobile hardware bring-up with a reliable recovery path.

Entry: approved test device, device-specific image/boot route, lawful/unlocked or otherwise supported boot chain, tested stock restore and owner authorization. Do not conflate a 'kernel boots' with a complete phone.
Tasks: CPU and memory, storage partition safety, display framebuffer/DRM/panel, touch, logging and input, charger/power/battery, USB debugging transport, power-off and recovery, error/halt path, persistent Host identity and generation, realistic hardware graph. Use source/device-tree/vendor modules as dictated by qualified hardware support.
Exit: exact hardware revision evidence of display/touch and essential power, bounded failure tests and factory/previous state restoration. No daily-driver claim.

### MOB-3 — Phone essentials and sleep/telephony proof

**Goal:** provide a *usable phone*, not merely Linux on a phone.

Tasks: modem/radio/SIM registration, voice call and emergency-path behavior where lawful to test, SMS, data, Wi-Fi/Bluetooth, audio/mic/earpiece, time, GPS/sensors, mobile data policy, battery/charging/suspend/resume/wake, thermal and resource pressure, encryption/key management, permissions, crash recovery. Camera may have separate provider/hardware gate but must not be silently advertised as supported.
Exit: written observed checklist with real network/call audio/data, mobile wake/standby, thermal/safety and recovery, missing hardware statuses. Emergency call testing must avoid contacting emergency services except through authorized controlled procedures. Without functioning approved basics, remain an experimental device port, not daily-driver build.

### MOB-4 — Prime Mobile Shell and ordinary application environment

**Goal:** production-quality touch-first Prime identity independent of stock Android/desktop theme.

Tasks: lock/login, launcher/home, gesture navigation, responsive compositor, on-screen keyboard/input methods, notifications/status, call/message UI over real service backend, Wi-Fi/Bluetooth/data/audio/battery quick controls, rotation, accessibility, privacy indicators, permissions, screen capture/media and configuration. Keep Prime visual language, reflow layout to mobile without merely shrinking desktop rail. App profile/exec and normal native ARM64 app launch; no hidden mandatory AI, compatibility framework or fake UI state.
Exit: physical touch/visual owner acceptance, interruption tests (incoming call during workload), UI responsiveness, permissions, real state labels and resource limits. Owner's pixel-level acceptance remains separate from logical functionality.

### MOB-5 — PHI memory, compute, battery and storage adaptation

**Goal:** prove the owner's key insight across actual mobile hardware, then feed approved generalizable changes back into shared PHI.

Tasks: compare actual memory/CPU/GPU/NPU health and backend drivers, measure swap configurations (physical RAM only, zRAM, zswap+storage swap where supported, no swap where flash/power/latency harms), cgroup budgets/PSI/reclaim, shared GPU memory pressure, CPU and GPU thermal power, optional device-dependent profile. Protect audio/calls/UI while compiling or hosting a background workload. Review flash writes, battery and suspend. Try Colibri only as OPTIONAL application and only with a compatible model/hardware; compare CPU against qualified GPU if available, never claim Mali GPU support because the name appears.
Exit: physical metrics, negative OOM/thrash tests, no essential-service regression, resource savings/performance proved where measurable, exact profile and fallback evidence. A phone with 'additional swap' does not become an equal-capacity high-speed-RAM phone.

### MOB-6 — Portable workstation and optional device-service providers

**Goal:** add the distinctive *field developer/technician* capability without making it default phone bloat.

Tasks: optional Prime Terminal/Manager interfaces (not new implementations), approved local web/API servers with explicit listener and background/power/storage policy, USB-C Host/OTG role, real qualified USB diagnostic/ADB/Fastboot/serial provider for devices and safe mode selection, authorized Oracle-style remote ATHENA/KRATOS workstations, secure device permission/reconnection/recovery. Use browser-native hardware APIs where actually supported; do not mandate a native helper. Integrate TTG-UNLOCK only as optional separate application with explicit customer ownership/approval, not a Prime OS core feature. Optional D-XQT event logic and Colibri inference may be demonstrated as independent user-installed packages.
Exit: per-transport functional proof of real USB data, real remote authentication/disconnect truth, local host availability and rollback; no 'fix any phone' claim without model/mode-specific evidence.

### MOB-7 — Application ecosystem and optional compatibility

**Goal:** allow third-party development, native applications and qualified additional platforms.

Tasks: signed/capability-scoped component installation, native ARM64 app developer SDK, install/update/remove rollback, catalog/Store client later under shared Prime P2/P3 component authority, reproducible builds/signing. Work on Android APK support only as an optional *application compatibility runtime* with proven Binder/ART/vendor facilities (P4B donor). iOS/macOS/Windows user apps use actual local qualification, distinct translation/personality or authorized remote Provider; recognizing IPA/PE is not evidence that it runs. Distribute optional D-XQT, Colibri and model weight packs only on user request. Preserve normal-user profile without dev tooling.
Exit: ordinary independent apps install/run/uninstall with evidence, app permissions, negative compatibility tests, no unsupported cross-platform marketing.

### MOB-8 — Device enablement SDK and multi-device expansion

**Goal:** independent developers add supported phones/boards safely.

Tasks: publish exact port manifest schema and versioned Host/device capability API, source/provenance/firmware legality, required config/modules/DT/driver/HAL, trusted-signed packages, device-specific kernel/generation update rules, immutable base separation, documentation and example port, CI static/emulator tests, hardware proof/evidence template, recovery/unbrick policy, trust tiers and revoked driver handling. Distinct reference device and third-party board should consume same Core contracts. Use governed external device-port repositories only when source maintainership and stable manifest justify split.
Exit: a second hardware family can be integrated by qualified independent contributors **without forked Prime Core or fake support**; accepted board-specific proof and signatures. No untested universal flash image.

### MOB-9 — Survival, release candidate and stable Mobile

**Goal:** independently ship a recoverable mobile product after all higher-impact hardware gates close.

Tasks: signed release target and reproducible image, protected per-device partition/firmware mapping, dual-generation/rollback or proven supported alternative; battery loss/download interruption/staging corruption/boot-failure recovery; user-data preservation and migration; screen/radio/audio/thermal regression campaign; app/driver compatibility through upgrades; privacy/security/revocation; physical daily-driver/hardware matrix and owner UX acceptance. Distinguish lab/developer preview/candidate/stable per exact board variant. Public stable requires continuous update/recovery and essential phone guarantees; experimental boot images never masquerade as stable.
Exit: independently reviewed exact-head source, hardware/feature evidence, recovery success and owner acceptance. The desktop P1/P1.5 and mobile MOB-9 proof remain separate.

## Proposed acceptance evidence for every MOB gate

- Source revision, branch/image digest, firmware/kernel and exact hardware ID (privacy-redacted) and method of boot/admission.
- Capability/Driver status: UNKNOWN/UNSUPPORTED/RECOGNIZED/BOUND/FUNCTIONAL/PHYSICALLY_PROVEN/DEGRADED with evidence references and limitations.
- Positive, negative, failure, recovery, resource, privacy and security cases; real radio/network/audio/USB/charging and power state where relevant.
- No bluffing about graphics: framebuffer image != GPU compute, recognized Mali != Vulkan acceleration, present modem != voice/SMS, USB-C shape != Host/PD, ADB connection != full platform qualification.
- Owner review for visual quality and destructive device actions. Independent engineering review/freeze before release. No generic script-run success promoting a device to daily-driver status.
- Maintain source snapshots and local worktree caution: current GitHub remote status may differ from active KRATOS or ATHENA bytes. Recover local work using Oracle when actually connected before asserting implementation.
- If a missing driver blocks a phase, report precise cause/dependency; do not invent an unsupported path or silently ship absent functionality as complete.

## Roadmap dependencies and immediate first bounded next task

PHI-0 design inventory and MOB-0 architecture may begin on paper now and should cross-reference rather than reimplement each other. MOB-1 ARM64 emulator should use frozen compatible Prime Core APIs. MOB-2 physical boot is gated by owner authorization/restore path. MOB-3 is a prerequisite for normal phone/daily-driver claim. MOB-6 and MOB-7 optional tools depend on stable enough system support and can be developed in simulation without making MOB-3 pass. MOB-8 creates qualified port scalability; MOB-9 makes a public release. Desktop ongoing P1 known-good and W8 guest/Windows certification should not be disturbed by Mobile planning.

**First task after this roadmap (not started here):** recover Galaxy A14 SM-A145P current physical stock/boot/firmware evidence read-only, select a plausible kernel/driver/HAL route, and freeze a device bring-up/recovery plan; in parallel qualify ARM64 virtual Prime Core. There is no authorization to flash, unlock, wipe, factory-reset or replace the daily-driver OS from this document.

## 2026-11-15 D-XQT shadow simulation candidate — NOT MOB-0 physical acceptance

The owner authorized a KRATOS-independent structural SIM using D-XQT and iAM/COG-style currentness. A real D-XQT Rust v6 WASM VM executed 19 synthetic Prime Mobile boot-admission / PHI zRAM-swap-GPU / iAM evidence scenarios in an isolated ChatGPT workspace; 19/19 passed with negative verifier, host-effect, step-limit and deterministic replay checks. The exact source and fixture were fetched back from GitHub and rerun with the same successful result.

Source/evidence authority: [TTG-Simulation draft PR #1](https://github.com/jaydumisuni/TTG-Simulation/pull/1); branch `work/prime-mobile-dxqt-shadow-20261115` at `ac4563c924ebf97dc9ee9cc9a626248c86a20f65`; full readout in `docs/PRIME_MOBILE_DXQT_SHADOW_20261115.md` of that PR. This is **candidate structural shadow evidence only**, not an independently accepted TTG-Simulation generation, a kernel/ARM64 guest boot, phone-driver test, PHI performance benchmark, model/firmware identity confirmation or authorization to flash. The user's photograph shows a TWRP UI unlock overlay, NOT bootloader-state proof; owner separately reports bootloader unlocked.

The D-XQT bundled standalone WASM is a v6 bytecode runtime and did not support the newer v8 `compileModules` entry point. The host used valid compiled v6 bytecode and the real Rust VM rather than a duplicate interpreter. No D-XQT language feature change was needed. Use the TTG-Virtual-Hardware-Lab for later actual ARM64 guest proof, and maintain MOB-0 device boot-chain/restore/physical evidence gates unchanged.

## 2026-11-15 offline implementation and fault-recovery checkpoint — DRAFT ONLY

With KRATOS unavailable, isolated development was completed entirely in the ChatGPT workspace and published as **three draft review PRs**. These are implemented/tested *candidate* boundaries, **not** a real phone boot, an accepted simulator generation or a PHI production integration.

**1. Prime-OS [PR #223](https://github.com/jaydumisuni/Prime-OS/pull/223)** — branch `work/mobile-mob0-phi-readonly-20261115` @ `596243656d333112e191c670b5c563aec1c48631`. Generic read-only Linux PHI memory/PSI/zRAM/zswap/swap probe; ARM64 Device Tree/board probe; read-only observation-to-MOB-0 evidence capture bridge, iAM/X-Ray-inspired MOB-0 evidence intake; third-party device-port capability audit; two candidate schemas and provisional `a14m` manifests; checkpoint `docs/PRIME_MOBILE_OFFLINE_FIRST_LIGHT_CHECKPOINT_2026-11-15.md`. **34/34 Python tests passed** on exact source fetched back from GitHub; both JSON Schemas and their examples validated. No system daemon, Prime Core contract change, device write or hardware qualification.

**2. [TTG-Simulation PR #1](https://github.com/jaydumisuni/TTG-Simulation/pull/1)** — branch `work/prime-mobile-dxqt-shadow-20261115` @ `ac4563c924ebf97dc9ee9cc9a626248c86a20f65`. Borrowed actual D-XQT Rust/WASM **v6 bytecode** engine (the standalone WASM does not expose the newer v8 source-compile action) and executed **19/19** deterministic Prime Mobile / PHI / iAM-currentness synthetic worlds with negative verifier/effect/step-limit checks. iAM's full runtime was not executed. No ARM64 instruction emulation.

**3. [TTG Virtual Hardware Lab PR #3](https://github.com/jaydumisuni/TTG-Virtual-Hardware-Lab/pull/3)** — branch `work/prime-mobile-mob0-synthetic-recovery-20261115` @ `42b09d0d3c000d2e4eb48042bd03f858d73f7d96`. Reuses the actual lab's G0 **synthetic** flash/boot/fork/checkpoint engine: corrupt boot+recovery → `NO_BOOT_ROUTE` → simulate interrupted restore `VIRTUAL_POWER_CUT` → checkpoint restore recovery → `RECOVERY_ONLY` → restore boot → `NORMAL`. Parent unchanged and both receipt chains replay; **2/2 tests passed** on exact GitHub source and full report reproducible. This is **not a Samsung A14/Android kernel flash or physical restore path**.

**Current MOB-0 actual-hardware limits remain unchanged:** the photo shows TWRP Advanced with UI lock; user separately reports bootloader unlocked; current exact physical model, electronic unlock state, vendor boot/partition topology, kernel/DTB, stock restore path and permitted image route have NOT been read/proved. No ownership approval to mutate the phone was acted on; no Apple/Android/MTK modem/GPU capability was inferred. KRATOS physical ARM64 guest/QEMU validation, Rust Prime Core cross-build, phone boot and essential-service testing remain open. These draft PRs require independent code/policy review and accepted contracts before merge.

**Next offline work may continue from these exact PRs**, focusing on contract review, hostile cases, source integration and documented onboarding; do not start from scratch or create a second PHI/iAM/simulation runtime. **Next hardware work**, once KRATOS/phone access is restored, begins with read-only evidence and independently proven restoration, not an immediate flash.

## 2026-11-15 MOB-1 ARM64 Linux early-init cross-build — candidate, NOT guest boot

**Draft [Prime-OS PR #224](https://github.com/jaydumisuni/Prime-OS/pull/224)** — isolated branch `work/mobile-mob1-arm64-initramfs-20261115`. With KRATOS offline, the ChatGPT workspace's Clang 17/LLD 17 compiled a genuine statically linked ELF64 AArch64 Linux `/init` (4616 bytes, entry 0x21039c, no dynamic interpreter). A deterministic `newc` cpio initramfs includes that `/init` plus `/dev`, `/proc`, `/sys`. Its read-only guest probe is intended to report actual kernel architecture/PID1, memory and Device Tree *if later executed*. Early-init marker ends in `EVIDENCE_ONLY_CORE_NOT_STARTED`, **never** `SHELL_READY`.

**Source and proof:** `mobile/first-light/{src/early_init.c,build.py,verify_serial.py,README.md,tests/test_first_light.py}` and `docs/PRIME_MOBILE_MOB1_OFFLINE_ARM64_CROSSBUILD_2026-11-15.md` in PR #224. The 5 committed files were fetched back from GitHub and byte-compared, then rebuilt independently with **17/17 tests PASS**. Two build directories yielded identical ELF SHA256 `ea11e6b1c442e587eda841a054e043b93836e775d740e706bef238b4e9925445` and gzip initramfs SHA256 `e46c7d3381a9a4e7b4ea25bf80a447ca2fc0fd390065b05812a43ce780ae7b85`.

**Shared Core audit:** the draft PR additionally includes `docs/PRIME_MOBILE_ARM64_SHARED_CORE_GAP_AUDIT_2026-11-15.md`, tracing exact Rust sources for DMI-only Host fingerprinting, x86 CPU/P1 health assumptions, PCI-centric discovery, and `systemd-run`/`bootc` guest prerequisites. No Core source change or accepted contract amendment resulted from the audit.

**MOB-1 console-node correction (same draft PR #224), 2026-11-15:** Independent kernel-initramfs review found that the originally packaged cpio lacked explicit `/dev/console` and `/dev/null` nodes. Linux early PID1 could therefore have no usable console; the earlier 17/17 tests did not catch this. The corrected archive encodes real cpio character-device entries (console 5:1, null 1:3), has a fail-closed archive verifier and passes **21/21 local tests**. The AArch64 ELF SHA256 stays `ea11e6b1c442e587eda841a054e043b93836e775d740e706bef238b4e9925445`; **current** corrected cpio SHA256 is `646f0ad4150dc1be7cdc6ff44e4ea141e5950876bbf3a2b6da0de8c75f66f018`, gzip SHA256 `9ecb64769e08a54ca290a3d2f08ea629c0b077b1278cb749f1e515c0056f9f28`. Three modified files were independently matched against GitHub blob hashes. Exact recovery note: [MOB-1 Console Archive Correction](https://github.com/jaydumisuni/Prime-OS/blob/work/mobile-mob1-arm64-initramfs-20261115/docs/PRIME_MOBILE_MOB1_CONSOLE_ARCHIVE_CORRECTION_2026-11-15.md). The earlier archive hashes remain historical and **must not be used as the current boot candidate**. Actual guest boot and Rust Core execution are still NOT proved.

**Open gate:** This workspace did not have `qemu-system-aarch64`, a qualified ARM64 Linux kernel Image, `cargo`, `rustc`, or the handset. Therefore **no ARM64 instructions executed**, no Linux guest or actual Prime Rust Core booted and no phone was modified. Use existing TTG-Virtual-Hardware-Lab G0.7 pinned QEMU and reviewed Linux Image for genuine guest proof later. The generated cpio is NOT an Android/Samsung image. This phase supplements, not promotes, earlier draft Prime MOB-0/PHI PR #223, TTG-Simulation D-XQT PR #1 and Virtual Hardware Lab PR #3. Independent review still required.

## Recovery instructions for any new chat

1. Read the current Prime-OS README, Master Plan, accepted supplements, Current Status, existing Desktop Roadmap, AI_HANDOFF and planning/state.json. These retain *shared OS* authority and Desktop proof.
2. Read THIS Prime Mobile roadmap and shared PHI proposal; understand proposal status versus accepted frozen contracts. Then read the preceding detailed Desktop cross-chat recovery. Do not begin from generic mobile assumptions.
3. Recover exact Prime main and relevant branch heads, local KRATOS worktrees and device/ARM64 evidence; then relevant Linux/GKI/vendor-kernel/donor repositories. Distinguish installed vs hypothetical software.
4. If owner asks 'which repo': use Prime-OS shared core and two product/release families; separate hardware port repos only after governed driver port manifest and owner decision. Do not create another OS core by default.
5. Do not equate Desktop P4B Android Personality with Prime Mobile system bring-up. Do not equate PHI with Colibri, D-XQT, an AI model or Linux mm itself. No default AI, no Docker assumptions, and never call SIM unless owner authorizes it.
6. Understand → Build → Review → Freeze → Prove → Submit/Ship. Classify current chat-only contribution. Before recommending updates check Google Recovery, TTG-ecosystem, OS-playbook, TTG-progress, TTG-decisions, TTG-deploy-lab and owning repos. Source evidence wins over summaries.
7. Treat the SamFW browser flashing as user-reported physical observation and the official Web Serial interface as browser capability demonstration, not proof of Prime Mobile browser support.
8. Do not overwrite a user's original ROM, vendor partitions or boot chain merely because a mobile roadmap now exists. Preserve recovery and explicit owner permission.

## Source and cross-project authority index

- Prime OS: https://github.com/jaydumisuni/Prime-OS
- Prime Master Plan: https://github.com/jaydumisuni/Prime-OS/blob/main/docs/PRIME_OS_MASTER_PLAN.md
- Prime Current Status: https://github.com/jaydumisuni/Prime-OS/blob/main/docs/PRIME_CURRENT_STATUS.md
- Prime AI Handoff: https://github.com/jaydumisuni/Prime-OS/blob/main/docs/AI_HANDOFF.md
- Existing Desktop Roadmap: https://github.com/jaydumisuni/Prime-OS/blob/main/docs/PRIME_OS_ROADMAP.md
- Desktop full recovery: https://github.com/jaydumisuni/Prime-OS/blob/main/docs/PRIME_DESKTOP_CAPABILITY_NATIVE_CROSS_CHAT_RECOVERY_2026-11-15.md
- Proposed PHI shared design: docs/PRIME_HARDWARE_INTELLIGENCE_PROPOSAL_2026-11-15.md
- Prime-Manager: https://github.com/jaydumisuni/Prime-Manager
- Prime-Terminal: https://github.com/jaydumisuni/Prime-Terminal
- D-XQT: https://github.com/jaydumisuni/TTG-Event-Language
- Colibri: https://github.com/jaydumisuni/colibri
- Oracle: https://github.com/jaydumisuni/Oracle-
- MIBU: https://github.com/jaydumisuni/MIBU
- TTG-UNLOCK: https://github.com/jaydumisuni/TTG-UNLOCK
- TTG-UNLOCK complete cross-chat handoff: https://docs.google.com/document/d/1iD1RYlgqclPHySmADfjX5jaaMANxCWD1mraXZZ4BV5Q/edit?usp=drivesdk
- SamFW OdinWeb: https://samfw.com/Odinweb
- Samsung A14 SM-A145P device donor (not Prime proof): https://github.com/forforksake/recovery_device_samsung_a14m
- Android GKI: https://source.android.com/docs/core/architecture/kernel/generic-kernel-image
- postmarketOS board support classifications: https://docs.postmarketos.org/pmaports/main/packaging/device-categorization.html
- Plasma Mobile porting warning: https://develop.kde.org/docs/plasma-mobile/porting/
- Ecosystem governance: https://github.com/jaydumisuni/TTG-ecosystem ; https://github.com/jaydumisuni/OS-playbook ; https://github.com/jaydumisuni/TTG-progress ; https://github.com/jaydumisuni/TTG-decisions ; https://github.com/jaydumisuni/TTG-deploy-lab

## Session contribution record

**NEW:** Prime OS Mobile independent initial roadmap within shared Prime-OS repository; first phone proof matrix; mobile kernel/HAL and essential phone function gates; product/release family decision.
**UPDATE existing documentation:** PHI shared design proposal and indexes for cross-chat recovery.
**Already documented — No update required:** Prime Host/Core/Exec/Capability/Workload Policy, driver trust, immutable base and component Store split, Desktop P1 and P4A status, cross-platform runtime caution.
**No code changed, no new OS repository, no device flash, no SIM run, no ROM downloaded, no release build, no supported handset claimed.**
