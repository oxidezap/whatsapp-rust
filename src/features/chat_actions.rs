//! Chat management via app state sync (syncd).
//!
//! ## Collections (from WhatsApp Web JS)
//! - `regular_low`: archive, pin, markChatAsRead
//! - `regular_high`: mute, star, deleteChat, deleteMessageForMe

use crate::appstate_sync::Mutation;
use crate::client::Client;
use anyhow::Result;
use log::debug;
use thiserror::Error;
use wacore::appstate::patch_decode::WAPatchName;
use wacore::appstate::schemas::{self, IndexPart, Schema};
use wacore::types::events::{
    ArchiveUpdate, ClearChatUpdate, ContactRemoved, ContactUpdate, DeleteChatUpdate,
    DeleteMessageForMeUpdate, Event, MarkChatAsReadUpdate, MuteUpdate, PinUpdate, StarUpdate,
    UserStatusMuteUpdate,
};
use wacore_binary::{Jid, JidExt};
use waproto::whatsapp as wa;

/// Error returned by app-state (syncd) requests — the shared failure domain of
/// chat actions ([`ChatActions`]), labels ([`crate::Labels`]) and re-syncs
/// ([`crate::Client::resync_app_state`]).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum AppStateError {
    /// The mutation arguments are invalid (e.g. a past mute timestamp, a
    /// non-phone-number contact id, a missing group participant, an empty
    /// label id).
    #[error("invalid app-state request: {0}")]
    InvalidRequest(String),
    /// The transport, not the request, is what stopped it: the client is
    /// shutting down, was never started, or lost the socket before the request
    /// could be sent or answered.
    #[error("no usable connection for the app-state request")]
    NotConnected,
    /// Encoding, key lookup, or sending the app-state patch failed.
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

/// WA Web uses `-1` for indefinite mute.
const MUTE_INDEFINITE: i64 = -1;

pub type SyncActionMessageRange = wa::sync_action_value::SyncActionMessageRange;

/// Enables multi-device conflict resolution. `None` is safe for clients without
/// a complete message database; callers with one can populate the range.
pub fn message_range(
    last_message_timestamp: i64,
    last_system_message_timestamp: Option<i64>,
    messages: Vec<(wa::MessageKey, i64)>,
) -> SyncActionMessageRange {
    SyncActionMessageRange {
        last_message_timestamp: Some(last_message_timestamp),
        last_system_message_timestamp,
        messages: messages
            .into_iter()
            .map(|(key, ts)| wa::sync_action_value::SyncActionMessage {
                key: buffa::MessageField::some(key),
                timestamp: Some(ts),
            })
            .collect(),
    }
}

pub fn message_key(
    id: impl Into<String>,
    remote_jid: &Jid,
    from_me: bool,
    participant: Option<&Jid>,
) -> wa::MessageKey {
    wa::MessageKey {
        id: Some(id.into()),
        remote_jid: Some(remote_jid.to_string()),
        from_me: Some(from_me),
        participant: participant.map(|j| j.to_string()),
    }
}

/// Returns `true` if handled, `false` if unknown (so other handlers can try).
pub(crate) fn dispatch_chat_mutation(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &Mutation,
    full_sync: bool,
) -> bool {
    if m.index.is_empty() {
        return false;
    }

    let kind = &m.index[0];

    // `contact` is the one chat action whose deletion arrives as a syncd
    // `Remove` (WAWebContactSync branches on `operation === "remove"`); every
    // other kind here encodes its "off" state inside a `Set` value, so a
    // non-`Set` operation on one of them is not ours to interpret.
    let is_contact_remove = m.operation == wa::syncd_mutation::SyncdOperation::REMOVE
        && kind == "contact"
        && m.index.len() > 1;
    if m.operation != wa::syncd_mutation::SyncdOperation::SET && !is_contact_remove {
        return false;
    }

    if !matches!(
        kind.as_str(),
        "mute"
            | "pin"
            | "pin_v1"
            | "archive"
            | "star"
            | "contact"
            | "mark_chat_as_read"
            | "markChatAsRead"
            | "deleteChat"
            | "clearChat"
            | "userStatusMute"
            | "deleteMessageForMe"
    ) {
        return false;
    }

    let ts = m
        .action_value
        .as_ref()
        .and_then(|v| v.timestamp)
        .unwrap_or(0);
    let time = wacore::time::from_millis_or_now(ts);
    let jid: Jid = if m.index.len() > 1 {
        match m.index[1].parse() {
            Ok(j) => j,
            Err(_) => {
                log::warn!(
                    "Skipping chat mutation '{}': malformed JID '{}'",
                    kind,
                    m.index[1]
                );
                return true;
            }
        }
    } else {
        log::warn!("Skipping chat mutation '{}': missing JID in index", kind);
        return true;
    };

    match kind.as_str() {
        "mute" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.mute_action.as_option()
            {
                event_bus.dispatch(Event::MuteUpdate(
                    MuteUpdate::builder()
                        .jid(jid)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "pin" | "pin_v1" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.pin_action.as_option()
            {
                event_bus.dispatch(Event::PinUpdate(
                    PinUpdate::builder()
                        .jid(jid)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "archive" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.archive_chat_action.as_option()
            {
                event_bus.dispatch(Event::ArchiveUpdate(
                    ArchiveUpdate::builder()
                        .jid(jid)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "star" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.star_action.as_option()
                && let Some((message_id, from_me, participant_jid)) =
                    parse_message_key_fields(kind, &m.index)
            {
                event_bus.dispatch(Event::StarUpdate(
                    StarUpdate::builder()
                        .chat_jid(jid)
                        .maybe_participant_jid(participant_jid)
                        .message_id(message_id)
                        .from_me(from_me)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "contact" if is_contact_remove => {
            event_bus.dispatch(Event::ContactRemoved(
                ContactRemoved::builder()
                    .jid(jid)
                    .timestamp(time)
                    .from_full_sync(full_sync)
                    .build(),
            ));
            true
        }
        "contact" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.contact_action.as_option()
            {
                event_bus.dispatch(Event::ContactUpdate(
                    ContactUpdate::builder()
                        .jid(jid)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "mark_chat_as_read" | "markChatAsRead" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.mark_chat_as_read_action.as_option()
            {
                event_bus.dispatch(Event::MarkChatAsReadUpdate(
                    MarkChatAsReadUpdate::builder()
                        .jid(jid)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "deleteChat" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.delete_chat_action.as_option()
            {
                // delete_media is in index[2], not in the proto (which only has messageRange)
                let delete_media = m.index.get(2).is_none_or(|v| v != "0");
                event_bus.dispatch(Event::DeleteChatUpdate(
                    DeleteChatUpdate::builder()
                        .jid(jid)
                        .delete_media(delete_media)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "clearChat" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.clear_chat_action.as_option()
            {
                // deleteStarred/deleteMedia live in the index (index[2]/index[3]),
                // not in ClearChatAction (which only has messageRange). WA Web's send
                // builder encodes both as "1"/"0".
                let delete_starred = m.index.get(2).is_some_and(|v| v == "1");
                let delete_media = m.index.get(3).is_some_and(|v| v == "1");
                event_bus.dispatch(Event::ClearChatUpdate(
                    ClearChatUpdate::builder()
                        .jid(jid)
                        .delete_starred(delete_starred)
                        .delete_media(delete_media)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "userStatusMute" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.user_status_mute_action.as_option()
            {
                event_bus.dispatch(Event::UserStatusMuteUpdate(
                    UserStatusMuteUpdate::builder()
                        .jid(jid)
                        .muted(act.muted.unwrap_or(false))
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "deleteMessageForMe" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.delete_message_for_me_action.as_option()
                && let Some((message_id, from_me, participant_jid)) =
                    parse_message_key_fields(kind, &m.index)
            {
                event_bus.dispatch(Event::DeleteMessageForMeUpdate(
                    DeleteMessageForMeUpdate::builder()
                        .chat_jid(jid)
                        .maybe_participant_jid(participant_jid)
                        .message_id(message_id)
                        .from_me(from_me)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        _ => false,
    }
}

/// Parse message-key fields (messageId, fromMe, participant) from index positions 2-4.
/// Returns `None` (with a warning log) if the index is too short or participant is malformed.
fn parse_message_key_fields(kind: &str, index: &[String]) -> Option<(String, bool, Option<Jid>)> {
    if index.len() < 5 {
        log::warn!(
            "Skipping {kind} mutation: expected 5 index elements, got {}",
            index.len()
        );
        return None;
    }
    let message_id = index[2].clone();
    let from_me = index[3] == "1";
    let participant_jid = if index[4] != "0" {
        match index[4].parse() {
            Ok(j) => Some(j),
            Err(_) => {
                log::warn!(
                    "Skipping {kind} mutation: malformed participant JID '{}'",
                    index[4]
                );
                return None;
            }
        }
    } else {
        None
    };
    Some((message_id, from_me, participant_jid))
}

/// Validate and own only the index args that must outlive the call: the chat JID
/// and (optional) participant JID. `messageId` and `fromMe` are passed through by
/// the caller without copying. Mirrors WAWebSyncdActionUtils.buildMessageKey.
fn message_key_owned(
    chat_jid: &Jid,
    participant_jid: Option<&Jid>,
    from_me: bool,
) -> Result<(String, Option<String>)> {
    // syncKeyToMsgKey rejects group non-fromMe without valid participant
    if chat_jid.is_group() && !from_me && participant_jid.is_none() {
        anyhow::bail!("participant_jid is required for group messages not sent by us");
    }
    Ok((chat_jid.to_string(), participant_jid.map(|j| j.to_string())))
}

/// The `"1"`/`"0"` wire string for a `fromMe` flag (no allocation).
#[inline]
fn bool_str(b: bool) -> &'static str {
    if b { "1" } else { "0" }
}

/// Assemble the JSON-array mutation index for `schema` from its non-literal index
/// args (in `schema.index_parts` order). The arg count must match.
/// A `contact` app-state mutation is keyed by a bare phone-number JID. Reject
/// LIDs (a separate WA Web path), group/status/broadcast/newsletter JIDs, and
/// AD/device JIDs (e.g. `123:4@s.whatsapp.net`) that would form an invalid index.
fn is_valid_contact_id(jid: &Jid) -> bool {
    jid.is_pn() && jid.device == 0
}

pub(crate) fn build_action_index(schema: &Schema, args: &[&str]) -> Result<Vec<u8>> {
    let non_literal = schema
        .index_parts
        .iter()
        .filter(|p| !matches!(p, IndexPart::Literal { .. }))
        .count();
    if args.len() != non_literal {
        anyhow::bail!(
            "index args for action '{}': expected {non_literal}, got {}",
            schema.name,
            args.len()
        );
    }
    let mut parts: Vec<&str> = Vec::with_capacity(schema.index_parts.len());
    let mut it = args.iter();
    for part in schema.index_parts {
        match part {
            IndexPart::Literal { value } => parts.push(value),
            _ => parts.push(it.next().expect("arg count checked above")),
        }
    }
    Ok(serde_json::to_vec(&parts)?)
}

/// Map a generated `Collection` to our `WAPatchName` (total — every generated
/// collection has a `WAPatchName` counterpart).
pub(crate) fn collection_patch_name(c: schemas::Collection) -> WAPatchName {
    use schemas::Collection;
    match c {
        Collection::Regular => WAPatchName::Regular,
        Collection::RegularLow => WAPatchName::RegularLow,
        Collection::RegularHigh => WAPatchName::RegularHigh,
        Collection::CriticalBlock => WAPatchName::CriticalBlock,
        Collection::CriticalUnblockLow => WAPatchName::CriticalUnblockLow,
    }
}

/// Access via `client.chat_actions()`.
pub struct ChatActions<'a> {
    client: &'a Client,
}

impl<'a> ChatActions<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    pub async fn archive_chat(
        &self,
        jid: &Jid,
        message_range: Option<SyncActionMessageRange>,
    ) -> Result<(), AppStateError> {
        debug!("Archiving chat {jid}");
        self.send_archive_mutation(jid, true, message_range).await
    }

    pub async fn unarchive_chat(
        &self,
        jid: &Jid,
        message_range: Option<SyncActionMessageRange>,
    ) -> Result<(), AppStateError> {
        debug!("Unarchiving chat {jid}");
        self.send_archive_mutation(jid, false, message_range).await
    }

    pub async fn pin_chat(&self, jid: &Jid) -> Result<(), AppStateError> {
        debug!("Pinning chat {jid}");
        self.send_pin_mutation(jid, true).await
    }

    pub async fn unpin_chat(&self, jid: &Jid) -> Result<(), AppStateError> {
        debug!("Unpinning chat {jid}");
        self.send_pin_mutation(jid, false).await
    }

    pub async fn mute_chat(&self, jid: &Jid) -> Result<(), AppStateError> {
        debug!("Muting chat {jid} indefinitely");
        self.send_mute_mutation(jid, true, MUTE_INDEFINITE).await
    }

    /// Must be in the future. Use [`mute_chat`](Self::mute_chat) for indefinite.
    pub async fn mute_chat_until(
        &self,
        jid: &Jid,
        mute_end_timestamp_ms: i64,
    ) -> Result<(), AppStateError> {
        if mute_end_timestamp_ms <= 0 {
            return Err(AppStateError::InvalidRequest(
                "mute_end_timestamp_ms must be a positive future timestamp (use mute_chat() for indefinite)".into(),
            ));
        }
        let now_ms = wacore::time::now_millis();
        if mute_end_timestamp_ms <= now_ms {
            return Err(AppStateError::InvalidRequest(format!(
                "mute_end_timestamp_ms is in the past ({mute_end_timestamp_ms} <= {now_ms})"
            )));
        }
        debug!("Muting chat {jid} until {mute_end_timestamp_ms}");
        self.send_mute_mutation(jid, true, mute_end_timestamp_ms)
            .await
    }

    pub async fn unmute_chat(&self, jid: &Jid) -> Result<(), AppStateError> {
        debug!("Unmuting chat {jid}");
        self.send_mute_mutation(jid, false, 0).await
    }

    /// `participant_jid`: required for group messages from others, `None` otherwise.
    pub async fn star_message(
        &self,
        chat_jid: &Jid,
        participant_jid: Option<&Jid>,
        message_id: &str,
        from_me: bool,
    ) -> Result<(), AppStateError> {
        debug!("Starring message {message_id} in {chat_jid}");
        self.send_star_mutation(chat_jid, participant_jid, message_id, from_me, true)
            .await
    }

    pub async fn unstar_message(
        &self,
        chat_jid: &Jid,
        participant_jid: Option<&Jid>,
        message_id: &str,
        from_me: bool,
    ) -> Result<(), AppStateError> {
        debug!("Unstarring message {message_id} in {chat_jid}");
        self.send_star_mutation(chat_jid, participant_jid, message_id, from_me, false)
            .await
    }

    /// Distinct from `readMessages` IQ receipts — this syncs state across linked devices.
    pub async fn mark_chat_as_read(
        &self,
        jid: &Jid,
        read: bool,
        message_range: Option<SyncActionMessageRange>,
    ) -> Result<(), AppStateError> {
        debug!(
            "Marking chat {jid} as {}",
            if read { "read" } else { "unread" }
        );
        let value = wa::SyncActionValue {
            mark_chat_as_read_action: buffa::MessageField::some(
                wa::sync_action_value::MarkChatAsReadAction {
                    read: Some(read),
                    message_range: message_range.into(),
                },
            ),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid = jid.to_string();
        self.client
            .send_app_state_action(&schemas::MARK_CHAT_AS_READ, &[jid.as_str()], &value)
            .await
    }

    pub async fn delete_chat(
        &self,
        jid: &Jid,
        delete_media: bool,
        message_range: Option<SyncActionMessageRange>,
    ) -> Result<(), AppStateError> {
        debug!("Deleting chat {jid}");
        let delete_media_str = if delete_media { "1" } else { "0" };
        let value = wa::SyncActionValue {
            delete_chat_action: buffa::MessageField::some(
                wa::sync_action_value::DeleteChatAction {
                    message_range: message_range.into(),
                },
            ),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid = jid.to_string();
        self.client
            .send_app_state_action(
                &schemas::DELETE_CHAT,
                &[jid.as_str(), delete_media_str],
                &value,
            )
            .await
    }

    /// Clears a chat's messages while keeping the chat (WA Web's clearChat).
    ///
    /// `delete_starred` also removes starred messages; `delete_media` also removes
    /// downloaded media. Both flags live only in the mutation index, not the proto.
    pub async fn clear_chat(
        &self,
        jid: &Jid,
        delete_starred: bool,
        delete_media: bool,
        message_range: Option<SyncActionMessageRange>,
    ) -> Result<(), AppStateError> {
        debug!("Clearing chat {jid}");
        // WA Web's $ClearChatSync$p_3 encodes both flags as "1"/"0".
        let delete_starred_str = if delete_starred { "1" } else { "0" };
        let delete_media_str = if delete_media { "1" } else { "0" };
        let value = wa::SyncActionValue {
            clear_chat_action: buffa::MessageField::some(wa::sync_action_value::ClearChatAction {
                message_range: message_range.into(),
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid = jid.to_string();
        self.client
            .send_app_state_action(
                &schemas::CLEAR_CHAT,
                &[jid.as_str(), delete_starred_str, delete_media_str],
                &value,
            )
            .await
    }

    /// Mute or unmute a contact/group/newsletter's status updates across devices
    /// (WA Web's userStatusMute). `muted = true` hides their status.
    pub async fn set_user_status_mute(&self, jid: &Jid, muted: bool) -> Result<(), AppStateError> {
        debug!("Setting userStatusMute for {jid} -> {muted}");
        let value = wa::SyncActionValue {
            user_status_mute_action: buffa::MessageField::some(
                wa::sync_action_value::UserStatusMuteAction { muted: Some(muted) },
            ),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid = jid.to_string();
        self.client
            .send_app_state_action(&schemas::USER_STATUS_MUTE, &[jid.as_str()], &value)
            .await
    }

    /// Deletes locally only (not for everyone).
    /// `participant_jid`: required for group messages from others, `None` otherwise.
    pub async fn delete_message_for_me(
        &self,
        chat_jid: &Jid,
        participant_jid: Option<&Jid>,
        message_id: &str,
        from_me: bool,
        delete_media: bool,
        message_timestamp: Option<i64>,
    ) -> Result<(), AppStateError> {
        debug!("Deleting message {message_id} for me in {chat_jid}");
        let (chat, participant) = message_key_owned(chat_jid, participant_jid, from_me)?;
        let value = wa::SyncActionValue {
            delete_message_for_me_action: buffa::MessageField::some(
                wa::sync_action_value::DeleteMessageForMeAction {
                    delete_media: Some(delete_media),
                    message_timestamp,
                },
            ),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        self.client
            .send_app_state_action(
                &schemas::DELETE_MESSAGE_FOR_ME,
                &[
                    chat.as_str(),
                    message_id,
                    bool_str(from_me),
                    participant.as_deref().unwrap_or("0"),
                ],
                &value,
            )
            .await
    }

    /// Save or rename a contact, syncing the name to the user's other linked devices.
    ///
    /// Writes a `contact` app-state SET mutation (WAWebContactSync.getContactSyncMutation)
    /// to the `critical_unblock_low` collection with index `["contact", jid]`.
    /// `full_name`/`first_name` are sent verbatim; an absent `first_name` is omitted
    /// (WA Web derives no short-name default). `save_on_primary_addressbook` controls
    /// whether it is saved to the phone's address book.
    ///
    /// The contact id must be a phone-number JID: WA Web refuses to send a contact
    /// mutation keyed by a LID (LID contacts use a separate path), so a LID is rejected.
    pub async fn save_contact(
        &self,
        jid: &Jid,
        full_name: Option<String>,
        first_name: Option<String>,
        save_on_primary_addressbook: bool,
    ) -> Result<(), AppStateError> {
        if !is_valid_contact_id(jid) {
            return Err(AppStateError::InvalidRequest(
                "save_contact: contact id must be a bare phone-number JID (not a LID, group, or device-specific JID)".into(),
            ));
        }
        debug!("Saving contact {jid}");
        let value = wa::SyncActionValue {
            contact_action: buffa::MessageField::some(wa::sync_action_value::ContactAction {
                full_name,
                first_name,
                save_on_primary_addressbook: Some(save_on_primary_addressbook),
                ..Default::default()
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid_str = jid.to_string();
        self.client
            .send_app_state_action(&schemas::CONTACT, &[jid_str.as_str()], &value)
            .await
    }

    /// Delete a saved contact, syncing the removal to the user's other linked
    /// devices.
    ///
    /// Writes the same `["contact", jid]` index as
    /// [`save_contact`](Self::save_contact) on `critical_unblock_low`, but as a
    /// syncd **`Remove`** — WA Web's `WAWebContactSync.getContactSyncMutation`
    /// selects the operation from its `isDelete` flag
    /// (`isDelete ? SyncdOperation.REMOVE : SyncdOperation.SET`) and its
    /// receiving side branches on `operation === "remove"` to drop the contact
    /// from the address book. A `Set` carrying an empty `ContactAction` would
    /// instead be applied as a rename to the empty string.
    ///
    /// The value is still sent: WA Web builds its `contactAction` before
    /// choosing the operation, so a removal carries an all-absent action.
    pub async fn remove_contact(&self, jid: &Jid) -> Result<(), AppStateError> {
        if !is_valid_contact_id(jid) {
            return Err(AppStateError::InvalidRequest(
                "remove_contact: contact id must be a bare phone-number JID (not a LID, group, or device-specific JID)".into(),
            ));
        }
        debug!("Removing contact {jid}");
        let value = wa::SyncActionValue {
            contact_action: buffa::MessageField::some(
                wa::sync_action_value::ContactAction::default(),
            ),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid_str = jid.to_string();
        self.client
            .remove_app_state_action(&schemas::CONTACT, &[jid_str.as_str()], &value)
            .await
    }

    async fn send_archive_mutation(
        &self,
        jid: &Jid,
        archived: bool,
        message_range: Option<SyncActionMessageRange>,
    ) -> Result<(), AppStateError> {
        let value = wa::SyncActionValue {
            archive_chat_action: buffa::MessageField::some(
                wa::sync_action_value::ArchiveChatAction {
                    archived: Some(archived),
                    message_range: message_range.into(),
                },
            ),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid = jid.to_string();
        self.client
            .send_app_state_action(&schemas::ARCHIVE, &[jid.as_str()], &value)
            .await
    }

    async fn send_pin_mutation(&self, jid: &Jid, pinned: bool) -> Result<(), AppStateError> {
        let value = wa::SyncActionValue {
            pin_action: buffa::MessageField::some(wa::sync_action_value::PinAction {
                pinned: Some(pinned),
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid = jid.to_string();
        self.client
            .send_app_state_action(&schemas::PIN, &[jid.as_str()], &value)
            .await
    }

    async fn send_mute_mutation(
        &self,
        jid: &Jid,
        muted: bool,
        mute_end_timestamp_ms: i64,
    ) -> Result<(), AppStateError> {
        // -1 = indefinite, 0 = unmuted, positive = expiry ms
        let mute_end = if muted {
            Some(mute_end_timestamp_ms)
        } else {
            Some(0)
        };
        let value = wa::SyncActionValue {
            mute_action: buffa::MessageField::some(wa::sync_action_value::MuteAction {
                muted: Some(muted),
                mute_end_timestamp: mute_end,
                ..Default::default()
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid = jid.to_string();
        self.client
            .send_app_state_action(&schemas::MUTE, &[jid.as_str()], &value)
            .await
    }

    async fn send_star_mutation(
        &self,
        chat_jid: &Jid,
        participant_jid: Option<&Jid>,
        message_id: &str,
        from_me: bool,
        starred: bool,
    ) -> Result<(), AppStateError> {
        let (chat, participant) = message_key_owned(chat_jid, participant_jid, from_me)?;
        let value = wa::SyncActionValue {
            star_action: buffa::MessageField::some(wa::sync_action_value::StarAction {
                starred: Some(starred),
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        self.client
            .send_app_state_action(
                &schemas::STAR,
                &[
                    chat.as_str(),
                    message_id,
                    bool_str(from_me),
                    participant.as_deref().unwrap_or("0"),
                ],
                &value,
            )
            .await
    }
}

impl Client {
    pub fn chat_actions(&self) -> ChatActions<'_> {
        ChatActions::new(self)
    }

    /// Encode a single app-state mutation (stamped with the action schema
    /// `version`) and send it as a patch on `collection`. Shared by the
    /// chat-action and label features.
    pub(crate) async fn send_app_state_mutation(
        &self,
        collection: WAPatchName,
        index: &[u8],
        value: &wa::SyncActionValue,
        version: i32,
        operation: wa::syncd_mutation::SyncdOperation,
    ) -> Result<(), AppStateError> {
        use rand::Rng;
        use wacore::appstate::encode::encode_record;

        let proc = self.get_app_state_processor();
        let key_id = proc
            .backend
            .get_latest_sync_key_id()
            .await
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| {
                AppStateError::InvalidRequest("no app state sync key available".into())
            })?;
        let keys = proc
            .get_app_state_key(&key_id)
            .await
            .map_err(|e| AppStateError::Internal(e.into()))?;

        let mut iv = [0u8; 16];
        rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut iv);

        let (mutation, _) = encode_record(operation, index, value, &keys, &key_id, &iv, version);

        self.send_app_state_patch(collection.as_str(), vec![mutation])
            .await?;
        Ok(())
    }

    /// Send any app-state (syncd) `Set` action, driven by a generated
    /// [`Schema`] from
    /// [`wacore::appstate::schemas`]. The collection, action version, and index
    /// shape come from the registry; the caller only fills the typed
    /// [`SyncActionValue`](wa::SyncActionValue) (its action sub-field, plus a
    /// `timestamp`) and supplies the non-literal index args in `index_parts`
    /// order. This is the generic escape hatch for actions without a dedicated
    /// helper (e.g. `clear_chat`, `favorites`, `quick_reply`); the typed APIs
    /// like [`ChatActions`] and [`Labels`](crate::Labels) wrap it.
    ///
    /// ```no_run
    /// # #![recursion_limit = "512"]
    /// # async fn ex(client: &whatsapp_rust::Client) -> anyhow::Result<()> {
    /// use whatsapp_rust::schemas;
    /// use whatsapp_rust::waproto::whatsapp as wa;
    /// let value = wa::SyncActionValue {
    ///     clear_chat_action: Some(Default::default()).into(),
    ///     timestamp: Some(1_700_000_000_000), // a real epoch-ms timestamp
    ///     ..Default::default()
    /// };
    /// // Args are the non-literal index parts in `schema.index_parts` order;
    /// // CLEAR_CHAT is [chatJid, deleteStarred, deleteMedia].
    /// client
    ///     .send_app_state_action(
    ///         &schemas::CLEAR_CHAT,
    ///         &["123@s.whatsapp.net", "0", "0"],
    ///         &value,
    ///     )
    ///     .await?;
    /// # Ok(()) }
    /// ```
    pub async fn send_app_state_action(
        &self,
        schema: &Schema,
        index_args: &[&str],
        value: &wa::SyncActionValue,
    ) -> Result<(), AppStateError> {
        self.send_app_state_action_op(
            schema,
            index_args,
            value,
            wa::syncd_mutation::SyncdOperation::SET,
        )
        .await
    }

    /// Send an app-state action as a syncd `Remove` rather than a `Set`.
    ///
    /// Only a few actions model deletion this way — most (labels, quick
    /// replies) carry a `deleted` flag inside a `Set` value instead, and sending
    /// a `Remove` for one of those would drop the record from the collection
    /// without the linked devices ever seeing the deletion. Check the action's
    /// WA Web builder before reaching for this;
    /// [`ChatActions::remove_contact`] is the one wrapped helper.
    ///
    /// `value` is still encoded and MAC'd (WA Web builds the same value struct
    /// for both branches of `getContactSyncMutation`), so pass the action's
    /// value type with the fields it would carry, plus a `timestamp`.
    pub async fn remove_app_state_action(
        &self,
        schema: &Schema,
        index_args: &[&str],
        value: &wa::SyncActionValue,
    ) -> Result<(), AppStateError> {
        self.send_app_state_action_op(
            schema,
            index_args,
            value,
            wa::syncd_mutation::SyncdOperation::REMOVE,
        )
        .await
    }

    async fn send_app_state_action_op(
        &self,
        schema: &Schema,
        index_args: &[&str],
        value: &wa::SyncActionValue,
        operation: wa::syncd_mutation::SyncdOperation,
    ) -> Result<(), AppStateError> {
        let index = build_action_index(schema, index_args)?;
        let collection = collection_patch_name(schema.collection);
        self.send_app_state_mutation(collection, &index, value, schema.version as i32, operation)
            .await
    }
}

/// Capture what an app-state verb actually puts on the wire.
///
/// Runs `send` against a mock transport with a seeded sync key, takes the
/// `<sync><collection><patch>` blob out of the first IQ it writes, and decodes
/// the single mutation inside it. The result is the same [`Mutation`] the
/// receiving device would see — operation, index and `SyncActionValue` — so a
/// test can assert on the wire form rather than on the builder that produced it.
#[cfg(test)]
pub(crate) async fn capture_app_state_mutation<F, Fut>(collection: &str, send: F) -> Mutation
where
    F: FnOnce(std::sync::Arc<Client>) -> Fut,
    Fut: Future<Output = Result<(), AppStateError>> + Send + 'static,
{
    use wacore::appstate::{decode_record, expand_app_state_keys};

    let (client, transport) = crate::test_utils::create_iq_test_client().await;
    let key_id = b"capture-key".to_vec();
    let backend = client.persistence_manager.backend();
    backend
        .set_sync_key(
            &key_id,
            crate::store::traits::AppStateSyncKey {
                key_data: vec![5u8; 32],
                ..Default::default()
            },
        )
        .await
        .expect("test backend should accept a sync key");
    backend
        .set_version(
            collection,
            wacore::appstate::hash::HashState {
                version: 7,
                ..Default::default()
            },
        )
        .await
        .expect("test backend should accept a version");

    // The verb blocks on the server's answer, which never comes; the patch it
    // wrote first is all this needs.
    let sending = tokio::spawn(send(std::sync::Arc::clone(&client)));
    let iq = crate::test_utils::decode_sent_iq(&transport, 0).await;
    sending.abort();

    let iq = iq.get().to_owned();
    let patch = iq
        .get_optional_child_by_tag(&["sync", "collection", "patch"])
        .expect("the verb should send a <patch>");
    let wacore_binary::NodeContent::Bytes(bytes) = patch.content.as_ref().expect("patch has bytes")
    else {
        panic!("a <patch> carries bytes");
    };
    let patch = waproto::codec::syncd_patch_decode(bytes).expect("the patch should decode");
    assert_eq!(patch.mutations.len(), 1, "verbs send one mutation each");
    let mutation = &patch.mutations[0];
    let operation = mutation
        .operation
        .expect("every mutation states its operation")
        .as_known()
        .expect("and it is a known one");
    let keys = expand_app_state_keys(&[5u8; 32]);
    let (decoded, _) = decode_record(
        operation,
        mutation.record.as_option().expect("mutation has a record"),
        &keys,
        &key_id,
        true,
    )
    .expect("the record we just wrote should decode and verify");
    decoded
}

#[cfg(test)]
mod registry_tests {
    use super::*;

    #[test]
    fn build_index_matches_legacy_shapes() {
        let cases: &[(&Schema, &[&str], &[&str])] = &[
            (
                &schemas::ARCHIVE,
                &["123@s.whatsapp.net"],
                &["archive", "123@s.whatsapp.net"],
            ),
            (
                &schemas::PIN,
                &["123@s.whatsapp.net"],
                &["pin_v1", "123@s.whatsapp.net"],
            ),
            (
                &schemas::MUTE,
                &["123@s.whatsapp.net"],
                &["mute", "123@s.whatsapp.net"],
            ),
            (
                &schemas::MARK_CHAT_AS_READ,
                &["123@s.whatsapp.net"],
                &["markChatAsRead", "123@s.whatsapp.net"],
            ),
            (
                &schemas::DELETE_CHAT,
                &["123@s.whatsapp.net", "1"],
                &["deleteChat", "123@s.whatsapp.net", "1"],
            ),
            (
                &schemas::CLEAR_CHAT,
                &["123@s.whatsapp.net", "0", "1"],
                &["clearChat", "123@s.whatsapp.net", "0", "1"],
            ),
            (
                &schemas::USER_STATUS_MUTE,
                &["123@s.whatsapp.net"],
                &["userStatusMute", "123@s.whatsapp.net"],
            ),
            (&schemas::LABEL_EDIT, &["5"], &["label_edit", "5"]),
            (
                &schemas::LABEL_JID,
                &["5", "123@s.whatsapp.net"],
                &["label_jid", "5", "123@s.whatsapp.net"],
            ),
            (
                &schemas::STAR,
                &["123@g.us", "MSGID", "1", "0"],
                &["star", "123@g.us", "MSGID", "1", "0"],
            ),
            (&schemas::SETTING_PUSH_NAME, &[], &["setting_pushName"]),
        ];
        for (schema, args, expected) in cases {
            assert_eq!(
                build_action_index(schema, args).unwrap(),
                serde_json::to_vec(expected).unwrap(),
                "index mismatch for {}",
                schema.name
            );
        }
    }

    #[test]
    fn build_index_rejects_arg_count_mismatch() {
        assert!(build_action_index(&schemas::ARCHIVE, &[]).is_err());
        assert!(build_action_index(&schemas::ARCHIVE, &["a", "b"]).is_err());
        assert!(build_action_index(&schemas::SETTING_PUSH_NAME, &["x"]).is_err());
    }

    #[test]
    fn registry_versions_match_whatsmeow() {
        // Locks the per-action versions the migration relies on (vs whatsmeow);
        // a regenerated registry that changes one will trip this for review.
        assert_eq!(schemas::MUTE.version, 2);
        assert_eq!(schemas::PIN.version, 5);
        assert_eq!(schemas::ARCHIVE.version, 3);
        assert_eq!(schemas::MARK_CHAT_AS_READ.version, 3);
        assert_eq!(schemas::STAR.version, 2);
        assert_eq!(schemas::CONTACT.version, 2);
        assert_eq!(schemas::DELETE_MESSAGE_FOR_ME.version, 3);
        assert_eq!(schemas::LABEL_EDIT.version, 3);
        assert_eq!(schemas::LABEL_JID.version, 3);
        assert_eq!(schemas::SETTING_PUSH_NAME.version, 1);
    }

    #[test]
    fn collection_mapping() {
        use schemas::Collection;
        // Every generated collection has a WAPatchName counterpart (total map).
        for c in [
            Collection::Regular,
            Collection::RegularLow,
            Collection::RegularHigh,
            Collection::CriticalBlock,
            Collection::CriticalUnblockLow,
        ] {
            // Round-trips through the wire name.
            assert_eq!(collection_patch_name(c).as_str(), c.as_str());
        }
    }

    #[test]
    fn contact_action_index_and_collection() {
        // WAWebContactSync writes ["contact", jid] to critical_unblock_low.
        let index = build_action_index(&schemas::CONTACT, &["5511999@s.whatsapp.net"]).unwrap();
        let parts: Vec<String> = serde_json::from_slice(&index).unwrap();
        assert_eq!(
            parts,
            vec!["contact".to_string(), "5511999@s.whatsapp.net".to_string()]
        );
        assert_eq!(
            collection_patch_name(schemas::CONTACT.collection),
            WAPatchName::CriticalUnblockLow
        );
    }

    /// The question this feature turns on: deleting a contact is a syncd
    /// `Remove`, not a `Set` carrying an empty `ContactAction`.
    ///
    /// WA Web's `WAWebContactSync.getContactSyncMutation` selects
    /// `isDelete ? SyncdOperation.REMOVE : SyncdOperation.SET` over the same
    /// `["contact", jid]` index, and its receiving side branches on
    /// `operation === "remove"` to drop the contact from the address book — a
    /// `Set` with empty names would be applied as a rename instead.
    #[tokio::test]
    async fn remove_contact_sends_a_remove_and_save_contact_a_set() {
        let jid: Jid = "12025550111@s.whatsapp.net".parse().expect("test JID");
        let collection = collection_patch_name(schemas::CONTACT.collection);

        let removed = capture_app_state_mutation(collection.as_str(), {
            let jid = jid.clone();
            move |client| async move { client.chat_actions().remove_contact(&jid).await }
        })
        .await;
        assert_eq!(
            removed.operation,
            wa::syncd_mutation::SyncdOperation::REMOVE,
            "contact deletion must be a Remove, not a Set with an empty action"
        );
        assert_eq!(removed.index, vec!["contact", "12025550111@s.whatsapp.net"]);

        let saved = capture_app_state_mutation(collection.as_str(), {
            let jid = jid.clone();
            move |client| async move {
                client
                    .chat_actions()
                    .save_contact(&jid, Some("Alex Doe".into()), Some("Alex".into()), true)
                    .await
            }
        })
        .await;
        assert_eq!(
            saved.operation,
            wa::syncd_mutation::SyncdOperation::SET,
            "saving must stay a Set — the two verbs differ by operation, not just by payload"
        );
        assert_eq!(saved.index, removed.index, "both key the same index");
        let action = saved
            .action_value
            .as_ref()
            .and_then(|v| v.contact_action.as_option())
            .expect("a save carries contactAction");
        assert_eq!(action.full_name.as_deref(), Some("Alex Doe"));
        assert_eq!(action.first_name.as_deref(), Some("Alex"));
        assert_eq!(action.save_on_primary_addressbook, Some(true));
    }

    /// A removal that comes back from a linked device must reach the consumer.
    /// Before this feature the Set-only gate in `dispatch_app_state_mutation`
    /// silently dropped it.
    #[tokio::test]
    async fn contact_removal_round_trips_to_an_event() {
        let jid: Jid = "12025550111@s.whatsapp.net".parse().expect("test JID");
        let mutation = capture_app_state_mutation(
            collection_patch_name(schemas::CONTACT.collection).as_str(),
            {
                let jid = jid.clone();
                move |client| async move { client.chat_actions().remove_contact(&jid).await }
            },
        )
        .await;

        let sent_at = mutation
            .action_value
            .as_ref()
            .and_then(|v| v.timestamp)
            .expect("the removal stamps its value with a timestamp");

        let (handled, events) = dispatch_into_recorder(&mutation);
        assert!(handled);
        assert_eq!(events.len(), 1);
        match &*events[0] {
            Event::ContactRemoved(u) => {
                assert_eq!(u.jid, jid);
                // The whole payload, not just the JID: the event carries the
                // mutation's own timestamp, not the moment it was dispatched.
                assert_eq!(u.timestamp, wacore::time::from_millis_or_now(sent_at));
                assert!(!u.from_full_sync, "this arrived as an incremental patch");
            }
            other => panic!("expected ContactRemoved, got {other:?}"),
        }

        // ...and a full-sync replay is marked as one.
        let (_, events) = dispatch_into_recorder_with(&mutation, true);
        match &*events[0] {
            Event::ContactRemoved(u) => assert!(u.from_full_sync),
            other => panic!("expected ContactRemoved, got {other:?}"),
        }
    }

    #[test]
    fn a_contact_set_still_dispatches_an_update_not_a_removal() {
        // The Remove arm must not swallow the Set one.
        let m = Mutation {
            index: vec!["contact".into(), "12025550111@s.whatsapp.net".into()],
            operation: wa::syncd_mutation::SyncdOperation::SET,
            action_value: Some(wa::SyncActionValue {
                contact_action: buffa::MessageField::some(wa::sync_action_value::ContactAction {
                    full_name: Some("Alex Doe".into()),
                    ..Default::default()
                }),
                ..Default::default()
            }),
        };
        let (handled, events) = dispatch_into_recorder(&m);
        assert!(handled);
        assert!(matches!(&*events[0], Event::ContactUpdate(_)));
    }

    #[test]
    fn a_remove_on_another_kind_is_not_claimed() {
        // Only `contact` models deletion as a Remove; a Remove on anything else
        // must fall through untouched rather than be read as its Set.
        for kind in ["mute", "pin", "archive", "star"] {
            let m = Mutation {
                index: vec![kind.into(), "12025550111@s.whatsapp.net".into()],
                operation: wa::syncd_mutation::SyncdOperation::REMOVE,
                action_value: Some(wa::SyncActionValue::default()),
            };
            let (handled, events) = dispatch_into_recorder(&m);
            assert!(!handled, "a Remove on '{kind}' must not be claimed");
            assert!(events.is_empty());
        }
    }

    #[tokio::test]
    async fn contact_verbs_reject_a_non_phone_number_id() {
        let client = crate::test_utils::create_test_client().await;
        let lid: Jid = "100000012345678@lid".parse().expect("test JID");
        assert!(client.chat_actions().remove_contact(&lid).await.is_err());
        assert!(
            client
                .chat_actions()
                .save_contact(&lid, None, None, false)
                .await
                .is_err()
        );
    }

    fn dispatch_into_recorder(m: &Mutation) -> (bool, Vec<std::sync::Arc<Event>>) {
        dispatch_into_recorder_with(m, false)
    }

    fn dispatch_into_recorder_with(
        m: &Mutation,
        full_sync: bool,
    ) -> (bool, Vec<std::sync::Arc<Event>>) {
        use std::sync::{Arc, Mutex};
        use wacore::types::events::{CoreEventBus, EventHandler, EventInterest};

        struct Recorder(Arc<Mutex<Vec<Arc<Event>>>>);
        impl EventHandler for Recorder {
            fn handle_event(&self, event: Arc<Event>) {
                self.0.lock().unwrap().push(event);
            }
            fn interest(&self) -> EventInterest {
                EventInterest::ALL
            }
        }

        let bus = CoreEventBus::new();
        let seen: Arc<Mutex<Vec<Arc<Event>>>> = Arc::new(Mutex::new(Vec::new()));
        bus.subscribe_handler(Arc::new(Recorder(seen.clone())))
            .detach();
        let handled = dispatch_chat_mutation(&bus, m, full_sync);
        let events = seen.lock().unwrap().clone();
        (handled, events)
    }

    #[test]
    fn contact_id_validation_accepts_only_bare_pn() {
        let valid = |s: &str| is_valid_contact_id(&s.parse::<Jid>().expect("test JID"));
        // bare PN -> accepted
        assert!(valid("5511999@s.whatsapp.net"));
        // AD/device-specific PN -> rejected (would form an invalid contact index)
        assert!(!valid("5511999:4@s.whatsapp.net"));
        // LID -> rejected (separate WA Web path)
        assert!(!valid("100000012345678@lid"));
        // group / newsletter / status -> rejected
        assert!(!valid("120363012345@g.us"));
        assert!(!valid("123@newsletter"));
        assert!(!valid("status@broadcast"));
    }
}
