#!/bin/sh
set -eu
cd "$(dirname "$0")"
prefix=${PREFIX:-"$HOME/.local"}
install -Dm755 bin/cliprust "$prefix/bin/cliprust"
cp -R share "$prefix/"
printf 'Installed to %s. Add %s/bin to PATH if needed.\n' "$prefix" "$prefix"
