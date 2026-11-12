// SPDX-License-Identifier: MIT
// Synthetic persisted receiver records.
use async_trait::async_trait;
use futures::executor::block_on;
use rand::{SeedableRng, rngs::StdRng};
use wacore_libsignal::protocol::*;
use waproto::protobuf::Message as _;
use waproto::whatsapp::SenderKeyRecordStructure;
type Result<T> = std::result::Result<T, SignalProtocolError>;
#[derive(Default)]
struct Store(Option<SenderKeyRecord>);
#[async_trait]
impl SenderKeyStore for Store {
    async fn load_sender_key(&self, _: &SenderKeyName) -> Result<Option<SenderKeyRecord>> {
        Ok(self.0.clone())
    }
    async fn store_sender_key(&mut self, _: &SenderKeyName, record: SenderKeyRecord) -> Result<()> {
        self.0 = Some(record);
        Ok(())
    }
}
#[test]
#[allow(clippy::disallowed_methods)]
fn stored_group_receiver_accepts_unused_private_lengths() {
    block_on(async {
        let name = SenderKeyName::new("contract.invalid@g.us".into(), "sender.invalid:1".into());
        let mut random = StdRng::seed_from_u64(0x1828_6677);
        let mut sender = Store::default();
        let distribution = create_sender_key_distribution_message(&name, &mut sender, &mut random)
            .await
            .unwrap();
        let mut receiver = Store::default();
        process_sender_key_distribution_message(&name, &distribution, &mut receiver)
            .await
            .unwrap();
        let bytes = receiver.0.as_ref().unwrap().serialize().unwrap();
        let packet = group_encrypt(
            &mut sender,
            &name,
            b"synthetic delayed group message",
            &mut random,
        )
        .await
        .unwrap()
        .serialized()
        .to_vec();
        let delayed = group_encrypt(&mut sender, &name, b"delayed after rotation", &mut random)
            .await
            .unwrap()
            .serialized()
            .to_vec();
        for length in [0, 1, 31, 32, 33, 64] {
            let mut proto = SenderKeyRecordStructure::decode_from_slice(&bytes).unwrap();
            proto.sender_key_states[0]
                .sender_signing_key
                .as_option_mut()
                .unwrap()
                .private = Some(vec![99; length].into());
            let record = SenderKeyRecord::deserialize(&proto.encode_to_vec());
            let record = record.unwrap_or_else(|error| {
                panic!("unused private length {length} must remain receivable: {error}")
            });
            let components = record.clone().into_components().unwrap();
            assert_eq!(
                components.states[0].signing_key.private,
                (length == 32).then(|| vec![99; 32])
            );
            // Canonical storage remains readable across a reload before use.
            let record = SenderKeyRecord::deserialize(&record.serialize().unwrap()).unwrap();
            if length != 32 {
                let mut cannot_sign = Store(Some(record.clone()));
                assert!(
                    group_encrypt(&mut cannot_sign, &name, b"must not sign", &mut random)
                        .await
                        .is_err()
                );
            }
            let mut receiver = Store(Some(record));
            assert_eq!(
                group_decrypt(&packet, &mut receiver, &name).await.unwrap(),
                b"synthetic delayed group message",
                "length={length}"
            );
            let mut next_sender = Store::default();
            let next_distribution =
                create_sender_key_distribution_message(&name, &mut next_sender, &mut random)
                    .await
                    .unwrap();
            process_sender_key_distribution_message(&name, &next_distribution, &mut receiver)
                .await
                .unwrap();
            // A rotated current state must retain delayed messages in the older
            // receiver state, including after persistence and reload.
            receiver.0 = Some(
                SenderKeyRecord::deserialize(&receiver.0.as_ref().unwrap().serialize().unwrap())
                    .unwrap(),
            );
            assert_eq!(
                group_decrypt(&delayed, &mut receiver, &name).await.unwrap(),
                b"delayed after rotation",
                "length={length}"
            );
            let current = group_encrypt(
                &mut next_sender,
                &name,
                b"current after rotation",
                &mut random,
            )
            .await
            .unwrap()
            .serialized()
            .to_vec();
            assert_eq!(
                group_decrypt(&current, &mut receiver, &name).await.unwrap(),
                b"current after rotation",
                "length={length}"
            );
            assert!(group_decrypt(&delayed, &mut receiver, &name).await.is_err());
        }
    });
}
