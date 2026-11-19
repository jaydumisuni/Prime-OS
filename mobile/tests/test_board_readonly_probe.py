from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from board_readonly_probe import snapshot, _compatible

class BoardProbeTests(unittest.TestCase):
    def write(self,root,path,data):
        target=root/path.lstrip('/')
        target.parent.mkdir(parents=True,exist_ok=True)
        target.write_bytes(data)

    def test_device_tree_synthetic_identity(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            self.write(root,'/proc/device-tree/model',b'Prototype phone board\0')
            self.write(root,'/proc/device-tree/compatible',b'vendor,board-a\0vendor,soc-b\0')
            self.write(root,'/proc/sys/kernel/osrelease',b'6.12.0-test\n')
            result=snapshot(root,'aarch64','2026-11-15T01:00:00Z')
        self.assertEqual(result['board_model'],'Prototype phone board')
        self.assertEqual(result['board_compatible'],['vendor,board-a','vendor,soc-b'])
        self.assertEqual(result['limitations'],[])
        self.assertIn('neither',result['proof_boundary'].lower())

    def test_no_dmi_no_guess(self):
        with tempfile.TemporaryDirectory() as t:
            result=snapshot(Path(t),'aarch64')
        self.assertIsNone(result['board_model'])
        self.assertIsNone(result['board_compatible'])
        self.assertIn('MODEL_NOT_OBSERVED',result['limitations'])

    def test_unterminated_or_bad_device_tree_rejected(self):
        self.assertIsNone(_compatible(b'vendor,board'))
        self.assertIsNone(_compatible(b'\xff\0'))
        self.assertIsNone(_compatible(b'\0'))

    def test_sysfs_fallback_and_arch_mismatch(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            self.write(root,'/sys/firmware/devicetree/base/model',b'Model X\0')
            result=snapshot(root,'x86_64')
        self.assertEqual(result['board_model'],'Model X')
        self.assertEqual(result['sources']['model'],'/sys/firmware/devicetree/base/model')
        self.assertIn('HOST_NOT_IDENTIFIED_AS_ARM64',result['limitations'])

    def test_symlink_escape_does_not_leak_host(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            (root/'proc/device-tree').mkdir(parents=True)
            (root/'proc/device-tree/model').symlink_to('/etc/passwd')
            result=snapshot(root,'aarch64')
        self.assertIsNone(result['board_model'])

if __name__=='__main__':unittest.main()