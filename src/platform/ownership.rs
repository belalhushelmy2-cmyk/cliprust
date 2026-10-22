//! Per-process offer identity, never a payload hash or a time-based exclusion.
//! Register the identity before publishing so even immediate notifications are safe.
use std::sync::{
    OnceLock,
    atomic::{AtomicU64, Ordering},
};
static PREFIX: OnceLock<String> = OnceLock::new();
static SEQUENCE: AtomicU64 = AtomicU64::new(0);
fn prefix() -> &'static str {
    PREFIX.get_or_init(|| {
        format!(
            "application/x-cliprust-owner-{}-{}-",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos()
        )
    })
}
pub(super) fn marker() -> String {
    format!("{}{}", prefix(), SEQUENCE.fetch_add(1, Ordering::Relaxed))
}
pub(super) fn is_own_offer(formats: &[String]) -> bool {
    formats.iter().any(|format| {
        format.strip_prefix(prefix()).is_some_and(|suffix| {
            suffix
                .parse::<u64>()
                .is_ok_and(|sequence| sequence < SEQUENCE.load(Ordering::Relaxed))
        })
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn suppresses_only_our_published_offers_not_identical_external_content() {
        let first = marker();
        let second = marker();
        assert!(is_own_offer(&["text/plain".into(), first]));
        assert!(is_own_offer(&["image/png".into(), second]));
        assert!(!is_own_offer(&["text/plain".into()]));
        assert!(!is_own_offer(&[
            "application/x-cliprust-owner-other-process-0".into()
        ]));
        assert!(!is_own_offer(&[format!("{}{}", prefix(), u64::MAX)]));
    }
}
