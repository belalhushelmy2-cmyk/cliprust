#!/usr/bin/env bash
# Shared private runtime for every binary Linux package; build on Ubuntu 22.04.
set -euo pipefail
cd "$(dirname "$0")/.."
binary=$(realpath "$1")
appdir=$(realpath -m "$2")
output=$(realpath -m "$3")
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
extra=['libX11-xcb.so.1','libX11.so.6','libxcb.so.1','libwayland-client.so.0','libxkbcommon-x11.so.0','libXcursor.so.1','libXi.so.6','libXrandr.so.2']
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
# Fallback fonts work even in a minimal desktop; retain their license.
mkdir -p "$appdir/usr/share/fonts/truetype/dejavu"
cp /usr/share/fonts/truetype/dejavu/*.ttf "$appdir/usr/share/fonts/truetype/dejavu/"
cp /usr/share/doc/fonts-dejavu-core/copyright "$appdir/usr/share/doc/cliprust/third-party/fonts-dejavu-core.copyright"
install -m755 /usr/bin/xdg-open "$appdir/usr/bin/xdg-open"
cp /usr/share/doc/xdg-utils/copyright "$appdir/usr/share/doc/cliprust/third-party/xdg-utils.copyright"
cp -R packaging/kde "$appdir/usr/share/doc/cliprust/"
cat > "$appdir/AppRun" <<'RUN'
#!/bin/sh
set -eu
root=$(dirname "$(readlink -f "$0")")
export CLIPRUST_LAUNCHER="${APPIMAGE:-${CLIPRUST_LAUNCHER:-$root/AppRun}}"
# Keep the user's original environment for opening external applications.
export CLIPRUST_SYSTEM_LD_LIBRARY_PATH="${CLIPRUST_SYSTEM_LD_LIBRARY_PATH-${LD_LIBRARY_PATH-}}"
export CLIPRUST_SYSTEM_FONTCONFIG_PATH="${CLIPRUST_SYSTEM_FONTCONFIG_PATH-${FONTCONFIG_PATH-}}"
export CLIPRUST_SYSTEM_FONTCONFIG_FILE="${CLIPRUST_SYSTEM_FONTCONFIG_FILE-${FONTCONFIG_FILE-}}"
export CLIPRUST_SYSTEM_XKB_CONFIG_ROOT="${CLIPRUST_SYSTEM_XKB_CONFIG_ROOT-${XKB_CONFIG_ROOT-}}"
export CLIPRUST_SYSTEM_XLOCALEDIR="${CLIPRUST_SYSTEM_XLOCALEDIR-${XLOCALEDIR-}}"
export PATH="$root/usr/bin:$PATH"
export LD_LIBRARY_PATH="$root/usr/lib${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
export XKB_CONFIG_ROOT="${XKB_CONFIG_ROOT:-$root/usr/share/X11/xkb}"
export XLOCALEDIR="${XLOCALEDIR:-$root/usr/share/X11/locale}"
export FONTCONFIG_PATH="${FONTCONFIG_PATH:-$root/etc/fonts}"
# Baseline Fontconfig cannot resolve relative <dir> entries. Generate a small
# per-user config with absolute bundle paths; never change system font settings.
if [ -z "${FONTCONFIG_FILE:-}" ]; then
    cache="${XDG_CACHE_HOME:-$HOME/.cache}/cliprust/fontconfig"
    (umask 077; mkdir -p "$cache")
    name=$(printf '%s' "$root" | cksum | cut -d ' ' -f 1)
    config="$cache/$name.conf"
    escaped=$(printf '%s' "$root" | sed 's/\&/\\&amp;/g; s/</\\&lt;/g; s/>/\\&gt;/g')
    if [ ! -f "$config" ]; then
        temporary=$(umask 077; mktemp "$cache/.fonts.XXXXXX")
        printf '<?xml version="1.0"?>\n<!DOCTYPE fontconfig SYSTEM "fonts.dtd">\n<fontconfig><include ignore_missing="no">%s/etc/fonts/fonts.conf</include><dir>%s/usr/share/fonts</dir></fontconfig>\n' "$escaped" "$escaped" > "$temporary"
        mv "$temporary" "$config"
    fi
    export FONTCONFIG_FILE="$config"
fi
exec "$root/usr/bin/cliprust" "$@"
RUN
chmod +x "$appdir/AppRun"
