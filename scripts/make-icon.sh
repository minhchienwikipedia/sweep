#!/usr/bin/env bash
# Generates src-tauri/icons/icon.png and icon.icns.
set -euo pipefail
cd "$(dirname "$0")/.."

ICONS=src-tauri/icons
TMP=$(mktemp -d)
swift scripts/make-icon.swift "$ICONS/icon.png"

SET="$TMP/icon.iconset"
mkdir -p "$SET"
for s in 16 32 128 256 512; do
  sips -z $s $s "$ICONS/icon.png" --out "$SET/icon_${s}x${s}.png" >/dev/null
  sips -z $((s * 2)) $((s * 2)) "$ICONS/icon.png" --out "$SET/icon_${s}x${s}@2x.png" >/dev/null
done
iconutil -c icns "$SET" -o "$ICONS/icon.icns"
rm -rf "$TMP"
echo "Icons written to $ICONS"
