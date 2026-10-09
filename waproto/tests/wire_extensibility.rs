//! Synthetic bytes exercise the persisted/wire contract independently of the
//! construction and matching restrictions checked by generated_api.
#![allow(clippy::disallowed_methods)]

use waproto::buffa::{Message, MessageView};
use waproto::whatsapp as wa;

#[test]
fn unknown_message_fields_survive_owned_and_view_edits() {
    // conversation = "old", then unknown field 1000 = 7.
    let wire = [0x0a, 3, b'o', b'l', b'd', 0xc0, 0x3e, 7];
    let mut message = wa::Message::decode_from_slice(&wire).unwrap();
    message.conversation = Some("new".into());
    let expected = [0x0a, 3, b'n', b'e', b'w', 0xc0, 0x3e, 7];
    assert_eq!(message.encode_to_vec(), expected);
    let handle = wa::MessageOwnedView::decode(wire.to_vec().into()).unwrap();
    let mut restored = handle.to_owned_message();
    restored.conversation = Some("new".into());
    assert_eq!(restored.encode_to_vec(), expected);
    let view = wa::MessageView::decode_view(&wire).unwrap();
    let mut message = view.to_owned_message().unwrap();
    message.conversation = Some("new".into());
    assert_eq!(message.encode_to_vec(), expected);
    let json = serde_json::to_value(message).unwrap();
    assert_eq!(json["conversation"], "new");
    assert!(
        json.as_object()
            .unwrap()
            .keys()
            .all(|key| !key.starts_with("__"))
    );
}

#[test]
fn unknown_closed_enum_value_survives_roundtrip() {
    // ADVDeviceIdentity.accountType = 99. A closed enum exposes no fabricated
    // known value, but its original number must survive a subsequent encode.
    let wire = [0x20, 99];
    let message = wa::ADVDeviceIdentity::decode_from_slice(&wire).unwrap();
    assert_eq!(message.account_type, None);
    assert_eq!(message.encode_to_vec(), wire);
    let view = wa::ADVDeviceIdentityView::decode_view(&wire).unwrap();
    assert_eq!(view.to_owned_message().unwrap().encode_to_vec(), wire);
}

#[test]
fn local_seed_and_future_persisted_fields_survive() {
    use wa::session_structure::chain::MessageKey;
    assert_eq!(
        waproto::tags::session_structure::chain::message_key::SEED,
        100
    );
    // Locally retained seed = [1, 2, 3], then a future persisted field 101 = 7.
    let wire = [0xa2, 0x06, 3, 1, 2, 3, 0xa8, 0x06, 7];
    let key = MessageKey::decode_from_slice(&wire).unwrap();
    assert_eq!(key.seed.as_deref(), Some([1, 2, 3].as_slice()));
    assert_eq!(key.encode_to_vec(), wire);
    let json = serde_json::to_value(key).unwrap();
    assert!(json.get("seed").is_none());
    assert!(json.get("__buffa_unknown_fields").is_none());
}

#[test]
fn retained_lid_mapping_keeps_wire_identity() {
    let mut mapping = wa::LIDMigrationMapping::default();
    mapping.pn = 15_550_000_001;
    mapping.assigned_lid = 90_000_000_001;
    mapping.latest_lid = Some(90_000_000_002);
    let mut payload = wa::LIDMigrationMappingSyncPayload::default();
    payload.pn_to_lid_mappings.push(mapping);
    payload.chat_db_migration_timestamp = Some(123);
    let wire = payload.encode_to_vec();
    assert_eq!(
        wa::LIDMigrationMappingSyncPayload::decode_from_slice(&wire).unwrap(),
        payload
    );
}
