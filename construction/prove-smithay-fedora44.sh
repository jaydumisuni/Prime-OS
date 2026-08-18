#!/usr/bin/env bash
set -euo pipefail

SOURCE_TAG="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["source_tag"])')"
PINNED_REFERENCE="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["pinned_reference"])')"
LOCKED_DIGEST="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["manifest_digest"])')"

docker pull "$SOURCE_TAG"
CURRENT_REF="$(docker image inspect "$SOURCE_TAG" --format '{{range .RepoDigests}}{{println .}}{{end}}' | grep '^quay.io/fedora/fedora-bootc@sha256:' | head -n1)"
CURRENT_DIGEST="${CURRENT_REF##*@}"

test "$CURRENT_DIGEST" = "$LOCKED_DIGEST"
docker pull "$PINNED_REFERENCE"

echo "P1_FEDORA_BASE_LOCK_MATCH=true"
echo "P1_FEDORA_PINNED_REFERENCE=$PINNED_REFERENCE"

docker build \
  --target compositor-builder \
  --build-arg PRIME_BASE_IMAGE="$PINNED_REFERENCE" \
  --build-arg TARGETARCH=amd64 \
  -f image/Containerfile \
  -t localhost/prime-compositor-image-proof:construction .

docker run --rm --entrypoint /bin/bash localhost/prime-compositor-image-proof:construction -ceu '
  test "$(rustc --version | cut -d " " -f2)" = "1.97.1"
  test -x /source/target/release/prime-compositor
  cargo metadata --locked --no-deps --format-version 1 >/dev/null
  cargo fmt --all -- --check
  cargo clippy --locked -p prime-compositor --all-targets -- -D warnings
  ! ldd /source/target/release/prime-compositor | grep -q "not found"
  echo "PRIME_COMPOSITOR_IMAGE_BUILDER=PASS"
  echo "cargo_lock_sha256=$(sha256sum /source/Cargo.lock | cut -d " " -f1)"
  echo "prime_compositor_sha256=$(sha256sum /source/target/release/prime-compositor | cut -d " " -f1)"
  rpm -q \
    libinput-devel-1.31.3-1.fc44 \
    systemd-devel-259.8-1.fc44 \
    mesa-libgbm-devel-26.1.6-1.fc44 \
    libseat-devel-0.9.3-1.fc44 \
    libxkbcommon-devel-1.13.1-2.fc44 \
    wayland-devel-1.25.0-1.fc44 \
    libglvnd-devel-1.7.0-9.fc44 \
    libdrm-devel-2.4.134-1.fc44
'
