use crate::history_session::{EntryKey, HistorySession};
use crate::{
    model::{Bundle, Entry},
    persistence::{Filter, Store},
    platform::{self, Event, Target, Writer},
    snippets::{self, Snippet},
};
use anyhow::{Context, Result, bail};
use chrono::TimeZone;
use crossbeam_channel::{Sender, bounded};
use slint::winit_030::WinitWindowAccessor;
use slint::{ComponentHandle, ModelRc, VecModel, language::ColorScheme};
use std::{
    path::{Path, PathBuf},
    rc::Rc,
};
slint::include_modules!();

fn apply_keymap(ui: &AppWindow, value: &str) -> Result<()> {
    let bindings = crate::shortcuts::parse_keymap(value)?;
    let mut labels = crate::shortcuts::defaults();
    let overrides: std::collections::BTreeMap<String, String> = serde_json::from_str(value)?;
    labels.extend(overrides);
    ui.set_paste_hint(labels["paste"].clone().into());
    ui.set_keep_open_hint(labels["keep-open"].clone().into());
    ui.set_actions_hint(labels["actions"].clone().into());
    ui.set_search_hint(labels["search-focus"].clone().into());
    ui.on_resolve_shortcut(move |key, ctrl, alt, shift, meta| {
        let pressed = crate::shortcuts::Shortcut {
            key: key.to_ascii_lowercase(),
            ctrl,
            alt,
            shift,
            meta,
        };
        bindings
            .iter()
            .find(|(_, binding)| **binding == pressed)
            .map(|(action, _)| action.as_str())
            .unwrap_or("")
            .into()
    });
    Ok(())
}

fn prepare_desktop_target(fallback: Target) -> Target {
    #[cfg(target_os = "linux")]
    if platform::sway::available() {
        return platform::sway::prepare_show(fallback).unwrap_or_else(|error| {
            eprintln!("Cliprust Sway: {error:#}");
            Target(0)
        });
    }
    #[cfg(target_os = "linux")]
    if platform::kde::is_kde() {
        return match platform::kde::request("prepare", fallback) {
            Ok(target) => target,
            Err(error) => {
                eprintln!("Cliprust desktop placement: {error:#}");
                Target(0)
            }
        };
    }
    fallback
}
fn finish_desktop_show(ui: &AppWindow) {
    #[cfg(target_os = "linux")]
    if platform::sway::available() || platform::kde::is_kde() {
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            // A newly mapped Wayland surface may not reach KWin until the next UI flush.
            let mut result = Ok(Target(0));
            for _ in 0..5 {
                std::thread::sleep(std::time::Duration::from_millis(40));
                result = if platform::sway::available() {
                    platform::sway::show().map(|()| Target(0))
                } else {
                    platform::kde::request("show", Target(0))
                };
                if result.is_ok() {
                    return;
                }
            }
            if let Err(error) = result {
                eprintln!("Cliprust desktop placement: {error:#}");
                let _ = weak.upgrade_in_event_loop(move |ui| {
                    ui.set_status(format!("Desktop placement: {error:#}").into())
                });
            }
        });
    }
    #[cfg(not(target_os = "linux"))]
    let _ = ui;
}

fn set_ui_theme(ui: &AppWindow, mode: i32, system_light: bool) {
    let palette = ui.global::<AppPalette>();
    palette.set_mode(mode);
    palette.set_system_light(system_light);
    ui.global::<Palette>()
        .set_color_scheme(if mode == 1 || (mode == 2 && system_light) {
            ColorScheme::Light
        } else {
            ColorScheme::Dark
        });
}

#[derive(Debug)]
enum Request {
    Hidden,
    Search(String),
    Group(String),
    Select(String),
    Filter(String, bool, String, String),
    Action(String),
    Save(String, String, String, String, bool, bool),
    SaveSettings(String, String, bool, i32, String),
    SaveShortcuts(String, String, String),
    Show(Target),
    #[cfg_attr(windows, allow(dead_code))]
    Toggle(Target),
    Activate(String),
    FileChosen(
        String,
        Option<Bundle>,
        std::result::Result<Option<PathBuf>, String>,
    ),
}
struct Worker {
    store: Store,
    writer: Option<Writer>,
    weak: slint::Weak<AppWindow>,
    query: String,
    filter: Filter,
    entries: Vec<Entry>,
    snippets: Vec<Snippet>,
    snippet_mode: bool,
    selected: usize,
    target: Target,
    status: String,
    paused: bool,
    session: Option<HistorySession>,
    visible: bool,
    select_latest: bool,
    select_first: bool,
    requests: Sender<Request>,
    theme_mode: i32,
    system_light: bool,
    theme_tx: crossbeam_channel::Sender<Event>,
    theme_watch_started: bool,
}
impl Worker {
    fn choose_file(&mut self, action: &str, save: bool, name: String, bundle: Option<Bundle>) {
        let action = action.to_owned();
        let tx = self.requests.clone();
        self.status = "Choose a file in the desktop dialog".into();
        std::thread::spawn(move || {
            let result = platform::files::choose(save, &name).map_err(|error| format!("{error:#}"));
            let _ = tx.send(Request::FileChosen(action, bundle, result));
        });
    }
    fn apply_theme(&self) {
        let mode = self.theme_mode;
        let system_light = self.system_light;
        self.send(move |ui| set_ui_theme(&ui, mode, system_light));
    }
    fn start_theme_watch(&mut self) {
        if self.theme_watch_started || self.theme_mode != 2 {
            return;
        }
        self.theme_watch_started = true;
        let tx = self.theme_tx.clone();
        std::thread::spawn(move || {
            if let Err(error) = platform::watch_system_theme(tx.clone()) {
                let _ = tx.send(Event::Diagnostic(format!(
                    "System theme tracking unavailable: {error:#}"
                )));
            }
        });
    }
    fn send(&self, f: impl FnOnce(AppWindow) + Send + 'static) {
        let weak = self.weak.clone();
        let _ = slint::invoke_from_event_loop(move || {
            if let Some(ui) = weak.upgrade() {
                f(ui);
            }
        });
    }
    fn refresh(&mut self) -> Result<()> {
        let previous = self.entries.get(self.selected).map(EntryKey::of);
        if self.session.is_none() {
            self.session = Some(HistorySession::capture(&self.store)?);
        }
        let session = self.session.as_ref().unwrap();
        self.snippets = session.snippets.clone();
        self.entries = session.list(&self.query, &self.filter, self.snippet_mode);
        let latest = session.latest;
        let grouping = self
            .store
            .setting("grouping")
            .unwrap_or_else(|| "none".into());
        let group_labels = crate::grouping::group(
            &mut self.entries,
            &grouping,
            chrono::Local::now().date_naive(),
        );
        self.selected = if self.select_latest {
            latest
                .and_then(|id| self.entries.iter().position(|e| e.id == id))
                .unwrap_or(0)
        } else if self.select_first {
            0
        } else {
            previous
                .and_then(|key| self.entries.iter().position(|e| EntryKey::of(e) == key))
                .unwrap_or(self.selected.min(self.entries.len().saturating_sub(1)))
        };
        self.select_latest = false;
        self.select_first = false;
        let now = chrono::Utc::now().timestamp();
        let mut row_top = 0;
        let rows: Vec<_> = self
            .entries
            .iter()
            .zip(group_labels)
            .map(|(e, group)| {
                let offset = row_top;
                row_top += 70 + if group.is_empty() { 0 } else { 24 };
                let age = now.saturating_sub(e.last_used).max(0);
                let time = if e.kind == "snippet" {
                    "Template".into()
                } else if age < 60 {
                    "Just now".into()
                } else if age < 3600 {
                    format!("{}m ago", age / 60)
                } else if age < 86400 {
                    format!("{}h ago", age / 3600)
                } else {
                    format!("{}d ago", age / 86400)
                };
                HistoryRow {
                    group: group.into(),
                    offset,
                    id: EntryKey::of(e).token().into(),
                    title: e
                        .preview
                        .lines()
                        .next()
                        .unwrap_or("Empty text")
                        .chars()
                        .take(140)
                        .collect::<String>()
                        .into(),
                    subtitle: format!(
                        "{} · {} · {} copies{}",
                        e.kind,
                        time,
                        e.copy_count,
                        e.source
                            .as_deref()
                            .map(|source| format!(" · {source}"))
                            .unwrap_or_default()
                    )
                    .into(),
                    badge: match e.kind.as_str() {
                        "image" => "IMG",
                        "files" => "FILE",
                        "link" => "↗",
                        "color" => "#",
                        "snippet" => "{ }",
                        _ => "Aa",
                    }
                    .into(),
                    pinned: e.pinned,
                }
            })
            .collect();
        let selected = self.selected as i32;
        let status = self.status.clone();
        let paused = self.paused;
        let supported = platform::paste_supported(self.target);
        let mut source_options = vec!["All applications".to_owned()];
        source_options.extend(self.store.sources()?);
        if let Some(source) = self.filter.source.as_ref()
            && !source_options.contains(source)
        {
            source_options.push(source.clone());
        }
        let source_options: Vec<slint::SharedString> =
            source_options.into_iter().map(Into::into).collect();
        let source_selection = self
            .filter
            .source
            .clone()
            .unwrap_or_else(|| "All applications".into());
        let recent_searches: Vec<slint::SharedString> =
            std::iter::once("Recent searches".to_owned())
                .chain(self.store.recent_searches())
                .map(Into::into)
                .collect();
        self.send(move |ui| {
            ui.set_recent_searches(ModelRc::from(Rc::new(VecModel::from(recent_searches))));
            ui.set_grouping(grouping.into());
            ui.set_rows_height(row_top);
            ui.set_rows(ModelRc::from(Rc::new(VecModel::from(rows))));
            ui.set_source_options(ModelRc::from(Rc::new(VecModel::from(source_options))));
            ui.set_source_selection(source_selection.into());
            ui.set_selected(selected);
            ui.set_status(status.into());
            ui.set_paused(paused);
            crate::tray::set_paused(paused);
            ui.set_paste_supported(supported);
        });
        self.preview()?;
        Ok(())
    }
    fn selected_bundle(&self) -> Result<Bundle> {
        let e = self
            .entries
            .get(self.selected)
            .context("Select an entry first")?;
        if e.id < 0 {
            let s = self
                .snippets
                .iter()
                .find(|s| s.name == e.identity)
                .context("Snippet no longer available")?;
            let clipboard = if s.body.contains("{{clipboard}}") {
                platform::current_text()?
            } else {
                String::new()
            };
            Ok(Bundle::text(&snippets::expand(
                &s.body,
                chrono::Local::now(),
                &clipboard,
            )))
        } else {
            self.store.bundle(e.id)
        }
    }
    fn preview(&self) -> Result<()> {
        let Some(entry) = self.entries.get(self.selected) else {
            self.send(|ui| {
                ui.set_preview("Your next idea starts with a copy.".into());
                ui.set_details("Clipboard content stays on this device.".into());
                ui.set_has_image(false);
            });
            return Ok(());
        };
        let bundle = self.selected_bundle()?;
        let text = bundle.summary();
        let details = format!(
            "{}\n{}\n{} bytes · {} formats",
            entry.kind,
            entry.source.as_deref().unwrap_or("Source unavailable"),
            bundle.formats.values().map(Vec::len).sum::<usize>(),
            bundle.formats.len()
        );
        let pixels = bundle
            .formats
            .iter()
            .find(|(mime, _)| mime.starts_with("image/"))
            .and_then(|(_, bytes)| {
                let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
                    .with_guessed_format()
                    .ok()?;
                let mut limits = image::Limits::default();
                limits.max_alloc = Some(64 * 1024 * 1024);
                limits.max_image_width = Some(16384);
                limits.max_image_height = Some(16384);
                reader.limits(limits);
                let image = reader.decode().ok()?.thumbnail(600, 440).to_rgba8();
                Some((image.width(), image.height(), image.into_raw()))
            });
        self.send(move |ui| {
            ui.set_preview(text.into());
            ui.set_details(details.into());
            ui.set_has_image(pixels.is_some());
            if let Some((w, h, bytes)) = pixels {
                let buffer =
                    slint::SharedPixelBuffer::<slint::Rgba8Pixel>::clone_from_slice(&bytes, w, h);
                ui.set_preview_image(slint::Image::from_rgba8(buffer));
            }
        });
        Ok(())
    }
    fn update_status(&self) -> Result<()> {
        let status = self.status.clone();
        let paused = self.paused;
        let supported = platform::paste_supported(self.target);
        self.send(move |ui| {
            ui.set_status(status.into());
            ui.set_paused(paused);
            crate::tray::set_paused(paused);
            ui.set_paste_supported(supported);
        });
        Ok(())
    }
    fn begin_session(&mut self) {
        self.visible = true;
        self.session = None;
        self.select_latest = true;
        self.query.clear();
        self.filter = Filter::default();
        self.snippet_mode = false;
        self.send(|ui| {
            ui.set_query("".into());
            ui.set_type_filter("all".into());
            ui.set_date_filter("all".into());
            ui.set_favorites(false);
            ui.set_source_selection("All applications".into());
        });
    }
    fn handle(&mut self, request: Request) -> Result<()> {
        let opening = matches!(&request, Request::Show(_) | Request::Activate(_)) && !self.visible;
        let changed = matches!(&request, Request::Save(..) | Request::SaveSettings(..))
            || matches!(&request, Request::FileChosen(action, _, Ok(Some(_))) if matches!(action.as_str(), "import-history" | "restore-backup"))
            || matches!(&request, Request::Action(action) if matches!(action.as_str(), "pin" | "pin-up" | "pin-down" | "delete" | "duplicate" | "clear-unpinned"));
        let view_changed = matches!(
            &request,
            Request::Search(_) | Request::Filter(..) | Request::Group(_)
        );
        if opening {
            self.begin_session();
        }
        if matches!(&request, Request::Search(_) | Request::Filter(..)) {
            self.select_first = true;
        }
        match request {
            Request::Hidden => {
                self.visible = false;
                self.store.remember_search(&self.query)?;
                return self.update_status();
            }
            Request::FileChosen(action, bundle, result) => {
                let Some(path) = result.map_err(anyhow::Error::msg)? else {
                    self.status = "File selection cancelled".into();
                    return self.update_status();
                };
                match action.as_str() {
                    "backup" => {
                        let file = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)?;
                        drop(file);
                        self.store.backup(&path)?;
                    }
                    "import-history" => {
                        let file = std::fs::File::open(&path)?;
                        if file.metadata()?.len() > 256 * 1024 * 1024 {
                            bail!("Import exceeds 256 MiB safety limit");
                        }
                        self.store
                            .import(serde_json::from_reader(std::io::BufReader::new(file))?)?;
                    }
                    "restore-backup" => self.store.merge_backup(&path)?,
                    "export-history" | "export-entry" => {
                        let file = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)?;
                        if let Some(bundle) = bundle {
                            serde_json::to_writer_pretty(file, &bundle)?;
                        } else {
                            serde_json::to_writer_pretty(file, &self.store.export()?)?;
                        }
                    }
                    "save-image" => {
                        use std::io::Write;
                        let bundle = bundle.context("Image selection no longer available")?;
                        let bytes = bundle
                            .formats
                            .iter()
                            .find(|(mime, _)| mime.starts_with("image/"))
                            .context("No image format")?
                            .1;
                        let mut file = std::fs::OpenOptions::new()
                            .write(true)
                            .create_new(true)
                            .open(&path)?;
                        file.write_all(bytes)?;
                    }
                    _ => bail!("Unknown file action"),
                }
                self.status = if action == "restore-backup" {
                    "Backup history merged; current history and settings retained".into()
                } else {
                    format!("Completed {action}: {}", path.display())
                };
            }
            Request::SaveShortcuts(global, paste, keep_open) => {
                let global_key = crate::shortcuts::Shortcut::global(&global)?;
                let mut overrides: std::collections::BTreeMap<String, String> =
                    serde_json::from_str(
                        &self.store.setting("keymap").unwrap_or_else(|| "{}".into()),
                    )?;
                overrides.insert("paste".into(), paste);
                overrides.insert("keep-open".into(), keep_open);
                let value = serde_json::to_string(&overrides)?;
                let bindings = crate::shortcuts::parse_keymap(&value)?;
                if bindings.values().any(|binding| *binding == global_key) {
                    bail!("Global shortcut conflicts with an in-window action");
                }
                let previous = self
                    .store
                    .setting("global_shortcut")
                    .unwrap_or_else(|| "Super+V".into());
                // In-window bindings are live; they never need a process restart.
                self.store.set_setting("keymap", &value)?;
                let applied = value.clone();
                self.send(move |ui| {
                    let _ = apply_keymap(&ui, &applied);
                });
                let launcher_changed = crate::shortcuts::Shortcut::global(&previous)? != global_key;
                #[cfg(target_os = "linux")]
                let launcher_changed = launcher_changed || platform::sway::available();
                self.status = if launcher_changed {
                    match platform::apply_launcher_shortcut(&global_key) {
                        Ok(message) => {
                            self.store.set_setting("global_shortcut", &global)?;
                            message.into()
                        }
                        Err(error) => {
                            format!("Paste shortcuts applied. Launcher unchanged: {error:#}")
                        }
                    }
                } else {
                    "Paste shortcuts applied immediately.".into()
                };
            }
            Request::Group(mode) => {
                if !matches!(
                    mode.as_str(),
                    "none" | "date" | "pins" | "type" | "application"
                ) {
                    bail!("Unknown grouping mode");
                }
                self.store.set_setting("grouping", &mode)?;
            }
            Request::Search(q) => {
                self.query = q;
                self.selected = 0;
            }
            Request::Filter(kind, pinned, period, source) => {
                self.snippet_mode = kind == "snippets";
                let (since, until) = date_bounds(&period);
                self.filter = Filter {
                    kind: if kind == "all" || self.snippet_mode {
                        None
                    } else {
                        Some(kind)
                    },
                    pinned,
                    source: (!source.is_empty()).then_some(source),
                    since,
                    until,
                };
                self.selected = 0;
            }
            Request::Select(key) => {
                if let Some(index) = self
                    .entries
                    .iter()
                    .position(|entry| EntryKey::of(entry).token() == key)
                {
                    self.selected = index;
                    return self.preview();
                }
                return self.update_status();
            }
            Request::Activate(token) => {
                #[cfg(not(target_os = "linux"))]
                let _ = &token;
                self.target = prepare_desktop_target(self.target);
                self.send(move |ui| {
                    if let Err(error) = ui.show() {
                        ui.set_status(format!("Show window: {error}").into());
                        return;
                    }
                    ui.invoke_focus_list();
                    finish_desktop_show(&ui);
                    #[cfg(target_os = "linux")]
                    ui.window().with_winit_window(|window| {
                        window.set_minimized(false);
                        if let Err(error) = platform::activation::activate(window, &token) {
                            eprintln!("Cliprust activation: {error:#}");
                            ui.set_status(format!("Window activation: {error:#}").into());
                        }
                    });
                });
            }
            Request::Toggle(target) => {
                let requests = self.requests.clone();
                self.send(move |ui| {
                    let focused = ui
                        .window()
                        .with_winit_window(|w| w.has_focus())
                        .unwrap_or(false);
                    if ui.window().is_visible() && focused {
                        let _ = ui.hide();
                        let _ = requests.send(Request::Hidden);
                    } else {
                        let _ = requests.try_send(Request::Show(target));
                    }
                });
            }
            Request::Show(target) => {
                let target = prepare_desktop_target(target);
                if target.0 != 0 || platform::is_wayland() {
                    self.target = target;
                }
                self.send(|ui| {
                    if let Err(error) = ui.show() {
                        ui.set_status(format!("Show window: {error}").into());
                        return;
                    }
                    ui.invoke_focus_list();
                    finish_desktop_show(&ui);
                    ui.window().with_winit_window(|w| {
                        w.set_minimized(false);
                        w.focus_window();
                    });
                });
            }
            Request::Save(name, body, tags, abbreviation, snippet, pinned) => {
                if snippet {
                    self.store.save_snippet(&Snippet {
                        name,
                        body,
                        tags: tags
                            .split(',')
                            .map(str::trim)
                            .filter(|t| !t.is_empty())
                            .map(str::to_owned)
                            .collect(),
                        abbreviation,
                        pinned,
                    })?;
                } else {
                    self.store.capture(
                        &Bundle::text(&body),
                        None,
                        chrono::Utc::now().timestamp(),
                    )?;
                }
                self.status = "Saved locally".into();
            }
            Request::SaveSettings(
                max_entries,
                retention_days,
                launch_at_login,
                theme_mode,
                excluded_apps,
            ) => {
                let excluded_apps = serde_json::to_string(
                    &excluded_apps
                        .lines()
                        .map(str::trim)
                        .filter(|name| !name.is_empty())
                        .collect::<Vec<_>>(),
                )?;
                crate::security::parse_exclusions(&excluded_apps)?;
                let max_entries: usize = max_entries
                    .trim()
                    .parse()
                    .context("Maximum entries must be a whole number")?;
                let retention_days: u32 = retention_days
                    .trim()
                    .parse()
                    .context("Retention days must be a whole number")?;
                if max_entries > 1_000_000 {
                    bail!("Maximum entries must be at most 1,000,000")
                }
                if retention_days > 36_500 {
                    bail!("Retention days must be at most 36,500")
                }
                if !(0..=2).contains(&theme_mode) {
                    bail!("Appearance must be Dark, Light, or System")
                }
                self.store.set_setting("excluded_apps", &excluded_apps)?;
                self.store
                    .set_setting("max_entries", &max_entries.to_string())?;
                self.store
                    .set_setting("retention_days", &retention_days.to_string())?;
                platform::set_autostart(launch_at_login)?;
                self.store.set_setting(
                    "launch_at_login",
                    if launch_at_login { "true" } else { "false" },
                )?;
                let theme = match theme_mode {
                    1 => "light",
                    2 => "system",
                    _ => "dark",
                };
                self.store.set_setting("theme", theme)?;
                self.theme_mode = theme_mode;
                if theme_mode == 2 {
                    if let Ok(system_light) = platform::system_uses_light_theme() {
                        self.system_light = system_light;
                    } else {
                        self.system_light = false;
                    }
                    self.start_theme_watch();
                }
                self.apply_theme();
                let removed = self.store.cleanup(
                    max_entries,
                    retention_days,
                    chrono::Utc::now().timestamp(),
                )?;
                self.status = format!("History settings saved · {removed} expired entries removed");
            }
            Request::Action(action) => self.action(&action)?,
        }
        if changed {
            self.session = None;
        }
        if opening || changed || view_changed {
            self.refresh()
        } else {
            self.update_status()
        }
    }
    fn action(&mut self, action: &str) -> Result<()> {
        match action {
            "backup" | "export-history" | "import-history" | "restore-backup" => {
                let save = matches!(action, "backup" | "export-history");
                let name = if action == "backup" {
                    "cliprust-backup.db"
                } else if save {
                    "cliprust-history.json"
                } else {
                    ""
                };
                self.choose_file(action, save, name.into(), None);
                return Ok(());
            }
            "configure-launcher" => {
                #[cfg(target_os = "linux")]
                {
                    let shortcut = crate::shortcuts::Shortcut::global(
                        &self
                            .store
                            .setting("global_shortcut")
                            .unwrap_or_else(|| "Super+V".into()),
                    )?;
                    self.status = platform::apply_launcher_shortcut(&shortcut)?.into();
                }
                #[cfg(not(target_os = "linux"))]
                {
                    self.status = "Edit Open history above, save, then restart Cliprust.".into();
                }
                return Ok(());
            }
            "clear-searches" => {
                self.store.set_setting("recent_searches", "[]")?;
                self.send(|ui| {
                    ui.set_recent_searches(ModelRc::from(Rc::new(VecModel::from(vec![
                        "Recent searches".into(),
                    ]))))
                });
                self.status = "Recent searches cleared".into();
                return Ok(());
            }
            "remember-search" => {
                self.store.remember_search(&self.query)?;
                return Ok(());
            }
            "settings" => {
                let (max_entries, retention_days) = self.store.retention();
                let launch_at_login = platform::autostart_enabled()?;
                self.theme_mode = match self.store.setting("theme").as_deref() {
                    Some("light") => 1,
                    Some("system") => 2,
                    _ => 0,
                };
                if self.theme_mode == 2 {
                    self.system_light =
                        platform::system_uses_light_theme().unwrap_or(self.system_light);
                    self.start_theme_watch();
                }
                let excluded_apps = crate::security::parse_exclusions(
                    &self
                        .store
                        .setting("excluded_apps")
                        .unwrap_or_else(|| "[]".into()),
                )?
                .join("\n");
                let global_shortcut = self
                    .store
                    .setting("global_shortcut")
                    .unwrap_or_else(|| "Super+V".into());
                let mut keymap = crate::shortcuts::defaults();
                let overrides: std::collections::BTreeMap<String, String> = serde_json::from_str(
                    &self.store.setting("keymap").unwrap_or_else(|| "{}".into()),
                )?;
                keymap.extend(overrides);
                let paste_shortcut = keymap["paste"].clone();
                let keep_shortcut = keymap["keep-open"].clone();
                let theme_mode = self.theme_mode;
                let system_light = self.system_light;
                self.send(move |ui| {
                    ui.set_global_shortcut(global_shortcut.into());
                    ui.set_paste_shortcut(paste_shortcut.into());
                    ui.set_keep_shortcut(keep_shortcut.into());
                    ui.set_excluded_apps(excluded_apps.into());
                    ui.set_retention_limit(max_entries.to_string().into());
                    ui.set_retention_days(retention_days.to_string().into());
                    ui.set_launch_at_login(launch_at_login);
                    ui.set_theme_index(theme_mode);
                    set_ui_theme(&ui, theme_mode, system_light);
                    ui.set_settings_open(true);
                });
                return Ok(());
            }
            "quit" => {
                let _ = slint::quit_event_loop();
                return Ok(());
            }
            "pause" => {
                self.paused = !self.paused;
                platform::set_capture_paused(self.paused);
                self.store
                    .set_setting("paused", if self.paused { "true" } else { "false" })?;
                self.status = if self.paused {
                    "Monitoring paused · privacy mode"
                } else {
                    "Monitoring active · local storage"
                }
                .into();
                return Ok(());
            }
            "clear-unpinned" => {
                self.store.clear_unpinned()?;
                self.status = "Deleted all unpinned history".into();
                return Ok(());
            }
            _ => {}
        }
        let entry = self
            .entries
            .get(self.selected)
            .context("Select an entry first")?
            .clone();
        match action {
            "duplicate" if entry.id > 0 => {
                self.store
                    .duplicate(entry.id, chrono::Utc::now().timestamp())?;
                self.status = "Created an independent duplicate entry".into();
            }
            "duplicate" => {
                let original = self
                    .snippets
                    .iter()
                    .find(|s| s.name == entry.identity)
                    .context("Snippet no longer available")?;
                let mut duplicate = original.clone();
                let mut number = 1;
                loop {
                    duplicate.name = format!("{} (copy {number})", original.name);
                    if !self
                        .snippets
                        .iter()
                        .any(|snippet| snippet.name == duplicate.name)
                    {
                        break;
                    }
                    number += 1;
                }
                duplicate.abbreviation.clear();
                self.store.save_snippet(&duplicate)?;
                self.status = "Duplicated snippet with a new name and no abbreviation".into();
            }
            "pin-up" if entry.id > 0 => {
                self.status = if self.store.move_pin(entry.id, -1)? {
                    "Pinned item moved up"
                } else {
                    "Item is not pinned or is already first"
                }
                .into();
            }
            "pin-down" if entry.id > 0 => {
                self.status = if self.store.move_pin(entry.id, 1)? {
                    "Pinned item moved down"
                } else {
                    "Item is not pinned or is already last"
                }
                .into();
            }
            "pin" if entry.id > 0 => self.store.pin(entry.id)?,
            "pin" if entry.id < 0 => {
                let mut snippet = self
                    .snippets
                    .iter()
                    .find(|s| s.name == entry.identity)
                    .context("Snippet no longer available")?
                    .clone();
                snippet.pinned = !snippet.pinned;
                self.store.save_snippet(&snippet)?;
            }
            "delete" if entry.id > 0 => self.store.delete(entry.id)?,
            "delete" => self.store.delete_snippet(&entry.identity)?,
            "copy" | "plain-copy" | "copy-image" | "copy-path" | "paste" | "keep-open"
            | "plain-paste" | "plain-keep-open" => {
                let paste = matches!(
                    action,
                    "paste" | "keep-open" | "plain-paste" | "plain-keep-open"
                );
                let keep_open = matches!(action, "keep-open" | "plain-keep-open");
                if paste {
                    platform::prepare_paste(self.target)?;
                }
                let mut bundle = self.selected_bundle()?;
                if matches!(action, "plain-copy" | "plain-paste" | "plain-keep-open") {
                    bundle = Bundle::text(
                        &bundle
                            .plain_text()
                            .context("This entry has no plain text representation")?,
                    );
                } else if action == "copy-image" {
                    let formats = bundle
                        .formats
                        .into_iter()
                        .filter(|(mime, _)| mime.starts_with("image/"))
                        .collect();
                    bundle = Bundle { formats };
                    bundle
                        .validate()
                        .context("This entry has no supported image format")?;
                } else if action == "copy-path" {
                    let paths = bundle
                        .file_paths()?
                        .context("Select a file-reference entry")?;
                    bundle = Bundle::text(&paths.join("\n"));
                }
                self.writer
                    .as_ref()
                    .context("Clipboard writer unavailable")?
                    .write(&bundle)?;
                if paste {
                    platform::paste(self.target)?;
                    // Keep-open path never hides/minimizes/destroys the window or resets selection/query.
                    if !keep_open {
                        self.visible = false;
                        self.send(|ui| {
                            let _ = ui.hide();
                        });
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(80));
                        self.send(|ui| {
                            finish_desktop_show(&ui);
                            if !platform::is_wayland() {
                                ui.window().with_winit_window(|w| w.focus_window());
                            }
                        });
                    }
                }
                self.store.remember_search(&self.query)?;
                if entry.id > 0 {
                    self.store.used(entry.id, chrono::Utc::now().timestamp())?;
                }
                self.status = if paste {
                    if action == "plain-paste" || action == "plain-keep-open" {
                        "Plain-text paste input sent to the previous application"
                    } else {
                        "Paste input sent to the previous application"
                    }
                } else {
                    "Copied to clipboard"
                }
                .into();
            }
            "edit" | "save-snippet" => {
                let body = self
                    .selected_bundle()?
                    .plain_text()
                    .context("Only text entries can be edited")?;
                let snippet = action == "save-snippet" || entry.id < 0;
                let existing = if entry.id < 0 {
                    self.snippets
                        .iter()
                        .find(|s| s.name == entry.identity)
                        .cloned()
                } else {
                    None
                };
                let edit_pinned = existing.as_ref().is_some_and(|s| s.pinned);
                self.send(move |ui| {
                    ui.set_edit_body(body.into());
                    ui.set_edit_name(
                        existing
                            .as_ref()
                            .map(|s| s.name.as_str())
                            .unwrap_or("")
                            .into(),
                    );
                    ui.set_edit_tags(
                        existing
                            .as_ref()
                            .map(|s| s.tags.join(", "))
                            .unwrap_or_default()
                            .into(),
                    );
                    ui.set_edit_abbreviation(
                        existing.map(|s| s.abbreviation).unwrap_or_default().into(),
                    );
                    ui.set_edit_pinned(edit_pinned);
                    ui.set_snippet_edit(snippet);
                    ui.set_editor_open(true);
                });
            }
            "export-entry" => {
                let bundle = self.selected_bundle()?;
                self.choose_file(
                    action,
                    true,
                    format!("entry-{}.json", entry.id),
                    Some(bundle),
                );
            }
            "save-image" => {
                let bundle = self.selected_bundle()?;
                let (mime, _) = bundle
                    .formats
                    .iter()
                    .find(|(m, _)| m.starts_with("image/"))
                    .context("Select an image entry")?;
                let ext = match mime.as_str() {
                    "image/png" => "png",
                    "image/jpeg" => "jpg",
                    "image/bmp" => "bmp",
                    _ => bail!("image format cannot be saved"),
                };
                self.choose_file(
                    action,
                    true,
                    format!("image-{}.{ext}", entry.id),
                    Some(bundle),
                );
            }
            "open" => {
                let bundle = self.selected_bundle()?;
                let text = bundle.plain_text().context("No URL or file reference")?;
                if entry.kind == "link" {
                    let url = text.trim();
                    if url.contains(['\n', '\r', '\0']) {
                        bail!("Invalid URL")
                    };
                    platform::open_external(url)?;
                } else if entry.kind == "files" {
                    let file = text
                        .lines()
                        .find(|l| !l.starts_with('#') && !l.is_empty())
                        .context("Empty file list")?;
                    platform::open_external(file)?;
                } else {
                    bail!("Open is available for links and file references")
                }
            }
            _ => bail!("Action is unavailable for this entry"),
        }
        Ok(())
    }
}

fn date_bounds(period: &str) -> (Option<i64>, Option<i64>) {
    let start_of_day = |offset: i64| {
        let date = chrono::Local::now().date_naive() + chrono::Duration::days(offset);
        let midnight = date.and_time(chrono::NaiveTime::MIN);
        chrono::Local
            .from_local_datetime(&midnight)
            .earliest()
            .or_else(|| chrono::Local.from_local_datetime(&midnight).latest())
            .map(|dt| dt.timestamp())
            .unwrap_or_else(|| midnight.and_utc().timestamp())
    };
    let today = start_of_day(0);
    let tomorrow = start_of_day(1);
    match period {
        "today" => (Some(today), Some(tomorrow)),
        "yesterday" => (Some(start_of_day(-1)), Some(today)),
        "last7" => (Some(start_of_day(-6)), Some(tomorrow)),
        _ => (None, None),
    }
}

pub fn run(
    path: &Path,
    initial_target: Target,
    snapshot: Option<PathBuf>,
    snapshot_settings: bool,
) -> Result<()> {
    #[cfg(target_os = "linux")]
    let startup_token = std::env::var("XDG_ACTIVATION_TOKEN").ok();
    let (tx, rx) = bounded::<Request>(128);
    #[cfg(unix)]
    let _socket = ipc::listen(path, tx.clone())?;
    slint::BackendSelector::new()
        .backend_name("winit".into())
        .with_winit_window_attributes_hook(move |attributes| {
            #[cfg(target_os = "linux")]
            let attributes = {
                use slint::winit_030::winit::platform::startup_notify::WindowAttributesExtStartupNotify;
                if let Some(token) = &startup_token {
                    attributes.with_activation_token(slint::winit_030::winit::window::ActivationToken::from_raw(token.clone()))
                } else { attributes }
            };
            let attributes = attributes
                .with_window_level(slint::winit_030::winit::window::WindowLevel::AlwaysOnTop);
            #[cfg(target_os = "linux")]
            let attributes = {
                use slint::winit_030::winit::platform::{
                    wayland::WindowAttributesExtWayland, x11::WindowAttributesExtX11,
                };
                let attributes = WindowAttributesExtWayland::with_name(
                    attributes,
                    "org.cliprust.Cliprust",
                    "Cliprust",
                );
                WindowAttributesExtX11::with_name(attributes, "cliprust", "Cliprust")
            };
            attributes
        })
        .select()?;
    #[cfg(target_os = "linux")]
    if let Err(error) = platform::kde::start() {
        eprintln!("Cliprust: {error:#}");
    }
    let initial_target = prepare_desktop_target(initial_target);
    let ui = AppWindow::new()?;
    let weak = ui.as_weak();
    let (capture_tx, capture_rx) = bounded::<Event>(16);
    {
        let tx = tx.clone();
        ui.on_change_group(move |mode| {
            let _ = tx.try_send(Request::Group(mode.to_string()));
        });
    }
    {
        let tx = tx.clone();
        ui.on_search(move |q| {
            let _ = tx.try_send(Request::Search(q.to_string()));
        });
    }
    {
        let tx = tx.clone();
        ui.on_select_row(move |i| {
            let _ = tx.try_send(Request::Select(i.to_string()));
        });
    }
    {
        let tx = tx.clone();
        ui.on_filter(move |k, p, d, s| {
            let _ = tx.try_send(Request::Filter(
                k.to_string(),
                p,
                d.to_string(),
                s.to_string(),
            ));
        });
    }
    {
        let tx = tx.clone();
        ui.on_action(move |a| {
            let _ = tx.try_send(Request::Action(a.to_string()));
        });
    }
    {
        let tx = tx.clone();
        ui.on_save_edit(move |n, b, t, a, s, p| {
            let _ = tx.try_send(Request::Save(
                n.to_string(),
                b.to_string(),
                t.to_string(),
                a.to_string(),
                s,
                p,
            ));
        });
    }
    {
        let tx = tx.clone();
        ui.on_save_shortcuts(move |global, paste, keep| {
            let _ = tx.try_send(Request::SaveShortcuts(
                global.to_string(),
                paste.to_string(),
                keep.to_string(),
            ));
        });
    }
    {
        let tx = tx.clone();
        ui.on_save_settings(
            move |max_entries, retention_days, launch_at_login, theme_mode, excluded_apps| {
                let _ = tx.try_send(Request::SaveSettings(
                    max_entries.to_string(),
                    retention_days.to_string(),
                    launch_at_login,
                    theme_mode,
                    excluded_apps.to_string(),
                ));
            },
        );
    }
    {
        let weak = ui.as_weak();
        let tx = tx.clone();
        ui.on_dismiss(move || {
            let _ = tx.try_send(Request::Hidden);
            if let Some(ui) = weak.upgrade() {
                let _ = ui.hide();
            }
        });
    }
    ui.window().on_close_requested({
        let weak = ui.as_weak();
        let tx = tx.clone();
        move || {
            let _ = tx.try_send(Request::Hidden);
            if let Some(ui) = weak.upgrade() {
                let _ = ui.hide();
            }
            slint::CloseRequestResponse::KeepWindowShown
        }
    });
    let store = Store::open(path)?;
    let theme_mode = match store.setting("theme").as_deref() {
        Some("light") => 1,
        Some("system") => 2,
        _ => 0,
    };
    let system_light = if theme_mode == 2 {
        platform::system_uses_light_theme().unwrap_or(false)
    } else {
        false
    };
    set_ui_theme(&ui, theme_mode, system_light);
    let paused = store.setting("paused").as_deref() == Some("true");
    platform::set_capture_paused(paused);
    if snapshot.is_none() {
        let requests = tx.clone();
        if let Err(error) = crate::tray::install(move |command| {
            let request = match command {
                crate::tray::Command::Show => Request::Show(platform::target().unwrap_or_default()),
                crate::tray::Command::Pause => Request::Action("pause".into()),
                crate::tray::Command::Quit => Request::Action("quit".into()),
            };
            let _ = requests.try_send(request);
        }) {
            let _ = capture_tx.try_send(Event::Diagnostic(format!("Tray unavailable: {error:#}")));
        }
        crate::tray::set_paused(paused);
    }
    let capture_guard = capture_tx.clone();
    apply_keymap(&ui, &store.setting("keymap").unwrap_or_else(|| "{}".into()))?;
    let shortcut = crate::shortcuts::Shortcut::global(
        &store
            .setting("global_shortcut")
            .unwrap_or_else(|| "Super+V".into()),
    )?;
    let shortcut_tx = capture_tx.clone();
    std::thread::spawn(move || {
        if let Err(e) = platform::hotkey(shortcut_tx.clone(), shortcut) {
            let _ = shortcut_tx.send(Event::Diagnostic(format!("Shortcut unavailable: {e:#}")));
        }
    });
    let clipboard_watch_tx = capture_tx.clone();
    std::thread::spawn(move || platform::watch_resilient(clipboard_watch_tx));
    let mut worker = Worker {
        store,
        writer: Writer::new().ok(),
        weak,
        query: String::new(),
        filter: Filter::default(),
        entries: Vec::new(),
        snippets: Vec::new(),
        snippet_mode: false,
        selected: 0,
        target: initial_target,
        status: if platform::is_wayland() {
            "Wayland · Paste requires a remembered target and supported desktop permissions"
        } else {
            "Monitoring clipboard · local storage"
        }
        .into(),
        paused,
        session: None,
        visible: true,
        select_latest: true,
        select_first: false,
        requests: tx.clone(),
        theme_mode,
        system_light,
        theme_tx: capture_tx.clone(),
        theme_watch_started: false,
    };
    worker.start_theme_watch();
    if snapshot_settings {
        tx.send(Request::Action("settings".into()))?;
    }
    let _ = tx;
    std::thread::spawn(move || {
        let _capture_guard = capture_guard;
        if let Err(e) = worker.refresh() {
            worker.status = format!("History error: {e:#}");
            let _ = worker.refresh();
        }
        let cleanup = crossbeam_channel::tick(std::time::Duration::from_secs(3600));
        loop {
            let result = crossbeam_channel::select! {
                recv(rx)->request=>match request {Ok(request)=>worker.handle(request),Err(_)=>break},
                recv(capture_rx)->event=>match event {
                    Ok(Event::Shortcut(target))=>worker.handle(Request::Show(target)),
                    Ok(Event::Activate(token))=>worker.handle(Request::Activate(token)),
                    Ok(Event::Captured {bundle,epoch,source})=>{
                        if worker.paused || !worker.store.capture_allowed(source.as_deref()) || !platform::token_current(epoch) {continue;}
                        match worker.store.capture(&bundle,source.as_deref(),chrono::Utc::now().timestamp()) {
                            Ok(_)=>{if !worker.visible {let _=worker.store.cleanup(worker.store.retention().0,worker.store.retention().1,chrono::Utc::now().timestamp());} Ok(())},Err(e)=>Err(e)
                        }
                    },
                    Ok(Event::SystemTheme(light))=>{
                        if worker.theme_mode == 2 && worker.system_light != light {
                            worker.system_light = light;
                            worker.apply_theme();
                        }
                        Ok(())
                    },
                    Ok(Event::Diagnostic(message))=>{eprintln!("Cliprust: {message}");worker.status=message;worker.update_status()},Err(_)=>break,
                },
                recv(cleanup)->_=>if worker.visible {Ok(())} else {worker.store.cleanup(worker.store.retention().0,worker.store.retention().1,chrono::Utc::now().timestamp()).map(|_|())},
            };
            if let Err(e) = result {
                eprintln!("Cliprust: {e:#}");
                worker.status = format!("{e:#}");
                let message = worker.status.clone();
                worker.send(move |ui| ui.set_status(message.into()));
            }
        }
    });
    ui.show()?;
    ui.invoke_focus_list();
    finish_desktop_show(&ui);
    let timer = slint::Timer::default();
    if let Some(path) = snapshot {
        let weak = ui.as_weak();
        timer.start(
            slint::TimerMode::SingleShot,
            std::time::Duration::from_secs(2),
            move || {
                if let Some(ui) = weak.upgrade() {
                    match ui.window().take_snapshot() {
                        Ok(pixels) => {
                            if let Err(e) = image::save_buffer(
                                &path,
                                pixels.as_bytes(),
                                pixels.width(),
                                pixels.height(),
                                image::ColorType::Rgba8,
                            ) {
                                eprintln!("Snapshot failed: {e}");
                            }
                        }
                        Err(e) => eprintln!("Snapshot failed: {e}"),
                    }
                }
                let _ = slint::quit_event_loop();
            },
        );
    }
    let result = slint::run_event_loop_until_quit();
    crate::tray::shutdown();
    result?;
    Ok(())
}

#[cfg(unix)]
pub mod ipc {
    use super::*;
    use std::{
        io::{Read, Write},
        os::unix::fs::PermissionsExt,
        os::unix::net::{UnixListener, UnixStream},
    };
    fn socket_path(db: &Path) -> PathBuf {
        db.with_extension("sock")
    }
    pub fn show(db: &Path, target: Target, toggle: bool) -> Result<bool> {
        match UnixStream::connect(socket_path(db)) {
            Ok(mut stream) => {
                stream.write_all(&target.0.to_le_bytes())?;
                if toggle {
                    stream.write_all(b"\0toggle")?;
                } else if let Ok(token) = std::env::var("XDG_ACTIVATION_TOKEN")
                    && token.len() <= 4096
                {
                    stream.write_all(token.as_bytes())?;
                }
                Ok(true)
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::NotFound | std::io::ErrorKind::ConnectionRefused
                ) =>
            {
                Ok(false)
            }
            Err(e) => Err(e.into()),
        }
    }
    pub struct Guard(PathBuf);
    impl Drop for Guard {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    pub(super) fn listen(db: &Path, tx: Sender<Request>) -> Result<Guard> {
        let path = socket_path(db);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if path.exists() {
            if UnixStream::connect(&path).is_ok() {
                bail!("Another Cliprust instance is already starting; invoke --show again");
            }
            std::fs::remove_file(&path)?;
        }
        let listener = UnixListener::bind(&path)?;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
        let guard = Guard(path);
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(1)));
                let mut data = [0u8; 8];
                if stream.read_exact(&mut data).is_ok() {
                    let mut token = String::new();
                    if (&mut stream).take(4097).read_to_string(&mut token).is_ok()
                        && !token.is_empty()
                        && token.len() <= 4096
                    {
                        if token == "\0toggle" {
                            let _ = tx.send(Request::Toggle(Target(u64::from_le_bytes(data))));
                        } else {
                            let _ = tx.send(Request::Activate(token));
                        }
                    } else {
                        let _ = tx.send(Request::Show(Target(u64::from_le_bytes(data))));
                    }
                }
            }
        });
        Ok(guard)
    }
}
