use chrono::{DateTime, Utc};
use whatsapp_rust::types::events::{
    LabelAssociationUpdate, LabelEditUpdate, MessageLabelAssociationUpdate,
};

#[test]
fn existing_label_builders_keep_the_optional_timestamp_unset() {
    let timestamp = DateTime::<Utc>::from_timestamp_millis(0).unwrap();
    let edit = LabelEditUpdate::builder()
        .label_id("5".into())
        .timestamp(timestamp)
        .action(Box::default())
        .from_full_sync(false)
        .build();
    let association = LabelAssociationUpdate::builder()
        .label_id("5".into())
        .chat_jid("12025550111@s.whatsapp.net".parse().unwrap())
        .timestamp(timestamp)
        .action(Box::default())
        .from_full_sync(false)
        .build();
    let message = MessageLabelAssociationUpdate::builder()
        .label_id("5".into())
        .chat_jid("12025550111@s.whatsapp.net".parse().unwrap())
        .message_id("MSGID".into())
        .timestamp(timestamp)
        .action(Box::default())
        .from_full_sync(false)
        .build();
    assert_eq!(edit.action_timestamp, None);
    assert_eq!(association.action_timestamp, None);
    assert_eq!(message.action_timestamp, None);
    for json in [
        serde_json::to_value(edit).unwrap(),
        serde_json::to_value(association).unwrap(),
        serde_json::to_value(message).unwrap(),
    ] {
        assert_eq!(json["timestamp"], "1970-01-01T00:00:00Z");
        assert_eq!(json.get("action_timestamp"), Some(&serde_json::Value::Null));
    }
}

#[test]
fn label_builders_accept_carried_and_optional_timestamps() {
    let timestamp = DateTime::<Utc>::from_timestamp_millis(0).unwrap();
    let edit = LabelEditUpdate::builder()
        .label_id("5".into())
        .timestamp(timestamp)
        .action_timestamp(timestamp)
        .action(Box::default())
        .from_full_sync(true)
        .build();
    let association = LabelAssociationUpdate::builder()
        .label_id("5".into())
        .chat_jid("12025550111@s.whatsapp.net".parse().unwrap())
        .timestamp(timestamp)
        .maybe_action_timestamp(Some(timestamp))
        .action(Box::default())
        .from_full_sync(true)
        .build();
    let message = MessageLabelAssociationUpdate::builder()
        .label_id("5".into())
        .chat_jid("12025550111@s.whatsapp.net".parse().unwrap())
        .message_id("MSGID".into())
        .timestamp(timestamp)
        .maybe_action_timestamp(Some(timestamp))
        .action(Box::default())
        .from_full_sync(true)
        .build();
    assert_eq!(edit.action_timestamp, Some(timestamp));
    assert_eq!(association.action_timestamp, Some(timestamp));
    assert_eq!(message.action_timestamp, Some(timestamp));
    for json in [
        serde_json::to_value(edit).unwrap(),
        serde_json::to_value(association).unwrap(),
        serde_json::to_value(message).unwrap(),
    ] {
        assert_eq!(json["action_timestamp"], "1970-01-01T00:00:00Z");
        assert_eq!(json["timestamp"], "1970-01-01T00:00:00Z");
    }
}
