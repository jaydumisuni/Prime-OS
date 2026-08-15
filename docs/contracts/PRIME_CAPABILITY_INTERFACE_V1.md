# Prime Capability Interface v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Contract family: `prime.capability.v1`

Transport for P1: HTTP/1.1 + JSON over `/run/prime/core.sock`.

## Purpose

Expose mechanical Prime Host capability truth without importing Origins, Ptah or model semantics into Prime Core.

## Required read endpoints

```text
GET /v1/versions
GET /v1/host
GET /v1/hardware
GET /v1/storage
GET /v1/health
GET /v1/capabilities
GET /v1/capabilities/{capability_id}
```

## P1 bounded mutation/calculation endpoints

```text
POST /v1/exec/native/launch
POST /v1/storage/preflight
```

`POST /v1/exec/native/launch` is defined by `PRIME_NATIVE_LAUNCH_V1.md`.

`POST /v1/storage/preflight` is defined by `PRIME_STORAGE_INVENTORY_V1.md`. It performs a fresh mechanical storage observation and computes update-space admission only; it does not download, stage, activate, or boot an update.

Both P1 routes are Host-local and require Unix peer UID `0`. Socket access alone does not imply execution or update authorization.

No generic command/shell mutation endpoint exists. Future mutating endpoints remain capability-specific and must pass Prime authorization and Workload Policy.

## Version negotiation

- major change = breaking semantic change;
- minor change = additive/backward-compatible;
- consumer sends the highest versions it supports;
- Prime selects the highest mutually supported version;
- zero overlap returns HTTP 409 with `PRIME_INTERFACE_INCOMPATIBLE`, Prime-supported versions and consumer-requested versions;
- no compatibility is fabricated.

All versioned reads and mutations except `GET /v1/versions` require explicit `Prime-Interface-Accept` negotiation.

## Host projection

```json
{
  "interface": "prime.capability.v1",
  "interface_version": "1.0",
  "host": {
    "host_id": "uuid",
    "host_arch": "x86_64",
    "generation_id": "string",
    "hardware_graph_revision": 1
  }
}
```

`GET /v1/hardware` returns the sanitized `prime.hardware-graph.v1` record. Raw DMI UUIDs, hardware serial numbers and raw network MAC addresses are not part of that public projection.

`GET /v1/storage` returns the latest Prime `prime.storage-inventory.v1` observation. The storage preflight path refreshes mount/capacity truth before calculating admission rather than relying on a stale cached observation.

## Capability descriptor

```json
{
  "capability_id": "prime.exec.native",
  "capability_version": "1.0.0",
  "family": "execution",
  "provider": {
    "id": "prime",
    "generation_id": "string"
  },
  "availability": "AVAILABLE|DEGRADED|UNAVAILABLE|INCOMPATIBLE",
  "effects": [],
  "accepts": {
    "formats": [],
    "runtime_families": [],
    "workload_arches": []
  },
  "permissions": [],
  "resources": {},
  "hardware_requirements": [],
  "limits": {},
  "health": {
    "status": "HEALTHY|DEGRADED|FAILED|UNKNOWN",
    "observed_at": "RFC3339",
    "evidence_refs": []
  },
  "limitations": [],
  "placement": {
    "scope": "HOST_LOCAL",
    "host_id": "uuid"
  },
  "expected_evidence": [],
  "rollback": {
    "supported": false,
    "mode": null,
    "limitations": []
  }
}
```

Fields that are unknown must be represented as unknown/null/empty with an explicit limitation where material. They must not be guessed.

## Origins seam

Origins may later map this descriptor into its own capability compiler/Node projection. Prime does not expose `Origins Node ID`, mission ordering or AgentOps lifecycle in this contract.

## Health truth

`AVAILABLE` is not synonymous with `HEALTHY`. A capability may exist but be degraded. A capability requiring unavailable hardware/provider/runtime reports that limitation explicitly.

Host health combines the truth required for Prime Host authority; capability health remains capability-specific. An optional storage reserve policy being unconfigured degrades the storage/update-preflight capability but does not crash unrelated Host authority. Loss of root local-physical capacity truth degrades Host health because Prime can no longer prove update/storage safety.

## Evidence

Capability observations carry timestamps and evidence references where applicable. Mechanical state is Prime truth; independent engineering assurance remains Sergeant's separate responsibility.
