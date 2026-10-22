#!/bin/sh
set -eu
prefix=${PREFIX:-"$HOME/.local"}
rm -f "$prefix/bin/cliprust" "$prefix/share/applications/cliprust.desktop" "$prefix/share/icons/hicolor/scalable/apps/cliprust.svg" "$prefix/share/metainfo/org.cliprust.Cliprust.metainfo.xml"
rm -rf "$prefix/share/doc/cliprust" "$prefix/share/licenses/cliprust"
printf 'Removed Cliprust from %s. Clipboard history was preserved.\n' "$prefix"
