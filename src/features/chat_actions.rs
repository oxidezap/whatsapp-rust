//! Chat management via app state sync (syncd).
//!
//! ## Collections (from WhatsApp Web JS)
//! - `regular_low`: archive, pin, markChatAsRead
//! - `regular_high`: mute, star, deleteChat, deleteMessageForMe

use crate::appstate_sync::Mutation;
use crate::client::Client;
use anyhow::Result;
use log::debug;
use wacore::appstate::patch_decode::WAPatchName;
use wacore::appstate::schemas::{self, IndexPart, Schema};
use wacore::types::events::{
    ArchiveUpdate, ContactUpdate, DeleteChatUpdate, DeleteMessageForMeUpdate, Event,
    MarkChatAsReadUpdate, MuteUpdate, PinUpdate, StarUpdate,
};
use wacore_binary::{Jid, JidExt};
use waproto::whatsapp as wa;

/// WA Web uses `-1` for indefinite mute.
const MUTE_INDEFINITE: i64 = -1;

pub type SyncActionMessageRange = wa::sync_action_value::SyncActionMessageRange;

/// Enables multi-device conflict resolution. `None` is safe (matches whatsmeow/Baileys).
/// Only WA Web (with a full message DB) populates this.
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
                key: Some(key),
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
    if m.operation != wa::syncd_mutation::SyncdOperation::Set || m.index.is_empty() {
        return false;
    }

    let kind = &m.index[0];

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
                && let Some(act) = &val.mute_action
            {
                event_bus.dispatch(Event::MuteUpdate(MuteUpdate {
                    jid,
                    timestamp: time,
                    action: Box::new(*act),
                    from_full_sync: full_sync,
                }));
            }
            true
        }
        "pin" | "pin_v1" => {
            if let Some(val) = &m.action_value
                && let Some(act) = &val.pin_action
            {
                event_bus.dispatch(Event::PinUpdate(PinUpdate {
                    jid,
                    timestamp: time,
                    action: Box::new(*act),
                    from_full_sync: full_sync,
                }));
            }
            true
        }
        "archive" => {
            if let Some(val) = &m.action_value
                && let Some(act) = &val.archive_chat_action
            {
                event_bus.dispatch(Event::ArchiveUpdate(ArchiveUpdate {
                    jid,
                    timestamp: time,
                    action: Box::new(act.clone()),
                    from_full_sync: full_sync,
                }));
            }
            true
        }
        "star" => {
            if let Some(val) = &m.action_value
                && let Some(act) = &val.star_action
                && let Some((message_id, from_me, participant_jid)) =
                    parse_message_key_fields(kind, &m.index)
            {
                event_bus.dispatch(Event::StarUpdate(StarUpdate {
                    chat_jid: jid,
                    participant_jid,
                    message_id,
                    from_me,
                    timestamp: time,
                    action: Box::new(*act),
                    from_full_sync: full_sync,
                }));
            }
            true
        }
        "contact" => {
            if let Some(val) = &m.action_value
                && let Some(act) = &val.contact_action
            {
                event_bus.dispatch(Event::ContactUpdate(ContactUpdate {
                    jid,
                    timestamp: time,
                    action: Box::new(act.clone()),
                    from_full_sync: full_sync,
                }));
            }
            true
        }
        "mark_chat_as_read" | "markChatAsRead" => {
            if let Some(val) = &m.action_value
                && let Some(act) = &val.mark_chat_as_read_action
            {
                event_bus.dispatch(Event::MarkChatAsReadUpdate(MarkChatAsReadUpdate {
                    jid,
                    timestamp: time,
                    action: Box::new(act.clone()),
                    from_full_sync: full_sync,
                }));
            }
            true
        }
        "deleteChat" => {
            if let Some(val) = &m.action_value
                && let Some(act) = &val.delete_chat_action
            {
                // delete_media is in index[2], not in the proto (which only has messageRange)
                let delete_media = m.index.get(2).is_none_or(|v| v != "0");
                event_bus.dispatch(Event::DeleteChatUpdate(DeleteChatUpdate {
                    jid,
                    delete_media,
                    timestamp: time,
                    action: Box::new(act.clone()),
                    from_full_sync: full_sync,
                }));
            }
            true
        }
        "deleteMessageForMe" => {
            if let Some(val) = &m.action_value
                && let Some(act) = &val.delete_message_for_me_action
                && let Some((message_id, from_me, participant_jid)) =
                    parse_message_key_fields(kind, &m.index)
            {
                event_bus.dispatch(Event::DeleteMessageForMeUpdate(DeleteMessageForMeUpdate {
                    chat_jid: jid,
                    participant_jid,
                    message_id,
                    from_me,
                    timestamp: time,
                    action: Box::new(*act),
                    from_full_sync: full_sync,
                }));
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
    ) -> Result<()> {
        debug!("Archiving chat {jid}");
        self.send_archive_mutation(jid, true, message_range).await
    }

    pub async fn unarchive_chat(
        &self,
        jid: &Jid,
        message_range: Option<SyncActionMessageRange>,
    ) -> Result<()> {
        debug!("Unarchiving chat {jid}");
        self.send_archive_mutation(jid, false, message_range).await
    }

    pub async fn pin_chat(&self, jid: &Jid) -> Result<()> {
        debug!("Pinning chat {jid}");
        self.send_pin_mutation(jid, true).await
    }

    pub async fn unpin_chat(&self, jid: &Jid) -> Result<()> {
        debug!("Unpinning chat {jid}");
        self.send_pin_mutation(jid, false).await
    }

    pub async fn mute_chat(&self, jid: &Jid) -> Result<()> {
        debug!("Muting chat {jid} indefinitely");
        self.send_mute_mutation(jid, true, MUTE_INDEFINITE).await
    }

    /// Must be in the future. Use [`mute_chat`](Self::mute_chat) for indefinite.
    pub async fn mute_chat_until(&self, jid: &Jid, mute_end_timestamp_ms: i64) -> Result<()> {
        if mute_end_timestamp_ms <= 0 {
            anyhow::bail!(
                "mute_end_timestamp_ms must be a positive future timestamp (use mute_chat() for indefinite)"
            );
        }
        let now_ms = wacore::time::now_millis();
        if mute_end_timestamp_ms <= now_ms {
            anyhow::bail!(
                "mute_end_timestamp_ms is in the past ({mute_end_timestamp_ms} <= {now_ms})"
            );
        }
        debug!("Muting chat {jid} until {mute_end_timestamp_ms}");
        self.send_mute_mutation(jid, true, mute_end_timestamp_ms)
            .await
    }

    pub async fn unmute_chat(&self, jid: &Jid) -> Result<()> {
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
    ) -> Result<()> {
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
    ) -> Result<()> {
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
    ) -> Result<()> {
        debug!(
            "Marking chat {jid} as {}",
            if read { "read" } else { "unread" }
        );
        let value = wa::SyncActionValue {
            mark_chat_as_read_action: Some(wa::sync_action_value::MarkChatAsReadAction {
                read: Some(read),
                message_range,
            }),
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
    ) -> Result<()> {
        debug!("Deleting chat {jid}");
        let delete_media_str = if delete_media { "1" } else { "0" };
        let value = wa::SyncActionValue {
            delete_chat_action: Some(wa::sync_action_value::DeleteChatAction { message_range }),
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
    ) -> Result<()> {
        debug!("Deleting message {message_id} for me in {chat_jid}");
        let (chat, participant) = message_key_owned(chat_jid, participant_jid, from_me)?;
        let value = wa::SyncActionValue {
            delete_message_for_me_action: Some(wa::sync_action_value::DeleteMessageForMeAction {
                delete_media: Some(delete_media),
                message_timestamp,
            }),
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
    ) -> Result<()> {
        if !is_valid_contact_id(jid) {
            anyhow::bail!(
                "save_contact: contact id must be a bare phone-number JID (not a LID, group, or device-specific JID)"
            );
        }
        debug!("Saving contact {jid}");
        let value = wa::SyncActionValue {
            contact_action: Some(wa::sync_action_value::ContactAction {
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

    async fn send_archive_mutation(
        &self,
        jid: &Jid,
        archived: bool,
        message_range: Option<SyncActionMessageRange>,
    ) -> Result<()> {
        let value = wa::SyncActionValue {
            archive_chat_action: Some(wa::sync_action_value::ArchiveChatAction {
                archived: Some(archived),
                message_range,
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        let jid = jid.to_string();
        self.client
            .send_app_state_action(&schemas::ARCHIVE, &[jid.as_str()], &value)
            .await
    }

    async fn send_pin_mutation(&self, jid: &Jid, pinned: bool) -> Result<()> {
        let value = wa::SyncActionValue {
            pin_action: Some(wa::sync_action_value::PinAction {
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
    ) -> Result<()> {
        // -1 = indefinite, 0 = unmuted, positive = expiry ms
        let mute_end = if muted {
            Some(mute_end_timestamp_ms)
        } else {
            Some(0)
        };
        let value = wa::SyncActionValue {
            mute_action: Some(wa::sync_action_value::MuteAction {
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
    ) -> Result<()> {
        let (chat, participant) = message_key_owned(chat_jid, participant_jid, from_me)?;
        let value = wa::SyncActionValue {
            star_action: Some(wa::sync_action_value::StarAction {
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

    /// Encode a single `Set` app-state mutation (stamped with the action schema
    /// `version`) and send it as a patch on `collection`. Shared by the
    /// chat-action and label features.
    pub(crate) async fn send_app_state_mutation(
        &self,
        collection: WAPatchName,
        index: &[u8],
        value: &wa::SyncActionValue,
        version: i32,
    ) -> Result<()> {
        use rand::Rng;
        use wacore::appstate::encode::encode_record;

        let proc = self.get_app_state_processor().await;
        let key_id = proc
            .backend
            .get_latest_sync_key_id()
            .await
            .map_err(|e| anyhow::anyhow!(e))?
            .ok_or_else(|| anyhow::anyhow!("No app state sync key available"))?;
        let keys = proc.get_app_state_key(&key_id).await?;

        let mut iv = [0u8; 16];
        rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut iv);

        let (mutation, _) = encode_record(
            wa::syncd_mutation::SyncdOperation::Set,
            index,
            value,
            &keys,
            &key_id,
            &iv,
            version,
        );

        self.send_app_state_patch(collection.as_str(), vec![mutation])
            .await
    }

    /// Send any app-state (syncd) `Set` action, driven by a generated
    /// [`Schema`](wacore::appstate::schemas::Schema) from
    /// [`wacore::appstate::schemas`]. The collection, action version, and index
    /// shape come from the registry; the caller only fills the typed
    /// [`SyncActionValue`](wa::SyncActionValue) (its action sub-field, plus a
    /// `timestamp`) and supplies the non-literal index args in `index_parts`
    /// order. This is the generic escape hatch for actions without a dedicated
    /// helper (e.g. `clear_chat`, `favorites`, `quick_reply`); the typed APIs
    /// like [`ChatActions`] and [`Labels`](crate::Labels) wrap it.
    ///
    /// ```no_run
    /// # async fn ex(client: &whatsapp_rust::Client) -> anyhow::Result<()> {
    /// use whatsapp_rust::schemas;
    /// use whatsapp_rust::waproto::whatsapp as wa;
    /// let value = wa::SyncActionValue {
    ///     clear_chat_action: Some(Default::default()),
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
    ) -> Result<()> {
        let index = build_action_index(schema, index_args)?;
        let collection = collection_patch_name(schema.collection);
        self.send_app_state_mutation(collection, &index, value, schema.version as i32)
            .await
    }
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
