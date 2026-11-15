#!/usr/bin/env python3
"""Read-only PHI + ARM64 board observation → MOB-0 provenance intake.

This is a testable candidate adapter, not iAM's full cognition runtime,
not live device identity attestation and never a permission to boot/flash.
"""
from __future__ import annotations

import argparse
from datetime import datetime,timezone
import hashlib
import json
from pathlib import Path
import platform
import sys

# Local, checked-in source donors; no network or arbitrary plugin loading.
from board_readonly_probe import snapshot as board_snapshot
from mob0_preflight import SCHEMA as MOB0_SCHEMA, evaluate
sys.path.insert(0, str(Path(__file__).resolve().parents[2] / 'tools'))
from phi_readonly_probe import snapshot as phi_snapshot


def _sha(data: object) -> str:
    normalized=json.dumps(data,sort_keys=True,separators=(',',':'),ensure_ascii=False).encode('utf8')
    return 'sha256:'+hashlib.sha256(normalized).hexdigest()


def capture(root: Path, alias: str, arch: str | None = None, observed_at: str | None = None) -> dict:
    if not isinstance(alias,str) or not alias or len(alias)>128:
        raise ValueError('caller must provide a nonempty privacy-preserving alias <=128 chars')
    root=Path(root)
    timestamp=observed_at or datetime.now(timezone.utc).isoformat(timespec='seconds')
    arch=arch or platform.machine()
    # A copied fixture is *not* a live device observation merely because it
    # contains a realistic Device Tree. Likewise an arbitrary arch override.
    live=root.resolve()==Path('/').resolve() and arch==platform.machine()
    kind='LIVE_READ' if live else 'SIMULATED'
    board=board_snapshot(root,arch,timestamp)
    phi=phi_snapshot(root,timestamp)
    observations={}
    def add(key, description, value):
        observations[key]={
            'state':'OBSERVED', 'source_kind':kind,
            'observed_at':timestamp,'evidence_ref':_sha({'key':key,'value':value,'subject_alias':alias,'source_kind':kind}),
            'subject_alias':alias, 'description':description,
        }
    if arch.lower() in ('aarch64','arm64'):
        if board['board_model'] is not None and board['board_compatible'] is not None:
            add('board_device_tree','Kernel-projected DT model/compatible, NOT independently verified board identity',
                {'model':board['board_model'],'compatible':board['board_compatible']})
        if board['kernel_release'] is not None:
            add('current_kernel_build','Read-only kernel release source; does not attest vendor firmware',board['kernel_release'])
    manifest={
        'schema': MOB0_SCHEMA,
        'status':'OBSERVATION_CAPTURE_CANDIDATE_ONLY',
        'subject':{'alias':alias,'qualifier':'Caller-selected alias, no attested Host or phone identity.'},
        'observations':observations,
        'action_boundary':'Never authorizes boot, flash, erase, bootloader changes or privileged device access.'
    }
    return {
        'schema':'prime.mobile.offline-observation-bundle.candidate.v0',
        'status':'READ_ONLY_UNATTESTED',
        'subject_alias':alias,
        'capture_kind':kind,
        'board':board,
        'phi_memory':phi,
        'mob0_manifest':manifest,
        'mob0_preflight':evaluate(manifest, datetime.fromisoformat(timestamp.replace('Z','+00:00'))),
        'device_modification_performed':False,
        'limitations':['Source paths and observed values are unauthenticated; device/bootloader identity and owner approval are separate.',
                       'Simulator/fixture root is NEVER live physical evidence.',
                       'Read-only PHI values are OS observations, not phone performance or actual swap-policy qualification.']
    }


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root',type=Path,default=Path('/'))
    p.add_argument('--alias',required=True)
    p.add_argument('--arch',help='Synthetic fixture override; does NOT attest an architecture')
    p.add_argument('--output',type=Path)
    a=p.parse_args()
    content=json.dumps(capture(a.root,a.alias,a.arch),sort_keys=True,indent=2)+'\n'
    if a.output:a.output.write_text(content,encoding='utf8')
    else:print(content,end='')

if __name__=='__main__':main()