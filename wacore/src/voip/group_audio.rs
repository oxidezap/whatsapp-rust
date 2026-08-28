//! Bounded, participant-aware PCM mixer for group-call playout.

use std::collections::{HashMap, HashSet, VecDeque};

/// 10 ms at the native 16 kHz mono playout rate.
pub const GROUP_MIX_CHUNK_SAMPLES: usize = 160;
/// Two 60 ms codec frames before a participant joins playout.
///
/// A FLOOR, not the target: both this and the capacity below are written in units of the 60 ms
/// frame, and a participant sending 120 ms packets would reach a fixed 1920 with its very first one
/// -- playable with no cushion at all, and drained again before the next packet arrives, so ordinary
/// relay jitter becomes recurring gaps. The queue scales both to the cadence it actually sees,
/// keeping the intent (two packets before playing, four queued at most) whatever the packet size,
/// and never dropping below what a 60 ms stream gets today.
pub const GROUP_MIX_PREFILL_SAMPLES: usize = 1_920;
/// Bound partial-buffer priming to roughly 200 ms (20 mixer chunks at 10 ms each). A participant
/// that speaks briefly and then enters DTX must not have that utterance retained until later speech.
const GROUP_MIX_MAX_PRIME_CHUNKS: u32 = 20;
/// Four 60 ms frames; overflow drops oldest samples to preserve real time.
pub const GROUP_MIX_QUEUE_CAPACITY: usize = 3_840;
/// Public sink frame size (60 ms at 16 kHz).
pub const GROUP_MIX_OUTPUT_SAMPLES: usize = 960;

struct ParticipantQueue {
    samples: VecDeque<i16>,
    primed: bool,
    priming_chunks: u32,
    /// The largest packet this participant has delivered, which is what its prefill and capacity are
    /// measured in. Largest rather than latest so a stream alternating sizes settles on the cushion
    /// its longest packet needs instead of flapping.
    packet_samples: usize,
}

impl ParticipantQueue {
    fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(GROUP_MIX_QUEUE_CAPACITY),
            primed: false,
            priming_chunks: 0,
            packet_samples: 0,
        }
    }

    /// Two packets, floored at the 60 ms default so no stream primes faster than today's.
    fn prefill(&self) -> usize {
        self.packet_samples
            .saturating_mul(2)
            .max(GROUP_MIX_PREFILL_SAMPLES)
    }

    /// Four packets, on the same footing: the capacity has to stay above the prefill or a queue
    /// could never reach it, and overflow would drop the very samples priming waits for.
    fn capacity(&self) -> usize {
        self.packet_samples
            .saturating_mul(4)
            .max(GROUP_MIX_QUEUE_CAPACITY)
    }
}

/// Independent bounded queues mixed into fixed 10 ms, saturating PCM chunks.
#[derive(Default)]
pub struct ParticipantAudioMixer {
    allowed: Option<HashSet<String>>,
    queues: HashMap<String, ParticipantQueue>,
}

impl ParticipantAudioMixer {
    pub fn new() -> Self {
        Self::default()
    }

    /// Atomically gate future inserts to the authoritative active roster and
    /// remove departed participant queues.
    pub fn retain<I, S>(&mut self, participants: I)
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let allowed = participants
            .into_iter()
            .map(Into::into)
            .collect::<HashSet<_>>();
        self.queues
            .retain(|participant, _| allowed.contains(participant));
        self.allowed = Some(allowed);
    }

    /// Drop queued PCM for a participant whose relay media session was replaced.
    pub fn reset(&mut self, participant: &str) {
        self.queues.remove(participant);
    }

    /// Queue owned PCM for one participant. Returns false for a departed or
    /// otherwise non-authoritative participant.
    ///
    /// `pcm` is ONE decoded packet. That is what every caller passes, and it is what lets the queue
    /// size its prefill and capacity to this participant's cadence rather than to a 60 ms frame it
    /// may not be sending; handing it several packets at once would read as one long packet and ask
    /// for a correspondingly long cushion.
    pub fn push(&mut self, participant: &str, pcm: &[i16]) -> bool {
        if pcm.is_empty()
            || self
                .allowed
                .as_ref()
                .is_some_and(|allowed| !allowed.contains(participant))
        {
            return false;
        }
        let queue = self
            .queues
            .entry(participant.to_string())
            .or_insert_with(ParticipantQueue::new);
        queue.packet_samples = queue.packet_samples.max(pcm.len());
        queue.samples.extend(pcm.iter().copied());
        let capacity = queue.capacity();
        while queue.samples.len() > capacity {
            queue.samples.pop_front();
        }
        if queue.samples.len() >= queue.prefill() {
            queue.primed = true;
            queue.priming_chunks = 0;
        }
        true
    }

    /// Mix one 10 ms chunk. A single ready participant is bit-identical;
    /// simultaneous speakers sum with i16 saturation.
    pub fn mix_chunk(&mut self) -> Option<Vec<i16>> {
        for queue in self.queues.values_mut().filter(|queue| !queue.primed) {
            if queue.samples.is_empty() {
                queue.priming_chunks = 0;
                continue;
            }
            queue.priming_chunks = queue.priming_chunks.saturating_add(1);
            if queue.priming_chunks >= GROUP_MIX_MAX_PRIME_CHUNKS {
                queue.primed = true;
                queue.priming_chunks = 0;
            }
        }
        let ready = self
            .queues
            .values()
            .filter(|queue| queue.primed && !queue.samples.is_empty())
            .count();
        if ready == 0 {
            return None;
        }

        let mut mixed = vec![0i32; GROUP_MIX_CHUNK_SAMPLES];
        for queue in self
            .queues
            .values_mut()
            .filter(|queue| queue.primed && !queue.samples.is_empty())
        {
            for sample in &mut mixed {
                *sample += i32::from(queue.samples.pop_front().unwrap_or_default());
            }
            if queue.samples.is_empty() {
                queue.primed = false;
                queue.priming_chunks = 0;
            }
        }
        Some(
            mixed
                .into_iter()
                .map(|sample| sample.clamp(i16::MIN as i32, i16::MAX as i32) as i16)
                .collect(),
        )
    }
}

/// Reframes 10 ms mixer chunks into the existing 60 ms audio sink contract.
#[derive(Default)]
pub struct ParticipantAudioFramer {
    samples: Vec<i16>,
}

impl ParticipantAudioFramer {
    pub fn new() -> Self {
        Self {
            samples: Vec::with_capacity(GROUP_MIX_OUTPUT_SAMPLES),
        }
    }

    pub fn push(&mut self, chunk: &[i16]) -> Option<Vec<i16>> {
        debug_assert_eq!(
            chunk.len(),
            GROUP_MIX_CHUNK_SAMPLES,
            "group mixer chunks must be exactly 10 ms"
        );
        if chunk.len() != GROUP_MIX_CHUNK_SAMPLES {
            return None;
        }
        self.samples.extend_from_slice(chunk);
        if self.samples.len() < GROUP_MIX_OUTPUT_SAMPLES {
            return None;
        }
        Some(std::mem::replace(
            &mut self.samples,
            Vec::with_capacity(GROUP_MIX_OUTPUT_SAMPLES),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefill_and_single_participant_are_bit_exact() {
        let mut mixer = ParticipantAudioMixer::new();
        assert!(mixer.push("alice", &vec![1234; 960]));
        assert!(mixer.mix_chunk().is_none());
        assert!(mixer.push("alice", &vec![1234; 960]));
        assert_eq!(
            mixer.mix_chunk().unwrap(),
            vec![1234; GROUP_MIX_CHUNK_SAMPLES]
        );
    }

    #[test]
    fn short_burst_flushes_after_bounded_priming() {
        let mut mixer = ParticipantAudioMixer::new();
        assert!(mixer.push("alice", &vec![1234; GROUP_MIX_OUTPUT_SAMPLES]));
        for _ in 1..GROUP_MIX_MAX_PRIME_CHUNKS {
            assert!(
                mixer.mix_chunk().is_none(),
                "partial speech stays buffered during the bounded jitter cushion"
            );
        }
        assert_eq!(
            mixer.mix_chunk().expect("short burst must eventually play"),
            vec![1234; GROUP_MIX_CHUNK_SAMPLES]
        );
        for _ in 1..(GROUP_MIX_OUTPUT_SAMPLES / GROUP_MIX_CHUNK_SAMPLES) {
            assert_eq!(
                mixer.mix_chunk().expect("the admitted burst drains"),
                vec![1234; GROUP_MIX_CHUNK_SAMPLES]
            );
        }
        assert!(mixer.mix_chunk().is_none());
    }

    // Both bounds are written in units of the 60 ms frame, so a participant on the newly supported
    // 120 ms packets reached a fixed 1920 prefill with its FIRST packet -- playable with no cushion,
    // and drained again before the next one arrives, which turns ordinary relay jitter into
    // recurring gaps. The cushion has to be measured in that participant's packets.
    #[test]
    fn a_participant_on_longer_packets_still_gets_a_packet_of_cushion() {
        const LONG: usize = 1_920; // 120 ms at 16 kHz

        let mut mixer = ParticipantAudioMixer::new();
        mixer.push("alice", &vec![50; LONG]);
        assert!(
            mixer.mix_chunk().is_none(),
            "one 120ms packet is the whole prefill under a fixed bound, and no cushion at all"
        );

        mixer.push("alice", &vec![50; LONG]);
        assert_eq!(
            mixer.mix_chunk().unwrap(),
            vec![50; GROUP_MIX_CHUNK_SAMPLES],
            "the second packet is what makes it playable"
        );

        // And a 60 ms participant is unchanged: two of its packets, as before.
        let mut short = ParticipantAudioMixer::new();
        short.push("bob", &vec![70; GROUP_MIX_OUTPUT_SAMPLES]);
        assert!(short.mix_chunk().is_none());
        short.push("bob", &vec![70; GROUP_MIX_OUTPUT_SAMPLES]);
        assert_eq!(
            short.mix_chunk().unwrap(),
            vec![70; GROUP_MIX_CHUNK_SAMPLES]
        );
    }

    #[test]
    fn simultaneous_speakers_sum_and_saturate() {
        let mut mixer = ParticipantAudioMixer::new();
        for _ in 0..2 {
            mixer.push("alice", &vec![20_000; GROUP_MIX_OUTPUT_SAMPLES]);
            mixer.push("bob", &vec![20_000; GROUP_MIX_OUTPUT_SAMPLES]);
        }
        assert_eq!(
            mixer.mix_chunk().unwrap(),
            vec![i16::MAX; GROUP_MIX_CHUNK_SAMPLES]
        );

        let mut negative = ParticipantAudioMixer::new();
        for _ in 0..2 {
            negative.push("alice", &vec![-20_000; GROUP_MIX_OUTPUT_SAMPLES]);
            negative.push("bob", &vec![-20_000; GROUP_MIX_OUTPUT_SAMPLES]);
        }
        assert_eq!(
            negative.mix_chunk().unwrap(),
            vec![i16::MIN; GROUP_MIX_CHUNK_SAMPLES]
        );
    }

    #[test]
    fn authoritative_roster_removes_and_blocks_departed_participants() {
        let mut mixer = ParticipantAudioMixer::new();
        for _ in 0..2 {
            mixer.push("alice", &vec![100; GROUP_MIX_OUTPUT_SAMPLES]);
            mixer.push("bob", &vec![200; GROUP_MIX_OUTPUT_SAMPLES]);
        }
        mixer.retain(["alice"]);
        assert!(!mixer.push("bob", &[200; GROUP_MIX_CHUNK_SAMPLES]));
        assert_eq!(
            mixer.mix_chunk().unwrap(),
            vec![100; GROUP_MIX_CHUNK_SAMPLES]
        );
    }

    #[test]
    fn overflow_discards_oldest_audio() {
        let mut mixer = ParticipantAudioMixer::new();
        for _ in 0..(GROUP_MIX_QUEUE_CAPACITY / GROUP_MIX_OUTPUT_SAMPLES) {
            mixer.push("alice", &vec![1; GROUP_MIX_OUTPUT_SAMPLES]);
        }
        mixer.push("alice", &vec![2; GROUP_MIX_CHUNK_SAMPLES]);
        assert_eq!(mixer.mix_chunk().unwrap(), vec![1; GROUP_MIX_CHUNK_SAMPLES]);
        for _ in 0..(GROUP_MIX_QUEUE_CAPACITY / GROUP_MIX_CHUNK_SAMPLES - 2) {
            mixer.mix_chunk();
        }
        assert_eq!(mixer.mix_chunk().unwrap(), vec![2; GROUP_MIX_CHUNK_SAMPLES]);
    }

    #[test]
    fn framer_emits_exactly_one_public_frame() {
        let mut framer = ParticipantAudioFramer::new();
        for index in 0..5 {
            assert!(framer.push(&vec![index; GROUP_MIX_CHUNK_SAMPLES]).is_none());
        }
        let frame = framer.push(&vec![5; GROUP_MIX_CHUNK_SAMPLES]).unwrap();
        assert_eq!(frame.len(), GROUP_MIX_OUTPUT_SAMPLES);
        assert_eq!(
            &frame[..GROUP_MIX_CHUNK_SAMPLES],
            &[0; GROUP_MIX_CHUNK_SAMPLES]
        );
        assert_eq!(
            &frame[5 * GROUP_MIX_CHUNK_SAMPLES..],
            &[5; GROUP_MIX_CHUNK_SAMPLES]
        );
    }

    #[test]
    #[should_panic(expected = "group mixer chunks must be exactly 10 ms")]
    fn framer_rejects_wrong_sized_chunks_loudly_in_debug_builds() {
        let mut framer = ParticipantAudioFramer::new();
        let _ = framer.push(&[0; GROUP_MIX_CHUNK_SAMPLES - 1]);
    }
}
