#!/usr/bin/env bash
# Builds Sweep.app and packages it as a drag-to-install DMG in ./dist
set -euo pipefail
cd "$(dirname "$0")/.."

./scripts/bundle.sh

APP=dist/Sweep.app
ICON=src-tauri/icons/icon.icns
VERSION=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" "$APP/Contents/Info.plist")
VOLNAME="Sweep"
DMG="dist/Sweep-$VERSION.dmg"
WORK=$(mktemp -d)
STAGE="$WORK/stage"
RW_DMG="$WORK/rw.dmg"
trap 'rm -rf "$WORK"' EXIT

# Stage contents: app, Applications shortcut, hidden background
mkdir -p "$STAGE/.background"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
swift scripts/dmg-background.swift "$WORK/bg.png" 1
swift scripts/dmg-background.swift "$WORK/bg@2x.png" 2
tiffutil -cathidpicheck "$WORK/bg.png" "$WORK/bg@2x.png" -out "$STAGE/.background/background.tiff" 2>/dev/null

# Detach any stale mount of the same volume
[[ -d "/Volumes/$VOLNAME" ]] && hdiutil detach "/Volumes/$VOLNAME" -force -quiet || true

hdiutil create -quiet -srcfolder "$STAGE" -volname "$VOLNAME" -fs HFS+ -format UDRW -size 60m -ov "$RW_DMG"
MOUNT=$(hdiutil attach -readwrite -noverify -noautoopen "$RW_DMG" | awk -F'\t' '/\/Volumes\//{print $NF}')

# Lay out the Finder window (needs Automation permission for Finder the first time)
osascript <<APPLESCRIPT
tell application "Finder"
    tell disk "$VOLNAME"
        open
        set current view of container window to icon view
        set toolbar visible of container window to false
        set statusbar visible of container window to false
        set the bounds of container window to {200, 120, 840, 548}
        set opts to the icon view options of container window
        set arrangement of opts to not arranged
        set icon size of opts to 112
        set text size of opts to 13
        set background picture of opts to file ".background:background.tiff"
        set position of item "Sweep.app" of container window to {170, 210}
        set position of item "Applications" of container window to {470, 210}
        update without registering applications
        delay 1
        close
    end tell
end tell
APPLESCRIPT

# Custom volume icon (after Finder, which otherwise deletes it)
cp "$ICON" "$MOUNT/.VolumeIcon.icns"
SetFile -c icnC "$MOUNT/.VolumeIcon.icns"
SetFile -a C "$MOUNT"
sync
hdiutil detach "$MOUNT" -quiet
rm -f "$DMG"
hdiutil convert -quiet "$RW_DMG" -format UDZO -imagekey zlib-level=9 -o "$DMG"

# Custom Finder icon on the .dmg file itself (stored in extended attributes)
swift scripts/set-file-icon.swift "$ICON" "$DMG"
echo "Built $DMG"
