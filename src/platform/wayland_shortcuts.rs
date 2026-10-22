use super::{Event, Target};
use anyhow::{Context, Result, bail};
use ashpd::desktop::global_shortcuts::{GlobalShortcuts, NewShortcut};
use futures_util::StreamExt;
use std::sync::atomic::{AtomicU32, Ordering};
static CONFIGURE: tokio::sync::Notify = tokio::sync::Notify::const_new();
static VERSION: AtomicU32 = AtomicU32::new(0);
pub fn configure() -> Result<()> {
    match VERSION.load(Ordering::Acquire) {
        0 => bail!(
            "Desktop Global Shortcuts portal unavailable. Set a custom desktop shortcut to cliprust --toggle."
        ),
        1 => bail!(
            "This desktop's shortcut portal cannot open shortcut settings. Change the launcher in desktop keyboard settings; paste shortcuts apply immediately."
        ),
        _ => {
            CONFIGURE.notify_one();
            Ok(())
        }
    }
}
struct Registration;
impl Drop for Registration {
    fn drop(&mut self) {
        VERSION.store(0, Ordering::Release);
    }
}

/// Registers the launcher shortcut through the user's desktop portal. The
/// compositor decides whether to accept the preferred trigger and may present
/// its own shortcut-selection UI.
pub fn hotkey(
    tx: crossbeam_channel::Sender<Event>,
    shortcut: crate::shortcuts::Shortcut,
) -> Result<()> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .context("start Wayland shortcut portal runtime")?;
    runtime.block_on(async move {
        if super::kde::is_kde() {
            match tokio::time::timeout(std::time::Duration::from_secs(2), super::kde::shortcut_conflict(&shortcut)).await {
                Ok(Ok(Some(owner))) => {
                    let _ = tx.try_send(Event::Diagnostic(format!("Preferred shortcut is registered to {owner}. Choose another trigger in the portal; no existing binding has been changed.")));
                }
                Ok(Ok(None)) => {}
                _ => { let _ = tx.try_send(Event::Diagnostic("Could not query KDE shortcut ownership; the desktop portal will handle conflict resolution.".into())); }
            }
        }
        let portal = GlobalShortcuts::new()
            .await
            .context("connect to the Global Shortcuts portal")?;
        let session = portal
            .create_session(Default::default())
            .await
            .context("create a desktop shortcut session")?;
        let mut activated = portal
            .receive_activated()
            .await
            .context("listen for desktop shortcut activations")?;
        let trigger = shortcut.portal_trigger();
        let shortcut =
            NewShortcut::new("cliprust-toggle-history", "Show clipboard history")
                .preferred_trigger(Some(trigger.as_str()));
        let _ = tx.try_send(Event::Diagnostic("Desktop shortcut permission requested. Choose an available launcher shortcut in your desktop dialog; existing bindings will not be replaced.".into()));
        let bound = portal
            .bind_shortcuts(&session, &[shortcut], None, Default::default())
            .await
            .context("request the configured desktop shortcut")?
            .response()
            .context("desktop declined the clipboard shortcut")?;

        let registered = bound.shortcuts().iter().find(|s| s.id() == "cliprust-toggle-history")
            .context("Portal did not register Show clipboard history; choose a free shortcut in desktop settings")?;
        let _ = tx.try_send(Event::Diagnostic(format!("Clipboard shortcut registered: {}", registered.trigger_description())));
        // Scope signals to this session; other apps can use the same shortcut ID.
        let _registration = Registration;
        VERSION.store(portal.version(), Ordering::Release);
        let session_path = serde_json::to_value(&session)?.as_str().context("Invalid portal session path")?.to_owned();
        loop {
            let event = match futures_util::future::select(
                Box::pin(activated.next()), Box::pin(CONFIGURE.notified())
            ).await {
                futures_util::future::Either::Left((Some(event), _)) => event,
                futures_util::future::Either::Left((None, _)) => break,
                futures_util::future::Either::Right(_) => {
                    if let Err(error)=portal.configure_shortcuts(&session, None, Default::default()).await {
                        let _=tx.try_send(Event::Diagnostic(format!("Desktop shortcut settings failed: {error}")));
                    }
                    continue;
                }
            };
            if event.session_handle().as_str() != session_path || event.shortcut_id() != "cliprust-toggle-history" {
                continue;
            }
            let token = event.options().get("activation_token")
                .and_then(|v| <&str>::try_from(v).ok()).filter(|s| !s.is_empty());
            let request = if let Some(token) = token {
                Event::Activate(token.to_owned())
            } else {
                let _ = tx.try_send(Event::Diagnostic("Shortcut portal supplied no activation token; the compositor may refuse focus. Use a desktop command binding to cliprust --show with startup notification.".into()));
                Event::Shortcut(Target(0))
            };
            if tx.send(request).is_err() {
                bail!("clipboard window event receiver stopped")
            }
        }
        bail!("Desktop shortcut session ended; restart Cliprust to register again")
    })
}
