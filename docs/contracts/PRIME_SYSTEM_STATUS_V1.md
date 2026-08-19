# Prime System Status v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Schema: `prime.system-status.v1`

Capability: `prime.system.status`

## Purpose

Provide Prime Shell with sanitized, mechanical Host-local status that Prime Core can prove without importing desktop policy or pretending unavailable control backends exist.

This capability is read-only in this phase and is exposed through the existing `GET /v1/capabilities` and `GET /v1/capabilities/prime.system.status` surfaces. The capability resources are refreshed when those endpoints are read.

## Required observations

The status resource may expose only the following P1 observations:

- network interface kernel name, wireless classification, link `operstate`, and carrier truth where mechanically readable;
- sanitized sound-card kernel name and card ID already present in Prime hardware inventory;
- power-supply kernel name, supply type, status, capacity percentage, and online truth where mechanically readable;
- thermal-zone kernel name/type and temperature in millidegrees Celsius where mechanically readable.

Raw MAC addresses, IP configuration, SSIDs, credentials, mixer state, microphone contents, battery serials, and arbitrary `/sys` values are outside this contract.

## Control truth

This phase does **not** earn control authority merely because status can be observed.

The resource must explicitly report:

- `network_control.ready=false` until a bounded Prime network mutation backend is separately implemented and proven;
- `audio_control.ready=false` until a bounded Prime audio mixer/control backend is separately implemented and proven;
- `power_mutation.ready=false` until restart/shutdown mutation is exposed through an authorized Prime capability route.

Prime Shell must render those controls disabled/unavailable rather than calling distro tools directly behind Prime Core.

## Freshness

`observed_at` is regenerated for each capability read. Network carrier/operstate, power-supply values, and thermal temperatures are observed from the current Host filesystem at that read boundary.

Sound-card identity and wireless classification remain bound to the current sanitized `prime.hardware-graph.v1` topology.

## Failure semantics

Missing optional hardware is represented by empty collections, not failure.

If Prime knows a network interface from the hardware graph but cannot read its current `operstate`, the capability is degraded and carries an explicit limitation naming only the sanitized interface identifier.

Failure of this optional status capability must not falsify Prime Host identity, generation, storage, or execution truth.

## P1 Shell seam

Prime Shell may consume `prime.system.status` for system rail, Settings, and degraded-state presentation. It must not infer control authority from hardware presence or from a successful status read.
