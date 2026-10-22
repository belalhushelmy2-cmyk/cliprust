# Feature matrix

Statuses distinguish implementation from live validation. “Partial” is not a parity claim.

| Requirement | Current implementation | Remaining / validation |
|---|---|---|
| Rust, Slint, SQLite, FTS5 | Implemented | Native binary/UI QA in TEST_RESULTS |
| Public Raycast research | Recorded with official sources | No proprietary source/algorithm access |
| Text, Unicode, URLs, colors | Bundle model, classification, persistence | Native platform results tracked separately |
| HTML, RTF | Capture/write common formats | Arbitrary native formats not preserved |
| Images | Native PNG; Wayland PNG/JPEG/BMP; lazy preview | Huge/malformed decoded safely; actual target apps need tests |
| Files/multiple files | One bundle with references | Windows URI normalization and file-manager roundtrip need validation |
| Event monitoring | Windows/X11 library; Wayland wlr-data-control with ext-data-control fallback | wlr live-tested including reconnect; standardized ext-only compositor validation pending |
| Source application | Clipboard owner process executable on Windows; owner/ancestor `WM_CLASS` on X11 | Wayland owner identity is not exposed by these data-control protocols; live Windows/X11 capture validation pending |
| Confidentiality | Known reliable type markers skipped | No universal password/source detection |
| Deduplication, persistence, pins, retention | Implemented | UI exposes history-count and age limits; pinned items reorder with Ctrl+Shift+Up/Down; backup and merge-restore UI implemented |
| Search | Indexed candidates + independent deterministic ranking; type, pinned, calendar date, and recorded source-app filters | Wayland does not expose owner identity; preview index limit and bounded fuzzy candidate set mean no full-history fuzzy guarantee |
| Appearance and UI | Original Slint interface, previews, filters, actions; Dark/Light/System palettes | System mode follows Linux XDG appearance notifications and Windows app-theme registry changes; date/pin/type/application grouping implemented; source icons pending |
| Keyboard | Navigation, search focus, paste actions, copy, pin, delete, pinned reordering, actions | Configurable seven-action keymap and three Settings bindings; validation rejects duplicates; restart required; navigation remains fixed |
| Global shortcut | Configurable native Windows/X11 shortcut (default Meta/Super+V); Sway Mod4+V compositor binding; Wayland portal preference + activation-token IPC; KDE conflict query | Wayland portal acceptance, Windows single-instance pending; manual command binding remains fallback |
| Paste | Native Windows/X11 focus + input code | Actual application validation pending; input sent does not prove acceptance |
| Paste and Keep Open | Distinct code path preserves visibility/query/selection | Actual Windows/X11 repeated-paste validation pending |
| Wayland paste | Sway native IPC focus/placement + native virtual keyboard; opt-in KWin target/focus bridge + keyboard-only RemoteDesktop portal | Implemented, not live-validated on KDE; explicit script enablement and portal consent required. Sway acceptance is recorded in TEST_RESULTS.md; other compositors remain unsupported |
| Snippets | Local CRUD/tags/aliases/placeholders and pinning; pinned snippets participate in favorites and all-history search includes name/body/tags/alias | Shared fuzzy/acronym ranking implemented; automatic typing expansion pending |
| Actions | Copy/plain copy/plain paste/plain Paste and Keep Open/copy image/copy file paths/edit/duplicate/pin/delete/export/save image; Linux/Windows URL/file open | Windows action compile-check only; chooser adapters implemented, interactive acceptance pending |
| History management | Retention, pause, exact source exclusions, recent search recall, export/import, backup, v1 migration | Merge-restore UI implemented; broader settings and encryption pending; unknown sources cannot match exclusions |
| Background operation | Hide keeps monitoring alive; native tray Show/Pause/Resume/Quit; optional per-user autostart on Linux and Windows | Linux SNI action test passed; Windows tray/autostart runtime and actual suspend/resume validation pending; monitor disconnect/reconnect tested in private Sway |
| Multi-monitor/HiDPI | Toolkit scaling and WM placement | Position settings and actual multi-monitor validation pending |
| Performance | Bounded result sets and off-UI work | Benchmarks and idle target validation in TEST_RESULTS |
| Releases | CI recipes; current AppImage built/smoke-tested locally; Arch and DEB builds from earlier checkpoints (see TEST_RESULTS.md) | Installation on Arch and Debian/Ubuntu, Windows NSIS runtime, AppImage cross-distribution compatibility and CI release execution pending |

This does not satisfy all final requirements yet. The main unresolved engineering work is KDE integration acceptance and other Wayland compositor support, real target-app acceptance tests, broader privacy settings, complete format negotiation, and installation/runtime validation on Windows, Arch, and Debian.
