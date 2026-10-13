use whatsapp_rust::types::events::{
    Event, EventInterest, EventKind, LabelSublistChange, LabelSublistUpdate,
};

#[test]
fn sublist_event_is_extensible_constructible_and_selectable() {
    let update = LabelSublistUpdate::builder()
        .predefined_id(11)
        .chat_jid("12025550111@s.whatsapp.net".parse().unwrap())
        .change(LabelSublistChange::Upsert { sub_list_id: 0 })
        .from_full_sync(true)
        .build();
    let LabelSublistUpdate {
        predefined_id,
        change,
        ..
    } = &update;
    assert_eq!(*predefined_id, 11);
    let stage = match change {
        LabelSublistChange::Upsert { sub_list_id } => Some(*sub_list_id),
        LabelSublistChange::Remove => None,
        _ => panic!("unrecognized future change"),
    };
    assert_eq!(stage, Some(0));
    let json = serde_json::to_value(&update).unwrap();
    assert_eq!(json["action_timestamp"], serde_json::Value::Null);
    assert_eq!(json["change"]["Upsert"]["sub_list_id"], 0);
    let event = Event::LabelSublistUpdate(update);
    assert_eq!(event.kind(), EventKind::LabelSublistUpdate);
    assert!(EventInterest::of(&[EventKind::LabelSublistUpdate]).wants(event.kind()));
    assert!(!EventInterest::of(&[EventKind::LabelAssociationUpdate]).wants(event.kind()));
}
