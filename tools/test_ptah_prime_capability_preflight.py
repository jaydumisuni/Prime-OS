"""Offline, non-authorizing Prime -> Ptah F04-F07 interface preparation."""
import unittest
from datetime import datetime, timedelta, timezone

from ptah_prime_capability_preflight import SnapshotRejected, preflight_snapshot


NOW = datetime(2026, 10, 25, 19, 50, tzinfo=timezone.utc)
HOST = "018f6b35-0000-7000-8000-123456789abc"
GEN = "prime-generation-p1"
CAP = "prime.exec.native"


def fixtures():
    return (
        {"interface": "prime.capability.v1", "interface_version": "1.0",
         "host": {"host_id": HOST, "generation_id": GEN, "host_arch": "x86_64"}},
        {"capability_id": CAP, "provider": {"id": "prime", "generation_id": GEN},
         "availability": "AVAILABLE", "placement": {"scope": "HOST_LOCAL", "host_id": HOST},
         "health": {"status": "HEALTHY", "observed_at": "2026-10-25T19:49:00Z"}},
    )


def check(projection, capability, **overrides):
    arguments = dict(expected_host_id=HOST, expected_generation_id=GEN,
                     requested_capability_id=CAP, now=NOW)
    arguments.update(overrides)
    return preflight_snapshot(projection, capability, **arguments)


class PrimePtahPreparationTests(unittest.TestCase):
    def test_fresh_consistent_snapshot_is_diagnostic_only(self):
        result = check(*fixtures())
        self.assertTrue(result["candidate_compatible"])
        self.assertEqual(result["evidence_scope"], "OFFLINE_PRIME_SNAPSHOT_ONLY")
        for key in ("ptah_node_enrolled", "execution_authorized", "live_verified"):
            self.assertFalse(result[key])
        self.assertNotIn("ptah_node_id", result)

    def test_host_identity_requires_canonical_uuidv7(self):
        # A matching forged opaque string must not become a Prime Host ID.
        invalid = (
            "not-a-uuid",
            "018f6b35-0000-4000-8000-123456789abc",  # UUIDv4
            "018f6b35-0000-7000-0000-123456789abc",  # invalid UUID variant
            HOST.upper(),  # noncanonical textual identity
            "{" + HOST + "}",  # accepted by UUID parser, not canonical
        )
        for forged in invalid:
            with self.subTest(host_id=forged):
                p, c = fixtures()
                p["host"]["host_id"] = forged
                c["placement"]["host_id"] = forged
                with self.assertRaisesRegex(SnapshotRejected, "canonical UUIDv7"):
                    check(p, c, expected_host_id=forged)
        self.assertTrue(check(*fixtures())["candidate_compatible"])

    def test_frozen_interface_mismatch_rejected(self):
        p, c = fixtures()
        p["interface_version"] = "2.0"
        with self.assertRaisesRegex(SnapshotRejected, "interface"):
            check(p, c)

    def test_host_identity_mismatch_rejected(self):
        for field in ("host", "placement"):
            with self.subTest(field=field):
                p, c = fixtures()
                (p["host"] if field == "host" else c["placement"])["host_id"] = "other-host"
                with self.assertRaisesRegex(SnapshotRejected, "Host identity"):
                    check(p, c)

    def test_generation_identity_mismatch_rejected(self):
        for field in ("host", "provider"):
            with self.subTest(field=field):
                p, c = fixtures()
                (p["host"] if field == "host" else c["provider"])["generation_id"] = "other-gen"
                with self.assertRaisesRegex(SnapshotRejected, "generation identity"):
                    check(p, c)

    def test_nonprime_or_remote_placement_rejected(self):
        for field, value in (("provider", "other"), ("placement", "REMOTE")):
            with self.subTest(field=field):
                p, c = fixtures()
                if field == "provider":
                    c[field]["id"] = value
                else:
                    c[field]["scope"] = value
                with self.assertRaisesRegex(SnapshotRejected, "nonlocal"):
                    check(p, c)

    def test_unhealthy_or_unavailable_rejected(self):
        for field, value in (("availability", "DEGRADED"), ("health", "UNKNOWN")):
            with self.subTest(field=field):
                p, c = fixtures()
                if field == "health":
                    c[field]["status"] = value
                else:
                    c[field] = value
                with self.assertRaisesRegex(SnapshotRejected, "not observed healthy"):
                    check(p, c)

    def test_stale_future_and_naive_observations_rejected(self):
        for value in ("2026-10-25T19:40:00Z", "2026-10-25T19:52:00Z", "2026-10-25T19:49:00"):
            with self.subTest(value=value):
                p, c = fixtures()
                c["health"]["observed_at"] = value
                with self.assertRaises(SnapshotRejected):
                    check(p, c)

    def test_invalid_shapes_and_ambiguous_ids_rejected(self):
        for p, c, overrides in (
            ([], {}, {}),
            ({"interface": "prime.capability.v1", "interface_version": "1.0", "host": []}, {}, {}),
            (*fixtures(), {"expected_host_id": "  "}),
            (*fixtures(), {"requested_capability_id": "prime.exec.native\n"}),
        ):
            with self.subTest(projection=p):
                with self.assertRaises(SnapshotRejected):
                    check(p, c, **overrides)

    def test_capability_mismatch_rejected(self):
        p, c = fixtures()
        c["capability_id"] = "prime.exec.foreign"
        with self.assertRaisesRegex(SnapshotRejected, "capability identity"):
            check(p, c)

    def test_any_future_observation_is_not_current_evidence(self):
        # An offline snapshot has no independently established clock-skew authority.
        # Even subsecond future observations cannot certify current health.
        for delta in (timedelta(microseconds=1), timedelta(seconds=59),
                      timedelta(minutes=1)):
            with self.subTest(delta=delta):
                p, c = fixtures()
                c["health"]["observed_at"] = (NOW + delta).isoformat()
                with self.assertRaisesRegex(SnapshotRejected, "future"):
                    check(p, c)
        p, c = fixtures()
        c["health"]["observed_at"] = NOW.isoformat()
        self.assertTrue(check(p, c)["candidate_compatible"])

    def test_exact_five_minute_freshness_boundary(self):
        p, c = fixtures()
        c["health"]["observed_at"] = "2026-10-25T19:45:00Z"
        self.assertTrue(check(p, c)["candidate_compatible"])
        c["health"]["observed_at"] = "2026-10-25T19:44:59Z"
        with self.assertRaisesRegex(SnapshotRejected, "stale"):
            check(p, c)


if __name__ == "__main__":
    unittest.main()
