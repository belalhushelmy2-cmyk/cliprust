use crate::model::{Bundle, confidential};
use anyhow::{Result, anyhow, bail};
use clipboard_rs::{
    Clipboard, ClipboardContent, ClipboardContext, ClipboardHandler, ClipboardWatcher,
    ClipboardWatcherContext, common::RustImage,
};
use crossbeam_channel::Sender as SyncSender;
use std::collections::BTreeMap;
#[cfg(all(target_os = "linux", feature = "desktop"))]
pub mod activation;
mod autostart;
pub mod files;
mod ownership;
mod recovery;
mod theme;
pub use recovery::watch_resilient;
#[cfg(target_os = "linux")]
pub mod kde;
#[cfg(target_os = "linux")]
pub mod sway;
#[cfg(target_os = "linux")]
mod sway_input;
#[cfg(target_os = "linux")]
mod wayland;
#[cfg(target_os = "linux")]
mod wayland_input;
#[cfg(target_os = "linux")]
mod wayland_shortcuts;
#[cfg(windows)]
mod windows;
#[cfg(target_os = "linux")]
mod x11;

#[derive(Debug)]
pub enum Event {
    Shortcut(Target),
    Activate(String),
    Captured {
        bundle: Bundle,
        epoch: u64,
        source: Option<String>,
    },
    SystemTheme(bool),
    Diagnostic(String),
}
static CAPTURE_PAUSED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
static CAPTURE_EPOCH: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
pub fn set_capture_paused(paused: bool) {
    use std::sync::atomic::Ordering;
    invalidate_capture_epoch();
    CAPTURE_PAUSED.store(paused, Ordering::SeqCst);
}
fn invalidate_capture_epoch() {
    CAPTURE_EPOCH.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
}
pub fn capture_token() -> Option<u64> {
    use std::sync::atomic::Ordering;
    if CAPTURE_PAUSED.load(Ordering::SeqCst) {
        None
    } else {
        Some(CAPTURE_EPOCH.load(Ordering::SeqCst))
    }
}
pub fn token_current(epoch: u64) -> bool {
    capture_token() == Some(epoch)
}
pub fn is_wayland() -> bool {
    std::env::var_os("WAYLAND_DISPLAY").is_some()
        || std::env::var("XDG_SESSION_TYPE").is_ok_and(|s| s == "wayland")
}
pub fn autostart_enabled() -> Result<bool> {
    autostart::enabled()
}
pub fn set_autostart(enabled: bool) -> Result<()> {
    autostart::set(enabled)
}
pub fn system_uses_light_theme() -> Result<bool> {
    theme::system_uses_light()
}
pub fn watch_system_theme(tx: SyncSender<Event>) -> Result<()> {
    theme::watch(tx)
}
fn source_application() -> Option<String> {
    #[cfg(windows)]
    return windows::source_application();
    #[cfg(target_os = "linux")]
    if !is_wayland() {
        return x11::source_application().ok().flatten();
    }
    #[allow(unreachable_code)]
    None
}
fn normalized_source_application(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() || name.chars().any(char::is_control) {
        return None;
    }
    Some(name.chars().take(128).collect())
}
#[cfg(test)]
mod source_tests {
    use super::normalized_source_application;

    #[test]
    fn source_application_names_are_bounded_and_reject_controls() {
        assert_eq!(
            normalized_source_application("  Firefox  ").as_deref(),
            Some("Firefox")
        );
        assert_eq!(normalized_source_application("bad\nname"), None);
        assert_eq!(normalized_source_application(" "), None);
        assert_eq!(
            normalized_source_application(&"a".repeat(200))
                .unwrap()
                .len(),
            128
        );
    }
}
pub fn watch(tx: SyncSender<Event>) -> Result<()> {
    #[cfg(target_os = "linux")]
    if is_wayland() {
        return wayland::watch(tx);
    }
    struct Handler {
        ctx: ClipboardContext,
        tx: SyncSender<Event>,
    }
    impl ClipboardHandler for Handler {
        fn on_clipboard_change(&mut self) {
            let Some(epoch) = capture_token() else {
                return;
            };
            let source_before = source_application();
            match read_native(&self.ctx) {
                Ok(Some(bundle)) => {
                    if token_current(epoch) {
                        let source = source_application()
                            .filter(|name| source_before.as_ref() == Some(name));
                        let _ = self.tx.try_send(Event::Captured {
                            bundle,
                            epoch,
                            source,
                        });
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    let _ = self
                        .tx
                        .try_send(Event::Diagnostic(format!("Clipboard read: {e}")));
                }
            }
        }
    }
    let ctx = ClipboardContext::new().map_err(|e| anyhow!("{e}"))?;
    let mut watcher = ClipboardWatcherContext::new().map_err(|e| anyhow!("{e}"))?;
    watcher.add_handler(Handler {
        ctx,
        tx: tx.clone(),
    });
    let _ = tx.try_send(Event::Diagnostic("Clipboard monitoring connected".into()));
    watcher.start_watch();
    Ok(())
}
fn read_native(ctx: &ClipboardContext) -> Result<Option<Bundle>> {
    let formats = ctx.available_formats().map_err(|e| anyhow!("{e}"))?;
    if confidential(&formats) || ownership::is_own_offer(&formats) {
        return Ok(None);
    }
    let mut data = BTreeMap::new();
    if let Ok(text) = ctx.get_text() {
        data.insert("text/plain".into(), text.into_bytes());
    }
    if let Ok(text) = ctx.get_html() {
        data.insert("text/html".into(), text.into_bytes());
    }
    if let Ok(text) = ctx.get_rich_text() {
        data.insert("text/rtf".into(), text.into_bytes());
    }
    if let Ok(image) = ctx.get_image()
        && let Ok(png) = image.to_png()
    {
        data.insert("image/png".into(), png.get_bytes().to_vec());
    }
    if let Ok(files) = ctx.get_files()
        && !files.is_empty()
    {
        data.insert("text/uri-list".into(), files.join("\r\n").into_bytes());
    }
    if data.is_empty() {
        return Ok(None);
    }
    let bundle = Bundle { formats: data };
    bundle.validate()?;
    Ok(Some(bundle))
}
pub struct Writer {
    native: Option<ClipboardContext>,
}
impl Writer {
    pub fn new() -> Result<Self> {
        if is_wayland() {
            return Ok(Self { native: None });
        }
        Ok(Self {
            native: Some(ClipboardContext::new().map_err(|e| anyhow!("{e}"))?),
        })
    }
    pub fn write(&self, bundle: &Bundle) -> Result<()> {
        bundle.validate()?;
        #[cfg(target_os = "linux")]
        if is_wayland() {
            return wayland::write(bundle);
        }
        let ctx = self
            .native
            .as_ref()
            .ok_or_else(|| anyhow!("clipboard unavailable"))?;
        let mut contents = Vec::new();
        for (mime, bytes) in &bundle.formats {
            let string = || String::from_utf8(bytes.clone());
            contents.push(match mime.as_str() {
                "text/plain" => ClipboardContent::Text(string()?),
                "text/html" => ClipboardContent::Html(string()?),
                "text/rtf" => ClipboardContent::Rtf(string()?),
                "image/png" => ClipboardContent::Image(
                    clipboard_rs::RustImageData::from_bytes(bytes).map_err(|e| anyhow!("{e}"))?,
                ),
                "text/uri-list" => ClipboardContent::Files(
                    string()?
                        .lines()
                        .filter(|s| !s.is_empty() && !s.starts_with('#'))
                        .map(str::to_owned)
                        .collect(),
                ),
                _ => ClipboardContent::Other(mime.clone(), bytes.clone()),
            });
        }
        contents.push(ClipboardContent::Other(ownership::marker(), vec![1]));
        ctx.set(contents).map_err(|e| anyhow!("{e}"))
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Target(pub u64);
pub fn target() -> Result<Target> {
    #[cfg(target_os = "linux")]
    if sway::available() {
        return sway::target();
    }
    if is_wayland() {
        return Ok(Target::default());
    }
    #[cfg(target_os = "linux")]
    {
        x11::target()
    }
    #[cfg(windows)]
    {
        windows::target()
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        bail!("unsupported platform")
    }
}
/// UI capability hint only; revalidate the bridge and target when the action runs.
pub fn paste_supported(target: Target) -> bool {
    if target.0 == 0 {
        return false;
    }
    #[cfg(target_os = "linux")]
    if sway::available() || kde::is_kde() {
        return true;
    }
    !is_wayland()
}
pub fn prepare_paste(target: Target) -> Result<()> {
    can_paste(target)?;
    #[cfg(target_os = "linux")]
    if kde::is_kde() {
        wayland_input::prepare()?;
    }
    Ok(())
}
pub fn can_paste(target: Target) -> Result<()> {
    #[cfg(target_os = "linux")]
    if sway::available() {
        return sway::can_paste(target);
    }
    #[cfg(target_os = "linux")]
    if kde::is_kde() {
        kde::request("check", target)?;
        return Ok(());
    }
    if is_wayland() {
        bail!(
            "Automatic paste is unavailable on Wayland: no granted input/focus integration. Use Copy, focus your application, and paste."
        );
    }
    if target.0 == 0 {
        bail!("No previous application recorded; reopen Cliprust from the target application.")
    }
    Ok(())
}
pub fn paste(target: Target) -> Result<()> {
    can_paste(target)?;
    #[cfg(target_os = "linux")]
    if sway::available() {
        return sway::paste(target);
    }
    #[cfg(target_os = "linux")]
    if kde::is_kde() {
        return wayland_input::paste(target);
    }
    #[cfg(target_os = "linux")]
    {
        x11::paste(target)
    }
    #[cfg(windows)]
    {
        windows::paste(target)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        bail!("unsupported platform")
    }
}

pub fn open_external(value: &str) -> Result<()> {
    #[cfg(target_os = "linux")]
    {
        std::process::Command::new("xdg-open").arg(value).spawn()?;
        Ok(())
    }
    #[cfg(windows)]
    {
        windows::open_external(value)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = value;
        bail!("Opening external resources is unavailable")
    }
}

/// Short-lived CLI commands delegate clipboard ownership to a child that lives
/// only until the compositor replaces the offer. The GUI owns its own offers.
pub fn copy_from_cli(bundle: &Bundle) -> Result<()> {
    #[cfg(target_os = "linux")]
    if is_wayland() {
        use std::io::{BufRead, Write};
        let mut command = std::process::Command::new(std::env::current_exe()?);
        use std::os::unix::process::CommandExt;
        command.process_group(0);
        let mut child = command
            .arg("serve-bundle")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .spawn()?;
        serde_json::to_writer(
            child
                .stdin
                .as_mut()
                .ok_or_else(|| anyhow!("clipboard helper stdin unavailable"))?,
            bundle,
        )?;
        child.stdin.take().unwrap().flush()?;
        let mut ready = String::new();
        std::io::BufReader::new(
            child
                .stdout
                .take()
                .ok_or_else(|| anyhow!("clipboard helper stdout unavailable"))?,
        )
        .read_line(&mut ready)?;
        if ready.trim() != "ready" {
            let _ = child.wait();
            bail!("Wayland clipboard owner failed to initialize")
        }
        return Ok(());
    }
    let writer = Writer::new()?;
    writer.write(bundle)?;
    #[cfg(target_os = "linux")]
    {
        eprintln!("Serving X11 clipboard; leave this command running until finished pasting.");
        loop {
            std::thread::park();
        }
    }
    #[cfg(not(target_os = "linux"))]
    Ok(())
}
pub fn serve_from_stdin() -> Result<()> {
    use std::io::Read;
    let mut bytes = Vec::new();
    std::io::stdin()
        .take((crate::model::MAX_BUNDLE_BYTES as u64) * 5 + 1)
        .read_to_end(&mut bytes)?;
    let bundle: Bundle = serde_json::from_slice(&bytes)?;
    bundle.validate()?;
    #[cfg(target_os = "linux")]
    if is_wayland() {
        return wayland::serve(&bundle);
    }
    bail!("clipboard ownership helper is only used on Wayland")
}

pub fn hotkey(
    tx: crossbeam_channel::Sender<Event>,
    shortcut: crate::shortcuts::Shortcut,
) -> Result<()> {
    #[cfg(target_os = "linux")]
    if sway::available() {
        let _ = tx.send(Event::Diagnostic(
            "Sway shortcut: bind Meta+V to cliprust --toggle in your Sway configuration".into(),
        ));
        return Ok(());
    }
    if is_wayland() {
        #[cfg(target_os = "linux")]
        return wayland_shortcuts::hotkey(tx, shortcut);
    }
    #[cfg(target_os = "linux")]
    {
        x11::hotkey(tx, shortcut)
    }
    #[cfg(windows)]
    {
        windows::hotkey(tx, shortcut)
    }
    #[cfg(not(any(target_os = "linux", windows)))]
    {
        let _ = tx;
        bail!("global shortcuts unavailable")
    }
}

pub fn current_text() -> Result<String> {
    #[cfg(target_os = "linux")]
    if is_wayland() {
        return wayland::current_text();
    }
    let context = ClipboardContext::new().map_err(|e| anyhow!("{e}"))?;
    context.get_text().map_err(|e| anyhow!("{e}"))
}
