#!/usr/bin/env bash
# Starts from labwc --session after Wayland and nested Xwayland displays are assigned.
set -Eeuo pipefail
: "$XDG_RUNTIME_DIR" "$WAYLAND_DISPLAY" "$PRIME_UBUNTU_TOOLS" "$PRIME_SHELL_BIN" "$PRIME_CORE_SOCKET"
if [[ -e "$PRIME_CORE_SOCKET" ]]; then
  echo "Preview socket unexpectedly occupied: $PRIME_CORE_SOCKET" >&2
  exit 2
fi
python3 "$PRIME_UBUNTU_TOOLS/bridge.py" --socket "$PRIME_CORE_SOCKET" >>"$XDG_RUNTIME_DIR/prime-on-ubuntu/bridge.log" 2>&1 &
bridge=$!
cleanup() {
  kill "$bridge" 2>/dev/null || true
  wait "$bridge" 2>/dev/null || true
}
trap cleanup EXIT
for ((i=0; i<40; i++)); do
  if [[ -S "$PRIME_CORE_SOCKET" ]]; then break; fi
  if ! kill -0 "$bridge" 2>/dev/null; then
    echo "Ubuntu preview bridge failed to start" >&2
    exit 3
  fi
  sleep 0.1
done
[[ -S "$PRIME_CORE_SOCKET" ]] || { echo "Bridge startup timeout" >&2; exit 3; }
echo "PRIME_ON_UBUNTU_WAYLAND=$WAYLAND_DISPLAY;DISPLAY=$DISPLAY;bridge_child=1" >&2
# Foreground Shell. Labwc -S exits on this process's exit.
"$PRIME_SHELL_BIN" >>"$XDG_RUNTIME_DIR/prime-on-ubuntu/shell.log" 2>&1
