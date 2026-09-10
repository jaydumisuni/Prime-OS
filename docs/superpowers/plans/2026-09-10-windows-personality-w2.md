# Prime Windows Personality W2 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add Prime-owned Windows installer/common-runtime lifecycle so dependency-bearing Windows profiles can prepare immutable prerequisite state and then launch through the proven W1 path.

**Architecture:** Keep the W1 provider launch path stable. Add typed component contracts plus a Core-side resolver/registry and a separate Prime Windows Component Engine that owns install/verify/recovery transactions against per-application compatibility state. Dependency references remain strings in Application Profile v1 but use one strict versioned grammar and exact digest pinning.

**Tech Stack:** Rust, serde/serde_json, sha2, uuid, systemd-run policy model, Fedora Wine 11 donor closure, MinGW-generated proof fixtures, Python static proof scripts.

**Spec:** `docs/superpowers/specs/2026-09-10-windows-personality-w2-design.md`

## Global Constraints

- Baseline is W1 commit `4fe487ffaa843e04745dafb262b0f90bab2a4733`.
- Prime remains donor-neutral; normal users never invoke Wine/winetricks/Bottles or choose donor backends.
- Dependency grammar is `windows-component:<component-id>@<revision>#sha256:<64-lowercase-hex>`.
- `component-id` matches `[a-z0-9][a-z0-9._-]*`, 1-128 chars; revision is positive base-10; digest is exactly 64 lowercase hex chars.
- Installer kinds are `MSI`, `EXE`, `BUILTIN`.
- Verification probes are observational only: `FILE_EXISTS`, `FILE_SHA256`, `REGISTRY_VALUE_EQUALS`.
- No shell invocation, no user-controlled installer flags, no untrusted relative installer paths.
- Same-app prefix mutation serializes under one exclusive compatibility-state lock; different apps remain independent.
- A successful installer exit without passing verification probes is not success.
- W2 does not claim .NET managed-app execution, GPU acceleration, COM completeness, services, USB passthrough, or VM fallback.
- Rootful sealed-image/boot proof is deferred; all host-provable gates must be completed on KRATOS first.

---

### Task 1: Windows Component Contracts and Strict Dependency Reference

**Files:**
- Create: `crates/prime-contracts/src/windows_component.rs`
- Modify: `crates/prime-contracts/src/lib.rs`
- Test: inline unit tests in `windows_component.rs`

**Interfaces:**
- Produces `WindowsComponentReference::parse(&str) -> Result<Self, WindowsComponentReferenceError>`.
- Produces `WindowsComponentManifest`, `WindowsComponentKind`, `WindowsInstallerKind`, `WindowsVerificationProbe`, `WindowsComponentOutcome`, and `WindowsComponentEvidence`.
- Later tasks consume these exact public types.

- [ ] **Step 1: Write RED parser/serde tests** covering one valid reference and malformed ID, zero revision, uppercase digest, short digest, duplicate separators, and round-trip display.

```rust
#[test]
fn component_reference_round_trips_exact_pin() {
    let raw = "windows-component:msvc.2015-2022.x64@1#sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    let parsed = WindowsComponentReference::parse(raw).unwrap();
    assert_eq!(parsed.component_id, "msvc.2015-2022.x64");
    assert_eq!(parsed.revision, 1);
    assert_eq!(parsed.digest, "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa");
    assert_eq!(parsed.to_string(), raw);
}
```

- [ ] **Step 2: Run** `cargo test -p prime-contracts windows_component -- --nocapture` and require RED because the module/types do not exist.
- [ ] **Step 3: Implement the strict parser and typed serde contracts**. `WindowsVerificationProbe` is a tagged enum with only:

```rust
#[serde(tag = "kind", rename_all = "SCREAMING_SNAKE_CASE")]
pub enum WindowsVerificationProbe {
    FileExists { path: String },
    FileSha256 { path: String, sha256: String },
    RegistryValueEquals { hive: String, key: String, name: String, value: String },
}
```

`WindowsComponentManifest` contains schema/id/revision/digest/display_name/kind/workload_arches/depends_on/installer_kind/artifact_identity/artifact_path/installer_args/accepted_exit_codes/restart_compatibility_environment/verification/limitations.
- [ ] **Step 4: Run targeted contracts plus full `prime-contracts` tests** and require GREEN.
- [ ] **Step 5: Commit** `feat(windows): add W2 component contracts`.

### Task 2: Immutable Component Registry and Dependency Graph Resolver

**Files:**
- Create: `crates/primed/src/windows_components.rs`
- Modify: `crates/primed/src/lib.rs`
- Test: `crates/primed/tests/windows_components.rs`

**Interfaces:**
- Produces `seal_component`, `verify_component`, `load_component_revision`, `resolve_component_plan`.
- Produces `ResolvedWindowsComponentPlan { ordered: Vec<WindowsComponentManifest> }`.

- [ ] **Step 1: Write RED tests** for manifest digest sealing/verification, exact reference digest binding, duplicate profile refs, missing component, architecture rejection, deterministic topological order, and cycle rejection.
- [ ] **Step 2: Run** `cargo test -p primed --test windows_components -- --nocapture` and require RED due missing module/API.
- [ ] **Step 3: Implement canonical component digesting** by cloning the manifest, clearing `digest`, serializing with serde_json, and storing `sha256:<hex>` exactly as the existing policy/profile registry does.
- [ ] **Step 4: Implement trusted registry loading** from a separate trusted `component_dir/<component-id>/revisions/<20-digit-revision>.json`; production `component_dir` is `/usr/lib/prime/windows-components`; reject path identity/revision/digest mismatches.
- [ ] **Step 5: Implement DFS topological resolution** using exact references from `ApplicationProfile.dependencies`, rejecting duplicate roots and cycles with explicit W2 errors.
- [ ] **Step 6: Run targeted tests and full `cargo test -p primed`**.
- [ ] **Step 7: Commit** `feat(windows): resolve immutable W2 component plans`.

### Task 3: Shared Windows Compatibility-State Lock and Paths

**Files:**
- Create: `crates/primed/src/windows_state.rs`
- Modify: `crates/primed/src/lib.rs`
- Modify: `crates/primed/src/bin/prime-windows-provider-wine.rs`
- Test: inline `windows_state.rs` tests and existing provider tests.

**Interfaces:**
- Produces `application_state_directory_name(Uuid) -> String`.
- Produces `application_state_root(Uuid) -> PathBuf`.
- Produces `ensure_private_directory(&Path)` and `acquire_compatibility_lock(&Path)`.
- W1 provider consumes these functions so W2 and W1 share one mutation lock.

- [ ] **Step 1: Write RED tests** proving stable state path, existing directory reuse, symlink rejection, and two same-process lock attempts serialize/fail according to the chosen blocking semantics.
- [ ] **Step 2: Run targeted tests and require RED**.
- [ ] **Step 3: Extract the current W1 path/lock logic without changing provider behavior.** Lock path becomes `locks/compatibility.lock`; old `.prime-w1-init.lock` is migrated by simply ceasing to use it—no deletion of existing state.
- [ ] **Step 4: Update W1 adapter to acquire the shared lock before prefix initialization and hold it only for mutation; preserve donor marker semantics and direct exec path.
- [ ] **Step 5: Run provider unit tests, W1 integration tests, and full `cargo test -p primed`**.
- [ ] **Step 6: Commit** `refactor(windows): share compatibility state locking`.

### Task 4: Component Satisfaction and Observational Verification Engine

**Files:**
- Create: `crates/primed/src/windows_component_engine.rs`
- Modify: `crates/primed/src/lib.rs`
- Test: `crates/primed/tests/windows_component_engine.rs`

**Interfaces:**
- Produces `verify_component_state(prefix: &Path, manifest: &WindowsComponentManifest) -> Result<Vec<WindowsProbeResult>, WindowsComponentEngineError>`.
- Produces `component_marker_path`, `marker_matches`, and atomic marker write.

- [ ] **Step 1: Write RED tests** for `FILE_EXISTS`, `FILE_SHA256`, safe relative path enforcement, traversal rejection (`..`, absolute, symlink escape), marker digest mismatch, and probe failure despite a present marker.
- [ ] **Step 2: Run targeted test and require RED**.
- [ ] **Step 3: Implement prefix-safe path resolution** by rejecting absolute paths/components containing parent/root/prefix components, joining to canonical prefix, and requiring the canonical result stay beneath canonical prefix.
- [ ] **Step 4: Implement file probes and SHA-256 probes**. Registry probe may initially return a typed `ProbeUnsupported` error until Task 5 provides donor registry-query execution; it must never silently pass.
- [ ] **Step 5: Implement atomic `components/<id>/installed.json` marker** carrying component revision/digest, donor fingerprint, timestamp and verification summary.
- [ ] **Step 6: Run targeted/full regression**.
- [ ] **Step 7: Commit** `feat(windows): verify W2 component state`.

### Task 5: Prime Windows Component Engine for BUILTIN, MSI and EXE

**Files:**
- Create: `crates/primed/src/bin/prime-windows-component-engine.rs`
- Modify: `crates/primed/Cargo.toml`
- Modify: `crates/primed/src/windows_component_engine.rs`
- Test: inline binary tests plus `crates/primed/tests/windows_component_engine.rs`

**Interfaces:**
- CLI ABI: `--manifest <absolute-json> --application-id <uuid> --transaction-id <uuid> --runtime-dir <absolute /run/prime-win-component-...>`.
- Exit `0` only for verified `INSTALLED` or `ALREADY_SATISFIED`.

- [ ] **Step 1: Write RED ABI/direct-argv tests** proving exact flags, duplicate/unknown rejection, manifest path regular-file/no-symlink checks, and no shell tokens.
- [ ] **Step 2: Write RED installer-spec tests**:
  - BUILTIN runs probes only.
  - MSI command is `/usr/bin/wine msiexec /i <artifact> <fixed args>`.
  - EXE command is `/usr/bin/wine <artifact> <fixed args>`.
  - Accepted exit codes come only from manifest.
- [ ] **Step 3: Implement donor environment reuse** with the existing application prefix and composite donor fingerprint; validate `/usr/bin/wine` and `/usr/sbin/wineserver` exactly.
- [ ] **Step 4: Implement registry verification** using fixed direct argv `wine reg query <key> /v <name>` and exact output parsing; no shell. Map allowed hives to fixed abbreviations only.
- [ ] **Step 5: Execute installer, wait `wineserver -w`, rerun all probes, then write marker**. Installer success + failed probe returns `WINDOWS_COMPONENT_VERIFY_FAILED`.
- [ ] **Step 6: Run targeted/full tests**.
- [ ] **Step 7: Commit** `feat(windows): add W2 component engine`.

### Task 6: Core Preparation, Evidence and W1 Launch Gating

**Files:**
- Modify: `crates/primed/src/windows_personality.rs`
- Modify: `crates/primed/src/shell_api.rs`
- Modify: `crates/prime-contracts/src/windows_component.rs`
- Test: `crates/primed/tests/windows_personality.rs`, `crates/primed/tests/windows_components.rs`

**Interfaces:**
- Produces `prepare_windows_dependencies(...) -> Result<WindowsDependencyPreparation, WindowsPersonalityError>`.
- Existing `launch_windows(...)` calls dependency preparation before generating W1 launch argv.

- [ ] **Step 1: Write RED integration tests** proving zero-dependency W1 profiles remain launchable unchanged, dependency-bearing profiles no longer fail `W1 dependency admission is not implemented`, missing components fail before adapter launch, and successful dependency preparation permits W1 launch.
- [ ] **Step 2: Add append-only W2 evidence phases** under `evidence/windows-components/<transaction-id>/` with admitted/completed JSON.
- [ ] **Step 3: Add systemd-run construction for the component engine** using the same selected Workload Policy enforcement plus `StateDirectory=prime-win-app-<id>`, runtime directory, and `prime-display` only when a recipe/probe actually requires GUI; default installer preparation is headless.
- [ ] **Step 4: Update Shell readiness** so valid unresolved dependencies become `INSTALLABLE` rather than generic unavailable, while missing/invalid dependencies remain not launch-ready with exact limitations.
- [ ] **Step 5: Run Core/Shell/W1 regression**.
- [ ] **Step 6: Commit** `feat(windows): gate launches on W2 dependency preparation`.

### Task 7: Controlled MSI/EXE Fixtures and Recovery/Idempotence Proof

**Files:**
- Create: `tools/windows-w2-fixtures/README.md`
- Create: `image/scripts/check-windows-w2-security.py`
- Create proof fixtures under cockpit `/home/kratos/.oracle-work/prime-w2-cockpit/fixtures/` (not committed binaries unless licensing/source permits).
- Test through the component engine on KRATOS.

**Interfaces:**
- Proof fixtures create deterministic files/registry values consumed by W2 probes.

- [ ] **Step 1: Build a tiny EXE installer fixture** with MinGW that writes `C:\PrimeW2\exe-installed.txt` and exits 0.
- [ ] **Step 2: Build a tiny MSI fixture** from a deterministic open-source MSI toolchain available in a proof container; MSI installs `C:\PrimeW2\msi-installed.txt`.
- [ ] **Step 3: Create exact component manifests** with fixture SHA-256 identities and observational probes.
- [ ] **Step 4: Prove cold EXE install, cold MSI install, second-run `ALREADY_SATISFIED`, and installer exit-0/probe-fail negative control.**
- [ ] **Step 5: Corrupt/remove final marker while leaving valid installed state; prove recovery re-verifies and reseals without reinstall.**
- [ ] **Step 6: Interrupt an installation before marker commit; prove next transaction converges to verified state.**
- [ ] **Step 7: Commit scripts/manifests/evidence schema** `test(windows): prove W2 installer lifecycle`.

### Task 8: Concurrency and Isolation Proof

**Files:**
- Modify tests/scripts created in Tasks 3/7.
- Evidence: cockpit `prime-w2-cockpit/evidence/`.

- [ ] **Step 1: Start two same-application component transactions concurrently** and require one mutation section at a time; capture timestamps proving non-overlap.
- [ ] **Step 2: Start two different-application transactions concurrently** and require their mutation windows overlap, proving no global serialization.
- [ ] **Step 3: Require no cross-prefix file/marker changes** by hashing both state trees before/after.
- [ ] **Step 4: Commit** `test(windows): prove W2 state concurrency isolation`.

### Task 9: Real Common-Runtime Component Proof

**Files:**
- Create trusted runtime manifest under `image/windows-components/` for one common prerequisite already present in or safely installable into Fedora Wine.
- Modify `image/Containerfile` only if the component artifact is image-owned and licensing permits redistribution.
- Modify `image/scripts/check-windows-w2-provider.py`.

- [ ] **Step 1: Choose the smallest real runtime component whose legal/source/package status permits reproducible proof**; prefer a BUILTIN donor capability if it exercises registry/file verification without adding a download dependency.
- [ ] **Step 2: Pin exact component revision/digest and verification probes.**
- [ ] **Step 3: Prove a dependency-bearing profile resolves and verifies this real component in a fresh application state.**
- [ ] **Step 4: Do not claim managed application execution; record that as W3.**
- [ ] **Step 5: Commit** `feat(windows): add first W2 common-runtime component`.

### Task 10: W2 Freeze, Adversarial Review and Host Completion

**Files:**
- Create: `image/evidence/windows-w2-benchmark-20260910.json`
- Create/update W2 cockpit verifier and Sergeant changed-file scope.
- Update: `docs/PRIME_OS_ROADMAP.md` only with earned local status.

- [ ] **Step 1: Run W2 security checker** requiring no shell invocation, no `prime-shell`, exact artifact identities, safe prefix paths and fixed manifest arguments.
- [ ] **Step 2: Run negative controls** for malformed ref, missing component, wrong digest, cycle, unsupported architecture, unavailable provider, denied policy and failed probe.
- [ ] **Step 3: Run `cargo fmt --all -- --check`, `cargo clippy --locked --workspace --all-targets -- -D warnings`, `cargo test --locked --workspace`, and `git diff --check <W1-base>..HEAD`.**
- [ ] **Step 4: Run Sergeant against every W2 changed file and correct every required finding.**
- [ ] **Step 5: Freeze candidate commit, rerun all deterministic host proofs against that exact SHA, and write Formula-style PASS markers.**
- [ ] **Step 6: Push the exact frozen W2 branch and verify `git ls-remote` returns the frozen SHA.**
- [ ] **Step 7: Record only the rootful sealed-image/boot integration proof as deferred.**
