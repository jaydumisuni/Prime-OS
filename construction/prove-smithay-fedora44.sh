#!/usr/bin/env bash
set -euo pipefail

PINNED_REFERENCE="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["pinned_reference"])')"
LOCKED_DIGEST="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["manifest_digest"])')"

docker pull "$PINNED_REFERENCE"

echo "P1_RENDERER_IMAGE_RUNTIME_PROBE=START"
echo "P1_FEDORA_PINNED_REFERENCE=$PINNED_REFERENCE"
echo "P1_FEDORA_LOCKED_DIGEST=$LOCKED_DIGEST"

docker run --rm "$PINNED_REFERENCE" bash -ceu '
  dnf -y install \
    libglvnd-egl-1.7.0-9.fc44 \
    libinput-1.31.3-1.fc44 \
    libseat-0.9.3-1.fc44 \
    mesa-dri-drivers-26.1.6-1.fc44 \
    mesa-libEGL-26.1.6-1.fc44 \
    mesa-libgbm-26.1.6-1.fc44

  rpm -q \
    libdrm-2.4.134-1.fc44 \
    libglvnd-egl-1.7.0-9.fc44 \
    libinput-1.31.3-1.fc44 \
    libseat-0.9.3-1.fc44 \
    mesa-dri-drivers-26.1.6-1.fc44 \
    mesa-libEGL-26.1.6-1.fc44 \
    mesa-libgbm-26.1.6-1.fc44

  test -e /usr/lib64/libEGL.so.1
  test -e /usr/lib64/libEGL_mesa.so.0
  test -e /usr/lib64/libgbm.so.1
  test -e /usr/lib64/libdrm.so.2
  test -e /usr/lib64/dri/iris_dri.so

  test "$(rpm -qf /usr/lib64/libEGL.so.1)" = "libglvnd-egl-1.7.0-9.fc44.x86_64"
  test "$(rpm -qf /usr/lib64/libEGL_mesa.so.0)" = "mesa-libEGL-26.1.6-1.fc44.x86_64"
  test "$(rpm -qf /usr/lib64/libgbm.so.1)" = "mesa-libgbm-26.1.6-1.fc44.x86_64"
  test "$(rpm -qf /usr/lib64/libdrm.so.2)" = "libdrm-2.4.134-1.fc44.x86_64"
  test "$(rpm -qf /usr/lib64/dri/iris_dri.so)" = "mesa-dri-drivers-26.1.6-1.fc44.x86_64"

  echo "renderer_runtime_packages_begin"
  rpm -q \
    libdrm \
    libglvnd-egl \
    libinput \
    libseat \
    mesa-dri-drivers \
    mesa-libEGL \
    mesa-libgbm
  echo "renderer_runtime_packages_end"
'

echo "P1_RENDERER_IMAGE_RUNTIME_PROBE=PASS"
