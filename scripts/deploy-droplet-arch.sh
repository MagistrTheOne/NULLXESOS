#!/usr/bin/env bash
# deploy-droplet-arch.sh
# One-shot pipeline for Arch Linux droplet:
#   1) install required packages
#   2) build workspace
#   3) build ISO via archiso
#   4) run smoke test in QEMU (serial expect harness)
#
# Usage:
#   bash scripts/deploy-droplet-arch.sh
# Optional env:
#   REPO_DIR=/opt/NULLXESOS
#   SKIP_SMOKE=1
#   SOURCE_DATE_EPOCH=...

set -euo pipefail

if [[ ! -f /etc/arch-release ]]; then
    echo "[nullxes] this script supports Arch Linux only." >&2
    exit 1
fi

if [[ "${EUID}" -ne 0 ]]; then
    echo "[nullxes] run as root (sudo -i)." >&2
    exit 1
fi

REPO_DIR="${REPO_DIR:-/opt/NULLXESOS}"
SKIP_SMOKE="${SKIP_SMOKE:-0}"

if [[ ! -d "${REPO_DIR}" ]]; then
    echo "[nullxes] repo dir not found: ${REPO_DIR}" >&2
    echo "[nullxes] clone first: git clone https://github.com/MagistrTheOne/NULLXESOS.git ${REPO_DIR}" >&2
    exit 1
fi

cd "${REPO_DIR}"

echo "[nullxes] syncing packages..."
pacman -Syu --noconfirm
pacman -S --noconfirm --needed \
    base-devel git rust pkgconf clang curl \
    wayland wayland-protocols libxkbcommon \
    libinput seatd libdrm mesa libgbm \
    pipewire wireplumber dbus pam fontconfig \
    archiso qemu-system-x86 edk2-ovmf expect

echo "[nullxes] ensuring local package repo path..."
mkdir -p /srv/nullxes-repo/x86_64
if [[ ! -f /srv/nullxes-repo/x86_64/nullxes.db.tar.gz ]]; then
    repo-add /srv/nullxes-repo/x86_64/nullxes.db.tar.gz
fi

if [[ -z "${SOURCE_DATE_EPOCH:-}" ]]; then
    SOURCE_DATE_EPOCH="$(git log -1 --pretty=%ct)"
    export SOURCE_DATE_EPOCH
fi
echo "[nullxes] SOURCE_DATE_EPOCH=${SOURCE_DATE_EPOCH}"

echo "[nullxes] building Rust workspace..."
cargo build --workspace --release --locked
cargo test --workspace --locked
cargo run --release --bin xtask -- check-theme

echo "[nullxes] building ISO..."
bash ./iso/scripts/build-iso.sh

ISO_PATH="$(ls -1 ./iso/out/*.iso 2>/dev/null | head -n 1 || true)"
if [[ -z "${ISO_PATH}" ]]; then
    echo "[nullxes] ISO was not produced in ./iso/out" >&2
    exit 1
fi
echo "[nullxes] ISO ready: ${ISO_PATH}"

if [[ "${SKIP_SMOKE}" == "1" ]]; then
    echo "[nullxes] smoke test skipped (SKIP_SMOKE=1)."
    exit 0
fi

echo "[nullxes] running smoke test..."
qemu-img create -f qcow2 /tmp/nullxes-smoke.qcow2 16G >/dev/null
bash ./iso/scripts/smoke.expect "${ISO_PATH}" /tmp/nullxes-smoke.qcow2
echo "[nullxes] smoke test passed."
