//! Client construction and connection lifecycle: connect, run, reconnect, shutdown.

use super::*;

/// Max groups with a cached resolved-device snapshot. LRU eviction covers
/// accounts in more groups; an evicted entry just recomputes on next send.
const GROUP_DEVICES_MEMO_CAPACITY: u64 = 64;

impl Client {
    /// WA Web `resetDelay: 30000` — only after a connection has stayed up this
    /// long is the reconnect backoff counter reset to its base.
    pub(crate) const STABLE_CONNECTION_RESET_MS: i64 = 30_000;

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

    /// Returns `true` when the client has completed its full startup:
    /// transport connected, server authenticated, and critical app state synced.
    /// This is the condition `wait_for_connected` uses to resolve.
    fn is_fully_ready(&self) -> bool {
        self.is_connected() && self.is_logged_in() && self.is_ready.load(Ordering::Relaxed)
    }

    /// Dispatch the Connected event and notify waiters.
    pub(crate) fn dispatch_connected(&self) {
        self.is_ready.store(true, Ordering::Relaxed);
        wacore::telemetry::set_connected(true);
        self.core
            .event_bus
            .dispatch(Event::Connected(crate::types::events::Connected));
        self.connected_notifier.notify(usize::MAX);
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
        Self::new_with_cache_config(
            runtime,
            persistence_manager,
            transport_factory,
            http_client,
            override_version,
            CacheConfig::default(),
        )
        .await
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
        let mut unique_id_bytes = [0u8; 2];
        rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut unique_id_bytes);

        let device_snapshot = persistence_manager.get_device_snapshot();
        let core = wacore::client::CoreClient::new(device_snapshot.core.clone());

        let (tx, rx) = async_channel::bounded(32);

        let device_topology = crate::client::device_topology::DeviceTopology::new();
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
            is_connecting: Arc::new(AtomicBool::new(false)),
            is_running: Arc::new(AtomicBool::new(false)),
            is_connected: Arc::new(AtomicBool::new(false)),
            send_active_receipts: AtomicU32::new(0),
            ik_handshake_failures: Arc::new(AtomicU32::new(0)),
            shutdown_notifier: wacore::runtime::ShutdownNotifier::new(),
            connection_shutdown: std::sync::Mutex::new(wacore::runtime::ShutdownNotifier::new()),
            stats: Arc::new(wacore::stats::SessionStats::new()),

            transport: Arc::new(Mutex::new(None)),
            transport_events: Arc::new(Mutex::new(None)),
            transport_factory,
            noise_socket: Arc::new(Mutex::new(None)),

            response_waiters: Arc::new(std::sync::Mutex::new(HashMap::new())),
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
            session_locks: Cache::builder()
                .max_capacity(cache_config.session_locks_capacity.max(1))
                .build(),
            chat_lanes: Cache::builder()
                .max_capacity(cache_config.chat_lanes_capacity.max(1))
                .build(),
            lid_pn_cache: Arc::new(LidPnCache::with_config(
                &cache_config.lid_pn_cache,
                cache_config.cache_stores.lid_pn_cache.clone(),
            )),
            ab_props: Arc::new(wacore::store::ab_props::AbPropsCache::new()),
            group_cache: async_lock::Mutex::new(None),

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
            offline_batch: Arc::new(crate::client::offline_resume::OfflineBatchCoordinator::new()),

            enable_auto_reconnect: Arc::new(AtomicBool::new(true)),
            auto_reconnect_errors: Arc::new(AtomicU32::new(0)),
            connected_at_ms: Arc::new(portable_atomic::AtomicI64::new(0)),
            backoff_reset_suppressed: Arc::new(AtomicBool::new(false)),

            needs_initial_full_sync: Arc::new(AtomicBool::new(false)),

            app_state_processor: async_lock::Mutex::new(None),
            app_state_key_requests: Arc::new(Mutex::new(HashMap::new())),
            app_state_syncing: Arc::new(Mutex::new(HashSet::new())),
            initial_keys_synced_notifier: Arc::new(event_listener::Event::new()),
            initial_app_state_keys_received: Arc::new(AtomicBool::new(false)),
            prekey_upload_lock: Arc::new(async_lock::Mutex::new(())),
            signed_pre_key_rotation_lock: Arc::new(async_lock::Mutex::new(())),
            tc_token_lock: Arc::new(async_lock::Mutex::new(())),
            offline_sync_notifier: Arc::new(event_listener::Event::new()),
            offline_sync_completed: Arc::new(AtomicBool::new(false)),
            offline_sync_finish_started: Arc::new(AtomicBool::new(false)),
            offline_receipt_buffer: std::sync::Mutex::new(Vec::new()),
            inbound_commit_batch: Default::default(),
            history_sync_tasks_in_flight: Arc::new(AtomicUsize::new(0)),
            history_sync_idle_notifier: Arc::new(event_listener::Event::new()),
            outbound_flush: Arc::new(crate::flush_scope::FlushScope::new()),
            presence_subscriptions: Arc::new(async_lock::Mutex::new(HashSet::new())),
            socket_ready_notifier: Arc::new(event_listener::Event::new()),
            is_ready: Arc::new(AtomicBool::new(false)),
            connected_notifier: Arc::new(event_listener::Event::new()),
            major_sync_task_sender: tx,
            pairing_cancellation_tx: Arc::new(Mutex::new(None)),
            pair_code_state: Arc::new(Mutex::new(wacore::pair_code::PairCodeState::default())),
            passkey_state: Arc::new(Mutex::new(crate::passkey::flow::PasskeyFlowState::default())),
            passkey_opening: AtomicBool::new(false),
            custom_enc_handlers: std::sync::OnceLock::new(),
            inbound_durability_hook: std::sync::OnceLock::new(),
            chatstate_handlers: Arc::new(RwLock::new(Vec::new())),
            pdo_pending_requests: cache_config.pdo_pending_requests.build_with_ttl(),
            pdo_requested: cache_config.pdo_requested.build_with_ttl(),
            device_registry_cache: crate::client::device_topology::DeviceRegistryCache::new(
                cache_config.device_registry_cache.build_typed_ttl(
                    cache_config.cache_stores.device_registry_cache.clone(),
                    "device_registry",
                ),
                Arc::clone(&device_topology),
            ),
            device_topology,
            group_devices_memo_enabled: cache_config.cache_stores.device_registry_cache.is_none()
                && cache_config.cache_stores.lid_pn_cache.is_none(),
            group_devices_memo: Cache::builder()
                .max_capacity(GROUP_DEVICES_MEMO_CAPACITY)
                .build(),
            // Evicting a lock whose guard is still held only lets one extra
            // send re-run that group's fan-out (the pre-single-flight
            // behavior); the sender-key chain lock still guarantees ratchet
            // correctness.
            group_distribution_locks: Cache::builder()
                .max_capacity(cache_config.group_distribution_locks_capacity.max(1))
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
            raw_node_forwarding: AtomicBool::new(false),
            #[cfg(feature = "voip")]
            call_registry: std::sync::Arc::new(wacore::voip::CallRegistry::new()),
            #[cfg(feature = "voip")]
            pending_outgoing_calls: std::sync::Arc::new(std::sync::Mutex::new(
                std::collections::HashMap::new(),
            )),
        };

        let arc = Arc::new(this);
        // Mapping changes alter which canonical record a device lookup
        // resolves to, so LidPnCache records into the same topology tracker.
        arc.lid_pn_cache
            .attach_topology(Arc::clone(&arc.device_topology));
        let _ = arc.self_weak.set(Arc::downgrade(&arc));

        // Warm up the LID-PN cache from persistent storage
        let warm_up_arc = arc.clone();
        arc.runtime
            .spawn(Box::pin(async move {
                if let Err(e) = warm_up_arc.warm_up_lid_pn_cache().await {
                    warn!("Failed to warm up LID-PN cache: {e}");
                }
            }))
            .detach();

        // Start background task to clean up stale device registry entries
        let cleanup_arc = arc.clone();
        arc.runtime
            .spawn(Box::pin(async move {
                cleanup_arc.device_registry_cleanup_loop().await;
            }))
            .detach();

        (arc, rx)
    }

    // Deliberately NOT instrumented: this span would live for the entire client
    // lifetime, distorting duration/throughput metrics just like the removed
    // keepalive-loop span. Identity (lid/pn) attribution comes from the
    // per-operation spans (send/request), which record it themselves.
    pub async fn run(self: &Arc<Self>) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            warn!("Client `run` method called while already running.");
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
                let is_transient = connect_err
                    .downcast_ref::<crate::handshake::HandshakeError>()
                    .is_some_and(|e| e.is_transient());
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
                    Ok(super::node_io::ReadLoopExit::Expected) => {
                        debug!("Message loop exited gracefully (expected disconnect).");
                        None
                    }
                    Ok(super::node_io::ReadLoopExit::ServerRecycle(reason)) => {
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
                        crate::types::events::Disconnected { reason },
                    ));
                }
            }

            if !self.enable_auto_reconnect.load(Ordering::Relaxed) {
                info!("Auto-reconnect disabled, shutting down.");
                self.is_running.store(false, Ordering::Relaxed);
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
            self.runtime.sleep(delay).await;
        }
        info!("Client run loop has shut down.");
    }

    /// Boxed barrier: see [`crate::bot::Bot::run`]. Coroutines are LocalCopy
    /// across crates, so consumers awaiting the connect graph directly would
    /// re-codegen it; the box makes them poll through a vtable instead.
    pub async fn connect(self: &Arc<Self>) -> Result<(), anyhow::Error> {
        self.connect_boxed().await
    }

    #[inline(never)]
    fn connect_boxed(
        self: &Arc<Self>,
    ) -> wacore::runtime::BoxFuture<'_, Result<(), anyhow::Error>> {
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
    async fn connect_graph(self: &Arc<Self>) -> Result<(), anyhow::Error> {
        #[cfg(feature = "tracing")]
        self.record_identity_on_span(&tracing::Span::current());

        if self.is_connecting.swap(true, Ordering::SeqCst) {
            return Err(ClientError::AlreadyConnected.into());
        }

        let _guard = scopeguard::guard((), |_| {
            self.is_connecting.store(false, Ordering::Relaxed);
        });

        if self.is_connected() {
            return Err(ClientError::AlreadyConnected.into());
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
            .map_err(|_| anyhow!("Version fetch timed out after {TRANSPORT_CONNECT_TIMEOUT:?}"))?
            .map_err(|e| anyhow!("Failed to resolve app version: {}", e))?;
        let (transport, mut transport_events) = transport_result.map_err(|_| {
            anyhow!("Transport connect timed out after {TRANSPORT_CONNECT_TIMEOUT:?}")
        })??;
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

        *self.transport.lock().await = Some(transport);
        *self.transport_events.lock().await = Some(transport_events);
        *self.noise_socket.lock().await = Some(noise_socket);
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
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.logout", level = "info", skip_all, err(Debug))
    )]
    pub async fn logout(self: &Arc<Self>) -> Result<()> {
        use wacore::iq::devices::RemoveCompanionDeviceSpec;

        self.enable_auto_reconnect.store(false, Ordering::Relaxed);

        if self.is_connected()
            && let Ok(jid) = self.require_pn()
            && let Err(e) = self.execute(RemoveCompanionDeviceSpec::new(&jid)).await
        {
            warn!("Failed to send logout IQ: {e}");
        }

        self.disconnect().await;

        self.core
            .event_bus
            .dispatch(Event::LoggedOut(crate::types::events::LoggedOut {
                on_connect: false,
                reason: ConnectFailureReason::LoggedOut,
            }));

        Ok(())
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
            .flush_inbound_commits_bounded(std::time::Duration::from_secs(5))
            .await
        {
            self.flush_offline_receipts();
        }
        // Prevent late receipt producers from escaping the drain window.
        self.outbound_flush.close();
        self.outbound_flush
            .flush(&*self.runtime, std::time::Duration::from_secs(5))
            .await;
        self.notify_connection_shutdown();

        if let Err(e) = self.persistence_manager.flush().await {
            log::error!("Failed to flush device state during disconnect: {e}");
        }

        // Close after flush; cleanup may also win this race on the run loop.
        if let Some(transport) = self.transport.lock().await.as_ref() {
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
    }

    /// Backoff step used by [`reconnect()`] to create an offline window.
    ///
    /// `fibonacci_backoff(RECONNECT_BACKOFF_STEP)` determines the delay before
    /// the run loop re-connects.  This must be longer than the mock server's
    /// chatstate TTL (`CHATSTATE_TTL_SECS=3`) so TTL-expiry tests pass.
    ///
    /// Sequence: fib(0)=1s, fib(1)=1s, fib(2)=2s, fib(3)=3s, **fib(4)=5s**.
    pub const RECONNECT_BACKOFF_STEP: u32 = 4;

    /// Drop the current connection and trigger the auto-reconnect loop.
    ///
    /// Unlike [`disconnect`], this does **not** stop the run loop. The client
    /// will reconnect automatically using the same persisted identity/store,
    /// just as it would after a network interruption. Use
    /// [`wait_for_connected`] to wait for the new connection to be ready.
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
        wacore::telemetry::reconnect();
        self.intentional_reconnect.store(true, Ordering::Relaxed);
        self.auto_reconnect_errors
            .store(Self::RECONNECT_BACKOFF_STEP, Ordering::Relaxed);
        // Deliberate step: the stability reset must not erase it.
        self.backoff_reset_suppressed.store(true, Ordering::Relaxed);

        // Same durable-before-receipts gate as disconnect().
        if self
            .flush_inbound_commits_bounded(std::time::Duration::from_secs(2))
            .await
        {
            self.flush_offline_receipts();
        }
        self.outbound_flush.close();
        self.outbound_flush
            .flush(&*self.runtime, std::time::Duration::from_secs(2))
            .await;
        self.notify_connection_shutdown();

        if let Some(transport) = self.transport.lock().await.as_ref() {
            transport.disconnect().await;
        }
    }

    /// Drop the current connection and reconnect immediately with no delay.
    ///
    /// Unlike [`reconnect`], which introduces a deliberate offline window,
    /// this method sets the `expected_disconnect` flag so the run loop
    /// skips the backoff delay and reconnects as fast as possible.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.reconnect_immediately", level = "info", skip_all)
    )]
    pub async fn reconnect_immediately(self: &Arc<Self>) {
        info!("Reconnecting immediately (expected disconnect).");
        self.expected_disconnect.store(true, Ordering::Relaxed);

        // Same durable-before-receipts gate as disconnect().
        if self
            .flush_inbound_commits_bounded(std::time::Duration::from_secs(2))
            .await
        {
            self.flush_offline_receipts();
        }
        self.outbound_flush.close();
        self.outbound_flush
            .flush(&*self.runtime, std::time::Duration::from_secs(2))
            .await;
        self.notify_connection_shutdown();

        if let Some(transport) = self.transport.lock().await.as_ref() {
            transport.disconnect().await;
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.cleanup", level = "debug", skip_all)
    )]
    pub(crate) async fn cleanup_connection_state(&self) {
        // Bump the generation FIRST: it is the "this connection is over"
        // signal every per-connection loop already polls. Chat-lane workers
        // stop draining their queues (their remaining stanzas were never
        // acked and redeliver), stale finishers/timers stand down, and —
        // combined with the post-permit generation re-check in
        // process_classified_message — no decrypt can START after the
        // permit-held cache settle below, so no rowless ratchet advances can
        // dirty the cache behind teardown's back.
        self.connection_generation.fetch_add(1, Ordering::SeqCst);
        // Note: node_waiters are intentionally NOT cleared here — they are
        // cross-connection (callers may register a waiter before an action that
        // completes on a subsequent connection, e.g. after 515 reconnect).
        // sent_node_waiters ARE cleared because they match pre-encryption
        // outgoing stanzas, which are transport-scoped.
        self.clear_sent_node_waiters();
        self.is_logged_in.store(false, Ordering::Relaxed);
        self.is_ready.store(false, Ordering::Relaxed);
        // Publish the disconnected state BEFORE draining VoIP calls (it used to be cleared only after
        // the socket teardown below): a concurrent accept()/call() setup that finishes its async work
        // in this window must see `!is_connected()` and bail instead of registering/connecting a call
        // after this sweep.
        self.is_connected.store(false, Ordering::Release);
        // Tear down every in-flight VoIP call: the relay socket and signaling are connection-scoped,
        // so a call can't survive a disconnect/reconnect. Aborts each media task and clears the map.
        #[cfg(feature = "voip")]
        {
            self.call_registry.abort_all();
            // Dormant outgoing calls (relay never arrived) live in pending_outgoing_calls, not the
            // registry, so abort_all misses them. Drain them and notify `ended` so any waiter wakes.
            crate::voip::facade::drain_pending_outgoing_on_disconnect(self);
        }
        // Signal the keepalive loop (and any other per-connection tasks) to
        // exit promptly. Without this, a stale keepalive loop can overlap
        // with the next one after reconnect. Uses the PER-CONNECTION signal
        // so the terminal shutdown_notifier stays clean for reconnects.
        self.notify_connection_shutdown();
        // Close the socket as part of cleanup so this path is authoritative
        // even when reached via the run loop's graceful-exit flow (not just
        // `Client::disconnect()`). Transport impls make `disconnect()`
        // idempotent, so the redundant call from `Client::disconnect()` is
        // safe.
        if let Some(transport) = self.transport.lock().await.take() {
            transport.disconnect().await;
        }
        *self.transport_events.lock().await = None;
        *self.noise_socket.lock().await = None;
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
        if let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) {
            client
                .teardown_inbound_commits_bounded(std::time::Duration::from_secs(5))
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
        self.pending_device_sync.clear().await;
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
        self.history_sync_tasks_in_flight
            .store(0, Ordering::Relaxed);
        self.history_sync_idle_notifier.notify(usize::MAX);
        // Drain all pending IQ waiters so they fail fast with InternalChannelClosed
        // instead of hanging until the 75s timeout.
        // Scoped so the sync guard is dropped before the awaits below (a
        // std::sync::MutexGuard held across an await would make this future !Send).
        let waiter_count = {
            let mut waiters_map = self.response_waiters_guard();
            let count = waiters_map.len();
            // Replace with new map to release backing storage; old senders drop
            // here, causing receivers to get RecvError → InternalChannelClosed.
            *waiters_map = HashMap::new();
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
        *self.app_state_syncing.lock().await = HashSet::new();

        // Drop stale media connection (auth tokens become invalid on reconnect)
        *self.media_conn.write().await = None;

        // Clear app state key cache — keys will be re-fetched from DB on demand
        if let Some(proc) = self.app_state_processor.lock().await.as_ref() {
            proc.clear_key_cache().await;
        }
    }

    /// Waits for the noise socket to be established.
    ///
    /// Returns `Ok(())` when the socket is ready, or `Err` on timeout.
    /// This is useful for code that needs to send messages before login,
    /// such as requesting a pair code during initial pairing.
    ///
    /// If the socket is already connected, returns immediately.
    pub async fn wait_for_socket(&self, timeout: std::time::Duration) -> Result<(), anyhow::Error> {
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
            .map_err(|_| anyhow::anyhow!("Timeout waiting for socket"))
    }

    /// Waits for the client to establish a connection and complete login.
    ///
    /// Returns `Ok(())` when connected, or `Err` on timeout.
    /// This is useful for code that needs to run after connection is established
    /// and authentication is complete.
    ///
    /// If the client is already connected and logged in, returns immediately.
    pub async fn wait_for_connected(
        &self,
        timeout: std::time::Duration,
    ) -> Result<(), anyhow::Error> {
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
            .map_err(|_| anyhow::anyhow!("Timeout waiting for connection"))
    }

    pub fn is_connected(&self) -> bool {
        self.is_connected.load(Ordering::Acquire)
    }

    /// Force the connected flag (tests only): the facade's connect path now gates on `is_connected`,
    /// so a unit test driving `spawn_call`/`place_call` must mark the client connected first.
    #[cfg(all(test, feature = "voip"))]
    pub(crate) fn set_connected_for_test(&self, connected: bool) {
        self.is_connected.store(connected, Ordering::Release);
    }

    pub fn is_logged_in(&self) -> bool {
        self.is_logged_in.load(Ordering::Relaxed)
    }
}
