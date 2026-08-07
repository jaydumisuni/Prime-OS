# Prime OS Storage Architecture

Status: **PLANNING AUTHORITY CANDIDATE — implementation remains NOT AUTHORIZED until the Prime plan is frozen.**

This document resolves the default filesystem direction for Prime OS and records how ext4, Btrfs and XFS fit without forcing one filesystem to do every job.

## Decision summary

### Default Prime filesystem: Btrfs

Btrfs is the default Prime system/workstation filesystem because Prime needs more than a conventional journaled filesystem. Prime's architecture requires generation-aware rollback, storage intelligence, checksummed data/metadata, compression, reflinks, snapshots/subvolumes, efficient copy-on-write cloning, online scrub and flexible resizing.

Prime should use Btrfs as a platform capability, not expose users to raw Btrfs administration for ordinary operation.

### ext4 role: conservative recovery / interoperability fallback

ext4 remains a supported Prime filesystem and recovery/interoperability option because of its mature journaling, broad Linux tooling, metadata checksums, fscrypt/fs-verity support and simple operational model.

ext4 is not the preferred default Prime workstation filesystem because Prime would have to rebuild snapshot/subvolume/generation behavior above it that Btrfs already supplies efficiently.

Potential uses:
- conservative recovery/data partition where independence from Btrfs is valuable;
- simple external/removable Linux volumes;
- compatibility/fallback install profile if Btrfs is unsuitable on a particular target;
- test/reference filesystem for Prime Storage Intelligence.

### XFS role: optional high-write / high-throughput data filesystem

XFS remains a supported optional filesystem for workloads that benefit from its mature high-concurrency allocation, reflink support, large-filesystem behavior and online scrub/repair direction.

XFS is not the default Prime system filesystem because Prime relies heavily on cheap system/user snapshots, subvolume separation and flexible rollback behavior that XFS does not provide natively in the same way.

Potential uses:
- dedicated build/cache volume;
- large VM/container image store;
- large media/artifact workspace;
- server/workstation data volume where benchmark evidence shows it is the stronger backend.

Do not split a small single-disk Prime install into XFS merely to use XFS. On single-disk systems, Btrfs subvolumes and workload-specific COW policy are preferred unless evidence says otherwise.

## First-host storage layout direction

For the HP 290 G4 first proof Host, the target architecture is:

1. UEFI System Partition — FAT32 as required by UEFI.
2. Prime recovery path — independent from the main Btrfs runtime; exact recovery-image format remains a P0 implementation choice. ext4 is an approved conservative backing option where a writable Linux recovery volume is useful.
3. Main Prime storage — Btrfs, preferably beneath LUKS2 when full-disk/user-data encryption is enabled.
4. Optional secondary data disk — may be Btrfs by default or XFS when explicitly selected for a proven high-write/high-throughput role.
5. RAM pressure — prefer zram for the first 8 GB Host rather than making a large writable swapfile part of the default Btrfs design. Hibernation/disk-backed swap is a separate capability decision.

## Btrfs subvolume model

The exact names can change during P0 contract freeze, but Prime requires separate policy domains for at least:

- system generations;
- user/project data;
- Prime configuration;
- Origins state;
- Application Profiles;
- logs;
- VM images;
- container state;
- build caches;
- temporary/scratch data;
- recovery metadata.

Nested subvolumes should be used where data must be excluded from a system-generation snapshot or given different retention/COW/compression policy.

## Generation and rollback rule

Btrfs snapshots are a storage primitive, not the authority that makes a Prime generation trusted.

A Prime generation must still be:
- built from an explicit manifest/image/tree;
- identified by generation identity;
- cryptographically verified according to the Prime update contract;
- staged independently from the currently booted generation;
- boot-selected deliberately;
- health-tested;
- retain a previous known-good generation;
- recoverable if the main generation fails.

Btrfs read-only subvolumes/snapshots may be used to store or clone generations efficiently. Prime must not equate "snapshot exists" with "generation is valid".

## Btrfs integrity and maintenance

Prime should expose Btrfs capabilities through Prime Core rather than requiring users to invoke filesystem-specific commands.

Relevant capabilities include:
- data and metadata checksums;
- online scrub;
- subvolume/snapshot health;
- free-space accounting;
- compression status/effectiveness;
- device errors;
- send/receive where useful for migration/backup;
- reflink-aware file copies;
- resize capability.

Snapshots are not backups. Prime backup/recovery policy must still include independent/off-host copies for important data.

Do not use Btrfs RAID5/6 as a Prime production default. Current upstream documentation continues to mark RAID56 as unreliable/unstable for some cases. Prefer single-device, RAID1-family or other proven storage designs until evidence and upstream status change.

## COW-sensitive workloads

VM images, databases and other heavy random-overwrite workloads require explicit treatment.

Prime must not globally disable Btrfs COW/checksums just to support them.

Available strategies, selected per workload/profile:
- normal Btrfs COW when performance is acceptable;
- dedicated Btrfs subvolume/directory using NOCOW for workloads that demonstrably need it, accepting that NOCOW also loses normal Btrfs data checksumming/compression for those files;
- dedicated XFS volume when benchmark/proof shows that is the cleaner design;
- VM/container format/backend choices that minimize pathological rewrite patterns.

Prime Storage Intelligence must report these policy differences clearly.

## ext4 capability support

Prime Storage Intelligence must understand ext4 even when ext4 is not the default. Relevant ext4 facts include:
- logical and allocated size;
- filesystem usage/free space;
- extents;
- inode/file metadata;
- journal state where exposed safely;
- metadata checksum capability;
- fs-verity/fscrypt capability;
- mount and error state.

Prime should not invent snapshots for ext4 and label them native filesystem snapshots. If a higher storage layer provides snapshotting, report that layer honestly.

## XFS capability support

Prime Storage Intelligence must understand XFS when mounted. Relevant capability areas include:
- logical and allocated size;
- reflinks/shared extents where detectable;
- project/user/group quota data;
- allocation/free-space information;
- filesystem health/scrub information;
- online repair capabilities as supported by the installed kernel/xfsprogs;
- grow capability;
- workload suitability for large parallel data sets.

Do not promise native XFS snapshot/rollback semantics that XFS itself does not provide.

## Filesystem abstraction

Prime Storage Intelligence should use a generic storage interface plus filesystem-specific adapters.

Conceptual structure:

Prime Storage Core
- Generic Linux/POSIX adapter
- Btrfs adapter
- ext4 adapter
- XFS adapter
- future filesystem adapters

Generic truth includes:
- mount/source/UUID;
- filesystem type;
- logical size;
- allocated/physical size where available;
- free/available space;
- file ownership/mode/timestamps;
- file type;
- inode/device identity;
- file change events;
- hashes when requested/available.

Filesystem-specific adapters add only facts the underlying filesystem actually supports.

## Prime Storage Intelligence

Prime will borrow the WinDirStat capability/architecture/UX pattern without porting its C++ implementation into Prime Core.

The Prime-native implementation should be Rust-first and Linux/filesystem aware.

Core capabilities:
- fast parallel directory scanning;
- logical vs allocated/physical usage;
- largest-file and largest-directory analysis;
- file-type/extension grouping;
- search/filtering;
- duplicate candidate detection;
- hardlink/shared-file awareness;
- filesystem event watching so Prime can update incrementally instead of repeatedly rescanning everything;
- persistent scan/index state where useful;
- safe cleanup candidates;
- filesystem-specific health/space context;
- build/cache/VM/container/application storage attribution;
- Prime generation/recovery storage awareness.

Prime Shell can expose:
- treemap or equivalent spatial view;
- directory tree;
- largest files;
- duplicate candidates;
- application/build/cache/VM/container usage;
- snapshot/generation usage;
- safe cleanup actions;
- storage health and pressure.

## Shared index rule

Prime should avoid having multiple subsystems independently rescan every file.

The long-term pattern should be:

Prime File/Storage Index
→ Prime Storage UI
→ update/recovery capacity checks
→ duplicate/hash analysis
→ Origins storage projections
→ Grid-Knight security-relevant file/change inputs

Grid-Knight remains responsible for cybersecurity interpretation. Prime Storage Intelligence reports mechanical file/storage truth; it does not decide that a file is malware.

## WinDirStat donor disposition

Repository: https://github.com/jaydumisuni/windirstat-disk

Disposition: **STUDY / ADAPT PATTERNS / NATIVE PRIME REIMPLEMENTATION**

Borrow:
- scanning workflow;
- logical vs physical size thinking;
- largest-file views;
- duplicate/hash workflow;
- file watching;
- search/filter workflow;
- treemap interaction concepts;
- cleanup UX and safety patterns;
- scan persistence/reporting ideas.

Do not inherit by default:
- Windows-only assumptions;
- Explorer/shell integration;
- NTFS-specific maintenance actions;
- C++/MFC implementation;
- WinDirStat product identity/UI wholesale.

Licence boundary:
WinDirStat is GPLv2-only. Prime should therefore prefer an independently implemented Rust equivalent based on documented behavior and architecture. If a particular GPL component is ever directly reused, it must remain within an intentionally reviewed licence boundary rather than being casually merged into Prime Core.

## P0 consequences

The Prime P0 storage work now needs to freeze:
- final Btrfs subvolume layout;
- encryption layout (LUKS2 and key/recovery policy where enabled);
- boot/recovery partition/image layout;
- generation storage/sealing mechanism;
- Btrfs snapshot retention rules;
- VM/container/database COW policy;
- optional XFS selection criteria;
- ext4 fallback/recovery criteria;
- zram/swap/hibernation policy;
- Prime Storage Intelligence v1 contract;
- shared file/storage index contract;
- filesystem adapter contract;
- storage cleanup safety/authority rules.

## Current recommendation

Prime v1 default:

FAT32 ESP
→ Prime boot/recovery entry
→ LUKS2 where encryption is enabled
→ Btrfs main Prime filesystem
→ subvolume-separated system/user/Origins/VM/container/cache/log state
→ zstd compression selectively/default where proven appropriate
→ workload-specific NOCOW only when justified
→ zram for normal memory pressure
→ optional XFS secondary high-write data volume when evidence justifies it
→ ext4 supported as fallback/recovery/interoperability filesystem

This decision preserves Prime's lightweight goal while giving the OS strong rollback, integrity, storage-intelligence and developer-workload foundations.
