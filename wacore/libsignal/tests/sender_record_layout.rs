// SPDX-License-Identifier: MIT
use async_trait::async_trait;
use futures::executor::block_on;
use rand::{SeedableRng, rngs::StdRng};
use wacore_libsignal::protocol::*;
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
#[cfg(target_pointer_width = "64")]
fn sender_record_stays_within_64_byte_inline_budget() {
    assert!(
        size_of::<SenderKeyRecord>() <= 64,
        "SenderKeyRecord is {} bytes",
        size_of::<SenderKeyRecord>()
    );
}
#[test]
fn group_publication_gate_survives_rotation_clone_and_resets() {
    block_on(async {
        let name = SenderKeyName::new("layout.invalid@g.us".into(), "sender.invalid:1".into());
        let mut random = StdRng::seed_from_u64(0x1828_6678);
        let mut store = Store::default();
        create_sender_key_distribution_message(&name, &mut store, &mut random)
            .await
            .unwrap();
        assert!(store.0.as_ref().unwrap().is_wire_gated());
        let mut fresh = Store::default();
        let incoming = create_sender_key_distribution_message(&name, &mut fresh, &mut random)
            .await
            .unwrap();
        process_sender_key_distribution_message(&name, &incoming, &mut store)
            .await
            .unwrap();
        let record = store.0.as_mut().unwrap();
        assert!(
            record.is_wire_gated(),
            "receiving a distribution must not forget pending publication"
        );
        assert!(record.clone().is_wire_gated());
        let before = record.serialize().unwrap();
        record.clear_wire_gated();
        assert_eq!(record.serialize().unwrap(), before);
        record.mark_wire_gated();
        record.waive_counter_lease().unwrap();
        assert!(!record.is_wire_gated());
        let mut loaded = SenderKeyRecord::deserialize(&record.serialize().unwrap()).unwrap();
        assert!(!loaded.is_wire_gated());
        loaded.reserve_iterations(0);
        assert!(loaded.is_wire_gated());
    });
}
