#!/usr/bin/env python3
from pathlib import Path

container = Path("image/Containerfile").read_text()
checks = {
    "gpu runtime builder exists": "AS windows-gpu-runtime-builder" in container,
    "upstream archive is exact": "https://github.com/doitsujin/dxvk/releases/download/v2.7.1/dxvk-2.7.1.tar.gz" in container,
    "archive digest pinned": "d85ce7c79f57ecd765aaa1b9e7007cb875e6fde9f6d331df799bce73d513ce87" in container,
    "x64 d3d11 digest pinned": "523da2cd765dd51d99fe1644b24f9ee7ee6247e4c98953bcc449ce84cd303405" in container,
    "x86 d3d11 digest pinned": "7406e13d2694244499783d2e9fb1be285b6324890a348137fe5b5042c64b5913" in container,
    "rootfs owns gpu runtime": "COPY --from=windows-gpu-runtime-builder /prime-w4-runtime /usr/lib/prime/windows-gpu-runtime" in container,
    "Vulkan multilib loaders are pinned": ("vulkan-loader-1.4.341.0-1.fc44.x86_64" in container and "vulkan-loader-1.4.341.0-1.fc44.i686" in container),
    "Mesa Vulkan multilib drivers match frozen closure": ("mesa-vulkan-drivers-26.1.8-1.fc44.x86_64" in container and "mesa-vulkan-drivers-26.1.8-1.fc44.i686" in container),
    "Vulkan loader path is asserted": "test -e /usr/lib64/libvulkan.so.1" in container,
    "Vulkan ICD directory is asserted": "test -d /usr/share/vulkan/icd.d" in container,
    "x86 Wine core is pinned": "wine-core-11.0-3.fc44.i686" in container,
    "Wine multilib Wayland runtime is version-aligned": ("libwayland-client-1.26.0-1.fc44.x86_64" in container and "libwayland-client-1.26.0-1.fc44.i686" in container),
}
failed = [name for name, ok in checks.items() if not ok]
for name, ok in checks.items():
    print(("PASS" if ok else "FAIL"), name)
raise SystemExit(1 if failed else 0)
