use crate::client::Client;
use crate::lid_pn_cache::LearningSource;
use crate::types::events::Event;
use log::{debug, warn};
use std::sync::Arc;
use wacore::types::events::{
    ContactNumberChanged, ContactSyncRequested, ContactUpdated, PictureUpdate, UserAboutUpdate,
};
use wacore_binary::Jid;
use wacore_binary::NodeContentRef;
use wacore_binary::NodeRef;

/// Handle profile picture change notifications.
///
/// Matches WhatsApp Web's `WAWebHandleProfilePicNotification`.
///
/// Structure:
/// ```xml
/// <notification type="picture" from="user@s.whatsapp.net" t="1234567890" id="...">
///   <set jid="user@s.whatsapp.net" id="pic_id" author="author@s.whatsapp.net"/>
/// </notification>
/// ```
///
/// Or for removal (no child or `<delete>` child):
/// ```xml
/// <notification type="picture" from="user@s.whatsapp.net" t="1234567890" id="...">
///   <delete jid="user@s.whatsapp.net"/>
/// </notification>
/// ```
pub(crate) fn handle_picture_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let from = crate::require_from_jid!(
        node,
        target: "Client/Picture",
        "picture notification"
    );

    let timestamp = notification_timestamp(node);

    // Look for <set>, <delete>, or <request> child to determine the action.
    // WhatsApp Web has two formats:
    // - With `jid` attr: direct update for that JID
    // - With `hash` attr (no `jid`): side contact, resolved via contact hash lookup
    let (jid, author, removed, picture_id) = if let Some(set_node) = node.get_optional_child("set")
    {
        let jid = set_node.attrs().optional_jid("jid").unwrap_or_else(|| {
            if set_node.attrs().optional_string("hash").is_some() {
                debug!(
                    target: "Client/Picture",
                    "Hash-based picture notification (no jid), using from={}", from.observe()
                );
            }
            from.clone()
        });
        let author = set_node.attrs().optional_jid("author");
        let pic_id = set_node
            .attrs()
            .optional_string("id")
            .map(|s| s.to_string());
        (jid, author, false, pic_id)
    } else if let Some(delete_node) = node.get_optional_child("delete") {
        let jid = delete_node
            .attrs()
            .optional_jid("jid")
            .unwrap_or_else(|| from.clone());
        let author = delete_node.attrs().optional_jid("author");
        (jid, author, true, None)
    } else {
        // No <set> or <delete> child. Check if notification has no children at all,
        // which WhatsApp uses as a deletion signal (bare notification).
        let children = node.children().map(|c| c.len()).unwrap_or(0);
        if children == 0 {
            let jid = node
                .attrs()
                .optional_jid("jid")
                .unwrap_or_else(|| from.clone());
            let author = node.attrs().optional_jid("author");
            (jid, author, true, None)
        } else {
            // Unknown child type (e.g., "request", "set_avatar") — log and skip
            let child_tag = node
                .children()
                .and_then(|c| c.first().map(|n| n.tag.as_ref()));
            debug!(
                target: "Client/Picture",
                "Ignoring picture notification with child {:?} from {}", child_tag, from.observe()
            );
            return;
        }
    };

    debug!(
        target: "Client/Picture",
        "Picture {}: jid={}, author={:?}, pic_id={:?}",
        if removed { "removed" } else { "updated" },
        jid.observe(), author, picture_id
    );

    let event = Event::PictureUpdate(
        PictureUpdate::builder()
            .jid(jid)
            .maybe_author(author)
            .timestamp(timestamp)
            .removed(removed)
            .maybe_picture_id(picture_id)
            .build(),
    );
    client.core.event_bus.dispatch(event);
}

/// Handle status/about text change notifications.
///
/// Matches WhatsApp Web's `WAWebHandleAboutNotification`.
///
/// Structure:
/// ```xml
/// <notification type="status" from="user@s.whatsapp.net" t="1234567890" notify="PushName">
///   <set>new status text</set>
/// </notification>
/// ```
pub(crate) fn handle_status_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let from = crate::require_from_jid!(
        node,
        target: "Client/Status",
        "status notification"
    );

    let timestamp = notification_timestamp(node);

    if let Some(set_node) = node.get_optional_child("set") {
        let status_text = match set_node.content.as_deref() {
            Some(NodeContentRef::String(s)) => s.to_string(),
            Some(NodeContentRef::Bytes(b)) => String::from_utf8_lossy(b.as_ref()).into_owned(),
            _ => String::new(),
        };

        debug!(
            target: "Client/Status",
            "Status update from {} (length={})", from.observe(), status_text.len()
        );

        let event = Event::UserAboutUpdate(
            UserAboutUpdate::builder()
                .jid(from)
                .status(status_text)
                .timestamp(timestamp)
                .build(),
        );
        client.core.event_bus.dispatch(event);
    } else {
        debug!(
            target: "Client/Status",
            "Status notification from {} without <set> child, ignoring", from.observe()
        );
    }
}

pub(crate) fn notification_timestamp(node: &NodeRef<'_>) -> chrono::DateTime<chrono::Utc> {
    node.attrs()
        .optional_u64("t")
        .and_then(|t| i64::try_from(t).ok())
        .and_then(wacore::time::from_secs)
        .unwrap_or_else(wacore::time::now_utc)
}

/// Learn LID-PN mappings from a contacts modify notification.
///
/// WA Web (`WAWebHandleContactNotification` → `WAWebDBCreateLidPnMappings`):
/// The `<modify>` child carries four attributes:
/// - `old` / `new` — old and new PN (phone number) JIDs
/// - `old_lid` / `new_lid` — old and new LID JIDs (optional)
///
/// When both `old_lid` and `new_lid` are present, WA Web creates two mappings:
/// `{ lid: old_lid, pn: old }` and `{ lid: new_lid, pn: new }`.
pub(crate) async fn learn_contact_modify_mappings(
    client: &Arc<Client>,
    old_pn: &Jid,
    new_pn: &Jid,
    old_lid: Option<&Jid>,
    new_lid: Option<&Jid>,
) {
    // WA Web: createLidPnMappings({mappings:[{lid:oldLid,pn:oldJid},{lid:newLid,pn:newJid}]})
    if let (Some(old_lid), Some(new_lid)) = (old_lid, new_lid) {
        for (lid, pn) in [(old_lid, old_pn), (new_lid, new_pn)] {
            if let Err(e) = client
                .add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::DeviceNotification)
                .await
            {
                warn!(
                    target: "Client/Contacts",
                    "Failed to add LID-PN mapping lid={} pn={}: {e}",
                    lid.observe(), pn.observe()
                );
            }
        }
    } else {
        debug!(
            target: "Client/Contacts",
            "Contacts modify without old_lid/new_lid, skipping LID-PN mapping (old={}, new={})",
            old_pn.observe(), new_pn.observe()
        );
    }
}

/// Handle contact change notifications.
///
/// WA Web: `WAWebHandleContactNotification`
///
/// These stanzas are sent as `<notification type="contacts">` with a single child action:
/// - `<update jid="..."/>` — contact profile changed. Consumers should
///   invalidate cached presence/profile picture (WA Web resets PresenceCollection
///   and refreshes profile pic thumb).
/// - `<modify old="..." new="..." old_lid="..." new_lid="..."/>` — contact
///   changed phone number. Creates LID-PN mappings when LID attrs present.
/// - `<sync after="..."/>` — server requests full contact re-sync.
/// - `<add .../>` or `<remove .../>` — lightweight roster changes (ACK only).
pub(crate) async fn handle_contacts_notification(client: &Arc<Client>, node: &NodeRef<'_>) {
    let timestamp = notification_timestamp(node);

    let Some(child) = node.children().and_then(|children| children.first()) else {
        debug!(
            target: "Client/Contacts",
            "Ignoring contacts notification without child action"
        );
        return;
    };

    match child.tag.as_ref() {
        "update" => {
            let Some(jid) = child.attrs().optional_jid("jid") else {
                // WA Web: when no jid, tries hash-based lookup against local contacts
                // (first 4 chars of contact userhash). If no match, it's a no-op.
                // We don't maintain a userhash index, so just ack and move on.
                debug!(target: "Client/Contacts", "contacts update with hash but no jid, ignoring (hash={:?})",
                    child.attrs().optional_string("hash"));
                return;
            };

            debug!(target: "Client/Contacts", "Contact updated for {}", jid.observe());
            client.core.event_bus.dispatch(Event::ContactUpdated(
                ContactUpdated::builder()
                    .jid(jid)
                    .timestamp(timestamp)
                    .build(),
            ));
        }
        "modify" => {
            // WA Web: old/new are PN JIDs, old_lid/new_lid are optional LID JIDs.
            let mut child_attrs = child.attrs();
            let Some(old_jid) = child_attrs.optional_jid("old") else {
                warn!(target: "Client/Contacts", "contacts modify missing 'old' attribute");
                return;
            };
            let Some(new_jid) = child_attrs.optional_jid("new") else {
                warn!(target: "Client/Contacts", "contacts modify missing 'new' attribute");
                return;
            };
            let old_lid = child_attrs.optional_jid("old_lid");
            let new_lid = child_attrs.optional_jid("new_lid");

            learn_contact_modify_mappings(
                client,
                &old_jid,
                &new_jid,
                old_lid.as_ref(),
                new_lid.as_ref(),
            )
            .await;

            debug!(
                target: "Client/Contacts",
                "Contact number changed: {} -> {} (old_lid={:?}, new_lid={:?})",
                old_jid.observe(), new_jid.observe(), old_lid, new_lid
            );
            client.core.event_bus.dispatch(Event::ContactNumberChanged(
                ContactNumberChanged::builder()
                    .old_jid(old_jid)
                    .new_jid(new_jid)
                    .maybe_old_lid(old_lid)
                    .maybe_new_lid(new_lid)
                    .timestamp(timestamp)
                    .build(),
            ));
        }
        "sync" => {
            let after = child
                .attrs()
                .optional_u64("after")
                .and_then(|after| wacore::time::from_secs(after as i64));

            debug!(
                target: "Client/Contacts",
                "Contact sync requested after {:?}",
                after
            );
            client.core.event_bus.dispatch(Event::ContactSyncRequested(
                ContactSyncRequested::builder()
                    .maybe_after(after)
                    .timestamp(timestamp)
                    .build(),
            ));
        }
        "add" | "remove" => {
            debug!(
                target: "Client/Contacts",
                "Contact {} notification handled without extra work",
                child.tag
            );
        }
        other => {
            debug!(
                target: "Client/Contacts",
                "Ignoring unknown contacts notification child {:?}",
                other
            );
        }
    }
}
