#!/usr/bin/env python3
"""Build a deterministic ARM64 Linux early-init *fixture*, not Prime OS or an OEM image.

No network, package install, root access, QEMU execution or device interaction.
"""
from __future__ import annotations

import argparse
import gzip
import hashlib
import json
from pathlib import Path
import shutil
import struct
import subprocess

ROOT = Path(__file__).resolve().parent
SOURCE = ROOT / 'src' / 'early_init.c'
SCHEMA = 'prime.mobile.mob1.generic-arm64-init-build.candidate.v0'
FLAGS = ('-target', 'aarch64-linux-gnu', '-fuse-ld=lld', '-nostdlib',
         '-static', '-ffreestanding', '-fno-stack-protector', '-fno-pic',
         '-fno-builtin', '-Wall', '-Wextra', '-Werror',
         '-Wl,-e,_start', '-Wl,--build-id=none')
MARKERS = (b'PRIME_MOB1_V0|stage=INIT_ENTERED',
           b'PRIME_MOB1_V0|stage=EVIDENCE_ONLY_CORE_NOT_STARTED')


def sha(data: bytes) -> str:
    return 'sha256:' + hashlib.sha256(data).hexdigest()


def inspect_arm64_elf(data: bytes) -> dict:
    """No external objdump dependency: verify required ELF load/entry properties."""
    if len(data) < 64 or data[:4] != b'\x7fELF' or data[4:6] != b'\x02\x01':
        raise ValueError('NOT_ELF64_LITTLE_ENDIAN')
    kind, machine, version = struct.unpack_from('<HHI', data, 16)
    if kind not in (2, 3) or machine != 183 or version != 1:
        raise ValueError('NOT_AARCH64_EXECUTABLE')
    entry, phoff = struct.unpack_from('<QQ', data, 24)
    phentsize, phnum = struct.unpack_from('<HH', data, 54)
    if not entry or phentsize < 56 or not (1 <= phnum <= 128):
        raise ValueError('ELF_HEADER_INVALID')
    if phoff + phentsize * phnum > len(data):
        raise ValueError('ELF_PROGRAM_HEADERS_TRUNCATED')
    executable_entry = False
    load_segments = 0
    for idx in range(phnum):
        at = phoff + idx * phentsize
        ptype, flags = struct.unpack_from('<II', data, at)
        offset, vaddr, _, fsize, msize = struct.unpack_from('<QQQQQ', data, at + 8)
        if ptype == 3:
            raise ValueError('DYNAMIC_INTERPRETER_NOT_ALLOWED')
        if ptype == 2:
            raise ValueError('DYNAMIC_SEGMENT_NOT_ALLOWED')
        if ptype == 1:
            load_segments += 1
            if fsize > msize or offset + fsize > len(data):
                raise ValueError('ELF_LOAD_SEGMENT_INVALID')
            if flags & 1 and vaddr <= entry < vaddr + msize:
                executable_entry = True
    if not executable_entry or not load_segments:
        raise ValueError('ENTRY_NOT_IN_EXECUTABLE_LOAD_SEGMENT')
    if not all(marker in data for marker in MARKERS):
        raise ValueError('BOOT_EVIDENCE_MARKERS_MISSING')
    return {'architecture': 'aarch64', 'elf_type': 'ET_EXEC' if kind == 2 else 'ET_DYN',
            'entry_address': hex(entry), 'load_segments': load_segments,
            'static': True, 'evidence_only': True, 'bytes': len(data), 'sha256': sha(data)}


def newc_entry(name: str, data: bytes, mode: int, inode: int,
               rdevmajor: int = 0, rdevminor: int = 0) -> bytes:
    """Encode a root-owned newc entry including optional character-device IDs.

    The bytes encode device metadata directly, without requiring host mknod,
    root privileges, filesystem changes, or a real device being present.
    """
    if any(not 0 <= n < (1 << 32) for n in (inode, mode, rdevmajor, rdevminor)):
        raise ValueError('NEWC_METADATA_OUT_OF_RANGE')
    if mode & 0o170000 == 0o020000 and data:
        raise ValueError('DEVICE_NODE_CANNOT_CONTAIN_DATA')
    name_bytes = name.encode('ascii') + b'\0'
    fields = (inode, mode, 0, 0, 2 if mode & 0o040000 else 1,
              0, len(data), 0, 0, rdevmajor, rdevminor, len(name_bytes), 0)
    head = b'070701' + b''.join(f'{f:08x}'.encode('ascii') for f in fields)
    record = head + name_bytes
    record += b'\0' * (-len(record) % 4)
    record += data
    record += b'\0' * (-len(data) % 4)
    return record


def initramfs(init_elf: bytes) -> bytes:
    # Explicitly include Linux character devices rather than depending on
    # which particular kernel ships its own default rootfs nodes. An absent
    # /dev/console can leave PID1's stdout/stderr closed.
    items = [
        ('dev', b'', 0o040755, 0, 0),
        ('dev/console', b'', 0o020600, 5, 1),
        ('dev/null', b'', 0o020666, 1, 3),
        ('proc', b'', 0o040755, 0, 0),
        ('sys', b'', 0o040755, 0, 0),
        ('init', init_elf, 0o100755, 0, 0),
        ('TRAILER!!!', b'', 0, 0, 0),
    ]
    return b''.join(newc_entry(name, data, mode, i + 1, major, minor)
                    for i, (name, data, mode, major, minor) in enumerate(items))


def inspect_initramfs(raw: bytes) -> dict:
    """Fail closed unless /init, /dev/console and /dev/null are exact newc nodes."""
    pos = 0
    entries = {}
    while pos < len(raw):
        if pos + 110 > len(raw) or raw[pos:pos+6] != b'070701':
            raise ValueError('INVALID_CPIO_HEADER')
        try:
            values = [int(raw[pos+6+i*8:pos+14+i*8], 16) for i in range(13)]
        except ValueError as error:
            raise ValueError('INVALID_CPIO_FIELDS') from error
        mode, size, major, minor, namesize = (values[1], values[6],
                                               values[9], values[10], values[11])
        start = pos + 110
        if not (1 <= namesize <= 256) or start + namesize > len(raw):
            raise ValueError('INVALID_CPIO_NAME_LENGTH')
        name_bytes = raw[start:start+namesize]
        if not name_bytes.endswith(b'\0'):
            raise ValueError('INVALID_CPIO_NAME')
        try:
            name = name_bytes[:-1].decode('ascii')
        except UnicodeDecodeError as error:
            raise ValueError('INVALID_CPIO_NAME') from error
        payload_start = (start + namesize + 3) & ~3
        end = payload_start + size
        if end > len(raw) or name in entries:
            raise ValueError('DUPLICATE_OR_TRUNCATED_CPIO_ENTRY')
        entries[name] = (mode, major, minor, raw[payload_start:end])
        pos = (end + 3) & ~3
        if name == 'TRAILER!!!':
            break
    if pos != len(raw):
        raise ValueError('CPIO_TRAILING_BYTES')
    if set(entries) != {'dev','dev/console','dev/null','proc','sys','init','TRAILER!!!'}:
        raise ValueError('CPIO_REQUIRED_ENTRIES_MISSING_OR_UNEXPECTED')
    for name in ('dev','proc','sys'):
        if (entries[name][0] & 0o170000) != 0o040000 or entries[name][3]:
            raise ValueError('CPIO_DIRECTORY_INVALID:' + name)
    for name, (major, minor) in {'dev/console':(5,1), 'dev/null':(1,3)}.items():
        mode, actual_major, actual_minor, data = entries[name]
        if mode & 0o170000 != 0o020000 or (actual_major, actual_minor)!=(major, minor) or data:
            raise ValueError('CPIO_DEVICE_NODE_INVALID:' + name)
    if (entries['init'][0] & 0o170000) != 0o100000 or not entries['init'][0] & 0o111:
        raise ValueError('CPIO_INIT_NOT_EXECUTABLE')
    inspect_arm64_elf(entries['init'][3])
    return {'validated': True, 'entries': sorted(entries),
            'console': {'type':'character', 'major':5, 'minor':1},
            'null': {'type':'character', 'major':1, 'minor':3}}


def build(out: Path, compiler: str | None = None) -> dict:
    compiler = compiler or shutil.which('clang')
    if not compiler or not shutil.which('ld.lld'):
        raise RuntimeError('ARM64 Clang and LLD are required; no simulated compilation')
    out.mkdir(parents=True, exist_ok=True)
    elf_path = out / 'init-aarch64'
    cmd = [compiler, *FLAGS, str(SOURCE), '-o', str(elf_path)]
    subprocess.run(cmd, check=True, capture_output=True, text=True, timeout=60)
    elf_data = elf_path.read_bytes()
    inspection = inspect_arm64_elf(elf_data)
    raw = initramfs(elf_data)
    archive_inspection = inspect_initramfs(raw)
    (out / 'initramfs.cpio').write_bytes(raw)
    gz = gzip.compress(raw, compresslevel=9, mtime=0)
    (out / 'initramfs.cpio.gz').write_bytes(gz)
    compiler_version = subprocess.run([compiler, '--version'], capture_output=True,
                                      text=True, check=True, timeout=5).stdout.splitlines()[0]
    receipt = {
        'schema': SCHEMA, 'target': 'qemu-virt-generic-arm64-not-phone',
        'stage': 'CROSS_COMPILED_ONLY_NOT_EXECUTED', 'source_sha256': sha(SOURCE.read_bytes()),
        'compiler': compiler_version, 'compile_flags': list(FLAGS),
        'elf': inspection, 'initramfs_structure': archive_inspection,
        'initramfs_cpio_sha256': sha(raw),
        'initramfs_gzip_sha256': sha(gz),
        'evidence_marker_names': [m.decode('ascii') for m in MARKERS],
        'prime_core_compiled': False, 'arm64_guest_boot_observed': False,
        'physical_phone_supported': False, 'device_modification_performed': False,
        'limitations': [
            'Kernel Image and qemu-system-aarch64 needed for execution; neither is bundled.',
            'The C /init is disposable Linux early-boot evidence, not Prime Core or daily-driver PID 1.',
            'Linux userspace ARM64 ELF cannot boot alone or act as a Samsung boot/recovery image.',
            'No hardware driver, telephony, DRM, battery, OTA or secure boot acceptance.',
        ],
    }
    (out / 'build-receipt.json').write_text(json.dumps(receipt, indent=2, sort_keys=True) + '\n')
    return receipt


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--out', type=Path, required=True,
                        help='Local generated-artifact directory')
    parser.add_argument('--clang', default=None)
    options = parser.parse_args()
    print(json.dumps(build(options.out, options.clang), indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
