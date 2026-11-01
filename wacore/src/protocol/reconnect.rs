//! Reconnect policy with explicit time and caller-supplied jitter.

use crate::time::Instant;
use std::time::Duration;

pub const STABLE_CONNECTION_RESET: Duration = Duration::from_secs(30);
pub const MAX_BACKOFF_ATTEMPTS: u32 = 64;

pub fn should_reset_backoff(
    connected_at: Option<Instant>,
    now: Instant,
    penalty_pending: bool,
) -> bool {
    !penalty_pending
        && connected_at
            .is_some_and(|at| now.saturating_duration_since(at) >= STABLE_CONNECTION_RESET)
}

pub fn next_backoff_attempt(previous: u32) -> u32 {
    previous.saturating_add(1).min(MAX_BACKOFF_ATTEMPTS)
}

pub fn fibonacci_base_millis(attempt: u32) -> u64 {
    const MAX_MS: u64 = 900_000; // WA Web: 9e5

    let mut a: u64 = 1000;
    let mut b: u64 = 1000;
    for _ in 0..attempt {
        // Every step past the cap yields the cap again, so the loop stops at
        // the first one instead of counting out an attempt number nothing else
        // bounds. Same clamp `prekeys.rs` applies to its own retry exponent,
        // and the same reason.
        if a >= MAX_MS {
            break;
        }
        let next = a.saturating_add(b).min(MAX_MS);
        a = b;
        b = next;
    }
    a.min(MAX_MS)
}

/// A uniform sample in `0..=2 * jitter_range` preserves the existing jitter.
pub fn backoff_delay(base_millis: u64, sample: u64) -> Option<Duration> {
    let spread = base_millis / 10;
    if base_millis > 900_000 || sample > spread * 2 {
        return None;
    }
    Some(Duration::from_millis(base_millis - spread + sample))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectAction {
    Stop,
    Immediate,
    Backoff,
}

pub fn reconnect_action(stopping: bool, expected_disconnect: bool) -> ReconnectAction {
    if stopping {
        ReconnectAction::Stop
    } else if expected_disconnect {
        ReconnectAction::Immediate
    } else {
        ReconnectAction::Backoff
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fibonacci_schedule_and_jitter_endpoints() {
        for (attempt, seconds) in [
            1, 1, 2, 3, 5, 8, 13, 21, 34, 55, 89, 144, 233, 377, 610, 900,
        ]
        .into_iter()
        .enumerate()
        {
            let base = fibonacci_base_millis(attempt as u32);
            assert_eq!(base, seconds * 1000);
            assert_eq!(
                backoff_delay(base, 0),
                Some(Duration::from_millis(base - base / 10))
            );
            assert_eq!(
                backoff_delay(base, base / 5),
                Some(Duration::from_millis(base + base / 10))
            );
        }
        assert_eq!(fibonacci_base_millis(u32::MAX), 900_000);
        assert_eq!(backoff_delay(1000, 201), None);
        assert_eq!(backoff_delay(u64::MAX, 0), None);
    }
    #[test]
    fn reset_uses_monotonic_time_and_preserves_penalties() {
        let at = Instant::ZERO + Duration::from_secs(10);
        assert!(!should_reset_backoff(
            Some(at),
            at + Duration::from_secs(29),
            false
        ));
        assert!(should_reset_backoff(
            Some(at),
            at + Duration::from_secs(30),
            false
        ));
        assert!(!should_reset_backoff(
            Some(at),
            at + Duration::from_secs(31),
            true
        ));
        assert!(!should_reset_backoff(None, at, false));
        assert!(!should_reset_backoff(Some(at), Instant::ZERO, false));
        assert_eq!(next_backoff_attempt(u32::MAX), MAX_BACKOFF_ATTEMPTS);
    }
    #[test]
    fn shutdown_takes_precedence_over_expected_reconnect() {
        assert_eq!(reconnect_action(true, true), ReconnectAction::Stop);
        assert_eq!(reconnect_action(true, false), ReconnectAction::Stop);
        assert_eq!(reconnect_action(false, true), ReconnectAction::Immediate);
        assert_eq!(reconnect_action(false, false), ReconnectAction::Backoff);
    }
}
