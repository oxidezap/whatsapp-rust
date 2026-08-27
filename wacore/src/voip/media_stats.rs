//! Per-call media counters and the audio-health watchdog.
//!
//! Issue #1105 stayed open for months because every discard on the receive path was silent: a
//! payload type we did not expect, an SRTP tag that did not verify, a codec frame the decoder
//! refused, and a jitter buffer trimming its own head all returned without leaving a trace. A call
//! that carried no audio and a call where nobody spoke produced identical observations.
//!
//! The rule this module exists to enforce: **every `return` that discards a peer's audio increments
//! exactly one named counter.** The setup guards that precede it (no media plane, no PCM state, a
//! call that is not the one this packet belongs to) are deliberately not counted: they fire before
//! a packet is a packet, and counting them would bury the discards that mean something. The counters
//! are per call and die with it, which is why they are not in
//! [`crate::stats::SessionStats`] — that surface describes the WhatsApp session socket, and
//! `agent_docs/observability.md` keeps the two apart deliberately.
//!
//! Everything here is `Copy` and allocation-free. The engine is sans-io and drives one call on one
//! task, so plain `u32` beats atomics; increments saturate rather than wrap so a pathological peer
//! cannot make a counter run backwards.

use super::engine::{Millis, NEVER};

/// Counters for one call's media plane, snapshot by value.
///
/// Read through `CallEngine::media_stats()` or `CallHandle::media_stats()`. Fields are additive for
/// the life of the call; a consumer that wants a rate samples twice and subtracts.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CallMediaStats {
    /// Audio RTP packets that authenticated and reached codec dispatch.
    pub rtp_received: u32,
    /// Packets whose payload type is outside the negotiated profile. A peer that switched RTP
    /// profiles under us shows up here and nowhere else.
    pub rtp_payload_type_unexpected: u32,
    /// Packets whose SRTP/WARP tag did not verify. Sustained non-zero with `rtp_received` at zero
    /// is the "deaf with no symptom" failure: wrong recv keys, wrong peer LID, or a stale ROC.
    pub srtp_unprotect_failed: u32,
    /// Packets where an SFrame session was installed but GCM did not authenticate, so the payload
    /// was passed through as plaintext.
    pub sframe_decrypt_failed: u32,
    /// Audio frames that produced PCM (`AudioIo::Pcm`).
    pub audio_frames_decoded: u32,
    /// Encoded payloads handed to the application sink (`AudioIo::Encoded`).
    pub audio_frames_delivered: u32,
    /// Frames the MLow decoder concealed instead of decoding.
    pub audio_frames_concealed: u32,
    /// MLow frames refused by the operating-point guard.
    pub mlow_off_point_dropped: u32,
    /// MLow frames that carried no decodable body (SID) or were coded inactive.
    pub mlow_inactive_or_sid: u32,
    /// Frames decoded by an injected [`super::audio::ForeignAudioCodec`].
    pub foreign_frames_decoded: u32,
    /// Frames the peer sent in a codec this build has no decoder for. Not recoverable inside the
    /// call, and the one silence reason a consumer can act on before the next call.
    pub audio_frames_without_decoder: u32,
    /// Samples discarded from the head of the playout buffer to hold the latency ceiling.
    pub playout_trimmed_samples: u32,
    /// Inbound media the relay read pump discarded under backpressure, before the engine.
    pub inbound_pipe_dropped: u32,
    /// Relay datagrams that failed to classify as STUN, RTP or RTCP.
    pub relay_packet_unclassified: u32,
    /// Group forwarding envelopes that failed to unwrap.
    pub forwarding_envelope_rejected: u32,
    /// Times the payload grammar in use changed within the negotiated timing.
    pub codec_switches: u16,
}

impl CallMediaStats {
    /// Audio units that actually reached a consumer, whichever I/O mode is in use.
    ///
    /// The modes cannot be summed blindly elsewhere: a PCM/MLOW call increments
    /// `audio_frames_decoded`, a PCM call rescued onto an injected codec increments
    /// `foreign_frames_decoded`, and an encoded call increments `audio_frames_delivered`. A consumer
    /// asking "is this call carrying audio" needs one number that means that in all three.
    #[must_use]
    pub const fn audio_produced(&self) -> u32 {
        self.audio_frames_decoded
            .saturating_add(self.audio_frames_delivered)
            .saturating_add(self.foreign_frames_decoded)
    }
}

/// A snapshot of [`CallMediaStats`] the drive loop publishes for a consumer to read.
///
/// The engine keeps plain `u32` because it is sans-io and single-threaded; this is the one place the
/// numbers cross a task boundary. The drive loop republishes whenever a counter moved, which on a
/// live call is most iterations, since `rtp_received` moves on every authenticated packet. That is
/// an uncontended lock at roughly the packet rate; measured, it does not appear in a profile.
#[derive(Debug, Default)]
pub struct MediaStatsCell(std::sync::Mutex<CallMediaStats>);

impl MediaStatsCell {
    /// Overwrite the published snapshot. Called by the drive loop; a poisoned lock is ignored,
    /// since a stale diagnostic must never take a live call down.
    pub fn publish(&self, stats: CallMediaStats) {
        if let Ok(mut slot) = self.0.lock() {
            *slot = stats;
        }
    }

    /// The most recent snapshot, or zeroes if the lock was poisoned or nothing has published yet.
    #[must_use]
    pub fn snapshot(&self) -> CallMediaStats {
        self.0.lock().map(|slot| *slot).unwrap_or_default()
    }
}

/// Why a call is carrying no audio, as far as the engine can tell.
///
/// Ordered by how specific the explanation is: the watchdog reports the most specific reason whose
/// counter dominates, because "we have no decoder for what the peer negotiated" and "the tags do
/// not verify" call for completely different fixes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AudioSilenceReason {
    /// The peer negotiated a codec this build cannot decode (no `voip-libopus`, or a wasm32/ESP32
    /// build with no foreign codec injected). Not recoverable inside the call.
    NoDecoderForNegotiatedCodec,
    /// Packets arrive and their tags do not verify. Almost always a keying problem.
    AuthenticationFailing,
    /// Packets arrive on a payload type outside the negotiated profile.
    UnexpectedPayloadType,
    /// The codec refused the frames: off operating point, or concealed as malformed.
    CodecRejectingFrames,
    /// The codec kept changing its mind. The probe latched to stop thrashing.
    CodecFlapping,
    /// Packets authenticated and decoded to nothing audible. Rare; keeps the enum total.
    Unknown,
}

/// What the watchdog decided on one evaluation.
///
/// Kept separate from `CallEvent` so this module does not depend on the engine's event enum; the
/// engine maps it. That also keeps the watchdog unit-testable without building a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AudioHealthAlarm {
    /// Audio RTP is arriving and none of it is becoming sound.
    Silent {
        silent_for_ms: Millis,
        rtp_received: u32,
        frames_produced: u32,
        dominant_reason: AudioSilenceReason,
    },
    /// No audio RTP has arrived at all since media came up. A transport problem, not a codec one,
    /// and the two are split precisely because conflating them is how #1105 stayed open.
    Stalled { silent_for_ms: Millis },
}

/// Evaluation cadence. Fine enough to catch the 2s window promptly, coarse enough that a call with
/// healthy audio pays one comparison per second.
const HEALTH_TICK_MS: Millis = 500;
/// Sliding window over which "packets in, no audio out" is judged.
const SILENT_WINDOW_MS: Millis = 2_000;
/// Packets that must land inside the window before silence is diagnosable. ~1.2s of media at the
/// 16.7 packets/s a 60ms stream produces; below this it is jitter, not a diagnosis.
const SILENT_WINDOW_MIN_PACKETS: u32 = 20;
/// No audio RTP at all for this long after media came up is a stalled reception.
const STALL_AFTER_MS: Millis = 3_000;
/// Re-alarm cadence while the condition persists, so a truncated log still catches it.
const REALARM_MS: Millis = 10_000;

/// Watches one call's audio for "connected but carrying nothing".
#[derive(Debug)]
pub(crate) struct AudioHealthWatch {
    /// When media came up. `NEVER` until then, which disables every rule below.
    started_at: Millis,
    /// Next evaluation deadline, folded into the engine's `poll_timeout`.
    deadline: Millis,
    window_start: Millis,
    window_rtp: u32,
    window_produced: u32,
    /// Arrivals since media came up, used only to tell "nothing arrived" from "nothing worked".
    total_arrivals: u32,
    /// Start of the current uninterrupted silence, for a monotonic `silent_for_ms`.
    silent_since: Option<Millis>,
    last_alarm_at: Option<Millis>,
    stall_reported: bool,
}

impl Default for AudioHealthWatch {
    fn default() -> Self {
        Self {
            started_at: NEVER,
            deadline: NEVER,
            window_start: 0,
            window_rtp: 0,
            window_produced: 0,
            total_arrivals: 0,
            silent_since: None,
            last_alarm_at: None,
            stall_reported: false,
        }
    }
}

impl AudioHealthWatch {
    /// Arm the watchdog. Called when the relay accepts the allocate, which is the first moment
    /// inbound media is even possible.
    pub(crate) fn media_started(&mut self, now: Millis) {
        if self.started_at != NEVER {
            return;
        }
        self.started_at = now;
        self.window_start = now;
        self.deadline = now.saturating_add(HEALTH_TICK_MS);
    }

    /// The next evaluation deadline, or [`NEVER`] while disarmed.
    pub(crate) const fn deadline(&self) -> Millis {
        self.deadline
    }

    /// One audio RTP packet arrived, whether or not it went on to authenticate or decode.
    pub(crate) fn on_rtp(&mut self) {
        self.window_rtp = self.window_rtp.saturating_add(1);
        self.total_arrivals = self.total_arrivals.saturating_add(1);
    }

    pub(crate) fn on_audio_produced(&mut self) {
        self.window_produced = self.window_produced.saturating_add(1);
    }

    /// Evaluate the two rules. Returns at most one alarm per call of this function.
    ///
    /// `stats` is read, never written: the watchdog decides *that* a call is silent, and the
    /// counters explain *why*.
    pub(crate) fn poll(&mut self, now: Millis, stats: &CallMediaStats) -> Option<AudioHealthAlarm> {
        if self.started_at == NEVER || now < self.deadline {
            return None;
        }
        self.deadline = now.saturating_add(HEALTH_TICK_MS);

        // Nothing at all has ARRIVED: a transport problem, distinct from packets arriving that we
        // cannot turn into sound. `window_rtp` counts arrivals rather than authenticated packets,
        // so a call whose every packet fails its tag falls through to the silence rule below with a
        // full window, which is where it belongs -- `dominant_reason` then names the failing tags.
        if self.total_arrivals == 0 {
            let elapsed = now.saturating_sub(self.started_at);
            if !self.stall_reported && elapsed >= STALL_AFTER_MS {
                self.stall_reported = true;
                return Some(AudioHealthAlarm::Stalled {
                    silent_for_ms: elapsed,
                });
            }
            return None;
        }

        if now.saturating_sub(self.window_start) < SILENT_WINDOW_MS {
            return None;
        }
        let (rtp, produced) = (self.window_rtp, self.window_produced);
        // Captured BEFORE the window rolls: the silence being reported started when this window
        // did, not now. Taking it after would make every first alarm claim zero milliseconds of
        // silence despite the two full seconds that authorised it.
        let window_began = self.window_start;
        self.window_start = now;
        self.window_rtp = 0;
        self.window_produced = 0;

        if produced > 0 || rtp < SILENT_WINDOW_MIN_PACKETS {
            // Audio is flowing, or too little arrived to judge. Either way the silence, if any,
            // is not established, so the run resets and `silent_for_ms` restarts on the next one.
            self.silent_since = None;
            self.last_alarm_at = None;
            return None;
        }

        let since = *self.silent_since.get_or_insert(window_began);
        let silent_for_ms = now.saturating_sub(since);
        let due = self
            .last_alarm_at
            .is_none_or(|at| now.saturating_sub(at) >= REALARM_MS);
        if !due {
            return None;
        }
        self.last_alarm_at = Some(now);
        Some(AudioHealthAlarm::Silent {
            silent_for_ms,
            rtp_received: rtp,
            frames_produced: produced,
            dominant_reason: dominant_reason(stats),
        })
    }
}

/// Pick the most specific explanation the counters support.
///
/// Order matters and is not by magnitude: a build with no decoder explains everything downstream of
/// it, and a failing tag explains a frame count of zero far better than "the codec refused it".
fn dominant_reason(stats: &CallMediaStats) -> AudioSilenceReason {
    // First because it explains everything downstream of it and is the only one a consumer can fix,
    // by building with a decoder for the codec the peer negotiated.
    if stats.audio_frames_without_decoder > 0 {
        return AudioSilenceReason::NoDecoderForNegotiatedCodec;
    }
    if stats.rtp_received == 0 && stats.srtp_unprotect_failed > 0 {
        return AudioSilenceReason::AuthenticationFailing;
    }
    if stats.rtp_received == 0 && stats.rtp_payload_type_unexpected > 0 {
        return AudioSilenceReason::UnexpectedPayloadType;
    }
    if stats.codec_switches >= CODEC_FLAP_LIMIT {
        return AudioSilenceReason::CodecFlapping;
    }
    if stats.mlow_off_point_dropped > 0 || stats.audio_frames_concealed > 0 {
        return AudioSilenceReason::CodecRejectingFrames;
    }
    AudioSilenceReason::Unknown
}

/// Switches past this in one call mean the evidence is contradicting itself; the probe latches and
/// the watchdog says so rather than letting the codec thrash for the whole call.
pub(crate) const CODEC_FLAP_LIMIT: u16 = 4;

#[cfg(test)]
mod tests {
    use super::*;

    fn armed(now: Millis) -> AudioHealthWatch {
        let mut watch = AudioHealthWatch::default();
        watch.media_started(now);
        watch
    }

    #[test]
    fn a_disarmed_watch_never_alarms() {
        let mut watch = AudioHealthWatch::default();
        assert_eq!(watch.deadline(), NEVER);
        assert_eq!(watch.poll(1_000_000, &CallMediaStats::default()), None);
    }

    #[test]
    fn no_rtp_at_all_stalls_once() {
        let mut watch = armed(0);
        let stats = CallMediaStats::default();
        assert_eq!(watch.poll(1_000, &stats), None, "under the stall deadline");
        let alarm = watch.poll(3_500, &stats).expect("stalled");
        assert!(matches!(
            alarm,
            AudioHealthAlarm::Stalled {
                silent_for_ms: 3_500
            }
        ));
        // A stalled call does not become more stalled; the consumer has the timestamp already.
        assert_eq!(watch.poll(9_000, &stats), None, "reported exactly once");
    }

    #[test]
    fn packets_arriving_with_no_audio_out_alarms_and_repeats_on_cadence() {
        let mut watch = armed(0);
        let mut stats = CallMediaStats::default();
        let feed = |watch: &mut AudioHealthWatch, stats: &mut CallMediaStats, n: u32| {
            for _ in 0..n {
                watch.on_rtp();
                stats.rtp_received = stats.rtp_received.saturating_add(1);
            }
        };
        feed(&mut watch, &mut stats, 40);
        stats.mlow_off_point_dropped = 40;
        let alarm = watch.poll(2_100, &stats).expect("silent");
        let AudioHealthAlarm::Silent {
            rtp_received,
            frames_produced,
            dominant_reason,
            ..
        } = alarm
        else {
            panic!("expected Silent, got {alarm:?}");
        };
        assert_eq!((rtp_received, frames_produced), (40, 0));
        assert_eq!(dominant_reason, AudioSilenceReason::CodecRejectingFrames);

        // Same condition inside the re-alarm window stays quiet.
        feed(&mut watch, &mut stats, 40);
        assert_eq!(watch.poll(4_500, &stats), None, "inside the realarm window");
        feed(&mut watch, &mut stats, 40);
        let repeat = watch.poll(13_000, &stats).expect("re-alarmed");
        let AudioHealthAlarm::Silent { silent_for_ms, .. } = repeat else {
            panic!("expected Silent, got {repeat:?}");
        };
        assert!(
            silent_for_ms >= 10_000,
            "silent_for_ms is monotonic across repeats, got {silent_for_ms}"
        );
    }

    #[test]
    fn one_decoded_frame_clears_the_run() {
        let mut watch = armed(0);
        let mut stats = CallMediaStats::default();
        for _ in 0..40 {
            watch.on_rtp();
            stats.rtp_received += 1;
        }
        watch.on_audio_produced();
        stats.audio_frames_decoded = 1;
        assert_eq!(watch.poll(2_100, &stats), None, "audio came out");
    }

    #[test]
    fn too_few_packets_in_the_window_is_jitter_not_a_diagnosis() {
        let mut watch = armed(0);
        let mut stats = CallMediaStats::default();
        for _ in 0..(SILENT_WINDOW_MIN_PACKETS - 1) {
            watch.on_rtp();
            stats.rtp_received += 1;
        }
        assert_eq!(watch.poll(2_100, &stats), None);
    }

    // The only reason a consumer can act on, so it outranks every downstream symptom.
    #[test]
    fn a_missing_decoder_outranks_everything_it_causes() {
        let stats = CallMediaStats {
            rtp_received: 100,
            audio_frames_without_decoder: 100,
            audio_frames_concealed: 100,
            mlow_off_point_dropped: 100,
            ..CallMediaStats::default()
        };
        assert_eq!(
            dominant_reason(&stats),
            AudioSilenceReason::NoDecoderForNegotiatedCodec
        );
    }

    #[test]
    fn failing_tags_outrank_a_codec_explanation() {
        let stats = CallMediaStats {
            rtp_received: 0,
            srtp_unprotect_failed: 120,
            mlow_off_point_dropped: 5,
            ..CallMediaStats::default()
        };
        assert_eq!(
            dominant_reason(&stats),
            AudioSilenceReason::AuthenticationFailing
        );
    }

    // Packets arriving on a payload type the profile does not accept never authenticate, so
    // `rtp_received` stays zero and this is the only counter that explains the silence.
    #[test]
    fn an_unexpected_payload_type_is_named_when_nothing_authenticates() {
        let stats = CallMediaStats {
            rtp_received: 0,
            rtp_payload_type_unexpected: 200,
            ..CallMediaStats::default()
        };
        assert_eq!(
            dominant_reason(&stats),
            AudioSilenceReason::UnexpectedPayloadType
        );
    }

    #[test]
    fn flapping_outranks_frame_rejection() {
        let stats = CallMediaStats {
            rtp_received: 50,
            codec_switches: CODEC_FLAP_LIMIT,
            audio_frames_concealed: 50,
            ..CallMediaStats::default()
        };
        assert_eq!(dominant_reason(&stats), AudioSilenceReason::CodecFlapping);
    }

    #[test]
    fn audio_produced_covers_every_way_audio_reaches_a_consumer() {
        let pcm = CallMediaStats {
            audio_frames_decoded: 7,
            ..CallMediaStats::default()
        };
        let encoded = CallMediaStats {
            audio_frames_delivered: 9,
            ..CallMediaStats::default()
        };
        let rescued = CallMediaStats {
            foreign_frames_decoded: 4,
            ..CallMediaStats::default()
        };
        assert_eq!(pcm.audio_produced(), 7);
        assert_eq!(encoded.audio_produced(), 9);
        assert_eq!(
            rescued.audio_produced(),
            4,
            "a rescued call is carrying audio"
        );
    }

    // A stream long enough to saturate a counter must still be diagnosable: the alarm compares
    // against a minimum, so a pinned counter has to stay pinned rather than wrap to zero and make a
    // silent call look like a quiet one.
    #[test]
    fn a_saturated_arrival_count_still_reports_the_call_as_silent() {
        let mut watch = armed(0);
        watch.window_rtp = u32::MAX;
        watch.total_arrivals = u32::MAX;
        watch.on_rtp();
        assert_eq!(watch.window_rtp, u32::MAX, "pinned, not wrapped");
        let stats = CallMediaStats {
            rtp_received: u32::MAX,
            mlow_off_point_dropped: 1,
            ..CallMediaStats::default()
        };
        assert!(
            matches!(
                watch.poll(2_100, &stats),
                Some(AudioHealthAlarm::Silent { .. })
            ),
            "a saturated counter must not turn a silent call quiet"
        );
    }
}
