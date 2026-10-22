# Cliprust distribution packages

These are development builds, not a claim of support for every historical Linux release or every desktop environment.

## Linux x86_64

Release binaries target glibc 2.35 or newer (Ubuntu 22.04 baseline). Ubuntu 22.04+, Debian 12+, current Fedora/openSUSE, Arch, Garuda and similar glibc distributions are the intended families. Actual distribution installation coverage is recorded in TEST_RESULTS.md. Alpine/musl, 32-bit, ARM and older glibc systems need a native source build and are not covered by these binaries.

| File | Use |
| --- | --- |
| `Cliprust-*-linux-x86_64.AppImage` | Portable GUI with bundled non-glibc libraries; chmod +x and run. Use `--appimage-extract-and-run` if FUSE is unavailable. |
| `cliprust-*-linux-x86_64` | Self-extracting executable with the complete private runtime. No FUSE needed; chmod +x and run. |
| `cliprust-*-linux-x86_64.tar.gz` | Complete private runtime, fonts, desktop integration and per-user install/uninstall scripts. |
| `cliprust_*_amd64.deb` | Ubuntu/Debian/Mint: `sudo apt install ./cliprust_*.deb` |
| `cliprust-*.x86_64.rpm` | Fedora: `sudo dnf install ./cliprust-*.rpm`; openSUSE: `sudo zypper install ./cliprust-*.rpm` |
| `cliprust-*-x86_64.pkg.tar.zst` | Arch/Garuda/Manjaro: `sudo pacman -U ./cliprust-*.pkg.tar.zst` |
| `cliprust-*-source.tar.gz` | Source snapshot for native builds on other architectures/distributions. |

All binary Linux formats now bundle the application’s non-glibc library closure (including dynamically loaded X11/Wayland libraries), matching Fontconfig configuration, XKB/compose data, DejaVu fallback fonts and xdg-open, with license notices. Native packages keep these under `/usr/lib/cliprust`, without replacing system libraries. SQLite is compiled into the application. No Rust, Python, Node, Electron, wtype or wl-clipboard installation is needed for normal use. The single-file launcher needs the standard shell/coreutils/tar/gzip utilities; native/tar installs do not extract on each launch.

The operating system still supplies glibc >= 2.35, a running graphical desktop and its session D-Bus/permission services. Desktop portals must run as part of the desktop session; copying a portal backend into an app cannot grant compositor permissions. DEB recommends the portal service, generic file-chooser backend and desktop opener; RPM/Arch declare appropriate recommended/optional integration packages. KDE/GNOME/other desktops need their matching portal backend for their supported features. Install native packages with apt/dnf/zypper/pacman (not bare dpkg/rpm) to resolve system dependencies.

### Wayland capabilities

- **Sway:** native capture, focus restoration and paste; Settings can update the global binding with conflict protection, backup and validation. Neither wtype nor swaymsg is required by Cliprust. The installed compositor’s `sway` executable validates its own config.
- **Other Wayland desktops:** launcher registration uses the Global Shortcuts portal where implemented. The Desktop launcher shortcut button opens the portal’s configuration UI when version 2 is available. Older/missing portals display an actionable diagnostic rather than claiming success.
- **KDE:** existing opt-in KWin bridge and user-approved RemoteDesktop input remain necessary for automatic target restoration/paste. The KWin integration files are included in the Linux runtime’s documentation directory. They are not silently enabled.
- **Capture:** requires `wlr-data-control` or `ext-data-control`. A compositor exposing neither cannot provide background clipboard history through the current backend.
- **Automatic paste:** remains Sway-native or KDE bridge/portal-based. Other Wayland desktops have no implemented verified focus-restoration backend in this release; manual paste remains necessary there. Packaging does not make all Wayland compositors equivalent.
- **File dialogs/tray/open links:** require the desktop’s file chooser portal, tray host and default destination application respectively. These desktop services are not private application dependencies.

See SWAY_INTEGRATION.md and KDE_INTEGRATION.md for desktop-specific details.
The tar archive's `install.sh` defaults to `~/.local`; use the same `PREFIX` for installation and uninstallation. User clipboard history is preserved when uninstalling. Packages do not change compositor keybindings or enable autostart automatically. Saving a launcher shortcut in Settings explicitly applies the Sway binding; other Wayland desktops own shortcut selection through their portal.

## Windows x86_64

`Cliprust-*-windows-x86_64-Setup.exe` is a per-user NSIS installer. It installs without administrator rights, creates a Start-menu shortcut and an uninstall entry, and preserves clipboard history on uninstall. Quit Cliprust before upgrading or uninstalling. The portable ZIP contains the same application and any required non-system runtime DLLs; extract the entire ZIP together. Windows 10/11 x64 is the intended runtime. The GNU build currently imports only Windows system DLLs; no Visual C++ redistributable or .NET installation is required. Packaging audits normal and delay-load imports and fails if a non-system DLL is missing. The installer offers to start Cliprust on completion. Installers are unsigned development artifacts. Build/inspection results are separate from actual Windows runtime acceptance.

## Verification and rebuilding

Check `SHA256SUMS` beside the artifacts before installation. `BUILD-INFO.txt` records the Linux ABI baseline. Packaging does not turn a failed or untested platform build into a verified release.

Build on Ubuntu 22.04 with Rust 1.92+ and Fontconfig/XKB/X11/Wayland development libraries, libx11-xcb1, fonts-dejavu-core and xdg-utils, then run `scripts/package-linux.sh`. `CLIPRUST_BINARY` selects a prebuilt Linux executable; `CLIPRUST_OUTPUT` selects the destination. `scripts/package-appimage.sh` packages the same binary with baseline runtime libraries. Use `scripts/package-windows.sh` in a Linux cross-build environment with MinGW-w64 and NSIS, or the Windows CI job with MSVC and NSIS. The GitHub workflow can be manually dispatched; it does not automatically publish a public release.
