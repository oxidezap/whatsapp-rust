//! The opened `SyncdMutation.operation` field must keep the closed-enum serde
//! contract in every feature mode (see `waproto::open_enum_serde`): variant
//! name / numeric repr / lowercase-name deserialize are all routed through the
//! enum's own derived impls; only `Unknown` values add the raw-integer form.

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
        operation: Some(waproto::buffa::EnumValue::Unknown(7)),
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
    assert_eq!(
        mutation.operation,
        Some(waproto::buffa::EnumValue::Unknown(7))
    );

    let absent: wa::SyncdMutation = serde_json::from_value(serde_json::json!({})).unwrap();
    assert_eq!(absent.operation, None);
}
