# WinDirStat Donor Record

**Prime subsystem:** Prime Storage Intelligence  
**Repository:** https://github.com/jaydumisuni/windirstat-disk  
**Classification:** architecture / behavior / UX donor and reference oracle  
**Disposition:** `STUDY / ADAPT / REFERENCE ORACLE`  
**Prime implementation target:** native Rust

## Licence boundary

The WinDirStat application is GPLv2-only. Some source portions may carry more permissive terms, but Prime must not assume the application can be incorporated into Prime Core under a different licence.

Default decision:

- do not merge WinDirStat C++ application code into Prime Core;
- study the architecture and behavior;
- independently implement the Prime equivalent in Rust against Linux/VFS and filesystem-specific APIs;
- if any individual source component is ever proposed for reuse, verify that component's exact licence separately before reuse.

## What Prime borrows/studies

- common scanner abstraction;
- generic scanner + filesystem-specific fast-path architecture;
- multithreaded/bounded scanning;
- suspend/resume/stop behavior;
- logical versus physical/allocated size presentation;
- hardlink handling;
- sparse/compressed filesystem edge cases;
- duplicate hashing workflow;
- file watcher/change-event workflow;
- largest-file/search/filter views;
- treemap and alternate storage visualizations;
- saved/reloaded scan concepts;
- cleanup workflow and safety lessons;
- render/result caching and memory trimming;
- NTFS fast-scan/MFT concepts as a future safe-accelerator research lane.

## What Prime does differently

Prime is OS-native and therefore can know storage ownership that WinDirStat cannot inherently know, including:

- current/previous/recovery Prime generations;
- update staging and rollback reserve;
- Origins/project ownership;
- build outputs and rebuildable caches;
- VM/container storage;
- downloads and temporary data;
- protected system state;
- future Grid-Knight quarantine/storage events.

Prime must classify cleanup candidates as protected/reclaimable/review/unknown instead of treating large size as permission to delete.

## Filesystem strategy

WinDirStat's NTFS specialization is borrowed as an architectural pattern, not as C++ code.

Prime uses a generic Linux/VFS scanner plus adapters/enrichment for:

- ext4;
- Btrfs;
- XFS;
- NTFS;
- exFAT/FAT;
- other mounted Linux filesystems through the generic path.

See `docs/PRIME_STORAGE_INTELLIGENCE.md` for the full contract and phase placement.

## Reference-oracle role

For NTFS fixtures, Prime may compare its output with WinDirStat for:

- logical size;
- allocated size;
- hardlinks;
- sparse files;
- duplicate groups;
- directory totals;
- largest-file ordering.

Disagreement triggers investigation. WinDirStat remains a reference source, not infallible authority.
