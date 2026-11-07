#!/usr/bin/env bash
# Build a single standalone Linux executable (no installer). Run on Linux.
set -euo pipefail
cd "$(dirname "$0")/.."
VER="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
cargo build --release -p tapo-desktop
mkdir -p dist
cp target/release/tapo-desktop "dist/tapo-desktop-$VER-linux-x86_64"
chmod +x "dist/tapo-desktop-$VER-linux-x86_64"
echo "built: dist/tapo-desktop-$VER-linux-x86_64"
