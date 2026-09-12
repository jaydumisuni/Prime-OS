# Prime Windows Personality W5 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Prove Wine COM as a first-class Prime Windows capability on x64 and WoW64 while preserving application-prefix isolation and fail-closed recovery.

**Architecture:** Reuse the frozen W4 Wine provider and sealed runtime. Add only proof/capability checks that bind the COM donor closure and runtime behavior; do not add a parallel COM implementation or alter unrelated launch routing.

**Tech Stack:** Rust, Wine 11.0 Fedora 44, VBScript/WSH smoke fixtures, MinGW-built COM fixtures for deeper activation tests, Podman proof images, Python provider checks.

**Spec:** `docs/superpowers/specs/2026-09-13-windows-personality-w5-design.md`

## Global Constraints

- Frozen predecessor is `f99c7f8245e3324ed699ad388a55aa40f71c7dc7`.
- x86 uses the WoW64 SysWOW64 lane inside an application-scoped prefix.
- Full physical Prime boot remains deferred.
- No W5 promotion without exact-SHA runtime replay, Sergeant approval, Formula PASS and remote SHA binding.

---

### Task 1: Bind W5 authority and donor closure

**Files:** create/update W5 donor/spec/benchmark and fixture files only.

- [ ] Verify W4 frozen SHA is ancestor of W5.
- [ ] Verify sealed proof image identity and required COM/OLE binaries in both architectures.
- [ ] Run x64 and SysWOW64 `Scripting.Dictionary` automation smoke fixture.
- [ ] Record benchmark gates 1-3 from exact evidence.
- [ ] Run diff integrity and commit.

### Task 2: Prove provider-routed in-process activation

**Files:** add W5 proof fixtures and provider-check script; modify production code only if a RED fixture proves a missing Prime contract.

- [ ] Add a failing provider-routed x64 COM activation proof.
- [ ] Run it and confirm the failure is due only to missing W5 proof/contract surface.
- [ ] Add the minimum implementation needed, or record no-code donor sufficiency if the existing provider already passes.
- [ ] Repeat for SysWOW64 x86.
- [ ] Promote gates 4-5 only from exact evidence.

### Task 3: Registration isolation and recovery

- [ ] Add two application IDs with separate prefixes and a hash-bound custom in-proc COM fixture.
- [ ] Prove registration in app A is unavailable in app B.
- [ ] Corrupt/stale the registration and prove deterministic repair or fail-closed recovery.
- [ ] Promote gate 6.

### Task 4: Local-server COM and RPCSS

- [ ] Build hash-bound x64 local-server COM server/client fixtures.
- [ ] Prove activation, request/response and clean RPCSS drain.
- [ ] Repeat through WoW64 x86.
- [ ] Promote gates 7-8.

### Task 5: Lifecycle/adversarial and freeze

- [ ] Run cold/warm repeated activation, killed-server recovery, stale registration, cross-app isolation and zero leaked Wine-process checks.
- [ ] Promote gate 9.
- [ ] Run source-clean, diff, fmt, clippy, workspace and W1-W5 regression checks.
- [ ] Freeze exact SHA, run Sergeant, exact frozen runtime replay and Formula.
- [ ] Push and verify remote SHA.
- [ ] Close W5 only when all predicates bind to the same SHA.
