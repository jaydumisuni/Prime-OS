#!/usr/bin/env python3
from pathlib import Path

path = Path("image/Containerfile")
text = path.read_text(encoding="utf-8")

old_packages = """    dnf -y install \\\n        libinput-1.31.3-1.fc44 \\\n        libseat-0.9.3-1.fc44 \\\n        systemd-boot-unsigned-259.8-1.fc44 \\\n        systemd-ukify-259.8-1.fc44; \\\n    rpm -q \\\n        libinput-1.31.3-1.fc44 \\\n        libseat-0.9.3-1.fc44 \\\n        systemd-boot-unsigned-259.8-1.fc44 \\\n        systemd-ukify-259.8-1.fc44; \\
"""

new_packages = """    dnf -y install \\\n        libglvnd-egl-1.7.0-9.fc44 \\\n        libinput-1.31.3-1.fc44 \\\n        libseat-0.9.3-1.fc44 \\\n        mesa-dri-drivers-26.1.6-1.fc44 \\\n        mesa-libEGL-26.1.6-1.fc44 \\\n        mesa-libgbm-26.1.6-1.fc44 \\\n        systemd-boot-unsigned-259.8-1.fc44 \\\n        systemd-ukify-259.8-1.fc44; \\\n    rpm -q \\\n        libdrm-2.4.134-1.fc44 \\\n        libglvnd-egl-1.7.0-9.fc44 \\\n        libinput-1.31.3-1.fc44 \\\n        libseat-0.9.3-1.fc44 \\\n        mesa-dri-drivers-26.1.6-1.fc44 \\\n        mesa-libEGL-26.1.6-1.fc44 \\\n        mesa-libgbm-26.1.6-1.fc44 \\\n        systemd-boot-unsigned-259.8-1.fc44 \\\n        systemd-ukify-259.8-1.fc44; \\
"""

old_checks = """    ! ldd /usr/libexec/prime/prime-compositor | grep -q 'not found'; \\\n    /usr/libexec/prime/prime-compositor --help | grep -F 'Usage: prime-compositor [--probe]'; \\
"""

new_checks = """    ! ldd /usr/libexec/prime/prime-compositor | grep -q 'not found'; \\\n    test -e /usr/lib64/libEGL.so.1; \\\n    test -e /usr/lib64/libEGL_mesa.so.0; \\\n    test -e /usr/lib64/libgbm.so.1; \\\n    test -e /usr/lib64/libdrm.so.2; \\\n    test -e /usr/lib64/dri/iris_dri.so; \\\n    rpm -qf /usr/lib64/libEGL.so.1 | grep -Fx 'libglvnd-egl-1.7.0-9.fc44.x86_64'; \\\n    rpm -qf /usr/lib64/libEGL_mesa.so.0 | grep -Fx 'mesa-libEGL-26.1.6-1.fc44.x86_64'; \\\n    rpm -qf /usr/lib64/libgbm.so.1 | grep -Fx 'mesa-libgbm-26.1.6-1.fc44.x86_64'; \\\n    rpm -qf /usr/lib64/libdrm.so.2 | grep -Fx 'libdrm-2.4.134-1.fc44.x86_64'; \\\n    rpm -qf /usr/lib64/dri/iris_dri.so | grep -Fx 'mesa-dri-drivers-26.1.6-1.fc44.x86_64'; \\\n    /usr/libexec/prime/prime-compositor --help | grep -F 'Usage: prime-compositor [--probe]'; \\
"""

for old, new, label in [
    (old_packages, new_packages, "runtime package block"),
    (old_checks, new_checks, "runtime renderer checks"),
]:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one {label}, found {count}")
    text = text.replace(old, new, 1)

path.write_text(text, encoding="utf-8")
