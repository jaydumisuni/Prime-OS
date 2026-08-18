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
trap cleanup_nbd EXIT

log "P1 local proof preflight"
[[ "$(uname -m)" == "x86_64" ]] || fail "P1 proof requires x86_64"
for tool in git python3 rustc cargo podman skopeo qemu-img qemu-system-x86_64 qemu-nbd sgdisk lsblk findmnt timeout sha256sum; do need "$tool"; done
sudo -n true || fail "non-interactive sudo is required for rootful Podman/NBD proof"
[[ -e /dev/fuse ]] || fail "/dev/fuse is unavailable"
[[ -f image/fedora-base.lock.json ]] || fail "missing Fedora base lock"
[[ -f image/image-builder.lock.json ]] || fail "missing image-builder lock"
[[ -f image/Containerfile ]] || fail "missing image/Containerfile"
[[ -f image/prime-os-release ]] || fail "missing Prime product identity"
[[ -f image/scripts/prepare-uki-cmdlines.py ]] || fail "missing UKI command-line helper"
[[ -x /usr/bin/env ]] || fail "invalid host environment"

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
cargo clippy --locked --workspace --all-targets -- -D warnings
cargo test --locked --workspace
cargo build --locked --release -p primed
[[ -x target/release/primed ]] || fail "primed release binary missing"
[[ -x target/release/prime-recovery ]] || fail "prime-recovery release binary missing"

log "Build provisional Prime bootc image"
"${PODMAN[@]}" pull "$BASE_IMAGE"
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
  -f image/Containerfile \
  -t localhost/prime-os:p1-provisional .

log "Compute canonical OCI-storage Composefs digest"
CANONICAL_DIGEST="$("${PODMAN[@]}" run --rm \
  --privileged \
  --security-opt label=disable \
  --tmpfs /var/tmp:size=8g \
  -v /var/lib/containers/storage:/var/lib/containers/storage \
  localhost/prime-os:p1-provisional \
  bootc container compute-composefs-digest-from-storage localhost/prime-os:p1-provisional | tail -n1)"
case "$CANONICAL_DIGEST" in ????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????????) ;; *) fail "invalid canonical Composefs digest: $CANONICAL_DIGEST" ;; esac
printf 'canonical digest: %s\n' "$CANONICAL_DIGEST"

log "Rebuild Prime with canonical normal and recovery UKI seals"
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

log "Prove final OCI-storage digest equals both embedded UKI digests"
FINAL_DIGEST="$("${PODMAN[@]}" run --rm \
  --privileged \
  --security-opt label=disable \
  --tmpfs /var/tmp:size=8g \
  -v /var/lib/containers/storage:/var/lib/containers/storage \
  localhost/prime-os:p1 \
  bootc container compute-composefs-digest-from-storage localhost/prime-os:p1 | tail -n1)"
UKI_DIGESTS="$("${PODMAN[@]}" run --rm --entrypoint /bin/bash localhost/prime-os:p1 -ceu '
  normal="$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.efi" ! -name "*.recovery.efi" -print -quit)"
  recovery="$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.recovery.efi" -print -quit)"
  test -n "$normal"
  test -n "$recovery"
  test "$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.efi" ! -name "*.recovery.efi" | wc -l)" -eq 1
  test "$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.recovery.efi" | wc -l)" -eq 1
  grep -aoE "composefs=[0-9a-f]{128}" "$normal" | head -n1 | cut -d= -f2
  grep -aoE "composefs=[0-9a-f]{128}" "$recovery" | head -n1 | cut -d= -f2
')"
NORMAL_EMBEDDED_DIGEST="$(printf '%s\n' "$UKI_DIGESTS" | sed -n '1p')"
RECOVERY_EMBEDDED_DIGEST="$(printf '%s\n' "$UKI_DIGESTS" | sed -n '2p')"
[[ -n "$NORMAL_EMBEDDED_DIGEST" ]] || fail "normal UKI Composefs digest missing"
[[ -n "$RECOVERY_EMBEDDED_DIGEST" ]] || fail "recovery UKI Composefs digest missing"
[[ "$FINAL_DIGEST" == "$CANONICAL_DIGEST" ]] || fail "final storage digest changed after reseal"
[[ "$NORMAL_EMBEDDED_DIGEST" == "$CANONICAL_DIGEST" ]] || fail "normal UKI Composefs digest does not match canonical storage digest"
[[ "$RECOVERY_EMBEDDED_DIGEST" == "$CANONICAL_DIGEST" ]] || fail "recovery UKI Composefs digest does not match canonical storage digest"
printf 'final digest:       %s\nnormal UKI digest:  %s\nrecovery UKI digest:%s\n' "$FINAL_DIGEST" "$NORMAL_EMBEDDED_DIGEST" "$RECOVERY_EMBEDDED_DIGEST"

log "Inspect final Prime image and recovery contract"
"${PODMAN[@]}" run --rm \
  -e EXPECTED_SOURCE_REVISION="$SOURCE_REVISION" \
  -e EXPECTED_CREATED_AT="$CREATED_AT" \
  -e EXPECTED_GENERATION_ID="$GENERATION_ID" \
  -e EXPECTED_BASE_DIGEST="$BASE_DIGEST" \
  --entrypoint /bin/bash localhost/prime-os:p1 -ceu '
    test -x /usr/libexec/prime/primed
    test -x /usr/libexec/prime/prime-recovery
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
    test -L /etc/systemd/system/multi-user.target.wants/primed.service
    test ! -e /etc/systemd/system/timers.target.wants/bootc-fetch-apply-updates.timer
    ! rpm -q bootupd >/dev/null 2>&1
    rpm -q systemd-ukify-259.8-1.fc44 systemd-boot-unsigned-259.8-1.fc44
    grep -q "^ID=prime$" /usr/lib/os-release
    grep -q "^PRETTY_NAME=\"Prime OS P1 First Light\"$" /usr/lib/os-release
    normal="$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.efi" ! -name "*.recovery.efi" -print -quit)"
    recovery="$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.recovery.efi" -print -quit)"
    test -n "$normal"
    test -n "$recovery"
    test "$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.efi" | wc -l)" -eq 2
    test "$(find /boot/EFI/Linux -maxdepth 1 -type f -name "*.recovery.efi" | wc -l)" -eq 1
    ukify inspect --json=short "$normal" > /tmp/prime-normal-uki.json
    ukify inspect --json=short "$recovery" > /tmp/prime-recovery-uki.json
    python3 -c '\''import json; n=json.load(open("/tmp/prime-normal-uki.json")); r=json.load(open("/tmp/prime-recovery-uki.json")); nc=n[".cmdline"]["text"]; rc=r[".cmdline"]["text"]; assert "prime.recovery=1" not in nc; assert "systemd.unit=prime-recovery.target" not in nc; assert "prime.recovery=1" in rc; assert "systemd.unit=prime-recovery.target" in rc; assert "Prime OS P1 First Light" in n[".osrel"]["text"]; assert "Prime OS Recovery" in r[".osrel"]["text"]'\''
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
"${PODMAN[@]}" run --rm \
  --privileged \
  --security-opt label=disable \
  -v /var/lib/containers/storage:/var/lib/containers/storage \
  "$BUILDER_REF" \
  bootc inspect --ref localhost/prime-os:p1 --format json > "$RUN_DIR/prime-image-builder-inspect.json"
python3 -c 'import json,sys; d=json.load(open(sys.argv[1])); assert d["UnifiedKernel"] is True,d; assert d["Arch"]=="amd64",d; assert d["Bootloader"]=="systemd",d; o=d["OSInfo"]["OSRelease"]; assert o["ID"]=="prime",d; assert o["VersionID"]=="0.1",d' "$RUN_DIR/prime-image-builder-inspect.json"

log "Build Prime QCOW2"
rm -rf "$OUTPUT_DIR"
mkdir -p "$OUTPUT_DIR"
"${PODMAN[@]}" run --rm \
  --privileged \
  --security-opt label=disable \
  -v /var/lib/containers/storage:/var/lib/containers/storage \
  -v "$OUTPUT_DIR:/output" \
  "$BUILDER_REF" \
  --output-dir /output \
  build \
  --bootc-ref localhost/prime-os:p1 \
  --bootc-default-fs ext4 \
  --with-manifest \
  --with-buildlog \
  --progress verbose \
  qcow2
mapfile -t DISKS < <(find "$OUTPUT_DIR" -type f -name '*.qcow2' -print)
[[ "${#DISKS[@]}" -eq 1 ]] || fail "expected exactly one QCOW2, found ${#DISKS[@]}"
DISK="${DISKS[0]}"
qemu-img info "$DISK" | tee "$RUN_DIR/qemu-img-info.txt"
qemu-img check "$DISK" | tee "$RUN_DIR/qemu-img-check.txt"

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
find "$MOUNT_ESP" -maxdepth 5 -type f -printf '%P\n' | sort | tee "$RUN_DIR/esp-files.txt"
[[ -f "$MOUNT_ESP/EFI/BOOT/BOOTX64.EFI" || -f "$MOUNT_ESP/EFI/systemd/systemd-bootx64.efi" ]] || fail "systemd-boot fallback/loader binary not found on ESP"
XBOOTLDR="$(lsblk -nrpo NAME,PARTTYPE "$NBD_DEV" | awk 'tolower($2)=="bc13c2ff-59e6-4262-a352-b275fd6f7172" {print $1; exit}')"
if [[ -n "$XBOOTLDR" ]]; then
  sudo -n mount -o ro "$XBOOTLDR" "$MOUNT_XBOOTLDR"
  find "$MOUNT_XBOOTLDR" -maxdepth 5 -type f -printf '%P\n' | sort | tee "$RUN_DIR/xbootldr-files.txt"
fi
mapfile -t INSTALLED_UKIS < <(find "$MOUNT_ESP" "$MOUNT_XBOOTLDR" -type f -path '*/EFI/Linux/*.efi' -print 2>/dev/null | sort)
[[ "${#INSTALLED_UKIS[@]}" -eq 2 ]] || fail "expected two installed Prime UKIs, found ${#INSTALLED_UKIS[@]}"
INSTALLED_RECOVERY_COUNT="$(printf '%s\n' "${INSTALLED_UKIS[@]}" | grep -c '\.recovery\.efi$' || true)"
[[ "$INSTALLED_RECOVERY_COUNT" -eq 1 ]] || fail "expected exactly one installed recovery UKI"
printf '%s\n' "${INSTALLED_UKIS[@]}" | tee "$RUN_DIR/prime-uki-files.txt"
cleanup_nbd

log "Boot normal QCOW2 through OVMF/QEMU"
rm -f "$OVERLAY" "$SERIAL_LOG"
qemu-img create -f qcow2 -F qcow2 -b "$(realpath "$DISK")" "$OVERLAY"
OVMF=""
for candidate in /usr/share/OVMF/OVMF_CODE.fd /usr/share/OVMF/OVMF_CODE_4M.fd; do
  if [[ -f "$candidate" ]]; then OVMF="$candidate"; break; fi
done
[[ -n "$OVMF" ]] || fail "OVMF_CODE firmware not found"
set +e
timeout --signal=TERM 90s qemu-system-x86_64 \
  -machine q35,accel=tcg \
  -cpu max \
  -smp 2 \
  -m 3072 \
  -bios "$OVMF" \
  -drive file="$OVERLAY",if=virtio,format=qcow2 \
  -display none \
  -serial "file:$SERIAL_LOG" \
  -monitor none \
  -net none \
  -no-reboot
QEMU_RC=$?
set -e
[[ "$QEMU_RC" -eq 0 || "$QEMU_RC" -eq 124 || "$QEMU_RC" -eq 143 ]] || fail "QEMU exited unexpectedly: $QEMU_RC"

log "Prove Prime Core persisted Host state after UEFI boot"
cleanup_nbd
sudo -n qemu-nbd --read-only --connect="$NBD_DEV" "$OVERLAY"
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
sudo -n env EXPECTED_GENERATION_ID="$GENERATION_ID" python3 -c 'import json,os,sys; g=json.load(open(sys.argv[1])); assert g["generation_id"]==os.environ["EXPECTED_GENERATION_ID"],g; assert g["state"]=="BOOT_TRY",g' "$GENERATION_FILE"
HOST_ID="$(sudo -n python3 -c 'import json,sys; print(json.load(open(sys.argv[1]))["host_id"])' "$IDENTITY_FILE")"
cleanup_nbd

log "Write local proof report"
DISK_SHA256="$(sha256sum "$DISK" | awk '{print $1}')"
export REPORT SOURCE_REVISION CREATED_AT GENERATION_ID BASE_IMAGE BASE_DIGEST BUILDER_REF BUILDER_DIGEST BUILDER_VERSION CANONICAL_DIGEST FINAL_DIGEST NORMAL_EMBEDDED_DIGEST RECOVERY_EMBEDDED_DIGEST DISK DISK_SHA256 HOST_ID
python3 -c 'import json,os,pathlib; p={"schema":"prime.p1-local-proof.v1","ok":True,"source_revision":os.environ["SOURCE_REVISION"],"created_at":os.environ["CREATED_AT"],"generation_id":os.environ["GENERATION_ID"],"base_image":os.environ["BASE_IMAGE"],"base_image_digest":os.environ["BASE_DIGEST"],"image_builder":os.environ["BUILDER_REF"],"image_builder_digest":os.environ["BUILDER_DIGEST"],"image_builder_version":os.environ["BUILDER_VERSION"],"prime_product_identity":True,"substrate":{"id":"fedora","version_id":"44","base_image_digest":os.environ["BASE_DIGEST"]},"composefs_canonical_digest":os.environ["CANONICAL_DIGEST"],"composefs_final_digest":os.environ["FINAL_DIGEST"],"normal_uki_embedded_digest":os.environ["NORMAL_EMBEDDED_DIGEST"],"recovery_uki_embedded_digest":os.environ["RECOVERY_EMBEDDED_DIGEST"],"recovery_uki_present":True,"recovery_boot_proven":False,"qcow2_path":os.environ["DISK"],"qcow2_sha256":os.environ["DISK_SHA256"],"qemu_uefi":"OVMF","prime_host_id":os.environ["HOST_ID"],"physical_kratos_boot_proven":False}; path=pathlib.Path(os.environ["REPORT"]); path.write_text(json.dumps(p,indent=2)+"\n",encoding="utf-8"); print(path.read_text())'

printf '\nP1_LOCAL_PROOF=PASS\nREPORT=%s\nQCOW2=%s\n' "$REPORT" "$DISK"
