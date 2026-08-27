//! Decides, from the bytes, whether a peer is speaking a different audio grammar than it negotiated.
//!
//! The official WhatsApp client does not do this: it registers one codec name against the payload
//! type at stream setup and never looks at a packet again. Diverging from that is deliberate, and it
//! is worth stating why, because "sniff the payload" is exactly the reflex that produced the bug
//! this module exists to fix.
//!
//! The client can trust its negotiation because it knows both halves of it: its own rollout flags
//! and the capability it received. We know one half late and sometimes never. A `<capability>` can
//! be absent (our own video `<accept>` omits one), and on the caller side it arrives only with the
//! `<preaccept>`/`<accept>`, which can lose the race with the first media packet. Negotiation stays
//! normative and drives what we *send*; this only rescues what we *receive*, and when it disagrees
//! with the negotiation it says so loudly, because that disagreement means our model of the peer is
//! wrong.
//!
//! What makes this a decision rather than a guess is that it never reads the payload alone. It asks
//! whether two independent statements by the same peer agree: the duration its Opus header declares,
//! and the duration its RTP timestamps actually advance by. A peer that is not sending Opus has no
//! reason to make those two numbers agree, and for the packets the MLow decoder accepts today it is
//! arithmetically impossible for them to. See `opus_packet::tests`.

use super::audio::AudioCodec;
use super::opus_packet::opus_packet_shape;

/// Consecutive agreeing packets required before switching.
///
/// One is too few: a single MLow body could coincidentally parse as a structurally valid Opus
/// header of the right duration. Three consecutive is ~180 ms of audio, short enough that the
/// rescued call loses nothing a listener notices, and long enough that coincidence is not a
/// plausible explanation.
const AGREEING_PACKETS_TO_SWITCH: u8 = 3;

/// Watches inbound payloads for evidence the peer negotiated one grammar and is sending another.
#[derive(Debug, Default)]
pub(crate) struct InboundCodecProbe {
    /// Consecutive packets whose declared Opus duration matched the observed timestamp step.
    agreeing: u8,
    /// Promote-once. The peer does not change codec mid-call on its own -- the gate is closed at
    /// negotiation -- so a second opinion could only come from an adversarial or broken peer, and
    /// letting it thrash the decoder is worse than staying wrong in one direction.
    decided: bool,
}

impl InboundCodecProbe {
    /// Feed one authenticated inbound payload. Returns the codec to switch to, once.
    ///
    /// `frame_span` is the RTP timestamp difference between the last two packets of this stream, in
    /// the negotiated clock. Without it there is no second statement to cross-check against, so the
    /// probe abstains rather than falling back to reading the payload alone.
    pub(crate) fn observe(
        &mut self,
        payload: &[u8],
        active: AudioCodec,
        frame_span: Option<u32>,
        clock_rate: u32,
    ) -> Option<AudioCodec> {
        if self.decided || active != AudioCodec::Mlow {
            return None;
        }
        let Some(span) = frame_span else {
            return None;
        };
        let agrees = opus_packet_shape(payload)
            .and_then(|shape| shape.total_samples(clock_rate))
            .is_some_and(|declared| declared == span);
        if !agrees {
            self.agreeing = 0;
            return None;
        }
        self.agreeing = self.agreeing.saturating_add(1);
        if self.agreeing < AGREEING_PACKETS_TO_SWITCH {
            return None;
        }
        self.decided = true;
        Some(AudioCodec::Opus)
    }

    /// Forget the streak without clearing the decision.
    ///
    /// Called when the stream's SSRC changes: a renumbered stream is a new set of timestamps, so
    /// the differences either side of the change are not comparable. The decision survives, because
    /// a peer that renumbers its SSRC has not thereby changed codec, and reopening the question
    /// would let a peer flap the decoder by renumbering.
    pub(crate) fn stream_restarted(&mut self) {
        self.agreeing = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A real WhatsApp Desktop payload shape: Opus SILK wideband, 60 ms, code 0.
    fn silk_wb_60ms(len: usize) -> Vec<u8> {
        core::iter::once(0x58u8)
            .chain((0..len).map(|i| (i % 251) as u8))
            .collect()
    }

    #[test]
    fn three_agreeing_packets_promote_to_opus() {
        let mut probe = InboundCodecProbe::default();
        let packet = silk_wb_60ms(80);
        assert_eq!(
            probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000),
            None
        );
        assert_eq!(
            probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000),
            None
        );
        assert_eq!(
            probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000),
            Some(AudioCodec::Opus)
        );
    }

    // The decision is taken once. A peer that alternates grammars is either adversarial or broken,
    // and thrashing the decoder for the rest of the call helps neither case.
    #[test]
    fn the_decision_is_taken_once() {
        let mut probe = InboundCodecProbe::default();
        let packet = silk_wb_60ms(80);
        for _ in 0..AGREEING_PACKETS_TO_SWITCH {
            probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000);
        }
        assert_eq!(
            probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000),
            None,
            "already decided"
        );
    }

    // The case that separates a decision from a guess. These bytes parse as a perfectly valid
    // 60 ms Opus packet, but the peer's own timestamps say it is sending 120 ms, so the two
    // statements disagree and nothing is promoted.
    #[test]
    fn a_valid_opus_header_that_contradicts_the_timestamps_promotes_nothing() {
        let mut probe = InboundCodecProbe::default();
        let packet = silk_wb_60ms(80);
        for _ in 0..10 {
            assert_eq!(
                probe.observe(&packet, AudioCodec::Mlow, Some(1_920), 16_000),
                None
            );
        }
    }

    #[test]
    fn a_single_disagreement_resets_the_streak() {
        let mut probe = InboundCodecProbe::default();
        let good = silk_wb_60ms(80);
        // Two agreeing, then a payload with no valid Opus shape at all, then two more agreeing:
        // still short of three consecutive.
        probe.observe(&good, AudioCodec::Mlow, Some(960), 16_000);
        probe.observe(&good, AudioCodec::Mlow, Some(960), 16_000);
        assert_eq!(
            probe.observe(&[], AudioCodec::Mlow, Some(960), 16_000),
            None
        );
        assert_eq!(
            probe.observe(&good, AudioCodec::Mlow, Some(960), 16_000),
            None
        );
        assert_eq!(
            probe.observe(&good, AudioCodec::Mlow, Some(960), 16_000),
            None
        );
        assert_eq!(
            probe.observe(&good, AudioCodec::Mlow, Some(960), 16_000),
            Some(AudioCodec::Opus)
        );
    }

    // Without a second statement to check against, reading the payload alone is the guesswork this
    // module exists to avoid.
    #[test]
    fn without_a_timestamp_step_the_probe_abstains() {
        let mut probe = InboundCodecProbe::default();
        let packet = silk_wb_60ms(80);
        for _ in 0..10 {
            assert_eq!(probe.observe(&packet, AudioCodec::Mlow, None, 16_000), None);
        }
    }

    // The probe rescues a receive path that negotiated MLow. A call already on Opus has nothing to
    // promote to, and must not be dragged back.
    #[test]
    fn a_call_already_on_opus_is_left_alone() {
        let mut probe = InboundCodecProbe::default();
        let packet = silk_wb_60ms(80);
        for _ in 0..10 {
            assert_eq!(
                probe.observe(&packet, AudioCodec::Opus, Some(960), 16_000),
                None
            );
        }
    }

    // A renumbered stream invalidates the streak but not the decision: a peer must not be able to
    // reopen the codec question by changing its SSRC.
    #[test]
    fn a_stream_restart_clears_the_streak_but_not_the_decision() {
        let mut probe = InboundCodecProbe::default();
        let packet = silk_wb_60ms(80);
        probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000);
        probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000);
        probe.stream_restarted();
        assert_eq!(
            probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000),
            None,
            "the streak restarted"
        );
        probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000);
        assert_eq!(
            probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000),
            Some(AudioCodec::Opus)
        );
        probe.stream_restarted();
        assert_eq!(
            probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000),
            None,
            "a restart must not reopen a decision already taken"
        );
    }

    // The arithmetic proof, exercised through the probe rather than the parser: no payload the MLow
    // decoder accepts today can promote a 60 ms stream, for any body length.
    #[test]
    fn no_currently_decodable_mlow_packet_can_promote_a_sixty_millisecond_stream() {
        for toc in [0x10u8, 0x11, 0x12, 0x13, 0x50, 0x51, 0x52, 0x53] {
            let mut probe = InboundCodecProbe::default();
            for len in 1..200usize {
                let packet: Vec<u8> = core::iter::once(toc)
                    .chain((0..len).map(|i| (i % 251) as u8))
                    .collect();
                assert_eq!(
                    probe.observe(&packet, AudioCodec::Mlow, Some(960), 16_000),
                    None,
                    "TOC {toc:#04x} with a {len}-byte body must never promote"
                );
            }
        }
    }
}
