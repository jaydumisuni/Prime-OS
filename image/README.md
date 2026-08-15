# Prime P1 image lane

This directory is the implementation home for the First Light bootable image.

Frozen foundation: `docs/adr/ADR-0001-P1-SYSTEM-FOUNDATION.md`.

The image lane will materialize:

- a digest-pinned Fedora 44 bootc build substrate;
- Prime-owned Rust binaries and Shell assets;
- `/usr/lib/prime/generation.json` with exact source revision and immutable image digest;
- systemd units;
- UEFI/systemd-boot/UKI boot material;
- update-aware generation/recovery layout.

Do not accept a mutable image tag as final proof. The exact base and produced image digests must be recorded before a generation is promoted.
