#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PRODUCT_SHA="7e4dcbae3706055928cb25d5fd7fc629c55a27cd"
FRAME_HASH="93c9feb3310d5242db565890ff0134fa377e8da0"
INPUT_HASH="5dd07796eb337c2f205f276f08ce47fe84b4e102"
MAIN_HASH="366a89dc5d3469065cf6bb3cdaaba3c0bd998a6e"
PROTOCOLS_HASH="b847f3ecac0b72fcaa6a9c692a2e1e6784975240"
READINESS_HASH="d9a0589b977bae4326c5e7220a3c84bb2a41ac14"
FRAME_CONTRACT_HASH="27f9a5a1f846ab23381fbcb3f4391e8671233ef8"

printf 'PRIME_FRAME_DIAG_HEAD=%s\n' "$(git rev-parse HEAD)"
printf 'PRIME_FRAME_DIAG_PRODUCT_PARENT=%s\n' "$PRODUCT_SHA"
test "$(git merge-base HEAD "$PRODUCT_SHA")" = "$PRODUCT_SHA"

test "$(git hash-object crates/prime-compositor/src/frame.rs)" = "$FRAME_HASH"
test "$(git hash-object crates/prime-compositor/src/input.rs)" = "$INPUT_HASH"
test "$(git hash-object crates/prime-compositor/src/main.rs)" = "$MAIN_HASH"
test "$(git hash-object crates/prime-compositor/src/protocols.rs)" = "$PROTOCOLS_HASH"
test "$(git hash-object docs/contracts/PRIME_COMPOSITOR_READINESS_V1.md)" = "$READINESS_HASH"
test "$(git hash-object docs/contracts/PRIME_P1_FRAME_LOOP_V1.md)" = "$FRAME_CONTRACT_HASH"
printf 'PRIME_FRAME_DIAG_PRODUCT_IDENTITY=PASS\n'

section() { printf '\n===== %s =====\n' "$1"; }
run_optional() {
  printf '+ %s\n' "$*"
  "$@" 2>&1 || printf '[exit=%s]\n' "$?"
}

section 'host identity'
run_optional hostname
run_optional uname -a
run_optional id
run_optional pwd
run_optional sh -c 'cat /etc/os-release'
run_optional sh -c 'printf "tty="; tty'
run_optional sh -c 'printf "cgroup:\n"; cat /proc/self/cgroup'

section 'session environment'
printf 'XDG_RUNTIME_DIR=%s\n' "${XDG_RUNTIME_DIR-}"
printf 'XDG_SESSION_ID=%s\n' "${XDG_SESSION_ID-}"
printf 'XDG_SESSION_TYPE=%s\n' "${XDG_SESSION_TYPE-}"
printf 'XDG_VTNR=%s\n' "${XDG_VTNR-}"
printf 'WAYLAND_DISPLAY=%s\n' "${WAYLAND_DISPLAY-}"
printf 'DISPLAY=%s\n' "${DISPLAY-}"
printf 'LIBSEAT_BACKEND=%s\n' "${LIBSEAT_BACKEND-}"
run_optional loginctl list-seats --no-pager
run_optional loginctl seat-status seat0 --no-pager
run_optional loginctl list-sessions --no-pager
run_optional systemctl is-active systemd-logind
run_optional systemctl is-active seatd
run_optional sh -c 'ls -la /run/seatd.sock /run/systemd/seats /run/systemd/sessions 2>/dev/null'

section 'privilege boundary'
run_optional sudo -n true
run_optional sh -c 'sudo -n id 2>&1'

section 'DRM devices and connectors'
run_optional sh -c 'ls -la /dev/dri 2>/dev/null'
run_optional sh -c 'for f in /sys/class/drm/card*-*/status; do [ -e "$f" ] || continue; printf "%s=" "$f"; cat "$f"; done'
run_optional sh -c 'for f in /sys/class/drm/card*-*/modes; do [ -e "$f" ] || continue; echo "--- $f"; head -n 20 "$f"; done'
run_optional sh -c 'for f in /sys/class/drm/card*/device/vendor /sys/class/drm/card*/device/device; do [ -e "$f" ] || continue; printf "%s=" "$f"; cat "$f"; done'

section 'input devices'
run_optional sh -c 'ls -la /dev/input 2>/dev/null | head -n 120'
run_optional sh -c 'grep -E "^(N: Name|H: Handlers)" /proc/bus/input/devices 2>/dev/null | head -n 120'

section 'graphics/session tools'
for tool in seatd-launch libseat-list loginctl modetest kmscube weston weston-info wayland-info cage sway gtk4-demo rustc cargo gcc cc pkg-config docker podman; do
  if command -v "$tool" >/dev/null 2>&1; then
    printf '%s=%s\n' "$tool" "$(command -v "$tool")"
  else
    printf '%s=MISSING\n' "$tool"
  fi
done
run_optional sh -c "ldconfig -p 2>/dev/null | grep -E 'libseat|libinput|libEGL|libgbm|libdrm|libwayland-client' | head -n 120"

section 'container/device visibility'
run_optional docker version
run_optional podman version
run_optional sh -c 'docker info 2>/dev/null | sed -n "1,80p"'
run_optional sh -c 'podman info 2>/dev/null | sed -n "1,80p"'

section 'kernel graphics ownership'
run_optional sh -c 'for p in /sys/class/drm/card*/device/driver; do [ -e "$p" ] || continue; printf "%s -> " "$p"; readlink -f "$p"; done'
run_optional sh -c 'lsmod | grep -E "^(i915|xe|amdgpu|nouveau|nvidia)" || true'

printf '\nPRIME_FRAME_HOST_DIAGNOSTIC=CAPTURED_NOT_PROOF\n'
printf 'FRAME_LOOP_READY remains unclaimed; this job intentionally exits non-zero after evidence capture.\n'
exit 42
