"""Mock ADB for optional TWRP-only recovery intake; never live-device proof."""
from pathlib import Path
import subprocess
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
from a14m_twrp_readonly_capture import capture, BLOCK

SERIAL = "private-serial-abc"
SHA_BOOT = "a" * 64
SHA_RECOVERY = "b" * 64


def fake_runner(*, device_state="device", has_twrp=True,
                same_block=False, hash_failure=False,
                wrong_size=False, extra_device=False):
    commands = []
    def runner(args, **kwargs):
        commands.append(args)
        if args == ["adb", "devices"]:
            rows = f"List of devices attached\n{SERIAL}\t{device_state}\n"
            if extra_device:
                rows += "other-serial\tdevice\n"
            return subprocess.CompletedProcess(args, 0, rows, "")
        if args[:4] != ["adb", "-s", SERIAL, "shell"]:
            return subprocess.CompletedProcess(args, 1, "", "")
        command = args[4:]
        if command == ["ls", "-l", "/sbin/twrp"]:
            return subprocess.CompletedProcess(args, 0 if has_twrp else 1,
                                               "-rwxr-xr-x /sbin/twrp" if has_twrp else "", "")
        if command[:2] == ["readlink", "-f"]:
            return subprocess.CompletedProcess(
                args, 0,
                "/dev/block/mmcblk0p41" if command[-1].endswith("boot") and not command[-1].endswith("recovery") else
                ("/dev/block/mmcblk0p41" if same_block else "/dev/block/mmcblk0p42"),
                "")
        if command[:2] == ["blockdev", "--getsize64"]:
            return subprocess.CompletedProcess(args, 0,
                                               "broken" if wrong_size else "80740352", "")
        if command[0] == "sha256sum":
            if hash_failure:
                return subprocess.CompletedProcess(args, 1, "", "Permission denied")
            digest = SHA_BOOT if command[-1] == BLOCK + "boot" else SHA_RECOVERY
            return subprocess.CompletedProcess(args, 0, digest + "  " + command[-1], "")
        raise AssertionError("unexpected command " + repr(command))
    return runner, commands


class TWROnlyReadOnlyCaptureTests(unittest.TestCase):
    def test_one_authorized_twrp_device_yields_unqualified_digests(self):
        runner, commands = fake_runner()
        d = capture(runner=runner, now="2026-11-16T09:00:00+00:00")
        self.assertEqual(d["reported_boot_sha256"], SHA_BOOT)
        self.assertEqual(d["reported_recovery_sha256"], SHA_RECOVERY)
        self.assertEqual(d["partition_size_bytes"]["boot"], 80740352)
        self.assertNotEqual(d["partition_paths"]["boot"], d["partition_paths"]["recovery"])
        self.assertFalse(d["claims_flash_authorized"])
        self.assertFalse(d["recovery_restore_rehearsal_proven"])
        self.assertFalse(d["physical_model_attested"])
        self.assertFalse(d["serial_recorded"])
        self.assertNotIn(SERIAL, repr(d))
        self.assertEqual(len(commands), 8)
        self.assertEqual(commands[0], ["adb", "devices"])
        self.assertTrue(all(command[1:3] == ["-s", SERIAL] for command in commands[1:]))
        self.assertTrue(all(command[3] == "shell" for command in commands[1:]))
        self.assertEqual({command[4] for command in commands[1:]},
                         {"ls", "readlink", "blockdev", "sha256sum"})

    def test_recovery_adb_transport_is_supported_without_android(self):
        runner, _ = fake_runner(device_state="recovery")
        d = capture(runner=runner)
        self.assertEqual(d["source_kind"], "RECOVERY_ADB_READONLY_UNATTESTED")

    def test_another_android_or_no_twrp_binary_stops(self):
        runner, commands = fake_runner(has_twrp=False)
        with self.assertRaisesRegex(RuntimeError, "TWRP_BINARY_NOT_OBSERVED_STOP"):
            capture(runner=runner)
        self.assertEqual(len(commands), 2)

    def test_multiple_devices_stop_before_any_device_shell_command(self):
        runner, commands = fake_runner(extra_device=True)
        with self.assertRaisesRegex(RuntimeError, "REQUIRE_ONE_AUTHORIZED"):
            capture(runner=runner)
        self.assertEqual(commands, [["adb", "devices"]])

    def test_offline_unauthorized_devices_are_rejected(self):
        for state in ("offline", "unauthorized"):
            with self.subTest(state=state):
                runner, _ = fake_runner(device_state=state)
                with self.assertRaisesRegex(RuntimeError, "REQUIRE_ONE_AUTHORIZED"):
                    capture(runner=runner)

    def test_shared_boot_recovery_device_is_rejected(self):
        runner, _ = fake_runner(same_block=True)
        with self.assertRaisesRegex(RuntimeError, "SAME_BLOCK_DEVICE"):
            capture(runner=runner)

    def test_missing_partition_hash_fails_closed(self):
        runner, _ = fake_runner(hash_failure=True)
        with self.assertRaisesRegex(RuntimeError, "HASH_MISSING"):
            capture(runner=runner)

    def test_unknown_partition_size_fails_closed(self):
        runner, _ = fake_runner(wrong_size=True)
        with self.assertRaisesRegex(RuntimeError, "SIZE_UNAVAILABLE"):
            capture(runner=runner)

    def test_subprocess_timeout_does_not_leak_serial(self):
        def runner(args, **kwargs):
            if args == ["adb", "devices"]:
                return subprocess.CompletedProcess(args, 0, "List of devices attached\n"+SERIAL+"\tdevice\n", "")
            raise subprocess.TimeoutExpired(args, 8)
        with self.assertRaisesRegex(RuntimeError, "TWRP_BINARY_NOT_OBSERVED_STOP") as context:
            capture(runner=runner)
        self.assertNotIn(SERIAL, str(context.exception))

    def test_empty_adb_path_rejected(self):
        with self.assertRaisesRegex(ValueError, "ADB_EXECUTABLE_REQUIRED"):
            capture(adb="")

    def test_readonly_capture_not_self_attestation(self):
        runner, _ = fake_runner()
        x = capture(runner=runner)
        self.assertFalse(x["bootloader_state_attested"])
        self.assertFalse(x["claims_flash_authorized"])
        self.assertFalse(x["device_contents_exfiltrated"])


if __name__ == "__main__":
    unittest.main()
