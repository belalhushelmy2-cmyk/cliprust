//! Sway IPC placement and focus verification, followed by native or optional wtype Ctrl+V.
use super::Target;
use anyhow::{Context, Result, bail};
use serde_json::Value;
use std::{
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::{Command, Stdio},
    sync::Mutex,
    time::{Duration, Instant},
};
#[derive(Clone)]
struct ContextState {
    workspace: String,
    output: String,
    target: u64,
    pid: u64,
}
static STATE: Mutex<Option<ContextState>> = Mutex::new(None);
#[derive(Clone, Copy, Debug, PartialEq)]
struct Rect {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}
impl Rect {
    fn from_json(value: &Value) -> Option<Self> {
        Some(Self {
            x: value["x"].as_i64()?,
            y: value["y"].as_i64()?,
            width: value["width"].as_i64()?,
            height: value["height"].as_i64()?,
        })
    }
}
#[derive(Clone)]
struct SavedWindow {
    output: String,
    rect: Rect,
    bounds: Rect,
}
static GEOMETRY: Mutex<Option<SavedWindow>> = Mutex::new(None);
fn placement(bounds: Rect, saved: Option<&SavedWindow>, output: &str) -> Rect {
    // Leave a small margin around the popup and clamp remembered geometry to the live output.
    let width = saved
        .map_or(780, |s| s.rect.width)
        .clamp(1, (bounds.width - 24).max(1));
    let height = saved
        .map_or(540, |s| s.rect.height)
        .clamp(1, (bounds.height - 24).max(1));
    let (x, y) = match saved.filter(|s| s.output == output) {
        Some(s) => (
            bounds.x + s.rect.x - s.bounds.x,
            bounds.y + s.rect.y - s.bounds.y,
        ),
        None => (
            bounds.x + (bounds.width - width) / 2,
            bounds.y + (bounds.height - height) / 2,
        ),
    };
    Rect {
        x: x.clamp(bounds.x, bounds.x + bounds.width - width),
        y: y.clamp(bounds.y, bounds.y + bounds.height - height),
        width,
        height,
    }
}
fn window(tree: &Value) -> Result<&Value> {
    visit(tree, &|n| {
        n["pid"].as_u64() == Some(std::process::id().into())
    })
    .context("Cliprust surface is not mapped yet")
}
pub fn remember_geometry() -> Result<()> {
    let tree = ipc(4, "")?;
    let Some(node) = visit(&tree, &|n| {
        n["pid"].as_u64() == Some(std::process::id().into())
    }) else {
        return Ok(());
    };
    let workspace = visit(&tree, &|n| {
        n["type"] == "workspace" && visit(n, &|child| child["id"] == node["id"]).is_some()
    })
    .context("Cliprust workspace unavailable")?;
    let workspaces = ipc(1, "")?;
    let workspace = workspaces
        .as_array()
        .and_then(|all| all.iter().find(|w| w["name"] == workspace["name"]))
        .context("Cliprust workspace was removed")?;
    *GEOMETRY.lock().unwrap() = Some(SavedWindow {
        output: workspace["output"]
            .as_str()
            .context("Output name missing")?
            .into(),
        rect: Rect::from_json(&node["rect"]).context("Window geometry missing")?,
        bounds: Rect::from_json(&workspace["rect"]).context("Workspace geometry missing")?,
    });
    Ok(())
}
/// Restore keyboard focus after Keep Open without resizing, moving or recentering the popup.
pub fn refocus() -> Result<()> {
    let tree = ipc(4, "")?;
    let id = window(&tree)?["id"]
        .as_u64()
        .context("Cliprust container ID missing")?;
    command(&format!("[con_id={id}] focus"))
}
/// Float only Cliprust at map time, before Sway can retile the destination app.
/// This is a session rule; it does not edit or reload the user's configuration.
pub fn configure_popup() -> Result<()> {
    command(r#"for_window [app_id="^(org[.]cliprust[.]Cliprust|cliprust)$"] floating enable"#)?;
    command(r#"for_window [app_id="^(org[.]cliprust[.]Cliprust|cliprust)$"] border pixel 1"#)
}
pub fn available() -> bool {
    super::is_wayland() && std::env::var_os("SWAYSOCK").is_some()
}
pub(super) fn ipc(kind: u32, payload: &str) -> Result<Value> {
    let socket = std::env::var_os("SWAYSOCK")
        .context("SWAYSOCK is missing; launch Cliprust from your Sway session")?;
    let mut stream = UnixStream::connect(socket).context("Connect to Sway IPC")?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    stream.set_write_timeout(Some(Duration::from_secs(2)))?;
    stream.write_all(b"i3-ipc")?;
    stream.write_all(&(payload.len() as u32).to_ne_bytes())?;
    stream.write_all(&kind.to_ne_bytes())?;
    stream.write_all(payload.as_bytes())?;
    let mut header = [0; 14];
    stream.read_exact(&mut header)?;
    if &header[..6] != b"i3-ipc" || u32::from_ne_bytes(header[10..14].try_into()?) != kind {
        bail!("Invalid Sway IPC reply");
    }
    let size = u32::from_ne_bytes(header[6..10].try_into()?) as usize;
    if size > 16 * 1024 * 1024 {
        bail!("Sway IPC reply exceeds 16 MiB");
    }
    let mut bytes = vec![0; size];
    stream.read_exact(&mut bytes)?;
    Ok(serde_json::from_slice(&bytes)?)
}
pub(super) fn command(value: &str) -> Result<()> {
    let reply = ipc(0, value)?;
    let replies = reply.as_array().context("Invalid Sway command reply")?;
    if replies.is_empty() {
        bail!("Empty Sway command reply");
    }
    for reply in replies {
        if reply["success"] != true {
            bail!(
                "Sway: {}",
                reply["error"].as_str().unwrap_or("command refused")
            );
        }
    }
    Ok(())
}
fn visit<'a>(node: &'a Value, predicate: &impl Fn(&Value) -> bool) -> Option<&'a Value> {
    if predicate(node) {
        return Some(node);
    }
    for key in ["nodes", "floating_nodes"] {
        if let Some(children) = node[key].as_array() {
            for child in children {
                if let Some(found) = visit(child, predicate) {
                    return Some(found);
                }
            }
        }
    }
    None
}
fn ours(node: &Value) -> bool {
    node["pid"].as_u64() == Some(std::process::id().into())
        || matches!(
            node["app_id"].as_str(),
            Some("cliprust" | "org.cliprust.Cliprust")
        )
        || node["window_properties"]["class"].as_str() == Some("cliprust")
}
fn leaf(node: &Value) -> bool {
    node["pid"].as_u64().is_some() && (node["app_id"].is_string() || node["window"].is_number())
}
pub fn target() -> Result<Target> {
    let tree = ipc(4, "")?;
    Ok(Target(
        visit(&tree, &|n| n["focused"] == true && leaf(n) && !ours(n))
            .and_then(|n| n["id"].as_u64())
            .unwrap_or(0),
    ))
}
pub fn prepare_show(fallback: Target) -> Result<Target> {
    let workspaces = ipc(1, "")?;
    let workspace = workspaces
        .as_array()
        .and_then(|a| a.iter().find(|w| w["focused"] == true))
        .context("No focused Sway workspace")?;
    let tree = ipc(4, "")?;
    let chosen = if fallback.0 != 0 { fallback } else { target()? };
    let target = visit(&tree, &|n| {
        n["id"].as_u64() == Some(chosen.0) && leaf(n) && !ours(n)
    });
    let mut state = STATE.lock().unwrap();
    let (id, pid) = if let Some(target) = target {
        (chosen.0, target["pid"].as_u64().unwrap_or(0))
    } else {
        state.as_ref().map(|s| (s.target, s.pid)).unwrap_or((0, 0))
    };
    *state = Some(ContextState {
        workspace: workspace["name"]
            .as_str()
            .context("Workspace name missing")?
            .into(),
        output: workspace["output"]
            .as_str()
            .context("Output name missing")?
            .into(),
        target: id,
        pid,
    });
    Ok(Target(id))
}
pub fn show() -> Result<()> {
    let state = STATE
        .lock()
        .unwrap()
        .clone()
        .context("No invoking Sway workspace recorded")?;
    let workspaces = ipc(1, "")?;
    // A workspace can disappear after its last window closes; recreate it only on the recorded live output.
    let outputs = ipc(3, "")?;
    if !outputs.as_array().is_some_and(|a| {
        a.iter()
            .any(|o| o["name"] == state.output && o["active"] == true)
    }) {
        bail!("Invoking monitor was disconnected; summon Cliprust again");
    }
    let tree = ipc(4, "")?;
    let window = visit(&tree, &|n| {
        n["pid"].as_u64() == Some(std::process::id().into())
    })
    .context("Cliprust surface is not mapped yet")?;
    let id = window["id"]
        .as_u64()
        .context("Cliprust container ID missing")?;
    let ws = serde_json::to_string(&state.workspace)?;
    let output = serde_json::to_string(&state.output)?;
    if !workspaces
        .as_array()
        .is_some_and(|a| a.iter().any(|w| w["name"] == state.workspace))
    {
        command(&format!("focus output {output}"))?;
        command(&format!("workspace {ws}"))?;
    }
    let workspaces = ipc(1, "")?;
    let workspace = workspaces
        .as_array()
        .and_then(|all| all.iter().find(|w| w["name"] == state.workspace))
        .context("Invoking workspace was removed")?;
    let bounds = Rect::from_json(&workspace["rect"]).context("Workspace geometry missing")?;
    let geometry = placement(bounds, GEOMETRY.lock().unwrap().as_ref(), &state.output);
    // Commit the tiled-to-floating transition before sizing: Sway/SwayFX can otherwise
    // apply a stale tiled configure after the resize in the same batch.
    command(&format!("[con_id={id}] floating enable"))?;
    command(&format!(
        "[con_id={id}] move container to workspace {ws}, resize set width {} px height {} px, move absolute position {} px {} px, focus",
        geometry.width, geometry.height, geometry.x, geometry.y
    ))?;
    let tree = ipc(4, "")?;
    if !visit(&tree, &|n| n["id"].as_u64() == Some(id)).is_some_and(|n| n["focused"] == true) {
        bail!("Sway did not focus Cliprust");
    }
    Ok(())
}
fn valid_target(tree: &Value, target: Target) -> Result<&Value> {
    if target.0 == 0 {
        bail!("No paste target; summon Cliprust from the destination application");
    }
    let node = visit(tree, &|n| n["id"].as_u64() == Some(target.0))
        .context("Original application window was closed")?;
    if ours(node) || !leaf(node) {
        bail!("Refusing to paste into Cliprust or a non-application container");
    }
    let state = STATE.lock().unwrap();
    if !state
        .as_ref()
        .is_some_and(|s| s.target == target.0 && node["pid"].as_u64() == Some(s.pid))
    {
        bail!("Paste target identity changed; summon Cliprust again");
    }
    Ok(node)
}
fn wtype_path() -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|p|p.join("wtype"))
        .find(|p| p.metadata().is_ok_and(|m|m.is_file() && m.permissions().mode()&0o111!=0))
        .context("wtype is missing. Install it with your package manager (Garuda: pacman -S wtype), then retry")
}
pub fn can_paste(target: Target) -> Result<()> {
    valid_target(&ipc(4, "")?, target)?;
    if use_wtype() {
        wtype_path()?;
    }
    Ok(())
}
fn use_wtype() -> bool {
    std::env::var("CLIPRUST_SWAY_INPUT").as_deref() == Ok("wtype")
}
pub fn paste(target: Target) -> Result<()> {
    can_paste(target)?;
    let native = if use_wtype() {
        None
    } else {
        Some(super::sway_input::Input::prepare()?)
    };
    command(&format!("[con_id={}] focus", target.0))?;
    let deadline = Instant::now() + Duration::from_millis(500);
    loop {
        let tree = ipc(4, "")?;
        if valid_target(&tree, target)?["focused"] == true {
            break;
        }
        if Instant::now() >= deadline {
            bail!("Sway refused to restore destination focus");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    super::wayland::verify_owned()?;
    let tree = ipc(4, "")?;
    if valid_target(&tree, target)?["focused"] != true {
        bail!("Destination lost focus before paste");
    }
    if let Some(input) = native {
        return input.paste();
    }
    // The round-trip confirms focus immediately before creating the virtual keyboard.
    let mut child = Command::new(wtype_path()?)
        .args(["-M", "ctrl", "v", "-m", "ctrl"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .context("Start wtype")?;
    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("wtype timed out; input delivery is unknown");
        }
        std::thread::sleep(Duration::from_millis(5));
    };
    if !status.success() {
        let mut error = String::new();
        if let Some(stderr) = child.stderr.take() {
            let _ = stderr.take(4096).read_to_string(&mut error);
        }
        bail!("wtype failed ({status}): {}", error.trim());
    }
    // Command completion is observable; clipboard consumption is not universally observable.
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn popup_geometry_preserves_size_and_position_and_fits_changed_outputs() {
        let bounds = Rect {
            x: 1280,
            y: 30,
            width: 1280,
            height: 770,
        };
        assert_eq!(
            placement(bounds, None, "DP-1"),
            Rect {
                x: 1530,
                y: 145,
                width: 780,
                height: 540
            }
        );
        let saved = SavedWindow {
            output: "DP-1".into(),
            rect: Rect {
                x: 1500,
                y: 200,
                width: 720,
                height: 460,
            },
            bounds,
        };
        assert_eq!(placement(bounds, Some(&saved), "DP-1"), saved.rect);
        let smaller = Rect {
            x: -800,
            y: 20,
            width: 800,
            height: 600,
        };
        assert_eq!(
            placement(smaller, Some(&saved), "HDMI-1"),
            Rect {
                x: -760,
                y: 90,
                width: 720,
                height: 460
            }
        );
        let clipped = placement(
            Rect {
                width: 680,
                height: 480,
                ..bounds
            },
            Some(&saved),
            "DP-1",
        );
        assert_eq!((clipped.width, clipped.height), (656, 456));
        assert!(clipped.x + clipped.width <= bounds.x + 680);
        assert!(clipped.y + clipped.height <= bounds.y + 480);
    }
    #[test]
    fn tree_search_includes_floating_xwayland_windows() {
        let tree = serde_json::json!({"nodes":[],"floating_nodes":[{"id":7,"pid":123,"window":22,"focused":true}]});
        assert_eq!(
            visit(&tree, &|n| n["focused"] == true && leaf(n)).unwrap()["id"],
            7
        );
        assert!(ours(&serde_json::json!({"app_id":"cliprust"})));
        assert!(!leaf(&serde_json::json!({"id":9,"type":"workspace"})));
    }
}
