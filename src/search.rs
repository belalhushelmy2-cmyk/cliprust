use crate::model::Entry;
use unicode_normalization::UnicodeNormalization;

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Weights {
    pub exact: f64,
    pub prefix: f64,
    pub substring: f64,
    pub boundary: f64,
    pub acronym: f64,
    pub fuzzy: f64,
    pub pinned: f64,
    pub frequency: f64,
    pub recency: f64,
}
impl Default for Weights {
    fn default() -> Self {
        Self {
            exact: 1000.,
            prefix: 600.,
            substring: 350.,
            boundary: 450.,
            acronym: 300.,
            fuzzy: 100.,
            pinned: 30.,
            frequency: 5.,
            recency: 20.,
        }
    }
}
pub fn normalize(text: &str) -> String {
    text.nfkc().flat_map(char::to_lowercase).collect()
}
pub fn score(entry: &Entry, query: &str, now: i64, weights: &Weights) -> Option<f64> {
    score_normalized(entry, &normalize(query.trim()), now, weights)
}
pub(crate) fn score_normalized(entry: &Entry, q: &str, now: i64, weights: &Weights) -> Option<f64> {
    let text = if q.is_empty() {
        String::new()
    } else {
        normalize(&entry.preview)
    };
    let mut relevance = 0.;
    for token in q.split_whitespace() {
        let points = if text == token {
            weights.exact
        } else if text.starts_with(token) {
            weights.prefix
        } else if text
            .split(|c: char| !c.is_alphanumeric())
            .any(|w| w.starts_with(token))
        {
            weights.boundary
        } else if text.contains(token) {
            weights.substring
        } else if text
            .split(|c: char| !c.is_alphanumeric())
            .filter_map(|w| w.chars().next())
            .collect::<String>()
            .starts_with(token)
        {
            weights.acronym
        } else {
            let mut chars = text.chars();
            if !token.chars().all(|c| chars.by_ref().any(|t| t == c)) {
                return None;
            }
            weights.fuzzy * token.chars().count() as f64 / text.chars().count().max(1) as f64
        };
        relevance += points;
    }
    let age_days = now.saturating_sub(entry.last_used).max(0) as f64 / 86400.;
    Some(
        relevance
            + if entry.pinned { weights.pinned } else { 0. }
            + (entry.copy_count.max(1) as f64).ln() * weights.frequency
            + weights.recency / (1. + age_days),
    )
}
pub fn rank(entries: Vec<Entry>, query: &str, now: i64, weights: &Weights) -> Vec<Entry> {
    let query = normalize(query.trim());
    let scored: Vec<_> = entries
        .into_iter()
        .filter_map(|entry| {
            score_normalized(&entry, &query, now, weights).map(|value| (entry, value))
        })
        .collect();
    rank_scored(scored)
}
pub(crate) fn rank_scored(mut scored: Vec<(Entry, f64)>) -> Vec<Entry> {
    scored.sort_by(|(a, av), (b, bv)| {
        bv.total_cmp(av)
            .then_with(|| b.last_used.cmp(&a.last_used))
            .then_with(|| a.id.cmp(&b.id))
    });
    scored.into_iter().map(|(entry, _)| entry).collect()
}
