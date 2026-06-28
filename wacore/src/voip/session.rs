//! Call state machine and the media pipeline composition (Opus payload to RTP WARP header to
//! E2E SRTP protect, and the reverse). The byte-level crypto/framing lives in the sibling
//! `wacore::voip` modules; this stitches it together. Pure logic: no socket, clock, or runtime.

use super::e2e_srtp::{
    E2eSrtpKeys, RecvRocTracker, RocTracker, append_warp_mi_tag, crypt_payload, derive_e2e_keys,
};
use super::rtp::{
    RTP_FIXED_HEADER_LEN, RtpHeader, RtpStream, encode_rtp_header, parse_rtp_header,
    rtp_header_byte_length,
};
use super::ssrc::format_e2e_srtp_participant_id;
use wacore_binary::Jid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallDirection {
    Outgoing,
    Incoming,
}

/// Lifecycle phase of a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CallPhase {
    Idle,
    Calling,
    Ringing,
    Connecting,
    Active,
    Ended,
}

/// Per-call signaling state. Transitions are validated so an out-of-order server message
/// can't silently advance a torn-down call.
#[derive(Debug, Clone)]
pub struct CallSession {
    pub call_id: String,
    pub peer_jid: Jid,
    pub call_creator: Jid,
    pub direction: CallDirection,
    pub is_video: bool,
    /// For an OUTGOING call: the callee device JIDs the offer rang, so when one accepts/rejects the
    /// caller can dismiss the rest (`accepted_elsewhere`). Empty for incoming calls and single-device
    /// callees. Lives on the session so it is dropped automatically whenever the call deregisters --
    /// no separate per-call map to clean up across the many call-end paths.
    pub ring_devices: Vec<Jid>,
    /// For an OUTGOING call: the callee device (`call.from` of the inbound `<accept>`) that actually
    /// answered, learned after the offer rang the bare LID. Call signaling other than the offer is
    /// addressed per device (WA Web `WAWebVoipSendSignalingXmpp` coerces the peer to a device JID), so
    /// a `<terminate>` must target this device, not the bare peer, or it can miss the companion that
    /// answered. `None` until the first `<accept>`; set-once (first answerer wins, like the rekey).
    pub answering_device: Option<Jid>,
    phase: CallPhase,
}

impl CallSession {
    pub fn new_outgoing(call_id: impl Into<String>, peer_jid: Jid, call_creator: Jid) -> Self {
        Self {
            call_id: call_id.into(),
            peer_jid,
            call_creator,
            direction: CallDirection::Outgoing,
            is_video: false,
            ring_devices: Vec::new(),
            answering_device: None,
            phase: CallPhase::Idle,
        }
    }

    pub fn new_incoming(call_id: impl Into<String>, peer_jid: Jid, call_creator: Jid) -> Self {
        Self {
            call_id: call_id.into(),
            peer_jid,
            call_creator,
            direction: CallDirection::Incoming,
            is_video: false,
            ring_devices: Vec::new(),
            answering_device: None,
            phase: CallPhase::Ringing,
        }
    }

    pub fn phase(&self) -> CallPhase {
        self.phase
    }

    pub fn is_active(&self) -> bool {
        self.phase == CallPhase::Active
    }

    pub fn is_ended(&self) -> bool {
        self.phase == CallPhase::Ended
    }

    /// Attempt a phase transition; returns false (no-op) if it is not legal from the current phase.
    ///
    /// The lifecycle order is `Idle → Calling → Ringing → Connecting → Active`. Forward progress is
    /// allowed and MAY skip intermediate phases: an accepted outgoing call commonly goes
    /// `Calling → Connecting` with no observed `Ringing`, and an immediate accept can reach `Active`
    /// directly. Backward moves are rejected. `Idle` leaves only to `Calling` (outgoing) or `Ended`.
    /// `Ended` is a sink reachable from any live phase (`Ended → Ended` is a no-op `false`).
    /// Self-transitions on a live phase are idempotent.
    pub fn transition_to(&mut self, next: CallPhase) -> bool {
        use CallPhase::*;
        let ok = match (self.phase, next) {
            (Ended, _) => false,
            (_, Ended) => true,
            (a, b) if a == b => true,
            (Idle, Calling) => self.direction == CallDirection::Outgoing,
            (Idle, _) => false,
            (from, to) => phase_rank(to) > phase_rank(from),
        };
        if ok {
            self.phase = next;
        }
        ok
    }
}

/// Lifecycle ordinal for the forward-progress check in [`CallSession::transition_to`] (higher =
/// later in the call). `Ended` is handled separately, so its rank is never compared.
fn phase_rank(p: CallPhase) -> u8 {
    match p {
        CallPhase::Idle => 0,
        CallPhase::Calling => 1,
        CallPhase::Ringing => 2,
        CallPhase::Connecting => 3,
        CallPhase::Active => 4,
        CallPhase::Ended => 5,
    }
}

/// Composes the outbound (protect) and inbound (unprotect) media pipeline for E2E 1:1.
/// SFrame is omitted (default-off on send; plain Opus inside WAHKDF SRTP).
pub struct MediaPipeline {
    send_keys: E2eSrtpKeys,
    recv_keys: E2eSrtpKeys,
    warp_mi_tag_len: usize,
    rtp: RtpStream,
    send_roc: RocTracker,
    recv_roc: RecvRocTracker,
}

/// Borrowed inputs for [`MediaPipeline::new`]. `self_lid`/`peer_lid` are the E2E-SRTP
/// participant JIDs (normalized inside `new`).
#[derive(Clone, Copy)]
pub struct MediaPipelineParams<'a> {
    pub call_key: &'a [u8],
    pub self_lid: &'a str,
    pub peer_lid: &'a str,
    pub ssrc: u32,
    pub samples_per_packet: u32,
    pub warp_mi_tag_len: usize,
}

impl MediaPipeline {
    /// Derive both directions from the 32-byte callKey. The HKDF `info` is the *sender's* own
    /// participant id, so send keys come from our self LID and recv keys from the peer LID
    /// (SFrame uses the opposite convention). JIDs are normalized with the E2E-SRTP
    /// participant-id rule (keep an existing `:device`, bare `@lid` becomes `:0@lid`), which
    /// must match the form the peer derives our SSRC from.
    /// Returns `None` when `call_key` is shorter than 32 bytes (a malformed peer callKey).
    pub fn new(p: &MediaPipelineParams<'_>) -> Option<Self> {
        // The WARP MI tag is sliced from the 20-byte HMAC-SHA1 digest; a relay-advertised length
        // above 20 (or zero) would panic on the first packet, so reject it at setup instead.
        if !(1..=20).contains(&p.warp_mi_tag_len) {
            return None;
        }
        Some(Self {
            send_keys: derive_e2e_keys(p.call_key, &format_e2e_srtp_participant_id(p.self_lid))?,
            recv_keys: derive_e2e_keys(p.call_key, &format_e2e_srtp_participant_id(p.peer_lid))?,
            warp_mi_tag_len: p.warp_mi_tag_len,
            rtp: RtpStream::new(p.ssrc, p.samples_per_packet, false),
            send_roc: RocTracker::default(),
            recv_roc: RecvRocTracker::default(),
        })
    }

    /// Caller-side: re-derive the recv keys for the device that actually answered. We dial the base
    /// callee LID, but a multi-device callee answers from one device (e.g. `:2`) and encrypts under
    /// its OWN participant id; keeping the base-LID recv keys decrypts every inbound frame to garbage.
    /// Send keys are untouched (they key on our self LID). The recv ROC resets: the answerer's RTP
    /// stream is fresh, so a stale `s_l` would mis-guess the index of its first packets. Returns
    /// `false` only on a malformed `call_key` (a setup invariant already checked in [`new`](Self::new)).
    pub fn rekey_recv(&mut self, call_key: &[u8], answering_peer_lid: &str) -> bool {
        let Some(keys) = derive_e2e_keys(
            call_key,
            &format_e2e_srtp_participant_id(answering_peer_lid),
        ) else {
            return false;
        };
        self.recv_keys = keys;
        self.recv_roc = RecvRocTracker::default();
        true
    }

    /// Outbound: wrap an Opus payload in an RTP WARP header, E2E-SRTP encrypt, append the WARP MI tag.
    pub fn protect_audio(&mut self, opus_payload: &[u8]) -> Vec<u8> {
        let header = self.rtp.next_packet(opus_payload, false);
        let roc = self.send_roc.advance(header.sequence_number);
        let header_bytes = encode_rtp_header(&header);
        let encrypted = crypt_payload(
            &self.send_keys,
            header.ssrc,
            header.sequence_number,
            roc,
            opus_payload,
        );
        let mut packet = header_bytes;
        packet.extend_from_slice(&encrypted);
        append_warp_mi_tag(&self.send_keys.auth_key, &packet, roc, self.warp_mi_tag_len)
    }

    /// Inbound: strip the WARP MI tag (not verified), parse the header, decrypt the payload.
    /// The ROC is derived per-packet from the recv tracker (RFC 3711 guess-index), so the keystream
    /// stays aligned with the sender's across 16-bit seq wraps even under reorder/loss.
    pub fn unprotect_audio(&mut self, packet: &[u8]) -> Option<(RtpHeader, Vec<u8>)> {
        if packet.len() < RTP_FIXED_HEADER_LEN + self.warp_mi_tag_len {
            return None;
        }
        let without_tag = &packet[..packet.len() - self.warp_mi_tag_len];
        let header = parse_rtp_header(without_tag)?;
        let header_len = rtp_header_byte_length(without_tag)?;
        if without_tag.len() <= header_len {
            return None;
        }
        let roc = self.recv_roc.guess_roc(header.sequence_number);
        let cipher = &without_tag[header_len..];
        let plain = crypt_payload(
            &self.recv_keys,
            header.ssrc,
            header.sequence_number,
            roc,
            cipher,
        );
        Some((header, plain))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::voip::warp::WARP_MI_TAG_LEN;
    use wacore_binary::Server;

    fn peer() -> Jid {
        Jid::new("222222222222222", Server::Lid)
    }
    fn creator() -> Jid {
        Jid::new("111111111111111", Server::Lid).with_device(1)
    }

    #[test]
    fn outgoing_lifecycle() {
        let mut s = CallSession::new_outgoing("CID", peer(), creator());
        assert_eq!(s.phase(), CallPhase::Idle);
        assert!(s.transition_to(CallPhase::Calling));
        assert!(s.transition_to(CallPhase::Ringing));
        assert!(s.transition_to(CallPhase::Connecting));
        assert!(s.transition_to(CallPhase::Active));
        assert!(s.is_active());
        // Illegal jump is rejected.
        assert!(!s.transition_to(CallPhase::Calling));
        assert!(s.transition_to(CallPhase::Ended));
        assert!(s.is_ended());
        // Nothing advances after Ended.
        assert!(!s.transition_to(CallPhase::Active));
    }

    #[test]
    fn incoming_starts_ringing_and_cannot_call() {
        let mut s = CallSession::new_incoming("CID", peer(), creator());
        assert_eq!(s.phase(), CallPhase::Ringing);
        // Incoming can't go to Calling.
        assert!(!s.transition_to(CallPhase::Calling));
        assert!(s.transition_to(CallPhase::Connecting));
        assert!(s.transition_to(CallPhase::Active));
    }

    #[test]
    fn forward_progress_may_skip_phases_but_not_go_backward() {
        // An accepted outgoing call commonly skips Ringing (Calling -> Connecting) and an immediate
        // accept can reach Active directly. Both are forward progress and must be allowed.
        let mut s = CallSession::new_outgoing("CID", peer(), creator());
        assert!(s.transition_to(CallPhase::Calling));
        assert!(
            s.transition_to(CallPhase::Connecting),
            "Calling->Connecting (ringing skipped) must be allowed"
        );
        assert!(s.transition_to(CallPhase::Active));

        let mut s2 = CallSession::new_outgoing("CID2", peer(), creator());
        assert!(s2.transition_to(CallPhase::Calling));
        assert!(
            s2.transition_to(CallPhase::Active),
            "Calling->Active (immediate accept) must be allowed"
        );

        // Idle still leaves only to Calling (outgoing), and backward moves stay rejected.
        let mut s3 = CallSession::new_outgoing("CID3", peer(), creator());
        assert!(
            !s3.transition_to(CallPhase::Connecting),
            "Idle cannot skip straight to Connecting"
        );
        assert!(s3.transition_to(CallPhase::Calling));
        assert!(s3.transition_to(CallPhase::Active));
        assert!(
            !s3.transition_to(CallPhase::Connecting),
            "no backward Active->Connecting"
        );
        assert!(
            !s3.transition_to(CallPhase::Ringing),
            "no backward Active->Ringing"
        );
    }

    #[test]
    fn media_pipeline_round_trips_composition() {
        // Same LID both directions so the loopback exercises header+crypt+tag stitching. This
        // cannot catch a send/recv direction inversion (the scheme is symmetric between two
        // equally-configured peers); `protect_uses_self_lid_for_send` guards that.
        let call_key: Vec<u8> = (0u8..32).collect();
        let lid = "222222222222222:0@lid";
        let params = MediaPipelineParams {
            call_key: &call_key,
            self_lid: lid,
            peer_lid: lid,
            ssrc: 0x12345678,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        };
        let mut tx = MediaPipeline::new(&params).unwrap();
        let mut rx = MediaPipeline::new(&params).unwrap();

        let opus = vec![0x48u8, 0x11, 0x22, 0x33, 0x44, 0x55];
        let packet = tx.protect_audio(&opus);
        // First packet: seq=1 gives roc=0.
        let (header, payload) = rx.unprotect_audio(&packet).unwrap();
        assert_eq!(header.sequence_number, 1);
        assert_eq!(header.ssrc, 0x12345678);
        assert_eq!(header.payload_type, 120);
        assert_eq!(payload, opus);
    }

    #[test]
    fn protect_uses_self_lid_for_send() {
        // The outbound keystream must be keyed by our *self* LID (the sender's id) so a real
        // WhatsApp peer, which derives its recv keys from our LID, can decrypt us. An inversion
        // back to the peer LID would re-key this body and break interop (was the garbled-audio /
        // reconnect bug). Round-trip tests can't see this; pinning the ciphertext can.
        let call_key: Vec<u8> = (0u8..32).collect();
        let self_lid = "111111111111111:0@lid";
        let peer_lid = "222222222222222:0@lid";
        let ssrc = 0x12345678u32;
        let mut pipe = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let opus = vec![0x10u8, 0x21, 0x32, 0x43];
        let packet = pipe.protect_audio(&opus);

        let without_tag = &packet[..packet.len() - WARP_MI_TAG_LEN];
        let header_len = rtp_header_byte_length(without_tag).unwrap();
        let body = &without_tag[header_len..];
        // First packet is seq=1, roc=0.
        let expect = crypt_payload(
            &derive_e2e_keys(&call_key, self_lid).unwrap(),
            ssrc,
            1,
            0,
            &opus,
        );
        assert_eq!(
            body,
            expect.as_slice(),
            "send must encrypt under the self LID"
        );
        // And NOT under the peer LID (the inverted form).
        let inverted = crypt_payload(
            &derive_e2e_keys(&call_key, peer_lid).unwrap(),
            ssrc,
            1,
            0,
            &opus,
        );
        assert_ne!(body, inverted.as_slice());
    }

    #[test]
    fn recv_uses_peer_lid_for_recv() {
        // The recv keystream must be keyed by the PEER's LID: a real peer encrypts under its own
        // (self) LID, which is our peer LID. A round-trip test can't catch a recv-direction key
        // inversion because the scheme is symmetric; this pins the direction.
        let call_key: Vec<u8> = (0u8..32).collect();
        let self_lid = "111111111111111:0@lid";
        let peer_lid = "222222222222222:0@lid";
        let ssrc = 0x12345678u32;

        // Our recv pipe (keys self=self_lid / peer=peer_lid).
        let mut us = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        // The peer's send pipe is OUR mirror: its self LID is our peer LID.
        let mut peer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: peer_lid,
            peer_lid: self_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();

        let opus = vec![0x48u8, 0x01, 0x02, 0x03, 0x04, 0x05];
        let from_peer = peer_tx.protect_audio(&opus);
        let (_, recovered) = us
            .unprotect_audio(&from_peer)
            .expect("peer packet must decrypt under our recv (peer-LID) keys");
        assert_eq!(recovered, opus, "recv must use the peer-LID keystream");

        // A packet a mis-keyed peer would send under OUR self LID must NOT recover: that proves the
        // recv side is not silently keyed by the self LID.
        let mut self_keyed_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let wrong = self_keyed_tx.protect_audio(&opus);
        let mut us2 = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid,
            peer_lid,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let (_, mis) = us2.unprotect_audio(&wrong).unwrap();
        assert_ne!(mis, opus, "recv must not recover a self-LID-keyed packet");
    }

    // The recv keystream is HKDF'd over the FULL peer LID INCLUDING the device suffix, so the caller
    // must key its recv path to the device that ANSWERS, not the base/dialed LID. A multi-device callee
    // whose companion (e.g. `:2`) answers sends under `derive(self=...:2)`; if the caller still keys
    // recv from the base `...:0` (the dialed LID), every inbound frame decrypts to garbage. This is the
    // rust↔rust "choppy audio" root cause (an Android phone answers from `:0`, so it never tripped).
    #[test]
    fn recv_keys_must_match_the_answering_device_lid() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let caller = "111111111111111:0@lid";
        let callee_base = "222222222222222:0@lid";
        let callee_answering = "222222222222222:2@lid"; // the companion that actually answered
        let ssrc = 0x12345678;
        let opus = vec![0x50u8, 0x11, 0x22, 0x33, 0x44, 0x55];

        // The answering companion's send pipe: self = its OWN device LID `:2`.
        let mut answerer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: callee_answering,
            peer_lid: caller,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let from_answerer = answerer_tx.protect_audio(&opus);

        // BUG: caller keys recv from the dialed BASE LID `:0` -> decrypts to garbage.
        let mut caller_base = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: caller,
            peer_lid: callee_base,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let (_, garbled) = caller_base.unprotect_audio(&from_answerer).unwrap();
        assert_ne!(
            garbled, opus,
            "base-LID recv keys must NOT recover a companion-device-keyed frame (proves the bug)"
        );

        // FIX: caller keys recv from the ANSWERING device LID `:2` -> recovers cleanly.
        let mut caller_fixed = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: caller,
            peer_lid: callee_answering,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let (_, recovered) = caller_fixed.unprotect_audio(&from_answerer).unwrap();
        assert_eq!(
            recovered, opus,
            "recv keys derived from the answering device LID must recover the frame"
        );
    }

    // The fix: `rekey_recv` switches the caller's recv keys from the base LID to the answering device,
    // so a companion's frames that decrypted to garbage start decrypting cleanly. Also asserts send keys
    // are untouched (the round-trip the OTHER way still works after a rekey).
    #[test]
    fn rekey_recv_recovers_companion_keyed_frame() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let caller = "111111111111111:0@lid";
        let callee_base = "222222222222222:0@lid";
        let callee_answering = "222222222222222:2@lid";
        let ssrc = 0x12345678;
        let opus = vec![0x50u8, 0x11, 0x22, 0x33, 0x44, 0x55];

        // The companion that answers keys its send by its OWN device LID.
        let mut answerer_tx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: callee_answering,
            peer_lid: caller,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();

        // Caller starts keyed to the dialed BASE LID (the bug state) -> a companion frame is garbage.
        let mut caller_pipe = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: caller,
            peer_lid: callee_base,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let frame1 = answerer_tx.protect_audio(&opus);
        let (_, garbled) = caller_pipe.unprotect_audio(&frame1).unwrap();
        assert_ne!(
            garbled, opus,
            "pre-rekey: a companion-keyed frame is garbage"
        );

        // Rekey to the answering device; a subsequent frame now recovers cleanly.
        assert!(caller_pipe.rekey_recv(&call_key, callee_answering));
        let frame2 = answerer_tx.protect_audio(&opus);
        let (_, recovered) = caller_pipe.unprotect_audio(&frame2).unwrap();
        assert_eq!(
            recovered, opus,
            "post-rekey: the companion's frames decrypt"
        );

        // Send keys were not touched: our outbound still round-trips to a peer keyed on us.
        let mut peer_rx = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: callee_answering,
            peer_lid: caller,
            ssrc,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        })
        .unwrap();
        let ours = caller_pipe.protect_audio(&opus);
        let (_, got) = peer_rx.unprotect_audio(&ours).unwrap();
        assert_eq!(got, opus, "rekey_recv must not disturb send keys");
    }

    // A relay-advertised non-4 WARP MI tag length must round-trip: the tag the sender appends and the
    // bytes the receiver strips must agree, or all inbound media silently fails to decode. Threads a
    // configurable length through both pipelines and proves a payload survives at 6 and at the default 4.
    #[test]
    fn rejects_out_of_range_warp_mi_tag_len() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let lid = "222222222222222:0@lid";
        let params = |tag_len| MediaPipelineParams {
            call_key: &call_key,
            self_lid: lid,
            peer_lid: lid,
            ssrc: 0x12345678,
            samples_per_packet: 960,
            warp_mi_tag_len: tag_len,
        };
        // Above the 20-byte HMAC digest (or zero) would panic when slicing the tag, so reject it.
        assert!(MediaPipeline::new(&params(21)).is_none());
        assert!(MediaPipeline::new(&params(0)).is_none());
        assert!(MediaPipeline::new(&params(WARP_MI_TAG_LEN)).is_some());
        assert!(MediaPipeline::new(&params(20)).is_some());
    }

    #[test]
    fn non_default_warp_mi_tag_len_round_trips() {
        let call_key: Vec<u8> = (0u8..32).collect();
        let lid = "222222222222222:0@lid";
        let opus = vec![0x48u8, 0x11, 0x22, 0x33, 0x44, 0x55];

        for tag_len in [WARP_MI_TAG_LEN, 6] {
            let params = MediaPipelineParams {
                call_key: &call_key,
                self_lid: lid,
                peer_lid: lid,
                ssrc: 0x12345678,
                samples_per_packet: 960,
                warp_mi_tag_len: tag_len,
            };
            let mut tx = MediaPipeline::new(&params).unwrap();
            let mut rx = MediaPipeline::new(&params).unwrap();
            let packet = tx.protect_audio(&opus);
            let (_, payload) = rx
                .unprotect_audio(&packet)
                .unwrap_or_else(|| panic!("tag_len {tag_len} must round-trip"));
            assert_eq!(payload, opus, "tag_len {tag_len} payload must survive");
        }

        // A mismatched recv tag length strips the wrong byte count and corrupts the payload, which is
        // exactly the failure this config plumbing prevents.
        let base = MediaPipelineParams {
            call_key: &call_key,
            self_lid: lid,
            peer_lid: lid,
            ssrc: 0x12345678,
            samples_per_packet: 960,
            warp_mi_tag_len: 6,
        };
        let mut tx = MediaPipeline::new(&base).unwrap();
        let mut rx = MediaPipeline::new(&MediaPipelineParams {
            warp_mi_tag_len: 4,
            ..base
        })
        .unwrap();
        let packet = tx.protect_audio(&opus);
        let mismatched = rx.unprotect_audio(&packet).map(|(_, p)| p);
        assert_ne!(
            mismatched.as_deref(),
            Some(opus.as_slice()),
            "a recv/send tag-length mismatch must NOT recover the payload"
        );
    }

    // The esp32 control/crypto plane. An embedded consumer with no UDP, no codec, and no audio
    // drives exactly this much of the call stack over its main WebSocket connection: the signaling
    // state machine plus E2E-SRTP key derivation. It never constructs the media engine and never
    // runs MLow. This pins that surface as pure sync logic (no runtime, no FFI), which is all the
    // esp32-S3 can do today; running the codec there is out of scope.
    #[test]
    fn esp32_control_plane_signaling_and_crypto_without_media() {
        // Signaling: drive an incoming call through its lifecycle.
        let peer = Jid::new("222222222222222", Server::Lid);
        let mut call = CallSession::new_incoming("CID", peer.clone(), peer);
        assert_eq!(call.phase(), CallPhase::Ringing);
        assert!(call.transition_to(CallPhase::Connecting));
        assert!(call.transition_to(CallPhase::Active));
        assert!(call.transition_to(CallPhase::Ended));

        // Crypto: derive the E2E-SRTP keys from the callKey. This is HKDF only (no codec, no FFI),
        // so it is viable on the esp32; building the pipeline does not encode or decode any audio.
        let call_key: Vec<u8> = (0u8..32).collect();
        let pipeline = MediaPipeline::new(&MediaPipelineParams {
            call_key: &call_key,
            self_lid: "111@lid",
            peer_lid: "222@lid",
            ssrc: 0x1234,
            samples_per_packet: 960,
            warp_mi_tag_len: WARP_MI_TAG_LEN,
        });
        assert!(
            pipeline.is_some(),
            "key derivation must succeed on the control plane"
        );
    }
}
