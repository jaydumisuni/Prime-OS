#!/usr/bin/env python3
"""Audit third-party Prime Mobile device-port *claims*, never grant driver trust.

A developer may document support without changing Prime Core. Acceptance and
signatures remain separate from this read-only candidate manifest inspector.
"""
from __future__ import annotations

import argparse
from collections import Counter
from datetime import datetime,timezone
import json
from pathlib import Path

SCHEMA = 'prime.mobile.device-port.candidate.v0'
FEATURES = (
    'boot','recovery','display','touch','audio','modem_calls','sms','cellular_data',
    'wifi','bluetooth','charging','suspend_resume','thermal','usb_host',
    'storage_encryption','camera','gnss','ota_rollback','gpu_compute'
)
STATES = {'UNKNOWN','DRIVER_PRESENT','DRIVER_BOUND','FUNCTIONAL_OBSERVED','PHYSICALLY_VERIFIED','BLOCKED'}
REQUIRED_PHONE = {'boot','recovery','display','touch','audio','modem_calls','sms','cellular_data',
                  'wifi','charging','suspend_resume','thermal','storage_encryption','ota_rollback'}


def audit(record: dict, observed_at: str | None = None) -> dict:
    if not isinstance(record,dict) or record.get('schema') != SCHEMA:
        raise ValueError('candidate port schema missing or invalid')
    board=record.get('board')
    if not isinstance(board,dict) or not isinstance(board.get('alias'),str) or not board['alias'].strip():
        raise ValueError('board alias missing')
    features=record.get('capabilities')
    if not isinstance(features,dict) or set(features) - set(FEATURES):
        raise ValueError('unknown device capability')
    out={}
    for name in FEATURES:
        item=features.get(name)
        if item is None:
            out[name]='UNKNOWN'
            continue
        if not isinstance(item,dict) or item.get('state') not in STATES:
            out[name]='INVALID'
            continue
        state=item['state']
        if state in ('DRIVER_BOUND','FUNCTIONAL_OBSERVED','PHYSICALLY_VERIFIED'):
            proof=item.get('evidence_ref')
            device=item.get('board_alias')
            if not isinstance(proof,str) or not proof.strip():
                out[name]='UNSUBSTANTIATED_CLAIM'
            elif device!=board['alias']:
                out[name]='BOARD_BINDING_MISMATCH'
            else:
                out[name]=state+'_REVIEW_CANDIDATE'
        else:
            out[name]=state
    kernel=record.get('kernel',{})
    if not isinstance(kernel,dict):
        raise ValueError('kernel must be an object')
    mode=kernel.get('strategy','UNKNOWN')
    if mode not in ('UNKNOWN','MAINLINE','VENDOR','HYBRID'):
        raise ValueError('invalid kernel strategy')
    missing=[k for k in REQUIRED_PHONE if out[k] != 'PHYSICALLY_VERIFIED_REVIEW_CANDIDATE']
    return {
        'schema':'prime.mobile.device-port-audit.candidate.v0',
        'status':'DEVELOPMENT_CLAIM_AUDIT_NOT_TRUST_OR_RELEASE',
        'evaluated_at':observed_at or datetime.now(timezone.utc).isoformat(timespec='seconds'),
        'board_alias':board['alias'],
        'kernel_strategy_proposed':mode,
        'features':out,
        'counts':dict(sorted(Counter(out.values()).items())),
        'phone_essentials_missing_review':sorted(missing),
        'claimed_phone_essentials_complete':not missing,
        'physical_acceptance':False,
        'device_port_installed':False,
        'kernel_or_firmware_mutated':False,
        'production_release_authorized':False,
        'limitations':[
            'A developer-supplied evidence reference is not independent physical certification.',
            'Linux kernel/device-tree compatibility and vendor licensing must be reviewed separately.',
            'Driver trust and image generation updates are owned by Prime system authority, not this manifest.',
        ]
    }


def main():
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('manifest',type=Path)
    p.add_argument('--output',type=Path)
    args=p.parse_args()
    result=audit(json.loads(args.manifest.read_text(encoding='utf-8')))
    body=json.dumps(result,indent=2,sort_keys=True)+'\n'
    if args.output:args.output.write_text(body,encoding='utf-8')
    else:print(body,end='')

if __name__=='__main__':main()