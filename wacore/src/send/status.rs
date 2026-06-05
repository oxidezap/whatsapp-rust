//! Status-broadcast participant assembly and privacy metadata.

use super::*;

/// Ensure the status stanza has a `<participants>` node listing all recipient
/// user JIDs. WhatsApp Web's `participantList` uses bare USER JIDs (not
/// device JIDs) -- `<to jid="user@s.whatsapp.net"/>` -- to tell the server
/// which users should receive the skmsg. The SKDM distribution list
/// (already in `<participants>`) uses device JIDs with `<enc>` children.
///
/// This is a pure function (no runtime or client dependencies).
pub fn ensure_status_participants(
    mut stanza: Node,
    group_info: &crate::client::context::GroupInfo,
) -> Node {
    use wacore_binary::NodeContent;
    use wacore_binary::builder::NodeBuilder;

    // Build bare <to jid="USER_JID"/> entries for each participant.
    // WhatsApp Web uses USER_JID (not DEVICE_JID) for the participantList.
    let bare_to_nodes: Vec<Node> = group_info
        .participants
        .iter()
        .map(|jid| NodeBuilder::new("to").attr("jid", jid.to_non_ad()).build())
        .collect();

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
        let existing_users: std::collections::HashSet<wacore_binary::CompactString> =
            participants_node
                .children()
                .unwrap_or_default()
                .iter()
                .filter_map(|n| n.attrs.get("jid").and_then(|v| v.to_jid()).map(|j| j.user))
                .collect();

        let new_to_nodes: Vec<Node> = bare_to_nodes
            .into_iter()
            .filter(|n| {
                n.attrs
                    .get("jid")
                    .and_then(|v| v.to_jid())
                    .is_some_and(|j| !existing_users.contains(&j.user))
            })
            .collect();

        if !new_to_nodes.is_empty() {
            match &mut participants_node.content {
                Some(NodeContent::Nodes(nodes)) => nodes.extend(new_to_nodes),
                _ => {
                    participants_node.content = Some(NodeContent::Nodes(new_to_nodes));
                }
            }
        }
    } else {
        // No <participants> node — create one with bare <to> entries.
        let participants_node = NodeBuilder::new("participants")
            .children(bare_to_nodes)
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
        .as_ref()
        .is_some_and(|pm| pm.r#type == Some(wa::message::protocol_message::Type::Revoke as i32));
    let is_reaction = msg.reaction_message.is_some() || msg.enc_reaction_message.is_some();
    !is_revoke && !is_reaction
}

/// Dedup a pre-resolved status recipient list by user, then anchor the sender's
/// own LID. Errors when no recipient was resolvable (matches WA Web's
/// `WAWebLidMigrationUtils.toUserLid` + `compactMap` dropping unresolvable
/// entries; an empty result means "nothing to send to").
///
/// Pure function: no allocations besides the returned `Vec` and (when needed)
/// the own-LID push. Dedup is a linear Vec scan — status lists stay small
/// enough that a HashSet is not worth its allocation.
pub fn assemble_status_participants<I>(resolved: I, own_lid: &Jid) -> anyhow::Result<Vec<Jid>>
where
    I: IntoIterator<Item = Option<Jid>>,
{
    let iter = resolved.into_iter();
    let (lower, _upper) = iter.size_hint();
    let mut out: Vec<Jid> = Vec::with_capacity(lower.saturating_add(1));
    for jid in iter.flatten() {
        if !out.iter().any(|r| r.user == jid.user) {
            out.push(jid);
        }
    }
    if out.is_empty() {
        anyhow::bail!("No valid status recipients after LID resolution");
    }
    if !out.iter().any(|r| r.user == own_lid.user) {
        out.push(own_lid.to_non_ad());
    }
    Ok(out)
}
