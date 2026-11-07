#!/usr/bin/env bash
# Build a double-clickable macOS .app (zipped for distribution — no installer). Run on macOS.
set -euo pipefail
cd "$(dirname "$0")/.."
APP="Tapo Desktop"; BIN="tapo-desktop"
VER="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
OUT="dist"; APPDIR="$OUT/$APP.app"

cargo build --release -p tapo-desktop
rm -rf "$APPDIR"; mkdir -p "$APPDIR/Contents/MacOS" "$APPDIR/Contents/Resources"
cp "target/release/$BIN" "$APPDIR/Contents/MacOS/$BIN"
cp assets/icon.icns "$APPDIR/Contents/Resources/icon.icns"
cat > "$APPDIR/Contents/Info.plist" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
  <key>CFBundleName</key><string>$APP</string>
  <key>CFBundleDisplayName</key><string>$APP</string>
  <key>CFBundleIdentifier</key><string>dev.dhananjay-jsr.tapo-desktop</string>
  <key>CFBundleExecutable</key><string>$BIN</string>
  <key>CFBundleIconFile</key><string>icon</string>
  <key>CFBundlePackageType</key><string>APPL</string>
  <key>CFBundleShortVersionString</key><string>$VER</string>
  <key>CFBundleVersion</key><string>$VER</string>
  <key>LSMinimumSystemVersion</key><string>10.15</string>
  <key>NSHighResolutionCapable</key><true/>
  <key>NSMicrophoneUsageDescription</key><string>Talk-back sends your voice to the camera speaker.</string>
</dict></plist>
PLIST
# Ad-hoc sign so macOS shows the normal "unidentified developer" (right-click > Open) flow
# instead of "is damaged" on download. (No Apple Developer certificate / notarization.)
codesign --force --deep --sign - "$APPDIR" 2>/dev/null || true

# Zip the .app (a bundle is a folder; zipping keeps it a single download). No .dmg, no installer.
( cd "$OUT" && ditto -c -k --sequesterRsrc --keepParent "$APP.app" "Tapo-Desktop-$VER-macos.zip" )
echo "built: $APPDIR  and  $OUT/Tapo-Desktop-$VER-macos.zip"
