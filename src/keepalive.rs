use crate::client::Client;
use crate::request::IqError;
use futures::FutureExt;
use log::{debug, warn};
use rand::RngExt;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;
use wacore::iq::spec::IqSpec;
use wacore::protocol::keepalive::{
    KEEP_ALIVE_INTERVAL_MAX, KEEP_ALIVE_INTERVAL_MIN, KEEP_ALIVE_RESPONSE_DEADLINE, elapsed_since,
    elapsed_since_at, is_dead_socket_at,
};

/// Keepalive ticks between two mid-session maintenance passes (~6 h).
///
/// The tick interval is drawn uniformly from
/// [`KEEP_ALIVE_INTERVAL_MIN`, `KEEP_ALIVE_INTERVAL_MAX`], so a tick count is
/// converted at the midpoint and the real period lands somewhere in 4-9 h. That
/// spread is fine because nothing here is scheduled *by* this cadence: the
/// signed pre-key rotation gates itself on its own 27-day interval and the
/// tcToken prune is idempotent. The cadence only has to be short enough that a
/// connection which never drops still reaches them, which the connect-only
/// callers could not promise.
const MAINTENANCE_TICKS: u32 = ticks_for(6 * 60 * 60);

/// Keepalive ticks between two storage-engine maintenance passes (~1 h).
///
/// Its own, shorter cadence than [`MAINTENANCE_TICKS`]: the backend pass is
/// local work with no IQ behind it, and the WAL truncation half only takes
/// effect on a pass that finds no reader holding a snapshot, so trying more
/// often is how it eventually succeeds.
const ENGINE_MAINTENANCE_TICKS: u32 = ticks_for(60 * 60);

/// Keepalive ticks that cover `period_secs`, at the midpoint of the randomized
/// interval.
const fn ticks_for(period_secs: u64) -> u32 {
    let midpoint = (KEEP_ALIVE_INTERVAL_MIN.as_secs() + KEEP_ALIVE_INTERVAL_MAX.as_secs()) / 2;
    (period_secs / midpoint) as u32
}

#[derive(Debug, PartialEq)]
enum KeepaliveResult {
    /// Server responded to the ping.
    Ok,
    /// Ping failed but the connection may recover (e.g. timeout, server error).
    TransientFailure,
    /// Connection is dead — loop should exit immediately.
    FatalFailure,
}

/// Classifies an IQ error into a keepalive result.
///
/// Fatal errors indicate the connection is already gone — there is no point
/// waiting for the grace window.  Transient errors (timeout, unexpected
/// server response) still count as failures but allow the grace window to
/// decide whether to force-reconnect.
fn classify_keepalive_error(e: &IqError) -> KeepaliveResult {
    match e {
        IqError::Socket(_)
        | IqError::EncryptSend(_)
        | IqError::ClientState(_)
        | IqError::Disconnected(_)
        | IqError::NotConnected
        | IqError::InternalChannelClosed
        | IqError::DuplicateRequestId(_)
        | IqError::EncodeError(_) => KeepaliveResult::FatalFailure,
        // Exhaustive: forces a compile error when new IqError variants are added
        // so the developer must decide the classification.
        IqError::Timeout
        | IqError::ServerError { .. }
        | IqError::UnexpectedResponseType { .. }
        | IqError::ParseError(_) => KeepaliveResult::TransientFailure,
        IqError::Unclassified(error) => {
            if error.is_transport_unavailable() {
                KeepaliveResult::FatalFailure
            } else {
                KeepaliveResult::TransientFailure
            }
        }
    }
}

/// Consecutive unanswered pings after which the connection is torn down rather
/// than pinged again.
///
/// Three, because a ping is only sent when the link has been *idle* for at
/// least `KEEP_ALIVE_INTERVAL_MIN` (the recent-activity early return skips it
/// otherwise), so three of them in a row is roughly a minute in which nothing
/// arrived AND nothing we sent was answered. A busy connection cannot reach
/// this count: every inbound frame resets it on the activity path, and most
/// ticks on such a connection never ping at all.
///
/// The hole this closes is the half-open socket the dead-socket watchdog is
/// blind to by design: `is_dead_socket_at` is cancelled by any receive, so a
/// link that still delivers inbound frames while our writes go nowhere looks
/// alive to it forever — and every send, ack and receipt is silently lost for
/// the rest of the session. Reconnect-looping against a merely slow server is
/// bounded by the two resets (`Ok` and recent activity) and by
/// `should_reset_backoff`, which keeps escalating the Fibonacci delay until a
/// connection stays up 30 s.
fn keepalive_failures_are_terminal(error_count: u32) -> bool {
    error_count >= KEEPALIVE_MAX_CONSECUTIVE_FAILURES
}

/// See [`keepalive_failures_are_terminal`].
const KEEPALIVE_MAX_CONSECUTIVE_FAILURES: u32 = 3;

/// Whether a keepalive ping error is just collateral of a teardown already
/// being handled elsewhere (the connection is gone, so the ping had nowhere to
/// go) rather than a genuine failure the keepalive surfaced first.
///
/// Used ONLY to pick the log level. It must stay narrower than the
/// `FatalFailure` set: Socket/EncryptSend/ClientState/EncodeError are also
/// fatal for control flow, but they mean the socket or send pipeline broke
/// while we still believed we were connected — a real failure that the
/// keepalive may be the first (or only) thing to observe, so it must stay loud.
fn is_benign_teardown(e: &IqError) -> bool {
    matches!(
        e,
        IqError::NotConnected | IqError::Disconnected(_) | IqError::InternalChannelClosed
    )
}

impl Client {
    /// Sends a keepalive ping and updates the server time offset from
    /// the pong's `t` attribute using RTT-adjusted midpoint calculation.
    ///
    /// WA Web: `sendPing` → `onClockSkewUpdate(Math.round((start + rtt/2) / 1000 - serverTime))`
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.keepalive.ping", level = "debug", skip_all)
    )]
    async fn send_keepalive(&self) -> KeepaliveResult {
        if !self.is_connected() {
            return KeepaliveResult::FatalFailure;
        }

        // WA Web: skip ping if there are pending IQs
        // (`activePing || ackHandlers.length || pendingIqs.size`)
        let has_pending = !self.response_waiters_guard().is_empty();
        if has_pending {
            debug!(target: "Client/Keepalive", "Skipping ping: IQ responses pending");
            return KeepaliveResult::Ok;
        }

        debug!(target: "Client/Keepalive", "Sending keepalive ping");

        // wall_rtt_ms feeds the WA Web onClockSkewUpdate formula, which
        // mixes start_ms with serverTime — both halves must be wall-clock.
        // rtt_monotonic is for the log only.
        let start_ms = wacore::time::now_millis();
        let rtt_start = wacore::time::Instant::now();
        let iq = wacore::iq::keepalive::KeepaliveSpec::with_timeout(KEEP_ALIVE_RESPONSE_DEADLINE)
            .build_iq();
        match self.send_iq(iq).await {
            Ok(response_node) => {
                let rtt_monotonic = rtt_start.elapsed();
                let wall_rtt_ms = wacore::time::now_millis().saturating_sub(start_ms).max(0);
                debug!(target: "Client/Keepalive", "Received keepalive pong (RTT: {rtt_monotonic:.2?})");
                self.unified_session.update_server_time_offset_with_rtt(
                    response_node.get(),
                    start_ms,
                    wall_rtt_ms,
                );
                KeepaliveResult::Ok
            }
            Err(e) => {
                let result = classify_keepalive_error(&e);
                // Log level is keyed on benign-teardown, NOT on FatalFailure: only
                // an already-gone connection (NotConnected/Disconnected/channel
                // closed, handled elsewhere) is quiet collateral. A broken
                // socket/send pipeline is also fatal for control flow but is a real
                // failure the keepalive may see first, so it stays loud — as do all
                // transient failures.
                if is_benign_teardown(&e) {
                    debug!(target: "Client/Keepalive", "Keepalive skipped, connection already closing: {e:?}");
                } else {
                    warn!(target: "Client/Keepalive", "Keepalive ping failed: {e:?}");
                }
                result
            }
        }
    }

    // Deliberately NOT instrumented: a span here would live for the whole
    // connection (tens of minutes), polluting duration/throughput metrics and
    // only reporting at disconnect. Per-ping visibility comes from
    // `wa.conn.keepalive.ping` on send_keepalive.
    ///
    /// `shutdown_signal` and `generation` are taken from the caller rather than
    /// read here, and both for the same reason: this task's first poll can be
    /// arbitrarily late. Subscribing inside it let a `reset_connection_shutdown`
    /// that already ran hand it the *next* connection's notifier, and reading
    /// the generation here would pin it to the same wrong connection — either
    /// way the previous connection's keepalive never exits and two of them ping
    /// the same socket. Captured at the spawn, they name the connection this
    /// loop was started for, whenever it happens to start.
    pub(crate) async fn keepalive_loop(
        self: Arc<Self>,
        shutdown_signal: wacore::runtime::ShutdownSignal,
        generation: u64,
    ) {
        let mut error_count = 0u32;
        let mut cleanup_counter = 0u32;
        let mut maintenance_counter = 0u32;
        let mut engine_maintenance_counter = 0u32;
        let sent_msg_ttl = self.cache_config.sent_message_ttl_secs;
        // Seeded once, not per tick: a fresh `StdRng` costs OS entropy and a
        // 320-byte state to draw one number, and this loop wakes every 15-30 s
        // on every connected client (measured 14.8 us vs 808 ns for the draw
        // alone, debug). The interval only needs to be unpredictable enough that
        // clients do not synchronise their pings.
        let mut interval_rng = rand::make_rng::<rand::rngs::StdRng>();

        loop {
            // Fresh listener each iteration (event_listener is edge-triggered);
            // the Weak underneath stays pinned to this connection's notifier.
            let shutdown = wacore::runtime::wait_for_shutdown(&shutdown_signal);

            let interval_ms = interval_rng.random_range(
                KEEP_ALIVE_INTERVAL_MIN.as_millis()..=KEEP_ALIVE_INTERVAL_MAX.as_millis(),
            );
            let interval = Duration::from_millis(interval_ms as u64);

            futures::select! {
                _ = self.runtime.sleep(interval).fuse() => {
                    if !self.is_connected() {
                        debug!(target: "Client/Keepalive", "Not connected, exiting keepalive loop.");
                        return;
                    }
                    // The connected flag alone cannot tell "still my connection"
                    // from "someone else's": a reconnect that completes before
                    // this task is first polled leaves it true again, and the
                    // loop would go on pinging a socket it was never started
                    // for, alongside that connection's own keepalive.
                    let current = self.connection_generation.load(
                        Ordering::Acquire,
                    );
                    if current != generation {
                        debug!(
                            target: "Client/Keepalive",
                            "Connection generation moved on ({generation} -> {current}), exiting keepalive loop.",
                        );
                        return;
                    }

                    // Periodic DB retention (~every 12 ticks ≈ 5 min). Driven by
                    // the interval tick itself, BEFORE the idle-ping early-return,
                    // so busy connections (which skip the ping) still prune.
                    cleanup_counter += 1;
                    if cleanup_counter >= 12 {
                        cleanup_counter = 0;
                        self.spawn_retention_cleanup(sent_msg_ttl);
                        self.spawn_cache_maintenance();
                    }

                    // Same placement and the same reason, on a much coarser
                    // counter: see MAINTENANCE_TICKS.
                    maintenance_counter += 1;
                    if maintenance_counter >= MAINTENANCE_TICKS {
                        maintenance_counter = 0;
                        self.spawn_session_maintenance();
                    }

                    engine_maintenance_counter += 1;
                    if engine_maintenance_counter >= ENGINE_MAINTENANCE_TICKS {
                        engine_maintenance_counter = 0;
                        self.spawn_engine_maintenance();
                    }

                    // Same reason as the retention sweep above: driven by the
                    // tick rather than by send_keepalive, because a connection
                    // with steady inbound traffic takes the early return below
                    // and would never sweep. A phash waiter whose ack was lost
                    // has nothing else to remove it, and it reads as an
                    // outstanding IQ.
                    self.response_waiters_guard().drop_expired_phash();

                    let last_recv = self.stats.last_data_received();

                    // WA Web: maybeScheduleHealthCheck — only send ping when idle.
                    // If we recently received data, the connection is proven alive;
                    // skip the ping and reschedule (same as WA Web rescheduling the
                    // healthCheckTimer after activity).
                    if let Some(since_recv) = elapsed_since(last_recv)
                        && since_recv < KEEP_ALIVE_INTERVAL_MIN
                    {
                        // Connection alive — reset error state, skip ping.
                        if error_count > 0 {
                            debug!(target: "Client/Keepalive", "Keepalive restored (recent activity).");
                            error_count = 0;
                        }
                        continue;
                    }

                    // Probe the connection BEFORE checking dead-socket so that a
                    // successful pong updates last_received_ms and prevents a
                    // false-positive dead-socket trigger on an idle-but-healthy
                    // connection.  WA Web uses a separate 20 s timer that is
                    // cancelled on any receive; our periodic loop needs to send the
                    // ping first to give the server a chance to prove it is alive.
                    match self.send_keepalive().await {
                        KeepaliveResult::Ok => {
                            if error_count > 0 {
                                debug!(target: "Client/Keepalive", "Keepalive restored after {error_count} failure(s).");
                            }
                            error_count = 0;
                        }
                        KeepaliveResult::FatalFailure => {
                            debug!(target: "Client/Keepalive", "Fatal keepalive failure, exiting loop.");
                            return;
                        }
                        KeepaliveResult::TransientFailure => {
                            error_count += 1;
                            warn!(target: "Client/Keepalive", "Keepalive timeout, error count: {error_count}");
                            if keepalive_failures_are_terminal(error_count) {
                                warn!(
                                    target: "Client/Keepalive",
                                    "{error_count} consecutive unanswered pings, forcing reconnect.",
                                );
                                self.reconnect_immediately().await;
                                return;
                            }
                        }
                    }

                    // WA Web: deadSocketTimer is an independent 20s watchdog armed on
                    // the FIRST send after a receive (onOrBefore keeps the earliest
                    // deadline) and cancelled on every receive. We approximate this by
                    // checking is_dead_socket on EVERY keepalive tick — not just after
                    // a failed ping. This catches scenarios where pending IQs caused
                    // the ping to be skipped, or where the ping "succeeded" but the
                    // connection died immediately after.
                    let first_send = self.stats.first_send_since_recv();
                    let last_recv = self.stats.last_data_received();
                    let now = wacore::time::Instant::now();
                    if is_dead_socket_at(first_send, last_recv, now) {
                        let elapsed = elapsed_since_at(first_send, now).unwrap_or_default();
                        warn!(
                            target: "Client/Keepalive",
                            "No data received for {:.1}s after send (dead socket), forcing reconnect.",
                            elapsed.as_secs_f64()
                        );
                        self.reconnect_immediately().await;
                        return;
                    }
                },
                _ = shutdown.fuse() => {
                    debug!(target: "Client/Keepalive", "Shutdown signaled, exiting keepalive loop.");
                    return;
                }
            }
        }
    }

    /// Sweep expired entries out of the in-process caches.
    ///
    /// The caches expire lazily: an entry is dropped on the access that finds
    /// it stale, or on capacity pressure from a new insert. Neither happens on
    /// a quiet connection, so without a timer a client that fanned out to a
    /// large group set once kept every one of those records — up to the
    /// 20 000-entry device registry — for as long as it stayed connected.
    /// This is that timer. Store-backed caches expire on their own and the
    /// wrappers no-op for them; capacity-only caches have nothing to expire
    /// and are left alone.
    ///
    /// Runs on the keepalive's ~5-minute maintenance tick, not every tick and
    /// never on the receive path: each sweep takes its cache's write lock for
    /// a full walk of the table.
    pub async fn run_cache_maintenance(&self) {
        self.recent_messages.run_pending_tasks().await;
        self.message_retry_counts.run_pending_tasks().await;
        self.session_recreate_history.run_pending_tasks().await;
        self.undecryptable_dispatched.run_pending_tasks().await;
        self.dispatched_messages.run_pending_tasks().await;
        self.pdo_pending_requests.run_pending_tasks().await;
        self.pdo_requested.run_pending_tasks().await;
        self.device_registry_cache.run_pending_tasks().await;
        self.sender_key_device_cache.run_pending_tasks().await;
        self.lid_pn_cache.run_pending_tasks().await;
        // `get()`, not `get_group_cache()`: maintenance must not be what
        // builds a cache the client never used.
        if let Some(group_cache) = self.group_cache.get() {
            group_cache.run_pending_tasks().await;
        }
    }

    fn spawn_cache_maintenance(self: &Arc<Self>) {
        let client = Arc::clone(self);
        self.runtime
            .spawn(Box::pin(async move {
                client.run_cache_maintenance().await;
            }))
            .detach();
    }

    /// Fire-and-forget DB retention sweeps. Each TTL gates its own delete so
    /// they enable/disable independently. `0` disables a sweep. TTLs are
    /// converted with a checked cast (absurd values clamp instead of wrapping
    /// the cutoff negative).
    ///
    /// One task running them in sequence, not three racing ones: every sweep
    /// goes through the store's single write permit anyway, so racing them only
    /// buys three blocking threads queueing for the same slot ahead of live
    /// traffic. Failures are logged at `warn!` because a sweep that fails every
    /// time is how a bounded table quietly stops being bounded.
    fn spawn_retention_cleanup(&self, sent_msg_ttl: u64) {
        let now = wacore::time::now_secs();
        let cutoff_for = |ttl: u64| now.saturating_sub(i64::try_from(ttl).unwrap_or(i64::MAX));

        let backend = self.persistence_manager.backend();
        let sent_cutoff = (sent_msg_ttl > 0).then(|| cutoff_for(sent_msg_ttl));
        // Pending inbound buffer retention (inbound durability hook): a row a
        // permanently-failing hook never commits would otherwise linger once the
        // server stops redelivering it. Run unconditionally (not gated on the hook
        // being set now) so rows buffered by a hook in a previous run are still
        // swept after it is disabled. Backends without the buffer return 0 from
        // the default impl, so this is a cheap no-op there.
        const PENDING_INBOUND_TTL_SECS: u64 = 7 * 24 * 60 * 60;
        let pending_cutoff = cutoff_for(PENDING_INBOUND_TTL_SECS);
        // A base key is recorded when a peer's retry #2 arrives and is read back
        // only by a retry #3 for the same message. One retry conversation is the
        // whole lifetime of the row: past that window it answers a question
        // nobody asks again, and before this sweep existed the common case (no
        // retry #3) kept it forever. An hour is generous — the retries it guards
        // arrive seconds apart — and a too-short TTL only degrades collision
        // detection to "no collision", the behaviour that predates the check.
        const BASE_KEY_TTL_SECS: u64 = 60 * 60;
        let base_key_cutoff = cutoff_for(BASE_KEY_TTL_SECS);
        let prune_msg_secrets = self.cache_config.msg_secret_policy.prunes();

        self.runtime
            .spawn(Box::pin(async move {
                if let Some(cutoff) = sent_cutoff
                    && let Err(e) = backend.delete_expired_sent_messages(cutoff).await
                {
                    warn!(target: "Client/Keepalive", "Sent message cleanup error: {e}");
                }

                if let Err(e) = backend.delete_expired_pending_inbound(pending_cutoff).await {
                    warn!(target: "Client/Keepalive", "Pending inbound cleanup error: {e}");
                }

                if let Err(e) = backend.delete_expired_base_keys(base_key_cutoff).await {
                    warn!(target: "Client/Keepalive", "Base key cleanup error: {e}");
                }

                // msg_secrets retention: prune rows whose per-row deadline has passed.
                // expires_at is absolute, so the cutoff is simply "now"; per-kind
                // horizons and never-expire (0) rows are baked in at write time.
                if prune_msg_secrets {
                    match backend.delete_expired_msg_secrets(now).await {
                        Ok(n) if n > 0 => {
                            debug!(target: "Client/Keepalive", "Pruned {n} expired msg_secrets");
                        }
                        Ok(_) => {}
                        Err(e) => {
                            warn!(target: "Client/Keepalive", "msg_secrets cleanup error: {e}");
                        }
                    }
                }
            }))
            .detach();
    }

    /// Mid-session key and token maintenance, on the keepalive tick.
    ///
    /// Both jobs used to run only from the connect-time background init
    /// (`client/node_io.rs`), which is fine for a process that reconnects often
    /// and useless for one that does not: a session held for longer than the
    /// 27-day rotation cadence never rotated its signed pre-key, and pruned
    /// tcTokens exactly once, at hour zero. The connect-time calls stay — they
    /// are the ones that cover a freshly started process.
    fn spawn_session_maintenance(self: &Arc<Self>) {
        let client = Arc::clone(self);
        let generation = self.connection_generation.load(Ordering::SeqCst);
        self.runtime
            .spawn(Box::pin(async move {
                client.run_session_maintenance(generation).await;
            }))
            .detach();
    }

    /// Storage-engine upkeep: whatever the backend needs to stay in shape over
    /// a session measured in weeks. A no-op for backends that don't implement
    /// it, and cheap enough for a live connection by contract
    /// (`DeviceStore::maintenance`).
    fn spawn_engine_maintenance(&self) {
        let backend = self.persistence_manager.backend();
        self.runtime
            .spawn(Box::pin(async move {
                if let Err(e) = backend.maintenance().await {
                    warn!(target: "Client/Keepalive", "Storage maintenance error: {e}");
                }
            }))
            .detach();
    }

    /// The body of [`Self::spawn_session_maintenance`]; separate so a test can
    /// await the pass instead of racing a detached task.
    pub(crate) async fn run_session_maintenance(&self, generation: u64) {
        // Only the rotation is generation-gated: it uploads the new key over
        // this connection, so a reconnect between the tick and the upload means
        // a newer background init already owns the work — the same guard the
        // connect-time caller applies. The tcToken prune is a local delete and
        // is correct on any generation.
        if self.connection_generation.load(Ordering::SeqCst) == generation {
            let rotation = self.maybe_rotate_signed_pre_key().await;
            if let Err(e) = rotation
                && !self.is_shutting_down()
            {
                warn!(target: "Client/Keepalive", "Signed pre-key rotation check failed: {e:?}");
            }
        }

        let pruned = self.tc_token().prune_expired().await;
        if let Err(e) = pruned
            && !self.is_shutting_down()
        {
            warn!(target: "Client/Keepalive", "Failed to prune expired tc_tokens: {e:?}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::socket::error::{EncryptSendError, SocketError};
    use wacore_binary::builder::NodeBuilder;

    // The maintenance cadence is a tick count, so the interval it really lands
    // on depends on where each randomized tick falls. Both ends must stay in
    // "a few times a day": short enough that a weeks-long connection reaches
    // the 27-day rotation check with room to spare, long enough that the pass
    // is not competing with real traffic for the write permit.
    #[test]
    fn the_maintenance_cadence_lands_in_hours_not_minutes_or_days() {
        let fastest = MAINTENANCE_TICKS as u64 * KEEP_ALIVE_INTERVAL_MIN.as_secs();
        let slowest = MAINTENANCE_TICKS as u64 * KEEP_ALIVE_INTERVAL_MAX.as_secs();
        assert!(
            (4 * 3600..=6 * 3600).contains(&fastest),
            "fastest maintenance period was {fastest}s"
        );
        assert!(
            (6 * 3600..=12 * 3600).contains(&slowest),
            "slowest maintenance period was {slowest}s"
        );
    }

    // The engine pass is the shorter of the two cadences, and deliberately so:
    // its WAL truncation only lands on a pass that finds no reader holding a
    // snapshot.
    #[test]
    fn the_engine_cadence_is_shorter_than_the_session_cadence() {
        const { assert!(ENGINE_MAINTENANCE_TICKS < MAINTENANCE_TICKS) };
        let fastest = ENGINE_MAINTENANCE_TICKS as u64 * KEEP_ALIVE_INTERVAL_MIN.as_secs();
        let slowest = ENGINE_MAINTENANCE_TICKS as u64 * KEEP_ALIVE_INTERVAL_MAX.as_secs();
        assert!(
            (30 * 60..=3600).contains(&fastest),
            "fastest engine period was {fastest}s"
        );
        assert!(
            (3600..=2 * 3600).contains(&slowest),
            "slowest engine period was {slowest}s"
        );
    }

    /// Three, and the two below it are not. The counter was already being
    /// computed and logged; this is the decision that makes it load-bearing.
    #[test]
    fn three_consecutive_unanswered_pings_are_terminal() {
        assert!(!keepalive_failures_are_terminal(0));
        assert!(!keepalive_failures_are_terminal(1));
        assert!(
            !keepalive_failures_are_terminal(2),
            "two unanswered pings is a slow server, not a broken one"
        );
        assert!(keepalive_failures_are_terminal(3));
        assert!(keepalive_failures_are_terminal(4));
    }

    /// A keepalive belongs to the connection it was started for. Its first poll
    /// can land after that connection is gone — the spawn only promises the
    /// task will run — and `is_connected()` is true again by then if a
    /// reconnect has completed, so without the generation check the old loop
    /// would go on pinging the new connection's socket alongside its own
    /// keepalive: two pingers on one socket, and one more with every reconnect.
    #[tokio::test(start_paused = true)]
    async fn keepalive_exits_when_its_connection_generation_is_retired() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let stale_generation = client.connection_generation.load(Ordering::Acquire);
        // The connection this loop was started for is retired, and (as after a
        // reconnect) the client is connected again on a newer one.
        client.connection_generation.fetch_add(1, Ordering::SeqCst);

        tokio::time::timeout(
            Duration::from_secs(600),
            client
                .clone()
                .keepalive_loop(client.connection_shutdown_signal(), stale_generation),
        )
        .await
        .expect("a keepalive on a retired generation must exit at its first tick");
        assert_eq!(
            transport.sent_count(),
            0,
            "a retired keepalive must not ping the connection that replaced it"
        );

        // The control, and the reason the assertion above is about the
        // generation rather than about a fixture that could not have sent
        // anything: the same loop on the live generation does ping (and then
        // ends itself, because nothing answers it).
        let live_generation = client.connection_generation.load(Ordering::Acquire);
        tokio::time::timeout(
            Duration::from_secs(600),
            client
                .clone()
                .keepalive_loop(client.connection_shutdown_signal(), live_generation),
        )
        .await
        .expect("an unanswered ping ends the loop through the dead-socket check");
        assert!(
            transport.sent_count() >= 1,
            "the fixture must be able to send a ping at all"
        );
    }

    #[test]
    fn test_classify_timeout_is_transient() {
        assert_eq!(
            classify_keepalive_error(&IqError::Timeout),
            KeepaliveResult::TransientFailure,
            "Timeout should be transient — connection may recover"
        );
    }

    #[test]
    fn test_classify_not_connected_is_fatal() {
        assert_eq!(
            classify_keepalive_error(&IqError::NotConnected),
            KeepaliveResult::FatalFailure,
        );
    }

    #[test]
    fn test_classify_internal_channel_closed_is_fatal() {
        assert_eq!(
            classify_keepalive_error(&IqError::InternalChannelClosed),
            KeepaliveResult::FatalFailure,
        );
    }

    #[test]
    fn test_classify_duplicate_request_id_is_fatal() {
        assert_eq!(
            classify_keepalive_error(&IqError::DuplicateRequestId("duplicate".into())),
            KeepaliveResult::FatalFailure,
        );
    }

    #[test]
    fn test_classify_socket_error_is_fatal() {
        assert_eq!(
            classify_keepalive_error(&IqError::Socket(SocketError::SocketClosed)),
            KeepaliveResult::FatalFailure,
        );
    }

    #[test]
    fn test_classify_disconnected_is_fatal() {
        let node = NodeBuilder::new("disconnect").build();
        assert_eq!(
            classify_keepalive_error(&IqError::Disconnected(Box::new(node))),
            KeepaliveResult::FatalFailure,
        );
    }

    #[test]
    fn test_classify_server_error_is_transient() {
        assert_eq!(
            classify_keepalive_error(&crate::test_utils::server_error_iq(
                500, "internal", None, None
            )),
            KeepaliveResult::TransientFailure,
            "ServerError should be transient — server may recover"
        );
    }

    #[test]
    fn test_classify_parse_error_is_transient() {
        assert_eq!(
            classify_keepalive_error(&IqError::ParseError(anyhow::anyhow!("bad response"))),
            KeepaliveResult::TransientFailure,
            "ParseError should be transient — bad response, not a dead connection"
        );
    }

    #[test]
    fn test_classify_unexpected_response_type_is_transient() {
        assert_eq!(
            classify_keepalive_error(&IqError::UnexpectedResponseType {
                got: Some("get".to_string()),
            }),
            KeepaliveResult::TransientFailure,
        );
    }

    // Happy path: the connection was already gone, so a failed ping is just
    // teardown collateral and is logged quietly.
    #[test]
    fn benign_teardown_errors_are_quiet() {
        assert!(is_benign_teardown(&IqError::NotConnected));
        assert!(is_benign_teardown(&IqError::InternalChannelClosed));
        let node = NodeBuilder::new("disconnect").build();
        assert!(is_benign_teardown(&IqError::Disconnected(Box::new(node))));
    }

    // Bad path: a broken socket/send pipeline or an encode failure is fatal for
    // control flow but is a REAL failure (we still thought we were connected), so
    // it must NOT be treated as benign — it has to stay loud. Transient failures
    // stay loud too. This is the guard against the keepalive ping silently
    // swallowing the first sign of a real connection/send break.
    #[test]
    fn real_failures_are_never_treated_as_benign() {
        assert!(!is_benign_teardown(&IqError::Socket(
            SocketError::SocketClosed
        )));
        assert!(!is_benign_teardown(&IqError::EncryptSend(
            EncryptSendError::transport(anyhow::anyhow!("broken pipe"))
        )));
        assert!(!is_benign_teardown(&IqError::EncodeError(anyhow::anyhow!(
            "encode failed"
        ))));
        assert!(!is_benign_teardown(&IqError::Timeout));
        assert!(!is_benign_teardown(&IqError::ParseError(anyhow::anyhow!(
            "bad response"
        ))));
        assert!(!is_benign_teardown(&crate::test_utils::server_error_iq(
            500, "internal", None, None
        )));
        assert!(!is_benign_teardown(&IqError::UnexpectedResponseType {
            got: Some("get".to_string()),
        }));
    }

    // elapsed_since, is_dead_socket, and constants tests live in wacore::protocol::keepalive
}
