# Prime Native Launch v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Request schema: `prime.native-launch-request.v1`

Evidence schema: `prime.native-launch-evidence.v1`

Capability route:

```text
POST /v1/exec/native/launch
```

## Purpose

Provide the first bounded Prime-managed native/Linux launch path without exposing arbitrary shell authority.

A successful request means Prime admitted one exact selected Application Profile revision, one exact Workload Policy revision, one exact artifact identity, and asked the local systemd manager to execute the resulting transient service under the compiled policy.

It does not imply broad Linux compatibility or P1 completion.

## P1 authorization

The P1 route is Host-local and accepts only a Unix peer credential with UID `0`.

This is deliberately restrictive until Prime user/session authorization is implemented. Socket possession alone is not execution authorization.

## Request

```json
{
  "schema": "prime.native-launch-request.v1",
  "application_id": "uuid",
  "artifact_path": "/absolute/candidate/path"
}
```

P1 v1 accepts no caller-supplied shell command, environment block, working directory, interpreter, or argument vector.

`artifact_path` is only a candidate source location. It is not artifact identity and is never the final systemd execution target.

## Admission sequence

Prime MUST, in order:

1. load the selected non-revoked Application Profile revision;
2. require `execution_backend=NATIVE`, `format=ELF`, `runtime_family=NATIVE_LINUX`;
3. load the exact Workload Policy ID/revision referenced by the profile;
4. require the loaded policy digest to equal the profile's pinned policy digest;
5. compile that policy through the P1 native fail-closed compiler;
6. copy the candidate artifact while observing one stable regular non-symlink file and hashing the copied bytes;
7. require the copied bytes to match the profile's artifact SHA-256 identity;
8. publish the verified bytes into the Prime-owned content-addressed artifact store;
9. inspect the staged artifact and require exact profile format/runtime/architecture plus native Host compatibility;
10. invoke systemd directly, without a shell, using the staged artifact as the executable;
11. emit immutable launch evidence.

Any failed step denies launch. Prime does not silently weaken policy or fall back to direct `execve`, shell execution, another runtime, or another artifact revision.

## Content-addressed artifact store

Production default:

```text
/var/lib/prime/artifacts/sha256/<64-lowercase-hex-digest>
```

The final artifact is root-owned by normal production deployment, executable/readable but not writable by application workloads, and lives beneath a Prime-controlled directory. Existing content at the same digest is re-inspected before use.

The source path may change after staging without changing the admitted workload because systemd receives only the staged content-addressed path.

## systemd transient service

P1 uses the system manager and a transient `.service` unit. Prime passes arguments to `systemd-run` as an argv vector; it does not invoke `/bin/sh`, `bash -c`, or any other shell parser.

The service uses:

```text
--system
--service-type=exec
--wait
--collect
--no-ask-password
```

plus the exact properties emitted by the native Workload Policy compiler.

Restricted P1 application classes also use a dynamic service identity rather than executing as `primed`/root.

`Type=exec` is required so preparatory execution failures are reported by the service manager rather than treating a successful fork as a successful start.

`--wait` keeps the admission call synchronous through service termination. `--collect` permits the transient unit to be unloaded after completion, including failure.

## P1 result semantics

A zero `systemd-run --wait` status is recorded as `EXITED_SUCCESS`.

A non-zero status is recorded as `SYSTEMD_OR_WORKLOAD_FAILURE` with the launcher exit code when available. P1 v1 does not fabricate a distinction between service-manager admission failure and workload exit failure when this frontend status alone cannot mechanically distinguish them.

A spawn/I/O failure before `systemd-run` runs is `LAUNCHER_FAILURE`.

## Evidence

```json
{
  "schema": "prime.native-launch-evidence.v1",
  "launch_id": "uuid",
  "host_id": "uuid",
  "generation_id": "string",
  "application_id": "uuid",
  "profile_revision": 1,
  "profile_digest": "sha256:...",
  "policy_id": "uuid",
  "policy_revision": 1,
  "policy_digest": "sha256:...",
  "artifact_identity": "sha256:...",
  "staged_artifact_path": "/var/lib/prime/artifacts/sha256/...",
  "unit_name": "prime-app-....service",
  "requested_at": "RFC3339",
  "completed_at": "RFC3339",
  "outcome": "EXITED_SUCCESS|SYSTEMD_OR_WORKLOAD_FAILURE|LAUNCHER_FAILURE",
  "launcher_exit_code": 0,
  "enforcement_properties": []
}
```

Evidence is append-only under Prime state. A failed launch is still evidence.

## Deliberate P1 limits

This contract does not yet provide:

- caller arguments or environment injection;
- writable filesystem exposures/Landlock rules;
- device allowlists or shared/exclusive GPU mediation for user/build/foreign workloads;
- secret grants;
- richer network modes beyond compiler-supported policy;
- interactive terminal attachment;
- user/session authorization;
- background detach semantics;
- foreign runtime execution.

Those remain separate capabilities and may not be inferred from native launch.