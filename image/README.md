# Prime P1 image lane

This directory is the implementation home for the First Light bootable image.

Frozen foundation: `docs/adr/ADR-0001-P1-SYSTEM-FOUNDATION.md`.

The image lane will materialize:

- a digest-pinned Fedora 44 bootc build substrate;
- Prime-owned Rust binaries and Shell assets;
- `/usr/lib/prime/generation-seed.json` with exact source revision, pinned base-image digest and explicit boot-attempt policy;
- systemd units;
- UEFI/systemd-boot/UKI boot material;
- update-aware generation/recovery layout.

The produced Prime image digest is **not** embedded into the image-owned seed because that would be self-referential. After boot, Prime Core binds the seed to the immutable actual booted image digest reported by bootc and persists the completed `prime.generation.v1` record under `/var/lib/prime/generations/current.json`.

Do not accept a mutable image tag as final proof. The exact base input digest, produced image digest, source revision and booted deployment digest must be recorded/compared before a generation is promoted.
