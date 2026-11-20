#!/usr/bin/env python3
"""Hash-pinned, bounded QEMU ARM64 Linux /init test using existing VLab QEMU.

No emulator provisioning, compilation, root, network, USB, host block device,
phone flash, firmware mutation, or Prime Core readiness is provided here.
This is a tiny proof adapter, not a second virtual hardware laboratory.
"""
from __future__ import annotations

import argparse
import hashlib
import zlib
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import time
from typing import Any

from build import inspect_initramfs
from verify_serial import evaluate

SCHEMA = 'prime.mobile.mob1.qemu-arm64-guest-run.candidate.v0'
HEX_SHA = re.compile(r'^(?:sha256:)?([0-9a-f]{64})$')


def digest_bytes(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def pinned_file(path: Path, expected: str, description: str, max_size: int) -> bytes:
    match = HEX_SHA.fullmatch(expected)
    if not match:
        raise ValueError(f'{description}:INVALID_SHA256_PIN')
    if not path.is_file():
        raise ValueError(f'{description}:FILE_UNAVAILABLE')
    size = path.stat().st_size
    if not 1 <= size <= max_size:
        raise ValueError(f'{description}:FILE_SIZE_UNACCEPTABLE')
    data = path.read_bytes()
    if digest_bytes(data) != match.group(1):
        raise ValueError(f'{description}:SHA256_MISMATCH')
    return data


def checked_invocation(qemu: Path, kernel: Path, initrd: Path,
                       qemu_sha256: str, kernel_sha256: str,
                       initrd_sha256: str, timeout_seconds: int = 30) -> dict[str, Any]:
    """Pin and structurally inspect every executable/guest input before execution."""
    if not (3 <= timeout_seconds <= 90):
        raise ValueError('BOUNDED_TIMEOUT_REQUIRED')
    # Explicit emulator executable, not a PATH-resolved shell/script.
    if qemu.name != 'qemu-system-aarch64' or not os.access(qemu, os.X_OK):
        raise ValueError('QUALIFIED_QEMU_AARCH64_EXECUTABLE_REQUIRED')
    qemu_data = pinned_file(qemu, qemu_sha256, 'QEMU', 100_000_000)
    if not qemu_data.startswith(b'\x7fELF'):
        raise ValueError('QEMU_BINARY_IS_NOT_ELF')
    kernel_data = pinned_file(kernel, kernel_sha256, 'KERNEL_IMAGE', 200_000_000)
    # ARM64 Linux flat uncompressed Image header magic at byte 56.
    if len(kernel_data) < 64 or kernel_data[56:60] != b'ARM\x64':
        raise ValueError('NOT_UNCOMPRESSED_ARM64_LINUX_IMAGE')
    # This diagnostic initramfs is intentionally tiny: reject compressed data
    # or expansion that would exhaust host memory, including gzip bombs.
    initrd_data = pinned_file(initrd, initrd_sha256, 'INITRAMFS', 1_048_576)
    try:
        inflater = zlib.decompressobj(16 + zlib.MAX_WBITS)
        archive = inflater.decompress(initrd_data, 2_097_152 + 1)
        if len(archive) > 2_097_152 or inflater.unconsumed_tail:
            raise ValueError('INITRAMFS_TOO_LARGE')
        if not inflater.eof or inflater.unused_data:
            raise ValueError('INITRAMFS_NOT_SINGLE_VALID_GZIP')
    except zlib.error as error:
        raise ValueError('INITRAMFS_NOT_VALID_GZIP') from error
    structure = inspect_initramfs(archive)  # missing console => hard block
    cmd = [str(qemu.resolve()), '-machine', 'virt', '-cpu', 'cortex-a57',
           '-m', '512M', '-display', 'none', '-serial', 'stdio',
           '-monitor', 'none', '-no-reboot', '-net', 'none',
           '-nodefaults', '-kernel', str(kernel.resolve()),
           '-initrd', str(initrd.resolve()), '-append',
           'console=ttyAMA0 rdinit=/init loglevel=4']
    return {
        'schema': SCHEMA,
        'qualification_status':'CANDIDATE',
        'mode':'GENERIC_VIRT_NO_NETWORK_NO_PHYSICAL_DEVICE',
        'command':cmd,
        'pins':{'qemu':digest_bytes(qemu_data),'kernel':digest_bytes(kernel_data),
                'initramfs':digest_bytes(initrd_data)},
        'archive':structure,
        'not_a_phone_boot':True,
        'not_prime_core':True,
    }


def run_guest(admission: dict[str, Any], output_dir: Path,
              timeout_seconds: int = 30) -> dict[str, Any]:
    """Execute only the pre-admitted argv and return observed process/serial proof.

    Must be called after externally validating that the pinned QEMU and kernel
    are trusted. QEMU is NOT treated as a malicious-kernel security sandbox.
    """
    output_dir.mkdir(parents=True, exist_ok=True)
    uart_path = output_dir / 'guest-uart.log'
    stderr_path = output_dir / 'qemu-stderr.log'
    started = time.monotonic()
    with uart_path.open('wb') as serial, stderr_path.open('wb') as stderr:
        process = subprocess.Popen(admission['command'], stdin=subprocess.DEVNULL,
                                   stdout=serial, stderr=stderr, start_new_session=True)
        timed_out=False
        try:
            process.wait(timeout=timeout_seconds)
        except subprocess.TimeoutExpired:
            timed_out=True
            # Kill the newly created QEMU process group; never the host group.
            os.killpg(process.pid, signal.SIGKILL)
            process.wait(timeout=5)
    elapsed=round(time.monotonic()-started,3)
    uart=uart_path.read_bytes()
    stderr=stderr_path.read_bytes()
    if len(uart)>1_048_576 or len(stderr)>1_048_576:
        raise ValueError('GUEST_LOG_OVERFLOW')
    verdict=evaluate(uart.decode('utf-8','replace'))
    # Normal /init loops forever as an observational fixture. A bounded host
    # timeout is not evidence of a guest kernel panic or successful shutdown.
    observed=verdict['candidate_init_observed']
    result={
        'schema':SCHEMA, 'qualification_status':'CANDIDATE',
        'launched_process':True, 'host_timed_out':timed_out,
        'exit_code':process.returncode,'elapsed_seconds':elapsed,
        'guest_uart_sha256':digest_bytes(uart),
        'qemu_stderr_sha256':digest_bytes(stderr),
        'inputs':admission['pins'],
        'guest_verdict':verdict,
        'status': 'GUEST_EARLY_INIT_OBSERVED_ONLY' if observed else 'GUEST_BOOT_UNQUALIFIED',
        'prime_core_ready':False,'prime_shell_ready':False,
        'physical_phone_qualified':False,'flash_authorization':False,
        'limitations':['serial markers are narrow early-init evidence, not Prime Core',
                       'generic QEMU virt does not emulate Samsung/MediaTek hardware',
                       'host execution is only safe for externally reviewed pinned inputs'],
    }
    (output_dir/'guest-receipt.json').write_text(json.dumps(result,indent=2,sort_keys=True)+'\n')
    return result


def main() -> None:
    ap=argparse.ArgumentParser(description=__doc__)
    ap.add_argument('--qemu',required=True,type=Path)
    ap.add_argument('--qemu-sha256',required=True)
    ap.add_argument('--kernel',required=True,type=Path)
    ap.add_argument('--kernel-sha256',required=True)
    ap.add_argument('--initramfs',required=True,type=Path)
    ap.add_argument('--initramfs-sha256',required=True)
    ap.add_argument('--output',required=True,type=Path)
    ap.add_argument('--timeout',default=30,type=int)
    ap.add_argument('--run',action='store_true',help='Without --run, admission only; no QEMU execution')
    opts=ap.parse_args()
    admission=checked_invocation(opts.qemu,opts.kernel,opts.initramfs,
                                 opts.qemu_sha256,opts.kernel_sha256,
                                 opts.initramfs_sha256,opts.timeout)
    if opts.run:
        print(json.dumps(run_guest(admission,opts.output,opts.timeout),indent=2,sort_keys=True))
    else:
        print(json.dumps({'status':'ADMISSION_ONLY_NOT_EXECUTED',**admission},indent=2,sort_keys=True))


if __name__=='__main__':main()
