//! Labels (etiquetas) via app state sync (syncd).
//!
//! Mirrors WhatsApp Web's `WAWebLabel*`. All label actions live in the
//! `regular` collection:
//! - `label_edit`    (index `["label_edit", labelId]`)         -> `LabelEditAction`
//! - `label_jid`     (index `["label_jid", labelId, chatJid]`) -> `LabelAssociationAction`
//! - `label_message` (index `["label_message", labelId, chatJid, messageId, fromMe, participant]`)
//!   -> `LabelAssociationAction`
//!
//! Collection, action version, and index shape come from the generated
//! `schemas::{LABEL_EDIT, LABEL_JID}` registry, except `label_message`, which
//! WA Web names but no longer builds — see
//! [`schemas_unlisted::LABEL_MESSAGE`](wacore::appstate::schemas_unlisted::LABEL_MESSAGE).

use crate::appstate_sync::Mutation;
use crate::client::Client;
use crate::features::chat_actions::AppStateError;
use log::debug;
use wacore::appstate::{schemas, schemas_unlisted};
use wacore::types::events::{
    Event, LabelAssociationUpdate, LabelEditUpdate, MessageLabelAssociationUpdate,
};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

/// Dispatch inbound label mutations synced from a linked device.
/// Returns `true` if handled, `false` if the mutation is not a label kind.
pub(crate) fn dispatch_label_mutation(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &Mutation,
    full_sync: bool,
) -> bool {
    if m.operation != wa::syncd_mutation::SyncdOperation::Set || m.index.is_empty() {
        return false;
    }

    let kind = m.index[0].as_str();
    if !matches!(kind, "label_edit" | "label_jid" | "label_message") {
        return false;
    }

    let ts = m
        .action_value
        .as_ref()
        .and_then(|v| v.timestamp)
        .unwrap_or(0);
    let time = wacore::time::from_millis_or_now(ts);

    let Some(label_id) = m.index.get(1).cloned() else {
        log::warn!("Skipping label mutation '{kind}': missing label id in index");
        return true;
    };

    match kind {
        "label_edit" => {
            if let Some(val) = &m.action_value
                && let Some(act) = val.label_edit_action.as_option()
            {
                event_bus.dispatch(Event::LabelEditUpdate(
                    LabelEditUpdate::builder()
                        .label_id(label_id)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "label_message" => {
            let Some(chat_jid) = parse_association_chat_jid(kind, &m.index) else {
                return true;
            };
            // Empty is as unusable as absent: the id is what the association
            // hangs off, and an event carrying "" points at no message. The
            // outbound side rejects it for the same reason.
            let Some(message_id) = m.index.get(3).filter(|id| !id.is_empty()).cloned() else {
                log::warn!("Skipping label_message mutation: missing or empty message id in index");
                return true;
            };
            if let Some(val) = &m.action_value
                && let Some(act) = val.label_association_action.as_option()
            {
                event_bus.dispatch(Event::MessageLabelAssociationUpdate(
                    MessageLabelAssociationUpdate::builder()
                        .label_id(label_id)
                        .chat_jid(chat_jid)
                        .message_id(message_id)
                        .timestamp(time)
                        .action(Box::new(act.clone()))
                        .from_full_sync(full_sync)
                        .build(),
                ));
            }
            true
        }
        "label_jid" => {
            let Some(chat_jid) = parse_association_chat_jid(kind, &m.index) else {
                return true;
            };
            if let Some(val) = &m.action_value
                && let Some(act) = val.label_association_action.as_option()
            {
                event_bus.dispatch(Event::LabelAssociationUpdate(
                    LabelAssociationUpdate::builder()
                        .label_id(label_id)
                        .chat_jid(chat_jid)
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

/// Both association actions carry the chat JID at index position 2. Returns
/// `None` (with a warning) when it is missing or unparseable, so the caller can
/// claim the mutation without emitting a half-formed event.
fn parse_association_chat_jid(kind: &str, index: &[String]) -> Option<Jid> {
    match index.get(2) {
        Some(s) => match s.parse() {
            Ok(jid) => Some(jid),
            Err(_) => {
                log::warn!("Skipping {kind} mutation: malformed chat JID '{s}'");
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
        // Don't log the label name (user content); the id/color are enough to trace.
        debug!(
            "Setting label {label_id} (name_len={}, color={color})",
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
        debug!("Deleting label {label_id}");
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
        message_id: &str,
    ) -> Result<(), AppStateError> {
        self.send_message_association(label_id, chat_jid, message_id, true)
            .await
    }

    /// Remove a label association from a single message.
    pub async fn remove_message_label(
        &self,
        label_id: &str,
        chat_jid: &Jid,
        message_id: &str,
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
            "{} label {label_id} {} chat {chat_jid}",
            if labeled { "Adding" } else { "Removing" },
            if labeled { "to" } else { "from" },
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
        message_id: &str,
        labeled: bool,
    ) -> Result<(), AppStateError> {
        if label_id.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "label_id cannot be empty".into(),
            ));
        }
        if message_id.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "message_id cannot be empty".into(),
            ));
        }
        debug!(
            "{} label {label_id} {} message {message_id} in {chat_jid}",
            if labeled { "Adding" } else { "Removing" },
            if labeled { "to" } else { "from" },
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
                &[label_id, chat.as_str(), message_id, "0", "0"],
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
    use wacore::appstate::patch_decode::WAPatchName;
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

    fn run(m: &Mutation) -> (bool, Vec<Arc<Event>>) {
        let bus = CoreEventBus::new();
        let rec = Arc::new(Recorder::default());
        bus.subscribe_handler(rec.clone()).detach();
        let handled = dispatch_label_mutation(&bus, m, false);
        let events = rec.events.lock().unwrap().clone();
        (handled, events)
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
        let (handled, events) = run(&m);
        assert!(handled);
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
        let (handled, events) = run(&m);
        assert!(handled);
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
        let collection =
            crate::features::chat_actions::collection_patch_name(LABEL_MESSAGE.collection);
        assert_eq!(collection, WAPatchName::Regular);
        assert_eq!(LABEL_MESSAGE.version, 3);

        let added = capture(collection.as_str(), {
            let chat = chat.clone();
            move |client| async move {
                client
                    .labels()
                    .add_message_label("5", &chat, "3EB0MSGID")
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

        let removed = capture(collection.as_str(), {
            let chat = chat.clone();
            move |client| async move {
                client
                    .labels()
                    .remove_message_label("5", &chat, "3EB0MSGID")
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
        let mutation = capture(
            crate::features::chat_actions::collection_patch_name(LABEL_MESSAGE.collection).as_str(),
            {
                let chat = chat.clone();
                move |client| async move {
                    client
                        .labels()
                        .add_message_label("5", &chat, "3EB0MSGID")
                        .await
                }
            },
        )
        .await;

        let (handled, events) = run(&mutation);
        assert!(handled);
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
            .add_message_label("", &jid, "MSGID")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("label_id cannot be empty"));

        let err = client
            .labels()
            .add_message_label("5", &jid, "")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("message_id cannot be empty"));

        assert!(
            client
                .labels()
                .remove_message_label("", &jid, "MSGID")
                .await
                .is_err()
        );
        assert!(
            client
                .labels()
                .remove_message_label("5", &jid, "")
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
            let (handled, events) = run(&m);
            assert!(handled, "{index:?} must not be retried by another handler");
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
        let (handled, events) = run(&m);
        assert!(!handled);
        assert!(events.is_empty());
    }

    #[test]
    fn label_jid_with_malformed_chat_is_claimed_but_not_dispatched() {
        // Claimed (returns true) so it isn't re-tried, but no event is emitted.
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
        let (handled, events) = run(&m);
        assert!(handled);
        assert!(events.is_empty());
    }
}
