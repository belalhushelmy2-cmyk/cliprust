#!/bin/sh
set -eu
cd "$(dirname "$0")"
prefix=${PREFIX:-"$HOME/.local"}
install -Dm755 bin/cliprust "$prefix/bin/cliprust"
mkdir -p "$prefix/lib"
# Replace only this application's private runtime, including on upgrades.
rm -rf "$prefix/lib/cliprust"
cp -R lib/cliprust "$prefix/lib/"
cp -R share "$prefix/"
# Desktop launchers do not necessarily inherit ~/.local/bin in PATH.
escaped=$(printf '%s' "$prefix/bin/cliprust" | sed 's/[\\"`$]/\\&/g; s/%/%%/g')
while IFS= read -r line; do
    case "$line" in
        Exec=*) printf 'Exec="%s" --show\n' "$escaped" ;;
        *) printf '%s\n' "$line" ;;
    esac
done < share/applications/cliprust.desktop > "$prefix/share/applications/cliprust.desktop"
printf 'Installed to %s. Add %s/bin to PATH if needed.\n' "$prefix" "$prefix"
