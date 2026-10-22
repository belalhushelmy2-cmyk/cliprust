# Cliprust

A Rust + Slint clipboard history application with SQLite FTS5 and local storage.

**Development status: incomplete.** This repository implements a usable history foundation, not the full requested Raycast-level release. In particular, KDE Wayland paste now has an opt-in KWin + consented RemoteDesktop implementation, but needs real Plasma acceptance testing; Sway has native IPC focus restoration and native virtual-keyboard paste; other Wayland desktops still lack automatic paste. Windows/X11 native paste still needs real application validation. See [FEATURE_PARITY.md](FEATURE_PARITY.md) before relying on platform support.

## Build and run

Requires Rust 1.92+, a desktop session and development libraries for Fontconfig, XKB, X11 and Wayland on Linux. No Node, browser runtime, Electron or WebView is used. Slint uses the software renderer.

```sh
cargo run --release
# Configure this command in KDE System Settings > Keyboard > Shortcuts:
cliprust --show
```

Bind the command to a free shortcut such as Meta+Shift+V if your desktop does not implement the Global Shortcuts portal. On Linux, the running process receives show requests over a private local socket. Closing the window or Escape hides it while capture continues. Use Quit to exit. Native Meta/Super+V registration is implemented for Windows/X11, with conflict diagnostics; Wayland requests registration through the desktop portal and reports when the portal is unavailable. Settings supports launch at sign-in and Dark, Light, or System appearance. System appearance follows the XDG desktop appearance portal on Linux and the current-user Windows app-theme registry setting; unavailable or unspecified system preferences fall back to dark. The native tray and monitor reconnection are implemented; real sleep/resume validation remains pending. See [KDE_INTEGRATION.md](KDE_INTEGRATION.md) for explicit KDE setup, conflict handling, and current test limits.

Clipboard capture uses native Windows clipboard notifications and X11 selection notifications via clipboard-rs. On Wayland it prefers wlr-data-control events and falls back to the standardized ext-data-control protocol. A compositor exposing neither displays a diagnostic. The application does not route Wayland clipboard monitoring or input through XWayland. The ext-data-control path compiles and shares the tested capture pipeline, but needs validation on a compositor that exposes only that protocol. Sway paste uses a native virtual keyboard; KDE portal paste requires explicit consent.

## Current interactions

- Type a query in search; Ctrl+F focuses search. Click a history entry, then Up/Down selects it.
- Enter sends paste input to the previous application on supported backends; Ctrl+Enter sends paste input and keeps the window open. Unsupported backends display a reason.
- Ctrl+C copies the selected history entry when the list has focus. Search-field text retains standard copy/select behavior.
- Ctrl+P pins/unpins; Ctrl+Shift+Up/Down reorders pinned entries; Delete deletes; Ctrl+K opens Actions; Escape dismisses a panel or the window.
- Type filters, Today/Yesterday/Last 7 Days filters, and a pinned-only view; original text, HTML, RTF, image and file-reference bundles are persisted where captured. Unknown application-specific formats are not promised to round-trip.
- Source application names are recorded from the active window on Windows and X11 when available. Wayland clipboard offers do not expose the owner application to this monitor, so that field is unavailable there.
- Use the APP selector to filter history to a recorded source application; the options update as sources appear in history.
- Preview images are decoded with allocation/dimension limits, generated lazily for the selected item.
- Create/edit/delete and pin tagged snippets; date, time and clipboard placeholders expand in a single pass. All-history search includes snippet names, bodies, tags and abbreviations; automatic typing expansion is not implemented.
- Pause stops storage of new capture events. Data stays local. No encryption at rest is implemented.

The window requests an always-on-top level where supported. Tiling Wayland compositors control floating placement. For Sway, add `for_window [app_id="^(org\.cliprust\.Cliprust|cliprust)$"] floating enable` to your own compositor configuration if desired.

Native UI actions include copy, plain copy, paste as plain text, plain-text Paste and Keep Open, copy image formats, copy file paths from URI lists, pin, text editing into a new entry, saving snippets, deletion, clearing unpinned entries, per-entry JSON export, image save and opening links/file references on Linux and Windows. Windows uses the native shell open verb. Exports/images currently save into the database directory and display the path.

The UI limits displayed results to 300. FTS supplies up to 1000 indexed candidates plus 1000 recent/pinned candidates for fuzzy matching. Search is indexed over an 8192-character preview; full oversized-text indexing and typo matching over the entire database remain pending. Ranking weights are explicit in `src/search.rs`; they are not Raycast's algorithm. Pins are exempt from retention. Desktop retention defaults to 30 days and 10000 unpinned entries; change these from the Settings button or with the `retention_days` and `max_entries` CLI settings. Zero retention days disables age-based expiry; zero maximum entries removes all unpinned entries. Appearance can be configured with `theme` (`dark`, `light`, or `system`), and ranking weights can be stored as a JSON `search_weights` setting.

## Command line

```sh
cliprust doctor
cliprust config max_entries 10000
cliprust config retention_days 30
cliprust config theme system
cliprust add 'some text'
cliprust list 'some'
cliprust pin 1
cliprust export history.json
cliprust import history.json
cliprust backup history-backup.db
cliprust prune --max 10000 --days 30
cliprust snippet save Greeting 'Hello! Today is {{date}}.' --tags work --abbreviation '!hi'
cliprust snippet list
cliprust monitor --seconds 30
```

Use `--database /absolute/path/history.db` to isolate a test database. CLI `list` and exports intentionally contain clipboard content; diagnostics and capture logs print identifiers/types only. Imports are capped at 256 MiB; bundles at 32 MiB. Exports and backups refuse to overwrite existing files. On Wayland, CLI copy starts a clipboard-owner child that exits when its offer is replaced. On X11, CLI copy stays in the foreground to serve the selection until stopped; the GUI handles ownership in its running process.

Unix-created files are private through a process umask. Existing file permissions are not modified. Pause does not erase older history. Reliable confidentiality markers are excluded conservatively; this is not general password detection. Settings accepts excluded application names, one per line, matched exactly without ASCII case sensitivity against identified clipboard owners. Unknown sources, including Wayland, cannot be excluded by name; use Pause for those. Sensitive-item expiry remains pending. CLI example: `cliprust config excluded_apps '["keepassxc","secret.exe"]'`. Invalid exclusion policy stops automatic capture.

## Verification

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
# Explicit live test: temporarily replaces then restores the clipboard
python3 tests/native_wayland.py
cargo test --release --test core benchmark_10000 -- --ignored --nocapture
# Render the actual window with an isolated database, then quit:
cliprust --database /tmp/cliprust-qa/history.db --snapshot /tmp/cliprust-window.png
```

See [TEST_RESULTS.md](TEST_RESULTS.md) for actual results, [PROGRESS.md](PROGRESS.md) for stage checkpoints, [ARCHITECTURE.md](ARCHITECTURE.md) for ownership/concurrency, and [docs/RESEARCH.md](docs/RESEARCH.md) for public-source evidence.

## Packaging

Packaging recipes are in `packaging/`. The Arch PKGBUILD has been built and its package contents inspected on Garuda/Arch Linux. `scripts/deb.sh` builds a Debian archive with cargo-deb; its metadata and payload were inspected, but installation still needs validation on Debian/Ubuntu. See [TEST_RESULTS.md](TEST_RESULTS.md). The x86_64 AppImage was built and smoke-tested on the development host. Its script accepts trusted APPIMAGETOOL and optional APPIMAGE_RUNTIME paths; CI downloads pinned, SHA-verified versions. System libraries are not bundled, so build on the oldest supported distribution and validate compatibility before distribution. The NSIS recipe describes a per-user Windows EXE installer. These are not verified release installers until run on their target platforms. CI builds/tests Windows and Linux and assembles release artifacts on version tags.

GPL-3.0-only. Original identity and implementation; not affiliated with Raycast. Slint and other dependency licenses apply separately.

Recent searches remembers up to 20 queries when a result is copied/pasted or the window is dismissed. Recall them from the search-bar menu; clear them in Settings. Pause prevents storing new queries.

### Configurable shortcuts

Settings configures the global launcher and the two independent paste actions. Quit and reopen Cliprust after saving. The portal may choose a different global trigger on Wayland. In-window action overrides are also available through the CLI:

```sh
cliprust config global_shortcut 'Ctrl+Alt+V'
cliprust config keymap '{"paste":"Alt+Enter","keep-open":"Ctrl+Shift+Enter","actions":"Ctrl+J"}'
```

Supported action names: `paste`, `keep-open`, `copy`, `pin`, `delete`, `actions`, `search-focus`. Omitted actions retain defaults. Modifiers are `Ctrl`, `Alt`, `Shift`, `Super`; keys are letters, digits, `Enter`, `Delete`. Letter/digit bindings require Ctrl, Alt or Super. Duplicate bindings and Ctrl+A are rejected; native global registration reports OS conflicts. Escape, arrow navigation and Ctrl+Shift+Up/Down pin ordering remain fixed.

Settings provides Backup, Restore Backup, Export History and Import History. Restore merges SQLite backup history/snippets, retains current settings and same-name local snippets, and validates backup integrity first. CLI equivalent: `cliprust restore /path/to/backup.db`. Export Entry and Save Image open destination choosers. Saving requires a new filename. Linux uses the desktop FileChooser portal; Windows uses native common dialogs. Missing portal services produce a diagnostic; CLI file operations remain available.

The history GROUP selector offers date, pinned, type and application sections; None preserves the ordinary ranked list. The choice persists across launches. Duplicate in Actions creates an independent history item (CLI: `cliprust duplicate ID`) or a separately named snippet. Automatic capture still deduplicates repeated content.

The tray menu offers Show History, Pause/Resume Monitoring, and Quit. Left-click opens history where supported by the tray host. Linux uses StatusNotifierItem/DBusMenu (supported by KDE and compatible panels such as Waybar) without GTK/AppIndicator; a desktop without a tray host may show no icon. Windows uses its notification area. `python3 tests/native_tray.py` is an explicit Linux acceptance test with an isolated temporary database; it briefly resumes monitoring but never replaces clipboard contents.

Clipboard monitoring reconnects after backend errors using delays from one to 30 seconds. Successful connections remain event-driven. The private headless-compositor test `python3 tests/native_reconnect.py` verifies late display startup and reconnect after restart, without touching the desktop clipboard. A session that changes DISPLAY/WAYLAND_DISPLAY requires restarting Cliprust with the new environment; actual suspend/resume still needs platform validation.

Snippets and clipboard entries share search ranking, including fuzzy and acronym matching. Snippet names, full bodies, tags and abbreviations participate before the display limit, so a strong snippet match can appear alongside or above clipboard matches.

For reproducible Linux release measurements, build with `cargo build --release`, then run `python3 tests/measure_linux.py`. It creates a private headless Sway display and a temporary 10,000-entry database; it reports process RSS/PSS, idle CPU and IPC-to-window-manager-visible latency without changing the desktop clipboard. See TEST_RESULTS.md for measured values and limitations.

AppImage build tool provenance: [appimagetool 1.9.1](https://github.com/AppImage/appimagetool/releases/tag/1.9.1) and [type2 runtime 20251108](https://github.com/AppImage/type2-runtime/releases/tag/20251108). AppStream metadata has no homepage because this checkout has no public project URL; validation treats only that advisory as informational.

On Sway, bind `Mod4+v` to `cliprust --toggle` (or an absolute AppImage path). Native IPC summons the floating window on the current workspace/monitor and restores the original application for paste. See [SWAY_INTEGRATION.md](SWAY_INTEGRATION.md) for setup, safe conflict checking, and testing.

### Stable history while browsing

Opening Cliprust selects the latest copied history entry even when pins appear above it. Enter pastes immediately; Up/Down navigate. The list initially has keyboard focus: type to search or press Ctrl+F, and Escape clears the query before closing the window.

The visible history and its search metadata stay frozen until the window closes and reopens. Copying or pasting an existing entry does not move it, and Paste and Keep Open preserves the query, selection and scroll. External copies continue recording in the background. Explicit history edits, pin changes, deletion and imports still update the view.

Distribution formats and build requirements are documented in [RELEASES.md](RELEASES.md).
