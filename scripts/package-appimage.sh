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
mkdir -p "$appdir/usr/bin" "$appdir/usr/lib" "$appdir/usr/share/doc/cliprust" "$appdir/usr/share/X11"
cp "$binary" "$appdir/usr/bin/cliprust"
cp packaging/linux/cliprust.desktop "$appdir/cliprust.desktop"
cp packaging/linux/cliprust.svg "$appdir/cliprust.svg"
cp LICENSE RELEASES.md TEST_RESULTS.md SWAY_INTEGRATION.md KDE_INTEGRATION.md protocols/virtual-keyboard-unstable-v1.xml "$appdir/usr/share/doc/cliprust/"
cp "$output/THIRD-PARTY-NOTICES.txt" "$appdir/usr/share/doc/cliprust/"
mkdir -p "$appdir/usr/share/doc/cliprust/third-party"
cp /usr/share/doc/xkb-data/copyright "$appdir/usr/share/doc/cliprust/third-party/xkb-data.copyright"
cp -a /usr/share/X11/xkb /usr/share/X11/locale "$appdir/usr/share/X11/"
mkdir -p "$appdir/etc"
cp -aL /etc/fonts "$appdir/etc/"
cp /usr/share/doc/fontconfig-config/copyright "$appdir/usr/share/doc/cliprust/third-party/fontconfig-config.copyright"
cp /usr/share/doc/libx11-data/copyright "$appdir/usr/share/doc/cliprust/third-party/libx11-data.copyright"
# Include libraries linked directly and loaded dynamically by the X11/Wayland backends.
python3 - "$binary" "$appdir/usr/lib" <<'PY'
import pathlib,re,subprocess,sys,shutil
out=pathlib.Path(sys.argv[2]);seen=set()
licenses=out.parent/'share/doc/cliprust/third-party';licenses.mkdir(parents=True,exist_ok=True)
cache=subprocess.check_output(['ldconfig','-p'],text=True)
extra=['libX11.so.6','libxcb.so.1','libwayland-client.so.0','libxkbcommon-x11.so.0','libXcursor.so.1','libXi.so.6','libXrandr.so.2']
queue=[sys.argv[1]]
for name in extra:
 match=re.search(r'^\s*'+re.escape(name)+r' .* => (.+)$',cache,re.M)
 if not match:raise SystemExit('Missing AppImage library: '+name)
 queue.append(match[1])
while queue:
 path=queue.pop()
 if path in seen:continue
 seen.add(path)
 name=pathlib.Path(path).name
 if path!=sys.argv[1]:
  if re.match(r'^(libc\.so|libm\.so|libpthread\.so|libdl\.so|librt\.so|ld-linux)',name):continue
  shutil.copy2(path,out/name,follow_symlinks=True)
  # Preserve distribution copyright/license notices for every bundled library.
  for candidate in [path,str(pathlib.Path(path).resolve())]:
   result=subprocess.run(['dpkg-query','-S',candidate],capture_output=True,text=True)
   if result.returncode==0:
    package=result.stdout.split(': ')[0].split(':')[0]
    notice=pathlib.Path('/usr/share/doc')/package/'copyright'
    if notice.exists():shutil.copy2(notice,licenses/(package+'.copyright'))
    break
 deps=subprocess.check_output(['ldd',path],text=True)
 if 'not found' in deps:raise SystemExit(deps)
 queue.extend(re.findall(r'=> (/\S+)',deps))
PY
cat > "$appdir/AppRun" <<'RUN'
#!/bin/sh
root=$(dirname "$(readlink -f "$0")")
export LD_LIBRARY_PATH="$root/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export XKB_CONFIG_ROOT="${XKB_CONFIG_ROOT:-$root/usr/share/X11/xkb}"
export XLOCALEDIR="${XLOCALEDIR:-$root/usr/share/X11/locale}"
export FONTCONFIG_PATH="${FONTCONFIG_PATH:-$root/etc/fonts}"
export FONTCONFIG_FILE="${FONTCONFIG_FILE:-$root/etc/fonts/fonts.conf}"
exec "$root/usr/bin/cliprust" "$@"
RUN
chmod +x "$appdir/AppRun"
ARCH=x86_64 APPIMAGE_EXTRACT_AND_RUN=1 "$APPIMAGETOOL" --no-appstream --runtime-file "$APPIMAGE_RUNTIME" "$appdir" "$output/Cliprust-$version-linux-x86_64.AppImage"
