# Prime Windows Personality W3 Implementation Plan

**Baseline:** `4296dd694ec3ab91ff27b580d0887fbaf969d944`

## Task 1 — CLR inspection
- Add bounded PE/CLI metadata inspector in `primed::exec`.
- RED/GREEN tests for native PE, valid managed PE, malformed COM descriptor/RVA/metadata signature.
- Validate against real W3 managed fixture.

## Task 2 — Managed dependency admission
- Add exact trusted Wine Mono component-reference constant/validator.
- Inspect staged artifact in `prepare_windows_launch` and carry internal managed state in `PreparedWindowsLaunch`.
- Managed PE without exact runtime dependency fails closed; unmanaged W1/W2 paths remain unchanged.

## Task 3 — Managed bridge contract
- Add `windows_managed` module with immutable source paths, source hashes, target paths, and W3 bridge fingerprint.
- Add safe regular-file/hash validation and atomic target projection helpers.
- Marker never overrides observed target state.

## Task 4 — Provider integration
- Under existing compatibility lock, after prefix init, prepare managed bridge only for managed artifacts.
- Keep direct Wine argv and existing W1 provider ABI unchanged.
- Preserve W1 error semantics and add explicit W3 bridge failures.

## Task 5 — Image closure
- Pin exact Fedora bridge-source packages/transitive package versions in `image/Containerfile`.
- Install/copy immutable bridge sources to Prime-owned runtime paths and validate SHA-256 at image build.
- Add W3 package/security checker and release-proof requirements.

## Task 6 — Real x64 managed console proof
- Build deterministic C# fixture using proof-only Wine Mono `csc.exe`.
- Run through actual Prime provider from fresh state with exact runtime dependency prepared.
- Require `PRIME_W3_MANAGED_EXECUTED` and exit 0; repeat warm launch.

## Task 7 — Real x86 managed console proof
- Build `/platform:x86` fixture.
- Run through same provider and prove x86 bridge files/hashes and exact output/exit.

## Task 8 — Managed GUI + lifecycle
- Build WinForms fixture.
- Launch through Prime provider with Wayland proof compositor and capture/visually inspect real window.
- Run cold/warm/repeat stress and isolation/recovery checks.

## Task 9 — Adversarial controls
- Dedicated sweep for malformed CLR, missing/wrong component pin, bridge source/target mismatch, symlink target, unsupported arch, unavailable provider.
- Re-run W1/W2 runtime/security checkers.

## Task 10 — Freeze/certify/ship
- Full workspace tests + clippy + format + diff integrity.
- Current Sergeant review across W3 delta.
- Freeze exact SHA and rerun decisive x64/x86/GUI/runtime proofs from frozen release binaries.
- Formula PASS manifest.
- Push branch and verify remote SHA.
- Update roadmap: W3 host implementation/proof complete; full boot integration deferred.
