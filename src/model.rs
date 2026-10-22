use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use url::Url;

pub const MAX_BUNDLE_BYTES: usize = 32 * 1024 * 1024;
pub const MAX_FORMATS: usize = 32;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Bundle {
    pub formats: BTreeMap<String, Vec<u8>>,
}
impl Bundle {
    pub fn text(text: &str) -> Self {
        Self {
            formats: BTreeMap::from([("text/plain".into(), text.as_bytes().to_vec())]),
        }
    }
    pub fn validate(&self) -> Result<()> {
        if self.formats.is_empty() || self.formats.len() > MAX_FORMATS {
            bail!("invalid format count");
        }
        let mut total = 0usize;
        for (mime, data) in &self.formats {
            if mime.is_empty() || mime.len() > 255 || mime.chars().any(char::is_control) {
                bail!("invalid format identifier");
            }
            total = total
                .checked_add(data.len())
                .ok_or_else(|| anyhow::anyhow!("size overflow"))?;
        }
        if total == 0 || total > MAX_BUNDLE_BYTES {
            bail!("clipboard bundle is empty or exceeds 32 MiB");
        }
        Ok(())
    }
    pub fn identity(&self) -> String {
        let mut hash = blake3::Hasher::new();
        for (mime, bytes) in &self.formats {
            hash.update(&(mime.len() as u64).to_le_bytes());
            hash.update(mime.as_bytes());
            hash.update(&(bytes.len() as u64).to_le_bytes());
            hash.update(bytes);
        }
        hash.finalize().to_hex().to_string()
    }
    pub fn plain_text(&self) -> Option<String> {
        [
            "text/plain",
            "text/plain;charset=utf-8",
            "UTF8_STRING",
            "text/uri-list",
        ]
        .iter()
        .find_map(|key| {
            self.formats
                .get(*key)
                .and_then(|b| String::from_utf8(b.clone()).ok())
        })
    }
    pub fn file_paths(&self) -> Result<Option<Vec<String>>> {
        let Some(data) = self.formats.get("text/uri-list") else {
            return Ok(None);
        };
        let text = std::str::from_utf8(data).context("File references are not valid UTF-8")?;
        let mut paths = Vec::new();
        for item in text
            .lines()
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
        {
            if item.starts_with("file://") {
                let url = Url::parse(item).context("Invalid file URL")?;
                let path = url
                    .to_file_path()
                    .map_err(|_| anyhow::anyhow!("File URL cannot be converted to a local path"))?;
                paths.push(path.to_string_lossy().into_owned());
            } else if item.contains("://") {
                bail!("Only local file references can be copied as paths")
            } else {
                paths.push(item.to_owned());
            }
        }
        if paths.is_empty() {
            bail!("File list is empty")
        }
        Ok(Some(paths))
    }
    pub fn kind(&self) -> &'static str {
        if self.formats.contains_key("text/uri-list") {
            return "files";
        }
        if self.formats.keys().any(|k| k.starts_with("image/")) {
            return "image";
        }
        let text = self.plain_text().unwrap_or_default();
        let text = text.trim();
        if text.starts_with("https://") || text.starts_with("http://") {
            "link"
        } else if text.starts_with('#')
            && [4, 5, 7, 9].contains(&text.len())
            && text[1..].chars().all(|c| c.is_ascii_hexdigit())
        {
            "color"
        } else {
            "text"
        }
    }
    pub fn summary(&self) -> String {
        self.plain_text()
            .unwrap_or_else(|| {
                if self.kind() == "image" {
                    "Copied image".into()
                } else {
                    self.formats.keys().cloned().collect::<Vec<_>>().join(", ")
                }
            })
            .chars()
            .take(8192)
            .collect()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Entry {
    pub id: i64,
    pub identity: String,
    pub kind: String,
    pub preview: String,
    pub created_at: i64,
    pub last_used: i64,
    pub copy_count: i64,
    pub pinned: bool,
    pub pin_order: i64,
    pub source: Option<String>,
}

pub fn confidential(formats: &[String]) -> bool {
    formats.iter().any(|f| {
        matches!(
            f.as_str(),
            "x-kde-passwordManagerHint"
                | "application/x-keepassxc"
                | "ExcludeClipboardContentFromMonitorProcessing"
                | "org.nspasteboard.ConcealedType"
        )
    })
}
