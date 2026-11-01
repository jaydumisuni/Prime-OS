# Prime F04 → Ptah package manifest preflight (proposal v1)

**Status: DRAFT / NON-AUTHORIZING.** This is a bounded, source-only preparatory contract for the OS-owned F04–F07 integration frontier. It is not a frozen Prime P1 contract, Ptah release, service installation, image build, promotion, or physical qualification.

## Authority and dependency

- Prime owns bootable image, package delivery, update/rollback and Host-local enforcement. Ptah owns its Workspace/Activity/Node/Provider/Facility semantics. The adapter must not move either authority.
- The controlling Ptah roadmap (`jaydumisuni/ptah-roadmap-`, `IMPLEMENTATION_ROADMAP.md`, 2026-10-22 correction) declares E06 and standalone E07 complete, F01–F03 merged, and F04–F07 **Prime OS-owned**. This preflight advances that internal dependency without reopening completed Ptah milestones.
- `Prime Host ID != Ptah Node ID`. A manifest never creates or rebinds either identity, and source metadata cannot authorize enrollment, execution, install, update, or rollback.
- Existing Prime `prime.host-identity.v1` and `prime.capability.v1` remain unchanged. This proposal is separate and subject to independent review.

## Candidate input

JSON object with exactly `schema`, `prime_host_id`, `ptah_node_id`, `source_commit`, and `artifacts`.

```json
{
  "schema": "prime.ptah-f04-package-preflight.v1",
  "prime_host_id": "018f5b43-8a5b-7b12-8a2c-aaaaaaaaaaaa",
  "ptah_node_id": "018f5b43-8a5b-7b12-8a2c-bbbbbbbbbbbb",
  "source_commit": "1111111111111111111111111111111111111111",
  "artifacts": [
    {"path": "bin/ptah-node", "sha256": "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa", "size_bytes": 1024}
  ]
}
```

The source commit and artifact hashes above are **illustrative placeholders**, not qualified Ptah release evidence. The preflight validates syntax and rejects empty/zero digests, but does not verify Git provenance or file bytes.

## Source-only denial and output

- At most 65,536 manifest bytes and 32 nonempty artifacts; no duplicate JSON keys, extra fields, nonfinite numbers or invalid UTF-8.
- Canonical lowercase UUIDv7 Prime Host ID, separate canonical lowercase UUIDv7 Ptah Node ID (`ptah-identifiers::NodeId` wraps `EntityId`; not a display label), and nonzero 40-digit lowercase Git source commit (the all-zero Git null object ID is denied; object existence/type remains unverified).
- Relative ASCII package paths only; reject absolute paths, traversal, dot segments, backslashes, control characters, case-insensitive duplicate paths, Windows device basenames (`CON`, `PRN`, `AUX`, `NUL`, `COM1`–`COM9`, `LPT1`–`LPT9`, including extensions), and trailing-dot path components. This is conservative cross-OS pathname custody, not proof that any installer can safely extract the manifest.
- Each artifact declares a positive bounded integer size and lowercase SHA-256 other than SHA-256(empty) or all-zero placeholder.
- Success emits `preflight_ok`, artifact count, and explicit `authority_granted=false`, `installation_performed=false`, `physical_proof_claimed=false`. Failure emits `preflight_denied` with no side effects.

Implementation: `tools/ptah_f04_manifest_preflight.py`; regression: `tests/test_ptah_f04_manifest_preflight.py`. No live node, credential, network, package or runtime is touched.

## Unearned gates

Independent schema acceptance; actual Ptah release artifact identities and byte hashes; immutable package custody; Prime image/installer integration; boot/rollback/recovery proof; local Host authorization; real service lifecycle and OS-ready F07 acceptance. All remain **PENDING**. This draft must not be consumed as a release manifest or production installer.
