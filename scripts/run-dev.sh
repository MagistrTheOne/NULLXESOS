#!/usr/bin/env bash
# run-dev.sh — fast iteration on FRAME inside an existing X11 / Wayland session.
#
# Builds in debug mode and runs FRAME in winit backend with a debug log filter.
# After this script exits, the FRAME process exits cleanly and removes its
# wayland-display file.

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."

export NULLXES_LOG="${NULLXES_LOG:-frame=debug,launcher=info,smithay=warn}"
export NULLXES_BACKEND="${NULLXES_BACKEND:-winit}"
export RUST_BACKTRACE=1

cargo build --manifest-path "$ROOT/Cargo.toml" --workspace
exec "$ROOT/target/debug/frame"
