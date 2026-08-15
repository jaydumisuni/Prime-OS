# Prime Workload Policy v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Schema identifier: `prime.workload-policy.v1`

## Invariant

Every Prime-managed workload has an exact policy revision, regardless of execution backend. A future Ptah/Origins constraint may tighten Prime policy but cannot weaken it.

## Required policy document

```json
{
  "schema": "prime.workload-policy.v1",
  "policy_id": "uuid",
  "revision": 1,
  "digest": "sha256:...",
  "class": "SYSTEM_CORE|SHELL|USER_APP|BUILD|FOREIGN_RUNTIME|RECOVERY",
  "cpu": {"weight": 100, "quota_percent": null},
  "memory": {"max_bytes": null, "swap_max_bytes": null},
  "gpu": {"mode": "DENY|SHARED|EXCLUSIVE|INHERIT"},
  "storage": {"quota_bytes": null, "io_weight": 100},
  "process": {"max_processes": null, "max_runtime_seconds": null},
  "network": {
    "mode": "OFFLINE|LAN_ONLY|OUTBOUND_INTERNET|DESTINATION_RESTRICTED|LOCAL_LISTENER|INBOUND_ALLOWED|UNRESTRICTED",
    "destinations": []
  },
  "filesystem": {"exposures": []},
  "devices": {"usb": [], "other": []},
  "secrets": {"grants": []},
  "background": {"allowed": false},
  "evidence": {"required": true, "classes": []}
}
```

## Digest and persistence

For v1, `digest` is SHA-256 over the compact UTF-8 JSON serialization of the typed policy with `digest` set to the empty string. The stored record must re-compute to the recorded digest before use.

P1 durable layout:

```text
/var/lib/prime/policies/<policy_id>/
  revisions/<20-digit-revision>.json
  selected
```

Revision files are append-only/create-once. `selected` is atomically replaced and may only identify an existing digest-valid revision.

## P1 native enforcement compiler

P1 separates **policy truth** from **backend enforcement support**.

The first native Linux compiler may emit a transient-systemd enforcement plan for controls that systemd/cgroup machinery can express mechanically, including:

- CPU weight and optional quota;
- memory and swap ceilings;
- I/O weight;
- task/process ceiling;
- maximum runtime;
- baseline no-new-privileges and kernel/control-group hardening;
- `OFFLINE` network isolation through a private network namespace;
- GPU/device isolation through `PrivateDevices` for a policy that denies GPU/device access.

The compiler must reject, rather than weaken, any hard policy it cannot yet enforce exactly.

In the initial compiler that means these remain **unsupported and launch-blocking** until their dedicated backend lands:

- storage quota;
- explicit filesystem exposure lists/Landlock rules;
- USB/other device allowlists;
- secret grants/broker delivery;
- exclusive GPU ownership;
- `LAN_ONLY`, `OUTBOUND_INTERNET`, `DESTINATION_RESTRICTED`, `LOCAL_LISTENER`, or `INBOUND_ALLOWED` network policies.

`UNRESTRICTED` is only accepted when the exact policy explicitly requests it. `OFFLINE` is accepted through isolated networking. No broader network access is inferred from an empty destination list.

This deliberately makes early Prime restrictive: unsupported policy semantics block launch instead of silently becoming best-effort.

## P1 native enforcement mapping

Prime records the exact properties/mechanisms it intends to install. Initial systemd/cgroup mappings include CPU/memory/task/I/O/runtime controls where requested. This follows the systemd resource-control contract rather than inventing a parallel cgroup schema.

The eventual launcher must verify that the transient unit was admitted by the system manager before claiming enforcement. Compilation alone is not runtime proof.

## Filesystem exposure

Each exposure binds an exact normalized path/object, access mode (`READ`, `WRITE`, `CREATE`, `EXECUTE`) and provenance/owner where relevant. Generic host-root exposure is not implied by application launch.

## Secret rule

Secrets are granted by identity/reference and delivered through an explicit broker or protected descriptor/file mechanism. Prime does not inherit the daemon's arbitrary environment into child workloads.

## Evidence

Launch evidence records policy ID/revision/digest, enforcement mechanisms actually installed, denied controls if any, workload identity, profile revision and lifecycle outcome. Infrastructure interruption must not be reported as workload success.
