#!/usr/bin/env python3
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sysusers = (ROOT / "image/sysusers/prime-shell.conf").read_text()
compositor = (ROOT / "image/systemd/prime-compositor.service").read_text()
shell = (ROOT / "image/systemd/prime-shell.service").read_text()
core = (ROOT / "image/systemd/primed.service").read_text()
windows = (ROOT / "crates/primed/src/windows_personality.rs").read_text()

checks = {
    "prime-display group exists": "g prime-display" in sysusers,
    "shell is display group member": "m prime-shell prime-display" in sysusers,
    "compositor primary group is display-only": "Group=prime-display" in compositor,
    "compositor runtime directory is group accessible": "RuntimeDirectoryMode=0750" in compositor,
    "compositor socket creation umask preserves group write": "UMask=0007" in compositor,
    "shell retains Core-authorized primary group": "Group=prime-shell" in shell,
    "shell explicitly receives display group": "SupplementaryGroups=prime-display" in shell,
    "Core remains in prime-shell group": "Group=prime-shell" in core,
    "Windows workload gets display group only": "SupplementaryGroups=prime-display" in windows,
    "Windows workload gets compositor runtime directory": "XDG_RUNTIME_DIR=/run/prime-compositor" in windows,
    "Windows workload never receives Core group": "SupplementaryGroups=prime-shell" not in windows,
}
failed = [name for name, ok in checks.items() if not ok]
for name, ok in checks.items():
    print(f"{'PASS' if ok else 'FAIL'} {name}")
if failed:
    raise SystemExit(f"Windows W1 display-security contract failed: {', '.join(failed)}")
