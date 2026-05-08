#!/usr/bin/env bash
# start-nullxes.sh — launch a NULLXES session.
#
# Stage 1 / hybrid process model: FRAME owns PANEL in-process; LAUNCHER /
# NX-NOTIF / NX-LOCK / NX-SETTINGS / NX-GREET are external clients that
# attach via the wayland socket FRAME publishes at:
#
#     $XDG_RUNTIME_DIR/nullxes/wayland-display
#
# Backend selection:
#   NULLXES_BACKEND=drm    bare-metal DRM/KMS (default on TTY)
#   NULLXES_BACKEND=winit  inside an existing Wayland/X11 session (default for dev)
#
# This script runs FRAME in the foreground and propagates SIGTERM on exit.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."

if [[ "${NULLXES_BUILD:-0}" == "1" ]]; then
    echo "[nullxes] cargo build --release --workspace"
    cargo build --manifest-path "$ROOT/Cargo.toml" --release --workspace --locked
fi

# Prefer installed binaries if present (via nullxes-frame package).
if command -v frame >/dev/null 2>&1 && [[ "${NULLXES_LOCAL:-0}" != "1" ]]; then
    BIN_DIR="$(dirname "$(command -v frame)")"
else
    BIN_DIR="$ROOT/target/release"
fi

# ── Environment ──────────────────────────────────────────────────────────────
export XDG_SESSION_TYPE="wayland"
export XDG_CURRENT_DESKTOP="nullxes"
export QT_QPA_PLATFORM="wayland;xcb"
export GDK_BACKEND="wayland,x11"
export SDL_VIDEODRIVER="wayland"
export CLUTTER_BACKEND="wayland"
export MOZ_ENABLE_WAYLAND="1"
export NULLXES_LOG="${NULLXES_LOG:-frame=info,launcher=info,smithay=warn}"
export RUST_BACKTRACE="${RUST_BACKTRACE:-1}"
RUNTIME_DIR="${XDG_RUNTIME_DIR:-/run/user/$(id -u)}"
mkdir -p "$RUNTIME_DIR/nullxes"

# Backend auto-detect.
if [[ "${NULLXES_BACKEND:-auto}" == "auto" ]]; then
    if [[ -z "${WAYLAND_DISPLAY:-}" && -z "${DISPLAY:-}" ]]; then
        export NULLXES_BACKEND="drm"
    else
        export NULLXES_BACKEND="winit"
    fi
fi

# ── Run FRAME in the foreground ──────────────────────────────────────────────
echo "[nullxes] FRAME (backend=$NULLXES_BACKEND)"
exec "$BIN_DIR/frame"
