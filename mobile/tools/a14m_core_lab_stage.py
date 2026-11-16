#!/usr/bin/env python3
"""Integrate a real unqualified AArch64 primed Core into AWG1 A14m image STAGING.

Strictly a source-qualified laboratory composition. No device, no flashing,
no bootc image enrollment, no generated install ZIP and no boot success claim.
"""
from __future__ import annotations
import argparse
import gzip
import hashlib
import json
from pathlib import Path
import re
import struct
import sys
import zlib

from a14m_stage_boot import (KERNEL_GIT_BLOB, DTB_GIT_BLOB, boot_image,
                             git_blob, inspect_boot_image, parse_boot_info, sha)

SCHEMA = 'prime.mobile.a14m.source-bound-real-core-staging.unqualified.v0'
CORE_HEAD = '028aa1cf93f782d0fe8ec295a97c71a485661947'
SOURCE_GUEST_SCHEMA = 'prime.mob1.primed-guest-unqualified-bundle.v0'
MAX_COMPRESSED = 8 * 1024 * 1024
MAX_DECOMPRESSED = 64 * 1024 * 1024
MAX_FILES = 40
DIRS = {'dev', 'proc', 'sys', 'run', 'run/prime-mob1-lab', 'usr',
        'usr/bin', 'lib', 'lib/aarch64-linux-gnu'}
NODES = {'dev/console': (5, 1), 'dev/null': (1, 3)}
FILES = {'init', 'usr/bin/primed', 'lib/ld-linux-aarch64.so.1'}


def read_gzip_cpio(path: Path) -> tuple[bytes, bytes]:
    if not path.is_file() or not 0 < path.stat().st_size <= MAX_COMPRESSED:
        raise ValueError('CORE_INITRAMFS_MISSING_OR_OVERSIZED')
    encoded = path.read_bytes()
    try:
        decompressor = zlib.decompressobj(16 + zlib.MAX_WBITS)
        raw = decompressor.decompress(encoded, MAX_DECOMPRESSED + 1)
    except zlib.error as exc:
        raise ValueError('CORE_INITRAMFS_NOT_GZIP') from exc
    if (not decompressor.eof or decompressor.unused_data or decompressor.unconsumed_tail
            or len(raw) > MAX_DECOMPRESSED):
        raise ValueError('CORE_INITRAMFS_TRAILING_OR_EXCESSIVE')
    return encoded, raw


def entries_newc(raw: bytes) -> dict[str, tuple[int, int, int, bytes]]:
    pos = 0
    entries: dict[str, tuple[int,int,int,bytes]] = {}
    for i in range(MAX_FILES):
        if pos + 110 > len(raw) or raw[pos:pos+6] != b'070701':
            raise ValueError('CORE_CPIO_BAD_HEADER')
        try:
            fields = [int(raw[pos+6+j*8:pos+14+j*8], 16) for j in range(13)]
        except ValueError as exc:
            raise ValueError('CORE_CPIO_NON_HEX_HEADER') from exc
        mode, size, major, minor, namelen = (fields[1], fields[6], fields[9],
                                              fields[10], fields[11])
        if not (1 <= namelen <= 256):
            raise ValueError('CORE_CPIO_BAD_NAME_LENGTH')
        name_start = pos + 110
        if name_start + namelen > len(raw):
            raise ValueError('CORE_CPIO_TRUNCATED_NAME')
        field = raw[name_start:name_start+namelen]
        if not field.endswith(b'\x00'):
            raise ValueError('CORE_CPIO_UNTERMINATED_NAME')
        try:
            name = field[:-1].decode('ascii')
        except UnicodeDecodeError as exc:
            raise ValueError('CORE_CPIO_NONASCII_NAME') from exc
        if (not name or name.startswith('/') or '..' in name.split('/')
                or name in entries or '\\' in name):
            raise ValueError('CORE_CPIO_UNSAFE_OR_DUPLICATE_PATH')
        start = (name_start + namelen + 3) & ~3
        if start + size > len(raw):
            raise ValueError('CORE_CPIO_TRUNCATED_DATA')
        payload = raw[start:start+size]
        entries[name] = (mode, major, minor, payload)
        pos = (start + size + 3) & ~3
        if name == 'TRAILER!!!':
            if size or mode or pos != len(raw):
                raise ValueError('CORE_CPIO_TRAILER_INVALID')
            break
    else:
        raise ValueError('CORE_CPIO_TRAILER_MISSING')
    return entries


def aarch64_elf(data: bytes, label: str, *, needs_interpreter: bool, static_only: bool = False) -> None:
    if (len(data) < 64 or data[:6] != b'\x7fELF\x02\x01'
            or struct.unpack_from('<H', data, 18)[0] != 183):
        raise ValueError(label + '_NOT_ARM64_ELF')
    phoff = struct.unpack_from('<Q', data, 32)[0]
    entsize, count = struct.unpack_from('<HH', data, 54)
    if not (1 <= count <= 128 and entsize >= 56
            and phoff + entsize * count <= len(data)):
        raise ValueError(label + '_BAD_PROGRAM_HEADERS')
    interp = None
    for i in range(count):
        off = phoff + i * entsize
        typ = struct.unpack_from('<I', data, off)[0]
        if typ == 3:
            poff, size = struct.unpack_from('<QQ', data, off+8)[0], struct.unpack_from('<Q', data, off+32)[0]
            if size > 256 or poff + size > len(data):
                raise ValueError(label + '_BAD_INTERPRETER')
            interp = data[poff:poff+size]
        if typ == 2 and static_only:
            raise ValueError(label + '_NOT_STATIC')
    if needs_interpreter:
        if interp != b'/lib/ld-linux-aarch64.so.1\x00':
            raise ValueError(label + '_WRONG_ARM64_LOADER')
    elif static_only and interp is not None:
        raise ValueError(label + '_NOT_STATIC')
    elif interp not in (None, b'/lib/ld-linux-aarch64.so.1\x00'):
        raise ValueError(label + '_UNEXPECTED_ARM64_INTERPRETER')


def check_core_bundle(raw: bytes, encoded: bytes, src: dict,
                      kernel_digest: str) -> dict:
    if src.get('schema') != SOURCE_GUEST_SCHEMA or src.get('admission') != 'UNQUALIFIED_LAB_TRANSPORT_ONLY_NOT_BOOTC':
        raise ValueError('CORE_SOURCE_MANIFEST_NOT_LAB')
    for flag in ('bootc_qualified', 'prime_core_deployment_proven',
                 'physical_phone_supported', 'writes_real_hardware'):
        if src.get(flag) is not False:
            raise ValueError('CORE_SOURCE_PRODUCTION_FLAG:' + flag)
    if sha(raw) != src.get('initramfs_uncompressed_sha256') or sha(encoded) != src.get('initramfs_sha256'):
        raise ValueError('CORE_SOURCE_INITRAMFS_DIGEST_MISMATCH')
    entries = entries_newc(raw)
    expected = src.get('entries')
    if not isinstance(expected, list) or len(expected) != len(entries) or set(expected) != set(entries):
        raise ValueError('CORE_SOURCE_ENTRIES_MISMATCH')
    libs = src.get('needed')
    if not isinstance(libs, list) or not libs or len(libs) != len(set(libs)):
        raise ValueError('CORE_SOURCE_LIBRARY_SET_INVALID')
    if any(not re.fullmatch(r'lib[A-Za-z0-9_.+-]+\.so(?:\.\d+)*', lib) for lib in libs):
        raise ValueError('CORE_SOURCE_LIBRARY_NAME_INVALID')
    required = DIRS | set(NODES) | FILES | {'lib/aarch64-linux-gnu/' + n for n in libs} | {'TRAILER!!!'}
    if set(entries) != required:
        raise ValueError('CORE_SOURCE_UNEXPECTED_CPIO_MEMBER')
    for directory in DIRS:
        mode, _, _, payload = entries[directory]
        if mode & 0o170000 != 0o040000 or payload:
            raise ValueError('CORE_SOURCE_BAD_DIRECTORY:' + directory)
    for name, (major, minor) in NODES.items():
        mode, maj, mi, data = entries[name]
        if (mode & 0o170000 != 0o020000 or data or (maj,mi)!=(major,minor)):
            raise ValueError('CORE_SOURCE_BAD_NODE:' + name)
    for name in FILES | {'lib/aarch64-linux-gnu/' + n for n in libs}:
        mode, _, _, data = entries[name]
        if mode & 0o170000 != 0o100000 or not mode & 0o111 or not data:
            raise ValueError('CORE_SOURCE_NONEXECUTABLE:' + name)
        aarch64_elf(data, name, needs_interpreter=(name=='usr/bin/primed'), static_only=(name=='init'))
    init = entries['init'][3]
    core = entries['usr/bin/primed'][3]
    if sha(init) != src.get('arm64_guest_init_sha256') or sha(core) != src.get('arm64_core_sha256'):
        raise ValueError('CORE_SOURCE_ELF_DIGEST_MISMATCH')
    if (b'PRIME_MOB1_CORE_V0|stage=TRANSPORT_ONLY_UNQUALIFIED' not in init
        or b'PRIME_A14M_SOURCE_BRIDGE|not_a_phone_boot_proof=true' not in init):
        raise ValueError('NOT_A14M_CORE_LAB_INIT')
    binding = b'PRIME_MOB1_LAB_INPUT_DIGEST=sha256:' + kernel_digest.encode('ascii')
    if binding not in init:
        raise ValueError('CORE_INIT_WRONG_HISTORICAL_SOURCE_DIGEST')
    return {'core_sha256':sha(core), 'init_sha256':sha(init),
            'loader_sha256':sha(entries['lib/ld-linux-aarch64.so.1'][3]),
            'aarch64_dependencies':{x:sha(entries['lib/aarch64-linux-gnu/'+x][3]) for x in libs},
            'entry_count':len(entries), 'ramdisk_sha256':sha(encoded),
            'ramdisk_uncompressed_bytes':len(raw), 'source_kernel_digest':kernel_digest}


def stage(kernel_path: Path, dtb_path: Path, info_path: Path,
          initrd_path: Path, core_manifest_path: Path,
          out: Path, source_commit: str, core_commit: str) -> dict:
    if not re.fullmatch('[a-f0-9]{40}', source_commit) or core_commit != CORE_HEAD:
        raise ValueError('UNPINNED_SOURCE_COMMIT')
    for path in (kernel_path, dtb_path, info_path, core_manifest_path):
        if not path.is_file() or not 0 < path.stat().st_size <= 32*1024*1024:
            raise ValueError('SOURCE_FILE_MISSING_OR_OVERSIZED')
    kernel = kernel_path.read_bytes()
    dtb = dtb_path.read_bytes()
    if git_blob(kernel) != KERNEL_GIT_BLOB or git_blob(dtb) != DTB_GIT_BLOB:
        raise ValueError('NOT_AWG1_PINNED_SOURCE_BYTES')
    info = parse_boot_info(info_path.read_text())
    src = json.loads(core_manifest_path.read_text())
    zipped, raw = read_gzip_cpio(initrd_path)
    core = check_core_bundle(raw, zipped, src, sha(kernel))
    image = boot_image(kernel, zipped, dtb, info)
    inspected = inspect_boot_image(image, info, kernel, zipped, dtb)
    out.mkdir(parents=True, exist_ok=True)
    image_name='a14m-core-lab.reference-v2.DO_NOT_FLASH.boot.img'
    image_path=out/image_name
    image_path.write_bytes(image)
    receipt = {
        'schema': SCHEMA,
        'scope':'SOURCE_QUALIFIED_UNQUALIFIED_LAB_ONLY',
        'source_commit':source_commit, 'core_source_commit':core_commit,
        'historical_kernel_git_blob':KERNEL_GIT_BLOB,
        'historical_dtb_git_blob':DTB_GIT_BLOB,
        'image_sha256':sha(image), 'image_bytes':len(image),
        'image_name':image_name,'image_header':inspected, 'core':core,
        'primed_arm64_bytes_integrated':True,
        'core_running_on_generic_qemu_proven_by_separate_mob1':True,
        'core_running_on_samsung_proven':False,
        'bootc_generation_bound':False,'device_boot_chain_qualified':False,
        'physical_phone_boot_observed':False,'recovery_fallback_qualified':False,
        'stock_restoration_qualified':False,'prime_shell_ready':False,
        'phone_hardware_supported':False,'device_flash_authorized':False,
        'needs_real_samsung_device_for_boot_acceptance':True,
        'notes':['Uses historic AWG1 boot kernel/DT table, not the current phone bootchain.',
                 'Production Prime Core bootc generation admission unchanged; this is compile-gated LAB only.',
                 'Only Linux PID1/primed transport; no mobile UI, vendor HALs, storage persistence, telephony or suspend.',
                 'No TWRP/stock partition access, no Odin/TWRP/fastboot/OEM flash output.'],
    }
    (out/'source-bridge-receipt.json').write_text(json.dumps(receipt,indent=2,sort_keys=True)+'\n')
    return receipt


def main() -> None:
    p=argparse.ArgumentParser(description=__doc__)
    for arg in ('kernel','dtb','boot-info','initramfs','core-manifest','out'):
        p.add_argument('--'+arg,required=True,type=Path)
    p.add_argument('--source-commit',required=True)
    p.add_argument('--core-source-commit',required=True)
    a=p.parse_args()
    print(json.dumps(stage(a.kernel,a.dtb,a.boot_info,a.initramfs,a.core_manifest,a.out,
                           a.source_commit,a.core_source_commit),indent=2,sort_keys=True))

if __name__ == '__main__': main()
