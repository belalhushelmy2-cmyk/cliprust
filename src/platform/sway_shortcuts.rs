//! Conservative native config editing: never replace a binding owned by another app.
use crate::shortcuts::Shortcut;
use anyhow::{Context, Result, bail};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

#[derive(Clone)]
struct Binding {
    path: PathBuf,
    line: usize,
    key: String,
    command: Vec<String>,
    simple: bool,
}
#[derive(Default)]
struct Config {
    files: BTreeMap<PathBuf, String>,
    variables: BTreeMap<String, String>,
    bindings: Vec<Binding>,
}
fn expand(value: &str, variables: &BTreeMap<String, String>) -> String {
    let mut value = value.to_owned();
    for _ in 0..16 {
        let mut next = String::new();
        let mut chars = value.chars().peekable();
        while let Some(c) = chars.next() {
            if c != '$' {
                next.push(c);
                continue;
            }
            let mut key = String::from("$");
            while chars
                .peek()
                .is_some_and(|c| c.is_ascii_alphanumeric() || *c == '_')
            {
                key.push(chars.next().unwrap());
            }
            next.push_str(variables.get(&key).unwrap_or(&key));
        }
        if next == value {
            return value;
        }
        value = next;
    }
    value
}
fn key(value: &str) -> BTreeSet<String> {
    value
        .split('+')
        .map(|s| match s.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => "control".into(),
            "super" | "meta" | "mod4" => "mod4".into(),
            "alt" | "mod1" => "mod1".into(),
            _ => s.to_ascii_lowercase(),
        })
        .collect()
}
impl Config {
    fn read(&mut self, path: &Path, stack: &mut Vec<String>) -> Result<()> {
        let path = path.canonicalize()?;
        if self.files.contains_key(&path) {
            bail!(
                "Repeated/cyclic Sway include needs manual review: {}",
                path.display()
            );
        }
        let text = fs::read_to_string(&path)?;
        self.files.insert(path.clone(), text.clone());
        for (line, raw) in text.lines().enumerate() {
            if raw.trim_end().ends_with('\\') {
                bail!(
                    "Sway line continuation needs manual review: {}:{}",
                    path.display(),
                    line + 1
                );
            }
            let w = shlex::split(raw).context("Cannot parse Sway configuration quoting")?;
            if w.is_empty() {
                continue;
            }
            if w == ["}"] {
                stack.pop();
                continue;
            }
            if w.last().is_some_and(|s| s == "{") {
                stack.push(w[0].clone());
                continue;
            }
            let block = stack.last().map(String::as_str).unwrap_or("");
            let values = if w[0] == "set" {
                &w[1..]
            } else if block == "set" {
                &w[..]
            } else {
                &[]
            };
            if values.len() >= 2 && values[0].starts_with('$') {
                self.variables.insert(
                    values[0].clone(),
                    expand(&values[1..].join(" "), &self.variables),
                );
                continue;
            }
            if w[0] == "include" {
                for value in &w[1..] {
                    let value = expand(value, &self.variables);
                    if value.contains(['$', '`']) {
                        bail!("Dynamic Sway include needs manual review");
                    }
                    let value = if let Some(rest) = value.strip_prefix("~/") {
                        std::env::var("HOME")? + "/" + rest
                    } else {
                        value
                    };
                    let pattern = if Path::new(&value).is_absolute() {
                        PathBuf::from(value)
                    } else {
                        path.parent().unwrap().join(value)
                    };
                    for child in glob::glob(pattern.to_str().context("Non-UTF8 Sway include")?)? {
                        self.read(&child?, stack)?;
                    }
                }
                continue;
            }
            let kind = if matches!(
                w[0].as_str(),
                "bindsym" | "bindcode" | "unbindsym" | "unbindcode"
            ) {
                w[0].as_str()
            } else {
                block
            };
            if !matches!(kind, "bindsym" | "bindcode" | "unbindsym" | "unbindcode") {
                continue;
            }
            let values = if kind == w[0] { &w[1..] } else { &w[..] };
            let values: Vec<_> = values
                .iter()
                .skip_while(|v| v.starts_with("--"))
                .cloned()
                .collect();
            if values.is_empty() {
                continue;
            }
            let expanded = expand(&values[0], &self.variables);
            if expanded.contains('$') || kind.contains("code") || w.iter().any(|s| s == "--to-code")
            {
                bail!(
                    "Unresolved/keycode Sway binding needs manual review: {}:{}",
                    path.display(),
                    line + 1
                );
            }
            self.bindings.push(Binding {
                path: path.clone(),
                line,
                key: expanded,
                command: values[1..].to_vec(),
                simple: stack.is_empty() && w[0] == "bindsym",
            });
        }
        Ok(())
    }
    fn edit(&self, root: &Path, wanted: &str, executable: &Path) -> Result<(PathBuf, String)> {
        let owns = |b: &Binding| {
            b.command.len() >= 3
                && b.command[0] == "exec"
                && b.command
                    .last()
                    .is_some_and(|v| v == "--toggle" || v == "--show")
                && Path::new(&b.command[1])
                    .file_name()
                    .and_then(|s| s.to_str())
                    .is_some_and(|s| {
                        s.eq_ignore_ascii_case("cliprust")
                            || s.to_ascii_lowercase().starts_with("cliprust")
                                && s.ends_with(".AppImage")
                    })
        };
        let owned: Vec<_> = self.bindings.iter().filter(|b| owns(b)).collect();
        for b in &self.bindings {
            if key(&b.key) == key(wanted) && !owns(b) {
                bail!(
                    "Shortcut already belongs to another binding at {}:{}; choose another shortcut",
                    b.path.display(),
                    b.line + 1
                );
            }
        }
        if owned.len() > 1 {
            bail!("Multiple Cliprust Sway bindings need manual review; nothing changed");
        }
        let executable = executable.to_str().context("Launcher path must be UTF-8")?;
        if executable.contains(['\n', '\r', ';', ',', '`', '$', '"', '\\']) {
            bail!("Launcher path contains characters unsupported by Sway shortcut setup");
        }
        let binding = format!("bindsym --no-repeat {wanted} exec \"{executable}\" --toggle");
        if let Some(b) = owned.first() {
            if !b.simple {
                bail!("Cliprust binding inside a Sway block needs manual review");
            }
            let mut lines: Vec<_> = self.files[&b.path].lines().map(str::to_owned).collect();
            lines[b.line] = binding;
            Ok((b.path.clone(), lines.join("\n") + "\n"))
        } else {
            Ok((
                root.to_owned(),
                format!("{}\n# Cliprust launcher\n{binding}\n", self.files[root]),
            ))
        }
    }
}
fn replace_file(path: &Path, content: &str) -> Result<()> {
    use std::io::Write;
    let temporary = path.with_file_name(format!(
        ".cliprust-{}-{}.tmp",
        std::process::id(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    let result = (|| -> Result<()> {
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)?;
        file.set_permissions(fs::metadata(path)?.permissions())?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?;
        fs::rename(&temporary, path)?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

pub fn apply(shortcut: &Shortcut, executable: &Path) -> Result<()> {
    let version = super::sway::ipc(7, "")?;
    let root = PathBuf::from(
        version["loaded_config_file_name"]
            .as_str()
            .context("Sway config path unavailable")?,
    )
    .canonicalize()?;
    let loaded = super::sway::ipc(9, "")?;
    if loaded["config"]
        .as_str()
        .context("Loaded Sway config unavailable")?
        .trim()
        != fs::read_to_string(&root)?.trim()
    {
        bail!("Sway has pending root configuration changes; reload it before editing shortcuts");
    }
    let mut config = Config::default();
    config.read(&root, &mut vec![])?;
    let mut parts = vec![];
    if shortcut.meta {
        parts.push("Mod4");
    }
    if shortcut.ctrl {
        parts.push("Control");
    }
    if shortcut.alt {
        parts.push("Mod1");
    }
    if shortcut.shift {
        parts.push("Shift");
    }
    parts.push(&shortcut.key);
    let (path, content) = config.edit(&root, &parts.join("+"), executable)?;
    let original = &config.files[&path];
    if *original == content {
        return Ok(());
    }
    let backup_dir = directories::BaseDirs::new()
        .context("User configuration directory unavailable")?
        .config_dir()
        .join("cliprust/sway-backups");
    fs::create_dir_all(&backup_dir)?;
    let backup = backup_dir.join(format!(
        "{}-{}.conf",
        path.file_name().unwrap().to_string_lossy(),
        chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    fs::copy(&path, &backup)?;
    // Check all scanned files again before touching anything.
    for (file, text) in &config.files {
        if fs::read_to_string(file)? != *text {
            bail!("Sway configuration changed during editing; try again");
        }
    }
    replace_file(&path, &content)?;
    let applied = (|| -> Result<()> {
        let status = std::process::Command::new("sway")
            .args(["--validate", "--config"])
            .arg(&root)
            .output()
            .context("Run Sway configuration validation")?;
        if !status.status.success() || String::from_utf8_lossy(&status.stderr).contains("[ERROR]") {
            bail!(
                "Sway validation failed: {}",
                String::from_utf8_lossy(&status.stderr)
            );
        }
        super::sway::command("reload")
    })();
    if let Err(error) = applied {
        replace_file(&path, original)?;
        let _ = super::sway::command("reload");
        return Err(error.context("Shortcut not applied; original configuration restored"));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn includes_variables_conflicts_and_own_binding() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("config");
        let child = t.path().join("keys");
        fs::write(&root, "set {\n $mod Mod4\n}\ninclude keys\n").unwrap();
        fs::write(&child,"bindsym {\n $mod+x exec terminal\n}\nbindsym Mod4+v exec \"/opt/Cliprust.AppImage\" --toggle\n").unwrap();
        let mut c = Config::default();
        c.read(&root, &mut vec![]).unwrap();
        assert!(
            c.edit(&root, "Mod4+x", Path::new("/opt/Cliprust.AppImage"))
                .is_err()
        );
        let (p, s) = c
            .edit(&root, "Mod4+Shift+v", Path::new("/opt/Cliprust.AppImage"))
            .unwrap();
        assert_eq!(p, child);
        assert!(s.contains("bindsym --no-repeat Mod4+Shift+v"));
        assert!(s.contains("$mod+x exec terminal"));
    }
    #[test]
    fn unknown_keycode_and_fake_cliprust_command_are_rejected() {
        let t = tempfile::tempdir().unwrap();
        let root = t.path().join("config");
        fs::write(&root, "bindcode Mod4+55 exec terminal\n").unwrap();
        assert!(Config::default().read(&root, &mut vec![]).is_err());
        fs::write(&root, "bindsym Mod4+v exec echo cliprust --toggle\n").unwrap();
        let mut c = Config::default();
        c.read(&root, &mut vec![]).unwrap();
        assert!(c.edit(&root, "Mod4+v", Path::new("/opt/cliprust")).is_err());
    }
}
