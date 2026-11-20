#!/usr/bin/env python3
"""Package an independently bootable *generic QEMU ARM64 laboratory* ZIP.

Never intended for Samsung OTA / fastboot / vendor flash tools. No image
re-signing or Android installation output. Standard Python library only.
"""
from __future__ import annotations
import argparse
import hashlib
import json
from pathlib import Path
import stat
import zipfile
import zlib

KIT_ROOT = 'prime-mobile-arm64-qemu-lab'
SCHEMA = 'prime.mobile.arm64.qemu-bootkit.unqualified.v1'
MAX_KERNEL = 128 * 1024 * 1024
MAX_COMPRESSED_INITRD = 32 * 1024 * 1024
MAX_DECOMPRESSED_INITRD = 128 * 1024 * 1024
MAX_VERIFIER = 64 * 1024


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_bounded(path: Path, max_size: int, description: str) -> bytes:
    if not path.is_file() or not 0 < path.stat().st_size <= max_size:
        raise ValueError(f'{description}: MISSING_OR_OVERSIZED')
    return path.read_bytes()


def read_initrd(path: Path) -> bytes:
    compressed = read_bounded(path, MAX_COMPRESSED_INITRD, 'INITRAMFS')
    try:
        decoder = zlib.decompressobj(16 + zlib.MAX_WBITS)
        decoded = decoder.decompress(compressed, MAX_DECOMPRESSED_INITRD + 1)
    except zlib.error as exc:
        raise ValueError('INITRAMFS_NOT_GZIP') from exc
    if (not decoder.eof or decoder.unused_data or decoder.unconsumed_tail
            or len(decoded) > MAX_DECOMPRESSED_INITRD):
        raise ValueError('INITRAMFS_INVALID_OR_EXCESSIVE')
    if not decoded.startswith(b'070701') or b'TRAILER!!!\0' not in decoded:
        raise ValueError('INITRAMFS_NOT_NEWC_CPIO')
    if b'usr/bin/primed\0' not in decoded:
        raise ValueError('INITRAMFS_NO_PRIMED')
    if b'init\0' not in decoded:
        raise ValueError('INITRAMFS_NO_INIT')
    return compressed


def run_script() -> bytes:
    return r'''#!/usr/bin/env bash
# Generic ARM64 Linux + Prime Core transport test; not a Samsung ROM/flash ZIP.
set -euo pipefail
HERE="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)"
OUTPUT_DIR="${1:-$PWD/prime-mobile-qemu-test-results}"
mkdir -p -- "$OUTPUT_DIR"
OUTPUT_DIR="$(cd -- "$OUTPUT_DIR" && pwd -P)"
for cmd in sha256sum qemu-system-aarch64 timeout python3; do
  command -v "$cmd" >/dev/null 2>&1 || { echo "MISSING_TOOL:$cmd" >&2; exit 69; }
done
(cd "$HERE" && sha256sum --check SHA256SUMS)
QEMU="$(command -v qemu-system-aarch64)"
"$QEMU" --version | head -n 1
sha256sum "$QEMU" > "$OUTPUT_DIR/qemu-host-sha256.txt"
set +e
timeout --signal=KILL 55s "$QEMU" \
  -machine virt -cpu cortex-a57 -m 768M \
  -display none -serial stdio -monitor none -no-reboot -net none -nodefaults \
  -kernel "$HERE/arm64-linux-Image" \
  -initrd "$HERE/prime-core-lab-initramfs.cpio.gz" \
  -append 'console=ttyAMA0 rdinit=/init loglevel=4' \
  > "$OUTPUT_DIR/guest-uart.log" 2> "$OUTPUT_DIR/qemu-stderr.log"
EXIT_CODE=$?
set -e
if [[ "$EXIT_CODE" -ne 124 && "$EXIT_CODE" -ne 137 ]]; then
  echo "UNEXPECTED_QEMU_EXIT:$EXIT_CODE" >&2
  exit 70
fi
python3 "$HERE/verify_primed_guest.py" "$OUTPUT_DIR/guest-uart.log" \
  > "$OUTPUT_DIR/core-transport-receipt.json"
python3 - "$OUTPUT_DIR/core-transport-receipt.json" <<'PY_CHECK'
import json,sys
p=sys.argv[1]
r=json.load(open(p,encoding='utf-8'))
assert r['transport_responded'] is True, r
assert r['blockers']==[],r
assert r['status']=='CORE_TRANSPORT_RESPONDED_UNQUALIFIED',r
assert r['capability_count']==7,r
assert r['bootc_generation_bound'] is False and r['core_deployment_accepted'] is False,r
assert r['flash_authorization'] is False and r['phone_qualified'] is False,r
print('PRIME_MOBILE_QEMU_ZIP_BOOT_TEST_PASS:UNQUALIFIED_LAB_ONLY')
PY_CHECK
(cd "$OUTPUT_DIR" && sha256sum guest-uart.log qemu-stderr.log core-transport-receipt.json \
  > OUTPUT_SHA256SUMS)
echo "Results directory: $OUTPUT_DIR"
''' .encode('utf-8')


def readme() -> bytes:
    return '''# Prime Mobile — ARM64 Linux + real Prime Core QEMU test kit

This is an executable **generic QEMU ARM64 laboratory test** package.
It is **not** a Samsung SM-A145P ROM, OTA ZIP, TWRP update, USB boot image,
Android firmware, installer, production Prime system, or flashable artifact.

Contents:
- arm64-linux-Image — verified Ubuntu ARM64 Linux kernel, raw Image format
- prime-core-lab-initramfs.cpio.gz — built existing Prime `primed` process,
  AArch64 Linux PID1, ARM64 glibc libraries, and RAM-only test generation
- run-qemu.sh — launches real Linux using local qemu-system-aarch64, no disks,
  no USB passthrough, no guest network, bounded 55-second lifetime
- verify_primed_guest.py — re-checks actual Core socket UART responses
- SHA256SUMS and manifest.json — hash pins and precise proof limitations

To test on a Linux machine with qemu-system-aarch64, Python 3, GNU timeout
and sha256sum installed:

    unzip prime-mobile-arm64-qemu-lab.zip
    bash prime-mobile-arm64-qemu-lab/run-qemu.sh ./prime-mobile-results

On a GitHub Ubuntu runner the same run script is executed on the **extracted**
ZIP. This is distinct from an earlier CI run that booted the source build.
The diagnostic test deliberately stops QEMU after 55 seconds and checks
`/v1/versions` (200), missing version (400), incompatible version (409),
`/v1/capabilities` (200), and seven real Core descriptors. The script will
FAIL if these markers are missing or inconsistent.

These tests use the authentic Prime Rust `primed` socket server in a
**compile-gated, unqualified lab-only generation**. It does not have a
bootc-installed system, persistent storage, known-good generation, full
service/recovery lifecycle, host identity enrollment, Prime Shell, or
physical device hardware support. Do not copy this ZIP to a phone or flash it.

The build manifest records exact input hashes, source commit and provenance.
Nothing in this kit changes your physical machines when used in QEMU;
QEMU is not a security sandbox for arbitrary malicious guest files. Only run
this pinned, reviewed test kit in an appropriate isolated environment.
'''.encode('utf-8')


def create_zip(kernel: Path, initrd: Path, verifier: Path,
               output: Path, commit: str, kernel_package_sha256: str) -> dict:
    if len(commit) != 40 or any(c not in '0123456789abcdef' for c in commit):
        raise ValueError('INVALID_SOURCE_COMMIT')
    if len(kernel_package_sha256) != 64 or any(c not in '0123456789abcdef' for c in kernel_package_sha256):
        raise ValueError('INVALID_KERNEL_PACKAGE_PIN')
    k = read_bounded(kernel, MAX_KERNEL, 'KERNEL')
    if k[56:60] != b'ARMd':
        raise ValueError('NOT_RAW_ARM64_LINUX_IMAGE')
    i = read_initrd(initrd)
    v = read_bounded(verifier, MAX_VERIFIER, 'VERIFIER')
    if b'CORE_TRANSPORT_RESPONDED_UNQUALIFIED' not in v or b'bootc_generation_bound' not in v:
        raise ValueError('WRONG_PROOF_VERIFIER')
    items = {
        'arm64-linux-Image': k,
        'prime-core-lab-initramfs.cpio.gz': i,
        'verify_primed_guest.py': v,
        'run-qemu.sh': run_script(),
        'README.md': readme(),
    }
    sums = ''.join(f'{digest(items[name])}  {name}\n' for name in sorted(items))
    manifest = {
        'schema': SCHEMA,
        'source_commit': commit,
        'linux_kernel_deb_sha256': kernel_package_sha256,
        'files': {name: {'bytes': len(data), 'sha256': digest(data)}
                  for name, data in sorted(items.items())},
        'scope': 'GENERIC_QEMU_LINUX_PRIMED_SOCKET_LAB_ONLY',
        'bootc_generation_bound': False,
        'core_deployment_accepted': False,
        'physical_phone_qualified': False,
        'flash_authorization': False,
        'contains_qemu_emulator': False,
        'requires_qemu_system_aarch64': True,
    }
    items['SHA256SUMS'] = sums.encode('utf-8')
    items['manifest.json'] = (json.dumps(manifest, indent=2, sort_keys=True) + '\n').encode('utf-8')
    output.parent.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED, compresslevel=9) as z:
        for name, data in sorted(items.items()):
            info = zipfile.ZipInfo(f'{KIT_ROOT}/{name}', date_time=(2026, 11, 15, 0, 0, 0))
            info.compress_type = zipfile.ZIP_DEFLATED
            info.external_attr = ((0o100755 if name == 'run-qemu.sh' else 0o100644) << 16)
            z.writestr(info, data, compress_type=zipfile.ZIP_DEFLATED, compresslevel=9)
    with zipfile.ZipFile(output) as z:
        assert z.testzip() is None
        for name, data in items.items():
            assert z.read(f'{KIT_ROOT}/{name}') == data
    return {'schema': SCHEMA, 'zip_sha256': digest(output.read_bytes()),
            'size_bytes': output.stat().st_size, 'source_commit': commit,
            'kernel_sha256': digest(k), 'initramfs_sha256': digest(i),
            'archive_members': len(items), 'bootable_in_qemu': True,
            'flash_authorization': False, 'core_deployment_accepted': False}


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--kernel', required=True, type=Path)
    p.add_argument('--initrd', required=True, type=Path)
    p.add_argument('--verifier', required=True, type=Path)
    p.add_argument('--output', required=True, type=Path)
    p.add_argument('--commit', required=True)
    p.add_argument('--kernel-package-sha256', required=True)
    a = p.parse_args()
    print(json.dumps(create_zip(a.kernel, a.initrd, a.verifier, a.output,
                               a.commit, a.kernel_package_sha256), indent=2, sort_keys=True))


if __name__ == '__main__':
    main()
