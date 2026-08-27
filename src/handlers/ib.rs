use super::traits::StanzaHandler;
use crate::client::Client;
use crate::types::events::{
    ClientExpirationChanged, DirtyState, Event, EventKind, OfflineSyncPreview,
};
use async_trait::async_trait;
use futures::FutureExt;
use log::{debug, info, warn};
use std::sync::Arc;
use wacore::appstate::patch_decode::WAPatchName;
use wacore::iq::dirty::{DirtyBit, DirtyType};
use wacore::stanza::ib::InfoBulletinType;
use wacore::stanza::wire_tags::StanzaTag;
use wacore::store::commands::DeviceCommand;
use wacore::store::device::{ClientExpirationUpdate, ServerClientExpiration};

/// Handler for `<ib>` (information broadcast) stanzas.
///
/// Processes various server notifications including:
/// - Dirty state notifications
/// - Edge routing information
/// - Offline sync previews and completion notifications
/// - Thread metadata
#[derive(Default)]
pub struct IbHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for IbHandler {
    fn tag(&self) -> &'static str {
        StanzaTag::InfoBanner.as_str()
    }

    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        handle_ib_impl(client, node.get()).await;
        true
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.recv.ib", level = "debug", skip_all)
)]
async fn handle_ib_impl(client: Arc<Client>, node: &wacore_binary::NodeRef<'_>) {
    for child in node.children().unwrap_or_default() {
        match InfoBulletinType::try_from(child.tag.as_ref()).unwrap_or(InfoBulletinType::Unknown) {
            InfoBulletinType::Dirty => {
                let mut attrs = child.attrs();
                let dirty_type_str = match attrs.optional_string("type") {
                    Some(t) => t.to_string(),
                    None => {
                        warn!("Dirty notification missing 'type' attribute");
                        continue;
                    }
                };
                let timestamp_str = attrs.optional_string("timestamp");

                let bit = match DirtyBit::from_raw(&dirty_type_str, timestamp_str.as_deref()) {
                    Ok(b) => b,
                    Err(e) => {
                        warn!("Invalid dirty notification: {e}");
                        continue;
                    }
                };

                let needs_offline_wait = matches!(
                    bit.dirty_type,
                    DirtyType::Groups | DirtyType::NewsletterMetadata
                );
                let needs_resync = bit.dirty_type == DirtyType::SyncdAppState;

                if client.core.event_bus.has_handler_for(EventKind::DirtyState) {
                    client.core.event_bus.dispatch(Event::DirtyState(
                        DirtyState::builder()
                            .dirty_type(bit.dirty_type.clone())
                            .maybe_timestamp(bit.timestamp)
                            .build(),
                    ));
                }

                debug!(
                    "Received dirty state notification for type: '{dirty_type_str}'. Sending clean IQ."
                );

                let client_clone = client.clone();
                // Opened before the wait below, so everything this task does is
                // attributed to the connection that asked for the re-sync rather
                // than to whichever one is live when it finishes.
                let scope = client.sync_scope(None);

                // Groups/newsletter_metadata: wait for offline sync per WAWebHandleDirtyBits.
                client
                    .runtime
                    .spawn(Box::pin(async move {
                        if needs_offline_wait {
                            client_clone.wait_for_offline_delivery_end().await;
                        }
                        if client_clone.is_shutting_down() {
                            return;
                        }
                        // The wait above can outlast the connection that asked for
                        // this, and the report is keyed to the scope opened
                        // before it — so a task that ran on the replacement
                        // socket would do the work only to have it thrown away,
                        // taking the retry with it. Stop before doing any.
                        if let Err(lost) = client_clone.admits(scope) {
                            debug!(target: "Client/AppState", "Dirty-bit task cancelled after the offline wait: {lost:?}");
                            return;
                        }
                        if let Err(e) = client_clone.clean_dirty_bits(bit).await
                            && !client_clone.is_shutting_down()
                        {
                            warn!("Failed to send clean dirty bits IQ: {e:?}");
                        }

                        // Rebound, not dropped. The server has already accepted
                        // the dirty bit as clean, so it will not raise it again,
                        // and an ordinary reconnect with a known push name runs
                        // no app-state bootstrap — returning here would leave
                        // these collections stale until something unrelated
                        // asked. The work carries over to the live connection;
                        // only the retired generation's outcome would not.
                        let mut scope = scope;
                        if scope.rebind(
                            client_clone
                                .connection_generation
                                .load(std::sync::atomic::Ordering::SeqCst),
                        ) {
                            debug!(
                                target: "Client/AppState",
                                "Dirty-bit resync rebinding after the clean IQ"
                            );
                        }

                        if needs_resync && !client_clone.is_shutting_down() {
                            info!("syncd_app_state dirty -- re-syncing all app state collections");
                            let requested = vec![
                                WAPatchName::CriticalBlock,
                                WAPatchName::CriticalUnblockLow,
                                WAPatchName::RegularLow,
                                WAPatchName::RegularHigh,
                                WAPatchName::Regular,
                            ];
                            let result = client_clone
                                .sync_collections_batched(requested.clone(), scope)
                                .await;
                            // Rebound again after the batch. The scope was
                            // pinned before it, so a reconnect during the sync
                            // would have `report_background_sync` discard the
                            // outcome — and with it the retry — for collections
                            // whose dirty bit the server already considers
                            // clean. Reporting against the live connection keeps
                            // the retry, and the outcome describes the
                            // collections either way.
                            let mut scope = scope;
                            scope.rebind(
                                client_clone
                                    .connection_generation
                                    .load(std::sync::atomic::Ordering::SeqCst),
                            );
                            // Reported unless the client is going away for
                            // good. A planned reconnect used to drop this, which
                            // took the retry with it — and the server already
                            // considers the dirty bit clean, so nothing would
                            // ask again. The scheduler rebinds once the
                            // replacement is live.
                            if !client_clone.is_terminal() {
                                client_clone.report_background_sync(
                                    "app state re-sync after dirty notification",
                                    scope,
                                    crate::client::SyncSettles::JustTheCollections,
                                    &requested,
                                    result,
                                );
                            }
                        }
                    }))
                    .detach();
            }
            InfoBulletinType::EdgeRouting => {
                // Edge routing info is used for optimized reconnection to WhatsApp servers.
                // When present, it should be sent as a pre-intro before the Noise handshake.
                // Format on wire: ED (2 bytes) + length (3 bytes BE) + routing_data + WA header
                if let Some(routing_info_node) = child.get_optional_child("routing_info")
                    && let Some(routing_bytes) = routing_info_node.content_bytes()
                    && !routing_bytes.is_empty()
                {
                    debug!(
                        "Received edge routing info ({} bytes), storing for reconnection",
                        routing_bytes.len()
                    );
                    let routing_bytes = routing_bytes.to_vec();
                    let client_clone = client.clone();
                    client
                        .runtime
                        .spawn(Box::pin(async move {
                            client_clone
                                .persistence_manager
                                .modify_device(|device| {
                                    device.edge_routing_info = Some(routing_bytes);
                                })
                                .await;
                        }))
                        .detach();
                }
            }
            InfoBulletinType::OfflinePreview => {
                let mut attrs = child.attrs();
                let total = attrs.optional_u64("count").unwrap_or(0) as i32;
                let app_data_changes = attrs.optional_u64("appdata").unwrap_or(0) as i32;
                let messages = attrs.optional_u64("message").unwrap_or(0) as i32;
                let notifications = attrs.optional_u64("notification").unwrap_or(0) as i32;
                let receipts = attrs.optional_u64("receipt").unwrap_or(0) as i32;
                let calls = attrs.optional_u64("call").unwrap_or(0) as i32;
                let statuses = attrs.optional_u64("status").unwrap_or(0) as i32;

                debug!(
                    target: "Client/OfflineSync",
                    "Offline preview: {} total ({} messages, {} statuses, {} notifications, {} receipts, {} calls, {} app data changes)",
                    total, messages, statuses, notifications, receipts, calls, app_data_changes,
                );

                client.core.event_bus.dispatch(Event::OfflineSyncPreview(
                    OfflineSyncPreview::builder()
                        .total(total)
                        .app_data_changes(app_data_changes)
                        .messages(messages)
                        .notifications(notifications)
                        .receipts(receipts)
                        .calls(calls)
                        .statuses(statuses)
                        .build(),
                ));

                // Drive pull-based delivery: without this the server stops
                // after the ~5-stanza primer and the rest of the backlog is
                // never delivered (`WAWebOfflineHandler`).
                if total > 0 {
                    let client_clone = Arc::clone(&client);
                    let total_usize = total as usize;
                    client
                        .runtime
                        .spawn(Box::pin(async move {
                            crate::client::offline_resume::send_first_batch(
                                client_clone,
                                total_usize,
                            )
                            .await;
                        }))
                        .detach();
                }
            }
            InfoBulletinType::Offline => {
                let mut attrs = child.attrs();
                let count = attrs.optional_u64("count").unwrap_or(0) as i32;

                debug!(target: "Client/OfflineSync", "Offline sync completed, received {} items", count);
                client.complete_offline_sync(count).await;

                let client_clone = Arc::clone(&client);
                // Per-connection: the offline flush is tied to THIS connection.
                // A reconnect fires the per-connection signal; the old task exits
                // and the new connection spawns a fresh flush.
                let shutdown = client_clone.connection_shutdown_signal();
                client
                    .runtime
                    .spawn(Box::pin(async move {
                        // WA Web: OFFLINE_DEVICE_SYNC_DELAY = 2000ms
                        futures::select! {
                            _ = client_clone.runtime.sleep(std::time::Duration::from_secs(2)).fuse() => {
                                client_clone.flush_pending_device_sync().await;
                            }
                            _ = wacore::runtime::wait_for_shutdown(&shutdown).fuse() => {}
                        }
                    }))
                    .detach();
            }
            InfoBulletinType::ClientExpiration => {
                handle_client_expiration(&client, child).await;
            }
            // WA Web parses this into its info-bulletin union and then has no
            // `case` for it in the dispatch switch, so it is a marker the
            // server sends and the client is expected to do nothing with.
            // Named here so it does not read as a gap.
            InfoBulletinType::PriorityOfflineComplete => {}
            InfoBulletinType::ThreadMetadata => {
                // Present in some sessions; safe to ignore for now until feature implemented.
                debug!("Received thread metadata, ignoring for now.");
            }
            InfoBulletinType::Tos | InfoBulletinType::RecoveryNonce | InfoBulletinType::Unknown => {
                warn!("Unhandled ib child: <{}>", child.tag);
            }
        }
    }
}

/// `<ib><client_expiration t=...>`: the server naming the date it expects to
/// stop accepting this build.
///
/// Recorded rather than acted on. The deadline is notice about the *build*, not
/// about this connection: the server keeps serving until the date arrives, and
/// what to do before then -- ship a newer version, alert an operator -- is the
/// consumer's call, not something this client can decide by disconnecting.
///
/// Stamped with the running version, because a deadline issued against one
/// build says nothing about the next.
async fn handle_client_expiration(client: &Arc<Client>, child: &wacore_binary::NodeRef<'_>) {
    // WA Web parses this as `attrIntRange(node, "t", 0, undefined)`: present
    // and non-negative, or a parse failure -- never silently absent. The
    // distinction matters here because an absent `t` is a withdrawal, so a
    // value we cannot read must not be mistaken for one and clear a deadline
    // the server never retracted. Read presence first, then the value.
    let raw = child.attrs().optional_string("t");
    let t = match raw.as_deref() {
        None => None,
        // `parse::<i64>` rejects a non-numeric value and one past `i64::MAX` in
        // one step; the sign check is WA Web's `min = 0`.
        Some(raw) => match raw.parse::<i64>() {
            Ok(t) if t >= 0 => Some(t),
            _ => {
                warn!(target: "Client/Ib", "client_expiration carried an unusable t={raw:?}; ignoring");
                return;
            }
        },
    };
    let snapshot = client.persistence_manager.get_device_snapshot();
    let version = (
        snapshot.app_version_primary,
        snapshot.app_version_secondary,
        snapshot.app_version_tertiary,
    );
    let decision = ServerClientExpiration::decide(
        snapshot.server_client_expiration.as_ref(),
        t,
        wacore::time::now_secs() as i64,
        version,
    );

    let (command, event) = match decision {
        ClientExpirationUpdate::Unchanged => {
            debug!(
                target: "Client/Ib",
                "client_expiration t={t:?} is not sooner than the deadline held; ignoring"
            );
            return;
        }
        ClientExpirationUpdate::Clear => {
            // Nothing held means nothing to withdraw, and an event announcing a
            // change that did not happen would be worse than silence.
            if snapshot.server_client_expiration.is_none() {
                return;
            }
            // A record left from an earlier build is dropped -- it describes a
            // build that is no longer running -- but silently: this build never
            // had a deadline, so it has nothing withdrawn from it.
            let applied = ServerClientExpiration::held_for(
                snapshot.server_client_expiration.as_ref(),
                version,
            )
            .is_some();
            if !applied {
                client
                    .persistence_manager
                    .process_command(DeviceCommand::SetServerClientExpiration(None))
                    .await;
                return;
            }
            info!(target: "Client/Ib", "server withdrew the client expiration deadline");
            (
                None,
                ClientExpirationChanged::builder()
                    .version(version)
                    .withdrawn(true)
                    .build(),
            )
        }
        ClientExpirationUpdate::Set(expiration) => {
            info!(
                target: "Client/Ib",
                "server set the client expiration deadline to {} for build {:?}",
                expiration.expires_at, expiration.version
            );
            let event = ClientExpirationChanged::builder()
                .expires_at(expiration.expires_at)
                .version(expiration.version)
                .withdrawn(false)
                .build();
            (Some(expiration), event)
        }
    };

    client
        .persistence_manager
        .process_command(DeviceCommand::SetServerClientExpiration(command))
        .await;
    client
        .core
        .event_bus
        .dispatch(Event::ClientExpirationChanged(event));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{TestEventCollector, create_test_client};
    use wacore_binary::builder::NodeBuilder;

    fn ib_with(child: wacore_binary::Node) -> wacore_binary::Node {
        NodeBuilder::new("ib").children([child]).build()
    }

    fn client_expiration(t: Option<i64>) -> wacore_binary::Node {
        let mut b = NodeBuilder::new("client_expiration");
        if let Some(t) = t {
            b = b.attr("t", t.to_string());
        }
        ib_with(b.build())
    }

    /// The stored deadline, read by borrowing from the cached snapshot rather
    /// than cloning the record out of it.
    fn stored_deadline(client: &Arc<Client>) -> Option<i64> {
        client
            .persistence_manager
            .get_device_snapshot()
            .server_client_expiration
            .as_ref()
            .map(|e| e.expires_at)
    }

    /// The deadline is persisted, not just announced: a consumer that restarts
    /// before the next `<ib>` still knows the build is being retired.
    #[tokio::test]
    async fn a_client_expiration_is_persisted_and_announced() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        let _subscription = client.subscribe_handler(collector.clone());

        let far = wacore::time::now_secs() as i64 + 90 * 86_400;
        handle_ib_impl(client.clone(), &client_expiration(Some(far)).as_node_ref()).await;

        let snapshot = client.persistence_manager.get_device_snapshot();
        let stored = snapshot
            .server_client_expiration
            .as_ref()
            .expect("deadline stored");
        assert_eq!(stored.expires_at, far);
        assert!(stored.applies_to((
            snapshot.app_version_primary,
            snapshot.app_version_secondary,
            snapshot.app_version_tertiary,
        )));

        assert!(collector.events().iter().any(|event| matches!(
            &**event,
            Event::ClientExpirationChanged(ClientExpirationChanged {
                expires_at: Some(t),
                withdrawn: false,
                ..
            }) if *t == far
        )));
    }

    /// A restatement that changes nothing must stay silent, or a consumer
    /// wired to alert on this event would alert on every reconnect.
    #[tokio::test]
    async fn an_unchanged_deadline_is_not_reannounced() {
        let client = create_test_client().await;
        let far = wacore::time::now_secs() as i64 + 90 * 86_400;
        handle_ib_impl(client.clone(), &client_expiration(Some(far)).as_node_ref()).await;

        let collector = Arc::new(TestEventCollector::default());
        let _subscription = client.subscribe_handler(collector.clone());
        handle_ib_impl(client.clone(), &client_expiration(Some(far)).as_node_ref()).await;

        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::ClientExpirationChanged(_))),
            "restating the same deadline must not dispatch"
        );
        assert_eq!(stored_deadline(&client), Some(far));
    }

    /// `<client_expiration>` with no `t` retracts the deadline.
    #[tokio::test]
    async fn a_missing_t_withdraws_the_deadline() {
        let client = create_test_client().await;
        let far = wacore::time::now_secs() as i64 + 90 * 86_400;
        handle_ib_impl(client.clone(), &client_expiration(Some(far)).as_node_ref()).await;

        let collector = Arc::new(TestEventCollector::default());
        let _subscription = client.subscribe_handler(collector.clone());
        handle_ib_impl(client.clone(), &client_expiration(None).as_node_ref()).await;

        assert!(stored_deadline(&client).is_none());
        assert!(collector.events().iter().any(|event| matches!(
            &**event,
            Event::ClientExpirationChanged(ClientExpirationChanged {
                expires_at: None,
                withdrawn: true,
                ..
            })
        )));
    }

    /// Nothing held means nothing to withdraw, so the common case -- a server
    /// that never set a deadline -- announces nothing.
    #[tokio::test]
    async fn withdrawing_a_deadline_never_held_is_silent() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        let _subscription = client.subscribe_handler(collector.clone());

        handle_ib_impl(client.clone(), &client_expiration(None).as_node_ref()).await;

        assert!(stored_deadline(&client).is_none());
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::ClientExpirationChanged(_)))
        );
    }

    /// A `t` we cannot read is not a withdrawal. Treating it as one would let
    /// a malformed or out-of-range value clear a deadline the server never
    /// retracted.
    #[tokio::test]
    async fn an_unusable_t_leaves_the_deadline_alone() {
        let client = create_test_client().await;
        let far = wacore::time::now_secs() as i64 + 90 * 86_400;
        handle_ib_impl(client.clone(), &client_expiration(Some(far)).as_node_ref()).await;

        let collector = Arc::new(TestEventCollector::default());
        let _subscription = client.subscribe_handler(collector.clone());

        for bad in ["9223372036854775808", "-1", "soon", ""] {
            let node = ib_with(NodeBuilder::new("client_expiration").attr("t", bad).build());
            handle_ib_impl(client.clone(), &node.as_node_ref()).await;
            assert_eq!(
                stored_deadline(&client),
                Some(far),
                "t={bad:?} must not disturb the stored deadline"
            );
        }
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::ClientExpirationChanged(_))),
            "an unusable t announces nothing"
        );
    }

    #[tokio::test]
    async fn valid_dirty_marker_dispatches_typed_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        let _subscription = client.subscribe_handler(collector.clone());
        let node = NodeBuilder::new("ib")
            .children([NodeBuilder::new("dirty")
                .attr("type", "account_sync")
                .attr("timestamp", "1725000000")
                .build()])
            .build();

        handle_ib_impl(client, &node.as_node_ref()).await;

        assert!(collector.events().iter().any(|event| {
            matches!(
                &**event,
                Event::DirtyState(DirtyState {
                    dirty_type: DirtyType::AccountSync,
                    timestamp: Some(1_725_000_000),
                    ..
                })
            )
        }));
    }
}
