use wacore::store::error::StoreError;
use wacore::store::in_memory::InMemoryBackend;
use wacore::store::traits::{MessageSecret, MsgSecretEntry, MsgSecretStore, StoredMessageSecret};

#[tokio::test]
async fn named_read_unknown_known_merge_and_retention() {
    let backend = InMemoryBackend::new();
    backend
        .put_msg_secret("c", "s", "unknown", &[177; 32])
        .await
        .unwrap();
    let unknown = backend
        .get_stored_msg_secret("c", "s", "unknown")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(unknown.message_ts, None);
    assert_eq!(unknown.secret.as_bytes(), &[177; 32]);
    assert!(!format!("{unknown:#?}").contains("177"));
    for (message_ts, expires_at) in [(120, 250), (0, 150), (90, 190), (100, 200)] {
        backend
            .put_msg_secrets(vec![MsgSecretEntry {
                chat: "c".into(),
                sender: "s".into(),
                msg_id: "known".into(),
                secret: [177; 32],
                message_ts,
                expires_at,
            }])
            .await
            .unwrap();
    }
    let known = backend
        .get_stored_msg_secret("c", "s", "known")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(known.message_ts, Some(120));
    assert_eq!(backend.delete_expired_msg_secrets(249).await.unwrap(), 0);
    assert_eq!(backend.delete_expired_msg_secrets(250).await.unwrap(), 1);
    assert!(
        backend
            .get_stored_msg_secret("c", "s", "known")
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        backend
            .get_stored_msg_secret("c", "s", "unknown")
            .await
            .unwrap()
            .is_some()
    );
    assert_eq!(
        StoredMessageSecret::new(MessageSecret::from_bytes([177; 32]), Some(0)).message_ts,
        None
    );
}

#[test]
fn invalid_stored_data_preserves_typed_source_without_bytes() {
    for length in [0, 31, 33, 256] {
        let error = StoredMessageSecret::from_stored_bytes(&vec![177; length], 123).unwrap_err();
        assert!(matches!(error, StoreError::InvalidMessageSecret(_)));
        let source = std::error::Error::source(&error)
            .unwrap()
            .downcast_ref::<wacore::types::message_secret::InvalidMessageSecret>()
            .unwrap();
        assert_eq!(source.actual, length);
        assert!(!format!("{error:#?}").contains("177"));
    }
}
