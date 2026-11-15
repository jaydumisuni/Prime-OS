#!/usr/bin/env python3
"""Offline A14m reference layout audit. No Android, ADB, kernel writes or flash paths.

Read *source files* from immutable references. These sources are historical and
cannot certify a specific physical handset, trusted kernel, or flash target.
"""
from __future__ import annotations
import argparse
from hashlib import sha256
import json
from pathlib import Path
import re

SCHEMA = 'prime.mobile.a14m.offline-layout-audit.candidate.v0'
REQUIRED_PARTITIONS = ('boot', 'recovery', 'vendor_boot', 'init_boot', 'dtbo', 'vbmeta', 'super')
REQUIRED_BOOT_FIELDS = ('page_size', 'base_addr', 'ramdisk_offset', 'tags_offset', 'kernel_size', 'dtb_size')
SOURCE_HAZARDS = {
    'BOARD_AVB_MAKE_VBMETA_IMAGE_ARGS': 'RECOVERY_TREE_DISABLES_AVB_FLAGS_DO_NOT_REUSE',
    'PLATFORM_SECURITY_PATCH': 'RECOVERY_TREE_SPOOFED_SECURITY_PATCH_DO_NOT_REUSE',
    'BOARD_SUPER_PARTITION_SIZE': 'RECOVERY_TREE_SUPER_SIZE_HARDCODED_TODO_NOT_AUTHORITY',
    'ALLOW_MISSING_DEPENDENCIES': 'RECOVERY_TREE_PERMISSIVE_BUILD_NOT_PRODUCTION',
}


def assignments(text: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for row in text.splitlines():
        match = re.match(r'^([A-Z][A-Z0-9_]*)\s*(?::=|\?=|\+=|=)\s*(.*)$', row)
        if match:
            key, value = match.groups()
            out[key] = (out.get(key, '') + ' ' + value.split('#', 1)[0].strip()).strip()
    return out


def image_info(text: str) -> dict[str, str]:
    out: dict[str, str] = {}
    for line in text.splitlines():
        m = re.fullmatch(r'([a-z0-9_]+)=(.*)', line.strip())
        if m: out[m.group(1)] = m.group(2).strip().strip('"\'')
    return out


def partition_names(text: str) -> list[str]:
    names = re.findall(r'^Partition Name:\s*(\S+)\s*$', text, re.M)
    if not names or len(names) > 200 or len(names) != len(set(names)):
        raise ValueError('UNUSABLE_OR_DUPLICATE_PIT_PARTITION_NAMES')
    return names


def numeric(raw: str | None) -> int | None:
    if raw is None: return None
    try: return int(raw, 0)
    except ValueError: return None


def audit(boardconfig: str, boot_info: str, recovery_info: str,
          pit_analysis: str, sources: dict | None = None) -> dict:
    board = assignments(boardconfig)
    boot = image_info(boot_info)
    recovery = image_info(recovery_info)
    pit = partition_names(pit_analysis)
    blockers: list[str] = []
    for name in REQUIRED_PARTITIONS:
        if name not in pit: blockers.append('PIT_PARTITION_ABSENT:' + name)
    for name, metadata in (('boot', boot), ('recovery', recovery)):
        for key in REQUIRED_BOOT_FIELDS:
            if key not in metadata: blockers.append(f'{name.upper()}_INFO_MISSING:{key}')
        for key in ('page_size', 'base_addr', 'ramdisk_offset', 'tags_offset'):
            if key in metadata and numeric(metadata[key]) is None:
                blockers.append(f'{name.upper()}_INFO_BAD_NUMBER:{key}')
    if numeric(board.get('BOARD_KERNEL_PAGESIZE')) != numeric(boot.get('page_size')):
        blockers.append('BOOT_PAGE_SIZE_DISAGREEMENT')
    if numeric(board.get('BOARD_KERNEL_PAGESIZE')) != numeric(recovery.get('page_size')):
        blockers.append('RECOVERY_PAGE_SIZE_DISAGREEMENT')
    for k, src in (('BOARD_KERNEL_BASE', 'base_addr'), ('BOARD_RAMDISK_OFFSET', 'ramdisk_offset'),
                   ('BOARD_KERNEL_TAGS_OFFSET', 'tags_offset')):
        for label, record in (('BOOT', boot), ('RECOVERY', recovery)):
            if numeric(board.get(k)) != numeric(record.get(src)):
                blockers.append(f'{label}_{k}_DISAGREEMENT')
    if board.get('BOARD_BOOTIMG_HEADER_VERSION') != '2':
        blockers.append('TWRP_REFERENCE_BOOT_HEADER_NOT_V2')
    if board.get('TARGET_FORCE_PREBUILT_KERNEL') != 'true':
        blockers.append('PREBUILT_KERNEL_NOT_ENABLED_IN_REFERENCE')
    if numeric(board.get('BOARD_BOOTIMAGE_PARTITION_SIZE')) is None:
        blockers.append('BOOT_PARTITION_SIZE_UNKNOWN_IN_REFERENCE')
    if numeric(board.get('BOARD_RECOVERYIMAGE_PARTITION_SIZE')) is None:
        blockers.append('RECOVERY_PARTITION_SIZE_UNKNOWN_IN_REFERENCE')
    if numeric(boot.get('kernel_size')) and numeric(recovery.get('kernel_size')) and boot['kernel_size'] == recovery['kernel_size']:
        blockers.append('BOOT_AND_RECOVERY_KERNEL_DISTINCTION_LOST')
    src = sources or {}
    rec_blobs = src.get('recovery_blobs', {})
    dump_blobs = src.get('dump_recovery_blobs', {})
    for filename in ('kernel', 'dtb.img', 'dtbo.img'):
        if (not isinstance(rec_blobs.get(filename), str)
                or rec_blobs.get(filename) != dump_blobs.get(filename)):
            blockers.append('RECOVERY_SOURCE_BLOB_MISMATCH_OR_UNKNOWN:' + filename)
    hazard_flags = [note for key, note in SOURCE_HAZARDS.items() if key in board]
    # Recovery sources embed disabled AVB flags and historical/spoofed patch levels;
    # copying them into a Prime device image must never be automatic.
    return {
        'schema': SCHEMA,
        'status': 'REFERENCE_LAYOUT_RECONCILED_NOT_DEVICE_QUALIFIED' if not blockers else 'STOP_REFERENCE_LAYOUT_CONFLICT',
        'source_scope': 'PINNED_HISTORICAL_AWG1_AND_TWRP_REFS_ONLY',
        'operating_system_required_for_this_audit': False,
        'pit_partition_count': len(pit),
        'pit_selected_partition_names': [name for name in REQUIRED_PARTITIONS if name in pit],
        'boot_reference': {k: boot.get(k) for k in REQUIRED_BOOT_FIELDS},
        'recovery_reference': {k: recovery.get(k) for k in REQUIRED_BOOT_FIELDS},
        'boardconfig_reference': {k: board.get(k) for k in ('BOARD_BOOTIMG_HEADER_VERSION', 'BOARD_KERNEL_PAGESIZE', 'BOARD_BOOTIMAGE_PARTITION_SIZE', 'BOARD_RECOVERYIMAGE_PARTITION_SIZE', 'TARGET_PREBUILT_KERNEL')},
        'recovery_prebuilt_blobs_match_dump': not any('RECOVERY_SOURCE_BLOB' in b for b in blockers),
        'reference_hazards': sorted(set(hazard_flags)),
        'limitations': [
            'Historical source partition names and legacy TWRP settings are not physical firmware/bootloader evidence.',
            'PIT includes vendor_boot and init_boot, but the TWRP recovery BoardConfig v2 cannot determine their current use.',
            'Never copy permissive AVB, spoofed patch date or unverified super partition sizes from a recovery tree.',
            'Android getprop is not required; TWRP-only first boot remains a separate physical test.',
        ],
        'blockers': blockers,
        'source_manifest_sha256_unattested': sha256(json.dumps(src,sort_keys=True).encode()).hexdigest(),
        'boot_image_ready': False,
        'boot_chain_qualified': False,
        'stock_restore_qualified': False,
        'device_flash_authorized': False,
        'physical_boot_observed': False,
    }


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    for k in ('boardconfig', 'boot-info', 'recovery-info', 'pit-analysis', 'sources'):
        p.add_argument('--' + k, type=Path, required=True)
    p.add_argument('--output', type=Path)
    a = p.parse_args()
    result = audit(a.boardconfig.read_text(), a.boot_info.read_text(),
                   a.recovery_info.read_text(), a.pit_analysis.read_text(),
                   json.loads(a.sources.read_text()))
    data = json.dumps(result, indent=2, sort_keys=True) + '\n'
    if a.output: a.output.write_text(data)
    else: print(data, end='')

if __name__ == '__main__': main()
