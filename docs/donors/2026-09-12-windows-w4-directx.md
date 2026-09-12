# Windows Personality W4 donor decision — DXVK / Vulkan

## Decision

Prime W4 uses Fedora-packaged DXVK behind the existing Prime Windows provider boundary to earn accelerated Direct3D 8/9/10/11 execution over Vulkan. Prime remains authority for workload admission, application state, compatibility isolation, launch supervision, evidence and failure semantics. DXVK is an implementation donor and is never a Prime-facing backend choice.

## Frozen donor target

- DXVK upstream release `v2.7.1`, exact archive SHA-256 `d85ce7c79f57ecd765aaa1b9e7007cb875e6fde9f6d331df799bce73d513ce87`, as the single architecture-complete image payload authority (`x64/` + `x32/`).
- Fedora 44 `wine-dxvk` 2.7.1-6.fc44 remains corroborating distribution evidence for the x86_64 Wine/DXVK integration shape, not the byte authority used by Prime.
- Fedora 44 Mesa Vulkan driver closure matching the Prime image substrate.
- Existing Fedora Wine 11.0-3.fc44 provider remains the launch donor.

Using the upstream release as Prime's immutable payload authority avoids mixing an x86_64 Fedora payload with a separately-built 32-bit payload. Prime pins every projected DLL by SHA-256 and stores both architectures under `/usr/lib/prime/windows-gpu-runtime/{x64,x86}`. Fedora packaging still confirms the intended Wine integration and package-level Vulkan dependencies. Prime owns deterministic per-application activation rather than relying on host alternatives state.

## W4 contract

1. W3 is immutable predecessor authority at `8af61f4cb604b64172e35c0ca76b4b4693a17623`.
2. W4 work happens only on `feature/windows-personality-w4-directx`.
3. DXVK payloads are image-owned, version-pinned and projected only into Prime-owned application compatibility state.
4. Target replacement is atomic; symlink/non-regular targets fail closed.
5. W4 acceleration proof must demonstrate a real Direct3D workload using Vulkan/DXVK, not WineD3D fallback.
6. x86 and x86_64 remain required workload architectures before W4 can be frozen.
7. Vulkan absence or an unusable ICD is an explicit `WINDOWS_GPU_ACCELERATION_UNAVAILABLE` condition; Prime must not silently call software/fallback rendering W4 proof.
8. Full sealed-image / physical Prime boot integration remains deferred and cannot be claimed from host proof.

## Scope boundary

W4 proves accelerated Direct3D 8-11 first. Direct3D 12 is not silently implied by DXVK and requires VKD3D evidence before it can be added to the W4 claim. W5 COM, W6 services, W7 device integration, W8 VM fallback and W9 TTG workload certification remain separate milestones.
