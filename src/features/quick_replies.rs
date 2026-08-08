//! Quick replies (business canned responses) via app state sync (syncd).
//!
//! Mirrors WhatsApp Web's `WAWebQuickRepliesSync`: the `quick_reply` action in
//! the `regular` collection, index `["quick_reply", id]`, carrying a
//! `QuickReplyAction`. Collection, action version and index shape come from the
//! generated `schemas::QUICK_REPLY` registry.
//!
//! Deletion is an ordinary `Set` with `deleted = true`, not a syncd `Remove` —
//! WA Web's `getQuickReplyDeleteMutation` sends `SyncdOperation.SET` and its
//! receiving side only inspects `deleted` on the `set` branch, so a `Remove`
//! would never reach the linked devices' quick-reply tables.

use crate::appstate_sync::Mutation;
use crate::client::Client;
use crate::features::chat_actions::AppStateError;
use log::debug;
use wacore::appstate::schemas;
use wacore::types::events::{Event, QuickReplyUpdate};
use waproto::whatsapp as wa;

/// Dispatch inbound quick-reply mutations synced from a linked device.
/// Returns `true` if handled, `false` if the mutation is not a quick reply.
pub(crate) fn dispatch_quick_reply_mutation(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &Mutation,
    full_sync: bool,
) -> bool {
    if m.operation != wa::syncd_mutation::SyncdOperation::Set
        || m.index.first().map(String::as_str) != Some(schemas::QUICK_REPLY.name)
    {
        return false;
    }

    let Some(id) = m.index.get(1).cloned() else {
        log::warn!("Skipping quick_reply mutation: missing id in index");
        return true;
    };

    let ts = m
        .action_value
        .as_ref()
        .and_then(|v| v.timestamp)
        .unwrap_or(0);
    let time = wacore::time::from_millis_or_now(ts);

    if let Some(val) = &m.action_value
        && let Some(act) = val.quick_reply_action.as_option()
    {
        event_bus.dispatch(Event::QuickReplyUpdate(
            QuickReplyUpdate::builder()
                .id(id)
                .timestamp(time)
                .action(Box::new(act.clone()))
                .from_full_sync(full_sync)
                .build(),
        ));
    } else {
        // Claimed but undeliverable. WA Web counts the same shape as a
        // malformed action value and logs it; without this the mutation would
        // vanish with no signal at all. The id is opaque, not user content.
        log::warn!("Skipping quick_reply mutation {id}: missing quickReplyAction value");
    }
    true
}

/// Access via `client.quick_replies()`.
pub struct QuickReplies<'a> {
    client: &'a Client,
}

impl<'a> QuickReplies<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Create or update a quick reply. App state is an upsert keyed by `id`, so
    /// this both adds a new quick reply and edits an existing one — WA Web has
    /// a single `getQuickReplyAddOrEditMutation` for the same reason.
    ///
    /// `shortcut` is the `/`-typed trigger and `message` the expanded text;
    /// both must be non-empty, because the receiving side rejects a mutation
    /// missing either as a malformed action value. `keywords` are the extra
    /// search terms WA Web indexes the reply under, and `count` is its usage
    /// counter (`0` for a new one).
    pub async fn set_quick_reply(
        &self,
        id: &str,
        shortcut: &str,
        message: &str,
        keywords: Vec<String>,
        count: i32,
    ) -> Result<(), AppStateError> {
        if id.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "quick reply id cannot be empty".into(),
            ));
        }
        if shortcut.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "quick reply shortcut cannot be empty".into(),
            ));
        }
        if message.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "quick reply message cannot be empty".into(),
            ));
        }
        // `count` is a usage tally WA Web only ever increments, so a negative one
        // is a caller bug with no wire behind it. The proto field is a signed
        // `int32` and would carry it to every linked device unremarked.
        if count < 0 {
            return Err(AppStateError::InvalidRequest(
                "quick reply count cannot be negative".into(),
            ));
        }
        // Don't log the shortcut or message (user content); the id is enough to trace.
        debug!("Setting quick reply {id} (count={count})");
        // `associatedLabelIds` is left empty on purpose, not by omission: both of
        // WA Web's builders hardcode `associatedLabelIds: []`, and its receiving
        // side never reads the field. A repeated field left at its default
        // encodes to the same bytes, so this is what the official client sends.
        let value = quick_reply_value(wa::sync_action_value::QuickReplyAction {
            shortcut: Some(shortcut.to_string()),
            message: Some(message.to_string()),
            keywords,
            count: Some(count),
            deleted: Some(false),
            ..Default::default()
        });
        self.client
            .send_app_state_action(&schemas::QUICK_REPLY, &[id], &value)
            .await
    }

    /// Delete a quick reply.
    ///
    /// Sends the same `Set` mutation with `deleted = true` and the payload
    /// fields cleared, exactly as WA Web's `getQuickReplyDeleteMutation` does.
    pub async fn delete_quick_reply(&self, id: &str) -> Result<(), AppStateError> {
        if id.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "quick reply id cannot be empty".into(),
            ));
        }
        debug!("Deleting quick reply {id}");
        let value = quick_reply_value(wa::sync_action_value::QuickReplyAction {
            shortcut: Some(String::new()),
            message: Some(String::new()),
            keywords: Vec::new(),
            count: Some(0),
            deleted: Some(true),
            ..Default::default()
        });
        self.client
            .send_app_state_action(&schemas::QUICK_REPLY, &[id], &value)
            .await
    }
}

fn quick_reply_value(action: wa::sync_action_value::QuickReplyAction) -> wa::SyncActionValue {
    wa::SyncActionValue {
        quick_reply_action: buffa::MessageField::some(action),
        timestamp: Some(wacore::time::now_millis()),
        ..Default::default()
    }
}

impl Client {
    pub fn quick_replies(&self) -> QuickReplies<'_> {
        QuickReplies::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::chat_actions::{
        build_action_index, capture_app_state_mutation as capture,
    };
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

    fn run(m: &Mutation) -> (bool, Vec<Arc<Event>>) {
        let bus = CoreEventBus::new();
        let rec = Arc::new(Recorder::default());
        bus.subscribe_handler(rec.clone()).detach();
        let handled = dispatch_quick_reply_mutation(&bus, m, false);
        let events = rec.events.lock().unwrap().clone();
        (handled, events)
    }

    #[test]
    fn quick_reply_index_matches_wa_web() {
        // WAWebQuickRepliesSync passes indexArgs `[id]` for both the add/edit
        // and the delete mutation.
        let index = build_action_index(&schemas::QUICK_REPLY, &["1700000000"]).unwrap();
        let parts: Vec<String> = serde_json::from_slice(&index).unwrap();
        assert_eq!(parts, vec!["quick_reply", "1700000000"]);
        assert_eq!(schemas::QUICK_REPLY.version, 2);
        assert_eq!(
            schemas::QUICK_REPLY.collection,
            schemas::Collection::Regular
        );
        assert_eq!(
            schemas::QUICK_REPLY.value_field,
            Some("quickReplyAction"),
            "quick_reply must ride on SyncActionValue.quickReplyAction"
        );
    }

    #[test]
    fn set_and_delete_values_match_wa_web() {
        let set = quick_reply_value(wa::sync_action_value::QuickReplyAction {
            shortcut: Some("hello".into()),
            message: Some("Hi there".into()),
            keywords: vec!["greeting".into()],
            count: Some(3),
            deleted: Some(false),
            ..Default::default()
        });
        let act = set.quick_reply_action.as_option().expect("action is set");
        assert_eq!(act.shortcut.as_deref(), Some("hello"));
        assert_eq!(act.message.as_deref(), Some("Hi there"));
        assert_eq!(act.keywords, vec!["greeting".to_string()]);
        assert_eq!(act.count, Some(3));
        assert_eq!(act.deleted, Some(false));
        assert!(set.timestamp.is_some());

        // The delete mutation clears the payload and flips `deleted`.
        let del = quick_reply_value(wa::sync_action_value::QuickReplyAction {
            shortcut: Some(String::new()),
            message: Some(String::new()),
            keywords: Vec::new(),
            count: Some(0),
            deleted: Some(true),
            ..Default::default()
        });
        let act = del.quick_reply_action.as_option().expect("action is set");
        assert_eq!(act.deleted, Some(true));
        assert_eq!(act.shortcut.as_deref(), Some(""));
        assert_eq!(act.message.as_deref(), Some(""));
        assert!(act.keywords.is_empty());
        assert_eq!(act.count, Some(0));
    }

    /// The exact bytes `set_quick_reply` and `delete_quick_reply` put on the
    /// wire, against `WAWebQuickRepliesSync`'s two builders.
    #[tokio::test]
    async fn quick_reply_verbs_match_wa_webs_builders_on_the_wire() {
        let collection =
            crate::features::chat_actions::collection_patch_name(schemas::QUICK_REPLY.collection);

        let set = capture(collection.as_str(), |client| async move {
            client
                .quick_replies()
                .set_quick_reply(
                    "1700000000",
                    "hello",
                    "Hi there",
                    vec!["greeting".into()],
                    3,
                )
                .await
        })
        .await;
        assert_eq!(set.index, vec!["quick_reply", "1700000000"]);
        assert_eq!(set.operation, wa::syncd_mutation::SyncdOperation::Set);
        let act = set
            .action_value
            .as_ref()
            .and_then(|v| v.quick_reply_action.as_option())
            .expect("the value rides on SyncActionValue.quickReplyAction");
        assert_eq!(act.shortcut.as_deref(), Some("hello"));
        assert_eq!(act.message.as_deref(), Some("Hi there"));
        assert_eq!(act.keywords, vec!["greeting".to_string()]);
        assert_eq!(act.count, Some(3));
        assert_eq!(act.deleted, Some(false));

        let deleted = capture(collection.as_str(), |client| async move {
            client
                .quick_replies()
                .delete_quick_reply("1700000000")
                .await
        })
        .await;
        assert_eq!(
            deleted.operation,
            wa::syncd_mutation::SyncdOperation::Set,
            "deletion is a Set carrying `deleted`, not a syncd Remove"
        );
        assert_eq!(deleted.index, set.index, "both key the same index");
        let act = deleted
            .action_value
            .as_ref()
            .and_then(|v| v.quick_reply_action.as_option())
            .expect("a delete still carries quickReplyAction");
        assert_eq!(act.deleted, Some(true));
        assert_eq!(act.shortcut.as_deref(), Some(""));
        assert_eq!(act.message.as_deref(), Some(""));
        assert!(act.keywords.is_empty());
        assert_eq!(act.count, Some(0));
    }

    /// What we emit, a linked device must be able to hand back.
    #[tokio::test]
    async fn quick_reply_round_trips_through_the_inbound_dispatch() {
        let mutation = capture(
            crate::features::chat_actions::collection_patch_name(schemas::QUICK_REPLY.collection)
                .as_str(),
            |client| async move {
                client
                    .quick_replies()
                    .set_quick_reply("1700000000", "hello", "Hi there", Vec::new(), 0)
                    .await
            },
        )
        .await;

        let (handled, events) = run(&mutation);
        assert!(handled);
        assert_eq!(events.len(), 1);
        match &*events[0] {
            Event::QuickReplyUpdate(u) => {
                assert_eq!(u.id, "1700000000");
                assert_eq!(u.action.shortcut.as_deref(), Some("hello"));
                assert_eq!(u.action.message.as_deref(), Some("Hi there"));
            }
            other => panic!("expected QuickReplyUpdate, got {other:?}"),
        }
    }

    #[test]
    fn inbound_set_dispatches_update() {
        let m = Mutation {
            index: vec!["quick_reply".into(), "1700000000".into()],
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(quick_reply_value(wa::sync_action_value::QuickReplyAction {
                shortcut: Some("hello".into()),
                message: Some("Hi there".into()),
                deleted: Some(false),
                ..Default::default()
            })),
        };
        let (handled, events) = run(&m);
        assert!(handled);
        assert_eq!(events.len(), 1);
        match &*events[0] {
            Event::QuickReplyUpdate(u) => {
                assert_eq!(u.id, "1700000000");
                assert_eq!(u.action.shortcut.as_deref(), Some("hello"));
                assert_eq!(u.action.deleted, Some(false));
            }
            other => panic!("expected QuickReplyUpdate, got {other:?}"),
        }
    }

    #[test]
    fn inbound_delete_is_a_set_carrying_the_deleted_flag() {
        let m = Mutation {
            index: vec!["quick_reply".into(), "1700000000".into()],
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(quick_reply_value(wa::sync_action_value::QuickReplyAction {
                deleted: Some(true),
                ..Default::default()
            })),
        };
        let (handled, events) = run(&m);
        assert!(handled);
        match &*events[0] {
            Event::QuickReplyUpdate(u) => assert_eq!(u.action.deleted, Some(true)),
            other => panic!("expected QuickReplyUpdate, got {other:?}"),
        }
    }

    #[test]
    fn missing_id_is_claimed_but_not_dispatched() {
        let m = Mutation {
            index: vec!["quick_reply".into()],
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(wa::SyncActionValue::default()),
        };
        let (handled, events) = run(&m);
        assert!(handled);
        assert!(events.is_empty());
    }

    #[test]
    fn missing_action_value_is_claimed_but_not_dispatched() {
        // The id is present, so this reaches the branch the missing-id test
        // never gets to: a well-indexed mutation with no quickReplyAction. It
        // must be claimed (nothing else can read it) and warned about, but must
        // not become an event describing a quick reply we know nothing about.
        for action_value in [Some(wa::SyncActionValue::default()), None] {
            let m = Mutation {
                index: vec!["quick_reply".into(), "1700000000".into()],
                operation: wa::syncd_mutation::SyncdOperation::Set,
                action_value,
            };
            let (handled, events) = run(&m);
            assert!(handled);
            assert!(events.is_empty());
        }
    }

    #[test]
    fn other_kinds_are_not_claimed() {
        let m = Mutation {
            index: vec!["label_edit".into(), "5".into()],
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(wa::SyncActionValue::default()),
        };
        let (handled, events) = run(&m);
        assert!(!handled);
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn methods_reject_missing_required_fields() {
        // Validation fires before any network/app-state work, so a key-less test
        // client still exercises the guards.
        let client = crate::test_utils::create_test_client().await;
        let qr = client.quick_replies();

        let err = qr
            .set_quick_reply("", "hi", "Hi there", Vec::new(), 0)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("id cannot be empty"));

        let err = qr
            .set_quick_reply("1", "", "Hi there", Vec::new(), 0)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("shortcut cannot be empty"));

        let err = qr
            .set_quick_reply("1", "hi", "", Vec::new(), 0)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("message cannot be empty"));

        let err = qr
            .set_quick_reply("1", "hi", "Hi there", Vec::new(), -1)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("count cannot be negative"));

        // Zero is the documented starting value, not a rejected one. It fails
        // later, on the app-state send, which is past the guards under test.
        let err = qr
            .set_quick_reply("1", "hi", "Hi there", Vec::new(), 0)
            .await
            .unwrap_err();
        assert!(!err.to_string().contains("count cannot be negative"));

        assert!(qr.delete_quick_reply("").await.is_err());
    }
}
