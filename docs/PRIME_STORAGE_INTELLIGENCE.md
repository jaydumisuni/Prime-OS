# Prime Storage Intelligence

**Status:** planning authority supplement  
**Implementation:** not started  
**Parent authority:** `docs/PRIME_OS_MASTER_PLAN.md`  
**Roadmap:** `docs/PRIME_OS_ROADMAP.md`

Prime Storage Intelligence is the OS-native storage observability, accounting, indexing, and cleanup-planning layer for Prime OS.

It is derived partly from WinDirStat's proven disk-analysis patterns, but Prime will not embed WinDirStat's GPLv2 C++ application into Prime Core. WinDirStat is used as a behavior, architecture, edge-case, and UX donor/reference oracle. Prime's implementation is native Rust around Linux/VFS and filesystem-specific interfaces.

The permanent rule is:

`WinDirStat -> study behavior/architecture/UX -> Prime Storage Intelligence specification -> native Rust implementation -> Prime OS`

---

## 1. Ownership boundary

Prime Storage Intelligence owns mechanical storage truth:

- mounted storage inventory;
- filesystem and mount identity;
- total/free/available/reserved accounting;
- file and directory indexing;
- logical size;
- allocated size;
- hardlink identity;
- sparse-file awareness;
- extent information where safely available;
- filesystem-specific shared/exclusive accounting where supported;
- change events;
- duplicate candidates;
- Prime-owned storage classification;
- cleanup safety classification;
- update/recovery space preflight;
- storage-pressure evidence.

It does not decide malware/security meaning. Grid-Knight may later consume file/storage events and hashes, but Grid-Knight owns threat interpretation, response, remediation, and false-positive policy.

Hunter may explain storage findings but does not decide whether protected Prime state is safe to delete.

Origins may consume project/mission storage projections but does not own the Host storage index.

---

## 2. Architecture

```text
Prime Storage Intelligence
|
+-- Storage Inventory
|   +-- block devices
|   +-- partitions
|   +-- filesystems
|   +-- mounts
|   +-- capacity/free/available/reserved
|
+-- Storage Index
|   +-- files/directories
|   +-- logical size
|   +-- allocated size
|   +-- filesystem + mount identity
|   +-- inode/file identity
|   +-- hardlinks
|   +-- sparse ranges
|   +-- timestamps/attributes
|   +-- hashes when requested
|
+-- Scan Engine
|   +-- Generic VFS scanner
|   +-- ext4 enrichment
|   +-- Btrfs enrichment
|   +-- XFS enrichment
|   +-- NTFS enrichment
|   +-- additional filesystem adapters
|
+-- Change Engine
|   +-- filesystem-wide event source where supported
|   +-- scoped fallback watcher
|   +-- incremental index updates
|
+-- Duplicate Engine
|
+-- Storage Ownership
|   +-- Prime generations
|   +-- rollback/recovery reserve
|   +-- user/project data
|   +-- Origins
|   +-- builds/toolchains
|   +-- containers
|   +-- VMs
|   +-- downloads
|   +-- caches/temp
|   +-- future Grid-Knight quarantine
|
+-- Cleanup Planner
|
+-- Storage Event Interface
|
+-- Prime Shell Storage UI
    +-- overview
    +-- ownership
    +-- treemap
    +-- largest files
    +-- duplicates
    +-- projects/builds
    +-- VMs/containers
    +-- cleanup
```

---

## 3. Generic Linux/VFS scanner

The generic scanner is the baseline for any mounted filesystem Prime can safely enumerate.

Use stable Linux/VFS interfaces instead of parsing live filesystem structures directly:

- mount discovery from Linux mount information and stable mount identifiers;
- `getdents64`/directory iteration for efficient enumeration;
- `statx` for file type, inode, link count, logical size, allocated blocks, timestamps, mount identity, and supported filesystem-specific metadata;
- `FIEMAP` for extent reporting where needed and supported;
- `SEEK_DATA` / `SEEK_HOLE` for sparse-file data/hole discovery where supported;
- safe dirfd/open-at style traversal to reduce path races and symlink surprises;
- filesystem events through `fanotify` where suitable, with scoped `inotify` fallback when necessary.

Direct reads of live mounted filesystem metadata from raw block devices are not the default Prime scanner path.

Prime must prefer kernel-mediated filesystem truth for mounted read/write filesystems.

---

## 4. Storage metrics must not lie

Prime must distinguish several different meanings of "size".

At minimum:

- `logical_bytes`: user-visible file length;
- `allocated_bytes`: blocks allocated to the file as reported by the filesystem/kernel;
- `data_bytes`: observed data ranges where sparse-file interfaces support it;
- `exclusive_physical_bytes`: only when the filesystem can support a trustworthy calculation;
- `shared_physical_bytes`: only when shared/reflink/snapshot ownership can be proven;
- `filesystem_used_bytes`: whole-filesystem usage;
- `filesystem_available_bytes`: space available under current filesystem policy;
- `filesystem_reserved_bytes`: space unavailable to ordinary allocation or reserved for system/filesystem operation;
- `unknown_or_metadata_bytes`: filesystem usage not honestly attributable to indexed user files.

Prime must not present a guessed physical number as exact.

This is especially important on Btrfs/XFS reflinks, snapshots, compression, sparse files, delayed allocation, hardlinks, and filesystems with significant metadata/reserved space.

---

## 5. Hardlink accounting

Prime identifies hardlinked files by filesystem/mount identity plus stable file identity/inode information.

The index must distinguish:

- path count;
- link count;
- one underlying file identity;
- logical path totals;
- physical allocated total without double counting the same hardlinked inode.

A directory view may intentionally show logical path ownership while the filesystem-wide physical view avoids duplicate hardlink allocation.

Both truths must be labelled.

---

## 6. Sparse-file accounting

Prime must understand files where logical size is much larger than allocated data.

Use allocated-block metadata as the inexpensive baseline and `FIEMAP`/`SEEK_DATA`/`SEEK_HOLE` when deeper range-level analysis is requested or needed.

Never infer that written zeroes are necessarily holes.

---

# Filesystem strategy

## 7. ext4

**Role:** first-class Prime local filesystem target.

Initial strategy:

- Generic VFS enumeration;
- `statx` logical/allocated metadata;
- `FIEMAP` for extent detail when required;
- `SEEK_DATA` / `SEEK_HOLE` for sparse files;
- filesystem capacity/reserved-space reporting;
- quota/project ownership integration where configured;
- filesystem event integration through the common Change Engine.

Prime should not parse ext4 raw metadata on a mounted read/write filesystem for ordinary storage accounting.

The ext4 on-disk format and e2fsprogs/libext2fs are useful recovery/offline-analysis donors, not a reason to bypass the live kernel filesystem implementation.

A later offline/recovery scanner may use ext4-specific raw metadata tooling on an unmounted/read-only target where doing so is deliberately proven safe.

## 8. Btrfs

**Role:** first-class CoW/snapshot-aware Prime filesystem target.

Btrfs requires more than ordinary directory totals because snapshots, subvolumes, reflinks, compression, and shared extents can make physical ownership ambiguous.

Initial strategy:

- Generic VFS enumeration;
- `statx`, including subvolume identity when available;
- `FIEMAP` for extent information;
- Btrfs ioctls for subvolume/filesystem-specific enrichment where needed;
- qgroup information when enabled and healthy;
- explicit shared/exclusive accounting states;
- snapshot/subvolume awareness;
- compression/reflink awareness;
- Btrfs-specific capacity/data/metadata/system presentation where available.

Important rule: qgroup data is useful but cannot be treated as universal truth when quotas are disabled or the qgroup state is inconsistent.

Prime must record confidence/source with shared/exclusive Btrfs accounting instead of manufacturing precision.

## 9. XFS

**Role:** first-class high-performance Prime filesystem target.

Initial strategy:

- Generic VFS enumeration;
- `statx` logical/allocated metadata;
- `FIEMAP` for file extent information;
- `SEEK_DATA` / `SEEK_HOLE` for sparse files;
- `FS_IOC_GETFSMAP`/XFS-supported filesystem mapping information for advanced physical-space analysis where appropriate;
- reflink/shared-extent awareness;
- project/quota enrichment where configured;
- filesystem event integration through the common Change Engine.

Prime should not require raw XFS metadata parsing for normal live storage indexing.

XFS-specific kernel/xfsprogs interfaces are enrichment donors behind a filesystem adapter.

## 10. NTFS

**Role:** important foreign/local data filesystem and Windows-workload storage source.

On Prime-mounted NTFS through the Linux NTFS driver, the generic VFS scanner is the safe baseline.

WinDirStat's fast NTFS/MFT scanner is a major architectural donor for a future accelerator, but Prime must not directly port the GPLv2 C++ code.

A future native Rust NTFS accelerator may be researched for read-only/unmounted/snapshot-safe targets or other explicitly safe conditions.

Prime must not use raw block-device MFT parsing against a live mounted read/write NTFS volume merely to gain benchmark speed.

WinDirStat remains a behavioral reference oracle for NTFS test fixtures: logical size, allocated size, sparse files, hardlinks, duplicates, directory totals, and largest-file results can be compared against Prime results on equivalent datasets.

## 11. exFAT / FAT

**Role:** removable/interchange storage.

Use the generic VFS scanner and only advertise advanced metrics actually supported by the mounted filesystem/kernel path.

No filesystem-specific accelerator is required for early Prime unless measurement proves a need.

## 12. F2FS and other Linux filesystems

The generic VFS scanner should make new Linux filesystems usable without new Prime architecture.

A filesystem-specific adapter is justified only where it provides meaningful additional correctness, performance, or filesystem semantics.

Examples may include F2FS, network filesystems, or future local filesystems.

## 13. Overlay, tmpfs, network and virtual filesystems

Prime must avoid accidental double counting.

Virtual/overlay/network mounts require capability-labelled accounting:

- logical usage may be available;
- local physical ownership may be unavailable or belong to another underlying mount/provider;
- virtual mounts must not automatically be added to physical disk totals;
- overlay layers must be correlated with their actual backing mounts where possible;
- tmpfs/memory-backed usage belongs to memory/storage observability with clear labeling;
- network filesystems expose remote capacity truth, not local disk ownership.

---

# Change Engine and shared index

## 14. Incremental change tracking

A full rescan should not be the only way Prime knows storage changed.

Prime maintains an initial scan plus incremental changes.

The Change Engine may emit normalized events such as:

- file created;
- file deleted;
- file modified;
- file renamed/moved;
- metadata changed;
- executable appeared/changed;
- size changed;
- hash changed when already tracked;
- mount added/removed;
- filesystem capacity changed materially.

The index must have a reconciliation/full-rescan path because event streams can overflow, be unavailable, or miss changes while Prime is offline.

## 15. Grid-Knight integration boundary

Prime Storage Intelligence may supply mechanical events/metadata/hashes to Grid-Knight later.

Prime says:

`file X appeared / changed / hash Y / profile Z`

Grid-Knight decides:

`security relevant / benign / suspicious / malicious / remediation required`

Prime Storage Intelligence does not become antivirus.

---

# Duplicate detection

## 16. Duplicate Engine

Hashing every file continuously would waste CPU, I/O, and battery/power.

Duplicate detection should be staged:

1. group by plausible size/metadata;
2. hash only candidate groups;
3. use a fast full-content digest appropriate for duplicate discovery;
4. before destructive deduplication/removal, apply stronger verification according to policy;
5. preserve hardlink/reflink semantics and never confuse shared extents with identical content automatically.

The hash algorithm is a contract decision, not hard-coded here.

Prime must never automatically delete user data because two hashes match.

---

# Prime-aware storage ownership

## 17. Ownership model

Prime has an advantage over standalone disk analyzers: it often knows what created the storage.

The index should classify known ownership such as:

- current Prime generation;
- previous-known-good generation;
- recovery generation;
- update staging;
- Prime logs/evidence;
- user files;
- Origins workspaces/missions;
- source repositories;
- build outputs;
- compiler/tool caches;
- SDKs/toolchains;
- containers/images/layers;
- VM disks/snapshots;
- downloads;
- temporary/scratch data;
- future Grid-Knight quarantine.

Example user-facing result:

`CodeOps Rust build cache — 38 GB — last used 19 days ago — rebuildable — estimated reclaim 34 GB`

rather than only:

`/some/path/cache — 38 GB`

## 18. Open-but-deleted files

Prime should eventually account for storage consumed by deleted files still held open by processes.

This is an OS-native advantage and explains cases where filesystem free-space usage does not match visible pathname totals.

The UI must label such storage by holding process/workload where safely attributable.

---

# Cleanup safety

## 19. Cleanup classifications

Prime Cleanup Planner classifies candidates rather than equating "large" with "safe to delete".

### PROTECTED

Examples:

- current Prime generation;
- previous-known-good generation inside retention policy;
- recovery generation;
- active update staging needed for a committed transition;
- active Origins mission state;
- active VM/container state;
- system-critical files.

### RECLAIMABLE

Examples may include:

- rebuildable build caches;
- expired temporary artifacts;
- download caches;
- superseded safe caches under explicit policy.

### REVIEW

Examples:

- unused VMs;
- old container images;
- duplicate user files;
- large downloads;
- stale project artifacts.

### UNKNOWN

Prime does not claim safety when ownership is unknown.

Hunter may explain the classification. Prime owns the underlying protection state.

---

# UI

## 20. Prime Shell Storage UI

Early UI:

- devices/filesystems;
- total/free/available/reserved;
- Prime generation usage;
- rollback/recovery reserve;
- update-space preflight;
- large storage owners;
- low-space warnings.

Later UI may include WinDirStat-inspired and improved views:

- interactive treemap;
- largest files;
- duplicate groups;
- extension/type view;
- search/filter;
- storage ownership view;
- project/build view;
- container/VM view;
- cleanup planner;
- optional sunburst/flamegraph-style visualizations where they improve understanding.

Prime should borrow WinDirStat's clarity and interaction lessons, not copy its Windows UI.

---

# Performance rules

## 21. Scanner performance

Prime should:

- parallelize directory work with bounded concurrency;
- avoid unbounded per-file task creation;
- prefer metadata reads over file-content reads;
- hash only when needed;
- cache/reuse trustworthy scan results;
- update incrementally from change events;
- yield to foreground workloads;
- apply I/O priority/resource policy;
- suspend/resume scanning cleanly;
- expose progress honestly;
- trim caches under memory pressure.

"Fast" does not justify bypassing filesystem safety.

---

# Proof matrix

## 22. Required storage fixtures

Prime Storage Intelligence must eventually prove at least:

- ordinary files/directories;
- permission-denied paths;
- deep directory trees;
- very large directories;
- hardlinks;
- symlinks;
- mount boundaries;
- bind mounts;
- sparse files;
- files modified during scan;
- files deleted during scan;
- open-but-deleted files;
- duplicate files;
- concurrent rename/move;
- event overflow/reconciliation;
- low free space;
- update staging near capacity;
- protected generation reserve;
- ext4 allocation/reserved-space behavior;
- Btrfs subvolumes/snapshots/reflinks/compression/shared extents;
- XFS sparse/reflink/extent behavior;
- NTFS hardlinks/sparse/allocated-size behavior where available;
- removable exFAT/FAT media;
- overlay/virtual filesystem non-double-counting.

Reference tools/results may include filesystem-native tooling plus WinDirStat for equivalent NTFS/Windows fixtures.

Disagreement must trigger investigation; a reference tool is evidence, not infallible authority.

---

# Roadmap placement

## P0 — Complete the Load

Freeze:

- WinDirStat donor record and GPL boundary;
- Prime Storage Intelligence architecture;
- Storage Inventory contract;
- Storage Index schema;
- scanner abstraction;
- metric semantics/confidence rules;
- filesystem adapter contract;
- ext4/Btrfs/XFS/NTFS strategy;
- Change Engine contract;
- cleanup safety states;
- storage-event consumer boundary;
- proof fixtures.

## P1 — First Light

Implement only the foundation required by First Light and Survival:

- block device/filesystem inventory;
- mount identity;
- total/free/available/reserved space;
- Prime generation accounting;
- rollback/recovery reserve visibility;
- update-space preflight;
- basic Storage UI;
- storage-pressure events.

The full WinDirStat-equivalent analyzer is not a P1 blocker.

## P1.5 — Survival

Prove:

- update refuses safely when staging would consume protected rollback/recovery reserve;
- interrupted update does not corrupt the storage model;
- low-space recovery works;
- generation rollback preserves protected storage/state;
- storage accounting remains valid across generation changes.

## P2 — Development Body

Implement the main native Rust Storage Intelligence body:

- generic VFS scanner;
- Storage Index;
- Change Engine;
- duplicate/hash engine;
- build/cache ownership;
- resource-aware scanning;
- richer cleanup planning.

## P3 — Origins Factory

Add:

- project/repository/mission storage projection;
- build artifact/cache visibility;
- Origins consumption of Prime storage capability without Origins owning the Host index.

## Later filesystem acceleration / security integration

Add only after evidence justifies it:

- deeper ext4/Btrfs/XFS adapters;
- safe native NTFS acceleration;
- advanced visualizations;
- Grid-Knight consumption of Prime storage/file events.

---

# Donor disposition

## WinDirStat

**Repository:** `https://github.com/jaydumisuni/windirstat-disk`  
**Upstream lineage:** WinDirStat  
**Licence boundary:** application GPLv2-only; some source portions may have more permissive terms, but Prime does not assume that for the application as a whole.  
**Disposition:** `STUDY / ADAPT / REFERENCE ORACLE`  
**Default code policy:** no direct WinDirStat C++ incorporation into Prime Core.  
**Prime target:** native Rust implementation.

Borrow/study:

- scanner abstraction;
- fast-filesystem-specialization pattern;
- logical versus physical size UX;
- hardlink/reparse/sparse handling lessons;
- duplicate hashing workflow;
- file-change watching;
- scan suspend/resume/cancel;
- treemap and alternate storage visualizations;
- largest-file/search/filter views;
- cleanup workflow and user-defined actions;
- render/result caching and memory trimming;
- behavioral test oracle for NTFS datasets.

Do not inherit blindly:

- Windows-only assumptions;
- Explorer/Windows maintenance commands;
- unsafe or misleading cleanup semantics;
- GPLv2 application code into Prime Core without an intentional licence decision;
- raw filesystem access patterns outside conditions Prime proves safe.

---

# Architectural conclusion

Prime Storage Intelligence is not "WinDirStat for Linux."

It is an OS-native storage truth layer that uses WinDirStat as one mature donor, Linux/VFS as the live filesystem authority, filesystem adapters for semantics that generic traversal cannot express, and Prime's own knowledge of generations, builds, VMs, containers, Origins, caches, and recovery state to provide better answers than a standalone disk analyzer can.

The governing rule remains:

> **Support broadly; activate narrowly; report storage truth without false precision.**
