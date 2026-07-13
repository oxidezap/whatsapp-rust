//! Coalesced write-behind for the inbound Signal cache flush.
//!
//! The live receive path used to flush the whole dirty Signal cache to storage
//! once per stanza — a session re-serialize plus a SQLite transaction per
//! message, dominated by the record the ratchet re-dirties every time. Routing
//! it through here collapses a burst of receives into one flush per coalescing
//! window.
//!
//! Scope and durability model (deliberate, bounded):
//! - Only the receive path coalesces. A lost receive-side advance re-derives
//!   forward (the Double Ratchet receiving chain derives `CK_n → CK_n+1`), and
//!   a consumed one-time prekey stays buffered until its session is durable, so
//!   a crash inside the window is recoverable. The SEND path flushes
//!   synchronously before returning: reusing an outbound counter would reuse
//!   its message key + IV, so that advance must be durable before `send_message`
//!   reports success.
//! - The offline drain, retry recovery, identity-change recovery and teardown
//!   keep their own synchronous flushes: those gate acks, receipts or
//!   follow-up reads on durability and are not routed here.
//!
//! Single-flight scheduler: at most one worker exists at a time. The first
//! request arms it; requests that arrive while it runs only mark it dirty
//! (never spawn a second worker), and it re-runs one more window if so. A
//! failing flush is retried by that same worker with exponential backoff, so a
//! backend outage cannot be reset to the base delay by concurrent traffic, and
//! a generation change (reconnect/teardown) stands the worker down.

use std::sync::atomic::Ordering;

use crate::client::Client;

/// Fixed coalescing window: the worker flushes this long after being armed.
/// Small enough that the widened crash-replay gap stays negligible next to
/// network RTTs; large enough to fold a burst of receives into one write.
const SIGNAL_FLUSH_WINDOW: std::time::Duration = std::time::Duration::from_millis(25);

/// Backoff ceiling for a failing flush. The delay doubles per consecutive
/// failure up to this cap, which bounds the retry and error-log rate during a
/// long-lived backend outage.
const SIGNAL_FLUSH_RETRY_CEILING: std::time::Duration = std::time::Duration::from_secs(5);

/// A worker is alive (armed or running/retrying).
const FLUSH_RUNNING: u32 = 0b01;
/// A request arrived while the worker was mid-flush; it must run one more window.
const FLUSH_DIRTY: u32 = 0b10;

impl Client {
    /// Request a coalesced flush of the receive-path Signal cache. The first
    /// request arms a single worker; concurrent requests only mark it dirty.
    pub(crate) fn schedule_signal_flush(&self) {
        loop {
            let cur = self.signal_flush_state.load(Ordering::Acquire);
            if cur & FLUSH_RUNNING == 0 {
                // Idle → arm a worker.
                if self
                    .signal_flush_state
                    .compare_exchange_weak(cur, FLUSH_RUNNING, Ordering::AcqRel, Ordering::Acquire)
                    .is_ok()
                {
                    self.spawn_signal_flush_worker();
                    return;
                }
            } else {
                // A worker is alive: mark dirty so it runs one more window.
                if self
                    .signal_flush_state
                    .compare_exchange_weak(
                        cur,
                        cur | FLUSH_DIRTY,
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    return;
                }
            }
            // CAS lost a race; retry.
        }
    }

    fn spawn_signal_flush_worker(&self) {
        let Some(weak) = self.self_weak.get() else {
            // Constructor edge: no Arc identity to hold from a timer task, so
            // clear the arm and let the next post-construction request drive.
            self.signal_flush_state.store(0, Ordering::Release);
            return;
        };
        let weak = weak.clone();
        let runtime = self.runtime.clone();
        let generation = self.connection_generation.load(Ordering::Acquire);
        self.runtime
            .spawn(Box::pin(async move {
                let mut backoff = SIGNAL_FLUSH_WINDOW;
                loop {
                    // Hold only the Weak across the sleep so an armed worker
                    // never extends the client's lifetime.
                    runtime.sleep(backoff).await;
                    let Some(client) = weak.upgrade() else {
                        return;
                    };
                    // A reconnect/teardown owns the state now (it reset the
                    // scheduler and the cache); stand down without touching the
                    // state so a fresh worker on the new connection is intact.
                    if client.connection_generation.load(Ordering::Acquire) != generation {
                        return;
                    }
                    match client.coalesced_flush_attempt().await {
                        Ok(()) => {
                            backoff = SIGNAL_FLUSH_WINDOW;
                            // Exit only if no request arrived mid-flush. If one
                            // did (DIRTY set), the RUNNING→IDLE CAS fails; clear
                            // DIRTY and run one more window.
                            if client
                                .signal_flush_state
                                .compare_exchange(
                                    FLUSH_RUNNING,
                                    0,
                                    Ordering::AcqRel,
                                    Ordering::Acquire,
                                )
                                .is_ok()
                            {
                                return;
                            }
                            client
                                .signal_flush_state
                                .fetch_and(!FLUSH_DIRTY, Ordering::AcqRel);
                        }
                        Err(e) => {
                            // Same worker retries with a growing backoff, so
                            // concurrent traffic cannot reset it to the base
                            // delay. The cache keeps its dirty entries.
                            backoff = (backoff * 2).min(SIGNAL_FLUSH_RETRY_CEILING);
                            log::error!(
                                "Coalesced signal flush failed; retrying in {backoff:?}: {e:?}"
                            );
                        }
                    }
                }
            }))
            .detach();
    }

    /// One flush attempt of the worker loop; tests inject failures via a
    /// `cfg(test)` counter (same pattern as the commit batcher's `fail_flushes`).
    async fn coalesced_flush_attempt(&self) -> Result<(), anyhow::Error> {
        #[cfg(test)]
        {
            let remaining = self.signal_flush_test_failures.load(Ordering::Acquire);
            if remaining > 0 {
                self.signal_flush_test_failures
                    .store(remaining - 1, Ordering::Release);
                anyhow::bail!("injected coalesced-flush failure");
            }
        }
        self.flush_signal_cache_batch_safe().await
    }

    /// Reset the scheduler at connection teardown so a worker stuck in a long
    /// retry backoff on the old connection can't delay the next connection's
    /// traffic: the worker's generation guard stands it down, and this clears
    /// the arm so fresh traffic spawns a worker at the base window.
    pub(crate) fn reset_signal_flush_scheduler(&self) {
        self.signal_flush_state.store(0, Ordering::Release);
    }

    #[cfg(test)]
    pub(crate) fn signal_flush_worker_alive(&self) -> bool {
        self.signal_flush_state.load(Ordering::Acquire) & FLUSH_RUNNING != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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

    /// A burst of requests rides one armed worker and persists every dirty
    /// entry written before it.
    #[tokio::test]
    async fn burst_of_requests_coalesces_and_persists() {
        let client = crate::test_utils::create_test_client().await;

        let mut addrs = Vec::new();
        for i in 0..10 {
            addrs.push(dirty_session(&client, &format!("155500011{i:02}")));
            client.schedule_signal_flush();
        }
        assert!(client.signal_flush_worker_alive(), "burst arms one worker");

        for addr in &addrs {
            wait_for_backend_session(&client, addr).await;
        }
        // Worker exits back to idle once nothing is dirty.
        let deadline = wacore::time::Instant::now() + Duration::from_secs(1);
        while client.signal_flush_worker_alive() {
            assert!(
                wacore::time::Instant::now() < deadline,
                "worker must return to idle"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// A request after a completed worker arms a new one — later dirty state is
    /// not stranded behind an exited worker.
    #[tokio::test]
    async fn reschedule_after_worker_exit_flushes_again() {
        let client = crate::test_utils::create_test_client().await;

        let first = dirty_session(&client, "15550002001");
        client.schedule_signal_flush();
        wait_for_backend_session(&client, &first).await;
        while client.signal_flush_worker_alive() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        let second = dirty_session(&client, "15550002002");
        client.schedule_signal_flush();
        wait_for_backend_session(&client, &second).await;
    }

    /// While the worker is mid-flush, only one worker ever exists no matter how
    /// many requests pile up; the dirty flag makes it run exactly one more
    /// window rather than spawning a fleet.
    #[tokio::test]
    async fn concurrent_requests_never_spawn_a_second_worker() {
        let client = crate::test_utils::create_test_client().await;
        // Block the first flush attempt so requests pile up against a running
        // worker.
        client
            .signal_flush_test_failures
            .store(3, Ordering::Release);

        for _ in 0..200 {
            client.schedule_signal_flush();
        }
        // State only ever carries the two defined bits; RUNNING stays set once,
        // never "two workers" (which the single-flight design makes
        // unrepresentable — this asserts the invariant holds under the pile-up).
        for _ in 0..50 {
            let s = client.signal_flush_state.load(Ordering::Acquire);
            assert!(s & FLUSH_RUNNING != 0, "a worker stays armed");
            assert!(
                s & !(FLUSH_RUNNING | FLUSH_DIRTY) == 0,
                "no stray state bits"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        let addr = dirty_session(&client, "15550005001");
        wait_for_backend_session(&client, &addr).await;
    }

    /// Failed attempts re-arm and back off in the SAME worker; the dirty entry
    /// still persists once the injected failures drain.
    #[tokio::test]
    async fn failed_worker_retries_until_the_dirty_entry_persists() {
        let client = crate::test_utils::create_test_client().await;
        let addr = dirty_session(&client, "15550004001");
        client
            .signal_flush_test_failures
            .store(2, Ordering::Release);

        client.schedule_signal_flush();

        // Success comes only after both injected failures are consumed by the
        // retry loop (25 + 50 + 100 ms of backoff), proving the same worker
        // re-armed rather than stranding the entry.
        wait_for_backend_session(&client, &addr).await;
        assert_eq!(
            client.signal_flush_test_failures.load(Ordering::Acquire),
            0,
            "the retry loop consumed every injected failure"
        );
    }

    /// A generation bump (reconnect/teardown) stands the worker down instead of
    /// imposing its retry backoff on the next connection.
    #[tokio::test]
    async fn generation_bump_stands_the_worker_down() {
        let client = crate::test_utils::create_test_client().await;
        // Keep the worker failing so it stays in the retry loop across a bump.
        client
            .signal_flush_test_failures
            .store(1_000, Ordering::Release);
        dirty_session(&client, "15550006001");
        client.schedule_signal_flush();
        assert!(client.signal_flush_worker_alive());

        // Simulate teardown: bump the generation and reset the scheduler.
        client.connection_generation.fetch_add(1, Ordering::SeqCst);
        client.reset_signal_flush_scheduler();

        // The old worker exits on its next wake; the reset already cleared the
        // arm, so a fresh request spawns a new worker immediately.
        client
            .signal_flush_test_failures
            .store(0, Ordering::Release);
        let addr = dirty_session(&client, "15550006002");
        client.schedule_signal_flush();
        wait_for_backend_session(&client, &addr).await;
    }
}
