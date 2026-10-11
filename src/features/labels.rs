//! Labels (etiquetas) via app state sync (syncd).
//!
//! Mirrors WhatsApp Web's `WAWebLabel*`. All label actions live in the
//! `regular` collection:
//! - `label_edit`    (index `["label_edit", labelId]`)         -> `LabelEditAction`
//! - `label_jid`     (index `["label_jid", labelId, chatJid]`) -> `LabelAssociationAction`
//! - `label_message` (index `["label_message", labelId, chatJid, messageId, fromMe, participant]`)
//!   -> `LabelAssociationAction`
//! - `label_sublist` (index `["label_sublist", predefinedId, chatJid]`)
//!   -> `LabelSublistUpdate` (inbound only; Set upserts, Remove deletes)
//!
//! Collection, action version, and index shape come from the generated
//! `schemas::{LABEL_EDIT, LABEL_JID}` registry, except `label_message`, which
//! WA Web names but no longer builds — see
//! [`schemas_unlisted::LABEL_MESSAGE`](wacore::appstate::schemas_unlisted::LABEL_MESSAGE).

use crate::appstate_sync::Mutation;
use crate::client::Client;
use crate::client::{AppStateDispatchOutcome, fingerprint_id, redact_jid};
use crate::features::chat_actions::AppStateError;
use log::debug;
use wacore::appstate::{schemas, schemas_unlisted};
use wacore::types::events::{
    Event, LabelAssociationUpdate, LabelEditUpdate, LabelSublistChange, LabelSublistUpdate,
    MessageLabelAssociationUpdate,
};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

/// Dispatch inbound label mutations synced from a linked device, returning the
/// [`crate::client::AppStateDispatchOutcome`] for the semantic per-mutation
/// log line.
pub(crate) fn dispatch_label_mutation_outcome(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &mut Mutation,
    event_full_sync: bool,
) -> AppStateDispatchOutcome {
    if m.index
        .first()
        .is_some_and(|kind| kind == schemas::LABEL_SUBLIST.name)
    {
        return dispatch_sublist_mutation(event_bus, m, event_full_sync);
    }
    if m.operation != wa::syncd_mutation::SyncdOperation::Set || m.index.is_empty() {
        return AppStateDispatchOutcome::Unclaimed;
    }

    let kind = m.index[0].as_str();
    if !matches!(kind, "label_edit" | "label_jid" | "label_message") {
        return AppStateDispatchOutcome::Unclaimed;
    }

    let ts = m.action_value.as_ref().and_then(|v| v.timestamp);
    let action_timestamp = ts.and_then(wacore::time::from_millis);
    // Preserve the legacy epoch fallback for absence and local dispatch time
    // for an unrepresentable value. Only `action_timestamp` preserves absence;
    // its presence does not distinguish replayed mutations from live changes.
    let time = wacore::time::from_millis_or_now(ts.unwrap_or(0));

    let Some(label_id) = m.index.get(1).cloned() else {
        log::warn!("Skipping label mutation '{kind}': missing label id in index");
        return AppStateDispatchOutcome::Skipped("missing-label-id");
    };

    match kind {
        "label_edit" => {
            if let Some(val) = &mut m.action_value
                && let Some(act) = val.label_edit_action.take()
            {
                event_bus.dispatch(Event::LabelEditUpdate(
                    LabelEditUpdate::builder()
                        .label_id(label_id)
                        .timestamp(time)
                        .maybe_action_timestamp(action_timestamp)
                        .action(Box::new(act))
                        .from_full_sync(event_full_sync)
                        .build(),
                ));
                AppStateDispatchOutcome::Event("LabelEditUpdate")
            } else {
                AppStateDispatchOutcome::Malformed("LabelEditUpdate")
            }
        }
        "label_message" => {
            let Some(chat_jid) = parse_association_chat_jid(kind, &m.index) else {
                return AppStateDispatchOutcome::Skipped("bad-chat-jid");
            };
            // Empty is as unusable as absent: the id is what the association
            // hangs off, and an event carrying "" points at no message. The
            // outbound side rejects it for the same reason.
            let Some(message_id) = m.index.get(3).filter(|id| !id.is_empty()).cloned() else {
                log::warn!("Skipping label_message mutation: missing or empty message id in index");
                return AppStateDispatchOutcome::Skipped("missing-message-id");
            };
            if let Some(val) = &mut m.action_value
                && let Some(act) = val.label_association_action.take()
            {
                event_bus.dispatch(Event::MessageLabelAssociationUpdate(
                    MessageLabelAssociationUpdate::builder()
                        .label_id(label_id)
                        .chat_jid(chat_jid)
                        .message_id(message_id)
                        .timestamp(time)
                        .maybe_action_timestamp(action_timestamp)
                        .action(Box::new(act))
                        .from_full_sync(event_full_sync)
                        .build(),
                ));
                AppStateDispatchOutcome::Event("MessageLabelAssociationUpdate")
            } else {
                AppStateDispatchOutcome::Malformed("MessageLabelAssociationUpdate")
            }
        }
        "label_jid" => {
            let Some(chat_jid) = parse_association_chat_jid(kind, &m.index) else {
                return AppStateDispatchOutcome::Skipped("bad-chat-jid");
            };
            if let Some(val) = &mut m.action_value
                && let Some(act) = val.label_association_action.take()
            {
                event_bus.dispatch(Event::LabelAssociationUpdate(
                    LabelAssociationUpdate::builder()
                        .label_id(label_id)
                        .chat_jid(chat_jid)
                        .timestamp(time)
                        .maybe_action_timestamp(action_timestamp)
                        .action(Box::new(act))
                        .from_full_sync(event_full_sync)
                        .build(),
                ));
                AppStateDispatchOutcome::Event("LabelAssociationUpdate")
            } else {
                AppStateDispatchOutcome::Malformed("LabelAssociationUpdate")
            }
        }
        _ => AppStateDispatchOutcome::Unclaimed,
    }
}

fn dispatch_sublist_mutation(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &Mutation,
    event_full_sync: bool,
) -> AppStateDispatchOutcome {
    // WAWebLabelSublistSync keys an upsert/removal by predefinedId + chatJid.
    // Use the signed integer namespace of LabelEditAction.predefinedId, not
    // ListType or an allowlist of the currently known predefined IDs.
    let Some(predefined_id) = m.index.get(1).and_then(|id| id.parse::<i32>().ok()) else {
        return AppStateDispatchOutcome::Skipped("bad-predefined-id");
    };
    let Some(chat_jid) = parse_association_chat_jid("label_sublist", &m.index) else {
        return AppStateDispatchOutcome::Skipped("bad-chat-jid");
    };
    let change = if m.operation == wa::syncd_mutation::SyncdOperation::Remove {
        // Remove's builder carries an empty action value. Never require a
        // subListId here or mistake Set(subListId=0) for this operation.
        LabelSublistChange::Remove
    } else if m.operation == wa::syncd_mutation::SyncdOperation::Set {
        let Some(sub_list_id) = m
            .action_value
            .as_ref()
            .and_then(|value| value.label_sublist_action.as_option())
            .and_then(|action| action.sub_list_id)
        else {
            return AppStateDispatchOutcome::Malformed("LabelSublistUpdate");
        };
        LabelSublistChange::Upsert { sub_list_id }
    } else {
        return AppStateDispatchOutcome::Unclaimed;
    };
    let action_timestamp = m
        .action_value
        .as_ref()
        .and_then(|value| value.timestamp)
        .and_then(wacore::time::from_millis);
    event_bus.dispatch(Event::LabelSublistUpdate(
        LabelSublistUpdate::builder()
            .predefined_id(predefined_id)
            .chat_jid(chat_jid)
            .change(change)
            .maybe_action_timestamp(action_timestamp)
            .from_full_sync(event_full_sync)
            .build(),
    ));
    AppStateDispatchOutcome::Event("LabelSublistUpdate")
}

/// Association and sublist actions carry the chat JID at index position 2. Returns
/// `None` (with a warning) when it is missing or unparseable, so the caller can
/// claim the mutation without emitting a half-formed event.
fn parse_association_chat_jid(kind: &str, index: &[String]) -> Option<Jid> {
    match index.get(2) {
        Some(s) => match s.parse() {
            Ok(jid) => Some(jid),
            Err(_) => {
                // Fingerprinted: `s` failed JID validation, so it is
                // untrusted wire input — never logged verbatim.
                log::warn!(
                    "Skipping {kind} mutation: malformed chat JID ({})",
                    fingerprint_id(s)
                );
                None
            }
        },
        None => {
            log::warn!("Skipping {kind} mutation: missing chat JID in index");
            None
        }
    }
}

/// Access via `client.labels()`.
pub struct Labels<'a> {
    client: &'a Client,
}

impl<'a> Labels<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Create or update a label. App state is an upsert keyed by `label_id`, so
    /// this both creates a new label and renames/recolors an existing one.
    /// `color` is a WhatsApp color index.
    pub async fn create_label(
        &self,
        label_id: &str,
        name: &str,
        color: i32,
    ) -> Result<(), AppStateError> {
        if label_id.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "label_id cannot be empty".into(),
            ));
        }
        if name.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "label name cannot be empty".into(),
            ));
        }
        // Don't log the label name (user content); the fingerprinted id/color are enough to trace.
        debug!(
            "Setting label {} (name_len={}, color={color})",
            fingerprint_id(label_id),
            name.len()
        );
        let value = wa::SyncActionValue {
            label_edit_action: buffa::MessageField::some(wa::sync_action_value::LabelEditAction {
                name: Some(name.to_string()),
                color: Some(color),
                deleted: Some(false),
                ..Default::default()
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        self.client
            .send_app_state_action(&schemas::LABEL_EDIT, &[label_id], &value)
            .await
    }

    /// Delete a label. Chats keep their association rows; WA Web prunes them
    /// from the local DB on receipt of the delete.
    pub async fn delete_label(&self, label_id: &str) -> Result<(), AppStateError> {
        if label_id.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "label_id cannot be empty".into(),
            ));
        }
        debug!("Deleting label {}", fingerprint_id(label_id));
        let value = wa::SyncActionValue {
            label_edit_action: buffa::MessageField::some(wa::sync_action_value::LabelEditAction {
                deleted: Some(true),
                ..Default::default()
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        self.client
            .send_app_state_action(&schemas::LABEL_EDIT, &[label_id], &value)
            .await
    }

    /// Associate a label with a chat.
    pub async fn add_chat_label(
        &self,
        label_id: &str,
        chat_jid: &Jid,
    ) -> Result<(), AppStateError> {
        self.send_association(label_id, chat_jid, true).await
    }

    /// Remove a label association from a chat.
    pub async fn remove_chat_label(
        &self,
        label_id: &str,
        chat_jid: &Jid,
    ) -> Result<(), AppStateError> {
        self.send_association(label_id, chat_jid, false).await
    }

    /// Associate a label with a single message.
    ///
    /// Distinct from [`add_chat_label`](Self::add_chat_label): the association
    /// is keyed by the message as well as the chat, under the `label_message`
    /// action. WA Web no longer builds this mutation (its action table lists
    /// only the chat association), so the schema is declared out of the
    /// generated registry — see
    /// [`schemas_unlisted::LABEL_MESSAGE`](wacore::appstate::schemas_unlisted::LABEL_MESSAGE)
    /// for the evidence behind its collection, version and index.
    ///
    /// One message per mutation, mirroring the wire: every message-scoped syncd
    /// action keys on a single message key, so labelling several messages means
    /// calling this once each.
    pub async fn add_message_label(
        &self,
        label_id: &str,
        chat_jid: &Jid,
        message_id: &crate::MessageId,
    ) -> Result<(), AppStateError> {
        self.send_message_association(label_id, chat_jid, message_id, true)
            .await
    }

    /// Remove a label association from a single message.
    pub async fn remove_message_label(
        &self,
        label_id: &str,
        chat_jid: &Jid,
        message_id: &crate::MessageId,
    ) -> Result<(), AppStateError> {
        self.send_message_association(label_id, chat_jid, message_id, false)
            .await
    }

    async fn send_association(
        &self,
        label_id: &str,
        chat_jid: &Jid,
        labeled: bool,
    ) -> Result<(), AppStateError> {
        if label_id.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "label_id cannot be empty".into(),
            ));
        }
        debug!(
            "{} label {} {} chat {}",
            if labeled { "Adding" } else { "Removing" },
            fingerprint_id(label_id),
            if labeled { "to" } else { "from" },
            redact_jid(chat_jid),
        );
        let chat = chat_jid.to_string();
        self.client
            .send_app_state_action(
                &schemas::LABEL_JID,
                &[label_id, chat.as_str()],
                &association_value(labeled),
            )
            .await
    }

    async fn send_message_association(
        &self,
        label_id: &str,
        chat_jid: &Jid,
        message_id: &crate::MessageId,
        labeled: bool,
    ) -> Result<(), AppStateError> {
        if label_id.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "label_id cannot be empty".into(),
            ));
        }
        debug!(
            "{} label {} {} message {} in {}",
            if labeled { "Adding" } else { "Removing" },
            fingerprint_id(label_id),
            if labeled { "to" } else { "from" },
            fingerprint_id(message_id.as_str()),
            redact_jid(chat_jid),
        );
        let chat = chat_jid.to_string();
        self.client
            .send_app_state_action(
                &schemas_unlisted::LABEL_MESSAGE,
                // The message-key tail is pinned to its defaults: no source —
                // WA Web's protobuf action table, whatsmeow, or Baileys — shows
                // `label_message` carrying a set `fromMe` or a participant, and
                // guessing one would key the association off a row the server
                // does not have.
                &[label_id, chat.as_str(), message_id.as_str(), "0", "0"],
                &association_value(labeled),
            )
            .await
    }
}

fn association_value(labeled: bool) -> wa::SyncActionValue {
    wa::SyncActionValue {
        label_association_action: buffa::MessageField::some(
            wa::sync_action_value::LabelAssociationAction {
                labeled: Some(labeled),
                ..Default::default()
            },
        ),
        timestamp: Some(wacore::time::now_millis()),
        ..Default::default()
    }
}

impl Client {
    pub fn labels(&self) -> Labels<'_> {
        Labels::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::chat_actions::capture_app_state_mutation as capture;
    use std::sync::{Arc, Mutex};
    use wacore::appstate::schemas_unlisted::LABEL_MESSAGE;
    use wacore::types::events::{CoreEventBus, EventHandler, EventInterest};

    #[derive(Default)]
    struct Recorder {
        events: Mutex<Vec<Arc<Event>>>,
    }
    impl EventHandler for Recorder {
        fn handle_event(&self, event: Arc<Event>) {
            self.events.lock().unwrap().push(event);
        }
        fn interest(&self) -> EventInterest {
            EventInterest::ALL
        }
    }

    fn set_mutation(index: Vec<&str>, value: wa::SyncActionValue) -> Mutation {
        Mutation {
            index: index.into_iter().map(String::from).collect(),
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(value),
        }
    }

    fn run(m: &Mutation) -> (AppStateDispatchOutcome, Vec<Arc<Event>>) {
        let bus = CoreEventBus::new();
        let rec = Arc::new(Recorder::default());
        bus.subscribe_handler(rec.clone()).detach();
        let outcome = dispatch_label_mutation_outcome(&bus, &mut m.clone(), false);
        let events = rec.events.lock().unwrap().clone();
        (outcome, events)
    }

    fn sublist_mutation(sub_list_id: Option<i32>) -> Mutation {
        // WAWebLabelSublistSync, verified WA 2.3000.1045368834:
        // regular/version 1, [label_sublist, predefinedId, chatJid].
        // predefinedId 11 is the lead parent, not ListType::AiResponding.
        let mut value = wa::SyncActionValue::default();
        value
            .label_sublist_action
            .get_or_insert_default()
            .sub_list_id = sub_list_id;
        set_mutation(
            vec!["label_sublist", "11", "12025550111@s.whatsapp.net"],
            value,
        )
    }

    #[test]
    fn label_sublist_preserves_ids_timestamps_and_sync_provenance() {
        for sub_list_id in [i32::MIN, 0, 2, i32::MAX] {
            for raw_timestamp in [
                None,
                Some(0),
                Some(1_700_000_000_123),
                Some(i64::MIN),
                Some(i64::MAX),
            ] {
                for full_sync in [false, true] {
                    let mut mutation = sublist_mutation(Some(sub_list_id));
                    mutation.index[1] = "2147483647".into();
                    mutation.index[2] = "120363000000000042@lid".into();
                    mutation.action_value.as_mut().unwrap().timestamp = raw_timestamp;
                    let bus = CoreEventBus::new();
                    let recorder = Arc::new(Recorder::default());
                    let _subscription = bus.subscribe_handler(recorder.clone());
                    for operation in [
                        wa::syncd_mutation::SyncdOperation::Set,
                        wa::syncd_mutation::SyncdOperation::Remove,
                    ] {
                        mutation.operation = operation;
                        assert_eq!(
                            dispatch_label_mutation_outcome(&bus, &mut mutation, full_sync),
                            AppStateDispatchOutcome::Event("LabelSublistUpdate")
                        );
                    }
                    let events = recorder.events.lock().unwrap();
                    assert_eq!(events.len(), 2);
                    for (index, event) in events.iter().enumerate() {
                        let Event::LabelSublistUpdate(update) = &**event else {
                            panic!("wrong event: {event:?}")
                        };
                        assert_eq!(update.predefined_id, i32::MAX);
                        assert_eq!(update.chat_jid.to_string(), "120363000000000042@lid");
                        assert_eq!(
                            update.action_timestamp,
                            raw_timestamp.and_then(wacore::time::from_millis)
                        );
                        assert_eq!(update.from_full_sync, full_sync);
                        assert_eq!(
                            update.change,
                            if index == 0 {
                                LabelSublistChange::Upsert { sub_list_id }
                            } else {
                                LabelSublistChange::Remove
                            }
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn label_sublist_set_requires_a_value_but_remove_does_not() {
        let mut absent = sublist_mutation(None);
        let mut no_action = absent.clone();
        no_action.action_value = Some(wa::SyncActionValue::default());
        let mut no_value = absent.clone();
        no_value.action_value = None;
        for mutation in [&mut absent, &mut no_action, &mut no_value] {
            let (outcome, events) = run(mutation);
            assert_eq!(
                outcome,
                AppStateDispatchOutcome::Malformed("LabelSublistUpdate")
            );
            assert!(events.is_empty());
            mutation.operation = wa::syncd_mutation::SyncdOperation::Remove;
            let (outcome, events) = run(mutation);
            assert_eq!(
                outcome,
                AppStateDispatchOutcome::Event("LabelSublistUpdate")
            );
            let Event::LabelSublistUpdate(update) = &*events[0] else {
                panic!("wrong event")
            };
            assert_eq!(update.change, LabelSublistChange::Remove);
            assert_eq!(update.action_timestamp, None);
        }
    }

    #[test]
    fn label_sublist_rejects_unusable_keys_without_partial_events() {
        for index in [
            vec!["label_sublist"],
            vec!["label_sublist", ""],
            vec!["label_sublist", "NaN", "12025550111@s.whatsapp.net"],
            vec!["label_sublist", "2147483648", "12025550111@s.whatsapp.net"],
            vec!["label_sublist", "1.5", "12025550111@s.whatsapp.net"],
            vec!["label_sublist", "11"],
            vec!["label_sublist", "11", ""],
            vec!["label_sublist", "11", "not a jid"],
        ] {
            for operation in [
                wa::syncd_mutation::SyncdOperation::Set,
                wa::syncd_mutation::SyncdOperation::Remove,
            ] {
                let mut mutation = sublist_mutation(Some(2));
                mutation.index = index.iter().map(|s| (*s).into()).collect();
                mutation.operation = operation;
                let (outcome, events) = run(&mutation);
                assert!(
                    matches!(outcome, AppStateDispatchOutcome::Skipped(_)),
                    "{index:?}"
                );
                assert!(events.is_empty(), "{index:?}");
            }
        }
    }

    #[test]
    fn label_sublist_replay_replaces_then_removes_without_label_assignment() {
        let mut state = std::collections::HashMap::new();
        for sub_list_id in [Some(1), Some(1), Some(0), Some(-1), None, None] {
            let mut mutation = sublist_mutation(sub_list_id);
            // As with existing label handlers, future index suffixes do not
            // erase the fields this version understands.
            mutation.index.push("future-extension".into());
            if sub_list_id.is_none() {
                mutation.operation = wa::syncd_mutation::SyncdOperation::Remove;
                mutation.action_value = None;
            }
            let (_, events) = run(&mutation);
            assert_eq!(events.len(), 1);
            let Event::LabelSublistUpdate(update) = &*events[0] else {
                panic!("must not fabricate a parent-label event")
            };
            let key = (update.predefined_id, update.chat_jid.clone());
            match update.change {
                LabelSublistChange::Upsert { sub_list_id } => {
                    state.insert(key.clone(), sub_list_id);
                }
                LabelSublistChange::Remove => {
                    state.remove(&key);
                }
                _ => panic!("unexpected future sublist change"),
            }
            assert_eq!(state.get(&key).copied(), sub_list_id);
        }
        for kind in [
            "label_jid",
            "label_edit",
            "label_message",
            "label_reordering",
        ] {
            let mut mutation = sublist_mutation(Some(2));
            mutation.index[0] = kind.into();
            mutation.operation = wa::syncd_mutation::SyncdOperation::Remove;
            let (outcome, events) = run(&mutation);
            assert_eq!(outcome, AppStateDispatchOutcome::Unclaimed);
            assert!(events.is_empty());
        }
    }

    #[test]
    fn label_timestamps_preserve_presence_and_legacy_fallbacks() {
        let epoch = wacore::time::from_millis(0).unwrap();
        let valid = wacore::time::from_millis(1_700_000_000_123).unwrap();
        for (raw, expected) in [
            (None, None),
            (Some(0), Some(epoch)),
            (Some(1_700_000_000_123), Some(valid)),
            (Some(i64::MIN), None),
            (Some(i64::MAX), None),
        ] {
            for from_full_sync in [false, true] {
                for index in [
                    vec!["label_edit", "5"],
                    vec!["label_jid", "5", "12025550111@s.whatsapp.net"],
                    vec![
                        "label_message",
                        "5",
                        "12025550111@s.whatsapp.net",
                        "MSGID",
                        "0",
                        "0",
                    ],
                ] {
                    let kind = index[0];
                    let mut mutation = set_mutation(index, wa::SyncActionValue::default());
                    let value = mutation.action_value.as_mut().unwrap();
                    value.timestamp = raw;
                    if kind == "label_edit" {
                        value.label_edit_action = buffa::MessageField::some(Default::default());
                    } else {
                        value.label_association_action =
                            buffa::MessageField::some(Default::default());
                        value
                            .label_association_action
                            .as_option_mut()
                            .unwrap()
                            .labeled = Some(true);
                    }
                    let bus = CoreEventBus::new();
                    let rec = Arc::new(Recorder::default());
                    bus.subscribe_handler(rec.clone()).detach();
                    let before = wacore::time::now_utc();
                    let outcome =
                        dispatch_label_mutation_outcome(&bus, &mut mutation, from_full_sync);
                    let after = wacore::time::now_utc();
                    let events = rec.events.lock().unwrap();
                    assert_eq!(events.len(), 1, "{kind} {raw:?}");
                    let (name, timestamp, full_sync, json) = match &*events[0] {
                        Event::LabelEditUpdate(u) => (
                            "LabelEditUpdate",
                            u.timestamp,
                            u.from_full_sync,
                            serde_json::to_value(u).unwrap(),
                        ),
                        Event::LabelAssociationUpdate(u) => (
                            "LabelAssociationUpdate",
                            u.timestamp,
                            u.from_full_sync,
                            serde_json::to_value(u).unwrap(),
                        ),
                        Event::MessageLabelAssociationUpdate(u) => (
                            "MessageLabelAssociationUpdate",
                            u.timestamp,
                            u.from_full_sync,
                            serde_json::to_value(u).unwrap(),
                        ),
                        other => panic!("unexpected event: {other:?}"),
                    };
                    assert_eq!(outcome, AppStateDispatchOutcome::Event(name));
                    assert_eq!(full_sync, from_full_sync);
                    assert_eq!(
                        json.get("action_timestamp"),
                        Some(&serde_json::to_value(expected).unwrap()),
                        "{kind} {raw:?}"
                    );
                    match raw {
                        None | Some(0) => assert_eq!(timestamp, epoch),
                        Some(1_700_000_000_123) => assert_eq!(timestamp, valid),
                        _ => assert!(timestamp >= before && timestamp <= after),
                    }
                }
            }
        }
    }

    #[test]
    fn label_edit_dispatches_update() {
        let m = set_mutation(
            vec!["label_edit", "5"],
            wa::SyncActionValue {
                label_edit_action: buffa::MessageField::some(
                    wa::sync_action_value::LabelEditAction {
                        name: Some("Work".into()),
                        color: Some(2),
                        deleted: Some(false),
                        ..Default::default()
                    },
                ),
                timestamp: Some(1000),
                ..Default::default()
            },
        );
        let (outcome, events) = run(&m);
        assert!(outcome != AppStateDispatchOutcome::Unclaimed);
        assert_eq!(events.len(), 1);
        match &*events[0] {
            Event::LabelEditUpdate(u) => {
                assert_eq!(u.label_id, "5");
                assert_eq!(u.action.name.as_deref(), Some("Work"));
                assert_eq!(u.action.color, Some(2));
                assert_eq!(u.action.deleted, Some(false));
            }
            other => panic!("expected LabelEditUpdate, got {other:?}"),
        }
    }

    #[test]
    fn label_jid_dispatches_association() {
        let m = set_mutation(
            vec!["label_jid", "5", "12025550111@s.whatsapp.net"],
            wa::SyncActionValue {
                label_association_action: buffa::MessageField::some(
                    wa::sync_action_value::LabelAssociationAction {
                        labeled: Some(true),
                        ..Default::default()
                    },
                ),
                timestamp: Some(1000),
                ..Default::default()
            },
        );
        let (outcome, events) = run(&m);
        assert!(outcome != AppStateDispatchOutcome::Unclaimed);
        assert_eq!(events.len(), 1);
        match &*events[0] {
            Event::LabelAssociationUpdate(u) => {
                assert_eq!(u.label_id, "5");
                assert_eq!(u.chat_jid.to_string(), "12025550111@s.whatsapp.net");
                assert_eq!(u.action.labeled, Some(true));
            }
            other => panic!("expected LabelAssociationUpdate, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn label_methods_reject_empty_id() {
        // Validation fires before any network/app-state work, so a key-less test
        // client still exercises the guard.
        let client = crate::test_utils::create_test_client().await;
        let jid: Jid = "12025550111@s.whatsapp.net".parse().unwrap();

        let err = client
            .labels()
            .create_label("", "Work", 0)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("label_id cannot be empty"));

        let err = client.labels().create_label("5", "", 0).await.unwrap_err();
        assert!(err.to_string().contains("label name cannot be empty"));

        assert!(client.labels().delete_label("").await.is_err());
        assert!(client.labels().add_chat_label("", &jid).await.is_err());
        assert!(client.labels().remove_chat_label("", &jid).await.is_err());
    }

    /// The exact index `label_message` puts on the wire.
    ///
    /// `["label_message", labelId, chatJid, messageId, "0", "0"]` — the same
    /// bytes whatsmeow's `BuildLabelMessage` and Baileys' `addMessageLabel`
    /// send, on the `regular` collection at version 3. An index that drifts from
    /// this doesn't fail anywhere in CI; it corrupts the user's synced state.
    #[tokio::test]
    async fn message_label_index_and_value_match_the_wire() {
        let chat: Jid = "12025550111@s.whatsapp.net".parse().expect("test JID");
        let collection = LABEL_MESSAGE.collection.as_str();
        assert_eq!(collection, "regular");
        assert_eq!(LABEL_MESSAGE.version, 3);

        let added = capture(collection, {
            let chat = chat.clone();
            move |client| async move {
                client
                    .labels()
                    .add_message_label("5", &chat, &crate::MessageId::new("3EB0MSGID").unwrap())
                    .await
            }
        })
        .await;
        assert_eq!(
            added.index,
            vec![
                "label_message",
                "5",
                "12025550111@s.whatsapp.net",
                "3EB0MSGID",
                "0",
                "0",
            ]
        );
        assert_eq!(added.operation, wa::syncd_mutation::SyncdOperation::Set);
        assert_eq!(
            added
                .action_value
                .as_ref()
                .and_then(|v| v.label_association_action.as_option())
                .and_then(|a| a.labeled),
            Some(true),
            "the association rides on SyncActionValue.labelAssociationAction"
        );

        let removed = capture(collection, {
            let chat = chat.clone();
            move |client| async move {
                client
                    .labels()
                    .remove_message_label("5", &chat, &crate::MessageId::new("3EB0MSGID").unwrap())
                    .await
            }
        })
        .await;
        assert_eq!(
            removed.index, added.index,
            "removal is the same index with labeled=false, not a syncd Remove"
        );
        assert_eq!(removed.operation, wa::syncd_mutation::SyncdOperation::Set);
        assert_eq!(
            removed
                .action_value
                .as_ref()
                .and_then(|v| v.label_association_action.as_option())
                .and_then(|a| a.labeled),
            Some(false)
        );
    }

    /// What we emit, a linked device must be able to hand back.
    #[tokio::test]
    async fn message_label_round_trips_through_the_inbound_dispatch() {
        let chat: Jid = "12025550111@s.whatsapp.net".parse().expect("test JID");
        let mutation = capture(LABEL_MESSAGE.collection.as_str(), {
            let chat = chat.clone();
            move |client| async move {
                client
                    .labels()
                    .add_message_label("5", &chat, &crate::MessageId::new("3EB0MSGID").unwrap())
                    .await
            }
        })
        .await;

        let (outcome, events) = run(&mutation);
        assert!(outcome != AppStateDispatchOutcome::Unclaimed);
        assert_eq!(events.len(), 1);
        match &*events[0] {
            Event::MessageLabelAssociationUpdate(u) => {
                assert_eq!(u.label_id, "5");
                assert_eq!(u.chat_jid, chat);
                assert_eq!(u.message_id, "3EB0MSGID");
                assert_eq!(u.action.labeled, Some(true));
            }
            other => panic!("expected MessageLabelAssociationUpdate, got {other:?}"),
        }
    }

    #[test]
    fn message_label_index_rejects_a_wrong_argument_count() {
        use crate::features::chat_actions::build_action_index;
        // Five index parts are non-literal; anything else is a caller bug that
        // must surface before the mutation is encrypted and sent.
        assert!(build_action_index(&LABEL_MESSAGE, &["5", "1@s.whatsapp.net", "ID"]).is_err());
        assert!(
            build_action_index(
                &LABEL_MESSAGE,
                &["5", "1@s.whatsapp.net", "ID", "0", "0", "x"]
            )
            .is_err()
        );
        assert!(
            build_action_index(&LABEL_MESSAGE, &["5", "1@s.whatsapp.net", "ID", "0", "0"]).is_ok()
        );
    }

    #[tokio::test]
    async fn message_label_methods_reject_empty_ids() {
        let client = crate::test_utils::create_test_client().await;
        let jid: Jid = "12025550111@s.whatsapp.net".parse().unwrap();

        let err = client
            .labels()
            .add_message_label("", &jid, &crate::MessageId::new("MSGID").unwrap())
            .await
            .unwrap_err();
        assert!(err.to_string().contains("label_id cannot be empty"));

        assert!(crate::MessageId::new("").is_err());

        assert!(
            client
                .labels()
                .remove_message_label("", &jid, &crate::MessageId::new("MSGID").unwrap())
                .await
                .is_err()
        );
    }

    #[test]
    fn message_label_missing_index_parts_are_claimed_but_not_dispatched() {
        for index in [
            vec!["label_message", "5"],
            vec!["label_message", "5", "not a jid", "MSGID", "0", "0"],
            vec!["label_message", "5", "12025550111@s.whatsapp.net"],
            // Present but empty: an association keyed off no message at all.
            vec![
                "label_message",
                "5",
                "12025550111@s.whatsapp.net",
                "",
                "0",
                "0",
            ],
        ] {
            let m = set_mutation(
                index.clone(),
                wa::SyncActionValue {
                    label_association_action: buffa::MessageField::some(
                        wa::sync_action_value::LabelAssociationAction {
                            labeled: Some(true),
                            ..Default::default()
                        },
                    ),
                    ..Default::default()
                },
            );
            let (outcome, events) = run(&m);
            assert!(
                outcome != AppStateDispatchOutcome::Unclaimed,
                "{index:?} must not be retried by another handler"
            );
            assert!(events.is_empty(), "{index:?} must not emit a partial event");
        }
    }

    #[test]
    fn non_label_kind_is_not_claimed() {
        // A chat-action mutation must fall through so its own handler runs.
        let m = set_mutation(
            vec!["mute", "12025550111@s.whatsapp.net"],
            wa::SyncActionValue::default(),
        );
        let (outcome, events) = run(&m);
        assert_eq!(outcome, AppStateDispatchOutcome::Unclaimed);
        assert!(events.is_empty());
    }

    #[test]
    fn label_jid_with_malformed_chat_is_claimed_but_not_dispatched() {
        // Claimed (a non-`Unclaimed` outcome) so it isn't re-tried, but no event is emitted.
        let m = set_mutation(
            vec!["label_jid", "5", "not a jid"],
            wa::SyncActionValue {
                label_association_action: buffa::MessageField::some(
                    wa::sync_action_value::LabelAssociationAction {
                        labeled: Some(true),
                        ..Default::default()
                    },
                ),
                ..Default::default()
            },
        );
        let (outcome, events) = run(&m);
        assert!(outcome != AppStateDispatchOutcome::Unclaimed);
        assert!(events.is_empty());
    }
}
