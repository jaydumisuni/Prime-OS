#!/usr/bin/env python3
from pathlib import Path

path = Path("tools/prove-p1-local.sh")
text = path.read_text(encoding="utf-8")

replacements = [
    (
        """    test -x /usr/libexec/prime/primed\n    test -x /usr/libexec/prime/prime-recovery\n    test -x /usr/sbin/bootc\n""",
        """    test -x /usr/libexec/prime/primed\n    test -x /usr/libexec/prime/prime-recovery\n    test -x /usr/libexec/prime/prime-compositor\n    test -x /usr/sbin/bootc\n""",
        "compositor executable assertion",
    ),
    (
        """    rpm -q systemd-ukify-259.8-1.fc44 systemd-boot-unsigned-259.8-1.fc44\n""",
        """    rpm -q \\\n      libdrm-2.4.134-1.fc44 \\\n      libglvnd-egl-1.7.0-9.fc44 \\\n      libinput-1.31.3-1.fc44 \\\n      libseat-0.9.3-1.fc44 \\\n      mesa-dri-drivers-26.1.6-1.fc44 \\\n      mesa-libEGL-26.1.6-1.fc44 \\\n      mesa-libgbm-26.1.6-1.fc44 \\\n      systemd-boot-unsigned-259.8-1.fc44 \\\n      systemd-ukify-259.8-1.fc44\n""",
        "renderer runtime RPM assertions",
    ),
    (
        """    grep -q \"^PRETTY_NAME=\\\"Prime OS P1 First Light\\\"$\" /usr/lib/os-release\n    normal=\"$(find /boot/EFI/Linux -maxdepth 1 -type f -name \"*.efi\" ! -name \"*.recovery.efi\" -print -quit)\"\n""",
        """    grep -q \"^PRETTY_NAME=\\\"Prime OS P1 First Light\\\"$\" /usr/lib/os-release\n    test -e /usr/lib64/libEGL.so.1\n    test -e /usr/lib64/libEGL_mesa.so.0\n    test -e /usr/lib64/libgbm.so.1\n    test -e /usr/lib64/libdrm.so.2\n    test -e /usr/lib64/dri/iris_dri.so\n    test \"$(rpm -qf /usr/lib64/libEGL.so.1)\" = \"libglvnd-egl-1.7.0-9.fc44.x86_64\"\n    test \"$(rpm -qf /usr/lib64/libEGL_mesa.so.0)\" = \"mesa-libEGL-26.1.6-1.fc44.x86_64\"\n    test \"$(rpm -qf /usr/lib64/libgbm.so.1)\" = \"mesa-libgbm-26.1.6-1.fc44.x86_64\"\n    test \"$(rpm -qf /usr/lib64/libdrm.so.2)\" = \"libdrm-2.4.134-1.fc44.x86_64\"\n    test \"$(rpm -qf /usr/lib64/dri/iris_dri.so)\" = \"mesa-dri-drivers-26.1.6-1.fc44.x86_64\"\n    ! ldd /usr/libexec/prime/prime-compositor | grep -q \"not found\"\n    /usr/libexec/prime/prime-compositor --help | grep -F \"Usage: prime-compositor [--probe]\"\n    test ! -e /etc/systemd/system/multi-user.target.wants/prime-compositor.service\n    normal=\"$(find /boot/EFI/Linux -maxdepth 1 -type f -name \"*.efi\" ! -name \"*.recovery.efi\" -print -quit)\"\n""",
        "renderer runtime file/linkage assertions",
    ),
]

for old, new, label in replacements:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"expected exactly one {label}, found {count}")
    text = text.replace(old, new, 1)

path.write_text(text, encoding="utf-8")
