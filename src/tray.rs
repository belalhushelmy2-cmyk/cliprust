use anyhow::Result;
use std::{cell::RefCell, sync::Arc};
use tray_icon::{
    Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem},
};

#[derive(Clone, Copy)]
pub enum Command {
    Show,
    Pause,
    Quit,
}
struct Tray {
    icon: TrayIcon,
    pause: MenuItem,
    paused: Option<bool>,
}
thread_local! { static TRAY: RefCell<Option<Tray>> = const { RefCell::new(None) }; }

pub fn install(callback: impl Fn(Command) + Send + Sync + 'static) -> Result<()> {
    let callback = Arc::new(callback);
    let menu = Menu::new();
    let show = MenuItem::with_id("cliprust-show", "Show History", true, None);
    let pause = MenuItem::with_id("cliprust-pause", "Pause Monitoring", true, None);
    let quit = MenuItem::with_id("cliprust-quit", "Quit Cliprust", true, None);
    menu.append_items(&[&show, &pause, &quit])?;
    // Original high-contrast C mark, generated directly as a small RGBA icon.
    let mut rgba = Vec::with_capacity(32 * 32 * 4);
    for y in 0i32..32 {
        for x in 0i32..32 {
            let radius = (x - 16).pow(2) + (y - 16).pow(2);
            let color = if (45..145).contains(&radius) && !(x > 18 && (10..23).contains(&y)) {
                [239, 230, 255, 255]
            } else if radius < 245 {
                [132, 98, 192, 255]
            } else {
                [0, 0, 0, 0]
            };
            rgba.extend_from_slice(&color);
        }
    }
    let icon = TrayIconBuilder::new()
        .with_id("cliprust")
        .with_tooltip("Cliprust — clipboard history")
        .with_menu(Box::new(menu))
        .with_menu_on_left_click(false)
        .with_icon(Icon::from_rgba(rgba, 32, 32)?)
        .build()?;
    let menu_callback = callback.clone();
    MenuEvent::set_event_handler(Some(move |event: MenuEvent| match event.id.as_ref() {
        "cliprust-show" => menu_callback(Command::Show),
        "cliprust-pause" => menu_callback(Command::Pause),
        "cliprust-quit" => menu_callback(Command::Quit),
        _ => {}
    }));
    TrayIconEvent::set_event_handler(Some(move |event| {
        if matches!(
            event,
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            }
        ) {
            callback(Command::Show);
        }
    }));
    TRAY.with(|slot| {
        *slot.borrow_mut() = Some(Tray {
            icon,
            pause,
            paused: None,
        })
    });
    Ok(())
}
pub fn set_paused(paused: bool) {
    TRAY.with(|slot| {
        if let Some(tray) = slot.borrow_mut().as_mut() {
            if tray.paused == Some(paused) {
                return;
            }
            tray.paused = Some(paused);
            tray.pause.set_text(if paused {
                "Resume Monitoring"
            } else {
                "Pause Monitoring"
            });
            let _ = tray.icon.set_tooltip(Some(if paused {
                "Cliprust — monitoring paused"
            } else {
                "Cliprust — monitoring active"
            }));
        }
    });
}
pub fn shutdown() {
    TRAY.with(|slot| *slot.borrow_mut() = None);
}
