"""Early ARM64 board witness receipt tests: no live phone required."""
from __future__ import annotations

import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from a14m_boot_observation import evaluate, SCHEMA

GOOD = "\n".join((
    "Booting Linux ...",
    "PRIME_A14M_SOURCE_BRIDGE|not_a_phone_boot_proof=true",
    "PRIME_MOB1_CORE_V0|stage=ENTERED",
    "PRIME_A14M_BOOT_OBSERVATION_V0|source=KERNEL_READ_ONLY",
    "PRIME_A14M_BOOT_OBSERVATION_V0|model=linux,dummy-virt",
    "PRIME_A14M_BOOT_OBSERVATION_V0|compatible_first=linux,dummy-virt",
    "PRIME_A14M_BOOT_OBSERVATION_V0|kernel_release=6.8.0-generic",
    "PRIME_A14M_BOOT_OBSERVATION_V0|physical_device_qualified=false",
    "PRIME_MOB1_CORE_V0|versions_status=200",
    "PRIME_MOB1_CORE_V0|stage=TRANSPORT_ONLY_UNQUALIFIED",
    "",
))


class FirstLightBoardWitnessTests(unittest.TestCase):
    def test_generic_guest_markers_do_not_promote_samsung(self):
        packet = evaluate(GOOD)
        self.assertEqual(packet["schema"], SCHEMA)
        self.assertEqual(packet["fields"]["model"], "linux,dummy-virt")
        self.assertEqual(len(packet["available_fields"]), 3)
        self.assertTrue(packet["real_primed_transport_marker_observed"])
        for key in ("image_booted_on_samsung_proven", "hardware_identity_qualified",
                    "boot_chain_qualified", "recovery_fallback_qualified",
                    "device_flash_authorized", "prime_mobile_shell_ready"):
            self.assertIs(packet[key], False)

    def test_missing_optional_kernel_field_is_explicit_unavailable(self):
        packet = evaluate(GOOD.replace("model=linux,dummy-virt",
                                       "model=UNAVAILABLE"))
        self.assertNotIn("model", packet["available_fields"])

    def test_simulated_samsung_named_board_does_not_become_physical(self):
        packet = evaluate(GOOD.replace("linux,dummy-virt",
                                       "Samsung SM-A145P reference"))
        self.assertEqual(packet["fields"]["model"], "Samsung SM-A145P reference")
        self.assertFalse(packet["hardware_identity_qualified"])
        self.assertFalse(packet["device_flash_authorized"])

    def test_repeated_witness_is_rejected(self):
        duplicate = "PRIME_A14M_BOOT_OBSERVATION_V0|model=linux,dummy-virt\n"
        with self.assertRaisesRegex(ValueError, "DUPLICATE"):
            evaluate(GOOD + duplicate)

    def test_missing_witness_is_rejected(self):
        reduced = GOOD.replace(
            "PRIME_A14M_BOOT_OBSERVATION_V0|kernel_release=6.8.0-generic\n", "")
        with self.assertRaisesRegex(ValueError, "MISSING"):
            evaluate(reduced)

    def test_order_is_required(self):
        out = GOOD.replace("source=KERNEL_READ_ONLY",
                           "source=kernel_read_only")
        with self.assertRaisesRegex(ValueError, "SOURCE_NOT_KERNEL"):
            evaluate(out)

    def test_physical_proof_claim_is_always_rejected(self):
        altered = GOOD.replace("physical_device_qualified=false",
                               "physical_device_qualified=true")
        with self.assertRaisesRegex(ValueError, "FALSE_HARDWARE"):
            evaluate(altered)

    def test_serial_like_control_data_is_not_accepted(self):
        altered = GOOD.replace("linux,dummy-virt",
                               "model-with-tab\tand-secret", 1)
        with self.assertRaisesRegex(ValueError, "MALFORMED"):
            evaluate(altered)

    def test_oversized_field_is_rejected(self):
        altered = GOOD.replace("model=linux,dummy-virt",
                               "model=" + "A" * 97)
        with self.assertRaisesRegex(ValueError, "MALFORMED"):
            evaluate(altered)

    def test_missing_core_marker_is_rejected(self):
        out = GOOD.replace(
            "PRIME_MOB1_CORE_V0|stage=TRANSPORT_ONLY_UNQUALIFIED\n", "")
        with self.assertRaisesRegex(ValueError, "CORE_OR_BRIDGE"):
            evaluate(out)

    def test_missing_bridge_marker_is_rejected(self):
        out = GOOD.replace(
            "PRIME_A14M_SOURCE_BRIDGE|not_a_phone_boot_proof=true\n", "")
        with self.assertRaisesRegex(ValueError, "CORE_OR_BRIDGE"):
            evaluate(out)

    def test_receipt_is_stable_for_identical_log(self):
        a = evaluate(GOOD)
        b = evaluate(GOOD)
        self.assertEqual(json.dumps(a, sort_keys=True), json.dumps(b, sort_keys=True))

    def test_oversized_input_is_refused(self):
        with self.assertRaisesRegex(ValueError, "UART_TOO_LARGE"):
            evaluate(GOOD + "X" * (4 * 1024 * 1024))


if __name__ == "__main__":
    unittest.main()
