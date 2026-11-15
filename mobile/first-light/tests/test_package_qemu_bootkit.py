"""Offline structural tests only; never claim the synthetic fixture boots."""
from __future__ import annotations
import gzip
import hashlib
from pathlib import Path
import tempfile
import unittest
import zipfile
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from package_qemu_bootkit import KIT_ROOT, create_zip

def newc_entry(name: str, data: bytes, mode: int, inode: int) -> bytes:
    # Independent structural fixture encoder; NOT bootable guest content.
    name_b=name.encode()+b'\0'
    fields=[inode,mode,0,0,1,0,len(data),0,0,0,0,len(name_b),0]
    header=b'070701'+b''.join(f'{x:08x}'.encode() for x in fields)
    output=header+name_b
    output+=b'\0'*((-len(output))%4)
    output+=data
    output+=b'\0'*((-len(data))%4)
    return output


SHA40 = 'a' * 40
SHA64 = 'b' * 64


class BootkitTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.kernel = self.root/'Image'
        header = bytearray(80)
        header[56:60] = b'ARMd'
        self.kernel.write_bytes(header)
        entries = [
            ('dev', b'', 0o040755),
            ('init', b'fake ELF is structural fixture only', 0o100755),
            ('usr/bin/primed', b'fake ELF is structural fixture only', 0o100755),
            ('TRAILER!!!', b'', 0),
        ]
        cpio = b''.join(newc_entry(name, data, mode, index + 1)
                        for index, (name, data, mode) in enumerate(entries))
        self.initrd = self.root/'test.cpio.gz'
        self.initrd.write_bytes(gzip.compress(cpio, mtime=0))
        self.verifier = self.root/'verify.py'
        self.verifier.write_bytes(b'CORE_TRANSPORT_RESPONDED_UNQUALIFIED\nbootc_generation_bound\n')

    def make(self, name='bootkit.zip'):
        return create_zip(self.kernel, self.initrd, self.verifier,
                          self.root/name, SHA40, SHA64)

    def test_package_has_hash_manifest_and_no_phone_claim(self):
        receipt = self.make()
        self.assertFalse(receipt['flash_authorization'])
        self.assertFalse(receipt['core_deployment_accepted'])
        with zipfile.ZipFile(self.root/'bootkit.zip') as z:
            self.assertIsNone(z.testzip())
            names = z.namelist()
            self.assertEqual(len(names), 7)
            self.assertTrue(all(x.startswith(KIT_ROOT+'/') for x in names))
            self.assertIn(KIT_ROOT+'/SHA256SUMS', names)
            self.assertIn(KIT_ROOT+'/run-qemu.sh', names)
            self.assertIn(b'-net none -nodefaults',z.read(KIT_ROOT+'/run-qemu.sh'))
            self.assertIn(b'CORE_TRANSPORT_RESPONDED_UNQUALIFIED',
                          z.read(KIT_ROOT+'/run-qemu.sh'))

    def test_output_is_byte_deterministic(self):
        a=self.make('one.zip')
        b=self.make('two.zip')
        self.assertEqual(a['zip_sha256'],b['zip_sha256'])
        self.assertEqual((self.root/'one.zip').read_bytes(),(self.root/'two.zip').read_bytes())

    def test_non_arm64_kernel_fails_closed(self):
        self.kernel.write_bytes(bytes(80))
        with self.assertRaisesRegex(ValueError, 'NOT_RAW_ARM64_LINUX_IMAGE'):
            self.make()

    def test_non_cpio_or_missing_primed_fails_closed(self):
        self.initrd.write_bytes(gzip.compress(b'not newc',mtime=0))
        with self.assertRaisesRegex(ValueError, 'INITRAMFS_NOT_NEWC_CPIO'):
            self.make()

    def test_unpinned_source_identity_fails_closed(self):
        with self.assertRaisesRegex(ValueError,'INVALID_SOURCE_COMMIT'):
            create_zip(self.kernel,self.initrd,self.verifier,self.root/'bad.zip','main',SHA64)

    def test_zip_contains_no_qemu_executable(self):
        self.make()
        with zipfile.ZipFile(self.root/'bootkit.zip') as z:
            self.assertFalse(any('qemu-system-aarch64' in x for x in z.namelist()))


if __name__=='__main__':
    unittest.main()
