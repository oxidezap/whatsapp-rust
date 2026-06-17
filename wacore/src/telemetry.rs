//! Optional metrics emission via the [`metrics`](https://docs.rs/metrics) facade.
//!
//! Off by default: the `metrics` cargo feature pulls the dependency. With it off
//! every function here is an empty `#[inline]` no-op and [`Timer`] is a zero-sized
//! type that reads no clock, so there is no dependency and no runtime cost.
//!
//! The library only *emits*; the application installs a recorder (Prometheus,
//! OTLP, ...). See `examples/metrics.rs`.
//!
//! Labels are strictly low-cardinality categorical values (outcome, kind,
//! namespace, ...). Never put a JID, phone number or message id in a label: it
//! would explode the metrics backend and leak PII. Durations are unlabeled
//! histograms (the matching `_total` counter carries the categorical breakdown).

/// Histogram metric names, used with [`timer`].
pub const IQ_DURATION: &str = "wa_iq_duration_seconds";
pub const CONNECT_DURATION: &str = "wa_connect_duration_seconds";
pub const DECRYPT_DURATION: &str = "wa_decrypt_duration_seconds";
pub const SEND_DURATION: &str = "wa_send_duration_seconds";
pub const APPSTATE_SYNC_DURATION: &str = "wa_appstate_sync_duration_seconds";

#[cfg(feature = "metrics")]
mod imp {
    use metrics::{
        counter, describe_counter, describe_gauge, describe_histogram, gauge, histogram,
    };

    /// Inbound message by decrypt outcome (`decrypted`/`duplicate`/`undecryptable`/`skmsg`).
    pub fn recv(outcome: &'static str) {
        counter!("wa_recv_total", "outcome" => outcome).increment(1);
    }
    /// Outgoing send attempt by kind (`dm`/`group`/`status`).
    pub fn send(kind: &'static str) {
        counter!("wa_send_total", "kind" => kind).increment(1);
    }
    /// Retry receipt sent, by reason.
    pub fn retry_receipt(reason: &'static str) {
        counter!("wa_retry_receipt_total", "reason" => reason).increment(1);
    }
    /// Retry receipt sent at the high-retry watermark (count >= MAX), by reason.
    /// Mirrors WA Web's MessageHighRetryCount WAM event (id 3132).
    pub fn high_retry(reason: &'static str) {
        counter!("wa_high_retry_total", "reason" => reason).increment(1);
    }
    /// Retry receipt from a device not in our registry, by sender type
    /// (`primary`/`companion`). Mirrors WA Web's MdRetryFromUnknownDevice WAM (id 2178).
    pub fn retry_unknown_device(sender_type: &'static str) {
        counter!("wa_retry_unknown_device_total", "sender_type" => sender_type).increment(1);
    }
    /// Retry refused at the MAX_RETRY loop guard. Aggregate health signal for a
    /// chronically thrashing peer (WA Web logs this via WALogger.LOG, no WAM).
    pub fn retry_refused() {
        counter!("wa_retry_refused_total").increment(1);
    }
    /// Base-key collision that forced a fresh session (same base key after a
    /// re-key, so the session was deleted and recreated).
    pub fn base_key_collision() {
        counter!("wa_base_key_collision_total").increment(1);
    }
    /// IQ request completed, by result (`ok`/`timeout`/`error`). Emitted at the
    /// single request chokepoint, so it covers both raw and spec-based IQs.
    pub fn iq(result: &'static str) {
        counter!("wa_iq_total", "result" => result).increment(1);
    }
    pub fn reconnect() {
        counter!("wa_reconnect_total").increment(1);
    }
    pub fn stream_error() {
        counter!("wa_stream_error_total").increment(1);
    }
    /// Connection attempt completed, by outcome (`ok`/`fail`).
    pub fn connect(outcome: &'static str) {
        counter!("wa_connect_total", "outcome" => outcome).increment(1);
    }
    /// App-state collection sync completed, by outcome (`ok`/`fail`).
    pub fn appstate_sync(outcome: &'static str) {
        counter!("wa_appstate_sync_total", "outcome" => outcome).increment(1);
    }
    pub fn appstate_mutations(n: u64) {
        counter!("wa_appstate_mutations_total").increment(n);
    }
    /// Peer identity change that triggered a session reset (past the
    /// companion/self/no-prior-identity gates).
    pub fn identity_change() {
        counter!("wa_identity_change_total").increment(1);
    }
    /// Pre-key upload completed, by outcome (`ok`/`fail`).
    pub fn prekey_upload(outcome: &'static str) {
        counter!("wa_prekey_upload_total", "outcome" => outcome).increment(1);
    }
    /// Connected state (1 while connected, 0 otherwise).
    pub fn set_connected(on: bool) {
        gauge!("wa_connected").set(if on { 1.0 } else { 0.0 });
    }

    /// Records elapsed seconds into its histogram on drop.
    pub struct Timer {
        start: crate::time::Instant,
        name: &'static str,
    }
    impl Drop for Timer {
        fn drop(&mut self) {
            histogram!(self.name).record(self.start.elapsed().as_secs_f64());
        }
    }
    /// Start a duration timer for one of the `*_DURATION` histograms; it records
    /// on drop. Hold the returned guard for the scope of the operation.
    pub fn timer(name: &'static str) -> Timer {
        Timer {
            start: crate::time::Instant::now(),
            name,
        }
    }

    /// Register descriptions/units for all metrics. Optional; call once at startup.
    pub fn describe() {
        use metrics::Unit;
        describe_counter!(
            "wa_recv_total",
            Unit::Count,
            "Inbound messages by decrypt outcome"
        );
        describe_counter!(
            "wa_send_total",
            Unit::Count,
            "Outgoing send attempts by kind"
        );
        describe_counter!(
            "wa_retry_receipt_total",
            Unit::Count,
            "Retry receipts sent, by reason"
        );
        describe_counter!(
            "wa_high_retry_total",
            Unit::Count,
            "Retry receipts sent at the high-retry watermark (count >= MAX), by reason"
        );
        describe_counter!(
            "wa_retry_unknown_device_total",
            Unit::Count,
            "Retries from devices not in our registry, by sender type"
        );
        describe_counter!(
            "wa_retry_refused_total",
            Unit::Count,
            "Retries refused at the MAX_RETRY loop guard"
        );
        describe_counter!(
            "wa_base_key_collision_total",
            Unit::Count,
            "Base-key collisions that forced a fresh session"
        );
        describe_counter!(
            "wa_iq_total",
            Unit::Count,
            "IQ requests by result (ok/timeout/error)"
        );
        describe_counter!("wa_reconnect_total", Unit::Count, "Reconnect attempts");
        describe_counter!(
            "wa_stream_error_total",
            Unit::Count,
            "Stream errors received"
        );
        describe_counter!(
            "wa_connect_total",
            Unit::Count,
            "Connection attempts by outcome"
        );
        describe_counter!(
            "wa_appstate_sync_total",
            Unit::Count,
            "App-state syncs by outcome"
        );
        describe_counter!(
            "wa_appstate_mutations_total",
            Unit::Count,
            "App-state mutations applied"
        );
        describe_counter!(
            "wa_identity_change_total",
            Unit::Count,
            "Peer identity changes that triggered a session reset"
        );
        describe_counter!(
            "wa_prekey_upload_total",
            Unit::Count,
            "Pre-key uploads by outcome"
        );
        describe_histogram!(IQ_DURATION, Unit::Seconds, "IQ request round-trip time");
        describe_histogram!(
            CONNECT_DURATION,
            Unit::Seconds,
            "Connection establishment time"
        );
        describe_histogram!(
            DECRYPT_DURATION,
            Unit::Seconds,
            "Inbound session-decrypt batch time"
        );
        describe_histogram!(SEND_DURATION, Unit::Seconds, "Outgoing send time");
        describe_histogram!(APPSTATE_SYNC_DURATION, Unit::Seconds, "App-state sync time");
        describe_gauge!(
            "wa_connected",
            Unit::Count,
            "1 while the client is connected"
        );
    }

    use super::{
        APPSTATE_SYNC_DURATION, CONNECT_DURATION, DECRYPT_DURATION, IQ_DURATION, SEND_DURATION,
    };
}

#[cfg(not(feature = "metrics"))]
mod imp {
    #[inline]
    pub fn recv(_outcome: &'static str) {}
    #[inline]
    pub fn send(_kind: &'static str) {}
    #[inline]
    pub fn retry_receipt(_reason: &'static str) {}
    #[inline]
    pub fn high_retry(_reason: &'static str) {}
    #[inline]
    pub fn retry_unknown_device(_sender_type: &'static str) {}
    #[inline]
    pub fn retry_refused() {}
    #[inline]
    pub fn base_key_collision() {}
    #[inline]
    pub fn iq(_result: &'static str) {}
    #[inline]
    pub fn reconnect() {}
    #[inline]
    pub fn stream_error() {}
    #[inline]
    pub fn connect(_outcome: &'static str) {}
    #[inline]
    pub fn appstate_sync(_outcome: &'static str) {}
    #[inline]
    pub fn appstate_mutations(_n: u64) {}
    #[inline]
    pub fn identity_change() {}
    #[inline]
    pub fn prekey_upload(_outcome: &'static str) {}
    #[inline]
    pub fn set_connected(_on: bool) {}
    /// Zero-sized no-op timer (reads no clock, records nothing).
    pub struct Timer;
    #[inline]
    pub fn timer(_name: &'static str) -> Timer {
        Timer
    }
    #[inline]
    pub fn describe() {}
}

pub use imp::*;
