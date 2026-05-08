#!/usr/bin/env bash
# Install NULLXES binaries + session file into standard system locations.
# Run as root (sudo) after a release build.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."
BIN="$ROOT/target/release"

install -Dm755 "$BIN/frame"            /usr/local/bin/frame
install -Dm755 "$BIN/nullxes-panel"    /usr/local/bin/nullxes-panel
install -Dm755 "$BIN/nullxes-launcher" /usr/local/bin/nullxes-launcher
install -Dm755 "$SCRIPT_DIR/start-nullxes.sh" /usr/local/bin/nullxes-session

# Wayland session entry for gdm / sddm / ly.
install -Dm644 "$ROOT/sessions/nullxes.desktop" \
    /usr/share/wayland-sessions/nullxes.desktop

# Default config (non-clobbering).
install -Dm644 -b "$ROOT/config/frame.toml" \
    /etc/nullxes/frame.toml
install -Dm644 -b "$ROOT/config/theme.toml" \
    /etc/nullxes/theme.toml

echo "NULLXES session installed."
echo "Select 'NULLXES' at the login manager, or run: nullxes-session"
