//! Status-broadcast participant assembly and privacy metadata.

use super::*;

/// Ensure the status stanza has a `<participants>` node listing all recipient
/// user JIDs. WhatsApp Web's `participantList` uses bare USER JIDs (not
/// device JIDs) -- `<to jid="user@s.whatsapp.net"/>` -- to tell the server
/// which users should receive the skmsg. The SKDM distribution list
/// (already in `<participants>`) uses device JIDs with `<enc>` children.
///
/// This is a pure function (no runtime or client dependencies).
pub fn ensure_status_participants(mut stanza: Node, group_info: &GroupRoutingInfo) -> Node {
    use wacore_binary::NodeContent;
    use wacore_binary::builder::NodeBuilder;

    let bare_to_node = |jid: &Jid| NodeBuilder::new("to").attr("jid", jid.to_non_ad()).build();

    // Check if <participants> already exists in the stanza children
    let children = match &mut stanza.content {
        Some(NodeContent::Nodes(nodes)) => nodes,
        _ => {
            stanza.content = Some(NodeContent::Nodes(vec![]));
            match &mut stanza.content {
                Some(NodeContent::Nodes(nodes)) => nodes,
                _ => unreachable!(),
            }
        }
    };

    if let Some(participants_node) = children.iter_mut().find(|n| n.tag == "participants") {
        // <participants> already exists (from SKDM distribution).
        // Add bare <to> user JID entries for users whose devices are NOT
        // already represented by SKDM device-level entries.
        let existing_users: HashSet<CompactString> = participants_node
            .children()
            .unwrap_or_default()
            .iter()
            .filter_map(|n| n.attrs.get("jid").and_then(|v| v.to_jid()).map(|j| j.user))
            .collect();

        let additions = group_info
            .participants
            .iter()
            .filter(|jid| !existing_users.contains(&jid.user))
            .map(bare_to_node);

        match &mut participants_node.content {
            Some(NodeContent::Nodes(nodes)) => nodes.extend(additions),
            content => {
                let nodes: Vec<_> = additions.collect();
                if !nodes.is_empty() {
                    *content = Some(NodeContent::Nodes(nodes));
                }
            }
        }
    } else {
        let participants_node = NodeBuilder::new("participants")
            .children(group_info.participants.iter().map(bare_to_node))
            .build();
        children.insert(0, participants_node);
    }

    stanza
}

/// True when a `status@broadcast` message should carry the
/// `<meta status_setting="..."/>` child. Only applies to actual status posts:
/// reactions (handled server-side as addons) and revokes must omit it, per
/// `WAWebEncryptAndSendStatusMsg` vs `WAWebSendReactionMsgAction`.
///
/// Descends `ephemeral_message` / `device_sent_message` / view-once wrappers
/// before classifying (same as `stanza_type_from_message`), so a reaction
/// nested inside a wrapper cannot slip past and re-trigger 479.
pub fn status_carries_privacy_meta(message: &wa::Message) -> bool {
    let msg = unwrap_message(message);
    let is_revoke = msg
        .protocol_message
        .as_option()
        .is_some_and(|pm| pm.r#type == Some(wa::message::protocol_message::Type::Revoke));
    let is_reaction = msg.reaction_message.is_set() || msg.enc_reaction_message.is_set();
    !is_revoke && !is_reaction
}

/// Return the message ID targeted by a status revoke, if this is a structurally
/// complete revoke protocol message.
///
/// The target ID belongs to the embedded protocol key. It is distinct from the
/// outer stanza ID generated for the revoke itself.
pub fn status_revoke_target_id(message: &wa::Message) -> Option<&str> {
    let protocol_message = unwrap_message(message).protocol_message.as_option()?;
    if protocol_message.r#type != Some(wa::message::protocol_message::Type::Revoke) {
        return None;
    }
    protocol_message.key.as_option()?.id.as_deref()
}

/// No status recipient resolved to an address.
#[derive(Debug, thiserror::Error)]
#[error("No valid status recipients after LID resolution")]
pub struct StatusRecipientsError;

/// Dedup a pre-resolved status recipient list by user, then anchor the sender's
/// own LID. Errors when no recipient was resolvable (matches WA Web's
/// `WAWebLidMigrationUtils.toUserLid` + `compactMap` dropping unresolvable
/// entries; an empty result means "nothing to send to").
///
/// Small audiences use a linear scan. Large audiences index the emitted users
/// so de-duplication preserves the first recipient without quadratic scans.
pub fn assemble_status_participants<I>(
    resolved: I,
    own_lid: &Jid,
) -> Result<Vec<Jid>, StatusRecipientsError>
where
    I: IntoIterator<Item = Option<Jid>>,
{
    let iter = resolved.into_iter();
    let (lower, _upper) = iter.size_hint();
    const LINEAR_LIMIT: usize = 64;
    let mut out: Vec<Jid> = Vec::with_capacity(lower.saturating_add(1));
    let mut recipients = iter.flatten();
    for jid in recipients.by_ref() {
        if !out.iter().any(|r| r.user == jid.user) {
            out.push(jid);
            if out.len() == LINEAR_LIMIT {
                break;
            }
        }
    }
    if out.len() == LINEAR_LIMIT
        && let Some(next) = recipients.next()
    {
        use std::hash::{BuildHasher, RandomState};

        let hash_builder = RandomState::new();
        let hash_user = |jid: &Jid| hash_builder.hash_one(&jid.user);
        let mut seen = hashbrown::HashTable::with_capacity(lower.max(out.len()));
        for (index, jid) in out.iter().enumerate() {
            seen.insert_unique(hash_user(jid), index, |&i| hash_user(&out[i]));
        }
        for jid in std::iter::once(next).chain(recipients) {
            let hash = hash_user(&jid);
            if seen.find(hash, |&i| out[i].user == jid.user).is_none() {
                seen.insert_unique(hash, out.len(), |&i| hash_user(&out[i]));
                out.push(jid);
            }
        }
    }
    if out.is_empty() {
        return Err(StatusRecipientsError);
    }
    if !out.iter().any(|r| r.user == own_lid.user) {
        out.push(own_lid.to_non_ad());
    }
    Ok(out)
}
