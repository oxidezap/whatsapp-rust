// SPDX-License-Identifier: MIT
//! Synthetic storage/ciphertext captured from the historical writer at the
//! recorded commit. The vectors are included in this test package.
use rand::{RngExt as _, SeedableRng as _, rngs::StdRng};
use serde_json::Value;
use wacore_libsignal::{
    core,
    engine::{self, RandomSource, RustCrypto},
};

fn vectors() -> Value {
    serde_json::from_str(include_str!("vectors/historical.json")).unwrap()
}
fn bytes(value: &Value, key: &str) -> Vec<u8> {
    hex::decode(value[key].as_str().unwrap()).unwrap()
}
fn incarnation() -> [u8; 16] {
    [0x19; 16]
}
struct Random(StdRng);
impl Random {
    fn seeded() -> Self {
        Self(StdRng::seed_from_u64(1828))
    }
}
impl RandomSource for Random {
    fn fill(&mut self, output: &mut [u8]) -> engine::Result<()> {
        self.0.fill(output);
        Ok(())
    }
}

#[test]
fn historical_key_json_and_cbor_need_no_conversion() {
    let all = vectors();
    let key = &all["key"];
    assert_eq!(
        all["source_commit"],
        "7ef89225a427d59397995b6032ba7cb46e8addff"
    );
    assert_eq!(all["synthetic_only"], true);
    let decoded: core::KeyPair = serde_json::from_value(key["json"].clone()).unwrap();
    assert_eq!(
        decoded.public_key.public_key_bytes().as_slice(),
        bytes(key, "public")
    );
    assert_eq!(
        decoded.private_key.serialize().as_slice(),
        bytes(key, "private")
    );
    assert_eq!(serde_json::to_value(&decoded).unwrap(), key["json"]);
    let binary = bytes(key, "cbor");
    let reloaded: core::KeyPair = ciborium::from_reader(binary.as_slice()).unwrap();
    assert_eq!(reloaded, decoded);
    let mut encoded = Vec::new();
    ciborium::into_writer(&decoded, &mut encoded).unwrap();
    assert_eq!(encoded, binary);
    let identity: core::IdentityKeyPair =
        serde_json::from_value(key["identity_json"].clone()).unwrap();
    assert_eq!(
        serde_json::to_value(identity).unwrap(),
        key["identity_json"]
    );
}

#[test]
fn historical_signatures_authenticate_and_reject_mutation() {
    use engine::CryptoProvider as _;
    let all = vectors();
    let public: [u8; 32] = bytes(&all["key"], "public").try_into().unwrap();
    for row in all["signatures"].as_array().unwrap() {
        let message = bytes(row, "message");
        let mut signature: [u8; 64] = bytes(row, "signature").try_into().unwrap();
        RustCrypto.verify(&public, &message, &signature).unwrap();
        signature[7] ^= 1;
        assert!(RustCrypto.verify(&public, &message, &signature).is_err());
    }
}

#[test]
fn historical_prekeys_accept_raw_and_tagged_keys_and_keep_opaque_timestamps() {
    let all = vectors();
    let expected_private = bytes(&all["key"], "private");
    let expected_public = bytes(&all["key"], "public");
    for input in all["pre_keys"].as_array().unwrap() {
        let input = hex::decode(input.as_str().unwrap()).unwrap();
        let key = engine::codec::decode_pre_key(&input).unwrap();
        assert_eq!(key.id, 73);
        assert_eq!(key.key_pair.private.as_slice(), expected_private);
        assert_eq!(key.key_pair.public.as_slice(), expected_public);
        let runtime = wacore_libsignal::protocol::PreKeyRecord::deserialize(&input).unwrap();
        assert_eq!(
            runtime.serialize().unwrap(),
            engine::codec::encode_pre_key(&key)
        );
    }
    for row in all["signed_pre_keys"].as_array().unwrap() {
        let key = engine::codec::decode_signed_pre_key(&bytes(row, "blob")).unwrap();
        assert_eq!(key.timestamp, row["timestamp"].as_u64().unwrap());
        assert_eq!(key.id, 42);
        assert_eq!(key.signature, [7; 64]);
        assert_eq!(key.key_pair.private.as_slice(), expected_private);
        assert_eq!(key.key_pair.public.as_slice(), expected_public);
    }
}

#[test]
fn existing_v3_and_v4_sessions_resume_with_exact_historical_ciphertext() {
    let all = vectors();
    for row in all["pairwise"].as_array().unwrap() {
        let mut random = Random::seeded();
        let mut engine = engine::Engine::new(&RustCrypto, &mut random);
        let mut sender = engine::codec::decode_session_record_live(
            &RustCrypto,
            &bytes(row, "sender_before"),
            &incarnation(),
        )
        .unwrap();
        assert_eq!(
            sender.current.as_ref().unwrap().version as u64,
            row["version"].as_u64().unwrap()
        );
        assert_eq!(
            engine
                .encrypt(&mut sender, &bytes(row, "plaintext"))
                .unwrap(),
            bytes(row, "first_wire")
        );
        let mut receiver = engine::codec::decode_session_record_live(
            &RustCrypto,
            &bytes(row, "receiver_before"),
            &incarnation(),
        )
        .unwrap();
        assert_eq!(
            engine
                .decrypt(&mut receiver, &bytes(row, "first_wire"))
                .unwrap(),
            bytes(row, "plaintext")
        );
        let mut reply_sender = engine::codec::decode_session_record_live(
            &RustCrypto,
            &bytes(row, "receiver_after"),
            &incarnation(),
        )
        .unwrap();
        assert_eq!(
            engine
                .encrypt(&mut reply_sender, &bytes(row, "reply_plaintext"))
                .unwrap(),
            bytes(row, "reply_wire")
        );
        let mut reply_receiver = engine::codec::decode_session_record_live(
            &RustCrypto,
            &bytes(row, "sender_after"),
            &incarnation(),
        )
        .unwrap();
        assert_eq!(
            engine
                .decrypt(&mut reply_receiver, &bytes(row, "reply_wire"))
                .unwrap(),
            bytes(row, "reply_plaintext")
        );
    }
}

#[test]
#[allow(clippy::disallowed_methods)]
fn historical_unused_receiver_private_bytes_do_not_strand_sessions() {
    use buffa::Message as _;
    let all = vectors();
    for row in all["pairwise"].as_array().unwrap() {
        for length in [0, 1, 31, 32, 33, 64] {
            let mut raw =
                waproto::whatsapp::RecordStructure::decode_from_slice(&bytes(row, "sender_after"))
                    .unwrap();
            raw.current_session.as_option_mut().unwrap().receiver_chains[0]
                .sender_ratchet_key_private = Some(vec![0x37; length]);
            let mut session = engine::codec::decode_session_record_live(
                &RustCrypto,
                &raw.encode_to_vec(),
                &incarnation(),
            )
            .unwrap();
            assert!(
                session.current.as_ref().unwrap().receivers[0]
                    .private
                    .is_none()
            );
            let mut random = Random::seeded();
            assert_eq!(
                engine::Engine::new(&RustCrypto, &mut random)
                    .decrypt(&mut session, &bytes(row, "reply_wire"))
                    .unwrap(),
                bytes(row, "reply_plaintext")
            );
        }
        for length in [1, 31, 33, 64] {
            let mut raw =
                waproto::whatsapp::RecordStructure::decode_from_slice(&bytes(row, "sender_after"))
                    .unwrap();
            raw.current_session
                .as_option_mut()
                .unwrap()
                .sender_chain
                .as_option_mut()
                .unwrap()
                .sender_ratchet_key_private = Some(vec![0x37; length]);
            assert!(
                engine::codec::decode_session_record_live(
                    &RustCrypto,
                    &raw.encode_to_vec(),
                    &incarnation()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn historical_group_component_exports_keep_delayed_messages_and_private_keys() {
    let all = vectors();
    for row in all["group_history"].as_array().unwrap() {
        let mut record =
            engine::codec::decode_sender_key_record(&RustCrypto, &bytes(row, "record")).unwrap();
        assert_eq!(
            record.states[0].iteration as u64,
            row["current_iteration"].as_u64().unwrap()
        );
        assert_eq!(record.states[1].iteration, 0);
        assert_eq!(
            record.states[1].signing_private.map(hex::encode),
            row["historical_private"].as_str().map(str::to_owned)
        );
        let mut runtime = wacore_libsignal::protocol::SenderKeyRecord::deserialize_for_store(
            &bytes(row, "record"),
            &incarnation(),
        )
        .unwrap();
        runtime.reserve_iterations(row["current_iteration"].as_u64().unwrap() as u32);
        let rebuilt = wacore_libsignal::protocol::SenderKeyRecord::from_components(
            runtime.into_components().unwrap(),
        )
        .unwrap();
        let mut exported =
            engine::codec::decode_sender_key_record(&RustCrypto, &rebuilt.serialize().unwrap())
                .unwrap();
        assert_eq!(
            exported.states[0].iteration as u64,
            row["current_iteration"].as_u64().unwrap() + 64
        );
        assert_eq!(exported.states[1].iteration, 0);
        assert_eq!(
            exported.states[1].signing_private.map(hex::encode),
            row["historical_private"].as_str().map(str::to_owned)
        );
        let mut random = Random::seeded();
        assert_eq!(
            engine::Engine::new(&RustCrypto, &mut random)
                .decrypt_sender_key(&mut exported, &bytes(row, "wire"))
                .unwrap(),
            bytes(row, "plaintext")
        );
        assert_eq!(
            engine::Engine::new(&RustCrypto, &mut random)
                .decrypt_sender_key(&mut record, &bytes(row, "wire"))
                .unwrap(),
            bytes(row, "plaintext")
        );
    }
}

#[test]
fn historical_archives_and_local_pending_fields_survive_roundtrip() {
    let all = vectors();
    let input = bytes(&all, "pending_archive");
    let decoded = engine::codec::decode_session_record(&RustCrypto, &input).unwrap();
    assert_eq!(decoded.previous.len(), 1);
    let current = decoded.current.as_ref().unwrap();
    assert_eq!(current.pending_key_exchange, Some(vec![8, 9]));
    let pending = current.pending_pre_key.as_ref().unwrap();
    assert_eq!(pending.pre_key_id, Some(73));
    assert_eq!(pending.signed_pre_key_id, Some(-1));
    assert_eq!(pending.kyber_pre_key_id, Some(8));
    assert_eq!(pending.kyber_ciphertext, Some(vec![7; 33]));
    let reloaded = engine::codec::decode_session_record(
        &RustCrypto,
        &engine::codec::encode_session_record(&decoded),
    )
    .unwrap();
    assert!(decoded == reloaded);
}

#[test]
fn historical_counter_reservations_distinguish_live_reload_from_restart() {
    let all = vectors();
    let input = bytes(&all["pairwise"][0], "sender_before");
    let warm =
        engine::codec::decode_session_record_live(&RustCrypto, &input, &incarnation()).unwrap();
    assert_eq!(
        warm.current
            .as_ref()
            .unwrap()
            .sender
            .as_ref()
            .unwrap()
            .index,
        0
    );
    let cold = engine::codec::decode_session_record(&RustCrypto, &input).unwrap();
    let unknown =
        engine::codec::decode_session_record_live(&RustCrypto, &input, &[0x91; 16]).unwrap();
    assert_eq!(
        cold.current
            .as_ref()
            .unwrap()
            .sender
            .as_ref()
            .unwrap()
            .index,
        64
    );
    assert_eq!(
        unknown
            .current
            .as_ref()
            .unwrap()
            .sender
            .as_ref()
            .unwrap()
            .index,
        64
    );
    fn append_varint(out: &mut Vec<u8>, mut value: u64) {
        while value >= 128 {
            out.push((value as u8 & 127) | 128);
            value >>= 7;
        }
        out.push(value as u8);
    }
    let mut duplicate = input;
    append_varint(&mut duplicate, 100 << 3);
    append_varint(&mut duplicate, 64);
    assert!(
        engine::codec::decode_session_record_live(&RustCrypto, &duplicate, &incarnation()).is_err()
    );
    let mut without_reservation = warm;
    without_reservation.reservation = engine::Reservation::default();
    let mut overflow = engine::codec::encode_session_record(&without_reservation);
    append_varint(&mut overflow, 100 << 3);
    append_varint(&mut overflow, u64::from(u32::MAX) + 1);
    assert!(engine::codec::decode_session_record(&RustCrypto, &overflow).is_err());
}

#[test]
fn closed_receiver_chains_keep_large_historical_seed_backlogs() {
    let all = vectors();
    let mut record = engine::codec::decode_session_record_live(
        &RustCrypto,
        &bytes(&all["pairwise"][0], "sender_before"),
        &incarnation(),
    )
    .unwrap();
    let chain = &mut record.current.as_mut().unwrap().receivers[0];
    chain.index = 3000;
    chain.exhausted = true;
    chain.key = [0; 32];
    chain.message_key_limit = Some(4000);
    chain.skipped = (0..3000)
        .map(|index| engine::MessageKey::Seed {
            index,
            seed: [(index % 251) as u8; 32],
        })
        .collect();
    let encoded = engine::codec::encode_session_record(&record);
    let runtime =
        wacore_libsignal::protocol::SessionRecord::deserialize_for_store(&encoded, &incarnation())
            .unwrap();
    let mut persisted = Vec::new();
    runtime.serialize_into_for_store(&mut persisted, &incarnation());
    let loaded =
        engine::codec::decode_session_record_live(&RustCrypto, &persisted, &incarnation()).unwrap();
    let chain = &loaded.current.as_ref().unwrap().receivers[0];
    assert!(chain.exhausted);
    assert_eq!(chain.index, 3000);
    assert_eq!(chain.message_key_limit, Some(4000));
    assert_eq!(chain.skipped.len(), 3000);
    assert!(
        matches!(chain.skipped[2999],engine::MessageKey::Seed{index:2999,seed} if seed==[(2999%251)as u8;32])
    );
}

#[test]
#[allow(clippy::disallowed_methods)]
fn historical_expanded_keys_take_precedence_over_seed_fields() {
    use buffa::Message as _;
    let all = vectors();
    let mut raw = waproto::whatsapp::RecordStructure::decode_from_slice(&bytes(
        &all["pairwise"][0],
        "sender_before",
    ))
    .unwrap();
    let mut key = waproto::whatsapp::session_structure::chain::MessageKey::default();
    key.index = Some(9);
    key.cipher_key = Some(bytes::Bytes::from_static(&[1; 32]));
    key.mac_key = Some(bytes::Bytes::from_static(&[2; 32]));
    key.iv = Some(bytes::Bytes::from_static(&[3; 16]));
    key.seed = Some(bytes::Bytes::from_static(&[4; 32]));
    raw.current_session.as_option_mut().unwrap().receiver_chains[0]
        .message_keys
        .push(key);
    let decoded = engine::codec::decode_session_record(&RustCrypto, &raw.encode_to_vec()).unwrap();
    let expected = engine::MessageKey::Derived {
        index: 9,
        material: Box::new(engine::DerivedMessageKey {
            cipher: [1; 32],
            mac: [2; 32],
            iv: [3; 16],
        }),
    };
    assert!(decoded.current.as_ref().unwrap().receivers[0].skipped[0] == expected);
    let stored = engine::codec::encode_session_record(&decoded);
    let components = wacore_libsignal::protocol::SessionRecord::deserialize(&stored)
        .unwrap()
        .into_components()
        .unwrap();
    assert!(
        components.current_session.unwrap().receiver_chains[0].message_keys[0].material
            == wacore_libsignal::protocol::SessionMessageKeyMaterial::Derived {
                cipher_key: [1; 32],
                mac_key: [2; 32],
                iv: [3; 16],
            }
    );
}

#[test]
fn stored_history_keeps_its_budget_separate_from_network_envelopes() {
    let all = vectors();
    let mut record =
        engine::codec::decode_session_record(&RustCrypto, &bytes(&all, "pending_archive")).unwrap();
    std::sync::Arc::make_mut(
        record
            .current
            .as_mut()
            .unwrap()
            .pending_pre_key
            .as_mut()
            .unwrap(),
    )
    .kyber_ciphertext = Some(vec![6; 16 * 1024 * 1024]);
    let input = engine::codec::encode_session_record(&record);
    assert!(input.len() > 16 * 1024 * 1024);
    let decoded = engine::codec::decode_session_record(&RustCrypto, &input).unwrap();
    assert_eq!(
        decoded
            .current
            .as_ref()
            .unwrap()
            .pending_pre_key
            .as_ref()
            .unwrap()
            .kyber_ciphertext
            .as_ref()
            .unwrap()
            .len(),
        16 * 1024 * 1024
    );
    let wire = engine::wire::SignalMessage {
        ratchet: [1; 32],
        counter: 0,
        previous_counter: 0,
        ciphertext: vec![7; 16 * 1024 * 1024],
        mac: [0; 8],
    }
    .encode();
    assert!(engine::wire::SignalMessageView::decode(&wire).is_err());
}
