# Sway integration

Run Cliprust inside the Sway session with `SWAYSOCK` and `WAYLAND_DISPLAY` available. The default paste backend implements the permitted Wayland virtual-keyboard protocol directly with a full evdev keyboard map. No root privileges are needed to run Cliprust or paste. Clipboard capture and restoration use Wayland data-control directly; `wl-clipboard` is useful for manual diagnostics but is not required by the GUI.

Configure Sway:

```
bindsym --no-repeat Mod4+v exec cliprust --toggle
```

For an AppImage, replace `cliprust` with its absolute quoted path. Meta+V hides the focused launcher, otherwise summons the existing process on the invoking workspace/monitor and focuses search. Native Sway IPC handles floating placement. `--show` always summons it.

Enter restores the selected entry's original clipboard formats, verifies publication, restores the recorded application container's focus, and sends Ctrl+V through the native virtual keyboard. Ctrl+Enter does the same while keeping the window visible and returning focus to it; the query and destination survive repeated pastes. Existing customized keymaps are preserved. Applications must support Ctrl+V: terminals commonly use Ctrl+Shift+V, so configure their paste binding to Ctrl+V when using this integration.

The target container/PID must still exist and must not be Cliprust. Closed windows, failed IPC, changed clipboard ownership and failed input execution produce diagnostics. Set `CLIPRUST_SWAY_INPUT=wtype` before launching to select the optional exact `wtype -M ctrl v -m ctrl` backend; missing/failing wtype then produces an actionable diagnostic. That backend failed stock GTK/Mousepad accelerator acceptance on this host, which is why the native backend is the default. Successful input submission cannot prove that every arbitrary destination accepted its clipboard format. There remains a small unavoidable focus race if another application or the user changes focus between verification and virtual-keyboard delivery.

## Configuration helper

Run `python3 scripts/sway-shortcut.py --executable /absolute/path/Cliprust.AppImage` to inspect the loaded root configuration and its on-disk includes. With `--apply`, the helper replaces only a unique existing Cliprust Meta+V binding, backs up its file under `~/.config/cliprust/sway-backups`, validates the entire configuration, then reloads Sway. It never overwrites another application's binding. If no binding exists, it prints the binding to add. Dynamic includes, unresolved variables and Meta keycode bindings require manual review. This conservative helper is not a complete Sway parser, and Sway does not expose fully expanded effective bindings through GET_CONFIG.

## Acceptance test

`python3 tests/native_sway_paste.py` creates an isolated two-output headless Sway session and temporary database. It drives actual keys and checks clipboard consumption in stock GTK and foot; Mousepad receives a separate changed-document/screenshot check when installed. It does not touch the desktop clipboard. Set `CLIPRUST_TEST_BINARY` to an absolute release binary path to test that build.
