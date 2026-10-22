# Architecture

Cliprust uses Rust, Slint's Winit backend and software renderer, SQLite with bundled FTS5. No browser, Node runtime or cloud services.

- model: validated format bundles, bounded content, canonical content hashes and summaries.
- persistence: versioned schema, transactions, blob/metadata separation, FTS5, pinning, retention, source-app filtering, export/import and backup.
- search: NFKC normalization, exact/prefix/substring/boundary/acronym/subsequence scoring with explicit weights.
- snippets: local templates and safe dynamic expansion; no global keystroke interception.
- platform: capability-specific native clipboard, shortcut, focus, per-user autostart, appearance, and source-application integration. Wayland clipboard monitoring prefers wlr-data-control and falls back to standardized ext-data-control; its global shortcut uses the XDG Global Shortcuts portal when available, and system appearance follows the XDG appearance portal. Windows appearance watches the current-user app-theme registry value. Source names come from the clipboard owner process on Windows and the owner/ancestor `WM_CLASS` on X11; Wayland does not expose the clipboard owner app through these data-control protocols. It never uses the X11 backend merely because DISPLAY is present.
- app: background worker owns storage and clipboard writes; UI receives bounded result sets and previews via event-loop callbacks. No SQLite or clipboard reads in key callbacks.

Clipboard data stays local in the user's application-data directory. Unix storage directory is private. Encryption at rest is not implemented. Do not imply password detection where no reliable signal exists. Raw imports are untrusted and must pass the same size/format validation as native capture.

Capture policy is shared by desktop and CLI monitoring, runs before persistence, and enforces pause and validated exact source exclusions. Invalid policy fails closed. Unknown owner metadata is retained as unknown rather than inferred from foreground focus. SQL filters precede bounded search candidate selection.

Shortcut parsing is shared by UI and CLI settings and platform adapters. In-window action resolution runs synchronously from an immutable startup keymap; native registration consumes a validated startup global binding. Configuration changes require restart. Native registration errors are surfaced through diagnostic events.

File chooser requests run on dedicated threads and return paths plus a snapshot of the selected bundle to the worker, avoiding selection races and keeping capture active while a dialog waits. SQLite merge restore opens the backup read-only, checks integrity, preserves same-name local snippets/settings, and merges through the import transaction.

Grouping stably rearranges displayed entries and attaches headers to the first item in each group; headers never become selectable rows. Scroll positions account for header height. Explicit duplicates use the content hash plus a random `:copy:` suffix as a storage identity; normal capture uses the unsuffixed hash. Import only preserves a copy suffix when its base matches the validated bundle hash and the suffix is 32 hexadecimal characters.

Tray ownership stays on the Slint UI thread. Native tray/menu handlers enqueue ordinary worker requests; pause changes update the label and tooltip only when state changes. The Linux backend provides StatusNotifierItem/DBusMenu via KSNI; Windows uses the native notification area. Dropping the tray on event-loop exit unregisters it. Snapshot runs skip tray creation.

A shared monitor supervisor retries backend termination/errors with exponential backoff capped at 30 seconds; a connection lasting at least a minute resets the delay. It invalidates capture epochs before reconnecting. Wayland device Finished events end the backend loop instead of leaving a disconnected worker blocked indefinitely. Reconnection uses the original process display environment.

Unified snippet search scores one transient snippet document at a time using the shared history scorer and configurable weights, retaining only display metadata and scores for sorting. Negative snippet IDs preserve the original snippet-array index. Empty-query pin ordering remains stable; explicit history grouping runs after ranking.
