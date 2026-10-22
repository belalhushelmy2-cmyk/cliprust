use super::Target;
use anyhow::{Context, Result, bail};
use x11rb::{
    connection::Connection,
    protocol::{
        xproto::{AtomEnum, ClientMessageData, ClientMessageEvent, ConnectionExt, EventMask},
        xtest::ConnectionExt as _,
    },
};
pub fn target() -> Result<Target> {
    let (conn, screen) = x11rb::connect(None)?;
    let atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
        .reply()?
        .atom;
    let value = conn
        .get_property(
            false,
            conn.setup().roots[screen].root,
            atom,
            AtomEnum::WINDOW,
            0,
            1,
        )?
        .reply()?;
    let window = value
        .value32()
        .and_then(|mut v| v.next())
        .unwrap_or_default();
    if window != 0 {
        let pid_atom = conn.intern_atom(false, b"_NET_WM_PID")?.reply()?.atom;
        let pid = conn
            .get_property(false, window, pid_atom, AtomEnum::CARDINAL, 0, 1)?
            .reply()?;
        if pid.value32().and_then(|mut v| v.next()) == Some(std::process::id()) {
            return Ok(Target(0));
        }
    }
    Ok(Target(window.into()))
}
pub fn source_application() -> Result<Option<String>> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let selection = conn.intern_atom(false, b"CLIPBOARD")?.reply()?.atom;
    let mut window = conn.get_selection_owner(selection)?.reply()?.owner;
    // Toolkits sometimes own the selection through a child/helper window.
    // Inspect its ancestors, never the unrelated foreground application.
    for _ in 0..32 {
        if window == 0 || window == root {
            break;
        }
        let class = conn
            .get_property(false, window, AtomEnum::WM_CLASS, AtomEnum::STRING, 0, 256)?
            .reply()?;
        if class.format == 8
            && class.type_ == u32::from(AtomEnum::STRING)
            && let Some(name) = wm_class_name(&class.value)
        {
            return Ok(Some(name));
        }
        let parent = conn.query_tree(window)?.reply()?.parent;
        if parent == window {
            break;
        }
        window = parent;
    }
    Ok(None)
}
fn wm_class_name(value: &[u8]) -> Option<String> {
    let parts: Vec<_> = value
        .split(|byte| *byte == 0)
        .filter(|part| !part.is_empty())
        .collect();
    let name = parts.get(1).or_else(|| parts.first())?;
    super::normalized_source_application(&String::from_utf8_lossy(name))
}
pub fn paste(target: Target) -> Result<()> {
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let atom = conn
        .intern_atom(false, b"_NET_ACTIVE_WINDOW")?
        .reply()?
        .atom;
    let window = u32::try_from(target.0)?;
    let event = ClientMessageEvent::new(32, window, atom, ClientMessageData::from([2, 0, 0, 0, 0]));
    conn.send_event(
        false,
        root,
        EventMask::SUBSTRUCTURE_REDIRECT | EventMask::SUBSTRUCTURE_NOTIFY,
        event,
    )?;
    conn.flush()?;
    for _ in 0..20 {
        if super::target()?.0 == target.0 {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if super::target()?.0 != target.0 {
        bail!("window manager refused focus restoration")
    }
    let mapping = conn
        .get_keyboard_mapping(
            conn.setup().min_keycode,
            conn.setup().max_keycode - conn.setup().min_keycode + 1,
        )?
        .reply()?;
    let find = |symbol: u32| -> Result<u8> {
        mapping
            .keysyms
            .chunks(mapping.keysyms_per_keycode.into())
            .position(|keys| keys.contains(&symbol))
            .map(|i| conn.setup().min_keycode + i as u8)
            .context("required key absent from X11 keyboard map")
    };
    let ctrl = find(0xffe3)?;
    let v = find(b'v'.into())?;
    for (kind, key) in [(2, ctrl), (2, v), (3, v), (3, ctrl)] {
        conn.xtest_fake_input(kind, key, 0, root, 0, 0, 0)?
            .check()?;
    }
    conn.flush()?;
    Ok(())
}

pub fn hotkey(
    tx: crossbeam_channel::Sender<super::Event>,
    shortcut: crate::shortcuts::Shortcut,
) -> Result<()> {
    use x11rb::protocol::{
        Event,
        xproto::{GrabMode, ModMask},
    };
    let (conn, screen) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen].root;
    let mapping = conn
        .get_keyboard_mapping(
            conn.setup().min_keycode,
            conn.setup().max_keycode - conn.setup().min_keycode + 1,
        )?
        .reply()?;
    let code = mapping
        .keysyms
        .chunks(mapping.keysyms_per_keycode.into())
        .position(|keys| keys.contains(&(shortcut.key.as_bytes()[0] as u32)))
        .map(|i| conn.setup().min_keycode + i as u8)
        .context("Shortcut key missing from keyboard mapping")?;
    for locks in [
        ModMask::default(),
        ModMask::LOCK,
        ModMask::M2,
        ModMask::LOCK | ModMask::M2,
    ] {
        conn.grab_key(
            false,
            root,
            (if shortcut.alt {
                ModMask::M1
            } else {
                ModMask::default()
            }) | (if shortcut.shift {
                ModMask::SHIFT
            } else {
                ModMask::default()
            }) | (if shortcut.ctrl {
                ModMask::CONTROL
            } else {
                ModMask::default()
            }) | (if shortcut.meta {
                ModMask::M4
            } else {
                ModMask::default()
            }) | locks,
            code,
            GrabMode::ASYNC,
            GrabMode::ASYNC,
        )?
        .check()
        .context("Configured shortcut conflicts with another X11 shortcut")?;
    }
    conn.flush()?;
    loop {
        if let Event::KeyPress(_) = conn.wait_for_event()? {
            let _ = tx.send(super::Event::Shortcut(target()?));
        }
    }
}
#[cfg(test)]
mod source_tests {
    use super::wm_class_name;

    #[test]
    fn wm_class_uses_application_class_and_falls_back_to_instance() {
        assert_eq!(
            wm_class_name(b"firefox\0Firefox\0").as_deref(),
            Some("Firefox")
        );
        assert_eq!(wm_class_name(b"instance\0").as_deref(), Some("instance"));
        assert_eq!(wm_class_name(b"\0"), None);
    }
}
