#!/usr/bin/env python3
"""Optional read-only TWRP-only Samsung recovery intake; NEVER flashes.

Use only when the owner deliberately connects an authorized recovery-mode
handset to a trusted local host. No Android OS required. No serial, IMEI, MAC,
partition contents or personal data is returned. Hashing boot/recovery reads
full devices but does not write them. Fail closed on missing recovery evidence.
"""
from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import subprocess
from typing import Callable

SCHEMA = "prime.mobile.a14m.twrp-owner-evidence.untrusted.v0"
BLOCK = "/dev/block/platform/bootdevice/by-name/"
PARTS = ("boot", "recovery")
HASH_LINE = re.compile(r"^([0-9a-f]{64})\s+\S+$")
ABS_BLOCK = re.compile(r"^/dev/block/[a-zA-Z0-9_/.+-]{1,128}$")


def capture(adb: str = "adb", *,
            runner: Callable[..., subprocess.CompletedProcess[str]] = subprocess.run,
            now: str | None = None) -> dict:
    if not isinstance(adb, str) or not adb.strip() or "\x00" in adb:
        raise ValueError("ADB_EXECUTABLE_REQUIRED")

    def call(args: list[str], timeout: int = 10) -> tuple[bool, str]:
        try:
            x = runner([adb, *args], text=True, capture_output=True,
                       stdin=subprocess.DEVNULL, timeout=timeout, check=False)
        except (OSError, subprocess.TimeoutExpired):
            # Never propagate private ADB serial through exception argv or logs.
            return False, ""
        if x.returncode or len(x.stdout or "") > 8192:
            return False, ""
        return True, (x.stdout or "").strip()

    ok, listing = call(["devices"])
    if not ok:
        raise RuntimeError("ADB_DEVICE_ENUMERATION_UNAVAILABLE")
    devices = []
    for row in listing.splitlines()[1:]:
        if not row.strip():
            continue
        fields = row.split("\t")
        if len(fields) != 2:
            raise RuntimeError("ADB_DEVICE_ENUMERATION_UNTRUSTED")
        devices.append((fields[0], fields[1].strip()))
    if len(devices) != 1 or devices[0][1] not in ("recovery", "device"):
        raise RuntimeError("REQUIRE_ONE_AUTHORIZED_RECOVERY_ADB_DEVICE")
    serial = devices[0][0]
    if not serial or any(c in serial for c in "\x00\r\n"):
        raise RuntimeError("UNSAFE_ADB_TRANSPORT")
    def read(*cmd: str, timeout: int = 12) -> tuple[bool, str]:
        return call(["-s", serial, "shell", *cmd], timeout=timeout)

    # TWRP may expose /sbin/twrp; avoid reading Android getprop, running
    # recovery installers or using unsafely guessed device boot modes.
    exists, binary = read("ls", "-l", "/sbin/twrp")
    if not exists or "/sbin/twrp" not in binary:
        raise RuntimeError("TWRP_BINARY_NOT_OBSERVED_STOP")
    resolved: dict[str, str] = {}
    fingerprints: dict[str, str] = {}
    sizes: dict[str, int] = {}
    for name in PARTS:
        path = BLOCK + name
        resolved_ok, link = read("readlink", "-f", path)
        if not resolved_ok or not ABS_BLOCK.fullmatch(link):
            raise RuntimeError("PARTITION_PATH_UNRESOLVED_STOP")
        resolved[name] = link
        ok_size, raw_size = read("blockdev", "--getsize64", path)
        if not ok_size or not re.fullmatch(r"[0-9]{1,16}", raw_size):
            raise RuntimeError("BLOCK_DEVICE_SIZE_UNAVAILABLE_STOP")
        byte_size = int(raw_size)
        if byte_size < 1024 * 1024 or byte_size > 1024 * 1024 * 1024:
            raise RuntimeError("BLOCK_DEVICE_SIZE_OUT_OF_RANGE")
        sizes[name] = byte_size
        ok_hash, raw_hash = read("sha256sum", path, timeout=100)
        match = HASH_LINE.fullmatch(raw_hash) if ok_hash else None
        if not match or not raw_hash.split()[-1] == path:
            raise RuntimeError("BLOCK_DEVICE_READONLY_HASH_MISSING")
        fingerprints[name] = match.group(1)
    if resolved["boot"] == resolved["recovery"]:
        raise RuntimeError("BOOT_AND_RECOVERY_RESOLVE_TO_SAME_BLOCK_DEVICE")
    return {
        "schema": SCHEMA,
        "source_kind": "RECOVERY_ADB_READONLY_UNATTESTED",
        "observed_at": now or datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "reported_recovery_sha256": fingerprints["recovery"],
        "reported_boot_sha256": fingerprints["boot"],
        "partition_size_bytes": sizes,
        "partition_paths": resolved,
        "recovery_binary_seen": True,
        "serial_recorded": False,
        "device_contents_exfiltrated": False,
        "claims_flash_authorized": False,
        "recovery_restore_rehearsal_proven": False,
        "physical_model_attested": False,
        "bootloader_state_attested": False,
        "notes": [
            "Observations are read-only transport data, not cryptographic Samsung identity.",
            "A partition hash is not a backed-up, restorable image or a successful rollback.",
            "The tool never installs or flashes an image, modifies vbmeta, roots Android, reboots or copies raw firmware.",
        ],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--adb", default="adb",
                        help="Absolute known platform-tools ADB path if not in PATH")
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    receipt = capture(args.adb)
    args.output.write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n",
                           encoding="utf-8")


if __name__ == "__main__":
    main()
