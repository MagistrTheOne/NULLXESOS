#!/usr/bin/env bash
# test-iso.sh — boot the latest NULLXES ISO under QEMU/KVM with UEFI.
# Usage:
#   ./iso/scripts/test-iso.sh ./iso/out/nullxes-2026.05.08-x86_64.iso

set -euo pipefail
ISO="${1:?path to .iso required}"
DISK="${2:-/tmp/nullxes-test.qcow2}"

if [[ ! -f "$DISK" ]]; then
    qemu-img create -f qcow2 "$DISK" 32G
fi

# OVMF firmware is shipped by `edk2-ovmf` on Arch.
OVMF="${OVMF:-/usr/share/edk2-ovmf/x64/OVMF.4m.fd}"
if [[ ! -f "$OVMF" ]]; then
    echo "OVMF firmware not found at $OVMF (set \$OVMF)" >&2
    exit 1
fi

exec qemu-system-x86_64 \
    -enable-kvm \
    -cpu host \
    -smp 4 \
    -m 4G \
    -bios "$OVMF" \
    -drive file="$ISO",media=cdrom,readonly=on \
    -drive file="$DISK",if=virtio,format=qcow2 \
    -device virtio-vga-gl \
    -display gtk,gl=on \
    -device intel-hda \
    -device hda-output \
    -netdev user,id=n0 \
    -device virtio-net,netdev=n0 \
    -boot d
