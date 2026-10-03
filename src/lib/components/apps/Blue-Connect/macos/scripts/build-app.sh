#!/usr/bin/env bash
# Builds a double-clickable "Blue Connect.app" (universal: Apple silicon + Intel)
# and zips it. Usage:  scripts/build-app.sh [version]      (default 0.1.0)
# Output:  dist/Blue-Connect-macos.zip  (+ dist/Blue Connect.app)
set -euo pipefail

cd "$(dirname "$0")/.."
VERSION="${1:-${BC_VERSION:-0.1.0}}"
APP="dist/Blue Connect.app"

echo "▸ swift build (release, arm64 + x86_64)"
swift build -c release --arch arm64 --arch x86_64
BIN="$(swift build -c release --arch arm64 --arch x86_64 --show-bin-path)/BlueConnect"
[ -f "$BIN" ] || { echo "binary not found at $BIN"; exit 1; }

echo "▸ assembling $APP"
rm -rf dist && mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
cp "$BIN" "$APP/Contents/MacOS/BlueConnect"
sed "s/__VERSION__/${VERSION}/g" Resources/Info.plist > "$APP/Contents/Info.plist"

echo "▸ ad-hoc code signing (so the keychain and Local Network permission attach to a stable identity)"
codesign --force --deep --sign - "$APP"
codesign --verify --deep --strict "$APP"

echo "▸ zipping"
ditto -c -k --keepParent "$APP" "dist/Blue-Connect-macos.zip"
echo "✔ dist/Blue-Connect-macos.zip"
