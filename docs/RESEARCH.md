# Public-source research — 2026-10-22

## Evidence and scope

- [Raycast Clipboard History manual](https://manual.raycast.com/clipboard-history): documents text, images, files, links, emails, colors; preserving original formats; multi-file grouping; search/type filters; paste into active input; action panel; edits, pinning, snippets and retention. Also documents OCR, QR recognition, sharing and AI actions. These last features are outside the initial local clipboard engine scope, not equivalent features we claim to ship.
- [Clipboard developer API](https://developers.raycast.com/api-reference/clipboard): documents copy, paste, read and concealed copies. It is an interface contract, not an implementation of the native engine.
- [Raycast GitHub organization](https://github.com/raycast), [extensions README](https://github.com/raycast/extensions/blob/master/README.md), [script commands](https://github.com/raycast/script-commands): public extension ecosystem and scripts. No proprietary clipboard engine or ranking implementation was accessed. No assets or implementation code copied.
- [Product](https://www.raycast.com), [Windows product](https://www.rayca.st/windows): keyboard-first launcher, clipboard history and snippets. Marketing does not establish technical implementation details or platform parity.

Paste and Keep Open is a Cliprust requirement. It is not established by the above evidence as a Raycast core feature. Ranking weights, deduplication, storage and UI here are original engineering decisions. No behavior has been personally observed in an installed Raycast client.

## Desktop integration evidence

- [wl-clipboard](https://github.com/bugaevc/wl-clipboard) supports compositor-dependent clipboard access and event watching.
- [ext-data-control-v1 protocol](https://gitlab.freedesktop.org/wayland/wayland-protocols/-/blob/main/staging/ext-data-control/ext-data-control-v1.xml): standardized privileged clipboard-manager access is a compositor capability, not universal Wayland access. Cliprust supports it as a fallback to the widely deployed wlr protocol; compositor-specific live acceptance is still required.
- [GlobalShortcuts portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html): session-based shortcuts and optional activation tokens.
- [RemoteDesktop portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html): input requires a granted session. It does not grant arbitrary previous-window activation.
- [clipboard-rs](https://github.com/ChurchTao/clipboard-rs): native Windows and X11 read/write/watch. Its optional Wayland watcher polls and misses successive image-only changes with identical MIME types; Cliprust must not use that watcher.

## Implementation specification

Keep format bundles as an entry, hash MIME names plus bytes in canonical order, never deduplicate only the visible preview. Store metadata separately from blobs. FTS5 supplies indexed candidates; deterministic Unicode normalization and independently weighted fuzzy scoring rerank them. Pins are exempt from automatic cleanup. Database transactions protect bundles and indexes.

The window preserves query and selected entry identity through background changes and keep-open operations. A failed paste must remain visible and must report the reason. Copy and paste are separate actions. Unsupported Wayland paste must return a capability error before changing the clipboard. No X11 input injection on Wayland, fake-input protocol, uinput, or privileged daemon fallback.

Platform acceptance requires real application testing per platform. Compilation, mocked focus transitions and storage tests do not establish native paste success. Performance targets require measurement; none are assumed.
