#!/usr/bin/env bash
# Package a prebuilt baseline binary. Run inside the oldest supported build image.
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
arch=$(uname -m)
[[ "$arch" == x86_64 ]] || { echo 'This release script currently supports x86_64 only.' >&2; exit 1; }
binary=${CLIPRUST_BINARY:-target/release/cliprust}
binary=$(realpath "$binary")
output=$(realpath -m "${CLIPRUST_OUTPUT:-dist/releases/$version}")
mkdir -p "$output"
python3 scripts/third-party-notices.py "$output/THIRD-PARTY-NOTICES.txt"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
stage="$work/root"
scripts/stage-linux-runtime.sh "$binary" "$stage/usr/lib/cliprust" "$output"
mkdir -p "$stage/usr/bin"
cat > "$stage/usr/bin/cliprust" <<'RUN'
#!/bin/sh
prefix=$(dirname "$(dirname "$(readlink -f "$0")")")
exec "$prefix/lib/cliprust/AppRun" "$@"
RUN
chmod +x "$stage/usr/bin/cliprust"
install -Dm644 packaging/linux/cliprust.desktop "$stage/usr/share/applications/cliprust.desktop"
install -Dm644 packaging/linux/cliprust.svg "$stage/usr/share/icons/hicolor/scalable/apps/cliprust.svg"
install -Dm644 packaging/linux/org.cliprust.Cliprust.metainfo.xml "$stage/usr/share/metainfo/org.cliprust.Cliprust.metainfo.xml"
mkdir -p "$stage/usr/share/doc/cliprust" "$stage/usr/share/licenses/cliprust"
cp README.md TEST_RESULTS.md SWAY_INTEGRATION.md KDE_INTEGRATION.md RELEASES.md protocols/virtual-keyboard-unstable-v1.xml "$stage/usr/share/doc/cliprust/"
cp LICENSE "$stage/usr/share/licenses/cliprust/LICENSE"
cp "$output/THIRD-PARTY-NOTICES.txt" "$stage/usr/share/licenses/cliprust/"
# Refuse accidentally packaging a binary from a newer host as a baseline release.
glibc=$(readelf --version-info "$binary" | grep -oE 'GLIBC_[0-9.]+' | sort -Vu | tail -1 | cut -d_ -f2)
[[ $(printf '%s\n' "$glibc" 2.35 | sort -V | tail -1) == 2.35 ]] || { echo "Binary needs glibc $glibc; rebuild on Ubuntu 22.04." >&2; exit 1; }
"$binary" --version
# The extensionless download is now a self-extracting runtime, not a bare ELF.
python3 scripts/package-standalone.py "$stage/usr/lib/cliprust" "$output/cliprust-$version-linux-x86_64"
portable="$work/cliprust-$version-linux-x86_64"
mkdir -p "$portable"
cp -a "$stage/usr/"* "$portable/"
cp packaging/release/install.sh packaging/release/uninstall.sh RELEASES.md "$portable/"
tar -C "$work" -czf "$output/cliprust-$version-linux-x86_64.tar.gz" "$(basename "$portable")"
mkdir -p "$stage/DEBIAN"
cat > "$stage/DEBIAN/control" <<CONTROL
Package: cliprust
Version: $version-2
Architecture: amd64
Maintainer: Cliprust contributors
Section: utils
Priority: optional
Installed-Size: $(du -sk "$stage/usr" | cut -f1)
Depends: libc6 (>= 2.35), coreutils, dash
Recommends: xdg-desktop-portal, xdg-desktop-portal-gtk, xdg-utils
Description: Native local clipboard history
 Rust and Slint desktop clipboard history with local SQLite storage.
CONTROL
dpkg-deb --root-owner-group --build "$stage" "$output/cliprust_${version}-2_amd64.deb"
rm -rf "$stage/DEBIAN"
# RPM uses SONAME capabilities so Fedora and openSUSE package naming both work.
mkdir -p "$work/rpm/BUILD" "$work/rpm/RPMS" "$work/rpm/SOURCES" "$work/rpm/SPECS" "$work/rpm/SRPMS" "$work/rpm/BUILDROOT"
cat > "$work/rpm/SPECS/cliprust.spec" <<SPEC
Name: cliprust
Version: $version
Release: 2
Summary: Native local clipboard history
License: GPL-3.0-only
BuildArch: x86_64
AutoReqProv: no
Requires: libc.so.6(GLIBC_2.35)(64bit), /bin/sh, /usr/bin/readlink
Recommends: xdg-desktop-portal, xdg-utils
%description
Rust and Slint desktop clipboard history with local SQLite storage.
%install
mkdir -p %{buildroot}
cp -a $stage/usr %{buildroot}/
%files
/usr/bin/cliprust
/usr/lib/cliprust
/usr/share/applications/cliprust.desktop
/usr/share/icons/hicolor/scalable/apps/cliprust.svg
/usr/share/metainfo/org.cliprust.Cliprust.metainfo.xml
/usr/share/doc/cliprust
/usr/share/licenses/cliprust
SPEC
rpmbuild --define "_topdir $work/rpm" --define '_build_id_links none' --define '__os_install_post %{nil}' -bb "$work/rpm/SPECS/cliprust.spec"
cp "$work/rpm/RPMS/x86_64/cliprust-$version-2.x86_64.rpm" "$output/"
cat > "$stage/.PKGINFO" <<PKGINFO
pkgname = cliprust
pkgbase = cliprust
pkgver = $version-2
pkgdesc = Native local clipboard history
builddate = $(date +%s)
packager = Cliprust contributors
size = $(du -sb "$stage/usr" | cut -f1)
arch = x86_64
license = GPL-3.0-only
depend = glibc>=2.35
depend = coreutils
optdepend = xdg-desktop-portal: desktop permission and file chooser services
optdepend = xdg-desktop-portal-gtk: generic file chooser backend
optdepend = xdg-utils: open links using the default application
PKGINFO
tar --zstd --owner=0 --group=0 -C "$stage" -cf "$output/cliprust-$version-2-x86_64.pkg.tar.zst" .PKGINFO usr
printf 'Linux baseline: Ubuntu 22.04, x86_64, glibc >= %s\n' "$glibc" > "$output/BUILD-INFO.txt"
rustc --version >> "$output/BUILD-INFO.txt"
cp RELEASES.md "$output/README.md"
echo "Linux packages: $output"
