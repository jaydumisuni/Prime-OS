# Prime Generation and Update Contract v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Schema family: `prime.generation.v1`

## Generation identity

A generation is a specific bootable Prime system image deployment. It is not the Prime Host identity.

Required fields:

```json
{
  "schema": "prime.generation.v1",
  "generation_id": "prime-gen-<uuidv7>",
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

## Required Host slots

Prime tracks independently:

- `current_generation`;
- `previous_known_good_generation`;
- `recovery_generation`;
- optional `staged_generation`.

User/project data and Prime Host identity are not stored only inside a generation.

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

## Verification

Before `STAGED`, Prime records at least:

- immutable image digest;
- expected source/release identity;
- local digest/integrity verification result;
- available-space/preflight result;
- compatibility preflight for the Capability Interface and profile schema where applicable.

A tag name alone is not verification.

## Boot health

A candidate is not marked `KNOWN_GOOD` merely because the kernel reached userspace. P1 health proof must at least confirm Prime Core, Host identity, generation identity, hardware baseline and Prime Shell/recovery reachability according to the phase proof contract.

## Persistent-state rule

Host identity lives under `/var/lib/prime`. Generation rollback must not silently replace it. Profile/application/user/project data are versioned or migrated independently from generation identity.

## Automation

P1 does not enable unattended automatic update/apply by default. Prime Core is the policy authority that decides when a verified bootc deployment may be activated.
