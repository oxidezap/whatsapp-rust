//! Chat labels (etiquetas) via app state sync (syncd).
//!
//! Mirrors WhatsApp Web's `WAWebLabel*`. Both label actions live in the
//! `regular` collection:
//! - `label_edit`  (index `["label_edit", labelId]`)        -> `LabelEditAction`
//! - `label_jid`   (index `["label_jid", labelId, chatJid]`) -> `LabelAssociationAction`
//!
//! Collection, action version, and index shape all come from the generated
//! `schemas::{LABEL_EDIT, LABEL_JID}` registry.

use crate::appstate_sync::Mutation;
use crate::client::Client;
use crate::features::chat_actions::AppStateError;
use log::debug;
use wacore::appstate::schemas;
use wacore::types::events::{Event, LabelAssociationUpdate, LabelEditUpdate};
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
    if !matches!(kind, "label_edit" | "label_jid") {
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
                && let Some(act) = &val.label_edit_action
            {
                event_bus.dispatch(Event::LabelEditUpdate(LabelEditUpdate {
                    label_id,
                    timestamp: time,
                    action: Box::new(act.clone()),
                    from_full_sync: full_sync,
                }));
            }
            true
        }
        "label_jid" => {
            let chat_jid: Jid = match m.index.get(2) {
                Some(s) => match s.parse() {
                    Ok(j) => j,
                    Err(_) => {
                        log::warn!("Skipping label_jid mutation: malformed chat JID '{s}'");
                        return true;
                    }
                },
                None => {
                    log::warn!("Skipping label_jid mutation: missing chat JID in index");
                    return true;
                }
            };
            if let Some(val) = &m.action_value
                && let Some(act) = &val.label_association_action
            {
                event_bus.dispatch(Event::LabelAssociationUpdate(LabelAssociationUpdate {
                    label_id,
                    chat_jid,
                    timestamp: time,
                    action: Box::new(act.clone()),
                    from_full_sync: full_sync,
                }));
            }
            true
        }
        _ => false,
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
            label_edit_action: Some(wa::sync_action_value::LabelEditAction {
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
            label_edit_action: Some(wa::sync_action_value::LabelEditAction {
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
        let value = wa::SyncActionValue {
            label_association_action: Some(wa::sync_action_value::LabelAssociationAction {
                labeled: Some(labeled),
                ..Default::default()
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        self.client
            .send_app_state_action(&schemas::LABEL_JID, &[label_id, chat.as_str()], &value)
            .await
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
    use std::sync::{Arc, Mutex};
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
        bus.add_handler(rec.clone());
        let handled = dispatch_label_mutation(&bus, m, false);
        let events = rec.events.lock().unwrap().clone();
        (handled, events)
    }

    #[test]
    fn label_edit_dispatches_update() {
        let m = set_mutation(
            vec!["label_edit", "5"],
            wa::SyncActionValue {
                label_edit_action: Some(wa::sync_action_value::LabelEditAction {
                    name: Some("Work".into()),
                    color: Some(2),
                    deleted: Some(false),
                    ..Default::default()
                }),
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
            vec!["label_jid", "5", "15551112222@s.whatsapp.net"],
            wa::SyncActionValue {
                label_association_action: Some(wa::sync_action_value::LabelAssociationAction {
                    labeled: Some(true),
                    ..Default::default()
                }),
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
                assert_eq!(u.chat_jid.to_string(), "15551112222@s.whatsapp.net");
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
        let jid: Jid = "15551112222@s.whatsapp.net".parse().unwrap();

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

    #[test]
    fn non_label_kind_is_not_claimed() {
        // A chat-action mutation must fall through so its own handler runs.
        let m = set_mutation(
            vec!["mute", "15551112222@s.whatsapp.net"],
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
                label_association_action: Some(wa::sync_action_value::LabelAssociationAction {
                    labeled: Some(true),
                    ..Default::default()
                }),
                ..Default::default()
            },
        );
        let (handled, events) = run(&m);
        assert!(handled);
        assert!(events.is_empty());
    }
}
