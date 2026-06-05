//! Client construction and connection lifecycle: connect, run, reconnect, shutdown.

use super::*;

impl Client {
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

        let device_snapshot = persistence_manager.get_device_snapshot().await;
        let core = wacore::client::CoreClient::new(device_snapshot.core.clone());

        let (tx, rx) = async_channel::bounded(32);

        let this = Self {
            runtime: runtime.clone(),
            core,
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
            last_data_received_ms: Arc::new(AtomicU64::new(0)),
            last_data_sent_ms: Arc::new(AtomicU64::new(0)),

            transport: Arc::new(Mutex::new(None)),
            transport_events: Arc::new(Mutex::new(None)),
            transport_factory,
            noise_socket: Arc::new(Mutex::new(None)),

            response_waiters: Arc::new(Mutex::new(HashMap::new())),
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

            recent_retry_reasons: cache_config.message_retry_counts.build_with_ttl(),

            session_recreate_history: cache_config.session_recreate_history.build_with_ttl(),

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

            needs_initial_full_sync: Arc::new(AtomicBool::new(false)),

            app_state_processor: async_lock::Mutex::new(None),
            app_state_key_requests: Arc::new(Mutex::new(HashMap::new())),
            app_state_syncing: Arc::new(Mutex::new(HashSet::new())),
            initial_keys_synced_notifier: Arc::new(event_listener::Event::new()),
            initial_app_state_keys_received: Arc::new(AtomicBool::new(false)),
            prekey_upload_lock: Arc::new(async_lock::Mutex::new(())),
            offline_sync_notifier: Arc::new(event_listener::Event::new()),
            offline_sync_completed: Arc::new(AtomicBool::new(false)),
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
            custom_enc_handlers: Arc::new(async_lock::RwLock::new(HashMap::new())),
            chatstate_handlers: Arc::new(RwLock::new(Vec::new())),
            pdo_pending_requests: cache_config.pdo_pending_requests.build_with_ttl(),
            device_registry_cache: cache_config.device_registry_cache.build_typed_ttl(
                cache_config.cache_stores.device_registry_cache.clone(),
                "device_registry",
            ),
            stanza_router: Self::create_stanza_router(),
            synchronous_ack: false,
            http_client,
            override_version,
            skip_history_sync: AtomicBool::new(false),
            wanted_pre_key_count: AtomicUsize::new(crate::prekeys::DEFAULT_WANTED_PRE_KEY_COUNT),
            cache_config,
            self_weak: std::sync::OnceLock::new(),
            saver_handle: std::sync::OnceLock::new(),
            raw_node_forwarding: AtomicBool::new(false),
        };

        let arc = Arc::new(this);
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

    pub async fn run(self: &Arc<Self>) {
        if self.is_running.swap(true, Ordering::SeqCst) {
            warn!("Client `run` method called while already running.");
            return;
        }
        while self.is_running.load(Ordering::Relaxed) {
            self.expected_disconnect.store(false, Ordering::Relaxed);

            if let Err(connect_err) = self.connect().await {
                let is_transient = connect_err
                    .downcast_ref::<crate::handshake::HandshakeError>()
                    .is_some_and(|e| e.is_transient());
                if is_transient {
                    debug!("Transient connect failure, will retry: {connect_err:#}");
                } else {
                    error!("Failed to connect: {connect_err:#}. Will retry...");
                }
            } else {
                let unexpected_disconnect = if self.read_messages_loop().await.is_err() {
                    // Check intentional_reconnect AFTER read loop exits — reconnect()
                    // sets this flag while the loop is running, so it must be read here.
                    if self.expected_disconnect.load(Ordering::Relaxed)
                        || self.intentional_reconnect.swap(false, Ordering::Relaxed)
                    {
                        debug!("Message loop exited during expected disconnect.");
                        false
                    } else {
                        warn!(
                            "Message loop exited with an error. Will attempt to reconnect if enabled."
                        );
                        true
                    }
                } else if self.expected_disconnect.load(Ordering::Relaxed) {
                    debug!("Message loop exited gracefully (expected disconnect).");
                    false
                } else {
                    info!("Message loop exited gracefully.");
                    false
                };

                self.cleanup_connection_state().await;

                // Dispatch after cleanup so handlers see cleared connection state.
                if unexpected_disconnect {
                    self.core
                        .event_bus
                        .dispatch(Event::Disconnected(crate::types::events::Disconnected));
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
                info!("Expected disconnect (e.g., 515), reconnecting immediately...");
                continue;
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

    pub async fn connect(self: &Arc<Self>) -> Result<(), anyhow::Error> {
        if self.is_connecting.swap(true, Ordering::SeqCst) {
            return Err(ClientError::AlreadyConnected.into());
        }

        let _guard = scopeguard::guard((), |_| {
            self.is_connecting.store(false, Ordering::Relaxed);
        });

        if self.is_connected() {
            return Err(ClientError::AlreadyConnected.into());
        }

        // Reset login state for new connection attempt. This ensures that
        // handle_success will properly process the <success> stanza even if
        // a previous connection's post-login task bailed out early.
        self.is_logged_in.store(false, Ordering::Relaxed);
        self.is_ready.store(false, Ordering::Relaxed);
        self.is_connected.store(false, Ordering::Relaxed);
        self.offline_sync_completed.store(false, Ordering::Relaxed);
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
    pub async fn logout(self: &Arc<Self>) -> Result<()> {
        use wacore::iq::devices::RemoveCompanionDeviceSpec;

        self.enable_auto_reconnect.store(false, Ordering::Relaxed);

        if self.is_connected()
            && let Ok(jid) = self.require_pn().await
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

    pub async fn disconnect(self: &Arc<Self>) {
        info!("Disconnecting client intentionally.");
        self.expected_disconnect.store(true, Ordering::Relaxed);
        self.is_running.store(false, Ordering::Relaxed);
        self.shutdown_notifier.notify();

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
    pub async fn reconnect(self: &Arc<Self>) {
        info!("Reconnecting: dropping transport for auto-reconnect.");
        self.intentional_reconnect.store(true, Ordering::Relaxed);
        self.auto_reconnect_errors
            .store(Self::RECONNECT_BACKOFF_STEP, Ordering::Relaxed);

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
    pub async fn reconnect_immediately(self: &Arc<Self>) {
        info!("Reconnecting immediately (expected disconnect).");
        self.expected_disconnect.store(true, Ordering::Relaxed);

        self.outbound_flush.close();
        self.outbound_flush
            .flush(&*self.runtime, std::time::Duration::from_secs(2))
            .await;
        self.notify_connection_shutdown();

        if let Some(transport) = self.transport.lock().await.as_ref() {
            transport.disconnect().await;
        }
    }

    pub(crate) async fn cleanup_connection_state(&self) {
        // Note: node_waiters are intentionally NOT cleared here — they are
        // cross-connection (callers may register a waiter before an action that
        // completes on a subsequent connection, e.g. after 515 reconnect).
        // sent_node_waiters ARE cleared because they match pre-encryption
        // outgoing stanzas, which are transport-scoped.
        self.clear_sent_node_waiters();
        self.is_logged_in.store(false, Ordering::Relaxed);
        self.is_ready.store(false, Ordering::Relaxed);
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
        // Clear is_connected AFTER noise_socket is None, so no task can see
        // is_connected==true with a cleared socket. send_node() independently
        // checks the socket, but this ordering avoids a confusing state window.
        self.is_connected.store(false, Ordering::Release);
        // Presence doesn't survive reconnects: demote presence-driven active
        // receipts (1 -> 0), leaving a forced value (2) untouched.
        let _ =
            self.send_active_receipts
                .compare_exchange(1, 0, Ordering::AcqRel, Ordering::Acquire);
        // Drop per-chat lanes so workers exit via channel close.
        self.chat_lanes.invalidate_all();
        // Clear pending retries so stale keys from detached scopeguard
        // cleanup don't suppress the first retry after reconnect.
        self.pending_retries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clear();
        // Flush before clear: clear() drops dirty entries, so a disconnect
        // racing an in-flight encrypt would lose the just-advanced sender-key
        // chain and force a full SKDM re-fanout. A disconnect is not a logout.
        // Only clear on a successful flush; on a backend error keep the cache so
        // the dirty state isn't dropped and the next operation can persist it.
        match self.flush_signal_cache().await {
            Ok(()) => self.signal_cache.clear().await,
            Err(e) => log::error!(
                "cleanup_connection_state: signal cache flush failed, keeping cache to avoid dropping Signal state: {e:?}"
            ),
        }
        // Reset semaphore to 1 permit for next offline sync.
        self.swap_message_semaphore(1);
        // Reset dead-socket timestamps so stale values from the previous
        // connection don't trigger an immediate reconnect on the next one.
        self.last_data_received_ms.store(0, Ordering::Relaxed);
        self.last_data_sent_ms.store(0, Ordering::Relaxed);
        self.pending_device_sync.clear().await;
        // Reset offline sync state for next connection
        self.offline_sync_completed.store(false, Ordering::Relaxed);
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
        let mut waiters_map = self.response_waiters.lock().await;
        let waiter_count = waiters_map.len();
        // Replace with new map to release backing storage; old senders drop here,
        // causing receivers to get RecvError → IqError::InternalChannelClosed
        *waiters_map = HashMap::new();
        drop(waiters_map);
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

    pub fn is_logged_in(&self) -> bool {
        self.is_logged_in.load(Ordering::Relaxed)
    }
}
