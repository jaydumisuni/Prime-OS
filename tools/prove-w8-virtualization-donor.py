#!/usr/bin/env python3
"""Read-only, fail-closed W8 Gate 2 QEMU/KVM donor inspection.

This proves local donor availability only; it does not start a guest or grant W8 release authority.
"""
import fcntl
import hashlib
import json
import os
from pathlib import Path
import shutil
import stat
import subprocess
import sys

KVM_GET_API_VERSION = 0xAE00
EXPECTED_KVM_API_VERSION = 12


def inspect():
    qemu = shutil.which("qemu-system-x86_64")
    if not qemu:
        raise RuntimeError("QEMU x86_64 executable missing")
    binary = Path(qemu).resolve(strict=True)
    metadata = binary.stat()
    if not stat.S_ISREG(metadata.st_mode) or metadata.st_mode & 0o022:
        raise RuntimeError("QEMU binary is not a regular, non-group-writable file")
    digest = hashlib.sha256()
    with binary.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    version = subprocess.run([str(binary), "--version"], capture_output=True, text=True, timeout=10, check=True)
    first_line = version.stdout.splitlines()[0] if version.stdout.splitlines() else ""
    if "QEMU emulator version" not in first_line:
        raise RuntimeError("Unexpected QEMU version output")
    fd = os.open("/dev/kvm", os.O_RDWR | os.O_CLOEXEC | os.O_NOFOLLOW)
    try:
        api_version = fcntl.ioctl(fd, KVM_GET_API_VERSION, 0)
    finally:
        os.close(fd)
    if api_version != EXPECTED_KVM_API_VERSION:
        raise RuntimeError(f"Unexpected KVM API version: {api_version}")
    return {"schema": "prime.w8.virtualization-donor-readonly.v1",
            "qemu_path": str(binary), "qemu_sha256": digest.hexdigest(),
            "qemu_version": first_line, "kvm_api_version": api_version,
            "kvm_open_read_write": True, "guest_started": False,
            "release_authority": False}


if __name__ == "__main__":
    try:
        print(json.dumps(inspect(), sort_keys=True))
    except (OSError, RuntimeError, subprocess.SubprocessError, IndexError) as exc:
        print(json.dumps({"status": "FAIL", "error": str(exc), "guest_started": False}), file=sys.stderr)
        sys.exit(1)
