"""Host-side negative testing only: no QEMU launch, guest or physical device."""
from __future__ import annotations
import hashlib
from pathlib import Path
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from run_qemu_guest import checked_invocation, pinned_file
from build import build


def sha(b:bytes)->str:
    return hashlib.sha256(b).hexdigest()

class QemuAdmissionTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temp=tempfile.TemporaryDirectory()
        cls.root=Path(cls.temp.name)
        cls.qemu=cls.root/'qemu-system-aarch64'
        cls.qemu.write_bytes(b'\x7fELF'+bytes(60))
        cls.qemu.chmod(0o755)
        cls.kernel=cls.root/'Image'
        image=bytearray(64)
        image[56:60]=b'ARM\x64'
        cls.kernel.write_bytes(image)
        cls.archive=cls.root/'build'
        build(cls.archive)
        cls.gz=cls.archive/'initramfs.cpio.gz'
    @classmethod
    def tearDownClass(cls): cls.temp.cleanup()
    def admitted(self,qemu=None,kernel=None,initrd=None,**kwargs):
        qemu=qemu or self.qemu
        kernel=kernel or self.kernel
        initrd=initrd or self.gz
        return checked_invocation(qemu,kernel,initrd,
                                  kwargs.get('qemu_sha') or (sha(qemu.read_bytes()) if qemu.is_file() else '0'*64),
                                  kwargs.get('kernel_sha',sha(kernel.read_bytes())),
                                  kwargs.get('initrd_sha',sha(initrd.read_bytes())),
                                  kwargs.get('timeout',30))
    def test_inputs_admit_only_pinned_raw_image_and_corrected_initramfs(self):
        x=self.admitted()
        self.assertTrue(x['archive']['validated'])
        self.assertIn('-net',x['command']);self.assertIn('none',x['command'])
        self.assertIn('-nodefaults',x['command'])
        self.assertNotIn('-drive',x['command'])
        self.assertNotIn('-device',x['command'])
        self.assertNotIn('-usb',x['command'])
        self.assertTrue(x['not_prime_core'])
        self.assertTrue(x['not_a_phone_boot'])
    def test_mismatched_emulator_pin_rejected(self):
        with self.assertRaisesRegex(ValueError,'QEMU:SHA256_MISMATCH'):
            self.admitted(qemu_sha='0'*64)
    def test_mismatched_kernel_pin_rejected(self):
        with self.assertRaisesRegex(ValueError,'KERNEL_IMAGE:SHA256_MISMATCH'):
            self.admitted(kernel_sha='0'*64)
    def test_mismatched_initramfs_pin_rejected(self):
        with self.assertRaisesRegex(ValueError,'INITRAMFS:SHA256_MISMATCH'):
            self.admitted(initrd_sha='0'*64)
    def test_missing_emulator_rejected(self):
        missing=self.root/'no-such-qemu-system-aarch64'
        with self.assertRaisesRegex(ValueError,'QUALIFIED_QEMU_AARCH64_EXECUTABLE_REQUIRED'):
            self.admitted(qemu=missing)
    def test_bad_kernel_magic_rejected(self):
        other=self.root/'bad-kernel'
        other.write_bytes(bytes(64))
        with self.assertRaisesRegex(ValueError,'NOT_UNCOMPRESSED_ARM64_LINUX_IMAGE'):
            self.admitted(kernel=other)
    def test_bad_qemu_magic_rejected(self):
        other=self.root/'emulators'/'qemu-system-aarch64';other.parent.mkdir()
        other.write_bytes(b'not-elf'+bytes(60));other.chmod(0o755)
        with self.assertRaisesRegex(ValueError,'QEMU_BINARY_IS_NOT_ELF'):
            self.admitted(qemu=other)
    def test_bad_raw_hash_format_rejected(self):
        with self.assertRaisesRegex(ValueError,'INVALID_SHA256_PIN'):
            self.admitted(qemu_sha='not-a-pin')
    def test_timeout_must_be_bounded(self):
        with self.assertRaisesRegex(ValueError,'BOUNDED_TIMEOUT_REQUIRED'):
            self.admitted(timeout=300)
    def test_compressed_bomb_rejected_before_cpio_parse(self):
        import gzip
        other=self.root/'gzip-bomb.gz'
        other.write_bytes(gzip.compress(b'X'*3_000_000,mtime=0))
        with self.assertRaisesRegex(ValueError,'INITRAMFS_TOO_LARGE'):
            self.admitted(initrd=other)

    def test_malformed_gzip_rejected(self):
        other=self.root/'bad.gz'
        other.write_bytes(b'NOT-GZIP')
        with self.assertRaisesRegex(ValueError,'INITRAMFS_NOT_VALID_GZIP'):
            self.admitted(initrd=other)

    def test_old_initramfs_cannot_pass_even_with_valid_hash(self):
        from build import initramfs,newc_entry
        import gzip
        # Simulate original incomplete rootfs layout using valid newc syntax.
        elf=(self.archive/'init-aarch64').read_bytes()
        old=b''.join(newc_entry(n,d,m,i+1) for i,(n,d,m) in enumerate([
            ('dev',b'',0o040755),('proc',b'',0o040755),
            ('sys',b'',0o040755),('init',elf,0o100755),
            ('TRAILER!!!',b'',0)]))
        previous=self.root/'old-initramfs.cpio.gz'
        previous.write_bytes(gzip.compress(old,mtime=0))
        with self.assertRaisesRegex(ValueError,'CPIO_REQUIRED_ENTRIES'):
            self.admitted(initrd=previous)

if __name__=='__main__':unittest.main()
