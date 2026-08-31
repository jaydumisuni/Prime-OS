#!/usr/bin/env bash
set -euo pipefail

DISK="${1:?usage: p1-finalize-xbootldr.sh <qcow2> [nbd-device]}"
NBD="${2:-/dev/nbd2}"
XBOOTLDR_GUID="bc13c2ff-59e6-4262-a352-b275fd6f7172"
MNT="$(mktemp -d /tmp/prime-p1-xbootldr.XXXXXX)"

[[ -f "$DISK" ]] || { echo "missing QCOW2: $DISK" >&2; exit 1; }
command -v qemu-nbd >/dev/null
command -v lsblk >/dev/null
command -v mkfs.vfat >/dev/null
sudo -n true

cleanup() {
  sudo -n umount "$MNT" >/dev/null 2>&1 || true
  sudo -n qemu-nbd --disconnect "$NBD" >/dev/null 2>&1 || true
  rmdir "$MNT" >/dev/null 2>&1 || true
}
trap cleanup EXIT
cleanup
mkdir -p "$MNT"

sudo -n modprobe nbd max_part=16
for _ in {1..100}; do
  [[ -b "$NBD" ]] && break
  sleep 0.1
done
[[ -b "$NBD" ]] || { echo "NBD block device did not appear after modprobe: $NBD" >&2; exit 1; }

connected=0
connect_err=""
for _ in {1..20}; do
  if connect_err="$(sudo -n qemu-nbd --connect="$NBD" "$DISK" 2>&1)"; then
    connected=1
    break
  fi
  sudo -n qemu-nbd --disconnect "$NBD" >/dev/null 2>&1 || true
  sleep 0.25
done
[[ "$connected" -eq 1 ]] || { echo "failed to connect QCOW2 to NBD after retries: $NBD: ${connect_err:-no stderr}" >&2; exit 1; }
sleep 2
sudo -n partprobe "$NBD"

XBOOTLDR="$(lsblk -nrpo NAME,PARTTYPE "$NBD" | awk -v guid="$XBOOTLDR_GUID" 'tolower($2)==guid {print $1; exit}')"
[[ -n "$XBOOTLDR" ]] || { echo "XBOOTLDR partition not found" >&2; exit 1; }
BEFORE="$(lsblk -nrpo FSTYPE "$XBOOTLDR" | tr '[:upper:]' '[:lower:]')"

case "$BEFORE" in
  vfat|fat|fat32)
    echo "P1_XBOOTLDR_ALREADY_FIRMWARE_READABLE=1"
    ;;
  ext4)
    sudo -n mount -o ro "$XBOOTLDR" "$MNT"
    sudo -n test ! -e "$MNT/lost+found" || sudo -n test -d "$MNT/lost+found"
    sudo -n test ! -e "$MNT/efi" || sudo -n test -d "$MNT/efi"
    UNEXPECTED="$(sudo -n find "$MNT" -mindepth 1 \
      ! -path "$MNT/lost+found" \
      ! -path "$MNT/efi" \
      -print -quit)"
    [[ -z "$UNEXPECTED" ]] || { echo "refusing to reformat XBOOTLDR with installed content: $UNEXPECTED" >&2; exit 1; }
    sudo -n umount "$MNT"
    sudo -n mkfs.vfat -F 32 -n XBOOTLDR "$XBOOTLDR" >/dev/null
    sudo -n sync
    sudo -n partprobe "$NBD"
    ;;
  *)
    echo "unexpected XBOOTLDR filesystem: ${BEFORE:-none}" >&2
    exit 1
    ;;
esac

AFTER="$(lsblk -nrpo FSTYPE "$XBOOTLDR" | tr '[:upper:]' '[:lower:]')"
[[ "$AFTER" == "vfat" ]] || { echo "XBOOTLDR finalization failed: $AFTER" >&2; exit 1; }

echo "P1_XBOOTLDR_PARTITION=$XBOOTLDR"
echo "P1_XBOOTLDR_FSTYPE_BEFORE=$BEFORE"
echo "P1_XBOOTLDR_FSTYPE_AFTER=$AFTER"
echo "P1_XBOOTLDR_FINALIZATION=PASS"
