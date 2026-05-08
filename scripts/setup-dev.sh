#!/usr/bin/env bash
# NULLXES OS — Stage 1 development environment setup (Debian/Ubuntu/Arch)
set -euo pipefail

DISTRO="unknown"
if [ -f /etc/arch-release ]; then DISTRO="arch"
elif [ -f /etc/debian_version ]; then DISTRO="debian"
fi

echo "[nullxes] detected: $DISTRO"

install_debian() {
    sudo apt-get update
    sudo apt-get install -y \
        build-essential \
        pkg-config \
        libwayland-dev \
        libxkbcommon-dev \
        libinput-dev \
        libudev-dev \
        libgbm-dev \
        libdrm-dev \
        libegl-dev \
        libgl-dev \
        libgles-dev \
        libseat-dev \
        libx11-dev \
        libxcb1-dev \
        clang \
        curl
}

install_arch() {
    sudo pacman -Sy --noconfirm \
        base-devel \
        wayland \
        wayland-protocols \
        libxkbcommon \
        libinput \
        systemd \
        mesa \
        seatd \
        libx11 \
        libxcb \
        clang \
        curl
}

case "$DISTRO" in
    debian) install_debian ;;
    arch)   install_arch   ;;
    *)      echo "[nullxes] Unknown distro — install deps manually"; exit 1 ;;
esac

# Install Rust if missing.
if ! command -v cargo &>/dev/null; then
    echo "[nullxes] installing Rust toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain stable
    source "$HOME/.cargo/env"
fi

rustup component add rust-src clippy rustfmt

echo "[nullxes] dev environment ready"
echo ""
echo "Build commands:"
echo "  cargo build --workspace              # debug build"
echo "  cargo build --workspace --release    # optimised build"
echo "  cargo test --workspace               # run tests"
echo ""
echo "Run FRAME in dev mode (inside existing Wayland/X11 session):"
echo "  NULLXES_LOG=debug cargo run -p frame"
echo ""
echo "Run PANEL (requires a running Wayland compositor with layer-shell):"
echo "  cargo run -p panel"
echo ""
echo "Run LAUNCHER:"
echo "  cargo run -p launcher"
