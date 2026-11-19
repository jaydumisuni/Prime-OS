"""Negative and source-based checks. Mock ADB is NOT physical hardware evidence."""
from __future__ import annotations

import copy
import json
from pathlib import Path
import subprocess
import sys
import unittest

ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'tools'))
from a14m_stock_preflight import (assess,capture_adb,MODEL,PROPS,CAPTURE_SCHEMA)

SOURCES=ROOT/'device-ports/samsung/a14m/source-intake.candidate.json'


def source() -> dict:
    return json.loads(SOURCES.read_text())


def live_capture() -> dict:
    return {
        'schema':CAPTURE_SCHEMA,
        'source_kind':'LIVE_READ_ADB_PROPERTIES_ONLY',
        'properties':{
            'ro.product.model':'SM-A145P',
            'ro.product.device':'a14m',
            'ro.hardware':'mt6768',
            'ro.build.fingerprint':'samsung/a14mnnxx/a14m:15/TEST/A145PXXUEDZJ2:user/release-keys',
            'ro.boot.flash.locked':'0'
        },
        'partition_names':['boot','vbmeta','super']
    }


class BootSourcePreflight(unittest.TestCase):
    def test_sample_metadata_does_not_prove_physical_device(self):
        out=assess(source())
        self.assertEqual(out['status'],'STOP_DEVICE_IMAGE_NOT_READY')
        self.assertEqual(out['model_observation'],'NOT_OBSERVED')
        self.assertIn('LIVE_PHONE_CAPTURE_MISSING',out['blockers'])
        self.assertFalse(out['device_flash_authorized'])
        self.assertFalse(out['stock_restore_verified'])

    def test_matching_properties_still_do_not_qualify_flash(self):
        out=assess(source(),live_capture())
        self.assertEqual(out['model_observation'],'MATCHED_ANDROID_PROPERTY_NOT_HARDWARE_ATTESTED')
        self.assertEqual(out['captured_build_candidate'],'A145PXXUEDZJ2')
        self.assertIn('HISTORICAL_KERNEL_SOURCE_DOES_NOT_MATCH_CAPTURED_FIRMWARE',out['blockers'])
        self.assertFalse(out['boot_image_compatible'])
        self.assertFalse(out['bootloader_unlock_qualified'])
        self.assertFalse(out['device_flash_authorized'])
        self.assertEqual(out['public_latest_referenced_build'],'A145PXXUEDZJ2')

    def test_wrong_model_or_board_is_never_a_supported_variant(self):
        for change in ({'ro.product.model':'SM-A145R'},{'ro.product.device':'a14'}):
            with self.subTest(change=change):
                snap=live_capture();snap['properties'].update(change)
                out=assess(source(),snap)
                self.assertEqual(out['model_observation'],'MISMATCH_OR_UNKNOWN')
                self.assertIn('DEVICE_MODEL_OR_BOARD_MISMATCH',out['blockers'])
                self.assertFalse(out['device_flash_authorized'])

    def test_2023_old_firmware_not_accidentally_classified_as_compatible(self):
        snap=live_capture()
        snap['properties']['ro.build.fingerprint']='samsung/a14m/a14m:13/TEST/A145PXXU2AWG1:user/release-keys'
        out=assess(source(),snap)
        self.assertEqual(out['captured_build_candidate'],'A145PXXU2AWG1')
        self.assertNotIn('HISTORICAL_KERNEL_SOURCE_DOES_NOT_MATCH_CAPTURED_FIRMWARE',out['blockers'])
        self.assertFalse(out['boot_image_compatible'])
        self.assertFalse(out['stock_restore_verified'])

    def test_fingerprint_without_pda_does_not_guess_installed_build(self):
        snap=live_capture();snap['properties']['ro.build.fingerprint']='unknown'
        out=assess(source(),snap)
        self.assertIn('CURRENT_BUILD_NOT_PROVEN',out['blockers'])
        self.assertIsNone(out['captured_build_candidate'])

    def test_source_community_references_must_be_pinned(self):
        s=source();s['public_reference_sources'][1].pop('exact_commit')
        with self.assertRaisesRegex(ValueError,'UNPINNED_COMMUNITY_SOURCE'):
            assess(s)

    def test_capture_unknown_fields_fail_closed(self):
        snap=live_capture();snap['properties']['ro.serialno']='PRIVATE'
        with self.assertRaisesRegex(ValueError,'UNEXPECTED_CAPTURE_FIELDS'):
            assess(source(),snap)

    def test_unknown_source_label_is_rejected(self):
        snap=live_capture();snap['source_kind']='SIMULATED'
        with self.assertRaisesRegex(ValueError,'UNQUALIFIED_CAPTURE'):
            assess(source(),snap)

    def test_excess_or_invalid_partition_names_fail_closed(self):
        snap=live_capture();snap['partition_names']=['boot','../danger']
        with self.assertRaisesRegex(ValueError,'INVALID_PARTITION_NAMES'):
            assess(source(),snap)

    def test_unlocked_getprop_is_not_bootloader_proof(self):
        snap=live_capture();snap['properties']['ro.boot.flash.locked']='0'
        out=assess(source(),snap)
        self.assertFalse(out['bootloader_unlock_qualified'])
        self.assertFalse(out['device_flash_authorized'])

    def test_multi_device_aborts_before_reading_properties(self):
        commands=[]
        def runner(args,**kwargs):
            commands.append(args)
            return subprocess.CompletedProcess(args,0,'List of devices attached\nserial-1\tdevice\nserial-2\tdevice\n','')
        with self.assertRaisesRegex(RuntimeError,'REQUIRE_EXACTLY_ONE_AUTHORIZED_ADB_DEVICE'):
            capture_adb(runner=runner)
        self.assertEqual(commands,[['adb','devices']])

    def test_single_device_reads_allowlisted_properties_without_persisting_serial(self):
        commands=[]
        def runner(args,**kwargs):
            commands.append(args)
            if args==['adb','devices']:
                return subprocess.CompletedProcess(args,0,'List of devices attached\nprivate-serial-123\tdevice\n','')
            if args[-2:]==['getprop','ro.product.model']:
                val=MODEL
            elif args[-2:]==['getprop','ro.product.device']:
                val='a14m'
            elif args[-3:]==['ls','-1','/dev/block/by-name']:
                val='vbmeta\nboot\nsuper\n'
            else:val=''
            return subprocess.CompletedProcess(args,0,val,'')
        captured=capture_adb(runner=runner)
        self.assertEqual(captured['partition_names'],['boot','super','vbmeta'])
        self.assertEqual(captured['properties']['ro.product.model'],'SM-A145P')
        self.assertFalse(captured['device_write_authorized'])
        self.assertFalse(captured['device_serial_recorded'])
        self.assertNotIn('private-serial-123',json.dumps(captured))
        self.assertEqual(len(commands),len(PROPS)+3)
        self.assertTrue(all(cmd[1:3]==['-s','private-serial-123'] for cmd in commands[1:]))
        self.assertTrue(all(cmd[3]=='shell' for cmd in commands[1:]))
        self.assertTrue(all(cmd[4] in {'getprop','uname','ls'} for cmd in commands[1:]))

    def test_offline_and_unauthorized_devices_are_refused(self):
        for state in ['offline','unauthorized']:
            with self.subTest(state=state):
                def runner(args,**kwargs):
                    return subprocess.CompletedProcess(args,0,'List of devices attached\nprivate123\t'+state+'\n','')
                with self.assertRaisesRegex(RuntimeError,'REQUIRE_EXACTLY_ONE_AUTHORIZED_ADB_DEVICE'):
                    capture_adb(runner=runner)

if __name__=='__main__':
    unittest.main()
