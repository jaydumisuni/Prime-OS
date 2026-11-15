#!/usr/bin/env python3
"""Samsung SM-A145P (a14m) read-only stock provenance and mismatch intake.

Development candidate; does not read or write firmware partitions, identify a
specific person, unlock a bootloader, or authorize device flashing. Requires
further independent verification even if every captured property matches.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess
from typing import Any, Callable

SCHEMA = 'prime.mobile.a14m-source-intake.candidate.v0'
CAPTURE_SCHEMA = 'prime.mobile.a14m-adb-properties.candidate.v0'
MODEL = 'SM-A145P'
# getprop is never treated as proof of OEM unlock/downloading eligibility.
PROPS = (
    'ro.product.model', 'ro.product.device', 'ro.product.board',
    'ro.hardware', 'ro.build.fingerprint', 'ro.build.version.release',
    'ro.build.version.security_patch', 'ro.boot.verifiedbootstate',
    'ro.boot.flash.locked', 'ro.boot.vbmeta.device_state',
    'ro.boot.bootloader',
)
ALLOWED_CMDS = tuple(("shell", "getprop", prop) for prop in PROPS) + (
    ('shell', 'uname', '-r'),
    ('shell', 'ls', '-1', '/dev/block/by-name'),
)
# These are public reference identifiers, NOT device measurements.
FIRMWARE_PATTERN = re.compile(r'^A145P[A-Z0-9]{5,20}$')
SAFE_PARTITION = re.compile(r'^[A-Za-z0-9_.-]{1,64}$')


def capture_adb(adb: str = 'adb', *, runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run) -> dict[str, Any]:
    """Read-only Android properties. Requires one authorized, ADB-online phone.

    No `adb root`, `pull`, `push`, `reboot`, `shell su`, partition reads or
    mutating commands. Never print or persist device serial/IMEI/MAC.
    """
    if not isinstance(adb,str) or not adb.strip():
        raise ValueError('ADB_PATH_REQUIRED')

    def run(*args: str) -> tuple[bool,str]:
        try:
            result = runner([adb, *args], text=True, capture_output=True, timeout=8, check=False)
        except (subprocess.TimeoutExpired, OSError):
            # Exceptions can include the ADB transport serial in argv. Never
            # propagate raw subprocess exception text into public receipts.
            return False, ''
        return result.returncode == 0, (result.stdout or '').strip()[:4096]

    # `adb devices` prints serial numbers; inspect only internally and discard.
    ok, raw = run('devices')
    if not ok:
        raise RuntimeError('ADB_DEVICE_ENUMERATION_FAILED')
    devices = [line.split('\t', 1) for line in raw.splitlines()[1:] if '\t' in line]
    online = [d for d in devices if len(d)==2 and d[1].strip()=='device']
    if len(online)!=1 or len(devices)!=1:
        raise RuntimeError('REQUIRE_EXACTLY_ONE_AUTHORIZED_ADB_DEVICE')
    serial = online[0][0]  # private transport token is NOT included in output
    if not serial or any(ch in serial for ch in '\n\r\x00'):
        raise RuntimeError('INVALID_ADB_TRANSPORT')
    # Pin the selected device for every subsequent read to avoid default-device races.
    def selected(*args: str) -> tuple[bool,str]:
        return run('-s', serial, *args)
    properties: dict[str, str | None] = {}
    failures: list[str] = []
    for cmd in ALLOWED_CMDS:
        passed,value=selected(*cmd)
        key=cmd[-1] if cmd[:2] == ('shell','getprop') else ('kernel_release' if cmd[1]=='uname' else 'partitions_by_name')
        if not passed or not value:
            properties[key]=None
            failures.append(key)
        else:
            properties[key]=value
    part = properties.pop('partitions_by_name')
    partitions = [] if part is None else [name for name in part.splitlines() if SAFE_PARTITION.fullmatch(name)]
    if part is not None and len(partitions)!=len(part.splitlines()):
        failures.append('PARTITION_NAMES_UNQUALIFIED')
        partitions=[]
    if len(partitions)>128:
        failures.append('PARTITION_ENUMERATION_TOO_LARGE')
        partitions=[]
    return {
        'schema':CAPTURE_SCHEMA,
        'source_kind':'LIVE_READ_ADB_PROPERTIES_ONLY',
        'observed_at':datetime.now(timezone.utc).isoformat(timespec='seconds'),
        'properties':properties,
        'partition_names':sorted(set(partitions)),
        'unavailable_fields':sorted(set(failures)),
        'source_limitations':[
            'ADB properties are claims from the running Android userspace, not Download Mode lock/AVB proof',
            'Partition names are a non-privileged directory listing, not partition geometry or image compatibility',
            'Capture alone cannot validate stock restoration, kernel ABI, signed images or flash permission',
        ],
        'device_write_authorized':False,
        'physical_boot_qualified':False,
        'device_serial_recorded':False,
    }


def assess(source: dict[str, Any], capture: dict[str, Any] | None = None) -> dict[str, Any]:
    if not isinstance(source,dict) or source.get('schema')!=SCHEMA:
        raise ValueError('SOURCE_INTAKE_SCHEMA_MISMATCH')
    target=source.get('target')
    if not isinstance(target,dict) or target.get('proposed_model')!=MODEL:
        raise ValueError('WRONG_PROPOSED_MODEL')
    leads=source.get('public_reference_sources')
    if not isinstance(leads,list) or not leads:
        raise ValueError('REFERENCE_SOURCES_MISSING')
    for item in leads:
        if not isinstance(item,dict) or not isinstance(item.get('url'),str) or not item['url'].startswith('https://'):
            raise ValueError('SOURCE_INVALID')
        if item.get('kind','').startswith('COMMUNITY_') and not re.fullmatch(r'[0-9a-f]{40}',str(item.get('exact_commit',''))):
            raise ValueError('UNPINNED_COMMUNITY_SOURCE')
    reasons = ['STOCK_RESTORE_NOT_VERIFIED','BOOT_IMAGE_NOT_BUILT','SIGNED_BOOT_CHAIN_NOT_VERIFIED']
    model_state = 'NOT_OBSERVED'
    current_build = None
    partitions = []
    if capture is not None:
        if not isinstance(capture,dict) or capture.get('schema')!=CAPTURE_SCHEMA or capture.get('source_kind')!='LIVE_READ_ADB_PROPERTIES_ONLY':
            raise ValueError('UNQUALIFIED_CAPTURE')
        props=capture.get('properties')
        if not isinstance(props,dict) or set(props)-set(PROPS)-{'kernel_release'}:
            raise ValueError('UNEXPECTED_CAPTURE_FIELDS')
        model=props.get('ro.product.model')
        board=props.get('ro.product.device')
        model_state = 'MATCHED_ANDROID_PROPERTY_NOT_HARDWARE_ATTESTED' if model==MODEL and board=='a14m' else 'MISMATCH_OR_UNKNOWN'
        if model_state!='MATCHED_ANDROID_PROPERTY_NOT_HARDWARE_ATTESTED':
            reasons.append('DEVICE_MODEL_OR_BOARD_MISMATCH')
        fp=props.get('ro.build.fingerprint')
        if isinstance(fp,str):
            # Samsung fingerprint normally contains a PDA/release marker; refuse guesses.
            for token in re.split(r'[/.:]',fp):
                if FIRMWARE_PATTERN.fullmatch(token):
                    current_build=token
                    break
        if current_build is None:
            reasons.append('CURRENT_BUILD_NOT_PROVEN')
        historical={item['reported_firmware'] for item in leads if item.get('kind')=='COMMUNITY_KERNEL_SOURCE_OLD' and item.get('reported_firmware')}
        if current_build and current_build not in historical:
            reasons.append('HISTORICAL_KERNEL_SOURCE_DOES_NOT_MATCH_CAPTURED_FIRMWARE')
        partitions=capture.get('partition_names',[])
        if not isinstance(partitions,list) or len(partitions)>128 or any(not isinstance(x,str) or not SAFE_PARTITION.fullmatch(x) for x in partitions):
            raise ValueError('INVALID_PARTITION_NAMES')
        if not partitions:reasons.append('PARTITION_TOPOLOGY_NOT_OBSERVED')
        else:reasons.append('PARTITION_GEOMETRY_AND_BOOT_CONTENTS_NOT_PROVEN')
    else:
        reasons.append('LIVE_PHONE_CAPTURE_MISSING')
    return {
        'schema':'prime.mobile.a14m-stock-preflight.candidate.v0',
        'status':'STOP_DEVICE_IMAGE_NOT_READY',
        'proposed_model':MODEL,
        'model_observation':model_state,
        'captured_build_candidate':current_build,
        'public_latest_referenced_build':next((x.get('listed_build') for x in leads if x.get('kind')=='OFFICIAL_FIRMWARE_RELEASE_NOTES'),None),
        'partition_names_observed_unverified_geometry':sorted(set(partitions)),
        'blockers':sorted(set(reasons)),
        'host_side_fixtures_or_references_can_be_used_for_research':True,
        'real_phone_identity_cryptographically_verified':False,
        'bootloader_unlock_qualified':False,
        'boot_image_compatible':False,
        'stock_restore_verified':False,
        'device_flash_authorized':False,
        'production_release_authorized':False,
        'limitations':[
            'A public latest Samsung firmware is not the phone installed firmware',
            'ADB getprop / device naming is not chip/fuse/boot-chain attestation',
            'An older public kernel/recovery tree is not evidence of current build compatibility',
            'Proof of signed boot compatibility and reversible stock restore requires separate reviewed physical evidence',
        ],
    }


def main() -> None:
    parser=argparse.ArgumentParser(description=__doc__)
    sub=parser.add_subparsers(dest='command',required=True)
    cap=sub.add_parser('capture',help='Read-only ADB properties from exactly one authorized device')
    cap.add_argument('--adb',default='adb')
    cap.add_argument('--out',type=Path,required=True)
    check=sub.add_parser('assess',help='Check source candidates and optional captured Android properties')
    check.add_argument('--sources',required=True,type=Path)
    check.add_argument('--capture',type=Path)
    check.add_argument('--out',type=Path)
    args=parser.parse_args()
    if args.command=='capture':
        output=capture_adb(args.adb)
        args.out.write_text(json.dumps(output,sort_keys=True,indent=2)+'\n',encoding='utf-8')
    else:
        sources=json.loads(args.sources.read_text(encoding='utf-8'))
        live=json.loads(args.capture.read_text(encoding='utf-8')) if args.capture else None
        out=json.dumps(assess(sources,live),sort_keys=True,indent=2)+'\n'
        if args.out:args.out.write_text(out,encoding='utf-8')
        else:print(out,end='')


if __name__=='__main__':
    main()
