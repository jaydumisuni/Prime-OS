# Prime Windows Personality W4 — DirectX/GPU acceleration design

## Goal

Advance P4A/W4 from the frozen W3 host benchmark by adding deterministic, donor-neutral accelerated Direct3D execution. Prime must prove that a real Windows Direct3D workload reaches DXVK/Vulkan on KRATOS while preserving W1-W3 behavior and per-application isolation.

## Architecture

The existing Prime Windows provider remains the only launch adapter. W4 adds a small `windows_gpu` layer beneath that adapter. The layer owns the immutable DXVK source contract, safe projection into the Prime-owned Wine prefix, an exact W4 fingerprint, DLL override construction and Vulkan capability validation. Shell, Exec, profiles and users continue to see only Prime Windows Personality contracts.

DXVK files are sourced from the immutable Prime image, never from the application directory, downloads, user PATH, or a mutable prefix. For each application, W4 copies the architecture-correct DXVK DLLs into the prefix under the existing compatibility lock and repairs corrupt/missing copies from image-owned sources. Projection rejects symlink/non-regular sources and targets.

## Activation

W4 activation is workload-sensitive. Prime will mechanically identify Direct3D/DXGI imports from the admitted PE rather than globally forcing DXVK for every Windows application. Direct3D 8/9/10/11 workloads receive the exact native DLL override set needed for their imports. Non-DirectX W1-W3 workloads retain their existing launch environment.

## Host capability gate

Accelerated launch requires the Vulkan loader plus at least one usable image-owned/runtime-visible ICD. The proof gate must additionally demonstrate DXVK evidence from a real Direct3D fixture. Missing Vulkan capability fails explicitly rather than being counted as W4 via WineD3D.

## Proof gates

1. W3 frozen predecessor and clean W4 branch ancestry.
2. Mechanical Direct3D import recognition with malformed-PE negative controls.
3. Exact image-owned DXVK source contract and W4 fingerprint.
4. Safe x64 projection/repair and override construction.
5. Safe x86 projection/repair and override construction.
6. Vulkan capability fail-closed controls.
7. Real x64 Direct3D accelerated execution with DXVK/Vulkan evidence.
8. Real x86 Direct3D accelerated execution with DXVK/Vulkan evidence.
9. Lifecycle/isolation/corruption/adversarial sweep plus W1-W3 regression.
10. Sergeant review, exact-SHA freeze, Formula verifier, remote binding.

Full sealed-image / physical Prime boot integration remains the only allowed deferred integration gate; W4 cannot claim that gate.
