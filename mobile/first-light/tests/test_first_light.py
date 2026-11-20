"""Host-side, purely structural tests for cross-compiled ARM64 early-init fixture."""
from __future__ import annotations

import gzip
import hashlib
import json
from pathlib import Path
import shutil
import struct
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
import build as b
from verify_serial import evaluate


class FirstLightTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if not shutil.which('clang') or not shutil.which('ld.lld'):
            raise unittest.SkipTest('Clang/LLD unavailable; no false compilation result')
        cls.tmp = tempfile.TemporaryDirectory()
        cls.out = Path(cls.tmp.name)/'a'
        cls.receipt = b.build(cls.out)
        cls.elf = (cls.out/'init-aarch64').read_bytes()

    @classmethod
    def tearDownClass(cls):
        if hasattr(cls, 'tmp'): cls.tmp.cleanup()

    def test_static_elf_real_arm64_entry_and_content(self):
        metadata=b.inspect_arm64_elf(self.elf)
        self.assertEqual(metadata['architecture'],'aarch64')
        self.assertEqual(metadata['elf_type'],'ET_EXEC')
        self.assertGreater(int(metadata['entry_address'],16),0)
        self.assertTrue(metadata['static'])
        self.assertGreaterEqual(metadata['load_segments'],1)
        self.assertIn(b'PRIME_MOB1_V0|stage=INIT_ENTERED',self.elf)
        self.assertFalse(self.receipt['prime_core_compiled'])
        self.assertFalse(self.receipt['arm64_guest_boot_observed'])

    def test_repeated_compiles_and_archives_byte_identical(self):
        second=b.build(Path(self.tmp.name)/'b')
        self.assertEqual(second['elf']['sha256'],self.receipt['elf']['sha256'])
        self.assertEqual(second['initramfs_cpio_sha256'],self.receipt['initramfs_cpio_sha256'])
        self.assertEqual(second['initramfs_gzip_sha256'],self.receipt['initramfs_gzip_sha256'])
        self.assertEqual((self.out/'initramfs.cpio.gz').read_bytes(),
                         (Path(self.tmp.name)/'b'/'initramfs.cpio.gz').read_bytes())

    def test_cpio_independently_parses_entries_and_init_bytes(self):
        raw=(self.out/'initramfs.cpio').read_bytes()
        self.assertEqual(gzip.decompress((self.out/'initramfs.cpio.gz').read_bytes()),raw)
        off=0;items={}
        while off<len(raw):
            self.assertEqual(raw[off:off+6],b'070701')
            vals=[int(raw[off+6+8*i:off+14+8*i],16) for i in range(13)]
            mode,size,namesize=vals[1],vals[6],vals[11]
            start=off+110
            name=raw[start:start+namesize-1].decode('ascii')
            self.assertEqual(raw[start+namesize-1],0)
            payload_start=(start+namesize+3)&~3
            data=raw[payload_start:payload_start+size]
            off=(payload_start+size+3)&~3
            items[name]=(mode,data)
            if name=='TRAILER!!!':break
        self.assertEqual(set(items),{'dev','dev/console','dev/null','proc','sys','init','TRAILER!!!'})
        self.assertEqual(items['init'][0]&0o170000,0o100000)
        self.assertEqual(items['init'][0]&0o111,0o111)
        self.assertEqual(items['init'][1],self.elf)
        for name in ('dev','proc','sys'):
            self.assertEqual(items[name][0]&0o170000,0o040000)
        self.assertEqual(off,len(raw))
        structure=b.inspect_initramfs(raw)
        self.assertTrue(structure['validated'])
        self.assertEqual(structure['console'],{'type':'character','major':5,'minor':1})

    def test_cpio_console_and_null_are_explicit_device_nodes(self):
        raw=(self.out/'initramfs.cpio').read_bytes()
        inspected=b.inspect_initramfs(raw)
        self.assertEqual(inspected['null'],{'type':'character','major':1,'minor':3})
        self.assertEqual(self.receipt['initramfs_structure'], inspected)

    def test_cpio_missing_console_fails_closed(self):
        original=(self.out/'initramfs.cpio').read_bytes()
        # Alter pathname without affecting record size or archive offsets.
        damaged=original.replace(b'dev/console\0',b'dev/consolX\0',1)
        with self.assertRaisesRegex(ValueError,'CPIO_REQUIRED_ENTRIES'):
            b.inspect_initramfs(damaged)

    def test_cpio_wrong_console_major_fails_closed(self):
        raw=bytearray((self.out/'initramfs.cpio').read_bytes())
        pos=raw.find(b'dev/console\0')
        self.assertGreater(pos,100)
        # cpio's rdevmajor field is the 10th 8-hex field at entry+6+9*8.
        entry=pos-110
        raw[entry+6+9*8:entry+14+9*8]=b'00000000'
        with self.assertRaisesRegex(ValueError,'CPIO_DEVICE_NODE_INVALID:dev/console'):
            b.inspect_initramfs(bytes(raw))

    def test_cpio_trailing_bytes_fail_closed(self):
        raw=(self.out/'initramfs.cpio').read_bytes()+b'UNTRUSTED'
        with self.assertRaisesRegex(ValueError,'CPIO_TRAILING_BYTES'):
            b.inspect_initramfs(raw)

    def test_wrong_elf_architecture_fails(self):
        raw=bytearray(self.elf);struct.pack_into('<H',raw,18,62) # x86_64
        with self.assertRaisesRegex(ValueError,'NOT_AARCH64'):
            b.inspect_arm64_elf(raw)

    def test_truncated_header_fails(self):
        with self.assertRaisesRegex(ValueError,'ELF'):
            b.inspect_arm64_elf(self.elf[:60])

    def test_modified_boot_marker_fails(self):
        marker=b'PRIME_MOB1_V0|stage=INIT_ENTERED'
        raw=self.elf.replace(marker,b'PRIME_MOB1_V0|stage=INIX_ENTERED')
        self.assertNotEqual(raw,self.elf)
        with self.assertRaisesRegex(ValueError,'MARKERS_MISSING'):
            b.inspect_arm64_elf(raw)

    def test_entry_outside_load_segments_fails(self):
        raw=bytearray(self.elf);struct.pack_into('<Q',raw,24,0x123456788)
        with self.assertRaisesRegex(ValueError,'ENTRY_NOT'):
            b.inspect_arm64_elf(raw)

    def test_no_dynamic_interpreter(self):
        header=b.inspect_arm64_elf(self.elf)
        self.assertTrue(header['static'])
        self.assertEqual(self.receipt['target'],'qemu-virt-generic-arm64-not-phone')


class SerialReceiptTests(unittest.TestCase):
    GOOD='''Kernel boot begins
PRIME_MOB1_V0|stage=INIT_ENTERED
PRIME_MOB1_V0|compiled_arch=aarch64
PRIME_MOB1_V0|pid1=true
PRIME_MOB1_V0|kernel_machine=aarch64
PRIME_MOB1_V0|proc_mount=AVAILABLE
PRIME_MOB1_V0|sys_mount=AVAILABLE
PRIME_MOB1_V0|mem_total_kib=505436
PRIME_MOB1_V0|device_tree_model=linux,dummy-virt
PRIME_MOB1_V0|stage=EVIDENCE_ONLY_CORE_NOT_STARTED
'''
    def test_valid_trace_only_confirms_fixture(self):
        x=evaluate(self.GOOD)
        self.assertTrue(x['candidate_init_observed'])
        self.assertEqual(x['status'],'GUEST_EARLY_INIT_OBSERVED_ONLY')
        self.assertFalse(x['prime_core_ready'])
        self.assertFalse(x['physical_phone_qualified'])
        self.assertFalse(x['flash_authorization'])

    def test_missing_stage_fails(self):
        x=evaluate(self.GOOD.replace('PRIME_MOB1_V0|stage=INIT_ENTERED\n',''))
        self.assertFalse(x['candidate_init_observed'])
        self.assertIn('MISSING_OR_INVALID_STAGE_ORDER',x['blockers'])

    def test_duplicate_field_fails(self):
        x=evaluate(self.GOOD+'PRIME_MOB1_V0|pid1=true\n')
        self.assertIn('DUPLICATE:pid1',x['blockers'])

    def test_wrong_kernel_arch_fails(self):
        x=evaluate(self.GOOD.replace('kernel_machine=aarch64','kernel_machine=x86_64'))
        self.assertFalse(x['candidate_init_observed'])
        self.assertIn('KERNEL_ARCH_UNKNOWN_OR_MISMATCH',x['blockers'])

    def test_non_pid1_fails(self):
        self.assertFalse(evaluate(self.GOOD.replace('pid1=true','pid1=false'))['candidate_init_observed'])

    def test_unknown_memory_fails(self):
        self.assertFalse(evaluate(self.GOOD.replace('mem_total_kib=505436','mem_total_kib=UNKNOWN'))['candidate_init_observed'])

    def test_unknown_dt_model_only_limits(self):
        x=evaluate(self.GOOD.replace('device_tree_model=linux,dummy-virt','device_tree_model=UNKNOWN'))
        self.assertTrue(x['candidate_init_observed'])
        self.assertIn('DEVICE_TREE_MODEL_UNKNOWN',x['limitations'])

    def test_untrusted_control_char_marker_rejected(self):
        x=evaluate(self.GOOD+'PRIME_MOB1_V0|stage=INIT_ENTERED\x1b[31m\n')
        self.assertFalse(x['candidate_init_observed'])
        self.assertIn('INVALID_CONTROL_OR_LENGTH',x['blockers'])

    def test_absent_markers_fails(self):
        x=evaluate('boot log with no Prime result')
        self.assertFalse(x['candidate_init_observed'])
        self.assertFalse(x['prime_core_ready'])


if __name__=='__main__':unittest.main()
