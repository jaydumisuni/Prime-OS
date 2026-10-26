#!/usr/bin/env python3
"""User-only Ubuntu app bridge. PREVIEW, never Prime Core or Prime Host."""
import argparse
import configparser
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
    ("Notes", "org.gnome.TextEditor.desktop"),
    ("Gallery", "org.gnome.Loupe.desktop"),
    ("ChatGPT", "chatgpt.desktop"),
    ("Lumi DM", "com.lumi.dm.desktop"),
    ("Oracle", "com.thetechguy.oracle.desktop"),
)
NAMESPACE = uuid.UUID("92898904-fcf0-43c9-9d70-469880413234")
HOST_SENTINEL = "00000000-0000-0000-0000-000000000000"
LIMIT = 4096

def discover_desktops(directories, *, cap=2048):
    """Bounded XDG application inventory. Desktop IDs, never arbitrary commands."""
    known = {}
    for directory in directories:
        directory = Path(directory)
        if not directory.is_dir() or directory.is_symlink():
            continue
        for path in sorted(directory.rglob("*.desktop")):
            if len(known) >= cap:
                return known
            if not path.is_file() or path.is_symlink():
                continue
            if any(part.is_symlink() for part in path.parents if part != directory and directory in part.parents):
                continue
            desktop_id = path.relative_to(directory).as_posix().replace("/", "-")
            if desktop_id in known:
                continue
            try:
                if path.stat().st_size > 128 * 1024:
                    continue
                conf = configparser.ConfigParser(interpolation=None, strict=False)
                conf.optionxform = str
                conf.read(path, encoding="utf-8")
                item = conf["Desktop Entry"]
                if item.get("Type") != "Application" or not item.get("Exec"):
                    continue
                if item.get("Hidden", "false").lower() == "true" or item.get("NoDisplay", "false").lower() == "true":
                    continue
                label = item.get("Name", "").strip()
                if not label:
                    continue
                digest = hashlib.sha256(path.read_bytes()).hexdigest()
            except (OSError, ValueError, KeyError, configparser.Error, UnicodeError):
                continue
            known[desktop_id] = (label, desktop_id, digest)
    return known


def approved_apps():
    user_apps = Path.home() / ".local/share/applications"
    standard = discover_desktops([user_apps, Path("/usr/local/share/applications"), APP_DIR])
    apps = {}
    # Preserve stable familiar aliases where actual desktop entries are installed.
    for name, desktop in APPS:
        if desktop not in standard:
            continue
        _label, desktop_id, digest = standard.pop(desktop)
        apps[str(uuid.uuid5(NAMESPACE, desktop_id))] = (name, desktop_id, digest)
    # Named system surfaces are real routes or explicitly not yet implemented;
    # they are never passed to gtk-launch as invented .desktop applications.
    for label,backend in (
        ("Machine","prime-system:machine"),
        ("Store","prime-pending:store"),
        ("Music","prime-pending:music"),
        ("Chat","prime-pending:chat"),
        ("Tools","prime-pending:tools"),
    ):
        key=str(uuid.uuid5(NAMESPACE,backend))
        apps[key]=(label,backend,hashlib.sha256(backend.encode()).hexdigest())
    for name, desktop, digest in standard.values():
        # Never recursively launch this temporary nested Prime preview.
        if desktop == "prime-on-ubuntu-preview.desktop":
            continue
        apps[str(uuid.uuid5(NAMESPACE, desktop))] = (name, desktop, digest)
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
             "launch_ready": not desktop.startswith("prime-pending:"),
             "limitations": (["Prime application backend not implemented"]
                if desktop.startswith("prime-pending:")
                else ["Ubuntu preview dispatch only, not native Prime Exec admission"])}
            for key, (name, desktop, digest) in apps.items()
        ],
        "limitations": [
            "Ubuntu preview only; no real Prime Host identity or generation",
            "Application dispatch via gtk-launch; native Prime Exec not in use",
            "System power, storage, and Prime system capability control disabled",
        ],
    }

def read_host_control_state():
    def run(args, timeout=2):
        try:
            result=subprocess.run(args,timeout=timeout,text=True,
                                  capture_output=True,check=False)
            return result.stdout.strip() if result.returncode==0 else None
        except (OSError,subprocess.SubprocessError):
            return None
    wifi=run(["/usr/bin/nmcli","-t","-f","WIFI","g"])
    profile=run(["/usr/bin/powerprofilesctl","get"])
    volume=run(["/usr/bin/wpctl","get-volume","@DEFAULT_AUDIO_SINK@"])
    volume_percent=None
    if volume:
        import re
        match=re.search(r"(0\.\d+|1(?:\.0+)?)",volume)
        if match:
            volume_percent=min(100,max(0,round(float(match.group(1))*100)))
    # bluetoothctl may hang for two seconds when BlueZ is absent. Never let an
    # optional panel probe exceed Prime Shell's 2 s IPC timeout.
    bluetooth=None
    adapter_dir=Path("/sys/class/bluetooth")
    if adapter_dir.is_dir() and any(adapter_dir.iterdir()):
        status=run(["/usr/bin/bluetoothctl","show"],timeout=0.25)
        if status:
            for line in status.splitlines():
                if "Powered:" in line:
                    bluetooth="On" if line.split("Powered:",1)[1].strip().lower()=="yes" else "Off"
                    break
    return {"wifi":wifi,"bluetooth":bluetooth,
            "performance":profile,"volume_percent":volume_percent,
            "brightness_percent":None,
            "focus":None,"dark_mode":None,"night_light":None}

def ubuntu_host_preview():
    """Read-only, explicitly Ubuntu-owned status; never native Prime authority."""
    links = []
    for device in sorted(Path("/sys/class/net").glob("*")):
        if device.name == "lo":
            continue
        try:
            oper = (device / "operstate").read_text().strip()
        except OSError:
            oper = "unknown"
        links.append(f"{device.name} {oper.upper()}")
    try:
        fs = os.statvfs(Path.home())
        total = fs.f_frsize * fs.f_blocks
        used = total - fs.f_frsize * fs.f_bfree
    except OSError:
        total = used = 0
    audio = Path("/proc/asound/cards")
    if audio.exists():
        try:
            audio_name = audio.read_text()[:180].splitlines()[0].strip()
        except (OSError, IndexError):
            audio_name = "Ubuntu managed"
    else:
        audio_name = "Ubuntu managed"
    return {
        "schema":"prime.ubuntu.host-preview.v1",
        "network":links[:3],
        "audio":audio_name,
        "storage_used_bytes":used,
        "storage_total_bytes":total,
        "controls":read_host_control_state(),
        "limitations":["Read-only Ubuntu host summary; Prime native controls unavailable"],
    }



def index_user_files(home=None, cap=4000):
    root = Path(home or Path.home())
    indexed = []
    for folder_name in ("Desktop", "Documents", "Downloads", "Pictures", "Music", "Videos"):
        folder = root / folder_name
        if not folder.is_dir() or folder.is_symlink():
            continue
        for directory, subdirs, filenames in os.walk(folder, followlinks=False):
            current = Path(directory)
            subdirs[:] = sorted(name for name in subdirs if not name.startswith(".") and name not in ("node_modules", "__pycache__", ".git"))
            if len(current.relative_to(folder).parts) >= 4:
                subdirs.clear()
            for name in sorted(filenames):
                if len(indexed) >= cap:
                    return indexed
                path = current / name
                if name.startswith(".") or path.is_symlink() or not path.is_file():
                    continue
                indexed.append({"name":name,"path":str(path),"kind":"file"})
    return indexed

def search_preview_files(query, files):
    query = query.strip().casefold()
    return [item for item in files if query in item["name"].casefold()][:6] if len(query) >= 2 else []

def open_indexed_file(path, files, opener):
    if not any(item["path"] == path for item in files):
        return 403, {"error":"File not in local search index"}
    candidate = Path(path)
    if candidate.is_symlink() or not candidate.is_file():
        return 404, {"error":"Indexed file unavailable"}
    try:
        opener(path)
    except OSError:
        return 503, {"error":"Ubuntu file dispatcher failed"}
    return 202, {"state":"DISPATCHED_TO_UBUNTU","preview_only":True}


def live_windows():
    """Actual compositor toplevels, not applications presumed open after dispatch."""
    try:
        response = subprocess.run(["/usr/bin/wlrctl", "toplevel", "list"],
            check=True, timeout=2, capture_output=True, text=True)
    except (OSError, subprocess.SubprocessError):
        return []
    windows = []
    for line in response.stdout.splitlines()[:128]:
        if ": " not in line:
            continue
        app_id, title = line.split(": ", 1)
        app_id = app_id.strip()
        if not app_id or len(app_id) > 200:
            continue
        windows.append({"app_id":app_id,"title":title[:160]})
    return windows

def focus_indexed_window(app_id, windows, focuser=None):
    if not isinstance(app_id,str) or not any(w["app_id"] == app_id for w in windows):
        return 403, {"error":"Window not currently reported by the compositor"}
    if focuser is None:
        def focuser(identifier):
            subprocess.run(["/usr/bin/wlrctl","toplevel","focus","app_id:"+identifier],
                timeout=3,check=True,capture_output=True)
    try:
        focuser(app_id)
    except (OSError, subprocess.SubprocessError):
        return 503, {"error":"Window focus request failed"}
    return 202, {"state":"FOCUS_REQUESTED","not_open_confirmation":True}

def dispatch(method, path, body, apps, launcher, files=None, file_opener=None):
    if method == "GET" and path == "/v1/applications":
        return 200, project(apps)
    if method == "GET" and path == "/v1/preview/host-status":
        return 200, ubuntu_host_preview()
    if method == "POST" and path == "/v1/preview/open-system":
        try:
            target=json.loads(body)["target"]
        except (ValueError,TypeError,KeyError):
            return 400,{"error":"Invalid system shortcut"}
        known={"machine":"computer:///","bin":"trash:///"}
        if target=="projects":
            folder=Path.home()/"Projects"
            if not folder.is_dir():
                return 404,{"error":"Projects folder does not exist"}
            destination=str(folder)
        elif target in known:
            destination=known[target]
        else:
            return 403,{"error":"Unknown system shortcut"}
        try:
            (file_opener or launch_file)(destination)
        except OSError:
            return 503,{"error":"Ubuntu system shortcut dispatch failed"}
        return 202,{"state":"DISPATCHED_TO_UBUNTU","preview_only":True}
    if method == "GET" and path == "/v1/preview/windows":
        return 200, {"schema":"prime.ubuntu.windows.v1","windows":live_windows()}
    if method == "POST" and path == "/v1/preview/focus":
        try:
            app_id=json.loads(body)["app_id"]
            if not isinstance(app_id,str): raise ValueError()
        except (ValueError,KeyError,TypeError):
            return 400,{"error":"Invalid window focus request"}
        return focus_indexed_window(app_id,live_windows())
    if method == "POST" and path == "/v1/preview/search":
        try:
            query=json.loads(body)["query"]
            if not isinstance(query,str) or len(query)>64: raise ValueError()
        except (ValueError,KeyError,TypeError):
            return 400,{"error":"Invalid local search query"}
        return 200,{"schema":"prime.ubuntu.file-search.v1","results":search_preview_files(query,files or [])}
    if method == "POST" and path == "/v1/preview/open-file":
        try:
            chosen=json.loads(body)["path"]
            if not isinstance(chosen,str) or len(chosen)>4096:raise ValueError()
        except (ValueError,KeyError,TypeError):
            return 400,{"error":"Invalid local file selection"}
        return open_indexed_file(chosen,files or [],file_opener or launch_file)
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
        backend=apps[key][1]
        if backend.startswith("prime-pending:"):
            return 403,{"error":"Prime application backend not implemented"}
        try:
            if backend=="prime-system:machine":
                (file_opener or launch_file)("computer:///")
            else:
                launcher(backend)
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
            status, result = dispatch(method, path, body, self.server.apps, self.server.launcher, self.server.files, self.server.file_opener)
        except (UnicodeError, ValueError, OSError):
            status, result = 400, {"error": "Malformed request"}
        wire = json.dumps(result).encode()
        reason = {200:"OK", 202:"Accepted", 400:"Bad Request", 403:"Forbidden", 404:"Not Found", 503:"Service Unavailable"}[status]
        header = "HTTP/1.1 %d %s\r\nContent-Type: application/json\r\nContent-Length: %d\r\nConnection: close\r\n\r\n" % (status, reason, len(wire))
        self.wfile.write(header.encode() + wire)

def launch(desktop_id):
    subprocess.Popen(["/usr/bin/gtk-launch", desktop_id], stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, start_new_session=True)

def launch_file(path):
    subprocess.Popen(["/usr/bin/gio","open",path],stdin=subprocess.DEVNULL,stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL,start_new_session=True)


def materialize_app_icons(apps, runtime):
    """Render XDG application icon artwork to an isolated user-owned PNG cache.
    Uses GTK's installed icon theme; never asks the Prime Shell to draw fake logos.
    """
    icon_dir = runtime / "icons"
    icon_dir.mkdir(mode=0o700, exist_ok=True)
    os.chmod(icon_dir, 0o700)
    try:
        import gi
        gi.require_version("Gtk", "3.0")
        from gi.repository import Gtk, GdkPixbuf
    except (ImportError, ValueError):
        return 0
    theme = Gtk.IconTheme.new()
    for search in ("/usr/share/icons/Yaru", "/usr/share/icons/Adwaita",
                   "/usr/share/icons/hicolor", "/usr/share/pixmaps"):
        if Path(search).is_dir():
            theme.append_search_path(search)
    # Directory ordering must match discover_desktops precedence.
    roots = (Path.home() / ".local/share/applications",
             Path("/usr/local/share/applications"), APP_DIR)
    desktop_paths = {}
    wanted = {value[1] for value in apps.values()}
    for directory in roots:
        if not directory.is_dir():
            continue
        for candidate in sorted(directory.rglob("*.desktop")):
            if candidate.is_symlink() or not candidate.is_file():
                continue
            key = candidate.relative_to(directory).as_posix().replace("/", "-")
            if key in wanted and key not in desktop_paths:
                desktop_paths[key] = candidate
    generated = 0
    for application_id, (_, desktop_id, _) in apps.items():
        desktop_path = desktop_paths.get(desktop_id)
        if desktop_path is None:
            continue
        try:
            conf = configparser.ConfigParser(interpolation=None, strict=False)
            conf.optionxform = str
            conf.read(desktop_path, encoding="utf-8")
            name = conf["Desktop Entry"].get("Icon", "").strip()
            if not name:
                continue
            if name.startswith("/"):
                # User-supplied absolute icon is treated as artwork, not executable.
                source = Path(name)
                if not source.is_file():
                    continue
                pixbuf = GdkPixbuf.Pixbuf.new_from_file_at_scale(str(source), 108, 108, True)
            else:
                candidate = theme.lookup_icon(name, 108, Gtk.IconLookupFlags.FORCE_SIZE)
                if candidate is None:
                    continue
                pixbuf = candidate.load_icon()
            output = icon_dir / f"{application_id}.png"
            pixbuf.savev(str(output), "png", [], [])
            os.chmod(output, 0o600)
            generated += 1
        except (OSError, KeyError, ValueError, configparser.Error, Exception) as error:
            # Theme renderers can reject malformed/corrupted third-party icons.
            print(f"PRIME_PREVIEW_ICON_SKIP={desktop_id}:{type(error).__name__}", flush=True)
    print(f"PRIME_PREVIEW_ICONS={generated}/{len(apps)}", flush=True)
    return generated


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--socket", type=Path, required=True)
    args = parser.parse_args()
    subdir=os.environ.get("PRIME_PREVIEW_SUBDIR","prime-on-ubuntu")
    if subdir not in ("prime-on-ubuntu","prime-visual-workspace"):
        parser.error("Unsupported preview runtime name")
    home = Path(os.environ["XDG_RUNTIME_DIR"]) / subdir
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
            materialize_app_icons(server.apps, home)
            server.files = index_user_files()
            server.launcher = launch
            server.file_opener = launch_file
            print("PRIME_UBUNTU_BRIDGE=ready;apps=%d;preview_only=1" % len(server.apps), flush=True)
            server.serve_forever(poll_interval=0.2)
    finally:
        if args.socket.is_socket():
            args.socket.unlink()

if __name__ == "__main__":
    main()
