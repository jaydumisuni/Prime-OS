#!/usr/bin/env python3
"""Bounded parser for captured generic ARM64 guest UART evidence.

Rejects invented/duplicate/contradictory markers; cannot grant device writes,
Prime Core readiness, phone boot or physical qualification.
"""
from __future__ import annotations

import argparse
import json
from pathlib import Path

SCHEMA = 'prime.mobile.mob1.generic-arm64-serial-evidence.candidate.v0'
PREFIX = 'PRIME_MOB1_V0|'
FIELDS = ('stage', 'compiled_arch', 'pid1', 'kernel_machine', 'proc_mount',
          'sys_mount', 'mem_total_kib', 'device_tree_model')


def evaluate(raw: str) -> dict:
    observations = {}
    conflicts = []
    for line in raw.splitlines():
        if PREFIX not in line:
            continue
        # Accept only a complete printable marker after optional guest prefix,
        # not arbitrary shell text containing fabricated carriage returns.
        candidate = line[line.index(PREFIX):].strip()
        if len(candidate) > 256 or '\r' in candidate or '\x1b' in candidate:
            conflicts.append('INVALID_CONTROL_OR_LENGTH'); continue
        if candidate.count('|') != 1 or '=' not in candidate:
            conflicts.append('INVALID_MARKER'); continue
        key, value = candidate[len(PREFIX):].split('=', 1)
        if key not in FIELDS or not value or any(ord(c) < 32 or ord(c) > 126 for c in value):
            conflicts.append('UNSUPPORTED_OR_INVALID_FIELD'); continue
        # The stage field is the only repeating one, and must be exactly two
        # transitions in the correct order. Otherwise no proof promotion.
        if key == 'stage':
            observations.setdefault('stages', []).append(value)
        elif key in observations:
            conflicts.append('DUPLICATE:' + key)
        else:
            observations[key] = value
    stages = observations.get('stages', [])
    expected = ['INIT_ENTERED', 'EVIDENCE_ONLY_CORE_NOT_STARTED']
    if stages != expected:
        conflicts.append('MISSING_OR_INVALID_STAGE_ORDER')
    if observations.get('compiled_arch') != 'aarch64':
        conflicts.append('NOT_AARCH64_COMPILATION_EVIDENCE')
    if observations.get('kernel_machine') != 'aarch64':
        conflicts.append('KERNEL_ARCH_UNKNOWN_OR_MISMATCH')
    if observations.get('pid1') != 'true':
        conflicts.append('NOT_PID1_OR_UNKNOWN')
    if observations.get('proc_mount') != 'AVAILABLE':
        conflicts.append('PROC_UNAVAILABLE')
    try:
        memory = int(observations.get('mem_total_kib', ''))
        if not 1 <= memory <= (1 << 50):
            raise ValueError()
    except ValueError:
        conflicts.append('MEMORY_OBSERVATION_UNKNOWN_OR_INVALID')
    # sysfs and DT absence are limitations, not invented board identity.
    limitations = []
    if observations.get('sys_mount') != 'AVAILABLE':
        limitations.append('SYSFS_UNAVAILABLE')
    if observations.get('device_tree_model', 'UNKNOWN') == 'UNKNOWN':
        limitations.append('DEVICE_TREE_MODEL_UNKNOWN')
    return {
        'schema': SCHEMA,
        'candidate_init_observed': not conflicts,
        'status': 'GUEST_EARLY_INIT_OBSERVED_ONLY' if not conflicts else 'UNQUALIFIED_GUEST_TRACE',
        'observations': observations,
        'blockers': sorted(set(conflicts)), 'limitations': limitations,
        'prime_core_ready': False, 'prime_shell_ready': False,
        'physical_phone_qualified': False, 'flash_authorization': False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('serial_log', type=Path)
    args = parser.parse_args()
    print(json.dumps(evaluate(args.serial_log.read_text(errors='replace')), indent=2, sort_keys=True))


if __name__ == '__main__':
    main()