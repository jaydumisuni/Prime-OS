# Prime P1 image lane

This directory is the implementation home for the First Light bootable image.

Frozen foundation: `docs/adr/ADR-0001-P1-SYSTEM-FOUNDATION.md`.

Current recovery candidate: `docs/contracts/PRIME_RECOVERY_ENTRY_V1.md`.

The image lane materializes:

- a digest-pinned Fedora 44 bootc build **substrate**;
- Prime OS product identity independent of Fedora branding;
- explicit substrate provenance at `/usr/lib/prime/substrate-release.json`;
- Prime-owned Rust binaries, including `primed` and the shell-independent P1 recovery console;
- `/usr/lib/prime/generation-seed.json` with exact source revision, pinned base-image digest and explicit boot-attempt policy;
- systemd units;
- UEFI/systemd-boot/UKI boot material;
- one normal First-Light UKI and one recovery UKI;
- update-aware generation/recovery layout.

Fedora remains the package/kernel source for P1, not Prime's runtime product identity. Package installation occurs while Fedora's build-time identity is present; the final image then installs Prime's `/usr/lib/os-release` and retains the exact Fedora base identity/digest separately as substrate provenance.

The normal and recovery UKIs use the same kernel, initramfs and canonical Composefs root digest. Recovery adds `systemd.unit=prime-recovery.target prime.recovery=1` and boots a tty recovery console without requiring Prime Shell or the compositor.

The produced Prime image digest is **not** embedded into the image-owned seed because that would be self-referential. After normal boot, Prime Core binds the seed to the immutable actual booted image digest reported by bootc and persists the completed `prime.generation.v1` record under `/var/lib/prime/generations/current.json`.

Do not accept a mutable image tag as final proof. The exact base input digest, produced image digest, source revision, booted deployment digest, normal UKI seal and recovery UKI seal must be recorded/compared before a generation is promoted.

The recovery candidate is not physically accepted until HP 290 G4 / Kratos proves the installed normal and recovery paths.
