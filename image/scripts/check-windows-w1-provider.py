#!/usr/bin/env python3
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
manifest_path = ROOT / "image/windows-providers/wine-v1.json"
donor_doc = ROOT / "docs/donors/2026-09-10-windows-w1-wine.md"
containerfile = (ROOT / "image/Containerfile").read_text()
cargo = (ROOT / "crates/primed/Cargo.toml").read_text()
proof = (ROOT / "tools/prove-p1-local.sh").read_text()
source = (ROOT / "crates/primed/src/bin/prime-windows-provider-wine.rs").read_text()
wine_source = (ROOT / "crates/primed/src/windows_wine.rs").read_text()

checks = {
    "neutral binary target declared": 'name = "prime-windows-provider-w1"' in cargo and 'path = "src/bin/prime-windows-provider-wine.rs"' in cargo,
    "adapter donor path fixed": 'pub const WINDOWS_WINE_BINARY: &str = "/usr/bin/wine";' in wine_source,
    "adapter donor fingerprint covers complete runtime": all(pkg in wine_source for pkg in ["wine-core-11.0-3.fc44", "wine-common-11.0-3.fc44", "wine-mono-10.4.1-2.fc44"]),
    "manifest exists": manifest_path.is_file(),
    "donor decision exists": donor_doc.is_file(),
    "image installs complete pinned Fedora donor": all(pkg in containerfile for pkg in ["wine-core-11.0-3.fc44", "wine-common-11.0-3.fc44", "wine-mono-10.4.1-2.fc44"]),
    "image avoids Cisco OpenH264 build dependency": "--disablerepo=fedora-cisco-openh264" in containerfile,
    "image disables weak donor dependencies": "dnf -y --setopt=install_weak_deps=False --disablerepo=fedora-cisco-openh264 install" in containerfile,
    "image suppresses donor weak dependencies": "dnf -y --setopt=install_weak_deps=False --disablerepo=fedora-cisco-openh264 install" in containerfile,
    "image verifies complete pinned Fedora donor": "rpm -q" in containerfile and all(pkg in containerfile for pkg in ["wine-core-11.0-3.fc44", "wine-common-11.0-3.fc44", "wine-mono-10.4.1-2.fc44"]),
    "image verifies donor executables": "test -x /usr/bin/wine" in containerfile and "test -x /usr/bin/wine64" in containerfile,
    "image copies Prime adapter": "COPY target/release/prime-windows-provider-w1 /usr/libexec/prime/prime-windows-provider-w1" in containerfile,
    "image copies trusted provider manifest": "COPY image/windows-providers/wine-v1.json /usr/lib/prime/windows-providers/w1-default.json" in containerfile,
    "local proof requires adapter release binary": '[[ -x target/release/prime-windows-provider-w1 ]]' in proof,
}

if manifest_path.is_file():
    try:
        manifest = json.loads(manifest_path.read_text())
    except Exception:
        manifest = {}
    checks.update({
        "manifest schema exact": manifest.get("schema") == "prime.windows-provider-manifest.v1",
        "manifest provider id donor-neutral": manifest.get("provider_id") == "prime.windows.w1.default",
        "manifest revision exact": manifest.get("provider_revision") == 1,
        "manifest adapter path Prime-owned": manifest.get("adapter_path") == "/usr/libexec/prime/prime-windows-provider-w1",
        "manifest PE formats exact": manifest.get("formats") == ["PE32", "PE32+"],
        "manifest W1 arches exact": manifest.get("workload_arches") == ["x86", "x86_64"],
        "manifest does not expose donor name": "wine" not in json.dumps(manifest).lower(),
    })

if donor_doc.is_file():
    doc = donor_doc.read_text().lower()
    checks.update({
        "donor record pins complete runtime": all(pkg in doc for pkg in ["wine-core-11.0-3.fc44", "wine-common-11.0-3.fc44", "wine-mono-10.4.1-2.fc44"]),
        "donor record keeps Prime authority": "implementation donor" in doc and "not" in doc and "prime" in doc,
        "donor record names W1 boundary": "pe32" in doc and "pe32+" in doc and "x86" in doc and "x86_64" in doc,
    })

failed = [name for name, ok in checks.items() if not ok]
for name, ok in checks.items():
    print(f"{'PASS' if ok else 'FAIL'} {name}")
if failed:
    raise SystemExit("Windows W1 provider packaging contract failed: " + ", ".join(failed))
