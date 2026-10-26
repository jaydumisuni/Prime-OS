#!/usr/bin/env python3
"""User-only Ubuntu app bridge. PREVIEW, never Prime Core or Prime Host."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import socketserver
import subprocess
import uuid

APP_DIR = Path("/usr/share/applications")
APPS = (
    ("Files", "thunar.desktop"),
    ("Terminal", "org.gnome.Ptyxis.desktop"),
    ("Browser", "brave-browser.desktop"),
    ("Settings", "xfce-settings-manager.desktop"),
    ("Text Editor", "org.gnome.TextEditor.desktop"),
    ("ChatGPT", "chatgpt.desktop"),
    ("Lumi DM", "com.lumi.dm.desktop"),
    ("Oracle", "com.thetechguy.oracle.desktop"),
)
NAMESPACE = uuid.UUID("92898904-fcf0-43c9-9d70-469880413234")
HOST_SENTINEL = "00000000-0000-0000-0000-000000000000"
LIMIT = 4096

def approved_apps():
    apps = {}
    for name, desktop in APPS:
        path = APP_DIR / desktop
        if not path.is_file() or path.is_symlink():
            continue
        apps[str(uuid.uuid5(NAMESPACE, desktop))] = (name, desktop, hashlib.sha256(path.read_bytes()).hexdigest())
    return apps

def project(apps):
    return {
        "schema": "prime.applications.v1", "interface": "prime.capability.v1",
        "interface_version": "1.0", "host_id": HOST_SENTINEL,
        "generation_id": "UBUNTU_PREVIEW_NOT_PRIME",
        "applications": [
            {"application_id": key, "display_name": name,
             "profile_revision": 1, "profile_digest": "ubuntu-desktop-sha256:" + digest,
             "execution_backend": "NATIVE", "compatibility": {"state": "RECOGNIZED", "evidence_refs": []},
             "launch_ready": True, "limitations": ["Ubuntu app passthrough, not Prime Exec admission"]}
            for key, (name, _, digest) in apps.items()
        ],
        "limitations": [
            "Ubuntu preview only; no real Prime Host identity or generation",
            "Application dispatch via gtk-launch; native Prime Exec not in use",
            "System power, storage, and Prime system capability control disabled",
        ],
    }

def dispatch(method, path, body, apps, launcher):
    if method == "GET" and path == "/v1/applications":
        return 200, project(apps)
    if method == "POST" and path == "/v1/shell/launch":
        try:
            req = json.loads(body)
            if req.get("schema") != "prime.shell-launch-request.v1":
                return 400, {"error": "Incorrect launch schema"}
            key = str(uuid.UUID(req["application_id"]))
        except (KeyError, ValueError, TypeError, AttributeError):
            return 400, {"error": "Invalid launch request"}
        if key not in apps:
            return 403, {"error": "Not on the explicit Ubuntu application allowlist"}
        try:
            launcher(apps[key][1])
        except OSError as exc:
            return 503, {"error": "Ubuntu dispatch failed", "detail": str(exc)}
        return 202, {"state": "DISPATCHED_TO_UBUNTU", "prime_native_acceptance": False}
    if path == "/v1/system/power":
        return 403, {"error": "Power disabled in Ubuntu preview"}
    if path in ("/v1/storage", "/v1/capabilities/prime.system.status"):
        return 503, {"error": "Native Prime Core unavailable; use Ubuntu settings"}
    return 404, {"error": "Unsupported preview route"}

class Handler(socketserver.StreamRequestHandler):
    def handle(self):
        self.connection.settimeout(3)
        try:
            tokens = self.rfile.readline(4096).decode("ascii").strip().split()
            if len(tokens) != 3 or not tokens[2].startswith("HTTP/1."):
                raise ValueError("Bad request line")
            method, path, _ = tokens
            length = 0
            for _ in range(40):
                line = self.rfile.readline(4096)
                if line == b"\r\n":
                    break
                if not line:
                    raise ValueError("Incomplete headers")
                k, v = line.decode("ascii").split(":", 1)
                if k.lower() == "content-length":
                    length = int(v.strip())
            else:
                raise ValueError("Too many headers")
            if length < 0 or length > LIMIT:
                raise ValueError("Invalid length")
            body = self.rfile.read(length)
            if len(body) != length:
                raise ValueError("Incomplete body")
            status, result = dispatch(method, path, body, self.server.apps, self.server.launcher)
        except (UnicodeError, ValueError, OSError):
            status, result = 400, {"error": "Malformed request"}
        wire = json.dumps(result).encode()
        reason = {200:"OK", 202:"Accepted", 400:"Bad Request", 403:"Forbidden", 404:"Not Found", 503:"Service Unavailable"}[status]
        header = "HTTP/1.1 %d %s\r\nContent-Type: application/json\r\nContent-Length: %d\r\nConnection: close\r\n\r\n" % (status, reason, len(wire))
        self.wfile.write(header.encode() + wire)

def launch(desktop_id):
    subprocess.Popen(["/usr/bin/gtk-launch", desktop_id], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--socket", type=Path, required=True)
    args = parser.parse_args()
    home = Path(os.environ["XDG_RUNTIME_DIR"]) / "prime-on-ubuntu"
    if args.socket.parent != home:
        parser.error("Socket must live in the user runtime directory")
    home.mkdir(mode=0o700, exist_ok=True)
    os.chmod(home, 0o700)
    if args.socket.exists() or args.socket.is_symlink():
        parser.error("Socket exists, refusing replacement")
    try:
        with socketserver.UnixStreamServer(str(args.socket), Handler) as server:
            os.chmod(args.socket, 0o600)
            server.apps = approved_apps()
            server.launcher = launch
            print("PRIME_UBUNTU_BRIDGE=ready;apps=%d;preview_only=1" % len(server.apps), flush=True)
            server.serve_forever(poll_interval=0.2)
    finally:
        if args.socket.is_socket():
            args.socket.unlink()

if __name__ == "__main__":
    main()
