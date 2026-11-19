import copy
from datetime import datetime, timezone
import json
from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from mob0_preflight import SCHEMA, REQUIRED, evaluate

NOW = datetime(2026, 11, 15, 2, 0, tzinfo=timezone.utc)


def manifest():
    return {
        'schema': SCHEMA,
        'subject': {'alias': 'lab-board-A'},
        'observations': {gate: {
            'state': 'OBSERVED',
            'source_kind': 'LIVE_READ' if gate in {'physical_device_identity','bootloader_lock_state','partition_topology','current_kernel_build'} else 'FILE_HASHED',
            'subject_alias': 'lab-board-A',
            'observed_at': '2026-11-15T01:00:00Z',
            'evidence_ref': 'sha256:synthetic-not-a-proof',
        } for gate in REQUIRED}
    }


class Mob0IntakeTests(unittest.TestCase):
    def test_all_claims_remain_candidate_never_flash(self):
        result = evaluate(manifest(), NOW)
        self.assertTrue(result['all_required_evidence_candidate'])
        self.assertFalse(result['device_write_authorized'])
        self.assertFalse(result['machine_verified_device_identity'])
        self.assertFalse(result['physical_acceptance'])

    def test_photo_does_not_prove_bootloader(self):
        case=manifest()
        case['observations']['bootloader_lock_state']['source_kind'] = 'PHOTO'
        result=evaluate(case,NOW)
        self.assertEqual(result['evidence_status']['bootloader_lock_state'], 'UNQUALIFIED_SOURCE')

    def test_owner_report_does_not_prove_bootloader(self):
        case=manifest()
        case['observations']['bootloader_lock_state']['source_kind'] = 'OWNER_REPORTED'
        result=evaluate(case,NOW)
        self.assertEqual(result['evidence_status']['bootloader_lock_state'], 'UNQUALIFIED_SOURCE')

    def test_simulated_result_never_physical(self):
        case=manifest()
        case['observations']['physical_device_identity']['source_kind'] = 'SIMULATED'
        result=evaluate(case,NOW)
        self.assertEqual(result['evidence_status']['physical_device_identity'], 'SIMULATED_NOT_PHYSICAL')
        self.assertFalse(result['all_required_evidence_candidate'])

    def test_provenance_and_device_binding(self):
        case=manifest()
        case['observations']['physical_device_identity']['subject_alias'] = 'different'
        case['observations']['boot_image_digest']['evidence_ref'] = ''
        result=evaluate(case,NOW)
        self.assertEqual(result['evidence_status']['physical_device_identity'], 'DEVICE_BINDING_MISMATCH')
        self.assertEqual(result['evidence_status']['boot_image_digest'], 'MISSING_PROVENANCE_REFERENCE')

    def test_stale_future_and_naive_time(self):
        case=manifest()
        case['observations']['bootloader_lock_state']['observed_at'] = '2026-11-01T01:00:00Z'
        case['observations']['partition_topology']['observed_at'] = '2026-11-16T02:00:00Z'
        case['observations']['current_kernel_build']['observed_at'] = '2026-11-15T01:00:00'
        result=evaluate(case,NOW)['evidence_status']
        self.assertEqual(result['bootloader_lock_state'], 'STALE')
        self.assertEqual(result['partition_topology'], 'FUTURE_OBSERVATION')
        self.assertEqual(result['current_kernel_build'], 'INVALID_OBSERVATION_TIME')

    def test_conflicted_is_not_unknown_or_proven(self):
        case=manifest()
        case['observations']['board_device_tree']['state']='CONFLICTED'
        self.assertEqual(evaluate(case,NOW)['evidence_status']['board_device_tree'], 'CONFLICTED')

    def test_offline_artifact_cannot_claim_current_device(self):
        case=manifest()
        case['observations']['physical_device_identity']['source_kind']='FILE_HASHED'
        self.assertEqual(evaluate(case,NOW)['evidence_status']['physical_device_identity'], 'NOT_LIVE_DEVICE_EVIDENCE')

    def test_missing_and_unknown(self):
        case=manifest()
        del case['observations']['partition_topology']
        case['observations']['board_device_tree']['state']='UNKNOWN'
        result=evaluate(case,NOW)['evidence_status']
        self.assertEqual(result['partition_topology'],'MISSING')
        self.assertEqual(result['board_device_tree'],'UNKNOWN')

    def test_unknown_fields_rejected(self):
        case=manifest()
        case['observations']['flash_now']={'state':'OBSERVED'}
        with self.assertRaises(ValueError):
            evaluate(case,NOW)

    def test_sample_preserves_photo_recovery_not_flash(self):
        p=Path(__file__).resolve().parents[1]/'device-ports/samsung/a14m/evidence-candidate.json'
        result=evaluate(json.loads(p.read_text()),NOW)
        self.assertTrue(result['recovery_ui_observation_present'])
        self.assertFalse(result['all_required_evidence_candidate'])
        self.assertFalse(result['device_write_authorized'])


if __name__=='__main__':
    unittest.main()