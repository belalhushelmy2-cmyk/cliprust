//! Keyboard-only portal session, created only when the user invokes Paste.
//! KWin must confirm the remembered destination before any input is sent.
use anyhow::{Context, Result, bail};
use ashpd::desktop::{
    Session,
    remote_desktop::{DeviceType, KeyState, RemoteDesktop, SelectDevicesOptions},
};
use std::sync::{Mutex, OnceLock};
struct Input {
    runtime: tokio::runtime::Runtime,
    portal: RemoteDesktop,
    session: Session<RemoteDesktop>,
}
static INPUT: OnceLock<Mutex<Option<Input>>> = OnceLock::new();
fn state() -> &'static Mutex<Option<Input>> {
    INPUT.get_or_init(|| Mutex::new(None))
}
pub fn prepare() -> Result<()> {
    let mut state = state().lock().unwrap();
    if state.is_some() {
        return Ok(());
    }
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    eprintln!(
        "Cliprust: requesting RemoteDesktop keyboard permission; no screen capture requested"
    );
    let (portal, session) = runtime.block_on(async {
        let portal = RemoteDesktop::new()
            .await
            .context("RemoteDesktop portal unavailable")?;
        let session = portal.create_session(Default::default()).await?;
        portal
            .select_devices(
                &session,
                SelectDevicesOptions::default().set_devices(Some(DeviceType::Keyboard.into())),
            )
            .await?
            .response()?;
        let response = portal
            .start(&session, None, Default::default())
            .await?
            .response()
            .context("Keyboard permission denied or cancelled")?;
        if !response.devices().contains(DeviceType::Keyboard) {
            bail!("Desktop did not grant keyboard permission");
        }
        Ok::<_, anyhow::Error>((portal, session))
    })?;
    *state = Some(Input {
        runtime,
        portal,
        session,
    });
    Ok(())
}
pub fn paste(target: super::Target) -> Result<()> {
    let mut state = state().lock().unwrap();
    let input = state
        .as_ref()
        .context("Keyboard permission has not been granted")?;
    super::kde::request("restore", target)?;
    // Let the compositor deliver the focus change before checking it again.
    std::thread::sleep(std::time::Duration::from_millis(40));
    super::kde::request("verify", target)?;
    let result = input.runtime.block_on(async {
        let send = |key, state| {
            input
                .portal
                .notify_keyboard_keysym(&input.session, key, state, Default::default())
        };
        let pressed_ctrl = send(0xffe3, KeyState::Pressed).await; // Control_L
        let pressed = if pressed_ctrl.is_ok() {
            send(0x76, KeyState::Pressed).await
        } else {
            Ok(())
        };
        // Always attempt both releases, including after a partial failure.
        let released_v = send(0x76, KeyState::Released).await;
        let released_ctrl = send(0xffe3, KeyState::Released).await;
        pressed_ctrl?;
        pressed?;
        released_v?;
        released_ctrl?;
        Ok::<_, ashpd::Error>(())
    });
    if let Err(error) = result {
        *state = None; // A revoked/closed session must request consent again.
        return Err(error).context(
            "Paste input request failed; the destination may have received partial input",
        );
    }
    Ok(()) // Input submitted, not proof that the destination consumed clipboard data.
}
