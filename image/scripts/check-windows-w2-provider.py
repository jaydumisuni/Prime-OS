#!/usr/bin/env python3
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
containerfile = (ROOT / 'image/Containerfile').read_text()
proof = (ROOT / 'tools/prove-p1-local.sh').read_text()
manifest_path = ROOT / 'image/windows-components/runtime.wine-mono/revisions/00000000000000000001.json'
manifest = json.loads(manifest_path.read_text()) if manifest_path.is_file() else {}
checks = {
    'trusted Wine Mono manifest exists': manifest_path.is_file(),
    'trusted Wine Mono identity exact': manifest.get('component_id') == 'runtime.wine-mono' and manifest.get('revision') == 1 and manifest.get('digest') == 'sha256:47526839b2fc8c981330d77d2c3b6a4b2528b5cff2ddd547f33aef0671782689',
    'trusted Wine Mono is BUILTIN runtime': manifest.get('kind') == 'RUNTIME' and manifest.get('installer_kind') == 'BUILTIN',
    'trusted Wine Mono pins file and registry probes': [p.get('kind') for p in manifest.get('verification', [])] == ['FILE_SHA256', 'REGISTRY_VALUE_EQUALS'],
    'W3 boundary remains explicit': any('managed application execution is W3' in x for x in manifest.get('limitations', [])),
    'image copies W2 component engine': 'COPY target/release/prime-windows-component-engine /usr/libexec/prime/prime-windows-component-engine' in containerfile,
    'image copies trusted component registry': 'COPY image/windows-components /usr/lib/prime/windows-components' in containerfile,
    'image validates W2 component engine executable': 'test -x /usr/libexec/prime/prime-windows-component-engine' in containerfile,
    'image validates trusted Wine Mono manifest': 'runtime.wine-mono/revisions/00000000000000000001.json' in containerfile,
    'local proof requires W2 component engine release binary': '[[ -x target/release/prime-windows-component-engine ]]' in proof,
}
failed=[]
for label, ok in checks.items():
    print(('PASS' if ok else 'FAIL') + ': ' + label)
    if not ok: failed.append(label)
if failed:
    raise SystemExit(1)
print(f'PASS_TOTAL={len(checks)}')
