#!/usr/bin/env bash
# Builds the DMG, publishes it as a GitHub release, and updates the Homebrew tap.
# Bump the version in src-tauri/Cargo.toml first. Requires the gh CLI, logged in.
set -euo pipefail
cd "$(dirname "$0")/.."

REPO=minhchienwikipedia/sweep
TAP=minhchienwikipedia/homebrew-tap

./scripts/make-dmg.sh
VERSION=$(/usr/libexec/PlistBuddy -c "Print CFBundleShortVersionString" dist/Sweep.app/Contents/Info.plist)
DMG="dist/Sweep-$VERSION.dmg"
SHA=$(shasum -a 256 "$DMG" | awk '{print $1}')

# Point the cask at this release
sed -i '' -E "s/^  version \".*\"/  version \"$VERSION\"/; s/^  sha256 \".*\"/  sha256 \"$SHA\"/" homebrew/sweep.rb

gh release create "v$VERSION" "$DMG" --repo "$REPO" --title "Sweep $VERSION" --generate-notes

WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
gh repo clone "$TAP" "$WORK/tap" -- --quiet
mkdir -p "$WORK/tap/Casks"
cp homebrew/sweep.rb "$WORK/tap/Casks/"
git -C "$WORK/tap" add Casks/sweep.rb
git -C "$WORK/tap" commit --quiet -m "sweep $VERSION"
git -C "$WORK/tap" push --quiet

echo "Released $VERSION. Commit the updated homebrew/sweep.rb too."
