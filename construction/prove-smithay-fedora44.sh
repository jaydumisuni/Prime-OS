#!/usr/bin/env bash
set -euo pipefail

PINNED_REFERENCE="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["pinned_reference"])')"
LOCKED_DIGEST="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json", encoding="utf-8"))["manifest_digest"])')"

docker pull "$PINNED_REFERENCE"

echo "P1_RENDERER_RUNTIME_PROBE=START"
echo "P1_FEDORA_PINNED_REFERENCE=$PINNED_REFERENCE"
echo "P1_FEDORA_LOCKED_DIGEST=$LOCKED_DIGEST"

docker run --rm "$PINNED_REFERENCE" bash -ceu '
  echo "installed_runtime_packages_begin"
  for package in \
    libdrm \
    mesa-libgbm \
    libglvnd-egl \
    mesa-libEGL \
    mesa-dri-drivers \
    libinput \
    libseat; do
    if rpm -q "$package" >/dev/null 2>&1; then
      rpm -q "$package" --qf "%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}\n"
    else
      echo "$package=NOT_INSTALLED"
    fi
  done
  echo "installed_runtime_packages_end"

  echo "renderer_library_owners_begin"
  for path in \
    /usr/lib64/libEGL.so.1 \
    /usr/lib64/libEGL_mesa.so.0 \
    /usr/lib64/libgbm.so.1 \
    /usr/lib64/libdrm.so.2 \
    /usr/lib64/dri/iris_dri.so; do
    if test -e "$path"; then
      printf "%s=" "$path"
      rpm -qf "$path" --qf "%{NAME}-%{VERSION}-%{RELEASE}.%{ARCH}\n"
    else
      echo "$path=MISSING"
    fi
  done
  echo "renderer_library_owners_end"
'

echo "P1_RENDERER_RUNTIME_PROBE=PASS"
