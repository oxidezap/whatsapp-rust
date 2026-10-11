use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

struct PausedHook {
    calls: AtomicUsize,
    entered: tokio::sync::Notify,
    release: tokio::sync::Notify,
}
#[async_trait::async_trait]
impl crate::types::durability_hook::InboundDurabilityHook for PausedHook {
    async fn on_messages(&self, _: Arc<Client>, _: &[InboundMessage]) -> anyhow::Result<()> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 0 {
            self.entered.notify_one();
            self.release.notified().await;
        }
        Ok(())
    }
}

#[tokio::test]
async fn refused_snapshot_restore_cannot_reenter_an_owned_hook_commit() {
    let client = crate::test_utils::create_test_client_with_failing_http("restore_owner").await;
    client.inbound_commit_batch.reset();
    client.swap_message_semaphore(1);
    let hook = Arc::new(PausedHook {
        calls: AtomicUsize::new(0),
        entered: tokio::sync::Notify::new(),
        release: tokio::sync::Notify::new(),
    });
    client
        .inbound_durability_hook
        .set(hook.clone())
        .ok()
        .unwrap();
    let mut message = wa::Message::default();
    message.conversation = Some("retained".into());
    let item = InboundMessage::builder()
        .message(Arc::new(message))
        .info(Arc::new(MessageInfo {
            id: "restore-owned".into(),
            source: crate::types::message::MessageSource {
                chat: "100@g.us".parse().unwrap(),
                sender: "200@s.whatsapp.net".parse().unwrap(),
                ..Default::default()
            },
            ..Default::default()
        }))
        .build();
    let retention = &client.inbound_commit_batch.retention;
    retention.begin(&item.info, retention.admit(100)).await;
    retention.stage(std::slice::from_ref(&item), false).unwrap();
    let (old, _) = retention.seal(&item.info, true);
    let _old_permit = client.acquire_message_processing_permit().await;
    client.cleanup_connection_state().await;
    client.enter_live_mode_for_tests();
    retention.begin(&item.info, retention.admit(200)).await;
    let refused = ReinsertGuard {
        batcher: &client.inbound_commit_batch,
        items: Some(old.clone()),
        commit_ticket: None,
        retained: retention.commit(&old),
    };
    assert!(refused.retained.is_none());
    let (current, _) = retention.seal(&item.info, false);
    let commit = tokio::spawn({
        let client = client.clone();
        async move {
            client
                .commit_inbound_batch(current, BatchOrigin::Live, None)
                .await
        }
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), hook.entered.notified())
        .await
        .unwrap();
    drop(refused); // The old snapshot owns no identity and must not downgrade it.
    assert!(!client.inbound_commit_batch.has_entries());
    assert!(
        client
            .flush_inbound_commits_under_permit(false, None, None)
            .await,
        "an obsolete snapshot must leave an empty, successfully flushed batch"
    );
    assert_eq!(hook.calls.load(Ordering::SeqCst), 1);
    hook.release.notify_one();
    assert!(commit.await.unwrap());
    assert_eq!(retention.stats(), (0, 0));
    // A later producer must also survive dropping an older, refused snapshot.
    retention.begin(&item.info, retention.admit(300)).await;
    let _collection = retention.collection_guard(&item.info);
    retention.stage(std::slice::from_ref(&item), false).unwrap();
    assert!(retention.has_plaintext(&item.info));
    assert_eq!(retention.stats(), (1, 300));
}
