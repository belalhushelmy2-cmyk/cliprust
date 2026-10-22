use super::Event;
use crossbeam_channel::{Sender, TrySendError};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Backoff(u32);
impl Backoff {
    fn after_disconnect(&mut self, connected_for: Duration) -> Duration {
        if connected_for >= Duration::from_secs(60) {
            self.0 = 0;
        }
        let delay = Duration::from_secs((1u64 << self.0.min(5)).min(30));
        self.0 = self.0.saturating_add(1);
        delay
    }
}

/// Retries connection failures, while successful backends remain event-driven.
pub fn watch_resilient(tx: Sender<Event>) {
    let mut backoff = Backoff::default();
    loop {
        let started = Instant::now();
        let outcome = super::watch(tx.clone());
        super::invalidate_capture_epoch();
        let delay = backoff.after_disconnect(started.elapsed());
        let reason = outcome
            .err()
            .map(|error| format!("{error:#}"))
            .unwrap_or_else(|| "event stream ended".into());
        if matches!(
            tx.try_send(Event::Diagnostic(format!(
                "Clipboard monitor disconnected: {reason}. Retrying in {}s",
                delay.as_secs()
            ))),
            Err(TrySendError::Disconnected(_))
        ) {
            break;
        }
        std::thread::sleep(delay);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn retries_back_off_and_reset_after_a_stable_connection() {
        let mut backoff = Backoff::default();
        let delays: Vec<_> = (0..9)
            .map(|_| backoff.after_disconnect(Duration::ZERO).as_secs())
            .collect();
        assert_eq!(delays, [1, 2, 4, 8, 16, 30, 30, 30, 30]);
        assert_eq!(
            backoff.after_disconnect(Duration::from_secs(60)).as_secs(),
            1
        );
        assert_eq!(backoff.after_disconnect(Duration::ZERO).as_secs(), 2);
    }
}
