#!/usr/bin/env bash
set -euo pipefail

product="${1:?product checkout path required}"
frozen="${2:?frozen artifact path required}"
source_root="${3:?integration source checkout path required}"
product_base="${PRODUCT_BASE:?PRODUCT_BASE required}"
parent_lock="${PARENT_LOCK_SHA256:?PARENT_LOCK_SHA256 required}"
storage_sha="${STORAGE_PATCH_SHA256:?STORAGE_PATCH_SHA256 required}"

cd "$product"
test "$(git rev-parse HEAD)" = "$product_base"
grep -Fx "CARGO_LOCK_SHA256=$parent_lock" "$frozen/handoff/MANIFEST.env"
test "$(sha256sum "$source_root/construction/p1-first-light/storage-ui.patch" | awk '{print $1}')" = "$storage_sha"

cp -a "$frozen/handoff/." .
rm -f MANIFEST.env
git apply --check "$source_root/construction/p1-first-light/storage-ui.patch"
git apply "$source_root/construction/p1-first-light/storage-ui.patch"

python3 - <<'PY'
import hashlib, json, pathlib
state={}
for p in sorted(pathlib.Path('crates').rglob('*.rs')):
    state[str(p)]=hashlib.sha256(p.read_bytes()).hexdigest()
pathlib.Path('/tmp/p1-rust-before.json').write_text(json.dumps(state, sort_keys=True))
PY

rustup toolchain install 1.97.1 --profile minimal --component rustfmt --no-self-update
cargo +1.97.1 fmt --all

python3 - <<'PY'
import hashlib, json, pathlib
before=json.loads(pathlib.Path('/tmp/p1-rust-before.json').read_text())
after={}
for p in sorted(pathlib.Path('crates').rglob('*.rs')):
    after[str(p)]=hashlib.sha256(p.read_bytes()).hexdigest()
changed=sorted(p for p in set(before)|set(after) if before.get(p)!=after.get(p))
expected=sorted([
    'crates/prime-shell/src/core_client.rs',
    'crates/prime-shell/src/visual.rs',
])
assert changed == expected, (changed, expected)
print('P1_STORAGE_FORMATTED=' + ','.join(changed))
PY

cargo +1.97.1 fmt --all -- --check
test "$(sha256sum Cargo.lock | awk '{print $1}')" = "$parent_lock"

install -D -m 0644 "$source_root/construction/p1-first-light/prime-shell.conf" image/sysusers/prime-shell.conf
install -D -m 0644 "$source_root/construction/p1-first-light/primed.service" image/systemd/primed.service
install -D -m 0644 "$source_root/construction/p1-first-light/prime-compositor.service" image/systemd/prime-compositor.service
install -D -m 0644 "$source_root/construction/p1-first-light/prime-shell.service" image/systemd/prime-shell.service
install -D -m 0644 "$source_root/construction/p1-first-light/prime-first-light-witness.service" image/systemd/prime-first-light-witness.service
install -D -m 0644 "$source_root/construction/p1-first-light/prime-shell-session" image/scripts/prime-shell-session
install -D -m 0644 "$source_root/construction/p1-first-light/prime-first-light-witness" image/scripts/prime-first-light-witness

python3 "$source_root/construction/p1-first-light/transform-final.py" "$PWD"

git diff --check
cargo +1.97.1 fmt --all -- --check
grep -F 'self.request("GET", "/v1/storage", None)' crates/prime-shell/src/core_client.rs
grep -F 'QUICK CONTROLS / SETTINGS' crates/prime-shell/src/visual.rs
grep -F 'User=prime-shell' image/systemd/prime-shell.service
grep -F "'status': 'SHELL_READY'" image/scripts/prime-first-light-witness
grep -F 'mechanical_shell_ready' tools/prove-p1-local.sh
grep -F 'prime-shell.service' image/Containerfile

git config user.name 'Prime Construction Bot'
git config user.email '65334711+jaydumisuni@users.noreply.github.com'
git add Cargo.toml Cargo.lock crates image tools/prove-p1-local.sh
git status --short
git commit -m 'feat(p1): integrate frozen Shell Core storage and First-Light image session'
proof_head="$(git rev-parse HEAD)"
printf 'PROOF_HEAD=%s\n' "$proof_head"
git push origin HEAD:refs/heads/proof/p1-first-light-final --force
