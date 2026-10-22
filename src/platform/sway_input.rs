//! Native virtual keyboard with a complete evdev keymap for toolkit accelerator compatibility.
use anyhow::{Context, Result};
use std::{
    io::Write,
    os::fd::{AsFd, FromRawFd},
    time::Duration,
};
use wayland_client::{
    Connection, Dispatch, QueueHandle,
    protocol::{wl_registry, wl_seat},
};
mod protocol {
    use wayland_client;
    use wayland_client::protocol::*;
    pub mod __interfaces {
        use wayland_client::protocol::__interfaces::*;
        wayland_scanner::generate_interfaces!("protocols/virtual-keyboard-unstable-v1.xml");
    }
    use self::__interfaces::*;
    wayland_scanner::generate_client_code!("protocols/virtual-keyboard-unstable-v1.xml");
}
use protocol::{
    zwp_virtual_keyboard_manager_v1::ZwpVirtualKeyboardManagerV1 as Manager,
    zwp_virtual_keyboard_v1::ZwpVirtualKeyboardV1 as Keyboard,
};
#[derive(Default)]
struct State {
    seat: Option<wl_seat::WlSeat>,
    manager: Option<Manager>,
}
impl Dispatch<wl_registry::WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        registry: &wl_registry::WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        qh: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global {
            name,
            interface,
            version,
        } = event
        {
            match interface.as_str() {
                "wl_seat" if state.seat.is_none() => {
                    state.seat = Some(registry.bind(name, version.min(7), qh, ()))
                }
                "zwp_virtual_keyboard_manager_v1" => {
                    state.manager = Some(registry.bind(name, 1, qh, ()))
                }
                _ => {}
            }
        }
    }
}
wayland_client::delegate_noop!(State: ignore wl_seat::WlSeat);
wayland_client::delegate_noop!(State: ignore Manager);
wayland_client::delegate_noop!(State: ignore Keyboard);
pub struct Input {
    queue: wayland_client::EventQueue<State>,
    state: State,
    keyboard: Keyboard,
    ctrl: u32,
}
impl Input {
    pub fn prepare() -> Result<Self> {
        let connection = Connection::connect_to_env().context("Connect Sway virtual keyboard")?;
        let mut queue = connection.new_event_queue();
        let qh = queue.handle();
        connection.display().get_registry(&qh, ());
        let mut state = State::default();
        queue.roundtrip(&mut state)?;
        let manager = state
            .manager
            .as_ref()
            .context("Compositor does not permit the Wayland virtual keyboard protocol")?;
        let seat = state.seat.as_ref().context("No Wayland keyboard seat")?;
        let keyboard = manager.create_virtual_keyboard(seat, &qh, ());
        let context = xkbcommon::xkb::Context::new(xkbcommon::xkb::CONTEXT_NO_FLAGS);
        let map = xkbcommon::xkb::Keymap::new_from_names(
            &context,
            "evdev",
            "pc105",
            "us",
            "",
            None,
            xkbcommon::xkb::KEYMAP_COMPILE_NO_FLAGS,
        )
        .context("Compile paste keyboard map")?;
        let ctrl = 1u32
            .checked_shl(map.mod_get_index(xkbcommon::xkb::MOD_NAME_CTRL))
            .context("Control modifier missing from keyboard map")?;
        let mut data = map
            .get_as_string(xkbcommon::xkb::KEYMAP_FORMAT_TEXT_V1)
            .into_bytes();
        data.push(0);
        // Anonymous, close-on-exec descriptor; no clipboard data is put into a filesystem file.
        let fd = unsafe { libc::memfd_create(c"cliprust-keymap".as_ptr(), libc::MFD_CLOEXEC) };
        if fd < 0 {
            return Err(std::io::Error::last_os_error().into());
        }
        let mut file = unsafe { std::fs::File::from_raw_fd(fd) };
        file.write_all(&data)?;
        keyboard.keymap(1, file.as_fd(), data.len() as u32);
        queue.roundtrip(&mut state)?;
        Ok(Self {
            queue,
            state,
            keyboard,
            ctrl,
        })
    }
    pub fn paste(mut self) -> Result<()> {
        let started = std::time::Instant::now();
        self.keyboard.key(0, 29, 1); // evdev KEY_LEFTCTRL
        self.keyboard.modifiers(self.ctrl, 0, 0, 0);
        self.queue.roundtrip(&mut self.state)?;
        std::thread::sleep(Duration::from_millis(20));
        self.keyboard
            .key(started.elapsed().as_millis() as u32, 47, 1); // evdev KEY_V
        self.queue.roundtrip(&mut self.state)?;
        std::thread::sleep(Duration::from_millis(20));
        self.keyboard
            .key(started.elapsed().as_millis() as u32, 47, 0);
        self.keyboard
            .key(started.elapsed().as_millis() as u32, 29, 0);
        self.keyboard.modifiers(0, 0, 0, 0);
        self.queue.roundtrip(&mut self.state)?;
        // Let clients consume accelerators before withdrawing the keyboard/keymap.
        std::thread::sleep(Duration::from_millis(80));
        Ok(())
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        self.keyboard.modifiers(0, 0, 0, 0);
        self.keyboard.destroy();
        let _ = self.queue.flush();
    }
}
