# Prime Mobile MOB-0 / PHI — KRATOS-independent implementation checkpoint

**Date:** 2026-11-15. **Status:** isolated **development candidate**, not a frozen Prime Core change, approved mobile port, product release or physical phone proof. **No firmware, partitions, bootloader, recovery or USB device changed by this work.**

## Contribution and owning authority

- The accepted direction remains **one Prime-OS repository and shared Core**, with Mobile's independent MOB-0–MOB-9 roadmap and PHI (Prime Hardware Intelligence) as a shared Desktop/Mobile proposal.
- Frozen Core truth remains in `crates/prime-hardware`, `primed`, `docs/contracts/PRIME_HARDWARE_GRAPH_V1.md`, `PRIME_CAPABILITY_INTERFACE_V1.md`, `PRIME_WORKLOAD_POLICY_V1.md`. This checkpoint deliberately **does not introduce a second Core daemon or change P1 proof-host gates**.
- iAM/COG is the **donor for separating actual, reported, simulated, stale and conflicting observations**; these Python tools do not embed iAM's full cognition runtime or claim iAM certification.
- The existing **TTG-Simulation draft PR #1** contains 19 D-XQT/Rust-WASM shadow scenarios. This checkpoint complements—not replaces—the D-XQT VM or Virtual Hardware Lab's ARM64 QEMU work.
- Contribution: **NEW isolated prototype** (PHI read-only Linux memory/pressure discovery, ARM64 Device Tree identification, MOB-0 evidence intake, formal candidate schema and negative tests). Code is NOT installed into Prime's boot/runtime image.

## Source files

- `tools/phi_readonly_probe.py`: generic read-only Linux source probe for `/proc/meminfo`, `/proc/pressure/memory`, `/proc/swaps`, zRAM `mm_stat` and zswap enabled state. Records source availability, missing/invalid facts, compressed RAM, physical RAM and swap separately. Never counts swap as physical RAM, interprets zswap enabled as usage, imports Colibri or discloses raw swap filenames. Root parameter supports virtual fixture replay.
- `mobile/tools/board_readonly_probe.py`: ARM64-friendly read-only source probe for `/proc/device-tree/{model,compatible}` and `/sys/firmware/devicetree/base/*` fallback, plus kernel release. It does not read device serials, MAC/IMEI, `/proc/cmdline` or secrets; Device Tree names are kernel projections, **not** proof of functioning radio, touch, GPU, phone recovery or lock state.
- `mobile/tools/mob0_preflight.py`: evidence intake and diagnostic classification, never a flash permission calculator. Preserves the distinction between `LIVE_READ`, `FILE_HASHED`, `PHOTO`, `OWNER_REPORTED`, `SIMULATED`, `THIRD_PARTY`; `UNKNOWN`, `CONFLICTED`, `UNSUPPORTED` and `OBSERVED`; subject binding, timestamps and provenance reference. Enforces provisional candidate freshness rules (24h for volatile live observations, 30 days for offline design evidence), not frozen OS policy.
- `mobile/contracts/device-evidence.candidate.v0.schema.json`: JSON Schema for a candidate evidence manifest. The Python intake remains dependency-free; schema validators are optional test tooling, not core runtime requirements.
- `mobile/device-ports/samsung/a14m/evidence-candidate.json`: reference record for the discussion, NOT the current physically proven handset identity. First candidate in the roadmap: Samsung Galaxy A14 4G `SM-A145P` / `a14m` / MediaTek Helio G80. The owner's photo shows TWRP Advanced and a TWRP UI lock. The owner says the bootloader is unlocked. **Neither the specific handset model nor actual bootloader state was electronically verified in this session.** Therefore the record uses an unverified local alias and reports the owner statement without promoting it to live evidence.
- `tests/test_phi_readonly_probe.py`, `mobile/tests/test_board_readonly_probe.py`, `mobile/tests/test_mob0_preflight.py`: synthetic positive/negative checks for parsing, missing sources, invalid counters, privacy, symlink-root escape, Device Tree NUL parsing, subject mismatch, stale/future evidence, untrusted owner/photo/simulated observations and never-return-flash-authority.

## Local reproducible commands (no KRATOS, QEMU or third-party packages required)

```bash
python3 -m unittest discover -s tests -v
python3 -m unittest discover -s mobile/tests -v
python3 tools/phi_readonly_probe.py --root / > phi-current-host-only.json
python3 mobile/tools/board_readonly_probe.py --root / > board-current-host-only.json
python3 mobile/tools/mob0_preflight.py mobile/device-ports/samsung/a14m/evidence-candidate.json
```

The exact development workspace execution reported **22/22 passed**: six PHI memory tests, five ARM64 board-probe tests and eleven MOB-0 evidence tests. A separate optional `jsonschema` 2020-12 validation of the checked-in example against the candidate schema passed. The probe observed only the temporary **ChatGPT workspace Linux host**, not the user's KRATOS, ATHENA or phone; memory fields and limitations from that container are not device-specific Prime performance evidence.

## Evidence/state semantics

1. **Kernel observation:** hardware probes read explicit Linux paths without permissions or side effects and return unknown/unavailable on missing sources. They do not produce a Prime Host ID, qualify a device, decide a model route or infer physical driver support.
2. **Metadata intake:** a manifest may cite owner reports, photographs, simulator results or live device reads. It must preserve source and subject instead of flattening everything into a true/false claim. Even `LIVE_READ` claims are **caller-supplied and unverified by this tool**.
3. **Preflight outcome:** `READ_ONLY_CANDIDATE_NEVER_FLASH_AUTHORITY`. Even every gate labelled candidate still sets `device_write_authorized:false`, `physical_acceptance:false`, `machine_verified_device_identity:false`. Human/OS approval and separate cryptographic/device proof cannot be imported by naming a JSON key.
4. **Versioning:** the prototypes use `candidate.v0` schema labels, **not** the frozen `prime.hardware-graph.v1` or `prime.capability.v1` contract. The exact approved extension must be reviewed before integrating into Rust Core.
5. **Simulation boundary:** D-XQT may process evidence and output deterministic findings. It does not independently acquire hardware readings, authorize effects or emulate ARM64 without an ISA/board model. TTG-Simulation and TTG-Virtual-Hardware-Lab retain their separate review/evidence authorities.

## Checks still open

- Exact **MOB-0 board identity, kernel config, boot and vendor partition layout, bootloader state, stock recovery package and safe return path** from an authorized live lab handset.
- ARM64 Prime Core cross-build with actual Rust toolchain and exact crate revision, genuine ARM64 QEMU guest (or external capable emulator) boot/serial, and physical device boot; no claim that these occurred.
- USB-C/OTG, power delivery, display/touch/DRM, Mali GPU compute, modem/telephony/audio, encryption/updates and battery/thermal acceptance, per exact board variant.
- Prime Rust Core integration only after source review of the memory/board fields and a versioned additive PHI contract that reuses Hardware Graph/Workload Policy; independent reviewer needed before merging this candidate branch.
- Thermal and memory tuning must be determined from physical workloads and kernel capabilities, not from this container or a generic phone datasheet.

## Cross-chat pickup

Resume in order: `README.md` → Prime Master Plan → Prime Current Status → `docs/PRIME_MOBILE_ROADMAP_2026-11-15.md` → `docs/PRIME_HARDWARE_INTELLIGENCE_PROPOSAL_2026-11-15.md` → **this checkpoint** → TTG-Simulation PR #1. Recover exact branch heads and actual connected host status before using currentness claims. Preserve unrelated Prime Desktop P1 and Windows W8 gates. Do not run a new SIM, merge or flash merely because these tools exist; an independent engineering review and accepted proof are separate phases.