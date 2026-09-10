# Prime Windows Personality W3 — Managed .NET Applications

**Date:** 2026-09-11
**Roadmap:** P4A / W3
**Authority baseline:** W2 frozen at `4296dd694ec3ab91ff27b580d0887fbaf969d944`

## Goal

Make managed Windows PE workloads first-class Prime Windows Personality workloads without adding a separate user-visible Mono backend and without duplicating W2 component lifecycle machinery.

W3 means Prime can mechanically recognize a CLR-managed PE, require the trusted W2 managed runtime dependency, prepare the small architecture-specific bridge state needed by Fedora Wine Mono, and execute real managed console and GUI workloads through the already-proven Prime Windows provider.

## Non-goals

W3 does not claim DirectX/GPU acceleration (W4), COM completeness (W5), Windows services (W6), USB/device integration (W7), VM fallback (W8), or TTG workload certification (W9). Full sealed-image/Prime boot integration remains deferred under the current KRATOS host-proof envelope.

## Evidence recovered before design

The pinned Fedora Wine Mono donor installs CLR v2/v4 framework trees, `csc.exe`, `msbuild.exe`, `mscoree.dll`, and `mscorlib.dll`. Actual managed execution exposed two packaging-layout gaps:

- x86_64 Mono bridge imports `iconv.dll`; Fedora owns it in `mingw64-win-iconv-0.0.10-4.fc44`, but it is outside Wine's Windows DLL search path. Projecting exact SHA-256 `74e23093e962adceccf77d2b05af4f4c7d6deee4a524140a296609f849700fc2` to `C:\\windows\\system32\\iconv.dll` made real x64 managed compilation/execution succeed.
- x86 Mono bridge imports `libgcc_s_dw2-1.dll`; that DLL imports `libwinpthread-1.dll`. Fedora owns them in `mingw32-libgcc-16.1.1-1.fc44` and `mingw32-winpthreads-13.0.0-3.fc44`. Projecting SHA-256 `09caac62f690912492418d5ca86c3340f5a76e5de07e5d6b7fe057e032b779d1` and `1a8f18e693e49581c2d11d02812cadbaffa1bf6184353fa69ef35ac2950b9d9e` to `C:\\windows\\syswow64` made a forced-x86 managed executable succeed.

Both architecture probes emitted `PRIME_W3_MANAGED_EXECUTED` and exited zero.

## Architecture

### 1. Managed PE inspection

Prime adds bounded internal CLR inspection; it does not change serialized `ApplicationArtifact` or profile digest shape.

Inspection must parse the PE optional-header COM descriptor (data-directory index 14), map RVAs through the PE section table, validate the CLI header, map its metadata RVA, and require the CLR metadata signature `BSJB`. A zero COM descriptor means native/unmanaged PE. A non-zero but malformed CLR structure fails closed.

Managed inspection runs against the already-staged artifact, after normal identity/profile verification.

### 2. Runtime dependency admission

A managed PE is launchable only when its selected Application Profile includes the exact trusted W2 Wine Mono component reference:

`runtime.wine-mono@1#sha256:47526839b2fc8c981330d77d2c3b6a4b2528b5cff2ddd547f33aef0671782689`

W2 remains responsible for resolving/preparing/verifying that component before W1/W3 provider launch. Native Win32 profiles remain unchanged.

### 3. Managed bridge state

W3 owns a small per-application bridge state needed to make Fedora's Wine Mono layout consumable by Wine. It is prepared only for managed PE workloads, under the same application compatibility lock used by W1/W2.

The product image carries immutable source DLLs at Prime-owned paths. W3 verifies each source is a regular non-symlink file with its pinned SHA before use, writes target files atomically, enforces exact target SHA after projection, and rejects symlink/non-regular targets.

x64 target:
- `wine-prefix/drive_c/windows/system32/iconv.dll`

x86 targets:
- `wine-prefix/drive_c/windows/syswow64/libgcc_s_dw2-1.dll`
- `wine-prefix/drive_c/windows/syswow64/libwinpthread-1.dll`

A W3 bridge marker is only a cache hint; real file verification always wins. Corruption must be repaired on the next managed launch.

### 4. Provider path

The existing Prime Windows provider remains the only launch adapter. After cold-prefix initialization, while holding the compatibility lock, it inspects the exact artifact. For managed PE it prepares/verifies the bridge state. It then releases the lock and `exec`s Wine exactly as W1 does.

No shell, no user-visible `mono.exe`, no direct `csc.exe` production path, and no `prime-shell` authority are introduced.

### 5. Image closure

The image pins the Fedora package owners needed to source the bridge DLLs. Their versions and source hashes form a W3 bridge fingerprint separate from the frozen W1/W2 Wine donor fingerprint. W2 revision 1 remains immutable.

## W3 10-point acceptance benchmark

1. **CLR inspection** — real managed PE recognized by valid COM descriptor + `BSJB`; native PE remains unmanaged; malformed CLR structures fail closed.
2. **Managed dependency admission** — managed profile requires exact `runtime.wine-mono@1` pin; missing/wrong pin rejected; unmanaged W1 profiles unaffected.
3. **Bridge source contract** — exact immutable x64/x86 source paths and SHA-256 identities, no symlinks or ambiguous source selection.
4. **Bridge projection/recovery** — atomic per-prefix projection under shared lock; corruption/marker loss repaired or resealed from observed state.
5. **x64 managed console** — real managed console workload executes through Prime provider and emits exact proof marker.
6. **x86 managed console** — forced-x86 managed workload executes through Prime provider and emits exact proof marker.
7. **Managed GUI** — real WinForms GUI opens visibly through Prime Wayland path and exits cleanly.
8. **Lifecycle/stress/isolation** — cold + warm launches, 10+ repeat launches without leaked live processes, same-app serialization and cross-app state isolation preserved.
9. **Adversarial controls** — malformed CLR, missing runtime dependency, bridge source/target hash mismatch, symlink target, unsupported managed architecture, unavailable provider all fail closed; native W1/W2 regression remains green.
10. **Freeze/certification** — full regression, format/clippy/diff/security/package checkers, current Sergeant approval, exact frozen-SHA runtime rerun, Formula PASS manifest, push and remote SHA verification.

## Deferred proof

Only full sealed-image/Prime boot integration is deferred. W3 host implementation/proof must otherwise be complete on KRATOS before moving to W4.
