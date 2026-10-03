//! Public run-loop admission controls. Real factory calls, no network or SQLite.
use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use whatsapp_rust::anyhow::Result;
use whatsapp_rust::http::{HttpClient, HttpRequest, HttpResponse};
use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::transport::{Transport, TransportEvent, TransportFactory};
use whatsapp_rust::wacore::store::in_memory::InMemoryBackend;
use whatsapp_rust::{Client, ConnectAdmission, ConnectError, RunCompletionReason, TokioRuntime};

struct OfflineHttp;
#[whatsapp_rust::async_trait]
impl HttpClient for OfflineHttp {
    async fn execute(&self, _: HttpRequest) -> Result<HttpResponse> {
        panic!("version override must keep these tests offline")
    }
}

#[derive(Debug, thiserror::Error)]
#[error("synthetic factory failure {0}")]
struct FactoryFailure(usize);

type Order = Arc<Mutex<Vec<&'static str>>>;
struct Policy {
    calls: Arc<AtomicUsize>,
    delays: Mutex<VecDeque<Duration>>,
    fallback: Duration,
    order: Order,
    entered: async_channel::Sender<()>,
}
impl ConnectAdmission for Policy {
    fn delay(&self) -> Duration {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.order.lock().unwrap().push("admission");
        self.entered.try_send(()).unwrap();
        self.delays
            .lock()
            .unwrap()
            .pop_front()
            .unwrap_or(self.fallback)
    }
}
struct Factory {
    calls: Arc<AtomicUsize>,
    order: Order,
    entered: async_channel::Sender<()>,
    release: async_channel::Receiver<()>,
}
#[whatsapp_rust::async_trait]
impl TransportFactory for Factory {
    async fn create_transport(
        &self,
    ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        self.order.lock().unwrap().push("factory");
        self.entered.send(()).await.unwrap();
        let _ = self.release.recv().await;
        Err(FactoryFailure(call).into())
    }
}
struct Fixture {
    client: Arc<Client>,
    decisions: Arc<AtomicUsize>,
    dials: Arc<AtomicUsize>,
    order: Order,
    admitted: async_channel::Receiver<()>,
    entered: async_channel::Receiver<()>,
    release: async_channel::Sender<()>,
}
impl Fixture {
    async fn new(delays: Option<Vec<Duration>>, fallback: Duration) -> Self {
        let decisions = Arc::new(AtomicUsize::new(0));
        let dials = Arc::new(AtomicUsize::new(0));
        let order = Arc::new(Mutex::new(Vec::new()));
        let (admit_tx, admitted) = async_channel::bounded(16);
        let (enter_tx, entered) = async_channel::bounded(16);
        let (release, release_rx) = async_channel::bounded(16);
        let pm = Arc::new(
            PersistenceManager::new(Arc::new(InMemoryBackend::new()))
                .await
                .unwrap(),
        );
        let mut builder = Client::builder()
            .with_runtime(TokioRuntime)
            .with_persistence_manager(pm)
            .with_http_client(OfflineHttp)
            .with_version_override((2, 3000, 1))
            .with_transport_factory(Factory {
                calls: dials.clone(),
                order: order.clone(),
                entered: enter_tx,
                release: release_rx,
            });
        if let Some(delays) = delays {
            // The Arc setter also proves a shared trait object works publicly.
            builder = builder.with_connect_admission_arc(Arc::new(Policy {
                calls: decisions.clone(),
                delays: Mutex::new(delays.into()),
                fallback,
                order: order.clone(),
                entered: admit_tx,
            }));
        }
        let client = builder.build().await.unwrap().into_client();
        Self {
            client,
            decisions,
            dials,
            order,
            admitted,
            entered,
            release,
        }
    }
    fn run(&self) -> tokio::task::JoinHandle<RunCompletionReason> {
        let client = self.client.clone();
        tokio::spawn(async move { client.run().await })
    }
    async fn stop(&self, run: tokio::task::JoinHandle<RunCompletionReason>) {
        self.client.signal_shutdown_sync();
        // Release only if a real factory call was already in flight.
        let _ = self.release.try_send(());
        scheduled().await;
        assert!(
            run.is_finished(),
            "shutdown must not wait for admission's timer"
        );
        assert!(matches!(
            run.await.unwrap(),
            RunCompletionReason::ShutdownRequested
        ));
    }
    fn no_attempt(&self) {
        assert_eq!(self.dials.load(Ordering::SeqCst), 0);
        assert_eq!(self.client.stats().reconnects, 0);
        assert_eq!(self.client.stats().reconnect_errors, 0);
        assert!(!self.client.is_socket_ready());
        assert!(!self.client.is_session_ready());
    }
}
async fn scheduled() {
    // Keep the test runnable: paused Tokio time must not auto-advance the delay
    // merely because the observer is waiting for a factory call that is forbidden.
    for _ in 0..10 {
        tokio::task::yield_now().await;
    }
}

#[tokio::test(start_paused = true)]
async fn connect_admission_first_and_forced_reconnect_order() {
    let f = Fixture::new(Some(vec![Duration::ZERO; 3]), Duration::from_secs(900)).await;
    let run = f.run();
    for dial in 1..=3 {
        f.admitted.recv().await.unwrap();
        f.entered.recv().await.unwrap();
        assert_eq!(f.decisions.load(Ordering::SeqCst), dial);
        assert_eq!(f.dials.load(Ordering::SeqCst), dial);
        assert_eq!(f.client.stats().reconnects, (dial - 1) as u64);
        assert_eq!(f.client.stats().reconnect_errors, 0);
        assert_eq!(
            *f.order.lock().unwrap(),
            ["admission", "factory"].repeat(dial)
        );
        if dial < 3 {
            // Exercise the public forced reconnect transition while the real
            // synthetic factory is held; no private run-loop branch is stubbed.
            f.client.reconnect_immediately().await;
            f.release.send(()).await.unwrap();
        }
    }
    f.stop(run).await;
}

#[tokio::test(start_paused = true)]
async fn connect_admission_wait_is_outside_transport_timeout() {
    let f = Fixture::new(Some(vec![Duration::from_secs(60)]), Duration::ZERO).await;
    f.client.set_auto_reconnect(false);
    let run = f.run();
    f.admitted.recv().await.unwrap();
    tokio::time::advance(Duration::from_secs(59)).await;
    scheduled().await;
    f.no_attempt();
    assert!(!run.is_finished());
    tokio::time::advance(Duration::from_secs(1)).await;
    f.entered.recv().await.unwrap();
    tokio::time::advance(Duration::from_secs(19)).await;
    scheduled().await;
    assert!(
        !run.is_finished(),
        "factory has its own full 20-second budget"
    );
    f.release.send(()).await.unwrap();
    match run.await.unwrap() {
        RunCompletionReason::AutoReconnectDisabled {
            connect_error: Some(ConnectError::Transport(error)),
            ..
        } => assert_eq!(error.downcast_ref::<FactoryFailure>().unwrap().0, 1),
        other => panic!("admission must not become a timeout: {other:?}"),
    }
    assert_eq!(f.decisions.load(Ordering::SeqCst), 1);
    f.client.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn connect_admission_does_not_extend_the_factory_timeout() {
    let f = Fixture::new(Some(vec![Duration::from_secs(60)]), Duration::ZERO).await;
    f.client.set_auto_reconnect(false);
    let run = f.run();
    f.admitted.recv().await.unwrap();
    tokio::time::advance(Duration::from_secs(60)).await;
    f.entered.recv().await.unwrap();
    tokio::time::advance(Duration::from_secs(20)).await;
    assert!(
        matches!(run.await.unwrap(), RunCompletionReason::AutoReconnectDisabled {
        connect_error: Some(ConnectError::Timeout { stage: whatsapp_rust::ConnectStage::Transport, timeout }), ..
    } if timeout == Duration::from_secs(20))
    );
    f.client.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn connect_admission_unset_and_zero_have_same_failure_backoff() {
    for delays in [None, Some(vec![Duration::ZERO])] {
        let has_policy = delays.is_some();
        let f = Fixture::new(delays, Duration::ZERO).await;
        let run = f.run();
        f.entered.recv().await.unwrap();
        assert_eq!(f.decisions.load(Ordering::SeqCst), usize::from(has_policy));
        f.release.send(()).await.unwrap();
        scheduled().await;
        assert_eq!(f.client.stats().reconnect_errors, 1);
        assert_eq!(f.client.stats().reconnects, 0);
        tokio::time::advance(Duration::from_millis(899)).await;
        scheduled().await;
        assert_eq!(
            f.dials.load(Ordering::SeqCst),
            1,
            "WA jittered first delay not bypassed"
        );
        tokio::time::advance(Duration::from_millis(202)).await;
        f.entered.recv().await.unwrap();
        assert_eq!(
            f.decisions.load(Ordering::SeqCst),
            2 * usize::from(has_policy)
        );
        assert_eq!(f.client.stats().reconnects, 1);
        f.stop(run).await;
    }
}

#[tokio::test(start_paused = true)]
async fn connect_admission_shutdown_abandons_wait_without_attempt() {
    for full_shutdown in [false, true] {
        let f = Fixture::new(Some(vec![]), Duration::from_secs(900)).await;
        let run = f.run();
        f.admitted.recv().await.unwrap();
        scheduled().await;
        assert!(matches!(
            f.client.run().await,
            RunCompletionReason::AlreadyRunning
        ));
        if full_shutdown {
            let report = f.client.shutdown().await;
            assert!(report.device.is_ok());
            assert_eq!(report.inbound, whatsapp_rust::DrainOutcome::Completed);
        } else {
            f.client.signal_shutdown_sync();
        }
        scheduled().await;
        assert!(
            run.is_finished(),
            "shutdown must cancel the 900-second wait now"
        );
        assert!(matches!(
            run.await.unwrap(),
            RunCompletionReason::ShutdownRequested
        ));
        f.no_attempt();
        f.client.resume();
        assert!(matches!(
            f.client.run().await,
            RunCompletionReason::ShutdownRequested
        ));
        assert_eq!(f.decisions.load(Ordering::SeqCst), 1);
    }
}

#[tokio::test(start_paused = true)]
async fn connect_admission_pause_resume_reserves_again_without_phantom_failure() {
    for rapid in [false, true] {
        let f = Fixture::new(
            Some(vec![Duration::from_secs(900), Duration::from_secs(60)]),
            Duration::ZERO,
        )
        .await;
        let run = f.run();
        f.admitted.recv().await.unwrap();
        f.client.pause().await;
        if !rapid {
            scheduled().await;
            tokio::time::advance(Duration::from_secs(1000)).await;
            scheduled().await;
            f.no_attempt();
            assert_eq!(f.decisions.load(Ordering::SeqCst), 1);
        }
        f.client.resume();
        scheduled().await;
        assert_eq!(
            f.decisions.load(Ordering::SeqCst),
            2,
            "resume must discard the old wait immediately"
        );
        f.admitted.recv().await.unwrap();
        f.no_attempt();
        tokio::time::advance(Duration::from_secs(59)).await;
        scheduled().await;
        f.no_attempt();
        tokio::time::advance(Duration::from_secs(1)).await;
        f.entered.recv().await.unwrap();
        assert_eq!(
            f.client.stats().reconnects,
            0,
            "first abandoned reservation wasn't a first dial"
        );
        f.release.send(()).await.unwrap();
        scheduled().await;
        assert_eq!(
            f.client.stats().reconnect_errors,
            1,
            "old pause cannot waive a real failure's backoff"
        );
        f.stop(run).await;
    }
}

#[tokio::test(start_paused = true)]
async fn connect_admission_reconnect_wait_does_not_count_as_a_dial() {
    let f = Fixture::new(Some(vec![Duration::ZERO]), Duration::from_secs(900)).await;
    let run = f.run();
    f.entered.recv().await.unwrap();
    f.client.reconnect_immediately().await;
    f.release.send(()).await.unwrap();
    // Consume both first and second consultation observations.
    f.admitted.recv().await.unwrap();
    f.admitted.recv().await.unwrap();
    assert_eq!(f.dials.load(Ordering::SeqCst), 1);
    assert_eq!(f.client.stats().reconnects, 0);
    assert_eq!(f.client.stats().reconnect_errors, 0);
    f.stop(run).await;
    assert_eq!(f.dials.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn connect_admission_manual_connect_is_not_consulted() {
    let f = Fixture::new(Some(vec![]), Duration::from_secs(900)).await;
    let client = f.client.clone();
    let connect = tokio::spawn(async move { client.connect().await.map(|_| ()) });
    f.entered.recv().await.unwrap();
    f.release.send(()).await.unwrap();
    assert!(matches!(
        connect.await.unwrap(),
        Err(ConnectError::Transport(_))
    ));
    assert_eq!(f.decisions.load(Ordering::SeqCst), 0);
    f.client.shutdown().await;
}

#[tokio::test(start_paused = true)]
async fn connect_admission_bot_builder_concrete_policy_is_consulted() {
    let calls = Arc::new(AtomicUsize::new(0));
    let order = Arc::new(Mutex::new(Vec::new()));
    let (entered, admitted) = async_channel::bounded(1);
    let (factory_entered, factory_calls) = async_channel::bounded(1);
    let (_release, release_rx) = async_channel::bounded(1);
    let bot = whatsapp_rust::bot::Bot::builder()
        .with_backend(InMemoryBackend::new())
        .with_runtime(TokioRuntime)
        .with_http_client(OfflineHttp)
        .with_version((2, 3000, 1))
        .with_transport_factory(Factory {
            calls: Arc::new(AtomicUsize::new(0)),
            order: order.clone(),
            entered: factory_entered,
            release: release_rx,
        })
        .with_connect_admission(Policy {
            calls: calls.clone(),
            delays: Mutex::new(VecDeque::new()),
            fallback: Duration::from_secs(900),
            order,
            entered,
        })
        .build()
        .await
        .unwrap();
    let client = bot.client();
    let run = tokio::spawn(async move { bot.run().await });
    admitted.recv().await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    client.signal_shutdown_sync();
    scheduled().await;
    assert!(run.is_finished());
    assert!(matches!(
        run.await.unwrap(),
        RunCompletionReason::ShutdownRequested
    ));
    assert!(factory_calls.is_empty());
}

// This boxes the actual new run graph the way a downstream async_trait host
// does, not merely the policy object. No async change to host dependencies.
#[whatsapp_rust::async_trait]
trait Driver {
    async fn drive(&self) -> RunCompletionReason;
}
#[whatsapp_rust::async_trait]
impl Driver for Arc<Client> {
    async fn drive(&self) -> RunCompletionReason {
        self.run().await
    }
}
#[tokio::test]
async fn connect_admission_boxed_async_trait_consumer() {
    let f = Fixture::new(Some(vec![]), Duration::ZERO).await;
    f.client.signal_shutdown_sync();
    let driver: Box<dyn Driver> = Box::new(f.client.clone());
    assert!(matches!(
        driver.drive().await,
        RunCompletionReason::ShutdownRequested
    ));
}
