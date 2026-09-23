//! Favorite chats via app state sync (syncd).
//!
//! Mirrors WhatsApp Web's `WAWebFavoritesSync`: the `favorites` action in the
//! `regular_high` collection, indexed by the literal `["favorites"]` alone.
//! Collection, action version and index shape come from the generated
//! `schemas::FAVORITES` registry entry.
//!
//! There is a single mutation for the whole account, so every `Set` carries
//! the complete list and replaces the previous one; removing the last
//! favorite is a `Set` with an empty list, not a syncd `Remove`.

use crate::appstate_sync::Mutation;
use crate::client::AppStateDispatchOutcome;
use wacore::appstate::schemas;
use wacore::types::events::{Event, FavoritesUpdate};
use waproto::whatsapp as wa;

/// Dispatch an inbound `favorites` mutation synced from a linked device,
/// returning the [`crate::client::AppStateDispatchOutcome`] for the semantic
/// per-mutation log line.
pub(crate) fn dispatch_favorites_mutation_outcome(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &mut Mutation,
    event_full_sync: bool,
) -> AppStateDispatchOutcome {
    if m.operation != wa::syncd_mutation::SyncdOperation::Set
        || m.index.first().map(String::as_str) != Some(schemas::FAVORITES.name)
    {
        return AppStateDispatchOutcome::Unclaimed;
    }

    let ts = m
        .action_value
        .as_ref()
        .and_then(|v| v.timestamp)
        .unwrap_or(0);

    // An empty `favoritesAction` is the valid "no favorites" state; only a
    // value without the action at all says nothing about the list.
    if let Some(val) = &mut m.action_value
        && let Some(act) = val.favorites_action.take()
    {
        event_bus.dispatch(Event::FavoritesUpdate(
            FavoritesUpdate::builder()
                .timestamp(wacore::time::from_millis_or_now(ts))
                .action(Box::new(act))
                .from_full_sync(event_full_sync)
                .build(),
        ));
        AppStateDispatchOutcome::Event("FavoritesUpdate")
    } else {
        // Warned once centrally by `report` (Malformed authority).
        AppStateDispatchOutcome::Malformed("FavoritesUpdate")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::chat_actions::build_action_index;
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

    fn run(m: &Mutation) -> (AppStateDispatchOutcome, Vec<Arc<Event>>) {
        let bus = CoreEventBus::new();
        let rec = Arc::new(Recorder::default());
        bus.subscribe_handler(rec.clone()).detach();
        let outcome = dispatch_favorites_mutation_outcome(&bus, &mut m.clone(), true);
        let events = rec.events.lock().unwrap().clone();
        (outcome, events)
    }

    fn favorites_mutation(ids: &[&str]) -> Mutation {
        Mutation {
            index: vec!["favorites".into()],
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(wa::SyncActionValue {
                favorites_action: buffa::MessageField::some(
                    wa::sync_action_value::FavoritesAction {
                        favorites: ids
                            .iter()
                            .map(|id| wa::sync_action_value::favorites_action::Favorite {
                                id: Some((*id).into()),
                            })
                            .collect(),
                    },
                ),
                timestamp: Some(1_700_000_000_000),
                ..Default::default()
            }),
        }
    }

    fn ids(event: &Event) -> Vec<String> {
        match event {
            Event::FavoritesUpdate(u) => u
                .action
                .favorites
                .iter()
                .map(|f| f.id.clone().unwrap_or_default())
                .collect(),
            other => panic!("expected FavoritesUpdate, got {other:?}"),
        }
    }

    #[test]
    fn favorites_index_matches_wa_web() {
        let index = build_action_index(&schemas::FAVORITES, &[]).unwrap();
        let parts: Vec<String> = serde_json::from_slice(&index).unwrap();
        assert_eq!(parts, vec!["favorites"]);
        assert_eq!(
            schemas::FAVORITES.collection,
            schemas::Collection::RegularHigh
        );
        assert_eq!(schemas::FAVORITES.value_field, Some("favoritesAction"));
    }

    #[test]
    fn inbound_favorites_dispatch_the_list_in_order() {
        // Deliberately not sorted: the phone's order is the user's order.
        let m = favorites_mutation(&["15550000002@s.whatsapp.net", "120363000000000042@g.us"]);
        let (outcome, events) = run(&m);
        assert_eq!(outcome, AppStateDispatchOutcome::Event("FavoritesUpdate"));
        assert_eq!(events.len(), 1);
        assert_eq!(
            ids(&events[0]),
            vec!["15550000002@s.whatsapp.net", "120363000000000042@g.us"]
        );
        match &*events[0] {
            Event::FavoritesUpdate(u) => {
                assert_eq!(u.timestamp.timestamp_millis(), 1_700_000_000_000);
                assert!(u.from_full_sync);
            }
            other => panic!("expected FavoritesUpdate, got {other:?}"),
        }
    }

    #[test]
    fn an_empty_list_still_dispatches() {
        // Unfavoriting the last chat leaves an empty list, which the
        // application must see to clear its own.
        let (outcome, events) = run(&favorites_mutation(&[]));
        assert_eq!(outcome, AppStateDispatchOutcome::Event("FavoritesUpdate"));
        assert_eq!(events.len(), 1);
        assert!(ids(&events[0]).is_empty());
    }

    #[test]
    fn favorites_without_an_action_is_malformed() {
        for action_value in [Some(wa::SyncActionValue::default()), None] {
            let m = Mutation {
                index: vec!["favorites".into()],
                operation: wa::syncd_mutation::SyncdOperation::Set,
                action_value,
            };
            let (outcome, events) = run(&m);
            assert_eq!(
                outcome,
                AppStateDispatchOutcome::Malformed("FavoritesUpdate")
            );
            assert!(events.is_empty());
        }
    }

    #[test]
    fn other_operations_and_actions_are_not_claimed() {
        let mut m = favorites_mutation(&["15550000002@s.whatsapp.net"]);
        m.operation = wa::syncd_mutation::SyncdOperation::Remove;
        let (outcome, events) = run(&m);
        assert_eq!(outcome, AppStateDispatchOutcome::Unclaimed);
        assert!(events.is_empty());

        let mut m = favorites_mutation(&["15550000002@s.whatsapp.net"]);
        m.index = vec!["quick_reply".into(), "1".into()];
        assert_eq!(run(&m).0, AppStateDispatchOutcome::Unclaimed);
    }
}
