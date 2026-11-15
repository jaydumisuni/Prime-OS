from pathlib import Path
import sys
import json
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]/'tools'))
from port_capability_audit import SCHEMA, audit, FEATURES, REQUIRED_PHONE

class PortCapabilityTests(unittest.TestCase):
    def fixture(self):
        return {'schema':SCHEMA,'board':{'alias':'test-arm64'},'kernel':{'strategy':'VENDOR'},'capabilities':{}}

    def test_missing_is_unknown_no_false_support(self):
        r=audit(self.fixture())
        self.assertEqual(len(r['features']),len(FEATURES))
        self.assertTrue(r['phone_essentials_missing_review'])
        self.assertFalse(r['physical_acceptance'])
        self.assertFalse(r['production_release_authorized'])

    def test_driver_detected_not_functional(self):
        f=self.fixture();f['capabilities']['gpu_compute']={'state':'DRIVER_PRESENT'}
        self.assertEqual(audit(f)['features']['gpu_compute'],'DRIVER_PRESENT')
        self.assertFalse(audit(f)['physical_acceptance'])

    def test_fake_evidence_not_qualified(self):
        f=self.fixture();f['capabilities']['boot']={'state':'PHYSICALLY_VERIFIED'}
        self.assertEqual(audit(f)['features']['boot'],'UNSUBSTANTIATED_CLAIM')

    def test_mismatched_board_binding(self):
        f=self.fixture();f['capabilities']['touch']={'state':'FUNCTIONAL_OBSERVED','board_alias':'someone-else','evidence_ref':'sha256:fake'}
        self.assertEqual(audit(f)['features']['touch'],'BOARD_BINDING_MISMATCH')

    def test_all_claimed_verified_remains_only_review_candidate(self):
        f=self.fixture()
        f['capabilities']={name:{'state':'PHYSICALLY_VERIFIED','board_alias':'test-arm64','evidence_ref':'sha256:illustrative-only'} for name in REQUIRED_PHONE}
        out=audit(f)
        self.assertTrue(out['claimed_phone_essentials_complete'])
        self.assertFalse(out['physical_acceptance'])
        self.assertFalse(out['production_release_authorized'])

    def test_invalid_capabilities_or_strategy_rejected(self):
        f=self.fixture();f['capabilities']['magic-unlock']={'state':'PHYSICALLY_VERIFIED'}
        with self.assertRaises(ValueError):audit(f)
        f=self.fixture();f['kernel']['strategy']='GUESS'
        with self.assertRaises(ValueError):audit(f)

    def test_candidate_a14m_has_no_supported_device_assumption(self):
        p=Path(__file__).resolve().parents[1]/'device-ports/samsung/a14m/port-candidate.json'
        result=audit(json.loads(p.read_text(encoding='utf8')))
        self.assertEqual(result['kernel_strategy_proposed'],'UNKNOWN')
        self.assertFalse(result['claimed_phone_essentials_complete'])
        self.assertFalse(result['production_release_authorized'])

if __name__=='__main__':unittest.main()