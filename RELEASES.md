# Cliprust distribution packages

These are development builds, not a claim of support for every historical Linux release or every desktop environment.

## Linux x86_64

Release binaries target glibc 2.35 or newer (Ubuntu 22.04 baseline). Ubuntu 22.04+, Debian 12+, current Fedora/openSUSE, Arch, Garuda and similar glibc distributions are the intended families. Actual distribution installation coverage is recorded in TEST_RESULTS.md. Alpine/musl, 32-bit, ARM and older glibc systems need a native source build and are not covered by these binaries.

| File | Use |
| --- | --- |
| `Cliprust-*-linux-x86_64.AppImage` | Portable GUI with bundled non-glibc libraries; chmod +x and run. Use `--appimage-extract-and-run` if FUSE is unavailable. |
| `cliprust-*-linux-x86_64` | Standalone executable; install the runtime libraries below. |
| `cliprust-*-linux-x86_64.tar.gz` | Executable, desktop integration, docs and per-user install/uninstall scripts. |
| `cliprust_*_amd64.deb` | Ubuntu/Debian/Mint: `sudo apt install ./cliprust_*.deb` |
| `cliprust-*.x86_64.rpm` | Fedora: `sudo dnf install ./cliprust-*.rpm`; openSUSE: `sudo zypper install ./cliprust-*.rpm` |
| `cliprust-*-x86_64.pkg.tar.zst` | Arch/Garuda/Manjaro: `sudo pacman -U ./cliprust-*.pkg.tar.zst` |
| `cliprust-*-source.tar.gz` | Source snapshot for native builds on other architectures/distributions. |

Raw executable/tar runtime dependencies: glibc, libgcc, Fontconfig, libxkbcommon, X11/libxcb, Xcursor/Xi/Xrandr, libxkbcommon-x11, Wayland client libraries and XKB keyboard data. Fonts are provided by the system. The AppImage bundles the non-glibc libraries, matching Fontconfig configuration, XKB and compose-key data. It still uses installed system/user fonts and respects explicit font/locale environment overrides. Desktop integration capabilities still depend on the compositor: see SWAY_INTEGRATION.md and KDE_INTEGRATION.md.

The tar archive's `install.sh` defaults to `~/.local`; use the same `PREFIX` for installation and uninstallation. User clipboard history is preserved when uninstalling. Packages do not change compositor keybindings or enable autostart automatically.

## Windows x86_64

`Cliprust-*-windows-x86_64-Setup.exe` is a per-user NSIS installer. It installs without administrator rights, creates a Start-menu shortcut and an uninstall entry, and preserves clipboard history on uninstall. Quit Cliprust before upgrading or uninstalling. The portable ZIP contains the same application and any required non-system runtime DLLs; extract the entire ZIP together. Windows 10/11 x64 is the intended runtime. Installers are unsigned development artifacts. Build/inspection results are separate from actual Windows runtime acceptance.

## Verification and rebuilding

Check `SHA256SUMS` beside the artifacts before installation. `BUILD-INFO.txt` records the Linux ABI baseline. Packaging does not turn a failed or untested platform build into a verified release.

Build on Ubuntu 22.04 with Rust 1.92+ and Fontconfig/XKB/X11/Wayland development libraries, then run `scripts/package-linux.sh`. `CLIPRUST_BINARY` selects a prebuilt Linux executable; `CLIPRUST_OUTPUT` selects the destination. `scripts/package-appimage.sh` packages the same binary with baseline runtime libraries. Use `scripts/package-windows.sh` in a Linux cross-build environment with MinGW-w64 and NSIS, or the Windows CI job with MSVC and NSIS. The GitHub workflow can be manually dispatched; it does not automatically publish a public release.
