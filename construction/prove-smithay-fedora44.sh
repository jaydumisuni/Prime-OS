#!/usr/bin/env bash
set -euo pipefail

BASE_IMAGE="quay.io/fedora/fedora-bootc:44-x86_64@sha256:130e3ea9633a00381ba8ea9b168fd04a4f90161eaa38af23c9eb927a0f1e5074"

docker pull "$BASE_IMAGE"
docker run --rm \
  -e CARGO_TERM_COLOR=never \
  -v "$PWD:/work" \
  -w /work \
  "$BASE_IMAGE" \
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

    curl --proto "=https" --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal --default-toolchain 1.97.1
    . "$HOME/.cargo/env"

    test "$(rustc --version | awk "{print \\$2}")" = "1.97.1"
    cargo metadata --locked --no-deps --format-version 1 >/dev/null
    cargo build --locked -p prime-compositor

    echo "PRIME_SMITHAY_FEDORA44_BUILD=PASS"
    echo "rustc=$(rustc --version)"
    echo "cargo=$(cargo --version)"
    echo "cargo_lock_sha256=$(sha256sum Cargo.lock | awk "{print \\$1}")"
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
  '
