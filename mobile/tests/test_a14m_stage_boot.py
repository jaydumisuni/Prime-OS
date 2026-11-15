from __future__ import annotations
import importlib.util
from pathlib import Path
import struct
import sys
import tempfile
import unittest
import zipfile

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from a14m_stage_boot import (BOOT_HEADER_SIZE,MAX_IMAGE,align,boot_image,
                             inspect_boot_image,make_ramdisk,parse_boot_info,
                             parse_newc,git_blob,safe_zip,stage,inspect_reference_dt_table)

SAMPLE="""kernel=kernel
ramdisk=ramdisk
page_size=2048
dt=dtb.img
kernel_size=11564226
ramdisk_size=1944709
dtb_offset=0x07808000
dtb_size=135300
base_addr=0x40078000
kernel_offset=0x00008000
ramdisk_offset=0x07c08000
tags_offset=0x0bc08000
dtbo_offset=0xbff88000
cmd_line='bootopt=64S3,32N2,64N2 loop.max_part=7'
board="SRPVJ12A002"
"""

def artificial_arm64_init()->bytes:
    # Small unexecutable fake ELF for source-format tests: authentic arm64
    # instruction execution is checked only in the real Clang CI build step.
    b=bytearray(512)
    b[:6]=b'\x7fELF\x02\x01';struct.pack_into('<H',b,16,2);struct.pack_into('<H',b,18,183)
    struct.pack_into('<Q',b,32,64);struct.pack_into('<HH',b,54,56,1)
    struct.pack_into('<II',b,64,1,5)
    b+=b'PRIME_A14M_FIRST_LIGHT|stage=ENTERED_UNQUALIFIED\x00'
    b+=b'NO_PRIME_CORE_NO_PHONE_QUALIFICATION\x00'
    return bytes(b)

def sample_android_dt_table()->bytes:
    # dt_table_header v0: one 32-byte entry pointing to 64-byte FDT at 64.
    fdt=bytes.fromhex('d00dfeed')+struct.pack('>I',64)+b'\0'*56
    head=struct.pack('<8I',0x1eabb7d7,128,32,32,1,32,2048,0)
    entry=struct.pack('<8I',64,64,0,0,0,0,0,0)
    return head+entry+fdt

def simple_info()->dict:
    return parse_boot_info(SAMPLE)

class A14mBootV2StagingTests(unittest.TestCase):
    def test_source_geometry_parsed_from_historical_dump(self):
        info=simple_info()
        self.assertEqual(info['base_addr'],0x40078000)
        self.assertEqual(info['dtb_offset'],0x07808000)
        self.assertEqual(info['cmd_line'],'bootopt=64S3,32N2,64N2 loop.max_part=7')

    def test_unknown_geometry_is_not_normalized_to_reference(self):
        with self.assertRaisesRegex(ValueError,'GEOMETRY'):
            parse_boot_info(SAMPLE.replace('page_size=2048','page_size=4096'))

    def test_rejects_fake_patch_permissive_cmdline(self):
        with self.assertRaisesRegex(ValueError,'UNSAFE_REFERENCE_CMDLINE'):
            parse_boot_info(SAMPLE.replace('loop.max_part=7','androidboot.selinux=permissive'))

    def test_bad_boot_metadata_is_fatal(self):
        for variant in (SAMPLE+'page_size=4096\n',SAMPLE.replace('board="SRPVJ12A002"','board="other"'),SAMPLE.replace('ramdisk_offset=0x07c08000','ramdisk_offset=nothex')):
            with self.subTest(variant=variant[-40:]):
                with self.assertRaises(ValueError):parse_boot_info(variant)

    def test_newc_contains_exact_character_devices_and_static_init(self):
        archive,detail=make_ramdisk(artificial_arm64_init())
        import gzip
        self.assertEqual(detail['entry_count'],8)
        self.assertEqual(parse_newc(gzip.decompress(archive)),detail)
        self.assertEqual(archive,make_ramdisk(artificial_arm64_init())[0])

    def test_ramdisk_rejects_wrong_arch(self):
        body=bytearray(artificial_arm64_init());struct.pack_into('<H',body,18,62)
        with self.assertRaisesRegex(ValueError,'INIT_NOT_AARCH64_ELF'):
            make_ramdisk(bytes(body))

    def test_ramdisk_rejects_dynamic_loader_and_missing_truth_marker(self):
        body=bytearray(artificial_arm64_init());struct.pack_into('<I',body,64,3)
        with self.assertRaisesRegex(ValueError,'INIT_NOT_STATIC'):
            make_ramdisk(bytes(body))
        body=artificial_arm64_init().replace(b'NO_PRIME_CORE_NO_PHONE_QUALIFICATION',b'NO_CORE_' + b'_'*30)
        with self.assertRaisesRegex(ValueError,'INIT_MISSING_TRUTH_MARKERS'):
            make_ramdisk(body)

    def test_boot_image_v2_header_and_independent_section_roundtrip(self):
        info=simple_info()
        info['kernel_size']=1024;info['dtb_size']=128
        kernel=b'kernel-awg1-boot-not-recovery'.ljust(1024,b'K')
        dtb=sample_android_dt_table()
        ramdisk,_=make_ramdisk(artificial_arm64_init())
        binary=boot_image(kernel,ramdisk,dtb,info)
        receipt=inspect_boot_image(binary,info,kernel,ramdisk,dtb)
        self.assertEqual(binary[:8],b'ANDROID!')
        self.assertEqual(receipt['header_version'],2)
        self.assertEqual(receipt['dtb_load_address'],hex(0x47880000))
        self.assertEqual(receipt['total_bytes']%2048,0)
        self.assertLessEqual(receipt['total_bytes'],MAX_IMAGE)
        self.assertEqual(len(binary[:BOOT_HEADER_SIZE]),1660)

    def test_tampered_image_byte_fails_closed(self):
        info=simple_info();info['kernel_size']=1024;info['dtb_size']=128
        kernel=b'K'*1024;dtb=sample_android_dt_table()
        rd,_=make_ramdisk(artificial_arm64_init())
        data=bytearray(boot_image(kernel,rd,dtb,info));data[2048]^=0xff
        with self.assertRaisesRegex(ValueError,'ROUNDTRIP'):
            inspect_boot_image(bytes(data),info,kernel,rd,dtb)

    def test_actual_awg1_dt_table_layout_preserved(self):
        packet=sample_android_dt_table()
        found=inspect_reference_dt_table(packet)
        self.assertEqual(found['format'],'ANDROID_DT_TABLE_V0_WITH_ONE_FDT')
        self.assertEqual((found['fdt_offset'],found['fdt_bytes']),(64,64))
        self.assertEqual(found['entry_count'],1)

    def test_dt_table_fdt_size_and_count_fail_closed(self):
        packet=bytearray(sample_android_dt_table())
        packet[16]=2  # bogus entry count instead of the single observed FDT
        with self.assertRaisesRegex(ValueError,'HEADER_UNEXPECTED'):
            inspect_reference_dt_table(bytes(packet))
        packet=bytearray(sample_android_dt_table())
        packet[68:72]=struct.pack('>I',63) # FDT disagrees with table size
        with self.assertRaisesRegex(ValueError,'SIZE_MISMATCH'):
            inspect_reference_dt_table(bytes(packet))
        packet=bytearray(sample_android_dt_table())
        packet[36:40]=struct.pack('<I',32)  # FDT overlap with table
        with self.assertRaisesRegex(ValueError,'ENTRY_INVALID'):
            inspect_reference_dt_table(bytes(packet))

    def test_dtb_magic_required(self):
        info=simple_info();info['kernel_size']=100;info['dtb_size']=128
        with self.assertRaisesRegex(ValueError,'AWG1_DT_TABLE_HEADER_UNEXPECTED'):
            boot_image(b'K'*100,b'R',b'X'*128,info)

    def test_stage_rejects_arbitrary_kernel_even_with_matching_size(self):
        with tempfile.TemporaryDirectory() as td:
            root=Path(td);info=root/'img_info';info.write_text(SAMPLE)
            k=root/'kernel';k.write_bytes(b'Z'*11564226)
            dtb=root/'dtb';dtb.write_bytes(bytes.fromhex('d00dfeed')+b'\0'*135296)
            early=root/'init';early.write_bytes(artificial_arm64_init())
            with self.assertRaisesRegex(ValueError,'HISTORICAL_KERNEL_BLOB_MISMATCH'):
                stage(k,dtb,info,early,root/'out','a'*40)
            self.assertFalse((root/'out'/'A14M_REFERENCE_STAGING_DO_NOT_FLASH.zip').exists())

    def test_safe_zip_deterministic_and_no_flash_package_root(self):
        with tempfile.TemporaryDirectory() as td:
            a=Path(td)/'a.zip';b=Path(td)/'b.zip'
            safe_zip(a,{'README_NOT_FOR_FLASH.txt':b'Only staging'})
            safe_zip(b,{'README_NOT_FOR_FLASH.txt':b'Only staging'})
            self.assertEqual(a.read_bytes(),b.read_bytes())
            with zipfile.ZipFile(a) as z:
                self.assertEqual(z.namelist(),['A14M_REFERENCE_STAGING_DO_NOT_FLASH/README_NOT_FOR_FLASH.txt'])

    def test_source_reference_git_sha_is_content_bound(self):
        self.assertEqual(git_blob(b'test'), '30d74d258442c7c65512eafab474568dd706c430')

if __name__=='__main__':unittest.main()
