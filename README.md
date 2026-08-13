# Prime OS

Prime OS is a **lightweight, integration-first development and execution operating system** designed so a developer can stay in one environment while building, running, testing, signing, packaging, and releasing software for multiple target platforms.

Prime uses the Linux kernel foundation but is not intended to be a themed Fedora/Ubuntu derivative. Prime owns the machine layer: hardware discovery, drivers, Prime Core, Prime Exec, workload policy, application profiles, capability exposure, updates, rollback, recovery, storage intelligence, host-security mechanisms, and the Prime Shell.

## Current project state

**Planning baseline:** documented and owner-approved for handoff.  
**Implementation:** not performed in the planning workstream.  
**Next implementation mission:** **P1 — First Light** in a fresh implementation workstream after recovering the repository authority.

Do not begin implementation from assumptions or from conversation memory. Read the durable authority first.

## Recovery order

1. [`docs/PRIME_OS_MASTER_PLAN.md`](docs/PRIME_OS_MASTER_PLAN.md) — canonical product, architecture, contracts, boundaries, and full roadmap.
2. [`docs/PRIME_STORAGE_INTELLIGENCE.md`](docs/PRIME_STORAGE_INTELLIGENCE.md) — accepted storage-intelligence supplement: WinDirStat donor boundary, generic Linux/VFS scanner, ext4/Btrfs/XFS/NTFS strategy, change engine, cleanup safety, and phase placement.
3. [`docs/PRIME_APPLE_FILESYSTEMS.md`](docs/PRIME_APPLE_FILESYSTEMS.md) — Apple storage supplement: APFS, HFS+/HFS, FileVault/encryption boundary, snapshots/clones/space sharing, Time Machine awareness, and Apple disk-image strategy.
4. [`docs/PRIME_HOST_SECURITY_INTERFACE.md`](docs/PRIME_HOST_SECURITY_INTERFACE.md) — host-security seam: Prime owns machine security mechanisms/mechanical events/enforcement; Grid-Knight owns threat interpretation, protection policy, cleanup/remediation and retest evidence.
5. [`docs/PRIME_OS_ROADMAP.md`](docs/PRIME_OS_ROADMAP.md) — operational implementation sequence.
6. [`docs/AI_HANDOFF.md`](docs/AI_HANDOFF.md) — concise recovery instructions for a new chat/AI/engineer.
7. [`planning/state.json`](planning/state.json) — machine-readable current state and resume order.

Donor-specific evidence is kept under [`docs/donors/`](docs/donors/), including [`WINDIRSTAT.md`](docs/donors/WINDIRSTAT.md) and [`APFS.md`](docs/donors/APFS.md).

If a summary conflicts with the Master Plan, the Master Plan wins unless a later explicitly accepted supplement/amendment supersedes the narrower topic. The accepted supplements are part of current P0 authority and must be recovered before implementation in their areas.

## Permanent role boundaries

```text
Prime OS      = body / operating system / machine authority
Prime Host    = self-local machine identity and capability authority
Origins       = mission workspace and capability composition
Hunter        = intelligence and reasoning
AgentOps      = semantic operation lifecycle
Ptah          = neutral mechanical Workspace/execution substrate
CodeOps       = engineering
Sergeant      = independent engineering review
X-Ray         = specialist evidence
Oracle        = authorized browser/OS control
Lumi          = downloads/transfers
Grid-Knight   = cybersecurity threat interpretation, response, cleanup/remediation and retest evidence
```

Prime does **not** implement Ptah, absorb Origins, replace Grid-Knight, or turn Sergeant into a runtime certification daemon.

## Core architecture rules

- **Support broadly; activate narrowly.**
- `build ≠ execute ≠ sign ≠ publish`.
- Prime Application compatibility state is Prime mechanical truth; Sergeant reviews engineering changes separately.
- All Prime execution backends use the same Prime Workload Policy.
- `Prime Host ID ≠ Origins Node ID ≠ Ptah Node ID`.
- Prime Host authority is self-only; Prime does not maintain a global Host registry.
- Moving Prime storage to materially different hardware requires Host rediscovery/re-enrollment.
- Prime Storage Intelligence reports logical/allocated/shared/exclusive storage truth without false precision.
- Storage scanning uses a generic Linux/VFS path plus filesystem-specific enrichment; WinDirStat is a donor/reference oracle, not Prime Core code.
- Prime host-security events are mechanical truth; Prime does not label unusual storage/process/network activity as malware by itself.
- Grid-Knight may consume Prime security events and request authorized enforcement, but cannot bypass Prime Workload Policy or machine authority.
- APFS is a first-class foreign filesystem target with read-only-first policy; experimental Linux APFS write support is not ordinary Prime functionality until a dedicated corruption/recovery proof campaign earns it.
- HFS+/HFS and Apple disk-image formats are explicit Apple-storage compatibility classes, not hidden under generic adapters.
- No silent architecture redesign during implementation.

## Phase order

```text
P0   Complete the Load / concrete authority
P1   First Light
P1.5 Survival
P2   Development Body
P3   Origins Factory
P4A  Windows Personality
P4B  Android Personality
P5   Cross-Architecture Execution
P6   Darwin Local Compatibility
P7   iOS Local Compatibility
P8   Distributed Prime Deployment
```

Windows and Android personalities may proceed in parallel only after their prerequisites are proven and when doing so does not create conflicting implementation ownership.

## First implementation mission

A fresh implementation workstream should begin with:

> **Build P1 — First Light according to the Prime authority.**

First Light includes the bootable Prime identity, Prime Host identity, Prime Core, hardware graph, storage separation, base HP 290 G4 hardware support, Prime Exec foundation, Application Profile registry, Capability Interface v1, Workload Policy v1, Prime Shell, Prime Orb baseline, recovery entry, update-aware generation layout, the minimal Prime Storage Intelligence foundation required for capacity/reserve/update preflight, and the minimal secure host-event/enforcement foundation Prime itself requires.

For Apple storage, P1 should safely recognize APFS/HFS+/HFS and report locked/read-only/unsupported states honestly. P1 does **not** require APFS write support.

Full Grid-Knight integration is **not** a P1 blocker. Grid-Knight remains a later cybersecurity Provider over Prime's versioned host-security seam.

It does **not** begin with Ptah, complete Windows/Android compatibility, the full WinDirStat-equivalent storage analyzer, Darwin/iOS local execution, distributed execution, or full cybersecurity automation.

## Architecture drift rule

If implementation discovers a genuine architectural contradiction:

```text
STOP
→ preserve evidence
→ return to planning authority
→ review amendment
→ freeze correction
→ resume implementation
```

Do not silently patch architectural decisions into implementation code.
