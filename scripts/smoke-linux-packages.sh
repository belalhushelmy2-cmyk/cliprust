#!/usr/bin/env bash
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
output=$(realpath "${CLIPRUST_OUTPUT:-dist/releases/$version}")
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
[[ $(dpkg-deb -f "$output/cliprust_${version}-1_amd64.deb" Architecture) == amd64 ]]
dpkg-deb -x "$output/cliprust_${version}-1_amd64.deb" "$work/deb"
"$work/deb/usr/bin/cliprust" --version
rpm -qp --qf '%{NAME} %{VERSION} %{ARCH}\n' "$output/cliprust-$version-1.x86_64.rpm"
rpm -qpl "$output/cliprust-$version-1.x86_64.rpm" | grep -Fx /usr/bin/cliprust
tar -xzf "$output/cliprust-$version-linux-x86_64.tar.gz" -C "$work"
PREFIX="$work/prefix" "$work/cliprust-$version-linux-x86_64/install.sh"
"$work/prefix/bin/cliprust" --database "$work/history.db" add 'Package acceptance مرحبا ∇'
"$work/prefix/bin/cliprust" --database "$work/history.db" list | grep -F 'Package acceptance'
PREFIX="$work/prefix" "$work/cliprust-$version-linux-x86_64/uninstall.sh"
[[ ! -e "$work/prefix/bin/cliprust" && -f "$work/history.db" ]]
"$output/Cliprust-$version-linux-x86_64.AppImage" --appimage-extract-and-run --version
echo 'PASS: Linux package metadata, payload launch, tar install/uninstall, SQLite Unicode, and AppImage launch'
