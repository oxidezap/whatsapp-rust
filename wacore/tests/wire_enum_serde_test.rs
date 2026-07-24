//! Wire-tag invariant tests for enums that derive `WireEnum`.
//!
//! The derive owns `Serialize`/`Deserialize`, which delegate to `as_str()` /
//! `TryFrom<&str>` — so the JSON representation MUST be exactly what the
//! `#[wire = "..."]` attribute declares. No PascalCase discriminator, no
//! serde `rename_all`, no hand-written impls.
//!
//! Cases cover unit-string mode (with and without `#[wire_fallback]`),
//! int mode (see `TempBanReason` / `ConnectFailureReason` serialization as
//! i32), and the sanity check on `EditAttribute` whose wire strings diverge
//! from variant names. Tagged mode is pinned here field-by-field, and covered
//! end-to-end inside `stanza::groups::tests`.

use serde_json::json;
use wacore::iq::usync::{
    UsyncAddressingMode, UsyncContactResult, UsyncDevicesResult, UsyncFeature, UsyncOutcome,
    UsyncProtocol, UsyncProtocolResult,
};
use wacore::stanza::business::BusinessNotificationType;
use wacore::stanza::devices::DeviceNotificationType;
use wacore::stanza::groups::{GroupNotificationAction, MembershipRequestMethod};
use wacore::types::events::{
    BusinessUpdateType, ConnectFailureReason, DecryptFailMode, DeviceListUpdateType, TempBanReason,
    UnavailableType,
};
use wacore::types::lid_pn::LearningSource;
use wacore::types::message::{AddressingMode, EditAttribute, MessageCategory};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Jid;

fn assert_roundtrip<T>(values: &[T])
where
    T: serde::Serialize + for<'de> serde::Deserialize<'de> + PartialEq + std::fmt::Debug + Clone,
{
    for v in values {
        let json = serde_json::to_value(v).expect("serialize");
        let back: T = serde_json::from_value(json.clone()).expect("deserialize");
        assert_eq!(&back, v, "round-trip mismatch for JSON {json}");
    }
}

/// `serde_json` discards the field count handed to `serialize_struct`, so a
/// wrong count survives every JSON assertion in this file and only surfaces in
/// a length-prefixed format. This serializer checks the count instead of the
/// bytes.
mod field_count {
    use serde::ser::{Impossible, Serialize, SerializeStruct, Serializer};
    use std::fmt;

    #[derive(Debug, PartialEq, Eq)]
    pub struct Error(pub String);

    impl fmt::Display for Error {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str(&self.0)
        }
    }

    impl std::error::Error for Error {}

    impl serde::ser::Error for Error {
        fn custom<T: fmt::Display>(message: T) -> Self {
            Self(message.to_string())
        }
    }

    pub struct Counter {
        declared: usize,
        written: usize,
    }

    impl SerializeStruct for Counter {
        type Ok = ();
        type Error = Error;

        fn serialize_field<T: ?Sized + Serialize>(
            &mut self,
            _key: &'static str,
            _value: &T,
        ) -> Result<(), Error> {
            self.written += 1;
            Ok(())
        }

        fn end(self) -> Result<(), Error> {
            if self.declared == self.written {
                Ok(())
            } else {
                Err(Error(format!(
                    "declared {} fields, wrote {}",
                    self.declared, self.written
                )))
            }
        }
    }

    pub struct CheckFieldCount;

    macro_rules! reject {
        ($($method:ident($($arg:ty),*);)*) => {
            $(fn $method(self $(, _: $arg)*) -> Result<(), Error> {
                Err(Error(concat!(stringify!($method), " is not a tagged struct").into()))
            })*
        };
    }

    impl Serializer for CheckFieldCount {
        type Ok = ();
        type Error = Error;
        type SerializeSeq = Impossible<(), Error>;
        type SerializeTuple = Impossible<(), Error>;
        type SerializeTupleStruct = Impossible<(), Error>;
        type SerializeTupleVariant = Impossible<(), Error>;
        type SerializeMap = Impossible<(), Error>;
        type SerializeStruct = Counter;
        type SerializeStructVariant = Impossible<(), Error>;

        fn serialize_struct(self, _name: &'static str, len: usize) -> Result<Counter, Error> {
            Ok(Counter {
                declared: len,
                written: 0,
            })
        }

        reject! {
            serialize_bool(bool);
            serialize_i8(i8);
            serialize_i16(i16);
            serialize_i32(i32);
            serialize_i64(i64);
            serialize_u8(u8);
            serialize_u16(u16);
            serialize_u32(u32);
            serialize_u64(u64);
            serialize_f32(f32);
            serialize_f64(f64);
            serialize_char(char);
            serialize_str(&str);
            serialize_bytes(&[u8]);
            serialize_none();
            serialize_unit();
            serialize_unit_struct(&'static str);
            serialize_unit_variant(&'static str, u32, &'static str);
        }

        fn serialize_some<T: ?Sized + Serialize>(self, _value: &T) -> Result<(), Error> {
            Err(Error("some is not a tagged struct".into()))
        }

        fn serialize_newtype_struct<T: ?Sized + Serialize>(
            self,
            _name: &'static str,
            _value: &T,
        ) -> Result<(), Error> {
            Err(Error("newtype struct is not a tagged struct".into()))
        }

        fn serialize_newtype_variant<T: ?Sized + Serialize>(
            self,
            _name: &'static str,
            _index: u32,
            _variant: &'static str,
            _value: &T,
        ) -> Result<(), Error> {
            Err(Error("newtype variant is not a tagged struct".into()))
        }

        fn serialize_seq(self, _len: Option<usize>) -> Result<Self::SerializeSeq, Error> {
            Err(Error("seq is not a tagged struct".into()))
        }

        fn serialize_tuple(self, _len: usize) -> Result<Self::SerializeTuple, Error> {
            Err(Error("tuple is not a tagged struct".into()))
        }

        fn serialize_tuple_struct(
            self,
            _name: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleStruct, Error> {
            Err(Error("tuple struct is not a tagged struct".into()))
        }

        fn serialize_tuple_variant(
            self,
            _name: &'static str,
            _index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeTupleVariant, Error> {
            Err(Error("tuple variant is not a tagged struct".into()))
        }

        fn serialize_map(self, _len: Option<usize>) -> Result<Self::SerializeMap, Error> {
            Err(Error("map is not a tagged struct".into()))
        }

        fn serialize_struct_variant(
            self,
            _name: &'static str,
            _index: u32,
            _variant: &'static str,
            _len: usize,
        ) -> Result<Self::SerializeStructVariant, Error> {
            Err(Error("struct variant is not a tagged struct".into()))
        }
    }
}

#[test]
fn device_notification_type_uses_wire_strings() {
    for (value, expected) in [
        (DeviceNotificationType::Add, "add"),
        (DeviceNotificationType::Remove, "remove"),
        (DeviceNotificationType::Update, "update"),
    ] {
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
    assert_roundtrip(&[
        DeviceNotificationType::Add,
        DeviceNotificationType::Remove,
        DeviceNotificationType::Update,
    ]);
}

#[test]
fn device_list_update_type_uses_wire_strings() {
    for (value, expected) in [
        (DeviceListUpdateType::Add, "add"),
        (DeviceListUpdateType::Remove, "remove"),
        (DeviceListUpdateType::Update, "update"),
    ] {
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
}

#[test]
fn business_notification_type_uses_wire_strings() {
    let expected = [
        (BusinessNotificationType::RemoveJid, "remove_jid"),
        (BusinessNotificationType::RemoveHash, "remove_hash"),
        (
            BusinessNotificationType::VerifiedNameJid,
            "verified_name_jid",
        ),
        (
            BusinessNotificationType::VerifiedNameHash,
            "verified_name_hash",
        ),
        (BusinessNotificationType::Profile, "profile"),
        (BusinessNotificationType::ProfileHash, "profile_hash"),
        (BusinessNotificationType::Product, "product"),
        (BusinessNotificationType::Collection, "collection"),
        (BusinessNotificationType::Subscriptions, "subscriptions"),
        (BusinessNotificationType::Unknown, "unknown"),
    ];
    for (value, expected) in expected {
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
}

#[test]
fn business_update_type_is_snake_case() {
    assert_eq!(
        serde_json::to_value(BusinessUpdateType::RemovedAsBusiness).unwrap(),
        "removed_as_business"
    );
    assert_eq!(
        serde_json::to_value(BusinessUpdateType::VerifiedNameChanged).unwrap(),
        "verified_name_changed"
    );
    assert_eq!(
        serde_json::to_value(BusinessUpdateType::Unknown).unwrap(),
        "unknown"
    );
}

#[test]
fn decrypt_fail_mode_is_lowercase() {
    assert_eq!(serde_json::to_value(DecryptFailMode::Show).unwrap(), "show");
    assert_eq!(serde_json::to_value(DecryptFailMode::Hide).unwrap(), "hide");
}

#[test]
fn unavailable_type_is_snake_case() {
    assert_eq!(
        serde_json::to_value(UnavailableType::Unknown).unwrap(),
        "unknown"
    );
    assert_eq!(
        serde_json::to_value(UnavailableType::ViewOnce).unwrap(),
        "view_once"
    );
}

#[test]
fn addressing_mode_matches_wire() {
    assert_eq!(serde_json::to_value(AddressingMode::Pn).unwrap(), "pn");
    assert_eq!(serde_json::to_value(AddressingMode::Lid).unwrap(), "lid");
    assert_roundtrip(&[AddressingMode::Pn, AddressingMode::Lid]);
}

#[test]
fn learning_source_matches_wire() {
    assert_eq!(
        serde_json::to_value(LearningSource::Usync).unwrap(),
        "usync"
    );
    assert_eq!(
        serde_json::to_value(LearningSource::BlocklistActive).unwrap(),
        "blocklist_active"
    );
    assert_eq!(
        serde_json::to_value(LearningSource::DeviceNotification).unwrap(),
        "device_notification"
    );
}

#[test]
fn edit_attribute_uses_wire_strings_not_variant_names() {
    // Regression: variants like `MessageEdit` used to serialize as
    // `"MessageEdit"` because the enum derived `Serialize` without
    // `rename_all`, even though its wire string was `"1"`.
    assert_eq!(
        serde_json::to_value(EditAttribute::MessageEdit).unwrap(),
        "1"
    );
    assert_eq!(
        serde_json::to_value(EditAttribute::SenderRevoke).unwrap(),
        "7"
    );
    assert_eq!(serde_json::to_value(EditAttribute::Empty).unwrap(), "");
}

#[test]
fn message_category_fallback_serializes_literal() {
    assert_eq!(serde_json::to_value(MessageCategory::Peer).unwrap(), "peer");
    assert_eq!(serde_json::to_value(MessageCategory::Empty).unwrap(), "");
    assert_eq!(
        serde_json::to_value(MessageCategory::Other("custom".into())).unwrap(),
        "custom"
    );
}

#[test]
fn temp_ban_reason_serializes_as_int_and_roundtrips() {
    for (value, expected) in [
        (TempBanReason::SentToTooManyPeople, 101),
        (TempBanReason::BlockedByUsers, 102),
        (TempBanReason::CreatedTooManyGroups, 103),
        (TempBanReason::SentTooManySameMessage, 104),
        (TempBanReason::BroadcastList, 106),
        (TempBanReason::Unknown(999), 999),
    ] {
        let json = serde_json::to_value(&value).unwrap();
        assert_eq!(json, expected);
        let back: TempBanReason = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
    }
}

/// Tagged variants serialize through the struct path, so the constant field
/// names stay constant. These pin the exact JSON so a serializer swap or a
/// derive refactor cannot silently reshape the payload.
#[test]
fn group_notification_action_serializes_every_field_present() {
    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Subject {
            subject: "Team".into(),
            subject_owner: Some("555000111@s.whatsapp.net".parse::<Jid>().unwrap()),
            subject_time: Some(1_704_067_200),
        })
        .unwrap(),
        json!({
            "type": "subject",
            "subject": "Team",
            "subject_owner": {
                "user": "555000111",
                "server": "s.whatsapp.net",
                "agent": 0,
                "device": 0,
                "integrator": 0,
            },
            "subject_time": 1_704_067_200,
        })
    );

    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Ephemeral {
            expiration: 86_400,
            trigger: Some(2),
        })
        .unwrap(),
        json!({ "type": "ephemeral", "expiration": 86_400, "trigger": 2 })
    );

    assert_eq!(
        serde_json::to_value(GroupNotificationAction::CreatedMembershipRequests {
            request_method: MembershipRequestMethod::NonAdminAdd,
            parent_group_jid: Some("555000222@g.us".parse::<Jid>().unwrap()),
            requests: vec![],
        })
        .unwrap(),
        json!({
            "type": "created_membership_requests",
            "request_method": "non_admin_add",
            "parent_group_jid": {
                "user": "555000222",
                "server": "g.us",
                "agent": 0,
                "device": 0,
                "integrator": 0,
            },
            "requests": [],
        })
    );
}

#[test]
fn group_notification_action_omits_none_fields() {
    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Subject {
            subject: "Team".into(),
            subject_owner: None,
            subject_time: None,
        })
        .unwrap(),
        json!({ "type": "subject", "subject": "Team" })
    );

    // Every field optional: only the discriminator survives.
    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Locked { threshold: None }).unwrap(),
        json!({ "type": "locked" })
    );
    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Locked {
            threshold: Some("admin".into()),
        })
        .unwrap(),
        json!({ "type": "locked", "threshold": "admin" })
    );

    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Ephemeral {
            expiration: 0,
            trigger: None,
        })
        .unwrap(),
        json!({ "type": "ephemeral", "expiration": 0 })
    );
}

#[test]
fn group_notification_action_skips_unit_and_skipped_fields() {
    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Unlocked).unwrap(),
        json!({ "type": "unlocked" })
    );

    let raw = NodeBuilder::new("link").attr("link_type", "sub").build();
    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Link {
            link_type: "sub".into(),
            raw: raw.clone(),
        })
        .unwrap(),
        json!({ "type": "link", "link_type": "sub" })
    );
    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Unlink {
            unlink_type: "sub".into(),
            unlink_reason: None,
            raw,
        })
        .unwrap(),
        json!({ "type": "unlink", "unlink_type": "sub" })
    );

    // Fallback: the captured tag IS the discriminator, never an extra field.
    assert_eq!(
        serde_json::to_value(GroupNotificationAction::Unknown {
            tag: "future_tag".into(),
        })
        .unwrap(),
        json!({ "type": "future_tag" })
    );
}

#[test]
fn group_notification_action_declares_exact_field_count() {
    use serde::Serialize;

    let raw = NodeBuilder::new("link").attr("link_type", "sub").build();
    let samples = vec![
        // Mixed constant + optional, both present and absent.
        GroupNotificationAction::Subject {
            subject: "Team".into(),
            subject_owner: Some("555000111@s.whatsapp.net".parse::<Jid>().unwrap()),
            subject_time: Some(1_704_067_200),
        },
        GroupNotificationAction::Subject {
            subject: "Team".into(),
            subject_owner: None,
            subject_time: Some(1_704_067_200),
        },
        GroupNotificationAction::Subject {
            subject: "Team".into(),
            subject_owner: None,
            subject_time: None,
        },
        // Only-optional, only-skipped, unit and fallback variants.
        GroupNotificationAction::Locked {
            threshold: Some("admin".into()),
        },
        GroupNotificationAction::Locked { threshold: None },
        GroupNotificationAction::Create { raw: raw.clone() },
        GroupNotificationAction::Unlink {
            unlink_type: "sub".into(),
            unlink_reason: None,
            raw,
        },
        GroupNotificationAction::Unlocked,
        GroupNotificationAction::Unknown {
            tag: "future_tag".into(),
        },
    ];

    for action in &samples {
        action
            .serialize(field_count::CheckFieldCount)
            .unwrap_or_else(|error| panic!("{action:?}: {error}"));
    }
}

#[test]
fn field_count_checker_rejects_a_wrong_count() {
    use serde::Serialize;
    use serde::ser::SerializeStruct;

    // Guards the guard: without this, a checker that accepted anything would
    // make the test above pass silently.
    struct OverCounted;

    impl Serialize for OverCounted {
        fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
            let mut state = serializer.serialize_struct("OverCounted", 2)?;
            state.serialize_field("only", &1u8)?;
            state.end()
        }
    }

    let error = OverCounted
        .serialize(field_count::CheckFieldCount)
        .expect_err("a 2-field header with 1 field written must be rejected");
    assert_eq!(error.0, "declared 2 fields, wrote 1");
}

#[test]
fn usync_protocol_serializes_adjacently_and_roundtrips() {
    let contact = UsyncProtocol::Contact {
        addressing_mode: UsyncAddressingMode::Lid,
    };
    assert_eq!(
        serde_json::to_value(&contact).unwrap(),
        json!({ "type": "contact", "data": { "addressing_mode": "lid" } })
    );

    assert_eq!(
        serde_json::to_value(UsyncProtocol::Status).unwrap(),
        json!({ "type": "status" })
    );

    let features = UsyncProtocol::Features(vec![UsyncFeature::Document]);
    assert_eq!(
        serde_json::to_value(&features).unwrap(),
        json!({ "type": "feature", "data": ["document"] })
    );

    assert_roundtrip(&[contact, UsyncProtocol::Status, features]);
}

#[test]
fn usync_protocol_result_serializes_adjacently_and_roundtrips() {
    // `#[non_exhaustive]` payloads are built from the wire form they parse from.
    let full: UsyncContactResult =
        serde_json::from_value(json!({ "contact_type": "in", "username": "ada", "content": "1" }))
            .unwrap();
    let full = UsyncProtocolResult::Contact(UsyncOutcome::Value(full));
    assert_eq!(
        serde_json::to_value(&full).unwrap(),
        json!({
            "type": "contact",
            "data": {
                "type": "value",
                "data": { "contact_type": "in", "username": "ada", "content": "1" },
            },
        })
    );

    let sparse: UsyncContactResult =
        serde_json::from_value(json!({ "contact_type": "in" })).unwrap();
    let sparse = UsyncProtocolResult::Contact(UsyncOutcome::Value(sparse));
    assert_eq!(
        serde_json::to_value(&sparse).unwrap(),
        json!({
            "type": "contact",
            "data": { "type": "value", "data": { "contact_type": "in" } },
        })
    );

    let empty: UsyncDevicesResult = serde_json::from_value(json!({})).unwrap();
    let empty = UsyncProtocolResult::Devices(UsyncOutcome::Value(empty));
    assert_eq!(
        serde_json::to_value(&empty).unwrap(),
        json!({ "type": "devices", "data": { "type": "value", "data": {} } })
    );

    assert_roundtrip(&[full, sparse, empty]);
}

#[test]
fn connect_failure_reason_serializes_as_int_and_roundtrips() {
    for (value, expected) in [
        (ConnectFailureReason::Generic, 400),
        (ConnectFailureReason::LoggedOut, 401),
        (ConnectFailureReason::TempBanned, 402),
        (ConnectFailureReason::ServiceUnavailable, 503),
        (ConnectFailureReason::Unknown(999), 999),
    ] {
        let json = serde_json::to_value(value).unwrap();
        assert_eq!(json, expected);
        let back: ConnectFailureReason = serde_json::from_value(json).unwrap();
        assert_eq!(back, value);
    }
}
