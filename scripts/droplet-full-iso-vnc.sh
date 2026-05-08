#!/usr/bin/env bash
# Build full NULLXES package repo + ISO on a Linux droplet and boot via VNC.
# Host requirements: docker, privileged containers.
#
# Usage:
#   bash scripts/droplet-full-iso-vnc.sh
# Optional env:
#   REPO_DIR=/opt/NULLXESOS
#   VNC_PORT=5901
#   SKIP_BOOT=1

set -euo pipefail

REPO_DIR="${REPO_DIR:-/opt/NULLXESOS}"
VNC_PORT="${VNC_PORT:-5901}"
SKIP_BOOT="${SKIP_BOOT:-0}"

if [[ ! -d "${REPO_DIR}" ]]; then
    echo "[nullxes] repo dir not found: ${REPO_DIR}" >&2
    exit 1
fi

cd "${REPO_DIR}"

docker run --rm -it --privileged -p "${VNC_PORT}:5901" \
  -v "${REPO_DIR}:/work" -w /work archlinux:latest \
  bash -lc '
set -euo pipefail

pacman -Syu --noconfirm
pacman -S --noconfirm --needed \
  base-devel git rust archiso qemu-system-x86 edk2-ovmf expect sudo zstd

# Build scripts in this repo expect unsigned local repo during development.
sed -i "s/SigLevel = Required DatabaseRequired/SigLevel = Optional TrustAll/" iso/profile/pacman.conf
find . -name "Cargo.toml" -exec sed -i "s/smithay-drm-extras = \"0.2\"/smithay-drm-extras = \"0.1\"/" {} +

# Drop stale package build trees from previous failed attempts. makepkg can
# otherwise reuse an old extracted source tree even after git pull.
find packaging/pkgbuilds iso/calamares \
  \( -name src -o -name pkg \) -type d -prune -exec rm -rf {} +
find packaging/pkgbuilds iso/calamares \
  \( -name "source.tar.zst" -o -name "nullxes-*.tar.zst" -o -name "calamares-config-nullxes-*.tar.zst" -o -name "*.pkg.tar.zst" -o -name "*.pkg.tar.zst.sig" \) \
  -type f -delete

mkdir -p /srv/nullxes-repo/x86_64
repo-add /srv/nullxes-repo/x86_64/nullxes.db.tar.gz || true

id -u builder >/dev/null 2>&1 || useradd -m builder
chown -R builder:builder /work /srv/nullxes-repo

# The repo may be checked out without Cargo.lock. Generate it in the Arch
# build environment, then include it in every source archive.
sudo -u builder -- bash -lc "cd /work && cargo generate-lockfile"

build_pkgbuild() {
  local dir="$1"
  sudo -u builder -- bash -lc "set -euo pipefail; cd \"$dir\"; rm -f ./*.pkg.tar.zst ./*.pkg.tar.zst.sig; makepkg --syncdeps --noconfirm --skippgpcheck --clean --cleanbuild --nodeps"
  cp -f "$dir"/*.pkg.tar.zst /srv/nullxes-repo/x86_64/
}

build_with_source_tar() {
  local dir="$1"
  local pkgname="$2"
  local pkgver="$3"
  sudo -u builder -- bash -lc "set -euo pipefail; rm -f \"$dir/source.tar.zst\" \"$dir/${pkgname}-${pkgver}.tar.zst\"; cd /work; tar --use-compress-program=zstd -caf \"$dir/source.tar.zst\" \
    --exclude=\"packaging/pkgbuilds/*/pkg\" \
    --exclude=\"packaging/pkgbuilds/*/src\" \
    --exclude=\"packaging/pkgbuilds/*/*.pkg.tar.zst\" \
    --exclude=\"packaging/pkgbuilds/*/*.pkg.tar.zst.sig\" \
    --exclude=\"packaging/pkgbuilds/*/source.tar.zst\" \
    --transform \"s,^,${pkgname}-${pkgver}/,\" crates Cargo.toml Cargo.lock packaging config scripts sessions iso"
  build_pkgbuild "$dir"
}

build_calamares_cfg() {
  local dir="/work/iso/calamares"
  sudo -u builder -- bash -lc "set -euo pipefail; cd \"$dir\"; rm -f source.tar.zst calamares-config-nullxes-*.tar.zst; tar --use-compress-program=zstd -caf source.tar.zst settings.conf branding modules"
  build_pkgbuild "$dir"
}

# Build local NULLXES packages (order matters for meta packages).
build_pkgbuild "/work/packaging/pkgbuilds/nullxes-keyring"
build_pkgbuild "/work/packaging/pkgbuilds/nullxes-fonts"
build_pkgbuild "/work/packaging/pkgbuilds/nullxes-icons"
build_pkgbuild "/work/packaging/pkgbuilds/nullxes-cursors"

build_with_source_tar "/work/packaging/pkgbuilds/nullxes-theme" "nullxes-theme" "0.1.0"
build_with_source_tar "/work/packaging/pkgbuilds/nullxes-frame" "nullxes-frame" "0.1.0"
build_with_source_tar "/work/packaging/pkgbuilds/nullxes-launcher" "nullxes-launcher" "0.1.0"
build_with_source_tar "/work/packaging/pkgbuilds/nullxes-slate" "nullxes-slate" "0.1.0"
build_with_source_tar "/work/packaging/pkgbuilds/nullxes-lock" "nullxes-lock" "0.1.0"
build_with_source_tar "/work/packaging/pkgbuilds/nullxes-notif" "nullxes-notif" "0.1.0"
build_with_source_tar "/work/packaging/pkgbuilds/nullxes-settings" "nullxes-settings" "0.1.0"
build_with_source_tar "/work/packaging/pkgbuilds/nullxes-greet" "nullxes-greet" "0.1.0"

build_pkgbuild "/work/packaging/pkgbuilds/nullxes-base"
build_pkgbuild "/work/packaging/pkgbuilds/nullxes-desktop"
build_calamares_cfg

repo-add /srv/nullxes-repo/x86_64/nullxes.db.tar.gz /srv/nullxes-repo/x86_64/*.pkg.tar.zst

export SOURCE_DATE_EPOCH="$(git log -1 --pretty=%ct)"
mkdir -p iso/work iso/out
mkarchiso -v -w iso/work -o iso/out iso/profile

ISO_PATH="$(ls -1 iso/out/*.iso | head -n 1)"
echo "[nullxes] ISO ready: ${ISO_PATH}"

if [[ "'"${SKIP_BOOT}"'" == "1" ]]; then
  exit 0
fi

qemu-img create -f qcow2 /tmp/nullxes-vnc.qcow2 16G >/dev/null 2>&1 || true
exec qemu-system-x86_64 \
  -accel tcg \
  -smp 2 \
  -m 4096 \
  -bios /usr/share/edk2/x64/OVMF.4m.fd \
  -drive file="${ISO_PATH}",media=cdrom,readonly=on \
  -drive file=/tmp/nullxes-vnc.qcow2,if=virtio,format=qcow2 \
  -netdev user,id=n0 \
  -device virtio-net,netdev=n0 \
  -vnc 0.0.0.0:1 \
  -display none
'