//! Opened enum fields must keep the closed-enum serde contract in every feature
//! mode (see `waproto::open_enum_serde`). Variant names, numeric repr, and
//! lowercase-name deserialization all use the enum's own derived impls. Only
//! `Unknown` values add the raw-integer form.

use waproto::whatsapp as wa;

fn set_mutation() -> wa::SyncdMutation {
    wa::SyncdMutation {
        operation: Some(wa::syncd_mutation::SyncdOperation::SET.into()),
        ..Default::default()
    }
}

#[test]
fn open_field_serializes_like_a_closed_enum() {
    let json = serde_json::to_value(set_mutation()).unwrap();

    #[cfg(feature = "serde-enum-repr")]
    assert_eq!(
        json["operation"],
        serde_json::json!(0),
        "opened field must keep the numeric repr contract, got {json}"
    );

    #[cfg(not(feature = "serde-enum-repr"))]
    assert_eq!(
        json["operation"],
        serde_json::json!("SET"),
        "opened field must keep the variant-name contract, got {json}"
    );
}

#[test]
fn open_field_serializes_unknown_as_raw_integer() {
    let mutation = wa::SyncdMutation {
        operation: Some(buffa::EnumValue::Unknown(7)),
        ..Default::default()
    };
    let json = serde_json::to_value(mutation).unwrap();
    assert_eq!(json["operation"], serde_json::json!(7));
}

#[cfg(all(feature = "serde-snake-case", not(feature = "serde-enum-repr")))]
#[test]
fn open_field_deserializes_from_lowercased_proto_name() {
    let mutation: wa::SyncdMutation =
        serde_json::from_value(serde_json::json!({"operation": "remove"})).unwrap();
    assert_eq!(
        mutation.operation,
        Some(wa::syncd_mutation::SyncdOperation::REMOVE.into())
    );
}

#[cfg(feature = "serde-deserialize")]
#[test]
fn open_field_deserializes_unknown_integer() {
    let mutation: wa::SyncdMutation =
        serde_json::from_value(serde_json::json!({"operation": 7})).unwrap();
    assert_eq!(mutation.operation, Some(buffa::EnumValue::Unknown(7)));

    let absent: wa::SyncdMutation = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(absent.operation, None);
}

#[test]
fn status_privacy_unknown_modes_survive_wire_roundtrip() {
    let action = waproto::codec::status_privacy_action_decode(&[0x08, 0x63, 0x30, 0x64])
        .expect("decode status privacy action");

    assert_eq!(action.mode, Some(buffa::EnumValue::Unknown(99)));
    assert_eq!(action.modes, vec![buffa::EnumValue::Unknown(100)]);

    let encoded = waproto::codec::status_privacy_action_to_vec(&action);
    let restored =
        waproto::codec::status_privacy_action_decode(&encoded).expect("restore status privacy");
    assert_eq!(restored, action);
}

#[test]
fn status_privacy_open_fields_keep_enum_serde_contract() {
    let action = wa::sync_action_value::StatusPrivacyAction {
        mode: Some(
            wa::sync_action_value::status_privacy_action::StatusDistributionMode::CUSTOM_LIST
                .into(),
        ),
        modes: vec![buffa::EnumValue::Unknown(99)],
        ..Default::default()
    };
    let json = serde_json::to_value(action).unwrap();

    #[cfg(feature = "serde-enum-repr")]
    assert_eq!(json["mode"], serde_json::json!(4));

    #[cfg(not(feature = "serde-enum-repr"))]
    assert_eq!(json["mode"], serde_json::json!("CUSTOM_LIST"));

    assert_eq!(json["modes"], serde_json::json!([99]));
}
