# Prime Host Security Interface

Status: **ACCEPTED P0 AUTHORITY SUPPLEMENT — PLANNING ONLY**

This contract defines the machine-security seam between Prime OS and cybersecurity systems such as Grid-Knight.

It does not make Prime a threat-intelligence or antivirus product, and it does not move cybersecurity judgment out of Grid-Knight.

## Authority boundary

```text
Prime OS
= machine security mechanisms + mechanical host truth + enforcement primitives

Grid-Knight / Cyber-Team
= threat interpretation + correlation + protection policy + false-positive handling + cleanup/remediation + retest evidence
```

Prime must remain secure even when Grid-Knight is not installed. Grid-Knight may consume Prime's security events and request authorized enforcement, but it does not bypass Prime Workload Policy or machine authority.

## Mechanical security events

Prime should expose versioned, permissioned events such as:

- file created / modified / renamed / removed;
- executable or script appeared;
- file/content hash changed when available;
- process launched / stopped / crashed;
- Prime Application Profile identity and revision for a workload;
- network connection opened/closed and policy violation;
- driver/module loaded or rejected;
- USB/device attached/removed;
- startup/persistence-related system change;
- Prime Workload Policy violation;
- filesystem protection/trust change;
- security-relevant generation/update/recovery event;
- Prime Storage Intelligence file/change/hash events where available.

These events are mechanical observations. Prime must not label an event `malware` merely because it is unusual.

## Authorized enforcement primitives

Subject to Prime permissions, policy, audit and owner/system authority, the interface may expose actions such as:

- deny or revoke workload launch;
- suspend or terminate a process/workload;
- tighten or remove network access;
- isolate a workload/container/VM;
- quarantine/move a file where safe and permitted;
- revoke or disable an Application Profile revision;
- restrict USB/device access;
- unload/block a non-critical driver where supported and safe;
- restore known-good system/application state through existing Prime recovery mechanisms;
- request deeper storage/content inspection through the relevant Prime facility.

Grid-Knight decides when cybersecurity evidence justifies requesting these actions. Prime decides whether the action is mechanically allowed and enforces it.

## Fail-closed rule

A cybersecurity Provider must not gain unrestricted host authority merely because it is trusted to interpret threats.

All actions remain subject to:

- Prime Workload Policy;
- explicit capability/permission boundaries;
- versioned API contracts;
- audit/event evidence;
- safety rules protecting current/previous/recovery generations and other protected storage;
- owner approval where the configured response policy requires it.

## Storage Intelligence relationship

Prime Storage Intelligence is a shared source of storage truth, not a malware engine.

Preferred flow:

```text
Prime Storage Index / Change Engine
        ↓
mechanical file metadata, hashes and change events
        ↓
Grid-Knight
        ↓
threat interpretation / deeper scan when warranted
        ↓
authorized Prime enforcement when required
```

This avoids multiple independent systems repeatedly rescanning the same filesystem without need.

## Grid-Knight relationship

Grid-Knight already records a future Antimalware / Endpoint Protection expansion in `jaydumisuni/Grid-Knight/docs/FUTURE_ANTIMALWARE_ENDPOINT_PROTECTION.md`.

That capability remains one department inside Grid-Knight's broader cybersecurity authority; Grid-Knight is not reduced to a Defender clone.

## P0/P1 placement

P0 must freeze:

- event schema families;
- authorization model;
- enforcement capability boundaries;
- audit/evidence requirements;
- version negotiation/deprecation behavior;
- failure/degraded behavior when Grid-Knight is absent or incompatible.

P1 only needs the minimal secure host-event/enforcement foundation required by Prime itself. Full Grid-Knight integration is not a First Light blocker.

Later integration must be proven independently before cybersecurity automation is enabled.
