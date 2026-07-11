//! Coalesced write-behind for the hot-path Signal cache flushes.
//!
//! The live receive path and the send epilogue used to flush the whole dirty
//! Signal cache to storage once per stanza — a serialize + SQLite transaction
//! per message, dominated by the session record the ratchet re-dirties every
//! time. Scheduling through here collapses those into one flush per debounce
//! window: under load, one storage write covers a burst of messages.
//!
//! Durability model (deliberate, bounded):
//! - Live acks already went out BEFORE the per-stanza flush, so coalescing
//!   does not reorder acks vs durability — it widens the existing
//!   crash-replay window from "one stanza" to at most [`SIGNAL_FLUSH_DEBOUNCE`]
//!   plus one flush. Inbound receive chains re-derive forward after a lost
//!   advance; consumed one-time prekeys stay buffered until their session is
//!   durable (the flush-internal atomicity is untouched).
//! - The offline drain, retry recovery, identity-change recovery and
//!   teardown keep their synchronous flushes: those paths gate acks,
//!   receipts or follow-up reads on durability and are not routed here.
//! - Disconnect teardown settles the whole cache itself; a fire that lands
//!   afterwards flushes an empty cache (no-op).

use std::sync::atomic::Ordering;

use crate::client::Client;

/// Fixed coalescing window for the flush. The fire runs this long after the
/// FIRST request; later requests inside the window ride the same fire (the
/// deadline is deliberately not extended — a true trailing-edge debounce
/// would defer the flush indefinitely under continuous traffic, while the
/// fixed window bounds the maximum deferral). Small enough that the widened
/// crash window stays negligible next to network RTTs; large enough to fold
/// a full receive+reply cycle (and bursts) into one storage write.
const SIGNAL_FLUSH_WINDOW: std::time::Duration = std::time::Duration::from_millis(25);

impl Client {
    /// Request a Signal-cache flush without paying one storage transaction
    /// per stanza: the first request arms a fixed-window timer and every
    /// request inside the window rides the same fire.
    ///
    /// The pending flag clears BEFORE the fire's flush runs, so a request
    /// that lands mid-flush arms a new fire instead of being absorbed by a
    /// flush that may already have snapshotted the dirty set. A failed flush
    /// re-arms the window, so dirty state is retried instead of sitting
    /// unwritten until unrelated traffic schedules again.
    pub(crate) async fn schedule_signal_flush(&self) {
        if self.signal_flush_pending.swap(true, Ordering::AcqRel) {
            return;
        }
        let Some(weak) = self.self_weak.get() else {
            // Constructor edge: no Arc identity to hold from the timer task.
            // Flush inline so the request is never silently dropped.
            self.signal_flush_pending.store(false, Ordering::Release);
            self.flush_signal_cache_batch_safe_logged("coalesced-inline", None)
                .await;
            return;
        };
        let weak = weak.clone();
        let runtime = self.runtime.clone();
        self.runtime
            .spawn(Box::pin(async move {
                loop {
                    // Hold only the Weak across the sleep so an armed fire
                    // never extends the client's lifetime.
                    runtime.sleep(SIGNAL_FLUSH_WINDOW).await;
                    let Some(client) = weak.upgrade() else {
                        return;
                    };
                    client.signal_flush_pending.store(false, Ordering::Release);
                    // Batch-safe: if an offline drain became active meanwhile,
                    // this routes under the processing permit like any
                    // out-of-band flush.
                    let Err(e) = client.flush_signal_cache_batch_safe().await else {
                        return;
                    };
                    log::error!("Coalesced signal flush failed; re-arming for retry: {e:?}");
                    // Re-arm inline with the same window as the retry backoff;
                    // the cache keeps its dirty entries until a flush
                    // succeeds. If a concurrent request re-armed already, its
                    // fire owns the retry.
                    if client.signal_flush_pending.swap(true, Ordering::AcqRel) {
                        return;
                    }
                }
            }))
            .detach();
    }

    #[cfg(test)]
    pub(crate) fn signal_flush_is_pending(&self) -> bool {
        self.signal_flush_pending.load(Ordering::Acquire)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::time::Duration;

    use wacore::libsignal::protocol::{ProtocolAddress, SessionRecord};

    async fn backend_session(
        client: &Arc<crate::client::Client>,
        addr: &ProtocolAddress,
    ) -> Option<bytes::Bytes> {
        client
            .persistence_manager
            .backend()
            .get_session(addr.as_str())
            .await
            .expect("backend read")
    }

    fn dirty_session(client: &Arc<crate::client::Client>, user: &str) -> ProtocolAddress {
        let addr = ProtocolAddress::new(user.to_string(), 1.into());
        assert!(
            client
                .signal_cache
                .try_put_session(&addr, SessionRecord::new_fresh())
                .is_ok()
        );
        addr
    }

    async fn wait_for_backend_session(client: &Arc<crate::client::Client>, addr: &ProtocolAddress) {
        let deadline = wacore::time::Instant::now() + Duration::from_secs(2);
        while backend_session(client, addr).await.is_none() {
            assert!(
                wacore::time::Instant::now() < deadline,
                "scheduled flush never persisted {addr}"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// Requests inside one debounce window coalesce into a single armed fire,
    /// and that fire persists every dirty entry written before it.
    #[tokio::test]
    async fn burst_of_requests_coalesces_and_persists() {
        let client = crate::test_utils::create_test_client().await;

        let mut addrs = Vec::new();
        for i in 0..10 {
            addrs.push(dirty_session(&client, &format!("155500011{i:02}")));
            client.schedule_signal_flush().await;
        }
        assert!(
            client.signal_flush_is_pending(),
            "burst must ride one armed fire"
        );

        for addr in &addrs {
            wait_for_backend_session(&client, addr).await;
        }
        assert!(
            !client.signal_flush_is_pending(),
            "the fire must clear the pending flag"
        );
    }

    /// A request after a completed fire arms a NEW fire — the flag round-trips
    /// and later dirty state is not stranded behind an absorbed request.
    #[tokio::test]
    async fn reschedule_after_fire_flushes_again() {
        let client = crate::test_utils::create_test_client().await;

        let first = dirty_session(&client, "15550002001");
        client.schedule_signal_flush().await;
        wait_for_backend_session(&client, &first).await;

        let second = dirty_session(&client, "15550002002");
        client.schedule_signal_flush().await;
        wait_for_backend_session(&client, &second).await;
    }
}
