#!/usr/bin/env bash
# Run in the same Ubuntu baseline environment as the Linux binary build.
set -euo pipefail
cd "$(dirname "$0")/.."
: "${APPIMAGETOOL:?Set APPIMAGETOOL}"
: "${APPIMAGE_RUNTIME:?Set APPIMAGE_RUNTIME}"
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
binary=$(realpath "${CLIPRUST_BINARY:-target/release/cliprust}")
output=$(realpath -m "${CLIPRUST_OUTPUT:-dist/releases/$version}")
mkdir -p "$output"
python3 scripts/third-party-notices.py "$output/THIRD-PARTY-NOTICES.txt"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
appdir="$work/AppDir"
scripts/stage-linux-runtime.sh "$binary" "$appdir" "$output"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGETOOL" --no-appstream --runtime-file "$APPIMAGE_RUNTIME" "$appdir" "$output/Cliprust-$version-linux-x86_64.AppImage"
