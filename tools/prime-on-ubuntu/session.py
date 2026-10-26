#!/usr/bin/env python3
"""Nested Prime UI session; preserves Ubuntu XFCE, GDM autologin and boot."""
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys

BASE = Path(__file__).resolve().parents[2]
TOOLS = Path(__file__).resolve().parent

def recover_owner_stale_socket(sock):
    if sock.is_symlink():
        raise RuntimeError("Refusing a symlink at preview bridge socket")
    if not sock.exists():
        return
    if not sock.is_socket():
        raise RuntimeError("Refusing non-socket preview path")
    probe = socket.socket(socket.AF_UNIX)
    try:
        probe.connect(str(sock))
    except ConnectionRefusedError:
        sock.unlink()
    else:
        raise RuntimeError("A preview bridge is already active; refusing duplicate")
    finally:
        probe.close()

def main():
    runtime = os.environ.get("XDG_RUNTIME_DIR")
    if not runtime or not Path(runtime).is_dir():
        sys.exit("Missing XDG_RUNTIME_DIR")
    shell = Path(os.environ.get("PRIME_SHELL_BIN", str(BASE / "target/release/prime-shell")))
    if not shell.is_file() or not os.access(shell, os.X_OK):
        sys.exit("Missing executable Prime Shell: " + str(shell))
    if not shutil.which("labwc"):
        sys.exit("labwc compositor is not installed")
    if not os.environ.get("DISPLAY") or not Path("/tmp/.X11-unix/X0").exists():
        sys.exit("Existing Ubuntu/XFCE X11 session required")
    folder = Path(runtime) / "prime-on-ubuntu"
    folder.mkdir(mode=0o700, exist_ok=True)
    os.chmod(folder, 0o700)
    sock = folder / "core.sock"
    try:
        recover_owner_stale_socket(sock)
    except RuntimeError as error:
        sys.exit(str(error))
    env = dict(os.environ)
    env.update(PRIME_CORE_SOCKET=str(sock), PRIME_SHELL_BIN=str(shell),
               PRIME_UBUNTU_TOOLS=str(TOOLS), PRIME_UBUNTU_PREVIEW="1",
               WLR_BACKENDS="x11", WLR_X11_OUTPUTS="1",
               XDG_CURRENT_DESKTOP="Prime-Ubuntu-Preview")
    print("PRIME_UBUNTU_SESSION=nested;parent_xfce_unchanged=1", flush=True)
    try:
        return subprocess.call(["labwc", "-C", str(TOOLS / "labwc"),
                                "-S", str(TOOLS / "labwc-dbus-session.sh")], env=env)
    finally:
        try:
            recover_owner_stale_socket(sock)
        except RuntimeError:
            print("Preview bridge still active; not replacing its socket", file=sys.stderr)

if __name__ == "__main__":
    sys.exit(main())
