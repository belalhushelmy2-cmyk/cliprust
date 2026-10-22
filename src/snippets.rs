use chrono::{DateTime, Local};
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Snippet {
    pub name: String,
    pub body: String,
    pub tags: Vec<String>,
    pub abbreviation: String,
    #[serde(default)]
    pub pinned: bool,
}
fn search_entry(index: usize, snippet: &Snippet, searchable: bool) -> crate::model::Entry {
    crate::model::Entry {
        id: -(index as i64) - 1,
        identity: snippet.name.clone(),
        kind: "snippet".into(),
        preview: if searchable {
            format!(
                "{}\n{}\n{}\n{}",
                snippet.name,
                snippet.abbreviation,
                snippet.tags.join(" "),
                snippet.body
            )
        } else {
            snippet.name.clone()
        },
        created_at: 0,
        last_used: 0,
        copy_count: 1,
        pinned: snippet.pinned,
        pin_order: 0,
        source: None,
    }
}
pub fn matches(snippet: &Snippet, query: &str) -> bool {
    crate::search::score(
        &search_entry(0, snippet, true),
        query,
        0,
        &crate::search::Weights::default(),
    )
    .is_some()
}
/// Rank snippets and clipboard candidates together before the display limit.
/// IDs retain their original snippet indices even when filtering/reordering.
pub fn rank_with_history(
    mut history: Vec<crate::model::Entry>,
    snippets: &[Snippet],
    query: &str,
    now: i64,
    weights: &crate::search::Weights,
    pinned_only: bool,
    limit: usize,
) -> Vec<crate::model::Entry> {
    let query = crate::search::normalize(query.trim());
    let candidates = snippets
        .iter()
        .enumerate()
        .filter(|(_, snippet)| !pinned_only || snippet.pinned);
    let mut ranked = if query.is_empty() {
        history.extend(candidates.map(|(index, snippet)| search_entry(index, snippet, false)));
        history.sort_by_key(|entry| std::cmp::Reverse(entry.pinned));
        history
    } else {
        let mut scored: Vec<_> = history
            .into_iter()
            .filter_map(|entry| {
                crate::search::score_normalized(&entry, &query, now, weights)
                    .map(|score| (entry, score))
            })
            .collect();
        for (index, snippet) in candidates {
            let mut entry = search_entry(index, snippet, true);
            if let Some(score) = crate::search::score_normalized(&entry, &query, now, weights) {
                // Retain only the display name, not a second copy of every body.
                entry.preview = snippet.name.clone();
                scored.push((entry, score));
            }
        }
        crate::search::rank_scored(scored)
    };
    ranked.truncate(limit);
    ranked
}

pub fn expand(body: &str, now: DateTime<Local>, clipboard: &str) -> String {
    // Single pass: clipboard content is never recursively interpreted as a template.
    let mut out = String::new();
    let mut rest = body;
    while let Some(start) = rest.find("{{") {
        out.push_str(&rest[..start]);
        rest = &rest[start..];
        if let Some(end) = rest.find("}}") {
            match &rest[2..end] {
                "date" => out.push_str(&now.format("%Y-%m-%d").to_string()),
                "time" => out.push_str(&now.format("%H:%M").to_string()),
                "clipboard" => out.push_str(clipboard),
                _ => out.push_str(&rest[..end + 2]),
            }
            rest = &rest[end + 2..];
        } else {
            break;
        }
    }
    out.push_str(rest);
    out
}
