//! A metadata snapshot for one visible history session. Clipboard capture and use
//! statistics continue in the store; only explicit user edits replace this snapshot.
use crate::{
    model::Entry,
    persistence::{Filter, Store},
    search,
    snippets::{self, Snippet},
};
use anyhow::Result;
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EntryKey {
    History(i64),
    Snippet(String),
}
impl EntryKey {
    pub fn token(&self) -> String {
        match self {
            Self::History(id) => format!("history:{id}"),
            Self::Snippet(name) => format!("snippet:{name}"),
        }
    }
    pub fn of(entry: &Entry) -> Self {
        if entry.id > 0 {
            Self::History(entry.id)
        } else {
            Self::Snippet(entry.identity.clone())
        }
    }
}
pub struct HistorySession {
    history: Vec<Entry>,
    pub snippets: Vec<Snippet>,
    weights: search::Weights,
    now: i64,
    pub latest: Option<i64>,
}
impl HistorySession {
    pub fn capture(store: &Store) -> Result<Self> {
        let history = store.history_snapshot()?;
        let latest = store
            .latest_captured_id()
            .filter(|id| history.iter().any(|e| e.id == *id))
            .or_else(|| {
                history
                    .iter()
                    .max_by_key(|e| (e.last_used, e.id))
                    .map(|e| e.id)
            });
        Ok(Self {
            history,
            snippets: store.snippets()?,
            weights: store.search_weights(),
            now: chrono::Utc::now().timestamp(),
            latest,
        })
    }
    pub fn list(&self, query: &str, filter: &Filter, snippet_mode: bool) -> Vec<Entry> {
        let candidates = self
            .history
            .iter()
            .filter(|e| {
                !snippet_mode
                    && (!filter.pinned || e.pinned)
                    && filter.kind.as_ref().is_none_or(|kind| *kind == e.kind)
                    && filter
                        .source
                        .as_ref()
                        .is_none_or(|source| e.source.as_ref() == Some(source))
                    && filter.since.is_none_or(|since| e.created_at >= since)
                    && filter.until.is_none_or(|until| e.created_at < until)
            })
            .cloned()
            .collect();
        let mut entries = search::rank(candidates, query, self.now, &self.weights);
        if query.trim().is_empty() {
            entries.sort_by(|a, b| {
                b.pinned
                    .cmp(&a.pinned)
                    .then_with(|| {
                        if a.pinned && b.pinned {
                            a.pin_order.cmp(&b.pin_order)
                        } else {
                            b.last_used.cmp(&a.last_used)
                        }
                    })
                    .then_with(|| b.id.cmp(&a.id))
            });
        }
        let dated = filter.since.is_some() || filter.until.is_some();
        if filter.source.is_none()
            && (snippet_mode || (filter.kind.is_none() && (!dated || filter.pinned)))
        {
            entries = snippets::rank_with_history(
                entries,
                &self.snippets,
                query,
                self.now,
                &self.weights,
                filter.pinned,
                300,
            );
        }
        entries.truncate(300);
        // A long pinned section must not crowd the opening selection out of the model.
        if query.trim().is_empty()
            && !snippet_mode
            && !filter.pinned
            && filter.kind.is_none()
            && filter.source.is_none()
            && filter.since.is_none()
            && filter.until.is_none()
            && let Some(latest) = self.history.iter().find(|e| Some(e.id) == self.latest)
            && !entries.iter().any(|e| e.id == latest.id)
        {
            if entries.len() == 300 {
                entries.pop();
            }
            entries.push(latest.clone());
        }
        entries
    }
}
