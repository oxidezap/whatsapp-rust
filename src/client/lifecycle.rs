//! Client construction and connection lifecycle: connect, run, reconnect, shutdown.

use super::*;
use wacore::net::DisconnectReason;

/// Max groups with a cached resolved-device snapshot. LRU eviction covers
/// accounts in more groups; an evicted entry just recomputes on next send.
const GROUP_DEVICES_MEMO_CAPACITY: u64 = 64;

/// Max 1:1 chats with a cached resolved-device snapshot. Higher than the
/// group bound because a bot's active DM set is typically much wider; each
/// entry is only the device list plus its member set.
const DM_DEVICES_MEMO_CAPACITY: u64 = 512;

/// `authenticated_generation` when no connection has published one.
///
/// Not zero: that is a real generation, the one a freshly built client is on,
/// so zero would read as authenticated before anything had authenticated.
/// `connection_generation` only ever counts up, so this can never collide.
pub(crate) const NO_AUTHENTICATED_GENERATION: u64 = u64::MAX;

impl Drop for Client {
    fn drop(&mut self) {
        self.signal_shutdown_sync();
    }
}

impl Client {
    /// WA Web `resetDelay: 30000` — only after a connection has stayed up this
    /// long is the reconnect backoff counter reset to its base.
    pub(crate) const STABLE_CONNECTION_RESET_MS: i64 = 30_000;

    /// Create a runtime-validated low-level client builder.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    pub fn shutdown_signal(&self) -> wacore::runtime::ShutdownSignal {
        self.shutdown_notifier.subscribe()
    }

    /// Synchronous flag-only equivalent of the first lines of `disconnect()`.
    /// Spawned tasks watching `is_shutting_down()` / `shutdown_notifier` exit
    /// on their next poll. Does NOT flush, close the transport, or touch
    /// persistence — prefer `disconnect()` whenever you can `await`. Exists
    /// for `Drop` impls on FFI wrappers (e.g. `WasmWhatsAppClient`) that
    /// can't run async cleanup synchronously.
    pub fn signal_shutdown_sync(&self) {
        self.publish_terminal_verdict();
        self.shutdown_notifier.notify();
        self.notify_session_state();
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.signal_shutdown_sync();
        }
        self.notify_connection_shutdown();
    }

    /// Publish "the session is over, and no reconnect follows" as one
    /// transition.
    ///
    /// The order and the `Release` are the whole point, and pair with the
    /// `Acquire` load in [`run`](Self::run)'s expected-disconnect branch: that
    /// branch reads `expected_disconnect` first and `is_running` second, so the
    /// reader flag has to be cleared *before* the verdict is published. Stored
    /// the other way round — or both `Relaxed`, as they were — the branch can
    /// read a torn pair, see a planned end with a reader still live, and
    /// announce a reconnect that this call has already ruled out.
    fn publish_terminal_verdict(&self) {
        self.is_running.store(false, Ordering::Relaxed);
        self.expected_disconnect.store(true, Ordering::Release);
    }

    /// Forget what the connection that just ended owed the reconnect backoff.
    ///
    /// A planned end is not a failure, whether the application asked for it or a
    /// protocol step like the post-pairing 515 did, so the consecutive-failure
    /// count driving the Fibonacci delay goes back to zero. Both planned exits
    /// call this because `stats().reconnect_errors` reports that count, and a
    /// number that depended on which exit was taken would make a change of
    /// branch a change of public behaviour.
    ///
    /// The auth timestamp is consumed for a separate reason: left standing, a
    /// later failed connect reads this cycle's value as a "stable" connection
    /// and resets the backoff on the strength of it.
    fn clear_connection_backoff_state(&self) {
        self.auto_reconnect_errors.store(0, Ordering::Relaxed);
        self.connected_at_ms.store(0, Ordering::Relaxed);
    }

    pub(crate) fn connection_shutdown_signal(&self) -> wacore::runtime::ShutdownSignal {
        self.connection_shutdown
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .subscribe()
    }

    /// Fire the per-connection shutdown. Per-connection subscribers exit;
    /// the terminal shutdown_notifier is untouched so reconnects still work.
    pub(crate) fn notify_connection_shutdown(&self) {
        self.connection_shutdown
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .notify();
        // Also the session signal, because this is the one point every
        // connection ends through — planned or fatal. `handle_stream_error`
        // makes a client terminal by setting `enable_auto_reconnect` and
        // `expected_disconnect` and then calling only this; work parked in
        // `await_connection` would otherwise wait for the run loop to unwind far
        // enough to announce it, and the invariant on `is_terminal` promises
        // better than "eventually, if some other loop gets there".
        //
        // A teardown that a reconnect follows wakes the wait for nothing, which
        // costs a state re-read and a re-park. That is the trade this notifier
        // is built for.
        self.notify_session_state();
    }

    /// Reset the per-connection notifier. Call at the start of each new
    /// connection so subscribers registered afterwards see a fresh signal.
    /// The previous notifier's subscribers have already been woken (either
    /// by notify on disconnect, or by falling out of scope).
    pub(crate) fn reset_connection_shutdown(&self) {
        *self
            .connection_shutdown
            .lock()
            .unwrap_or_else(|p| p.into_inner()) = wacore::runtime::ShutdownNotifier::new();
    }

    pub(crate) fn is_shutting_down(&self) -> bool {
        self.expected_disconnect.load(Ordering::Relaxed) || !self.is_running.load(Ordering::Relaxed)
    }

    /// Whether the client is finished for good, as opposed to being between
    /// connections.
    ///
    /// [`is_shutting_down`](Self::is_shutting_down) answers both at once, which
    /// is a trap for work that outlives a connection: a planned reconnect makes
    /// it true, so anything reading it as "stop" throws itself away instead of
    /// waiting for the replacement.
    ///
    /// Built from the signals that actually mean *terminal*. The shutdown
    /// notifier is deliberately left untouched by reconnects, and it is fired by
    /// `disconnect`, `logout` and `signal_shutdown_sync`. The stream errors that
    /// end a session without going through those clear `enable_auto_reconnect`
    /// and set `expected_disconnect` together, which is what tells them apart
    /// from an application merely turning auto-reconnect off.
    ///
    /// Not `is_running` on its own: that tracks whether anything is driving the
    /// client, which a client that only connected never has, so a healthy one
    /// would look permanently stopped the moment its application expressed a
    /// reconnect preference.
    ///
    /// Every transition that can make this true fires
    /// [`session_state_notifier`](Client::session_state_notifier), because a
    /// waiter parked on a connection that is never coming has no other way to
    /// learn that it should stop.
    pub(crate) fn is_terminal(&self) -> bool {
        if self.shutdown_signal().is_fired() {
            return true;
        }
        // `enable_auto_reconnect` alone is a preference, not proof: it is public,
        // and an application may clear it on a healthy connection to mean "do
        // not come back after this one ends". The internal paths that really do
        // end the session — conflict, 516, an unrecoverable connect failure —
        // always set `expected_disconnect` alongside it, so the pair is what
        // separates a policy from a verdict.
        //
        // The second half is the run loop's own exit: it stops by clearing
        // `is_running` and breaking, without firing the notifier or touching
        // `expected_disconnect`, so that pairing has to count too. It is read
        // together with a dead socket, because `is_running` is also false for a
        // direct-connect client that never started a supervision loop — and one
        // of those with a live connection is not finished, it just never had a
        // loop to end. By the time the run loop breaks, `cleanup_connection_state`
        // has already cleared `is_connected`, so the real exit still reads as one.
        !self.enable_auto_reconnect.load(Ordering::Relaxed)
            && (self.expected_disconnect.load(Ordering::Relaxed)
                || (!self.is_running.load(Ordering::Relaxed) && !self.is_connected()))
    }

    /// Wake everything waiting on whether this client can still do work.
    ///
    /// Call after any store that may have flipped [`is_terminal`](Self::is_terminal)
    /// or completed authentication. Spurious calls are free: every waiter
    /// re-reads the state and parks again, so this is only ever a hint that the
    /// answer is worth asking for again.
    pub(crate) fn notify_session_state(&self) {
        self.session_state_notifier.notify(usize::MAX);
    }

    /// The reader of this client (the supervision loop, or a directly driven
    /// connection) stopping for good.
    ///
    /// A named transition rather than two stores at the branch, because the
    /// notify is not optional here and there is nowhere else to learn of it:
    /// this is the only terminal transition that fires no notifier — a later
    /// `run()` must stay possible, so the shutdown one is deliberately left
    /// alone — and announces no socket, ever again. Work parked waiting for a
    /// connection finds out here or not at all.
    pub(crate) fn stop_supervision_loop(&self) {
        self.is_running.store(false, Ordering::Relaxed);
        self.notify_session_state();
    }

    /// Returns `true` when the client has completed its full startup: transport
    /// connected, server authenticated, and the critical app state sync settled
    /// one way or the other. This is the condition `wait_for_connected` uses to
    /// resolve.
    fn is_fully_ready(&self) -> bool {
        self.is_connected()
            && self.is_logged_in()
            && self.is_ready.load(Ordering::Relaxed)
            // A pause publishes its flag before the read loop has cleared any
            // of the three above, so without this the waiters hand a caller the
            // very connection the application just asked to have closed.
            && !self.is_paused()
    }

    /// Dispatch the Connected event and notify waiters for the originating connection.
    pub(crate) async fn dispatch_connected(&self, expected_generation: u64) {
        #[cfg(feature = "client-lifecycle")]
        {
            if let Some(lifecycle) = &self.lifecycle {
                if !lifecycle.ready(expected_generation).await {
                    debug!(
                        "Skipping Connected dispatch for retired generation {expected_generation}"
                    );
                    return;
                }

                // Cleanup takes the same lock before retiring the generation, so the final
                // validation and publication form one transition with its generation bump.
                let _login_transition = self
                    .login_transition
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner());
                if !self.still_announceable(expected_generation) {
                    debug!(
                        "Skipping Connected dispatch after generation {expected_generation} retired"
                    );
                    return;
                }
                if !lifecycle.publish_ready(expected_generation, || self.publish_connected()) {
                    debug!("Skipping Connected dispatch after lifecycle cancellation");
                }
                return;
            }
        }

        #[cfg(feature = "client-lifecycle")]
        let _login_transition = self
            .login_transition
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        if !self.still_announceable(expected_generation) {
            debug!("Skipping Connected dispatch after its connection retired");
            return;
        }
        self.publish_connected();
    }

    /// Whether `expected_generation` is still a connection worth announcing.
    ///
    /// Asked under `login_transition` at the point of publication, never before
    /// it: the lifecycle path awaits an extension host's readiness hook first,
    /// and a check made on the near side of that await answers for a connection
    /// the far side may no longer have.
    ///
    /// `paused` alongside the retirement checks: a pause spends up to four
    /// seconds flushing before it closes the socket, and a post-login task
    /// finishing in that window passes its generation check and would announce a
    /// session that is being taken offline. `is_logged_in` for the case that has
    /// no flag of its own: 429 and 503 mark the session rejected inline and fall
    /// through without retiring the generation, so every other condition here
    /// still reads as a healthy connection.
    ///
    /// Not atomic with those stores, and deliberately not: a rejection landing
    /// between this read and the publication announces a connection that is
    /// about to drop, which is what a rejection one instruction later does too.
    /// Nothing durable comes of it, because `is_fully_ready` asks
    /// `is_logged_in` again next to `is_ready`. Closing the window would mean
    /// taking a lock on the read loop's stream-error path for a distinction no
    /// consumer can observe.
    fn still_announceable(&self, expected_generation: u64) -> bool {
        self.connection_generation.load(Ordering::SeqCst) == expected_generation
            && !self.expected_disconnect.load(Ordering::Acquire)
            && !self.is_paused()
            && self.is_logged_in()
    }

    fn publish_connected(&self) {
        self.is_ready.store(true, Ordering::Relaxed);
        wacore::telemetry::set_connected(true);
        self.core.event_bus.dispatch(Event::Connected(
            crate::types::events::Connected::builder().build(),
        ));
        self.connected_notifier.notify(usize::MAX);
    }

    #[cfg(feature = "client-lifecycle")]
    pub(super) async fn shutdown_lifecycle(&self) {
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.shutdown().await;
        }
    }

    #[cfg(feature = "client-lifecycle")]
    fn request_lifecycle_shutdown(&self) {
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.request_shutdown();
        }
    }

    /// Create a new `Client` with default cache configuration.
    ///
    /// This is the standard constructor. Use [`Client::new_with_cache_config`]
    /// if you need to customise cache TTL / capacity.
    pub async fn new(
        runtime: Arc<dyn Runtime>,
        persistence_manager: Arc<PersistenceManager>,
        transport_factory: Arc<dyn crate::transport::TransportFactory>,
        http_client: Arc<dyn crate::http::HttpClient>,
        override_version: Option<(u32, u32, u32)>,
    ) -> (Arc<Self>, async_channel::Receiver<MajorSyncTask>) {
        ClientBuilder::build_required(
            runtime,
            persistence_manager,
            transport_factory,
            http_client,
            override_version,
            CacheConfig::default(),
        )
        .await
        .into_parts()
    }

    /// Create a new `Client` with a custom [`CacheConfig`].
    pub async fn new_with_cache_config(
        runtime: Arc<dyn Runtime>,
        persistence_manager: Arc<PersistenceManager>,
        transport_factory: Arc<dyn crate::transport::TransportFactory>,
        http_client: Arc<dyn crate::http::HttpClient>,
        override_version: Option<(u32, u32, u32)>,
        cache_config: CacheConfig,
    ) -> (Arc<Self>, async_channel::Receiver<MajorSyncTask>) {
        ClientBuilder::build_required(
            runtime,
            persistence_manager,
            transport_factory,
            http_client,
            override_version,
            cache_config,
        )
        .await
        .into_parts()
    }

    pub(super) fn assemble(
        runtime: Arc<dyn Runtime>,
        persistence_manager: Arc<PersistenceManager>,
        transport_factory: Arc<dyn crate::transport::TransportFactory>,
        http_client: Arc<dyn crate::http::HttpClient>,
        override_version: Option<(u32, u32, u32)>,
        cache_config: CacheConfig,
        extensions: ClientExtensions,
    ) -> ClientAssembly {
        let ClientExtensions {
            #[cfg(feature = "client-lifecycle")]
            lifecycle,
            #[cfg(feature = "plugins")]
            plugin_host,
        } = extensions;
        let mut unique_id_bytes = [0u8; 2];
        rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut unique_id_bytes);

        let device_snapshot = persistence_manager.get_device_snapshot();
        let core = wacore::client::CoreClient::new(device_snapshot.core.clone());

        let (tx, rx) = async_channel::bounded(32);

        let device_topology = device_topology::DeviceTopology::new();
        let sent_frame_tap = Arc::new(SentFrameTap::new(core.event_bus.clone()));
        let this = Self {
            runtime: runtime.clone(),
            core,
            msg_secret_buffer: crate::msg_secret_buffer::MsgSecretWriteBuffer::new(
                persistence_manager.backend(),
                runtime.clone(),
            ),
            persistence_manager: persistence_manager.clone(),
            media_conn: Arc::new(RwLock::new(None)),
            is_logged_in: Arc::new(AtomicBool::new(false)),
            #[cfg(feature = "client-lifecycle")]
            login_transition: std::sync::Mutex::new(()),
            is_connecting: Arc::new(AtomicBool::new(false)),
            is_running: Arc::new(AtomicBool::new(false)),
            is_connected: Arc::new(AtomicBool::new(false)),
            send_active_receipts: AtomicU32::new(0),
            ik_handshake_failures: Arc::new(AtomicU32::new(0)),
            shutdown_notifier: wacore::runtime::ShutdownNotifier::new(),
            connection_shutdown: std::sync::Mutex::new(wacore::runtime::ShutdownNotifier::new()),
            #[cfg(feature = "client-lifecycle")]
            lifecycle,
            #[cfg(feature = "plugins")]
            plugin_host,
            stats: Arc::new(wacore::stats::SessionStats::new()),

            transport: Arc::new(Mutex::new(None)),
            transport_events: Arc::new(Mutex::new(None)),
            transport_factory,
            noise_socket: Arc::new(std::sync::Mutex::new(None)),

            response_waiters: Arc::new(std::sync::Mutex::new(ResponseWaiterMap::default())),
            node_waiters: std::sync::Mutex::new(Vec::new()),
            node_waiter_count: AtomicUsize::new(0),
            sent_node_waiters: std::sync::Mutex::new(Vec::new()),
            sent_node_waiter_count: AtomicUsize::new(0),
            unique_id: format!("{}.{}", unique_id_bytes[0], unique_id_bytes[1]),
            id_counter: Arc::new(AtomicU64::new(0)),
            unified_session: crate::unified_session::UnifiedSessionManager::new(),

            signal_cache: Arc::new(crate::store::signal_cache::SignalStoreCache::new()),
            message_processing_semaphore: std::sync::Mutex::new(Arc::new(
                async_lock::Semaphore::new(1),
            )),
            message_semaphore_generation: Arc::new(AtomicU64::new(0)),
            // Coordination caches: capacity-only eviction, no TTL/TTI.
            // These hold live mutexes and channel senders; time-based eviction
            // while tasks hold references would silently break serialisation.
            // The evict_guard also blocks capacity eviction of a mutex a task is
            // holding (strong_count > 1) — evicting it would mint a second mutex
            // and let two writers race the same Signal session.
            session_locks: Cache::builder()
                .max_capacity(cache_config.session_locks_capacity.max(1))
                .evict_guard(|m| Arc::strong_count(m) <= 1)
                .build(),
            ensure_inflight: Arc::default(),
            group_metadata_inflight: Arc::default(),
            chat_lanes: Cache::builder()
                .max_capacity(cache_config.chat_lanes_capacity.max(1))
                .evict_guard(|lane: &ChatLane| Arc::strong_count(&lane.enqueue_lock) <= 1)
                .build(),
            lid_pn_cache: Arc::new(LidPnCache::with_config(
                &cache_config.lid_pn_cache,
                cache_config.cache_stores.lid_pn_cache.clone(),
            )),
            ab_props: Arc::new(wacore::store::ab_props::AbPropsCache::new()),
            group_cache: std::sync::OnceLock::new(),

            expected_disconnect: Arc::new(AtomicBool::new(false)),
            intentional_reconnect: AtomicBool::new(false),
            connection_generation: Arc::new(AtomicU64::new(0)),

            recent_messages: cache_config.recent_messages.build_with_ttl(),

            sender_key_device_cache: crate::sender_key_device_cache::SenderKeyDeviceCache::new(
                &cache_config.sender_key_devices_cache,
            ),

            pending_device_sync: crate::pending_device_sync::PendingDeviceSync::new(),

            pending_retries: Arc::new(std::sync::Mutex::new(HashSet::new())),

            pending_lid_refreshes: Arc::new(std::sync::Mutex::new(HashSet::new())),

            message_retry_counts: cache_config.message_retry_counts.build_with_ttl(),

            session_recreate_history: cache_config.session_recreate_history.build_with_ttl(),

            resend_rate_limiter: crate::resend_rate_limiter::ResendRateLimiter::new(
                cache_config.resend_rate_limiter_capacity,
                crate::resend_rate_limiter::DEFAULT_RESEND_BURST,
                crate::resend_rate_limiter::DEFAULT_RESEND_REFILL_PER_MIN,
            ),

            undecryptable_dispatched: cache_config.undecryptable_dispatched.build_with_ttl(),

            offline_sync_metrics: Arc::new(OfflineSyncMetrics {
                active: AtomicBool::new(false),
                total_messages: AtomicUsize::new(0),
                processed_messages: AtomicUsize::new(0),
                start_time: std::sync::Mutex::new(None),
            }),
            offline_batch: Arc::new(offline_resume::OfflineBatchCoordinator::new()),

            enable_auto_reconnect: Arc::new(AtomicBool::new(true)),
            paused: AtomicBool::new(false),
            pause_state_notifier: Arc::new(event_listener::Event::new()),
            pause_teardown_pending: AtomicBool::new(false),
            pause_generation: AtomicU64::new(0),
            connection_publish: Mutex::new(()),
            auto_reconnect_errors: Arc::new(AtomicU32::new(0)),
            connected_at_ms: Arc::new(AtomicI64::new(0)),
            backoff_reset_suppressed: Arc::new(AtomicBool::new(false)),

            needs_initial_full_sync: Arc::new(app_state::BootstrapGate::new(false)),

            app_state_processor: std::sync::OnceLock::new(),
            app_state_key_requests: Arc::new(Mutex::new(HashMap::new())),
            app_state_syncing: app_state::SyncInFlight::new(),
            app_state_send_lock: Arc::new(Mutex::new(())),
            initial_keys_synced_notifier: Arc::new(event_listener::Event::new()),
            initial_app_state_keys_received: Arc::new(AtomicBool::new(false)),
            prekey_upload_lock: Arc::new(Mutex::new(())),
            signed_pre_key_rotation_lock: Arc::new(Mutex::new(())),
            offline_sync_notifier: Arc::new(event_listener::Event::new()),
            offline_sync_completed: Arc::new(AtomicBool::new(false)),
            offline_sync_finish_started: Arc::new(AtomicBool::new(false)),
            offline_receipt_buffer: std::sync::Mutex::new(Vec::new()),
            inbound_commit_batch: Default::default(),
            history_sync_activity: Arc::new(crate::sync_task::HistorySyncActivity::new()),
            outbound_flush: Arc::new(crate::flush_scope::FlushScope::new()),
            delivery_receipt_queue: std::sync::OnceLock::new(),
            transport_ack_queue: std::sync::OnceLock::new(),
            presence_subscriptions: Arc::new(std::sync::Mutex::new(HashSet::new())),
            socket_ready_notifier: Arc::new(event_listener::Event::new()),
            is_ready: Arc::new(AtomicBool::new(false)),
            connected_notifier: Arc::new(event_listener::Event::new()),
            authenticated_generation: Arc::new(AtomicU64::new(NO_AUTHENTICATED_GENERATION)),
            session_state_notifier: Arc::new(event_listener::Event::new()),
            major_sync_task_sender: tx,
            pairing_cancellation_tx: Arc::new(Mutex::new(None)),
            pairing_qr_refresh_tx: Arc::new(Mutex::new(None)),
            pair_code_state: Arc::new(Mutex::new(wacore::pair_code::PairCodeState::default())),
            subsystems: subsystem::Subsystems::default(),
            signal_flush_state: AtomicU64::new(0),
            signal_flush_lifecycle: Mutex::new(()),
            #[cfg(test)]
            signal_flush_test_failures: AtomicU32::new(0),
            #[cfg(test)]
            signal_flush_test_block: AtomicBool::new(false),
            #[cfg(test)]
            signal_flush_test_in_attempt: AtomicU32::new(0),
            #[cfg(test)]
            app_state_key_share_prepare_test_failures: AtomicU32::new(0),
            #[cfg(test)]
            chatstate_events_built: AtomicU32::new(0),
            custom_enc_handlers: std::sync::OnceLock::new(),
            inbound_durability_hook: std::sync::OnceLock::new(),
            retry_admission: std::sync::OnceLock::new(),
            chatstate_handlers: Arc::new(std::sync::RwLock::new(Arc::from([]))),
            chatstate_handler_count: AtomicUsize::new(0),
            pdo_pending_requests: cache_config.pdo_pending_requests.build_with_ttl(),
            pdo_requested: cache_config.pdo_requested.build_with_ttl(),
            device_registry_cache: device_topology::DeviceRegistryCache::new(
                cache_config.device_registry_cache.build_typed_ttl(
                    cache_config.cache_stores.device_registry_cache.clone(),
                    "device_registry",
                ),
                Arc::clone(&device_topology),
            ),
            device_topology,
            device_memos_enabled: cache_config.cache_stores.device_registry_cache.is_none()
                && cache_config.cache_stores.lid_pn_cache.is_none(),
            group_devices_memo: Cache::builder()
                .max_capacity(GROUP_DEVICES_MEMO_CAPACITY)
                .build(),
            dm_devices_memo: Cache::builder()
                .max_capacity(DM_DEVICES_MEMO_CAPACITY)
                .build(),
            #[cfg(test)]
            dm_devices_memo_recomputes: AtomicU64::new(0),
            device_memo_counters: DeviceMemoCounters::default(),
            // A live lane also protects recipient-tracker reset/update ordering.
            group_distribution_locks: Cache::builder()
                .max_capacity(cache_config.group_distribution_locks_capacity.max(1))
                .evict_guard(|m| Arc::strong_count(m) <= 1)
                .build(),
            skdm_warm_memo: Cache::builder()
                .max_capacity(GROUP_DEVICES_MEMO_CAPACITY)
                .build(),
            stanza_router: Self::create_stanza_router(),
            synchronous_ack: false,
            http_client,
            override_version,
            skip_history_sync: AtomicBool::new(false),
            wanted_pre_key_count: AtomicUsize::new(crate::prekeys::DEFAULT_WANTED_PRE_KEY_COUNT),
            cache_config,
            self_weak: std::sync::OnceLock::new(),
            saver_handle: std::sync::OnceLock::new(),
            alloc_meter: std::sync::OnceLock::new(),
            raw_node_forwarding: AtomicUsize::new(0),
            decrypted_payload_forwarding: AtomicUsize::new(0),
            enc_decrypt_failed_forwarding: AtomicUsize::new(0),
            sent_frame_tap,
            stanza_interceptors: std::sync::RwLock::new(Arc::new(Vec::new())),
            stanza_interceptor_count: AtomicUsize::new(0),
            next_interceptor_id: AtomicU64::new(0),
        };

        let arc = Arc::new(this);
        // Mapping changes alter which canonical record a device lookup
        // resolves to, so LidPnCache records into the same topology tracker.
        arc.lid_pn_cache
            .attach_topology(Arc::clone(&arc.device_topology));
        let _ = arc.self_weak.set(Arc::downgrade(&arc));

        ClientAssembly::new(arc, rx)
    }

    pub(super) fn start_services(self: &Arc<Self>) {
        let warm_up_arc = self.clone();
        self.runtime
            .spawn(Box::pin(async move {
                if let Err(e) = warm_up_arc.warm_up_lid_pn_cache().await {
                    warn!("Failed to warm up LID-PN cache: {e}");
                }
            }))
            .detach();
    }

    /// Run the session: connect, read the socket until the connection ends,
    /// reconnect, repeat.
    ///
    /// This is the call that reads. [`connect`](Self::connect) only establishes
    /// the socket and hands back a [`Connection`]; until that connection is
    /// driven, by this loop or by [`Connection::read_until_disconnected`], no
    /// frame is decoded and no event is emitted.
    ///
    /// Returns when the session is over for good: [`disconnect`](Self::disconnect),
    /// [`logout`](Self::logout), or a connection ending with auto-reconnect off.
    /// [`pause`](Self::pause) does not end it — the loop parks and this call
    /// keeps running until [`resume`](Self::resume) or one of the above.
    // Deliberately NOT instrumented: this span would live for the entire client
    // lifetime, distorting duration/throughput metrics just like the removed
    // keepalive-loop span. Identity (lid/pn) attribution comes from the
    // per-operation spans (send/request), which record it themselves.
    pub async fn run(self: &Arc<Self>) {
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &self.lifecycle
            && !lifecycle.wait_until_active().await
        {
            warn!("Client `run` rejected before construction completed.");
            return;
        }
        let shutdown = self.shutdown_signal();
        if shutdown.is_fired() {
            warn!("Client `run` called after shutdown.");
            return;
        }
        if self.is_running.swap(true, Ordering::SeqCst) {
            warn!("Client `run` method called while already running.");
            return;
        }
        if shutdown.is_fired() {
            self.is_running.store(false, Ordering::SeqCst);
            return;
        }
        // Reconnects are counted at iteration start: every pass after the
        // first is an attempt actually being made. Counting at the branches
        // below would also count a final pass that never reconnects (a user
        // disconnect() flips is_running while the branch runs).
        let mut first_connect = true;
        while self.is_running.load(Ordering::Relaxed) {
            // The one place a pause is honoured, and it is before the attempt
            // rather than after one fails: a `pause()` can land during a
            // reconnect backoff as easily as during a live connection, and
            // either way the next thing this loop must not do is connect.
            // Parking here rather than where the connection ended is what makes
            // that true for both. `continue` re-tests `is_running`, so a
            // `disconnect()` during the pause ends the session from here.
            if self.paused.load(Ordering::Relaxed) {
                info!("Session paused; not connecting until resumed.");
                self.wait_while_paused().await;
                continue;
            }
            if !first_connect {
                self.stats.record_reconnect();
            }
            first_connect = false;
            self.expected_disconnect.store(false, Ordering::Relaxed);

            match self.connect().await {
                Err(connect_err) => {
                    wacore::telemetry::connect("fail");
                    match &connect_err {
                        // The loop is about to exit on the same shutdown, so this
                        // is the teardown reporting itself, not a failure.
                        ConnectError::Shutdown => {
                            debug!("Connect abandoned, the client is shutting down.");
                        }
                        // The other requested refusal, and the same shape: the
                        // loop is about to park on the pause that caused it, so
                        // this is the teardown reporting itself.
                        ConnectError::Paused => {
                            debug!("Connect abandoned, the session is paused.");
                            // Retracted by a pause, so no backoff is owed even
                            // if a resume has already cleared the flag this
                            // branch cannot see any more.
                            self.pause_teardown_pending.store(true, Ordering::Relaxed);
                        }
                        ConnectError::Handshake(e) if e.is_transient() => {
                            debug!("Transient connect failure, will retry: {connect_err:#}");
                        }
                        _ => error!("Failed to connect: {connect_err:#}. Will retry..."),
                    }
                }
                Ok(connection) => {
                    wacore::telemetry::connect("ok");
                    let _ = connection.read_until_disconnected().await;
                }
            }

            // The application having asked this client to stop, which
            // `expected_disconnect` cannot answer on its own — twice over. The
            // post-pairing 515 sets that flag too, and every attempt *clears* it
            // at the top of this loop, so a `disconnect()` landing during the
            // attempt (a window as wide as the 20s connect timeout) leaves no
            // trace in it at all. Left to it, the loop announced a reconnect
            // that the shutdown had already ruled out, then fell straight out of
            // the `while` — the log claiming the opposite of what had happened.
            //
            // Asked instead from the two signals a per-attempt reset cannot
            // reach: the shutdown latch, set once and never cleared by
            // `disconnect`, `logout` and `signal_shutdown_sync`, and
            // `is_running`, this loop's own stop condition. The trailing pair is
            // the same question at the ordering the flags are published in — the
            // Acquire pairs with the Release in `publish_terminal_verdict`,
            // which clears the reader flag *before* publishing the verdict, so a
            // verdict read here guarantees the cleared flag is visible to the
            // load beside it. That covers the window where the stores have
            // landed but the latch's notify has not.
            //
            // What is left is a `disconnect()` that *begins* after this question
            // has been answered, and no check placed earlier can observe one:
            // the lines below report the state at the moment they are written,
            // which is all a log can do. The loop is still correct there — the
            // `while` condition, and the shutdown the backoff races, both see
            // it — so the cost is one already-stale line, not a session that
            // fails to end.
            if shutdown.is_fired()
                || !self.is_running.load(Ordering::Relaxed)
                || (self.expected_disconnect.load(Ordering::Acquire)
                    && !self.is_running.load(Ordering::Relaxed))
            {
                self.clear_connection_backoff_state();
                info!("Disconnect requested, shutting down without reconnecting.");
                break;
            }

            if !self.enable_auto_reconnect.load(Ordering::Relaxed) {
                info!("Auto-reconnect disabled, shutting down.");
                self.stop_supervision_loop();
                break;
            }

            // If this was an expected disconnect (e.g., 515 after pairing), reconnect immediately
            if self.expected_disconnect.load(Ordering::Relaxed) {
                self.clear_connection_backoff_state();
                info!("Expected disconnect (e.g., 515), reconnecting immediately...");
                continue;
            }

            // Registered before the pause state is read, not at the `select!`
            // below: `event_listener` is edge-triggered, so a pause and resume
            // that both land in the gap between the read and a later
            // registration are lost, and the loop then serves a delay of up to
            // the 900s cap that `resume` promised it would not.
            let pause_changed = self.pause_state_notifier.listen();

            // A `pause()` is what ended this connection, so no backoff is owed
            // and none is announced: the wait belongs to the park at the top of
            // the loop, and computing a delay here would only log a reconnect
            // time that a resume is free to beat.
            //
            // Read as a one-shot fact about the connection that just ended, not
            // as current state: `pause()` returns once the socket is closed,
            // which is before this loop has finished unwinding, so a `resume()`
            // in that window would otherwise leave this reading `paused` as
            // false and charging the ordinary backoff for a teardown the
            // application requested.
            if self.pause_teardown_pending.swap(false, Ordering::Relaxed)
                || self.paused.load(Ordering::Relaxed)
            {
                self.clear_connection_backoff_state();
                continue;
            }

            // Reset the backoff only after a stable connection, unless an
            // explicit penalty (429 / manual reconnect) must survive — WA Web
            // `resetDelay` + `cancelReset`.
            let connected_at = self.connected_at_ms.swap(0, Ordering::Relaxed);
            let penalty = self.backoff_reset_suppressed.load(Ordering::Relaxed);
            if should_reset_backoff(connected_at, wacore::time::now_millis(), penalty) {
                self.auto_reconnect_errors.store(0, Ordering::Relaxed);
            }

            let error_count = self.auto_reconnect_errors.fetch_add(1, Ordering::SeqCst);
            // WA Web: Fibonacci backoff with 10% jitter, max 900s.
            // algo: { type: "fibonacci", first: 1000, second: 1000 }
            // jitter: 0.1, max: 9e5
            let delay = fibonacci_backoff(error_count);
            info!(
                "Will attempt to reconnect in {:?} (attempt {})",
                delay,
                error_count + 1
            );
            // Race the wait against the terminal shutdown: the loop only tests
            // `is_running` at the top, so a bare sleep would hold a shutdown
            // unobserved for as long as the backoff runs — up to the 900s cap,
            // which a 429 reaches in a couple of stream errors. Falling through
            // is the whole fix; the loop condition handles the exit.
            //
            // Fresh listener per iteration (event_listener is edge-triggered);
            // `shutdown` itself is subscribed once above and holds the notifier
            // alive. Deliberately NOT `connection_shutdown_signal()`: that one
            // fires on every disconnect the loop is here to reconnect from, so
            // watching it would collapse the backoff instead of interrupting it.
            let shutdown_fired = wacore::runtime::wait_for_shutdown(&shutdown);
            // Third arm for the pause state, on the listener registered
            // above. `pause()` landing here must not wait out a delay before
            // parking, and `resume()` must not wait one out at all — the
            // contract is that a pause owes no backoff, and the cap is 900s.
            // Deliberately NOT `session_state_notifier`, which every teardown
            // fires: watching that would collapse the backoff this loop exists
            // to serve and hammer the server. Only the two transitions that
            // have to be seen promptly fire this one, and the loop re-reads the
            // state either way.
            futures::select! {
                _ = self.runtime.sleep(delay).fuse() => {}
                _ = shutdown_fired.fuse() => {
                    debug!("Shutdown signalled during reconnect backoff, exiting run loop.");
                }
                _ = pause_changed.fuse() => {
                    debug!("Pause state changed during reconnect backoff, re-reading it.");
                }
            }
        }
        #[cfg(feature = "client-lifecycle")]
        self.shutdown_lifecycle().await;
        info!("Client run loop has shut down.");
    }

    /// Open one connection and hand it back for the caller to read.
    ///
    /// The handshake is done when this returns and that is all it does: the
    /// server's frames queue in the transport channel until the returned
    /// [`Connection`] is driven. [`run`](Self::run) is that read plus the
    /// reconnect loop, and is what a session normally wants.
    ///
    /// This used to resolve to `Result<(), ConnectError>`, and a caller that
    /// awaited it and then waited for events waited forever. The old call is
    /// now an `unused_must_use`:
    ///
    /// ```compile_fail
    /// #![deny(unused_must_use)]
    /// # use std::sync::Arc;
    /// # use whatsapp_rust::{Client, ConnectError};
    /// # async fn before(client: &Arc<Client>) -> Result<(), ConnectError> {
    /// client.connect().await?;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// ```no_run
    /// # use std::sync::Arc;
    /// # use whatsapp_rust::{Client, ConnectError};
    /// # async fn after(client: &Arc<Client>) -> Result<(), ConnectError> {
    /// client.connect().await?.read_until_disconnected().await;
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Boxed barrier: see [`crate::bot::Bot::run`]. Coroutines are LocalCopy
    /// across crates, so consumers awaiting the connect graph directly would
    /// re-codegen it; the box makes them poll through a vtable instead.
    pub async fn connect(self: &Arc<Self>) -> Result<Connection<'_>, ConnectError> {
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &self.lifecycle
            && !lifecycle.wait_until_active().await
        {
            return Err(ConnectError::NotActivated);
        }
        // Same refusal `run` makes: a shutdown is published once and for good,
        // and a connection established after it would be one the application
        // has already been told does not exist. A new client is the way back.
        if self.shutdown_signal().is_fired() {
            return Err(ConnectError::Shutdown);
        }
        // A pause is a standing instruction that no connection exists, and it
        // has to bind the act of connecting rather than the loop that calls it:
        // a flag only the loop consults is one an in-flight attempt can outrun,
        // publishing a live socket for a client the application has parked.
        // Refused here and re-read at every step below, `pause()` means what it
        // says. Not final, unlike a shutdown — `resume()` lifts it.
        if self.is_paused() {
            return Err(ConnectError::Paused);
        }
        self.connect_boxed().await
    }

    /// Why this connection attempt must not go on, if it must not.
    ///
    /// Re-asked at every step of the connect graph, because both answers can
    /// arrive while a version fetch, a transport and a handshake are being
    /// awaited — a window as wide as the 20s connect timeout. Checked once at
    /// the start, either one would be published straight over.
    fn connect_refusal(&self, attempt_pause_generation: u64) -> Option<ConnectError> {
        if self.shutdown_signal().is_fired() {
            return Some(ConnectError::Shutdown);
        }
        // The generation, not just the flag: a `pause()` and `resume()` that
        // both complete while this attempt is awaiting its handshake leave every
        // level-triggered read of the flag saying `false`, and the attempt sails
        // on — carrying an `outbound_flush` scope it reopened before the pause
        // closed, so its receipts, retries and transport acks would be dropped
        // for the life of the connection. An attempt that spanned a pause is not
        // the connection the resume asked for; the next one is.
        if self.is_paused()
            || self.pause_generation.load(Ordering::SeqCst) != attempt_pause_generation
        {
            return Some(ConnectError::Paused);
        }
        None
    }

    #[inline(never)]
    fn connect_boxed(
        self: &Arc<Self>,
    ) -> wacore::runtime::BoxFuture<'_, Result<Connection<'_>, ConnectError>> {
        Box::pin(self.connect_graph())
    }

    /// One connection from its first frame to its teardown: read until the
    /// socket ends, clear the connection state, and announce an end nobody
    /// asked for. Shared by [`run`](Self::run)'s loop body and by the
    /// single-connection [`Connection::read_until_disconnected`], so the two
    /// cannot drift on what a connection ending means.
    async fn drive_connection(self: &Arc<Self>) -> Option<DisconnectReason> {
        // Started here rather than at the end of connect: its ping goes through
        // `can_reach_server`, so a tick taken before anything reads is refused
        // as NotConnected, which the loop classifies as fatal and exits on,
        // leaving the connection with no idle ping and no dead-socket watchdog.
        let keepalive = self.clone();
        self.runtime
            .spawn(Box::pin(async move { keepalive.keepalive_loop().await }))
            .detach();

        let loop_result = self.read_messages_loop().await;
        // Consume intentional_reconnect on EVERY exit, reading it AFTER the loop
        // ends (reconnect() sets it while the loop runs, then tears down via the
        // shutdown signal — the Expected path). Consuming it only on some paths
        // left it stale for the next connection, misclassifying the next genuine
        // disconnect as intentional and swallowing its Disconnected event.
        let intentional = self.intentional_reconnect.swap(false, Ordering::Relaxed);
        // Some(reason) = unexpected disconnect worth a `Disconnected` event; the
        // reason distinguishes a routine server recycle from a real failure so
        // consumers don't have to.
        let unexpected_disconnect = match loop_result {
            Ok(node_io::ReadLoopExit::Expected) => {
                debug!("Message loop exited gracefully (expected disconnect).");
                None
            }
            Ok(node_io::ReadLoopExit::ServerRecycle(reason)) => {
                if self.expected_disconnect.load(Ordering::Relaxed) || intentional {
                    debug!("Message loop exited during expected disconnect.");
                    None
                } else {
                    // read_messages_loop already logged this at info; a clean
                    // recycle stays quiet here too.
                    Some(reason)
                }
            }
            Err(e) => {
                if self.expected_disconnect.load(Ordering::Relaxed) || intentional {
                    debug!("Message loop exited during expected disconnect.");
                    None
                } else {
                    // read_messages_loop already logged the cause at warn; keep
                    // this at debug to avoid double-reporting.
                    debug!("Message loop exited, will reconnect if enabled: {e:#}");
                    Some(e.into_reason())
                }
            }
        };

        self.cleanup_connection_state().await;

        // Dispatch after cleanup so handlers see cleared connection state.
        if let Some(reason) = unexpected_disconnect.clone() {
            self.core.event_bus.dispatch(Event::Disconnected(
                crate::types::events::Disconnected::builder()
                    .reason(reason)
                    .build(),
            ));
        }
        unexpected_disconnect
    }

    /// Hand a connection to a test without a server to handshake with.
    #[cfg(test)]
    pub(crate) fn connection_for_test(self: &Arc<Self>) -> Connection<'_> {
        Connection { client: self }
    }

    // err(level = "warn", ...): run()'s caller already classifies failures here itself
    // (debug! for a transient HandshakeError worth a quiet retry, error! otherwise — see
    // run()'s connect_err handling) — the default ERROR level on this span ignored that
    // and turned every transient handshake retry into its own GlitchTip issue. A genuine
    // failure still surfaces via that caller's error! call, independent of this span's level.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.conn.connect",
            level = "info",
            skip_all,
            fields(lid = tracing::field::Empty, pn = tracing::field::Empty),
            err(level = "warn", Debug)
        )
    )]
    async fn connect_graph(self: &Arc<Self>) -> Result<Connection<'_>, ConnectError> {
        #[cfg(feature = "tracing")]
        self.record_identity_on_span(&tracing::Span::current());

        if self.is_connecting.swap(true, Ordering::SeqCst) {
            return Err(ConnectError::AlreadyConnected);
        }

        let _guard = scopeguard::guard((), |_| {
            self.is_connecting.store(false, Ordering::Relaxed);
        });

        if self.is_connected() {
            return Err(ConnectError::AlreadyConnected);
        }
        let _t = wacore::telemetry::timer(wacore::telemetry::CONNECT_DURATION);
        // Read once, compared at every checkpoint below: this is the attempt's
        // claim to belong to the current pause era.
        let attempt_pause_generation = self.pause_generation.load(Ordering::SeqCst);

        // Reset login state for new connection attempt. This ensures that
        // handle_success will properly process the <success> stanza even if
        // a previous connection's post-login task bailed out early.
        self.is_logged_in.store(false, Ordering::Relaxed);
        self.is_ready.store(false, Ordering::Relaxed);
        self.is_connected.store(false, Ordering::Relaxed);
        // The retired connection's verdict, not this one's. Inherited, it makes
        // the read loop exit on the first frame it decodes and `handle_success`
        // refuse the login. `run` already clears it before each attempt; a
        // caller driving connections itself has nowhere else to learn of it.
        self.expected_disconnect.store(false, Ordering::Relaxed);
        self.offline_sync_completed.store(false, Ordering::Relaxed);
        self.offline_sync_finish_started
            .store(false, Ordering::Relaxed);
        self.clear_offline_receipt_buffer();
        // Uncommitted batch entries were never acked; the server redelivers
        // them on this fresh connection. The cache decision is coupled to the
        // drop: entries present here mean their cache-only ratchet advances
        // have no rows (e.g. a stanza that outlived the teardown settle and
        // enqueued late), and flushing those later would make each redelivery
        // an ackable duplicate — so the cache falls with them. With nothing
        // dropped, anything resident is state a failed teardown flush
        // deliberately retained (committed/acked, never redelivered) for the
        // next successful flush to persist.
        if self.inbound_commit_batch.reset() {
            log::warn!(
                "connect: dropping unflushed Signal state along with late uncommitted drain entries"
            );
            self.signal_cache.clear().await;
        }
        self.offline_batch.reset();
        self.outbound_flush.reopen();

        // WA Web: both MQTT and DGW transports use a 20s connect timeout.
        // Without this, a dead network blocks on the OS TCP SYN timeout (~60-75s).
        // Version fetch is also wrapped so a hung HTTP request doesn't block connect().
        let version_future = rt_timeout(
            &*self.runtime,
            TRANSPORT_CONNECT_TIMEOUT,
            crate::version::resolve_and_update_version(
                &self.persistence_manager,
                &self.http_client,
                self.override_version,
            ),
        );
        let transport_future = rt_timeout(
            &*self.runtime,
            TRANSPORT_CONNECT_TIMEOUT,
            self.transport_factory.create_transport(),
        );

        debug!("Connecting WebSocket and fetching latest client version in parallel...");
        let (version_result, transport_result) = futures::join!(version_future, transport_future);

        version_result
            .map_err(|_| ConnectError::Timeout {
                stage: ConnectStage::VersionFetch,
                timeout: TRANSPORT_CONNECT_TIMEOUT,
            })?
            .map_err(ConnectError::Version)?;
        let (transport, mut transport_events) = transport_result
            .map_err(|_| ConnectError::Timeout {
                stage: ConnectStage::Transport,
                timeout: TRANSPORT_CONNECT_TIMEOUT,
            })?
            .map_err(ConnectError::Transport)?;
        debug!("Version fetch and transport connection established.");

        // The check in `connect` cannot stand for the whole attempt: a version
        // fetch, a transport and a handshake are awaited after it, and a
        // shutdown landing in that window would otherwise be published over.
        // Re-read before each remaining step, and hand back the socket opened
        // for a client that is no longer there.
        if let Some(refusal) = self.connect_refusal(attempt_pause_generation) {
            transport.disconnect().await;
            return Err(refusal);
        }

        let noise_socket = match handshake::do_handshake(
            self.runtime.clone(),
            &self.persistence_manager,
            &self.ik_handshake_failures,
            transport.clone(),
            &mut transport_events,
            crate::socket::noise_socket::SendObservers::with_stats(self.stats.clone())
                .with_sent_frames(self.sent_frame_tap.clone()),
        )
        .await
        {
            Ok(socket) => socket,
            Err(e) => {
                transport.disconnect().await;
                return Err(e.into());
            }
        };

        if let Some(refusal) = self.connect_refusal(attempt_pause_generation) {
            transport.disconnect().await;
            return Err(refusal);
        }

        // Fresh per-connection shutdown so subscribers registered during this
        // connection see a clean signal; the previous notifier was already
        // fired on the prior cleanup_connection_state.
        self.reset_connection_shutdown();

        // Invalidated before the socket is published, not after `<success>`
        // lands. `handle_success` sets `is_logged_in` one step before it
        // increments the generation, and the value left behind by the *previous*
        // connection equals the generation still in place during that step — so
        // without this the window reads as authenticated on a generation the
        // next instruction retires. Nothing is authenticated until this
        // connection says so itself.
        self.authenticated_generation
            .store(NO_AUTHENTICATED_GENERATION, Ordering::SeqCst);

        // The final refusal, the publish and the connected flag are one section,
        // taken against `pause()`'s own capture. Checked and then published
        // without it, a pause landing between the two reads `is_connected` as
        // false and captures no transport, while this goes on to publish and
        // flag a connection nothing will ever tear down. Nothing here awaits
        // network I/O — the slot writes are the whole of it — so this is not the
        // hazard the pause/resume ordering lock was.
        let orphan = {
            let _publish = self.connection_publish.lock().await;

            *self.transport.lock().await = Some(transport);
            *self.transport_events.lock().await = Some(transport_events);
            *self.noise_socket.lock().unwrap_or_else(|p| p.into_inner()) = Some(noise_socket);

            // Nothing has been announced yet, so undoing the publish is the
            // whole retraction.
            match self.connect_refusal(attempt_pause_generation) {
                Some(refusal) => {
                    let orphan = self.transport.lock().await.take();
                    *self.transport_events.lock().await = None;
                    *self.noise_socket.lock().unwrap_or_else(|p| p.into_inner()) = None;
                    Some((orphan, refusal))
                }
                None => {
                    self.is_connected.store(true, Ordering::Release);
                    None
                }
            }
        };
        // Closed outside the section: this one does write to the network.
        if let Some((orphan, refusal)) = orphan {
            if let Some(orphan) = orphan {
                orphan.disconnect().await;
            }
            return Err(refusal);
        }

        // Notify waiters that socket is ready (before login)
        self.socket_ready_notifier.notify(usize::MAX);

        Ok(Connection { client: self })
    }

    /// Deregister this companion device and disconnect.
    /// Does NOT wipe stored keys. Delete the storage backend to fully clear credentials.
    ///
    /// Infallible on purpose: the deregistration IQ is best-effort (it cannot be
    /// sent at all while offline), and the local teardown runs either way, so a
    /// caller has nothing to branch on. A failed IQ is logged at warn.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.logout", level = "info", skip_all)
    )]
    pub async fn logout(self: &Arc<Self>) {
        use wacore::iq::devices::RemoveCompanionDeviceSpec;

        self.enable_auto_reconnect.store(false, Ordering::Relaxed);

        if self.is_connected()
            && let Ok(jid) = self.require_pn()
            && let Err(e) = self.execute(RemoveCompanionDeviceSpec::new(&jid)).await
        {
            warn!("Failed to send logout IQ: {e}");
        }

        self.core.event_bus.dispatch(Event::LoggedOut(
            crate::types::events::LoggedOut::builder()
                .on_connect(false)
                .reason(ConnectFailureReason::LoggedOut)
                .build(),
        ));

        self.disconnect().await;
    }

    /// End this client's session for good: close the socket, stop the run
    /// loop, and flush what is still pending.
    ///
    /// Terminal, not a pause. The shutdown it fires is published once and for
    /// all, so afterwards [`run`](Self::run) returns immediately and
    /// [`connect`](Self::connect) refuses with [`ConnectError::Shutdown`] — a
    /// new [`Client`] over the same store is the way back. That is the semantics
    /// [`logout`](Self::logout) relies on by ending with this call, and the one
    /// [`signal_shutdown_sync`](Self::signal_shutdown_sync) reproduces for
    /// `Drop`.
    ///
    /// What it does **not** offer is a session you can pick up again. To drop
    /// the current connection and have the client come back on its own, use
    /// [`reconnect`](Self::reconnect) or
    /// [`reconnect_immediately`](Self::reconnect_immediately): those leave the
    /// run loop in place, which is what makes them reversible.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.disconnect", level = "info", skip_all)
    )]
    pub async fn disconnect(self: &Arc<Self>) {
        info!("Disconnecting client intentionally.");
        wacore::telemetry::set_connected(false);
        self.publish_terminal_verdict();
        self.shutdown_notifier.notify();
        self.notify_session_state();
        #[cfg(feature = "client-lifecycle")]
        self.request_lifecycle_shutdown();

        // Drain buffered offline receipts into the flush window before
        // closing it, so a disconnect mid-offline-sync still acks the
        // already-processed backlog (issue #571 semantics). close() only stops
        // outbound task spawns, not buffering, so a message still in flight can
        // re-buffer after this drain; those entries are dropped by the
        // connection-state reset (clear_offline_receipt_buffer) and the server
        // redelivers their messages on the next connect, where they are
        // re-acked fresh.
        //
        // Commit any accumulated drain batch first so its acks land in this
        // receipt drain. Bounded like the outbound flush below: on timeout the
        // entries simply stay unacked and the server redelivers them — and the
        // buffered receipts stay unsent too, because their SKDM/session state
        // may not be durable yet (receipting an SKDM whose sender key only
        // lives in the cache would lose it to a crash with no redelivery).
        if self
            .flush_inbound_commits_bounded(Duration::from_secs(5))
            .await
        {
            self.flush_offline_receipts();
        }
        // Prevent late receipt producers from escaping the drain window.
        self.outbound_flush.close();
        self.outbound_flush
            .flush(&*self.runtime, Duration::from_secs(5))
            .await;
        self.notify_connection_shutdown();

        if let Err(e) = self.persistence_manager.flush().await {
            log::error!("Failed to flush device state during disconnect: {e}");
        }

        // Close after flush; cleanup may also win this race on the run loop.
        // Guard dropped first: edition 2024 keeps an `if let` scrutinee temporary alive
        // for the whole matching arm, so holding it across `disconnect()` (an untimed
        // socket write) would park `connect_internal`, which installs through this mutex.
        let transport = self.transport.lock().await.clone();
        if let Some(transport) = transport {
            transport.disconnect().await;
        }
        self.cleanup_connection_state().await;

        // The write-behind secret drain is detached; a clean exit right after
        // a capture must not lose the only copy. Sealing first degrades any
        // straggler capture (a lane worker still draining its backlog) to an
        // inline write, so nothing can land on the detached drain after the
        // final flush below and then be acked.
        self.msg_secret_buffer.seal();
        self.msg_secret_buffer.flush().await;
        #[cfg(feature = "client-lifecycle")]
        self.shutdown_lifecycle().await;
    }

    /// Backoff step used by [`reconnect()`](Self::reconnect) to create an offline window.
    ///
    /// `fibonacci_backoff(RECONNECT_BACKOFF_STEP)` determines the delay before
    /// the run loop re-connects.  This must be longer than the mock server's
    /// chatstate TTL (`CHATSTATE_TTL_SECS=3`) so TTL-expiry tests pass.
    ///
    /// Sequence: fib(0)=1s, fib(1)=1s, fib(2)=2s, fib(3)=3s, **fib(4)=5s**.
    pub const RECONNECT_BACKOFF_STEP: u32 = 4;

    /// Drop the current connection and trigger the auto-reconnect loop.
    ///
    /// Unlike [`disconnect`](Self::disconnect), this does **not** stop the run loop. The client
    /// will reconnect automatically using the same persisted identity/store,
    /// just as it would after a network interruption. Use
    /// [`wait_for_connected`](Self::wait_for_connected) to wait for the new connection to be ready.
    ///
    /// This is useful for:
    /// - Handling network changes (e.g., Wi-Fi → cellular)
    /// - Forcing a fresh server session
    /// - Testing offline message delivery
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.reconnect", level = "info", skip_all)
    )]
    pub async fn reconnect(self: &Arc<Self>) {
        info!("Reconnecting: dropping transport for auto-reconnect.");
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.cancel_active_scope();
        }
        wacore::telemetry::reconnect();
        self.intentional_reconnect.store(true, Ordering::Relaxed);
        self.auto_reconnect_errors
            .store(Self::RECONNECT_BACKOFF_STEP, Ordering::Relaxed);
        // Deliberate step: the stability reset must not erase it.
        self.backoff_reset_suppressed.store(true, Ordering::Relaxed);

        // Same durable-before-receipts gate as disconnect().
        if self
            .flush_inbound_commits_bounded(Duration::from_secs(2))
            .await
        {
            self.flush_offline_receipts();
        }
        self.outbound_flush.close();
        self.outbound_flush
            .flush(&*self.runtime, Duration::from_secs(2))
            .await;
        self.notify_connection_shutdown();

        let transport = self.transport.lock().await.clone();
        if let Some(transport) = transport {
            transport.disconnect().await;
        }
    }

    /// Drop the current connection and reconnect immediately with no delay.
    ///
    /// Unlike [`reconnect`](Self::reconnect), which introduces a deliberate offline window,
    /// this method sets the `expected_disconnect` flag so the run loop
    /// skips the backoff delay and reconnects as fast as possible.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.reconnect_immediately", level = "info", skip_all)
    )]
    pub async fn reconnect_immediately(self: &Arc<Self>) {
        info!("Reconnecting immediately (expected disconnect).");
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.cancel_active_scope();
        }
        self.expected_disconnect.store(true, Ordering::Relaxed);

        // Same durable-before-receipts gate as disconnect().
        if self
            .flush_inbound_commits_bounded(Duration::from_secs(2))
            .await
        {
            self.flush_offline_receipts();
        }
        self.outbound_flush.close();
        self.outbound_flush
            .flush(&*self.runtime, Duration::from_secs(2))
            .await;
        self.notify_connection_shutdown();

        let transport = self.transport.lock().await.clone();
        if let Some(transport) = transport {
            transport.disconnect().await;
        }
    }

    /// Drop the current connection and stay offline until [`resume`](Self::resume).
    ///
    /// The middle of the range between [`reconnect`](Self::reconnect), which
    /// comes back on the library's schedule, and [`disconnect`](Self::disconnect),
    /// which does not come back at all. The supervision loop started by
    /// [`run`](Self::run) stays alive and parked, so the future a caller is
    /// awaiting keeps running and the client is **not** terminal — it is between
    /// connections, on purpose, for as long as the application wants.
    ///
    /// What it guarantees: once this returns, the socket is closed, pending
    /// receipts and Signal state have been flushed on the same terms as
    /// `disconnect()`, and **no connection will be opened by anyone** until
    /// resumed. [`connect`](Self::connect) is refused with
    /// [`ConnectError::Paused`] for as long as it holds — including an attempt
    /// already in flight when this was called, which is retracted rather than
    /// published. A guarantee the run loop alone could not give: it consults
    /// its state between attempts, and an attempt can outrun that. Idempotent —
    /// pausing a paused client just tears down again.
    ///
    /// One qualification on "the socket is closed": an attempt still inside its
    /// handshake holds its transport in that future, where this call cannot
    /// reach it. That socket is closed by the attempt itself at its next
    /// refusal check, which can land just after this returns. What holds
    /// unconditionally is the part a caller can act on — no such attempt ever
    /// becomes a published, readable connection.
    ///
    /// What it does **not** do. It dispatches no
    /// [`Event::Disconnected`](crate::types::events::Event) — the application
    /// ended this connection, so the teardown is not news, the same reasoning
    /// `reconnect()` applies. It is not a protocol-level presence change: the
    /// account stays registered and other devices see nothing. And it does not
    /// override `enable_auto_reconnect`: a client whose application has already
    /// asked it not to come back still stops when its connection ends, pause or
    /// no pause, because that is the same preference the stream errors that end
    /// a session for good are read through.
    ///
    /// Resuming reconnects like any other reconnect — new connection
    /// generation, fresh `<success>`, app state re-synced. Nothing is carried
    /// across the pause that a network drop would not also have taken.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.pause", level = "info", skip_all)
    )]
    pub async fn pause(self: &Arc<Self>) {
        // Deliberately NOT serialised against `resume` with a lock. The obvious
        // way to keep a resume from cutting across this teardown is to hold one
        // for the whole of it — and that parks `resume` behind an untimed
        // socket close, so a transport that never answers turns "come back on
        // my word" into a session that never comes back at all. The ordering
        // that actually mattered is the reconnect backoff, and
        // `pause_teardown_pending` below settles that as a fact about the
        // connection rather than a race between two callers.
        info!("Pausing the session: dropping the connection until resumed.");
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.cancel_active_scope();
        }
        // One section with `connect_graph`'s publish, so this call cannot read
        // "no connection" from an attempt that is a statement away from
        // publishing one. Everything in here is a flag or a slot read; the
        // flushes and the socket close are outside it.
        let (had_connection, torn_down) = {
            let _publish = self.connection_publish.lock().await;

            // Before the teardown, so the run loop cannot reach its
            // post-connection branches and start a backoff for a connection that
            // is not coming back on its own. Bumped with it, so an attempt
            // already in flight is refused at its next checkpoint whether or not
            // a resume has landed by then.
            self.paused.store(true, Ordering::SeqCst);
            self.pause_generation.fetch_add(1, Ordering::SeqCst);

            let had_connection = self.is_connected();
            // `intentional_reconnect` keeps a requested drop out of
            // `Event::Disconnected`, and is set only when there is a connection
            // to consume it: with no reader running it would stand into the
            // connection *after* the resume, whose first genuine drop would then
            // go unreported.
            if had_connection {
                self.intentional_reconnect.store(true, Ordering::Relaxed);
            }
            // Not conditional on that, though. It says "the end of this attempt
            // owes no backoff", and an attempt refused before it ever published
            // a connection owes one just as little — otherwise a pause and
            // resume that both land mid-connect leave the loop serving a delay
            // of up to the 900s cap for a retraction the application asked for.
            self.pause_teardown_pending.store(true, Ordering::Relaxed);

            // Bound to the connection this call is ending, before the first
            // await. The flushes below are bounded but not instant, and a
            // `resume()` landing across them can have the run loop publish a
            // replacement into this slot — a lookup made afterwards would close
            // the connection the resume just asked for.
            let torn_down = self.transport.lock().await.clone();
            (had_connection, torn_down)
        };
        // Two audiences: `session_state_notifier` for work parked in
        // `await_connection`, which must re-read a reachability that this call
        // has just changed, and the pause notifier for a run loop sitting in a
        // reconnect backoff it should no longer be serving.
        self.notify_session_state();
        self.pause_state_notifier.notify(usize::MAX);

        // Same durable-before-receipts gate as disconnect().
        if self
            .flush_inbound_commits_bounded(Duration::from_secs(2))
            .await
        {
            self.flush_offline_receipts();
        }
        // Everything from here touches connection-scoped shared state, and a
        // `resume()` across the await above can have a replacement installed
        // that has already reopened the outbound scope and reset the
        // per-connection notifier for itself. Fencing unconditionally would
        // silence the connection the resume asked for — its receipts, retries
        // and transport acks rejected for the life of the connection, its
        // subscribers told to stand down. Asked under the publish section, so a
        // replacement cannot appear between the question and the act it gates.
        let fenced = {
            let _publish = self.connection_publish.lock().await;
            let ours = self.still_owns_connection(torn_down.as_ref()).await;
            if ours {
                self.outbound_flush.close();
            }
            ours
        };
        if fenced {
            self.outbound_flush
                .flush(&*self.runtime, Duration::from_secs(2))
                .await;
            // Re-asked: the drain is another await, and the notifier is the one
            // piece of this that a replacement would feel immediately.
            let _publish = self.connection_publish.lock().await;
            if self.still_owns_connection(torn_down.as_ref()).await {
                self.notify_connection_shutdown();
            }
        }

        if let Some(transport) = &torn_down {
            transport.disconnect().await;
        }

        // A connection nobody is reading has no `drive_connection` to run the
        // teardown behind it, and `connect()` publishes `is_connected` before
        // handing the `Connection` back — so a caller that connected directly
        // and never drove it would be left flagged connected for good, and
        // every later `connect()` refused as `AlreadyConnected`. `is_running`
        // is how the client asks whether anything is reading; when nothing is,
        // this call owns the cleanup.
        //
        // Gated on identity, not on the pause still standing: cleanup clears
        // connection state wholesale, so the question is whether the state still
        // belongs to the connection this call tore down. Asking "am I still
        // paused" instead got that wrong in both directions — it skipped the
        // cleanup after an overlapping resume, leaving a direct caller flagged
        // connected for good, and it would have run it for a replacement.
        if had_connection
            && !self.is_running.load(Ordering::Relaxed)
            && self.still_owns_connection(torn_down.as_ref()).await
        {
            self.cleanup_connection_state().await;
        }
    }

    /// Whether the connection a teardown captured is still the published one.
    ///
    /// The question every step of [`pause`](Self::pause) after its first await
    /// has to ask before touching anything connection-scoped. A `resume()` can
    /// have a replacement installed by then, and that replacement has already
    /// claimed the shared state — the outbound scope, the per-connection
    /// notifier, the connected flag — for itself.
    async fn still_owns_connection(
        &self,
        torn_down: Option<&Arc<dyn crate::transport::Transport>>,
    ) -> bool {
        match (torn_down, &*self.transport.lock().await) {
            (Some(mine), Some(published)) => Arc::ptr_eq(mine, published),
            _ => false,
        }
    }

    /// Release a [`pause`](Self::pause): the run loop reconnects at once, with
    /// no backoff owed for an offline window the application chose.
    ///
    /// Returns once the loop has been told, not once it is connected — wait for
    /// that with [`wait_for_connected`](Self::wait_for_connected). A no-op on a
    /// client that is not paused, and on one that has since been disconnected:
    /// this clears the pause, it does not undo a shutdown.
    ///
    /// Safe to call while a [`pause`](Self::pause) is still tearing down, and
    /// deliberately does not wait for one: the teardown ends in an untimed
    /// socket close, and a resume that queued behind it could be held off for
    /// as long as an unresponsive transport cared to take. "No backoff is owed"
    /// does not depend on the two being ordered — the run loop reads that from
    /// the connection that ended, not from whichever call landed first.
    pub fn resume(&self) {
        if !self.paused.swap(false, Ordering::SeqCst) {
            return;
        }
        info!("Resuming the session.");
        self.notify_session_state();
        self.pause_state_notifier.notify(usize::MAX);
    }

    /// Whether [`pause`](Self::pause) is in effect and no connection will be
    /// opened by [`run`](Self::run) until [`resume`](Self::resume).
    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::Relaxed)
    }

    /// Park the supervision loop for the length of a pause.
    ///
    /// Returns when the pause is released or when nothing is driving the client
    /// any more — `disconnect()` and `logout()` clear `is_running`, and the
    /// caller re-tests it, so a session ended mid-pause ends here rather than
    /// waiting for a resume that is never coming.
    ///
    /// `session_state_notifier` is the wake for both, which is why no notifier
    /// of its own is needed: every transition that changes either answer already
    /// fires it, and a wake that settles nothing simply re-parks.
    async fn wait_while_paused(&self) {
        loop {
            // Registered before the re-check, so a resume landing in the gap
            // between the two is not missed and parked-for-good.
            let changed = self.session_state_notifier.listen();
            if !self.paused.load(Ordering::Relaxed) || !self.is_running.load(Ordering::Relaxed) {
                return;
            }
            changed.await;
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.cleanup", level = "debug", skip_all)
    )]
    #[cfg(not(feature = "client-lifecycle"))]
    pub(crate) async fn cleanup_connection_state(self: &Arc<Self>) {
        self.cleanup_connection_state_inner().await;
        self.clear_connection_scoped_pair_code().await;
    }

    /// A pair-code flow belongs to the connection that carried it: the pairing
    /// ref and any in-flight `companion_hello` die with the socket, and the
    /// server routes no `primary_hello` to a session it has dropped. Left
    /// standing, the outstanding-code guard would reject the very request that
    /// reconnecting exists to make.
    ///
    /// Runs after the inner teardown, so the generation is already retired and
    /// the transport already closed: a request that claims the slot from here
    /// on is one the next connection will carry.
    async fn clear_connection_scoped_pair_code(self: &Arc<Self>) {
        *self.pair_code_state.lock().await = wacore::pair_code::PairCodeState::Idle;
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.cleanup", level = "debug", skip_all)
    )]
    #[cfg(feature = "client-lifecycle")]
    pub(crate) async fn cleanup_connection_state(self: &Arc<Self>) {
        if self.lifecycle.is_none() {
            self.cleanup_connection_state_inner().await;
            self.clear_connection_scoped_pair_code().await;
            return;
        }

        // Scope closure must survive a caller dropping its cleanup waiter.
        let (completed, completion) = futures::channel::oneshot::channel();
        let client = Arc::clone(self);
        self.runtime
            .spawn(Box::pin(async move {
                let result = std::panic::AssertUnwindSafe(client.cleanup_connection_state_inner())
                    .catch_unwind()
                    .await;
                let _ = completed.send(result);
            }))
            .detach();
        match completion.await {
            Ok(Ok(())) => {}
            Ok(Err(panic)) => std::panic::resume_unwind(panic),
            Err(_) => error!("Detached connection cleanup stopped before completion"),
        }
        self.clear_connection_scoped_pair_code().await;
    }

    async fn cleanup_connection_state_inner(&self) {
        #[cfg(feature = "client-lifecycle")]
        let login_transition = self
            .login_transition
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        // Bump the generation FIRST: it is the "this connection is over"
        // signal every per-connection loop already polls. Chat-lane workers
        // stop draining their queues (their remaining stanzas were never
        // acked and redeliver), stale finishers/timers stand down, and —
        // combined with the post-permit generation re-check in
        // process_classified_message — no decrypt can START after the
        // permit-held cache settle below, so no rowless ratchet advances can
        // dirty the cache behind teardown's back.
        #[cfg(feature = "client-lifecycle")]
        let closed_generation = self.connection_generation.fetch_add(1, Ordering::SeqCst);
        #[cfg(not(feature = "client-lifecycle"))]
        self.connection_generation.fetch_add(1, Ordering::SeqCst);
        #[cfg(feature = "client-lifecycle")]
        let scope_close = self.lifecycle.as_ref().map(|lifecycle| {
            let lifecycle = Arc::clone(lifecycle);
            scopeguard::guard((lifecycle, closed_generation), |(lifecycle, generation)| {
                if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    lifecycle.close_scope(generation);
                }))
                .is_err()
                {
                    error!("Client lifecycle scope closure panicked");
                }
            })
        });
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.cancel_scope(closed_generation);
        }
        self.notify_connection_shutdown();
        // The coalesced-flush scheduler needs no explicit reset: its state is
        // generation-scoped, so the bump above already hands ownership to the
        // next connection's first request and retires any stale worker.
        // Note: node_waiters are intentionally NOT cleared here — they are
        // cross-connection (callers may register a waiter before an action that
        // completes on a subsequent connection, e.g. after 515 reconnect).
        // sent_node_waiters ARE cleared because they match pre-encryption
        // outgoing stanzas, which are transport-scoped.
        self.clear_sent_node_waiters();
        self.is_logged_in.store(false, Ordering::Relaxed);
        #[cfg(feature = "client-lifecycle")]
        drop(login_transition);
        self.is_ready.store(false, Ordering::Relaxed);
        // Publish the disconnected state BEFORE draining VoIP calls (it used to be cleared only after
        // the socket teardown below): a concurrent accept()/call() setup that finishes its async work
        // in this window must see `!is_connected()` and bail instead of registering/connecting a call
        // after this sweep.
        self.is_connected.store(false, Ordering::Release);
        // Let each attached subsystem release what this connection owned.
        subsystem::on_connection_cleanup(self).await;
        // Close the socket as part of cleanup so this path is authoritative
        // even when reached via the run loop's graceful-exit flow (not just
        // `Client::disconnect()`). Transport impls make `disconnect()`
        // idempotent, so the redundant call from `Client::disconnect()` is
        // safe.
        // All three slots are cleared before the close is awaited, not after. The guard on
        // `transport` no longer spans that await, so a `connect()` racing this teardown can
        // publish its own transport, events and socket while the close is in flight; clearing
        // afterwards would strip the replacement connection instead of the one being torn down.
        let transport = self.transport.lock().await.take();
        *self.transport_events.lock().await = None;
        *self.noise_socket.lock().unwrap_or_else(|p| p.into_inner()) = None;
        if let Some(transport) = transport {
            transport.disconnect().await;
        }
        // Authoritative point for the gauge: every disconnect (intentional or a
        // run-loop drop/reconnect) funnels through here, so disconnect()'s early
        // set is just a prompt redundant signal. (`is_connected` was already cleared above, before
        // the VoIP drain, so no task can observe is_connected==true with a cleared socket.)
        wacore::telemetry::set_connected(false);
        // Presence doesn't survive reconnects: demote presence-driven active
        // receipts (1 -> 0), leaving a forced value (2) untouched.
        let _ =
            self.send_active_receipts
                .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Acquire);
        // Drop per-chat lanes so workers exit via channel close. Reliable
        // (awaited) clear: a skipped invalidation would leave a stale ChatLane
        // whose worker exits on the generation check after reconnect.
        self.chat_lanes.clear().await;
        // Clear pending retries so stale keys from detached scopeguard
        // cleanup don't suppress the first retry after reconnect.
        self.pending_retries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        // `pending_lid_refreshes` is deliberately NOT cleared here. Its keys are
        // released by a `scopeguard` that runs on drop as well as on completion,
        // so a refresh whose query dies with the socket still frees its own key,
        // and there is nothing stale left to sweep. Clearing anyway would drop a
        // reservation belonging to a live task: a refresh spanning a reconnect
        // would release a key the new connection had since taken, and the peer
        // would get the duplicate query the set exists to prevent.
        // Commit any accumulated drain batch and settle the Signal cache in
        // ONE permit-held section (see teardown_inbound_commits_bounded):
        // persisting ratchet advances while dropping their uncommitted batch
        // entries — or while an old lane worker is mid-decrypt — would turn
        // redeliveries into ackable duplicates with no buffered copy.
        // Acks/events from this commit are best-effort (the socket is gone);
        // the durable hook commit is what matters. Reached on every teardown
        // path, including the run loop's unexpected read-loop exit, which
        // never goes through disconnect().
        //
        // Hold the coalesced-flush barrier across the whole settle: a stale flush
        // worker that already passed its generation check must not interleave a
        // backend write between our commit and the next connection's drain, or it
        // could persist that drain's rowless advances. The worker re-checks the
        // generation (bumped above) once it gets the gate, so it stands down.
        let flush_gate = self.signal_flush_lifecycle.lock().await;
        if let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) {
            client
                .teardown_inbound_commits_bounded(Duration::from_secs(5))
                .await;
        } else {
            // Same class of bug as the complete_offline_sync twin: a silent
            // skip here is the acked-before-committed loss — make it loud,
            // and drop the dirty cache with the entries it covers.
            log::error!(
                "cleanup_connection_state: self_weak upgrade failed; dropping uncommitted drain entries and their unflushed Signal state"
            );
            self.signal_cache.clear().await;
        }
        // Reset semaphore to 1 permit for next offline sync.
        self.swap_message_semaphore(1);
        // Reset dead-socket timestamps so stale values from the previous
        // connection don't trigger an immediate reconnect on the next one.
        self.stats.reset_connection_activity();
        self.pending_device_sync.clear();
        // Reset offline sync state for next connection
        self.offline_sync_completed.store(false, Ordering::Relaxed);
        self.offline_sync_finish_started
            .store(false, Ordering::Relaxed);
        self.clear_offline_receipt_buffer();
        // Same rule as receipts: uncommitted entries drop here and the server
        // redelivers them on the next connect. The cache falls with dropped
        // entries (rowless advances — including a timed-out settle's restored
        // batch); with nothing dropped it survives for the next flush.
        if self.inbound_commit_batch.reset() {
            log::warn!(
                "cleanup_connection_state: dropping unflushed Signal state along with late uncommitted drain entries"
            );
            self.signal_cache.clear().await;
        }
        // Cache is settled and any dropped entries cleared; a worker may run again.
        drop(flush_gate);
        self.offline_batch.reset();
        self.offline_sync_metrics
            .active
            .store(false, Ordering::Release);
        self.offline_sync_metrics
            .total_messages
            .store(0, Ordering::Release);
        self.offline_sync_metrics
            .processed_messages
            .store(0, Ordering::Release);
        match self.offline_sync_metrics.start_time.lock() {
            Ok(mut guard) => *guard = None,
            Err(poison) => *poison.into_inner() = None,
        }
        self.history_sync_activity.reset();
        // Drain all pending IQ waiters so they fail fast with InternalChannelClosed
        // instead of hanging until the 75s timeout.
        // Scoped so the sync guard is dropped before the awaits below (a
        // std::sync::MutexGuard held across an await would make this future !Send).
        let waiter_count = {
            let mut waiters_map = self.response_waiters_guard();
            let count = waiters_map.len();
            // Release the backing storage while preserving the generation
            // sequence; an old request guard may drop after reconnect and must
            // not match a new waiter that reused the same explicit ID.
            waiters_map.clear();
            count
        };
        if waiter_count > 0 {
            debug!(
                "Dropping {} orphaned IQ response waiter(s) on disconnect",
                waiter_count
            );
        }

        // Clear app state tracking maps to prevent unbounded growth across reconnections.
        // Replace with new collections to release backing storage.
        *self.app_state_key_requests.lock().await = HashMap::new();
        self.app_state_syncing.clear();

        // Drop stale media connection (auth tokens become invalid on reconnect)
        *self.media_conn.write().await = None;

        // Clear app state key cache — keys will be re-fetched from DB on demand
        // main took the processor out of the mutex before awaiting so the guard
        // did not span the clear; the write-once cell has no guard to span, so
        // the borrow is the whole of it.
        if let Some(proc) = self.app_state_processor.get() {
            proc.clear_key_cache().await;
        }
        #[cfg(feature = "client-lifecycle")]
        drop(scope_close);
    }

    /// Waits for the noise socket to be established.
    ///
    /// Returns `Ok(())` when the socket is ready, or `Err` on timeout.
    /// This is useful for code that needs to send messages before login,
    /// such as requesting a pair code during initial pairing.
    ///
    /// If the socket is already connected, returns immediately.
    pub async fn wait_for_socket(&self, timeout: Duration) -> Result<(), ConnectError> {
        // Fast path: already connected. Not while paused, though — the flag is
        // published before the teardown clears `is_connected`, and the socket
        // this would report is the one being closed.
        if self.is_connected() && !self.is_paused() {
            return Ok(());
        }

        // Register waiter and re-check to avoid race condition:
        // If socket becomes ready between checks, the notified future captures it.
        let notified = self.socket_ready_notifier.listen();
        if self.is_connected() && !self.is_paused() {
            return Ok(());
        }

        rt_timeout(&*self.runtime, timeout, notified)
            .await
            .map_err(|_| ConnectError::Timeout {
                stage: ConnectStage::Socket,
                timeout,
            })
    }

    /// Waits for the client to establish a connection and complete login.
    ///
    /// Returns `Ok(())` when connected, or `Err` on timeout.
    /// This is useful for code that needs to run after connection is established
    /// and authentication is complete.
    ///
    /// If the client is already connected and logged in, returns immediately.
    pub async fn wait_for_connected(&self, timeout: Duration) -> Result<(), ConnectError> {
        // Fast path: fully ready (connected + logged in + critical sync done).
        if self.is_fully_ready() {
            return Ok(());
        }

        // Register waiter and re-check to avoid TOCTOU race:
        // dispatch_connected() could fire between the check above and notified() registration.
        let notified = self.connected_notifier.listen();
        if self.is_fully_ready() {
            return Ok(());
        }

        rt_timeout(&*self.runtime, timeout, notified)
            .await
            .map_err(|_| ConnectError::Timeout {
                stage: ConnectStage::Ready,
                timeout,
            })
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Acquire)
    }

    /// Force the connected flag for tests that exercise connected-only
    /// operations. Also reached by the `bench-harness` fixture, which drives
    /// the same connected-only send path with no socket behind it.
    #[cfg(any(test, feature = "bench-harness"))]
    pub(crate) fn set_connected_for_test(&self, connected: bool) {
        self.is_connected.store(connected, Ordering::Release);
    }

    pub fn is_logged_in(&self) -> bool {
        self.is_logged_in.load(Ordering::Relaxed)
    }

    /// Whether an IQ sent right now could actually be answered.
    ///
    /// Three separate conditions that were once asked as one, each of which
    /// alone admits a request that cannot come back: a socket, so there is
    /// somewhere to send it; authentication, because `<success>` both makes the
    /// server willing to answer and fixes the generation the answer is admitted
    /// under; and a reader, because an answer nobody decodes is a request that
    /// times out. `is_running` is set by `run` and by
    /// `Connection::read_until_disconnected`, the two calls that read.
    ///
    /// Authentication is read as *the generation is final*, not as
    /// `is_logged_in` alone: that flag is set by the duplicate-`<success>` guard
    /// one step before the increment, and a caller that binds a scope in between
    /// binds a generation the next instruction retires.
    pub(crate) fn can_reach_server(&self) -> bool {
        self.is_connected()
            && self.is_logged_in()
            && self.authenticated_generation.load(Ordering::SeqCst)
                == self.connection_generation.load(Ordering::SeqCst)
            && self.is_running.load(Ordering::Relaxed)
            // A socket already marked for retirement will answer, and then the
            // answer will be thrown away. `reconnect_immediately` sets this
            // before its bounded flushes and closes the transport only after,
            // so the window is wide enough to admit a whole sync that the
            // replacement generation then retires — attempt charged, work lost.
            && !self.expected_disconnect.load(Ordering::Relaxed)
            // Same reasoning as the line above, for the other requested
            // teardown: `pause()` publishes the flag and only then flushes and
            // closes, so the socket answers for the whole of that window and
            // the answer is thrown away with the connection. Asked here rather
            // than at each waiter, so a send, an IQ and a retry all get the
            // same verdict.
            && !self.is_paused()
    }
}

/// An established connection that nothing is reading yet.
///
/// [`Client::connect`] stops at the handshake. The frames the server sends next
/// sit in the transport channel, and it takes
/// [`read_until_disconnected`](Self::read_until_disconnected) to turn them into
/// nodes and events. Hence `#[must_use]`: a connection nobody drives fails by
/// staying silent, with no timeout, error or log to say so.
///
/// [`Client::run`] does the same read inside its reconnect loop. Reach for this
/// instead when the session must not outlive its first connection.
#[must_use = "this connection is not being read: nothing decodes frames or emits events until \
              `read_until_disconnected` drives it (or use `Client::run`)"]
pub struct Connection<'a> {
    client: &'a Arc<Client>,
}

impl Connection<'_> {
    /// Read this connection until it ends, then tear it down.
    ///
    /// Returns the reason an unexpected end carried (the same one dispatched
    /// as `Event::Disconnected` just before returning), or `None` when the end
    /// was not one to report: a requested disconnect, or a protocol step that
    /// ends the stream on purpose, such as the 515 that follows pairing and
    /// expects another connection. No reconnect happens here either way, so a
    /// caller that wants one connects again.
    ///
    /// Dropping this future stops the read without tearing the connection
    /// down, exactly as dropping [`Client::run`] does: the socket stays open
    /// and the client keeps reporting itself connected, so the next
    /// [`Client::connect`] is refused until [`Client::disconnect`] releases
    /// it. Only the reader flag is given back, so a later `run` is not refused
    /// as already running.
    pub async fn read_until_disconnected(self) -> Option<DisconnectReason> {
        // Reading is what the Drop warning asks for, so the drop that ends this
        // call must not fire it.
        let this = std::mem::ManuallyDrop::new(self);
        // `is_running` is how the rest of the client asks whether anything is
        // reading (`can_reach_server`, `is_terminal`). Left false, a request
        // sent over this connection would be refused for want of the reader
        // that is right here. `run` sets it before connecting and owns clearing
        // it, so only a directly driven connection claims it.
        // Guarded, not a store after the await: a caller that drops this future
        // (a wrapping timeout, an aborted task) stops reading too, and a flag
        // left standing would advertise a reader that no longer exists.
        let owns_reader = !this.client.is_running.swap(true, Ordering::SeqCst);
        let _release_reader = owns_reader.then(|| {
            scopeguard::guard((), |_| {
                // Only the run loop consumes the pause marker, and this path is
                // not it. Left standing, a later `resume(); run()` would carry
                // it into the new session and let its first genuine failure pass
                // as a planned pause — clearing the counter and retrying at once
                // instead of honouring the backoff, rate-limit penalty included.
                this.client
                    .pause_teardown_pending
                    .store(false, Ordering::Relaxed);
                this.client.stop_supervision_loop();
            })
        });
        this.client.drive_connection().await
    }
}

impl std::fmt::Debug for Connection<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Connection").finish_non_exhaustive()
    }
}

impl Drop for Connection<'_> {
    fn drop(&mut self) {
        warn!(
            "Connection dropped without being read; nothing will be decoded from it. Drive it with `read_until_disconnected`, or use `Client::run`."
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    /// A connection with one frame already waiting on it, and everything needed
    /// to see whether that frame ever becomes a node.
    ///
    /// The frame is one the client encrypted itself: both directions run the
    /// same test key on independent counters, so the first write decrypts as
    /// the first read.
    struct PendingFrame {
        client: Arc<Client>,
        events: async_channel::Sender<crate::transport::TransportEvent>,
        decoded: Arc<crate::test_utils::TestEventCollector>,
        _raw_nodes: RawNodeLease,
    }

    impl PendingFrame {
        async fn new() -> Self {
            let (client, transport) = crate::test_utils::create_iq_test_client().await;
            let decoded = Arc::new(crate::test_utils::TestEventCollector::default());
            client
                .core
                .event_bus
                .subscribe_handler(decoded.clone())
                .detach();
            let raw_nodes = client.acquire_raw_node_forwarding();

            client
                .send_node(NodeBuilder::new("ib").build())
                .await
                .expect("the fixture frame must reach the mock transport");
            crate::test_utils::poll_until("the fixture frame to be written", || {
                transport.sent_count() >= 1
            })
            .await;
            let frame = transport.sent().remove(0);

            let (events, receiver) = async_channel::bounded(4);
            *client.transport_events.lock().await = Some(receiver);
            events
                .send(crate::transport::TransportEvent::DataReceived(frame))
                .await
                .expect("the fixture channel must accept the frame");

            Self {
                client,
                events,
                decoded,
                _raw_nodes: raw_nodes,
            }
        }

        fn decoded_tags(&self) -> Vec<String> {
            self.decoded
                .events()
                .iter()
                .filter_map(|event| match &**event {
                    Event::RawNode(node) => Some(node.tag().to_string()),
                    _ => None,
                })
                .collect()
        }
    }

    /// The symptom this contract exists for: the connection is up, the server's
    /// bytes have arrived, and reading them is what turns them into a node.
    #[tokio::test]
    async fn a_driven_connection_decodes_the_first_frame_and_reports_its_end() {
        let fixture = PendingFrame::new().await;
        fixture
            .events
            .send(crate::transport::TransportEvent::Disconnected(
                DisconnectReason::StreamEnded,
            ))
            .await
            .expect("the fixture channel must accept the close");

        let reason = tokio::time::timeout(
            Duration::from_secs(10),
            fixture
                .client
                .connection_for_test()
                .read_until_disconnected(),
        )
        .await
        .expect("reading must end once the transport reports the close");

        assert_eq!(
            fixture.decoded_tags(),
            vec!["ib".to_string()],
            "the first frame of a driven connection must be decoded"
        );
        assert!(
            matches!(reason, Some(DisconnectReason::StreamEnded)),
            "an unannounced close must come back as the reason, got {reason:?}"
        );
    }

    /// The other half of the contract: connecting alone still guarantees a live
    /// socket, and guarantees nothing about reading it. The frame stays queued,
    /// no node is decoded, and no error says so, which is why the connection
    /// is `#[must_use]`.
    #[tokio::test]
    async fn a_connection_nobody_reads_leaves_the_frame_queued() {
        let fixture = PendingFrame::new().await;

        drop(fixture.client.connection_for_test());
        for _ in 0..64 {
            tokio::task::yield_now().await;
        }

        assert!(
            fixture.client.is_connected(),
            "connecting alone still leaves the socket up"
        );
        assert!(
            fixture.decoded_tags().is_empty(),
            "nothing may decode a frame while no reader is running"
        );
        assert_eq!(
            fixture.events.len(),
            1,
            "the frame must still be waiting for a reader"
        );
        assert!(
            fixture.client.transport_events.lock().await.is_some(),
            "the reader's inlet must be untouched"
        );
    }

    /// Shutdown is published once and for good, and `run` refuses to start
    /// after it. Connecting has to refuse too, or the connection this hands
    /// back revives a client the application was told was finished.
    #[tokio::test]
    async fn connecting_is_refused_after_the_client_is_shut_down() {
        let client = crate::test_utils::create_test_client().await;
        client.disconnect().await;

        let error = tokio::time::timeout(Duration::from_millis(500), client.connect())
            .await
            .expect("the refusal must be immediate, with no I/O attempted")
            .expect_err("a shut-down client must refuse to connect");
        assert!(matches!(error, ConnectError::Shutdown));
    }

    /// The post-pairing 515 ends a connection with `expected_disconnect` set,
    /// and the read loop exits on the first frame it decodes while that holds.
    /// `run` clears it at the top of each attempt; a caller driving connections
    /// itself gets the same start, or its second connection dies on arrival.
    #[tokio::test]
    async fn connecting_clears_the_retired_connection_expected_disconnect() {
        let client = crate::test_utils::create_test_client().await;
        client.expected_disconnect.store(true, Ordering::Relaxed);

        // The reset runs before any I/O, so the attempt itself need not finish.
        let _ = tokio::time::timeout(Duration::from_millis(500), client.connect()).await;

        assert!(
            !client.expected_disconnect.load(Ordering::Relaxed),
            "a new connection must not inherit the previous one's disconnect verdict"
        );
    }

    /// A read that is dropped mid-connection has stopped reading, so the flag
    /// that says otherwise has to come back. Left standing it would refuse the
    /// next `run()` as already running and admit requests to a dead reader.
    #[tokio::test]
    async fn a_cancelled_read_gives_the_reader_flag_back() {
        let fixture = PendingFrame::new().await;
        // The fixture client is pre-marked as driven; this one does the driving.
        fixture.client.is_running.store(false, Ordering::SeqCst);

        let cancelled = tokio::time::timeout(
            Duration::from_millis(200),
            fixture
                .client
                .connection_for_test()
                .read_until_disconnected(),
        )
        .await;

        assert!(
            cancelled.is_err(),
            "the read must still be waiting for frames when the timeout cancels it"
        );
        assert!(
            !fixture.client.is_running.load(Ordering::SeqCst),
            "a cancelled read must not leave the client advertising a reader"
        );
    }

    /// The connection handed back by `connect` is a borrow, so the happy path
    /// carries the same bytes and the same allocations it did when connecting
    /// resolved to `()`.
    #[test]
    fn handing_back_a_connection_costs_the_caller_nothing() {
        assert_eq!(
            size_of::<Result<Connection<'static>, ConnectError>>(),
            size_of::<Result<(), ConnectError>>()
        );
    }

    #[tokio::test]
    async fn wait_for_socket_resolves_immediately_once_connected() {
        let client = crate::test_utils::create_test_client().await;
        client.set_connected_for_test(true);

        client
            .wait_for_socket(Duration::from_millis(50))
            .await
            .expect("an already connected client must not wait");
    }

    #[tokio::test]
    async fn wait_for_socket_times_out_at_the_socket_stage() {
        let client = crate::test_utils::create_test_client().await;

        let timeout = Duration::from_millis(50);
        let error = client
            .wait_for_socket(timeout)
            .await
            .expect_err("a disconnected client must time out");
        assert!(matches!(
            error,
            ConnectError::Timeout {
                stage: ConnectStage::Socket,
                timeout: waited,
            } if waited == timeout
        ));
    }

    #[tokio::test]
    async fn wait_for_connected_resolves_immediately_once_fully_ready() {
        let client = crate::test_utils::create_test_client().await;
        client.set_connected_for_test(true);
        client.is_logged_in.store(true, Ordering::Relaxed);
        client.is_ready.store(true, Ordering::Relaxed);

        client
            .wait_for_connected(Duration::from_millis(50))
            .await
            .expect("a fully ready client must not wait");
    }

    #[tokio::test]
    async fn wait_for_connected_times_out_at_the_ready_stage() {
        let client = crate::test_utils::create_test_client().await;
        // Connected but never logged in: readiness, not the socket, is missing.
        client.set_connected_for_test(true);

        let timeout = Duration::from_millis(50);
        let error = client
            .wait_for_connected(timeout)
            .await
            .expect_err("a client that never logged in must time out");
        assert!(matches!(
            error,
            ConnectError::Timeout {
                stage: ConnectStage::Ready,
                timeout: waited,
            } if waited == timeout
        ));
    }

    #[tokio::test]
    async fn logout_tears_down_an_offline_client_without_sending_the_iq() {
        let client = crate::test_utils::create_test_client().await;

        tokio::time::timeout(Duration::from_secs(5), client.logout())
            .await
            .expect("logout must not block on an offline client");

        assert!(!client.enable_auto_reconnect.load(Ordering::Relaxed));
        assert!(!client.is_connected());
    }

    #[tokio::test]
    async fn logout_still_tears_down_when_the_deregistration_iq_fails() {
        let client = crate::test_utils::create_test_client().await;
        // Flagged connected with no socket behind it, so the IQ cannot be sent.
        client.set_connected_for_test(true);

        tokio::time::timeout(Duration::from_secs(5), client.logout())
            .await
            .expect("a failed deregistration IQ must not block logout");

        assert!(!client.enable_auto_reconnect.load(Ordering::Relaxed));
        assert!(!client.is_connected());
    }

    #[tokio::test]
    async fn connect_rejects_an_already_connected_client() {
        let client = crate::test_utils::create_test_client().await;
        client.set_connected_for_test(true);

        let error = client
            .connect()
            .await
            .expect_err("connecting twice must be refused");
        assert!(matches!(error, ConnectError::AlreadyConnected));
    }

    /// Where the run loop's own account of itself lands.
    const RUN_LOOP_LOG: &str = "whatsapp_rust::client::lifecycle";

    /// A transport factory that parks in `create_transport()` until released,
    /// holding the run loop inside a connect attempt. That is the window a
    /// caller's `disconnect()` lands in, and parking it makes the interleaving
    /// a fixture instead of a race.
    struct ParkedConnect {
        entered: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
    }

    #[async_trait::async_trait]
    impl crate::transport::TransportFactory for ParkedConnect {
        async fn create_transport(
            &self,
        ) -> Result<(
            Arc<dyn crate::transport::Transport>,
            async_channel::Receiver<crate::transport::TransportEvent>,
        )> {
            let _ = self.entered.send(()).await;
            let _ = self.release.recv().await;
            // No socket is ever opened: what these tests are about is the
            // branch the loop takes once an attempt has ended, and a failed
            // attempt reaches it by the same route a dropped connection does.
            Err(anyhow::anyhow!("this factory never opens a transport"))
        }
    }

    /// A client whose every connect attempt parks. `entered` yields one item
    /// per attempt reached; each `release` item lets one attempt fail, and
    /// dropping the sender releases all the rest.
    async fn client_parked_in_connect() -> (
        Arc<Client>,
        async_channel::Receiver<()>,
        async_channel::Sender<()>,
    ) {
        let (entered_tx, entered_rx) = async_channel::bounded(4);
        let (release_tx, release_rx) = async_channel::bounded(4);
        let client =
            crate::test_utils::create_test_client_with_transport_factory(Arc::new(ParkedConnect {
                entered: entered_tx,
                release: release_rx,
            }))
            .await;
        (client, entered_rx, release_tx)
    }

    /// Blocks until the run loop has reached its next connect attempt.
    async fn next_connect_attempt(entered: &async_channel::Receiver<()>) {
        tokio::time::timeout(Duration::from_secs(5), entered.recv())
            .await
            .expect("the run loop must reach a connect attempt")
            .expect("the observer channel must stay open");
    }

    /// The misreport this branch's guard exists for. `disconnect()` is
    /// terminal: it clears `is_running`, which is the loop's own stop
    /// condition. Landing it while a connect attempt is in flight used to make
    /// the loop announce an immediate reconnect on its way out — and that line
    /// is the only account a reader has of a session that never came back.
    #[tokio::test]
    async fn a_requested_disconnect_is_not_announced_as_a_reconnect() {
        if crate::test_utils::log_capture::delegated_to_child(
            "client::lifecycle::tests::a_requested_disconnect_is_not_announced_as_a_reconnect",
        ) {
            return;
        }
        let logs = crate::test_utils::log_capture::session();
        let (client, entered, release) = client_parked_in_connect().await;

        let runner = Arc::clone(&client);
        let run = tokio::spawn(async move { runner.run().await });
        next_connect_attempt(&entered).await;

        // Parked, so this is guaranteed to land before the loop reaches the
        // branch that reports what happens next.
        client.disconnect().await;
        drop(release);

        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("run() must return once the client has been disconnected")
            .expect("the run task must not panic");

        let said = logs.records_for(RUN_LOOP_LOG);
        assert!(
            !said
                .iter()
                .any(|(_, message)| message.contains("reconnecting immediately")),
            "a requested disconnect must not announce a reconnect the shutdown forbids: {said:?}",
        );
        assert!(
            said.iter()
                .any(|(_, message)| message.contains("Disconnect requested")),
            "the loop must still say why it stopped: {said:?}",
        );
    }

    /// The case the branch is really for, kept intact: a connection that ended
    /// with `expected_disconnect` set while the loop is still running — the
    /// post-pairing 515 — is announced as an immediate reconnect, and the
    /// attempt that follows is what makes the announcement true.
    #[tokio::test]
    async fn a_protocol_reconnect_is_still_announced_and_still_made() {
        if crate::test_utils::log_capture::delegated_to_child(
            "client::lifecycle::tests::a_protocol_reconnect_is_still_announced_and_still_made",
        ) {
            return;
        }
        let logs = crate::test_utils::log_capture::session();
        let (client, entered, release) = client_parked_in_connect().await;

        let runner = Arc::clone(&client);
        let run = tokio::spawn(async move { runner.run().await });
        next_connect_attempt(&entered).await;

        // What a 515 leaves behind: the end was planned, and nobody asked the
        // client to stop. Set from inside the attempt, so it survives the reset
        // `connect` performs on entry.
        client.expected_disconnect.store(true, Ordering::Relaxed);
        release
            .send(())
            .await
            .expect("the release channel must stay open");

        next_connect_attempt(&entered).await;
        let said = logs.records_for(RUN_LOOP_LOG);
        assert!(
            said.iter()
                .any(|(_, message)| message.contains("reconnecting immediately")),
            "an expected disconnect that leaves the loop running still reconnects: {said:?}",
        );

        client.disconnect().await;
        drop(release);
        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("run() must return once the client has been disconnected")
            .expect("the run task must not panic");
    }

    /// The wider half of the same window. `run` clears `expected_disconnect` at
    /// the top of every attempt, so a `disconnect()` that lands just before that
    /// reset — anywhere in a window as wide as the 20s connect timeout — leaves
    /// the flag false by the time the branch reads it. Judged on that flag alone
    /// the loop announces a backoff for a reconnect `connect()` is already
    /// refusing, which is the same untrue line one branch further down.
    ///
    /// The clear here stands in for the reset, because the reset runs before the
    /// factory parks and the state it leaves is the whole of what the loop sees.
    #[tokio::test]
    async fn a_disconnect_the_attempt_reset_erased_still_stops_the_loop() {
        if crate::test_utils::log_capture::delegated_to_child(
            "client::lifecycle::tests::a_disconnect_the_attempt_reset_erased_still_stops_the_loop",
        ) {
            return;
        }
        let logs = crate::test_utils::log_capture::session();
        let (client, entered, release) = client_parked_in_connect().await;

        let runner = Arc::clone(&client);
        let run = tokio::spawn(async move { runner.run().await });
        next_connect_attempt(&entered).await;

        client.disconnect().await;
        client.expected_disconnect.store(false, Ordering::Relaxed);
        drop(release);

        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("run() must return even with its verdict flag erased")
            .expect("the run task must not panic");

        let said = logs.records_for(RUN_LOOP_LOG);
        assert!(
            !said
                .iter()
                .any(|(_, message)| message.contains("Will attempt to reconnect in")),
            "a shut-down client owes no backoff, so none may be announced: {said:?}",
        );
        assert!(
            said.iter()
                .any(|(_, message)| message.contains("Disconnect requested")),
            "the loop must still say why it stopped: {said:?}",
        );
    }

    /// The branch's per-connection reset runs whichever exit it takes. Doing it
    /// only on the reconnecting side would leave `stats().reconnect_errors`
    /// reporting failures from before the connection that succeeded — a public
    /// number a change about a log line has no business moving.
    #[tokio::test]
    async fn a_requested_disconnect_still_clears_the_connection_counters() {
        let (client, entered, release) = client_parked_in_connect().await;
        // What a session that struggled and then settled leaves behind.
        client.auto_reconnect_errors.store(7, Ordering::Relaxed);
        client.connected_at_ms.store(1, Ordering::Relaxed);

        let runner = Arc::clone(&client);
        let run = tokio::spawn(async move { runner.run().await });
        next_connect_attempt(&entered).await;

        client.disconnect().await;
        drop(release);
        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("run() must return once the client has been disconnected")
            .expect("the run task must not panic");

        assert_eq!(
            client.stats().reconnect_errors,
            0,
            "a planned end owes no backoff, so its counter is cleared on both exits"
        );
        assert_eq!(
            client.connected_at_ms.load(Ordering::Relaxed),
            0,
            "and the auth timestamp is consumed, so no later connect reads it as stable"
        );
    }

    /// The capability the disconnect/reconnect pair left out: go offline, stay
    /// offline for as long as the application wants, come back on its word. The
    /// run loop must open no connection in between, and `run()` must still be
    /// running when it does.
    #[tokio::test]
    async fn a_paused_session_stops_connecting_and_resumes_on_request() {
        let (client, entered, release) = client_parked_in_connect().await;

        let runner = Arc::clone(&client);
        let run = tokio::spawn(async move { runner.run().await });
        next_connect_attempt(&entered).await;

        client.pause().await;
        assert!(client.is_paused(), "the pause is in effect once it returns");
        release
            .send(())
            .await
            .expect("the release channel must stay open");

        // Parked on the session notifier is the loop's own observable for
        // "waiting out a pause" — no sleep can stand in for it.
        crate::test_utils::wait_for_notifier_listeners(&client.session_state_notifier, 1).await;
        assert!(
            entered.is_empty(),
            "a paused loop must not open a connection while it waits"
        );
        assert!(
            !client.is_terminal(),
            "a paused client is between connections, not finished"
        );
        assert!(
            !run.is_finished(),
            "and `run()` is still the call driving it"
        );

        client.resume();
        assert!(!client.is_paused());
        next_connect_attempt(&entered).await;

        client.disconnect().await;
        drop(release);
        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("run() must return once the client has been disconnected")
            .expect("the run task must not panic");
    }

    /// A pause is not a failure, so it is not announced as one. The loop owes no
    /// backoff for an offline window the application chose, and a delay logged
    /// here would be a reconnect time any `resume()` is free to beat — the same
    /// class of untrue line the terminal-disconnect branch was fixed for.
    #[tokio::test]
    async fn pausing_announces_no_backoff_it_will_not_wait() {
        if crate::test_utils::log_capture::delegated_to_child(
            "client::lifecycle::tests::pausing_announces_no_backoff_it_will_not_wait",
        ) {
            return;
        }
        let logs = crate::test_utils::log_capture::session();
        let (client, entered, release) = client_parked_in_connect().await;
        // A session that already struggled: without the skip, this is the
        // counter the post-connection branch would build a delay from.
        client.auto_reconnect_errors.store(6, Ordering::Relaxed);

        let runner = Arc::clone(&client);
        let run = tokio::spawn(async move { runner.run().await });
        next_connect_attempt(&entered).await;

        client.pause().await;
        release
            .send(())
            .await
            .expect("the release channel must stay open");
        crate::test_utils::wait_for_notifier_listeners(&client.session_state_notifier, 1).await;

        let said = logs.records_for(RUN_LOOP_LOG);
        assert!(
            !said
                .iter()
                .any(|(_, message)| message.contains("Will attempt to reconnect in")),
            "a pause owes no backoff, so none may be announced: {said:?}",
        );
        assert!(
            said.iter()
                .any(|(_, message)| message.contains("Session paused")),
            "the loop must say why it stopped connecting: {said:?}",
        );

        client.resume();
        client.disconnect().await;
        drop(release);
        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("run() must return once the client has been disconnected")
            .expect("the run task must not panic");
    }

    /// A pause parks the loop, so `disconnect()` has to reach it there — the
    /// wait is on the session notifier, which every path that clears
    /// `is_running` fires. Missed, the run future outlives the shutdown for as
    /// long as the pause does, which is forever.
    #[tokio::test]
    async fn disconnecting_a_paused_session_still_ends_the_run_loop() {
        let (client, entered, release) = client_parked_in_connect().await;

        let runner = Arc::clone(&client);
        let run = tokio::spawn(async move { runner.run().await });
        next_connect_attempt(&entered).await;

        client.pause().await;
        drop(release);
        crate::test_utils::wait_for_notifier_listeners(&client.session_state_notifier, 1).await;

        client.disconnect().await;
        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("a paused run loop must still return when the client is disconnected")
            .expect("the run task must not panic");
        assert!(client.is_terminal(), "and the client is finished for good");
    }

    /// The guarantee a loop-only flag could not give. `pause()` returning means
    /// no connection exists and none will be opened — by the run loop or by a
    /// caller reaching for `connect()` directly — until `resume()` lifts it.
    /// Refused, not queued, and not final: the error says so.
    #[tokio::test]
    async fn connecting_is_refused_while_the_session_is_paused() {
        let client = crate::test_utils::create_test_client().await;
        client.pause().await;

        let error = tokio::time::timeout(Duration::from_millis(500), client.connect())
            .await
            .expect("the refusal must be immediate, with no I/O attempted")
            .expect_err("a paused client must refuse to connect");
        assert!(matches!(error, ConnectError::Paused));
        assert!(
            !client.is_terminal(),
            "and refusing is not the same as being finished"
        );

        client.resume();
        // Not `Paused` any more: this one gets as far as the transport the
        // fixture has no server behind, which is the refusal being lifted.
        let after = tokio::time::timeout(Duration::from_secs(5), client.connect()).await;
        assert!(
            !matches!(after, Ok(Err(ConnectError::Paused))),
            "resuming must lift the refusal, got {after:?}"
        );
    }

    /// The half the run loop cannot cover on its own: a `pause()` landing while
    /// an attempt is already opening a transport. Nothing is published for the
    /// teardown to close, so without the re-check inside the connect graph the
    /// attempt goes on to publish a live socket for a client the application
    /// has parked, and `is_paused()` stays true over a session that is online.
    #[tokio::test]
    async fn a_pause_mid_attempt_retracts_the_connection_it_would_have_published() {
        let (client, entered, release) = client_parked_in_connect().await;

        let connecting = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.connect().await.map(|_| ()) }
        });
        next_connect_attempt(&entered).await;

        // Parked inside the attempt, exactly the window where no transport has
        // been published yet.
        client.pause().await;
        drop(release);

        let refused = tokio::time::timeout(Duration::from_secs(10), connecting)
            .await
            .expect("the attempt must finish once the transport call returns")
            .expect("the connect task must not panic");
        assert!(
            refused.is_err(),
            "an attempt overtaken by a pause must not hand back a connection"
        );
        assert!(
            !client.is_connected(),
            "and must leave nothing published behind it"
        );
        assert!(client.transport.lock().await.is_none());
    }

    /// `connect()` publishes `is_connected` before it hands the `Connection`
    /// back, so a caller that connected directly and has not started reading is
    /// flagged connected with no `drive_connection` behind it to run a teardown.
    /// Pausing there has to do the cleanup itself, or the flag stands for good
    /// and every later `connect()` is refused as `AlreadyConnected` — a pause
    /// that cannot be resumed from.
    #[tokio::test]
    async fn pausing_an_undriven_connection_still_clears_the_connected_flag() {
        let client = crate::test_utils::create_test_client().await;
        let (transport, _entered_rx, release_tx, closes) = parked_transport();
        drop(release_tx); // an ordinary prompt close
        *client.transport.lock().await = Some(transport);
        // What `connect()` leaves behind for a `Connection` nobody drives.
        client.set_connected_for_test(true);
        assert!(!client.is_running.load(Ordering::Relaxed), "and no reader");

        tokio::time::timeout(Duration::from_secs(10), client.pause())
            .await
            .expect("pause must not block");

        // At least once: the cleanup closes the socket too, and transports make
        // `disconnect()` idempotent for exactly this overlap — the same one
        // `Client::disconnect()` has.
        assert!(closes.load(Ordering::SeqCst) >= 1, "the socket is closed");
        assert!(
            !client.is_connected(),
            "and the client no longer claims a connection nothing was reading"
        );
        assert!(client.transport.lock().await.is_none());

        client.resume();
        let after = tokio::time::timeout(Duration::from_secs(5), client.connect()).await;
        assert!(
            !matches!(after, Ok(Err(ConnectError::AlreadyConnected))),
            "so resuming can connect again, got {after:?}"
        );
    }

    /// The cleanup for an unread connection has to survive a `resume()` landing
    /// across the teardown. Gated on the pause still standing, it did not: the
    /// resume cleared the flag, the only cleanup path was skipped, and a caller
    /// who had connected directly was left flagged connected for good with every
    /// later `connect()` refused as `AlreadyConnected` — a resume that cannot
    /// reconnect. Identity is the right question, and the answer is the same
    /// either way: the state is this call's to clear because the socket it tore
    /// down is still the published one.
    #[tokio::test]
    async fn a_resume_across_the_teardown_does_not_strand_an_unread_connection() {
        let client = crate::test_utils::create_test_client().await;
        let (transport, _entered, release, _closes) = parked_transport();
        drop(release);
        *client.transport.lock().await = Some(transport);
        // What `connect()` leaves for a `Connection` nobody drives.
        client.set_connected_for_test(true);
        assert!(!client.is_running.load(Ordering::Relaxed), "and no reader");

        let in_flight = client
            .outbound_flush
            .try_track()
            .expect("the flush scope must be open on a fresh client");
        let pausing = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.pause().await }
        });
        crate::test_utils::poll_until("the pause to reach its outbound flush", || {
            client.outbound_flush.flush_waiters() >= 1
        })
        .await;

        // The overlap: the application changes its mind mid-teardown.
        client.resume();

        drop(in_flight);
        tokio::time::timeout(Duration::from_secs(10), pausing)
            .await
            .expect("the pause must finish once its flush drains")
            .expect("the pause task must not panic");

        assert!(
            !client.is_connected(),
            "the torn-down connection must not be left published"
        );
        assert!(client.transport.lock().await.is_none());
        let after = tokio::time::timeout(Duration::from_secs(5), client.connect()).await;
        assert!(
            !matches!(after, Ok(Err(ConnectError::AlreadyConnected))),
            "so the resumed session can connect, got {after:?}"
        );
    }

    /// A teardown must stop fencing the moment it stops owning the connection.
    /// `outbound_flush` and the per-connection shutdown notifier are shared and
    /// unversioned, and a replacement installed by a resume has already reopened
    /// and reset them for itself — so an old pause finishing its drain and then
    /// firing them would reject that connection's receipts, retries and
    /// transport acks for its whole life and stand its subscribers down.
    #[tokio::test]
    async fn a_teardown_that_lost_the_connection_stops_fencing_it() {
        let client = crate::test_utils::create_test_client().await;
        let (torn_down, _entered, release, _closes) = parked_transport();
        drop(release);
        *client.transport.lock().await = Some(torn_down);

        let in_flight = client
            .outbound_flush
            .try_track()
            .expect("the flush scope must be open on a fresh client");
        let pausing = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.pause().await }
        });
        crate::test_utils::poll_until("the pause to reach its outbound flush", || {
            client.outbound_flush.flush_waiters() >= 1
        })
        .await;

        // Exactly what a resume plus a fresh connection do on their way up.
        client.resume();
        let (replacement, _r_entered, r_release, _r_closes) = parked_transport();
        drop(r_release);
        *client.transport.lock().await = Some(replacement);
        client.outbound_flush.reopen();
        client.reset_connection_shutdown();
        let replacement_shutdown = client.connection_shutdown_signal();

        drop(in_flight);
        tokio::time::timeout(Duration::from_secs(10), pausing)
            .await
            .expect("the pause must finish once its flush drains")
            .expect("the pause task must not panic");

        assert!(
            !replacement_shutdown.is_fired(),
            "the old teardown must not shut down the connection the resume asked for"
        );
        assert!(
            client.outbound_flush.try_track().is_some(),
            "and must leave its outbound scope open, or it sends nothing for its whole life"
        );
    }

    /// Which socket a pause owns is decided when it starts, not when it gets
    /// round to closing one. Its flushes are bounded but not instant, and a
    /// `resume()` landing across them lets the run loop publish a replacement
    /// into the same slot — so a teardown that looked the slot up afterwards
    /// would close the connection the resume had just asked for, and the
    /// resumed session would die on arrival.
    #[tokio::test]
    async fn a_pause_closes_the_socket_it_started_with_not_whatever_replaced_it() {
        let client = crate::test_utils::create_test_client().await;
        let (torn_down, _torn_entered, torn_release, torn_closes) = parked_transport();
        drop(torn_release); // the ordinary prompt close
        let (replacement, _repl_entered, repl_release, repl_closes) = parked_transport();
        drop(repl_release);
        *client.transport.lock().await = Some(torn_down);

        // Holds the outbound flush open, which is where `pause()` waits — the
        // window a resume and a fresh connection fit into.
        let in_flight = client
            .outbound_flush
            .try_track()
            .expect("the flush scope must be open on a fresh client");

        let pausing = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.pause().await }
        });
        crate::test_utils::poll_until("the pause to reach its outbound flush", || {
            client.outbound_flush.flush_waiters() >= 1
        })
        .await;

        // Exactly what a resume plus a fresh connection leave behind.
        client.resume();
        *client.transport.lock().await = Some(replacement);

        drop(in_flight);
        tokio::time::timeout(Duration::from_secs(10), pausing)
            .await
            .expect("the pause must finish once its flush drains")
            .expect("the pause task must not panic");

        assert_eq!(
            torn_closes.load(Ordering::SeqCst),
            1,
            "the pause closes the socket it set out to close"
        );
        assert_eq!(
            repl_closes.load(Ordering::SeqCst),
            0,
            "and never the one the resume asked for"
        );
        assert!(
            client.transport.lock().await.is_some(),
            "which is left published for the resumed session to use"
        );
    }

    /// The teardown ends in an untimed socket close, so a `resume()` that had to
    /// queue behind it would be held off for as long as an unresponsive
    /// transport cared to take — turning "come back on my word" into a session
    /// that never comes back. It must not wait, and the guarantee it exists for
    /// must not depend on its waiting: the loop reads the no-backoff fact off
    /// the connection that ended, not off which caller won.
    #[tokio::test]
    async fn resuming_does_not_queue_behind_a_stalled_teardown() {
        let client = crate::test_utils::create_test_client().await;
        let (transport, entered_rx, release_tx, _closes) = parked_transport();
        *client.transport.lock().await = Some(transport);
        client.set_connected_for_test(true);

        let pausing = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.pause().await }
        });
        // Parked in the close, which is where a dead transport leaves it.
        tokio::time::timeout(Duration::from_secs(5), entered_rx.recv())
            .await
            .expect("the pause must reach the socket close")
            .expect("the observer channel must stay open");

        client.resume();
        assert!(
            !client.is_paused(),
            "resuming must take effect while the socket close is still hanging"
        );
        assert!(
            client.pause_teardown_pending.load(Ordering::Relaxed),
            "and the teardown still owes the run loop its no-backoff answer, \
             which is what makes waiting unnecessary"
        );

        drop(release_tx);
        tokio::time::timeout(Duration::from_secs(5), pausing)
            .await
            .expect("the pause must finish once the close returns")
            .expect("the pause task must not panic");
    }

    /// `resume()` clears a pause and nothing else. On a client that has been
    /// disconnected in the meantime there is nothing to go back to, and saying
    /// otherwise would make the pair look like a way out of a shutdown.
    #[tokio::test]
    async fn resuming_does_not_undo_a_shutdown() {
        let client = crate::test_utils::create_test_client().await;

        client.pause().await;
        client.disconnect().await;
        client.resume();

        assert!(!client.is_paused(), "the pause itself is cleared");
        assert!(
            client.is_terminal(),
            "but a resumed client is still a shut-down one"
        );
        let error = client
            .connect()
            .await
            .expect_err("and it still refuses to connect");
        assert!(matches!(error, ConnectError::Shutdown));
    }

    /// The promise the branch above now keeps: `disconnect()` ends the session,
    /// not just the socket. There is no way back for this client — `run()`
    /// refuses to start another supervision loop — which is exactly why the
    /// loop must not advertise one on its way out.
    #[tokio::test]
    async fn a_disconnected_client_cannot_be_run_again() {
        let client = crate::test_utils::create_test_client().await;
        client.disconnect().await;

        assert!(
            client.is_terminal(),
            "a disconnected client is finished, not between connections"
        );
        tokio::time::timeout(Duration::from_secs(5), client.run())
            .await
            .expect("run() must refuse at once after a disconnect, not start a session");
        assert!(
            !client.is_running.load(Ordering::SeqCst),
            "a refused run() must not leave the client advertising a reader"
        );
    }

    /// Far enough up the Fibonacci sequence that the next backoff is the 900s
    /// cap. The cap is what makes these tests decisive: an uninterruptible
    /// wait parks the loop for 15 minutes, so a prompt return can only come
    /// from the wait racing the shutdown signal.
    const CAPPED_BACKOFF_ATTEMPTS: u32 = 40;

    /// Starts `run()` and returns once the loop has actually reached its
    /// reconnect backoff. The attempt counter is bumped immediately before the
    /// wait, so observing the bump is proof the loop is parked there — no
    /// timed guess involved.
    async fn run_until_parked_in_backoff(client: &Arc<Client>) -> tokio::task::JoinHandle<()> {
        client
            .auto_reconnect_errors
            .store(CAPPED_BACKOFF_ATTEMPTS, Ordering::Relaxed);

        let runner = client.clone();
        let run = tokio::spawn(async move { runner.run().await });

        crate::test_utils::poll_until("the run loop to reach its reconnect backoff", || {
            client.auto_reconnect_errors.load(Ordering::Relaxed) > CAPPED_BACKOFF_ATTEMPTS
        })
        .await;

        run
    }

    /// A shutdown that lands *during* the reconnect backoff must be observed
    /// then, not when the sleep happens to expire. `disconnect()` returns
    /// promptly either way; what the consumer awaits is the run future, and
    /// with an uninterruptible wait that future outlives the shutdown by up to
    /// the 900s cap — long enough that a supervisor awaiting `Bot::run` reads
    /// it as a hang.
    #[tokio::test]
    async fn disconnect_interrupts_the_reconnect_backoff() {
        let client = crate::test_utils::create_test_client().await;
        let run = run_until_parked_in_backoff(&client).await;

        client.disconnect().await;

        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("run() must return when disconnect() fires, not after the 900s backoff")
            .expect("the run task must not panic");
    }

    /// `signal_shutdown_sync()` is the flag-only path taken by `Drop` impls on
    /// FFI wrappers, and its contract is the same: watchers exit on their next
    /// poll. The run loop is a watcher, so the parked backoff must wake here
    /// too — it is the path a `Drop` cannot follow up with an `await`.
    #[tokio::test]
    async fn signal_shutdown_sync_interrupts_the_reconnect_backoff() {
        let client = crate::test_utils::create_test_client().await;
        let run = run_until_parked_in_backoff(&client).await;

        client.signal_shutdown_sync();

        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("run() must return when signal_shutdown_sync() fires")
            .expect("the run task must not panic");
    }

    /// The counterpart guard: the *per-connection* shutdown fires on every
    /// disconnect the loop is supposed to reconnect from, so the backoff must
    /// not watch it. Subscribing to the wrong signal would pass the two tests
    /// above while silently turning every backoff into a no-op and hammering
    /// the server — this pins the normal path down.
    #[tokio::test]
    async fn a_connection_level_shutdown_does_not_cut_the_backoff_short() {
        let client = crate::test_utils::create_test_client().await;
        let run = run_until_parked_in_backoff(&client).await;

        client.notify_connection_shutdown();

        // Still parked: the loop must not have come back around to bump the
        // counter for another attempt.
        tokio::time::sleep(Duration::from_millis(200)).await;
        assert_eq!(
            client.auto_reconnect_errors.load(Ordering::Relaxed),
            CAPPED_BACKOFF_ATTEMPTS + 1,
            "a per-connection shutdown must not release the reconnect backoff"
        );

        client.disconnect().await;
        tokio::time::timeout(Duration::from_secs(10), run)
            .await
            .expect("run() must still return on a terminal shutdown")
            .expect("the run task must not panic");
    }

    /// A transport whose `disconnect()` parks until released, standing in for the real
    /// close-frame write: a TLS write with no timeout, behind the sender's own mutex.
    struct ParkedDisconnect {
        entered: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
        closes: Arc<AtomicUsize>,
    }

    #[async_trait::async_trait]
    impl crate::transport::Transport for ParkedDisconnect {
        async fn send(&self, _data: bytes::Bytes) -> Result<()> {
            Ok(())
        }

        async fn disconnect(&self) {
            self.closes.fetch_add(1, Ordering::SeqCst);
            let _ = self.entered.send(()).await;
            let _ = self.release.recv().await;
        }
    }

    fn parked_transport() -> (
        Arc<ParkedDisconnect>,
        async_channel::Receiver<()>,
        async_channel::Sender<()>,
        Arc<AtomicUsize>,
    ) {
        let (entered_tx, entered_rx) = async_channel::bounded(4);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let closes = Arc::new(AtomicUsize::new(0));
        let transport = Arc::new(ParkedDisconnect {
            entered: entered_tx,
            release: release_rx,
            closes: closes.clone(),
        });
        (transport, entered_rx, release_tx, closes)
    }

    /// The teardown paths must not hold the `transport` mutex across the socket close:
    /// `connect_internal` installs the next connection's transport through that same mutex,
    /// so a close that never returns would take the whole reconnect down with it.
    #[tokio::test]
    async fn an_in_flight_socket_close_does_not_park_the_transport_slot() {
        let client = crate::test_utils::create_test_client().await;
        let (transport, entered_rx, release_tx, _closes) = parked_transport();
        *client.transport.lock().await = Some(transport);

        let cleanup = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.cleanup_connection_state().await }
        });
        tokio::time::timeout(Duration::from_secs(5), entered_rx.recv())
            .await
            .expect("cleanup must reach the socket close")
            .expect("the observer channel must stay open");

        // Exactly what `connect_internal` does once the next socket is up.
        tokio::time::timeout(Duration::from_secs(5), async {
            *client.transport.lock().await = Some(Arc::new(crate::transport::mock::MockTransport));
        })
        .await
        .expect("installing the next transport must not wait on the in-flight close");

        drop(release_tx);
        tokio::time::timeout(Duration::from_secs(5), cleanup)
            .await
            .expect("cleanup must finish once the close returns")
            .expect("cleanup must not panic");
    }

    /// The other half of releasing the guard: now that a `connect()` can publish its own
    /// connection while the old close is still in flight, cleanup must not come back and
    /// strip the replacement's state. Everything cleanup clears, it clears before the close.
    #[tokio::test]
    async fn a_connection_published_during_cleanup_is_not_stripped_by_it() {
        let client = crate::test_utils::create_test_client().await;
        let (transport, entered_rx, release_tx, _closes) = parked_transport();
        *client.transport.lock().await = Some(transport);
        *client.transport_events.lock().await = Some(async_channel::bounded(1).1);

        let cleanup = tokio::spawn({
            let client = Arc::clone(&client);
            async move { client.cleanup_connection_state().await }
        });
        tokio::time::timeout(Duration::from_secs(5), entered_rx.recv())
            .await
            .expect("cleanup must reach the socket close")
            .expect("the observer channel must stay open");

        // The publish half of `connect_internal`, minus the handshake.
        *client.transport.lock().await = Some(Arc::new(crate::transport::mock::MockTransport));
        *client.transport_events.lock().await = Some(async_channel::bounded(1).1);

        drop(release_tx);
        tokio::time::timeout(Duration::from_secs(5), cleanup)
            .await
            .expect("cleanup must finish once the close returns")
            .expect("cleanup must not panic");

        assert!(
            client.transport.lock().await.is_some(),
            "the replacement transport must survive the teardown it did not belong to"
        );
        assert!(
            client.transport_events.lock().await.is_some(),
            "the replacement's event receiver must survive too, or its read loop never starts"
        );
    }

    /// The happy path the fix must preserve: cleanup still closes the socket it owns and
    /// still leaves the slot empty for the next connection.
    #[tokio::test]
    async fn cleanup_closes_the_transport_and_clears_the_slot() {
        let client = crate::test_utils::create_test_client().await;
        let (transport, _entered_rx, release_tx, closes) = parked_transport();
        drop(release_tx); // never park: this is the ordinary, prompt close
        *client.transport.lock().await = Some(transport);

        tokio::time::timeout(Duration::from_secs(5), client.cleanup_connection_state())
            .await
            .expect("cleanup must not block");

        assert_eq!(
            closes.load(Ordering::SeqCst),
            1,
            "the socket must be closed"
        );
        assert!(
            client.transport.lock().await.is_none(),
            "cleanup owns the teardown and must leave the slot free"
        );
    }

    /// Same for the user-facing path: `disconnect()` closes the socket and hands a cleared
    /// slot back, so nothing from the dead connection survives into the next one.
    #[tokio::test]
    async fn disconnect_closes_the_transport_and_clears_the_slot() {
        let client = crate::test_utils::create_test_client().await;
        let (transport, _entered_rx, release_tx, closes) = parked_transport();
        drop(release_tx);
        *client.transport.lock().await = Some(transport);

        tokio::time::timeout(Duration::from_secs(10), client.disconnect())
            .await
            .expect("disconnect must not block");

        assert!(
            closes.load(Ordering::SeqCst) >= 1,
            "the socket must be closed"
        );
        assert!(client.transport.lock().await.is_none());
    }
}
