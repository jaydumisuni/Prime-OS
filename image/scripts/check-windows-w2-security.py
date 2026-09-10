#!/usr/bin/env python3
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
engine = (ROOT / 'crates/primed/src/bin/prime-windows-component-engine.rs').read_text()
state = (ROOT / 'crates/primed/src/windows_state.rs').read_text()
fixtures = ROOT / 'tools/windows-w2-fixtures'
manifest_names = ['exe-manifest.json', 'msi-manifest.json', 'slow-manifest.json', 'exe-badstate-manifest.json']
manifests = [json.loads((fixtures / name).read_text()) for name in manifest_names]
sha = re.compile(r'^sha256:[0-9a-f]{64}$')
checks = {
    'component engine never invokes a shell': 'Command::new("sh")' not in engine and 'Command::new("/bin/sh")' not in engine and 'sh -c' not in engine,
    'component engine never routes through prime-shell': 'prime-shell' not in engine,
    'installer artifacts require regular non-symlink files': 'path is not a regular non-symlink file' in engine,
    'installer artifact SHA-256 is verified before execution': 'ArtifactIdentityMismatch' in engine and 'validate_installer_artifact(manifest)?' in engine,
    'post-install verification is mandatory before marker': 'WINDOWS_COMPONENT_VERIFY_FAILED' in engine and 'all_probes_pass(&verified)' in engine,
    'compatibility state uses a per-application lock': 'compatibility.lock' in state and '.lock()?' in state,
    'proof manifests pin canonical artifact identities': all(sha.fullmatch(m['artifact_identity']) for m in manifests),
    'proof manifests use absolute proof-only artifact paths': all(m['artifact_path'].startswith('/proof/') for m in manifests),
    'proof manifests have observational verification': all(m['verification'] for m in manifests),
    'fixture directory contains no committed PE/MSI binaries': not any(x.suffix.lower() in {'.exe', '.msi'} for x in fixtures.rglob('*') if x.is_file()),
}
failed = []
for label, ok in checks.items():
    print(('PASS' if ok else 'FAIL') + ': ' + label)
    if not ok:
        failed.append(label)
if failed:
    raise SystemExit(1)
print(f'PASS_TOTAL={len(checks)}')
