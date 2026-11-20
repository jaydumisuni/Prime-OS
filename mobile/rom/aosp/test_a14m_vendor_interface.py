"""Historical A14m VINTF source parser tests; never full framework-VINTF proof."""
from __future__ import annotations
import unittest
from unittest.mock import patch
import a14m_vendor_interface as v

HALS = list(v.ESSENTIAL) + [f"vendor.sample.test{i}" for i in range(12)]
MANIFEST = (
    '<manifest version="4.0" type="device" target-level="5">'
    + ''.join(f'<hal format="hidl"><name>{x}</name><version>1.0</version></hal>' for x in HALS)
    + '</manifest>'
).encode()
MATRIX = (
    '<compatibility-matrix version="4.0" type="device">'
    '<vendor-ndk><version>31</version></vendor-ndk>'
    '<system-sdk><version>31</version></system-sdk>'
    '</compatibility-matrix>'
).encode()


def assess(manifest=MANIFEST, matrix=MATRIX):
    with patch.object(v, "VENDOR_MANIFEST_BLOB", v.git_blob(manifest)), \
         patch.object(v, "VENDOR_MATRIX_BLOB", v.git_blob(matrix)):
        return v.assess(manifest, matrix)


class DonorInterfaceTests(unittest.TestCase):
    def test_historical_reference_is_never_a_system_image_or_phone_qualification(self):
        receipt = assess()
        self.assertEqual(receipt["hal_entry_count"], 20)
        self.assertEqual(receipt["vendor_fcm_target_level"], 5)
        self.assertEqual(receipt["vendor_ndk_version"], 31)
        self.assertEqual(receipt["system_sdk_requirement"], 31)
        self.assertEqual(len(receipt["essential_historical_hal_declarations"]), 8)
        for key in ("vendor_partition_contents_complete",
                    "vintf_framework_compatibility_passed",
                    "actual_samsung_vendor_matches_source",
                    "android15_gsi_approved_for_phone",
                    "phone_boot_proven", "twrp_install_approved"):
            self.assertFalse(receipt[key])

    def test_mismatch_against_pinned_actual_hash_is_rejected(self):
        with self.assertRaisesRegex(ValueError, "UNPINNED_AWG1_VENDOR_MANIFEST"):
            v.assess(MANIFEST, MATRIX)

    def test_missing_radio_or_graphics_hal_fails(self):
        for name in ("android.hardware.radio", "android.hardware.graphics.composer"):
            with self.subTest(name=name):
                bad = MANIFEST.replace(name.encode(), b"vendor.bad.unknown")
                with self.assertRaisesRegex(ValueError, "EXPECTED_DONOR_HAL"):
                    assess(bad)

    def test_wrong_fcm_target_fails(self):
        bad = MANIFEST.replace(b'target-level="5"', b'target-level="6"')
        with self.assertRaisesRegex(ValueError, "AWG1_FCM_TARGET_LEVEL"):
            assess(bad)

    def test_vendor_ndk_version_mismatch_fails(self):
        bad = MATRIX.replace(b'<version>31</version>', b'<version>30</version>', 1)
        with self.assertRaisesRegex(ValueError, "AWG1_VENDOR_NDK_SDK"):
            assess(MANIFEST, bad)

    def test_malformed_xml_is_rejected(self):
        bad = MANIFEST[:-12]
        with self.assertRaisesRegex(ValueError, "MALFORMED_VENDOR_XML"):
            assess(bad)

    def test_dtd_is_rejected(self):
        bad = b'<!DOCTYPE manifest [<!ENTITY x SYSTEM "file:///etc/passwd">]>' + MANIFEST
        with self.assertRaisesRegex(ValueError, "UNSAFE_OR_EXCESSIVE_VENDOR_XML"):
            assess(bad)


if __name__ == "__main__":
    unittest.main()
