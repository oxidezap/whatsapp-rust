//! Call history synced from the primary device via app state sync (syncd).
//!
//! Mirrors WhatsApp Web's `WAWebCallLogSync`: the `call_log` action in the
//! `regular` collection, carrying a `CallLogAction`. Collection and action
//! version come from the generated [`schemas::CALL_LOG`] registry.
//!
//! # The index carries what the record can leave out
//!
//! The index is `["call_log", callCreatorJid, callId, fromMe]`, not the bare
//! literal `schemas::CALL_LOG.index_parts` declares. WA Web builds it as
//! `JSON.stringify([action, ...indexArgs])` (`WAWebSyncdActionUtils.buildIndex`)
//! and `getCallLogMutation` passes `indexArgs: [d, p, m]` — the call creator, the
//! call id, and whether this account placed the call.
//!
//! That matters because the *record*'s `callCreatorJid` is optional and WA Web
//! leaves it unset for calls it did not receive one for, while the index's is
//! filled in either way — `d == null && (d = fromMe ? me : peerJid)`. A consumer
//! given only the record cannot always say who the call was with.
//!
//! # `is_incoming` does not mean what it says
//!
//! WA Web writes the record with `isIncoming: n.fromMe`, so the field carries
//! "this account placed the call" — the opposite of its name. The index's
//! `fromMe` is the same value under an honest name, which is why
//! [`CallLogSync::from_me`] comes from there and is the field to trust.

use crate::appstate_sync::Mutation;
use wacore::appstate::schemas;
use wacore::types::events::{CallLogSync, Event};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

/// Dispatch inbound call-log mutations synced from the primary device.
/// Returns `true` if handled, `false` if the mutation is not a call log.
pub(crate) fn dispatch_call_log_mutation(
    event_bus: &wacore::types::events::CoreEventBus,
    m: &Mutation,
    full_sync: bool,
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
    // WA Web writes "1"/"0" (`m = n.fromMe ? "1" : "0"`). Anything else is a
    // shape we do not know how to read, and guessing would mislabel the call.
    let from_me = match m.index.get(3).map(String::as_str) {
        Some("1") => true,
        Some("0") => false,
        other => {
            log::warn!("Skipping call_log mutation: unreadable fromMe {other:?} in index");
            return true;
        }
    };

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

    fn dispatch(mutation: &Mutation, full_sync: bool) -> (bool, Vec<Arc<Event>>) {
        let bus = CoreEventBus::new();
        let recorder = Arc::new(Recorder::default());
        bus.subscribe_handler(recorder.clone()).detach();
        let handled = dispatch_call_log_mutation(&bus, mutation, full_sync);
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

    fn full_index() -> [&'static str; 4] {
        ["call_log", "5511999990000@s.whatsapp.net", "call-42", "1"]
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
        assert_eq!(
            update.call_creator_jid.to_string(),
            "5511999990000@s.whatsapp.net"
        );
        assert_eq!(update.call_id, "call-42");
        assert!(
            update.from_me,
            "the index says this account placed the call"
        );
        assert_eq!(update.timestamp.timestamp_millis(), 1_700_000_000_000);
    }

    /// WA Web writes the record's `isIncoming` from its local `fromMe`, so the
    /// two disagree by name and agree by value. A consumer trusting the field
    /// name would file every call backwards; `from_me` is the honest one. Both
    /// directions, so neither a constant nor the record can stand in for it.
    #[test]
    fn from_me_comes_from_the_index_not_the_record() {
        for (index_from_me, record_is_incoming, expected) in
            [("0", Some(true), false), ("1", Some(false), true)]
        {
            let record = wa::CallLogRecord {
                is_incoming: record_is_incoming,
                ..Default::default()
            };
            let index = [
                "call_log",
                "5511999990000@s.whatsapp.net",
                "call-7",
                index_from_me,
            ];
            let (_, events) = dispatch(&call_log_mutation(&index, Some(record)), false);

            let Event::CallLogSync(update) = events[0].as_ref() else {
                panic!("expected CallLogSync event");
            };
            assert_eq!(
                update.from_me, expected,
                "the index is authoritative, and it says fromMe={index_from_me}"
            );
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
    /// event a consumer could trust.
    #[test]
    fn an_unreadable_index_is_claimed_without_event() {
        for index in [
            &["call_log"][..],
            &["call_log", "5511999990000@s.whatsapp.net"][..],
            &["call_log", "5511999990000@s.whatsapp.net", "call-42"][..],
            &["call_log", "5511999990000@s.whatsapp.net", "call-42", "yes"][..],
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
