# Prime Generation Seed v1

Status: **FROZEN FOR P1 IMPLEMENTATION**

Schema identifier: `prime.generation-seed.v1`

## Purpose

A Prime bootable image cannot embed its own final OCI manifest digest in an image-owned file: changing that file changes the produced image digest and makes the value self-referential.

Prime therefore separates **image-authored generation provenance** from **runtime-observed generation identity**.

The image carries a generation seed. After boot, Prime Core binds that seed to the immutable booted image digest reported by bootc and produces the completed `prime.generation.v1` record.

## Image-owned seed

Path:

```text
/usr/lib/prime/generation-seed.json
```

Required document:

```json
{
  "schema": "prime.generation-seed.v1",
  "generation_id": "prime-gen-<uuidv7>",
  "channel": "LAB|CANDIDATE|STABLE",
  "created_at": "RFC3339",
  "source_revision": "git-sha",
  "base_image_digest": "sha256:...",
  "boot_attempt_limit": 3
}
```

Rules:

- `generation_id` is opaque and non-empty;
- `source_revision` is non-empty and identifies the Prime source revision used to build the image;
- `base_image_digest` is the immutable digest of the pinned input/base image used by the build; it is **not** the final produced Prime image digest;
- `boot_attempt_limit` is explicit image policy and must be greater than zero;
- no mutable tag is accepted as the seed's base-image identity;
- the seed does not contain a guessed or post-hoc final image digest.

## Runtime bootc binding

Prime obtains mechanical booted-deployment identity through the stable programmatic bootc status interface:

```text
bootc status --format=json --format-version=1
```

For the bootc v1 `BootcHost` schema, Prime requires:

```text
status.booted.image.imageDigest
status.booted.image.architecture
```

The upstream schema explicitly allows `status.booted` and `BootEntry.image` to be absent. Prime fails closed when either is absent, when the boot entry is marked incompatible, when the digest is not canonical SHA-256, or when the reported image architecture does not mechanically match the Prime Host architecture.

Prime parses only the fields it owns semantically and ignores additive bootc fields.

## Completed generation record

The completed runtime record remains `prime.generation.v1`:

```json
{
  "schema": "prime.generation.v1",
  "generation_id": "prime-gen-<uuidv7>",
  "image_digest": "sha256:<actual-booted-manifest-digest>",
  "channel": "LAB|CANDIDATE|STABLE",
  "created_at": "RFC3339",
  "source_revision": "git-sha",
  "state": "BOOT_TRY",
  "boot_attempts_remaining": 3,
  "evidence_refs": ["bootc.status.v1"]
}
```

The first successful binding starts in `BOOT_TRY`; reaching userspace does not make a generation `KNOWN_GOOD`.

## Persistent binding

Prime persists the bound current record under:

```text
/var/lib/prime/generations/current.json
```

Rules:

- a persisted record is reused only when its generation ID and observed bootc image digest exactly match the current seed/deployment;
- a corrupt persisted record fails closed;
- a persisted record whose generation ID or image digest conflicts with the current booted seed/deployment fails closed rather than silently resetting state;
- later state transitions may preserve/update the record, but may not replace its bound image digest with a mutable tag;
- Host identity remains separate under `/var/lib/prime/identity`.

## Proof boundary

Hosted parser/tests prove contract handling and fail-closed binding. They do not prove that a Prime image has actually booted through UEFI/systemd-boot/UKI or that bootc's deployment state on Kratos matches the build artifact. Those require physical Host proof.