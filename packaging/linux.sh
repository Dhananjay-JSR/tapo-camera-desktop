#!/usr/bin/env bash
# Build a Linux tarball (binary + .desktop + icon) for tapo-desktop. Run on Linux.
set -euo pipefail
cd "$(dirname "$0")/.."
VER="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
STAGE="dist/tapo-desktop-$VER-linux"
cargo build --release -p tapo-desktop
rm -rf "$STAGE"; mkdir -p "$STAGE"
cp target/release/tapo-desktop "$STAGE/"
cp assets/icon.png "$STAGE/tapo-desktop.png"
cat > "$STAGE/tapo-desktop.desktop" <<DESK
[Desktop Entry]
Type=Application
Name=Tapo Desktop
Comment=Remote live view for TP-Link Tapo cameras
Exec=tapo-desktop
Icon=tapo-desktop
Categories=AudioVideo;Utility;
DESK
cat > "$STAGE/INSTALL.txt" <<TXT
Run ./tapo-desktop . To add to your menu, copy tapo-desktop to ~/.local/bin,
tapo-desktop.png to ~/.local/share/icons, and tapo-desktop.desktop to ~/.local/share/applications.
TXT
tar czf "dist/tapo-desktop-$VER-linux-x86_64.tar.gz" -C dist "tapo-desktop-$VER-linux"
echo "built: dist/tapo-desktop-$VER-linux-x86_64.tar.gz"
