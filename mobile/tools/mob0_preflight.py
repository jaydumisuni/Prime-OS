#!/usr/bin/env python3
"""MOB-0 evidence intake: read-only, fail-closed, never flash authorization.

Development prototype for Prime Mobile bring-up, not Prime Host Core authority.
Each observation preserves iAM/X-Ray-style actual/reported/simulated provenance.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
from typing import Any

SCHEMA = 'prime.mobile.device-evidence.candidate.v0'
REQUIRED = (
    'physical_device_identity', 'bootloader_lock_state', 'partition_topology',
    'current_kernel_build', 'board_device_tree', 'stock_restoration_path',
    'first_light_boot_route', 'boot_image_digest', 'driver_dependency_inventory',
)
# The labels are evidence classes, never privilege or certification.
EVIDENCE_KINDS = {'LIVE_READ', 'FILE_HASHED', 'OWNER_REPORTED', 'PHOTO', 'THIRD_PARTY', 'SIMULATED'}
EVIDENCE_STATES = {'OBSERVED', 'UNKNOWN', 'CONFLICTED', 'UNSUPPORTED'}
LIVE_ONLY = {'physical_device_identity', 'bootloader_lock_state', 'partition_topology', 'current_kernel_build'}
# A candidate freshness convention for the development tool, not frozen OS policy.
MAX_AGE_SECONDS = {
    'physical_device_identity': 86400,
    'bootloader_lock_state': 86400,
    'partition_topology': 86400,
    'current_kernel_build': 86400,
    'board_device_tree': 30 * 86400,
    'stock_restoration_path': 30 * 86400,
    'first_light_boot_route': 30 * 86400,
    'boot_image_digest': 30 * 86400,
    'driver_dependency_inventory': 30 * 86400,
}


def _timestamp(value: str | None) -> datetime | None:
    if not isinstance(value, str):
        return None
    try:
        result = datetime.fromisoformat(value.replace('Z','+00:00'))
        return result.astimezone(timezone.utc) if result.tzinfo is not None else None
    except ValueError:
        return None


def evaluate(document: dict[str, Any], now: datetime | None = None) -> dict[str, Any]:
    """Evaluate evidence metadata without trusting any supplied claim as authority.

    A fully satisfied *synthetic* manifest can only become design candidate.
    It never authorizes device writes, and does not cryptographically verify proofs.
    """
    now = now or datetime.now(timezone.utc)
    if now.tzinfo is None:
        raise ValueError('now must carry a timezone')
    now = now.astimezone(timezone.utc)
    if not isinstance(document, dict) or document.get('schema') != SCHEMA:
        raise ValueError('device evidence schema mismatch')
    subject = document.get('subject')
    if not isinstance(subject, dict) or not isinstance(subject.get('alias'), str) or not subject['alias']:
        raise ValueError('subject must contain a nonempty privacy-preserving alias')
    observations = document.get('observations')
    if not isinstance(observations, dict):
        raise ValueError('observations must be an object')
    unsupported = sorted(set(observations) - set(REQUIRED) - {'recovery_interface'})
    if unsupported:
        raise ValueError('unknown observation field(s): ' + ','.join(unsupported))

    outcomes: dict[str, str] = {}
    for key in REQUIRED:
        entry = observations.get(key)
        if entry is None:
            outcomes[key] = 'MISSING'
            continue
        if not isinstance(entry, dict):
            outcomes[key] = 'MALFORMED'
            continue
        state = entry.get('state')
        kind = entry.get('source_kind')
        observed = _timestamp(entry.get('observed_at'))
        reference = entry.get('evidence_ref')
        subject_alias = entry.get('subject_alias')
        if state not in EVIDENCE_STATES or kind not in EVIDENCE_KINDS:
            outcomes[key] = 'MALFORMED'
        elif state == 'CONFLICTED':
            outcomes[key] = 'CONFLICTED'
        elif state == 'UNSUPPORTED':
            outcomes[key] = 'UNSUPPORTED'
        elif state != 'OBSERVED':
            outcomes[key] = 'UNKNOWN'
        elif kind == 'SIMULATED':
            outcomes[key] = 'SIMULATED_NOT_PHYSICAL'
        elif kind in {'OWNER_REPORTED','PHOTO','THIRD_PARTY'}:
            outcomes[key] = 'UNQUALIFIED_SOURCE'
        elif key in LIVE_ONLY and kind != 'LIVE_READ':
            outcomes[key] = 'NOT_LIVE_DEVICE_EVIDENCE'
        elif subject_alias != subject['alias']:
            outcomes[key] = 'DEVICE_BINDING_MISMATCH'
        elif not isinstance(reference,str) or not reference.strip():
            outcomes[key] = 'MISSING_PROVENANCE_REFERENCE'
        elif observed is None:
            outcomes[key] = 'INVALID_OBSERVATION_TIME'
        elif observed > now:
            outcomes[key] = 'FUTURE_OBSERVATION'
        elif (now - observed).total_seconds() > MAX_AGE_SECONDS[key]:
            outcomes[key] = 'STALE'
        else:
            outcomes[key] = 'CANDIDATE_NEEDS_INDEPENDENT_VERIFICATION'

    recovery = observations.get('recovery_interface')
    observed_recovery = isinstance(recovery,dict) and recovery.get('state') == 'OBSERVED'
    counts = {state: list(outcomes.values()).count(state) for state in sorted(set(outcomes.values()))}
    return {
        'schema': 'prime.mobile.mob0-preflight-result.candidate.v0',
        'status': 'READ_ONLY_CANDIDATE_NEVER_FLASH_AUTHORITY',
        'subject_alias': subject['alias'],
        'evaluated_at': now.isoformat(timespec='seconds'),
        'all_required_evidence_candidate': all(x == 'CANDIDATE_NEEDS_INDEPENDENT_VERIFICATION' for x in outcomes.values()),
        'machine_verified_device_identity': False,
        'device_write_authorized': False,
        'boot_or_flash_attempted': False,
        'physical_acceptance': False,
        'recovery_ui_observation_present': observed_recovery,
        'evidence_status': outcomes,
        'counts': counts,
        'limitations': [
            'Inputs are caller-supplied metadata; hashes, live device identity, proof references and signatures are not independently verified.',
            'Approval is a separate owner/security authority; this tool cannot grant it.',
            'This is a MOB-0 development intake, not a Prime Host capability or a flashing program.',
        ]
    }


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument('manifest', type=Path)
    ap.add_argument('--output', type=Path)
    a = ap.parse_args()
    result = evaluate(json.loads(a.manifest.read_text(encoding='utf-8')))
    serialized = json.dumps(result, indent=2, sort_keys=True) + '\n'
    if a.output:
        a.output.write_text(serialized, encoding='utf-8')
    else:
        print(serialized, end='')


if __name__ == '__main__':
    main()