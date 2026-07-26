//! Bounded, participant-aware PCM mixer for group-call playout.

use std::collections::{HashMap, HashSet, VecDeque};

/// 10 ms at the native 16 kHz mono playout rate.
pub const GROUP_MIX_CHUNK_SAMPLES: usize = 160;
/// Two 60 ms codec frames before a participant joins playout.
pub const GROUP_MIX_PREFILL_SAMPLES: usize = 1_920;
/// Four 60 ms frames; overflow drops oldest samples to preserve real time.
pub const GROUP_MIX_QUEUE_CAPACITY: usize = 3_840;
/// Public sink frame size (60 ms at 16 kHz).
pub const GROUP_MIX_OUTPUT_SAMPLES: usize = 960;

struct ParticipantQueue {
    samples: VecDeque<i16>,
    primed: bool,
}

impl ParticipantQueue {
    fn new() -> Self {
        Self {
            samples: VecDeque::with_capacity(GROUP_MIX_QUEUE_CAPACITY),
            primed: false,
        }
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

    /// Queue owned PCM for one participant. Returns false for a departed or
    /// otherwise non-authoritative participant.
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
        queue.samples.extend(pcm.iter().copied());
        while queue.samples.len() > GROUP_MIX_QUEUE_CAPACITY {
            queue.samples.pop_front();
        }
        if queue.samples.len() >= GROUP_MIX_PREFILL_SAMPLES {
            queue.primed = true;
        }
        true
    }

    /// Mix one 10 ms chunk. A single ready participant is bit-identical;
    /// simultaneous speakers sum with i16 saturation.
    pub fn mix_chunk(&mut self) -> Option<Vec<i16>> {
        let ready = self
            .queues
            .values()
            .filter(|queue| queue.primed && queue.samples.len() >= GROUP_MIX_CHUNK_SAMPLES)
            .count();
        if ready == 0 {
            return None;
        }

        let mut mixed = vec![0i32; GROUP_MIX_CHUNK_SAMPLES];
        for queue in self
            .queues
            .values_mut()
            .filter(|queue| queue.primed && queue.samples.len() >= GROUP_MIX_CHUNK_SAMPLES)
        {
            for sample in &mut mixed {
                *sample += i32::from(queue.samples.pop_front().unwrap_or_default());
            }
            if queue.samples.len() < GROUP_MIX_CHUNK_SAMPLES {
                queue.primed = false;
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
    fn simultaneous_speakers_sum_and_saturate() {
        let mut mixer = ParticipantAudioMixer::new();
        mixer.push("alice", &vec![20_000; GROUP_MIX_PREFILL_SAMPLES]);
        mixer.push("bob", &vec![20_000; GROUP_MIX_PREFILL_SAMPLES]);
        assert_eq!(
            mixer.mix_chunk().unwrap(),
            vec![i16::MAX; GROUP_MIX_CHUNK_SAMPLES]
        );

        let mut negative = ParticipantAudioMixer::new();
        negative.push("alice", &vec![-20_000; GROUP_MIX_PREFILL_SAMPLES]);
        negative.push("bob", &vec![-20_000; GROUP_MIX_PREFILL_SAMPLES]);
        assert_eq!(
            negative.mix_chunk().unwrap(),
            vec![i16::MIN; GROUP_MIX_CHUNK_SAMPLES]
        );
    }

    #[test]
    fn authoritative_roster_removes_and_blocks_departed_participants() {
        let mut mixer = ParticipantAudioMixer::new();
        mixer.push("alice", &vec![100; GROUP_MIX_PREFILL_SAMPLES]);
        mixer.push("bob", &vec![200; GROUP_MIX_PREFILL_SAMPLES]);
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
        mixer.push("alice", &vec![1; GROUP_MIX_QUEUE_CAPACITY]);
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
}
