#![cfg(not(target_family = "wasm"))]
#[path = "../../../wacore/tests/support/tc_token_contract.rs"]
mod contract;
#[path = "support/storage_fixture.rs"]
mod fixture;
use wacore::store::traits::ProtocolStore;
use whatsapp_rust_sqlite_storage::SqliteDatabase;

#[tokio::test]
async fn sqlite_preserves_token_fields_and_device_isolation() {
    let fixture = fixture::Fixture::new();
    let db = SqliteDatabase::open(&fixture.url(), Default::default())
        .await
        .unwrap();
    let a = db.provision_device(1).await.unwrap();
    let b = db.create_device().await.unwrap();
    b.store_received_tc_token("100001@lid", b"other-device", 10)
        .await
        .unwrap();
    b.touch_tc_token_sender_timestamp("100001@lid", 20)
        .await
        .unwrap();
    contract::assert_tc_token_contract(&a).await;
    let other = b.get_tc_token("100001@lid").await.unwrap().unwrap();
    assert_eq!(other.token, b"other-device");
    assert_eq!(other.token_timestamp, 10);
    assert_eq!(other.sender_timestamp, Some(20));
    assert!(b.get_tc_token("race-0@lid").await.unwrap().is_none());
}
