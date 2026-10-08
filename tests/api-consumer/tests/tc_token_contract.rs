//! Compiles without SDK/runtime features or workspace dev-dependency unification.
use wacore::store::{InMemoryBackend, traits::ProtocolStore};

#[test]
fn downstream_consumer_preserves_token_fields() {
    futures::executor::block_on(async {
        let store = InMemoryBackend::new();
        let jid = "100001@lid";
        store
            .touch_tc_token_sender_timestamp(jid, 9000)
            .await
            .unwrap();
        store
            .store_received_tc_token(jid, b"received", 3000)
            .await
            .unwrap();
        store
            .store_received_tc_token(jid, b"stale", 1000)
            .await
            .unwrap();
        store
            .touch_tc_token_sender_timestamp(jid, 1000)
            .await
            .unwrap();
        let row = store.get_tc_token(jid).await.unwrap().unwrap();
        assert_eq!(row.token, b"received");
        assert_eq!(row.token_timestamp, 3000);
        assert_eq!(row.sender_timestamp, Some(9000));
    });
}
