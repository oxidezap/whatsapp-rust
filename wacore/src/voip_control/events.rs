//! The public call event stream, owned by the control plane.
//!
//! This is the type a consumer reads from `CallHandle::events()` and a media backend raises through
//! [`VoipMediaSession::subscribe`](super::VoipMediaSession::subscribe). It used to be the engine's
//! `CallEvent`, which meant the public API depended on the media engine's enum. It lives here now:
//! neutral, engine-free, with every payload naming a neutral type. `crate::voip::CallEvent` is a
//! re-export, so the historical path still resolves, but the type belongs to the control plane.
//!
//! Signaling-born events (`VideoStateChanged`, `GroupUpdated`, `WaitingRoomUpdated`, `HandRaised`,
//! `ScreenShareChanged`, `Reaction`) are produced by the signaling handler, not the media engine;
//! they share this one stream so a consumer has a single ordered view of the call.

use bytes::Bytes;
use wacore_binary::Jid;

use crate::types::call::VideoState;
use crate::types::group_call::{GroupCallUpdate, ScreenShare, WaitingRoom};

use super::media_stats::Millis;
use super::{
    MediaAudioCodec, MediaCloseReason, MediaCodecDecisionSource, MediaEncodedFrame,
    MediaGroupControlKind, MediaRtcpFeedback, MediaRtcpReportBlock, MediaSilenceReason,
    MediaVideoUpgradeToken,
};

/// One event in a call's public, ordered stream.
///
/// `#[non_exhaustive]`: a later variant does not break a consumer that matches on the ones it knows.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallEvent {
    /// The relay accepted our allocate (an allocate/binding success arrived); media path is live.
    RelayAllocated,
    /// A standard Opus packet carried through MLOW's in-profile escape while PCM/MLOW I/O is
    /// selected. Shells with an Opus decoder can play it; codec selection still follows signaling.
    ForeignAudio(Bytes),
    /// A standard Opus fallback packet received in PCM/MLOW mode from one authenticated group
    /// participant. The participant metadata lets consumers keep one stateful decoder per sender.
    ForeignGroupAudio(MediaEncodedFrame),
    /// The peer selected signaling rates incompatible with the single profile offered locally.
    AudioFormatMismatch {
        expected_rate: u32,
        received_rates: Vec<u32>,
    },
    /// The relay rejected our allocate. Terminal; carries the STUN error code (class*100 + number).
    RelayAllocateFailed(u16),
    /// The relay never acked the allocate within the deadline (wedged relay). Terminal.
    RelayAllocateTimedOut,
    /// The media path was never built, and this is why. Terminal.
    ///
    /// Distinct from the two above, which are the relay *answering* badly. This one is everything
    /// before there is a relay to answer: no `<relay>` in the offer ack, an engine that would not
    /// build, or a platform whose transport provider refused.
    MediaSetupFailed(String),
    /// Replacing a migrated relay transport did not finish within the reconnect deadline.
    RelayReconnectTimedOut,
    /// The peer's `<video state=N>` signaling arrived (upgrade requested/accepted, stopped, ...).
    ///
    /// The compatibility spelling of [`Self::PeerVideoStateChanged`]; identity-aware consumers
    /// should use that one and ignore this. A media backend never raises it.
    VideoStateChanged {
        state: VideoState,
        orientation: Option<u8>,
        upgrade_token: Option<MediaVideoUpgradeToken>,
    },
    /// A committed peer video-state notification with its signaling identity.
    PeerVideoStateChanged {
        source: Jid,
        call_creator: Jid,
        state: VideoState,
        orientation: Option<u8>,
        /// The token to pass back to accept an upgrade. `None` when signaling already resolved
        /// simultaneous requests.
        upgrade_token: Option<MediaVideoUpgradeToken>,
    },
    /// Outbound video needs an IDR before anything can go on the wire.
    VideoKeyframeNeeded,
    /// A newer authoritative group membership/relay snapshot was committed.
    GroupUpdated(Box<GroupCallUpdate>),
    /// A newer authoritative call-link admission snapshot was committed.
    WaitingRoomUpdated(Box<WaitingRoom>),
    /// Repeated waiting-room heartbeats failed and the pending call-link admission was abandoned.
    WaitingRoomHeartbeatFailed,
    /// One signaling/app-data control was rejected while the call itself remained healthy.
    GroupControlRejected { control: MediaGroupControlKind },
    /// A server-requested shared epoch could not be distributed or committed locally.
    GroupRekeyFailed,
    /// One participant raised or lowered their hand.
    HandRaised { participant: Jid, raised: bool },
    /// One participant started or stopped screen sharing.
    ScreenShareChanged {
        participant: Jid,
        screen_share: ScreenShare,
    },
    /// One authenticated, participant-attributed RTC reaction.
    Reaction {
        participant: Jid,
        device: Jid,
        pid: Option<u32>,
        /// `None` removes the participant's previous reaction.
        emoji: Option<String>,
        removed: bool,
    },
    /// Authenticated peer RTCP.
    RtcpReceived {
        packet_types: Vec<u8>,
        sender_ssrc: u32,
        referenced_ssrcs: Vec<u32>,
        reports_audio: bool,
        reports_video: bool,
        report_blocks: Vec<MediaRtcpReportBlock>,
        feedback: Vec<MediaRtcpFeedback>,
    },
    /// Relay-send backpressure discarded complete media units before transmission.
    OutboundMediaDropped {
        video_access_units: u32,
        packets: u32,
    },
    /// Audio RTP keeps arriving and none of it is becoming sound.
    AudioSilent {
        silent_for_ms: Millis,
        /// Packets counted in the window that produced this alarm, not for the whole call.
        rtp_received: u32,
        frames_produced: u32,
        dominant_reason: MediaSilenceReason,
    },
    /// The payload grammar in use changed inside the negotiated RTP timing.
    AudioCodecSwitched {
        from: MediaAudioCodec,
        to: MediaAudioCodec,
        source: MediaCodecDecisionSource,
        packets_observed: u32,
    },
    /// The peer speaks one codec and this call's encoded source emits another, and neither can move.
    AudioCodecSourceIsFixed {
        sending: MediaAudioCodec,
        peer_expects: MediaAudioCodec,
        source: MediaCodecDecisionSource,
    },
    /// Audio RTP has stopped arriving.
    AudioReceptionStalled { silent_for_ms: Millis },
    /// The media session closed, with why.
    ///
    /// Raised by a backend when its media ends; a signaling-driven teardown produces it through the
    /// registry's close path rather than from the engine.
    Closed(MediaCloseReason),
}

impl CallEvent {
    /// Heap retained by the event's payload, for the queue's byte budget.
    #[must_use]
    pub fn heap_bytes(&self) -> usize {
        use core::mem::size_of;

        use crate::stats::HeapSize;

        match self {
            Self::VideoKeyframeNeeded => 0,
            Self::ForeignAudio(data) => data.len(),
            Self::ForeignGroupAudio(frame) => {
                frame.data.len()
                    + frame.sender.as_ref().map_or(0, HeapSize::heap_bytes)
                    + frame.device.as_ref().map_or(0, HeapSize::heap_bytes)
            }
            Self::AudioFormatMismatch { received_rates, .. } => {
                received_rates.capacity() * size_of::<u32>()
            }
            Self::GroupUpdated(update) => size_of::<GroupCallUpdate>() + update.heap_bytes(),
            Self::WaitingRoomUpdated(room) => size_of::<WaitingRoom>() + room.heap_bytes(),
            Self::HandRaised { participant, .. } | Self::ScreenShareChanged { participant, .. } => {
                participant.heap_bytes()
            }
            Self::Reaction {
                participant,
                device,
                emoji,
                ..
            } => {
                participant.heap_bytes()
                    + device.heap_bytes()
                    + emoji.as_ref().map_or(0, String::capacity)
            }
            Self::RtcpReceived {
                packet_types,
                referenced_ssrcs,
                report_blocks,
                feedback,
                ..
            } => {
                packet_types.capacity()
                    + referenced_ssrcs.capacity() * size_of::<u32>()
                    + report_blocks.capacity() * size_of::<MediaRtcpReportBlock>()
                    + report_blocks
                        .iter()
                        .map(|report| report.profile_extension.capacity())
                        .sum::<usize>()
                    + feedback.capacity() * size_of::<MediaRtcpFeedback>()
                    + feedback
                        .iter()
                        .map(|item| item.fci.capacity())
                        .sum::<usize>()
            }
            Self::MediaSetupFailed(reason) => reason.capacity(),
            Self::Closed(reason) => match reason {
                MediaCloseReason::SendFailed(reason) | MediaCloseReason::SetupFailed(reason) => {
                    reason.capacity()
                }
                MediaCloseReason::Local | MediaCloseReason::RelayDisconnected => 0,
            },
            Self::PeerVideoStateChanged {
                source,
                call_creator,
                ..
            } => source.heap_bytes() + call_creator.heap_bytes(),
            Self::RelayAllocated
            | Self::RelayAllocateFailed(_)
            | Self::RelayAllocateTimedOut
            | Self::RelayReconnectTimedOut
            | Self::VideoStateChanged { .. }
            | Self::WaitingRoomHeartbeatFailed
            | Self::GroupControlRejected { .. }
            | Self::GroupRekeyFailed
            | Self::OutboundMediaDropped { .. }
            | Self::AudioSilent { .. }
            | Self::AudioCodecSwitched { .. }
            | Self::AudioCodecSourceIsFixed { .. }
            | Self::AudioReceptionStalled { .. } => 0,
        }
    }
}
