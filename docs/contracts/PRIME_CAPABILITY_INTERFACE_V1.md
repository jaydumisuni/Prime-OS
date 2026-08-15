# Prime Capability Interface v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Contract family: `prime.capability.v1`

Transport for P1: HTTP/1.1 + JSON over `/run/prime/core.sock`.

## Purpose

Expose mechanical Prime Host capability truth without importing Origins, Ptah or model semantics into Prime Core.

## Required endpoints

```text
GET /v1/versions
GET /v1/host
GET /v1/health
GET /v1/capabilities
GET /v1/capabilities/{capability_id}
```

Mutating endpoints are capability-specific and must pass Prime authorization and Workload Policy.

## Version negotiation

- major change = breaking semantic change;
- minor change = additive/backward-compatible;
- consumer sends the highest versions it supports;
- Prime selects the highest mutually supported version;
- zero overlap returns HTTP 409 with `PRIME_INTERFACE_INCOMPATIBLE`, Prime-supported versions and consumer-requested versions;
- no compatibility is fabricated.

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

## Evidence

Capability observations carry timestamps and evidence references where applicable. Mechanical state is Prime truth; independent engineering assurance remains Sergeant's separate responsibility.
