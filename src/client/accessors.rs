//! Small accessors, config setters, node waiters and sync-error helpers.

use super::*;

/// Identity for span/error tagging. Named fields, not a tuple — LID/PN transposition would
/// otherwise be a silent, unchecked bug at call sites.
#[cfg(feature = "tracing")]
#[derive(Debug, Clone, Default)]
pub struct IdentityTags {
    pub lid: Option<String>,
    pub pn: Option<String>,
}

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

    /// Retune the per-chat outbound resend rate limiter live (no reconnect).
    ///
    /// Outbound resends to a chat are bounded by a token bucket: `burst` is the
    /// instantaneous allowance and `refill_per_min` the sustained ceiling per
    /// chat. This caps the aggregate resend rate that WhatsApp's anti-abuse
    /// penalizes during a PN to LID migration fan-out, while throttled devices
    /// still recover via the fresh-SKDM mark. A `burst` of 0 disables the limiter.
    ///
    /// Takes effect on each chat's next retry; a lowered `burst` clamps a live
    /// bucket on its next access.
    pub fn set_resend_rate_limit(&self, burst: u32, refill_per_min: u32) {
        self.resend_rate_limiter.set_rate(burst, refill_per_min);
    }

    /// Cumulative wire I/O and activity counters for this client session.
    ///
    /// Always available, no feature gate: recording costs one relaxed atomic
    /// add per wire frame. Byte counts are post-noise wire bytes (frame
    /// headers and AEAD tags included; handshake and TLS/WebSocket overhead
    /// excluded), so two clients in one process can be compared directly.
    pub fn stats(&self) -> StatsSnapshot {
        let mut snapshot = self.stats.snapshot();
        snapshot.reconnect_errors = self.auto_reconnect_errors.load(Ordering::Relaxed);
        snapshot.resends_throttled = self.resend_rate_limiter.throttled_total();
        snapshot
    }

    /// Entry counts plus estimated retained heap bytes for the client's
    /// internal collections. See [`MemoryReport`] for the semantics of the
    /// byte figures.
    ///
    /// On-demand only: walks the in-process caches under their locks when
    /// called, costs nothing otherwise. Counts are approximate (caches may
    /// have pending evictions); call `run_pending_tasks()` on individual
    /// caches first if you need exact counts.
    pub async fn memory_report(&self) -> MemoryReport {
        use wacore::stats::{CollectionStats, HeapSize};

        let (signal_sessions, signal_identities, signal_sender_keys) =
            self.signal_cache.memory_stats().await;
        let (lid_pn_lid_entries, lid_pn_pn_entries) = self.lid_pn_cache.memory_stats().await;
        let pending_retries_count = self
            .pending_retries
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .len();

        // Only the Arc is taken under the mutex — the walk must not block
        // get_group_cache(), which every group send goes through.
        let group_cache_arc = self.group_cache.lock().await.clone();
        let group_cache = match group_cache_arc {
            // Arc<T>'s HeapSize already includes size_of::<GroupInfo>().
            Some(cache) => {
                cache
                    .memory_stats(|k, v| k.heap_bytes() + v.heap_bytes())
                    .await
            }
            None => CollectionStats::default(),
        };

        let recent_messages = self
            .recent_messages
            .memory_stats(|k, v| k.chat.heap_bytes() + k.id.heap_bytes() + v.heap_bytes())
            .await;

        let group_devices_memo = self
            .group_devices_memo
            .memory_stats(|k, v| k.heap_bytes() + v.heap_bytes())
            .await;

        // Each count read into a local so no two guards are ever held at once.
        let response_waiters = self.response_waiters.lock().await.len();
        let presence_subscriptions = self.presence_subscriptions.lock().await.len();
        let app_state_key_requests = self.app_state_key_requests.lock().await.len();
        let app_state_syncing = self.app_state_syncing.lock().await.len();
        let chatstate_handlers = self.chatstate_handlers.read().await.len();

        MemoryReport {
            group_cache,
            device_registry_cache: self.device_registry_cache.memory_stats().await,
            lid_pn_lid_entries,
            lid_pn_pn_entries,
            recent_messages,
            sender_key_device_cache: self.sender_key_device_cache.memory_stats().await,
            group_devices_memo,
            message_retry_counts: self.message_retry_counts.entry_count(),
            undecryptable_dispatched: self.undecryptable_dispatched.entry_count(),
            pdo_pending_requests: self.pdo_pending_requests.entry_count(),
            pdo_requested: self.pdo_requested.entry_count(),
            session_locks: self.session_locks.entry_count(),
            chat_lanes: self.chat_lanes.entry_count(),
            resend_rate_limiter_chats: self.resend_rate_limiter.entry_count(),
            response_waiters,
            node_waiters: self.node_waiter_count.load(Ordering::Relaxed),
            pending_retries: pending_retries_count,
            presence_subscriptions,
            app_state_key_requests,
            app_state_syncing,
            signal_sessions,
            signal_identities,
            signal_sender_keys,
            chatstate_handlers,
            custom_enc_handlers: self.custom_enc_handlers.get().map_or(0, |m| m.len()),
        }
    }

    /// Get access to the PersistenceManager for this client.
    /// This is useful for multi-account scenarios to get the device ID.
    pub fn persistence_manager(&self) -> Arc<PersistenceManager> {
        self.persistence_manager.clone()
    }

    // The owned returns below are the only clones left: the snapshot read
    // itself is an Arc refcount bump (no lock against writers). Callers that
    // only need a borrow can hold `persistence_manager().get_device_snapshot()`
    // and read fields directly.
    pub fn get_push_name(&self) -> String {
        self.persistence_manager
            .get_device_snapshot()
            .push_name
            .clone()
    }

    pub fn get_pn(&self) -> Option<Jid> {
        self.persistence_manager.get_device_snapshot().pn.clone()
    }

    pub fn get_lid(&self) -> Option<Jid> {
        self.persistence_manager.get_device_snapshot().lid.clone()
    }

    /// Snapshot-consistent identity for span/error tagging (redacted PN, raw LID). Named
    /// fields, not a tuple — LID/PN transposition would otherwise be a silent, unchecked bug.
    #[cfg(feature = "tracing")]
    pub fn identity_tags(&self) -> IdentityTags {
        let snapshot = self.persistence_manager.get_device_snapshot();
        IdentityTags {
            lid: snapshot.lid.as_ref().map(|j| j.to_string()),
            pn: snapshot.pn.as_ref().map(|j| j.observe().to_string()),
        }
    }

    /// Shared so every identity-tagged span leaves a field absent (not `""`) when unknown —
    /// duplicating this per call site would drift out of sync. Skips the snapshot read when
    /// the span is disabled.
    #[cfg(feature = "tracing")]
    pub(crate) fn record_identity_on_span(&self, span: &tracing::Span) {
        if span.is_disabled() {
            return;
        }
        let tags = self.identity_tags();
        if let Some(lid) = tags.lid {
            span.record("lid", tracing::field::display(lid));
        }
        if let Some(pn) = tags.pn {
            span.record("pn", tracing::field::display(pn));
        }
    }

    pub(crate) fn require_pn(&self) -> Result<Jid> {
        self.get_pn().ok_or(ClientError::NotLoggedIn.into())
    }

    /// Resolve our own JID for a group, respecting its addressing mode.
    ///
    /// Returns LID for LID-addressing groups, PN otherwise.
    /// Matches WhatsApp Web's `getMeUserLidOrJidForChat`.
    pub(crate) async fn get_own_jid_for_group(
        &self,
        group_jid: &Jid,
    ) -> Result<Jid, anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
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
        let device_snapshot = self.persistence_manager.get_device_snapshot();
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
