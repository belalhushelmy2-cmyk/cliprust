# KDE Plasma Wayland integration

This integration is implemented but has not been accepted on a live Plasma session. Development/contract tests ran on Sway. Do not interpret compilation or the JavaScript contract tests as proof of working KDE paste.

## Explicit setup

1. Install the Cliprust desktop file and binary/AppImage normally. Use a native Wayland session, with `WAYLAND_DISPLAY` inherited from Plasma. Do not force the X11 backend.
2. Check System Settings → Keyboard → Shortcuts for **Meta+V**, especially Clipboard/Klipper. Cliprust performs a read-only KGlobalAccel availability check and requests its preferred trigger through the GlobalShortcuts portal. The portal displays the accepted shortcut. Choose another free key, such as Meta+Shift+V, if necessary. Cliprust does not unregister Klipper or edit KDE settings. A command shortcut invoking `cliprust --show` is an alternative; enable startup notification on the launcher.
3. To opt into placement and paste focus restoration, install the supplied KWin script: `kpackagetool6 --type KWin/Script --install packaging/kde/cliprust`. For the standalone AppImage, first extract it with `Cliprust.AppImage --appimage-extract` and install `squashfs-root/usr/share/doc/cliprust/kde/cliprust` instead. Then explicitly enable **Cliprust window integration** under System Settings → Window Management → KWin Scripts. These commands/settings are not run automatically. Restart Cliprust after enabling. To remove it, disable it there, then run `kpackagetool6 --type KWin/Script --remove cliprust`.
4. Summon Cliprust from the intended destination application. The script remembers that window, desktop and monitor. It refuses closed/stale destinations. A first launch without summoning may have no target: invoke the shortcut from the destination app first.
5. Enter requests paste. The first request opens the desktop's **RemoteDesktop keyboard permission** dialog. Only keyboard access is requested, not pointer access or screen capture. Denial/cancellation fails before clipboard replacement. Permission is held only for this app process; a failed/revoked session requests permission again on the next attempt.
6. **Ctrl+Enter** is the default Paste and Keep Open binding. Existing custom bindings are preserved; change Settings → Shortcuts or use `cliprust config keymap '{"keep-open":"Ctrl+Enter"}'` and restart to change them. This CLI example replaces the custom keymap, so merge other custom actions if needed.

The script runs inside KWin only after explicit enablement and uses documented scripting APIs for placement and focus. It does not register global shortcuts. Its private D-Bus bridge records only transient target identifiers; no clipboard contents pass through it. Clipboard MIME bundles are restored via the existing data-control writer. Input uses the consented RemoteDesktop portal; no X11 injection or privileged device access is used on Wayland. No fallback sends keys to an unidentified active application.

A successful paste result means **the compositor confirmed the destination and the portal accepted the input requests**. Clipboard consumption and application-level insertion are not observable here. Cliprust stays visible on errors. Keep-open returns focus to Cliprust after input submission; it keeps query and selection. KWin focus policy, permission UI, protocol availability, physical held modifiers, and destination behavior still require runtime acceptance. There is no universal portal API for enumerating/focusing arbitrary application windows, which is why the optional KWin script is needed.

## Acceptance checklist (pending on Plasma)

Test with actual Plasma 6 and xdg-desktop-portal-kde: free/conflicting Meta+V including Klipper, declined shortcut registration, hidden/minimized existing window, repeated invocation on another desktop, search focus/query retention, two monitors and monitor removal, text/HTML/image/URI-list restoration into compatible apps, Enter and Ctrl+Enter, denied/revoked keyboard permission, closed destination and focus refusal. Check stderr for diagnostics. Also validate Windows and X11 paste/shortcut behavior on their native sessions.

## References

- [GlobalShortcuts portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GlobalShortcuts.html)
- [RemoteDesktop portal](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.RemoteDesktop.html)
- [KWin scripting API](https://develop.kde.org/docs/plasma/kwin/api/)
- [KGlobalAccel D-Bus interface](https://github.com/KDE/kglobalaccel/blob/master/src/org.kde.KGlobalAccel.xml)
