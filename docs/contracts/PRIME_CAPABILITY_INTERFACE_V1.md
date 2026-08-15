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
GET /v1/health
GET /v1/capabilities
GET /v1/capabilities/{capability_id}
```

## P1 mutation endpoint

```text
POST /v1/exec/native/launch
```

The first mutation route is defined by `PRIME_NATIVE_LAUNCH_V1.md`. In P1 it is Host-local and requires Unix peer UID `0`. Socket access alone does not imply execution authorization.

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

Host health combines the truth required for Prime Host authority; capability health remains capability-specific. For example, an absent TPM is a valid inventory result, while an unreadable required kernel inventory source is a probe limitation.

## Evidence

Capability observations carry timestamps and evidence references where applicable. Mechanical state is Prime truth; independent engineering assurance remains Sergeant's separate responsibility.
