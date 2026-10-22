//! Use the compositor-issued xdg-activation token on the existing native surface.
use anyhow::{Context, Result, bail};
use raw_window_handle::{HasDisplayHandle, HasWindowHandle, RawDisplayHandle, RawWindowHandle};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    protocol::{wl_registry, wl_surface},
};
use wayland_protocols::xdg::activation::v1::client::xdg_activation_v1;

struct State;
impl Dispatch<wl_registry::WlRegistry, wayland_client::globals::GlobalListContents> for State {
    fn event(
        _: &mut Self,
        _: &wl_registry::WlRegistry,
        _: wl_registry::Event,
        _: &wayland_client::globals::GlobalListContents,
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
wayland_client::delegate_noop!(State: ignore xdg_activation_v1::XdgActivationV1);

pub fn activate(window: &slint::winit_030::winit::window::Window, token: &str) -> Result<()> {
    let (RawDisplayHandle::Wayland(display), RawWindowHandle::Wayland(surface)) = (
        window.display_handle()?.as_raw(),
        window.window_handle()?.as_raw(),
    ) else {
        bail!("Activation token requires a native Wayland window");
    };
    // Winit owns these handles. They stay alive for this synchronous call on
    // its UI thread; the foreign backend never disconnects/destroys them.
    let backend = unsafe {
        wayland_backend::client::Backend::from_foreign_display(display.display.as_ptr().cast())
    };
    let connection = Connection::from_backend(backend);
    let id = unsafe {
        wayland_backend::client::ObjectId::from_ptr(
            wl_surface::WlSurface::interface(),
            surface.surface.as_ptr().cast(),
        )
    }?;
    let surface = wl_surface::WlSurface::from_id(&connection, id)?;
    let (globals, queue) = wayland_client::globals::registry_queue_init::<State>(&connection)?;
    let activation: xdg_activation_v1::XdgActivationV1 =
        globals
            .bind(&queue.handle(), 1..=1, ())
            .context("Compositor does not expose xdg_activation_v1")?;
    activation.activate(token.to_owned(), &surface);
    activation.destroy();
    connection
        .flush()
        .context("Send compositor activation request")?;
    Ok(())
}
