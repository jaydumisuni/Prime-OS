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
          'sys_mount', 'mem_total_kib', 'device_tree_model',
          'mem_available_kib', 'swap_total_kib', 'swap_free_kib',
          'psi_memory_some_avg10', 'zswap_state')


def evaluate(raw: str) -> dict:
    observations = {}
    conflicts = []
    stage_seen = False
    finished = False
    for line in raw.splitlines():
        if PREFIX not in line:
            continue
        # A serial proof line must actually start with the guest marker.
        # Arbitrary embedded substrings and console control injection cannot
        # become machine evidence. Guest markers must be in the init window.
        if not line.startswith(PREFIX):
            conflicts.append('EMBEDDED_OR_PREFIXED_MARKER'); continue
        candidate = line.strip()
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
            if value == 'INIT_ENTERED' and not stage_seen and not finished:
                stage_seen = True
            elif value == 'EVIDENCE_ONLY_CORE_NOT_STARTED' and stage_seen and not finished:
                finished = True
            else:
                conflicts.append('INVALID_STAGE_TRANSITION')
        elif key in observations:
            conflicts.append('DUPLICATE:' + key)
            if not stage_seen or finished:
                conflicts.append('MARKER_OUTSIDE_INIT_WINDOW')
        elif not stage_seen or finished:
            conflicts.append('MARKER_OUTSIDE_INIT_WINDOW')
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
    # New PHI observations are read-only and OPTIONAL. Older guest snapshots
    # cannot claim PHI completeness. Malformed or contradictory numeric values
    # must not be promoted as valid evidence.
    phi = {}
    for key in ('mem_available_kib','swap_total_kib','swap_free_kib'):
        raw_value = observations.get(key)
        if raw_value is None or raw_value == 'UNKNOWN':
            phi[key] = None
            limitations.append('PHI_OBSERVATION_UNKNOWN:' + key)
            continue
        if not raw_value.isascii() or not raw_value.isdecimal() or len(raw_value)>20:
            conflicts.append('PHI_MALFORMED:' + key)
            phi[key] = None
            continue
        value = int(raw_value)
        if value > (1 << 50):
            conflicts.append('PHI_COUNTER_OUT_OF_RANGE:' + key)
        phi[key] = value
    if isinstance(locals().get('memory'), int) and phi.get('mem_available_kib') is not None:
        if phi['mem_available_kib'] > memory:
            conflicts.append('PHI_MEMORY_AVAILABLE_EXCEEDS_TOTAL')
    total, free = phi.get('swap_total_kib'), phi.get('swap_free_kib')
    if total is not None and free is not None and free > total:
        conflicts.append('PHI_SWAP_FREE_EXCEEDS_TOTAL')
    psi_raw = observations.get('psi_memory_some_avg10','UNKNOWN')
    if psi_raw == 'UNKNOWN':
        phi['psi_memory_some_avg10'] = None
        limitations.append('PHI_OBSERVATION_UNKNOWN:psi_memory_some_avg10')
    else:
        try:
            if len(psi_raw)>8 or psi_raw.count('.')!=1 or not all(part.isdecimal() for part in psi_raw.split('.')):
                raise ValueError()
            psi = float(psi_raw)
            if not 0 <= psi <= 100:
                raise ValueError()
            phi['psi_memory_some_avg10'] = psi
        except ValueError:
            conflicts.append('PHI_PSI_MALFORMED')
            phi['psi_memory_some_avg10'] = None
    zswap=observations.get('zswap_state','UNKNOWN')
    if zswap not in ('ENABLED','DISABLED','UNKNOWN'):
        conflicts.append('PHI_ZSWAP_INVALID_STATE')
    phi['zswap_state'] = zswap if zswap in ('ENABLED','DISABLED') else None
    if phi['zswap_state'] is None:
        limitations.append('PHI_OBSERVATION_UNKNOWN:zswap_state')
    phi['swap_is_not_physical_ram'] = True
    phi['zswap_enabled_does_not_prove_usage'] = True
    return {
        'schema': SCHEMA,
        'candidate_init_observed': not conflicts,
        'status': 'GUEST_EARLY_INIT_OBSERVED_ONLY' if not conflicts else 'UNQUALIFIED_GUEST_TRACE',
        'observations': observations, 'phi_memory_readonly': phi,
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
