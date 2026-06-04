use serde::{Deserialize, Serialize};

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(from = "String")]
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
}
