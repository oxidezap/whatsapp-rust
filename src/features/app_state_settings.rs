//! Account-level settings carried by app state sync (syncd).
//!
//! Distinct from the `privacy` IQ namespace (`Client::set_privacy_setting`):
//! these settings have no query/set stanza at all, they exist only as syncd
//! mutations replicated across the user's linked devices.
//!
//! - `setting_disableLinkPreviews` (index `["setting_disableLinkPreviews"]`,
//!   `regular`) -> `PrivacySettingDisableLinkPreviewsAction`

use crate::appstate_sync::Mutation;
use crate::client::Client;
use crate::features::chat_actions::AppStateError;
use log::debug;
use wacore::appstate::schemas;
use wacore::types::events::{DisableLinkPreviewsUpdate, Event};
use waproto::whatsapp as wa;

/// Dispatch inbound syncd setting mutations synced from a linked device.
/// Returns `true` if handled, `false` if the mutation is not one of them.
pub(crate) fn dispatch_app_state_setting_mutation(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &Mutation,
    full_sync: bool,
) -> bool {
    if m.operation != wa::syncd_mutation::SyncdOperation::Set
        || m.index.first().map(String::as_str) != Some(schemas::DISABLE_LINK_PREVIEWS.name)
    {
        return false;
    }

    let ts = m
        .action_value
        .as_ref()
        .and_then(|v| v.timestamp)
        .unwrap_or(0);
    let time = wacore::time::from_millis_or_now(ts);

    // WA Web counts a mutation whose `isPreviewsDisabled` is absent as a
    // malformed action value and applies nothing, so there is no flag to report.
    if let Some(val) = &m.action_value
        && let Some(act) = val.privacy_setting_disable_link_previews_action.as_option()
        && let Some(disabled) = act.is_previews_disabled
    {
        event_bus.dispatch(Event::DisableLinkPreviewsUpdate(
            DisableLinkPreviewsUpdate::builder()
                .previews_disabled(disabled)
                .timestamp(time)
                .action(Box::new(act.clone()))
                .from_full_sync(full_sync)
                .build(),
        ));
    } else {
        log::warn!(
            "Skipping setting_disableLinkPreviews mutation: missing isPreviewsDisabled flag"
        );
    }
    true
}

/// Access via `client.app_state_settings()`.
pub struct AppStateSettings<'a> {
    client: &'a Client,
}

impl<'a> AppStateSettings<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Turn outgoing link previews off or on for the whole account.
    ///
    /// Mirrors WA Web's `WAWebDisableLinkPreviewsSync.getMutation`: a `Set` on
    /// the `regular` collection with an empty index argument list, carrying
    /// `privacySettingDisableLinkPreviewsAction.isPreviewsDisabled`.
    ///
    /// This is the account's stored preference, replicated to the linked
    /// devices. It does not stop this client from attaching a preview it was
    /// explicitly asked to send.
    pub async fn set_link_previews_disabled(&self, disabled: bool) -> Result<(), AppStateError> {
        debug!("Setting disableLinkPreviews -> {disabled}");
        let value = wa::SyncActionValue {
            privacy_setting_disable_link_previews_action: buffa::MessageField::some(
                wa::sync_action_value::PrivacySettingDisableLinkPreviewsAction {
                    is_previews_disabled: Some(disabled),
                },
            ),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        self.client
            .send_app_state_action(&schemas::DISABLE_LINK_PREVIEWS, &[], &value)
            .await
    }
}

impl Client {
    pub fn app_state_settings(&self) -> AppStateSettings<'_> {
        AppStateSettings::new(self)
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
        let handled = dispatch_app_state_setting_mutation(&bus, m, false);
        let events = rec.events.lock().unwrap().clone();
        (handled, events)
    }

    fn set_mutation(value: wa::SyncActionValue) -> Mutation {
        Mutation {
            index: vec!["setting_disableLinkPreviews".into()],
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(value),
        }
    }

    #[test]
    fn disable_link_previews_index_matches_wa_web() {
        // WAWebDisableLinkPreviewsSync passes `indexArgs: []`, so the index is
        // the bare action name.
        let index = build_action_index(&schemas::DISABLE_LINK_PREVIEWS, &[]).unwrap();
        let parts: Vec<String> = serde_json::from_slice(&index).unwrap();
        assert_eq!(parts, vec!["setting_disableLinkPreviews"]);
        assert_eq!(schemas::DISABLE_LINK_PREVIEWS.version, 8);
        assert_eq!(
            schemas::DISABLE_LINK_PREVIEWS.collection,
            schemas::Collection::Regular
        );
        assert_eq!(
            schemas::DISABLE_LINK_PREVIEWS.value_field,
            Some("privacySettingDisableLinkPreviewsAction")
        );
    }

    #[test]
    fn index_rejects_any_argument() {
        assert!(build_action_index(&schemas::DISABLE_LINK_PREVIEWS, &["1"]).is_err());
    }

    /// The exact bytes `set_link_previews_disabled` puts on the wire, against
    /// `WAWebDisableLinkPreviewsSync.getMutation`.
    #[tokio::test]
    async fn link_preview_setting_matches_wa_webs_builder_on_the_wire() {
        let collection = crate::features::chat_actions::collection_patch_name(
            schemas::DISABLE_LINK_PREVIEWS.collection,
        );
        for disabled in [true, false] {
            let mutation = capture(collection.as_str(), move |client| async move {
                client
                    .app_state_settings()
                    .set_link_previews_disabled(disabled)
                    .await
            })
            .await;
            assert_eq!(mutation.index, vec!["setting_disableLinkPreviews"]);
            assert_eq!(mutation.operation, wa::syncd_mutation::SyncdOperation::Set);
            assert_eq!(
                mutation
                    .action_value
                    .as_ref()
                    .and_then(|v| v.privacy_setting_disable_link_previews_action.as_option())
                    .and_then(|a| a.is_previews_disabled),
                Some(disabled),
                "the flag rides on SyncActionValue.privacySettingDisableLinkPreviewsAction"
            );

            // What we emit, a linked device must be able to hand back.
            let (handled, events) = run(&mutation);
            assert!(handled);
            assert_eq!(events.len(), 1);
            match &*events[0] {
                Event::DisableLinkPreviewsUpdate(u) => assert_eq!(u.previews_disabled, disabled),
                other => panic!("expected DisableLinkPreviewsUpdate, got {other:?}"),
            }
        }
    }

    #[test]
    fn inbound_set_dispatches_the_flag() {
        for disabled in [true, false] {
            let m = set_mutation(wa::SyncActionValue {
                privacy_setting_disable_link_previews_action: buffa::MessageField::some(
                    wa::sync_action_value::PrivacySettingDisableLinkPreviewsAction {
                        is_previews_disabled: Some(disabled),
                    },
                ),
                timestamp: Some(1000),
                ..Default::default()
            });
            let (handled, events) = run(&m);
            assert!(handled);
            assert_eq!(events.len(), 1);
            match &*events[0] {
                Event::DisableLinkPreviewsUpdate(u) => {
                    assert_eq!(u.previews_disabled, disabled);
                    assert_eq!(u.action.is_previews_disabled, Some(disabled));
                }
                other => panic!("expected DisableLinkPreviewsUpdate, got {other:?}"),
            }
        }
    }

    #[test]
    fn absent_flag_is_claimed_but_not_dispatched() {
        // WA Web logs it as a malformed action value and applies nothing; an
        // event with a made-up default would be worse than none.
        let m = set_mutation(wa::SyncActionValue {
            privacy_setting_disable_link_previews_action: buffa::MessageField::some(
                wa::sync_action_value::PrivacySettingDisableLinkPreviewsAction {
                    is_previews_disabled: None,
                },
            ),
            ..Default::default()
        });
        let (handled, events) = run(&m);
        assert!(handled);
        assert!(events.is_empty());
    }

    #[test]
    fn other_kinds_are_not_claimed() {
        let m = Mutation {
            index: vec!["setting_pushName".into()],
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(wa::SyncActionValue::default()),
        };
        let (handled, events) = run(&m);
        assert!(!handled);
        assert!(events.is_empty());
    }
}
