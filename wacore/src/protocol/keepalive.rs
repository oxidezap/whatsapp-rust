//! Pure helpers for keepalive / dead-socket detection.
//!
//! Constants and predicate functions with no runtime dependencies
//! (`self`, `Client`, spawn, sleep). The keepalive loop orchestration
//! and IQ error classification remain in `whatsapp-rust/src/keepalive.rs`
//! because `IqError` depends on `SocketError` which lives in whatsapp-rust.

use crate::time::Instant;
use std::time::Duration;

/// WA Web: `healthCheckInterval = 15` -> `15 * (1 + random())` = 15-30 s.
pub const KEEP_ALIVE_INTERVAL_MIN: Duration = Duration::from_secs(15);
/// Upper bound of the randomized keepalive interval (30 s).
pub const KEEP_ALIVE_INTERVAL_MAX: Duration = Duration::from_secs(30);
/// Maximum time to wait for a keepalive pong before declaring timeout (20 s).
pub const KEEP_ALIVE_RESPONSE_DEADLINE: Duration = Duration::from_secs(20);
/// WA Web: `deadSocketTime = 20_000` -- if no data arrives for this long
/// after a send, the socket is considered dead and forcibly closed.
pub const DEAD_SOCKET_TIME: Duration = Duration::from_secs(20);

/// Time elapsed since `anchor`, or `None` when nothing is anchored.
pub fn elapsed_since(anchor: Option<Instant>) -> Option<Duration> {
    elapsed_since_at(anchor, Instant::now())
}

/// Same as [`elapsed_since`], but against a caller-supplied `now`.
///
/// The dead-socket branch of the keepalive tick evaluates this and
/// [`is_dead_socket_at`] against one instant instead of reading the clock per
/// predicate, and a test can supply the instant outright.
pub fn elapsed_since_at(anchor: Option<Instant>, now: Instant) -> Option<Duration> {
    anchor.map(|anchor| now.saturating_duration_since(anchor))
}

/// Checks the dead-socket condition: [`DEAD_SOCKET_TIME`] elapsed since the timer
/// was armed without a receive cancelling it.
///
/// Every argument is a monotonic [`Instant`], never a wall-clock timestamp: this
/// asks how much time passed, and a wall clock answers that wrongly whenever the
/// system clock is adjusted -- a laptop resuming from suspend re-syncs NTP, and
/// the jump alone used to read as twenty silent seconds and kill a healthy
/// socket seconds after it authenticated.
///
/// `armed` is the anchor WA Web's `deadSocketTimer.onOrBefore` keeps: the FIRST
/// send after the last receive (`None` when unarmed / cancelled). It must NOT be
/// the most-recent send -- anchoring there lets continued outgoing traffic keep
/// pushing the deadline out and hide a half-open socket forever. The caller feeds
/// `SessionStats::first_send_since_recv`, cleared on every receive
/// (`parseAndHandleStanza` -> `cancel()`).
pub fn is_dead_socket(armed: Option<Instant>, last_received: Option<Instant>) -> bool {
    is_dead_socket_at(armed, last_received, Instant::now())
}

/// Same as [`is_dead_socket`], but against a caller-supplied `now`, so the
/// decision is testable without depending on the platform clock.
pub fn is_dead_socket_at(
    armed: Option<Instant>,
    last_received: Option<Instant>,
    now: Instant,
) -> bool {
    // Timer not armed (never sent since the last receive).
    let Some(armed) = armed else {
        return false;
    };
    // Received data after (or at) the armed instant -- timer cancelled.
    if last_received.is_some_and(|received| received >= armed) {
        return false;
    }
    now.saturating_duration_since(armed) > DEAD_SOCKET_TIME
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(secs: u64) -> Instant {
        Instant::ZERO + Duration::from_secs(secs)
    }

    // -- elapsed_since tests --

    #[test]
    fn elapsed_since_never_set() {
        assert_eq!(elapsed_since(None), None);
    }

    #[test]
    fn elapsed_since_recent() {
        let elapsed = elapsed_since(Some(Instant::now())).unwrap();
        assert!(
            elapsed < Duration::from_millis(100),
            "should be near-zero, got {elapsed:?}"
        );
    }

    #[test]
    fn elapsed_since_stale() {
        let elapsed = elapsed_since_at(Some(at(100)), at(130)).unwrap();
        assert_eq!(elapsed, Duration::from_secs(30));
    }

    // -- is_dead_socket tests --

    #[test]
    fn dead_socket_never_sent() {
        assert!(!is_dead_socket(None, None));
    }

    #[test]
    fn dead_socket_received_after_send() {
        let t = Instant::now();
        assert!(!is_dead_socket(Some(t), Some(t + Duration::from_millis(1))));
    }

    #[test]
    fn dead_socket_sent_recently() {
        assert!(!is_dead_socket(Some(Instant::now()), None));
    }

    #[test]
    fn dead_socket_sent_long_ago_no_reply() {
        assert!(is_dead_socket_at(Some(at(100)), None, at(130)));
    }

    #[test]
    fn dead_socket_sent_long_ago_old_reply() {
        assert!(is_dead_socket_at(Some(at(100)), Some(at(99)), at(130)));
    }

    #[test]
    fn dead_socket_sent_long_ago_recent_reply() {
        assert!(!is_dead_socket_at(Some(at(100)), Some(at(129)), at(130)));
    }

    // -- controlled-clock boundary --

    /// The deadline itself is not yet dead; one millisecond past it is. Pinned
    /// against a supplied `now` so the boundary does not depend on how long the
    /// test took to run.
    #[test]
    fn dead_socket_boundary_is_exact() {
        let armed = at(1_000);
        let dead_at = armed + DEAD_SOCKET_TIME;
        assert!(!is_dead_socket_at(Some(armed), None, dead_at));
        assert!(is_dead_socket_at(
            Some(armed),
            None,
            dead_at + Duration::from_millis(1)
        ));
    }

    /// A silent socket is still detected while sends keep flowing: the anchor
    /// stays put, so elapsed time is measured from the first unanswered send.
    #[test]
    fn continued_sends_do_not_hide_a_silent_socket() {
        let stats = crate::stats::SessionStats::new();
        stats.record_frame_sent(10);
        let armed = stats.first_send_since_recv();
        for _ in 0..100 {
            stats.record_frame_sent(10);
        }
        assert_eq!(stats.first_send_since_recv(), armed);

        let now = armed.unwrap() + DEAD_SOCKET_TIME + Duration::from_millis(1);
        assert!(is_dead_socket_at(armed, None, now));
    }

    /// A socket idle for a moment is not a dead socket: a receive, then the send
    /// that arms the anchor, then one second of elapsed time.
    ///
    /// The wake-from-suspend false positive (issue #1376) is ruled out a level
    /// up rather than here: these arguments are monotonic `Instant`s, so a
    /// wall-clock timestamp no longer type-checks into this predicate, and the
    /// anchors that feed it are pinned to the monotonic clock by
    /// `wire_bookkeeping_reads_the_clock_only_where_a_value_is_used`.
    #[test]
    fn a_briefly_idle_socket_is_not_dead() {
        let received = at(1_000);
        let armed = received + Duration::from_millis(1);
        assert!(!is_dead_socket_at(
            Some(armed),
            Some(received),
            armed + Duration::from_secs(1)
        ));
    }

    /// The other half of the regression: real silence past the deadline still
    /// forces a reconnect, so the fix is not "disable the watchdog".
    #[test]
    fn real_silence_past_the_deadline_is_still_a_dead_socket() {
        let received = at(1_000);
        let armed = received + Duration::from_millis(1);
        assert!(is_dead_socket_at(
            Some(armed),
            Some(received),
            armed + DEAD_SOCKET_TIME + Duration::from_secs(1)
        ));
    }

    // -- constant sanity --

    #[test]
    fn constants_match_wa_web() {
        assert_eq!(KEEP_ALIVE_INTERVAL_MIN, Duration::from_secs(15));
        assert_eq!(KEEP_ALIVE_INTERVAL_MAX, Duration::from_secs(30));
        assert_eq!(DEAD_SOCKET_TIME, Duration::from_secs(20));
    }
}
