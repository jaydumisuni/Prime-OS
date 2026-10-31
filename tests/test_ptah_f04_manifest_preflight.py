"""Source-only F04 package-preflight regression tests; no device or installation."""
import copy
import json
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
import ptah_f04_manifest_preflight as f04


def candidate():
    return {
        "schema": f04.SCHEMA,
        "prime_host_id": "018f5b43-8a5b-7b12-8a2c-aaaaaaaaaaaa",
        "ptah_node_id": "018f5b43-8a5b-7b12-8a2c-bbbbbbbbbbbb",
        "source_commit": "1" * 40,
        "artifacts": [{
            "path": "bin/ptah-node",
            "sha256": "a" * 64,
            "size_bytes": 1024,
        }],
    }


class ManifestPreflightTests(unittest.TestCase):
    def test_valid_source_only_candidate_is_not_release_authority(self):
        result = f04.load_manifest(json.dumps(candidate()).encode())
        self.assertEqual(result["result"], "preflight_ok")
        self.assertEqual(result["artifact_count"], 1)
        self.assertFalse(result["authority_granted"])
        self.assertFalse(result["installation_performed"])
        self.assertFalse(result["physical_proof_claimed"])

    def test_host_and_ptah_node_identities_must_be_distinct(self):
        value = candidate()
        value["ptah_node_id"] = value["prime_host_id"]
        with self.assertRaisesRegex(f04.ManifestError, "invalid_ptah_node_id"):
            f04.validate_manifest(value)

    def test_host_must_be_canonical_uuidv7(self):
        for host in ("", "018f5b43-8a5b-4b12-8a2c-aaaaaaaaaaaa",
                     "018F5B43-8A5B-7B12-8A2C-AAAAAAAAAAAA"):
            with self.subTest(host=host):
                value = candidate()
                value["prime_host_id"] = host
                with self.assertRaisesRegex(f04.ManifestError, "invalid_prime_host_id"):
                    f04.validate_manifest(value)

    def test_node_id_requires_canonical_ptah_uuidv7(self):
        # The frozen Ptah NodeId is EntityId(UUIDv7), not a display label.
        for node in (
            "", "ptah-node-a", "node-\ud800", " node ",
            "018f5b43-8a5b-4b12-8a2c-bbbbbbbbbbbb",  # UUIDv4
            "018f5b43-8a5b-7b12-2a2c-bbbbbbbbbbbb",  # wrong variant
            "018F5B43-8A5B-7B12-8A2C-BBBBBBBBBBBB",  # noncanonical case
            "018f5b43-8a5b-7b12-8a2c-bbbbbbbbbbbb\n",
        ):
            with self.subTest(node=repr(node)):
                value = candidate()
                value["ptah_node_id"] = node
                with self.assertRaisesRegex(f04.ManifestError, "invalid_ptah_node_id"):
                    f04.validate_manifest(value)

    def test_source_commit_rejects_null_oid_and_malformed_identifiers(self):
        # All-zero is Git's null object ID, never an actual source commit.
        for commit in ("0" * 40, "a" * 39, "a" * 41, "A" * 40,
                       None, True):
            with self.subTest(commit=repr(commit)):
                value = candidate()
                value["source_commit"] = commit
                with self.assertRaisesRegex(f04.ManifestError, "invalid_source_commit"):
                    f04.validate_manifest(value)
                with self.assertRaisesRegex(f04.ManifestError, "invalid_source_commit"):
                    f04.load_manifest(json.dumps(value).encode())

    def test_paths_cannot_escape_or_alias_package_root(self):
        for path in ("../bin/ptah-node", "/bin/ptah-node", "bin//node",
                     "bin/./node", "bin/../node", "bin\\node",
                     "bin/node\n", "bin/ｎode"):
            with self.subTest(path=repr(path)):
                value = candidate()
                value["artifacts"][0]["path"] = path
                with self.assertRaisesRegex(f04.ManifestError, "invalid_artifact_path"):
                    f04.validate_manifest(value)

    def test_duplicate_paths_rejected_even_on_case_insensitive_consumers(self):
        value = candidate()
        second = copy.deepcopy(value["artifacts"][0])
        second["path"] = "BIN/PTAH-NODE"
        value["artifacts"].append(second)
        with self.assertRaisesRegex(f04.ManifestError, "duplicate_artifact_path"):
            f04.validate_manifest(value)

    def test_unbound_digest_sentinels_rejected(self):
        for digest in ("0" * 64, f04.EMPTY_SHA256, "A" * 64, "a" * 63):
            with self.subTest(digest=digest):
                value = candidate()
                value["artifacts"][0]["sha256"] = digest
                with self.assertRaisesRegex(f04.ManifestError, "invalid_artifact_digest"):
                    f04.validate_manifest(value)

    def test_size_must_be_positive_integer_and_bounded(self):
        for size in (0, -1, True, 1.0, 1 << 41):
            with self.subTest(size=size):
                value = candidate()
                value["artifacts"][0]["size_bytes"] = size
                with self.assertRaisesRegex(f04.ManifestError, "invalid_artifact_size"):
                    f04.validate_manifest(value)

    def test_duplicate_json_fields_are_not_silently_normalized(self):
        raw = json.dumps(candidate()).replace('"schema":', '"schema": "forged", "schema":', 1)
        with self.assertRaises(f04.ManifestError):
            f04.load_manifest(raw.encode())

    def test_unbounded_or_nonfinite_manifest_is_denied(self):
        with self.assertRaisesRegex(f04.ManifestError, "invalid_manifest_size"):
            f04.load_manifest(b" " * (f04.MAX_MANIFEST_BYTES + 1))
        with self.assertRaises(f04.ManifestError):
            f04.load_manifest(b'{"schema":NaN}')

    def test_unknown_fields_and_empty_artifacts_denied(self):
        value = candidate()
        value["authorized"] = True
        with self.assertRaisesRegex(f04.ManifestError, "invalid_manifest_fields"):
            f04.validate_manifest(value)
        value = candidate()
        value["artifacts"] = []
        with self.assertRaisesRegex(f04.ManifestError, "invalid_artifact_count"):
            f04.validate_manifest(value)


if __name__ == "__main__":
    unittest.main()
