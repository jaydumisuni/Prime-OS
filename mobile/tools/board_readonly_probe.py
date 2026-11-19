#!/usr/bin/env python3
"""Read-only ARM64 board/Device Tree probe; never a boot/driver proof.

Development bridge for generic Linux device porting. No DMI or vendor-only
identifier required; no serial, MAC, IMEI, secure key or command line copied.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
import platform
from pathlib import Path

SCHEMA = 'prime.mobile.board-observation.candidate.v0'
MAX_SOURCE = 4096


def _read(root: Path, name: str) -> bytes | None:
    try:
        root = root.resolve(strict=True)
        path = (root / name.lstrip('/')).resolve(strict=True)
        if not path.is_relative_to(root) or not path.is_file() or path.stat().st_size > MAX_SOURCE:
            return None
        return path.read_bytes()
    except (OSError,ValueError):
        return None


def _decode(data: bytes | None) -> str | None:
    if not data:
        return None
    try:
        text = data.split(b'\0',1)[0].decode('utf-8').strip()
    except UnicodeError:
        return None
    return text if text and all(ord(c) >= 32 for c in text) else None


def _compatible(data: bytes | None) -> list[str] | None:
    if not data:
        return None
    if not data.endswith(b'\0'):
        return None
    try:
        items = [v.decode('utf-8') for v in data.split(b'\0')[:-1]]
    except UnicodeError:
        return None
    if not items or not all(v and len(v)<=128 and all(32<=ord(c)<127 for c in v) for v in items):
        return None
    return items[:32] if len(items)<=32 else None


def snapshot(root: Path = Path('/'), arch: str | None = None, observed_at: str | None = None) -> dict:
    arch=arch or platform.machine() or 'UNKNOWN'
    observed_at=observed_at or datetime.now(timezone.utc).isoformat(timespec='seconds')
    paths={
        'model': ['/proc/device-tree/model','/sys/firmware/devicetree/base/model'],
        'compatible': ['/proc/device-tree/compatible','/sys/firmware/devicetree/base/compatible'],
        'kernel_release': ['/proc/sys/kernel/osrelease'],
    }
    values={}
    sources={}
    limits=[]
    for field, options in paths.items():
        val=None;source=None
        for name in options:
            raw=_read(root,name)
            if raw is None: continue
            candidate=_compatible(raw) if field=='compatible' else _decode(raw)
            if candidate is not None:
                val=candidate;source=name;break
        values[field]=val;sources[field]=source
        if val is None:limits.append(field.upper()+'_NOT_OBSERVED')
    if arch.lower() not in ('aarch64','arm64'):
        limits.append('HOST_NOT_IDENTIFIED_AS_ARM64')
    return {
        'schema':SCHEMA,'status':'KERNEL_READ_ONLY_NOT_DEVICE_CERTIFICATION',
        'observed_at':observed_at,'host_arch':arch,
        'board_model':values['model'],'board_compatible':values['compatible'],
        'kernel_release':values['kernel_release'],'sources':sources,
        'limitations':sorted(limits),
        'proof_boundary': 'A kernel-projected board name or compatible string proves neither vendor identity, bootloader lock, full drivers, telephony, recovery nor safe flashing.'
    }


def main() -> None:
    p=argparse.ArgumentParser(description=__doc__)
    p.add_argument('--root',type=Path,default=Path('/'))
    p.add_argument('--arch',help='Explicit arch ONLY for a fixture. Does not prove physical architecture.')
    p.add_argument('--output',type=Path)
    a=p.parse_args()
    out=json.dumps(snapshot(a.root,a.arch),indent=2,sort_keys=True)+'\n'
    if a.output:a.output.write_text(out,encoding='utf-8')
    else:print(out,end='')


if __name__=='__main__':main()