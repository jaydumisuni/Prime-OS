"""Host-only, synthetic UART checks for optional read-only PHI ARM64 probes.

All guest strings here are fixtures, never real boot or kernel evidence.
"""
from __future__ import annotations

import sys
from pathlib import Path
import tempfile
import unittest

sys.path.insert(0,str(Path(__file__).resolve().parents[1]))
from verify_serial import evaluate
from build import build

BASE='''Linux boot...
PRIME_MOB1_V0|stage=INIT_ENTERED
PRIME_MOB1_V0|compiled_arch=aarch64
PRIME_MOB1_V0|pid1=true
PRIME_MOB1_V0|kernel_machine=aarch64
PRIME_MOB1_V0|proc_mount=AVAILABLE
PRIME_MOB1_V0|sys_mount=AVAILABLE
PRIME_MOB1_V0|mem_total_kib=1048576
PRIME_MOB1_V0|device_tree_model=linux,dummy-virt
PRIME_MOB1_V0|stage=EVIDENCE_ONLY_CORE_NOT_STARTED
'''
PHI='''PRIME_MOB1_V0|mem_available_kib=524288
PRIME_MOB1_V0|swap_total_kib=131072
PRIME_MOB1_V0|swap_free_kib=65536
PRIME_MOB1_V0|psi_memory_some_avg10=0.00
PRIME_MOB1_V0|zswap_state=DISABLED
'''

def trace(phi=PHI):
    return BASE.replace('PRIME_MOB1_V0|stage=EVIDENCE_ONLY_CORE_NOT_STARTED\n',
                        phi+'PRIME_MOB1_V0|stage=EVIDENCE_ONLY_CORE_NOT_STARTED\n')

class PhiArm64HostObservationTests(unittest.TestCase):
    def test_valid_optional_phi_not_ai_or_computation(self):
        result=evaluate(trace())
        self.assertTrue(result['candidate_init_observed'])
        self.assertEqual(result['phi_memory_readonly']['mem_available_kib'],524288)
        self.assertEqual(result['phi_memory_readonly']['swap_total_kib'],131072)
        self.assertEqual(result['phi_memory_readonly']['swap_free_kib'],65536)
        self.assertEqual(result['phi_memory_readonly']['psi_memory_some_avg10'],0.0)
        self.assertEqual(result['phi_memory_readonly']['zswap_state'],'DISABLED')
        self.assertTrue(result['phi_memory_readonly']['swap_is_not_physical_ram'])
        self.assertTrue(result['phi_memory_readonly']['zswap_enabled_does_not_prove_usage'])
        self.assertFalse(result['prime_core_ready'])
        self.assertFalse(result['flash_authorization'])

    def test_older_fixture_preserves_unknown_not_fake_zero(self):
        r=evaluate(BASE)
        self.assertTrue(r['candidate_init_observed'])
        self.assertIsNone(r['phi_memory_readonly']['swap_total_kib'])
        self.assertIsNone(r['phi_memory_readonly']['zswap_state'])
        self.assertIn('PHI_OBSERVATION_UNKNOWN:zswap_state',r['limitations'])

    def test_available_cannot_exceed_physical(self):
        r=evaluate(trace().replace('mem_available_kib=524288','mem_available_kib=2097152'))
        self.assertFalse(r['candidate_init_observed'])
        self.assertIn('PHI_MEMORY_AVAILABLE_EXCEEDS_TOTAL',r['blockers'])

    def test_swap_free_cannot_exceed_swap_total(self):
        r=evaluate(trace().replace('swap_free_kib=65536','swap_free_kib=262144'))
        self.assertFalse(r['candidate_init_observed'])
        self.assertIn('PHI_SWAP_FREE_EXCEEDS_TOTAL',r['blockers'])

    def test_negative_memory_counter_fails(self):
        r=evaluate(trace().replace('mem_available_kib=524288','mem_available_kib=-1'))
        self.assertIn('PHI_MALFORMED:mem_available_kib',r['blockers'])

    def test_oversized_memory_counter_fails(self):
        r=evaluate(trace().replace('mem_available_kib=524288','mem_available_kib=99999999999999999999'))
        self.assertIn('PHI_COUNTER_OUT_OF_RANGE:mem_available_kib',r['blockers'])

    def test_psi_over_hundred_fails(self):
        r=evaluate(trace().replace('psi_memory_some_avg10=0.00','psi_memory_some_avg10=101.00'))
        self.assertIn('PHI_PSI_MALFORMED',r['blockers'])

    def test_psi_nonfinite_fails(self):
        r=evaluate(trace().replace('psi_memory_some_avg10=0.00','psi_memory_some_avg10=NaN'))
        self.assertIn('PHI_PSI_MALFORMED',r['blockers'])

    def test_psi_missing_is_not_zero(self):
        r=evaluate(trace().replace('psi_memory_some_avg10=0.00','psi_memory_some_avg10=UNKNOWN'))
        self.assertTrue(r['candidate_init_observed'])
        self.assertIsNone(r['phi_memory_readonly']['psi_memory_some_avg10'])
        self.assertIn('PHI_OBSERVATION_UNKNOWN:psi_memory_some_avg10',r['limitations'])

    def test_unexpected_zswap_state_fails(self):
        r=evaluate(trace().replace('zswap_state=DISABLED','zswap_state=MAGIC'))
        self.assertIn('PHI_ZSWAP_INVALID_STATE',r['blockers'])

    def test_zswap_enabled_does_not_mean_memory_savings(self):
        r=evaluate(trace().replace('zswap_state=DISABLED','zswap_state=ENABLED'))
        self.assertTrue(r['candidate_init_observed'])
        self.assertEqual(r['phi_memory_readonly']['zswap_state'],'ENABLED')
        self.assertTrue(r['phi_memory_readonly']['zswap_enabled_does_not_prove_usage'])

    def test_duplicate_phi_field_fails(self):
        r=evaluate(trace(PHI+'PRIME_MOB1_V0|zswap_state=ENABLED\n'))
        self.assertIn('DUPLICATE:zswap_state',r['blockers'])

    def test_physical_phi_marker_cannot_appear_before_init(self):
        injected='PRIME_MOB1_V0|zswap_state=ENABLED\n'+trace()
        r=evaluate(injected)
        self.assertFalse(r['candidate_init_observed'])
        self.assertIn('MARKER_OUTSIDE_INIT_WINDOW',r['blockers'])

    def test_guest_marker_cannot_appear_after_final_stage(self):
        r=evaluate(trace()+'PRIME_MOB1_V0|zswap_state=ENABLED\n')
        self.assertFalse(r['candidate_init_observed'])
        self.assertIn('MARKER_OUTSIDE_INIT_WINDOW',r['blockers'])

    def test_embedded_uart_marker_is_not_proof(self):
        r=evaluate(trace().replace('PRIME_MOB1_V0|pid1=true',
                                   'arbitrary program says PRIME_MOB1_V0|pid1=true'))
        self.assertFalse(r['candidate_init_observed'])
        self.assertIn('EMBEDDED_OR_PREFIXED_MARKER',r['blockers'])

    def test_build_contains_bounded_phi_guest_probe_markers(self):
        with tempfile.TemporaryDirectory() as temp:
            receipt=build(Path(temp))
            data=(Path(temp)/'init-aarch64').read_bytes()
            for marker in (b'mem_available_kib',b'swap_total_kib',b'swap_free_kib',
                           b'|psi_memory_some_avg10=',b'|zswap_state='):
                self.assertIn(marker,data)
            self.assertFalse(receipt['prime_core_compiled'])
            self.assertFalse(receipt['arm64_guest_boot_observed'])

if __name__=='__main__':unittest.main()
