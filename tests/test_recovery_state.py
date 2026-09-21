import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]

class RecoveryStateContract(unittest.TestCase):
    def test_integrated_p1_and_windows_frontier_are_consistent(self):
        state = json.loads((ROOT / 'planning/state.json').read_text(encoding='utf-8'))
        self.assertEqual(state['implementation_state'], 'p1_integrated_main_p4a_windows_branch_lanes')
        p1 = state['implementation_frontiers']['P1']
        self.assertEqual(p1['integrated_candidate_head'], '086ee97f4bef57f78b725af9b220e98faf9f87a1')
        self.assertEqual(p1['integrated_local_uefi_proof'], 'pass')
        self.assertEqual(p1['integrated_known_good'], 'not_earned')
        self.assertEqual(p1['integrated_physical_acceptance'], 'pending')
        windows = state['implementation_frontiers']['P4A_Windows']
        for wave in ('W1','W2','W3','W4','W5','W6','W7'):
            self.assertIn('frozen', windows[wave]['state'])
        self.assertEqual(windows['W8']['state'], 'healthy_waiting_authorized_guest')
        self.assertEqual(windows['W8']['blocker'], 'authorized_digest_bound_windows_guest_execution')
        self.assertEqual(windows['W8']['control_rebind'], 'complete')

    def test_recovery_docs_do_not_overclaim_p1(self):
        readme = (ROOT / 'README.md').read_text(encoding='utf-8')
        status = (ROOT / 'docs/PRIME_CURRENT_STATUS.md').read_text(encoding='utf-8')
        self.assertIn('integrated P1 source/proof lineage is promoted to `main`', readme)
        self.assertIn('`main` now carries the integrated P1 source/proof/recovery lineage', status)
        self.assertIn('physical KRATOS boot acceptance', status)
        self.assertIn('remain unearned', status)
        self.assertIn('HEALTHY_WAITING / authorized_guest', status)

if __name__ == '__main__':
    unittest.main()
