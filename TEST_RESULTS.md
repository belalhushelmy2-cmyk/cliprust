# Test results

Environment: Garuda/Arch Linux, Rust 1.99.1, 2026-10-22. The active Sway Wayland session was available for visual snapshots; clipboard protocol checks ran in a private headless Sway compositor. Windows runtime behavior, X11 paste, and target applications such as editors/file managers are not available here.

- `cargo test --offline --all-targets`: latest run 30 passed; 1 opt-in performance benchmark ignored (2 Linux autostart tests, 2 source-name tests, 1 recovery-backoff test, and 25 core integration tests). Core persistence tests cover source option discovery and source-filtered results.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo fmt --all -- --check` and `git diff --check`: passed.
- `cargo build --release --locked`: passed on this Linux host with source attribution, source-app filtering, and Dark/Light/System appearance support. Dark and light snapshots include the application selector; the system snapshot showed the host's `NoPreference` dark fallback. Live portal change delivery was not exercised.
- `tests/native_wayland.py`: passed in a private headless Sway compositor. Verified event capture, format bundle storage and clipboard reads for text/HTML/RTF, PNG, and a two-file URI list; verified two consecutive PNG offers with the same MIME type remain separate entries. The script byte-verified restoration of offered formats and accepts an initially empty clipboard.
- Windows native API modules: isolated `cargo check --target x86_64-pc-windows-gnu` passed for clipboard actions, clipboard-owner source attribution, autostart, and the system-theme registry watcher. These checks Rust API use only, not a full Windows build or runtime.
- Visual snapshots: the current Slint window rendered in Sway with dark, light, and system appearance. Built-in text controls follow the selected palette. Window placement/size is compositor-controlled; the README includes an optional floating rule. The Settings modal and grouped history were subsequently rendered and inspected (see later entries).
- Release benchmark: 10,000 in-memory inserts took 494.7 ms. Mean of 10 indexed searches over that history was 24.8 ms. This is one synthetic query pattern and does not establish disk-backed worst-case latency or the idle memory/CPU targets.
- Arch packaging: `makepkg --force --noconfirm --skipchecksums` passed from source commit `0734eac`; build and test phases passed (21 tests, one ignored), and `pacman -Qip` / `pacman -Qlp` confirmed metadata and expected binary, desktop entry, icon, README, and license files. Rebuild with the source-app selector is pending; package installation was not tested.
- Debian packaging: `scripts/deb.sh` rebuilt `dist/cliprust_0.1.0-1_amd64.deb` from source commit `0734eac`. Archive control metadata and payload paths were inspected with `ar`/`tar`; expected binary, desktop entry, icon, README, and license are present with the expected runtime dependencies. Rebuild with the source-app selector is pending. This host lacks `dpkg`/`dpkg-shlibdeps`, so installation on Debian/Ubuntu remains unverified.
- Wayland protocol fallback: standardized `ext-data-control-v1` support compiles and passes the core test suite and Clippy. The private Sway compositor exposes wlr-data-control, so the ext-only fallback still needs a suitable compositor for live acceptance.
- Retention settings UI: Slint interface and Rust callback compile; existing persistence/retention tests pass. The main window was rendered in headless Sway, but the Settings layout was subsequently inspected; all interactive controls still need end-to-end acceptance.
- Windows platform modules, including URL/file open via `ShellExecuteW`, clipboard-owner source attribution, and the app-theme registry watcher: passed isolated `x86_64-pc-windows-gnu` cargo checks. Windows runtime remains untested; this host lacks the MinGW C compiler needed for a complete app build.
- Source attribution: Linux unit tests cover bounded/sanitized names and X11 `WM_CLASS` parsing; the Windows process-query API compiles in the isolated cross-target check. A live Windows/X11 capture session was not available; Wayland source remains unavailable by protocol.
- Wayland global shortcut: portal registration/event loop compiles. The active session’s portal does not expose `org.freedesktop.portal.GlobalShortcuts`, so runtime registration cannot be verified here; manual `cliprust --show` binding remains documented as a fallback.
- Unified snippet search: core matching tests cover name, body, tags and aliases case-insensitively; all-target tests pass.
- Plain-text paste actions: the Slint action panel exposes plain Paste and plain Paste and Keep Open; both reuse the guarded paste capability check and build a text-only clipboard bundle. UI/build compilation and Clippy pass; target-app behavior remains subject to platform acceptance testing.
- Pinned reordering: persistence test covers initial order, moving up/down boundaries, and unpinned rejection; keyboard bindings are Ctrl+Shift+Up/Down.
- Copy image/path actions: image copy retains only image MIME formats; file URI-list parsing converts percent-encoded local file URLs to native paths without reading file contents. Core path-conversion test passes.
- Date filters: Store filters use an inclusive lower bound and exclusive upper bound; a core test covers records before, on, within, and at the end of a date range. The UI exposes local-calendar Today, Yesterday, and Last 7 Days selections.
- Snippet favorites: legacy snippet JSON without pin state loads as unpinned; archive round-trip preserves pinned snippets. The UI editor saves the pinned state and Ctrl+P toggles it; favorites filtering includes pinned snippets. Core suite currently contains 25 passing tests plus one ignored benchmark.
- Linux autostart: tests create/remove the XDG desktop entry in an isolated temporary directory, including safe path quoting and control-character rejection. Windows registry autostart code passed an isolated `x86_64-pc-windows-gnu` Rust check; whole-app cross-check still stops in bundled SQLite because MinGW is unavailable.
- Windows installer workflow: corrected its working directory and ensured NSIS failures propagate to CI. The Windows release step cannot be executed on this Linux host.

Not verified: Windows/X11 clipboard capture in real apps, paste acceptance, repeated Paste and Keep Open, KDE Plasma Wayland, clean-machine package installation, Windows tray behavior, actual sign-in launch, or published release installers. The UI explicitly reports Wayland automatic paste as unavailable.

- Capture exclusions: policy tests verify exact case-insensitive matches, unknown sources, pause, malformed configuration failing closed, and invalid names. Source-filter regression covers 1,100 unrelated newer candidates.
- Current source changes passed all-target tests, Clippy, and the isolated Windows clipboard-owner module check. Release archives have not been rebuilt for these changes; the prior Arch rebuild after source filtering was interrupted. Settings scrolling compiles but has not yet been visually inspected.

- Recent-search persistence test covers the 20-query cap, trimming, deduplication, pause, reopening the database and clearing. Native dark search-bar snapshot inspected after adding recall; latest debug build and Clippy pass.

- Configurable shortcuts: tests verify defaults, independent paste bindings, modifier parsing, collision rejection, reserved Ctrl+A, and preservation of ordinary typing. Isolated Windows hotkey module compiles with configurable modifiers/key; native delivery is not claimed. CLI accepted custom bindings and rejected Ctrl+C collision. Native Settings snapshot with custom values inspected; scrollable layout renders. Final all-target tests, debug build and Clippy pass.

- Backup restore: regression test checks merged history, pins, preserved current settings/same-name snippets, byte-identical source backup, and invalid-backup rejection without lost entries. Windows chooser module passed the isolated cross-target check. The host FileChooser portal exposes OpenFile/SaveFile (version 4), but interactive chooser selection/cancellation remains unverified.

- Grouping: tests cover local Today/Yesterday/Previous days, pins, type, application/unknown groups and stable order within sections. Native date-group screenshot inspected; header positioning corrected after first render.
- Duplicates: test covers independent IDs/formats/pins/deletion, ordinary capture deduplication, and repeated archive import retaining exactly two entries. All-target suite, Clippy, debug build and formatting pass.

- Tray: all-target tests (28 passed, one benchmark ignored), debug build and Clippy pass with the new dependency. Isolated Windows tray module compiles. `tests/native_tray.py` passed on the running Sway/Waybar session: SNI registration, Show dispatch, Resume/Pause database state and menu-label updates, and graceful Quit. Test uses an isolated temporary database and does not change clipboard contents; Windows tray runtime remains unverified.

- Monitor recovery: unit test verifies increasing/capped delays and reset after stable connection. `tests/native_reconnect.py` passed against a private headless Sway compositor: monitor started before display creation, captured after startup, then captured again after compositor termination/restart on the same socket. This proves connection recovery, not actual OS suspend/resume or a changed display environment. Suite: 29 passed, one optional benchmark ignored.

- Unified snippet search: regression test covers a winning snippet beyond position 300, fuzzy matching, abbreviation relevance against clipboard candidates, stable negative action IDs, favorites, and a body match beyond 8,192 characters. Latest all-target suite: 30 passed, one benchmark ignored; Clippy passes.

## Release performance checkpoint (`a571886`, 2026-10-22)

`cargo build --release --offline --locked` passed with all features through unified snippet ranking; binary size is 25,857,368 bytes. The expanded opt-in benchmark passed with 10,000 entries and 20 warmed queries per case:

| Storage | Query | Median | P95 | Maximum |
|---|---|---:|---:|---:|
| Memory | transaction | 25.853 ms | 26.300 ms | 26.344 ms |
| Memory | project 9876 | 16.241 ms | 19.042 ms | 19.054 ms |
| Memory | engneerng | 15.122 ms | 15.516 ms | 16.421 ms |
| Disk WAL | transaction | 25.731 ms | 26.194 ms | 26.210 ms |
| Disk WAL | project 9876 | 16.114 ms | 16.396 ms | 16.475 ms |
| Disk WAL | engneerng | 14.965 ms | 15.107 ms | 15.268 ms |

10,000 inserts: 545.8 ms in memory, 933.5 ms on disk WAL. These synthetic queries pass the 30 ms target, not a universal latency guarantee.

`python3 tests/measure_linux.py` ran the release application with 10,000 text entries, active monitoring, a hidden window, and a private headless Sway display. Over 20.005 seconds: RSS stayed at 38.406 MiB (40.27 decimal MB), PSS was 31.732 MiB, CPU averaged 0.050% of one core. The strict under-40-decimal-MB RSS target is narrowly missed. Five IPC-to-window-manager-visible observations were 98.63, 111.97, 98.98, 102.64, and 98.49 ms (median 98.98 ms). Observation overhead is included; this is not frame-presentation timing and not every sample met 100 ms. Other desktops, image-heavy histories, large snippet libraries, and Windows remain unmeasured.

## AppImage checkpoint

Built `dist/Cliprust.AppImage` (11,991,544 bytes), binary source through `a571886`, with appimagetool 1.9.1 (SHA256 `ed4ce84f0d9caff66f50bcca6ff6f35aae54ce8135408b3fa33abfc3cb384eb0`) and type2 runtime 20251108 (`2fca8b443c92510f1483a883f60061ad09b46b978b2631c807cd873a47ec260d`), verified against GitHub release asset digests. Artifact SHA256: `ec371c3d2206f1372eb215978ddddd8557197bb81e9e28044c8b91752d40680b`.

Normal invocation `--version` passed. Extract-and-run mode passed version, isolated Unicode entry insert/search, and native Slint snapshot smoke tests. The desktop tiled the snapshot below the requested minimum size, clipping controls; this verifies packaged rendering, not small-window layout acceptance. AppStream tree validation passed with only missing-homepage downgraded to informational (there is no public project homepage); shell syntax and diff checks passed. CI now builds/smoke-tests and uploads AppImage/checksum artifacts with pinned tools, but that workflow was not executed here. Host libraries are not bundled, and compatibility with other distributions remains unverified. Arch/DEB archives have not been rebuilt for the new metadata/source.

## Compact floating launcher (2026-10-22)
- Rebuilt AppImage with 860×580 default window, smaller margins and preview pane; inspected isolated native Wayland snapshot at that size with all primary controls visible.
- On the user's Sway session, verified the rebuilt app is native Wayland, floating, focused, and 860×580 at (530,277) after running the launcher helper. Installed persistent floating rules and Ctrl+Alt+V binding in the user's Sway config; the binding was accepted by the running compositor. Physical keypress delivery was not simulated.
- Shell syntax and git whitespace checks passed. AppImage replacement now works while the previous image is mounted/running.

- Follow-up: changed the local Sway binding to Mod+V and removed its splitv conflict; configured Paste and Keep Open as Ctrl+Shift+Enter and restarted. Verified the helper brings an existing floating window from a different workspace back to the invoking workspace (workspace 3), focused and floating.

## KDE integration source checkpoint (2026-10-22)
- Environment verified: Sway/Wayland, no KWin or Plasma executable, no GlobalShortcuts or RemoteDesktop portal implementation exposed here. No KDE settings changed and no KWin script installed/enabled.
- 31 Rust tests passed, one opt-in benchmark ignored; Clippy all-targets with warnings denied passed. KWin JavaScript contract tests passed for placement on another desktop/monitor, stale/closed destination rejection, focus refusal, repeated restore and monitor removal. Bridge unit test rejects wrong/expired acknowledgements.
- Windows platform module (actual source via isolated harness) cross-compiled for x86_64-pc-windows-gnu; full Windows application, installer and interaction untested.
- Rebuilt native Wayland UI snapshot at 860×580 inspected; private Sway tests confirm one-window IPC, hidden-window reopen and safe invalid-token transport (not activation permission acceptance). Private release monitor recovered from missing display and compositor restart and captured text afterwards.
- Real KDE shortcut conflict/registration UI, valid activation token focus, KWin script installation/API execution, RemoteDesktop permission grant/denial/revocation, text/rich/image/file paste, multi-monitor behavior, and actual X11/Windows app paste remain pending. Input submission is never reported as destination clipboard consumption.

- Isolated session-bus transport test passed using the actual exported KWin bridge with a mock peer (prepare/show/report wire signatures and replies). This validates D-Bus plumbing, not KWin execution. Preview no longer performs blocking bridge calls; action-time target checks remain enforced.

## Sway automatic paste — 2026-10-22

- Native IPC + real Wayland virtual-keyboard acceptance runs in a private two-output headless Sway session, without changing the user's clipboard.
- Stock GTK TextView consumes and inserts Arabic/Unicode text, renders a PNG, and receives original file-URI bytes. Repeated Ctrl+Enter preserves the search and target; Meta+V hides/summons a single process across workspaces. A real foot terminal configured for Ctrl+V receives the UTF-8 content.
- Closed targets produce errors while Cliprust remains visible. With the optional wtype backend selected, missing wtype and an executable returning failure also produce errors. The native backend was verified without wtype on its PATH.
- Rust tests: 32 passed, one pre-existing ignored benchmark. Clippy with warnings denied passed. Windows platform modules compile with the installed rustup Windows GNU target; full Windows application/runtime and X11 runtime were not tested.
- Firefox is unavailable here. Physical monitor arrangements, arbitrary destination format support, and XWayland destination acceptance remain unverified. File testing verifies URI transfer, not copying files on disk. Successful input execution is reported as input sent, not as guaranteed consumption by every destination.
- The optional exact wtype backend did not activate stock GTK/Mousepad accelerators on this host. The default native backend uses a full evdev map and passes stock GTK insertion; Mousepad insertion was visually verified in /tmp/cliprust-sway-mousepad.png. Mousepad save via wtype was not verified.
- Final release acceptance passed with `CLIPRUST_TEST_BINARY=target/release/cliprust` (absolute path). PNG dimensions and RGBA pixels matched the restored source. The AppImage checksum/CLI passed; the restarted desktop window was verified floating at 860×580. Existing Mod4+V configuration was backed up, validated, and reloaded successfully.

## Stable history session and initial selection (2026-10-22)

- `tests/native_history_session.py` runs in a private headless Sway compositor, with its own clipboard and SQLite database. A fixed floating-window rule isolates list behavior from compositor decoration/resize differences. It checks actual GTK text insertion, latest-copy selection beneath twelve pins, immediate Enter, Up/Down navigation, typing into search, Escape clear/close, repeated Ctrl+Enter, and pixel-identical list viewport/selection/scroll before and after pasting. Genuine identical and new external copies update SQLite without altering the open list; closing/reopening refreshes and selects the latest external copy.
- Regression tests cover stable metadata/search ranking across use and capture updates, snippet identity, empty/filtered histories, retained latest-copy identity after using an older item, and a latest item beneath more than 300 pinned candidates.
- Ownership tests check that only this process's published offer markers are suppressed; ordinary external formats, foreign markers and unpublished marker sequences are not suppressed. This replaces the previous three-second hash-based suppression.
- Windows and X11 share the snapshot/focus implementation and native clipboard-marker path. Linux/Sway runtime evidence does not establish Windows or independent X11 runtime acceptance.

## Distribution builds including the history/focus fix (2026-10-22)

- `cargo test --all-targets --locked --offline`: 36 passed, one opt-in benchmark ignored. `cargo fmt --check` and Linux all-target Clippy with `-D warnings` passed.
- Full Linux x86_64 release compiled inside an isolated Ubuntu 22.04 root filesystem. `readelf` confirms the maximum required GLIBC symbol version is **2.35**. Full Windows x86_64 GNU release cross-compilation also passed, including Slint and bundled SQLite; this supersedes earlier isolated-module-only build limitations.
- Produced raw Linux executable, portable tar with user install/uninstall scripts, AppImage, DEB, RPM, Arch package, Windows portable ZIP and per-user NSIS installer. Dependency license notices are included. Source archive and SHA-256 manifest accompany the outputs in `dist/releases/0.1.0/`.
- Ubuntu 22.04: DEB installed with `dpkg`, launched successfully, and uninstalled in the isolated environment. The tar installer/uninstaller and Unicode SQLite CLI operations passed. RPM and Arch metadata/file payloads were inspected, and packaged executable hashes match the release binaries; RPM/Arch installation on those distributions is not claimed.
- The **packaged AppImage** passed `tests/native_history_session.py` in private Sway/GTK: latest selection beneath pins, immediate Enter, arrows, printable search, stable keep-open viewport, true external-copy capture, reopen refresh and repeated snippet expansion without new history rows. The native paste suite also passed with monitoring active and no self-capture for text, PNG or file MIME restoration, including two workspaces/outputs, foot and stock Mousepad.
- NSIS compiled without warnings. Its extracted `cliprust.exe` SHA-256 matches the full Windows cross-build and portable ZIP; PE/import inspection found only Windows system DLL imports. Actual Windows installation/uninstallation, Windows clipboard/hotkey/paste, and an independent X11 desktop were not available for runtime acceptance. The installer is unsigned.
- These are x86_64 development packages for glibc >= 2.35, not binaries for every Linux version/architecture. Alpine/musl, ARM, 32-bit and older glibc require source builds. GitHub packaging workflows were added but were not run remotely.
- AppImage compatibility follow-up: matched Fontconfig configuration and X11 compose data are bundled alongside baseline libraries. The packaged Sway acceptance test passes again and asserts no Fontconfig/xkbcommon configuration errors. Explicit Xcursor/Xi/Xrandr/libxkbcommon-x11 dependencies are included for native X11 package installs.

## Bundled runtime and shortcut follow-up (2026-10-22)

- Rust unit/integration suite: 38 passed, one explicit benchmark ignored. Formatting and Clippy with warnings denied passed.
- `tests/native_shortcut_setup.py`: isolated Sway test passed for native config editing, live old/new key behavior, variable/include conflict detection, backups and rollback after invalid configuration. It does not modify the user's desktop config.
- `tests/isolated_linux_runtime.py`: staged runtime launches the real GUI and creates a snapshot with no host application libraries/fonts/FUSE; Unicode database writes passed and `fc-match` selected the bundled DejaVuSans.ttf. The test root contains only glibc and standard shell tools plus an fc-match probe; a private Sway compositor supplies the display socket.
- Packaged history regression passed on private Sway: initial selection, keyboard search, Enter, repeated Keep Open, stable scroll/selection/order and genuine external clipboard capture.
- Windows x86_64 GNU release compiled; NSIS installer and portable archive built. Import audit confirms only Windows system DLLs are needed. No Windows desktop runtime or portal-v2 desktop is available here, so those interactive paths are not claimed as verified.
- Package checks cover DEB/RPM metadata, tar install/uninstall, quoted install paths, self-extracting launch and stdin, AppImage launch, and checksums. Target-distribution installation coverage remains separate from artifact inspection.

## 2026-10-23 — keyboard history auto-scroll

- Replaced the history scroll-position binding with an explicit reveal-selection operation on arrow navigation and list refresh. It scrolls only enough to show the selected row, includes group-header height, and clamps to the viewport bounds.
- Private Sway regression passed: newest selection beneath 12 pinned entries, 12 Up / 14 Down / 2 Up steps with screenshot assertions that the selected border remains visible, immediate Enter, and repeated Keep Open preserving identical viewport pixels. External-copy and search/session regressions also passed.
- Rust tests (38 passed, one benchmark ignored), formatting and Clippy passed. Windows and independent X11 runtime checks were not performed for this change.

## 2026-10-23 — v0.1.1 published packages

- Published https://github.com/belalhushelmy2-cmyk/cliprust/releases/tag/v0.1.1 from build commit 4c8192826a1385e908bd150d797c8f7aca3c0ed2. GitHub Actions run 39880400944 completed successfully: Ubuntu 22.04 and Windows checks, Linux packages, Windows MSVC static-CRT installer/portable ZIP.
- Linux CI passed 38 tests; Windows CI passed 31 tests (one benchmark ignored on each), formatting and Clippy. Linux package smoke tests passed. Downloaded CI AppImage passed private Sway history/auto-scroll/keep-open acceptance and isolated runtime/font verification. Windows release --version passed in CI; installer and ZIP executable hashes match and import audit passes.
- Verified both workflow artifact digests and all 16 final GitHub asset sizes/SHA256 digests. SHA256SUMS and VALIDATION.txt accompany downloads. Updated the local AppImage to the same verified v0.1.1 Linux package.
- Windows desktop GUI/clipboard and independent X11 desktop acceptance remain unverified; installers are unsigned. See the release VALIDATION.txt for exact coverage.

## 2026-10-24 — smaller UI and flexible floating placement

- Reduced the default popup from 860×580 to 780×540, with a 640×420 minimum. Refined dark surfaces, spacing and buttons; moved date/application filters behind Filters, shortened Recent, and kept Quit in Actions. Preview, settings and action panels fit the smaller layout. System font preferences remain respected.
- Header uses Slint's native WindowMoveArea; a bottom-right grip starts native resize and cancels the UI grab so later clicks/drags work correctly. Removed forced Winit AlwaysOnTop. The opt-in KDE bridge preserves/clamps existing geometry and clears keepAbove.
- Sway/SwayFX receives Cliprust-only floating/border session rules before mapping, with an explicit stable XDG app ID. This avoids retile flashes in the destination app without editing/reloading user configuration. Remembered size/position survives hide/reopen while the process runs, is bounded to the destination output, and Keep Open restores focus without recentering/resizing. Superseded deferred placement requests are cancelled.
- Local Rust tests: 39 passed, one benchmark ignored; formatting, Clippy and diff checks passed. Real Wayland pointer acceptance on SwayFX and stock Sway verifies header drag, corner resize, drag after resize, geometry retention, two outputs/workspaces, unaffected destination layout, Arabic/text/image/file paste, foot, Mousepad and closed-target rejection. Packaged history/auto-scroll/focus regression passed; dark/light/settings snapshots were visually checked.
- KDE geometry logic passed a mocked-workspace check; live KDE, Windows and X11 desktop validation were not performed. This is a local development update; published v0.1.1 release assets were not replaced.

## v0.1.2 release package validation (2026-10-24)

GitHub Actions run 40048225572, source fedee623557d50cd6c2835c6ce07816ec8712656: Linux 39 and Windows 31 Rust tests passed, one benchmark ignored per platform; formatting and Clippy passed. Ubuntu 22.04 package smoke checks, native Windows release CLI and normal/delay DLL audit passed. Windows packaging retry succeeded after a Chocolatey gateway timeout.

Downloaded AppImage passed the SwayFX history, initial selection, auto-scroll, frozen ordering and custom geometry suite. Extracted release AppRun passed stock Sway real pointer drag/resize, geometry retention, workspace/output activation, destination layout stability, Arabic/Unicode, PNG/URI, repeated Keep Open, foot/Mousepad and closed-target tests, including native virtual keyboard without wtype. AppRun's standard shell helpers were supplied in the pointer harness restricted PATH; direct AppRun preserves the harness process-ID assertion. Artifact and per-file SHA256 manifests/source revisions verified. Installer/portable executable hashes match.

Live Windows GUI/clipboard, independent X11 and live KDE desktop checks remain unverified. See the release VALIDATION.txt for package hashes and exact scope.
