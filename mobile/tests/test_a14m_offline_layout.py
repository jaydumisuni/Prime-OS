"""Source-only expectations. Synthetic fixtures never validate physical boot."""
from pathlib import Path
import sys
import unittest
sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from a14m_offline_layout import audit, partition_names

BOARD='''BOARD_BOOTIMG_HEADER_VERSION := 2
BOARD_KERNEL_BASE := 0x40078000
BOARD_KERNEL_PAGESIZE := 2048
BOARD_RAMDISK_OFFSET := 0x07c08000
BOARD_KERNEL_TAGS_OFFSET := 0x0bc08000
BOARD_BOOTIMAGE_PARTITION_SIZE := 80740352
BOARD_RECOVERYIMAGE_PARTITION_SIZE := 80740352
TARGET_FORCE_PREBUILT_KERNEL := true
BOARD_AVB_MAKE_VBMETA_IMAGE_ARGS += --flags 3
PLATFORM_SECURITY_PATCH := 2099-12-31
BOARD_SUPER_PARTITION_SIZE := 9126805504 # TODO
ALLOW_MISSING_DEPENDENCIES := true
'''
BOOT='''kernel=kernel
page_size=2048
kernel_size=11564226
dtb_size=135300
base_addr=0x40078000
ramdisk_offset=0x07c08000
tags_offset=0x0bc08000
'''
RECOVERY=BOOT.replace('11564226','11532508')
PIT='Header Magic: 0x12349876\n'+'\n'.join('Partition Name:             '+n for n in ('boot','recovery','vendor_boot','init_boot','dtbo','vbmeta','super'))
SOURCES={'recovery_blobs': {'kernel':'abc','dtb.img':'def','dtbo.img':'ghi'},'dump_recovery_blobs': {'kernel':'abc','dtb.img':'def','dtbo.img':'ghi'}}

class OfflineLayoutTests(unittest.TestCase):
    def result(self,b=BOARD,boot=BOOT,rec=RECOVERY,pit=PIT,sources=SOURCES):
        return audit(b,boot,rec,pit,sources)
    def test_matching_historical_reference_is_not_device_qualified(self):
        r=self.result()
        self.assertEqual(r['status'],'REFERENCE_LAYOUT_RECONCILED_NOT_DEVICE_QUALIFIED')
        self.assertEqual(r['pit_partition_count'],7)
        self.assertTrue(r['recovery_prebuilt_blobs_match_dump'])
        self.assertFalse(r['operating_system_required_for_this_audit'])
        for f in ('boot_image_ready','boot_chain_qualified','stock_restore_qualified','device_flash_authorized','physical_boot_observed'):
            self.assertFalse(r[f])
    def test_missing_vendor_boot_is_hard_stop(self):
        r=self.result(pit=PIT.replace('Partition Name:             vendor_boot','Other Name: vendor_boot'))
        self.assertIn('PIT_PARTITION_ABSENT:vendor_boot',r['blockers'])
        self.assertEqual(r['status'],'STOP_REFERENCE_LAYOUT_CONFLICT')
    def test_page_size_conflict_is_stop(self):
        r=self.result(boot=BOOT.replace('page_size=2048','page_size=4096'))
        self.assertIn('BOOT_PAGE_SIZE_DISAGREEMENT',r['blockers'])
    def test_base_offset_conflict_is_stop(self):
        r=self.result(rec=RECOVERY.replace('ramdisk_offset=0x07c08000','ramdisk_offset=0x07c09000'))
        self.assertIn('RECOVERY_BOARD_RAMDISK_OFFSET_DISAGREEMENT',r['blockers'])
    def test_recovery_blob_conflict_is_stop(self):
        s={'recovery_blobs':dict(SOURCES['recovery_blobs']),'dump_recovery_blobs':dict(SOURCES['dump_recovery_blobs'])}
        s['recovery_blobs']['kernel']='changed'
        r=self.result(sources=s)
        self.assertIn('RECOVERY_SOURCE_BLOB_MISMATCH_OR_UNKNOWN:kernel',r['blockers'])
    def test_recovery_hazards_never_inherited_as_image_authority(self):
        h=self.result()['reference_hazards']
        self.assertIn('RECOVERY_TREE_DISABLES_AVB_FLAGS_DO_NOT_REUSE',h)
        self.assertIn('RECOVERY_TREE_SPOOFED_SECURITY_PATCH_DO_NOT_REUSE',h)
        self.assertIn('RECOVERY_TREE_SUPER_SIZE_HARDCODED_TODO_NOT_AUTHORITY',h)
    def test_duplicate_pit_names_fail_closed(self):
        with self.assertRaises(ValueError):partition_names(PIT+'\nPartition Name: boot\n')
    def test_bad_header_is_rejected(self):
        r=self.result(b=BOARD.replace('BOARD_BOOTIMG_HEADER_VERSION := 2','BOARD_BOOTIMG_HEADER_VERSION := 4'))
        self.assertIn('TWRP_REFERENCE_BOOT_HEADER_NOT_V2',r['blockers'])
    def test_source_not_required_for_os_or_adb(self):
        r=self.result()
        self.assertFalse(r['device_flash_authorized'])
        self.assertIn('vendor_boot',r['pit_selected_partition_names'])
        self.assertIn('init_boot',r['pit_selected_partition_names'])

if __name__=='__main__':unittest.main()
