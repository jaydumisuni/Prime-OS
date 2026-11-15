import json
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from capture_readonly_evidence import capture

class CaptureAdapterTests(unittest.TestCase):
    def write(self,root,path,data):
        f=root/path.lstrip('/');f.parent.mkdir(parents=True,exist_ok=True);f.write_bytes(data)

    def test_fixture_marked_simulated_and_no_bootloader_claim(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            self.write(root,'/proc/device-tree/model',b'Test phone\0')
            self.write(root,'/proc/device-tree/compatible',b'vendor,test\0')
            self.write(root,'/proc/sys/kernel/osrelease',b'6.12-lab\n')
            self.write(root,'/proc/meminfo',b'MemTotal: 8192000 kB\nMemAvailable: 4096000 kB\nSwapTotal: 0 kB\nSwapFree: 0 kB\n')
            x=capture(root,'case-1','aarch64','2026-11-15T01:00:00Z')
        self.assertEqual(x['capture_kind'],'SIMULATED')
        self.assertIn('board_device_tree',x['mob0_manifest']['observations'])
        self.assertIn('current_kernel_build',x['mob0_manifest']['observations'])
        self.assertNotIn('bootloader_lock_state',x['mob0_manifest']['observations'])
        self.assertEqual(x['mob0_preflight']['evidence_status']['board_device_tree'],'SIMULATED_NOT_PHYSICAL')
        self.assertEqual(x['mob0_preflight']['evidence_status']['physical_device_identity'],'MISSING')
        self.assertFalse(x['mob0_preflight']['device_write_authorized'])
        self.assertFalse(x['device_modification_performed'])
        self.assertEqual(x['phi_memory']['memory']['physical_total_bytes'],8192000*1024)

    def test_unknown_source_does_not_invent_board_identity(self):
        with tempfile.TemporaryDirectory() as t:
            x=capture(Path(t),'case-2','aarch64','2026-11-15T01:00:00Z')
        self.assertEqual(x['mob0_manifest']['observations'],{})
        self.assertEqual(x['mob0_preflight']['evidence_status']['board_device_tree'],'MISSING')

    def test_canonical_digest_reproducible(self):
        with tempfile.TemporaryDirectory() as t:
            r=Path(t)
            self.write(r,'/proc/device-tree/model',b'Board X\0')
            self.write(r,'/proc/device-tree/compatible',b'vendor,x\0')
            a=capture(r,'case-3','aarch64','2026-11-15T01:00:00Z')
            b=capture(r,'case-3','aarch64','2026-11-15T01:00:00Z')
        self.assertEqual(json.dumps(a,sort_keys=True),json.dumps(b,sort_keys=True))
        self.assertTrue(a['mob0_manifest']['observations']['board_device_tree']['evidence_ref'].startswith('sha256:'))

    def test_reject_empty_alias(self):
        with tempfile.TemporaryDirectory() as t:
            with self.assertRaises(ValueError):capture(Path(t),'')

    def test_x86_fixture_does_not_claim_mobile_dt(self):
        with tempfile.TemporaryDirectory() as t:
            r=Path(t)
            self.write(r,'/proc/device-tree/model',b'Desktop board\0')
            self.write(r,'/proc/device-tree/compatible',b'vendor,x\0')
            x=capture(r,'case-x86','x86_64','2026-11-15T01:00:00Z')
        self.assertEqual(x['mob0_manifest']['observations'],{})
        self.assertFalse(x['mob0_preflight']['physical_acceptance'])

if __name__=='__main__':unittest.main()