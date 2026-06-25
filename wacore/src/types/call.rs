use chrono::{DateTime, Utc};
use serde::Serialize;
use wacore_binary::Jid;

/// The encrypted callKey + parsed relay carried by an `<offer>`, captured so the media facade can
/// decrypt the callKey and connect the relay without re-walking the raw stanza. Binary/media-only,
/// so it is kept off the `serde` shape (downstream JS consumers see only the signaling fields).
/// Behind the `voip` feature: it carries the parsed `RelayData`, which lives in `crate::voip`.
#[cfg(feature = "voip")]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct MediaOffer {
    /// The `<enc>` blocks carrying the Signal-encrypted callKey. A single-device offer has one entry
    /// addressed to us directly (`to: None`, a bare `<enc>` child); a multi-device offer carries one
    /// per `<destination><to jid>`, and [`enc_for`](Self::enc_for) selects the one for this device.
    pub encs: Vec<OfferRecipientEnc>,
    /// The parsed `<relay>` block (endpoints + crypto material), when the offer carried one.
    pub relay: Option<crate::voip::relay_parse::RelayData>,
}

#[cfg(feature = "voip")]
impl MediaOffer {
    /// The callKey `<enc>` to decrypt for our own device: the entry whose `<to jid>` equals
    /// `own_jid`, else the single unaddressed entry (a bare `<enc>` child targeting us directly).
    /// `None` when the offer carried no enc we can use (a multi-device offer that doesn't list us).
    pub fn enc_for(&self, own_jid: Option<&Jid>) -> Option<&OfferEnc> {
        if let Some(own) = own_jid
            && let Some(matched) = self.encs.iter().find(|e| e.to.as_ref() == Some(own))
        {
            return Some(&matched.enc);
        }
        match self.encs.as_slice() {
            [only] if only.to.is_none() => Some(&only.enc),
            _ => None,
        }
    }
}

/// One per-recipient `<enc>` from an `<offer>`: the Signal ciphertext plus the `<to jid>` it was
/// addressed to (`None` for a bare `<enc>` child on a single-device offer).
#[cfg(feature = "voip")]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct OfferRecipientEnc {
    pub to: Option<Jid>,
    pub enc: OfferEnc,
}

/// The `<enc>` child of an `<offer>` addressed to this device: the Signal ciphertext of the
/// callKey message plus the wire `type`/`v` needed to decrypt and unpad it.
#[cfg(feature = "voip")]
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct OfferEnc {
    /// Signal message type wire string (`pkmsg` or `msg`).
    pub enc_type: String,
    /// `v` attr (padding version); defaults to 2 when absent.
    pub version: u8,
    pub ciphertext: Vec<u8>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CallAudioCodec {
    pub enc: String,
    pub rate: u32,
}

/// Fields kept per-variant (not a shared `BasicCallMeta`) so the `serde` shape
/// mirrors the stanza 1:1 for downstream JS consumers.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
// Forward-compat: WA can add call sub-types, so an external exhaustive match must keep a wildcard.
#[non_exhaustive]
pub enum CallAction {
    Offer {
        call_id: String,
        call_creator: Jid,
        #[serde(skip_serializing_if = "Option::is_none")]
        caller_pn: Option<Jid>,
        #[serde(skip_serializing_if = "Option::is_none")]
        caller_country_code: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        device_class: Option<String>,
        joinable: bool,
        is_video: bool,
        audio: Vec<CallAudioCodec>,
        /// Set on group calls. Primary group signal per `WAWebVoipGatingUtils`.
        #[serde(skip_serializing_if = "Option::is_none")]
        group_jid: Option<Jid>,
    },
    /// Group-call notification fan-out to members. No offer-receipt expected;
    /// the generic call ack is enough (router handles it via `should_ack`).
    OfferNotice {
        call_id: String,
        call_creator: Jid,
        /// `media == "video"` per `WAWebHandleVoipOfferNotice`.
        is_video: bool,
        /// `type == "group"` per `WAWebHandleVoipOfferNotice`.
        is_group: bool,
    },
    PreAccept {
        call_id: String,
        call_creator: Jid,
    },
    Accept {
        call_id: String,
        call_creator: Jid,
    },
    Reject {
        call_id: String,
        call_creator: Jid,
    },
    Terminate {
        call_id: String,
        call_creator: Jid,
        /// Why the peer ended the call. WA Web maps this to the call-log outcome:
        /// `accepted_elsewhere`/`rejected_elsewhere` mean another of the callee's devices
        /// answered/declined (NOT a missed call); `timeout`/`group_call_ended`/absent mean missed.
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration: Option<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        audio_duration: Option<u32>,
    },
    /// ICE/relay candidate exchange. `transport_message_type`: 1 relay candidate,
    /// 3 peer ICE (callee replies 9), 9 keepalive/reply.
    Transport {
        call_id: String,
        call_creator: Jid,
        #[serde(skip_serializing_if = "Option::is_none")]
        p2p_cand_round: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        transport_message_type: Option<String>,
    },
    /// Per-relay RTT probe from the peer; the client replies with a relaylatency ack.
    RelayLatency {
        call_id: String,
        call_creator: Jid,
    },
}

impl CallAction {
    pub fn call_id(&self) -> &str {
        match self {
            Self::Offer { call_id, .. }
            | Self::OfferNotice { call_id, .. }
            | Self::PreAccept { call_id, .. }
            | Self::Accept { call_id, .. }
            | Self::Reject { call_id, .. }
            | Self::Terminate { call_id, .. }
            | Self::Transport { call_id, .. }
            | Self::RelayLatency { call_id, .. } => call_id,
        }
    }

    pub fn call_creator(&self) -> &Jid {
        match self {
            Self::Offer { call_creator, .. }
            | Self::OfferNotice { call_creator, .. }
            | Self::PreAccept { call_creator, .. }
            | Self::Accept { call_creator, .. }
            | Self::Reject { call_creator, .. }
            | Self::Terminate { call_creator, .. }
            | Self::Transport { call_creator, .. }
            | Self::RelayLatency { call_creator, .. } => call_creator,
        }
    }

    /// The wire tag name of the action variant (`offer`, `accept`, ...), for logging.
    pub fn action_kind(&self) -> &'static str {
        match self {
            Self::Offer { .. } => "offer",
            Self::OfferNotice { .. } => "offer_notice",
            Self::PreAccept { .. } => "preaccept",
            Self::Accept { .. } => "accept",
            Self::Reject { .. } => "reject",
            Self::Terminate { .. } => "terminate",
            Self::Transport { .. } => "transport",
            Self::RelayLatency { .. } => "relaylatency",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct IncomingCall {
    pub from: Jid,
    /// Stanza id; distinct from `CallAction::call_id`.
    pub stanza_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub platform: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    pub offline: bool,
    pub action: CallAction,
    /// Media material from an `<offer>` (the encrypted callKey + parsed relay), captured by the
    /// parser so the `voip` media facade can drive the call. `None` for non-offer actions or an
    /// offer with no `<enc>` for us. Boxed so the large `RelayData` doesn't bloat every `Event`
    /// (the no-media common case stays one pointer). Skipped on the `serde` shape (binary-only).
    #[cfg(feature = "voip")]
    #[serde(skip)]
    pub media: Option<Box<MediaOffer>>,
}

/// A call that must NOT ring: surfaced instead of [`IncomingCall`] so a consumer cannot auto-accept
/// it. Currently this is an offer the server replayed from the offline queue on reconnect (the
/// `<call>` carried the `offline` attribute) -- the call is long dead (no relay, not connectable).
/// Mirrors WA Web's `cancel_call` + `missed_call` path for `offerReceivedWhileOffline`.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct MissedCall {
    pub from: Jid,
    /// The call id (from the `<offer>` action); distinct from the `<call>` stanza id.
    pub call_id: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    pub reason: MissedReason,
}

impl MissedCall {
    /// Construct a missed-call event. `#[non_exhaustive]` blocks the struct literal cross-crate, so
    /// this is how the high-level crate builds one.
    pub fn new(from: Jid, call_id: String, timestamp: DateTime<Utc>, reason: MissedReason) -> Self {
        Self {
            from,
            call_id,
            timestamp,
            reason,
        }
    }
}

/// Why a call surfaced as missed rather than ringing.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum MissedReason {
    /// The offer was replayed from the offline queue on reconnect (server-set `offline` attribute).
    Offline,
    /// A `<terminate>` arrived for an incoming call we never answered (the peer gave up). Mirrors WA
    /// Web's "missed" call-log outcome for an unanswered call.
    Remote,
}

/// An incoming call we were ringing for was resolved on ANOTHER of our devices (multi-device): the
/// caller dismissed this device with a `<terminate reason="accepted_elsewhere"|"rejected_elsewhere">`.
/// Distinct from [`MissedCall`] (a genuinely unanswered call) so a consumer can render "answered on
/// another device" instead of a missed call. Mirrors WA Web's AcceptedElsewhere / Rejected outcomes.
#[derive(Debug, Clone, Serialize)]
#[non_exhaustive]
pub struct CallEndedElsewhere {
    pub from: Jid,
    /// The call id (from the `<offer>` action); distinct from the `<call>` stanza id.
    pub call_id: String,
    #[serde(with = "chrono::serde::ts_seconds")]
    pub timestamp: DateTime<Utc>,
    pub outcome: ElsewhereOutcome,
}

impl CallEndedElsewhere {
    /// `#[non_exhaustive]` blocks the struct literal cross-crate, so the high-level crate builds one
    /// here.
    pub fn new(
        from: Jid,
        call_id: String,
        timestamp: DateTime<Utc>,
        outcome: ElsewhereOutcome,
    ) -> Self {
        Self {
            from,
            call_id,
            timestamp,
            outcome,
        }
    }
}

/// Which terminal outcome another of our devices reached for a call we were ringing for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum ElsewhereOutcome {
    /// Another of our devices answered the call (`reason="accepted_elsewhere"`).
    Accepted,
    /// Another of our devices declined the call (`reason="rejected_elsewhere"`).
    Rejected,
}

impl IncomingCall {
    /// Minimal constructor for in-tree tests in dependent crates; `#[non_exhaustive]` blocks the
    /// struct literal cross-crate, so this is the supported way to build one outside `wacore`. The
    /// optional/media fields default to absent; mutate the public fields after for other shapes.
    #[doc(hidden)]
    pub fn new_for_test(
        from: Jid,
        stanza_id: String,
        timestamp: DateTime<Utc>,
        action: CallAction,
    ) -> Self {
        Self {
            from,
            stanza_id,
            notify: None,
            platform: None,
            version: None,
            timestamp,
            offline: false,
            action,
            #[cfg(feature = "voip")]
            media: None,
        }
    }
}
