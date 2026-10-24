# Cliprust progress

Project started 2026-10-22 in an empty repository. This is an incomplete development build, not a completed Raycast-parity release.

1. Public-source research and original feature specification: recorded in docs/RESEARCH.md.
2. Rust + Slint foundation and dark keyboard-first UI: implemented and rendered.
3. SQLite bundles, FTS5 candidates, deterministic ranking, type/pin/date filtering, retention, pins and pinned reordering, pinnable snippets, import/export and backup: implemented and unit tested. UI exposes entry-count and age retention limits plus backup/merge-restore/import/export.
4. Clipboard monitoring: event-driven Wayland wlr-data-control capture verified in private headless Sway; standardized ext-data-control fallback implemented and compiled, but an ext-only compositor is not available here for live acceptance. Windows/X11 runtime acceptance remains incomplete.
5. Paste and Paste and Keep Open: separate X11/Windows code paths exist but lack real-app validation. An opt-in KWin focus bridge plus keyboard-only RemoteDesktop portal path is now implemented for KDE Wayland; real KDE validation is still pending. Other Wayland desktops remain unsupported for automatic paste.
6. Clipboard actions and platform format coverage: partial. Plain Paste, plain Paste and Keep Open, copy image, and copy file paths join copy/plain copy and preserved-format paste; URL/file open has a native Windows shell path, compile-checked separately. See FEATURE_PARITY.md.
7. Global shortcut: Windows/X11 native registration remains; Wayland now requests the desktop Global Shortcuts portal and retains a manual command binding fallback when unavailable.
8. Core validation and a 10,000-entry synthetic search measurement: recorded in TEST_RESULTS.md. Release Linux idle CPU/RSS and activation observations are now recorded; other platforms and heavier histories remain unmeasured.
9. Packaging: Arch PKGBUILD built from the committed checkout; cargo-deb produced an amd64 DEB whose metadata and file list were inspected. Arch/Debian installation, NSIS runtime, AppImage cross-distribution testing and CI release outputs still need target-system validation.
10. Launch at sign-in: Settings now writes/removes the per-user XDG autostart entry on Linux and the current-user Run value on Windows. Linux file generation tests pass; Windows registry code is cross-checked, with runtime validation pending.
11. Appearance: Settings now persists Dark, Light, or System. The Linux system mode follows the XDG appearance portal; Windows watches the current-user app-theme registry value. Unavailable or unspecified system preferences fall back to dark.
12. Source application: Windows records the clipboard owner process executable and X11 reads the clipboard owner/ancestor `WM_CLASS` when accessible; history previews show the source. Wayland leaves it blank because its clipboard protocols do not reveal the owner application.
13. Source filtering: the history view now lists recorded application names and filters entries by the selected source; tests cover source discovery and filtered results.

No proprietary source or ranking algorithm was accessed. Never mark a platform or installer working based only on compiling its Rust code.

14. Capture privacy: Settings and CLI configure exact application exclusions. Both desktop and headless monitor enforce exclusions and pause before persistence. Unknown sources cannot match application exclusions. Invalid policy fails closed.
15. Search correctness: filters now apply before bounded candidate selection; regression tests include 1,100 unrelated newer entries.

16. Recent searches: the last 20 completed queries are persisted, deduplicated, and recalled from the search bar. Pause prevents new query storage; Settings clears the list.

17. Configurable shortcuts: Settings exposes launcher/paste/keep-open bindings; CLI configures seven in-window actions. Validation rejects collisions and typing-only bindings; Windows/X11 registration and Wayland portal preference consume the configured global trigger. Restart applies changes. Tests and Windows module compile-check pass; native delivery still needs target validation.

18. File workflows: native chooser adapters (Linux FileChooser portal and Windows common dialogs) run on separate threads. Settings offers backup, merge restore, JSON import/export; entry export and image saving use destination dialogs. Restore keeps local settings and same-name snippets. Core restore tests pass; interactive dialog acceptance is not yet tested.

19. Grouped history: persisted None/Date/Pins/Type/Application selector, local-calendar date groups, stable within-group ordering, and nonselectable headers with correct scroll offsets. Native date-group layout inspected.
20. Explicit duplicates: history duplicates preserve all formats and survive archive roundtrip while ordinary capture still deduplicates; snippets duplicate under a fresh name without reusing the abbreviation.

21. System tray: native tray library with Windows notification-area integration and Linux StatusNotifierItem/DBusMenu backend (no GTK requirement). Show History, Pause/Resume and Quit are wired; pause label/tooltip follow application state. Live Linux SNI test verifies registration and action delivery. Windows module compiles; Windows interaction remains unverified.

22. Monitor recovery: desktop and CLI reconnect after backend errors/ended streams with bounded exponential backoff; stable connections reset delay. Ended Wayland data-control sessions trigger reconnect, stale transfers are invalidated, and reconnection updates diagnostics. Private headless Sway test verifies late display startup and capture after compositor restart.

23. Unified snippet ranking: snippets and history now share query normalization, weighted fuzzy/acronym relevance and deterministic ordering before the 300-result display cap. Search includes full snippet bodies, tags and abbreviations; action IDs remain stable after filtering/sorting. Empty queries retain manual history pin order and include pinned snippets.

24. Performance checkpoint: current release built; warmed memory/disk search benchmark and reproducible private-display idle/activation measurement completed. CPU and tested search queries meet targets; RSS narrowly exceeds 40 decimal MB and some activation observations exceed 100 ms. Exact scope/results are in TEST_RESULTS.md.

25. AppImage: built a current x86_64 artifact using SHA-verified appimagetool 1.9.1/runtime 20251108. Normal --version, extract-and-run CLI/SQLite and native UI smoke tests passed on this host. Fresh AppDir staging, AppStream metadata, checksum output, and pinned-tool CI packaging/smoke steps added. CI execution and other distributions remain unverified.

26. KDE integration checkpoint: Meta/Super+V preference, read-only KGlobalAccel conflict check, portal-returned shortcut reporting and session filtering; activation tokens flow through portal/Unix IPC to the existing native Wayland surface. Show restores minimized state and focuses search without clearing its query. Added opt-in KWin script + D-Bus bridge for invoking desktop/monitor placement and verified destination restoration, followed by keyboard-only RemoteDesktop consent and input submission. Keep-open preserves the destination/query. Failures keep the window open and are logged; submitted input is not claimed as consumed paste. Script installation/enablement and KDE settings are never automated.

The attached request's default Ctrl+Enter is retained in source. Previously customized bindings remain user data. KDE, its portal backend, real input permission prompts, live rich/image paste, and multi-monitor acceptance are unavailable on this Sway host. KWin contract tests are not substitutes for those tests. Windows full-application single-instance handling and runtime acceptance remain outstanding.

27. Sway integration: native IPC captures the target container/PID and invoking workspace/output, floats and summons the existing process, and verifies destination focus before native virtual-keyboard input (optional wtype backend retained). `--toggle` supports a compositor-owned Mod4+V binding; search is focused on opening. Wayland restoration preserves original formats and verifies a unique clipboard-offer marker before input. Enter and Ctrl+Enter work from search; keep-open retains query/target. Recreated Slint surfaces are found by process ID because their app ID may be absent. Missing/failing optional wtype and closed targets produce visible diagnostics. A conservative shortcut helper inspects root/includes, refuses unrelated bindings, backs up the edited file, validates, and only then reloads. See SWAY_INTEGRATION.md and TEST_RESULTS.md for acceptance scope and limits.

Sway delivery verified: rebuilt `dist/Cliprust.AppImage`, checked its checksum and CLI, and restarted the desktop instance. Sway reported one focused floating Cliprust surface at 860×580. Updated the existing Cliprust Mod4+V binding to `--toggle`; configuration validation and reload succeeded, with backup `~/.config/cliprust/sway-backups/cliprust.conf.20261022-200302-033645`. Release acceptance passed for stock GTK Arabic/Unicode insertion, PNG pixel data, file URIs, repeated keep-open, workspace/output movement, foot, closed-target rejection, and native input without wtype. Mousepad insertion was visually checked. The native full-keymap backend is the default because the exact wtype command failed stock GTK/Mousepad accelerators; the optional wtype backend and its missing/failing-executable diagnostics remain tested.

## 2026-10-22 — stable open history and keyboard selection

- Open windows now use a metadata-only history/session snapshot. Clipboard capture and use timestamps continue updating SQLite without replacing the visible Slint row model. Search/filter operations use the same snapshot; explicit edits/pin/delete/import operations refresh it. Closing and reopening creates a fresh snapshot.
- Paste/Copy/Keep Open and monitor diagnostics update status without resetting rows, selection, query, or scroll. Retention cleanup from background capture is deferred while the history window is visible so displayed entries remain available.
- Clipboard publication uses process-specific offer markers on Wayland, Windows and X11. Removed the three-second content-hash exclusion; identical genuine external copies remain eligible for capture. Snippet actions and row-selection callbacks now resolve stable history IDs or snippet names.
- Opening selects the latest captured history ID independently of pinned order and paste-use recency. The list gets keyboard focus; printable input or Ctrl+F starts search, the first matching result is selected, and Escape clears a query before dismissing. The typing transition explicitly places the caret after the first character.
- Private Sway/GTK acceptance verifies immediate Enter, pins above the latest item, Up/Down, typing/search, repeated Keep Open, exact viewport preservation, self-event suppression, external-copy recording and reopen refresh. No shortcut or UI layout redesign was made. Windows runtime and independent X11 desktop acceptance remain pending.

## 2026-10-22 — portable Linux packages and Windows installer

- Built a Linux x86_64 release on Ubuntu 22.04 (glibc 2.35 baseline), plus a complete Windows x86_64 GNU release with Slint/SQLite. Outputs include raw binary, tar installer, AppImage, DEB, RPM, Arch package, Windows portable ZIP and per-user NSIS installer, with dependency notices, source archive and checksums.
- Verified the packaged AppImage's history/focus behavior in private Sway. Verified DEB install/launch/remove on isolated Ubuntu 22.04, tar install/uninstall and Unicode database use; inspected RPM/Arch metadata and Windows installer payload identity. Windows runtime and other distribution installation coverage remain explicitly unverified.
- Packaging scripts and manual/tag GitHub artifact workflows now reproduce the outputs without automatically publishing a public release. See RELEASES.md and the final section of TEST_RESULTS.md for supported binary targets and exact evidence.
- Final AppImage launch validation found and fixed old-library/new-host configuration warnings by bundling matching font/compose configuration. The packaged Sway test passes with configuration-error assertions enabled.

## 2026-10-22 — bundled runtime and editable shortcuts

- Unified Linux binary packaging around a private runtime: linked and dynamically loaded libraries (including libX11-xcb), matching Fontconfig/XKB/Compose data, DejaVu fallback fonts and xdg-open. DEB/RPM/Arch use package revision 2; tar installer deploys the same runtime and quotes menu launcher paths. The extensionless download is a FUSE-free self-extracting bundle. Windows packaging audits normal/delay-load imports, and NSIS offers to launch the app after installation.
- Verified the staged GUI and Unicode SQLite operations in a minimal filesystem containing glibc and standard shell tools, without system fonts or X11/Wayland/Fontconfig application libraries. Baseline Fontconfig ignored relative font directories, so the launcher now writes a private absolute-path font configuration into the user's cache. Bundled configuration/library variables are restored before opening external applications. Autostart records the persistent package launcher.
- Paste and Keep Open shortcut edits replace the live callback and hints immediately. Settings stays open and displays save failures. Sway launcher editing now uses native IPC/config parsing, conflict rejection, backups, validation and reload. Explicit error diagnostics are checked even if Sway exits validation with status zero. A private compositor test verifies old-key removal, new-key activation, conflict rejection and rollback.
- Other Wayland desktops can open their Global Shortcuts v2 configuration UI from Settings. This remains an event-driven portal session; version 1/missing portals report the desktop-settings fallback. This does not add universal compositor capture/focus permission or universal automatic paste. RELEASES.md distinguishes bundled runtime dependencies from required desktop services and current platform limitations.

## 2026-10-23 — keyboard history auto-scroll

- Replaced the history scroll-position binding with an explicit reveal-selection operation on arrow navigation and list refresh. It scrolls only enough to show the selected row, includes group-header height, and clamps to the viewport bounds.
- Private Sway regression passed: newest selection beneath 12 pinned entries, 12 Up / 14 Down / 2 Up steps with screenshot assertions that the selected border remains visible, immediate Enter, and repeated Keep Open preserving identical viewport pixels. External-copy and search/session regressions also passed.
- Rust tests (38 passed, one benchmark ignored), formatting and Clippy passed. Windows and independent X11 runtime checks were not performed for this change.
- Rebuilt and verified the local AppImage with the same private Sway suite, then replaced dist/Cliprust.AppImage and restarted the user's app successfully. This local build uses the current host runtime; published cross-distribution release assets were not changed.

## 2026-10-23 — v0.1.1 release preparation

- Bumped the patch version for the keyboard auto-scroll fix. Dependency notice generation may fetch locked crates for other targets, fixing clean CI packaging failures caused by offline metadata resolution.

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

## 2026-10-24 — v0.1.2 release preparation

- Bumped the patch version for the compact UI and floating geometry fixes. Linux and Windows packages will be rebuilt in CI; published validation records distinguish automated checks from live desktop acceptance.
