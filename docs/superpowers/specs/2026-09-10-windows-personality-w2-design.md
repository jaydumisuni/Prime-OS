# Prime Windows Personality W2 Design

**Date:** 2026-09-10
**Scope:** P4A / W2 — installers and common runtimes
**Baseline:** W1 remote/frozen commit `4fe487ffaa843e04745dafb262b0f90bab2a4733`

## 1. Purpose

W2 makes Windows installer and prerequisite state a first-class Prime lifecycle instead of an opaque side effect of launching a PE file. Prime must be able to resolve declared Windows dependencies, install or verify them inside one application's persistent compatibility state, recover from interrupted installation, and record bound evidence before the normal Windows launch path begins.

W2 does not expose Wine, winetricks, Bottles, MSI command lines, prefix management, or donor selection to normal users. Donor mechanisms remain beneath Prime-owned contracts.

## 2. Architectural decision

W2 does **not** turn the W1 launch adapter into a general package manager. The proven W1 launch path remains launch-only. W2 adds a separate Prime-owned Windows Component Engine.

```text
Application Profile
  -> Prime dependency references
  -> Core resolves immutable Windows Component records
  -> Windows Component Engine
       -> prepare/lock application compatibility state
       -> install or verify component
       -> wait for donor quiescence
       -> run deterministic verification probes
       -> write component-state marker
  -> Core records installation evidence
  -> W1 Windows launch adapter
  -> application
```

This separation keeps W1 regression risk small and gives W3-W6 one reusable prerequisite mechanism.

## 3. W2 scope

W2 supports:

- immutable dependency/component records;
- exact dependency references from `ApplicationProfile.dependencies`;
- dependency graph resolution with cycle rejection;
- architecture admission for `x86`, `x86_64`, and architecture-neutral components;
- installer kinds `MSI`, `EXE`, and `BUILTIN`;
- application-scoped installation into the existing persistent Windows compatibility state;
- trusted installer artifacts with exact SHA-256 identity;
- fixed manifest-owned installer arguments and accepted exit codes;
- deterministic verification probes;
- idempotent already-satisfied checks;
- interrupted/stale installation recovery;
- installation evidence bound to host, generation, application, profile, component revision/digest, provider identity and resulting state;
- explicit install-only transactions for trusted MSI/EXE recipes, even when automatic creation of a post-install launch profile is not yet available.

W2 does **not** claim:

- arbitrary unattended switches for unknown third-party installers;
- automatic discovery of an arbitrary installer's final executable;
- .NET application compatibility beyond component lifecycle plumbing (W3);
- DirectX/DXVK/VKD3D acceleration (W4);
- COM completeness (W5);
- Windows service lifecycle (W6);
- USB/device passthrough (W7);
- VM fallback (W8).

## 4. Dependency reference contract

`ApplicationProfile.dependencies` remains `Vec<String>` for schema-v1 compatibility. W2 defines one canonical grammar rather than changing the P0 profile schema:

```text
windows-component:<component-id>@<revision>#sha256:<64-lowercase-hex>
```

Example:

```text
windows-component:msvc.2015-2022.x64@1#sha256:0123...abcd
```

The grammar is strict: `component-id` is 1-128 lowercase ASCII characters matching `[a-z0-9][a-z0-9._-]*`; `revision` is a positive base-10 integer with no sign; the digest is exactly `sha256:` followed by 64 lowercase hexadecimal characters. A Windows profile may contain only dependency references understood by its selected runtime personality. Malformed, duplicate, unpinned, wrong-digest, wrong-architecture, missing, or cyclic references fail closed before any installer is executed.

The profile digest already covers the dependency strings, so the exact prerequisite set remains revision-pinned.

## 5. Windows Component Manifest v1

Prime owns a root/image-owned component registry. Each immutable record contains at least:

- `schema = prime.windows-component.v1`;
- `component_id`;
- `revision`;
- `digest`;
- `display_name`;
- `kind = RUNTIME | APPLICATION_INSTALLER | SUPPORT`;
- `workload_arches`;
- `depends_on` as exact component references;
- `installer_kind = MSI | EXE | BUILTIN`;
- `artifact_identity` and trusted absolute artifact path for MSI/EXE;
- fixed `installer_args`;
- accepted installer exit codes;
- whether an accepted installer exit requests restart of the Windows compatibility environment; W2 may recycle the donor server/prefix session but must never reboot the Prime Host as an installer side effect;
- deterministic verification probes;
- limitations.

Component records are content-digested using the same canonical JSON principle as policies/profiles. Revisions are immutable. The selected application profile pins exact component revisions and digests.

## 6. Verification probes

W2 permits only bounded, typed probes. Initial probe types are:

- `FILE_EXISTS` — path relative to the application prefix;
- `FILE_SHA256` — path plus expected SHA-256;
- `REGISTRY_VALUE_EQUALS` — fixed hive/key/value/value-data;

W2 deliberately excludes arbitrary executable verification probes. Verification must be observational and non-mutating. No manifest may embed a shell command. Paths may not escape the application state. Probe failure means the component is not satisfied even if the installer returned success.

## 7. Installation lifecycle

For each dependency in topological order:

1. Core loads and validates the exact component record.
2. Core verifies host/workload architecture compatibility and component dependency closure.
3. Core records an `ADMITTED` installation-evidence phase.
4. The Component Engine takes the same per-application exclusive compatibility-state lock used for prefix initialization.
5. It validates the complete donor runtime fingerprint.
6. It runs verification probes first. If all pass and the component state marker matches the exact component digest + donor fingerprint, installation is skipped as already satisfied.
7. Otherwise it runs the typed installer operation using direct argv and the application's compatibility prefix.
8. It waits for the donor server to quiesce.
9. It reruns every verification probe.
10. Only after all probes pass does it atomically write the component-state marker.
11. Core records `INSTALLED`, `ALREADY_SATISFIED`, or a precise failure outcome.
12. Normal W1 application launch is admitted only after every dependency is satisfied.

An interrupted transaction has no valid final marker. The next attempt re-verifies actual state; if probes already pass it can seal the state without unnecessary reinstall, otherwise it retries installation according to the immutable recipe.

## 8. Installer execution

### MSI

MSI recipes invoke the packaged donor directly through:

```text
/usr/bin/wine msiexec /i <trusted-msi> <manifest-owned-args>
```

The manifest may request quiet/no-restart semantics only through explicit typed fields; user input cannot append arguments.

### EXE

EXE installer recipes execute the trusted installer PE directly with manifest-owned arguments. Prime does not guess `/S`, `/quiet`, `/silent`, or vendor-specific switches. Unknown installers can be recognized as installable but remain `INSTALL_RECIPE_REQUIRED` until an approved recipe exists.

### BUILTIN

BUILTIN components represent compatibility-runtime capabilities already delivered by the selected donor closure. They perform version/fingerprint/probe verification without downloading or executing a third-party installer.

## 9. State layout

Application compatibility state remains under systemd `StateDirectory=prime-win-app-<application-id>`. W2 adds:

```text
/var/lib/prime-win-app-<id>/
  wine-prefix/
  components/
    <component-id>/
      installed.json
  locks/
    compatibility.lock
```

The marker records component ID, revision, component digest, donor fingerprint, installation timestamp, verification summary and resulting state. It is cache/evidence of satisfaction, not authority: probes are authoritative whenever a marker is stale or inconsistent.

## 10. Concurrency and locking

Prefix initialization, dependency installation and component verification all share one per-application exclusive lock. Two launches cannot mutate one prefix concurrently.

Different applications remain independent and may prepare in parallel.

The W1 private lock implementation should be factored into a small shared Windows-state module rather than duplicated between launch and component engines.

## 11. Core API and readiness

W2 adds Core operations for:

- resolving the selected profile's Windows dependency plan;
- querying component satisfaction/readiness;
- installing a trusted component plan;
- launching only when the plan is satisfied.

The existing Shell launch request remains stable. A launch request may now trigger bounded prerequisite preparation before launch. Shell/Application projection readiness must distinguish:

- `READY`;
- `INSTALLABLE`;
- `PREPARING`;
- `DEPENDENCY_UNAVAILABLE`;
- `INSTALL_RECIPE_REQUIRED`;
- `BROKEN`.

No UI is required to expose donor-level controls.

## 12. Error semantics

W2 distinguishes at least:

- `WINDOWS_COMPONENT_REFERENCE_INVALID`;
- `WINDOWS_COMPONENT_UNAVAILABLE`;
- `WINDOWS_COMPONENT_DIGEST_MISMATCH`;
- `WINDOWS_COMPONENT_ARCH_UNSUPPORTED`;
- `WINDOWS_COMPONENT_DEPENDENCY_CYCLE`;
- `WINDOWS_COMPONENT_ARTIFACT_MISMATCH`;
- `WINDOWS_INSTALL_RECIPE_REQUIRED`;
- `WINDOWS_INSTALLER_FAILED`;
- `WINDOWS_COMPONENT_VERIFY_FAILED`;
- `WINDOWS_COMPONENT_STATE_INVALID`;
- `WINDOWS_COMPONENT_LOCK_FAILED`.

Errors fail the specific launch/install transaction and preserve prior valid application state. They do not mutate the selected profile or silently weaken policy.

## 13. Evidence

Windows component evidence is append-only and records:

- transaction ID;
- host/generation;
- application/profile identity;
- component ID/revision/digest;
- provider/donor fingerprint;
- installer artifact identity when applicable;
- admitted/completed timestamps;
- operation (`VERIFY`, `INSTALL`, `RECOVER`);
- outcome;
- installer exit code;
- verification results;
- enforcement properties;
- resulting component-state marker digest.

A successful process exit without successful probes is never `INSTALLED`.

## 14. Security invariants

- No shell invocation in installer or probe paths.
- No user-controlled installer flags.
- Installer artifacts are absolute, regular, non-symlink files with exact identity.
- Component manifests are root/image-owned trusted records.
- Application components mutate only that application's compatibility state.
- Windows installers do not receive `prime-shell` authority.
- Existing Workload Policy remains in force for installation transactions; installation cannot be used to request broader network/device/filesystem access than the profile/policy permits.
- A component recipe requiring capabilities denied by policy fails closed.

## 15. W2 proof ladder on KRATOS

W2 is locally complete only when the exact frozen revision proves all of the following without booting a Prime image:

1. valid component references parse and bind exact revision/digest;
2. malformed/unpinned/digest-mismatched references fail closed;
3. dependency graph topological ordering and cycle rejection;
4. MSI controlled fixture installs into a cold application prefix;
5. EXE controlled installer fixture installs into a cold application prefix;
6. verification probes prove resulting state rather than trusting exit code;
7. repeat launch detects already-satisfied components and does not reinstall;
8. interrupted/stale marker state recovers deterministically;
9. concurrent same-app installation attempts serialize under one lock;
10. two different application prefixes can prepare independently;
11. a real common-runtime recipe is installed or verified and then consumed by a dependent PE fixture;
12. normal W1 portable app with zero dependencies still follows the existing path unchanged;
13. provider absent / component missing / wrong architecture / denied policy negative controls pass;
14. full workspace regression + clippy + format + diff integrity;
15. independent Sergeant review and frozen-SHA verifier.

Rootful sealed-image/boot verification is intentionally deferred under the owner's instruction. W2 local completion must leave that as an explicit final integration gate rather than blocking KRATOS development.

## 16. Completion rule

W2 is complete when Prime can take an exact dependency-bearing Windows Application Profile, resolve its immutable prerequisite graph, install or verify those components inside that application's persistent compatibility state under Prime policy, recover safely from interruption, emit bound evidence, and then launch through the already-proven Windows Personality path.
