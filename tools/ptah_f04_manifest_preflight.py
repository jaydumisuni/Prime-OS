"""Non-authorizing Prime-owned F04 Ptah package manifest preflight.

This checks a proposed source-only package *description*, not installed files,
Ptah enrollment, Prime Host authority, or a released/bootable image.
"""
from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

SCHEMA = "prime.ptah-f04-package-preflight.v1"
MAX_MANIFEST_BYTES = 65_536
MAX_ARTIFACTS = 32
MAX_ARTIFACT_BYTES = 1 << 40
EMPTY_SHA256 = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
UUID7 = re.compile(r"[0-9a-f]{8}-[0-9a-f]{4}-7[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}\Z")
SHA256 = re.compile(r"[0-9a-f]{64}\Z")
COMMIT = re.compile(r"[0-9a-f]{40}\Z")
PACKAGE_PATH = re.compile(r"[A-Za-z0-9_.-]+(?:/[A-Za-z0-9_.-]+)*\Z")
# Windows treats these ASCII device basenames as reserved even with extensions.
# F04 source manifests must not encode aliases for a future cross-OS consumer.
WINDOWS_DEVICE_NAMES = frozenset(
    {"CON", "PRN", "AUX", "NUL"}
    | {f"{prefix}{number}" for prefix in ("COM", "LPT") for number in range(1, 10)}
)


def _portable_artifact_path(path: str) -> bool:
    for part in path.split("/"):
        if (part in {".", ".."} or part.endswith(".")
                or part.split(".", 1)[0].upper() in WINDOWS_DEVICE_NAMES):
            return False
    return True


class ManifestError(ValueError):
    """Structured denial; never grants installation or release authority."""


def _exact_keys(value: object, keys: set[str], name: str) -> dict:
    if type(value) is not dict or set(value) != keys:
        raise ManifestError(f"invalid_{name}_fields")
    return value


def _unique_pairs(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise ManifestError("duplicate_json_field")
        result[key] = value
    return result


def _reject_constant(_: str) -> None:
    raise ManifestError("nonfinite_json_number")


def validate_manifest(value: object) -> dict:
    manifest = _exact_keys(value, {
        "schema", "prime_host_id", "ptah_node_id", "source_commit", "artifacts"
    }, "manifest")
    if manifest["schema"] != SCHEMA:
        raise ManifestError("invalid_schema")
    host = manifest["prime_host_id"]
    node = manifest["ptah_node_id"]
    if type(host) is not str or UUID7.fullmatch(host) is None:
        raise ManifestError("invalid_prime_host_id")
    # Ptah's frozen NodeId wraps EntityId: a canonical lowercase UUIDv7.
    # A human-readable label is not a Node identity and cannot bind F04.
    if type(node) is not str or UUID7.fullmatch(node) is None or node == host:
        raise ManifestError("invalid_ptah_node_id")
    commit = manifest["source_commit"]
    if (type(commit) is not str or COMMIT.fullmatch(commit) is None
            or commit == "0" * 40):
        raise ManifestError("invalid_source_commit")
    artifacts = manifest["artifacts"]
    if type(artifacts) is not list or not 0 < len(artifacts) <= MAX_ARTIFACTS:
        raise ManifestError("invalid_artifact_count")
    seen = set()
    for artifact in artifacts:
        item = _exact_keys(artifact, {"path", "sha256", "size_bytes"}, "artifact")
        path = item["path"]
        if (type(path) is not str or not 0 < len(path) <= 256
                or PACKAGE_PATH.fullmatch(path) is None
                or not _portable_artifact_path(path)):
            raise ManifestError("invalid_artifact_path")
        folded = path.casefold()
        if folded in seen:
            raise ManifestError("duplicate_artifact_path")
        seen.add(folded)
        digest = item["sha256"]
        if (type(digest) is not str or SHA256.fullmatch(digest) is None
                or digest in {"0" * 64, EMPTY_SHA256}):
            raise ManifestError("invalid_artifact_digest")
        size = item["size_bytes"]
        if type(size) is not int or not 0 < size <= MAX_ARTIFACT_BYTES:
            raise ManifestError("invalid_artifact_size")
    return {
        "result": "preflight_ok",
        "artifact_count": len(artifacts),
        "authority_granted": False,
        "installation_performed": False,
        "physical_proof_claimed": False,
    }


def load_manifest(raw: bytes) -> dict:
    if type(raw) is not bytes or not 0 < len(raw) <= MAX_MANIFEST_BYTES:
        raise ManifestError("invalid_manifest_size")
    try:
        value = json.loads(raw.decode("utf-8", errors="strict"),
                           object_pairs_hook=_unique_pairs,
                           parse_constant=_reject_constant)
    except (UnicodeError, ValueError, RecursionError) as error:
        raise ManifestError("invalid_manifest_json") from error
    return validate_manifest(value)


def main() -> int:
    parser = argparse.ArgumentParser(description="Prime F04 Ptah package source-only preflight")
    parser.add_argument("--manifest", type=Path, required=True)
    args = parser.parse_args()
    try:
        with args.manifest.open("rb") as stream:
            raw = stream.read(MAX_MANIFEST_BYTES + 1)
        result = load_manifest(raw)
    except (ManifestError, OSError) as error:
        print(json.dumps({"result": "preflight_denied",
                          "reason": str(error)[:120],
                          "authority_granted": False}))
        return 2
    print(json.dumps(result, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
