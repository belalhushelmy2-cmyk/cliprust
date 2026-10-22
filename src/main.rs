use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};
use cliprust::{
    model::Bundle,
    persistence::{Archive, Filter, Store},
    platform,
    snippets::Snippet,
};
use std::{
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Parser)]
#[command(version, about = "Cliprust — native local clipboard history")]
struct Cli {
    #[arg(long, global = true)]
    database: Option<PathBuf>,
    /// Show the existing window; usable from a desktop custom shortcut.
    #[arg(long)]
    show: bool,
    /// Hide if focused; otherwise summon the existing window on this workspace.
    #[arg(long, conflicts_with = "show")]
    toggle: bool,
    /// Render a PNG of the native window, then quit (visual QA).
    #[arg(long)]
    snapshot: Option<PathBuf>,
    /// Include the settings panel in a visual QA snapshot.
    #[arg(long, requires = "snapshot")]
    snapshot_settings: bool,
    #[command(subcommand)]
    command: Option<Command>,
}
#[derive(Subcommand)]
enum Command {
    /// Configure retention, privacy, theme, global_shortcut, keymap (JSON), or search_weights (JSON).
    Config {
        key: String,
        value: Option<String>,
    },
    #[command(hide = true)]
    ServeBundle,
    /// Add UTF-8 text (or '-' to read stdin).
    Add {
        text: String,
    },
    List {
        #[arg(default_value = "")]
        query: String,
        #[arg(long)]
        pinned: bool,
        #[arg(long)]
        kind: Option<String>,
    },
    Copy {
        id: i64,
        #[arg(long)]
        plain: bool,
    },
    Pin {
        id: i64,
    },
    Delete {
        id: i64,
    },
    Duplicate {
        id: i64,
    },
    Export {
        path: PathBuf,
    },
    Import {
        path: PathBuf,
    },
    Backup {
        path: PathBuf,
    },
    /// Merge history and snippets from a SQLite backup; keep current settings.
    Restore {
        path: PathBuf,
    },
    /// Load a single JSON format bundle, validate, save and optionally copy it.
    LoadBundle {
        path: PathBuf,
        #[arg(long)]
        copy: bool,
    },
    Prune {
        #[arg(long, default_value_t = 10000)]
        max: usize,
        #[arg(long, default_value_t = 30)]
        days: u32,
    },
    Doctor,
    /// Apply a launcher binding using the desktop's supported shortcut mechanism.
    SetupShortcut {
        shortcut: String,
    },
    /// Headless event-driven monitoring (Ctrl+C to stop).
    Monitor {
        #[arg(long, default_value_t = 0)]
        seconds: u64,
    },
    Snippet {
        #[command(subcommand)]
        command: SnippetCommand,
    },
}
#[derive(Subcommand)]
enum SnippetCommand {
    Save {
        name: String,
        body: String,
        #[arg(long, default_value = "")]
        tags: String,
        #[arg(long, default_value = "")]
        abbreviation: String,
    },
    List,
    Delete {
        name: String,
    },
}
fn default_db() -> Result<PathBuf> {
    let dirs = directories::ProjectDirs::from("org", "cliprust", "Cliprust")
        .context("User data directory unavailable")?;
    Ok(dirs.data_local_dir().join("history.db"))
}
fn main() {
    // Set once before spawning threads; database, WAL, exports and socket remain private.
    #[cfg(unix)]
    unsafe {
        libc::umask(0o077);
    }
    if let Err(error) = execute() {
        eprintln!("Cliprust: {error:#}");
        std::process::exit(1);
    }
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    let file = std::fs::File::open(path)?;
    if file.metadata()?.len() > 256 * 1024 * 1024 {
        bail!("Import exceeds 256 MiB safety limit");
    }
    Ok(serde_json::from_reader(std::io::BufReader::new(file))?)
}
fn execute() -> Result<()> {
    let cli = Cli::parse();
    let path = cli.database.unwrap_or(default_db()?);
    if cli.command.is_none() {
        #[cfg(feature = "desktop")]
        let target = platform::target().unwrap_or_default();
        #[cfg(all(unix, feature = "desktop"))]
        if cli.snapshot.is_none() && cliprust::app::ipc::show(&path, target, cli.toggle)? {
            return Ok(());
        }
        #[cfg(feature = "desktop")]
        return cliprust::app::run(&path, target, cli.snapshot, cli.snapshot_settings);
        #[cfg(not(feature = "desktop"))]
        bail!("This build has no desktop interface; build with default features");
    }
    if matches!(cli.command, Some(Command::ServeBundle)) {
        return platform::serve_from_stdin();
    }
    let mut store = Store::open(&path)?;
    let now = chrono::Utc::now().timestamp();
    match cli.command.unwrap() {
        Command::ServeBundle => unreachable!(),
        Command::SetupShortcut { shortcut } => {
            let parsed = cliprust::shortcuts::Shortcut::global(&shortcut)?;
            println!("{}", platform::apply_launcher_shortcut(&parsed)?);
            store.set_setting("global_shortcut", &shortcut)?;
        }
        Command::Config { key, value } => {
            if let Some(value) = value {
                match key.as_str() {
                    "max_entries" => {
                        let v: usize = value.parse()?;
                        if v > 1_000_000 {
                            bail!("max_entries must be at most 1000000")
                        }
                    }
                    "retention_days" => {
                        let v: u32 = value.parse()?;
                        if v > 36500 {
                            bail!("retention_days must be at most 36500")
                        }
                    }
                    "paused" => {
                        let _: bool = value.parse()?;
                    }
                    "grouping"
                        if matches!(
                            value.as_str(),
                            "none" | "date" | "pins" | "type" | "application"
                        ) => {}
                    "grouping" => bail!("grouping must be none, date, pins, type, or application"),
                    "theme" if matches!(value.as_str(), "dark" | "light" | "system") => {}
                    "theme" => bail!("theme must be dark, light, or system"),
                    "global_shortcut" => {
                        let global = cliprust::shortcuts::Shortcut::global(&value)?;
                        let bindings = cliprust::shortcuts::parse_keymap(
                            &store.setting("keymap").unwrap_or_else(|| "{}".into()),
                        )?;
                        if bindings.values().any(|binding| *binding == global) {
                            bail!("Global shortcut conflicts with an in-window action");
                        }
                    }
                    "keymap" => {
                        let bindings = cliprust::shortcuts::parse_keymap(&value)?;
                        let global = cliprust::shortcuts::Shortcut::global(
                            &store
                                .setting("global_shortcut")
                                .unwrap_or_else(|| "Super+V".into()),
                        )?;
                        if bindings.values().any(|binding| *binding == global) {
                            bail!("Global shortcut conflicts with an in-window action");
                        }
                    }
                    "excluded_apps" => {
                        cliprust::security::parse_exclusions(&value)?;
                    }
                    "search_weights" => {
                        let _: cliprust::search::Weights = serde_json::from_str(&value)?;
                    }
                    _ => bail!("Unknown setting: {key}"),
                }
                store.set_setting(&key, &value)?;
            } else {
                println!(
                    "{}",
                    store.setting(&key).unwrap_or_else(|| "default".into())
                );
            }
        }
        Command::Add { mut text } => {
            if text == "-" {
                text.clear();
                std::io::stdin()
                    .take(32 * 1024 * 1024 + 1)
                    .read_to_string(&mut text)?;
            }
            println!("{}", store.capture(&Bundle::text(&text), None, now)?);
        }
        Command::List {
            query,
            pinned,
            kind,
        } => {
            println!(
                "{}",
                serde_json::to_string_pretty(&store.list(
                    &query,
                    &Filter {
                        kind,
                        pinned,
                        ..Filter::default()
                    },
                    now,
                    2000
                )?)?
            );
        }
        Command::Copy { id, plain } => {
            let mut bundle = store.bundle(id)?;
            if plain {
                bundle = Bundle::text(
                    &bundle
                        .plain_text()
                        .context("No plain text representation")?,
                );
            }
            platform::copy_from_cli(&bundle)?;
            store.used(id, now)?;
        }
        Command::Pin { id } => store.pin(id)?,
        Command::Delete { id } => store.delete(id)?,
        Command::Duplicate { id } => println!("{}", store.duplicate(id, now)?),
        Command::Export { path } => {
            let file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)?;
            serde_json::to_writer_pretty(file, &store.export()?)?;
        }
        Command::Import { path } => store.import(read_json::<Archive>(&path)?)?,
        Command::Restore { path } => store.merge_backup(&path)?,
        Command::Backup { path } => {
            if path.exists() {
                bail!("Backup destination already exists");
            }
            store.backup(&path)?;
        }
        Command::LoadBundle { path, copy } => {
            let bundle: Bundle = read_json(&path)?;
            let id = store.capture(&bundle, None, now)?;
            if copy {
                platform::copy_from_cli(&bundle)?;
            }
            println!("{id}");
        }
        Command::Prune { max, days } => {
            println!("Removed {} entries", store.cleanup(max, days, now)?)
        }
        Command::Doctor => {
            println!(
                "Desktop: {}\nWayland socket environment: {}",
                std::env::var("XDG_CURRENT_DESKTOP").unwrap_or_else(|_| "unknown".into()),
                std::env::var("WAYLAND_DISPLAY")
                    .unwrap_or_else(|_| "missing (launch from the desktop session)".into())
            );
            println!(
                "Database: {}\nIntegrity: {}\nSession: {}",
                path.display(),
                store.integrity()?,
                if platform::is_wayland() {
                    "Wayland"
                } else {
                    "Windows/X11"
                }
            );
            println!(
                "Clipboard writer: {}",
                match platform::Writer::new() {
                    Ok(_) => "initialized".into(),
                    Err(e) => format!("unavailable: {e}"),
                }
            );
            println!(
                "Automatic paste: {}",
                match platform::can_paste(platform::target().unwrap_or_default()) {
                    Ok(()) => "implementation present; real target verification required".into(),
                    Err(e) => e.to_string(),
                }
            );
            println!(
                "Encryption at rest: not implemented\nWayland global shortcut: Global Shortcuts portal registration is attempted at launch; bind cliprust --show in desktop settings if unavailable\nRetention: desktop default 30 days, 10000 unpinned entries\nMax bundle: 32 MiB"
            );
        }
        Command::Monitor { seconds } => {
            let (tx, rx) = crossbeam_channel::bounded(16);
            std::thread::spawn(move || platform::watch_resilient(tx));
            let deadline = if seconds == 0 {
                None
            } else {
                Some(std::time::Instant::now() + std::time::Duration::from_secs(seconds))
            };
            loop {
                if deadline.is_some_and(|end| std::time::Instant::now() >= end) {
                    break;
                }
                let wait = deadline
                    .map(|end| end.saturating_duration_since(std::time::Instant::now()))
                    .unwrap_or(std::time::Duration::from_secs(3600));
                match rx.recv_timeout(wait) {
                    Ok(platform::Event::Shortcut(_)) => {}
                    Ok(platform::Event::Captured {
                        bundle,
                        source,
                        epoch,
                    }) => {
                        if !store.capture_allowed(source.as_deref())
                            || !platform::token_current(epoch)
                        {
                            continue;
                        }
                        let id = store.capture(
                            &bundle,
                            source.as_deref(),
                            chrono::Utc::now().timestamp(),
                        )?;
                        println!("Captured entry {id} ({})", bundle.kind());
                    }
                    Ok(platform::Event::Diagnostic(message)) => {
                        eprintln!("Clipboard diagnostic: {message}")
                    }
                    Ok(platform::Event::SystemTheme(_) | platform::Event::Activate(_)) => {}
                    Err(crossbeam_channel::RecvTimeoutError::Timeout) if deadline.is_none() => {
                        continue;
                    }
                    Err(_) => break,
                }
                if deadline.is_some_and(|d| std::time::Instant::now() >= d) {
                    break;
                }
            }
        }
        Command::Snippet { command } => match command {
            SnippetCommand::Save {
                name,
                body,
                tags,
                abbreviation,
            } => store.save_snippet(&Snippet {
                name,
                body,
                tags: tags
                    .split(',')
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned)
                    .collect(),
                abbreviation,
                pinned: false,
            })?,
            SnippetCommand::List => {
                println!("{}", serde_json::to_string_pretty(&store.snippets()?)?)
            }
            SnippetCommand::Delete { name } => store.delete_snippet(&name)?,
        },
    }
    Ok(())
}
