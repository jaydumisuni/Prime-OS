#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

PODMAN=(sudo -n podman)
WORK_ROOT="${PRIME_P1_WORK_ROOT:-/var/tmp/prime-p1-local-proof}"
RUN_DIR="$WORK_ROOT/run"
OUTPUT_DIR="$RUN_DIR/output"
REPORT="$RUN_DIR/prime-p1-local-proof.json"
SERIAL_LOG="$RUN_DIR/prime-serial.log"
OVERLAY="$RUN_DIR/prime-p1-boot-overlay.qcow2"
MOUNT_ESP="$RUN_DIR/mnt-esp"
MOUNT_XBOOTLDR="$RUN_DIR/mnt-xbootldr"
MOUNT_ROOT="$RUN_DIR/mnt-root"
NBD_DEV="${PRIME_P1_NBD_DEVICE:-/dev/nbd0}"

BWRAP_POLICY="/etc/apparmor.d/bwrap-userns-restrict"
LSBLK_POLICY="/etc/apparmor.d/lsblk"
EXPECTED_BWRAP_POLICY_SHA256="d61facde27707b9c47ffe47921b7273e788784484cb5530eb819e6daac1f1990"
EXPECTED_LSBLK_POLICY_SHA256="6b3097d4b9fc10c34bc593c5fed2c95af86d619eb68fa7611f98b55cec841569"
APPARMOR_WINDOW_OPEN=0

mkdir -p "$RUN_DIR" "$OUTPUT_DIR" "$MOUNT_ESP" "$MOUNT_XBOOTLDR" "$MOUNT_ROOT"

log() { printf '\n==> %s\n' "$*"; }
fail() { printf 'ERROR: %s\n' "$*" >&2; exit 1; }
need() { command -v "$1" >/dev/null 2>&1 || fail "missing required tool: $1"; }

cleanup_nbd() {
  sudo -n umount "$MOUNT_ESP" 2>/dev/null || true
  sudo -n umount "$MOUNT_XBOOTLDR" 2>/dev/null || true
  sudo -n umount "$MOUNT_ROOT" 2>/dev/null || true
  sudo -n qemu-nbd --disconnect "$NBD_DEV" >/dev/null 2>&1 || true
}

restore_apparmor_profiles() {
  if [[ "${APPARMOR_WINDOW_OPEN:-0}" -eq 1 ]]; then
    sudo -n apparmor_parser -r "$BWRAP_POLICY" >/dev/null 2>&1 || true
    sudo -n apparmor_parser -r "$LSBLK_POLICY" >/dev/null 2>&1 || true
    APPARMOR_WINDOW_OPEN=0
  fi
}

cleanup() {
  restore_apparmor_profiles
  cleanup_nbd
}
trap cleanup EXIT

log "P1 local proof preflight"
[[ "$(uname -m)" == "x86_64" ]] || fail "P1 proof requires x86_64"
for tool in git python3 rustc cargo podman skopeo qemu-img qemu-system-x86_64 qemu-nbd sgdisk lsblk findmnt timeout sha256sum objcopy apparmor_parser; do need "$tool"; done
sudo -n true || fail "non-interactive sudo is required for rootful Podman/NBD proof"
[[ -e /dev/fuse ]] || fail "/dev/fuse is unavailable"
[[ -f image/fedora-base.lock.json ]] || fail "missing Fedora base lock"
[[ -f image/image-builder.lock.json ]] || fail "missing image-builder lock"
[[ -f image/Containerfile ]] || fail "missing image/Containerfile"
[[ -f image/prime-os-release ]] || fail "missing Prime product identity"
[[ -f image/scripts/prepare-uki-cmdlines.py ]] || fail "missing UKI command-line helper"
[[ -f image/scripts/check-uki-contract.py ]] || fail "missing UKI contract checker"
[[ -x /usr/bin/env ]] || fail "invalid host environment"
[[ -f "$BWRAP_POLICY" ]] || fail "missing canonical bwrap AppArmor policy"
[[ -f "$LSBLK_POLICY" ]] || fail "missing canonical lsblk AppArmor policy"
[[ "$(sha256sum "$BWRAP_POLICY" | awk '{print $1}')" == "$EXPECTED_BWRAP_POLICY_SHA256" ]] || fail "bwrap AppArmor policy drift"
[[ "$(sha256sum "$LSBLK_POLICY" | awk '{print $1}')" == "$EXPECTED_LSBLK_POLICY_SHA256" ]] || fail "lsblk AppArmor policy drift"
sudo -n cat /sys/kernel/security/apparmor/profiles | grep -E '^(unpriv_bwrap|bwrap|lsblk) \(enforce\)$' | wc -l | grep -Fx 3 >/dev/null || fail "expected bwrap/unpriv_bwrap/lsblk AppArmor profiles in enforce mode"

SOURCE_REVISION="$(git rev-parse HEAD)"
CREATED_AT="$(git show -s --format=%cI HEAD)"
GENERATION_ID="p1-first-light-${SOURCE_REVISION:0:12}"
BASE_IMAGE="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json"))["pinned_reference"])')"
BASE_DIGEST="$(python3 -c 'import json; print(json.load(open("image/fedora-base.lock.json"))["manifest_digest"])')"
BUILDER_REF="$(python3 -c 'import json; print(json.load(open("image/image-builder.lock.json"))["pinned_reference"])')"
BUILDER_DIGEST="$(python3 -c 'import json; print(json.load(open("image/image-builder.lock.json"))["manifest_digest"])')"
BUILDER_VERSION="$(python3 -c 'import json; print(json.load(open("image/image-builder.lock.json"))["version"])')"

case "$BASE_DIGEST" in sha256:????????????????????????????????????????????????????????????????) ;; *) fail "invalid Fedora base digest" ;; esac
case "$BUILDER_DIGEST" in sha256:????????????????????????????????????????????????????????????????) ;; *) fail "invalid image-builder digest" ;; esac
[[ "$(rustc --version | awk '{print $2}')" == "1.97.1" ]] || fail "Rust compiler is not pinned 1.97.1"

log "Verify locked external inputs"
BASE_INSPECT="$(skopeo inspect --override-os linux --override-arch amd64 --format '{{.Digest}} {{.Os}} {{.Architecture}}' "docker://$BASE_IMAGE")"
[[ "$BASE_INSPECT" == "$BASE_DIGEST linux amd64" ]] || fail "Fedora base lock mismatch: $BASE_INSPECT"
BUILDER_INSPECT="$(skopeo inspect --override-os linux --override-arch amd64 --format '{{.Digest}} {{.Os}} {{.Architecture}}' "docker://$BUILDER_REF")"
[[ "$BUILDER_INSPECT" == "$BUILDER_DIGEST linux amd64" ]] || fail "image-builder lock mismatch: $BUILDER_INSPECT"

log "Prime Core and recovery locked proof"
cargo metadata --locked --no-deps --format-version 1 >/dev/null
cargo fmt --all -- --check
cargo clippy --locked --workspace --exclude prime-compositor --exclude prime-shell --all-targets -- -D warnings
cargo test --locked --workspace --exclude prime-compositor --exclude prime-shell
cargo build --locked --release -p primed
[[ -x target/release/primed ]] || fail "primed release binary missing"
[[ -x target/release/prime-recovery ]] || fail "prime-recovery release binary missing"

log "Verify locked Fedora bootc filesystem"
"${PODMAN[@]}" pull "$BASE_IMAGE"
"${PODMAN[@]}" run --rm --entrypoint /bin/bash "$BASE_IMAGE" -ceu \
  'grep -q "^ID=fedora$" /usr/lib/os-release && grep -q "^VERSION_ID=44$" /usr/lib/os-release && test -x /usr/sbin/bootc && test -x /usr/bin/systemctl'

log "Build canonical sealed Prime rootfs"
"${PODMAN[@]}" build \
  --cap-add=all \
  --device /dev/fuse \
  --security-opt label=disable \
  --platform linux/amd64 \
  --target sealed-rootfs \
  --build-arg PRIME_BASE_IMAGE="$BASE_IMAGE" \
  --build-arg PRIME_BASE_DIGEST="$BASE_DIGEST" \
  --build-arg TARGETARCH=amd64 \
  --build-arg PRIME_GENERATION_ID="$GENERATION_ID" \
  --build-arg PRIME_SOURCE_REVISION="$SOURCE_REVISION" \
  --build-arg PRIME_CREATED_AT="$CREATED_AT" \
  --build-arg PRIME_BOOT_ATTEMPT_LIMIT=3 \
  -f image/Containerfile \
  -t localhost/prime-os:p1-rootfs .

log "Compute canonical OCI-storage Composefs digest from sealed rootfs"
CANONICAL_DIGEST="$("${PODMAN[@]}" run --rm \
  --privileged \
  --security-opt label=disable \
  --tmpfs /var/tmp:size=8g \
  -v /var/lib/containers/storage:/var/lib/containers/storage \
  localhost/prime-os:p1-rootfs \
  bootc container compute-composefs-digest-from-storage localhost/prime-os:p1-rootfs | tail -n1)"
case "$CANONICAL_DIGEST" in ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????) ;; *) fail "invalid canonical Composefs digest: $CANONICAL_DIGEST" ;; esac
printf 'sealed rootfs digest: %s\n' "$CANONICAL_DIGEST"

log "Build Prime once with canonical normal and recovery UKI seals"
"${PODMAN[@]}" build \
  --cap-add=all \
  --device /dev/fuse \
  --security-opt label=disable \
  --platform linux/amd64 \
  --build-arg PRIME_BASE_IMAGE="$BASE_IMAGE" \
  --build-arg PRIME_BASE_DIGEST="$BASE_DIGEST" \
  --build-arg TARGETARCH=amd64 \
  --build-arg PRIME_GENERATION_ID="$GENERATION_ID" \
  --build-arg PRIME_SOURCE_REVISION="$SOURCE_REVISION" \
  --build-arg PRIME_CREATED_AT="$CREATED_AT" \
  --build-arg PRIME_BOOT_ATTEMPT_LIMIT=3 \
  --build-arg PRIME_COMPOSEFS_DIGEST="$CANONICAL_DIGEST" \
  -f image/Containerfile \
  -t localhost/prime-os:p1 .

log "Prove sealed-rootfs digest equals both embedded UKI digests"
FINAL_DIGEST="$("${PODMAN[@]}" run --rm \
  --privileged \
  --security-opt label=disable \
  --tmpfs /var/tmp:size=8g \
  -v /var/lib/containers/storage:/var/lib/containers/storage \
  localhost/prime-os:p1 \
  bootc container compute-composefs-digest-from-storage localhost/prime-os:p1 | tail -n1)"
case "$FINAL_DIGEST" in ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????) ;; *) fail "invalid final image storage digest: $FINAL_DIGEST" ;; esac
[[ "$FINAL_DIGEST" == "$CANONICAL_DIGEST" ]] || fail "final image Composefs digest does not match sealed rootfs digest"
UKI_DIGESTS="$("${PODMAN[@]}" run --rm -e EXPECTED_DIGEST="$CANONICAL_DIGEST" --entrypoint /bin/bash localhost/prime-os:p1 -ceu '
  normal="$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.efi" ! -name "*.recovery.efi" -print -quit)"
  recovery="/boot/EFI/Prime/prime-recovery-${EXPECTED_DIGEST}.efi"
  test -n "$normal"
  test -s "$recovery"
  test "$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.efi" ! -name "*.recovery.efi" | wc -l)" -eq 1
  test "$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.recovery.efi" | wc -l)" -eq 0
  grep -aoE "composefs=[0-9a-f]{128}" "$normal" | head -n1 | cut -d= -f2
  grep -aoE "composefs=[0-9a-f]{128}" "$recovery" | head -n1 | cut -d= -f2
')"
NORMAL_EMBEDDED_DIGEST="$(printf '%s\n' "$UKI_DIGESTS" | sed -n '1p')"
RECOVERY_EMBEDDED_DIGEST="$(printf '%s\n' "$UKI_DIGESTS" | sed -n '2p')"
[[ -n "$NORMAL_EMBEDDED_DIGEST" ]] || fail "normal UKI Composefs digest missing"
[[ -n "$RECOVERY_EMBEDDED_DIGEST" ]] || fail "recovery UKI Composefs digest missing"
[[ "$NORMAL_EMBEDDED_DIGEST" == "$CANONICAL_DIGEST" ]] || fail "normal UKI Composefs digest does not match sealed rootfs digest"
[[ "$RECOVERY_EMBEDDED_DIGEST" == "$CANONICAL_DIGEST" ]] || fail "recovery UKI Composefs digest does not match sealed rootfs digest"
printf 'sealed rootfs digest: %s\nfinal image digest:   %s\nnormal UKI digest:    %s\nrecovery UKI digest:  %s\n' "$CANONICAL_DIGEST" "$FINAL_DIGEST" "$NORMAL_EMBEDDED_DIGEST" "$RECOVERY_EMBEDDED_DIGEST"

log "Inspect final Prime image and recovery contract"
"${PODMAN[@]}" run --rm \
  -e EXPECTED_SOURCE_REVISION="$SOURCE_REVISION" \
  -e EXPECTED_CREATED_AT="$CREATED_AT" \
  -e EXPECTED_GENERATION_ID="$GENERATION_ID" \
  -e EXPECTED_BASE_DIGEST="$BASE_DIGEST" \
  -e EXPECTED_DIGEST="$CANONICAL_DIGEST" \
  --entrypoint /bin/bash localhost/prime-os:p1 -ceu '
    test -x /usr/libexec/prime/primed
    test -x /usr/libexec/prime/prime-recovery
    test -x /usr/libexec/prime/prime-compositor
    test -x /usr/libexec/prime/prime-shell
    test -x /usr/libexec/prime/prime-shell-session
    test -x /usr/libexec/prime/prime-first-light-witness
    test -f /usr/lib/sysusers.d/prime-shell.conf
    test -x /usr/sbin/bootc
    test -x /usr/sbin/systemd-run
    test -x /usr/bin/ukify
    test -f /usr/lib/systemd/boot/efi/systemd-bootx64.efi
    test -f /usr/lib/systemd/boot/efi/linuxx64.efi.stub
    test -f /usr/lib/prime/generation-seed.json
    test -f /usr/lib/prime/substrate-release.json
    test -f /usr/lib/bootc/install/10-prime.toml
    test -f /usr/lib/systemd/system/prime-recovery.service
    test -f /usr/lib/systemd/system/prime-recovery.target
    test ! -e /kernel
    test -L /etc/systemd/system/multi-user.target.wants/primed.service
    test ! -e /etc/systemd/system/timers.target.wants/bootc-fetch-apply-updates.timer
    ! rpm -q bootupd >/dev/null 2>&1
    rpm -q \
      libdrm-2.4.134-1.fc44 \
      libglvnd-egl-1.7.0-9.fc44 \
      libinput-1.31.3-1.fc44 \
      libseat-0.9.3-1.fc44 \
      libwayland-client-1.25.0-1.fc44 \
      libxkbcommon-1.13.1-2.fc44 \
      mesa-dri-drivers-26.1.7-1.fc44 \
      mesa-libEGL-26.1.7-1.fc44 \
      mesa-libgbm-26.1.7-1.fc44 \
      systemd-boot-unsigned-259.8-1.fc44 \
      systemd-ukify-259.8-1.fc44
    grep -q "^ID=prime$" /usr/lib/os-release
    grep -q "^PRETTY_NAME=\"Prime OS P1 First Light\"$" /usr/lib/os-release
    test -e /usr/lib64/libEGL.so.1
    test -e /usr/lib64/libEGL_mesa.so.0
    test -e /usr/lib64/libgbm.so.1
    test -e /usr/lib64/libdrm.so.2
    test -e /usr/lib64/dri/iris_dri.so
    test "$(rpm -qf /usr/lib64/libEGL.so.1)" = "libglvnd-egl-1.7.0-9.fc44.x86_64"
    test "$(rpm -qf /usr/lib64/libEGL_mesa.so.0)" = "mesa-libEGL-26.1.7-1.fc44.x86_64"
    test "$(rpm -qf /usr/lib64/libgbm.so.1)" = "mesa-libgbm-26.1.7-1.fc44.x86_64"
    test "$(rpm -qf /usr/lib64/libdrm.so.2)" = "libdrm-2.4.134-1.fc44.x86_64"
    test "$(rpm -qf /usr/lib64/dri/iris_dri.so)" = "mesa-dri-drivers-26.1.7-1.fc44.x86_64"
    ! ldd /usr/libexec/prime/prime-compositor | grep -q "not found"
    ! ldd /usr/libexec/prime/prime-shell | grep -q "not found"
    /usr/libexec/prime/prime-compositor --help | grep -F "Usage: prime-compositor [--probe]"
    test -L /etc/systemd/system/graphical.target.wants/prime-compositor.service
    test -L /etc/systemd/system/graphical.target.wants/prime-shell.service
    test -L /etc/systemd/system/graphical.target.wants/prime-first-light-witness.service
    normal="$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.efi" ! -name "*.recovery.efi" -print -quit)"
    recovery="/boot/EFI/Prime/prime-recovery-${EXPECTED_DIGEST}.efi"
    test -n "$normal"
    test -s "$recovery"
    test "$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.efi" | wc -l)" -eq 1
    test "$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.recovery.efi" | wc -l)" -eq 0
    ukify --json=short inspect "$normal" > /tmp/prime-normal-uki.json
    ukify --json=short inspect "$recovery" > /tmp/prime-recovery-uki.json
    python3 -c '\''import json; n=json.load(open("/tmp/prime-normal-uki.json")); r=json.load(open("/tmp/prime-recovery-uki.json")); nc=n[".cmdline"]["text"]; rc=r[".cmdline"]["text"]; assert "prime.recovery=1" not in nc; assert "systemd.unit=prime-recovery.target" not in nc; assert "prime.recovery=1" in rc; assert "systemd.unit=prime-recovery.target" in rc; assert "Prime OS P1 First Light" in n[".osrel"]["text"]; assert "Prime OS Recovery" in r[".osrel"]["text"]'\''
    rm -f /tmp/prime-normal-uki.json /tmp/prime-recovery-uki.json
    ! find /usr/lib/modules -type f \( -name vmlinuz -o -name initramfs.img \) -print -quit | grep -q .
    bootc container lint --fatal-warnings
    bootc container inspect --json > /tmp/prime-container.json
    bootc install print-configuration > /tmp/prime-install.json
    python3 -c '\''import json; c=json.load(open("/tmp/prime-container.json")); assert c["kernel"]["unified"] is True, c; i=json.load(open("/tmp/prime-install.json")); assert i.get("bootloader") == "systemd", i'\''
    python3 -c '\''import json,os; s=json.load(open("/usr/lib/prime/generation-seed.json")); assert s["schema"]=="prime.generation-seed.v1"; assert s["generation_id"]==os.environ["EXPECTED_GENERATION_ID"]; assert s["channel"]=="LAB"; assert s["created_at"]==os.environ["EXPECTED_CREATED_AT"]; assert s["source_revision"]==os.environ["EXPECTED_SOURCE_REVISION"]; assert s["base_image_digest"]==os.environ["EXPECTED_BASE_DIGEST"]; assert s["boot_attempt_limit"]==3; sub=json.load(open("/usr/lib/prime/substrate-release.json")); assert sub["schema"]=="prime.substrate-release.v1"; assert sub["id"]=="fedora"; assert sub["version_id"]=="44"; assert sub["base_image_digest"]==os.environ["EXPECTED_BASE_DIGEST"]'\''
  '

log "Prove pinned image-builder sees unified Prime image"
"${PODMAN[@]}" pull "$BUILDER_REF"
"${PODMAN[@]}" run --rm "$BUILDER_REF" version
BUILDER_RUN_ARGS=(
  --privileged
  --security-opt label=disable
  --device /dev/fuse:/dev/fuse
)

log "Prove pinned image-builder nested mount authority"
"${PODMAN[@]}" run --rm \
  "${BUILDER_RUN_ARGS[@]}" \
  -v /var/lib/containers/storage:/var/lib/containers/storage \
  --entrypoint /bin/bash \
  "$BUILDER_REF" -ceu '
    test "$(id -u)" -eq 0
    mkdir -p /run/osbuild/containers/storage
    trap "umount --lazy /run/osbuild/containers/storage >/dev/null 2>&1 || true" EXIT
    mount --make-private -o rbind,rw,0755 /var/lib/containers/storage /run/osbuild/containers/storage
    findmnt -n /run/osbuild/containers/storage >/dev/null
  '

"${PODMAN[@]}" run --rm \
  "${BUILDER_RUN_ARGS[@]}" \
  -v /var/lib/containers/storage:/var/lib/containers/storage \
  "$BUILDER_REF" \
  bootc inspect --ref localhost/prime-os:p1 --format json > "$RUN_DIR/prime-image-builder-inspect.json"
python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); assert d["UnifiedKernel"] is True,d; assert d["Arch"]=="amd64",d; assert d["Bootloader"]=="systemd",d; o=d["OSInfo"]["OSRelease"]; assert o["ID"]=="prime",d; assert o["VersionID"]=="0.1",d' "$RUN_DIR/prime-image-builder-inspect.json"

log "Build Prime QCOW2"
[[ "$(sha256sum "$BWRAP_POLICY" | awk '{print $1}')" == "$EXPECTED_BWRAP_POLICY_SHA256" ]] || fail "bwrap AppArmor policy drift before QCOW2 build"
[[ "$(sha256sum "$LSBLK_POLICY" | awk '{print $1}')" == "$EXPECTED_LSBLK_POLICY_SHA256" ]] || fail "lsblk AppArmor policy drift before QCOW2 build"
sudo -n cat /sys/kernel/security/apparmor/profiles | grep -E '^(unpriv_bwrap|bwrap|lsblk) \(enforce\)$' | wc -l | grep -Fx 3 >/dev/null || fail "AppArmor profiles not enforced before QCOW2 build"

APPARMOR_WINDOW_OPEN=1
sudo -n apparmor_parser -r -C "$BWRAP_POLICY"
sudo -n apparmor_parser -r -C "$LSBLK_POLICY"
sudo -n cat /sys/kernel/security/apparmor/profiles | grep -E '^(unpriv_bwrap|bwrap|lsblk) \(complain\)$' | wc -l | grep -Fx 3 >/dev/null || fail "bounded AppArmor complain window did not open"

rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"
set +e
timeout --signal=TERM --kill-after=10s 1200s "${PODMAN[@]}" run --rm \
  "${BUILDER_RUN_ARGS[@]}" \
  -v /var/lib/containers/storage:/var/lib/containers/storage \
  -v "$OUTPUT_DIR:/output" \
  "$BUILDER_REF" \
  --output-dir /output \
  build \
  --bootc-ref localhost/prime-os:p1 \
  --bootc-build-ref "$BASE_IMAGE" \
  --bootc-default-fs ext4 \
  --with-manifest \
  --with-buildlog \
  --progress verbose \
  qcow2
BUILDER_RC=$?
set -e

restore_apparmor_profiles
[[ "$(sha256sum "$BWRAP_POLICY" | awk '{print $1}')" == "$EXPECTED_BWRAP_POLICY_SHA256" ]] || fail "bwrap AppArmor policy changed during QCOW2 build"
[[ "$(sha256sum "$LSBLK_POLICY" | awk '{print $1}')" == "$EXPECTED_LSBLK_POLICY_SHA256" ]] || fail "lsblk AppArmor policy changed during QCOW2 build"
sudo -n cat /sys/kernel/security/apparmor/profiles | grep -E '^(unpriv_bwrap|bwrap|lsblk) \(enforce\)$' | wc -l | grep -Fx 3 >/dev/null || fail "AppArmor profiles were not restored to enforce mode"
[[ "$BUILDER_RC" -eq 0 ]] || fail "bounded rootful image-builder QCOW2 build failed: $BUILDER_RC"
mapfile -t DISKS < <(find "$OUTPUT_DIR" -type f -name '*.qcow2' -print)
[[ "${#DISKS[@]}" -eq 1 ]] || fail "expected exactly one QCOW2, found ${#DISKS[@]}"
DISK="${DISKS[0]}"
qemu-img info "$DISK" | tee "$RUN_DIR/qemu-img-info.txt"
qemu-img check "$DISK" | tee "$RUN_DIR/qemu-img-check.txt"

log "Finalize firmware-readable XBOOTLDR"
bash tools/p1-finalize-xbootldr.sh "$DISK" /dev/nbd2
qemu-img check "$DISK" | tee "$RUN_DIR/qemu-img-check-xbootldr.txt"

log "Install and inspect bootc-normal / Prime-recovery UKI split"
cleanup_nbd
sudo -n modprobe nbd max_part=16
sudo -n qemu-nbd --connect="$NBD_DEV" "$DISK"
sleep 2
sudo -n partprobe "$NBD_DEV"
sudo -n sgdisk -p "$NBD_DEV" | tee "$RUN_DIR/gpt.txt"
lsblk -b -o NAME,SIZE,TYPE,FSTYPE,PARTTYPE,PARTLABEL "$NBD_DEV" | tee "$RUN_DIR/lsblk.txt"
ESP="$(lsblk -nrpo NAME,PARTTYPE "$NBD_DEV" | awk 'tolower($2)=="c12a7328-f81f-11d2-ba4b-00a0c93ec93b" {print $1; exit}')"
[[ -n "$ESP" ]] || fail "ESP not found"
sudo -n mount "$ESP" "$MOUNT_ESP"
sudo -n find "$MOUNT_ESP" -maxdepth 5 -type f -printf '%P\n' | sort | tee "$RUN_DIR/esp-files-before-recovery.txt"
[[ -f "$MOUNT_ESP/EFI/BOOT/BOOTX64.EFI" || -f "$MOUNT_ESP/EFI/systemd/systemd-bootx64.efi" ]] || fail "systemd-boot fallback/loader binary not found on ESP"
XBOOTLDR="$(lsblk -nrpo NAME,PARTTYPE "$NBD_DEV" | awk 'tolower($2)=="bc13c2ff-59e6-4262-a352-b275fd6f7172" {print $1; exit}')"
[[ -n "$XBOOTLDR" ]] || fail "XBOOTLDR not found"
XBOOTLDR_FSTYPE="$(lsblk -nrpo FSTYPE "$XBOOTLDR" | tr '[:upper:]' '[:lower:]')"
[[ "$XBOOTLDR_FSTYPE" == "vfat" ]] || fail "XBOOTLDR is not firmware-readable vfat: $XBOOTLDR_FSTYPE"
sudo -n mount -o ro "$XBOOTLDR" "$MOUNT_XBOOTLDR"
sudo -n find "$MOUNT_XBOOTLDR" -maxdepth 5 -type f -printf '%P\n' | sort | tee "$RUN_DIR/xbootldr-files.txt"

mapfile -t BOOTC_UKIS < <(sudo -n find "$MOUNT_ESP" "$MOUNT_XBOOTLDR" -type f -path '*/EFI/Linux/bootc/*.efi' -print | sort)
[[ "${#BOOTC_UKIS[@]}" -eq 1 ]] || fail "expected exactly one bootc-managed normal UKI, found ${#BOOTC_UKIS[@]}"

SOURCE_HASHES="$("${PODMAN[@]}" run --rm -e EXPECTED_DIGEST="$CANONICAL_DIGEST" --entrypoint /bin/bash localhost/prime-os:p1 -ceu '
  normal="$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.efi" -print -quit)"
  recovery="/boot/EFI/Prime/prime-recovery-${EXPECTED_DIGEST}.efi"
  test -s "$normal"; test -s "$recovery"
  printf "NORMAL %s\n" "$(sha256sum "$normal" | cut -d" " -f1)"
  printf "RECOVERY %s\n" "$(sha256sum "$recovery" | cut -d" " -f1)"
')"
SOURCE_NORMAL_SHA="$(printf '%s\n' "$SOURCE_HASHES" | awk '$1=="NORMAL" {print $2}')"
SOURCE_RECOVERY_SHA="$(printf '%s\n' "$SOURCE_HASHES" | awk '$1=="RECOVERY" {print $2}')"
[[ -n "$SOURCE_NORMAL_SHA" && -n "$SOURCE_RECOVERY_SHA" ]] || fail "source UKI hashes missing"
INSTALLED_NORMAL_SHA="$(sudo -n sha256sum "${BOOTC_UKIS[0]}" | awk '{print $1}')"
[[ "$INSTALLED_NORMAL_SHA" == "$SOURCE_NORMAL_SHA" ]] || fail "bootc-installed UKI is not the normal Prime UKI"

mapfile -t NORMAL_BLS < <(sudo -n find "$MOUNT_ESP" "$MOUNT_XBOOTLDR" -type f -path '*/loader/entries/bootc_prime-0.1-*.conf' -print | sort)
[[ "${#NORMAL_BLS[@]}" -eq 1 ]] || fail "expected one bootc normal BLS entry, found ${#NORMAL_BLS[@]}"
! sudo -n find "$MOUNT_ESP" "$MOUNT_XBOOTLDR" -type f -path '*/loader/entries/bootc_prime-0.0-*.conf' -print | grep -q . || fail "bootc installed recovery metadata instead of normal"

RECOVERY_NAME="prime-recovery-${CANONICAL_DIGEST}.efi"
RECOVERY_REL="/EFI/Prime/${RECOVERY_NAME}"
RECOVERY_COPY="$RUN_DIR/$RECOVERY_NAME"
rm -f "$RECOVERY_COPY"
"${PODMAN[@]}" run --rm --security-opt label=disable -e EXPECTED_DIGEST="$CANONICAL_DIGEST" -v "$RUN_DIR:/proof" --entrypoint /bin/bash localhost/prime-os:p1 -ceu '
  cp "/boot/EFI/Prime/prime-recovery-${EXPECTED_DIGEST}.efi" "/proof/prime-recovery-${EXPECTED_DIGEST}.efi"
'
sudo -n chown "$(id -u):$(id -g)" "$RECOVERY_COPY"
[[ "$(sha256sum "$RECOVERY_COPY" | awk '{print $1}')" == "$SOURCE_RECOVERY_SHA" ]] || fail "recovery extraction identity mismatch"

sudo -n install -D -m 0644 "$RECOVERY_COPY" "$MOUNT_ESP$RECOVERY_REL"
sudo -n install -d -m 0755 "$MOUNT_ESP/loader/entries"
BLS_COPY="$RUN_DIR/prime-recovery.conf"
cat > "$BLS_COPY" <<EOF
title Prime OS Recovery
version 0.0
sort-key zzz-prime-recovery
efi $RECOVERY_REL
EOF
sudo -n install -m 0644 "$BLS_COPY" "$MOUNT_ESP/loader/entries/prime-recovery.conf"
sudo -n sync

[[ "$(sudo -n sha256sum "$MOUNT_ESP$RECOVERY_REL" | awk '{print $1}')" == "$SOURCE_RECOVERY_SHA" ]] || fail "installed recovery UKI identity mismatch"
sudo -n grep -Fx 'title Prime OS Recovery' "$MOUNT_ESP/loader/entries/prime-recovery.conf"
sudo -n grep -Fx 'version 0.0' "$MOUNT_ESP/loader/entries/prime-recovery.conf"
sudo -n grep -Fx "efi $RECOVERY_REL" "$MOUNT_ESP/loader/entries/prime-recovery.conf"
[[ "$(sudo -n find "$MOUNT_ESP" "$MOUNT_XBOOTLDR" -type f -path '*/EFI/Linux/bootc/*.efi' | wc -l)" -eq 1 ]] || fail "bootc UKI namespace changed during recovery install"
sudo -n find "$MOUNT_ESP" -maxdepth 5 -type f -printf '%P\n' | sort | tee "$RUN_DIR/esp-files-after-recovery.txt"
printf '%s\n' "${BOOTC_UKIS[@]}" | tee "$RUN_DIR/prime-normal-uki-files.txt"
printf '%s\n' "$MOUNT_ESP$RECOVERY_REL" | tee "$RUN_DIR/prime-recovery-uki-files.txt"
cleanup_nbd

log "Finalize Discoverable Root and boot-entry contracts"
bash tools/p1-finalize-discoverable-root.sh "$DISK" /dev/nbd2

(
PROMOTE_NBD="/dev/nbd2"
PROMOTE_MNT="$RUN_DIR/mnt-promote-esp"
ROOT_X86_64_GUID="4f68bce3-e8cd-4db1-96e7-fbcaf984b709"
mkdir -p "$PROMOTE_MNT"
promote_cleanup() {
  sudo -n umount "$PROMOTE_MNT" >/dev/null 2>&1 || true
  sudo -n qemu-nbd --disconnect "$PROMOTE_NBD" >/dev/null 2>&1 || true
}
promote_cleanup
trap promote_cleanup EXIT
sudo -n modprobe nbd max_part=16
sudo -n qemu-nbd --connect="$PROMOTE_NBD" "$DISK"
sleep 2
sudo -n partprobe "$PROMOTE_NBD"

PROMOTE_ROOT="$(lsblk -b -nrpo NAME,TYPE,FSTYPE,SIZE "$PROMOTE_NBD" | awk '$2=="part" && $3=="ext4" {print $4,$1}' | sort -nr | head -n1 | awk '{print $2}')"
[[ -n "$PROMOTE_ROOT" ]] || fail "promotion root partition not found"
PROMOTE_ROOT_TYPE="$(lsblk -nrpo PARTTYPE "$PROMOTE_ROOT" | tr '[:upper:]' '[:lower:]')"
[[ "$PROMOTE_ROOT_TYPE" == "$ROOT_X86_64_GUID" ]] || fail "Discoverable Root finalization did not persist: $PROMOTE_ROOT_TYPE"

PROMOTE_ESP="$(lsblk -nrpo NAME,PARTTYPE "$PROMOTE_NBD" | awk 'tolower($2)=="c12a7328-f81f-11d2-ba4b-00a0c93ec93b" {print $1; exit}')"
[[ -n "$PROMOTE_ESP" ]] || fail "promotion ESP not found"
sudo -n mount "$PROMOTE_ESP" "$PROMOTE_MNT"

NORMAL_BLS="$(sudo -n find "$PROMOTE_MNT/loader/entries" -maxdepth 1 -type f -name 'bootc_prime-0.1-*.conf' -print | head -n1)"
RECOVERY_BLS="$PROMOTE_MNT/loader/entries/prime-recovery.conf"
NORMAL_UKI="$(sudo -n find "$PROMOTE_MNT" -type f -path '*/EFI/Linux/bootc/bootc_composefs-*.efi' -print | head -n1)"
[[ -n "$NORMAL_BLS" ]] || fail "normal bootc BLS not found"
sudo -n test -f "$RECOVERY_BLS" || fail "Prime Recovery BLS not found"
[[ -n "$NORMAL_UKI" ]] || fail "normal bootc UKI not found"

NORMAL_SHA_BEFORE="$(sudo -n sha256sum "$NORMAL_UKI" | awk '{print $1}')"
sudo -n cp "$NORMAL_UKI" "$RUN_DIR/prime-normal-promote.efi"
sudo -n chown "$(id -u):$(id -g)" "$RUN_DIR/prime-normal-promote.efi"
objcopy --dump-section .cmdline="$RUN_DIR/prime-normal-promote.cmdline" "$RUN_DIR/prime-normal-promote.efi"
BASE_CMDLINE="$(tr -d '\000' < "$RUN_DIR/prime-normal-promote.cmdline")"
printf '%s\n' "$BASE_CMDLINE" | grep -Eq '^composefs=[0-9a-f]{128}$' || fail "unexpected sealed normal UKI cmdline"

if sudo -n grep -q '^options ' "$NORMAL_BLS"; then
  EXISTING_OPTIONS="$(sudo -n awk '$1=="options" {$1=""; sub(/^ /,""); print; exit}' "$NORMAL_BLS")"
  [[ "$EXISTING_OPTIONS" == "$BASE_CMDLINE root=gpt-auto rw" ]] || \
    fail "unexpected pre-existing normal BLS options: $EXISTING_OPTIONS"
else
  printf 'options %s root=gpt-auto rw\n' "$BASE_CMDLINE" | sudo -n tee -a "$NORMAL_BLS" >/dev/null
fi
[[ "$(sudo -n sha256sum "$NORMAL_UKI" | awk '{print $1}')" == "$NORMAL_SHA_BEFORE" ]] || \
  fail "normal UKI bytes changed while promoting BLS root locator"

RECOVERY_OLD="$(sudo -n awk '$1=="efi" {print $2; exit}' "$RECOVERY_BLS")"
[[ -n "$RECOVERY_OLD" ]] || fail "Recovery BLS efi path missing"
RECOVERY_OLD_ABS="$PROMOTE_MNT$RECOVERY_OLD"
sudo -n test -f "$RECOVERY_OLD_ABS" || fail "Recovery UKI missing at $RECOVERY_OLD"
RECOVERY_SHA="$(sudo -n sha256sum "$RECOVERY_OLD_ABS" | awk '{print $1}')"

NORMAL_BASE="${NORMAL_UKI##*/}"
RECOVERY_NEW="/EFI/Prime/$NORMAL_BASE"
RECOVERY_NEW_ABS="$PROMOTE_MNT$RECOVERY_NEW"

if [[ "$RECOVERY_OLD" != "$RECOVERY_NEW" ]]; then
  sudo -n install -D -m 0644 "$RECOVERY_OLD_ABS" "$RECOVERY_NEW_ABS"
  if [[ "$(sudo -n sha256sum "$RECOVERY_NEW_ABS" | awk '{print $1}')" != "$RECOVERY_SHA" ]]; then
    sudo -n rm -f "$RECOVERY_NEW_ABS" >/dev/null 2>&1 || true
    fail "Recovery UKI identity changed during bootc-compatible rename"
  fi
  if ! sudo -n sed -i "s#^efi .*#efi $RECOVERY_NEW#" "$RECOVERY_BLS"; then
    sudo -n rm -f "$RECOVERY_NEW_ABS"
    fail "Recovery BLS update failed"
  fi
  if ! sudo -n grep -Fx "efi $RECOVERY_NEW" "$RECOVERY_BLS" >/dev/null; then
    sudo -n rm -f "$RECOVERY_NEW_ABS"
    fail "Recovery BLS does not point to bootc-compatible basename"
  fi
  sudo -n rm -f "$RECOVERY_OLD_ABS"
fi

sudo -n grep -Fx "efi $RECOVERY_NEW" "$RECOVERY_BLS" >/dev/null || \
  fail "Recovery BLS does not point to bootc-compatible basename"
[[ "${RECOVERY_NEW##*/}" == bootc_composefs-*.efi ]] || fail "Recovery basename is not bootc-compatible"
mapfile -t LEGACY_RECOVERY_UKIS < <(sudo -n find "$PROMOTE_MNT/EFI/Prime" -maxdepth 1 -type f -name 'prime-recovery-*.efi' -print | sort)
for legacy_recovery in "${LEGACY_RECOVERY_UKIS[@]}"; do
  sudo -n rm -f "$legacy_recovery" || fail "legacy Recovery cleanup failed: $legacy_recovery"
done
if sudo -n find "$PROMOTE_MNT/EFI/Prime" -maxdepth 1 -type f -name 'prime-recovery-*.efi' -print | grep -q .; then
  fail "legacy Recovery basename remains installed"
fi

sudo -n sync
)
promoted_check_ok=0
promoted_check_out=""
for _ in {1..20}; do
  if promoted_check_out="$(qemu-img check "$DISK" 2>&1)"; then
    promoted_check_ok=1
    break
  fi
  sleep 0.25
done
[[ "$promoted_check_ok" -eq 1 ]] || fail "promoted QCOW2 did not become available after NBD disconnect: ${promoted_check_out:-no stderr}"
printf '%s\n' "$promoted_check_out" | tee "$RUN_DIR/qemu-img-check-promoted.txt"

log "Inspect GPT, ESP and normal/recovery UKI placement"
cleanup_nbd
sudo -n modprobe nbd max_part=16
sudo -n qemu-nbd --read-only --connect="$NBD_DEV" "$DISK"
sleep 2
sudo -n partprobe "$NBD_DEV"
sudo -n sgdisk -p "$NBD_DEV" | tee "$RUN_DIR/gpt.txt"
lsblk -b -o NAME,SIZE,TYPE,FSTYPE,PARTTYPE,PARTLABEL "$NBD_DEV" | tee "$RUN_DIR/lsblk.txt"
ESP="$(lsblk -nrpo NAME,PARTTYPE "$NBD_DEV" | awk 'tolower($2)=="c12a7328-f81f-11d2-ba4b-00a0c93ec93b" {print $1; exit}')"
[[ -n "$ESP" ]] || fail "ESP not found"
sudo -n mount -o ro "$ESP" "$MOUNT_ESP"
sudo -n find "$MOUNT_ESP" -maxdepth 5 -type f -printf '%P\n' | sort | tee "$RUN_DIR/esp-files.txt"
[[ -f "$MOUNT_ESP/EFI/BOOT/BOOTX64.EFI" || -f "$MOUNT_ESP/EFI/systemd/systemd-bootx64.efi" ]] || fail "systemd-boot fallback/loader binary not found on ESP"
XBOOTLDR="$(lsblk -nrpo NAME,PARTTYPE "$NBD_DEV" | awk 'tolower($2)=="bc13c2ff-59e6-4262-a352-b275fd6f7172" {print $1; exit}')"
[[ -n "$XBOOTLDR" ]] || fail "XBOOTLDR not found"
XBOOTLDR_FSTYPE="$(lsblk -nrpo FSTYPE "$XBOOTLDR" | tr '[:upper:]' '[:lower:]')"
[[ "$XBOOTLDR_FSTYPE" == "vfat" ]] || fail "XBOOTLDR is not firmware-readable vfat: $XBOOTLDR_FSTYPE"
sudo -n mount -o ro "$XBOOTLDR" "$MOUNT_XBOOTLDR"
sudo -n find "$MOUNT_XBOOTLDR" -maxdepth 5 -type f -printf '%P\n' | sort | tee "$RUN_DIR/xbootldr-files.txt"
mapfile -t INSTALLED_NORMAL_UKIS < <(sudo -n find "$MOUNT_ESP" "$MOUNT_XBOOTLDR" -type f -path '*/EFI/Linux/bootc/bootc_composefs-*.efi' -print | sort)
[[ "${#INSTALLED_NORMAL_UKIS[@]}" -eq 1 ]] || fail "expected exactly one bootc-owned normal UKI, found ${#INSTALLED_NORMAL_UKIS[@]}"
mapfile -t INSTALLED_RECOVERY_UKIS < <(sudo -n find "$MOUNT_ESP" -type f -path '*/EFI/Prime/bootc_composefs-*.efi' -print | sort)
[[ "${#INSTALLED_RECOVERY_UKIS[@]}" -eq 1 ]] || fail "expected exactly one Prime-owned Recovery UKI, found ${#INSTALLED_RECOVERY_UKIS[@]}"
RECOVERY_BLS_INSTALLED="$MOUNT_ESP/loader/entries/prime-recovery.conf"
sudo -n test -f "$RECOVERY_BLS_INSTALLED" || fail "installed Prime Recovery BLS missing"
NORMAL_INSTALLED_BASE="${INSTALLED_NORMAL_UKIS[0]##*/}"
RECOVERY_INSTALLED_BASE="${INSTALLED_RECOVERY_UKIS[0]##*/}"
[[ "$RECOVERY_INSTALLED_BASE" == "$NORMAL_INSTALLED_BASE" ]] || fail "Recovery UKI basename is not bootc-compatible"
sudo -n grep -Fx "efi /EFI/Prime/$RECOVERY_INSTALLED_BASE" "$RECOVERY_BLS_INSTALLED" >/dev/null || fail "Recovery BLS path mismatch"
printf '%s\n' "${INSTALLED_NORMAL_UKIS[@]}" "${INSTALLED_RECOVERY_UKIS[@]}" | tee "$RUN_DIR/prime-uki-files.txt"
cleanup_nbd

log "Boot normal QCOW2 through OVMF/QEMU"
OVMF_CODE=/usr/share/OVMF/OVMF_CODE_4M.fd
OVMF_VARS_TEMPLATE=/usr/share/OVMF/OVMF_VARS_4M.fd
OVMF_VARS="$RUN_DIR/OVMF_VARS_4M.fd"
rm -f "$OVERLAY" "$SERIAL_LOG" "$OVMF_VARS"
qemu-img create -f qcow2 -F qcow2 -b "$(realpath "$DISK")" "$OVERLAY"
[[ -r "$OVMF_CODE" ]] || fail "OVMF CODE firmware not readable"
[[ -r "$OVMF_VARS_TEMPLATE" ]] || fail "OVMF VARS template not readable"
cp "$OVMF_VARS_TEMPLATE" "$OVMF_VARS"
[[ -w "$OVMF_VARS" ]] || fail "disposable OVMF VARS is not writable"
set +e
timeout --signal=TERM 120s qemu-system-x86_64 \
  -machine q35,accel=tcg \
  -cpu max \
  -smp 2 \
  -m 3072 \
  -drive if=pflash,format=raw,readonly=on,file="$OVMF_CODE" \
  -drive if=pflash,format=raw,file="$OVMF_VARS" \
  -drive file="$OVERLAY",if=virtio,format=qcow2 \
  -display none \
  -serial "file:$SERIAL_LOG" \
  -monitor none \
  -net none \
  -no-reboot
QEMU_RC=$?
set -e
[[ "$QEMU_RC" -eq 0 || "$QEMU_RC" -eq 124 || "$QEMU_RC" -eq 143 ]] || fail "QEMU exited unexpectedly: $QEMU_RC"

log "Prove Prime Core persisted Host state and entered health proving after UEFI boot"
cleanup_nbd
# Keep the disposable overlay block device writable so ext4 may replay its journal;
# the filesystem itself remains mounted read-only for evidence recovery.
sudo -n qemu-nbd --connect="$NBD_DEV" "$OVERLAY"
sleep 2
sudo -n partprobe "$NBD_DEV"
ROOT_PART="$(lsblk -b -nrpo NAME,TYPE,FSTYPE,SIZE "$NBD_DEV" | awk '$2=="part" && $3=="ext4" {print $4,$1}' | sort -nr | head -n1 | awk '{print $2}')"
[[ -n "$ROOT_PART" ]] || fail "ext4 root partition not found"
sudo -n mount -o ro "$ROOT_PART" "$MOUNT_ROOT"
PRIME_DIR=""
for candidate in "$MOUNT_ROOT/state/os/default/var/lib/prime" "$MOUNT_ROOT/var/lib/prime"; do
  if sudo -n test -f "$candidate/hardware/current.json"; then PRIME_DIR="$candidate"; break; fi
done
[[ -n "$PRIME_DIR" ]] || fail "Prime persisted state not found"
IDENTITY_FILE="$PRIME_DIR/identity/host.json"
HARDWARE_FILE="$PRIME_DIR/hardware/current.json"
GENERATION_FILE="$PRIME_DIR/generations/current.json"
sudo -n test -f "$IDENTITY_FILE"
sudo -n test -f "$HARDWARE_FILE"
sudo -n test -f "$GENERATION_FILE"
sudo -n python3 -c 'import json,sys; h=json.load(open(sys.argv[1])); hw=json.load(open(sys.argv[2])); assert h["host_id"]; assert str(h["host_arch"]).lower() in ("x86_64","amd64"); assert hw' "$IDENTITY_FILE" "$HARDWARE_FILE"
sudo -n env EXPECTED_GENERATION_ID="$GENERATION_ID" python3 -c 'import json,os,sys; g=json.load(open(sys.argv[1])); assert g["generation_id"]==os.environ["EXPECTED_GENERATION_ID"],g; assert g["state"]=="HEALTH_PROVING",g; assert "prime.core.socket.bound.v1" in g.get("evidence_refs",[]),g; assert g.get("boot_attempts_remaining")==3,g' "$GENERATION_FILE"
WITNESS_FILE="$PRIME_DIR/first-light/mechanical.json"
sudo -n test -f "$WITNESS_FILE"
sudo -n python3 -c 'import json,sys; w=json.load(open(sys.argv[1])); assert w["schema"]=="prime.first-light-mechanical.v1",w; assert w["status"]=="SHELL_READY",w; assert w["compositor_phase"]=="SHELL_READY",w; assert w["shell_ready"] is True,w; assert w["frame_loop_ready"] is True,w; assert w["wayland_listener_ready"] is True,w; assert w["clients_accepted"]>=1,w; assert w["mapped_surface_frames_submitted"]>=1,w; assert w["core_socket_group_nonzero"] is True,w; assert w["owner_visual_acceptance"] is False,w' "$WITNESS_FILE"
HOST_ID="$(sudo -n python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["host_id"])' "$IDENTITY_FILE")"
cleanup_nbd

log "Write local proof report"
DISK_SHA256="$(sha256sum "$DISK" | awk '{print $1}')"
export REPORT SOURCE_REVISION CREATED_AT GENERATION_ID BASE_IMAGE BASE_DIGEST BUILDER_REF BUILDER_DIGEST BUILDER_VERSION CANONICAL_DIGEST FINAL_DIGEST NORMAL_EMBEDDED_DIGEST RECOVERY_EMBEDDED_DIGEST DISK DISK_SHA256 HOST_ID
python3 -c 'import json,os,pathlib; p={"schema":"prime.p1-local-proof.v1","ok":True,"source_revision":os.environ["SOURCE_REVISION"],"created_at":os.environ["CREATED_AT"],"generation_id":os.environ["GENERATION_ID"],"generation_state":"HEALTH_PROVING","known_good_proven":False,"base_image":os.environ["BASE_IMAGE"],"base_image_digest":os.environ["BASE_DIGEST"],"image_builder":os.environ["BUILDER_REF"],"image_builder_digest":os.environ["BUILDER_DIGEST"],"image_builder_version":os.environ["BUILDER_VERSION"],"prime_product_identity":True,"substrate":{"id":"fedora","version_id":"44","base_image_digest":os.environ["BASE_DIGEST"]},"composefs_canonical_digest":os.environ["CANONICAL_DIGEST"],"composefs_final_digest":os.environ["FINAL_DIGEST"],"normal_uki_embedded_digest":os.environ["NORMAL_EMBEDDED_DIGEST"],"recovery_uki_embedded_digest":os.environ["RECOVERY_EMBEDDED_DIGEST"],"recovery_uki_present":True,"recovery_boot_proven":False,"qcow2_path":os.environ["DISK"],"qcow2_sha256":os.environ["DISK_SHA256"],"qemu_uefi":"OVMF","prime_host_id":os.environ["HOST_ID"],"mechanical_shell_ready":True,"owner_visual_acceptance":False,"physical_kratos_boot_proven":False}; path=pathlib.Path(os.environ["REPORT"]); path.write_text(json.dumps(p,indent=2)+"\n",encoding="utf-8"); print(path.read_text())'

printf '\nP1_LOCAL_PROOF=PASS\nREPORT=%s\nQCOW2=%s\n' "$REPORT" "$DISK"
