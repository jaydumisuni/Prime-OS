"""TWRP-only source and first-device-boot stop gate; no phone, no writes."""
from __future__ import annotations

import copy
from pathlib import Path
import sys
import unittest
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from a14m_twrp_recovery_gate import (
    FSTAB_BLOB, PIT_BLOB, SCHEMA, evaluate, git_blob, historical_fstab,
    historical_pit,
)

FSTAB = (
    "# historical A145P recovery tree\n"
    "system /system ext4 ro logical\n"
    "/dev/block/platform/bootdevice/by-name/boot /boot emmc defaults\n"
    "/dev/block/platform/bootdevice/by-name/recovery /recovery emmc defaults\n"
).encode()
PARTS = ["boot", "recovery", "vendor_boot", "init_boot", "vbmeta", "dtbo", "super"]
PARTS += ["reference_only_" + str(i) for i in range(50)]
PIT = ("".join("Partition Name:             " + p + "\r\n" for p in PARTS)).encode()

LEDGER = {
    "schema": "prime.mobile.a14m.offline-source-ledger.candidate.v0",
    "scope": "HISTORICAL_AWG1_TWRP_REFERENCES_NOT_CURRENT_HANDSET",
    "twrp_recovery_tree": {
        "commit": "cdbc13945796b6863362ed041ce7177ce7cf3ca5",
        "recovery_fstab_blob": FSTAB_BLOB,
    },
    "historical_firmware_dump": {
        "commit": "699855860e672eb5f3582ecf8e2144de0ed8b2d1",
        "pit_analysis_blob": PIT_BLOB,
        "historical_boot_kernel_blob": "1d3968ea089f88deb19c0b8e1efe482b85ef7e65",
    },
}

IMAGE = {
    "schema": "prime.mobile.a14m.source-bound-real-core-staging.unqualified.v0",
    "image_sha256": "a" * 64,
    "historical_kernel_git_blob": LEDGER["historical_firmware_dump"]["historical_boot_kernel_blob"],
    "core_running_on_samsung_proven": False,
    "device_boot_chain_qualified": False,
    "physical_phone_boot_observed": False,
    "recovery_fallback_qualified": False,
    "stock_restoration_qualified": False,
    "device_flash_authorized": False,
}
OWNER = {
    "schema": "prime.mobile.a14m.twrp-owner-evidence.untrusted.v0",
    "reported_recovery_sha256": "b" * 64,
    "claims_flash_authorized": False,
}


def source_only(ledger=None, fstab=FSTAB, pit=PIT, image=None, owner=None):
    # Source fixtures are synthetic. Integration CI independently validates
    # downloaded immutable Git blobs without mocking.
    with patch("a14m_twrp_recovery_gate.git_blob", side_effect=[FSTAB_BLOB, PIT_BLOB]):
        return evaluate(ledger if ledger is not None else LEDGER, fstab, pit, image, owner)


class TWRPRecoverySourceTests(unittest.TestCase):
    def test_two_source_partitions_are_separate_but_not_phone_proven(self):
        r = source_only()
        self.assertEqual(r["schema"], SCHEMA)
        self.assertEqual(r["status"], "REFERENCE_SEPARATE_BOOT_RECOVERY_VERIFIED_NO_FLASH")
        self.assertEqual(r["historical_partition_count"], 57)
        self.assertNotEqual(r["historical_mount_mapping"]["/boot"], r["historical_mount_mapping"]["/recovery"])
        self.assertTrue(r["legacy_boot_recovery_are_distinct_in_source"])
        self.assertFalse(r["physical_recovery_fallback_proven"])
        self.assertFalse(r["safe_flash_authorized"])
        self.assertIn("NO_VERIFIED_TWRP_RECOVERY_RESTORE_REHEARSAL", r["blockers"])

    def test_core_image_receipt_accepted_only_as_unqualified_candidate(self):
        r = source_only(image=IMAGE)
        self.assertTrue(r["candidate"]["source_bound_core_image_candidate"])
        self.assertEqual(r["candidate"]["image_sha256"], "a" * 64)
        self.assertFalse(r["safe_flash_authorized"])
        self.assertFalse(r["device_boot_qualified"])

    def test_claimed_recovery_image_sha_not_rollback_proof(self):
        r = source_only(image=IMAGE, owner=OWNER)
        self.assertNotIn("OWNER_CURRENT_TWRP_HASH_NOT_PRESENT", r["blockers"])
        self.assertFalse(r["physical_recovery_fallback_proven"])
        self.assertFalse(r["twrp_restore_rehearsed"])
        self.assertFalse(r["safe_flash_authorized"])

    def test_false_positive_owner_flash_claim_rejected(self):
        owner = copy.deepcopy(OWNER)
        owner["claims_flash_authorized"] = True
        with self.assertRaisesRegex(ValueError, "UNTRUSTED_FLASH_AUTHORIZATION"):
            source_only(owner=owner)

    def test_invalid_owner_digest_and_schema_rejected(self):
        owner = copy.deepcopy(OWNER)
        owner["reported_recovery_sha256"] = "BAD"
        with self.assertRaisesRegex(ValueError, "RECOVERY_HASH_INVALID"):
            source_only(owner=owner)
        owner["reported_recovery_sha256"] = "a" * 64
        owner["schema"] = "SIMULATED"
        with self.assertRaisesRegex(ValueError, "UNQUALIFIED_RECOVERY_EVIDENCE_SCHEMA"):
            source_only(owner=owner)

    def test_image_claim_cannot_promote_flash(self):
        for key in (
            "core_running_on_samsung_proven", "device_boot_chain_qualified",
            "physical_phone_boot_observed", "recovery_fallback_qualified",
            "stock_restoration_qualified", "device_flash_authorized",
        ):
            with self.subTest(key=key):
                image = copy.deepcopy(IMAGE)
                image[key] = True
                with self.assertRaisesRegex(ValueError, "IMAGE_RECEIPT_FALSE_DEVICE_CLAIM"):
                    source_only(image=image)

    def test_image_source_kernel_mismatch_rejected(self):
        image = copy.deepcopy(IMAGE)
        image["historical_kernel_git_blob"] = "f" * 40
        with self.assertRaisesRegex(ValueError, "CORE_IMAGE_NOT_SOURCE_BOUND"):
            source_only(image=image)

    def test_missing_boot_or_recovery_mapping_fails(self):
        for key in ("boot", "recovery"):
            with self.subTest(key=key):
                altered = FSTAB.replace(b"/by-name/" + key.encode(), b"/by-name/unverified")
                with self.assertRaisesRegex(ValueError, "MISSING_OR_SHARED"):
                    historical_fstab(altered.decode())

    def test_recovery_mapping_duplicates_rejected(self):
        altered = FSTAB.decode() + "/dev/block/platform/bootdevice/by-name/recovery /recovery emmc defaults\n"
        with self.assertRaisesRegex(ValueError, "DUPLICATE"):
            historical_fstab(altered)

    def test_unknown_boot_fstype_rejected(self):
        with self.assertRaisesRegex(ValueError, "UNEXPECTED_BOOT_RECOVERY"):
            historical_fstab(FSTAB.decode().replace("/boot emmc", "/boot ext4"))

    def test_incomplete_or_duplicate_pit_fails(self):
        with self.assertRaisesRegex(ValueError, "PIT_COUNT"):
            historical_pit(PIT.decode().replace("Partition Name:             recovery", "Partition Name:             boot"))
        with self.assertRaisesRegex(ValueError, "PIT_COUNT"):
            historical_pit(PIT.decode().replace("Partition Name:             init_boot\r\n", ""))

    def test_historical_source_sha_pins_are_mandatory(self):
        bad = copy.deepcopy(LEDGER)
        bad["historical_firmware_dump"]["pit_analysis_blob"] = "x" * 40
        with self.assertRaisesRegex(ValueError, "UNPINNED_AWG1"):
            source_only(ledger=bad)

    def test_source_file_hash_mismatch_rejected_without_mock(self):
        with self.assertRaisesRegex(ValueError, "FSTAB_SOURCE_BLOB_MISMATCH"):
            evaluate(LEDGER, FSTAB, PIT, IMAGE)

    def test_record_contains_no_device_writes_or_installer_actions(self):
        r = source_only()
        for k in ("physical_device_touched", "safe_flash_authorized", "device_boot_qualified",
                  "exact_device_firmware_and_avb_verified", "will_overwrite_recovery"):
            self.assertFalse(r[k])
        self.assertTrue(r["owner_report_twrp_only_no_android"])

    def test_git_blob_hash_matches_known_git_rule(self):
        self.assertEqual(git_blob(b"test"), "30d74d258442c7c65512eafab474568dd706c430")


if __name__ == "__main__":
    unittest.main()
