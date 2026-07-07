use crate::client::Client;
use crate::types::events::Event;
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

    // Resolve to a LID key. We borrow from Jid.user (CompactString) or from
    // get_current_lid (CompactString), then pass as &str to the storage layer.
    let resolved_lid: Option<wacore_binary::CompactString>;
    let sender_lid: &str = if let Some(ref lid_jid) = sender_lid_jid {
        &lid_jid.user
    } else {
        let from = match &from_jid {
            Some(jid) => jid,
            None => {
                warn!(target: "Client/TcToken", "privacy_token notification missing 'from' attribute");
                return;
            }
        };

        if from.is_lid() {
            &from.user
        } else {
            resolved_lid = client.lid_pn_cache.get_current_lid(&from.user).await;
            match &resolved_lid {
                Some(lid) => lid.as_str(),
                None => {
                    debug!(
                        target: "Client/TcToken",
                        "Cannot resolve LID for privacy_token sender {}, storing under PN",
                        from.observe()
                    );
                    &from.user
                }
            }
        }
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

    let backend = client.persistence_manager.backend();
    let mut token_stored = false;

    for received in &received_tokens {
        let existing = match backend.get_tc_token(sender_lid).await {
            Ok(entry) => entry,
            Err(e) => {
                warn!(target: "Client/TcToken", "Failed to read tc_token for {}: {e}, skipping", sender_lid);
                continue;
            }
        };

        if let Some(existing) = &existing {
            // Same bytes: only advance the timestamp forward.
            if existing.token == received.token {
                if received.timestamp > existing.token_timestamp
                    && let Err(e) = backend
                        .store_received_tc_token(sender_lid, &received.token, received.timestamp)
                        .await
                {
                    warn!(target: "Client/TcToken", "Failed to refresh tc_token timestamp for {}: {e}", sender_lid);
                }
                continue;
            }

            // Don't replace a real token with an older one. A byte-less
            // placeholder (written by sender-side issuance) has no real token
            // yet, so always accept the contact's first real token.
            if !existing.token.is_empty() && received.timestamp < existing.token_timestamp {
                debug!(
                    target: "Client/TcToken",
                    "Skipping older token for {} (incoming={}, existing={})",
                    sender_lid, received.timestamp, existing.token_timestamp
                );
                continue;
            }
        }

        // store_received_tc_token preserves any sender_timestamp written
        // concurrently by the issuance path.
        if let Err(e) = backend
            .store_received_tc_token(sender_lid, &received.token, received.timestamp)
            .await
        {
            warn!(target: "Client/TcToken", "Failed to store tc_token for {}: {e}", sender_lid);
        } else {
            debug!(target: "Client/TcToken", "Stored tc_token for {} (t={})", sender_lid, received.timestamp);
            token_stored = true;
        }
    }

    // Re-subscribe presence with the updated token.
    if token_stored
        && let Some(from) = &from_jid
        && let Err(e) = client.presence().re_subscribe_when_active(from).await
    {
        debug!(target: "Client/TcToken", "Failed to re-subscribe presence for {}: {e}", from.observe());
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

    let update_type = BusinessUpdateType::from(notification.notification_type.clone());
    let verified_name = notification
        .verified_name
        .as_ref()
        .and_then(|vn| vn.name.clone());

    let event = Event::BusinessStatusUpdate(
        BusinessStatusUpdate::builder()
            .jid(notification.from.clone())
            .update_type(update_type)
            .timestamp(wacore::time::from_secs_or_now(notification.timestamp))
            .maybe_target_jid(notification.jid.clone())
            .maybe_hash(notification.hash.clone())
            .maybe_verified_name(verified_name)
            .product_ids(notification.product_ids.clone())
            .collection_ids(notification.collection_ids.clone())
            .subscriptions(notification.subscriptions.clone())
            .build(),
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

    client.core.event_bus.dispatch(event);
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
