use serde::{Deserialize, Serialize, Serializer};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Presence {
    Available,
    Unavailable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatPresence {
    Composing,
    Paused,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ChatPresenceMedia {
    #[serde(rename = "")]
    #[default]
    Text,
    #[serde(rename = "audio")]
    Audio,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(from = "String")]
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

    pub fn parse(s: &str) -> Self {
        Self::from_known(s).unwrap_or_else(|| Self::Other(s.to_string()))
    }

    /// Canonical wire `type` value. Inverse of [`Self::parse`] (`Delivered`
    /// maps to `"delivery"`, though it is sent as a dropped attr in practice).
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

    /// Variant name as it appears in the serde form (`"ReadSelf"`, not the
    /// wire `"read-self"`). Exposed so consumers that observe the serialized
    /// event shape can name a variant without round-tripping through a
    /// serializer or duplicating the list of this `#[non_exhaustive]` enum.
    /// The [`Serialize`] impl is built on it, so the two cannot drift.
    pub fn variant_name(&self) -> &'static str {
        match self {
            Self::Delivered => "Delivered",
            Self::Sent => "Sent",
            Self::Sender => "Sender",
            Self::Retry => "Retry",
            Self::EncRekeyRetry => "EncRekeyRetry",
            Self::Read => "Read",
            Self::ReadSelf => "ReadSelf",
            Self::Played => "Played",
            Self::PlayedSelf => "PlayedSelf",
            Self::ServerError => "ServerError",
            Self::Inactive => "Inactive",
            Self::PeerMsg => "PeerMsg",
            Self::HistorySync => "HistorySync",
            Self::Other(_) => "Other",
        }
    }
}

impl Serialize for ReceiptType {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        // Indices follow declaration order so the output stays byte-identical
        // to the derive this replaced.
        let index = match self {
            Self::Delivered => 0,
            Self::Sent => 1,
            Self::Sender => 2,
            Self::Retry => 3,
            Self::EncRekeyRetry => 4,
            Self::Read => 5,
            Self::ReadSelf => 6,
            Self::Played => 7,
            Self::PlayedSelf => 8,
            Self::ServerError => 9,
            Self::Inactive => 10,
            Self::PeerMsg => 11,
            Self::HistorySync => 12,
            Self::Other(_) => 13,
        };
        match self {
            Self::Other(inner) => {
                serializer.serialize_newtype_variant("ReceiptType", index, "Other", inner)
            }
            _ => serializer.serialize_unit_variant("ReceiptType", index, self.variant_name()),
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
                variant.variant_name(),
                *expected,
                "name drift for {variant:?}"
            );
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
        assert_eq!(other.variant_name(), "Other");
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
    fn variant_name_does_not_leak_the_wire_form() {
        // The two mappings differ on purpose; guard against one being wired
        // to the other.
        assert_eq!(ReceiptType::ReadSelf.variant_name(), "ReadSelf");
        assert_eq!(ReceiptType::ReadSelf.as_wire_str(), "read-self");
        assert_eq!(ReceiptType::Delivered.variant_name(), "Delivered");
        assert_eq!(ReceiptType::Delivered.as_wire_str(), "delivery");
        assert_eq!(
            ReceiptType::Other("custom-type".to_string()).variant_name(),
            "Other"
        );
    }
}
