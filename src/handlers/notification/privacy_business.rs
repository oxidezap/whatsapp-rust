use crate::client::Client;
use crate::types::events::{Event, EventKind};
use log::{debug, info, warn};
use std::sync::Arc;
use wacore::stanza::business::BusinessNotification;
use wacore::types::events::{BusinessStatusUpdate, BusinessUpdateType};
use wacore_binary::NodeRef;

/// Handle incoming privacy_token notification.
///
/// Stores trusted contact tokens from contacts. Matches WhatsApp Web's
/// `WAWebHandlePrivacyTokenNotification`.
///
/// Structure:
/// ```xml
/// <notification type="privacy_token" from="user@s.whatsapp.net" sender_lid="user@lid">
///   <tokens>
///     <token type="trusted_contact" t="1707000000"><!-- bytes --></token>
///   </tokens>
/// </notification>
/// ```
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.notif.privacy_token", level = "debug", skip_all)
)]
pub(crate) async fn handle_privacy_token_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    use wacore::iq::tctoken::parse_privacy_token_notification;

    let from_jid = node.attrs().optional_jid("from");

    // Resolve the sender to a LID key for storage.
    // WA Web uses `sender_lid` attr if present, otherwise resolves from `from`.
    let sender_lid_jid = node
        .attrs()
        .optional_jid("sender_lid")
        .filter(|j| !j.user.is_empty());

    let Some(source) = sender_lid_jid.clone().or_else(|| from_jid.clone()) else {
        warn!(target: "Client/TcToken", "privacy_token notification missing 'from' attribute");
        return;
    };

    // Parse the token data from the notification
    let received_tokens = match parse_privacy_token_notification(node) {
        Ok(tokens) => tokens,
        Err(e) => {
            warn!(target: "Client/TcToken", "Failed to parse privacy_token notification: {e}");
            return;
        }
    };

    if received_tokens.is_empty() {
        debug!(target: "Client/TcToken", "privacy_token notification had no trusted_contact tokens");
        return;
    }

    let mut token_stored = false;
    let mut storage_key = source.clone();

    for received in received_tokens {
        match client
            .persist_tc_token_owned(
                source.clone(),
                Some((received.token, received.timestamp)),
                None,
                true,
            )
            .await
        {
            Ok(outcome) => {
                storage_key = outcome.key;
                token_stored |= outcome.changed;
            }
            Err(e) => {
                warn!(target: "Client/TcToken", "Failed to store tc_token for {}: {e}", source.observe());
            }
        }
    }

    // Re-subscribe presence with the updated token.
    if token_stored && let Some(from) = &from_jid {
        // The notification's PN and the active presence key can be different
        // namespaces. Prefer the tracked LID, retaining legacy PN subscriptions.
        let lid = if let Some(lid) = sender_lid_jid {
            Some(lid)
        } else if storage_key.is_lid() {
            Some(storage_key.clone())
        } else {
            client
                .lid_pn_cache
                .get_current_lid(&from.user)
                .await
                .map(wacore_binary::Jid::lid)
        };
        let target = lid
            .as_ref()
            .filter(|lid| client.is_presence_subscription_tracked(lid))
            .unwrap_or(from);
        if let Err(e) = client
            .presence()
            .re_subscribe_when_active(target, &storage_key.user, !storage_key.is_lid())
            .await
        {
            debug!(target: "Client/TcToken", "Failed to re-subscribe presence for {}: {e}", target.observe());
        }
    }
}

/// Handle business notification (WhatsApp Web: `WAWebHandleBusinessNotification`).
pub(crate) async fn handle_business_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let notification = match BusinessNotification::try_parse(node) {
        Ok(n) => n,
        Err(e) => {
            warn!(target: "Client/Business", "Failed to parse business notification: {e}");
            return;
        }
    };

    debug!(
        target: "Client/Business",
        "Business notification: from={}, type={}, jid={:?}",
        notification.from.observe(),
        notification.notification_type,
        notification.jid
    );

    match notification.notification_type {
        wacore::stanza::business::BusinessNotificationType::RemoveJid
        | wacore::stanza::business::BusinessNotificationType::RemoveHash => {
            info!(
                target: "Client/Business",
                "Contact {} is no longer a business account",
                notification.from.observe()
            );
        }
        wacore::stanza::business::BusinessNotificationType::VerifiedNameJid
        | wacore::stanza::business::BusinessNotificationType::VerifiedNameHash => {
            if let Some(name) = &notification
                .verified_name
                .as_ref()
                .and_then(|vn| vn.name.as_ref())
            {
                info!(
                    target: "Client/Business",
                    "Contact {} verified business name: {}",
                    notification.from.observe(),
                    name
                );
            }
        }
        wacore::stanza::business::BusinessNotificationType::Profile
        | wacore::stanza::business::BusinessNotificationType::ProfileHash => {
            debug!(
                target: "Client/Business",
                "Contact {} business profile updated (hash: {:?})",
                notification.from.observe(),
                notification.hash
            );
        }
        _ => {}
    }

    client
        .core
        .event_bus
        .dispatch_with(EventKind::BusinessStatusUpdate, || {
            Event::BusinessStatusUpdate(
                BusinessStatusUpdate::builder()
                    .jid(notification.from.clone())
                    .update_type(BusinessUpdateType::from(
                        notification.notification_type.clone(),
                    ))
                    .timestamp(
                        wacore::time::from_secs(notification.timestamp)
                            .unwrap_or_else(|| client.runtime.wall_time_utc()),
                    )
                    .maybe_target_jid(notification.jid.clone())
                    .maybe_hash(notification.hash.clone())
                    .maybe_verified_name(
                        notification
                            .verified_name
                            .as_ref()
                            .and_then(|vn| vn.name.clone()),
                    )
                    .product_ids(notification.product_ids.clone())
                    .collection_ids(notification.collection_ids.clone())
                    .subscriptions(notification.subscriptions.clone())
                    .build(),
            )
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_client;
    use wacore::store::traits::TcTokenEntry;
    use wacore_binary::builder::NodeBuilder;

    fn privacy_token_notification(from_lid: &str, t: i64, bytes: Vec<u8>) -> wacore_binary::Node {
        NodeBuilder::new("notification")
            .attr("type", "privacy_token")
            .attr("from", from_lid)
            .children([NodeBuilder::new("tokens")
                .children([NodeBuilder::new("token")
                    .attr("type", "trusted_contact")
                    .attr("t", t.to_string())
                    .bytes(bytes)
                    .build()])
                .build()])
            .build()
    }

    async fn assert_legacy_token_retry_during_mapping_change(after_store: bool) {
        use crate::lid_pn_cache::LearningSource;
        use crate::store::release::tests::ProbeBackend;
        use crate::test_utils::{create_iq_test_client_with_backend, decode_sent_iq};
        use std::time::Duration;
        use wacore_binary::Jid;

        let (mut probe, _) = ProbeBackend::new();
        let raw = probe.backend.clone();
        let (release, gate) = async_channel::unbounded();
        let (entered, arrived) = async_channel::unbounded();
        probe.gate_method = if after_store {
            "after:store_received_tc_token"
        } else {
            "store_received_tc_token"
        };
        probe.gate = Some(gate);
        probe.entered = Some(entered);
        probe.gate_once = true;
        let (client, transport) = create_iq_test_client_with_backend(Arc::new(probe)).await;
        let phone = Jid::pn("12025550160");
        let lid = Jid::lid("100000000000260");
        client.track_presence_subscription(phone.clone());
        let token = b"synthetic-legacy-token".to_vec();
        let timestamp = client.runtime.wall_time_secs();
        let notification = privacy_token_notification(&phone.to_string(), timestamp, token.clone());
        let handler = {
            let client = client.clone();
            tokio::spawn(async move {
                handle_privacy_token_notification(&client, &notification.as_node_ref()).await;
            })
        };
        tokio::time::timeout(Duration::from_secs(5), arrived.recv())
            .await
            .unwrap()
            .unwrap();
        let mut ownership = Box::pin(client.lid_pn_cache.lock_mutation());
        assert!(futures::poll!(ownership.as_mut()).is_pending());
        drop(ownership);
        let mapping = {
            let client = client.clone();
            let phone = phone.clone();
            let lid = lid.clone();
            tokio::spawn(async move {
                client
                    .add_lid_pn_mapping(&lid.user, &phone.user, LearningSource::Usync)
                    .await
            })
        };
        assert!(
            client
                .lid_pn_cache
                .get_current_lid(&phone.user)
                .await
                .is_none()
        );
        if after_store {
            assert_eq!(
                raw.get_tc_token(&phone.user).await.unwrap().unwrap().token,
                token
            );
        } else {
            assert!(raw.get_tc_token(&phone.user).await.unwrap().is_none());
        }
        release.send(()).await.unwrap();
        tokio::time::timeout(Duration::from_secs(5), mapping)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        tokio::time::timeout(Duration::from_secs(5), handler)
            .await
            .unwrap()
            .unwrap();
        let sent = decode_sent_iq(&transport, 0).await;
        assert_eq!(sent.get().tag, "presence");
        assert_eq!(sent.get().attrs().optional_jid("to"), Some(phone.clone()));
        assert_eq!(
            sent.get().attrs().optional_string("type").as_deref(),
            Some("subscribe")
        );
        assert_eq!(
            sent.get()
                .get_optional_child("tctoken")
                .and_then(|child| child.content_bytes()),
            Some(token.as_slice())
        );
        assert!(raw.get_tc_token(&phone.user).await.unwrap().is_none());
        assert_eq!(
            raw.get_tc_token(&lid.user).await.unwrap().unwrap().token,
            token
        );
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn adv_review_round3_new_notification_token_survives_mapping() {
        use crate::lid_pn_cache::LearningSource;
        use crate::store::release::tests::ProbeBackend;
        use crate::test_utils::create_iq_test_client_with_backend;
        use std::time::Duration;
        use wacore_binary::Jid;
        for write_after_delete in [false, true] {
            let (mut deleting, _) = ProbeBackend::new();
            let raw = deleting.backend.clone();
            let (release_delete, gate) = async_channel::unbounded();
            let (deleting_tx, deleting_rx) = async_channel::unbounded();
            deleting.gate_method = "delete_tc_token";
            deleting.gate = Some(gate);
            deleting.entered = Some(deleting_tx);
            let (mut writing, _) = ProbeBackend::new();
            writing.backend = Arc::new(deleting);
            let (release_write, gate) = async_channel::unbounded();
            let (writing_tx, writing_rx) = async_channel::unbounded();
            writing.gate_method = "store_received_tc_token";
            writing.gate_once = true;
            writing.gate = Some(gate);
            writing.entered = Some(writing_tx);
            let (client, _) = create_iq_test_client_with_backend(Arc::new(writing)).await;
            tokio::time::timeout(Duration::from_secs(5), async {
                while !client.lid_pn_cache.startup_finished_for_tests() {
                    tokio::task::yield_now().await;
                }
                drop(client.lid_pn_cache.lock_mutation().await);
            })
            .await
            .unwrap();
            let phone = Jid::pn("12025550177");
            let lid = Jid::lid("100000000000277");
            let now = client.runtime.wall_time_secs();
            raw.store_received_tc_token(&phone.user, b"synthetic-old", now - 1)
                .await
                .unwrap();
            let notification =
                privacy_token_notification(&phone.to_string(), now, b"synthetic-new".to_vec());
            let handler = {
                let client = client.clone();
                tokio::spawn(async move {
                    handle_privacy_token_notification(&client, &notification.as_node_ref()).await
                })
            };
            tokio::time::timeout(Duration::from_secs(5), writing_rx.recv())
                .await
                .unwrap()
                .unwrap();
            let mut check = Box::pin(client.lid_pn_cache.lock_mutation());
            let serialized = match futures::poll!(check.as_mut()) {
                std::task::Poll::Pending => true,
                std::task::Poll::Ready(guard) => {
                    drop(guard);
                    false
                }
            };
            drop(check);
            let mapping = {
                let client = client.clone();
                let phone = phone.clone();
                let lid = lid.clone();
                tokio::spawn(async move {
                    client
                        .add_lid_pn_mapping(&lid.user, &phone.user, LearningSource::Usync)
                        .await
                })
            };
            if serialized {
                // A correct writer fences migration until its chosen key/write
                // reaches ACK. Drive that admitted ordering, with the same data
                // assertions as the formerly possible lost/orphan interleavings.
                release_write.send(()).await.unwrap();
                tokio::time::timeout(Duration::from_secs(5), handler)
                    .await
                    .unwrap()
                    .unwrap();
                tokio::time::timeout(Duration::from_secs(5), deleting_rx.recv())
                    .await
                    .unwrap()
                    .unwrap();
                release_delete.send(()).await.unwrap();
                tokio::time::timeout(Duration::from_secs(5), mapping)
                    .await
                    .unwrap()
                    .unwrap()
                    .unwrap();
            } else {
                tokio::time::timeout(Duration::from_secs(5), deleting_rx.recv())
                    .await
                    .unwrap()
                    .unwrap();
                if write_after_delete {
                    release_delete.send(()).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(5), mapping)
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap();
                    release_write.send(()).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(5), handler)
                        .await
                        .unwrap()
                        .unwrap();
                } else {
                    release_write.send(()).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(5), handler)
                        .await
                        .unwrap()
                        .unwrap();
                    release_delete.send(()).await.unwrap();
                    tokio::time::timeout(Duration::from_secs(5), mapping)
                        .await
                        .unwrap()
                        .unwrap()
                        .unwrap();
                }
            }
            assert_eq!(
                raw.get_tc_token(&lid.user).await.unwrap().unwrap().token,
                b"synthetic-new"
            );
            assert!(raw.get_tc_token(&phone.user).await.unwrap().is_none());
            client.signal_shutdown_sync();
        }
    }

    #[tokio::test]
    async fn review_pn_token_notification_resubscribes_the_tracked_lid() {
        use crate::test_utils::{create_iq_test_client, decode_sent_iq};
        use wacore_binary::Jid;

        for explicit_lid in [true, false] {
            let (client, transport) = create_iq_test_client().await;
            let phone = Jid::pn("12025550162");
            let lid = Jid::lid("100000000000262");
            if !explicit_lid {
                client
                    .learn_lid_pn_mapping_fast(
                        &lid.user,
                        &phone.user,
                        crate::lid_pn_cache::LearningSource::Usync,
                        true,
                    )
                    .await;
            }
            client.track_presence_subscription(lid.clone());
            let token = b"synthetic-tracked-lid-token".to_vec();
            let mut notification = NodeBuilder::new("notification")
                .attr("type", "privacy_token")
                .attr("from", phone.to_string());
            if explicit_lid {
                notification = notification.attr("sender_lid", lid.to_string());
            }
            let notification = notification
                .children([NodeBuilder::new("tokens")
                    .children([NodeBuilder::new("token")
                        .attr("type", "trusted_contact")
                        .attr("t", client.runtime.wall_time_secs().to_string())
                        .bytes(token.clone())
                        .build()])
                    .build()])
                .build();
            handle_privacy_token_notification(&client, &notification.as_node_ref()).await;
            let sent = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                decode_sent_iq(&transport, 0),
            )
            .await
            .expect("the active LID subscription must be retried");
            assert_eq!(sent.get().attrs().optional_jid("to"), Some(lid));
            assert_eq!(
                sent.get()
                    .get_optional_child("tctoken")
                    .and_then(|child| child.content_bytes()),
                Some(token.as_slice())
            );
            client.signal_shutdown_sync();
        }
    }

    #[tokio::test]
    async fn review_token_retry_keeps_pn_key_when_mapping_appears_during_save() {
        assert_legacy_token_retry_during_mapping_change(false).await;
    }

    #[tokio::test]
    async fn review_token_retry_finds_a_pn_token_moved_before_retry() {
        assert_legacy_token_retry_during_mapping_change(true).await;
    }

    #[tokio::test]
    async fn review_token_retry_does_not_replace_expired_explicit_lid_with_pn_token() {
        use crate::test_utils::{create_iq_test_client, decode_sent_iq};
        use wacore_binary::Jid;
        let (client, transport) = create_iq_test_client().await;
        let phone = Jid::pn("12025550161");
        let lid = Jid::lid("100000000000261");
        client.track_presence_subscription(phone.clone());
        client
            .persistence_manager
            .backend()
            .store_received_tc_token(
                &phone.user,
                b"synthetic-current-pn",
                client.runtime.wall_time_secs(),
            )
            .await
            .unwrap();
        let notification = NodeBuilder::new("notification")
            .attr("type", "privacy_token")
            .attr("from", phone.to_string())
            .attr("sender_lid", lid.to_string())
            .children([NodeBuilder::new("tokens")
                .children([NodeBuilder::new("token")
                    .attr("type", "trusted_contact")
                    .attr("t", "1")
                    .bytes(b"synthetic-expired-lid".to_vec())
                    .build()])
                .build()])
            .build();
        handle_privacy_token_notification(&client, &notification.as_node_ref()).await;
        let sent = decode_sent_iq(&transport, 0).await;
        assert!(sent.get().get_optional_child("tctoken").is_none());
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn stores_new_token_with_no_sender_timestamp() {
        let client = create_test_client().await;
        let node = privacy_token_notification("880000001@lid", 1_700_000_000, vec![0xAA, 0xBB]);

        handle_privacy_token_notification(&client, &node.as_node_ref()).await;

        let stored = client
            .persistence_manager
            .backend()
            .get_tc_token("880000001")
            .await
            .unwrap()
            .expect("token should be stored");
        assert_eq!(stored.token, vec![0xAA, 0xBB]);
        assert_eq!(stored.token_timestamp, 1_700_000_000);
        assert_eq!(stored.sender_timestamp, None);
    }

    #[tokio::test]
    async fn real_token_replaces_byteless_placeholder_and_keeps_sender_timestamp() {
        let client = create_test_client().await;
        let backend = client.persistence_manager.backend();

        // Placeholder written by sender-side issuance: no bytes yet, but a
        // sender_timestamp that is newer than the contact's incoming token.
        backend
            .put_tc_token(
                "880000002",
                &TcTokenEntry {
                    token: Vec::new(),
                    token_timestamp: 1_700_000_500,
                    sender_timestamp: Some(1_700_000_500),
                },
            )
            .await
            .unwrap();

        // Incoming real token has an OLDER timestamp than the placeholder — the
        // monotonicity guard must not drop it.
        let node =
            privacy_token_notification("880000002@lid", 1_700_000_000, vec![0x01, 0x02, 0x03]);
        handle_privacy_token_notification(&client, &node.as_node_ref()).await;

        let stored = backend.get_tc_token("880000002").await.unwrap().unwrap();
        assert_eq!(
            stored.token,
            vec![0x01, 0x02, 0x03],
            "real token must replace the placeholder even with an older timestamp"
        );
        assert_eq!(stored.token_timestamp, 1_700_000_000);
        assert_eq!(
            stored.sender_timestamp,
            Some(1_700_000_500),
            "sender_timestamp must survive the first real token"
        );
    }
}
