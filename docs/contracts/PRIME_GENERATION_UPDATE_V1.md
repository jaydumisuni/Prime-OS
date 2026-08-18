# Prime Generation and Update Contract v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Schema family: `prime.generation.v1`

Image-authored seed schema: `prime.generation-seed.v1`

Boot-health evidence schema: `prime.generation-health.v1`

## Generation identity

A generation is a specific bootable Prime system image deployment. It is not the Prime Host identity.

The completed runtime record contains:

```json
{
  "schema": "prime.generation.v1",
  "generation_id": "prime-gen-<opaque>",
  "image_digest": "sha256:...",
  "channel": "LAB|CANDIDATE|STABLE",
  "created_at": "RFC3339",
  "source_revision": "git-sha",
  "state": "STAGED|BOOT_TRY|HEALTH_PROVING|KNOWN_GOOD|REJECTED|ROLLED_BACK|RECOVERY",
  "boot_attempts_remaining": null,
  "evidence_refs": []
}
```

`generation_id` is opaque. Image digest and source revision provide reproducibility; the ID is not derived from mutable labels/tags.

## Image/runtime identity split

The final produced image digest is not embedded in the same image as an authored field because that would be self-referential.

The image instead contains `/usr/lib/prime/generation-seed.json` as defined by `PRIME_GENERATION_SEED_V1.md`. The seed records build provenance including the pinned base-image digest, but not the final produced Prime image digest.

At boot, Prime Core queries the stable bootc programmatic status interface and binds the seed to the immutable digest of the actual booted deployment. That binding creates or validates the completed `prime.generation.v1` record under `/var/lib/prime/generations/current.json`.

The first new runtime binding enters `BOOT_TRY`; userspace reachability alone is not `KNOWN_GOOD`.

## Required Host slots

Prime tracks independently:

- `current_generation`;
- `previous_known_good_generation`;
- `recovery_generation`;
- optional `staged_generation`.

User/project data and Prime Host identity are not stored only inside a generation.

P1 currently implements the exact `current_generation` binding and health-state semantics required for First Light. Full slot rotation, transactional activation and exhaustive rollback behavior are P1.5 proof/implementation work and must not be implied by the presence of the enum values.

## Update state machine

```text
DISCOVERED
  -> DOWNLOADED
  -> VERIFIED
  -> STAGED
  -> BOOT_TRY
  -> HEALTH_PROVING
  -> KNOWN_GOOD
```

Failure can transition to `REJECTED` and/or boot/queue `previous_known_good_generation`.

P1 establishes the state and boot layout. P1.5 proves corruption, interruption, power-loss, candidate-health, late-regression and rollback cases.

### P1 runtime transition boundary

A newly bound generation remains `BOOT_TRY` while Prime Core is still establishing its Host-local runtime.

After `/run/prime/core.sock` is successfully bound and its permissions are established, `primed` transitions the exact persisted generation to `HEALTH_PROVING` and records:

```text
prime.core.socket.bound.v1
```

as generation evidence.

This transition means **health proof has begun**. It does not mean the generation is healthy or known-good.

A persisted `HEALTH_PROVING` generation is reused idempotently after a restart when its generation ID/image digest/source/channel binding still matches the actual booted deployment. Prime must not regress it back to `BOOT_TRY` merely because `primed` restarted.

## Verification

Before `STAGED`, Prime records at least:

- immutable image digest;
- expected source/release identity;
- local digest/integrity verification result;
- available-space/preflight result;
- compatibility preflight for the Capability Interface and profile schema where applicable.

A tag name alone is not verification.

At runtime the current generation's `image_digest` must agree with the immutable digest reported for the actual booted bootc deployment. A missing, malformed or conflicting booted image identity fails closed.

## Boot health

A candidate is not marked `KNOWN_GOOD` merely because the kernel reached userspace or because the Core socket exists.

P1 health proof must at least confirm:

- Prime Core interface readiness;
- Host identity readiness;
- the exact generation/image binding;
- required hardware baseline readiness;
- Prime Shell reachability;
- recovery reachability.

The evidence carrier is:

```json
{
  "schema": "prime.generation-health.v1",
  "generation_id": "...",
  "image_digest": "sha256:...",
  "observed_at": "RFC3339",
  "core_interface_ready": true,
  "host_identity_ready": true,
  "hardware_baseline_ready": true,
  "shell_ready": true,
  "recovery_ready": true,
  "limitations": []
}
```

A health report can promote only the **same generation ID and immutable image digest** currently in `HEALTH_PROVING`. Empty/mismatched identity, any false required gate, or any limitation prevents promotion.

### Reconciled P1 report builder

`primed::p1_health::build_report` is the single P1 reconciliation point for the report above. It does not decide that all gates have passed merely because it was called.

It derives:

- `generation_id` and `image_digest` from the exact current `GenerationRecord`;
- Host readiness from the enrolled `prime.host-identity.v1` record, matching Host/Hardware Graph architecture, SHA-256 enrollment evidence and HIGH/MEDIUM fingerprint confidence;
- `hardware_baseline_ready` from `primed::hardware::p1_baseline_limitations` and therefore the frozen HP 290 G4 / Kratos baseline;
- `core_interface_ready`, `shell_ready`, and `recovery_ready` only from explicit owning-layer inputs.

The builder keeps every unearned input false and emits a limitation for it. It also emits a limitation if a caller attempts to describe a generation that is not in `HEALTH_PROVING` as a P1 health candidate.

This prevents the implementation from spreading five unrelated booleans across service code or inferring Shell/recovery success from process existence. The report is reconciled evidence; promotion remains a separate generation-authority action.

On successful promotion Prime first persists the health report as append-only evidence under:

```text
/var/lib/prime/evidence/generation-health/<uuidv7>.json
```

and only then atomically writes the current generation as `KNOWN_GOOD`. A known-good generation clears `boot_attempts_remaining` and records an evidence reference to the persisted `prime.generation-health.v1` object.

P1 does not fabricate a health report. Until Shell, recovery and the physical proof Host have produced the required evidence, the current candidate must remain `HEALTH_PROVING` and its health projection must remain non-healthy.

## Capability/health truth

`prime.generation.current` and `/v1/health` must reflect generation state truthfully:

- `KNOWN_GOOD` may report generation health `HEALTHY`;
- `BOOT_TRY`, `STAGED`, and `HEALTH_PROVING` report generation health `UNKNOWN` and carry an explicit limitation;
- `REJECTED` reports failed generation health;
- `ROLLED_BACK` and `RECOVERY` report degraded generation health.

Future P1.5 scope is described separately as a limitation and is not itself evidence that an otherwise `KNOWN_GOOD` P1 generation is unhealthy.

## Persistent-state rule

Host identity lives under `/var/lib/prime`. Generation rollback must not silently replace it. Profile/application/user/project data are versioned or migrated independently from generation identity.

The persisted generation record may retain state transitions, but its bound immutable image digest and generation ID cannot silently change underneath that history.

## Current P1 proof expectation

The current local/QEMU First-Light proof is expected to finish with the exact booted generation in:

```text
HEALTH_PROVING
```

with `prime.core.socket.bound.v1` present in `evidence_refs` and the original boot-attempt budget still intact.

That proof demonstrates that normal UEFI boot reached Prime Core and entered the health campaign. It deliberately does **not** claim `KNOWN_GOOD`; Shell/recovery/physical acceptance remain required before promotion.

## Automation

P1 does not enable unattended automatic update/apply by default. Prime Core is the policy authority that decides when a verified bootc deployment may be activated.
