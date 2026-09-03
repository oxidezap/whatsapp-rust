use crate::client::Client;
use crate::types::events::{Event, EventKind};
use log::{debug, warn};
use std::sync::Arc;
use wacore::stanza::groups::{GroupNotification, GroupNotificationAction};
use wacore::types::events::{GroupUpdate, MexNotification};
use wacore_binary::NodeContentRef;
use wacore_binary::{NodeRef, OwnedNodeRef};

#[inline]
fn clone_or_take_last<T: Clone>(value: &mut Option<T>, is_last: bool) -> Option<T> {
    if is_last { value.take() } else { value.clone() }
}

/// Sync is fire-and-forget (spawned), so this is not async -- it parses
/// collection nodes synchronously and spawns the async sync task.
pub(crate) fn handle_server_sync_notification(client: &Arc<Client>, nr: &NodeRef<'_>) {
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
                            Ok(Some(state)) if state.version >= server_version => {
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
                    let requested = to_sync.clone();
                    let scope = client_clone.sync_scope(None);
                    let result = client_clone.sync_collections_batched(to_sync, scope).await;
                    if !client_clone.is_shutting_down() {
                        client_clone.report_background_sync(
                            "app state sync from server_sync",
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
    // `<groups_dirty>` is the one `w:gp2` stanza the server sends about itself:
    // `from` is `s.whatsapp.net`, not a group. Parsing it as an ordinary group
    // notification would name the server as the group in every event it
    // produced, so it is routed out before that can happen.
    if let Some(groups) = wacore::stanza::groups::parse_groups_dirty(node.get()) {
        handle_groups_dirty(client, groups);
        client
            .core
            .event_bus
            .dispatch(Event::Notification(Arc::clone(&node)));
        return;
    }

    let mut notification = match GroupNotification::try_from_node_ref(node.get()) {
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

    let actions = std::mem::take(&mut notification.actions);
    let action_count = actions.len();

    for (action_index, action) in actions.into_iter().enumerate() {
        // Granularly patch group cache instead of invalidating — matches WA Web's
        // addParticipantInfo / removeParticipantInfo pattern and avoids a
        // group metadata IQ round-trip.
        match &action {
            GroupNotificationAction::Add { participants, .. } => {
                let metadata = client.lock_group_metadata(&notification.group_jid).await;
                if let Some(info) = metadata.current().await {
                    let mut info = Arc::unwrap_or_clone(info);
                    info.add_participants(
                        participants
                            .iter()
                            .map(|p| (&p.jid, p.phone_number.as_ref())),
                    );
                    metadata.publish(Arc::new(info)).await;
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
                    metadata.invalidate().await;
                }
            }
            GroupNotificationAction::Remove { participants, .. } => {
                let users: Vec<&str> = participants.iter().map(|p| p.jid.user.as_str()).collect();
                let metadata = client.lock_group_metadata(&notification.group_jid).await;
                if let Some(info) = metadata.current().await {
                    let mut info = Arc::unwrap_or_clone(info);
                    info.remove_participants(&users);
                    metadata.publish(Arc::new(info)).await;
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
                    metadata.invalidate().await;
                }
                drop(metadata);
                client
                    .rotate_sender_key_on_participant_remove(&notification.group_jid, &users)
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
                let metadata = client.lock_group_metadata(&notification.group_jid).await;
                metadata.invalidate().await;
                drop(metadata);
                client
                    .force_rotate_own_sender_key(&notification.group_jid)
                    .await;
            }
            _ => {}
        }

        debug!(
            target: "Client/Group",
            "Group notification: group={}, action={}",
            notification.group_jid.observe(), action.tag_name()
        );

        let is_last_action = action_index + 1 == action_count;
        client
            .core
            .event_bus
            .dispatch_with(EventKind::GroupUpdate, || {
                Event::GroupUpdate(
                    GroupUpdate::builder()
                        .group_jid(notification.group_jid.clone())
                        .maybe_notification_id(clone_or_take_last(
                            &mut notification.notification_id,
                            is_last_action,
                        ))
                        .maybe_notify(clone_or_take_last(&mut notification.notify, is_last_action))
                        .maybe_offline(clone_or_take_last(
                            &mut notification.offline,
                            is_last_action,
                        ))
                        .action_index(u32::try_from(action_index).unwrap_or(u32::MAX))
                        .maybe_participant(clone_or_take_last(
                            &mut notification.participant,
                            is_last_action,
                        ))
                        .maybe_participant_pn(clone_or_take_last(
                            &mut notification.participant_pn,
                            is_last_action,
                        ))
                        .maybe_participant_username(clone_or_take_last(
                            &mut notification.participant_username,
                            is_last_action,
                        ))
                        .maybe_participant_country_code(clone_or_take_last(
                            &mut notification.participant_country_code,
                            is_last_action,
                        ))
                        .timestamp(timestamp)
                        .is_lid_addressing_mode(notification.is_lid_addressing_mode)
                        .has_incomplete_participant_information(
                            notification.has_incomplete_participant_information,
                        )
                        .action(Box::new(action))
                        .build(),
                )
            });
    }

    // Also dispatch legacy generic notification for backward compatibility
    client
        .core
        .event_bus
        .dispatch(Event::Notification(Arc::clone(&node)));
}

/// The server named these groups' cached metadata stale.
///
/// Dropping the group's snapshot is what every other staleness signal in this
/// file does (see the `Remove` / expired-cache arms above); the next send or
/// `group_info` query re-fetches. Doing nothing would leave a group's
/// participant list wrong for as long as the cache lives, which is a message
/// encrypted to a device set the group no longer has.
///
/// Two properties are in tension here, and the lane wins.
///
/// It goes through `lock_group_metadata` rather than dropping the cache entry
/// directly, because that lane *is* the invalidation protocol. A cold
/// `query_info` holds the guard across its IQ precisely so an invalidation
/// cannot be lost against it — with no cached `Arc` to compare, the lane is
/// the only thing that distinguishes "still absent" from "a notification
/// invalidated an already-absent snapshot", and it publishes its result
/// unconditionally. An eviction taken outside the lane is invisible to that
/// query, which then republishes membership it fetched before this
/// notification. (The warm path is safe either way: it publishes only when
/// `Arc::ptr_eq` says its snapshot is still current.)
///
/// It is spawned, because `Client::process_node` defers this stanza's
/// transport `<ack>` until the router's dispatch returns, and one notification
/// may name up to 10 000 groups — WA Web's parser bounds the `<group>`
/// children at `1..1e4`. A lane acquisition and a storage round trip each,
/// drained serially, is long enough for the server to retransmit and start the
/// work over.
///
/// That leaves a window between this returning and the task running, in which
/// a send can still read a stale snapshot. It is bounded by an uncontended
/// lane acquisition and the fan-out below, and it is strictly narrower than the official client's:
/// `handleGroupsDirtyNotificationJob` queues a persisted job and does not run
/// it until `waitForOfflineDeliveryEnd()` and `waitForConnection()` have both
/// resolved. Closing it entirely needs an invalidation generation the cache
/// does not have — see the PR discussion.
///
/// Deliberately unguarded, unlike the dirty-bit resync in `handlers/ib.rs`.
/// That one sends an IQ whose result belongs to the connection that asked for
/// it, so it checks the connection generation and `is_shutting_down`. Dropping
/// a snapshot is idempotent and correct on any generation, and
/// `is_shutting_down` would be actively wrong: it is `expected_disconnect ||
/// !is_running`, true between connections as well as at the end of one, so the
/// work would throw itself away exactly when a reconnect is about to make the
/// stale metadata reachable again.
fn handle_groups_dirty(client: &Arc<Client>, groups: Vec<wacore_binary::Jid>) {
    use futures::StreamExt;

    // Each group's work sits behind its own per-JID lane, so entries never
    // contend with one another; this bounds how many storage round trips — and,
    // for a custom cache store, remote ones — are in flight at once. Draining
    // serially would make a group's stale window the sum of every entry ahead
    // of it, which with the parser's 10 000-group ceiling is a tail measured in
    // whole round-trip multiples rather than one.
    const GROUPS_DIRTY_INVALIDATE_CONCURRENCY: usize = 16;

    let client = Arc::clone(client);
    client
        .clone()
        .runtime
        .spawn(Box::pin(async move {
            futures::stream::iter(groups)
                .map(|jid| {
                    let client = Arc::clone(&client);
                    async move {
                        debug!(
                            target: "Client/Group",
                            "groups_dirty: invalidating cached metadata for {}",
                            jid.observe()
                        );
                        client.lock_group_metadata(&jid).await.invalidate().await;
                    }
                })
                .buffer_unordered(GROUPS_DIRTY_INVALIDATE_CONCURRENCY)
                .for_each(|()| std::future::ready(()))
                .await;
        }))
        .detach();
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
                    .map(|r| {
                        NewsletterLiveUpdateReaction::builder()
                            .code(r.code)
                            .count(r.count)
                            .build()
                    })
                    .collect();

                Some(
                    NewsletterLiveUpdateMessage::builder()
                        .server_id(server_id)
                        .reactions(reactions)
                        .build(),
                )
            })
            .collect();

        if !messages.is_empty() {
            client.core.event_bus.dispatch(Event::NewsletterLiveUpdate(
                NewsletterLiveUpdate::builder()
                    .newsletter_jid(newsletter_jid)
                    .messages(messages)
                    .build(),
            ));
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
    let parsed = match update_node.content.as_ref() {
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
        .dispatch_with(EventKind::MexNotification, || {
            Event::MexNotification(
                MexNotification::builder()
                    .op_name(op_name)
                    .maybe_from(from)
                    .maybe_stanza_id(stanza_id)
                    .maybe_offline(offline)
                    .payload(payload)
                    .build(),
            )
        });
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
            wacore::types::events::DisappearingModeChanged::builder()
                .from(from)
                .duration(duration)
                .setting_timestamp(setting_timestamp)
                .build(),
        ));
}
