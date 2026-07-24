//! Coalesced write-behind for the inbound Signal cache flush.
//!
//! The live receive path used to flush the whole dirty Signal cache to storage
//! once per stanza — a session re-serialize plus a SQLite transaction per
//! message, dominated by the record the ratchet re-dirties every time. Routing
//! it through here collapses a burst of receives into one flush per coalescing
//! window.
//!
//! Scope and durability model (deliberate, bounded):
//! - The receive path coalesces: a lost receive-side advance re-derives
//!   forward (the Double Ratchet receiving chain derives `CK_n → CK_n+1`), and
//!   a consumed one-time prekey stays buffered until its session is durable, so
//!   a crash inside the window is recoverable. The SEND path coalesces too
//!   whenever its advance is covered by a durable counter lease (see
//!   `SessionRecord::reserve_sender_chain_counters`,
//!   `SenderKeyRecord::reserve_iterations`, and
//!   `Client::persist_signal_state_pre_wire`); only a send that raises either
//!   lease still flushes
//!   synchronously before the wire, because reusing an outbound counter would
//!   reuse its message key + IV.
//! - The offline drain, retry recovery, identity-change recovery and teardown
//!   keep their own synchronous flushes: those gate acks, receipts or
//!   follow-up reads on durability and are not routed here.
//!
//! Single-flight scheduler: at most one worker per generation. The first
//! request arms it; requests that arrive while it runs only mark it dirty
//! (never spawn a second worker for that generation), and it re-runs one more
//! window if so. A failing flush is retried by that same worker with
//! exponential backoff, so a backend outage cannot be reset to the base delay
//! by concurrent traffic. Across a reconnect a stale worker and the new
//! generation's worker can briefly coexist; the stale one stands down at its
//! generation check. Actual backend writes are additionally serialized against
//! teardown's cache settle by the `signal_flush_lifecycle` gate, so at most one
//! flush is ever mid-write — see `Client::signal_flush_lifecycle`.

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

// Scheduler state is `(connection_generation << 2) | flags`. Embedding the
// generation makes worker ownership generation-scoped: a worker from an old
// connection cannot mutate state a new-connection worker owns, because its CAS
// targets its own generation's exact value. So a reconnect during an in-flight
// flush needs no teardown reset — the next request on the new generation takes
// over via CAS, and the stale worker retires when it sees a foreign generation.
const FLUSH_RUNNING: u64 = 0b01;
/// A request arrived while the worker was mid-flush; it must run one more window.
const FLUSH_DIRTY: u64 = 0b10;
#[cfg(test)]
const FLUSH_FLAGS: u64 = FLUSH_RUNNING | FLUSH_DIRTY;

#[inline]
fn pack_flush_state(generation: u64, flags: u64) -> u64 {
    (generation << 2) | flags
}

impl Client {
    /// Request a coalesced flush of the receive-path Signal cache for the
    /// caller's already-validated `lane_generation`. The first request for the
    /// live generation arms a single worker; concurrent requests only mark it
    /// dirty. A request from a torn-down connection is a no-op — it can neither
    /// arm a worker for a dead generation nor move the scheduler backwards.
    pub(crate) fn schedule_signal_flush(&self, lane_generation: u64) {
        loop {
            // A stale caller (its connection was torn down) must not touch the
            // scheduler; the live generation is the source of truth.
            if self.connection_generation.load(Ordering::Acquire) != lane_generation {
                return;
            }
            let cur = self.signal_flush_state.load(Ordering::Acquire);
            let cur_gen = cur >> 2;
            // The state never moves to an older generation: a newer worker
            // already owns it, so this request is redundant.
            if cur_gen > lane_generation {
                return;
            }
            let running_this_generation = cur_gen == lane_generation && cur & FLUSH_RUNNING != 0;
            if !running_this_generation {
                // Idle, or only an older generation's leftover state → take over
                // for the live generation.
                if self
                    .signal_flush_state
                    .compare_exchange_weak(
                        cur,
                        pack_flush_state(lane_generation, FLUSH_RUNNING),
                        Ordering::AcqRel,
                        Ordering::Acquire,
                    )
                    .is_ok()
                {
                    self.spawn_signal_flush_worker(lane_generation);
                    return;
                }
            } else {
                // A worker for this generation is alive: mark dirty so it runs
                // one more window.
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

    fn spawn_signal_flush_worker(&self, generation: u64) {
        let Some(weak) = self.self_weak.get() else {
            // Constructor edge: no Arc identity to hold from a timer task, so
            // release the arm and let the next post-construction request drive.
            self.signal_flush_state
                .store(pack_flush_state(generation, 0), Ordering::Release);
            return;
        };
        let weak = weak.clone();
        let runtime = self.runtime.clone();
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
                    // Take the lifecycle barrier for the flush, then re-check the
                    // generation under it. Teardown holds this same gate while it
                    // settles the cache, so either we got here first (and flush
                    // this generation's state before any settle) or teardown ran
                    // and the generation moved (stand down). Without the gate the
                    // bare generation check races: we could pass it, get preempted,
                    // and resume the flush after teardown settled and the next
                    // connection's drain dirtied the cache — persisting rowless
                    // advances out of band.
                    let flush_result = {
                        let _gate = client.signal_flush_lifecycle.lock().await;
                        if client.connection_generation.load(Ordering::Acquire) != generation {
                            return;
                        }
                        client.coalesced_flush_attempt().await
                    };
                    match flush_result {
                        Ok(()) => {
                            backoff = SIGNAL_FLUSH_WINDOW;
                            // Settle ownership under a CAS scoped to our
                            // generation: exit to idle if nothing is dirty, run
                            // one more window if it is, or stand down if a
                            // reconnect handed the state to a new generation.
                            loop {
                                let cur = client.signal_flush_state.load(Ordering::Acquire);
                                if (cur >> 2) != generation {
                                    return;
                                }
                                let next = if cur & FLUSH_DIRTY != 0 {
                                    pack_flush_state(generation, FLUSH_RUNNING)
                                } else {
                                    pack_flush_state(generation, 0)
                                };
                                if client
                                    .signal_flush_state
                                    .compare_exchange_weak(
                                        cur,
                                        next,
                                        Ordering::AcqRel,
                                        Ordering::Acquire,
                                    )
                                    .is_ok()
                                {
                                    if next & FLUSH_RUNNING == 0 {
                                        return;
                                    }
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            // A reconnect handed the state to a new generation:
                            // stand down instead of imposing a stale backoff.
                            if (client.signal_flush_state.load(Ordering::Acquire) >> 2)
                                != generation
                            {
                                return;
                            }
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
            self.signal_flush_test_in_attempt
                .fetch_add(1, Ordering::AcqRel);
            while self.signal_flush_test_block.load(Ordering::Acquire) {
                tokio::task::yield_now().await;
            }
            let remaining = self.signal_flush_test_failures.load(Ordering::Acquire);
            if remaining > 0 {
                self.signal_flush_test_failures
                    .store(remaining - 1, Ordering::Release);
                anyhow::bail!("injected coalesced-flush failure");
            }
        }
        self.flush_signal_cache_batch_safe().await
    }

    #[cfg(test)]
    pub(crate) fn signal_flush_worker_alive(&self) -> bool {
        self.signal_flush_state.load(Ordering::Acquire) & FLUSH_RUNNING != 0
    }

    /// Schedule for the live generation, as the receive path does with its
    /// validated `lane_generation`.
    #[cfg(test)]
    pub(crate) fn schedule_signal_flush_live(&self) {
        self.schedule_signal_flush(self.connection_generation.load(Ordering::Acquire));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use std::time::Duration;

    use wacore::libsignal::protocol::{ProtocolAddress, SessionRecord};

    async fn backend_session(client: &Arc<Client>, addr: &ProtocolAddress) -> Option<bytes::Bytes> {
        client
            .persistence_manager
            .backend()
            .get_session(addr.as_str())
            .await
            .expect("backend read")
    }

    fn dirty_session(client: &Arc<Client>, user: &str) -> ProtocolAddress {
        let addr = ProtocolAddress::new(user.to_string(), 1.into());
        assert!(
            client
                .signal_cache
                .try_put_session(&addr, SessionRecord::new_fresh())
                .is_ok()
        );
        addr
    }

    async fn wait_for_backend_session(client: &Arc<Client>, addr: &ProtocolAddress) {
        let deadline = wacore::time::Instant::now() + Duration::from_secs(2);
        while backend_session(client, addr).await.is_none() {
            assert!(
                wacore::time::Instant::now() < deadline,
                "scheduled flush never persisted {addr}"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// The send-path gate: a raised counter lease flushes synchronously
    /// before returning; lease-covered dirty state only schedules the
    /// coalesced worker and still lands within its window.
    #[tokio::test]
    async fn pre_wire_gate_flushes_leases_synchronously_and_coalesces_the_rest() {
        let client = crate::test_utils::create_test_client().await;

        // Lease-covered advance (no raised reservation): no synchronous write —
        // the session is still absent from the backend when the gate returns.
        let covered = dirty_session(&client, "15550003001");
        client.persist_signal_state_pre_wire().await.unwrap();
        assert!(
            backend_session(&client, &covered).await.is_none(),
            "a covered advance must not flush synchronously"
        );
        // ...but it rides the coalescer and still becomes durable.
        wait_for_backend_session(&client, &covered).await;

        // A raised lease must be durable when the call returns.
        let addr = ProtocolAddress::new("15550003002".to_string(), 1.into());
        let mut record = SessionRecord::new_fresh();
        record.reserve_sender_chain_counters(0);
        assert!(client.signal_cache.try_put_session(&addr, record).is_ok());
        client.persist_signal_state_pre_wire().await.unwrap();
        assert!(
            backend_session(&client, &addr).await.is_some(),
            "the leased session must be durable when the gate returns"
        );
    }

    /// A burst of requests rides one armed worker and persists every dirty
    /// entry written before it.
    #[tokio::test]
    async fn burst_of_requests_coalesces_and_persists() {
        let client = crate::test_utils::create_test_client().await;

        let mut addrs = Vec::new();
        for i in 0..10 {
            addrs.push(dirty_session(&client, &format!("155500011{i:02}")));
            client.schedule_signal_flush_live();
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
        client.schedule_signal_flush_live();
        wait_for_backend_session(&client, &first).await;
        while client.signal_flush_worker_alive() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }

        let second = dirty_session(&client, "15550002002");
        client.schedule_signal_flush_live();
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
            client.schedule_signal_flush_live();
        }
        // RUNNING stays set for generation 0 the whole time — never a second
        // worker; only the two flag bits are ever set (generation 0 leaves the
        // high bits clear), so the pile-up never mints stray state.
        for _ in 0..50 {
            let s = client.signal_flush_state.load(Ordering::Acquire);
            assert!(s & FLUSH_RUNNING != 0, "a worker stays armed");
            assert_eq!(s & !FLUSH_FLAGS, 0, "generation 0: no stray high bits");
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

        client.schedule_signal_flush_live();

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

    /// A generation bump (reconnect/teardown) hands scheduler ownership to the
    /// next connection without an explicit reset: the new request takes over
    /// via a generation-scoped CAS while the stale worker is still looping, and
    /// the stale worker cannot clobber the new worker's state.
    #[tokio::test]
    async fn generation_bump_hands_off_without_clobbering() {
        let client = crate::test_utils::create_test_client().await;
        // Keep the old worker failing so it stays in the retry loop across the
        // bump (a stale worker that could still reach its exit CAS).
        client
            .signal_flush_test_failures
            .store(1_000, Ordering::Release);
        dirty_session(&client, "15550006001");
        client.schedule_signal_flush_live();
        assert!(client.signal_flush_worker_alive());

        // Bump the generation as teardown does — no explicit scheduler reset.
        client.connection_generation.fetch_add(1, Ordering::SeqCst);
        // The next connection's first request takes over for the new
        // generation; stop injecting failures so its worker succeeds.
        client
            .signal_flush_test_failures
            .store(0, Ordering::Release);
        let addr = dirty_session(&client, "15550006002");
        client.schedule_signal_flush_live();
        wait_for_backend_session(&client, &addr).await;

        // The state must be tagged with the new generation, proving the
        // hand-off (and that the stale worker did not reclaim it).
        let new_gen = client.connection_generation.load(Ordering::Acquire);
        let deadline = wacore::time::Instant::now() + Duration::from_secs(1);
        loop {
            let s = client.signal_flush_state.load(Ordering::Acquire);
            if s >> 2 == new_gen {
                break;
            }
            assert!(
                wacore::time::Instant::now() < deadline,
                "scheduler state must carry the new generation, got {s:#x}"
            );
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }

    /// The race the generation-scoped CAS closes: an old worker held INSIDE the
    /// flush while a reconnect hands the scheduler to a new-generation worker
    /// must not clobber the new worker's state when it finally completes.
    #[tokio::test]
    async fn stale_worker_inside_flush_cannot_clobber_new_generation() {
        let client = crate::test_utils::create_test_client().await;

        // Hold every flush attempt inside the flush until we release it.
        client
            .signal_flush_test_block
            .store(true, Ordering::Release);

        // Arm worker A on generation 0 and wait until it is inside the flush.
        dirty_session(&client, "15550007001");
        client.schedule_signal_flush_live();
        let deadline = wacore::time::Instant::now() + Duration::from_secs(1);
        while client.signal_flush_test_in_attempt.load(Ordering::Acquire) == 0 {
            assert!(
                wacore::time::Instant::now() < deadline,
                "worker A must enter the flush"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }

        // Reconnect: bump the generation, then a new request takes over and
        // arms worker B on the new generation (also blocked in the flush).
        let new_gen = client.connection_generation.fetch_add(1, Ordering::SeqCst) + 1;
        dirty_session(&client, "15550007002");
        client.schedule_signal_flush_live();
        let s = client.signal_flush_state.load(Ordering::Acquire);
        assert_eq!(s >> 2, new_gen, "worker B took over for the new generation");

        // Release both. The stale worker A completes its flush and tries to
        // settle ownership — its generation-scoped CAS must fail, leaving B's
        // state intact rather than reverting to generation 0.
        client
            .signal_flush_test_block
            .store(false, Ordering::Release);
        for _ in 0..100 {
            let s = client.signal_flush_state.load(Ordering::Acquire);
            assert!(
                s >> 2 >= new_gen,
                "stale worker A clobbered the new generation's state: {s:#x}"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        // And B still makes progress: its dirty entry lands.
        let addr = ProtocolAddress::new("15550007002".to_string(), 1.into());
        wait_for_backend_session(&client, &addr).await;
    }

    /// A schedule call carrying a torn-down connection's generation must not
    /// move the scheduler backwards — the exact bit-state must be untouched.
    #[tokio::test]
    async fn stale_schedule_cannot_move_the_generation_backwards() {
        let client = crate::test_utils::create_test_client().await;

        // Live generation 1, with generation 1's running+dirty state installed.
        client.connection_generation.store(1, Ordering::SeqCst);
        let installed = pack_flush_state(1, FLUSH_RUNNING | FLUSH_DIRTY);
        client
            .signal_flush_state
            .store(installed, Ordering::Release);

        // A stale gen-0 schedule is a no-op (live-generation guard).
        client.schedule_signal_flush(0);
        assert_eq!(
            client.signal_flush_state.load(Ordering::Acquire),
            installed,
            "a stale (gen-0) schedule must not alter gen-1 state"
        );

        // And the no-regress guard holds even if the live generation matched
        // the lane while the state already carried a newer generation (the
        // window between the live-generation check and the CAS).
        client.connection_generation.store(0, Ordering::SeqCst);
        client.schedule_signal_flush(0);
        assert_eq!(
            client.signal_flush_state.load(Ordering::Acquire),
            installed,
            "a schedule must never CAS gen-1 state down to gen-0"
        );
    }

    /// A stale schedule arriving while a new-generation worker is mid-flush must
    /// not steal the DIRTY bit — the pending update still gets its own window.
    #[tokio::test]
    async fn stale_schedule_does_not_starve_the_new_generation() {
        let client = crate::test_utils::create_test_client().await;
        client
            .signal_flush_test_block
            .store(true, Ordering::Release);

        // Arm worker B on generation 1 and wait until it is inside the flush.
        client.connection_generation.fetch_add(1, Ordering::SeqCst);
        dirty_session(&client, "15550008001");
        client.schedule_signal_flush_live();
        let deadline = wacore::time::Instant::now() + Duration::from_secs(1);
        while client.signal_flush_test_in_attempt.load(Ordering::Acquire) == 0 {
            assert!(
                wacore::time::Instant::now() < deadline,
                "worker B must enter"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }

        // A live request during the flush marks DIRTY; a stale gen-0 schedule
        // must not clear it.
        let second = dirty_session(&client, "15550008002");
        client.schedule_signal_flush_live();
        assert!(
            client.signal_flush_state.load(Ordering::Acquire) & FLUSH_DIRTY != 0,
            "the live request set DIRTY"
        );
        client.schedule_signal_flush(0);
        assert!(
            client.signal_flush_state.load(Ordering::Acquire) & FLUSH_DIRTY != 0,
            "a stale schedule must not clear the pending DIRTY"
        );

        // Release: B's first (blocked) attempt completes, then — because DIRTY is
        // still set — it runs a SECOND attempt. Proving the second window ran means
        // requiring the attempt counter to reach 2, not just that `second` landed
        // (the first attempt alone would persist it, since it was dirtied before
        // the blocked flush actually executed).
        client
            .signal_flush_test_block
            .store(false, Ordering::Release);
        let deadline = wacore::time::Instant::now() + Duration::from_secs(2);
        while client.signal_flush_test_in_attempt.load(Ordering::Acquire) < 2 {
            assert!(
                wacore::time::Instant::now() < deadline,
                "the pending DIRTY must trigger a second flush attempt"
            );
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
        wait_for_backend_session(&client, &second).await;
    }

    /// The lifecycle gate is a hard barrier: while it is held (as teardown holds
    /// it across the cache settle), a worker cannot write to the backend; once
    /// released, it flushes. This is what stops a stale flush from interleaving
    /// a backend write into teardown's settle.
    #[tokio::test]
    async fn lifecycle_gate_blocks_the_worker_flush_until_released() {
        let client = crate::test_utils::create_test_client().await;
        let s1 = dirty_session(&client, "15550009001");

        // Hold the gate as teardown's settle would.
        let gate = client.signal_flush_lifecycle.lock().await;

        client.schedule_signal_flush_live();
        // The worker arms, wakes after the window, and blocks acquiring the gate.
        // It genuinely cannot proceed, so the dirty session never reaches the
        // backend no matter how long we wait here.
        tokio::time::sleep(SIGNAL_FLUSH_WINDOW * 3).await;
        assert!(
            backend_session(&client, &s1).await.is_none(),
            "the worker must not flush while the lifecycle gate is held"
        );

        // Release as teardown would after settling; the worker proceeds.
        drop(gate);
        wait_for_backend_session(&client, &s1).await;
    }

    /// A worker that only reaches the gate after teardown bumped the generation
    /// re-checks under the gate and stands down: it never enters the flush, so
    /// the attempt counter stays 0 and the session is not persisted out of band.
    #[tokio::test]
    async fn worker_stands_down_if_it_reaches_the_gate_after_the_bump() {
        let client = crate::test_utils::create_test_client().await;
        let s1 = dirty_session(&client, "15550009101");

        // Teardown-first: hold the gate, arm a worker for the current generation,
        // then bump under the gate (mirrors cleanup_connection_state bumping the
        // generation and holding the gate across the settle).
        let gate = client.signal_flush_lifecycle.lock().await;
        client.schedule_signal_flush_live();
        client.connection_generation.fetch_add(1, Ordering::SeqCst);

        // Let the worker wake and block on the gate, then release it.
        tokio::time::sleep(SIGNAL_FLUSH_WINDOW * 3).await;
        drop(gate);
        tokio::time::sleep(SIGNAL_FLUSH_WINDOW * 3).await;

        assert_eq!(
            client.signal_flush_test_in_attempt.load(Ordering::Acquire),
            0,
            "a worker reaching the gate after the bump must stand down before flushing"
        );
        assert!(
            backend_session(&client, &s1).await.is_none(),
            "the stale worker must not persist S1 after teardown"
        );
    }
}
