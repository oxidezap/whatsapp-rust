//! Private state controls complement the real public factory tests.
use super::*;
use crate::ConnectAdmission;

struct Wait;
impl ConnectAdmission for Wait {
    fn delay(&self) -> Duration {
        Duration::from_secs(900)
    }
}

async fn client() -> Arc<Client> {
    Client::builder()
        .with_runtime(crate::runtime_impl::TokioRuntime)
        .with_persistence_manager(Arc::new(
            PersistenceManager::new(Arc::new(wacore::store::in_memory::InMemoryBackend::new()))
                .await
                .unwrap(),
        ))
        .with_http_client(crate::test_utils::MockHttpClient)
        .with_transport_factory(crate::transport::mock::MockTransportFactory::new())
        .with_connect_admission(Wait)
        .build()
        .await
        .unwrap()
        .into_client()
}

#[tokio::test]
async fn admission_stop_preserves_stopped_reason() {
    let client = client().await;
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });
    crate::test_utils::wait_for_notifier_listeners(&client.session_state_notifier, 1).await;
    client.stop_supervision_loop();
    assert!(matches!(
        tokio::time::timeout(Duration::from_secs(1), run)
            .await
            .unwrap()
            .unwrap(),
        RunCompletionReason::Stopped
    ));
    assert_eq!(client.stats().reconnects, 0);
    assert_eq!(client.stats().reconnect_errors, 0);
    client.shutdown().await;
}

#[tokio::test]
async fn admission_ignores_unrelated_wakes_and_preserves_generation_penalty() {
    let client = client().await;
    // A rate-limit penalty and old generation must not be erased by waiting.
    client.auto_reconnect_errors.store(5, Ordering::Relaxed);
    client
        .backoff_reset_suppressed
        .store(true, Ordering::Relaxed);
    let generation = client.connection_generation.load(Ordering::SeqCst);
    let runner = client.clone();
    let run = tokio::spawn(async move { runner.run().await });
    crate::test_utils::wait_for_notifier_listeners(&client.session_state_notifier, 1).await;
    for _ in 0..3 {
        client.notify_connection_shutdown();
        client.notify_session_state();
        // Wait until the prior listener was consumed and a fresh one registered.
        tokio::task::yield_now().await;
        crate::test_utils::wait_for_notifier_listeners(&client.session_state_notifier, 1).await;
        assert!(!client.is_connecting.load(Ordering::Relaxed));
        assert_eq!(client.stats().reconnects, 0);
        assert_eq!(client.stats().reconnect_errors, 5);
        assert!(client.backoff_reset_suppressed.load(Ordering::Relaxed));
        assert_eq!(
            client.connection_generation.load(Ordering::SeqCst),
            generation
        );
    }
    client.signal_shutdown_sync();
    assert!(matches!(
        run.await.unwrap(),
        RunCompletionReason::ShutdownRequested
    ));
    // Repeated terminal cleanup still retires generations; no admission can
    // publish a socket or revive readiness during/after either invocation.
    for expected in [generation + 1, generation + 2] {
        let report = client.shutdown().await;
        assert!(report.device.is_ok());
        assert_eq!(
            client.connection_generation.load(Ordering::SeqCst),
            expected
        );
        client.resume();
        assert!(matches!(
            client.run().await,
            RunCompletionReason::ShutdownRequested
        ));
        assert!(!client.is_socket_ready());
        assert!(!client.is_session_ready());
    }
}
