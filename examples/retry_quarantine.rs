//! Bot-side retry-receipt quarantine, built on the [`RetryAdmission`] hook.
//!
//! WhatsApp Web processes every inbound retry receipt (no volume throttle), and
//! this SDK does the same by default. In a large group a cohort of members
//! whose sessions never establish can retry on every message, driving the
//! repair path (markForgetSenderKey, key-bundle processing, resend) at storm
//! rate. That cost only shows up at bot send volume, so the policy lives here in
//! the bot rather than in the SDK: the core stays WA Web compliant, and this
//! opt-in bounds the storm for deployments that want it.
//!
//! This mirrors the mechanism proposed in oxidezap/whatsapp-rust#982 by
//! @Salientekill: a token bucket keyed by (chat user, requester user), burst 2,
//! refill 2/day. One mark repairs a healthy member (the next send carries the
//! SKDM), so past the burst further receipts from the same pair are dropped
//! before any repair work; the refill keeps genuine recovery possible. The
//! device is excluded from the key on purpose: WA Web re-targets the whole user
//! when the primary goes cold, so all devices of a broken account share one
//! budget. Our own companion devices never reach this policy (the SDK exempts
//! `is_peer`), so their group session can always rebuild.
//!
//!   cargo run --example retry_quarantine
//!
//! Run with `RUST_LOG=debug` to see the SDK's "dropped by RetryAdmission policy"
//! line when a pair is quarantined.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::atomic::Ordering;

use log::{error, info};
use portable_atomic::AtomicU64;
use wacore::time::Instant;
use whatsapp_rust::RetryAdmission;
use whatsapp_rust::prelude::*;

/// Lazy monotonic token bucket. Pure given `now`/`burst`/`refill_per_sec`, so a
/// lowered burst clamps on the next access and an idle pair never accrues an
/// unbounded reserve.
struct TokenBucket {
    tokens: f64,
    last_refill: Instant,
}

impl TokenBucket {
    fn new(initial: f64, now: Instant) -> Self {
        Self {
            tokens: initial,
            last_refill: now,
        }
    }

    fn try_take(&mut self, now: Instant, burst: f64, refill_per_sec: f64) -> bool {
        // saturating: two receipts in the same instant give elapsed 0, never a panic.
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

/// Per-(chat user, requester user) quarantine. A `burst` of 0 disables it.
struct RetryQuarantine {
    buckets: Mutex<HashMap<(String, String), TokenBucket>>,
    burst: f64,
    refill_per_sec: f64,
    /// Capacity guard: the keyspace is O(groups x broken members), so bound it.
    /// A production policy would use an LRU / moka cache here (as #982 does with
    /// a dedicated capacity); this example just refuses new pairs past the cap,
    /// which fails open (admits) rather than growing without limit.
    max_pairs: usize,
    quarantined_total: AtomicU64,
}

impl RetryQuarantine {
    fn new(burst: u32, refill_per_day: u32, max_pairs: usize) -> Self {
        Self {
            buckets: Mutex::new(HashMap::new()),
            burst: burst as f64,
            refill_per_sec: refill_per_day as f64 / 86_400.0,
            max_pairs,
            quarantined_total: AtomicU64::new(0),
        }
    }

    /// Receipts dropped since start (your own observability, no SDK coupling).
    fn quarantined_total(&self) -> u64 {
        self.quarantined_total.load(Ordering::Relaxed)
    }
}

impl RetryAdmission for RetryQuarantine {
    fn admit(&self, chat: &Jid, requester: &Jid, _retry_count: u8) -> bool {
        if self.burst == 0.0 {
            return true;
        }
        let now = Instant::now();
        let key = (chat.user.to_string(), requester.user.to_string());

        // Synchronous, so the receive path never awaits a policy: the critical
        // section is a map lookup plus the bucket arithmetic.
        let mut buckets = self.buckets.lock().expect("quarantine mutex poisoned");
        // Fail open if we would exceed the cap with a brand-new pair, so the
        // guard never blocks repair for a pair it isn't already tracking.
        if !buckets.contains_key(&key) && buckets.len() >= self.max_pairs {
            return true;
        }
        let bucket = buckets
            .entry(key)
            .or_insert_with(|| TokenBucket::new(self.burst, now));
        let allowed = bucket.try_take(now, self.burst, self.refill_per_sec);
        drop(buckets);

        if !allowed {
            self.quarantined_total.fetch_add(1, Ordering::Relaxed);
        }
        allowed
    }
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .expect("failed to build tokio runtime");

    rt.block_on(async {
        let store = match SqliteStore::new("whatsapp.db").await {
            Ok(store) => store,
            Err(e) => {
                error!("failed to create SQLite backend: {e}");
                return;
            }
        };

        let bot = match Bot::builder()
            .with_backend(store)
            .on_qr_code(|code, _timeout| async move {
                info!("scan to pair:\n{code}");
            })
            .on_connected(|_client| async {
                info!("connected; inbound group retry receipts now pass through the quarantine");
            })
            .build()
            .await
        {
            Ok(bot) => bot,
            Err(e) => {
                error!("failed to build bot: {e}");
                return;
            }
        };

        // Opt in before connecting. Without this call the client keeps its
        // default behavior (every retry receipt admitted, matching WA Web).
        // burst 2, refill 2/day, up to 32768 tracked pairs.
        let quarantine = Arc::new(RetryQuarantine::new(2, 2, 32_768));
        if !bot.client().set_retry_admission(quarantine.clone()) {
            error!("retry admission policy was already set");
            return;
        }

        bot.run().await;
        info!(
            "quarantined {} retry receipt(s) this session",
            quarantine.quarantined_total()
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn bounds_per_pair_and_isolates_pairs() {
        // burst 2, refill 0: two attempts, then quarantined.
        let q = RetryQuarantine::new(2, 0, 100);
        let g: Jid = "123-456@g.us".parse().unwrap();
        let a: Jid = "111@lid".parse().unwrap();
        let b: Jid = "222@lid".parse().unwrap();
        assert!(q.admit(&g, &a, 1));
        assert!(q.admit(&g, &a, 1));
        assert!(!q.admit(&g, &a, 1), "third receipt quarantined");
        // A different requester in the same chat has its own budget.
        assert!(q.admit(&g, &b, 1));
        assert_eq!(q.quarantined_total(), 1);
    }

    #[test]
    fn burst_zero_disables() {
        let q = RetryQuarantine::new(0, 0, 100);
        let g: Jid = "123-456@g.us".parse().unwrap();
        let a: Jid = "111@lid".parse().unwrap();
        for _ in 0..10 {
            assert!(q.admit(&g, &a, 1));
        }
        assert_eq!(q.quarantined_total(), 0);
    }

    #[test]
    fn fails_open_past_capacity_for_new_pairs() {
        // Cap 1: the first pair is tracked and bounded, a second brand-new pair
        // is admitted rather than evicting/blocking.
        let q = RetryQuarantine::new(1, 0, 1);
        let g: Jid = "123-456@g.us".parse().unwrap();
        let a: Jid = "111@lid".parse().unwrap();
        let b: Jid = "222@lid".parse().unwrap();
        assert!(q.admit(&g, &a, 1));
        assert!(!q.admit(&g, &a, 1), "tracked pair still bounded");
        assert!(q.admit(&g, &b, 1), "new pair fails open past cap");
    }

    // The bucket is the seam where time matters; `admit` reads the real clock,
    // so the refill/clamp claims are driven here with explicit `now` values.

    #[test]
    fn refill_restores_admission_gradually() {
        // A quarantined pair must recover, but one token at a time — a refill is
        // not a reset, so a still-broken member cannot immediately storm again.
        let t0 = Instant::now();
        let (burst, refill_per_sec) = (2.0, 1.0); // 1 token/sec
        let mut b = TokenBucket::new(burst, t0);

        assert!(b.try_take(t0, burst, refill_per_sec));
        assert!(b.try_take(t0, burst, refill_per_sec));
        assert!(!b.try_take(t0, burst, refill_per_sec), "burst exhausted");

        // One refill period later exactly one token is back.
        let t1 = t0 + Duration::from_secs(1);
        assert!(b.try_take(t1, burst, refill_per_sec), "one token refilled");
        assert!(
            !b.try_take(t1, burst, refill_per_sec),
            "only one token, not a full reset"
        );
    }

    #[test]
    fn idle_does_not_accrue_beyond_burst() {
        // An idle pair must not bank an unbounded reserve: after a long quiet
        // spell it still gets only `burst` immediate attempts, so a member that
        // was silent for hours cannot burst 3600 retries at once.
        let t0 = Instant::now();
        let (burst, refill_per_sec) = (2.0, 1.0);
        let mut b = TokenBucket::new(burst, t0);

        let much_later = t0 + Duration::from_secs(3600);
        assert!(b.try_take(much_later, burst, refill_per_sec));
        assert!(b.try_take(much_later, burst, refill_per_sec));
        assert!(
            !b.try_take(much_later, burst, refill_per_sec),
            "tokens clamp at burst regardless of idle time"
        );
    }
}
