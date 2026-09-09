# Prime Windows Personality W1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Run selected portable/simple x86/x86_64 Windows PE applications on x86_64 Prime through a provider-neutral `PERSONALITY/WINDOWS` backend with Prime policy, display isolation and bound evidence.

**Architecture:** Prime Exec remains the binary/runtime classifier. A new Windows Personality module validates an image-owned provider manifest, revalidates the exact staged PE artifact, compiles the selected Workload Policy, and launches a Prime-owned provider adapter through `systemd-run` without a shell. The first adapter can use Wine underneath, but only the adapter knows that donor; Shell, profiles, Exec and evidence use Prime `PERSONALITY/WINDOWS` contracts.

**Tech Stack:** Rust workspace, serde/serde_json, systemd transient services, systemd-sysusers, bootc/Fedora 44 image, Wayland compositor, provider adapter process.

**Spec:** `docs/superpowers/specs/2026-09-10-windows-personality-w1-design.md`

## Global Constraints

- Follow `ttg.tenfold.v1` and the `Understand -> Build -> Review -> Freeze -> Prove -> Ship` cycle.
- W1 supports PE32/PE32+ x86/x86_64 only on x86_64 Prime.
- Prime-facing APIs must not require a donor name.
- No `$PATH` runtime discovery and no shell command construction.
- Every launch binds exact profile revision, policy revision and artifact SHA-256.
- Missing/incompatible provider fails explicitly.
- Core socket authorization must remain separate from display authorization.
- W2+ features remain out of scope.

---

### Task 1: Freeze Windows Personality contracts

**Files:**
- Modify: `crates/prime-contracts/src/exec.rs`
- Modify: `crates/prime-contracts/src/lib.rs`
- Test: contract unit tests in `crates/prime-contracts/src/exec.rs`

**Interfaces:**
- Produces `WINDOWS_PROVIDER_MANIFEST_SCHEMA`, `WINDOWS_LAUNCH_EVIDENCE_SCHEMA`.
- Produces `WindowsProviderManifest`, `PersonalityLaunchOutcome`, `WindowsLaunchEvidence`, and an untagged `LaunchEvidence::{Native,Windows}` wrapper so existing native response JSON remains unchanged.

- [ ] **Step 1: Write failing serialization tests** proving a provider manifest and Windows launch evidence serialize with `PERSONALITY`, `WINDOWS`, provider identity and exact artifact/profile/policy fields.
- [ ] **Step 2: Run** `cargo test -p prime-contracts windows_ -- --nocapture` and confirm failure because types/constants do not exist.
- [ ] **Step 3: Add minimal typed contracts** with exact fields from the W1 spec. `WindowsProviderManifest` must carry `schema`, `provider_id`, `provider_revision`, `adapter_path`, `formats`, `workload_arches`, and `limitations`. `WindowsLaunchEvidence` must carry host/generation/application/profile/policy/artifact/provider/unit/timestamps/outcome/exit/enforcement data. Add `#[serde(untagged)] LaunchEvidence` with `Native(NativeLaunchEvidence)` and `Windows(WindowsLaunchEvidence)` variants so native wire shape does not regress.
- [ ] **Step 4: Re-run contract tests** and keep all prime-contracts tests green.
- [ ] **Step 5: Commit** `feat(windows): add W1 personality contracts`.

### Task 2: Provider registry and deterministic selection

**Files:**
- Create: `crates/primed/src/windows_personality.rs`
- Modify: `crates/primed/src/lib.rs`
- Test: module tests in `windows_personality.rs`

**Interfaces:**
- Produces `load_provider(dir: &Path, format: &ArtifactFormat, arch: &str) -> Result<ValidatedWindowsProvider, WindowsPersonalityError>`.
- `ValidatedWindowsProvider` exposes manifest fields plus canonical absolute adapter path.

- [ ] **Step 1: Write failing tests** for missing provider, relative adapter rejection, non-executable adapter rejection, unsupported architecture rejection and deterministic compatible-provider selection.
- [ ] **Step 2: Run** `cargo test -p primed windows_personality::tests -- --nocapture` and confirm RED.
- [ ] **Step 3: Implement manifest loading** from the configured trusted directory. Accept only regular JSON files with the exact schema, non-empty provider ID/revision, absolute adapter paths, regular executable adapters and declared PE format/architecture compatibility. Sort candidates deterministically by provider ID/revision before selection.
- [ ] **Step 4: Re-run module and workspace tests.**
- [ ] **Step 5: Commit** `feat(windows): add provider-neutral personality registry`.

### Task 3: Windows profile admission and launch preparation

**Files:**
- Modify: `crates/primed/src/windows_personality.rs`
- Reuse: `crates/primed/src/registry.rs`, `crates/primed/src/exec.rs`, `crates/primed/src/policy.rs`
- Test: `windows_personality.rs`

**Interfaces:**
- Produces `PreparedWindowsLaunch`.
- Produces `prepare_windows_launch(state_dir, provider_dir, request/profile, host_arch)`.

- [ ] **Step 1: Write failing tests** proving native profiles are rejected, PE profile/runtime/backend mismatches are rejected, x86/x86_64 are admitted on x86_64 only, and staged artifact identity is revalidated.
- [ ] **Step 2: Run targeted tests and confirm RED.**
- [ ] **Step 3: Implement minimal preparation** by loading the selected profile/policy, requiring `execution_backend=PERSONALITY`, `runtime_family=WINDOWS`, `format=PE32|PE32+`, supported architecture, empty W1 dependencies/permissions, compatible mechanical state, content-addressed staging and fresh Exec inspection. Reuse `compile_native` only as the current systemd enforcement compiler; rename/generalize types only where required to avoid lying that policy is native-specific.
- [ ] **Step 4: Re-run targeted and full tests.**
- [ ] **Step 5: Commit** `feat(windows): admit exact PE profiles under Prime policy`.

### Task 4: Provider launch argv and evidence

**Files:**
- Modify: `crates/primed/src/windows_personality.rs`
- Modify if needed: `crates/primed/src/policy.rs`
- Test: `windows_personality.rs`

**Interfaces:**
- Produces `windows_systemd_run_args(&PreparedWindowsLaunch) -> Vec<String>`.
- Produces `launch_windows(...) -> Result<WindowsLaunchEvidence, WindowsPersonalityError>`.

- [ ] **Step 1: Write failing tests** proving argv contains the absolute provider adapter path plus Prime-defined positional/flag inputs, contains no shell (`sh`, `bash`, `-c`), includes policy properties, and records provider identity/backend/runtime in evidence.
- [ ] **Step 2: Confirm RED.**
- [ ] **Step 3: Implement launch** through `/usr/sbin/systemd-run --system --service-type=exec --wait --collect` with the compiled enforcement properties. Allocate a systemd-managed per-launch runtime directory and pass it to the adapter. Persist admitted/completed evidence under the launch evidence tree.
- [ ] **Step 4: Confirm GREEN plus native launcher regression tests.**
- [ ] **Step 5: Commit** `feat(windows): launch personality providers with bound evidence`.

### Task 5: Shell dispatch and capability truth

**Files:**
- Modify: `crates/primed/src/shell_api.rs`
- Modify: `crates/primed/src/lib.rs`
- Modify: `crates/primed/src/main.rs`
- Test: `shell_api.rs` / `lib.rs` tests

**Interfaces:**
- `CoreState` gains trusted Windows provider directory configuration.
- `launch_selected` dispatches `NATIVE` to the native launcher and `PERSONALITY/WINDOWS` to the Windows launcher.
- Adds capability `prime.exec.windows-personality` whose availability reflects validated provider presence.

- [ ] **Step 1: Write failing tests** for projection readiness when provider exists/does not exist, native-vs-Windows dispatch, and truthful unavailable state.
- [ ] **Step 2: Confirm RED.**
- [ ] **Step 3: Implement dispatch/capability logic** without changing the Shell launch request shape. Successful response may serialize backend-specific evidence because the current Shell client ignores the body.
- [ ] **Step 4: Run Shell/Core tests and workspace regression.**
- [ ] **Step 5: Commit** `feat(windows): route Shell launches through Windows Personality`.

### Task 6: Separate display authority from Core authority

**Files:**
- Modify: `image/sysusers/prime-shell.conf`
- Modify: `image/systemd/prime-compositor.service`
- Modify: `image/systemd/prime-shell.service`
- Modify: `image/Containerfile` copy/install assertions as required
- Test: static service assertions plus image/proof checks

**Interfaces:**
- Adds group `prime-display`.
- Compositor runtime directory group becomes `prime-display` with mode permitting admitted clients.
- Shell keeps primary `prime-shell` and gains supplementary `prime-display`.
- Windows workloads gain only `prime-display`, never `prime-shell`.

- [ ] **Step 1: Add failing static tests/checks** that assert the display group exists, compositor uses it, Shell retains Core-authorized primary group, and Windows systemd argv does not add `prime-shell`.
- [ ] **Step 2: Confirm RED.**
- [ ] **Step 3: Apply the minimal sysusers/service changes.**
- [ ] **Step 4: Verify service/security assertions and workspace tests.**
- [ ] **Step 5: Commit** `security(windows): separate display and Core authorization`.

### Task 7: First Prime-owned provider adapter and image integration

**Files:**
- Create: `crates/primed/src/bin/prime-windows-provider-wine.rs`
- Create: `image/windows-providers/wine-v1.json`
- Modify: `crates/primed/Cargo.toml` only if a bin declaration is needed
- Modify: `image/Containerfile`
- Add: donor decision record under `docs/donors/`

**Interfaces:**
- Adapter CLI: `--artifact <absolute-path> --application-id <uuid> --launch-id <uuid> --runtime-dir <absolute-path>`.
- Adapter owns donor-specific environment and invokes the donor with argv, never a shell.

- [ ] **Step 1: Write failing adapter argument-validation tests** and image manifest validation tests.
- [ ] **Step 2: Confirm RED.**
- [ ] **Step 3: Implement the adapter** so it validates absolute regular PE artifact/runtime paths, creates donor-specific runtime state inside the supplied runtime dir, locates only the fixed packaged donor binary path, and `exec`s/spawns it with the PE artifact as argv. No Prime-facing contract exposes the donor name.
- [ ] **Step 4: Add the Fedora 44 donor package(s)** needed for the W1 x86/x86_64 proof and copy the trusted provider manifest into the image. Pin/verify package versions in the image build rather than runtime-installing them.
- [ ] **Step 5: Re-run tests and image static checks.**
- [ ] **Step 6: Commit** `feat(windows): package first W1 provider adapter`.

### Task 8: Real PE proof on KRATOS and freeze

**Files:**
- Add proof scripts/evidence under the repository's established proof location only after recovering existing convention.
- Update W1 design/roadmap status only with earned evidence.

**Interfaces:**
- Frozen candidate SHA.
- Real PE fixture identity and result.

- [ ] **Step 1: Acquire/build a tiny known PE fixture** whose behavior is deterministic (for example exits with a known status and optionally writes a bounded marker inside its allowed runtime dir). Record SHA-256 and architecture.
- [ ] **Step 2: Install/use the donor on KRATOS only as a proof dependency**, then run the PE through the Prime Windows Personality adapter/path rather than directly through the donor.
- [ ] **Step 3: Negative controls:** remove/point away the provider manifest and prove explicit unavailable; use unsupported PE architecture and prove denial; prove Windows workload lacks Core socket group authority.
- [ ] **Step 4: Freeze candidate** with `git rev-parse HEAD`, `git status --short`, and evidence paths.
- [ ] **Step 5: Run** `cargo test --workspace`, `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, and `git diff --check`.
- [ ] **Step 6: Perform independent/adversarial review** against all 20 W1 proof obligations; correct anything found, re-freeze and re-prove.
- [ ] **Step 7: Push the exact proven branch** and report what is proven versus what remains (especially GUI window attachment if not yet mechanically proven).
