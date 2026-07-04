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

/// Default burst for the per-(chat, requester) retry-receipt quarantine: how
/// many "fresh SKDM" repair attempts one member gets at full speed. One mark
/// is enough to repair a healthy member (the next send carries the SKDM), so
/// anything past a small burst is a member whose session never establishes.
pub(crate) const DEFAULT_RETRY_MARK_BURST: u32 = 2;

/// Sustained repair attempts per day per (chat, requester) once the burst is
/// spent. Keeps a genuinely-recovering member repairable (a fresh attempt
/// every ~12h) while bounding a permanently-broken member to O(1)/day instead
/// of O(group messages)/day.
pub(crate) const DEFAULT_RETRY_MARK_REFILL_PER_DAY: u32 = 2;

/// Per-(chat, requester) quarantine for inbound group retry receipts.
///
/// In large groups a cohort of members whose pairwise sessions never
/// establish (dead registrations, exhausted prekeys, re-registered LIDs)
/// sends a retry receipt for every single group message. Each receipt pays
/// `markForgetSenderKey` (a DB write plus a sender-key cache invalidation for
/// the whole group), bundle processing and possibly a resend — all upstream
/// of the per-chat resend cap, which only bounds the resend itself. Observed
/// in production at ~58k marks per 3.5 days from 468 members of a single
/// 1012-participant group. This bounds the whole repair path per member:
/// past the burst, further receipts from the same (chat, requester) are
/// dropped before any work happens; the bucket refills so real recovery is
/// still possible.
pub(crate) struct RetryMarkQuarantine {
    /// One bucket per (chat user, requester user). Capacity-only: evicting an
    /// idle pair only forgives rate (it recreates full), never over-restricts.
    buckets: Cache<(String, String), Arc<Mutex<TokenBucket>>>,
    burst: AtomicU32,
    refill_per_day: AtomicU32,
    throttled_total: AtomicU64,
}

impl RetryMarkQuarantine {
    pub(crate) fn new(capacity: u64, burst: u32, refill_per_day: u32) -> Self {
        Self {
            buckets: Cache::builder().max_capacity(capacity.max(1)).build(),
            burst: AtomicU32::new(burst),
            refill_per_day: AtomicU32::new(refill_per_day),
            throttled_total: AtomicU64::new(0),
        }
    }

    /// Retune the rate live. A `burst` of 0 disables the quarantine.
    pub(crate) fn set_rate(&self, burst: u32, refill_per_day: u32) {
        self.burst.store(burst, Ordering::Relaxed);
        self.refill_per_day.store(refill_per_day, Ordering::Relaxed);
    }

    /// Try to consume one repair token for (chat, requester). `true` lets the
    /// retry receipt through; `false` quarantines it. Device is intentionally
    /// excluded from the key so all devices of a broken account share one
    /// budget (WA Web re-targets the whole user when the primary goes cold).
    pub(crate) async fn try_acquire(&self, chat: &Jid, requester: &Jid) -> bool {
        let burst = self.burst.load(Ordering::Relaxed);
        if burst == 0 {
            return true;
        }
        let burst = burst as f64;
        let refill_per_sec = self.refill_per_day.load(Ordering::Relaxed) as f64 / 86_400.0;

        // The owned key allocates two small strings per receipt (no
        // Borrow<(&str, &str)> for (String, String)); acceptable here — this
        // runs once per retry receipt, not per message, and replaces a DB
        // write + whole-group cache invalidation when it quarantines.
        let key = (chat.user.to_string(), requester.user.to_string());
        let bucket = self
            .buckets
            .get_with(key, async move {
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

    /// Total receipts quarantined since start (observability).
    pub(crate) fn throttled_total(&self) -> u64 {
        self.throttled_total.load(Ordering::Relaxed)
    }

    /// Number of (chat, requester) pairs holding a live bucket (diagnostics).
    pub(crate) fn entry_count(&self) -> u64 {
        self.buckets.entry_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn quarantine_bounds_per_pair_and_isolates_pairs() {
        // burst 2, refill 0: two repair attempts then quarantined.
        let q = RetryMarkQuarantine::new(100, 2, 0);
        let g: Jid = "123-456@g.us".parse().unwrap();
        let a: Jid = "111@lid".parse().unwrap();
        let b: Jid = "222@lid".parse().unwrap();
        assert!(q.try_acquire(&g, &a).await);
        assert!(q.try_acquire(&g, &a).await);
        assert!(!q.try_acquire(&g, &a).await, "third receipt quarantined");
        // Another requester in the same chat has its own budget.
        assert!(q.try_acquire(&g, &b).await);
        assert_eq!(q.throttled_total(), 1);
        // burst 0 disables.
        let off = RetryMarkQuarantine::new(100, 0, 0);
        assert!(off.try_acquire(&g, &a).await);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 4)]
    async fn quarantine_concurrent_acquires_for_one_pair_do_not_exceed_burst() {
        // Mirrors the ResendRateLimiter contention test: hundreds of members
        // hammering one group is the exact storm this guards, so burst
        // enforcement must hold under concurrent receipts for the SAME pair.
        let q = Arc::new(RetryMarkQuarantine::new(100, 5, 0));
        let g: Jid = "123-456@g.us".parse().unwrap();
        let r: Jid = "999@lid".parse().unwrap();
        let allowed = Arc::new(AtomicU64::new(0));

        let mut handles = Vec::new();
        for _ in 0..40 {
            let q = q.clone();
            let g = g.clone();
            let r = r.clone();
            let allowed = allowed.clone();
            handles.push(tokio::spawn(async move {
                if q.try_acquire(&g, &r).await {
                    allowed.fetch_add(1, Ordering::Relaxed);
                }
            }));
        }
        for h in handles {
            h.await.unwrap();
        }

        assert_eq!(
            allowed.load(Ordering::Relaxed),
            5,
            "exactly burst repairs pass under contention, no bypass"
        );
        assert_eq!(q.throttled_total(), 35);
    }

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
