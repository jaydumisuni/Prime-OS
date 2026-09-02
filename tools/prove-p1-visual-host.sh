#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

RUN_DIR="${PRIME_P1_VISUAL_HOST_ROOT:-/var/tmp/prime-p1-visual-host-$(date -u +%Y%m%dT%H%M%SZ)-$$}"
mkdir -p "$RUN_DIR"
LOG="$RUN_DIR/host-proof.log"
exec > >(tee "$LOG") 2>&1

fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "missing required tool: $1"; }

for tool in git cargo rustc grep awk; do need "$tool"; done
[[ "$(rustc --version | awk '{print $2}')" == "1.97.1" ]] || fail "Rust compiler is not pinned to 1.97.1"
[[ "$(git branch --show-current)" == "design/p1-first-light-visual" ]] || fail "visual proof must run on design/p1-first-light-visual"

cargo metadata --locked --no-deps --format-version 1 > "$RUN_DIR/cargo-metadata.json"
cargo fmt --all -- --check
cargo test --workspace --locked
cargo clippy --locked --workspace --all-targets -- -D warnings
printf 'P1_VISUAL_HOST_STATIC=PASS\n'

cargo build --release --locked -p prime-compositor -p prime-shell
[[ -x target/release/prime-compositor ]] || fail "release prime-compositor missing"
[[ -x target/release/prime-shell ]] || fail "release prime-shell missing"
[[ "$(target/release/prime-shell --font-probe)" == "PRIME_SHELL_FONT_PROBE=Noto Sans" ]] || fail "Prime production font probe failed"
grep -Fq 'prime.shell.background' crates/prime-shell/src/main.rs
grep -Fq 'prime.shell.rail' crates/prime-shell/src/main.rs
grep -Fq 'prime.shell.orb' crates/prime-shell/src/main.rs
grep -Fq 'prime.shell.quick-controls' crates/prime-shell/src/main.rs
grep -Fq 'Prime glass effects are in fallback mode' crates/prime-compositor/src/effects.rs
printf 'P1_VISUAL_HOST_BUILD=PASS\n'

if grep -Eqi '(^|[-_])(gtk|gtk4|qt|qmetaobject|cosmic|iced|egui|slint)([-_]|$)' Cargo.lock; then
  fail "borrowed desktop/UI toolkit dependency detected in Cargo.lock"
fi
if grep -Eqi '(gnome-shell|plasma-desktop|cosmic|gtk[234]?|qt[56]?|display-manager)' image/Containerfile; then
  fail "borrowed desktop/runtime package detected in image/Containerfile"
fi
grep -Fq 'google-noto-sans-fonts-20251201-2.fc44' image/Containerfile
printf 'P1_VISUAL_HOST_NO_BORROWED_DESKTOP=PASS\n'
printf 'P1_VISUAL_HOST_EVIDENCE=%s\n' "$RUN_DIR"
