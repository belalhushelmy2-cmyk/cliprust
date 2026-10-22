//! Opt-in KWin scripting bridge. The script must be explicitly enabled by the user.
use super::Target;
use anyhow::{Context, Result, bail};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

struct Command {
    id: u64,
    action: String,
    target: u64,
    expires: Instant,
    reply: crossbeam_channel::Sender<(bool, u64, String)>,
}
struct Bridge {
    receiver: tokio::sync::Mutex<mpsc::Receiver<Command>>,
    pending: Arc<Mutex<Option<Command>>>,
}
static COMMANDS: OnceLock<mpsc::Sender<Command>> = OnceLock::new();
static REQUEST_LOCK: Mutex<()> = Mutex::new(());
static SEQUENCE: AtomicU64 = AtomicU64::new(1);
pub fn is_kde() -> bool {
    super::is_wayland()
        && std::env::var("XDG_CURRENT_DESKTOP")
            .unwrap_or_default()
            .split(':')
            .any(|s| s.eq_ignore_ascii_case("KDE"))
}
#[zbus::interface(name = "org.cliprust.KWin")]
impl Bridge {
    async fn next_request(&self) -> (String, String, String, String) {
        let mut receiver = self.receiver.lock().await;
        while let Ok(Some(command)) =
            tokio::time::timeout(Duration::from_secs(5), receiver.recv()).await
        {
            if command.expires <= Instant::now() {
                continue;
            }
            let result = (
                command.id.to_string(),
                command.action.clone(),
                command.target.to_string(),
                (std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis()
                    + command
                        .expires
                        .saturating_duration_since(Instant::now())
                        .as_millis())
                .to_string(),
            );
            *self.pending.lock().unwrap() = Some(command);
            return result;
        }
        ("0".into(), "idle".into(), "0".into(), "0".into())
    }
    fn report(&self, id: String, success: bool, target: String, message: String) {
        let (Ok(id), Ok(target)) = (id.parse::<u64>(), target.parse::<u64>()) else {
            return;
        };
        let mut pending = self.pending.lock().unwrap();
        if pending.as_ref().is_some_and(|c| c.id == id) {
            let command = pending.take().unwrap();
            if command.expires > Instant::now() {
                let _ = command.reply.send((success, target, message));
            }
        }
    }
}
pub fn start() -> Result<()> {
    if !is_kde() || COMMANDS.get().is_some() {
        return Ok(());
    }
    let (sender, receiver) = mpsc::channel(8);
    let (ready, result) = crossbeam_channel::bounded(1);
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build();
        let result = (|| -> Result<()> {
            runtime?.block_on(async move {
                let bridge = Bridge {
                    receiver: tokio::sync::Mutex::new(receiver),
                    pending: Arc::new(Mutex::new(None)),
                };
                let connection = zbus::connection::Builder::session()?
                    .name("org.cliprust.KWin")?
                    .serve_at("/org/cliprust/KWin", bridge)?
                    .build()
                    .await?;
                let _ = ready.send(());
                let _connection = connection;
                std::future::pending::<()>().await;
                Ok(())
            })
        })();
        if let Err(error) = result {
            eprintln!("Cliprust KWin bridge: {error:#}");
        }
    });
    result
        .recv_timeout(Duration::from_secs(2))
        .context("Start KWin bridge; another Cliprust process may own it")?;
    let _ = COMMANDS.set(sender);
    Ok(())
}
pub fn request(action: &str, target: Target) -> Result<Target> {
    let _serial = REQUEST_LOCK.lock().unwrap();
    let sender = COMMANDS.get().context("KWin integration is not running")?;
    let (reply, result) = crossbeam_channel::bounded(1);
    sender
        .try_send(Command {
            id: SEQUENCE.fetch_add(1, Ordering::Relaxed),
            action: action.into(),
            target: target.0,
            expires: Instant::now() + Duration::from_millis(800),
            reply,
        })
        .context("KWin integration request queue is full")?;
    let (success, target, message) = result.recv_timeout(Duration::from_millis(850))
        .context("KWin integration did not respond. Install and enable packaging/kde/cliprust in KWin Scripts; see KDE_INTEGRATION.md")?;
    if !success {
        bail!("KWin: {message}");
    }
    Ok(Target(target))
}

/// Read-only live KDE conflict query. Qt encodes modifiers in the high bits.
pub async fn shortcut_conflict(shortcut: &crate::shortcuts::Shortcut) -> Result<Option<String>> {
    let connection = zbus::Connection::session().await?;
    let proxy = zbus::Proxy::new(
        &connection,
        "org.kde.kglobalaccel",
        "/kglobalaccel",
        "org.kde.KGlobalAccel",
    )
    .await?;
    let key = shortcut.key.as_bytes()[0].to_ascii_uppercase() as i32
        | if shortcut.shift { 0x02000000 } else { 0 }
        | if shortcut.ctrl { 0x04000000 } else { 0 }
        | if shortcut.alt { 0x08000000 } else { 0 }
        | if shortcut.meta { 0x10000000 } else { 0 };
    // Do not unregister, reassign or steal any existing shortcut.
    let available: bool = proxy.call("isGlobalShortcutAvailable", &(key, "")).await?;
    if available {
        return Ok(None);
    }
    // KDE's single-key compatibility query reports component and action names.
    let owner: Vec<String> = proxy.call("action", &(key,)).await.unwrap_or_default();
    let owner = owner
        .into_iter()
        .map(|part| {
            part.chars()
                .filter(|c| !c.is_control())
                .take(128)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(" / ");
    Ok(Some(if owner.is_empty() {
        "another KDE action (owner unavailable)".into()
    } else {
        owner
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bridge_ignores_stale_and_mismatched_reports() {
        let (_, receiver) = mpsc::channel(1);
        let (reply, result) = crossbeam_channel::bounded(1);
        let bridge = Bridge {
            receiver: tokio::sync::Mutex::new(receiver),
            pending: Arc::new(Mutex::new(Some(Command {
                id: 3,
                action: "restore".into(),
                target: 7,
                expires: Instant::now() + Duration::from_secs(1),
                reply,
            }))),
        };
        bridge.report("2".into(), true, "7".into(), "".into());
        assert!(result.try_recv().is_err());
        bridge.report("3".into(), false, "7".into(), "Focus refused".into());
        assert_eq!(result.recv().unwrap(), (false, 7, "Focus refused".into()));
        let (reply, result) = crossbeam_channel::bounded(1);
        *bridge.pending.lock().unwrap() = Some(Command {
            id: 4,
            action: "restore".into(),
            target: 7,
            expires: Instant::now() - Duration::from_secs(1),
            reply,
        });
        bridge.report("4".into(), true, "7".into(), "".into());
        assert!(result.try_recv().is_err());
    }
}
