#!/usr/bin/env bash
set -euo pipefail

LOCK_FILE="image/fedora-base.lock.json"
test -f "$LOCK_FILE"
SOURCE_TAG="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["source_tag"])')"
PINNED_REFERENCE="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["pinned_reference"])')"
LOCKED_DIGEST="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["manifest_digest"])')"

docker pull "$SOURCE_TAG"
CURRENT_REF="$(docker image inspect "$SOURCE_TAG" --format '{{range .RepoDigests}}{{println .}}{{end}}' | grep '^quay.io/fedora/fedora-bootc@sha256:' | head -n1)"
CURRENT_DIGEST="${CURRENT_REF##*@}"

test -n "$CURRENT_REF"
test -n "$CURRENT_DIGEST"

echo "P1_FEDORA_SOURCE_TAG=$SOURCE_TAG"
echo "P1_FEDORA_PINNED_REFERENCE=$PINNED_REFERENCE"
echo "P1_FEDORA_LOCKED_DIGEST=$LOCKED_DIGEST"
echo "P1_FEDORA_CURRENT_REF=$CURRENT_REF"
echo "P1_FEDORA_CURRENT_DIGEST=$CURRENT_DIGEST"
if [[ "$CURRENT_DIGEST" != "$LOCKED_DIGEST" ]]; then
  echo "P1_FEDORA_BASE_LOCK_MATCH=false" >&2
  echo "Fedora source tag no longer resolves to the locked Prime P1 substrate" >&2
  exit 1
fi
echo "P1_FEDORA_BASE_LOCK_MATCH=true"
docker pull "$PINNED_REFERENCE"

docker run --rm \
  -e CARGO_TERM_COLOR=never \
  -e RUSTUP_HOME=/tmp/prime-rustup \
  -e CARGO_HOME=/tmp/prime-cargo \
  -v "$PWD:/work" \
  -w /work \
  "$PINNED_REFERENCE" \
  bash -ceu '
    dnf -y install \
      ca-certificates \
      curl \
      gcc \
      gcc-c++ \
      make \
      pkgconf-pkg-config \
      "pkgconfig(libinput)" \
      "pkgconfig(libudev)" \
      "pkgconfig(gbm)" \
      "pkgconfig(libseat)" \
      "pkgconfig(xkbcommon)" \
      "pkgconfig(wayland-server)" \
      "pkgconfig(egl)" \
      "pkgconfig(glesv2)" \
      "pkgconfig(libdrm)"

    rm -rf "$RUSTUP_HOME" "$CARGO_HOME"
    mkdir -p "$RUSTUP_HOME" "$CARGO_HOME"
    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.97.1 --no-modify-path
    export PATH="$CARGO_HOME/bin:$PATH"
    rustup component add rustfmt clippy --toolchain 1.97.1

    test "$(rustc --version | cut -d " " -f2)" = "1.97.1"
    cargo metadata --locked --no-deps --format-version 1 >/dev/null
    cargo fmt --all -- --check
    cargo clippy --locked -p prime-compositor --all-targets -- -D warnings
    cargo build --locked --release -p prime-compositor

    echo "PRIME_COMPOSITOR_FEDORA44_BUILD=PASS"
    echo "rustc=$(rustc --version)"
    echo "cargo=$(cargo --version)"
    echo "cargo_lock_sha256=$(sha256sum Cargo.lock | cut -d " " -f1)"
    echo "prime_compositor_sha256=$(sha256sum target/release/prime-compositor | cut -d " " -f1)"
    echo "smithay_version=0.7.0"
    echo "native_pkgconfig_providers_begin"
    for capability in \
      "pkgconfig(libinput)" \
      "pkgconfig(libudev)" \
      "pkgconfig(gbm)" \
      "pkgconfig(libseat)" \
      "pkgconfig(xkbcommon)" \
      "pkgconfig(wayland-server)" \
      "pkgconfig(egl)" \
      "pkgconfig(glesv2)" \
      "pkgconfig(libdrm)"; do
      printf "%s=" "$capability"
      rpm -q --whatprovides "$capability" --qf "%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}\n" | paste -sd, -
    done
    echo "native_pkgconfig_providers_end"

    echo "runtime_library_owners_begin"
    while read -r first second third rest; do
      path=""
      if [[ "$first" == /* ]]; then
        path="$first"
      elif [[ "${second:-}" == "=>" && "${third:-}" == /* ]]; then
        path="$third"
      fi
      if [[ -n "$path" ]]; then
        printf "%s=" "$path"
        rpm -qf "$path" --qf "%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}\n"
      fi
    done < <(ldd target/release/prime-compositor)
    echo "runtime_library_owners_end"
  '
