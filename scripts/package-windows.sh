#!/usr/bin/env bash
# Build once with: cargo build --release --target x86_64-pc-windows-gnu --locked
set -euo pipefail
cd "$(dirname "$0")/.."
version=$(sed -n 's/^version = "\([^"]*\)"/\1/p' Cargo.toml | head -1)
binary=$(realpath "${CLIPRUST_WINDOWS_BINARY:-target/x86_64-pc-windows-gnu/release/cliprust.exe}")
output=$(realpath -m "${CLIPRUST_OUTPUT:-dist/releases/$version}")
mkdir -p "$output"
python3 scripts/third-party-notices.py "$output/THIRD-PARTY-NOTICES.txt"
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT
payload="$work/cliprust-$version-windows-x86_64"
mkdir -p "$payload"
cp "$binary" "$payload/cliprust.exe"
cp LICENSE TEST_RESULTS.md "$output/THIRD-PARTY-NOTICES.txt" "$payload/"
cp RELEASES.md "$payload/README.md"
# Bundle only known MinGW runtimes actually imported by the executable/dependencies.
for _ in 1 2 3; do
  for file in "$payload"/*.exe "$payload"/*.dll; do
    [[ -f "$file" ]] || continue
    while read -r dll; do
      case "$dll" in
        libgcc_s_seh-1.dll|libstdc++-6.dll|libwinpthread-1.dll)
          path=$(x86_64-w64-mingw32-gcc -print-file-name="$dll")
          [[ -f "$path" ]] || { echo "Missing runtime $dll" >&2; exit 1; }
          cp "$path" "$payload/$dll" ;;
      esac
    done < <(x86_64-w64-mingw32-objdump -p "$file" | sed -n 's/.*DLL Name: //p')
  done
done
if compgen -G "$payload/*.dll" >/dev/null; then
  for notice in /usr/share/doc/gcc-mingw-w64-x86-64-posix-runtime/copyright /usr/share/doc/mingw-w64-x86-64-dev/copyright; do
    [[ -f "$notice" ]] || { echo "Runtime license notice missing: $notice" >&2; exit 1; }
    cat "$notice" >> "$payload/THIRD-PARTY-LICENSES.txt"
  done
fi
python3 scripts/check-windows-runtime.py "$payload" "$output/WINDOWS-IMPORTS.txt"
makensis -DVERSION="$version" -DPAYLOAD="$payload" -DOUTPUT="$output/Cliprust-$version-windows-x86_64-Setup.exe" packaging/windows/installer.nsi
python3 - "$payload" "$output/cliprust-$version-windows-x86_64-portable.zip" <<'PY'
import pathlib,sys,zipfile
root=pathlib.Path(sys.argv[1])
with zipfile.ZipFile(sys.argv[2],'w',zipfile.ZIP_DEFLATED) as archive:
 for path in sorted(root.iterdir()):archive.write(path,root.name+'/'+path.name)
PY
echo "Windows packages: $output"
