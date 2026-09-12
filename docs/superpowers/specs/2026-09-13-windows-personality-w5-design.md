# Prime Windows Personality W5 — COM Design

## Scope

W5 proves COM as a first-class capability of the existing Prime Wine personality without adding a parallel Windows runtime. W4 at `f99c7f8245e3324ed699ad388a55aa40f71c7dc7` is the frozen predecessor. W5 keeps application-scoped Wine prefixes, donor fingerprinting, launch routing, and fail-closed state rules unchanged.

## Decision

Use Wine 11.0 COM as the donor and prove it mechanically. Do not reimplement COM in Prime. Prime adds only the minimum capability contract and proof surface needed to ensure the sealed image owns the required donor closure and that COM state remains application-local.

## Required donor surface

Both x86_64 and x86 lanes must expose the Wine COM/OLE stack required by real applications: `ole32.dll`, `oleaut32.dll`, `combase.dll`, `rpcrt4.dll`, `rpcss.exe`, `regsvr32.exe`, `wscript.exe`, `cscript.exe`, and `scrrun.dll`. The donor remains bound to the W4 Wine fingerprint unless package content changes.

## Host-provable benchmark gates

1. Preserve exact W4 predecessor authority and isolation.
2. Prove sealed-image COM donor closure for x64 and x86.
3. Prove apartment initialization plus ProgID/CLSID resolution and IDispatch automation.
4. Prove x64 in-process COM activation through the real Prime Wine runtime.
5. Prove x86/WoW64 in-process COM activation through the same application-prefix model.
6. Prove application-local registration/state isolation and deterministic corruption recovery.
7. Prove x64 local-server COM activation/RPCSS lifecycle.
8. Prove x86 local-server COM activation/RPCSS lifecycle.
9. Run lifecycle/adversarial regression: cold/warm launches, cross-application isolation, stale registration, killed server/RPC recovery, and zero leaked Wine processes after drain.
10. Freeze only after source-clean, diff, format, clippy, workspace regressions, W1-W5 provider checks, current Sergeant approval, exact frozen-SHA runtime replay, Formula manifest, push, and remote SHA binding.

## Proof fixtures

A built-in automation fixture may use `Scripting.Dictionary` because it exercises COM activation, registry class resolution, `IDispatch`, `VARIANT`/OLE automation, and method/property dispatch without introducing an external runtime. Custom in-proc and local-server fixtures may be compiled outside the product image and must be hash-bound before runtime proof.

## Failure policy

Missing donor binaries, unresolved class registration, architecture mismatch, registry escape, cross-application state leakage, RPCSS lifecycle failure, or stale/corrupt registration fails the corresponding gate. No fallback may silently promote the milestone.

## Deferred

Full sealed-image Prime physical boot integration remains deferred. W6 services, W7 USB/device integration, W8 VM fallback, and W9 TTG workload certification remain separate milestones.
