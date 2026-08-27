//! Call history synced from the primary device via app state sync (syncd).
//!
//! Mirrors WhatsApp Web's `WAWebCallLogSync`: the `call_log` action in the
//! `regular` collection, carrying a `CallLogAction`. Collection and action
//! version come from the generated [`schemas::CALL_LOG`] registry.
//!
//! # The index carries what the record can leave out
//!
//! The index is `["call_log", callCreatorJid, callId, direction]`, not the bare
//! literal `schemas::CALL_LOG.index_parts` declares. WA Web builds it as
//! `JSON.stringify([action, ...indexArgs])` (`WAWebSyncdActionUtils.buildIndex`)
//! and `getCallLogMutation` passes `indexArgs: [d, p, m]` — the call creator,
//! the call id, and a direction flag whose meaning is not agreed on (below).
//!
//! That matters because the *record*'s `callCreatorJid` is optional and WA Web
//! leaves it unset for calls it did not receive one for, while the index's is
//! filled in either way — `d == null && (d = fromMe ? me : peerJid)`. A consumer
//! given only the record cannot always say who the call was with.
//!
//! # The direction fields disagree with each other, so neither is read
//!
//! Two fields claim to carry the call's direction, and which one is honest
//! depends on who wrote the mutation:
//!
//! - WA Web's own writer sets both from its local `fromMe`: `indexArgs: [d, p,
//!   m]` with `m = n.fromMe ? "1" : "0"`, and `isIncoming: n.fromMe`. Its
//!   `isIncoming` is therefore inverted against its own name.
//! - Mutations authored by the phone carry them the other way round, literally
//!   as `isIncoming` — a call the account placed arrives as `0`/`false`.
//!
//! Since app state fans a companion's mutations out to every other device, both
//! flavors reach this client, and no fixed reading of either field is right for
//! both.
//!
//! WA Web's reader sidesteps the whole thing, and so does this module:
//! `generateCallLogFromCallSyncRecord` destructures the record without ever
//! touching `isIncoming`, and takes direction from
//! `getCallLogTargetDetails`, which is `fromMe: isMeAccount(callCreatorWid)` —
//! the call creator compared against this account. [`CallLogSync::from_me`] is
//! derived the same way. That is also why the creator is worth having in the
//! index: it is what the direction is computed from.

use crate::appstate_sync::Mutation;
use wacore::appstate::schemas;
use wacore::types::events::{CallLogSync, Event};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

/// Dispatch inbound call-log mutations synced from the primary device.
/// Returns `true` if handled, `false` if the mutation is not a call log.
///
/// `is_own_jid` decides the call's direction and is consulted only once the
/// mutation is known to be a call log, so the app-state path pays nothing for
/// it on the mutations it is not.
pub(crate) fn dispatch_call_log_mutation(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &Mutation,
    full_sync: bool,
    is_own_jid: impl FnOnce(&Jid) -> bool,
) -> bool {
    if m.operation != wa::syncd_mutation::SyncdOperation::Set
        || m.index.first().map(String::as_str) != Some(schemas::CALL_LOG.name)
    {
        return false;
    }

    // Claimed from here on: the mutation is ours whether or not it is one we can
    // read, so returning `false` would only hand a malformed call log to
    // dispatchers that key on other indexes.
    let Some(call_creator_jid) = parse_call_creator_jid(&m.index) else {
        return true;
    };
    let Some(call_id) = m.index.get(2).cloned() else {
        log::warn!("Skipping call_log mutation: missing call id in index");
        return true;
    };
    // Direction comes from the creator; see the module docs for why not from
    // either field that claims to carry it.
    let from_me = is_own_jid(&call_creator_jid);

    // The mutation's own time, which is metadata rather than the call's: WA Web
    // measures it against the pairing timestamp to drop records that predate the
    // device. A missing one falls back to now, as it does for every other
    // app-state event — losing the whole call log over a field that is not even
    // the call's time would trade the record for its envelope, and
    // `record.start_time` is where the call's time actually is.
    let ts = m
        .action_value
        .as_ref()
        .and_then(|v| v.timestamp)
        .unwrap_or(0);
    let timestamp = wacore::time::from_millis_or_now(ts);

    let Some(record) = m
        .action_value
        .as_ref()
        .and_then(|value| value.call_log_action.as_option())
        .and_then(|action| action.call_log_record.as_option())
    else {
        log::warn!("Skipping call_log mutation for {call_id}: missing record in action value");
        return true;
    };

    event_bus.dispatch(Event::CallLogSync(
        CallLogSync::builder()
            .call_creator_jid(call_creator_jid)
            .call_id(call_id)
            .from_me(from_me)
            .timestamp(timestamp)
            .record(Box::new(record.clone()))
            .from_full_sync(full_sync)
            .build(),
    ));

    true
}

fn parse_call_creator_jid(index: &[String]) -> Option<Jid> {
    match index.get(1) {
        Some(s) => match s.parse() {
            Ok(jid) => Some(jid),
            Err(_) => {
                log::warn!("Skipping call_log mutation: malformed call creator JID '{s}'");
                None
            }
        },
        None => {
            log::warn!("Skipping call_log mutation: missing call creator JID in index");
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use wacore::types::events::{CoreEventBus, EventHandler, EventInterest, EventKind};

    #[derive(Default)]
    struct Recorder {
        events: Mutex<Vec<Arc<Event>>>,
    }

    impl EventHandler for Recorder {
        fn handle_event(&self, event: Arc<Event>) {
            self.events.lock().unwrap().push(event);
        }

        /// Narrow on purpose: subscribing to everything would pass even if
        /// `Event::kind()` mapped `CallLogSync` to the wrong kind, which is the
        /// mapping the bus filters on.
        fn interest(&self) -> EventInterest {
            EventInterest::of(&[EventKind::CallLogSync])
        }
    }

    /// This account, as either identity: a creator matching one of these is a
    /// call we placed. Stands in for `Client::is_own_jid`, which compares a JID
    /// against the device's own PN and LID.
    const OWN_PN: &str = "5511888880000@s.whatsapp.net";
    const OWN_LID: &str = "111122223333444@lid";
    /// Whoever we were talking to. Not us, under either identity.
    const PEER: &str = "5511999990000@s.whatsapp.net";

    fn is_own(jid: &Jid) -> bool {
        matches!(jid.user.as_str(), "5511888880000" | "111122223333444")
    }

    fn dispatch(mutation: &Mutation, full_sync: bool) -> (bool, Vec<Arc<Event>>) {
        dispatch_as(mutation, full_sync, is_own)
    }

    fn dispatch_as(
        mutation: &Mutation,
        full_sync: bool,
        is_own_jid: impl FnOnce(&Jid) -> bool,
    ) -> (bool, Vec<Arc<Event>>) {
        let bus = CoreEventBus::new();
        let recorder = Arc::new(Recorder::default());
        bus.subscribe_handler(recorder.clone()).detach();
        let handled = dispatch_call_log_mutation(&bus, mutation, full_sync, is_own_jid);
        let events = recorder.events.lock().unwrap().clone();
        (handled, events)
    }

    /// The shape WA Web sends: `["call_log", callCreatorJid, callId, fromMe]`.
    fn call_log_mutation(index: &[&str], record: Option<wa::CallLogRecord>) -> Mutation {
        Mutation {
            operation: wa::syncd_mutation::SyncdOperation::Set,
            index: index.iter().map(|part| (*part).to_string()).collect(),
            action_value: Some(wa::SyncActionValue {
                timestamp: Some(1_700_000_000_000),
                call_log_action: buffa::MessageField::some(wa::sync_action_value::CallLogAction {
                    call_log_record: record.into(),
                }),
                ..Default::default()
            }),
        }
    }

    /// A call this account placed: the creator is us, which is what the
    /// direction is read from. The fourth part is the writers' disputed field
    /// and is deliberately set to the value that would give the wrong answer if
    /// anything still read it as `fromMe`.
    fn full_index() -> [&'static str; 4] {
        ["call_log", OWN_PN, "call-42", "0"]
    }

    #[test]
    fn dispatches_call_log_record() {
        let record = wa::CallLogRecord {
            call_id: Some("call-42".into()),
            duration: Some(91),
            is_incoming: Some(false),
            is_video: Some(true),
            ..Default::default()
        };
        let (handled, events) = dispatch(&call_log_mutation(&full_index(), Some(record)), true);

        assert!(handled);
        assert_eq!(events.len(), 1);
        let Event::CallLogSync(update) = events[0].as_ref() else {
            panic!("expected CallLogSync event");
        };
        assert_eq!(update.record.call_id.as_deref(), Some("call-42"));
        assert_eq!(update.record.duration, Some(91));
        assert_eq!(update.record.is_video, Some(true));
        assert!(update.from_full_sync);
    }

    /// The index is the only place the call's own identity is guaranteed to be:
    /// the record's `callCreatorJid` is optional and WA Web leaves it unset for
    /// calls it did not receive one for.
    #[test]
    fn carries_the_identity_the_index_holds() {
        // A record with none of it, which is what an outbound call can arrive as.
        let (handled, events) = dispatch(
            &call_log_mutation(&full_index(), Some(wa::CallLogRecord::default())),
            false,
        );

        assert!(handled);
        let Event::CallLogSync(update) = events[0].as_ref() else {
            panic!("expected CallLogSync event");
        };
        assert_eq!(update.call_creator_jid.to_string(), OWN_PN);
        assert_eq!(update.call_id, "call-42");
        assert_eq!(update.timestamp.timestamp_millis(), 1_700_000_000_000);
    }

    /// The regression: a call this account placed reads as `from_me`.
    ///
    /// Both of the fields that claim to carry direction are set the way the
    /// phone sends them for an outbound call — index `"0"`, record
    /// `is_incoming: false` — because that combination is what made this read
    /// backwards. Direction comes from the creator, so it survives them.
    #[test]
    fn a_call_this_account_placed_is_from_me() {
        for creator in [OWN_PN, OWN_LID] {
            let record = wa::CallLogRecord {
                is_incoming: Some(false),
                ..Default::default()
            };
            let index = ["call_log", creator, "call-7", "0"];
            let (_, events) = dispatch(&call_log_mutation(&index, Some(record)), false);

            let Event::CallLogSync(update) = events[0].as_ref() else {
                panic!("expected CallLogSync event");
            };
            assert!(
                update.from_me,
                "{creator} is this account, so it placed the call"
            );
        }
    }

    /// The other direction, and the failure case for the fix: a call somebody
    /// else placed must not become ours just because the direction fields say
    /// so. WA Web's writer sets both to `fromMe`, so an inbound call it logged
    /// carries index `"0"` and `is_incoming: false` — identical to the outbound
    /// fixture above, and told apart only by the creator.
    #[test]
    fn a_call_the_peer_placed_is_not_from_me() {
        for (index_part, record_is_incoming) in [("0", Some(false)), ("1", Some(true))] {
            let record = wa::CallLogRecord {
                is_incoming: record_is_incoming,
                ..Default::default()
            };
            let index = ["call_log", PEER, "call-7", index_part];
            let (_, events) = dispatch(&call_log_mutation(&index, Some(record)), false);

            let Event::CallLogSync(update) = events[0].as_ref() else {
                panic!("expected CallLogSync event");
            };
            assert!(
                !update.from_me,
                "the creator is the peer, whatever index[3]={index_part} claims"
            );
        }
    }

    /// The fourth index part is no longer read, so a value we cannot parse is
    /// no longer a reason to drop the call: everything the event carries comes
    /// from parts we did read.
    #[test]
    fn an_unreadable_direction_part_no_longer_drops_the_call() {
        for index in [
            ["call_log", OWN_PN, "call-42", "yes"],
            ["call_log", OWN_PN, "call-42", ""],
        ] {
            let (handled, events) = dispatch(
                &call_log_mutation(&index, Some(wa::CallLogRecord::default())),
                false,
            );

            assert!(handled);
            assert_eq!(events.len(), 1, "{index:?} is still a usable call log");
            let Event::CallLogSync(update) = events[0].as_ref() else {
                panic!("expected CallLogSync event");
            };
            assert!(update.from_me);
        }
    }

    #[test]
    fn missing_record_is_claimed_without_event() {
        let (handled, events) = dispatch(&call_log_mutation(&full_index(), None), false);

        assert!(handled);
        assert!(events.is_empty());
    }

    /// An index we cannot read is still ours — handing it on would only offer it
    /// to dispatchers keyed on other indexes — but it cannot be turned into an
    /// event a consumer could trust. Only the parts the event is built from:
    /// without a creator there is no direction either.
    #[test]
    fn an_unreadable_index_is_claimed_without_event() {
        for index in [
            &["call_log"][..],
            &["call_log", PEER][..],
            &["call_log", "not a jid", "call-42", "1"][..],
        ] {
            let (handled, events) = dispatch(
                &call_log_mutation(index, Some(wa::CallLogRecord::default())),
                false,
            );

            assert!(handled, "{index:?} is a call_log mutation");
            assert!(
                events.is_empty(),
                "{index:?} must not become an event a consumer would misread"
            );
        }
    }

    #[test]
    fn unrelated_mutation_is_not_claimed() {
        let mutation = Mutation {
            operation: wa::syncd_mutation::SyncdOperation::Set,
            index: vec!["setting_pushName".into()],
            action_value: Some(wa::SyncActionValue::default()),
        };
        let (handled, events) = dispatch(&mutation, false);

        assert!(!handled);
        assert!(events.is_empty());
    }
}
