#!/usr/bin/env python3
"""Source-pinned SM-A145P TWRP/boot separation admission; never flash approval.

No Android installation, ADB session, root, device IO or recovery mutation is
needed to assess published references. Physical rollback remains a separate gate.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import re

SCHEMA = "prime.mobile.a14m.twrp-first-boot-source-admission.candidate.v0"
IMAGE_SCHEMA = "prime.mobile.a14m.source-bound-real-core-staging.unqualified.v0"
HISTORICAL_SCOPE = "HISTORICAL_AWG1_TWRP_REFERENCES_NOT_CURRENT_HANDSET"
REF_TWRP = "cdbc13945796b6863362ed041ce7177ce7cf3ca5"
REF_AWG1 = "699855860e672eb5f3582ecf8e2144de0ed8b2d1"
FSTAB_BLOB = "7f352215b7de3cfce674cdabfa2f933e10700256"
PIT_BLOB = "05e2a2010e9d1cba27b0348ed75823be95715f91"
PARTITIONS = ("boot", "recovery", "vendor_boot", "init_boot", "vbmeta", "dtbo", "super")
SHA256 = re.compile(r"^[0-9a-f]{64}$")
HEX40 = re.compile(r"^[0-9a-f]{40}$")
BLOCK_PATH = re.compile(r"^/dev/block/platform/bootdevice/by-name/[a-z_]+$")
SHA1_HISTORICAL_BLOB_NOTICE = (
    "Git blob checks pin public 2023-era source files, not installed Samsung firmware."
)

# First-party maintainer statement about a withdrawn experimental TWRP image.
# It is NOT evidence the owner's installed TWRP is this specific build.
WITHDRAWN_TWRP_RELEASE = (
    "https://github.com/forforksake/recovery_device_samsung_a14m/"
    "releases/tag/TWRP_3.7.0_12_2_CFKod"
)
WITHDRAWN_TWRP_IMAGE = "twrp_08092023-2358.img"


def git_blob(raw: bytes) -> str:
    return hashlib.sha1(b"blob " + str(len(raw)).encode("ascii") + b"\x00" + raw).hexdigest()


def historical_fstab(fstab: str) -> dict[str, str]:
    entries: dict[str, str] = {}
    for raw in fstab.splitlines():
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        cols = line.split()
        if len(cols) < 4:
            raise ValueError("MALFORMED_HISTORICAL_FSTAB")
        if cols[1] in ("/boot", "/recovery"):
            mountpoint = cols[1]
            if mountpoint in entries:
                raise ValueError("DUPLICATE_BOOT_OR_RECOVERY_FSTAB")
            if cols[2] != "emmc" or cols[3] != "defaults":
                raise ValueError("UNEXPECTED_BOOT_RECOVERY_FSTAB_TYPE")
            entries[mountpoint] = cols[0]
    expected = {
        "/boot": "/dev/block/platform/bootdevice/by-name/boot",
        "/recovery": "/dev/block/platform/bootdevice/by-name/recovery",
    }
    if entries != expected:
        raise ValueError("MISSING_OR_SHARED_BOOT_RECOVERY_MAPPING")
    if not all(BLOCK_PATH.fullmatch(v) for v in entries.values()):
        raise ValueError("UNEXPECTED_BOOT_RECOVERY_BLOCK_PATH")
    return entries


def historical_pit(pit: str) -> dict[str, int]:
    names = re.findall(r"^Partition Name:\s*(\S+)\s*$", pit, re.M)
    if len(names) != 57 or len(set(names)) != len(names):
        raise ValueError("HISTORICAL_PIT_COUNT_OR_DUPLICATES")
    if any(n not in names for n in PARTITIONS):
        raise ValueError("HISTORICAL_PIT_REQUIRED_PARTITION_MISSING")
    # Published PIT documents a separate boot and recovery entry, not actual
    # current phone partition targeting, slots or verified boot chain.
    return {"partition_count": len(names), "required_partition_names_present": len(PARTITIONS)}


def evaluate(
    source_ledger: dict,
    recovery_fstab_bytes: bytes,
    pit_analysis_bytes: bytes,
    image_receipt: dict | None = None,
    owner_recovery_evidence: dict | None = None,
) -> dict:
    if not isinstance(source_ledger, dict) or source_ledger.get("schema") != "prime.mobile.a14m.offline-source-ledger.candidate.v0":
        raise ValueError("SOURCE_LEDGER_SCHEMA_MISMATCH")
    if source_ledger.get("scope") != HISTORICAL_SCOPE:
        raise ValueError("HISTORICAL_SCOPE_MISMATCH")
    recovery = source_ledger.get("twrp_recovery_tree", {})
    firmware = source_ledger.get("historical_firmware_dump", {})
    if recovery.get("commit") != REF_TWRP or recovery.get("recovery_fstab_blob") != FSTAB_BLOB:
        raise ValueError("UNPINNED_TWRP_SOURCE_LEDGER")
    if firmware.get("commit") != REF_AWG1 or firmware.get("pit_analysis_blob") != PIT_BLOB:
        raise ValueError("UNPINNED_AWG1_SOURCE_LEDGER")
    if git_blob(recovery_fstab_bytes) != FSTAB_BLOB:
        raise ValueError("FSTAB_SOURCE_BLOB_MISMATCH")
    if git_blob(pit_analysis_bytes) != PIT_BLOB:
        raise ValueError("PIT_SOURCE_BLOB_MISMATCH")
    mapping = historical_fstab(recovery_fstab_bytes.decode("utf-8"))
    layout = historical_pit(pit_analysis_bytes.decode("utf-8"))
    blockers = [
        "NO_LIVE_SAMSUNG_BOOT_CHAIN_ATTESTATION",
        "NO_VERIFIED_TWRP_RECOVERY_RESTORE_REHEARSAL",
        "INSTALLED_TWRP_LINEAGE_NOT_VERIFIED_AGAINST_WITHDRAWN_FSTAB_BUILD",
        "NO_CURRENT_COMPATIBLE_STOCK_RESTORE_IMAGE_HASH",
        "ACTIVE_VENDOR_BOOT_INIT_BOOT_AND_AVB_ROUTE_UNKNOWN",
    ]
    image = {
        "present": False,
        "source_bound_core_image_candidate": False,
        "image_sha256": None,
    }
    if image_receipt is None:
        blockers.append("CURRENT_CORE_IMAGE_BUILD_RECEIPT_MISSING")
    else:
        if not isinstance(image_receipt, dict) or image_receipt.get("schema") != IMAGE_SCHEMA:
            raise ValueError("CORE_IMAGE_RECEIPT_SCHEMA_MISMATCH")
        expected_false = (
            "core_running_on_samsung_proven",
            "device_boot_chain_qualified",
            "physical_phone_boot_observed",
            "recovery_fallback_qualified",
            "stock_restoration_qualified",
            "device_flash_authorized",
        )
        if any(image_receipt.get(key) is not False for key in expected_false):
            raise ValueError("IMAGE_RECEIPT_FALSE_DEVICE_CLAIM")
        image_sha = image_receipt.get("image_sha256")
        if not isinstance(image_sha, str) or not SHA256.fullmatch(image_sha):
            raise ValueError("IMAGE_SHA256_UNUSABLE")
        if image_receipt.get("historical_kernel_git_blob") != firmware.get("historical_boot_kernel_blob"):
            raise ValueError("CORE_IMAGE_NOT_SOURCE_BOUND_TO_BOOT_KERNEL")
        image = {
            "present": True,
            "source_bound_core_image_candidate": True,
            "image_sha256": image_sha,
        }
    if owner_recovery_evidence is not None:
        # A caller-supplied JSON claim is not physical proof, even if it uses
        # strings that resemble a real recovery SHA256 or a test outcome.
        if not isinstance(owner_recovery_evidence, dict) or owner_recovery_evidence.get("schema") != "prime.mobile.a14m.twrp-owner-evidence.untrusted.v0":
            raise ValueError("UNQUALIFIED_RECOVERY_EVIDENCE_SCHEMA")
        if owner_recovery_evidence.get("claims_flash_authorized") is True:
            raise ValueError("UNTRUSTED_FLASH_AUTHORIZATION_CLAIM")
        digest = owner_recovery_evidence.get("reported_recovery_sha256")
        if digest is not None and (not isinstance(digest, str) or not SHA256.fullmatch(digest)):
            raise ValueError("RECOVERY_HASH_INVALID")
    else:
        blockers.append("OWNER_CURRENT_TWRP_HASH_NOT_PRESENT")
    return {
        "schema": SCHEMA,
        "status": "REFERENCE_SEPARATE_BOOT_RECOVERY_VERIFIED_NO_FLASH",
        "source_kind": "PINNED_HISTORICAL_PUBLIC_SOURCE_NOT_INSTALLED_HANDSET",
        "historical_twrp_git_commit": REF_TWRP,
        "historical_awg1_git_commit": REF_AWG1,
        "historical_partition_count": layout["partition_count"],
        "historical_mount_mapping": mapping,
        "legacy_boot_recovery_are_distinct_in_source": True,
        "historical_vendor_boot_init_boot_present": True,
        "known_historical_recovery_warning": {
            "source": WITHDRAWN_TWRP_RELEASE,
            "withdrawn_image": WITHDRAWN_TWRP_IMAGE,
            "reported_issue": "MAINTAINER_REPORTED_FSTAB_BACKUP_RESTORE_FAILURES",
            "applies_to_installed_twrp": "UNKNOWN",
        },
        "candidate": image,
        "blockers": sorted(set(blockers)),
        "read_only_source_audit_passed": True,
        "owner_report_twrp_only_no_android": True,
        "physical_recovery_fallback_proven": False,
        "exact_device_firmware_and_avb_verified": False,
        "twrp_restore_rehearsed": False,
        "device_boot_qualified": False,
        "safe_flash_authorized": False,
        "will_overwrite_recovery": False,
        "physical_device_touched": False,
        "notes": [
            SHA1_HISTORICAL_BLOB_NOTICE,
            "Separate historical boot/recovery entries do not prove that recovery will remain bootable after a boot image change.",
            "Samsung Download Mode and TWRP support do not imply fastboot boot, bootloader bypass, AVB relaxation or safe downgrade.",
            "TWRP-owned boot backups and a compatible stock restore must be verified without replacing existing TWRP.",
            "The maintainer withdrew a 2023 experimental recovery image after fstab-related restore failures; this does not identify the owner's current installed recovery.",
            "No Android getprop, device scan, flash, erase, reboot, sideload or installer output is used by this audit.",
        ],
    }


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("ledger", "fstab", "pit"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--image-receipt", type=Path)
    parser.add_argument("--owner-evidence", type=Path)
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    result = evaluate(
        json.loads(args.ledger.read_text(encoding="utf-8")),
        args.fstab.read_bytes(),
        args.pit.read_bytes(),
        json.loads(args.image_receipt.read_text(encoding="utf-8")) if args.image_receipt else None,
        json.loads(args.owner_evidence.read_text(encoding="utf-8")) if args.owner_evidence else None,
    )
    payload = json.dumps(result, sort_keys=True, indent=2) + "\n"
    if args.output:
        args.output.write_text(payload, encoding="utf-8")
    else:
        print(payload, end="")


if __name__ == "__main__":
    main()
