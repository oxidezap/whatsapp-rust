//! Inbound node I/O: read loop, frame decryption, node routing, acks and stream errors.

use super::*;

impl Client {
    /// Read the current semaphore generation and Arc atomically under the mutex.
    pub(crate) fn read_message_semaphore(&self) -> (u64, Arc<async_lock::Semaphore>) {
        let guard = match self.message_processing_semaphore.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        (
            self.message_semaphore_generation.load(Ordering::SeqCst),
            guard.clone(),
        )
    }

    /// Replace the message processing semaphore and bump the generation counter.
    ///
    /// Both operations happen under the same mutex hold so readers always see
    /// a consistent (generation, Arc) pair. Must be called from a non-async
    /// context or inside a scoped block (MutexGuard is !Send).
    pub(crate) fn swap_message_semaphore(&self, permits: usize) {
        let mut guard = match self.message_processing_semaphore.lock() {
            Ok(g) => g,
            Err(poisoned) => poisoned.into_inner(),
        };
        *guard = Arc::new(async_lock::Semaphore::new(permits));
        self.message_semaphore_generation
            .fetch_add(1, Ordering::SeqCst);
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.read_loop", level = "debug", skip_all, err(Debug))
    )]
    pub(crate) async fn read_messages_loop(self: &Arc<Self>) -> Result<(), anyhow::Error> {
        debug!("Starting message processing loop...");

        let mut rx_guard = self.transport_events.lock().await;
        let transport_events = rx_guard
            .take()
            .ok_or_else(|| anyhow::anyhow!("Cannot start message loop: not connected"))?;
        drop(rx_guard);

        // Frame decoder to parse incoming data
        let mut frame_decoder = wacore::framing::FrameDecoder::new();
        let shutdown = self.connection_shutdown_signal();

        loop {
            futures::select_biased! {
                    _ = wacore::runtime::wait_for_shutdown(&shutdown).fuse() => {
                        debug!("Shutdown signaled in message loop. Exiting message loop.");
                        return Ok(());
                    },
                    event_result = transport_events.recv().fuse() => {
                        match event_result {
                            Ok(crate::transport::TransportEvent::DataReceived(data)) => {
                                // Update dead-socket timer (WA Web: deadSocketTimer reset)
                                self.last_data_received_ms.store(
                                    wacore::time::now_millis().max(0) as u64,
                                    Ordering::Relaxed,
                                );

                                // Feed data into the frame decoder
                                frame_decoder.feed(&data);

                                // Process all complete frames.
                                // Frame decryption must be sequential (noise protocol counter),
                                // but we spawn node processing concurrently after decryption.
                                let mut frames_in_batch: u32 = 0;

                                while let Some(encrypted_frame) = frame_decoder.decode_frame() {
                                    // Decrypt the frame synchronously (required for noise counter ordering)
                                    if let Some(node) = self.decrypt_frame(encrypted_frame).await {
                                        // Determine processing mode for this node:
                                        // - Critical nodes (success/failure/stream:error): inline, required for state
                                        // - Message nodes: inline, preserves arrival order for per-chat queues
                                        //   (MessageHandler just enqueues + ACKs, heavy crypto runs in workers)
                                        // - ib (in-band): inline, ensures offline sync tracking (expected count)
                                        //   is set up before offline messages are processed
                                        // - Everything else: spawned concurrently for parallelism
                                        let process_inline = matches!(
                                            node.tag(),
                                            "success" | "failure" | "stream:error" | "message" | "ib"
                                        );

                                        if process_inline {
                                            self.process_decrypted_node(node).await;
                                        } else {
                                            let client = self.clone();
                                            self.runtime.spawn(Box::pin(async move {
                                                client.process_decrypted_node(node).await;
                                            })).detach();
                                        }
                                    }

                                    // Check if we should exit after processing (e.g., after 515 stream error)
                                    if self.expected_disconnect.load(Ordering::Relaxed) {
                                        debug!("Expected disconnect signaled during frame processing. Exiting message loop.");
                                        return Ok(());
                                    }

                                    // Cooperative yield — frequency and behavior are runtime-defined.
                                    frames_in_batch += 1;
                                    if frames_in_batch.is_multiple_of(self.runtime.yield_frequency())
                                        && let Some(yield_fut) = self.runtime.yield_now()
                                    {
                                        yield_fut.await;
                                    }
                                }

                                // Refresh timestamp after processing the entire batch so
                                // the keepalive loop sees the batch completion time, not
                                // just the arrival time. Prevents stale reads when a
                                // large batch (e.g. offline sync) takes seconds to drain.
                                if frames_in_batch > 1 {
                                    self.last_data_received_ms.store(
                                        wacore::time::now_millis().max(0) as u64,
                                        Ordering::Relaxed,
                                    );
                                }
                            },
                            Ok(crate::transport::TransportEvent::Disconnected(reason)) => {
                                if !self.expected_disconnect.load(Ordering::Relaxed) {
                                    // Classify the level: a routine server recycle (clean EOF /
                                    // normal close) is logged quietly, but a real transport error
                                    // stays at WARN so it's never hidden behind reconnect noise.
                                    if reason.is_clean_shutdown() {
                                        info!("Connection closed by server ({reason}); reconnecting.");
                                    } else {
                                        warn!("Transport disconnected: {reason}; reconnecting.");
                                    }
                                    return Err(anyhow::anyhow!("Transport disconnected: {reason}"));
                                } else {
                                    debug!("Transport disconnected as expected: {reason}");
                                    return Ok(());
                                }
                            }
                            // Event channel closed (no DisconnectReason available) — the
                            // transport task ended without reporting why. No reason means we
                            // can't prove it was a clean recycle, so it stays loud (WARN),
                            // matching the conservative `Unknown` rule in is_clean_shutdown.
                            Err(_) => {
                                if !self.expected_disconnect.load(Ordering::Relaxed) {
                                    warn!("Transport event channel closed; reconnecting.");
                                    return Err(anyhow::anyhow!("Transport event channel closed"));
                                } else {
                                    return Ok(());
                                }
                            }
                            Ok(crate::transport::TransportEvent::Connected) => {
                                // Already handled during handshake, but could be useful for logging
                                debug!("Transport connected event received");
                            }
                    }
                }
            }
        }
    }

    /// Decrypt a frame and return the parsed node as a zero-copy OwnedNodeRef.
    /// This must be called sequentially due to noise protocol counter requirements.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.decrypt_frame", level = "trace", skip_all)
    )]
    pub(crate) async fn decrypt_frame(
        self: &Arc<Self>,
        encrypted_frame: bytes::BytesMut,
    ) -> Option<wacore_binary::OwnedNodeRef> {
        let noise_socket = match self.get_noise_socket().await {
            Ok(s) => s,
            Err(_) => {
                log::error!("Cannot process frame: not connected (no noise socket)");
                return None;
            }
        };

        let decrypted_payload = match noise_socket.decrypt_frame(encrypted_frame) {
            Ok(p) => p,
            Err(e) => {
                log::error!("Failed to decrypt frame: {e}");
                return None;
            }
        };

        let buffer = match wacore_binary::util::unpack_bytes(decrypted_payload) {
            Ok(data) => data,
            Err(e) => {
                log::warn!(target: "Client/Recv", "Failed to decompress frame: {e}");
                return None;
            }
        };

        match wacore_binary::OwnedNodeRef::new(buffer) {
            Ok(owned) => Some(owned),
            Err(e) => {
                log::warn!(target: "Client/Recv", "Failed to unmarshal node: {e}");
                None
            }
        }
    }

    /// Process an already-decrypted node.
    /// This can be spawned concurrently since it doesn't depend on noise protocol state.
    /// The node is wrapped in Arc to avoid cloning when passing through handlers.
    pub(crate) async fn process_decrypted_node(
        self: &Arc<Self>,
        node: wacore_binary::OwnedNodeRef,
    ) {
        // Wrap in Arc once - all handlers will share this same allocation
        let node_arc = Arc::new(node);
        self.process_node(node_arc).await;
    }

    /// Process a node wrapped in Arc. Handlers receive the Arc and can share/store it cheaply.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.node", level = "trace", skip_all, fields(tag = %node.get().tag.as_ref()))
    )]
    pub(crate) async fn process_node(self: &Arc<Self>, node: Arc<wacore_binary::OwnedNodeRef>) {
        use wacore::xml::DisplayableNodeRef;
        let nr = node.get();

        // --- Offline Sync Tracking ---
        if nr.tag.as_ref() == "ib" {
            // Check for offline_preview child to get expected count
            if let Some(preview) = nr.get_optional_child("offline_preview") {
                let count: usize = preview
                    .get_attr("count")
                    .map(|v| v.as_str())
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(0);

                if count == 0 {
                    self.offline_sync_metrics
                        .active
                        .store(false, Ordering::Release);
                    debug!(target: "Client/OfflineSync", "Sync COMPLETED: 0 items.");
                } else {
                    // Use stronger memory ordering for state transitions
                    self.offline_sync_metrics
                        .total_messages
                        .store(count, Ordering::Release);
                    self.offline_sync_metrics
                        .processed_messages
                        .store(0, Ordering::Release);
                    self.offline_sync_metrics
                        .active
                        .store(true, Ordering::Release);
                    match self.offline_sync_metrics.start_time.lock() {
                        Ok(mut guard) => *guard = Some(wacore::time::Instant::now()),
                        Err(poison) => *poison.into_inner() = Some(wacore::time::Instant::now()),
                    }
                    debug!(target: "Client/OfflineSync", "Sync STARTED: Expecting {} items.", count);
                }
            } else if self.offline_sync_metrics.active.load(Ordering::Acquire)
                && nr.get_optional_child("offline").is_some()
            {
                // Handle end marker: <ib><offline count="N"/> signals sync completion
                // Only <ib> with an <offline> child is a real end marker.
                // Other <ib> children (thread_metadata, edge_routing, dirty) are NOT end markers.
                let processed = self
                    .offline_sync_metrics
                    .processed_messages
                    .load(Ordering::Acquire);
                let elapsed = match self.offline_sync_metrics.start_time.lock() {
                    Ok(guard) => guard.map(|t| t.elapsed()).unwrap_or_default(),
                    Err(poison) => poison.into_inner().map(|t| t.elapsed()).unwrap_or_default(),
                };
                debug!(target: "Client/OfflineSync", "Sync COMPLETED: End marker received. Processed {} items in {:.2?}.", processed, elapsed);
                self.offline_sync_metrics
                    .active
                    .store(false, Ordering::Release);
            }
        }

        // Track progress if active
        if self.offline_sync_metrics.active.load(Ordering::Acquire) {
            // Check for 'offline' attribute on relevant stanzas
            if nr.get_attr("offline").is_some() {
                let processed = self
                    .offline_sync_metrics
                    .processed_messages
                    .fetch_add(1, Ordering::Release)
                    + 1;
                let total = self
                    .offline_sync_metrics
                    .total_messages
                    .load(Ordering::Acquire);

                if processed.is_multiple_of(50) || processed == total {
                    trace!(target: "Client/OfflineSync", "Sync Progress: {}/{}", processed, total);
                }

                // Drive WA Web pull-batch loop (non-adaptive `$13`): when
                // remaining drops to <=C and no batch request is in flight,
                // schedule the next one.
                let pending = total.saturating_sub(processed);
                crate::client::offline_resume::on_offline_stanza_arrived(self, pending);

                if processed >= total {
                    let elapsed = match self.offline_sync_metrics.start_time.lock() {
                        Ok(guard) => guard.map(|t| t.elapsed()).unwrap_or_default(),
                        Err(poison) => poison.into_inner().map(|t| t.elapsed()).unwrap_or_default(),
                    };
                    debug!(target: "Client/OfflineSync", "Sync COMPLETED: Processed {} items in {:.2?}.", processed, elapsed);
                    self.offline_sync_metrics
                        .active
                        .store(false, Ordering::Release);
                }
            }
        }
        // --- End Tracking ---

        if nr.tag.as_ref() == "iq"
            && let Some(sync_node) = nr.get_optional_child("sync")
            && let Some(collection_node) = sync_node.get_optional_child("collection")
        {
            let name = collection_node.attrs().optional_string("name");
            let name = name.as_deref().unwrap_or("<unknown>");
            debug!(target: "Client/Recv", "Received app state sync response for '{name}' (hiding content).");
        } else {
            debug!(target: "Client/Recv","{}", DisplayableNodeRef(nr));
        }

        // Prepare deferred ACK cancellation flag (sent after dispatch unless cancelled)
        let mut cancelled = false;

        // Emit raw node before any early returns so all decoded stanzas
        // (including IQ responses and xmlstreamend) reach external observers
        if self.raw_node_forwarding.load(Ordering::Relaxed) {
            self.core
                .event_bus
                .dispatch(Event::RawNode(Arc::clone(&node)));
        }

        if nr.tag.as_ref() == "xmlstreamend" {
            if self.expected_disconnect.load(Ordering::Relaxed) {
                debug!("Received <xmlstreamend/>, expected disconnect.");
            } else {
                // A bare <xmlstreamend/> is the server cleanly ending the stream
                // (a recycle). We reconnect, so this is routine, not an error.
                info!("Received <xmlstreamend/> (server stream end); reconnecting.");
            }
            self.notify_connection_shutdown();
            return;
        }

        // Check generic node waiters (zero-cost when none registered)
        if self.node_waiter_count.load(Ordering::Acquire) > 0 {
            self.resolve_node_waiters(&node);
        }

        if nr.tag.as_ref() == "iq"
            && let Some(id) = nr.get_attr("id").map(|v| v.as_str())
        {
            // Single lock acquisition: try to remove the waiter directly.
            let waiter = self.response_waiters.lock().await.remove(id.as_ref());
            if let Some(waiter) = waiter {
                if waiter.send(Arc::clone(&node)).is_err() {
                    warn!(target: "Client/IQ", "Failed to send IQ response to waiter. Receiver was likely dropped.");
                }
                return;
            }
        }

        // Dispatch to appropriate handler using the router
        // Clone Arc (cheap - just reference count) not the Node itself
        if !self
            .stanza_router
            .dispatch(self.clone(), Arc::clone(&node), &mut cancelled)
            .await
        {
            warn!(
                "Received unknown top-level node: {}",
                DisplayableNodeRef(nr)
            );
        }

        // Send the deferred ACK if applicable and not cancelled by handler
        if self.should_ack(nr) && !cancelled {
            self.maybe_deferred_ack(node).await;
        }
    }

    /// Per WA Web (`Handle/MsgSendReceipt.js`), only newsletter `<message>`
    /// gets `<ack class="message">` on the success path; DM/group use
    /// `<receipt>`. Failure paths (retry/backfill/nack) emit `<ack>` from
    /// their dedicated handlers, not via this gate.
    ///
    /// status@broadcast is included as a fallback: drop paths in
    /// `process_group_enc_batch` (expired status, missing sender key, generic
    /// decrypt error) intentionally skip the delivery receipt to avoid
    /// inflating the server-side offline counter for messages we'll never
    /// process. Without the transport `<ack>` from this gate, the server
    /// would redeliver indefinitely. WA Web emits `<receipt context="status">`
    /// in the success path on top of this; the duplicate is tolerated.
    pub(crate) fn should_ack(&self, node: &wacore_binary::NodeRef<'_>) -> bool {
        let tag = node.tag.as_ref();
        if node.get_attr("id").is_none() {
            return false;
        }
        let Some(from) = node.get_attr("from") else {
            return false;
        };
        match tag {
            "receipt" | "notification" | "call" => true,
            "message" => from
                .to_jid()
                .is_some_and(|j| j.is_newsletter() || j.is_status_broadcast()),
            _ => false,
        }
    }

    /// Possibly send a deferred ack: either immediately or via spawned task.
    /// Handlers can cancel by setting `cancelled` to true.
    /// Uses Arc<OwnedNodeRef> to avoid cloning when spawning the async task.
    async fn maybe_deferred_ack(self: &Arc<Self>, node: Arc<wacore_binary::OwnedNodeRef>) {
        if self.synchronous_ack {
            if let Err(e) = self.send_ack_for(node.get()).await
                && !e.is_transport_unavailable()
            {
                warn!("Failed to send ack: {e:?}");
            }
        } else {
            let this = self.clone();
            self.runtime
                .spawn(Box::pin(async move {
                    if let Err(e) = this.send_ack_for(node.get()).await
                        && !e.is_transport_unavailable()
                    {
                        warn!("Failed to send ack: {e:?}");
                    }
                }))
                .detach();
        }
    }

    /// Build and send an <ack/> node corresponding to the given stanza.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.ack", level = "trace", skip_all, err(Debug))
    )]
    pub(crate) async fn send_ack_for(
        &self,
        node: &wacore_binary::NodeRef<'_>,
    ) -> Result<(), ClientError> {
        if self.expected_disconnect.load(Ordering::Relaxed) {
            return Ok(());
        }
        if !self.is_connected() {
            return Err(ClientError::NotConnected);
        }
        let own_pn = self.get_pn().await;
        let buf = match encode_ack_bytes(node, own_pn.as_ref()) {
            Ok(Some(buf)) => buf,
            Ok(None) => return Ok(()),
            Err(e) => {
                log::warn!("Failed to encode ack: {e}");
                return Ok(());
            }
        };
        self.send_raw_bytes(buf).await
    }

    /// Send a transport ack so the server stops replaying a stanza from the
    /// offline queue. Awaitable so callers can order it after a retry receipt
    /// in a single flushed task.
    pub(crate) async fn send_transport_ack(&self, info: &crate::types::message::MessageInfo) {
        let source = message_ack_source_node(info);
        let own_pn = self.get_pn().await;
        match encode_ack_bytes(&source.as_node_ref(), own_pn.as_ref()) {
            Ok(Some(buf)) => {
                if let Err(e) = self.send_raw_bytes(buf).await
                    && !e.is_transport_unavailable()
                {
                    log::warn!("Failed to send transport ack for undecryptable message: {e:?}");
                }
            }
            Ok(None) => {}
            Err(e) => log::warn!("Failed to encode transport ack: {e}"),
        }
    }

    /// Spawn [`Self::send_transport_ack`], tracked via `outbound_flush` so
    /// `disconnect()` flushes it (issue #571), same as delivery receipts.
    pub(crate) fn spawn_message_ack(
        self: &Arc<Self>,
        info: &Arc<crate::types::message::MessageInfo>,
    ) {
        let client = Arc::clone(self);
        let info = Arc::clone(info);
        self.outbound_flush.spawn(&*self.runtime, async move {
            client.send_transport_ack(&info).await;
        });
    }

    /// Tracked ack encoded from the original node. Use when the stanza carries
    /// `recipient` (LID-routed/hosted-companion/peer) since `MessageInfo`
    /// drops it on non-self branches and the server needs it for routing.
    pub(crate) async fn spawn_node_transport_ack(
        self: &Arc<Self>,
        node: &wacore_binary::NodeRef<'_>,
    ) {
        let own_pn = self.get_pn().await;
        let buf = match encode_ack_bytes(node, own_pn.as_ref()) {
            Ok(Some(b)) => b,
            Ok(None) => return,
            Err(e) => {
                log::warn!("Failed to encode node transport ack: {e}");
                return;
            }
        };
        let client = Arc::clone(self);
        self.outbound_flush.spawn(&*self.runtime, async move {
            if let Err(e) = client.send_raw_bytes(buf).await
                && !e.is_transport_unavailable()
            {
                log::warn!("Failed to send node transport ack: {e:?}");
            }
        });
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.success", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_success(self: &Arc<Self>, node: &wacore_binary::NodeRef<'_>) {
        // Skip processing if an expected disconnect is pending (e.g., 515 received).
        // This prevents race conditions where a spawned success handler runs after
        // cleanup_connection_state has already reset is_logged_in.
        if self.expected_disconnect.load(Ordering::Relaxed) {
            debug!("Ignoring <success> stanza: expected disconnect pending");
            return;
        }

        // Guard against multiple <success> stanzas (WhatsApp may send more than one during
        // routing/reconnection). Only process the first one per connection.
        if self.is_logged_in.swap(true, Ordering::SeqCst) {
            debug!("Ignoring duplicate <success> stanza (already logged in)");
            return;
        }

        // Increment connection generation to invalidate any stale post-login tasks
        // from previous connections (e.g., during 515 reconnect cycles).
        let current_generation = self.connection_generation.fetch_add(1, Ordering::SeqCst) + 1;

        info!(
            "Successfully authenticated with WhatsApp servers! (gen={})",
            current_generation
        );
        self.auto_reconnect_errors.store(0, Ordering::Relaxed);

        self.update_server_time_offset(node);

        // Extract LID from the node before spawning (node isn't Send).
        let lid_from_server = match node.get_attr("lid") {
            Some(lid_value) => match lid_value.to_jid() {
                Some(lid) => Some(lid),
                None => {
                    warn!("Failed to parse LID from success stanza: {lid_value}");
                    None
                }
            },
            None => {
                warn!("LID not found in <success> stanza. Group messaging may fail.");
                None
            }
        };

        let client_clone = self.clone();
        let task_generation = current_generation;
        self.runtime.spawn(Box::pin(async move {
            // Update LID if changed (moved here to avoid blocking the read loop
            // on Device snapshot + write lock).
            if let Some(lid) = lid_from_server {
                let device_snapshot =
                    client_clone.persistence_manager.get_device_snapshot().await;
                if device_snapshot.lid.as_ref() != Some(&lid) {
                    debug!("Updating LID from server to '{}'", lid.observe());
                    client_clone
                        .persistence_manager
                        .process_command(DeviceCommand::SetLid(Some(lid)))
                        .await;
                }
            }

            // WA Web bumps `lc` after each successful auth (Start/Backend.js
            // listener on `onOpenSocketStream`). The Comms `onConnect` handler
            // gates the trigger on `isRegistered()`, so the bump only happens
            // for already-paired logins — never during the pairing XX
            // handshake. We mirror that by skipping when `device.pn` is None.
            let already_paired = client_clone
                .persistence_manager
                .get_device_snapshot()
                .await
                .pn
                .is_some();
            if already_paired {
                client_clone
                    .persistence_manager
                    .process_command(DeviceCommand::IncrementLoginCounter)
                    .await;
            }

            // Macro to check if this task is still valid (connection hasn't been replaced)
            macro_rules! check_generation {
                () => {
                    if client_clone.connection_generation.load(Ordering::SeqCst) != task_generation
                    {
                        debug!("Post-login task cancelled: connection generation changed");
                        return;
                    }
                };
            }

            debug!(
                "Starting post-login initialization sequence (gen={})...",
                task_generation
            );

            // Check if we need initial app state sync (empty pushname indicates fresh pairing
            // where pushname will come from app state sync's setting_pushName mutation)
            let device_snapshot = client_clone.persistence_manager.get_device_snapshot().await;
            let needs_pushname_from_sync = device_snapshot.push_name.is_empty();
            if needs_pushname_from_sync {
                debug!("Push name is empty - will be set from app state sync (setting_pushName)");
            }

            // Check connection before network operations.
            // During pairing, a 515 disconnect happens quickly after success,
            // so the socket may already be gone.
            if !client_clone.is_connected() {
                debug!(
                    "Skipping post-login init: connection closed (likely pairing phase reconnect)"
                );
                return;
            }

            check_generation!();
            client_clone.send_unified_session().await;

            // === Establish session with primary phone for PDO ===
            // This must happen BEFORE we exit passive mode (before offline messages arrive).
            // PDO needs a session with device 0 to request decrypted content from our phone.
            // Matches WhatsApp Web's bootstrapDeviceCapabilities() pattern.
            check_generation!();
            if let Err(e) = client_clone
                .establish_primary_phone_session_immediate()
                .await
            {
                warn!(target: "Client/PDO", "Failed to establish session with primary phone on login: {:?}", e);
                // Don't fail login - PDO will retry via ensure_e2e_sessions fallback
            }

            // Sync own device list so DM fan-out includes all companions
            check_generation!();
            if let Err(e) = client_clone.sync_own_device_list().await {
                client_clone.log_sync_error("sync own device list", &e);
            }

            check_generation!();
            if !client_clone.is_connected() {
                debug!("Skipping passive tasks: connection closed");
                return;
            }
            if let Err(e) = client_clone.upload_pre_keys_at_login().await
                && !client_clone.is_shutting_down()
            {
                warn!("Failed to upload pre-keys during startup: {e:?}");
            }

            // === Send active IQ ===
            // The server sends <ib><offline count="X"/></ib> AFTER we exit passive mode.
            // This matches WhatsApp Web's behavior: executePassiveTasks() -> sendPassiveModeProtocol("active")
            check_generation!();
            if !client_clone.is_connected() {
                debug!("Skipping active IQ: connection closed");
                return;
            }
            if let Err(e) = client_clone.set_passive(false).await
                && !client_clone.is_shutting_down()
            {
                warn!("Failed to send post-connect active IQ: {e:?}");
            }

            // === Wait for offline sync to complete ===
            // The server sends <ib><offline count="X"/></ib> after we exit passive mode.
            client_clone.wait_for_offline_delivery_end().await;

            // Check if connection was replaced while waiting
            check_generation!();

            // Re-check connection and generation before sending presence
            check_generation!();
            if !client_clone.is_connected() {
                debug!("Skipping presence: connection closed");
                return;
            }

            // Background initialization queries (can run in parallel, non-blocking)
            let bg_client = client_clone.clone();
            let bg_generation = task_generation;
            client_clone.runtime.spawn(Box::pin(async move {
                // Check connection and generation before starting background queries
                if bg_client.connection_generation.load(Ordering::SeqCst) != bg_generation {
                    debug!("Skipping background init queries: connection generation changed");
                    return;
                }
                if !bg_client.is_connected() {
                    debug!("Skipping background init queries: connection closed");
                    return;
                }

                debug!(
                    "Sending background initialization queries (Props, Blocklist, Privacy, Digest)..."
                );

                let props_fut = bg_client.fetch_props();
                let binding = bg_client.blocking();
                let blocklist_fut = binding.get_blocklist();
                let privacy_fut = bg_client.fetch_privacy_settings();
                let digest_fut = bg_client.validate_digest_key();

                let (r_props, r_block, r_priv, r_digest) =
                    futures::join!(props_fut, blocklist_fut, privacy_fut, digest_fut);

                // Suppress warnings if connection closed while queries were in-flight
                if !bg_client.is_shutting_down() {
                    if let Err(e) = r_props {
                        warn!("Background init: Failed to fetch props: {e:?}");
                    }
                    if let Err(e) = r_block {
                        warn!("Background init: Failed to fetch blocklist: {e:?}");
                    }
                    if let Err(e) = r_priv {
                        warn!("Background init: Failed to fetch privacy settings: {e:?}");
                    }
                    if let Err(e) = r_digest {
                        warn!("Background init: Failed to validate digest key: {e:?}");
                    }
                }

                // Prune expired tcTokens on connect (matches WhatsApp Web's PrivacyTokenJob)
                if let Err(e) = bg_client.tc_token().prune_expired().await
                    && !bg_client.is_shutting_down()
                {
                    warn!("Background init: Failed to prune expired tc_tokens: {e:?}");
                }
            })).detach();

            check_generation!();

            let flag_set = client_clone.needs_initial_full_sync.load(Ordering::Relaxed);
            let needs_initial_sync = flag_set || needs_pushname_from_sync;

            if needs_initial_sync {
                // === Fresh pairing path ===
                // Like WhatsApp Web's syncCriticalData(): await critical collections before
                // dispatching Connected, so blocklist/privacy settings are applied first.
                debug!(
                    target: "Client/AppState",
                    "Starting Initial App State Sync (flag_set={flag_set}, needs_pushname={needs_pushname_from_sync})"
                );

                if !client_clone
                    .initial_app_state_keys_received
                    .load(Ordering::Relaxed)
                {
                    debug!(
                        target: "Client/AppState",
                        "Waiting up to 5s for app state keys..."
                    );
                    let _ = rt_timeout(
                        &*client_clone.runtime,
                        Duration::from_secs(5),
                        client_clone.initial_keys_synced_notifier.listen(),
                    )
                    .await;

                    // Check if connection was replaced while waiting
                    check_generation!();
                }

                // Start the critical sync timeout timer matching WhatsApp Web's
                // WAWebSyncBootstrap.$15 (setSyncDCriticalDataSyncTimeout).
                // WhatsApp Web uses 180s and calls socketLogout(SyncdTimeout) if
                // the critical data hasn't synced by then.
                const CRITICAL_SYNC_TIMEOUT_SECS: u64 = 180;
                let timeout_client = client_clone.clone();
                let timeout_generation = task_generation;
                let timeout_rt = client_clone.runtime.clone();
                let critical_sync_timeout_handle = timeout_rt.spawn(Box::pin(async move {
                    timeout_client.runtime.sleep(Duration::from_secs(CRITICAL_SYNC_TIMEOUT_SECS)).await;
                    // Check generation — if connection was replaced, this timeout is stale
                    if timeout_client.connection_generation.load(Ordering::SeqCst)
                        != timeout_generation
                    {
                        return;
                    }
                    // Matches WhatsApp Web's $16(): check if SettingPushName was synced.
                    // If push_name is still empty after 180s, critical sync failed.
                    let push_name = timeout_client.get_push_name().await;
                    if push_name.is_empty() {
                        warn!(
                            target: "Client/AppState",
                            "Critical app state sync timed out after {CRITICAL_SYNC_TIMEOUT_SECS}s \
                             (push_name not synced). Reconnecting to retry."
                        );
                        // WhatsApp Web does socketLogout here which clears device identity.
                        // We reconnect instead — preserving credentials and keeping the
                        // run loop active so auto-reconnect can retry the sync.
                        timeout_client.reconnect_immediately().await;
                    } else {
                        debug!(
                            target: "Client/AppState",
                            "Critical sync timeout fired but push_name was already synced"
                        );
                    }
                }));

                // Await critical collections via batched IQ before dispatching Connected.
                check_generation!();
                match client_clone
                    .sync_collections_batched(vec![
                        WAPatchName::CriticalBlock,
                        WAPatchName::CriticalUnblockLow,
                    ])
                    .await
                {
                    Ok(()) => {
                        // Critical sync completed — cancel the timeout timer
                        critical_sync_timeout_handle.abort();

                        check_generation!();

                        client_clone
                            .resubscribe_presence_subscriptions(task_generation)
                            .await;

                        check_generation!();

                        // Dispatch Connected after critical sync completes.
                        // Presence is NOT sent here — WhatsApp Web sends presence from the
                        // setting_pushName mutation handler (WAWebPushNameSync), not from
                        // criticalSyncDone. Our setting_pushName handler already does this.
                        client_clone.dispatch_connected();
                    }
                    Err(e) => {
                        client_clone.log_sync_error("critical app state sync", &e);
                        // Don't abort the timeout or dispatch Connected — the sync failed,
                        // so the timeout watchdog should remain active to force a reconnect
                        // if needed. Return early to avoid emitting a spurious Connected event.
                        return;
                    }
                }

                // Spawn remaining non-critical collections in background
                let sync_client = client_clone.clone();
                let sync_generation = task_generation;
                client_clone.runtime.spawn(Box::pin(async move {
                    if sync_client.connection_generation.load(Ordering::SeqCst) != sync_generation {
                        debug!("App state sync cancelled: connection generation changed");
                        return;
                    }

                    if let Err(e) = sync_client
                        .sync_collections_batched(vec![
                            WAPatchName::RegularLow,
                            WAPatchName::RegularHigh,
                            WAPatchName::Regular,
                        ])
                        .await
                    {
                        sync_client.log_sync_error("non-critical app state sync", &e);
                    }

                    sync_client
                        .needs_initial_full_sync
                        .store(false, Ordering::Relaxed);
                    debug!(target: "Client/AppState", "Initial App State Sync Completed.");
                })).detach();
            } else {
                // === Reconnection path ===
                // Pushname is already known, send presence and Connected immediately.
                let device_snapshot = client_clone.persistence_manager.get_device_snapshot().await;
                if !device_snapshot.push_name.is_empty() {
                    if let Err(e) = client_clone.presence().set_available().await {
                        warn!("Failed to send initial presence: {e:?}");
                    } else {
                        debug!("Initial presence sent successfully.");
                    }
                }

                client_clone
                    .resubscribe_presence_subscriptions(task_generation)
                    .await;

                // Re-check generation after awaits to avoid dispatching Connected
                // for an outdated connection that was replaced mid-await.
                check_generation!();

                client_clone.dispatch_connected();
            }
        })).detach();
    }

    /// Handles incoming `<ack/>` stanzas by resolving pending response waiters.
    ///
    /// If an ack with an ID that matches a pending task in `response_waiters`,
    /// the task is resolved and the function returns `true`. Otherwise, returns `false`.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.ack_response", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_ack_response(&self, node: &wacore_binary::NodeRef<'_>) -> bool {
        // Surface server nack codes for diagnosability. A nacked send still
        // resolves Ok to the caller, so without this the failure is invisible.
        if let Some(error_code) = node.get_attr("error") {
            let code = error_code.as_str();
            let id = node.get_attr("id").map(|v| v.as_str().into_owned());
            match code.as_ref() {
                "463" => {
                    warn!(
                        target: "Client/Ack",
                        "Received 463 (MissingTcToken) nack for msg {:?}. \
                         The recipient requires a valid tctoken or cstoken. \
                         This may indicate a reachout timelock on the account.",
                        id
                    );
                }
                "479" => {
                    warn!(
                        target: "Client/Ack",
                        "Received 479 (SmaxInvalid) nack for msg {:?}. \
                         A stanza field has an incorrect format (e.g. wrong JID format or content type).",
                        id
                    );
                }
                other => {
                    warn!(
                        target: "Client/Ack",
                        "Received {other} nack for msg {:?}; the message was likely \
                         not delivered (e.g. 400 = malformed stanza, 404 = recipient \
                         not found, 503 = service unavailable).",
                        id
                    );
                }
            }
        }

        let id_opt = node.get_attr("id").map(|v| v.as_str().into_owned());
        if let Some(id) = id_opt
            && let Some(waiter) = self.response_waiters.lock().await.remove(&id)
        {
            // ACK responses are infrequent; re-encode into OwnedNodeRef for the channel.
            // marshal_ref prepends a leading 0x00 format byte; OwnedNodeRef::new expects raw
            // protocol bytes without it, matching what unpack() produces from the network.
            match wacore_binary::marshal::marshal_ref(node)
                .and_then(|bytes| wacore_binary::OwnedNodeRef::new(bytes[1..].to_vec()))
            {
                Ok(onr) => {
                    if waiter.send(Arc::new(onr)).is_err() {
                        warn!(target: "Client/Ack", "Failed to send ACK response to waiter for ID {id}. Receiver was likely dropped.");
                    }
                }
                Err(e) => {
                    warn!(target: "Client/Ack", "Failed to re-encode ACK node for waiter: {e}");
                }
            }
            return true;
        }
        false
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.stream_error", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_stream_error(&self, node: &wacore_binary::NodeRef<'_>) {
        wacore::telemetry::stream_error();
        // is_logged_in handling: opt-in branches (515/516/401/409/conflict) clear it
        // in the disconnect block below; 429/503 clear it inline because the server
        // explicitly rejected the session and outgoing sends should bail fast; the
        // unknown/code-less catch-all keeps it true so is_fully_ready()-gated work
        // (notably prekey uploads) survives ack-shaped routing errors.
        let mut attrs = node.attrs();
        let code_cow = attrs.optional_string("code");
        let code = code_cow.as_deref().unwrap_or("");
        let conflict_type = node
            .get_optional_child("conflict")
            .map(|n| {
                n.attrs()
                    .optional_string("type")
                    .as_deref()
                    .unwrap_or("")
                    .to_string()
            })
            .unwrap_or_default();

        // Whether to proactively disconnect the transport after handling.
        let mut should_disconnect = false;

        if !conflict_type.is_empty() {
            info!(
                "Got stream error indicating client was removed or replaced (conflict={}). Logging out.",
                conflict_type
            );
            self.expected_disconnect.store(true, Ordering::Relaxed);
            self.enable_auto_reconnect.store(false, Ordering::Relaxed);

            let event = if conflict_type == "replaced" {
                Event::StreamReplaced(crate::types::events::StreamReplaced)
            } else {
                Event::LoggedOut(crate::types::events::LoggedOut {
                    on_connect: false,
                    reason: ConnectFailureReason::LoggedOut,
                })
            };
            self.core.event_bus.dispatch(event);
            should_disconnect = true;
        } else {
            match code {
                "515" => {
                    info!(
                        "Got 515 stream error, server is closing stream (expected after pairing). Will auto-reconnect."
                    );
                    self.expected_disconnect.store(true, Ordering::Relaxed);
                    should_disconnect = true;
                }
                "516" => {
                    info!("Got 516 stream error (device removed). Logging out.");
                    self.expected_disconnect.store(true, Ordering::Relaxed);
                    self.enable_auto_reconnect.store(false, Ordering::Relaxed);
                    self.core.event_bus.dispatch(Event::LoggedOut(
                        crate::types::events::LoggedOut {
                            on_connect: false,
                            reason: ConnectFailureReason::LoggedOut,
                        },
                    ));
                    should_disconnect = true;
                }
                "401" => {
                    info!("Got 401 stream error (unauthorized). Logging out.");
                    self.expected_disconnect.store(true, Ordering::Relaxed);
                    self.enable_auto_reconnect.store(false, Ordering::Relaxed);
                    self.core.event_bus.dispatch(Event::LoggedOut(
                        crate::types::events::LoggedOut {
                            on_connect: false,
                            reason: ConnectFailureReason::LoggedOut,
                        },
                    ));
                    should_disconnect = true;
                }
                "409" => {
                    info!("Got 409 stream error (conflict). Another session replaced this one.");
                    self.expected_disconnect.store(true, Ordering::Relaxed);
                    self.enable_auto_reconnect.store(false, Ordering::Relaxed);
                    self.core
                        .event_bus
                        .dispatch(Event::StreamReplaced(crate::types::events::StreamReplaced));
                    should_disconnect = true;
                }
                "429" => {
                    // Server signalled rate-limit on this session: mark logged-out so
                    // outgoing sends bail fast instead of being interpreted as abuse
                    // while we wait for the (likely-imminent) reconnect.
                    warn!(
                        "Got 429 stream error (rate limited). Will auto-reconnect with extended backoff."
                    );
                    self.is_logged_in.store(false, Ordering::Relaxed);
                    self.auto_reconnect_errors.fetch_add(5, Ordering::Relaxed);
                }
                "503" => {
                    // Server is going down/restarting: mark logged-out so sends fail
                    // fast against the soon-to-die socket. Auto-reconnect handles recovery.
                    info!("Got 503 service unavailable, will auto-reconnect.");
                    self.is_logged_in.store(false, Ordering::Relaxed);
                }
                _ => {
                    // Server wraps per-stanza routing failures in <stream:error> without a
                    // code (e.g. <ack/>): treat as informational so we don't trigger reconnect
                    // storms. is_logged_in stays true on purpose — whatsmeow clears it eagerly,
                    // but here is_fully_ready() gates prekey uploads and we want them to keep
                    // working while the socket is still alive. Severity is warn!, not error!,
                    // because the connection is intentionally preserved.
                    // WA Web (StreamError.js) knows <stream:error><ack/> (type "ack");
                    // name it instead of "Unknown". Root cause is usually an un-acked
                    // offline stanza; the server's <xmlstreamend/> drives the reconnect.
                    if let Some(ack) = node.get_optional_child("ack") {
                        let id = ack
                            .get_attr("id")
                            .map(|v| v.as_str().to_string())
                            .unwrap_or_default();
                        let class = ack
                            .get_attr("class")
                            .map(|v| v.as_str().to_string())
                            .unwrap_or_default();
                        warn!(
                            "Stream error carrying <ack> (class={class:?}, id={id}): server-driven stream rotation, not an ack rejection; reconnect follows on stream end"
                        );
                    } else {
                        warn!("Unknown stream error: {}", DisplayableNodeRef(node));
                    }
                    self.core.event_bus.dispatch(Event::StreamError(
                        crate::types::events::StreamError {
                            code: code.to_string(),
                            raw: Some(node.to_owned()),
                        },
                    ));
                }
            }
        }

        // Single is_logged_in clear + transport disconnect for every opt-in branch
        // (515/516/401/409 and conflict). 429/503/unknown fall through so the
        // socket layer notices a real teardown without us forcing one.
        if should_disconnect {
            self.is_logged_in.store(false, Ordering::Relaxed);
            let transport_opt = self.transport.lock().await.clone();
            if let Some(transport) = transport_opt {
                self.runtime
                    .spawn(Box::pin(async move {
                        transport.disconnect().await;
                    }))
                    .detach();
            }
            info!("Notifying connection shutdown from stream error handler");
            self.notify_connection_shutdown();
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.connect_failure", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_connect_failure(&self, node: &wacore_binary::NodeRef<'_>) {
        self.expected_disconnect.store(true, Ordering::Relaxed);
        self.notify_connection_shutdown();

        let mut attrs = node.attrs();
        let reason_code = attrs.optional_u64("reason").unwrap_or(0) as i32;
        let reason = ConnectFailureReason::from(reason_code);

        if reason.should_reconnect() {
            self.expected_disconnect.store(false, Ordering::Relaxed);
        } else {
            self.enable_auto_reconnect.store(false, Ordering::Relaxed);
        }

        if reason.is_logged_out() {
            // Log the full <failure> so a server-side lock/ban is diagnosable;
            // `location` (e.g. "rva") is a routing token, not the cause.
            warn!(
                "Got {reason:?} connect failure, logging out: {}",
                DisplayableNodeRef(node)
            );
            self.core
                .event_bus
                .dispatch(wacore::types::events::Event::LoggedOut(
                    crate::types::events::LoggedOut {
                        on_connect: true,
                        reason,
                    },
                ));
        } else if let ConnectFailureReason::TempBanned = reason {
            let ban_code = attrs.optional_u64("code").unwrap_or(0) as i32;
            let expire_secs = attrs.optional_u64("expire").unwrap_or(0);
            let expire_duration =
                chrono::Duration::try_seconds(expire_secs as i64).unwrap_or_default();
            warn!(
                "Temporary ban connect failure: {}",
                DisplayableNodeRef(node)
            );
            self.core
                .event_bus
                .dispatch(Event::TemporaryBan(crate::types::events::TemporaryBan {
                    code: crate::types::events::TempBanReason::from(ban_code),
                    expire: expire_duration,
                }));
        } else if let ConnectFailureReason::ClientOutdated = reason {
            error!("Client is outdated and was rejected by server.");
            self.core
                .event_bus
                .dispatch(Event::ClientOutdated(crate::types::events::ClientOutdated));
        } else {
            warn!("Unknown connect failure: {}", DisplayableNodeRef(node));
            self.core.event_bus.dispatch(Event::ConnectFailure(
                crate::types::events::ConnectFailure {
                    reason,
                    message: attrs
                        .optional_string("message")
                        .as_deref()
                        .unwrap_or("")
                        .to_string(),
                    raw: Some(node.to_owned()),
                },
            ));
        }
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.conn.iq_in", level = "debug", skip_all)
    )]
    pub(crate) async fn handle_iq(self: &Arc<Self>, node: &wacore_binary::NodeRef<'_>) -> bool {
        // Pong a server-initiated ping (a request: type="get" or, like WA Web's
        // type-agnostic handleIq, an absent type), but not a type="result"/"error"
        // ping — that's a response to our own ping, and ponging it back is wrong.
        // The previous gate required type=="get" exactly, dropping an absent-type
        // ping and risking a keepalive timeout/disconnect.
        let is_ping_request = node.get_attr("type").is_none_or(|s| s.as_str() == "get")
            && (node.get_optional_child("ping").is_some()
                || node
                    .get_attr("xmlns")
                    .is_some_and(|s| s.as_str() == "urn:xmpp:ping"));
        if is_ping_request {
            debug!("Received ping, sending pong.");
            let mut parser = node.attrs();
            let from_jid = parser.jid("from");
            let id = parser.optional_string("id").map(|s| s.to_string());
            let pong = build_pong(from_jid.to_string(), id.as_deref());
            if let Err(e) = self.send_node(pong).await {
                warn!("Failed to send pong: {e:?}");
            }
            return true;
        }

        if pair::handle_iq(self, node).await {
            return true;
        }

        false
    }

    pub(crate) fn update_server_time_offset(&self, node: &wacore_binary::NodeRef<'_>) {
        self.unified_session.update_server_time_offset(node);
    }
}
