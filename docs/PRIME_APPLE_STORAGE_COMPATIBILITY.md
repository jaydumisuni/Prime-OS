# Prime OS — Apple Storage Compatibility

**Status:** planning authority supplement  
**Implementation:** not started  
**Parent authority:** `docs/PRIME_OS_MASTER_PLAN.md`  
**Storage authority:** `docs/PRIME_STORAGE_INTELLIGENCE.md`

Prime must understand modern and legacy Apple storage without pretending that Linux currently provides production-grade native write support for every Apple filesystem.

This supplement closes the Apple-storage gap in Prime Storage Intelligence.

The permanent rule is:

`Apple format -> detect honestly -> use safest proven access backend -> normalize storage truth -> expose exact capability/limitations`

Prime must never turn `recognized` into `safe read/write` merely because a parser or experimental driver exists.

---

## 1. Apple storage formats Prime must understand

### First-class

- APFS / Apple File System
- HFS+ / Mac OS Extended
- HFS+ Journaled
- HFS+ Case-sensitive variants
- APFS case-sensitive variants
- APFS encrypted variants

### Legacy / compatibility

- HFS / Mac OS Standard

### Related image/container formats

These are not filesystems themselves but matter to Apple development, recovery and interchange:

- DMG / UDIF disk images
- sparse images / sparse bundles where implementation evidence supports them
- GPT partitioning around Apple storage

Prime should classify the image/container separately from the filesystem inside it.

---

## 2. APFS is not a normal one-volume filesystem

Prime Storage Intelligence must model APFS at multiple levels:

```text
physical device / image
        |
        v
APFS container
        |
        +-- volume
        +-- volume
        +-- volume group
        +-- snapshots
        +-- shared free space
```

Important APFS semantics include:

- container-level space sharing;
- multiple logical volumes sharing one physical capacity pool;
- snapshots;
- copy-on-write metadata;
- file/directory cloning;
- sparse files;
- encryption;
- case-sensitive and case-insensitive variants;
- volume roles;
- modern macOS System/Data volume groups;
- booting modern macOS from a system-volume snapshot.

Prime must therefore avoid showing each APFS volume as though it independently owns the full free space of its container.

---

## 3. APFS accounting model

Prime Storage Intelligence should expose at least:

- physical store identity;
- APFS container identity;
- container capacity;
- container allocated/used space;
- container available/unallocated space;
- volume identity;
- volume role where known;
- volume-group membership where known;
- per-volume referenced/logical usage where trustworthy;
- snapshot usage where trustworthy;
- clone/shared-space status where detectable;
- encryption state;
- case-sensitivity state;
- source/confidence for every advanced metric.

The UI must distinguish:

```text
CONTAINER FREE SPACE
!=
VOLUME LOGICAL USAGE
!=
EXCLUSIVE PHYSICAL OWNERSHIP
```

APFS shared space, clones and snapshots make naive per-directory physical ownership misleading.

Prime must prefer `UNKNOWN/SHARED` over fabricated precision.

---

## 4. APFS access strategy on Prime

### Phase A — recognition and safe inspection

Prime should first support:

- GPT/APFS-container detection;
- APFS container and volume inventory;
- read-only metadata inspection;
- volume names/UUIDs/roles where available;
- encryption-state detection;
- snapshots/volume groups where the selected backend can prove them;
- read-only filesystem access when a proven backend supports the target.

### Phase B — read-only mounted/interchange support

Read-only APFS should be the default initial capability on Prime unless a specific backend and fixture matrix proves more.

Potential donors/references:

1. **Apple File System Reference** — format authority/reference.
2. **linux-apfs/linux-apfs-rw** — Linux kernel-module implementation research; contains experimental write support and must not be treated as production-safe merely because writes exist.
3. **sgan81/apfs-fuse** — read-only FUSE implementation; useful for container/volume, encrypted-volume, DMG and snapshot behavior research.
4. **libyal/libfsapfs** — inspection/forensics-oriented APFS library and format documentation; useful as an independent parser/reference oracle.
5. **linux-apfs/apfsprogs** — experimental APFS user-space tooling and fsck/mkfs/snapshot research donor.

### Phase C — write support only after dedicated proof

Prime must not make APFS read/write a baseline promise.

A future write path may be considered only after:

- upstream/donor maturity review;
- exact APFS feature matrix;
- encrypted/unencrypted fixtures;
- snapshots/clones/space-sharing fixtures;
- dirty/crash recovery testing;
- cross-check on real macOS-created media;
- destructive negative testing on disposable images/devices;
- rollback/recovery proof;
- Sergeant engineering review;
- explicit owner acceptance.

Until then Prime reports APFS write capability honestly as unavailable/experimental rather than exposing unsafe writes by default.

---

## 5. APFS encryption

Prime must represent encryption separately from filesystem recognition.

Possible states:

```text
APFS_RECOGNIZED_LOCKED
APFS_RECOGNIZED_UNLOCKABLE_WITH_SUPPORTED_PROVIDER
APFS_READ_ONLY_UNLOCKED
APFS_UNSUPPORTED_ENCRYPTION_MODE
```

Credentials/keys must remain within Prime secret policy and never be retained in storage-index logs or normal evidence output.

Hardware-bound Apple encryption modes that third-party Linux tooling cannot support must fail explicitly rather than falling back to guesswork.

---

## 6. APFS snapshots, clones and volume groups

Prime must treat these as native accounting concepts, not unusual edge cases.

### Snapshots

A snapshot can keep blocks alive even when ordinary directory traversal suggests the data is gone.

Prime should eventually surface:

- snapshot identity;
- creation time where available;
- role/source;
- retained-space contribution where the backend can prove it;
- whether a snapshot is boot/system critical;
- cleanup protection state.

Prime must not delete snapshots merely because they consume significant storage.

### Clones

Cloned files/directories can share physical storage.

Prime should distinguish logical duplicated content from exclusive physical allocation where the backend can prove sharing.

### Volume groups

Modern macOS uses linked System/Data volume groups and supporting Preboot/VM/Recovery volumes.

Prime must preserve those relationships in inventory so a macOS installation is not presented as unrelated independent volumes.

For recovery/inspection, Prime should be able to show the group coherently even if it cannot mount every role read/write.

---

## 7. HFS+ / Mac OS Extended

**Role:** first-class legacy Apple filesystem/interchange target.

Linux already contains an in-kernel HFS+ implementation. Prime should use the kernel/VFS path as the default mounted-filesystem interface rather than inventing a new raw parser for ordinary access.

Prime should recognize at least:

- HFS+;
- HFS+ Journaled;
- case-sensitive HFS+;
- clean/unclean volume state where available;
- extended attributes/resource forks where supported by the Linux path.

Safety rule:

- read-only is always acceptable when the kernel/backend requires it;
- Prime must respect the kernel's refusal to enable writes on unsafe/unclean/locked/journaled cases rather than forcing a mount;
- write capability should be advertised only for configurations proven safe by the selected Linux HFS+ path and Prime fixture matrix.

The Linux HFS+ implementation itself is a donor/reference for mounted support and metadata semantics.

---

## 8. Legacy HFS / Mac OS Standard

**Role:** compatibility/read-mostly historical format.

Prime should support:

- detection;
- safe read access where the Linux HFS driver supports it;
- extraction/copy-out;
- metadata visibility;
- disk/image inspection.

Prime should not make legacy HFS a preferred writable Prime filesystem.

If write support exists in a backend, it remains a separately proven capability rather than the default user promise.

---

## 9. Apple disk images

Prime development/recovery work will encounter Apple disk images even when the underlying filesystem is APFS or HFS+.

Prime should separate:

```text
image/container format
        ↓
partition map
        ↓
filesystem
```

Example:

```text
DMG/UDIF
→ GPT
→ APFS container
→ APFS volumes
```

A future Apple Image Adapter may support:

- read-only DMG inspection/mounting;
- compressed-image handling where proven;
- encrypted-image handling through secret policy;
- conversion/export to a neutral raw image for analysis;
- sparse image/bundle inspection where supported.

APFS FUSE and other Apple-storage donors may be used as behavior references, but Prime should prefer a clean native interface/adapter boundary rather than embedding a foreign application UX.

---

## 10. Prime Storage Intelligence integration

The filesystem adapter tree becomes:

```text
Prime Storage Scanner
|
+-- Generic Linux/VFS scanner
+-- ext4 adapter
+-- Btrfs adapter
+-- XFS adapter
+-- NTFS adapter
+-- APFS adapter
+-- HFS+ adapter
+-- HFS legacy adapter
+-- exFAT/FAT adapter
+-- other filesystem adapters
```

APFS may initially be an inspection/read-only provider rather than a normal Linux-mounted VFS path.

The normalized Storage Index must not care whether information came from:

- kernel VFS;
- FUSE;
- a read-only parser;
- a filesystem-specific helper;
- a remote macOS Provider.

It cares about normalized, capability-labelled mechanical truth.

---

## 11. Remote/official macOS provider integration

Prime may also obtain Apple storage truth from a real macOS Provider when that is the safest or most complete path.

Example:

```text
Prime/Origins
→ authenticated macOS Provider
→ diskutil / native Apple filesystem APIs
→ normalized Prime storage capability/evidence
```

This does not make Prime dependent on macOS for ordinary storage operations.

It provides a high-confidence path for Apple-only details, validation and cross-checking.

---

## 12. Apple filesystem proof fixtures

Prime Storage Intelligence should eventually prove:

### APFS

- single-volume container;
- multi-volume shared-space container;
- case-sensitive volume;
- case-insensitive volume;
- encrypted volume with supported unlock path;
- unsupported/locked encryption mode;
- snapshots;
- cloned files/directories;
- sparse files;
- volume groups;
- System/Data relationship;
- Preboot/Recovery/VM roles;
- boot snapshot/system snapshot recognition where available;
- DMG containing APFS;
- read-only extraction;
- corrupted/disposable image negative tests;
- no unsafe automatic write enablement.

### HFS+

- ordinary HFS+;
- journaled HFS+;
- case-sensitive HFS+;
- clean and unclean volumes;
- resource forks/extended attributes where supported;
- read/write capability reporting;
- safe fallback to read-only.

### HFS

- detection;
- read/copy-out;
- legacy metadata;
- rejection of unsafe/unsupported writes.

Reference results should be cross-checked against native macOS tools where possible. Reference tools are evidence, not infallible authority.

---

## 13. Roadmap placement

### P0 — Complete the Load

Freeze:

- Apple filesystem taxonomy;
- APFS container/volume/group accounting model;
- APFS donor matrix and write-safety boundary;
- HFS+ kernel/VFS strategy;
- HFS legacy strategy;
- Apple image/container adapter boundary;
- encryption/secret handling boundary;
- Apple filesystem fixtures and proof matrix.

### P1 — First Light

Required only for basic detection/inventory where practical:

- identify Apple-formatted attached storage;
- show filesystem/container type;
- protect unknown/unsupported Apple storage from accidental write operations;
- expose clear capability state.

APFS full read support is not a First Light blocker unless specifically promoted by P0 evidence.

### P1.5 — Survival

Prove Prime update/recovery logic does not accidentally consume or modify foreign Apple volumes.

### P2 — Development Body

Add mature read-only Apple-storage inspection and extraction providers where selected by P0.

### P3 Origins and later

Expose Apple storage to Origins as a capability; allow project/recovery workflows to use local Apple media or a native macOS Provider.

### Later

Research higher-performance APFS indexing and write support only when evidence justifies promotion.

---

## 14. Donor disposition

| Donor | Prime use | Initial disposition |
|---|---|---|
| Apple File System Reference / Apple docs | format and semantic authority | ADOPT AS REFERENCE |
| linux-apfs/linux-apfs-rw | kernel design + experimental write research | STUDY / ADAPT PATTERNS |
| linux-apfs/apfsprogs | user-space tooling, fsck/mkfs/snapshot research | STUDY |
| sgan81/apfs-fuse | read-only APFS, encryption, image/snapshot behavior | STUDY / REFERENCE ORACLE |
| libyal/libfsapfs | independent parser/forensic validation | STUDY / REFERENCE ORACLE |
| Linux `fs/hfsplus` | HFS+ mounted filesystem support | ADOPT KERNEL CAPABILITY / STUDY |
| Linux `fs/hfs` | legacy HFS compatibility | ADOPT KERNEL CAPABILITY / STUDY |

No donor is allowed to upgrade itself from experimental to production merely because it compiles or mounts one fixture.

---

## 15. Final Apple-storage principle

Prime should eventually be able to attach a disk or image and say something honest like:

```text
APPLE STORAGE DETECTED

Container: APFS
Volumes: 5
Volume group: macOS System + Data
Snapshots: 3 detected
Encryption: Data volume locked
Read support: AVAILABLE
Write support: NOT PROVEN / DISABLED

Safe actions:
- inspect
- unlock with supported credential provider
- copy/extract
- create evidence image

Unavailable until proven:
- modify APFS filesystem
- delete snapshots
- repair filesystem
```

That is preferable to either pretending Apple storage is unsupported or exposing experimental write capability as if it were safe.