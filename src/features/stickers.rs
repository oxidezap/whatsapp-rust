//! Favorite and recent stickers via app state sync (syncd).
//!
//! Mirrors WhatsApp Web's `WAWebStickersFavoriteSyncAction` and
//! `WAWebStickersRemoveRecentSyncAction`: the `favoriteSticker` and
//! `removeRecentSticker` actions in the `regular_low` collection, both indexed
//! `[action, filehash]`. Collection, action version and index shape come from
//! the generated `schemas::FAVORITE_STICKER` / `schemas::REMOVE_RECENT_STICKER`
//! registry entries.
//!
//! Both are `Set`-only: WA Web marks any other operation unsupported, and
//! unfavoriting is a `Set` carrying `isFavorite = false`, not a syncd `Remove`.

use crate::appstate_sync::Mutation;
use crate::client::AppStateDispatchOutcome;
use wacore::appstate::schemas;
use wacore::types::events::{Event, FavoriteStickerUpdate, RemoveRecentStickerUpdate};
use waproto::whatsapp as wa;

/// Dispatch inbound sticker mutations synced from a linked device, returning
/// the [`crate::client::AppStateDispatchOutcome`] for the semantic
/// per-mutation log line.
pub(crate) fn dispatch_sticker_mutation_outcome(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &mut Mutation,
    event_full_sync: bool,
) -> AppStateDispatchOutcome {
    if m.operation != wa::syncd_mutation::SyncdOperation::Set {
        return AppStateDispatchOutcome::Unclaimed;
    }
    let command = m.index.first().map(String::as_str);
    let is_favorite = command == Some(schemas::FAVORITE_STICKER.name);
    if !is_favorite && command != Some(schemas::REMOVE_RECENT_STICKER.name) {
        return AppStateDispatchOutcome::Unclaimed;
    }

    // The filehash is the sticker's identity; without one (or with an empty
    // one, which WA Web's favorite handler also rejects) nothing is addressed.
    let Some(filehash) = m.index.get(1).filter(|h| !h.is_empty()).cloned() else {
        log::warn!("Skipping sticker mutation: missing filehash in index");
        return AppStateDispatchOutcome::Skipped("missing-filehash");
    };

    let ts = m
        .action_value
        .as_ref()
        .and_then(|v| v.timestamp)
        .unwrap_or(0);
    let time = wacore::time::from_millis_or_now(ts);

    if is_favorite {
        // `isFavorite` is what says which way the mutation goes, so WA Web
        // counts a `stickerAction` without it as a malformed action value.
        if let Some(val) = &mut m.action_value
            && val
                .sticker_action
                .as_option()
                .is_some_and(|a| a.is_favorite.is_some())
            && let Some(act) = val.sticker_action.take()
        {
            event_bus.dispatch(Event::FavoriteStickerUpdate(
                FavoriteStickerUpdate::builder()
                    .filehash(filehash)
                    .timestamp(time)
                    .action(Box::new(act))
                    .from_full_sync(event_full_sync)
                    .build(),
            ));
            AppStateDispatchOutcome::Event("FavoriteStickerUpdate")
        } else {
            // Warned once centrally by `report` (Malformed authority).
            AppStateDispatchOutcome::Malformed("FavoriteStickerUpdate")
        }
    } else {
        // WA Web reads `lastStickerSentTs` optionally: without it the recent
        // entry is dropped whatever its age, so a missing action is not
        // malformed. The default action carries exactly that `None`.
        let act = m
            .action_value
            .as_mut()
            .and_then(|v| v.remove_recent_sticker_action.take())
            .unwrap_or_default();
        event_bus.dispatch(Event::RemoveRecentStickerUpdate(
            RemoveRecentStickerUpdate::builder()
                .filehash(filehash)
                .timestamp(time)
                .action(Box::new(act))
                .from_full_sync(event_full_sync)
                .build(),
        ));
        AppStateDispatchOutcome::Event("RemoveRecentStickerUpdate")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::chat_actions::build_action_index;
    use std::sync::{Arc, Mutex};
    use wacore::types::events::{CoreEventBus, EventHandler, EventInterest};

    const FILEHASH: &str = "n4bQgYhMfWWaL+qgxVrQFaO/TxsrC4Is0V1sFbDwCgg=";

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
        let outcome = dispatch_sticker_mutation_outcome(&bus, &mut m.clone(), true);
        let events = rec.events.lock().unwrap().clone();
        (outcome, events)
    }

    fn favorite_value(
        mut sticker: wa::sync_action_value::StickerAction,
        favorite: bool,
    ) -> wa::SyncActionValue {
        sticker.is_favorite = Some(favorite);
        wa::SyncActionValue {
            sticker_action: buffa::MessageField::some(sticker),
            timestamp: Some(1_700_000_000_000),
            ..Default::default()
        }
    }

    fn sticker() -> wa::sync_action_value::StickerAction {
        wa::sync_action_value::StickerAction {
            direct_path: Some("/v/t62.15575-24/sticker.enc".into()),
            media_key: Some(vec![7; 32]),
            file_enc_sha256: Some(vec![9; 32]),
            mimetype: Some("image/webp".into()),
            width: Some(512),
            height: Some(512),
            ..Default::default()
        }
    }

    #[test]
    fn sticker_indexes_match_wa_web() {
        // Both sync actions pass indexArgs `[filehash]`.
        for schema in [&schemas::FAVORITE_STICKER, &schemas::REMOVE_RECENT_STICKER] {
            let index = build_action_index(schema, &[FILEHASH]).unwrap();
            let parts: Vec<String> = serde_json::from_slice(&index).unwrap();
            assert_eq!(parts, vec![schema.name, FILEHASH]);
            assert_eq!(schema.collection, schemas::Collection::RegularLow);
            assert_eq!(schema.version, 7);
        }
        assert_eq!(schemas::FAVORITE_STICKER.value_field, Some("stickerAction"));
    }

    #[test]
    fn inbound_favorite_dispatches_update_with_media_fields() {
        for favorite in [true, false] {
            let m = Mutation {
                index: vec!["favoriteSticker".into(), FILEHASH.into()],
                operation: wa::syncd_mutation::SyncdOperation::Set,
                action_value: Some(favorite_value(
                    wa::sync_action_value::StickerAction {
                        device_id_hint: Some(3),
                        ..sticker()
                    },
                    favorite,
                )),
            };
            let (outcome, events) = run(&m);
            assert_eq!(
                outcome,
                AppStateDispatchOutcome::Event("FavoriteStickerUpdate")
            );
            assert_eq!(events.len(), 1);
            match &*events[0] {
                Event::FavoriteStickerUpdate(u) => {
                    assert_eq!(u.filehash, FILEHASH);
                    assert_eq!(u.action.is_favorite, Some(favorite));
                    assert_eq!(
                        u.action.direct_path.as_deref(),
                        Some("/v/t62.15575-24/sticker.enc")
                    );
                    assert_eq!(u.action.media_key.as_deref(), Some(&[7u8; 32][..]));
                    assert_eq!(u.action.device_id_hint, Some(3));
                    assert!(u.from_full_sync);
                }
                other => panic!("expected FavoriteStickerUpdate, got {other:?}"),
            }
        }
    }

    #[test]
    fn favorite_without_is_favorite_is_malformed() {
        // WA Web counts a stickerAction missing isFavorite as a malformed
        // action value: it cannot tell which way the mutation goes.
        for action_value in [
            Some(wa::SyncActionValue {
                sticker_action: buffa::MessageField::some(sticker()),
                ..Default::default()
            }),
            Some(wa::SyncActionValue::default()),
            None,
        ] {
            let m = Mutation {
                index: vec!["favoriteSticker".into(), FILEHASH.into()],
                operation: wa::syncd_mutation::SyncdOperation::Set,
                action_value,
            };
            let (outcome, events) = run(&m);
            assert_eq!(
                outcome,
                AppStateDispatchOutcome::Malformed("FavoriteStickerUpdate")
            );
            assert!(events.is_empty());
        }
    }

    #[test]
    fn inbound_remove_recent_dispatches_update() {
        let m = Mutation {
            index: vec!["removeRecentSticker".into(), FILEHASH.into()],
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(wa::SyncActionValue {
                remove_recent_sticker_action: buffa::MessageField::some(
                    wa::sync_action_value::RemoveRecentStickerAction {
                        last_sticker_sent_ts: Some(1_700_000_000_000),
                    },
                ),
                timestamp: Some(1_700_000_000_500),
                ..Default::default()
            }),
        };
        let (outcome, events) = run(&m);
        assert_eq!(
            outcome,
            AppStateDispatchOutcome::Event("RemoveRecentStickerUpdate")
        );
        match &*events[0] {
            Event::RemoveRecentStickerUpdate(u) => {
                assert_eq!(u.filehash, FILEHASH);
                assert_eq!(u.action.last_sticker_sent_ts, Some(1_700_000_000_000));
                assert_eq!(u.timestamp.timestamp_millis(), 1_700_000_000_500);
            }
            other => panic!("expected RemoveRecentStickerUpdate, got {other:?}"),
        }
    }

    #[test]
    fn remove_recent_without_an_action_removes_unconditionally() {
        // WA Web treats a missing `lastStickerSentTs` as "drop whatever is
        // there", so the event still fires, carrying `None`.
        for action_value in [Some(wa::SyncActionValue::default()), None] {
            let m = Mutation {
                index: vec!["removeRecentSticker".into(), FILEHASH.into()],
                operation: wa::syncd_mutation::SyncdOperation::Set,
                action_value,
            };
            let (outcome, events) = run(&m);
            assert_eq!(
                outcome,
                AppStateDispatchOutcome::Event("RemoveRecentStickerUpdate")
            );
            match &*events[0] {
                Event::RemoveRecentStickerUpdate(u) => {
                    assert_eq!(u.action.last_sticker_sent_ts, None);
                }
                other => panic!("expected RemoveRecentStickerUpdate, got {other:?}"),
            }
        }
    }

    #[test]
    fn missing_filehash_and_other_operations_are_not_dispatched() {
        for index in [
            vec!["favoriteSticker".to_string()],
            vec!["removeRecentSticker".to_string(), String::new()],
        ] {
            let m = Mutation {
                index,
                operation: wa::syncd_mutation::SyncdOperation::Set,
                action_value: Some(favorite_value(sticker(), true)),
            };
            let (outcome, events) = run(&m);
            assert_eq!(
                outcome,
                AppStateDispatchOutcome::Skipped("missing-filehash")
            );
            assert!(events.is_empty());
        }

        // WA Web marks anything but a Set unsupported.
        let m = Mutation {
            index: vec!["favoriteSticker".into(), FILEHASH.into()],
            operation: wa::syncd_mutation::SyncdOperation::Remove,
            action_value: Some(favorite_value(sticker(), true)),
        };
        assert_eq!(run(&m).0, AppStateDispatchOutcome::Unclaimed);

        let m = Mutation {
            index: vec!["quick_reply".into(), "1".into()],
            operation: wa::syncd_mutation::SyncdOperation::Set,
            action_value: Some(wa::SyncActionValue::default()),
        };
        assert_eq!(run(&m).0, AppStateDispatchOutcome::Unclaimed);
    }
}
