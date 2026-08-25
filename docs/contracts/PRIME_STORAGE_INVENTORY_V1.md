# Prime Storage Inventory and Preflight v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Inventory schema: `prime.storage-inventory.v1`

Reserve-policy schema: `prime.storage-reserve-policy.v1`

Preflight schema: `prime.storage-preflight.v1`

Pressure schema: `prime.storage-pressure.v1`

## Purpose

This contract implements only the Prime Storage Intelligence foundation required by P1 First Light and P1.5 Survival.

It owns mechanical Host storage truth for:

- mounted filesystem identity;
- capacity/free/available/reserved accounting;
- local-physical versus remote/memory/overlay/virtual classification;
- non-double-counted local physical totals;
- generation/reserve visibility;
- update-space admission;
- storage pressure state/evidence.

It does **not** implement the P2 recursive file index, duplicate engine, full Change Engine, treemap, or filesystem-specific shared/exclusive extent analysis.

## Kernel sources

P1 inventory uses:

- `/proc/self/mountinfo` for mount identity/topology;
- `statvfs(3)` for mounted-filesystem capacity.

Prime does not parse live filesystem metadata directly from raw block devices for ordinary P1 capacity accounting.

## Mount identity

Each mount record contains:

```json
{
  "mount_id": 36,
  "parent_mount_id": 35,
  "major_minor": "259:2",
  "root": "/",
  "mount_point": "/",
  "filesystem_type": "btrfs",
  "mount_source": "/dev/nvme0n1p2",
  "read_only": false,
  "scope": "LOCAL_PHYSICAL",
  "filesystem_key": "259:2|btrfs",
  "capacity": {},
  "limitations": []
}
```

Linux mount IDs are namespace-local mount-instance identifiers and may be reused after unmount. They are not durable Prime object IDs.

`filesystem_key` is only a P1 accounting key used to avoid counting multiple mounts/subvolume/bind views of the same mounted filesystem capacity more than once. It is not a globally stable filesystem UUID.

## Path representation

Linux paths need not be valid UTF-8.

Prime stores mount `root`, `mount_point`, and `mount_source` in a byte-safe display form:

- valid UTF-8 is represented directly;
- non-UTF-8 bytes are represented using `\xNN` escapes.

The capacity probe still operates on the original decoded OS pathname bytes rather than the display representation.

## Storage scope

P1 classifies mounts conservatively:

- `LOCAL_PHYSICAL` — confidently local filesystem capacity;
- `REMOTE` — remote/provider-backed filesystem capacity;
- `MEMORY` — RAM-backed filesystems such as tmpfs;
- `OVERLAY` — overlay/composed view whose backing storage belongs elsewhere;
- `VIRTUAL` — kernel/pseudo/control filesystem;
- `UNKNOWN` — Prime cannot safely determine local physical ownership.

Only unique `LOCAL_PHYSICAL` filesystem keys contribute to Host local-physical totals.

Remote, memory, overlay, virtual, and unknown capacity may still be reported per mount when `statvfs` succeeds, but it is not added to local disk ownership totals.

## Capacity semantics

```json
{
  "source": "statvfs",
  "fragment_size_bytes": 4096,
  "total_bytes": 1000000,
  "free_bytes": 300000,
  "available_bytes": 250000,
  "used_bytes": 700000,
  "reserved_bytes": 50000
}
```

Definitions:

- `total_bytes = f_blocks * f_frsize`;
- `free_bytes = f_bfree * f_frsize`;
- `available_bytes = f_bavail * f_frsize`;
- `used_bytes = total_bytes - free_bytes`;
- `reserved_bytes = free_bytes - available_bytes` when that subtraction is valid.

For P1, `reserved_bytes` means **free space not available to an ordinary unprivileged allocation according to `statvfs`**. It may reflect filesystem reservation and/or allocation policy. Prime does not relabel this as exact filesystem metadata usage.

Arithmetic overflow or internally inconsistent counters are reported as limitations; Prime does not wrap or invent a value.

## Inventory projection

```json
{
  "schema": "prime.storage-inventory.v1",
  "observed_at": "RFC3339",
  "mount_namespace_source": "/proc/self/mountinfo",
  "mounts": [],
  "local_physical_totals": {
    "filesystem_count": 1,
    "total_bytes": 0,
    "free_bytes": 0,
    "available_bytes": 0,
    "used_bytes": 0,
    "reserved_bytes": 0
  },
  "root_mount_id": 36,
  "generation_accounting": {
    "current_generation_id": "prime-gen-...",
    "current_generation_bytes": null,
    "previous_known_good_bytes": null,
    "recovery_generation_bytes": null,
    "staged_generation_bytes": null,
    "limitations": []
  },
  "reserve": {
    "policy_configured": false,
    "protected_rollback_recovery_bytes": null,
    "limitations": []
  },
  "pressure": {
    "state": "UNKNOWN|NORMAL|LOW|CRITICAL",
    "available_bytes": null,
    "low_threshold_bytes": null,
    "critical_threshold_bytes": null,
    "limitations": []
  },
  "limitations": []
}
```

Generation byte attribution remains `null` until Prime's generation storage layout can mechanically attribute those bytes without snapshot/reflink double counting. P1 must expose that uncertainty rather than fabricate per-generation usage.

## Reserve policy

The image may provide:

```text
/usr/lib/prime/storage-reserve-policy.json
```

with:

```json
{
  "schema": "prime.storage-reserve-policy.v1",
  "protected_rollback_recovery_bytes": 123,
  "low_space_warning_bytes": 456,
  "critical_space_bytes": 234
}
```

Rules:

- values are explicit image policy, not guessed percentages;
- `critical_space_bytes <= low_space_warning_bytes`;
- update preflight fails closed if the protected rollback/recovery reserve is not configured;
- pressure state is `UNKNOWN` if pressure thresholds are not configured;
- policy parsing/schema/arithmetic failures are explicit limitations.

No default byte amount is invented by this contract because the accepted Prime planning authority does not freeze one.

## Update-space preflight

Input:

```json
{
  "schema": "prime.storage-preflight.v1",
  "required_staging_bytes": 100,
  "target_mount_id": null
}
```

If `target_mount_id` is null, Prime uses the mount containing `/`.

Result:

```json
{
  "schema": "prime.storage-preflight.v1",
  "target_mount_id": 36,
  "required_staging_bytes": 100,
  "available_bytes": 1000,
  "protected_rollback_recovery_bytes": 500,
  "remaining_after_stage_bytes": 900,
  "admitted": true,
  "reason": "SPACE_AVAILABLE_WITH_PROTECTED_RESERVE",
  "observed_at": "RFC3339"
}
```

Admission rule:

```text
available_bytes >= required_staging_bytes + protected_rollback_recovery_bytes
```

All addition/subtraction is checked arithmetic.

Preflight is denied mechanically when:

- reserve policy is unconfigured/invalid;
- target mount is missing;
- target mount capacity is unavailable;
- target mount is not `LOCAL_PHYSICAL`;
- arithmetic overflows;
- staging would consume protected reserve.

P1 does not provide an override path that silently consumes rollback/recovery reserve.

## Storage pressure

Pressure is evaluated against the configured thresholds on the root local-physical filesystem:

- `CRITICAL` when `available_bytes <= critical_space_bytes`;
- `LOW` when `available_bytes <= low_space_warning_bytes`;
- otherwise `NORMAL`;
- `UNKNOWN` when the root/capacity/threshold truth is incomplete.

P1 persists pressure transition evidence. The full incremental filesystem Change Engine remains P2.

## Capability Interface

P1 adds read projection:

```text
GET /v1/storage
```

and root-authorized preflight calculation:

```text
POST /v1/storage/preflight
```

The preflight endpoint computes mechanical admission only; it does not download, stage, activate, or boot an update.

## Proof boundary

Hosted fixture tests must prove:

- mountinfo parsing including optional fields;
- mountinfo escaped path decoding;
- malformed mount lines are limited rather than guessed;
- bind/subvolume duplicate filesystem keys do not double count Host totals;
- overlay/remote/memory/virtual/unknown mounts do not enter local physical totals;
- `statvfs` arithmetic semantics;
- reserve-policy validation;
- admitted and denied update-space preflight;
- unknown pressure when policy is absent;
- low/critical threshold boundaries.

Physical Prime Host proof must later compare Prime inventory/preflight against the running kernel/filesystem and generation layout. Hosted tests alone do not prove Btrfs/ext4/XFS physical behavior on Kratos.