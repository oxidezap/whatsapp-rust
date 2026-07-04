use crate::client::Client;
use crate::types::events::Event;
use log::{debug, warn};
use std::sync::Arc;
use wacore::stanza::groups::{GroupNotification, GroupNotificationAction};
use wacore::types::events::{GroupUpdate, MexNotification};
use wacore_binary::NodeContentRef;
use wacore_binary::{NodeRef, OwnedNodeRef};

/// Sync is fire-and-forget (spawned), so this is not async -- it parses
/// collection nodes synchronously and spawns the async sync task.
pub(crate) fn handle_server_sync_notification(
    client: &Arc<Client>,
    nr: &wacore_binary::NodeRef<'_>,
) {
    use std::str::FromStr;
    use wacore::appstate::patch_decode::WAPatchName;

    let mut collections = Vec::new();
    if let Some(children) = nr.children() {
        for collection_node in children.iter().filter(|c| c.tag == "collection") {
            let name_cow = collection_node.attrs().optional_string("name");
            let name_str = name_cow.as_deref().unwrap_or("<unknown>");
            let server_version = collection_node.attrs().optional_u64("version").unwrap_or(0);
            debug!(
                target: "Client/AppState",
                "Received server_sync for collection '{}' version {}",
                name_str, server_version
            );
            if let Ok(patch_name) = WAPatchName::from_str(name_str)
                && !matches!(patch_name, WAPatchName::Unknown)
            {
                collections.push((patch_name, server_version));
            }
        }
    }

    if !collections.is_empty() {
        let client_clone = client.clone();
        let generation = client
            .connection_generation
            .load(std::sync::atomic::Ordering::Acquire);
        client
            .runtime
            .spawn(Box::pin(async move {
                if client_clone
                    .connection_generation
                    .load(std::sync::atomic::Ordering::Acquire)
                    != generation
                {
                    log::debug!(target: "Client/AppState", "server_sync task cancelled: connection generation changed");
                    return;
                }

                let backend = client_clone.persistence_manager.backend();
                let mut to_sync = Vec::new();
                for (name, server_version) in collections {
                    if server_version > 0 {
                        match backend.get_version(name.as_str()).await {
                            Ok(state) if state.version >= server_version => {
                                debug!(
                                    target: "Client/AppState",
                                    "Skipping server_sync for {:?}: local version {} >= server version {}",
                                    name, state.version, server_version
                                );
                                continue;
                            }
                            Ok(_) => {}
                            Err(e) => {
                                warn!(
                                    target: "Client/AppState",
                                    "Failed to get local version for {:?}: {e}, syncing anyway",
                                    name
                                );
                            }
                        }
                    }
                    to_sync.push(name);
                }

                if !to_sync.is_empty() {
                    if client_clone.is_shutting_down() {
                        log::debug!(target: "Client/AppState", "Skipping server_sync: client is shutting down");
                        return;
                    }
                    if client_clone
                        .connection_generation
                        .load(std::sync::atomic::Ordering::Acquire)
                        != generation
                    {
                        log::debug!(target: "Client/AppState", "server_sync task cancelled: connection generation changed during version check");
                        return;
                    }
                    if let Err(e) = client_clone.sync_collections_batched(to_sync, None).await
                        && !client_clone.is_shutting_down()
                    {
                        warn!(
                            target: "Client/AppState",
                            "Failed to batch sync app state from server_sync: {e}"
                        );
                    }
                }
            }))
            .detach();
    }
}

/// Handle w:gp2 group notifications.
///
/// Parses all child actions (participant changes, setting changes, metadata updates)
/// and dispatches typed `Event::GroupUpdate` events for each.
///
/// Reference: WhatsApp Web `WAWebHandleGroupNotification` (Ri7Gf1BxhsX.js:12556-12962)
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.notif.group", level = "debug", skip_all)
)]
pub(crate) async fn handle_group_notification(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    let notification = match GroupNotification::try_from_node_ref(node.get()) {
        Some(n) => n,
        None => {
            warn!(target: "Client/Group", "w:gp2 notification missing 'from' attribute");
            return;
        }
    };

    let timestamp = i64::try_from(notification.timestamp)
        .ok()
        .and_then(wacore::time::from_secs)
        .unwrap_or_else(wacore::time::now_utc);

    for action in notification.actions {
        // Granularly patch group cache instead of invalidating — matches WA Web's
        // addParticipantInfo / removeParticipantInfo pattern and avoids a
        // group metadata IQ round-trip.
        match &action {
            GroupNotificationAction::Add { participants, .. } => {
                let group_cache = client.get_group_cache().await;
                if let Some(info) = group_cache.get(&notification.group_jid).await {
                    let mut info = Arc::unwrap_or_clone(info);
                    info.add_participants(
                        participants
                            .iter()
                            .map(|p| (&p.jid, p.phone_number.as_ref())),
                    );
                    client
                        .persist_group_metadata(&notification.group_jid, &info)
                        .await;
                    group_cache
                        .insert(notification.group_jid.clone(), Arc::new(info))
                        .await;
                    debug!(
                        target: "Client/Group",
                        "Patched group cache for {}: added {} participants",
                        notification.group_jid.observe(), participants.len()
                    );
                } else {
                    // Cache expired: can't patch in place, so drop the now-stale blob.
                    debug!(
                        target: "Client/Group",
                        "Group cache expired for {}: invalidating persisted metadata (add)",
                        notification.group_jid.observe()
                    );
                    client
                        .invalidate_persisted_group_metadata(&notification.group_jid)
                        .await;
                }
            }
            GroupNotificationAction::Remove { participants, .. } => {
                let users: Vec<&str> = participants.iter().map(|p| p.jid.user.as_str()).collect();
                let group_cache = client.get_group_cache().await;
                if let Some(info) = group_cache.get(&notification.group_jid).await {
                    let mut info = Arc::unwrap_or_clone(info);
                    info.remove_participants(&users);
                    client
                        .persist_group_metadata(&notification.group_jid, &info)
                        .await;
                    group_cache
                        .insert(notification.group_jid.clone(), Arc::new(info))
                        .await;
                    debug!(
                        target: "Client/Group",
                        "Patched group cache for {}: removed {} participants",
                        notification.group_jid.observe(), participants.len()
                    );
                } else {
                    // Cache expired: can't patch in place, so drop the now-stale blob.
                    debug!(
                        target: "Client/Group",
                        "Group cache expired for {}: invalidating persisted metadata (remove)",
                        notification.group_jid.observe()
                    );
                    client
                        .invalidate_persisted_group_metadata(&notification.group_jid)
                        .await;
                }
                client
                    .rotate_sender_key_on_participant_remove(
                        &notification.group_jid.to_string(),
                        &users,
                    )
                    .await;
            }
            GroupNotificationAction::Modify { .. } => {
                // A participant changed number / migrated PN<->LID. WA Web
                // (`modifyParticipantInfo`) deletes the old user's sender-key
                // entry, adds the new device, and sets `rotateKey`
                // unconditionally. We can't granularly patch (the action
                // carries only the new participants, not the old JID), so drop
                // the now-stale metadata blob and force-rotate our own sender
                // key so the next send regenerates and redistributes — matching
                // WA Web's forward-secrecy rotation and clearing the stale
                // has_key tracking for the departed number.
                debug!(
                    target: "Client/Group",
                    "Group modify (number/LID migration) for {}: invalidating metadata and rotating sender key",
                    notification.group_jid.observe()
                );
                client
                    .invalidate_persisted_group_metadata(&notification.group_jid)
                    .await;
                // Also drop the in-memory blob: query_info() checks group_cache
                // before storage, so a stale Arc<GroupInfo> here would let the
                // next send rebuild the sender key against the old participant
                // list (missing the migrated JID, still targeting the old one).
                client
                    .get_group_cache()
                    .await
                    .invalidate(&notification.group_jid)
                    .await;
                client
                    .force_rotate_own_sender_key(&notification.group_jid.to_string())
                    .await;
            }
            _ => {}
        }

        debug!(
            target: "Client/Group",
            "Group notification: group={}, action={}",
            notification.group_jid.observe(), action.tag_name()
        );

        client
            .core
            .event_bus
            .dispatch(Event::GroupUpdate(GroupUpdate {
                group_jid: notification.group_jid.clone(),
                participant: notification.participant.clone(),
                participant_pn: notification.participant_pn.clone(),
                timestamp,
                is_lid_addressing_mode: notification.is_lid_addressing_mode,
                action,
            }));
    }

    // Also dispatch legacy generic notification for backward compatibility
    client
        .core
        .event_bus
        .dispatch(Event::Notification(Arc::clone(&node)));
}

/// Handle `<notification type="newsletter">` — live updates with reaction counts.
///
/// Format:
/// ```xml
/// <notification from="NL_JID" type="newsletter" id="..." t="...">
///   <live_updates>
///     <messages jid="NL_JID" t="...">
///       <message server_id="123" ...>
///         <reactions><reaction code="👍" count="3"/></reactions>
///       </message>
///     </messages>
///   </live_updates>
/// </notification>
/// ```
pub(crate) fn handle_newsletter_notification(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    use crate::features::newsletter::parse_reaction_counts;
    use wacore::types::events::{
        NewsletterLiveUpdate, NewsletterLiveUpdateMessage, NewsletterLiveUpdateReaction,
    };

    let nr = node.get();

    let Some(newsletter_jid) = nr.attrs().optional_jid("from") else {
        return;
    };

    if let Some(live_updates) = nr.get_optional_child("live_updates")
        && let Some(messages_node) = live_updates.get_optional_child("messages")
        && let Some(children) = messages_node.children()
    {
        let messages: Vec<_> = children
            .iter()
            .filter(|n| n.tag.as_ref() == "message")
            .filter_map(|msg_node| {
                let server_id = msg_node
                    .get_attr("server_id")
                    .map(|v| v.as_str())
                    .and_then(|s| s.parse::<u64>().ok())?;

                let reactions = parse_reaction_counts(msg_node)
                    .into_iter()
                    .map(|r| NewsletterLiveUpdateReaction {
                        code: r.code,
                        count: r.count,
                    })
                    .collect();

                Some(NewsletterLiveUpdateMessage {
                    server_id,
                    reactions,
                })
            })
            .collect();

        if !messages.is_empty() {
            client
                .core
                .event_bus
                .dispatch(Event::NewsletterLiveUpdate(NewsletterLiveUpdate {
                    newsletter_jid,
                    messages,
                }));
        }
    }

    // Also dispatch raw notification for backward compatibility
    client
        .core
        .event_bus
        .dispatch(Event::Notification(Arc::clone(&node)));
}

/// `<notification type="mex"><update op_name="…">{json}</update></notification>`
/// Routed by `op_name` so the dispatcher survives bundle rebuilds.
pub(crate) fn handle_mex_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let Some(update_node) = node.get_optional_child("update") else {
        warn!(
            target: "Client/Mex",
            "mex notification missing <update> child: {}",
            wacore::xml::DisplayableNodeRef(node)
        );
        return;
    };

    let Some(op_name) = update_node.attrs().optional_string("op_name") else {
        warn!(
            target: "Client/Mex",
            "mex notification <update> missing op_name attribute: {}",
            wacore::xml::DisplayableNodeRef(node)
        );
        return;
    };

    // `from_str` skips the redundant UTF-8 validation `from_slice` would
    // do on a `&str`.
    let parsed = match update_node.content.as_deref() {
        Some(NodeContentRef::String(s)) => serde_json::from_str(s),
        Some(NodeContentRef::Bytes(b)) => serde_json::from_slice(b.as_ref()),
        _ => {
            warn!(target: "Client/Mex", "mex notification op={op_name} has no JSON body");
            return;
        }
    };
    let payload: serde_json::Value = match parsed {
        Ok(v) => v,
        Err(e) => {
            warn!(target: "Client/Mex", "mex notification op={op_name} JSON parse failed: {e}");
            return;
        }
    };

    let mut attrs = node.attrs();
    let from = attrs.optional_jid("from");
    let stanza_id = attrs.optional_string("id").map(|s| s.into_owned());
    let offline = attrs.optional_string("offline").map(|s| s.into_owned());
    let op_name = op_name.into_owned();

    debug!(
        target: "Client/Mex",
        "mex notification received: op_name={op_name} offline={}",
        offline.is_some()
    );
    client
        .core
        .event_bus
        .dispatch(Event::MexNotification(MexNotification {
            op_name,
            from,
            stanza_id,
            offline,
            payload,
        }));
}

/// Handle `<notification type="disappearing_mode">` — a contact changed
/// their default disappearing messages setting.
///
/// WA Web: `WAWebHandleDisappearingModeNotification` parses the
/// `<disappearing_mode duration="..." t="..."/>` child and calls
/// `WAWebUpdateDisappearingModeForContact` which applies the update only
/// if the new timestamp is newer than the stored one.
///
/// We dispatch `Event::DisappearingModeChanged` and let consumers decide
/// how to persist/apply it.
pub(crate) fn handle_disappearing_mode_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let mut attrs = node.attrs();
    let from = attrs.jid("from").to_non_ad();

    let Some(dm_node) = node.get_optional_child("disappearing_mode") else {
        warn!(
            "disappearing_mode notification missing <disappearing_mode> child: {}",
            wacore::xml::DisplayableNodeRef(node)
        );
        return;
    };

    let mut dm_attrs = dm_node.attrs();

    // WA Web: `t.attrInt("duration", 0)` — defaults to 0 (disabled).
    let duration = dm_attrs
        .optional_string("duration")
        .and_then(|s| s.parse::<u32>().ok())
        .unwrap_or(0);

    // WA Web: `t.attrTime("t")` — required, no default.
    let Some(setting_timestamp) = dm_attrs
        .optional_string("t")
        .and_then(|s| s.parse::<i64>().ok())
        .and_then(wacore::time::from_secs)
    else {
        warn!(
            "disappearing_mode notification missing or invalid 't' attribute: {}",
            wacore::xml::DisplayableNodeRef(node)
        );
        return;
    };

    debug!(
        "Disappearing mode changed for {}: duration={}s, t={}",
        from.observe(),
        duration,
        setting_timestamp
    );

    client
        .core
        .event_bus
        .dispatch(Event::DisappearingModeChanged(
            wacore::types::events::DisappearingModeChanged {
                from,
                duration,
                setting_timestamp,
            },
        ));
}
