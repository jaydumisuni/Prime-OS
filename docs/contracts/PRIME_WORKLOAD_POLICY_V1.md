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
  "digest": "sha256",
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

## P1 native enforcement mapping

Prime uses the strongest available matching Linux mechanism and records what was actually enforced:

- systemd transient scopes/cgroup v2 — CPU, memory, process and I/O controls;
- namespaces — process/mount/network isolation where required;
- seccomp — syscall restriction;
- Landlock — supported unprivileged filesystem restriction;
- network namespaces/nftables — restrictive network modes;
- explicit broker/allowlist — device and secret access.

A launch requiring a policy control that Prime cannot enforce is denied with a mechanical reason. `best effort` is not silently substituted for a hard policy.

## Filesystem exposure

Each exposure binds an exact normalized path/object, access mode (`READ`, `WRITE`, `CREATE`, `EXECUTE`) and provenance/owner where relevant. Generic host-root exposure is not implied by application launch.

## Secret rule

Secrets are granted by identity/reference and delivered through an explicit broker or protected descriptor/file mechanism. Prime does not inherit the daemon's arbitrary environment into child workloads.

## Evidence

Launch evidence records policy ID/revision/digest, enforcement mechanisms actually installed, denied controls if any, workload identity, profile revision and lifecycle outcome. Infrastructure interruption must not be reported as workload success.
