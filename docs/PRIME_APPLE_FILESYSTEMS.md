# Prime OS — Apple Filesystem and Disk-Image Strategy

**Status:** planning authority supplement  
**Implementation:** not started  
**Parent authority:** `docs/PRIME_OS_MASTER_PLAN.md`  
**Storage authority:** `docs/PRIME_STORAGE_INTELLIGENCE.md`

Prime must treat Apple storage as an explicit filesystem family rather than leaving it hidden under `additional filesystem adapters`.

The Apple storage lane covers modern APFS, legacy HFS+/HFS, Apple-specific partition/container semantics, encrypted volumes, Time Machine-related storage, and common Apple disk-image formats used by development, recovery, repair, and migration workflows.

The permanent rule is:

`recognize broadly -> default safe/read-only where Prime lacks proven write support -> preserve Apple semantics -> add write capability only after independent proof`

Prime must never market or expose experimental APFS writes as ordinary safe filesystem support.

---

## 1. Apple storage classes

Prime Storage Intelligence must recognize at least:

- APFS containers and volumes;
- APFS encrypted volumes / FileVault-related encrypted storage where technically accessible;
- APFS volume groups and volume roles where discoverable;
- APFS snapshots;
- APFS sealed/system volumes where discoverable;
- HFS+ / Mac OS Extended;
- case-sensitive HFS+ variants;
- journaled HFS+ variants;
- legacy HFS / Mac OS Standard;
- GPT Apple partitions;
- legacy Apple Partition Map media where encountered;
- DMG / UDIF disk images;
- sparse image / sparse bundle style Apple disk images where supported by the selected image-provider path;
- APFS/HFS+ filesystems contained inside disk images;
- Time Machine volumes/images where their underlying format can be identified safely.

These are storage formats and containers, not Prime execution personalities.

---

## 2. APFS — first-class foreign filesystem target

**Role:** modern Apple filesystem, important for Mac/iPhone/iPad development, repair, recovery, migration, disk inspection, and later Darwin workflows.

Prime must understand that APFS differs materially from ext4/NTFS-style single-volume accounting.

Important semantics include:

- APFS container versus APFS volume identity;
- multiple volumes sharing one container's free space;
- copy-on-write metadata;
- file/directory clones;
- snapshots;
- sparse files;
- encryption;
- compression;
- extended attributes;
- case-sensitive and case-insensitive volumes;
- volume roles / system-data relationships where available;
- sealed system volumes and related integrity metadata where available;
- firmlinks/system-volume projection where relevant and supported;
- shared physical allocation that cannot be honestly attributed using naïve per-file summation.

### Prime accounting rule

For APFS, Prime must distinguish:

- container capacity;
- container free/available space;
- per-volume logical usage;
- shared container space;
- snapshot-retained storage;
- clone/shared-extent storage where it can be determined;
- metadata/unknown storage;
- encrypted/locked storage whose contents cannot yet be inspected.

Prime must not add every APFS volume's reported capacity together and pretend the result is physical disk capacity.

### Initial Prime access policy

APFS support is **read-only-first**.

Prime may initially provide:

1. partition/container recognition;
2. APFS volume discovery;
3. locked/unlocked state;
4. volume role and feature reporting where available;
5. read-only mount/extraction through a proven isolated provider;
6. read-only Storage Intelligence indexing;
7. snapshot/container accounting where the selected provider can prove it;
8. disk-image inspection;
9. explicit unsupported-feature reporting.

APFS write support must not become a P1/P2 assumption.

Any future APFS write path requires a separate proof campaign covering corruption, crash consistency, encryption, snapshots, cloning, power loss, fsck/recovery behavior, and cross-check against macOS-native results.

Until that exists, workloads requiring safe APFS modification can use an appropriate macOS/Apple Provider rather than Prime pretending experimental Linux write support is production-safe.

---

## 3. APFS donor/reference set

Prime should use multiple independent references rather than copying one implementation.

### Apple File System Reference / Apple documentation

Classification: **primary format/semantics reference**.

Use for:

- APFS structures and terminology;
- container/volume model;
- object/checkpoint concepts;
- snapshots/clones/space-sharing semantics;
- feature interpretation.

Do not assume the public specification covers every modern implementation detail; unsupported/unknown fields remain explicit.

### `libyal/libfsapfs`

Classification: **read-only parser/reference donor**.

Useful for:

- APFS v2 parsing;
- encryption handling supported by the library;
- extended attributes;
- compression handling;
- forensic/read-only access patterns;
- cross-platform test fixtures.

Important: its own project status is experimental and not every APFS feature is supported. Prime treats it as evidence/donor, not universal APFS truth.

### `sgan81/apfs-fuse`

Classification: **read-only mount/behavior donor**.

Useful for:

- FUSE-based read-only mount architecture;
- encrypted volumes;
- fusion-drive reading;
- snapshots/sealed-volume handling;
- DMG support;
- APFS structure-reading patterns.

Known limitations must remain visible, including unsupported firmlinks and incomplete compression support.

Its GPLv2 code is not copied into Prime Core by default.

### `linux-apfs/linux-apfs-rw`

Classification: **experimental research donor only**.

Useful for:

- Linux VFS/kernel integration patterns;
- APFS metadata mapping;
- snapshot/container behavior;
- understanding what a native Linux APFS driver requires.

Its write support is explicitly experimental and warns of possible corruption. Prime must not make it a normal write backend merely because the code exists.

### Future native Prime APFS adapter

Preferred long-term direction:

`Apple specifications + independent open implementations + real APFS fixtures -> Prime APFS contract -> native Rust read/inspection implementation where justified`

A native Prime APFS implementation should be pursued only where it produces clear correctness, performance, integration, or long-term ownership benefits over a safely isolated provider.

---

## 4. APFS encryption and FileVault-related volumes

Prime must distinguish filesystem support from authorization to decrypt.

Rules:

- encrypted APFS volumes remain `LOCKED` until the owner provides an authorized credential/key path;
- credentials pass through Prime's secret policy and are never logged in plaintext;
- unlocking does not automatically grant write access;
- unsupported T2/Secure-Enclave-dependent cases are reported honestly;
- failure to decrypt does not become filesystem corruption;
- Grid-Knight or other higher systems do not gain decryption secrets merely because they consume storage events.

Prime Storage Intelligence may report mechanical facts about a locked container/volume without claiming access to its contents.

---

## 5. APFS snapshots, clones, and shared space

Snapshots/clones make naïve disk analysis misleading.

Prime must support states such as:

- unique allocation known;
- shared allocation known;
- snapshot-retained allocation known;
- shared allocation estimated;
- physical attribution unavailable.

Prime must not claim that deleting a visible clone or file will reclaim its logical size.

Cleanup Planner must understand that snapshots can retain otherwise-deleted blocks.

Where snapshot ownership cannot be calculated reliably, the UI should say so rather than manufacturing reclaim estimates.

---

## 6. HFS+ / Mac OS Extended

**Role:** important legacy Apple filesystem for older Macs, external media, recovery, migration, and historical Time Machine storage.

Prime strategy:

- recognize HFS+ and its case-sensitive/journaled variants;
- use the Linux HFS+ driver or a deliberately isolated parser where appropriate;
- default to read-only for foreign/recovery media until write conditions are proven safe;
- preserve Unicode/name semantics;
- preserve resource forks, Finder metadata, extended attributes, and other Apple metadata where supported;
- expose journaling/dirty/locked state;
- never use a force-write option silently.

The Linux HFS+ driver exposes a `force` option for writing volumes marked journaled/locked and explicitly warns that this is at the user's risk. Prime must not make that the default behavior.

Apple's HFS Plus Volume Format documentation and Linux HFS+ implementation are primary donors for HFS+ semantics.

---

## 7. HFS / Mac OS Standard

**Role:** legacy recovery/interchange support only.

Prime should:

- recognize HFS;
- expose legacy metadata accurately where possible;
- default to read-only;
- preserve resource-fork/Finder metadata semantics during extraction where possible;
- avoid presenting HFS as a modern Prime writable filesystem.

HFS support is for historical media and recovery, not a preferred Prime storage format.

---

## 8. Mac disk-image formats

Apple workflows frequently package filesystems inside disk-image containers.

Prime should treat disk-image handling as a layer above the filesystem adapter:

`disk image -> partition/container discovery -> contained filesystem adapter -> Storage Intelligence`

Important classes include:

- `.dmg` / UDIF;
- sparse images;
- sparse bundles;
- raw images that contain GPT/APFS/HFS+;
- recovery/installer images.

Prime must not confuse the image container's file size with the logical/physical usage of the filesystem contained inside it.

Disk images should initially mount/extract read-only unless a specific write-capable image format/provider has been separately proven.

---

## 9. Time Machine awareness

Prime Storage Intelligence should recognize Time Machine-related storage as a special ownership/workload class when identifiable.

Possible underlying storage includes older HFS+ backup volumes and modern APFS-based backup layouts.

Prime must not treat backup snapshots/backup data as generic duplicates or safe cleanup candidates.

Default cleanup classification for recognized Time Machine backup data is `REVIEW` or `PROTECTED` according to ownership and active-backup state.

---

## 10. Prime Storage Intelligence integration

The Scan Engine becomes:

```text
Prime Storage Scanner
|
+-- Generic Linux/VFS scanner
+-- ext4 adapter
+-- Btrfs adapter
+-- XFS adapter
+-- NTFS adapter
+-- APFS adapter/provider
+-- HFS+ adapter
+-- legacy HFS adapter
+-- exFAT/FAT
+-- disk-image provider
+-- additional filesystem adapters
```

Not every filesystem adapter must be kernel-native.

Prime may use:

- kernel filesystem drivers;
- isolated userspace/FUSE providers;
- read-only parsers;
- remote/official platform providers;
- later native Rust implementations.

The backend is less important than presenting one truthful normalized storage model.

---

## 11. Proof fixtures

Apple-storage proof must eventually include at least:

### APFS

- single-volume APFS container;
- multiple volumes sharing container space;
- case-sensitive and case-insensitive volumes;
- sparse files;
- clones;
- snapshots;
- extended attributes;
- compressed files;
- encrypted volume with valid credentials;
- encrypted volume with invalid/no credentials;
- unsupported encryption feature;
- sealed/system volume where available;
- deleted file retained by snapshot;
- container free-space accounting;
- APFS inside DMG/image;
- clean read-only detach/unmount;
- malformed/corrupt image negative fixture.

### HFS+

- ordinary HFS+;
- case-sensitive HFS+;
- journaled HFS+;
- dirty/locked volume;
- resource forks;
- extended attributes/Finder metadata;
- Unicode filenames;
- sparse/large files where supported;
- HFS+ inside disk image.

### Legacy/image

- HFS read-only fixture;
- GPT Apple media;
- Apple Partition Map legacy fixture where available;
- DMG/UDIF;
- sparse image/bundle where supported;
- Time Machine sample structure where legally/technically available.

Reference results should be cross-checked against macOS-native tools and at least one independent read-only implementation when possible.

---

## 12. Roadmap placement

### P0 — Complete the Load

Freeze:

- Apple filesystem family classification;
- APFS container/volume model;
- APFS safe-access policy;
- APFS donor/reference matrix;
- HFS+/HFS strategy;
- Apple disk-image strategy;
- FileVault/encryption secret boundary;
- APFS snapshot/clone/shared-space metrics;
- Apple metadata preservation rules;
- Apple-storage proof fixtures.

### P1 — First Light

Prime Storage Inventory should at minimum:

- detect Apple partition/filesystem signatures safely;
- identify APFS/HFS+/HFS as foreign filesystems;
- display locked/encrypted state where detectable;
- avoid destructive automatic mounting;
- expose honest unsupported/read-only status.

P1 does not require APFS write support.

### P1.5 — Survival

Prove that Prime update/recovery workflows do not accidentally alter attached foreign Apple volumes.

Apple volumes used only for inspection should remain read-only throughout survival tests.

### P2 — Development Body

Add useful read-only Apple storage workflows:

- APFS container/volume discovery;
- read-only mount/extraction provider;
- HFS+ read-only integration;
- disk-image inspection;
- Storage Intelligence indexing;
- Apple metadata preservation;
- encryption/unlock workflow where supported and authorized.

### P3 — Origins Factory

Origins can project Apple storage into developer/repair missions without taking ownership of the Prime Host storage index.

Examples:

- inspect Mac development disk;
- recover files;
- compare project artifacts across Prime/macOS storage;
- prepare migration;
- attach disk-image evidence.

### Later

Research only when justified by real workload evidence:

- native Rust APFS acceleration/parser;
- deeper snapshot/clone physical accounting;
- safe writable HFS+ if required;
- APFS write support only after a dedicated corruption/recovery proof campaign;
- tighter Darwin/macOS Provider integration.

---

## 13. Non-goal

Prime's goal is not to become the world's most aggressive APFS write driver.

Prime's goal is to understand Apple storage correctly, inspect it safely, preserve its semantics, integrate it into developer/repair workflows, and only write when a backend has earned that level of trust.
