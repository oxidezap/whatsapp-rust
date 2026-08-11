use crate::appstate_sync::Mutation;
use wacore::appstate::schemas;
use wacore::types::events::{CallLogSync, Event};
use waproto::whatsapp as wa;

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

    if let Some(value) = &m.action_value
        && let Some(action) = value.call_log_action.as_option()
        && let Some(record) = action.call_log_record.as_option()
    {
        event_bus.dispatch(Event::CallLogSync(
            CallLogSync::builder()
                .record(Box::new(record.clone()))
                .from_full_sync(full_sync)
                .build(),
        ));
    }

    true
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

    fn dispatch(mutation: &Mutation, full_sync: bool) -> (bool, Vec<Arc<Event>>) {
        let bus = CoreEventBus::new();
        let recorder = Arc::new(Recorder::default());
        bus.subscribe_handler(recorder.clone()).detach();
        let handled = dispatch_call_log_mutation(&bus, mutation, full_sync);
        let events = recorder.events.lock().unwrap().clone();
        (handled, events)
    }

    fn call_log_mutation(record: Option<wa::CallLogRecord>) -> Mutation {
        Mutation {
            operation: wa::syncd_mutation::SyncdOperation::Set,
            index: vec![schemas::CALL_LOG.name.to_string()],
            action_value: Some(wa::SyncActionValue {
                call_log_action: buffa::MessageField::some(wa::sync_action_value::CallLogAction {
                    call_log_record: record.into(),
                }),
                ..Default::default()
            }),
        }
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
        let (handled, events) = dispatch(&call_log_mutation(Some(record)), true);

        assert!(handled);
        assert_eq!(events.len(), 1);
        let Event::CallLogSync(update) = events[0].as_ref() else {
            panic!("expected CallLogSync event");
        };
        assert_eq!(update.record.call_id.as_deref(), Some("call-42"));
        assert_eq!(update.record.duration, Some(91));
        assert_eq!(update.record.is_incoming, Some(false));
        assert_eq!(update.record.is_video, Some(true));
        assert!(update.from_full_sync);
    }

    #[test]
    fn missing_record_is_claimed_without_event() {
        let (handled, events) = dispatch(&call_log_mutation(None), false);

        assert!(handled);
        assert!(events.is_empty());
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
