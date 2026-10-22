use anyhow::{Result, bail};
use std::collections::{BTreeMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Shortcut {
    pub key: String,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}
impl Shortcut {
    pub fn parse(value: &str) -> Result<Self> {
        let mut shortcut = Self {
            key: String::new(),
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        };
        for token in value
            .split('+')
            .map(|part| part.trim().to_ascii_lowercase())
        {
            let flag = match token.as_str() {
                "ctrl" => Some(&mut shortcut.ctrl),
                "alt" => Some(&mut shortcut.alt),
                "shift" => Some(&mut shortcut.shift),
                "super" | "meta" | "win" => Some(&mut shortcut.meta),
                _ => None,
            };
            if let Some(flag) = flag {
                if *flag {
                    bail!("Repeated shortcut modifier");
                }
                *flag = true;
            } else {
                if !shortcut.key.is_empty()
                    || !(token == "enter"
                        || token == "delete"
                        || (token.len() == 1 && token.as_bytes()[0].is_ascii_alphanumeric()))
                {
                    bail!("Use Ctrl, Alt, Shift, Super and a letter, digit, Enter or Delete");
                }
                shortcut.key = token;
            }
        }
        if shortcut.key.is_empty() {
            bail!("Shortcut needs a key");
        }
        if shortcut.key.len() == 1 && !shortcut.ctrl && !shortcut.alt && !shortcut.meta {
            bail!("Letter and digit shortcuts need Ctrl, Alt or Super so typing remains available");
        }
        Ok(shortcut)
    }
    pub fn global(value: &str) -> Result<Self> {
        let shortcut = Self::parse(value)?;
        if shortcut.key.len() != 1 || !(shortcut.ctrl || shortcut.alt || shortcut.meta) {
            bail!("Global shortcut needs Ctrl, Alt or Super plus a letter or digit");
        }
        Ok(shortcut)
    }
    pub fn portal_trigger(&self) -> String {
        let mut parts = Vec::new();
        if self.ctrl {
            parts.push("CTRL");
        }
        if self.alt {
            parts.push("ALT");
        }
        if self.shift {
            parts.push("SHIFT");
        }
        if self.meta {
            parts.push("LOGO");
        }
        let key = self.key.to_ascii_uppercase();
        parts.push(&key);
        parts.join("+")
    }
}
pub fn defaults() -> BTreeMap<String, String> {
    [
        ("paste", "Enter"),
        ("keep-open", "Ctrl+Enter"),
        ("copy", "Ctrl+C"),
        ("pin", "Ctrl+P"),
        ("delete", "Delete"),
        ("actions", "Ctrl+K"),
        ("search-focus", "Ctrl+F"),
    ]
    .into_iter()
    .map(|(action, key)| (action.into(), key.into()))
    .collect()
}
pub fn parse_keymap(value: &str) -> Result<BTreeMap<String, Shortcut>> {
    if value.len() > 4096 {
        bail!("Shortcut configuration is too large");
    }
    let overrides: BTreeMap<String, String> = serde_json::from_str(value)?;
    let mut bindings = defaults();
    for (action, key) in overrides {
        if !bindings.contains_key(&action) {
            bail!("Unknown shortcut action: {action}");
        }
        bindings.insert(action, key);
    }
    let mut seen = HashSet::new();
    bindings
        .into_iter()
        .map(|(action, value)| {
            let key = Shortcut::parse(&value)?;
            if key == Shortcut::parse("Ctrl+A")? {
                bail!("Ctrl+A is reserved for selecting search text");
            }
            if !seen.insert(key.clone()) {
                bail!("Conflicting shortcut: {value}");
            }
            Ok((action, key))
        })
        .collect()
}
