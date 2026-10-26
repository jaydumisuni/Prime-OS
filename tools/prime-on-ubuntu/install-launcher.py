#!/usr/bin/env python3
"""Install a user-only launcher. No session default, autostart, or GDM changes."""
from pathlib import Path
import os
import sys

root = Path(__file__).resolve().parents[2]
launch = root / "tools/prime-on-ubuntu/session.py"
target = Path.home() / ".local/share/applications/prime-on-ubuntu-preview.desktop"
if not launch.is_file():
    sys.exit("Prime session source missing")
target.parent.mkdir(parents=True, exist_ok=True)
text = (
    "[Desktop Entry]\n"
    "Type=Application\n"
    "Version=1.0\n"
    "Name=Prime OS (Ubuntu Preview)\n"
    "Comment=Run Prime Shell inside Ubuntu without changing XFCE\n"
    "Exec=/usr/bin/python3 " + str(launch) + "\n"
    "TryExec=/usr/bin/labwc\n"
    "Icon=applications-system\n"
    "Terminal=false\n"
    "StartupNotify=true\n"
    "Categories=System;Development;\n"
    "X-Prime-Integration=ubuntu-nested-preview-only\n"
)
if target.exists():
    if target.read_text() != text:
        sys.exit("Existing launcher differs; refusing overwrite")
    print("ALREADY_INSTALLED", target)
else:
    target.write_text(text)
    target.chmod(0o644)
    print("INSTALLED", target)
