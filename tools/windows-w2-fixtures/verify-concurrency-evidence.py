#!/usr/bin/env python3
import json, sys
from pathlib import Path
if len(sys.argv) != 2:
    raise SystemExit('usage: verify-concurrency-evidence.py EVIDENCE.json')
e=json.loads(Path(sys.argv[1]).read_text())
checks={}
checks['schema']=e.get('schema')=='prime.windows-w2-concurrency-proof.v1'
same=e['same_application']
checks['same-app one installed one satisfied']=sorted(same['results'])==['ALREADY_SATISFIED','INSTALLED']
checks['same-app one mutation window']=len(same['mutation_windows'])==1 and same['marker_count']==1
diff=e['different_applications']
a,b=diff['windows']
checks['different-app both installed']=diff['results']==['INSTALLED','INSTALLED']
checks['different-app windows overlap']=max(a['start'],b['start']) < min(a['end'],b['end'])
checks['different-app exact final payloads']=a['final_sha256']==b['final_sha256']==e['expected_final_sha256']
cross=e['cross_prefix']
checks['B unchanged during A recovery']=cross['b_before_a_recovery']==cross['b_after_a_recovery']
checks['A unchanged during B recovery']=cross['a_before_b_recovery']==cross['a_after_b_recovery']
failed=[]
for label,ok in checks.items():
    print(('PASS' if ok else 'FAIL')+': '+label)
    if not ok: failed.append(label)
if failed: raise SystemExit(1)
print(f'PASS_TOTAL={len(checks)}')
