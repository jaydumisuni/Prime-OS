#!/usr/bin/env python3
"""Read-only historical SM-A145P vendor VINTF donor classification.

The 2023 AWG1 Git source tree is NOT a mounted/reconstructed vendor partition,
a redistributable ROM, an actual-phone hardware probe or Android 15 acceptance.
"""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
import xml.etree.ElementTree as ET

SCHEMA = "prime.mobile.a14m.historical-vendor-vintf.candidate.v0"
AWG1_COMMIT = "699855860e672eb5f3582ecf8e2144de0ed8b2d1"
VENDOR_MANIFEST_BLOB = "82bef8b1e126e23d4c5049595ede5982a5fb712c"
VENDOR_MATRIX_BLOB = "41ef108da5aa9f209a4df85d0f32cba599687bee"
ESSENTIAL = (
    "android.hardware.audio", "android.hardware.camera.provider",
    "android.hardware.graphics.allocator", "android.hardware.graphics.composer",
    "android.hardware.radio", "android.hardware.radio.config",
    "android.hardware.sensors", "android.hardware.security.keymint",
)


def git_blob(payload: bytes) -> str:
    return hashlib.sha1(b"blob " + str(len(payload)).encode() + b"\0" + payload).hexdigest()


def root(payload: bytes, tag: str) -> ET.Element:
    if len(payload) > 128 * 1024 or not payload or b"<!DOCTYPE" in payload:
        raise ValueError("UNSAFE_OR_EXCESSIVE_VENDOR_XML")
    try:
        tree = ET.fromstring(payload)
    except ET.ParseError as error:
        raise ValueError("MALFORMED_VENDOR_XML") from error
    if tree.tag != tag or tree.attrib.get("type") != "device" or tree.attrib.get("version") != "4.0":
        raise ValueError("VINTF_ROOT_IDENTITY_MISMATCH")
    return tree


def assess(manifest: bytes, matrix: bytes) -> dict:
    if git_blob(manifest) != VENDOR_MANIFEST_BLOB:
        raise ValueError("UNPINNED_AWG1_VENDOR_MANIFEST")
    if git_blob(matrix) != VENDOR_MATRIX_BLOB:
        raise ValueError("UNPINNED_AWG1_VENDOR_MATRIX")
    donor = root(manifest, "manifest")
    requirements = root(matrix, "compatibility-matrix")
    target_level = donor.get("target-level")
    if target_level != "5":
        raise ValueError("AWG1_FCM_TARGET_LEVEL_UNEXPECTED")
    ndk = [x.text.strip() for x in requirements.findall("vendor-ndk/version") if x.text]
    sdk = [x.text.strip() for x in requirements.findall("system-sdk/version") if x.text]
    if ndk != ["31"] or sdk != ["31"]:
        raise ValueError("AWG1_VENDOR_NDK_SDK_REQUIREMENTS_UNEXPECTED")
    roles: dict[str, list[str]] = {}
    for hal in donor.findall("hal"):
        name = hal.findtext("name")
        if not name:
            raise ValueError("UNNAMED_VENDOR_HAL")
        formats = roles.setdefault(name, [])
        for version in hal.findall("version"):
            if version.text:
                formats.append(version.text.strip())
        for fq in hal.findall("fqname"):
            if fq.text:
                formats.append(fq.text.strip())
    missing = [name for name in ESSENTIAL if name not in roles]
    if missing:
        raise ValueError("EXPECTED_DONOR_HAL_NOT_DECLARED:" + ",".join(missing))
    count = len(donor.findall("hal"))
    if count < 20:
        raise ValueError("VENDOR_HAL_DECLARATIONS_UNEXPECTEDLY_FEW")
    return {
        "schema": SCHEMA,
        "source": "PINNED_PUBLIC_2023_AWG1_VENDOR_TREE_NOT_INSTALLED_PARTITION",
        "source_commit": AWG1_COMMIT,
        "vendor_manifest_git_blob": VENDOR_MANIFEST_BLOB,
        "vendor_matrix_git_blob": VENDOR_MATRIX_BLOB,
        "vendor_fcm_target_level": int(target_level),
        "vendor_ndk_version": int(ndk[0]),
        "system_sdk_requirement": int(sdk[0]),
        "hal_entry_count": count,
        "distinct_hal_names": len(roles),
        "essential_historical_hal_declarations": {
            k: sorted(set(roles[k]))[:8] for k in ESSENTIAL
        },
        "missing_essential_historical_hal_names": missing,
        "vendor_partition_contents_complete": False,
        "vintf_framework_compatibility_passed": False,
        "actual_samsung_vendor_matches_source": False,
        "android15_gsi_approved_for_phone": False,
        "phone_boot_proven": False,
        "twrp_install_approved": False,
        "limitations": [
            "A vendor HAL declared in XML is not proof its binder service or driver is functional.",
            "The AWG1 donor is Android 13-era; this is NOT a full VINTF check against an actual selected AOSP framework matrix.",
            "No exact owner phone vendor, odm, vendor_dlkm, active firmware or AVB measurement was obtained.",
            "Google developer GSIs cannot be silently redistributed as a Prime Mobile release.",
            "A vendor tree is not a complete extracted system/vendor/super image or a bootable ROM.",
        ],
    }


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--manifest", required=True, type=Path)
    p.add_argument("--matrix", required=True, type=Path)
    p.add_argument("--output", type=Path)
    args = p.parse_args()
    result = assess(args.manifest.read_bytes(), args.matrix.read_bytes())
    data = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.output:
        args.output.write_text(data)
    else:
        print(data, end="")


if __name__ == "__main__":
    main()
