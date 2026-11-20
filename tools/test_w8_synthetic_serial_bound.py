"""Source-only bounded serial regression; never starts QEMU."""
import importlib.util
from pathlib import Path
import tempfile
import unittest
from unittest.mock import Mock, patch

SCRIPT = Path(__file__).with_name("prove-w8-synthetic-runtime.py")
spec = importlib.util.spec_from_file_location("w8_host_probe", SCRIPT)
probe = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe)


class SerialBoundTests(unittest.TestCase):
    def test_marker_within_bound(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "serial.log"
            path.write_bytes(probe.MARKER)
            self.assertIn(probe.MARKER, probe.bounded_serial(path))

    def test_excess_serial_fails_closed(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "serial.log"
            path.write_bytes(probe.MARKER + b"x" * probe.MAX_SERIAL_BYTES)
            with self.assertRaisesRegex(RuntimeError, "exceeded host-proof bound"):
                probe.bounded_serial(path)

    def test_fifo_serial_path_fails_without_blocking(self):
        import os
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "serial.log"
            os.mkfifo(path)
            with self.assertRaisesRegex(RuntimeError, "not a regular file"):
                probe.bounded_serial(path)

    def test_symlink_serial_path_fails_closed(self):
        with tempfile.TemporaryDirectory() as root:
            target = Path(root) / "outside.log"
            target.write_bytes(probe.MARKER)
            path = Path(root) / "serial.log"
            path.symlink_to(target)
            with self.assertRaises(OSError):
                probe.bounded_serial(path)

    def test_exact_bound_is_allowed(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "serial.log"
            path.write_bytes(probe.MARKER + b"x" * (probe.MAX_SERIAL_BYTES - len(probe.MARKER)))
            self.assertEqual(len(probe.bounded_serial(path)), probe.MAX_SERIAL_BYTES)


class ProcessTeardownRaceTests(unittest.TestCase):
    def test_process_group_disappears_between_poll_and_signal(self):
        proc = Mock()
        proc.poll.return_value = None
        proc.pid = 12345
        proc.wait.return_value = 0
        with patch.object(probe.os, "killpg", side_effect=ProcessLookupError):
            self.assertEqual(probe.stop(proc, probe.signal.SIGTERM), 0)
        proc.wait.assert_called_once_with(timeout=3)

    def test_already_exited_process_is_not_signalled(self):
        proc = Mock()
        proc.poll.return_value = 0
        proc.wait.return_value = 0
        with patch.object(probe.os, "killpg") as killpg:
            self.assertEqual(probe.stop(proc, probe.signal.SIGTERM), 0)
            killpg.assert_not_called()

    def test_timeout_then_vanished_group_is_reaped(self):
        proc = Mock()
        proc.poll.return_value = None
        proc.pid = 12345
        proc.wait.side_effect = [probe.subprocess.TimeoutExpired("qemu", 3), -9]
        with patch.object(probe.os, "killpg", side_effect=[None, ProcessLookupError]) as killpg:
            self.assertEqual(probe.stop(proc, probe.signal.SIGTERM), -9)
            self.assertEqual(killpg.call_count, 2)


if __name__ == "__main__":
    unittest.main()
