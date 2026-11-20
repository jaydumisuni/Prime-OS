"""Source-only bounded serial regression; never starts QEMU."""
import importlib.util
from pathlib import Path
import tempfile
import unittest

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

    def test_exact_bound_is_allowed(self):
        with tempfile.TemporaryDirectory() as root:
            path = Path(root) / "serial.log"
            path.write_bytes(probe.MARKER + b"x" * (probe.MAX_SERIAL_BYTES - len(probe.MARKER)))
            self.assertEqual(len(probe.bounded_serial(path)), probe.MAX_SERIAL_BYTES)


if __name__ == "__main__":
    unittest.main()
