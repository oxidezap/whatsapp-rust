use serde::{Deserialize, Serialize};

/// Global online/offline availability, shared by incoming events and outgoing presence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
#[non_exhaustive]
pub enum PresenceStatus {
    #[wire = "available"]
    Available,
    #[wire = "unavailable"]
    Unavailable,
}

/// Activity within one chat, independently of global availability.
///
/// Serde names describe activity, not literal stanza tags. Recording audio projects
/// to `<composing media="audio"/>`; idle projects to `<paused/>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ChatActivity {
    Typing,
    RecordingAudio,
    #[default]
    Idle,
}

/// Receipt semantics. Serde uses Rust variant names, independently of wire `type` values.
/// Unknown values retain their payload in `{ "Other": "..." }`, even for known-name collisions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ReceiptType {
    Delivered,
    /// Sent but NOT delivered: WA Web downgrades a delivery ack to this when the
    /// receipt carries `<error reason="lid" type="feature-incapable">` (the LID peer
    /// can't receive the message). Produced by the receipt parser, not sent by us.
    Sent,
    Sender,
    Retry,
    /// VoIP call encryption re-keying retry.
    ///
    /// WA Web: `ENC_RETRY_RECEIPT_ATTRS.GROUP_CALL = "enc_rekey_retry"`.
    /// Sent when a peer fails to decrypt VoIP call encryption data and
    /// needs the sender to re-key.  Uses `<enc_rekey>` child (with
    /// `call-creator`, `call-id`, `count`) instead of `<retry>`.
    EncRekeyRetry,
    Read,
    ReadSelf,
    Played,
    PlayedSelf,
    ServerError,
    Inactive,
    PeerMsg,
    HistorySync,
    Other(String),
}

impl ReceiptType {
    /// Single source of truth for the wire-string -> known-variant mapping
    /// (the inverse of [`Self::as_wire_str`]). Returns `None` for an
    /// unrecognized value so callers can decide how to build `Other` (clone vs
    /// move) without duplicating the match.
    fn from_known(s: &str) -> Option<Self> {
        Some(match s {
            "" | "delivery" => Self::Delivered,
            "sent" => Self::Sent,
            "sender" => Self::Sender,
            "retry" => Self::Retry,
            "enc_rekey_retry" => Self::EncRekeyRetry,
            "read" => Self::Read,
            "read-self" => Self::ReadSelf,
            "played" => Self::Played,
            "played-self" => Self::PlayedSelf,
            "server-error" => Self::ServerError,
            "inactive" => Self::Inactive,
            "peer_msg" => Self::PeerMsg,
            "hist_sync" => Self::HistorySync,
            _ => return None,
        })
    }

    /// Parse the wire `type` value. Empty or `delivery` means delivered.
    /// This is independent of the externally tagged Serde representation.
    pub fn parse(s: &str) -> Self {
        Self::from_known(s).unwrap_or_else(|| Self::Other(s.to_string()))
    }

    /// Canonical wire `type` value for known variants; the raw payload for `Other`.
    /// `Delivered` maps to `"delivery"`, though it is sent without a type attribute.
    /// Parsing an `Other` payload that collides with a known wire value normalizes
    /// it to that known variant. Use Serde when exact enum roundtrips are required.
    pub fn as_wire_str(&self) -> &str {
        match self {
            Self::Delivered => "delivery",
            Self::Sent => "sent",
            Self::Sender => "sender",
            Self::Retry => "retry",
            Self::EncRekeyRetry => "enc_rekey_retry",
            Self::Read => "read",
            Self::ReadSelf => "read-self",
            Self::Played => "played",
            Self::PlayedSelf => "played-self",
            Self::ServerError => "server-error",
            Self::Inactive => "inactive",
            Self::PeerMsg => "peer_msg",
            Self::HistorySync => "hist_sync",
            Self::Other(s) => s,
        }
    }
}

impl From<String> for ReceiptType {
    fn from(s: String) -> Self {
        // Reuse the owned `s` for the `Other` fallback (no extra allocation).
        match Self::from_known(&s) {
            Some(known) => known,
            None => Self::Other(s),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::ReceiptType;

    #[test]
    fn receipt_type_maps_delivery_string_to_delivered() {
        assert_eq!(ReceiptType::from("".to_string()), ReceiptType::Delivered);
        assert_eq!(
            ReceiptType::from("delivery".to_string()),
            ReceiptType::Delivered
        );
    }

    #[test]
    fn receipt_type_maps_retry_variants() {
        assert_eq!(ReceiptType::from("retry".to_string()), ReceiptType::Retry);
        assert_eq!(
            ReceiptType::from("enc_rekey_retry".to_string()),
            ReceiptType::EncRekeyRetry
        );
    }

    #[test]
    fn as_wire_str_round_trips_through_parse() {
        // as_wire_str is the hand-maintained inverse of parse(); guard the
        // hyphen/underscore variants against drift.
        let variants = [
            ReceiptType::Delivered,
            ReceiptType::Sent,
            ReceiptType::Sender,
            ReceiptType::Retry,
            ReceiptType::EncRekeyRetry,
            ReceiptType::Read,
            ReceiptType::ReadSelf,
            ReceiptType::Played,
            ReceiptType::PlayedSelf,
            ReceiptType::ServerError,
            ReceiptType::Inactive,
            ReceiptType::PeerMsg,
            ReceiptType::HistorySync,
        ];
        for v in variants {
            assert_eq!(
                ReceiptType::parse(v.as_wire_str()),
                v,
                "round-trip failed for {v:?} (wire={:?})",
                v.as_wire_str()
            );
        }
        let other = ReceiptType::Other("custom-type".to_string());
        assert_eq!(other.as_wire_str(), "custom-type");
    }

    /// Every unit variant paired with the exact JSON the previous
    /// `#[derive(Serialize)]` produced.
    const UNIT_VARIANTS: [(ReceiptType, &str); 13] = [
        (ReceiptType::Delivered, "Delivered"),
        (ReceiptType::Sent, "Sent"),
        (ReceiptType::Sender, "Sender"),
        (ReceiptType::Retry, "Retry"),
        (ReceiptType::EncRekeyRetry, "EncRekeyRetry"),
        (ReceiptType::Read, "Read"),
        (ReceiptType::ReadSelf, "ReadSelf"),
        (ReceiptType::Played, "Played"),
        (ReceiptType::PlayedSelf, "PlayedSelf"),
        (ReceiptType::ServerError, "ServerError"),
        (ReceiptType::Inactive, "Inactive"),
        (ReceiptType::PeerMsg, "PeerMsg"),
        (ReceiptType::HistorySync, "HistorySync"),
    ];

    #[test]
    fn unit_variants_serialize_to_their_variant_name() {
        for (variant, expected) in &UNIT_VARIANTS {
            assert_eq!(
                serde_json::to_value(variant).expect("serialization is infallible"),
                serde_json::Value::String((*expected).to_string()),
                "serialized form drift for {variant:?}"
            );
        }
    }

    #[test]
    fn other_serializes_as_a_newtype_variant() {
        let other = ReceiptType::Other("custom-type".to_string());
        assert_eq!(
            serde_json::to_value(&other).expect("serialization is infallible"),
            serde_json::json!({ "Other": "custom-type" })
        );

        // An empty payload still nests under the variant name rather than
        // collapsing to a bare string.
        let empty = ReceiptType::Other(String::new());
        assert_eq!(
            serde_json::to_value(&empty).expect("serialization is infallible"),
            serde_json::json!({ "Other": "" })
        );
    }

    #[test]
    fn serde_roundtrip_preserves_every_variant_and_other_payload() {
        for (variant, _) in &UNIT_VARIANTS {
            let json = serde_json::to_string(variant).unwrap();
            assert_eq!(
                serde_json::from_str::<ReceiptType>(&json).unwrap(),
                *variant
            );
        }
        for payload in [
            "",
            "custom-type",
            "ReadSelf",
            "Delivered",
            "Other",
            "read-self",
            "delivery",
        ] {
            let variant = ReceiptType::Other(payload.to_owned());
            let json = serde_json::to_value(&variant).unwrap();
            assert_eq!(json, serde_json::json!({ "Other": payload }));
            assert_eq!(
                serde_json::from_value::<ReceiptType>(json).unwrap(),
                variant
            );
        }
    }

    #[test]
    fn serde_names_are_separate_from_wire_values() {
        assert_eq!(
            ReceiptType::parse("ReadSelf"),
            ReceiptType::Other("ReadSelf".into())
        );
        assert_eq!(ReceiptType::parse("read-self"), ReceiptType::ReadSelf);
        assert!(serde_json::from_str::<ReceiptType>(r#""read-self""#).is_err());
    }
}
