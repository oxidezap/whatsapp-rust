//! Pure helpers for keepalive / dead-socket detection.
//!
//! Constants and predicate functions with no runtime dependencies
//! (`self`, `Client`, spawn, sleep). The keepalive loop orchestration
//! and IQ error classification remain in `whatsapp-rust/src/keepalive.rs`
//! because `IqError` depends on `SocketError` which lives in whatsapp-rust.

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

/// Returns the number of milliseconds elapsed since a stored timestamp.
/// Returns `None` if the timestamp was never set (value 0).
pub fn ms_since(timestamp_ms: u64) -> Option<u64> {
    ms_since_at(timestamp_ms, now_ms())
}

/// Same as [`ms_since`], but against a caller-supplied `now`.
///
/// The dead-socket branch of the keepalive tick evaluates this and
/// [`is_dead_socket_at`] against one instant instead of reading the clock per
/// predicate, and a test can supply the instant outright.
pub fn ms_since_at(timestamp_ms: u64, now_ms: u64) -> Option<u64> {
    if timestamp_ms == 0 {
        return None;
    }
    Some(now_ms.saturating_sub(timestamp_ms))
}

/// The wall clock in the units these predicates compare.
pub fn now_ms() -> u64 {
    crate::time::now_millis().max(0) as u64
}

/// Checks the dead-socket condition: [`DEAD_SOCKET_TIME`] elapsed since the timer
/// was armed without a receive cancelling it.
///
/// `armed_ms` is the anchor WA Web's `deadSocketTimer.onOrBefore` keeps: the FIRST
/// send after the last receive (0 when unarmed / cancelled). It must NOT be the
/// most-recent send — anchoring there lets continued outgoing traffic keep pushing
/// the deadline out and hide a half-open socket forever. The caller feeds
/// `SessionStats::first_send_since_recv_ms`, reset to 0 on every receive
/// (`parseAndHandleStanza` → `cancel()`).
pub fn is_dead_socket(armed_ms: u64, last_received_ms: u64) -> bool {
    is_dead_socket_at(armed_ms, last_received_ms, now_ms())
}

/// Same as [`is_dead_socket`], but against a caller-supplied `now`, so the
/// decision is testable without depending on the platform clock.
pub fn is_dead_socket_at(armed_ms: u64, last_received_ms: u64, now_ms: u64) -> bool {
    // Timer not armed (never sent since the last receive).
    if armed_ms == 0 {
        return false;
    }
    // Received data after (or at) the armed instant -- timer cancelled.
    if last_received_ms >= armed_ms {
        return false;
    }
    ms_since_at(armed_ms, now_ms)
        .map(|elapsed| elapsed > DEAD_SOCKET_TIME.as_millis() as u64)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- ms_since tests --

    #[test]
    fn ms_since_never_set() {
        assert_eq!(ms_since(0), None);
    }

    #[test]
    fn ms_since_recent() {
        let now_ms = crate::time::now_millis().max(0) as u64;
        let elapsed = ms_since(now_ms).unwrap();
        assert!(elapsed < 100, "should be near-zero, got {elapsed}ms");
    }

    #[test]
    fn ms_since_stale() {
        let thirty_sec_ago = (crate::time::now_millis().max(0) as u64).saturating_sub(30_000);
        let elapsed = ms_since(thirty_sec_ago).unwrap();
        assert!(
            (29_000..=31_000).contains(&elapsed),
            "should be ~30s, got {elapsed}ms"
        );
    }

    // -- is_dead_socket tests --

    #[test]
    fn dead_socket_never_sent() {
        assert!(!is_dead_socket(0, 0));
    }

    #[test]
    fn dead_socket_received_after_send() {
        let t = crate::time::now_millis().max(0) as u64;
        assert!(!is_dead_socket(t, t + 1));
    }

    #[test]
    fn dead_socket_sent_recently() {
        let now = crate::time::now_millis().max(0) as u64;
        assert!(!is_dead_socket(now, 0));
    }

    #[test]
    fn dead_socket_sent_long_ago_no_reply() {
        let thirty_ago = (crate::time::now_millis().max(0) as u64).saturating_sub(30_000);
        assert!(is_dead_socket(thirty_ago, 0));
    }

    #[test]
    fn dead_socket_sent_long_ago_old_reply() {
        let thirty_ago = (crate::time::now_millis().max(0) as u64).saturating_sub(30_000);
        let thirty_one_ago = thirty_ago.saturating_sub(1_000);
        assert!(is_dead_socket(thirty_ago, thirty_one_ago));
    }

    #[test]
    fn dead_socket_sent_long_ago_recent_reply() {
        let thirty_ago = (crate::time::now_millis().max(0) as u64).saturating_sub(30_000);
        let one_ago = (crate::time::now_millis().max(0) as u64).saturating_sub(1_000);
        assert!(!is_dead_socket(thirty_ago, one_ago));
    }

    // -- controlled-clock boundary --

    /// The deadline itself is not yet dead; one millisecond past it is. Pinned
    /// against a supplied `now` so the boundary does not depend on how long the
    /// test took to run.
    #[test]
    fn dead_socket_boundary_is_exact() {
        let armed = 1_000_000u64;
        let dead_at = armed + DEAD_SOCKET_TIME.as_millis() as u64;
        assert!(!is_dead_socket_at(armed, 0, dead_at));
        assert!(is_dead_socket_at(armed, 0, dead_at + 1));
    }

    /// A silent socket is still detected while sends keep flowing: the anchor
    /// stays put, so elapsed time is measured from the first unanswered send.
    #[test]
    fn continued_sends_do_not_hide_a_silent_socket() {
        let stats = crate::stats::SessionStats::new();
        stats.record_frame_sent(10);
        let armed = stats.first_send_since_recv_ms();
        for _ in 0..100 {
            stats.record_frame_sent(10);
        }
        assert_eq!(stats.first_send_since_recv_ms(), armed);

        let now = armed + DEAD_SOCKET_TIME.as_millis() as u64 + 1;
        assert!(is_dead_socket_at(armed, 0, now));
    }

    // -- constant sanity --

    #[test]
    fn constants_match_wa_web() {
        assert_eq!(KEEP_ALIVE_INTERVAL_MIN, Duration::from_secs(15));
        assert_eq!(KEEP_ALIVE_INTERVAL_MAX, Duration::from_secs(30));
        assert_eq!(DEAD_SOCKET_TIME, Duration::from_secs(20));
    }
}
