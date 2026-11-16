"""Fail-closed source-only A14m Core lab integration checks; not boot tests."""
from __future__ import annotations
import copy
import gzip
import hashlib
import json
from pathlib import Path
import struct
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'tools'))
from a14m_core_lab_stage import (SCHEMA, check_core_bundle, entries_newc,
                                 aarch64_elf, read_gzip_cpio, CORE_HEAD)
from a14m_stage_boot import newc, sha

KERNEL_SHA='a'*64


def elf(payload=b'', dynamic=False, interp=False):
    data=bytearray(1024)
    data[:6]=b'\x7fELF\x02\x01'
    struct.pack_into('<H',data,16,2)
    struct.pack_into('<H',data,18,183)
    struct.pack_into('<Q',data,32,64)
    struct.pack_into('<HH',data,54,56,2 if interp else 1)
    struct.pack_into('<II',data,64,2 if dynamic else 1,5)
    if interp:
        struct.pack_into('<II',data,120,3,4)
        struct.pack_into('<Q',data,128,800)
        string=b'/lib/ld-linux-aarch64.so.1\0'
        struct.pack_into('<Q',data,152,len(string))
        data[800:800+len(string)]=string
    data[350:350+len(payload)]=payload
    return bytes(data)


def fixture():
    init=elf(b'PRIME_MOB1_CORE_V0|stage=TRANSPORT_ONLY_UNQUALIFIED|'
             b'PRIME_A14M_SOURCE_BRIDGE|not_a_phone_boot_proof=true|'
             b'PRIME_MOB1_LAB_INPUT_DIGEST=sha256:'+KERNEL_SHA.encode())
    core=elf(b'authentic-in-ci-only',interp=True)
    lib=elf(b'synthetic-lib')
    loader=elf(b'synthetic-loader')
    files=[
        ('dev',b'',0o040755,0,0), ('proc',b'',0o040755,0,0),
        ('sys',b'',0o040755,0,0), ('run',b'',0o040755,0,0),
        ('run/prime-mob1-lab',b'',0o040755,0,0),
        ('usr',b'',0o040755,0,0), ('usr/bin',b'',0o040755,0,0),
        ('lib',b'',0o040755,0,0), ('lib/aarch64-linux-gnu',b'',0o040755,0,0),
        ('dev/console',b'',0o020600,5,1), ('dev/null',b'',0o020666,1,3),
        ('init',init,0o100755,0,0), ('usr/bin/primed',core,0o100755,0,0),
        ('lib/ld-linux-aarch64.so.1',loader,0o100755,0,0),
        ('lib/aarch64-linux-gnu/libc.so.6',lib,0o100755,0,0),
        ('TRAILER!!!',b'',0,0,0)
    ]
    raw=b''.join(newc(n,b,mode,i+1,major,minor)
                 for i,(n,b,mode,major,minor) in enumerate(files))
    zipped=gzip.compress(raw,compresslevel=9,mtime=0)
    manifest={
        'schema':'prime.mob1.primed-guest-unqualified-bundle.v0',
        'admission':'UNQUALIFIED_LAB_TRANSPORT_ONLY_NOT_BOOTC',
        'arm64_core_sha256':sha(core), 'arm64_guest_init_sha256':sha(init),
        'initramfs_sha256':sha(zipped), 'initramfs_uncompressed_sha256':sha(raw),
        'entries':[x[0] for x in files], 'needed':['libc.so.6'],
        'bootc_qualified':False, 'prime_core_deployment_proven':False,
        'physical_phone_supported':False, 'writes_real_hardware':False
    }
    return raw,zipped,manifest


class LabBundleTests(unittest.TestCase):
    def test_source_only_bundle_membership_and_hashes(self):
        raw,z,m=fixture()
        out=check_core_bundle(raw,z,m,KERNEL_SHA)
        self.assertEqual(out['ramdisk_sha256'],sha(z))
        self.assertEqual(out['core_sha256'],m['arm64_core_sha256'])
        self.assertEqual(out['entry_count'],len(m['entries']))

    def test_wrong_kernel_digest_is_not_accepted(self):
        raw,z,m=fixture()
        with self.assertRaisesRegex(ValueError,'WRONG_HISTORICAL_SOURCE_DIGEST'):
            check_core_bundle(raw,z,m,'b'*64)

    def test_bootc_qualified_claim_is_rejected(self):
        raw,z,m=fixture();m['bootc_qualified']=True
        with self.assertRaisesRegex(ValueError,'PRODUCTION_FLAG'):
            check_core_bundle(raw,z,m,KERNEL_SHA)

    def test_missing_required_loader_is_rejected(self):
        raw,z,m=fixture();m['entries'].remove('lib/ld-linux-aarch64.so.1')
        with self.assertRaisesRegex(ValueError,'ENTRIES_MISMATCH'):
            check_core_bundle(raw,z,m,KERNEL_SHA)

    def test_manifest_hash_mismatch_is_rejected(self):
        raw,z,m=fixture();m['arm64_core_sha256']='0'*64
        with self.assertRaisesRegex(ValueError,'ELF_DIGEST_MISMATCH'):
            check_core_bundle(raw,z,m,KERNEL_SHA)

    def test_non_arm64_core_is_rejected(self):
        raw,z,m=fixture()
        b=bytearray(elf(interp=True));struct.pack_into('<H',b,18,62)
        with self.assertRaisesRegex(ValueError,'NOT_ARM64_ELF'):
            aarch64_elf(bytes(b),'usr/bin/primed',needs_interpreter=True)

    def test_dynamic_pid1_is_rejected(self):
        with self.assertRaisesRegex(ValueError,'NOT_STATIC'):
            aarch64_elf(elf(dynamic=True),'init',needs_interpreter=False,static_only=True)

    def test_non_absolute_or_unknown_loader_is_rejected(self):
        with self.assertRaisesRegex(ValueError,'WRONG_ARM64_LOADER'):
            aarch64_elf(elf(),'primed',needs_interpreter=True)

    def test_duplicate_cpio_member_is_rejected(self):
        raw,z,m=fixture()
        bad=raw.replace(b'sys\x00',b'dev\x00',1)
        with self.assertRaisesRegex(ValueError,'DUPLICATE'):
            entries_newc(bad)
        with self.assertRaisesRegex(ValueError,'BAD_HEADER'):
            entries_newc(raw[:140])

    def test_unqualified_labels_remain_explicit(self):
        self.assertIn('unqualified',SCHEMA)
        self.assertEqual(len(CORE_HEAD),40)
        raw,z,m=fixture()
        self.assertIs(False,m['physical_phone_supported'])

if __name__=='__main__': unittest.main()
