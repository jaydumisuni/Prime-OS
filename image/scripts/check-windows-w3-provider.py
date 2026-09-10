#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
containerfile = (ROOT / "image/Containerfile").read_text()
managed = (ROOT / "crates/primed/src/windows_managed.rs").read_text()
personality = (ROOT / "crates/primed/src/windows_personality.rs").read_text()
provider = (ROOT / "crates/primed/src/bin/prime-windows-provider-wine.rs").read_text()

checks = {
    "build-only managed runtime stage exists": "AS windows-managed-runtime-builder" in containerfile,
    "final rootfs copies only extracted W3 runtime": "COPY --from=windows-managed-runtime-builder /prime-w3-runtime /usr/lib/prime/windows-managed-runtime" in containerfile,
    "x64 iconv owner pinned in build stage": "mingw64-win-iconv-0.0.10-4.fc44" in containerfile,
    "x86 libgcc owner pinned in build stage": "mingw32-libgcc-16.1.1-1.fc44" in containerfile,
    "x86 winpthreads dependency pinned and verified": "mingw32-winpthreads-13.0.0-3.fc44" in containerfile,
    "x64 iconv hash pinned in code and image": managed.count("74e23093e962adceccf77d2b05af4f4c7d6deee4a524140a296609f849700fc2") >= 1 and containerfile.count("74e23093e962adceccf77d2b05af4f4c7d6deee4a524140a296609f849700fc2") >= 2,
    "x86 libgcc hash pinned in code and image": managed.count("09caac62f690912492418d5ca86c3340f5a76e5de07e5d6b7fe057e032b779d1") >= 1 and containerfile.count("09caac62f690912492418d5ca86c3340f5a76e5de07e5d6b7fe057e032b779d1") >= 2,
    "x86 winpthread hash pinned in code and image": managed.count("1a8f18e693e49581c2d11d02812cadbaffa1bf6184353fa69ef35ac2950b9d9e") >= 1 and containerfile.count("1a8f18e693e49581c2d11d02812cadbaffa1bf6184353fa69ef35ac2950b9d9e") >= 2,
    "managed runtime exact component pin enforced": "TRUSTED_WINE_MONO_COMPONENT_REFERENCE" in personality and "ManagedRuntimeDependencyMissing" in personality,
    "provider prepares managed runtime under compatibility path": "prepare_managed_runtime(" in provider and "acquire_initialization_lock" in provider,
}
failed=[]
for label, ok in checks.items():
    print(("PASS" if ok else "FAIL") + ": " + label)
    if not ok: failed.append(label)
if failed:
    raise SystemExit(1)
print(f"PASS_TOTAL={len(checks)}")
