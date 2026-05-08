#!/usr/bin/env bash
# Download Inter and JetBrains Mono into assets/fonts/.
# Run once before the first build.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OUT="$SCRIPT_DIR/../assets/fonts"
mkdir -p "$OUT"

need() { command -v "$1" &>/dev/null || { echo "ERROR: $1 not found"; exit 1; }; }
need curl
need unzip

# ── Inter (from GitHub release) ───────────────────────────────────────────────
INTER_VER="4.0"
INTER_ZIP="$OUT/Inter-$INTER_VER.zip"
if [ ! -f "$OUT/Inter-Regular.ttf" ]; then
    echo "[fonts] downloading Inter $INTER_VER..."
    curl -fsSL "https://github.com/rsms/inter/releases/download/v${INTER_VER}/Inter-${INTER_VER}.zip" \
        -o "$INTER_ZIP"
    # The zip layout: Inter Desktop/Inter-Regular.ttf etc.
    # Try both known layouts.
    if unzip -p "$INTER_ZIP" "Inter Desktop/Inter-Regular.ttf" > "$OUT/Inter-Regular.ttf" 2>/dev/null; then
        unzip -p "$INTER_ZIP" "Inter Desktop/Inter-Medium.ttf"  > "$OUT/Inter-Medium.ttf"
        echo "[fonts] Inter extracted (Desktop layout)"
    elif unzip -p "$INTER_ZIP" "Inter-Regular.ttf" > "$OUT/Inter-Regular.ttf" 2>/dev/null; then
        unzip -p "$INTER_ZIP" "Inter-Medium.ttf" > "$OUT/Inter-Medium.ttf"
        echo "[fonts] Inter extracted (flat layout)"
    else
        echo "[fonts] WARNING: could not find Inter TTF in zip — listing contents:"
        unzip -l "$INTER_ZIP" | grep -i "\.ttf" | head -20
        echo "        Manually copy Inter-Regular.ttf and Inter-Medium.ttf to $OUT/"
        rm -f "$OUT/Inter-Regular.ttf"
    fi
    rm -f "$INTER_ZIP"
else
    echo "[fonts] Inter already present"
fi

# ── JetBrains Mono (from GitHub release) ─────────────────────────────────────
JBMONO_VER="2.304"
JBMONO_ZIP="$OUT/JetBrainsMono-$JBMONO_VER.zip"
if [ ! -f "$OUT/JetBrainsMono-Regular.ttf" ]; then
    echo "[fonts] downloading JetBrains Mono $JBMONO_VER..."
    curl -fsSL \
        "https://github.com/JetBrains/JetBrainsMono/releases/download/v${JBMONO_VER}/JetBrainsMono-${JBMONO_VER}.zip" \
        -o "$JBMONO_ZIP"
    # Layout: fonts/ttf/JetBrainsMono-Regular.ttf
    if unzip -p "$JBMONO_ZIP" "fonts/ttf/JetBrainsMono-Regular.ttf" > "$OUT/JetBrainsMono-Regular.ttf" 2>/dev/null; then
        echo "[fonts] JetBrains Mono extracted"
    else
        echo "[fonts] WARNING: could not find JetBrainsMono-Regular.ttf in zip"
        unzip -l "$JBMONO_ZIP" | grep -i "regular" | head -10
        rm -f "$OUT/JetBrainsMono-Regular.ttf"
    fi
    rm -f "$JBMONO_ZIP"
else
    echo "[fonts] JetBrains Mono already present"
fi

echo ""
echo "[fonts] contents of $OUT:"
ls -lh "$OUT/" 2>/dev/null || echo "(empty)"
echo ""
echo "[fonts] done — you can now build the project."
