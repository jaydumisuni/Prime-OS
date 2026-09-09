# Prime Windows Personality W1 Design

**Status:** Owner-approved implementation direction
**Date:** 2026-09-10
**Repository:** `jaydumisuni/Prime-OS`
**Execution standard:** `ttg.tenfold.v1`

## Goal

Deliver the first useful Prime Windows Personality slice so an x86_64 Prime Host can admit and run portable/simple x86/x86_64 PE applications as first-class Prime workloads. Prime owns detection, profile binding, policy, provider selection, launch, display access, evidence and failure semantics. Compatibility projects such as Wine or ReactOS remain replaceable implementation donors behind a Prime-owned provider boundary.

## Existing authority recovered

- Prime Exec already recognizes PE32/PE32+ and classifies the runtime family as `WINDOWS`.
- Prime Application Profile v1 already defines `execution_backend = PERSONALITY`.
- Every launch must pin an immutable Application Profile revision and exact SHA-256 artifact identity.
- Every workload, including personalities, must pass Prime Workload Policy.
- Prime intentionally hides Wine/Waydroid/QEMU-style backend plumbing from normal user interaction.
- P4A defines W0 PE recognition/profiles, W1 portable/simple Win32, then later installers, .NET, graphics, COM, services, USB, VM fallback and TTG certification.

## W1 scope

W1 supports:

- PE32 and PE32+ artifacts mechanically identified from bytes, not extension alone;
- x86 and x86_64 Windows workloads on an x86_64 Prime Host;
- selected, non-revoked Application Profiles with `runtime_family = WINDOWS` and `execution_backend = PERSONALITY`;
- portable/simple Win32 process launch;
- GUI attachment to the Prime compositor through an explicitly separated display-access group;
- a provider-neutral Prime Windows Personality runtime contract;
- at least one packaged provider adapter sufficient to prove a real PE workload;
- exact launch evidence including provider identity, profile revision, policy revision, artifact identity and exit result;
- explicit unavailable/incompatible failure states.

W1 does **not** claim:

- MSI/installer lifecycle or shared runtime installation (W2);
- .NET compatibility (W3);
- DirectX/DXVK/VKD3D acceleration (W4);
- COM completeness (W5);
- Windows services (W6);
- USB/device passthrough (W7);
- VM fallback (W8);
- broad TTG application certification (W9);
- cross-architecture translation;
- arbitrary command-line arguments or caller environment injection unless separately frozen.

## Architecture

### 1. Prime Exec stays the format/runtime authority

`ExecInspection` continues to identify PE format, Windows runtime family and workload architecture. It must not claim native compatibility. On x86_64 hosts, x86/x86_64 PE artifacts are mechanically eligible for the Windows Personality lane; actual availability is decided by the Windows Personality capability/provider registry at launch time.

### 2. Windows Personality is the public backend

Prime-facing contracts use `PERSONALITY` and `WINDOWS`. They never require an application, Shell, profile or user to name Wine, ReactOS or another donor.

The flow is:

```text
PE artifact
  -> Prime Exec inspection
  -> selected immutable Application Profile
  -> Workload Policy compilation
  -> Windows Personality provider selection
  -> provider adapter
  -> compatibility donor/runtime
  -> process/window
  -> Prime launch evidence
```

### 3. Provider ABI

W1 defines an image-owned/root-owned provider manifest directory and a Prime-owned adapter invocation contract. A provider manifest binds:

- schema/version;
- provider ID and revision;
- absolute adapter path;
- supported PE formats;
- supported workload architectures;
- capability limitations.

Prime never searches `$PATH` for a Windows runtime. It accepts only a validated manifest from the configured trusted provider directory and an absolute executable adapter path.

The adapter receives Prime-defined inputs: staged artifact path, application ID, launch ID and an isolated runtime-state directory. Donor-specific environment such as a Wine prefix is owned inside that adapter, not by Prime Exec or Shell.

The first adapter may use Wine to earn W1 proof. Replacing or supplementing it with ReactOS-derived or other compatibility technology must not change the Prime launch/profile API.

### 4. Workload isolation and state

The PE artifact is copied into the existing content-addressed Prime artifact store and re-inspected before execution. Windows workloads run under the same systemd transient-service enforcement model as native workloads. W1 runtime state is per-launch and bounded to a systemd-managed runtime directory; persistent installer/runtime state belongs to W2.

W1 foreign workloads retain `NoNewPrivileges`, kernel/control-group protection, process/memory/CPU limits and network/device restrictions compiled from Workload Policy. Unsupported policy semantics fail closed.

### 5. Display authority separation

Current Prime compositor state is owned by `prime-shell` with `0700` runtime-directory access, while the Core socket is also intentionally reachable by the Shell identity. W1 must not give foreign workloads `prime-shell` group membership merely to display a window.

Introduce `prime-display` as a separate non-control group:

- compositor primary group: `prime-display`;
- compositor runtime directory: group-readable/writable only as required for Wayland clients;
- Prime Shell: keeps `prime-shell` as its Core-authorized primary group and gains `prime-display` supplementary membership;
- admitted GUI workloads: may receive `prime-display` supplementary membership only;
- Core socket remains owned/authorized by `prime-shell`, so a foreign workload cannot gain mutation authority merely from display access.

### 6. Launch evidence

W1 adds backend-neutral/personality launch evidence rather than mislabeling Windows execution as native. Evidence records:

- schema;
- launch ID;
- host/generation;
- application/profile/policy identities and revisions;
- artifact SHA-256 and staged path;
- `execution_backend = PERSONALITY`;
- `runtime_family = WINDOWS`;
- provider ID/revision;
- systemd unit;
- timestamps;
- admission/completion outcome and exit code;
- enforced systemd properties.

Shell currently ignores the successful launch-response body, so extending the server to return Windows-specific/generic launch evidence does not require a visible Shell protocol redesign.

## Failure semantics

W1 must distinguish at least:

- `WINDOWS_PERSONALITY_UNAVAILABLE` — no trusted compatible provider is installed;
- `WINDOWS_PROVIDER_INVALID` — manifest/adapter validation failed;
- `WINDOWS_WORKLOAD_ARCH_UNSUPPORTED` — PE architecture is outside W1;
- `WINDOWS_PROFILE_MISMATCH` — profile/backend/runtime/format mismatch;
- `WINDOWS_ARTIFACT_MISMATCH` — staged bytes no longer match profile identity;
- `POLICY_DENIED` — Workload Policy cannot be compiled/enforced;
- provider/process failure with evidence rather than fake success.

No unsupported PE may silently fall back to native execution, arbitrary `$PATH` lookup, an unrelated Linux executable, or an unmanaged VM.

## Proof obligations

The W1 candidate is not complete until the exact frozen revision proves:

1. PE32+ x86_64 recognition and Windows runtime classification.
2. PE32 x86 recognition and Windows runtime classification.
3. Windows profiles are rejected by the native launcher.
4. Native profiles are rejected by the Windows launcher.
5. Missing provider fails explicitly before workload execution.
6. Untrusted/relative/non-executable provider adapter is rejected.
7. Provider selection is deterministic for format/architecture.
8. Artifact identity is revalidated after staging.
9. Workload policy remains applied to the transient unit.
10. Windows evidence names `PERSONALITY/WINDOWS`, never `NATIVE_LINUX`.
11. Provider identity is recorded in evidence.
12. No shell parser is used to invoke the provider.
13. Display access does not grant the Core socket group.
14. Existing native launch tests remain green.
15. Existing Shell application projection remains truthful.
16. A real portable PE process executes on KRATOS through the Prime Windows Personality path.
17. A negative-control PE of unsupported architecture is denied.
18. Provider removal changes capability/launch state to unavailable, not fake success.
19. Full workspace tests pass on the frozen candidate.
20. `git diff --check` and independent/adversarial review find no unresolved contract drift.

## Completion gate

W1 is complete when Prime can take a selected portable/simple x86/x86_64 PE Application Profile on an x86_64 Host, route it through `PERSONALITY/WINDOWS`, execute it via a validated provider adapter under Prime Workload Policy, produce bound launch evidence, and prove at least one real PE workload on KRATOS. GUI display attachment is accepted only when the `prime-display` separation is mechanically proven; otherwise the process-execution slice remains truthful and the desktop gate stays open.
