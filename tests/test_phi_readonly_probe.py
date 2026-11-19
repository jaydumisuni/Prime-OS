import sys
import tempfile
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'tools'))
from phi_readonly_probe import snapshot, _ki_bytes, _pressure, _swap_entries


class MemoryTierProbeTests(unittest.TestCase):
    def write(self, root, path, content):
        dest = root / path.lstrip('/')
        dest.parent.mkdir(parents=True, exist_ok=True)
        dest.write_text(content, encoding='utf-8')

    def test_missing_sources_are_unknown_not_zero(self):
        with tempfile.TemporaryDirectory() as temp:
            out = snapshot(Path(temp), '2026-11-15T01:00:00+00:00')
        self.assertIsNone(out['memory']['physical_total_bytes'])
        self.assertEqual(out['zswap']['state'], 'UNKNOWN')
        self.assertIn('PHYSICAL_AND_SWAP_UNAVAILABLE', out['limitations'])
        self.assertTrue(out['facts']['swap_is_not_physical_ram'])

    def test_realistic_source_counts_and_no_swap_path(self):
        with tempfile.TemporaryDirectory() as t:
            root = Path(t)
            self.write(root, '/proc/meminfo', 'MemTotal: 8192000 kB\nMemAvailable: 4096000 kB\nSwapTotal: 2097152 kB\nSwapFree: 1048576 kB\n')
            self.write(root, '/proc/swaps', 'Filename Type Size Used Priority\n/private/user/swap file 2097152 1048576 -2\n')
            self.write(root, '/proc/pressure/memory', 'some avg10=0.50 avg60=0.20 avg300=0.10 total=6000\nfull avg10=0.00 avg60=0.00 avg300=0.00 total=100\n')
            self.write(root, '/sys/module/zswap/parameters/enabled', 'Y\n')
            self.write(root, '/sys/block/zram0/mm_stat', '1200000 300000 450000 0 0 0\n')
            out = snapshot(root, '2026-11-15T01:00:00+00:00')
        self.assertEqual(out['memory']['physical_total_bytes'], 8192000 * 1024)
        self.assertEqual(out['memory']['swap_total_bytes'], 2097152 * 1024)
        self.assertEqual(out['memory_pressure']['some']['avg10'], 0.5)
        self.assertEqual(out['zram'][0]['memory_used_bytes'], 450000)
        self.assertEqual(out['zswap']['state'], 'ENABLED')
        self.assertEqual(len(out['swap_devices']), 1)
        self.assertNotIn('/private/user/swap', str(out))
        self.assertEqual(out['limitations'], [])

    def test_inconsistent_counters_degrade(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            self.write(root,'/proc/meminfo','MemTotal: 100 kB\nMemAvailable: 200 kB\nSwapTotal: 100 kB\nSwapFree: 200 kB\n')
            self.write(root,'/proc/swaps','Filename Type Size Used Priority\n')
            out=snapshot(root)
        self.assertIn('MEMORY_COUNTER_INCONSISTENT',out['limitations'])
        self.assertIn('SWAP_COUNTER_INCONSISTENT',out['limitations'])

    def test_integer_overflow_and_bad_pressure_are_unknown(self):
        self.assertIsNone(_ki_bytes(str(2**64) + ' kB'))
        self.assertEqual(_pressure('\nnot-pressure\nsome avg10=nan avg60=1 avg300=1 total=1\n')['some'], None)
        self.assertEqual(_swap_entries('incorrect header')[1],False)

    def test_outside_symlink_not_read(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            (root/'proc').mkdir()
            (root/'proc/meminfo').symlink_to('/proc/meminfo')
            out=snapshot(root)
        self.assertIsNone(out['memory']['physical_total_bytes'])

    def test_zram_missing_mm_stat_does_not_claim_memory(self):
        with tempfile.TemporaryDirectory() as t:
            root=Path(t)
            (root/'sys/block/zram0').mkdir(parents=True)
            out=snapshot(root)
        self.assertIsNone(out['zram'][0]['original_data_bytes'])
        self.assertIn('ZRAM_MM_STAT_UNAVAILABLE:zram0',out['limitations'])


if __name__ == '__main__':
    unittest.main()