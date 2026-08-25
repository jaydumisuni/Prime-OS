# Prime Recovery Entry v1

Status: **IMPLEMENTED P1 CANDIDATE — LIVE PROOF REQUIRED**

Authority: `docs/adr/ADR-0001-P1-SYSTEM-FOUNDATION.md` and `docs/PRIME_OS_ROADMAP.md`

## Purpose

P1 must retain a recovery path that does not depend on Prime Shell or the Prime compositor. This contract freezes the narrow First-Light implementation used to satisfy that requirement without expanding into P1.5 rollback automation.

## Boot contract

The P1 image contains two Boot Loader Specification Type #2 UKIs under `/EFI/Linux/`:

- the normal First-Light UKI;
- one recovery UKI with a `.recovery.efi` filename suffix.

Both UKIs contain the same Prime kernel and initramfs and, after the final image reseal, the same canonical Composefs root digest.

The normal UKI command line must **not** contain recovery selectors.

The recovery UKI command line must contain both:

```text
systemd.unit=prime-recovery.target
prime.recovery=1
```

The normal UKI embeds Prime OS product identity with `VERSION_ID="0.1"`.

The recovery UKI embeds `PRETTY_NAME="Prime OS Recovery"` and a lower `VERSION_ID="0.0"`. This keeps the ordinary First-Light entry ahead of recovery under Boot Loader Specification version ordering when the entries otherwise belong to the same Prime installation. The normal-boot QEMU proof remains the mechanical guard: if recovery becomes the automatic default, `primed` will not establish the expected normal Host state and the proof fails.

No boot-loader default is silently stored in firmware by the image to force this result.

## Recovery target

The recovery UKI selects:

```text
prime-recovery.target
```

That target requires local filesystems and `prime-recovery.service`, conflicts with the graphical target, and does not require Prime Shell or the compositor.

`prime-recovery.service` runs `/usr/libexec/prime/prime-recovery` on `tty1` only when `prime.recovery=1` is present on the kernel command line.

The service treats `/var/lib/prime` as read-only. P1 recovery does not silently mutate Host identity, generation state, hardware state, storage state, or project/user data.

## Recovery console

`prime-recovery` reads bounded persisted state when available:

- `/var/lib/prime/identity/host.json`;
- `/var/lib/prime/generations/current.json`;
- `/var/lib/prime/hardware/current.json`;
- `/var/lib/prime/storage/current.json`.

Missing, corrupt, unreadable, or oversized state is reported as a limitation and must not prevent the recovery console from starting.

The P1 console exposes only:

- refresh/read state;
- JSON status output;
- reboot;
- power off.

It does **not** claim a P1 rollback/update mutation UI. Transactional rollback, failure injection, candidate-health recovery, and exhaustive generation survival remain P1.5 work.

## Product/substrate identity

Prime owns product identity:

```text
ID=prime
PRETTY_NAME="Prime OS P1 First Light"
```

Fedora 44 remains only the pinned P1 package/kernel substrate. Substrate provenance is retained separately at:

```text
/usr/lib/prime/substrate-release.json
```

The substrate record contains the Fedora identity/version and the exact pinned base-image digest. Runtime product identity must not fall back to Fedora branding merely because Fedora supplies the substrate.

## Hosted/local proof obligations

Before this candidate may be described as proven, `tools/prove-p1-local.sh` must establish against one frozen commit that:

1. locked Rust metadata, formatting, Clippy and workspace tests pass;
2. `primed` and `prime-recovery` release binaries exist;
3. the final image identifies as Prime OS while retaining exact Fedora substrate provenance;
4. exactly one normal and one recovery UKI are present;
5. both UKIs contain the final canonical Composefs digest;
6. the normal UKI lacks recovery selectors;
7. the recovery UKI contains both recovery selectors and the recovery product label;
8. the built disk contains both UKIs in a Boot Loader Specification location;
9. default QEMU/OVMF boot reaches normal Prime Core and persists Host/generation/hardware state.

## Physical proof obligations

P1 still requires physical HP 290 G4 / Kratos evidence. At minimum:

- normal UEFI boot reaches Prime rather than Fedora product identity;
- the ordinary boot path reaches Prime Core and Host state;
- the recovery entry is visible/selectable through the installed boot path;
- selecting recovery reaches the recovery console without Prime Shell/compositor;
- missing/corrupt optional persisted state is reported rather than turning recovery into false success;
- reboot/poweroff controls behave as claimed;
- recovery does not silently modify protected Prime state.

Until that physical evidence exists, this document records an **implemented candidate**, not physical acceptance.

## Non-claims

This contract does not claim:

- Secure Boot signing;
- automatic rollback;
- P1.5 survival completion;
- Prime Shell/compositor completion;
- numeric rollback/recovery reserve policy acceptance;
- HP/Kratos physical acceptance.
