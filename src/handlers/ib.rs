use super::traits::StanzaHandler;
use crate::client::Client;
use crate::types::events::{DirtyState, Event, EventKind, OfflineSyncPreview};
use async_trait::async_trait;
use futures::FutureExt;
use log::{debug, info, warn};
use std::sync::Arc;
use wacore::appstate::patch_decode::WAPatchName;
use wacore::iq::dirty::{DirtyBit, DirtyType};
use wacore::stanza::wire_tags::StanzaTag;

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
        match child.tag.as_ref() {
            "dirty" => {
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
            "edge_routing" => {
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
            "offline_preview" => {
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
            "offline" => {
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
            "thread_metadata" => {
                // Present in some sessions; safe to ignore for now until feature implemented.
                debug!("Received thread metadata, ignoring for now.");
            }
            _ => {
                warn!("Unhandled ib child: <{}>", child.tag);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{TestEventCollector, create_test_client};
    use wacore_binary::builder::NodeBuilder;

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
