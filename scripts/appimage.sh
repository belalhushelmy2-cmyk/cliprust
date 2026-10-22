#!/usr/bin/env bash
set -euo pipefail
: "${APPIMAGETOOL:?Set APPIMAGETOOL to an installed, trusted appimagetool executable}"
cargo build --release --locked
mkdir -p dist
appdir=$(mktemp -d "$PWD/dist/cliprust-appdir.XXXXXX")
mkdir -p "$appdir/usr/bin" "$appdir/usr/share/icons/hicolor/scalable/apps" "$appdir/usr/share/doc/cliprust" "$appdir/usr/share/metainfo" "$appdir/usr/share/applications"
install -m755 target/release/cliprust "$appdir/usr/bin/cliprust"
install -m644 packaging/linux/cliprust.desktop "$appdir/cliprust.desktop"
install -m644 packaging/linux/cliprust.desktop "$appdir/usr/share/applications/cliprust.desktop"
install -m644 packaging/linux/cliprust.svg "$appdir/cliprust.svg"
install -m644 packaging/linux/cliprust.svg "$appdir/usr/share/icons/hicolor/scalable/apps/cliprust.svg"
install -m644 packaging/linux/org.cliprust.Cliprust.metainfo.xml "$appdir/usr/share/metainfo/org.cliprust.Cliprust.metainfo.xml"
install -m644 README.md LICENSE KDE_INTEGRATION.md SWAY_INTEGRATION.md protocols/virtual-keyboard-unstable-v1.xml "$appdir/usr/share/doc/cliprust/"
mkdir -p "$appdir/usr/share/doc/cliprust/kde/cliprust/contents/code"
install -m644 packaging/kde/cliprust/metadata.json "$appdir/usr/share/doc/cliprust/kde/cliprust/"
install -m644 packaging/kde/cliprust/contents/code/main.js "$appdir/usr/share/doc/cliprust/kde/cliprust/contents/code/"
cat > "$appdir/AppRun" <<'RUN'
#!/bin/sh
exec "$(dirname "$(readlink -f "$0")")/usr/bin/cliprust" "$@"
RUN
chmod +x "$appdir/AppRun"
# The project has no public homepage yet; retain metadata without inventing one.
# appimagetool treats that AppStream advisory as fatal, so validate separately.
if command -v appstreamcli >/dev/null; then
    appstreamcli validate-tree --no-net --override=url-homepage-missing=info "$appdir"
fi
runtime_args=()
if [[ -n "${APPIMAGE_RUNTIME:-}" ]]; then
    runtime_args=(--runtime-file "$APPIMAGE_RUNTIME")
fi
ARCH="${APPIMAGE_ARCH:-$(uname -m)}" APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGETOOL" --no-appstream "${runtime_args[@]}" "$appdir" "$appdir/../Cliprust.next.AppImage"
# Replace atomically so rebuilding also works while the previous image is running.
mv "$appdir/../Cliprust.next.AppImage" dist/Cliprust.AppImage
sha256sum dist/Cliprust.AppImage > dist/Cliprust.AppImage.sha256
# Build on the oldest supported distribution: system libraries are not bundled.
# Each run uses a fresh AppDir to avoid accidentally packaging stale files.
