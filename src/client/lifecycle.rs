//! Client construction and connection lifecycle: connect, run, reconnect, shutdown.

use super::*;

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
        self.expected_disconnect.store(true, Ordering::Relaxed);
        self.is_running.store(false, Ordering::Relaxed);
        self.shutdown_notifier.notify();
        self.notify_session_state();
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &self.lifecycle {
            lifecycle.signal_shutdown_sync();
        }
        self.notify_connection_shutdown();
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
    /// Not `is_running` on its own: that tracks whether `run()`'s supervision
    /// loop is active, which a direct-connect client never starts, so a healthy
    /// one would look permanently stopped the moment its application expressed a
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

    /// The supervision loop giving up for good.
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

    /// Returns `true` when the client has completed its full startup:
    /// transport connected, server authenticated, and critical app state synced.
    /// This is the condition `wait_for_connected` uses to resolve.
    fn is_fully_ready(&self) -> bool {
        self.is_connected() && self.is_logged_in() && self.is_ready.load(Ordering::Relaxed)
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
                if self.connection_generation.load(Ordering::SeqCst) != expected_generation
                    || self.expected_disconnect.load(Ordering::Acquire)
                {
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
        if self.connection_generation.load(Ordering::SeqCst) != expected_generation
            || self.expected_disconnect.load(Ordering::Acquire)
        {
            debug!("Skipping Connected dispatch after its connection retired");
            return;
        }
        self.publish_connected();
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
            passkey_state: Arc::new(Mutex::new(crate::passkey::flow::PasskeyFlowState::default())),
            passkey_opening: AtomicBool::new(false),
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
            stanza_interceptors: std::sync::RwLock::new(Arc::new(Vec::new())),
            stanza_interceptor_count: AtomicUsize::new(0),
            next_interceptor_id: AtomicU64::new(0),
            #[cfg(feature = "voip-runtime")]
            call_registry: Arc::new(wacore::voip::CallRegistry::new()),
            #[cfg(feature = "voip-runtime")]
            pending_call_link_joins: Arc::new(std::sync::Mutex::new(
                voip::PendingCallLinkJoins::default(),
            )),
            #[cfg(feature = "voip-runtime")]
            pending_call_link_join_lane: Arc::new(Mutex::new(())),
            #[cfg(feature = "voip-runtime")]
            answer_transition_locks: std::array::from_fn(|_| Arc::new(Mutex::new(()))),
            #[cfg(feature = "voip-runtime")]
            pending_outgoing_calls: Arc::new(std::sync::Mutex::new(HashMap::new())),
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
            if !first_connect {
                self.stats.record_reconnect();
            }
            first_connect = false;
            self.expected_disconnect.store(false, Ordering::Relaxed);

            if let Err(connect_err) = self.connect().await {
                wacore::telemetry::connect("fail");
                let is_transient = matches!(
                    &connect_err,
                    ConnectError::Handshake(e) if e.is_transient()
                );
                if is_transient {
                    debug!("Transient connect failure, will retry: {connect_err:#}");
                } else {
                    error!("Failed to connect: {connect_err:#}. Will retry...");
                }
            } else {
                wacore::telemetry::connect("ok");
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
                if let Some(reason) = unexpected_disconnect {
                    self.core.event_bus.dispatch(Event::Disconnected(
                        crate::types::events::Disconnected::builder()
                            .reason(reason)
                            .build(),
                    ));
                }
            }

            if !self.enable_auto_reconnect.load(Ordering::Relaxed) {
                info!("Auto-reconnect disabled, shutting down.");
                self.stop_supervision_loop();
                break;
            }

            // If this was an expected disconnect (e.g., 515 after pairing), reconnect immediately
            if self.expected_disconnect.load(Ordering::Relaxed) {
                self.auto_reconnect_errors.store(0, Ordering::Relaxed);
                // Consume the auth timestamp so a later failed connect can't
                // read this cycle's stale value as a "stable" connection.
                self.connected_at_ms.store(0, Ordering::Relaxed);
                info!("Expected disconnect (e.g., 515), reconnecting immediately...");
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
            futures::select! {
                _ = self.runtime.sleep(delay).fuse() => {}
                _ = shutdown_fired.fuse() => {
                    debug!("Shutdown signalled during reconnect backoff, exiting run loop.");
                }
            }
        }
        #[cfg(feature = "client-lifecycle")]
        self.shutdown_lifecycle().await;
        info!("Client run loop has shut down.");
    }

    /// Boxed barrier: see [`crate::bot::Bot::run`]. Coroutines are LocalCopy
    /// across crates, so consumers awaiting the connect graph directly would
    /// re-codegen it; the box makes them poll through a vtable instead.
    pub async fn connect(self: &Arc<Self>) -> Result<(), ConnectError> {
        #[cfg(feature = "client-lifecycle")]
        if let Some(lifecycle) = &self.lifecycle
            && !lifecycle.wait_until_active().await
        {
            return Err(ConnectError::NotActivated);
        }
        self.connect_boxed().await
    }

    #[inline(never)]
    fn connect_boxed(self: &Arc<Self>) -> wacore::runtime::BoxFuture<'_, Result<(), ConnectError>> {
        Box::pin(self.connect_graph())
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
    async fn connect_graph(self: &Arc<Self>) -> Result<(), ConnectError> {
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

        // Reset login state for new connection attempt. This ensures that
        // handle_success will properly process the <success> stanza even if
        // a previous connection's post-login task bailed out early.
        self.is_logged_in.store(false, Ordering::Relaxed);
        self.is_ready.store(false, Ordering::Relaxed);
        self.is_connected.store(false, Ordering::Relaxed);
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

        let noise_socket = match handshake::do_handshake(
            self.runtime.clone(),
            &self.persistence_manager,
            &self.ik_handshake_failures,
            transport.clone(),
            &mut transport_events,
            Some(self.stats.clone()),
        )
        .await
        {
            Ok(socket) => socket,
            Err(e) => {
                transport.disconnect().await;
                return Err(e.into());
            }
        };

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

        *self.transport.lock().await = Some(transport);
        *self.transport_events.lock().await = Some(transport_events);
        *self.noise_socket.lock().unwrap_or_else(|p| p.into_inner()) = Some(noise_socket);
        self.is_connected.store(true, Ordering::Release);

        // Notify waiters that socket is ready (before login)
        self.socket_ready_notifier.notify(usize::MAX);

        let client_clone = self.clone();
        self.runtime
            .spawn(Box::pin(async move { client_clone.keepalive_loop().await }))
            .detach();

        Ok(())
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

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.disconnect", level = "info", skip_all)
    )]
    pub async fn disconnect(self: &Arc<Self>) {
        info!("Disconnecting client intentionally.");
        wacore::telemetry::set_connected(false);
        self.expected_disconnect.store(true, Ordering::Relaxed);
        self.is_running.store(false, Ordering::Relaxed);
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
        // Tear down every in-flight VoIP call: the relay socket and signaling are connection-scoped,
        // so a call can't survive a disconnect/reconnect. Aborts each media task and clears the map.
        #[cfg(feature = "voip-runtime")]
        {
            self.call_registry.abort_all();
            // Dormant outgoing calls (relay never arrived) live in pending_outgoing_calls, not the
            // registry, so abort_all misses them. Drain them and notify `ended` so any waiter wakes.
            crate::voip::facade::drain_pending_outgoing_on_disconnect(self);
        }
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
        // Fast path: already connected
        if self.is_connected() {
            return Ok(());
        }

        // Register waiter and re-check to avoid race condition:
        // If socket becomes ready between checks, the notified future captures it.
        let notified = self.socket_ready_notifier.listen();
        if self.is_connected() {
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

    /// Force the connected flag for tests that exercise connected-only operations.
    #[cfg(test)]
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
    /// under; and a supervision loop, because `send_and_wait_iq` refuses without
    /// one — a direct-connect client has no reader, so its every request would
    /// time out.
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

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
