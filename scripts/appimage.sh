#!/usr/bin/env bash
# Use the same complete runtime as release packages.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build --release --locked
scripts/package-appimage.sh
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
cp "${CLIPRUST_OUTPUT:-dist/releases/$version}/Cliprust-$version-linux-x86_64.AppImage" dist/Cliprust.next.AppImage
mv dist/Cliprust.next.AppImage dist/Cliprust.AppImage
sha256sum dist/Cliprust.AppImage > dist/Cliprust.AppImage.sha256
