use wacore::store::traits::ProtocolStore;

/// The same public contract is exercised by both built-in stores and a downstream crate.
pub async fn assert_tc_token_contract(store: &impl ProtocolStore) {
    let jid = "100001@lid";
    store
        .touch_tc_token_sender_timestamp(jid, 9000)
        .await
        .unwrap();
    store
        .store_received_tc_token(jid, b"first", 3000)
        .await
        .unwrap();
    let row = store.get_tc_token(jid).await.unwrap().unwrap();
    assert_eq!(row.token, b"first", "placeholder cannot block real bytes");
    assert_eq!(row.token_timestamp, 3000);
    assert_eq!(row.sender_timestamp, Some(9000));
    store
        .store_received_tc_token(jid, b"new", 6000)
        .await
        .unwrap();
    store
        .store_received_tc_token(jid, b"old", 2000)
        .await
        .unwrap();
    store
        .touch_tc_token_sender_timestamp(jid, 1000)
        .await
        .unwrap();
    let row = store.get_tc_token(jid).await.unwrap().unwrap();
    assert_eq!(row.token, b"new");
    assert_eq!(row.token_timestamp, 6000);
    assert_eq!(row.sender_timestamp, Some(9000));
    store
        .store_received_tc_token(jid, b"equal", 6000)
        .await
        .unwrap();
    assert_eq!(
        store.get_tc_token(jid).await.unwrap().unwrap().token,
        b"equal"
    );

    for round in 0..16 {
        let jid = format!("race-{round}@lid");
        let (received, sender, stale, old_sender) = tokio::join!(
            store.store_received_tc_token(&jid, b"new", 6000),
            store.touch_tc_token_sender_timestamp(&jid, 9000),
            store.store_received_tc_token(&jid, b"old", 2000),
            store.touch_tc_token_sender_timestamp(&jid, 1000),
        );
        received.unwrap();
        sender.unwrap();
        stale.unwrap();
        old_sender.unwrap();
        let row = store.get_tc_token(&jid).await.unwrap().unwrap();
        assert_eq!(row.token, b"new");
        assert_eq!(row.token_timestamp, 6000);
        assert_eq!(row.sender_timestamp, Some(9000));
    }
}
