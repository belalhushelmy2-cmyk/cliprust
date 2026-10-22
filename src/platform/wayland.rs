use super::Event;
use crate::model::{Bundle, MAX_BUNDLE_BYTES, confidential};
use anyhow::{Context, Result, bail};
use crossbeam_channel::Sender as SyncSender;
use std::{
    collections::{BTreeMap, HashMap},
    io::Read,
    os::{
        fd::{AsFd, BorrowedFd},
        unix::net::UnixStream,
    },
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};
use wayland_client::{
    Connection, Dispatch, Proxy, QueueHandle,
    protocol::{wl_registry, wl_seat},
};
use wayland_protocols::ext::data_control::v1::client::{
    ext_data_control_device_v1 as ext_device, ext_data_control_manager_v1 as ext_manager,
    ext_data_control_offer_v1 as ext_offer,
};
use wayland_protocols_wlr::data_control::v1::client::{
    zwlr_data_control_device_v1 as device, zwlr_data_control_manager_v1 as manager,
    zwlr_data_control_offer_v1 as offer,
};

trait DataOffer {
    fn object_id(&self) -> wayland_client::backend::ObjectId;
    fn receive_mime(&self, mime: String, fd: BorrowedFd<'_>);
    fn destroy_offer(&self);
}

impl DataOffer for offer::ZwlrDataControlOfferV1 {
    fn object_id(&self) -> wayland_client::backend::ObjectId {
        Proxy::id(self)
    }
    fn receive_mime(&self, mime: String, fd: BorrowedFd<'_>) {
        self.receive(mime, fd);
    }
    fn destroy_offer(&self) {
        self.destroy();
    }
}

impl DataOffer for ext_offer::ExtDataControlOfferV1 {
    fn object_id(&self) -> wayland_client::backend::ObjectId {
        Proxy::id(self)
    }
    fn receive_mime(&self, mime: String, fd: BorrowedFd<'_>) {
        self.receive(mime, fd);
    }
    fn destroy_offer(&self) {
        self.destroy();
    }
}

struct State {
    finished: bool,
    manager: Option<manager::ZwlrDataControlManagerV1>,
    ext_manager: Option<ext_manager::ExtDataControlManagerV1>,
    seat: Option<wl_seat::WlSeat>,
    offers: HashMap<wayland_client::backend::ObjectId, Vec<String>>,
    tx: SyncSender<Event>,
    in_flight: Arc<AtomicUsize>,
}
pub fn watch(tx: SyncSender<Event>) -> Result<()> {
    let conn = Connection::connect_to_env().context("connect Wayland display")?;
    let mut queue = conn.new_event_queue();
    let qh = queue.handle();
    let mut state = State {
        finished: false,
        manager: None,
        ext_manager: None,
        seat: None,
        offers: HashMap::new(),
        tx,
        in_flight: Arc::new(AtomicUsize::new(0)),
    };
    conn.display().get_registry(&qh, ());
    queue.roundtrip(&mut state)?;
    let seat = state.seat.as_ref().context("Wayland seat unavailable")?;
    let _device = if let Some(manager) = state.manager.as_ref() {
        DataDevice::Wlr {
            _device: manager.get_data_device(seat, &qh, ()),
        }
    } else if let Some(manager) = state.ext_manager.as_ref() {
        DataDevice::Ext {
            _device: manager.get_data_device(seat, &qh, ()),
        }
    } else {
        bail!(
            "Compositor exposes neither wlr-data-control nor ext-data-control; this backend cannot monitor the clipboard."
        )
    };
    let _ = state.tx.try_send(Event::Diagnostic(
        "Wayland clipboard monitoring connected".into(),
    ));
    loop {
        queue.blocking_dispatch(&mut state)?;
        if state.finished {
            bail!("Compositor ended the clipboard data-control session");
        }
    }
}

enum DataDevice {
    Wlr {
        _device: device::ZwlrDataControlDeviceV1,
    },
    Ext {
        _device: ext_device::ExtDataControlDeviceV1,
    },
}

impl State {
    fn remember_offer<O: DataOffer>(&mut self, offer: O) {
        self.offers.entry(offer.object_id()).or_default();
    }

    fn capture_offer<O: DataOffer>(&mut self, offer: O, conn: &Connection) {
        let id = offer.object_id();
        let Some(epoch) = super::capture_token() else {
            self.offers.remove(&id);
            offer.destroy_offer();
            return;
        };
        if self.in_flight.fetch_add(1, Ordering::SeqCst) >= 4 {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            offer.destroy_offer();
            let _ = self.tx.try_send(Event::Diagnostic(
                "Clipboard burst exceeded four concurrent transfers; offer skipped.".into(),
            ));
            return;
        }
        let types = self.offers.remove(&id).unwrap_or_default();
        if confidential(&types) || super::ownership::is_own_offer(&types) {
            self.in_flight.fetch_sub(1, Ordering::SeqCst);
            offer.destroy_offer();
            return;
        }
        let unsupported = types.iter().filter(|mime| !supported(mime)).count();
        if unsupported > 0 {
            eprintln!(
                "Cliprust clipboard: skipped {unsupported} unsupported advertised MIME types"
            );
        }
        if !types.iter().any(|mime| supported(mime)) {
            let _ = self.tx.try_send(Event::Diagnostic("Clipboard offer has no supported text, image, rich-text or file-reference MIME types".into()));
        }
        let mut streams = Vec::new();
        for mime in types.iter().filter(|m| supported(m)).take(32) {
            if let Ok((reader, writer)) = UnixStream::pair() {
                let _ = reader.set_read_timeout(Some(Duration::from_secs(2)));
                offer.receive_mime(mime.clone(), writer.as_fd());
                streams.push((mime.clone(), reader));
            }
        }
        let _ = conn.flush();
        offer.destroy_offer();
        let tx = self.tx.clone();
        let in_flight = self.in_flight.clone();
        std::thread::spawn(move || {
            struct Transfer(Arc<AtomicUsize>);
            impl Drop for Transfer {
                fn drop(&mut self) {
                    self.0.fetch_sub(1, Ordering::SeqCst);
                }
            }
            let _transfer = Transfer(in_flight);
            let mut formats = BTreeMap::new();
            let mut remaining = MAX_BUNDLE_BYTES;
            for (mime, reader) in streams {
                let mut data = Vec::new();
                match reader.take(remaining as u64 + 1).read_to_end(&mut data) {
                    Ok(_) if data.len() <= remaining => {
                        remaining -= data.len();
                        formats.insert(mime, data);
                    }
                    _ => {
                        let _ = tx.try_send(Event::Diagnostic(
                            "Clipboard offer exceeded size limit or timed out; skipped.".into(),
                        ));
                        return;
                    }
                }
            }
            let bundle = Bundle { formats };
            if bundle.validate().is_ok() && super::token_current(epoch) {
                let _ = tx.try_send(Event::Captured {
                    bundle,
                    epoch,
                    source: None,
                });
            }
        });
    }
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
                "zwlr_data_control_manager_v1" => {
                    state.manager = Some(registry.bind(name, version.min(2), qh, ()))
                }
                "ext_data_control_manager_v1" => {
                    state.ext_manager = Some(registry.bind(name, version.min(1), qh, ()))
                }
                "wl_seat" if state.seat.is_none() => {
                    state.seat = Some(registry.bind(name, version.min(7), qh, ()))
                }
                _ => {}
            }
        }
    }
}
impl Dispatch<wl_seat::WlSeat, ()> for State {
    fn event(
        _: &mut Self,
        _: &wl_seat::WlSeat,
        _: wl_seat::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
impl Dispatch<offer::ZwlrDataControlOfferV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &offer::ZwlrDataControlOfferV1,
        event: offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let offer::Event::Offer { mime_type } = event {
            state.offers.entry(proxy.id()).or_default().push(mime_type);
        }
    }
}

impl Dispatch<ext_offer::ExtDataControlOfferV1, ()> for State {
    fn event(
        state: &mut Self,
        proxy: &ext_offer::ExtDataControlOfferV1,
        event: ext_offer::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let ext_offer::Event::Offer { mime_type } = event {
            state.offers.entry(proxy.id()).or_default().push(mime_type);
        }
    }
}
impl Dispatch<device::ZwlrDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &device::ZwlrDataControlDeviceV1,
        event: device::Event,
        _: &(),
        conn: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            device::Event::Selection { id: Some(offer) } => state.capture_offer(offer, conn),
            device::Event::Selection { id: None } => {}
            device::Event::PrimarySelection { id: Some(offer) } => {
                state.offers.remove(&offer.id());
                offer.destroy();
            }
            device::Event::Finished => {
                state.finished = true;
            }
            _ => {}
        }
    }
    wayland_client::event_created_child!(State,device::ZwlrDataControlDeviceV1,[0=>(offer::ZwlrDataControlOfferV1,())]);
}

impl Dispatch<ext_device::ExtDataControlDeviceV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &ext_device::ExtDataControlDeviceV1,
        event: ext_device::Event,
        _: &(),
        conn: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            ext_device::Event::DataOffer { id } => state.remember_offer(id),
            ext_device::Event::Selection { id: Some(offer) } => state.capture_offer(offer, conn),
            ext_device::Event::Selection { id: None } => {}
            ext_device::Event::PrimarySelection { id: Some(offer) } => {
                state.offers.remove(&offer.id());
                offer.destroy();
            }
            ext_device::Event::PrimarySelection { id: None } => {}
            ext_device::Event::Finished => {
                state.finished = true;
            }
            _ => {}
        }
    }
    wayland_client::event_created_child!(State, ext_device::ExtDataControlDeviceV1, [0 => (ext_offer::ExtDataControlOfferV1, ())]);
}
impl Dispatch<manager::ZwlrDataControlManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &manager::ZwlrDataControlManagerV1,
        _: manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<ext_manager::ExtDataControlManagerV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &ext_manager::ExtDataControlManagerV1,
        _: ext_manager::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
fn supported(mime: &str) -> bool {
    matches!(
        mime,
        "text/plain"
            | "text/plain;charset=utf-8"
            | "text/html"
            | "text/rtf"
            | "application/rtf"
            | "text/uri-list"
            | "image/png"
            | "image/jpeg"
            | "image/bmp"
            | "x-special/gnome-copied-files"
    )
}
static LAST_OWNER: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);
pub fn verify_owned() -> Result<()> {
    use wl_clipboard_rs::paste::{ClipboardType, Seat, get_mime_types};
    let marker = LAST_OWNER
        .lock()
        .unwrap()
        .clone()
        .context("No restored clipboard offer")?;
    if !get_mime_types(ClipboardType::Regular, Seat::Unspecified)?.contains(&marker) {
        bail!("Clipboard ownership changed before paste; select the entry and try again");
    }
    Ok(())
}
pub fn write(bundle: &Bundle) -> Result<()> {
    use wl_clipboard_rs::copy::{MimeSource, MimeType, Options, Source};
    if bundle.formats.is_empty() {
        bail!("empty clipboard bundle");
    }
    let mut sources: Vec<_> = bundle
        .formats
        .iter()
        .map(|(mime, data)| MimeSource {
            mime_type: MimeType::Specific(mime.clone()),
            source: Source::Bytes(data.clone().into_boxed_slice()),
        })
        .collect();
    // An opaque offer marker confirms publication without reading the content back.
    // It is excluded by our capture MIME allowlist and never stored in history.
    let marker = super::ownership::marker();
    sources.push(MimeSource {
        mime_type: MimeType::Specific(marker.clone()),
        source: Source::Bytes(Box::new([])),
    });
    *LAST_OWNER.lock().unwrap() = None;
    Options::new()
        .copy_multi(sources)
        .context("publish Wayland clipboard formats")?;
    *LAST_OWNER.lock().unwrap() = Some(marker);
    let deadline = std::time::Instant::now() + Duration::from_millis(500);
    loop {
        match verify_owned() {
            Ok(()) => return Ok(()),
            Err(error) if std::time::Instant::now() >= deadline => {
                return Err(error).context("Restored clipboard offer was not published");
            }
            Err(_) => std::thread::sleep(Duration::from_millis(5)),
        }
    }
}

pub fn serve(bundle: &Bundle) -> Result<()> {
    use wl_clipboard_rs::copy::{MimeSource, MimeType, Options, Source};
    let sources = bundle
        .formats
        .iter()
        .map(|(mime, data)| MimeSource {
            mime_type: MimeType::Specific(mime.clone()),
            source: Source::Bytes(data.clone().into_boxed_slice()),
        })
        .collect();
    let mut options = Options::new();
    options.foreground(true);
    let prepared = options
        .prepare_copy_multi(sources)
        .context("claim Wayland clipboard")?;
    use std::io::Write;
    println!("ready");
    std::io::stdout().flush()?;
    prepared.serve().context("serve Wayland clipboard")
}

pub fn current_text() -> Result<String> {
    use std::os::fd::AsRawFd;
    use wl_clipboard_rs::paste::{ClipboardType, MimeType, Seat, get_contents};
    let (mut pipe, _) = get_contents(ClipboardType::Regular, Seat::Unspecified, MimeType::Text)?;
    let fd = pipe.as_raw_fd();
    let deadline = std::time::Instant::now() + Duration::from_secs(2);
    let mut data = Vec::new();
    let mut buffer = [0u8; 8192];
    loop {
        let remaining = deadline
            .saturating_duration_since(std::time::Instant::now())
            .as_millis()
            .min(2000) as i32;
        if remaining == 0 {
            bail!("Clipboard text read timed out")
        }
        let mut poll = libc::pollfd {
            fd,
            events: libc::POLLIN,
            revents: 0,
        };
        let result = unsafe { libc::poll(&mut poll, 1, remaining) };
        if result <= 0 {
            bail!("Clipboard text source did not respond")
        }
        let count = pipe.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        if data.len() + count > 1024 * 1024 {
            bail!("Clipboard placeholder exceeds 1 MiB limit")
        }
        data.extend_from_slice(&buffer[..count]);
    }
    Ok(String::from_utf8(data)?)
}
