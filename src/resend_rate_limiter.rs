//! Per-chat outbound resend rate limiter.
//!
//! WhatsApp's anti-abuse penalizes the aggregate rate of outbound resends to a
//! chat, not any single device's depth. During a mass PN to LID migration,
//! hundreds of distinct devices retry the same messages, so per-device and
//! per-message caps never engage while the aggregate rate climbs into
//! AccountLocked. This bounds it with one token bucket per chat.
//!
//! A throttled resend is dropped, not queued: the requester was already marked
//! for fresh sender-key distribution earlier in the retry path, so it recovers
//! on the next send. Dropping keeps the hot path allocation-free with no timers.
//! Buckets refill lazily off the monotonic [`Instant`] (correct over long
//! sessions, immune to clock jumps); the rate is atomic so it retunes live.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use async_lock::Mutex;
use portable_atomic::AtomicU64;
use wacore::time::Instant;
use wacore_binary::jid::Jid;

use crate::cache::Cache;

/// Bucket capacity: the burst of resends allowed to one chat before the refill
/// rate gates it. Buckets start full so a chat's first activity is never
/// throttled.
pub(crate) const DEFAULT_RESEND_BURST: u32 = 20;

/// Tokens replenished per minute per chat, i.e. the sustained resend ceiling.
/// Conservative on purpose: well under the rate observed to trip AccountLocked,
/// yet above any healthy chat's steady resend need.
pub(crate) const DEFAULT_RESEND_REFILL_PER_MIN: u32 = 10;

struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    #[inline]
    fn new(initial: f64, now: Instant) -> Self {
        Self {
            tokens: initial,
            last_refill: now,
        }
    }

    /// Refill for the time since the last access, then try to take one token.
    /// Pure given `now`/`burst`/`refill_per_sec` so the rate logic is unit
    /// tested without sleeping. `tokens` is clamped to `burst`, so an idle chat
    /// cannot accumulate an unbounded reserve and a lowered `burst` takes effect
    /// on the next access.
    #[inline]
    fn try_take(&mut self, now: Instant, burst: f64, refill_per_sec: f64) -> bool {
        let elapsed = now
            .saturating_duration_since(self.last_refill)
            .as_secs_f64();
        self.tokens = (self.tokens + elapsed * refill_per_sec).min(burst);
        self.last_refill = now;
        if self.tokens >= 1.0 {
            self.tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

/// Per-chat token-bucket limiter for outbound retry resends.
pub(crate) struct ResendRateLimiter {
    /// One bucket per chat. Capacity-only: evicting an idle chat's bucket only
    /// forgives rate (it recreates full), never over-restricts.
    buckets: Cache<Jid, Arc<Mutex<TokenBucket>>>,
    burst: AtomicU32,
    refill_per_min: AtomicU32,
    throttled_total: AtomicU64,
}

impl ResendRateLimiter {
    pub(crate) fn new(capacity: u64, burst: u32, refill_per_min: u32) -> Self {
        Self {
            buckets: Cache::builder().max_capacity(capacity.max(1)).build(),
            burst: AtomicU32::new(burst),
            refill_per_min: AtomicU32::new(refill_per_min),
            throttled_total: AtomicU64::new(0),
        }
    }

    /// Retune the rate live. Takes effect on each chat's next acquire; a lowered
    /// `burst` is clamped in on that bucket's next refill.
    pub(crate) fn set_rate(&self, burst: u32, refill_per_min: u32) {
        self.burst.store(burst, Ordering::Relaxed);
        self.refill_per_min.store(refill_per_min, Ordering::Relaxed);
    }

    /// Try to consume one resend token for `chat`. `true` allows the resend,
    /// `false` drops it. A `burst` of 0 disables the limiter (always allows) and
    /// skips all bucket work.
    pub(crate) async fn try_acquire(&self, chat: &Jid) -> bool {
        let burst = self.burst.load(Ordering::Relaxed);
        if burst == 0 {
            return true;
        }
        let burst = burst as f64;
        let refill_per_sec = self.refill_per_min.load(Ordering::Relaxed) as f64 / 60.0;

        // Single-flight get-or-create so concurrent receipts for the same chat
        // (each dispatched as a detached task) share one bucket; the per-bucket
        // mutex then serializes the read-modify-write so the rate cannot be
        // bypassed by interleaving.
        let bucket = self
            .buckets
            .get_with_by_ref(chat, async move {
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

    /// Total resends dropped by the limiter since start (observability).
    pub(crate) fn throttled_total(&self) -> u64 {
        self.throttled_total.load(Ordering::Relaxed)
    }

    /// Number of chats holding a live bucket (diagnostics).
    pub(crate) fn entry_count(&self) -> u64 {
        self.buckets.entry_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn chat(s: &str) -> Jid {
        s.parse().unwrap()
    }

    // --- Pure bucket arithmetic (deterministic, no sleeps) ---

    #[test]
    fn empty_bucket_refuses_then_refills_over_time() {
        let t0 = Instant::now();
        let mut b = TokenBucket::new(0.0, t0);
        assert!(!b.try_take(t0, 10.0, 1.0), "empty bucket must refuse");

        // 1 token/sec for 3s accrues 3 tokens: three takes pass, the fourth fails.
        let t3 = t0 + Duration::from_secs(3);
        assert!(b.try_take(t3, 10.0, 1.0));
        assert!(b.try_take(t3, 10.0, 1.0));
        assert!(b.try_take(t3, 10.0, 1.0));
        assert!(
            !b.try_take(t3, 10.0, 1.0),
            "only the accrued tokens are spendable"
        );
    }

    #[test]
    fn idle_does_not_accumulate_beyond_burst() {
        let t0 = Instant::now();
        let mut b = TokenBucket::new(5.0, t0);
        for _ in 0..5 {
            assert!(b.try_take(t0, 5.0, 1.0));
        }
        assert!(!b.try_take(t0, 5.0, 1.0), "bucket drained");

        // Idle an hour at 1 token/sec would accrue thousands, but the cap is burst.
        let t1 = t0 + Duration::from_secs(3600);
        let mut allowed = 0;
        for _ in 0..100 {
            if b.try_take(t1, 5.0, 1.0) {
                allowed += 1;
            }
        }
        assert_eq!(allowed, 5, "refill is clamped to burst, not unbounded");
    }

    // --- Async limiter, happy + bad paths (refill 0 makes token count exact) ---

    #[tokio::test]
    async fn under_burst_allows_then_over_burst_refuses() {
        let limiter = ResendRateLimiter::new(100, 5, 0);
        let c = chat("123456789@g.us");
        for _ in 0..5 {
            assert!(limiter.try_acquire(&c).await, "within burst must pass");
        }
        assert!(!limiter.try_acquire(&c).await, "past burst must drop");
        assert!(!limiter.try_acquire(&c).await);
        assert_eq!(limiter.throttled_total(), 2);
    }

    #[tokio::test]
    async fn disabled_limiter_allows_everything() {
        let limiter = ResendRateLimiter::new(100, 0, 10);
        let c = chat("123456789@g.us");
        for _ in 0..100 {
            assert!(limiter.try_acquire(&c).await);
        }
        assert_eq!(
            limiter.entry_count(),
            0,
            "disabled limiter creates no buckets"
        );
        assert_eq!(limiter.throttled_total(), 0);
    }

    #[tokio::test]
    async fn buckets_are_per_chat() {
        let limiter = ResendRateLimiter::new(100, 2, 0);
        let a = chat("111@g.us");
        let b = chat("222@g.us");
        assert!(limiter.try_acquire(&a).await);
        assert!(limiter.try_acquire(&a).await);
        assert!(!limiter.try_acquire(&a).await, "a exhausted its own budget");
        assert!(limiter.try_acquire(&b).await, "b has an independent budget");
        assert!(limiter.try_acquire(&b).await);
        assert!(!limiter.try_acquire(&b).await);
    }

    #[tokio::test]
    async fn set_rate_lowers_an_existing_bucket_ceiling() {
        let limiter = ResendRateLimiter::new(100, 10, 0);
        let c = chat("123@g.us");
        // Create the bucket at burst 10 (one token spent, nine remain).
        assert!(limiter.try_acquire(&c).await);
        // Lower the ceiling: the nine remaining tokens clamp down to three.
        limiter.set_rate(3, 0);
        let mut allowed = 0;
        for _ in 0..10 {
            if limiter.try_acquire(&c).await {
                allowed += 1;
            }
        }
        assert_eq!(allowed, 3, "lowered burst clamps the live bucket");
    }

    // --- Concurrency: the rate cannot be bypassed by interleaved receipts ---

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn concurrent_acquires_for_one_chat_do_not_exceed_burst() {
        let limiter = Arc::new(ResendRateLimiter::new(100, 10, 0));
        let c = chat("123456789@g.us");
        let allowed = Arc::new(AtomicU64::new(0));

        let mut handles = Vec::new();
        for _ in 0..40 {
            let limiter = limiter.clone();
            let c = c.clone();
            let allowed = allowed.clone();
            handles.push(tokio::spawn(async move {
                if limiter.try_acquire(&c).await {
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
            "exactly burst resends pass under contention, no bypass"
        );
        assert_eq!(limiter.throttled_total(), 30);
    }
}
