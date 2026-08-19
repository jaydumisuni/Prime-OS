#!/usr/bin/env bash
set -euo pipefail

bash -n tools/prove-p1-local.sh

for required in \
  'test -x /usr/libexec/prime/prime-compositor' \
  'libglvnd-egl-1.7.0-9.fc44' \
  'mesa-dri-drivers-26.1.6-1.fc44' \
  'mesa-libEGL-26.1.6-1.fc44' \
  'mesa-libgbm-26.1.6-1.fc44' \
  '/usr/lib64/libEGL.so.1' \
  '/usr/lib64/libEGL_mesa.so.0' \
  '/usr/lib64/libgbm.so.1' \
  '/usr/lib64/libdrm.so.2' \
  '/usr/lib64/dri/iris_dri.so' \
  'ldd /usr/libexec/prime/prime-compositor' \
  'Usage: prime-compositor [--probe]' \
  'multi-user.target.wants/prime-compositor.service'; do
  grep -F -- "$required" tools/prove-p1-local.sh >/dev/null
 done

test "$(grep -Fc 'cargo clippy --locked --workspace --exclude prime-compositor --all-targets -- -D warnings' tools/prove-p1-local.sh)" -eq 1
test "$(grep -Fc 'cargo test --locked --workspace --exclude prime-compositor' tools/prove-p1-local.sh)" -eq 1

echo 'P1_COMPOSITOR_PROOF_SURFACE=PASS'
