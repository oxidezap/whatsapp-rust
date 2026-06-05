//! Small accessors, config setters, node waiters and sync-error helpers.

use super::*;

impl Client {
    pub(crate) async fn get_group_cache(&self) -> Arc<GroupCache> {
        let mut guard = self.group_cache.lock().await;
        if let Some(cache) = guard.as_ref() {
            return cache.clone();
        }
        debug!("Initializing Group Cache for the first time.");
        let cache = Arc::new(
            self.cache_config
                .group_cache
                .build_typed_ttl(self.cache_config.cache_stores.group_cache.clone(), "group"),
        );
        *guard = Some(cache.clone());
        cache
    }

    /// Registers an external event handler to the core event bus.
    pub fn register_handler(&self, handler: Arc<dyn wacore::types::events::EventHandler>) {
        self.core.event_bus.add_handler(handler);
    }

    /// Enable or disable raw node forwarding.
    /// When enabled, `Event::RawNode` is emitted for every decoded stanza before
    /// the stanza router dispatches it. Only enable when external consumers need
    /// raw protocol access (e.g. voice call stanzas).
    pub fn set_raw_node_forwarding(&self, enabled: bool) {
        self.raw_node_forwarding.store(enabled, Ordering::Relaxed);
    }

    /// Enable or disable skipping of history sync notifications at runtime.
    ///
    /// When enabled, the client will acknowledge incoming history sync
    /// notifications but will not download or process the data.
    pub fn set_skip_history_sync(&self, enabled: bool) {
        self.skip_history_sync.store(enabled, Ordering::Relaxed);
    }

    /// Returns `true` if history sync notifications are currently being skipped.
    pub fn skip_history_sync_enabled(&self) -> bool {
        self.skip_history_sync.load(Ordering::Relaxed)
    }

    /// Set how many one-time pre-keys are generated per upload batch.
    ///
    /// Defaults to WA Web's UPLOAD_KEYS_COUNT (812). Call before connecting; it
    /// takes effect on the next pre-key upload. The value is clamped to the
    /// protocol-safe range at upload time, so out-of-range values are coerced
    /// (and logged) rather than rejected here.
    pub fn set_wanted_pre_key_count(&self, count: usize) {
        self.wanted_pre_key_count.store(count, Ordering::Relaxed);
    }

    /// Returns the configured pre-key upload batch size (the raw value, before
    /// the upload-time clamp).
    pub fn wanted_pre_key_count(&self) -> usize {
        self.wanted_pre_key_count.load(Ordering::Relaxed)
    }

    /// Returns a snapshot of all internal collection sizes for memory leak detection.
    ///
    /// Moka caches report approximate counts (pending evictions may not be reflected).
    /// Call `run_pending_tasks()` on individual caches first if you need exact counts.
    ///
    /// Requires the `debug-diagnostics` feature.
    #[cfg(feature = "debug-diagnostics")]
    pub async fn memory_diagnostics(&self) -> MemoryDiagnostics {
        let (sig_sessions, sig_identities, sig_sender_keys) =
            self.signal_cache.entry_counts().await;
        let (lid_lid, lid_pn) = self.lid_pn_cache.entry_counts();
        let pending_retries_count = self
            .pending_retries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len();

        MemoryDiagnostics {
            group_cache: self
                .group_cache
                .lock()
                .await
                .as_ref()
                .map_or(0, |c| c.entry_count()),
            device_registry_cache: self.device_registry_cache.entry_count(),
            lid_pn_lid_entries: lid_lid,
            lid_pn_pn_entries: lid_pn,
            recent_messages: self.recent_messages.entry_count(),
            sender_key_device_cache: self.sender_key_device_cache.entry_count(),
            message_retry_counts: self.message_retry_counts.entry_count(),
            undecryptable_dispatched: self.undecryptable_dispatched.entry_count(),
            pdo_pending_requests: self.pdo_pending_requests.entry_count(),
            session_locks: self.session_locks.entry_count(),
            chat_lanes: self.chat_lanes.entry_count(),
            response_waiters: self.response_waiters.lock().await.len(),
            node_waiters: self.node_waiter_count.load(Ordering::Relaxed),
            pending_retries: pending_retries_count,
            presence_subscriptions: self.presence_subscriptions.lock().await.len(),
            app_state_key_requests: self.app_state_key_requests.lock().await.len(),
            app_state_syncing: self.app_state_syncing.lock().await.len(),
            signal_cache_sessions: sig_sessions,
            signal_cache_identities: sig_identities,
            signal_cache_sender_keys: sig_sender_keys,
            chatstate_handlers: self.chatstate_handlers.read().await.len(),
            custom_enc_handlers: self.custom_enc_handlers.read().await.len(),
        }
    }

    /// Get access to the PersistenceManager for this client.
    /// This is useful for multi-account scenarios to get the device ID.
    pub fn persistence_manager(&self) -> Arc<PersistenceManager> {
        self.persistence_manager.clone()
    }

    pub async fn get_push_name(&self) -> String {
        self.persistence_manager
            .get_device_arc()
            .await
            .read()
            .await
            .push_name
            .clone()
    }

    pub async fn get_pn(&self) -> Option<Jid> {
        self.persistence_manager
            .get_device_arc()
            .await
            .read()
            .await
            .pn
            .clone()
    }

    pub async fn get_lid(&self) -> Option<Jid> {
        self.persistence_manager
            .get_device_arc()
            .await
            .read()
            .await
            .lid
            .clone()
    }

    pub(crate) async fn require_pn(&self) -> Result<Jid> {
        self.get_pn().await.ok_or(ClientError::NotLoggedIn.into())
    }

    /// Resolve our own JID for a group, respecting its addressing mode.
    ///
    /// Returns LID for LID-addressing groups, PN otherwise.
    /// Matches WhatsApp Web's `getMeUserLidOrJidForChat`.
    pub(crate) async fn get_own_jid_for_group(
        &self,
        group_jid: &Jid,
    ) -> Result<Jid, anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot().await;
        let own_pn = device_snapshot
            .pn
            .clone()
            .ok_or_else(|| anyhow::Error::from(ClientError::NotLoggedIn))?;

        let addressing_mode = self
            .groups()
            .query_info(group_jid)
            .await
            .map(|info| info.addressing_mode)
            .unwrap_or(crate::types::message::AddressingMode::Pn);

        Ok(match addressing_mode {
            crate::types::message::AddressingMode::Lid => {
                device_snapshot.lid.clone().unwrap_or(own_pn)
            }
            crate::types::message::AddressingMode::Pn => own_pn,
        })
    }

    pub(crate) async fn update_push_name_and_notify(self: &Arc<Self>, new_name: String) {
        let device_snapshot = self.persistence_manager.get_device_snapshot().await;
        let old_name = device_snapshot.push_name.clone();

        if old_name == new_name {
            return;
        }

        log::debug!("Updating push name from '{}' -> '{}'", old_name, new_name);
        self.persistence_manager
            .process_command(DeviceCommand::SetPushName(new_name.clone()))
            .await;

        self.core.event_bus.dispatch(Event::SelfPushNameUpdated(
            crate::types::events::SelfPushNameUpdated {
                from_server: true,
                old_name,
                new_name: new_name.clone(),
            },
        ));

        let client_clone = self.clone();
        self.runtime
            .spawn(Box::pin(async move {
                if let Err(e) = client_clone.presence().set_available().await {
                    log::warn!("Failed to send presence after push name update: {:?}", e);
                } else {
                    log::debug!("Sent presence after push name update.");
                }
            }))
            .detach();
    }

    /// Register a waiter for an incoming node matching the given filter.
    ///
    /// Returns a receiver that resolves when a matching node arrives.
    /// The waiter starts buffering immediately, so register it **before**
    /// performing the action that triggers the expected node.
    ///
    /// When multiple waiters match the same node, each matching waiter
    /// receives a clone of the node (broadcast within a single resolve pass).
    ///
    /// # Example
    /// ```ignore
    /// let waiter = client.wait_for_node(
    ///     NodeFilter::tag("notification").attr("type", "w:gp2"),
    /// );
    /// client.groups().add_participants(&group_jid, &[jid_c]).await?;
    /// let node = waiter.await.expect("notification arrived");
    /// ```
    pub fn wait_for_node(
        &self,
        filter: NodeFilter,
    ) -> futures::channel::oneshot::Receiver<Arc<wacore_binary::OwnedNodeRef>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.node_waiter_count.fetch_add(1, Ordering::Release);
        let mut waiters = self
            .node_waiters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        waiters.push(NodeWaiter { filter, tx });
        rx
    }

    /// Register a waiter for an outgoing node before it is encrypted and sent.
    ///
    /// This is intended for tests and diagnostics that need to inspect the raw
    /// stanza built by the client, such as asserting whether `<tctoken>` or
    /// `<cstoken>` was attached.
    pub fn wait_for_sent_node(
        &self,
        filter: NodeFilter,
    ) -> futures::channel::oneshot::Receiver<Arc<Node>> {
        let (tx, rx) = futures::channel::oneshot::channel();
        self.sent_node_waiter_count.fetch_add(1, Ordering::Release);
        let mut waiters = self
            .sent_node_waiters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        waiters.push(SentNodeWaiter { filter, tx });
        rx
    }

    /// Check pending node waiters against an incoming node.
    /// Only called when `node_waiter_count > 0`.
    pub(crate) fn resolve_node_waiters(&self, node: &Arc<wacore_binary::OwnedNodeRef>) {
        resolve_waiters(&self.node_waiters, &self.node_waiter_count, node);
    }

    pub(crate) fn resolve_sent_node_waiters(&self, node: &Arc<Node>) {
        let nr = node.as_node_ref();
        let mut waiters = self
            .sent_node_waiters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let mut i = 0;
        while i < waiters.len() {
            if waiters[i].tx.is_canceled() {
                waiters.swap_remove(i);
                self.sent_node_waiter_count.fetch_sub(1, Ordering::Release);
            } else if waiters[i].filter.matches(&nr) {
                let w = waiters.swap_remove(i);
                self.sent_node_waiter_count.fetch_sub(1, Ordering::Release);
                let _ = w.tx.send(Arc::clone(node));
            } else {
                i += 1;
            }
        }
    }

    pub(crate) fn clear_sent_node_waiters(&self) {
        let mut waiters = self
            .sent_node_waiters
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let count = waiters.len();
        if count > 0 {
            waiters.clear();
            self.sent_node_waiter_count
                .fetch_sub(count, Ordering::Release);
        }
    }

    fn should_downgrade_sync_error(&self, err: &anyhow::Error) -> bool {
        if self.is_shutting_down() {
            return true;
        }

        matches!(
            err.downcast_ref::<crate::request::IqError>(),
            Some(
                crate::request::IqError::NotConnected
                    | crate::request::IqError::InternalChannelClosed
            )
        )
    }

    /// Log a sync error, downgrading to debug level during shutdown/disconnect.
    pub(crate) fn log_sync_error(&self, context: &str, err: &anyhow::Error) {
        if self.should_downgrade_sync_error(err) {
            debug!("Skipping {context} during shutdown: {err}");
        } else {
            warn!("Failed {context}: {err}");
        }
    }

    /// Create and configure the stanza router with all the handlers.
    pub(crate) fn create_stanza_router() -> crate::handlers::router::StanzaRouter {
        use crate::handlers::{
            basic::{AckHandler, FailureHandler, StreamErrorHandler, SuccessHandler},
            chatstate::ChatstateHandler,
            ib::IbHandler,
            iq::IqHandler,
            message::MessageHandler,
            notification::NotificationHandler,
            receipt::ReceiptHandler,
            router::StanzaRouter,
        };

        let mut router = StanzaRouter::new();

        // Register all handlers
        router.register(Arc::new(MessageHandler));
        router.register(Arc::new(ReceiptHandler));
        router.register(Arc::new(IqHandler));
        router.register(Arc::new(SuccessHandler));
        router.register(Arc::new(FailureHandler));
        router.register(Arc::new(StreamErrorHandler));
        router.register(Arc::new(IbHandler));
        router.register(Arc::new(NotificationHandler));
        router.register(Arc::new(AckHandler));
        router.register(Arc::new(ChatstateHandler));

        router.register(Arc::new(crate::handlers::call::CallHandler));

        // Register unimplemented handlers
        router.register(Arc::new(crate::handlers::presence::PresenceHandler));

        router
    }
}
