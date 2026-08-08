# Donor Record — APFS and Apple Storage

**Prime subsystem:** Prime Storage Intelligence  
**Status:** P0 donor study / planning authority  
**Code integration:** no direct Prime Core reuse approved

## Purpose

Prime needs safe support for modern and legacy Apple storage used by Mac/iOS development, repair, recovery, migration, and disk-image workflows.

This donor lane covers APFS, HFS+, HFS, and Apple disk-image/container formats.

## Primary references

### Apple APFS documentation / Apple File System Reference

Use as the primary terminology and format-semantics reference for APFS containers, volumes, snapshots, clones, space sharing, encryption, and related features.

Disposition: `STUDY / SPECIFICATION REFERENCE`.

### `libyal/libfsapfs`

Repository: `https://github.com/libyal/libfsapfs`

Observed role:

- library/tools for Apple File System access;
- experimental project status;
- read-only APFS v2 support;
- supported feature work includes encryption, extended attributes, and several compression methods;
- known unsupported areas include APFS v1, snapshots, T2 encryption, Fusion and some compression modes according to its current README.

Licence reported by repository: `LGPL-3.0-or-later` for the library, with GPL tooling present in the repository.

Disposition: `STUDY / READ-ONLY REFERENCE / POSSIBLE ISOLATED PROVIDER AFTER LICENCE REVIEW`.

Do not treat it as complete APFS truth.

### `sgan81/apfs-fuse`

Repository: `https://github.com/sgan81/apfs-fuse`

Observed role:

- read-only FUSE APFS driver;
- software-encrypted volumes;
- Fusion-drive reading;
- snapshots and sealed volumes;
- DMG support;
- some compression support.

Known limitations include unsupported firmlinks and incomplete compression handling.

Licence: GPL-2.0.

Disposition: `STUDY / BEHAVIOR REFERENCE / OPTIONAL ISOLATED READ-ONLY PROVIDER ONLY IF DELIBERATELY ACCEPTED`.

Do not merge its GPLv2 code into Prime Core by default.

### `linux-apfs/linux-apfs-rw`

Repository: `https://github.com/linux-apfs/linux-apfs-rw`

Observed role:

- Linux APFS kernel module;
- read-only by default;
- experimental write support;
- repository explicitly warns that write support can corrupt containers;
- encryption support is limited.

Disposition: `RESEARCH DONOR ONLY` for Linux VFS integration and APFS-driver architecture.

Prime must not enable its experimental write support as ordinary safe Prime functionality.

## HFS+/HFS references

Primary references:

- Apple's HFS Plus Volume Format documentation (TN1150);
- Linux `hfsplus` filesystem implementation/documentation;
- Linux `hfs` filesystem implementation/documentation;
- libyal `libfshfs` or similar read-only forensic implementations where useful.

Disposition: `STUDY / KERNEL-MEDIATED READ PATH / READ-ONLY-FIRST FOR FOREIGN MEDIA`.

## Prime implementation direction

Preferred sequence:

`Apple specifications + several independent parsers/drivers + real APFS/HFS fixtures -> Prime Apple Storage contract -> safe normalized provider interface -> native Rust implementation only where justified`

Prime must keep filesystem support truth separate from implementation backend.

The first useful Prime capability is safe recognition/read/inspection, not risky write support.

## Reference-oracle role

Prime may cross-check the same Apple test images against:

- macOS native tools;
- `libfsapfs` where supported;
- `apfs-fuse` where supported;
- Linux HFS+/HFS paths;
- later additional independent implementations.

Disagreement triggers investigation. No one donor is infallible.

## Write-support gate

Any APFS write backend requires dedicated evidence for:

- ordinary writes;
- create/delete/rename;
- clones;
- snapshots;
- encryption;
- crash consistency;
- interrupted/power-loss writes;
- corruption detection;
- fsck/recovery behavior;
- cross-check on macOS;
- rollback/recovery of test data.

Until that is proven, Prime remains read-only for APFS and uses an appropriate macOS/Apple Provider when safe APFS modification is required.
