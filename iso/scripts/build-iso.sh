#!/usr/bin/env bash
# build-iso.sh — assemble the NULLXES live + installer ISO via archiso.
#
# Prerequisites:
#   - Arch Linux build host with `archiso` package installed.
#   - Local repo populated under /srv/nullxes-repo/x86_64/ with signed pkgs.
#   - SOURCE_DATE_EPOCH exported (set to commit timestamp by CI).

set -euo pipefail
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT="$SCRIPT_DIR/.."
PROFILE="$ROOT/profile"
WORK_DIR="$ROOT/work"
OUT_DIR="$ROOT/out"

if ! command -v mkarchiso >/dev/null 2>&1; then
    echo "mkarchiso not found — install archiso (pacman -S archiso)" >&2
    exit 1
fi

if [[ -z "${SOURCE_DATE_EPOCH:-}" ]]; then
    echo "WARNING: SOURCE_DATE_EPOCH not set; build will not be reproducible." >&2
fi

mkdir -p "$WORK_DIR" "$OUT_DIR"
sudo mkarchiso -v -w "$WORK_DIR" -o "$OUT_DIR" "$PROFILE"

ls -lh "$OUT_DIR"
