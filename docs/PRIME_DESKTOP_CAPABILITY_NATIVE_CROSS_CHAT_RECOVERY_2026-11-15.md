# Prime OS Desktop — Capability-Native, Model-Optional and Portable-Host Direction
## Comprehensive cross-chat discussion recovery | 15 November 2026

> **Classification: planning/architecture discussion recovery, NOT an accepted amendment, implementation order, qualification verdict, product release, or modification to Prime's frozen contracts.**
> **Authority order:** the existing Prime Master Plan, accepted supplements, current status, exact owning project contracts, reviewed evidence and later explicit owner decisions override proposals in this note.
> **Purpose:** preserve the complete direction agreed/explored in the recent discussion so a new chat, agent, reviewer or engineer can recover it without replaying the conversation. This note mainly concerns **Prime OS Desktop**; it also records the proposed **Prime OS Mobile** sibling and the cross-device scenarios because omitting them would lose the reason for the desktop capabilities.

## 0. READ THIS FIRST: the whole direction in two minutes

1. **Prime OS is a clean, developer-neutral operating system, not a preinstalled THETECHGUY agent ecosystem.** Its Linux-kernel foundation, hardware truth, Prime Core, Prime Exec, security/Workload Policy, service and component lifecycle, Prime Capability Interface, Shell, update/recovery and storage mechanisms are OS responsibilities. Developers choose their own applications, runtimes, inference engines, local/remote providers, and background workloads.
2. **Support broadly; activate narrowly.** Do not load Windows/Android/Apple compatibility personalities, virtual machines, SDKs, AI models, huge weights, GPU helpers, USB repair engines, web servers, deep telemetry, model routers, Colibri, D-XQT or optional THETECHGUY agents merely because Prime supports them. Base support, optional package installation, activation, running, quiescence and removal are distinct states. A normal user should not pay the CPU/RAM/GPU/battery costs of unused features.
3. **Prime Exec is the format/architecture/backend selector; it is not magical universal compatibility.** Recognition of ELF/PE/EXE/JAR/DEX/APK/WASM/Mach-O/IPA does not prove execution. The chosen native/personality/VM/remote/specialist backend requires actual availability, appropriate ABI/API compatibility, signed/admitted workload and independently earned proof.
4. **Prime Manager and Prime Terminal have separate existing repositories and roles.** Manager is a workload-first, process-deep human operator app over Prime Core truth and typed actions, not the host authority. Terminal accepts common POSIX/CMD/PowerShell/macOS-overlap vocabulary by deterministic canonicalization, selects real backends through Prime Exec, and has proposed optional AI-assisted operations; it does not make AI the interpreter or grant root commands.
5. **D-XQT** (TTG-Event-Language) is a promising **first-class optional** compact deterministic event-program runtime. The experimental v8 Rust/native/WASM adapter can compile and run an event with explicit application-supplied host callbacks and then stop. It is not a privileged daemon, remote transport, application-permission engine or USB/flashing implementation; existing host effect/replay contracts do not supply a durable journal automatically.
6. **Colibri** is a possible **first-class optional inference provider/runtime**; it is not a third AI model. With compatible downloadable weights, it may support hardware-aware local inference, including mixture-of-experts weight/expert streaming. RAM and disk placement can make otherwise infeasible model residency possible but do not eliminate storage bandwidth, compute, power, latency or thermal limits. Current Cloudflare 70B is a REMOTE provider route, not proof that KRATOS/ATHENA already run an equivalent local model.
7. **AI is an opt-in client**. User-selected local Colibri, Cloudflare-hosted 70B, other hosted models, or deterministic programs can request typed machine capabilities. Prime authorizes and executes; models never become identity, root, approval, evidence or device ownership authorities. Ordinary non-AI use must work identically.
8. **Remote control and device work can compose with Prime.** Oracle provides a model-neutral authorized remote machine/transport donor; MIBU provides tangible ADB/Fastboot/device-state work; Prime Manager/Terminal can expose authorized local machine functions. Prime desktop may operate local USB/diagnostic device providers or remote specialist hosts. A powerful future Prime Mobile phone could be a portable USB-C/OTG host, repair workstation and/or remote client to ATHENA/KRATOS, subject to actual drivers/protocol/hardware proof.
9. **Browser hardware access is real.** The user reports personally flashing a Samsung phone using SamFW OdinWeb on 15 Nov 2026. SamFW uses browser-native Web Serial with supported desktop Chrome/Edge; other qualified workflows may use WebUSB. Therefore a native helper or Oracle is NOT universally required for website flashing. Remote technician supervision is a separate optional control-plane capability. The detailed TTG-UNLOCK customer-site handoff is linked below.
10. **On large-RAM phone hardware, a Prime Mobile host could optionally serve websites, APIs, smaller services, development workloads, and compatible local AI without KRATOS.** Both stationary and daily-carried phones are contemplated; the OS must enforce power/thermal/background/network/storage policies and battery essentials. KRATOS and ATHENA are specialist providers when needed, never mandatory for a self-sufficient local workload.
11. **Cross-chat replaceability** is an engineering value: recover exact project authority, evidence, failures and current frontier; reuse qualified findings rather than rerunning SIM automatically. The THETECHGUY cognition/engineering stack demonstrates this cooperation, but Prime neither embeds those tools nor owns their decision-making.
12. **This note saves ideas, not a build mandate.** Do not modify the Master Plan, implementation phase gates or Prime source based on this note alone. Recover exact heads and existing proof before assessing a candidate integration.

## 1. Existing Prime desktop identity and user intent

- Canonical repo: https://github.com/jaydumisuni/Prime-OS . Repo currently public.
- Prime's purpose is a lightweight, integration-first development/execution operating system that can support building, running, testing, signing, packaging and releasing for diverse targets where qualified.
- Prime uses the Linux kernel foundation but is not intended as a themed stock Ubuntu/Fedora/GNOME/KDE clone. It owns its system identity, Prime Shell and platform interfaces.
- The owner explicitly wants a minimal clean OS, similar in lean operating discipline to Linux or purpose-focused systems, with addable capabilities; not the idle/background accumulation typical of a bloated desktop/mobile installation.
- User-facing default desktop design includes Prime branding, glass/depth visual language, responsive shell, simple rail/home behavior, quick controls, and no redundant UI surfaces. Prior accepted naming replaced the old Orb label with the fixed **Prime** home entry. Current visual implementation and acceptance evidence, NOT this recap, determine exactly which elements shipped.
- Support many app formats and activities, but activation should be based on the operation and available hardware. Heavy software becomes possible only when hardware and proven backend can carry it; absence of support must be shown honestly.
- **Do not architect Prime around Wine/Winlator as a mandatory execution dependency.** Wine/ReactOS and similar code may be donor/reference material under the Master Plan; provider-neutral Prime Exec and Prime-owned compatibility/personality contracts remain the authority. No blanket claim of independently reimplementing all Windows/macOS/iOS APIs.
- A large variety of users must be served: an ordinary phone/desktop user, a web developer, software builder, game developer, repair technician, server operator, and a user who installs their own AI. THETECHGUY-specific demands are example workloads and compatibility fixtures, not definitions of every Prime installation.

## 2. Strict architecture and responsibility boundaries

### Prime-owned, generally available OS foundation

Hardware graph and Host identity; boot and kernel/generation identity; driver trust and binding; minimum system/console/Shell; Prime Core and Prime Exec; application admission and Application Profiles; capability discovery and health; storage inventory/truth and safe update/recovery; resource allocation; filesystem/network/device/secret policy; service lifecycle and event admission; power and thermal mechanisms; typed Host actions and machine evidence. These are generic and model-independent.

One Prime Host has self-local machine authority. A Prime Host ID is not an Origins Node ID or Ptah Node ID. No hidden global Prime host registry. Remote discovery, clustering and orchestration must not create a second host root of trust.

### Independently installable or externally provided

- Prime Manager: human deep system management client over approved Core capabilities.
- Prime Terminal: command/PTY client over Prime Exec and the same approved machine capability contracts.
- D-XQT: optional native/WASM deterministic program runtime.
- Colibri: optional local inference engine + separately selected compatible model weights.
- Other inference providers / toolchains / SDKs / compatibility personalities / VMs / containers / web servers / development languages / device-repair protocol adapters.
- Oracle, MIBU, Hunter, Ptah, Origins, AgentOps, CodeOps, Cookpit, Tenfold, Formula, SIM, Sergeant, Grid-Knight, Lumi and customer/business applications as appropriate separate products, optional providers or external services—not built into every Prime install.
- Remote non-Prime machines (Windows, macOS, specialist hardware or cloud) are **Providers**, never silently relabeled as Prime Hosts.

### Authority must remain where it belongs

Prime: machine truth, admission, workload policies, executable routing, typed authorization, effect evidence and local recovery.
Manager: presentation of resource/workload/service/process observations and operator-request UX; no direct privileged writes or fake system truth.
Terminal: session/PTY, deterministic dialect recognition and normalization, command UX; no private root backdoor or invented backend.
D-XQT: source language/compiler/VM/event evaluation/checkpoint semantics. **Application host** owns optional effects, permission/identity, persistence and idempotency.
Colibri/model: inference; not host action authorization.
Oracle: authenticated, model-neutral remote transport/allowed control capabilities where qualified, not a universal required middleman.
MIBU/device provider: device-specific protocol, mode/connection and actual verified operation within granted scope.
Origins: mission composition and workspace projections.
Ptah: neutral Workspace/Activity/Attempt/Provider/Grant/Lease/Receipt/evidence semantics and mechanical work environment.
Hunter/iAM/PETE: cognition, self-model and provider selection at the application/ecosystem level.
AgentOps: semantic operation lifecycle.
CodeOps/Tenfold: bounded coding/engineering execution.
Formula/SIM: mathematical and simulation/reasoning functions; SIM recommendations are not authority.
Cookpit: project supervision, not Prime Core scheduling.
Sergeant: independent engineering assurance, separate from ordinary mechanical Prime process truth.
Grid-Knight: cybersecurity threat interpretation/remediation, not Prime's process/storage mechanical truth.

No generic high-privilege 'execute anything' endpoint for AI, web content, UI process, terminal or remote clients. No forced THETECHGUY account, agent, cloud service, payment, model provider or server stack on all users.

## 3. Prime Exec — broad recognition, evidence-bound actual execution

Existing P1 format inspection recognizes ELF, PE32/PE32+, JVM class/JAR, APK/DEX, WASM, Mach-O and IPA where mechanically identifiable. It binds inspection to file bytes and architecture; runtime support is a different proof.
ExecutionBackend families in Prime's Rust contract: Native, Personality, Container, VM, RemoteProvider, SpecializedProvider. They are routing possibilities, not claims each works for every artifact.
Hardware architecture and workload architecture both matter: x86_64 vs x86, ARM/AArch64, RISC-V and foreign ABI/API compatibility. Host device capabilities, graphics, storage, OS service assumptions, drivers, network and legal/authorized distribution still affect compatibility.
Prime must visibly distinguish at least: format recognized; backend available/unavailable; route selected; admission accepted/denied; launch attempted; functional result observed; physically proven or pending; remote/VM fallback required; unsupported/unknown. Do not collapse 'recognized' into 'runs'. Do not conflate building for another OS with running or signing/publishing its applications.
The user wants eventual practical Android and iOS application support and useful Windows, Linux, macOS and cross-architecture workflows, but the approved Master Plan phases constrain what is local, experimental, remote-provider-only, inspection-only or unsupported. In particular, arbitrary IPA execution and Darwin API compatibility are evidence-dependent rather than guaranteed.
Cross-runtime activation must not start full Android services, Windows compatibility stacks, VMs or translation providers just because Prime inspects an APK/EXE. Load providers only on relevant requests; release resources when safe, respecting shared/system consumers.

## 4. Delivery / minimalism / store and on-demand behavior

The approved architecture already distinguishes image-owned Prime base generations from optional Prime Component packages and later Prime Store discovery/UI.
- P1's image-owned base is not live-mutated merely to prepare for a Store.
- P1.5 is about safe system-generation update/rollback and recovery proof, not general runtime package lifecycle.
- P2 specifies signed/versioned independent package manifests with publisher/digest, architecture/runtime dependencies, Application Profile and Workload Policy, offline/local install, transaction evidence, update/remove/rollback and persistent user-data ownership.
- P3 Store is a catalog/discover/install/update interface **over** the component authority; CLI/API/offline install remains possible. Kernel/core/boot/required driver changes go through generation updates, never ad hoc writes to image-owned /usr.
- 'First-class optional' Colibri/D-XQT means stable supported capability metadata and adapters once qualified; NOT default download/installation, no resident idle service, no preloaded model weights, no mandatory process or storage occupancy.
- 'Supported' ≠ 'installed' ≠ 'enabled' ≠ 'running' ≠ 'healthy' ≠ 'earned physical proof.' Deep GPU, foreign filesystem access, hardware probing and model loading should not wake unused devices or read large media without an authorized relevant request.
- Maintain clean normal-user and developer-user experiences without forking Prime into a THETECHGUY-only distribution.

## 5. Prime Manager donor — existing separate product

Repo: https://github.com/jaydumisuni/Prime-Manager ; authoritative ROADMAP.md. Published roadmap status at last inspection: saturated PRE-PM0 architecture, not completed Prime Manager implementation.
Manager is workload-first/process-deep: it must identify workloads, processes and services; owner/initiator/admission profile/policy; CPU/memory/GPU/storage/I/O/network/thermal/power; hardware/driver graph; supported/dormant/degraded/incompatible capabilities; exact Core/Provider epochs; and whether a resource value is measured, derived, shared, stale or unknown.
Potential permitted action UX: restart/suspend/resume/terminate an approved workload or service; adjust applicable priority/resource limits; request GPU reprobe where actual provider supports; inspect why work is blocked; review update/quiescence/recovery constraints.
Manager only requests typed machine actions through Core and approved Providers; no direct writes to /proc,/sys/cgroups/systemd, no silent sudo, no root-renderer app, no generic execution API. Opening Manager should not trigger expensive dormant GPU/VM/provider activation or diagnostic scans.
For AI or remote clients, the reusable boundary is Prime's typed machine observation/action/evidence interface, **not** a private AI-to-Manager UI hack. An AI could explain Manager facts if installed and authorized, but Manager truth never depends on a model.
Roadmap PM0–PM5 differentiates contract freeze and permissions from local operator first light, physical/adversarial proof, deep controls, intelligence, diagnostics and later remote/fleet. The PM5 remote Host interface remains future/design-only until authenticated Host capability authority exists.
Prime Manager's repo may have newer independent progress; recheck exact branch and evidence before claiming implementation.
Source: https://github.com/jaydumisuni/Prime-Manager/blob/main/ROADMAP.md

## 6. Prime Terminal donor — existing separate product

Repo: https://github.com/jaydumisuni/Prime-Terminal ; canonical ROADMAP.md and COOKPIT_ROADMAP_FINISH_HANDOFF.md.
Product rule: 'You choose the command language. Prime chooses the execution backend.' Common POSIX/Linux/macOS, Windows CMD and PowerShell command intent can normalize deterministically for supported overlap. Examples: ls / dir / Get-ChildItem; ps / tasklist / Get-Process; file read/copy/move/remove/open equivalents. Unknown flags or semantics must not be silently mistranslated, and recognized command text never grants privilege.
Terminal owns PTY, tabs/sessions, input/output, command parsing/dialect classification, canonical overlap operations and honest native-runtime/pass-through routes. Core owns real admission, backend capabilities, security and Workload Policy. Foreign platform commands require a real provider/personality/VM/remote service, otherwise fail with explicit missing-backend reason.
PT5 in the roadmap contemplates optional Hunter-assisted command suggestions, explanation, translation, repair suggestions, multi-step drafting and log interpretation. An AI may propose a concrete deterministic command/operation, but cannot secretly reinterpret the submitted command, bypass authorization or certify unearned success.
Terminal is not 'Bash with aliases', a universal PowerShell clone, an unrestricted root API or an AI chat window that directly executes arbitrary strings.
Current saved handoff describes pre-PT0 saturation work; archived v231 snapshot was UNFROZEN and transfer was for roadmap completion only. Do not infer PT0 implementation or physical acceptance without new proof.
Sources: https://github.com/jaydumisuni/Prime-Terminal/blob/main/ROADMAP.md and https://github.com/jaydumisuni/Prime-Terminal/blob/main/COOKPIT_ROADMAP_FINISH_HANDOFF.md

## 7. D-XQT — optional deterministic programming capability

Source: https://github.com/jaydumisuni/TTG-Event-Language ; experimental branch feat/dxqt-wasm-core-v3.
The source-reviewed v8 development baseline, merged into experimental development on 11 Nov 2026, exposes browser/language.mjs: createDxqtLanguage(WASM bytes), compile, run, runSource, runDxqtOnce. The underlying Rust VM supports native and WebAssembly and requires no mandatory cloud, background daemon or app-specific permission framework. It accepts caller-provided bounded events, input and explicit host effects. The source hash, imports, result/receipt and native/WASM parity are development evidence.
D-XQT v5 replay/effect contracts distinguish a program-bound idempotency key and a possible stable replay key; neither is authorization or a durable transaction. Embedding applications must persist trusted checkpoints, prove device/provider state, reconcile ambiguous effects and not blindly replay writes.
**Prime integration hypothesis:** a signed optional deterministic runtime package/Provider using Prime's existing capability, admission, resource and service model. It could be invoked for bounded background events, automations, device utility logic, game/application events, diagnostics, build triggers or approved host-specific tasks. It can run and exit with no AI and no resident worker. The application chooses which effects are exposed, and Prime independently enforces machine policy where effects touch Host resources.
Do not move account policy, payments, storage, remote transport, security decisions, Host mutation authority or domain-specific protocols into D-XQT language semantics. Do not replace Formula, SIM, Tenfold, Prime Exec or Prime service scheduler with D-XQT.
**Qualification boundary:** v8 is experimental development-proven by Rust/Node/Chrome/native/WASM and independent-reference checks; not production security certified, not untrusted-program acceptance, not Android/iOS package/USB proof, and not automatically included in Prime. Measure activation/idle memory/startup later, not assume free execution.
Sources:
- https://github.com/jaydumisuni/TTG-Event-Language/blob/feat/dxqt-wasm-core-v3/docs/V8_PORTABLE_EMBEDDING_SCOPE.md
- https://github.com/jaydumisuni/TTG-Event-Language/blob/feat/dxqt-wasm-core-v3/docs/V5_HOST_REPLAY_CONTRACT.md

## 8. Colibri — optional inference runtime, not operating-system intelligence

Source: https://github.com/jaydumisuni/colibri .
Colibri is a model inference ENGINE; actual inference requires compatible selected model weights. Its documented sparse MoE route keeps dense/shared portions resident, streams routed experts from storage and caches hot experts, with CPU/Vulkan/CUDA paths where supported. This can trade disk placement/cache/RAM for model feasibility but does not create more compute or memory bandwidth.
**Prime integration hypothesis:** independent signed optional local inference provider, discoverable through Prime capability metadata; users install it, choose/import/download compatible model(s), request model load, set workload policy and optionally expose a local application-approved inference interface. No giant weights, GPU wakeups or background model services on ordinary installations.
A developer may select their own engine/model or hosted provider. Do not require Colibri, Hunter, PETE, Cloudflare or a THETECHGUY login for AI applications; provider models are replaceable workers and never own Prime Core.
Cloudflare 70B route is presently a remote inference resource in the user's infrastructure. Do not write 'local 70B installed on KRATOS/ATHENA'. Suitable future mini PC or DGX-class hardware may change capacity, but is not a currently proven purchased/deployed dependency. Large RAM is necessary for some models, not sufficient for high token throughput; CPU/GPU, quantization, expert-cache hit rate, storage I/O, thermal envelope and power matter.
Suggested generic resource reporting for developers: supported model/backend, memory footprint or estimate with provenance, disk/weight size, warm/cold activation costs measured where available, actual GPU/CPU support, thermal/energy budget and remote/provider choices. Any resource estimator must disclose unknown/stale values, not invent exact performance.

## 9. One common pattern: deterministic first, AI only when requested

A user-authorized application receives an event/intent.
- If the task is already formally defined, native deterministic code or optional D-XQT may process it without a model.
- If the application genuinely needs flexible reasoning, it selects a user-approved local/remote model provider, not a system-wide mandatory model.
- The program/model proposes typed actions; Prime Capability Interface/Prime Exec/Workload Policy and device-specific providers decide capability availability, identity and authorization. High-impact effects may require a trusted approval surface.
- A provider executes and returns truthful observed evidence. Application/AgentOps/Cookpit/Sergeant may analyze or review according to their own independent contracts. The OS never treats a fluent model answer as physical proof.
- The caller owns its project/workflow reasoning, high-leverage task choice, semantic state and durable continuation; Prime supplies general mechanism and machine truth.

This mirrors the THETECHGUY idea of small sufficient model plus specialist capabilities, recovered state, and accepted progress, but it is NOT a mandate that all developers use Hunter, SIM or Formula.
Prime should not build a new OS-level planning model, supervisor swarm, generic AgentOps clone, duplicate global scheduler or second proof authority. The value lies in standards allowing optional consumer cooperation.

## 10. Model-neutral remote machine and device control

Existing donors:
- Oracle: https://github.com/jaydumisuni/Oracle- ; model-neutral Live transport and authorized remote machine/browser control. Its documented control contract treats GPT/Hunter/other/local models as clients, not execution authorities. Clients authenticate through transport; Oracle validates, applies policy, executes bounded admitted capabilities and normalizes results. The Android transport module has USB inventory, interface/session and serial/transaction code, but its physical OTG/Live proof is gated.
- MIBU: https://github.com/jaydumisuni/MIBU ; independent Xiaomi PC helper + Android APK, real ADB/Fastboot discovery, version/app proof, bounded selected devices, Wi-Fi/mobile-data actions and after-action state verification, including optional remote ADB server configuration. Official Xiaomi outcomes stay authoritative; MIBU does not prove arbitrary phone unlocking.
- Prime Manager/Terminal are local clients of Prime Core's machine/exec/action contracts; an external AI/remote application should use typed authorized capabilities, not screen-scrape a Manager panel or inherit full root shell.
- Oracle may connect Prime to ATHENA and KRATOS for execution of specialist workloads. A remote provider is not inherently a Prime Host. Host identity, exact target/machine, connected status, request authorization and result evidence must be verified, including stale/disconnected states.

Illustrative request: 'Inspect why this computer is slow and fix what you safely can'. The application/model could ask for workload/process/resource and service observations, propose a constrained service restart or resource action, obtain applicable approval, execute through Prime's typed capability and verify the postcondition. This is a TARGET workflow; advanced Manager/Terminal/remote adapters are not currently qualified as one deployed integration.
Device tooling analog: read model/USB VID/PID/mode, list installed apps, check network state, conditionally install app and verify version, or service an authorized connected device. Device-specific write/flash/erase/bootloader paths require separate higher-risk provider policy, technical authorization and explicit identity/firmware/approval, never a generic model or browser prompt granting all access.

## 11. TTG-UNLOCK / SamFW browser correction — vital cross-device example

TTG-UNLOCK client site repo https://github.com/jaydumisuni/TTG-UNLOCK ; its Device Check branch ttg-device-check-v1 / PR #1 is an unmerged prototype with real local Python USB/ADB/Fastboot/Apple readers, confidence/flash-risk/report code, responsive mock/sample-data UI and incomplete online report/remote technician/service backend. See the complete 15 Nov 2026 handoff:
https://docs.google.com/document/d/1iD1RYlgqclPHySmADfjX5jaaMANxCWD1mraXZZ4BV5Q/edit?usp=drivesdk

Critical user correction: on 15 Nov the owner states they actually flashed a Samsung device using only https://samfw.com/Odinweb . SamFW OdinWeb's page exposes Web Serial-based browser-native firmware flashing using desktop Chrome/Edge and Samsung Download Mode. Separately WebUSB can support direct browser USB protocols in compatible circumstances (e.g. PyOdin-WebUSB as a donor). Previous statement that *every* website repair requires a native local device bridge was WRONG.
Keep THREE routes distinct:
A. Customer browser directly uses qualified Web Serial / WebUSB / WebHID hardware protocol with local explicit device permission. No required native helper/Oracle when compatible.
B. Customer browser owns the USB endpoint while a remote technician supervises authorized case/session tasks. Needs authenticated case/session/control and typed approvals, but not necessarily USB passthrough or local helper.
C. Technician uses Prime Desktop or future Prime Mobile, directly connecting repair device via USB or optionally using a remote specialist host such as ATHENA. An advanced remote forwarder is a separate protocol/session feature to qualify.
Actual phone flashing involves exact manufacturer/model/firmware/slot/binary/CSC and device-state checks, sufficient power, approved repair procedure, signed/hashed firmware where appropriate, operator/customer consent, and reconnection/replay safety. Neither D-XQT nor Prime Core implements Samsung Odin protocol simply by existing.
TTG-UNLOCK is a **business application** and a useful optional Prime-compatible workload, NOT an OS default product. SamFW is a hardware-API/protocol capability demonstration, not permission to copy proprietary code or claim universal device support. Never advertise 'fix any phone' as proven until exact families and physical modes are individually qualified.

## 12. Portable workstation and Prime OS Mobile sibling — user vision

The owner intends eventually to walk into town/store carrying a sufficiently powerful mobile phone, connect another device through Type-C/OTG and perform supported service locally; alternatively remote into ATHENA or KRATOS for Windows/Linux specialist operations. The phone itself might serve as development console, field diagnostic host, website/API host or an optional AI computer, subject to hardware fit.
Prospective FIRST experimental Mobile hardware discussed: Samsung Galaxy A14 4G SM-A145P, MediaTek Helio G80, Mali-G52. NOT a Snapdragon/Turnip GPU target. This does not prove bootloader access, Linux/Prime boot, USB Host + charge coexistence, modem/telephony, DRM, GPU, serial driver or hardware flashing support. Mobile is proposed future work and must earn its own hardware/device acceptance; desktop main does not prove mobile.
Proposed shared Core philosophy: one generic Prime machine/capability/exec/policy contract family, architecture-specific host boot/device drivers/services, UI scaling from desktop to mobile, independent installable packages and optional model providers. Do not assert identical binaries, kernel builds, all desktop capabilities on phone, or all phone telephony running outside needed resident services.
Prospective operational profiles (discussion candidates, not implemented policy):
- Mobile: telephony, interactive UI, battery integrity, notifications and essential phone services prioritized; prohibit surprise background drains.
- Docked Host: explicitly enabled long-running web/API/edge services, network ingress and persistence with charging/thermal constraints.
- Compute: developer builds or optional inference within negotiated CPU/GPU/RAM/storage/power/thermal budget.
Not three mandatory always-running OS modes and not automatic server hosting merely because the hardware has large RAM.
On a high-RAM stationary or carried phone, a small web/API/database/service host may replace KRATOS for compatible work. It still needs stable power/charging, storage durability, process supervision, overheating/battery management, restart/recovery, permissions, availability and networking. A public website requires reachable ingress (public IPv4/IPv6, supported NAT/port mapping, secure outbound tunnel, reverse proxy or hosted endpoint as appropriate), not mandatory Cloudflare. Ordinary consumer mobile sleep/Doze assumptions cannot be treated as durable server policy without a Prime-specific background-service design.
Large inference: RAM capacity does not equal speed. Memory bandwidth, cache, GPU/CPU, disk and thermal/power effects matter. Lightweight routing and deterministic tools improve *useful work per compute unit*, not the physical floor of model weights/processing.
Heavy tasks may run on ATHENA/KRATOS or another authorized provider, but independence is a design goal: remote machine use is optional if local capability is sufficient. Prime Mobile could also act as an Oracle-type remote client, without Oracle becoming Prime's host authority.
All Type-C assertions must be scoped: port shape alone does not promise USB 3, DisplayPort, host mode, power delivery, simultaneous charging, proprietary device service modes or custom drivers. Powered USB-C hub or specialist adapter may be needed. No user was asked to purchase specific hardware in this discussion.

## 13. Generic developer experiences — no THETECHGUY lock-in

Example A — ordinary desktop user: Prime boots cleanly and runs native apps without installing Colibri, D-XQT, Oracle, Hunter, VM images or device toolchains. Basic Shell/Settings/services/updates remain useful.
Example B — web developer: installs a web server, runtime and optional DB, starts only approved listeners and background services; may inspect with Manager and use Terminal; no AI required.
Example C — game/application developer: installs SDKs/graphics provider; may use D-XQT for deterministic events and optional Colibri with a model for AI characters, or use entirely different choices.
Example D — repair technician: installs a device-protocol Provider, uses USB locally; optionally remote to authorized specialist tooling; asks a model to interpret evidence only if chosen.
Example E — independent AI developer: installs their own model runtime or hosted API adapter; discovers hardware/energy constraints; uses machine/terminal capabilities only with explicit permissions. Prime's app APIs do not privilege THETECHGUY brands.
Example F — software builder: Build target can precede local Run target; signing/release Provider remains independent. Prime Exec selects only qualified local/remote backend and reports unsupported truth.
Prime should be valuable in EVERY scenario without the THETECHGUY ecosystem installed.

## 14. Resource-awareness and continuation proposals — inspect for gaps first

Proposed useful developer-facing extensions, if existing Prime contracts do not already cover them:
- A capability cost/availability observation with measured or estimated CPU, RAM, GPU, disk bandwidth, activation latency, power/thermal constraints, service background permission and version/provenance. The app/OS policy chooses eligibility; the app/model chooses preference. Don't misrepresent estimates as guaranteed speed or let a cheap read wake costly dormant hardware.
- On-demand runtime activation/deactivation with explicit shared-consumer ownership so disabling an optional runtime doesn't break another service or essential telephony.
- App-controlled resumable work with durable application state, restart-safe receipt/correlation, error/failure/unknown result classes, idempotency and explicit reauthorization on changed device identity. Prime handles machine lifecycle and safe enforcement; the app owns semantic continuation, not a new Prime-global engineering-operation scheduler.
- Model-agnostic capability requests from approved local UI, scripts, D-XQT host effects and optional remote clients through SAME typed Prime authority. No AI-exclusive API bypass.
- Optional phone/field connection projection: USB and device protocol capability metadata, physical mode, assigned provider, supported local/remote route, active lease and disconnect/reconnect event. Ensure user ownership/consent and minimal identifier disclosure.
- Service-host profile as a policy/configuration consumer: permitted listeners, persistent storage, background admission, power and network constraints; not a new always-on AI daemon.
Many components already exist as architecture/partial implementation: Workload Policy v1 CPU/memory/gpu/storage/process/network/device/secret/background/evidence; Capability Interface v1; Prime Exec; component manifest/Store design; service control in Manager plan. Therefore start with **delta review** instead of creating duplicates.
Anything not already in Master Plan that changes Prime-owned authority needs its own proposal, source comparison, owner decision, independent review and freeze. This recap is not an implicit contract amendment.

## 15. Hardware/implementation truth at the time of this save

**Inspection 2026-11-15:** Prime-OS main HEAD before this recovery documentation: 5c410644ad6fdadac19b5dc9d78a1951803e8b69 (commit message 'Record Prime OS apps menu proof and sync', 24 Sep 2026). Next commits generated by this *documentation-only* save should not be mistaken for product implementation.
The authoritative current status file is docs/PRIME_CURRENT_STATUS.md (last text update 22 Sep 2026), followed by exact branch/proof records. Main carries integrated P1 source, evidence and recovery; P4A Windows is branch-isolated.
P1 evidence: canonical local P1 proof PASS at integrated source 086ee97f4bef57f78b725af9b220e98faf9f87a1; UEFI/OVMF QCOW2 boot, normal/recovery UKI, Composefs, persisted Host/generation identity and mechanical SHELL_READY. Earlier KRATOS physical Intel UHD630 DRM/Wayland and glass UI owner acceptance, S3 suspend/resume, Wi-Fi, ALSA stream, USB2 path and software display revalidation proof exist at exact lineage. **P1 integrated SHA KNOWN_GOOD, integrated physical acceptance and phase closeout were not earned in the status record.** Physical USB3, live Ethernet, audible playback and actual physical display hotplug remained separate proof gates. Later shell/apps-menu source does not retroactively satisfy those.
P4A Windows W1 simple Win32 frozen 4fe487f; W2 installers/common runtimes frozen 4296dd6; W3 managed .NET frozen 8af61f4; W4 DirectX/GPU frozen/host-certified f99c7f8; W5 COM frozen 706e90a; W6 services frozen/host-certified 0428413; W7 USB mediation frozen 8115b4c; W8 VM fallback head c2cc9de in HEALTHY_WAITING on authorized_guest; W9 real TTG workload certification not started. W8 host-side QEMU/KVM/fencing proof does NOT mean an authorized Windows guest has run; guest-dependent physical/VM proof unearned. Do not infer W8 finished or Windows compatibility delivered.
Manager: separate repo roadmap PRE-PM0. Terminal: separate roadmap/pre-PT0 unfrozen snapshot and Cookpit handoff; do not describe as shipped. D-XQT v8: experimental review/CI/Chrome proof, no Prime package or mobile production qualification. Colibri: source/runtime reference, no Prime Store install proof or broad model-hardware qualification. Oracle Android USB: source boundary, actual OTG/Live physical proof gated. Prime Mobile: proposal, not implemented in desktop Prime main.
No new VM, model run, hardware test, local machine probing or simulation was conducted to create this recovery note.

## 16. Cross-chat method and resumption — exact order

1. Read Prime repository README.md, docs/PRIME_OS_MASTER_PLAN.md and accepted supplements (PRIME_STORAGE_INTELLIGENCE.md, PRIME_APPLE_FILESYSTEMS.md, PRIME_HOST_SECURITY_INTERFACE.md).
2. Read docs/PRIME_CURRENT_STATUS.md, docs/PRIME_OS_ROADMAP.md, docs/AI_HANDOFF.md, planning/state.json, then THIS discussion recovery note.
3. Recheck current main, active P1 and P4A branch heads, exact merged/unmerged status, reviews, local dirty worktrees, generated images/artifacts and KRATOS physical evidence. Historical SHAs here are a dated snapshot, not timeless state.
4. For Manager/Terminal/D-XQT/Colibri/Oracle/MIBU/TTG-UNLOCK, read each owning project's current ROADMAP, README, qualified source/PR and machine proof. Don't infer integration because both repos contain similar architecture.
5. Apply Understand → Build → Review → Freeze → Prove → Submit/Ship. Recover prior evidence before reasoning. Distinguish NEW / UPDATE / COMPLETION / CORRECTION / VERIFICATION / NO CHANGE per session, not across all conversations.
6. Before recommending any new change, compare TTG-ecosystem, OS-playbook, TTG-progress, TTG-decisions, TTG-deploy-lab, Google Recovery and Prime/relevant donor repos. If already documented, report 'Already documented — No update required.' If an actual gap is discovered, report 'Update existing documentation.' Do not open competing duplicate planners or simulation tools.
7. This discussion says NO immediate new SIM; previous valid frozen simulation/evidence can be reused if applicable. Simulation is its own tool; do not relabel Ptah/Oracle/Cookpit as SIM. High-leverage engineering planning belongs at the project/application level, never implicitly in Prime Core.
8. Avoid changing unrelated desktop P1 visual/proof scope, P4A Windows guest gate, accepted immutable-base architecture, Prime Host self-authority or AgentOps/Ptah semantics merely to add a potential model/device provider.
9. Future actions require the owner's next concrete instruction and matching project authority; no software implementation, store package, branch merge, public deployment, protocol write, model download or device flash is authorized by saving this note.
10. Make any accepted change recoverable with exact SHA, evidence, correction record, owner decision and a pointer from the relevant repo. One chat should be replaceable by another.

## 17. Source index — authoritative and related

Prime:
- https://github.com/jaydumisuni/Prime-OS
- https://github.com/jaydumisuni/Prime-OS/blob/main/docs/PRIME_OS_MASTER_PLAN.md
- https://github.com/jaydumisuni/Prime-OS/blob/main/docs/PRIME_OS_ROADMAP.md
- https://github.com/jaydumisuni/Prime-OS/blob/main/docs/PRIME_CURRENT_STATUS.md
- https://github.com/jaydumisuni/Prime-OS/blob/main/docs/AI_HANDOFF.md
- https://github.com/jaydumisuni/Prime-OS/blob/main/docs/contracts/PRIME_EXEC_V1.md
- https://github.com/jaydumisuni/Prime-OS/blob/main/docs/contracts/PRIME_CAPABILITY_INTERFACE_V1.md
- https://github.com/jaydumisuni/Prime-OS/blob/main/docs/contracts/PRIME_WORKLOAD_POLICY_V1.md
- https://github.com/jaydumisuni/Prime-OS/blob/main/planning/state.json

Prime apps:
- https://github.com/jaydumisuni/Prime-Manager/blob/main/ROADMAP.md
- https://github.com/jaydumisuni/Prime-Terminal/blob/main/ROADMAP.md
- https://github.com/jaydumisuni/Prime-Terminal/blob/main/COOKPIT_ROADMAP_FINISH_HANDOFF.md

Optional runtime donors:
- https://github.com/jaydumisuni/TTG-Event-Language
- https://github.com/jaydumisuni/TTG-Event-Language/blob/feat/dxqt-wasm-core-v3/docs/V8_PORTABLE_EMBEDDING_SCOPE.md
- https://github.com/jaydumisuni/TTG-Event-Language/blob/feat/dxqt-wasm-core-v3/docs/V5_HOST_REPLAY_CONTRACT.md
- https://github.com/jaydumisuni/colibri

Remote control / device / public site:
- https://github.com/jaydumisuni/Oracle-
- https://github.com/jaydumisuni/Oracle-/blob/main/docs/MODEL_AGNOSTIC_CONTROL.md
- https://github.com/jaydumisuni/Oracle-/blob/main/docs/ORACLE_LIVE_PROTOCOL_V1.md
- https://github.com/jaydumisuni/MIBU
- https://github.com/jaydumisuni/TTG-UNLOCK
- https://github.com/jaydumisuni/TTG-UNLOCK/pull/1
- https://samfw.com/Odinweb
- https://github.com/pyodinweb/pyodinweb
- Detailed customer website handoff: https://docs.google.com/document/d/1iD1RYlgqclPHySmADfjX5jaaMANxCWD1mraXZZ4BV5Q/edit?usp=drivesdk

Ecosystem authority:
- https://github.com/jaydumisuni/TTG-ecosystem
- https://github.com/jaydumisuni/OS-playbook
- https://github.com/jaydumisuni/TTG-progress
- https://github.com/jaydumisuni/TTG-decisions
- https://github.com/jaydumisuni/TTG-deploy-lab
- Earlier broad strategy handoff: https://docs.google.com/document/d/1om_Er4YMpBnzTAy9mmwJ8uWMrvAGsgFQmwjH_vV2C_g/edit?usp=drivesdk

## 18. Contribution record and final nonclaims

**Contribution type: NEW recovery note; UPDATE existing recovery entry points only when linked.**
This session consolidated existing Prime desktop architecture and the recent generic capability-native, model-optional, D-XQT/Colibri and Mobile/remote-device design discussion. It is NOT a product implementation, plan freeze, SIM conclusion, permission grant, route-to-production, launch success or release acceptance.
**Already documented — No update required:** minimal Prime Core/Exec/Capability/Workload Policy, Host self-authority, Store/component split, broad app-recognition limitation, service boundaries, Manager and Terminal independent ownership; those existing contracts remain authoritative.
**Update existing documentation:** this recovery/reference note and durable pointers. Potential new optional adapters, capability cost reporting or Mobile/USB forwarding must be examined as deltas and then separately approved before altering frozen Prime contracts.
Preserve the user's critical intent: Prime must be useful to every developer and everyday user, not just THETECHGUY; optional AI should help those who choose it; unused heavyweight capability must not become bloatware; a portable phone workstation may handle local services and repairs while ATHENA/KRATOS remain optional specialists; browser-native flashing is real and should not be mischaracterized; D-XQT is a proposed deterministic execution improvement across the product family.
**No source/implementation changes, hardware mutations, downloads, flashes, deployments, simulations, merges, or freeze promotions are authorized or performed by this handoff.**
