#!/usr/bin/env bash
set -euo pipefail

DISK="${1:?usage: p1-finalize-discoverable-root.sh <qcow2> [nbd-device]}"
NBD="${2:-/dev/nbd2}"
ROOT_X86_64_GUID="4f68bce3-e8cd-4db1-96e7-fbcaf984b709"
GENERIC_LINUX_GUID="0fc63daf-8483-4772-8e79-3d69d8477de4"

[[ -f "$DISK" ]] || { echo "missing QCOW2: $DISK" >&2; exit 1; }
command -v qemu-nbd >/dev/null
command -v sgdisk >/dev/null
command -v lsblk >/dev/null
sudo -n true

cleanup() {
  sudo -n qemu-nbd --disconnect "$NBD" >/dev/null 2>&1 || true
}
trap cleanup EXIT
cleanup

sudo -n modprobe nbd max_part=16
for _ in {1..100}; do
  [[ -b "$NBD" ]] && break
  sleep 0.1
done
[[ -b "$NBD" ]] || { echo "NBD block device did not appear after modprobe: $NBD" >&2; exit 1; }
sudo -n qemu-nbd --connect="$NBD" "$DISK"
sleep 2
sudo -n partprobe "$NBD"

ROOT="$(lsblk -b -nrpo NAME,TYPE,FSTYPE,SIZE "$NBD" | awk '$2=="part" && $3=="ext4" {print $4,$1}' | sort -nr | head -n1 | awk '{print $2}')"
[[ -n "$ROOT" ]] || { echo "ext4 root partition not found" >&2; exit 1; }
PARTNUM="${ROOT##*p}"
BEFORE="$(lsblk -nrpo PARTTYPE "$ROOT" | tr '[:upper:]' '[:lower:]')"

case "$BEFORE" in
  "$ROOT_X86_64_GUID")
    echo "P1_ROOT_GPT_ALREADY_DISCOVERABLE=1"
    ;;
  "$GENERIC_LINUX_GUID")
    sudo -n sgdisk --typecode="${PARTNUM}:${ROOT_X86_64_GUID}" "$NBD" >/dev/null
    sudo -n partprobe "$NBD"
    ;;
  *)
    echo "unexpected root GPT type: $BEFORE" >&2
    exit 1
    ;;
esac

AFTER="$(lsblk -nrpo PARTTYPE "$ROOT" | tr '[:upper:]' '[:lower:]')"
[[ "$AFTER" == "$ROOT_X86_64_GUID" ]] || { echo "root GPT type finalization failed: $AFTER" >&2; exit 1; }

echo "P1_ROOT_PARTITION=$ROOT"
echo "P1_ROOT_GPT_BEFORE=$BEFORE"
echo "P1_ROOT_GPT_AFTER=$AFTER"
echo "P1_DISCOVERABLE_ROOT_FINALIZATION=PASS"
