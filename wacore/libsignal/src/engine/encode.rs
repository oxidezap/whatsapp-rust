// SPDX-License-Identifier: MIT
//! Direct protobuf encoding from the data schema in
//! `waproto/signal-storage.snapshot`. A counting sink computes nested lengths;
//! the writing sink then appends those messages to the caller's buffer.
use crate::engine::state::{
    ArchivedSession, Chain, DerivedMessageKey, MessageKey, PendingPreKey, Reservation,
    SenderKeyRecord, SenderKeyState, SenderMessageKey, SessionState,
};

trait Encodable {
    fn encode<S: Sink>(&self, sink: &mut S);
}

trait Sink {
    fn raw(&mut self, bytes: &[u8]);
    fn message<M: Encodable + ?Sized>(&mut self, field: u32, message: &M);

    fn varint(&mut self, mut value: u64) {
        let mut encoded = [0; 10];
        let mut used = 0;
        while value >= 128 {
            encoded[used] = (value as u8 & 127) | 128;
            used += 1;
            value >>= 7;
        }
        encoded[used] = value as u8;
        self.raw(&encoded[..=used]);
    }

    fn uint(&mut self, field: u32, value: u64) {
        self.varint(u64::from(field) << 3);
        self.varint(value);
    }

    fn bytes(&mut self, field: u32, value: &[u8]) {
        self.varint((u64::from(field) << 3) | 2);
        self.varint(value.len() as u64);
        self.raw(value);
    }

    fn public_key(&mut self, field: u32, public: &[u8; 32], tagged: bool) {
        self.varint((u64::from(field) << 3) | 2);
        self.varint(if tagged { 33 } else { 32 });
        if tagged {
            self.raw(&[5]);
        }
        self.raw(public);
    }
}

#[derive(Default)]
struct CountSink(usize);

fn encoded_len<M: Encodable + ?Sized>(message: &M) -> usize {
    let mut sink = CountSink::default();
    message.encode(&mut sink);
    sink.0
}

impl Sink for CountSink {
    fn raw(&mut self, bytes: &[u8]) {
        self.0 += bytes.len();
    }

    fn message<M: Encodable + ?Sized>(&mut self, field: u32, message: &M) {
        let length = encoded_len(message);
        self.varint((u64::from(field) << 3) | 2);
        self.varint(length as u64);
        self.0 += length;
    }
}

struct VecSink<'a>(&'a mut Vec<u8>);

impl Sink for VecSink<'_> {
    fn raw(&mut self, bytes: &[u8]) {
        self.0.extend_from_slice(bytes);
    }

    fn message<M: Encodable + ?Sized>(&mut self, field: u32, message: &M) {
        let length = encoded_len(message);
        self.varint((u64::from(field) << 3) | 2);
        self.varint(length as u64);
        message.encode(self);
    }
}

impl Encodable for DerivedMessageKey {
    fn encode<S: Sink>(&self, sink: &mut S) {
        sink.bytes(2, &self.cipher);
        sink.bytes(3, &self.mac);
        sink.bytes(4, &self.iv);
    }
}

impl Encodable for MessageKey {
    fn encode<S: Sink>(&self, sink: &mut S) {
        sink.uint(1, self.index().into());
        match self {
            Self::Seed { seed, .. } => sink.bytes(100, seed),
            Self::Derived { material, .. } => material.encode(sink),
            Self::Stored { material, .. } => {
                material.material.encode(sink);
                sink.bytes(100, &material.seed);
            }
        }
    }
}

struct ChainPosition<'a>(&'a Chain);

impl Encodable for ChainPosition<'_> {
    fn encode<S: Sink>(&self, sink: &mut S) {
        sink.uint(1, self.0.index.into());
        if !self.0.exhausted {
            sink.bytes(2, &self.0.key);
        }
    }
}

impl Encodable for Chain {
    fn encode<S: Sink>(&self, sink: &mut S) {
        sink.public_key(1, &self.ratchet, true);
        if let Some(private) = &self.private {
            sink.bytes(2, private);
        }
        sink.message(3, &ChainPosition(self));
        for key in &self.skipped {
            sink.message(4, key);
        }
        if let Some(limit) = self.message_key_limit {
            sink.uint(100, limit.into());
        }
    }
}

impl Encodable for PendingPreKey {
    fn encode<S: Sink>(&self, sink: &mut S) {
        if let Some(id) = self.pre_key_id {
            sink.uint(1, id.into());
        }
        sink.public_key(2, &self.base_key, true);
        if let Some(id) = self.signed_pre_key_id {
            sink.uint(3, id as i64 as u64);
        }
        if let Some(id) = self.kyber_pre_key_id {
            sink.uint(4, id.into());
        }
        if let Some(ciphertext) = &self.kyber_ciphertext {
            sink.bytes(5, ciphertext);
        }
    }
}

impl Encodable for SessionState {
    fn encode<S: Sink>(&self, sink: &mut S) {
        sink.uint(1, self.version.into());
        sink.public_key(2, &self.local_identity, true);
        sink.public_key(3, &self.remote_identity, true);
        sink.bytes(4, &self.root);
        sink.uint(5, self.previous_counter.into());
        if let Some(sender) = &self.sender {
            sink.message(6, sender);
        }
        for receiver in &self.receivers {
            sink.message(7, receiver);
        }
        if let Some(exchange) = &self.pending_key_exchange {
            sink.bytes(8, exchange);
        }
        if let Some(pending) = self.pending_pre_key.as_deref() {
            sink.message(9, pending);
        }
        sink.uint(10, self.remote_registration.into());
        sink.uint(11, self.local_registration.into());
        sink.uint(12, u64::from(self.needs_refresh));
        if let Some(base) = &self.alice_base_key {
            sink.public_key(13, base, self.alice_base_key_tagged);
        }
    }
}

struct ReservationFields<'a> {
    reservation: &'a Reservation,
    incarnation: Option<&'a [u8; 16]>,
}

impl Encodable for ReservationFields<'_> {
    fn encode<S: Sink>(&self, sink: &mut S) {
        if let Some(ceiling) = self.reservation.ceiling.filter(|ceiling| *ceiling > 0) {
            sink.uint(100, ceiling.into());
            if let Some(token) = self.incarnation.or(self.reservation.incarnation.as_ref()) {
                sink.bytes(101, token);
            }
        }
    }
}

struct SessionParts<'a> {
    current: Option<&'a SessionState>,
    previous: &'a [ArchivedSession],
    metadata: ReservationFields<'a>,
}

impl Encodable for SessionParts<'_> {
    fn encode<S: Sink>(&self, sink: &mut S) {
        if let Some(current) = self.current {
            sink.message(1, current);
        }
        for previous in self.previous {
            sink.bytes(2, previous.encoded());
        }
        self.metadata.encode(sink);
    }
}

struct SenderPosition<'a>(&'a SenderKeyState);

impl Encodable for SenderPosition<'_> {
    fn encode<S: Sink>(&self, sink: &mut S) {
        sink.uint(1, self.0.iteration.into());
        sink.bytes(2, &self.0.chain_key);
    }
}

struct SenderSigningKey<'a>(&'a SenderKeyState);

impl Encodable for SenderSigningKey<'_> {
    fn encode<S: Sink>(&self, sink: &mut S) {
        sink.public_key(1, &self.0.signing_public, true);
        if let Some(private) = &self.0.signing_private {
            sink.bytes(2, private);
        }
    }
}

impl Encodable for SenderMessageKey {
    fn encode<S: Sink>(&self, sink: &mut S) {
        sink.uint(1, self.iteration.into());
        sink.bytes(2, &self.seed);
    }
}

impl Encodable for SenderKeyState {
    fn encode<S: Sink>(&self, sink: &mut S) {
        sink.uint(1, self.id.into());
        sink.message(2, &SenderPosition(self));
        sink.message(3, &SenderSigningKey(self));
        for key in &self.skipped {
            sink.message(4, key);
        }
    }
}

struct SenderRecord<'a> {
    record: &'a SenderKeyRecord,
    metadata: ReservationFields<'a>,
}

impl Encodable for SenderRecord<'_> {
    fn encode<S: Sink>(&self, sink: &mut S) {
        for state in &self.record.states {
            sink.message(1, state);
        }
        self.metadata.encode(sink);
    }
}

pub(crate) fn append_session_parts(
    current: Option<&SessionState>,
    previous: &[ArchivedSession],
    reservation: &Reservation,
    incarnation: Option<&[u8; 16]>,
    out: &mut Vec<u8>,
) {
    let message = SessionParts {
        current,
        previous,
        metadata: ReservationFields {
            reservation,
            incarnation,
        },
    };
    // Size fresh buffers once, including sessions without history. Reused
    // buffers keep the direct path unless their archived bytes cannot fit.
    let history_bytes: usize = previous.iter().map(|state| state.encoded().len()).sum();
    if out.capacity() == 0 || history_bytes > out.capacity() - out.len() {
        out.reserve(encoded_len(&message));
    }
    message.encode(&mut VecSink(out));
}

pub(crate) fn append_sender_record(
    record: &SenderKeyRecord,
    incarnation: Option<&[u8; 16]>,
    out: &mut Vec<u8>,
) {
    let message = SenderRecord {
        record,
        metadata: ReservationFields {
            reservation: &record.reservation,
            incarnation,
        },
    };
    if out.capacity() == 0 {
        out.reserve(encoded_len(&message));
    }
    message.encode(&mut VecSink(out));
}

pub(crate) fn session_state_size(state: &SessionState) -> usize {
    encoded_len(state)
}

pub(crate) fn receiver_field_size(chain: &Chain, omitted_bytes: usize) -> Option<usize> {
    let length = encoded_len(chain).checked_sub(omitted_bytes)?;
    let mut sink = CountSink::default();
    sink.varint((7 << 3) | 2);
    sink.varint(length as u64);
    Some(sink.0 + length)
}

pub(crate) fn append_session_state(state: &SessionState, out: &mut Vec<u8>) {
    state.encode(&mut VecSink(out));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{RustCrypto, codec};
    use std::collections::VecDeque;

    #[test]
    #[allow(clippy::disallowed_methods)]
    fn imported_receiver_size_preserves_nested_length_prefix_boundaries() {
        use buffa::Message as _;
        use waproto::whatsapp::{SessionStructure, session_structure};
        for count in [0, 1, 2, 3, 416, 417, 418, 419] {
            for presence in 0..3 {
                let mut chain = Chain::new([7; 32], None, [0; 32]);
                chain.exhausted = true;
                chain.message_key_limit = Some(8192);
                chain.skipped = (0..count)
                    .map(|_| MessageKey::Seed {
                        index: u32::MAX,
                        seed: [8; 32],
                    })
                    .collect();
                let mut proto = session_structure::Chain::default();
                proto.sender_ratchet_key =
                    Some(crate::engine::wire::tagged_key(&chain.ratchet).to_vec());
                proto.message_key_limit = chain.message_key_limit;
                if presence > 0 {
                    let mut position = session_structure::chain::ChainKey::default();
                    position.index = (presence == 2).then_some(0);
                    proto.chain_key = Some(position).into();
                }
                proto.message_keys = (0..count)
                    .map(|_| {
                        let mut key = session_structure::chain::MessageKey::default();
                        key.index = Some(u32::MAX);
                        key.seed = Some(bytes::Bytes::from_static(&[8; 32]));
                        key
                    })
                    .collect();
                let mut session = SessionStructure::default();
                session.receiver_chains = vec![proto];
                let omitted = [4, 2, 0][presence];
                assert_eq!(
                    receiver_field_size(&chain, omitted).unwrap(),
                    session.encode_to_vec().len(),
                    "count={count}, presence={presence}"
                );
            }
        }
    }

    fn state() -> SessionState {
        let mut sender = Chain::new([4; 32], Some([5; 32]), [6; 32]);
        sender.index = 128;
        sender.message_key_limit = Some(2500);
        for index in 0..40 {
            sender.skipped.push_back(if index % 2 == 0 {
                MessageKey::Seed {
                    index,
                    seed: [index as u8; 32],
                }
            } else {
                MessageKey::Derived {
                    index,
                    material: Box::new(DerivedMessageKey {
                        cipher: [index as u8; 32],
                        mac: [8; 32],
                        iv: [9; 16],
                    }),
                }
            });
        }
        let mut receiver = Chain::new([7; 32], None, [0; 32]);
        receiver.index = u32::MAX;
        receiver.exhausted = true;
        SessionState {
            metadata: std::sync::Arc::new(crate::engine::SessionMetadata {
                version: 3,
                local_identity: [1; 32],
                remote_identity: [2; 32],
                root: [3; 32],
                previous_counter: u32::MAX,
                local_registration: 0,
                remote_registration: u32::MAX,
                needs_refresh: true,
                alice_base_key: Some([12; 32]),
                alice_base_key_tagged: false,
                pending_key_exchange: Some(vec![8, 127]),
            }),
            sender: Some(sender),
            receivers: vec![receiver],
            pending_pre_key: Some(std::sync::Arc::new(PendingPreKey {
                pre_key_id: Some(u32::MAX),
                signed_pre_key_id: Some(-1),
                base_key: [10; 32],
                kyber_pre_key_id: Some(0),
                kyber_ciphertext: Some(vec![11; 130]),
            })),
        }
    }

    #[test]
    fn session_snapshots_share_metadata_until_a_ratchet_field_changes() {
        let original = state();
        let expected = codec::encode_session_state(&original);
        let mut sibling = original.clone();
        let shared = std::sync::Arc::downgrade(&original.metadata);
        sibling.sender.as_mut().unwrap().index += 1;
        assert!(std::sync::Arc::ptr_eq(
            &original.metadata,
            &sibling.metadata
        ));
        sibling.root = [19; 32];
        assert!(!std::sync::Arc::ptr_eq(
            &original.metadata,
            &sibling.metadata
        ));
        assert_eq!(original.root, [3; 32]);
        assert_eq!(sibling.root, [19; 32]);
        assert_eq!(codec::encode_session_state(&original), expected);
        assert_ne!(codec::encode_session_state(&sibling), expected);
        drop(original);
        assert!(shared.upgrade().is_none());
        assert!(size_of::<SessionState>() <= 176);
        println!(
            "session metadata allocation: {} B; hot state: {} B; record: {} B",
            size_of::<crate::engine::SessionMetadata>() + 2 * size_of::<usize>(),
            size_of::<SessionState>(),
            size_of::<crate::protocol::SessionRecord>(),
        );
    }

    #[test]
    fn pending_prekey_snapshots_share_metadata_and_isolate_changes() {
        let original = state();
        let expected = codec::encode_session_state(&original);
        let mut sibling = original.clone();
        assert!(std::sync::Arc::ptr_eq(
            original.pending_pre_key.as_ref().unwrap(),
            sibling.pending_pre_key.as_ref().unwrap()
        ));
        std::sync::Arc::make_mut(sibling.pending_pre_key.as_mut().unwrap()).signed_pre_key_id =
            Some(17);
        assert_eq!(codec::encode_session_state(&original), expected);
        assert_ne!(codec::encode_session_state(&sibling), expected);
        sibling.pending_pre_key = None;
        assert!(original.pending_pre_key.is_some());
        assert_eq!(codec::encode_session_state(&original), expected);
        assert_eq!(
            size_of::<Option<std::sync::Arc<PendingPreKey>>>(),
            size_of::<usize>()
        );
        println!(
            "session state: {} B; record: {} B; optional prekey inline: {} B; allocated prekey: {} B",
            size_of::<SessionState>(),
            size_of::<crate::protocol::SessionRecord>(),
            size_of::<Option<std::sync::Arc<PendingPreKey>>>(),
            size_of::<PendingPreKey>() + 2 * size_of::<usize>(),
        );
    }

    #[test]
    fn direct_state_encoding_matches_count_and_preserves_all_supported_data() {
        let mut state = state();
        for tagged in [false, true] {
            state.alice_base_key_tagged = tagged;
            let length = encoded_len(&state);
            assert!(length > 1024);
            let mut out = Vec::with_capacity(length + 2);
            out.extend_from_slice(&[0xaa, 0xbb]);
            let capacity = out.capacity();
            append_session_state(&state, &mut out);
            assert_eq!(&out[..2], &[0xaa, 0xbb]);
            assert_eq!(out.len(), length + 2);
            assert_eq!(out.capacity(), capacity);
            let decoded = codec::decode_session_state(&out[2..]).unwrap();
            assert!(decoded == state);
        }
        std::sync::Arc::make_mut(state.pending_pre_key.as_mut().unwrap()).signed_pre_key_id = None;
        let mut out = Vec::new();
        append_session_state(&state, &mut out);
        assert!(codec::decode_session_state(&out).unwrap() == state);
    }

    #[test]
    fn zero_ceiling_omits_all_reservation_fields_and_override_wins() {
        let mut reservation = Reservation {
            incarnation: Some([7; 16]),
            ..Reservation::default()
        };
        for ceiling in [None, Some(0)] {
            reservation.ceiling = ceiling;
            let mut out = Vec::new();
            append_session_parts(None, &[], &reservation, Some(&[8; 16]), &mut out);
            assert!(out.is_empty());
        }
        reservation.ceiling = Some(1);
        let mut out = Vec::new();
        append_session_parts(None, &[], &reservation, Some(&[8; 16]), &mut out);
        let mut expected = vec![0xa0, 0x06, 1, 0xaa, 0x06, 16];
        expected.extend_from_slice(&[8; 16]);
        assert_eq!(out, expected);
    }

    #[test]
    fn session_parts_append_archives_verbatim_in_order() {
        let current = state();
        let mut first = Vec::new();
        append_session_state(&current, &mut first);
        let mut second_state = state();
        second_state.remote_registration = 17;
        let mut second = Vec::new();
        append_session_state(&second_state, &mut second);
        let previous = [
            ArchivedSession {
                bytes: first.into(),
            },
            ArchivedSession {
                bytes: second.into(),
            },
        ];
        let reservation = Reservation {
            ceiling: Some(129),
            incarnation: Some([7; 16]),
            ..Reservation::default()
        };
        let parts = SessionParts {
            current: Some(&current),
            previous: &previous,
            metadata: ReservationFields {
                reservation: &reservation,
                incarnation: Some(&[8; 16]),
            },
        };
        let length = encoded_len(&parts);
        let mut out = Vec::with_capacity(length);
        let capacity = out.capacity();
        append_session_parts(
            Some(&current),
            &previous,
            &reservation,
            Some(&[8; 16]),
            &mut out,
        );
        assert_eq!(out.len(), length);
        assert_eq!(out.capacity(), capacity);
        let decoded = codec::decode_session_record_live(&RustCrypto, &out, &[8; 16]).unwrap();
        assert!(decoded.current.as_ref() == Some(&current));
        for (decoded, original) in decoded.previous.iter().zip(previous.iter()) {
            assert_eq!(decoded.encoded(), original.encoded());
        }
        assert_eq!(decoded.previous.len(), previous.len());
        assert_eq!(decoded.reservation.ceiling, Some(129));
        assert_eq!(decoded.reservation.incarnation, Some([8; 16]));
    }

    #[test]
    fn sender_record_direct_nested_encoding_preserves_positions_and_keys() {
        let mut record = SenderKeyRecord::default();
        for id in [0, u32::MAX] {
            let mut skipped = VecDeque::new();
            for iteration in 0..40 {
                skipped.push_back(SenderMessageKey {
                    iteration,
                    seed: [iteration as u8; 32],
                });
            }
            record.states.push(SenderKeyState {
                id,
                iteration: 128,
                chain_key: [1; 32],
                signing_public: [2; 32],
                signing_private: (id == 0).then_some([3; 32]),
                skipped: skipped.into(),
            });
        }
        record.reservation.ceiling = Some(129);
        let message = SenderRecord {
            record: &record,
            metadata: ReservationFields {
                reservation: &record.reservation,
                incarnation: Some(&[8; 16]),
            },
        };
        let length = encoded_len(&message);
        assert!(length > 1024);
        let mut out = Vec::with_capacity(length);
        let capacity = out.capacity();
        append_sender_record(&record, Some(&[8; 16]), &mut out);
        assert_eq!(out.len(), length);
        assert_eq!(out.capacity(), capacity);
        let decoded = codec::decode_sender_key_record_live(&RustCrypto, &out, &[8; 16]).unwrap();
        assert!(decoded.states == record.states);
    }
}
