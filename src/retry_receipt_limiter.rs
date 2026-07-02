//! Per-sender inbound retry-receipt rate limiter.
//!
//! When a peer's Signal session with us is chronically broken, every message
//! they send fails to decrypt and we ask them to re-encrypt (a retry receipt).
//! A peer that never re-establishes — or a draining offline queue of messages
//! that can no longer decrypt — turns this into an unbounded outbound storm. The
//! per-message cap [`MAX_DECRYPT_RETRIES`](crate::message::MAX_DECRYPT_RETRIES)
//! bounds receipts per message id, not per sender, so a sender flooding distinct
//! ids is never gated and the aggregate climbs into AccountLocked — the same
//! anti-abuse class the per-chat [`resend_rate_limiter`](crate::resend_rate_limiter)
//! bounds for outbound resends. This bounds it with one token bucket per sender.
//!
//! A throttled receipt is dropped and the stanza is acked instead (drained from
//! the offline queue) so the server stops redelivering it: we give up on that
//! one message rather than ask forever. The session still self-heals — the
//! bucket refills lazily and any fresh handshake recovers it, so a transient
//! desync (a handful of retries, well under the burst) is never penalized.

use std::sync::Arc;
use std::sync::atomic::Ordering;

use async_lock::Mutex;
use portable_atomic::AtomicU64;
use wacore::time::Instant;
use wacore_binary::jid::Jid;

use crate::cache::Cache;
use crate::resend_rate_limiter::TokenBucket;

/// Burst of retry receipts allowed to one sender before the refill gates it.
/// Buckets start full, so a sender's first failures (a real transient desync)
/// always retry and recover; only a sustained flood crosses it.
pub(crate) const DEFAULT_RETRY_RECEIPT_BURST: u32 = 10;

/// Retry receipts replenished per minute per sender, i.e. the sustained ceiling.
/// Low on purpose: a healthy sender never needs sustained retries, a bricked one
/// is throttled to a trickle that still lets the session recover if the peer
/// fixes itself, without storming the anti-abuse signal.
pub(crate) const DEFAULT_RETRY_RECEIPT_REFILL_PER_MIN: u32 = 2;

/// Pack `burst` (high 32 bits) + `refill_per_min` (low 32 bits) into one word.
#[inline]
fn pack_rate(burst: u32, refill_per_min: u32) -> u64 {
    ((burst as u64) << 32) | refill_per_min as u64
}

#[inline]
fn unpack_rate(packed: u64) -> (u32, u32) {
    ((packed >> 32) as u32, packed as u32)
}

/// Per-sender token-bucket limiter for inbound retry receipts.
pub(crate) struct RetryReceiptLimiter {
    /// One bucket per sender. Capacity-only: evicting an idle sender's bucket
    /// only forgives rate (it recreates full), never over-restricts.
    buckets: Cache<Jid, Arc<Mutex<TokenBucket>>>,
    /// `burst` and `refill_per_min` packed into one word so a live `set_rate`
    /// publishes both as a single atomic snapshot — `try_acquire` can never read
    /// a torn (new burst, old refill) pair.
    rate: AtomicU64,
    throttled_total: AtomicU64,
}

impl RetryReceiptLimiter {
    pub(crate) fn new(capacity: u64, burst: u32, refill_per_min: u32) -> Self {
        Self {
            buckets: Cache::builder().max_capacity(capacity.max(1)).build(),
            rate: AtomicU64::new(pack_rate(burst, refill_per_min)),
            throttled_total: AtomicU64::new(0),
        }
    }

    /// Retune the rate live. Takes effect on each sender's next acquire; a
    /// lowered `burst` clamps a live bucket on its next refill.
    pub(crate) fn set_rate(&self, burst: u32, refill_per_min: u32) {
        self.rate
            .store(pack_rate(burst, refill_per_min), Ordering::Relaxed);
    }

    /// Try to consume one retry-receipt token for `sender`. `true` allows the
    /// receipt, `false` drops it (caller acks-and-drops instead). A `burst` of 0
    /// disables the limiter (always allows) and skips all bucket work.
    pub(crate) async fn try_acquire(&self, sender: &Jid) -> bool {
        // One load: burst and refill are always a consistent snapshot.
        let (burst, refill_per_min) = unpack_rate(self.rate.load(Ordering::Relaxed));
        if burst == 0 {
            return true;
        }
        let burst = burst as f64;
        let refill_per_sec = refill_per_min as f64 / 60.0;

        // Single-flight get-or-create so concurrent failures for the same sender
        // (each dispatched as a detached task) share one bucket; the per-bucket
        // mutex then serializes the read-modify-write so the rate cannot be
        // bypassed by interleaving.
        let bucket = self
            .buckets
            .get_with_by_ref(sender, async move {
                Arc::new(Mutex::new(TokenBucket::new(burst, Instant::now())))
            })
            .await;

        let allowed = bucket
            .lock()
            .await
            .try_take(Instant::now(), burst, refill_per_sec);
        if !allowed {
            self.throttled_total.fetch_add(1, Ordering::Relaxed);
        }
        allowed
    }

    /// Total retry receipts dropped by the limiter since start (observability).
    pub(crate) fn throttled_total(&self) -> u64 {
        self.throttled_total.load(Ordering::Relaxed)
    }

    /// Number of senders holding a live bucket (diagnostics).
    #[cfg_attr(not(feature = "debug-diagnostics"), allow(dead_code))]
    pub(crate) fn entry_count(&self) -> u64 {
        self.buckets.entry_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sender(s: &str) -> Jid {
        s.parse().unwrap()
    }

    #[tokio::test]
    async fn under_burst_allows_then_over_burst_refuses() {
        let limiter = RetryReceiptLimiter::new(100, 5, 0);
        let s = sender("100000000000001@lid");
        for _ in 0..5 {
            assert!(limiter.try_acquire(&s).await, "within burst must pass");
        }
        assert!(!limiter.try_acquire(&s).await, "past burst must drop");
        assert!(!limiter.try_acquire(&s).await);
        assert_eq!(limiter.throttled_total(), 2);
    }

    #[tokio::test]
    async fn disabled_limiter_allows_everything() {
        let limiter = RetryReceiptLimiter::new(100, 0, 2);
        let s = sender("100000000000001@lid");
        for _ in 0..100 {
            assert!(limiter.try_acquire(&s).await);
        }
        assert_eq!(
            limiter.entry_count(),
            0,
            "disabled limiter creates no buckets"
        );
        assert_eq!(limiter.throttled_total(), 0);
    }

    #[tokio::test]
    async fn buckets_are_per_sender() {
        let limiter = RetryReceiptLimiter::new(100, 2, 0);
        let a = sender("111@lid");
        let b = sender("222@lid");
        assert!(limiter.try_acquire(&a).await);
        assert!(limiter.try_acquire(&a).await);
        assert!(!limiter.try_acquire(&a).await, "a exhausted its own budget");
        assert!(limiter.try_acquire(&b).await, "b has an independent budget");
        assert!(limiter.try_acquire(&b).await);
        assert!(!limiter.try_acquire(&b).await);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_acquires_for_one_sender_do_not_exceed_burst() {
        let limiter = Arc::new(RetryReceiptLimiter::new(100, 10, 0));
        let s = sender("100000000000001@lid");
        let allowed = Arc::new(AtomicU64::new(0));

        let mut handles = Vec::new();
        for _ in 0..40 {
            let limiter = limiter.clone();
            let s = s.clone();
            let allowed = allowed.clone();
            handles.push(tokio::spawn(async move {
                if limiter.try_acquire(&s).await {
                    allowed.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        for h in handles {
            h.await.unwrap();
        }
        assert_eq!(
            allowed.load(Ordering::Relaxed),
            10,
            "interleaved acquires cannot exceed the burst"
        );
    }
}
