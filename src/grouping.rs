use crate::model::Entry;
use chrono::{DateTime, Local, NaiveDate};

/// Stable grouping preserves search relevance and manual pin order within groups.
pub fn group(entries: &mut [Entry], mode: &str, today: NaiveDate) -> Vec<String> {
    fn key(entry: &Entry, mode: &str, today: NaiveDate) -> (u8, String) {
        match mode {
            "date" if entry.pinned => (0, "Pinned".into()),
            "date" if entry.kind == "snippet" => (4, "Snippets".into()),
            "date" => {
                let date = DateTime::from_timestamp(entry.last_used, 0)
                    .map(|value| value.with_timezone(&Local).date_naive());
                if date.is_some_and(|date| date >= today) {
                    (1, "Today".into())
                } else if date.is_some() && date == today.pred_opt() {
                    (2, "Yesterday".into())
                } else {
                    (3, "Previous days".into())
                }
            }
            "pins" => {
                if entry.pinned {
                    (0, "Pinned".into())
                } else {
                    (1, "Unpinned".into())
                }
            }
            "type" => (
                0,
                match entry.kind.as_str() {
                    "image" => "Images",
                    "files" => "Files",
                    "link" => "Links",
                    "color" => "Colors",
                    "snippet" => "Snippets",
                    _ => "Text",
                }
                .into(),
            ),
            "application" => entry
                .source
                .as_ref()
                .map(|source| (0, source.clone()))
                .unwrap_or((1, "Unknown application".into())),
            _ => (0, String::new()),
        }
    }
    entries.sort_by_cached_key(|entry| key(entry, mode, today));
    let mut previous = None;
    entries
        .iter()
        .map(|entry| {
            let current = key(entry, mode, today);
            let label = if previous.as_ref() == Some(&current) {
                String::new()
            } else {
                current.1.clone()
            };
            previous = Some(current);
            label
        })
        .collect()
}
