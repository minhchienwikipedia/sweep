#!/usr/bin/env bash
# Builds an optimized release and packages it as dist/Sweep.app.
set -euo pipefail
cd "$(dirname "$0")/.."
[ -f "$HOME/.cargo/env" ] && source "$HOME/.cargo/env"

APP=Sweep
ID=com.minhchien.sweep
VERSION=$(grep -m1 '^version' src-tauri/Cargo.toml | cut -d'"' -f2)

[ -f src-tauri/icons/icon.icns ] || ./scripts/make-icon.sh
cargo build --release --manifest-path src-tauri/Cargo.toml

OUT="dist/$APP.app"
rm -rf "$OUT"
mkdir -p "$OUT/Contents/MacOS" "$OUT/Contents/Resources"
cp src-tauri/target/release/sweep "$OUT/Contents/MacOS/$APP"
cp src-tauri/icons/icon.icns "$OUT/Contents/Resources/icon.icns"

cat > "$OUT/Contents/Info.plist" <<EOF
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>CFBundleName</key><string>$APP</string>
  <key>CFBundleDisplayName</key><string>$APP</string>
  <key>CFBundleIdentifier</key><string>$ID</string>
  <key>CFBundleExecutable</key><string>$APP</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VERSION</string>
  <key>CFBundleVersion</key><string>$VERSION</string>
  <key>LSMinimumSystemVersion</key><string>11.0</string>
  <key>LSApplicationCategoryType</key><string>public.app-category.utilities</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSDesktopFolderUsageDescription</key><string>Sweep looks for project folders like node_modules to free up space.</string>
  <key>NSDocumentsFolderUsageDescription</key><string>Sweep looks for project folders like node_modules to free up space.</string>
  <key>NSDownloadsFolderUsageDescription</key><string>Sweep finds leftover installers and project folders to free up space.</string>
</dict>
</plist>
EOF

codesign --force --sign - "$OUT"
echo "Built $OUT ($(du -sh "$OUT" | cut -f1))"
