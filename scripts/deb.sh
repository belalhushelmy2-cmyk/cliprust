#!/usr/bin/env bash
set -euo pipefail

if command -v cargo-deb >/dev/null; then
  deb_cmd="$(command -v cargo-deb)"
elif [[ -x "$HOME/.cargo/bin/cargo-deb" ]]; then
  deb_cmd="$HOME/.cargo/bin/cargo-deb"
else
  echo "cargo-deb is required (install with: cargo install cargo-deb --locked)" >&2
  exit 127
fi

cargo build --release --locked
mkdir -p dist
"$deb_cmd" --no-build --locked --output dist
