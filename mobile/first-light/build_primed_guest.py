#!/usr/bin/env python3
"""Build isolated generic ARM64 Linux Core transport-test initramfs.

NOT a bootc/OSTree deployment or a mobile release. Every executable is an
actual arm64 ELF; the script refuses non-arm64 bytes and missing glibc libs.
"""
from __future__ import annotations
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys

# Reuse the already-reviewed kernel initramfs newc encoder.
from build import newc_entry

def sha(b: bytes) -> str: return hashlib.sha256(b).hexdigest()

def elf_arm64(data:bytes) -> None:
    if len(data)<64 or data[:6]!=b'\x7fELF\x02\x01' or int.from_bytes(data[18:20],'little')!=183:
        raise ValueError('NOT_ARM64_ELF')

def inspect_needed(executable:Path)->list[str]:
    run=subprocess.run(['readelf','-d',str(executable)],check=True,capture_output=True,text=True)
    names=re.findall(r'Shared library: \[(.+?)\]',run.stdout)
    if not names: raise ValueError('EXPECTED_DYN_LINKED_PRIMED')
    if len(names)>25 or any('/' in n or n.startswith('.') for n in names):raise ValueError('UNSAFE_SHARED_LIBRARY_NAME')
    return sorted(set(names))

def build(core:Path,init:Path,libs:Path,out:Path)->dict:
    core_bytes=core.read_bytes(); init_bytes=init.read_bytes()
    elf_arm64(core_bytes);elf_arm64(init_bytes)
    required=inspect_needed(core)
    root=libs.resolve()
    # Ubuntu cross-built ARM64 libc root. No host-x86 binaries are embedded.
    loader=root/'ld-linux-aarch64.so.1'
    if not loader.is_file():raise ValueError('ARM64_DYNAMIC_LOADER_MISSING')
    entries=[]
    for d in ['dev','proc','sys','run','run/prime-mob1-lab','usr','usr/bin','lib','lib/aarch64-linux-gnu']:
        entries.append((d,b'',0o040755,0,0))
    entries.extend([('dev/console',b'',0o020600,5,1),('dev/null',b'',0o020666,1,3)])
    entries.append(('init',init_bytes,0o100755,0,0))
    entries.append(('usr/bin/primed',core_bytes,0o100755,0,0))
    ld_bytes=loader.read_bytes();elf_arm64(ld_bytes)
    entries.append(('lib/ld-linux-aarch64.so.1',ld_bytes,0o100755,0,0))
    for name in required:
        candidate=root/name
        if not candidate.is_file():raise ValueError('MISSING_ARM64_DEPENDENCY:'+name)
        binary=candidate.read_bytes();elf_arm64(binary)
        entries.append(('lib/aarch64-linux-gnu/'+name,binary,0o100755,0,0))
    entries.append(('TRAILER!!!',b'',0,0,0))
    if len(entries)>40: raise ValueError('UNBOUNDED_ARCHIVE_ENTRIES')
    archive=b''.join(newc_entry(name,b,mode,i+1,maj,min) for i,(name,b,mode,maj,min) in enumerate(entries))
    if len(archive)>64*1024*1024:raise ValueError('ARCHIVE_TOO_LARGE')
    zipped=gzip.compress(archive,compresslevel=9,mtime=0)
    out.mkdir(parents=True,exist_ok=True)
    (out/'primed-lab-initramfs.cpio.gz').write_bytes(zipped)
    manifest={'schema':'prime.mob1.primed-guest-unqualified-bundle.v0',
      'admission':'UNQUALIFIED_LAB_TRANSPORT_ONLY_NOT_BOOTC',
      'arm64_core_sha256':sha(core_bytes),'arm64_guest_init_sha256':sha(init_bytes),
      'initramfs_sha256':sha(zipped),'initramfs_uncompressed_sha256':sha(archive),
      'needed':required,'entries':[x[0] for x in entries],
      'bootc_qualified':False,'prime_core_deployment_proven':False,
      'physical_phone_supported':False,'writes_real_hardware':False}
    (out/'primed-lab-manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
    return manifest

if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for key in ('core','init','libroot','out'):p.add_argument('--'+key,required=True,type=Path)
    a=p.parse_args();print(json.dumps(build(a.core,a.init,a.libroot,a.out),indent=2,sort_keys=True))
