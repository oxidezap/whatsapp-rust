//! Public consumer coverage: portable host assembly, real stanza dispatch and observer lifetime.
use std::future::Future;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Weak};
use std::time::Duration;
use whatsapp_rust::ChatActivity;
use whatsapp_rust::bot::{Bot, BotRunOutcome};
use whatsapp_rust::handlers::chatstate::ChatstateHandler;
use whatsapp_rust::handlers::traits::StanzaHandler;
use whatsapp_rust::http::{HttpClient, HttpRequest, HttpResponse};
use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::transport::{Transport, TransportEvent, TransportFactory};
use whatsapp_rust::types::events::{
    ChannelEventHandler, Event, EventHandler, EventInterest, EventKind,
};
use whatsapp_rust::wacore::runtime::{AbortHandle, BoxFuture, Runtime};
use whatsapp_rust::wacore::store::in_memory::InMemoryBackend;
use whatsapp_rust::{
    CallbackEventHandler, Client, EventDelivery, NodeBuilder, RunCompletionReason, TokioRuntime,
    anyhow, async_channel, wacore_binary,
};

struct OfflineHttp;
#[whatsapp_rust::async_trait]
impl HttpClient for OfflineHttp {
    async fn execute(&self, _: HttpRequest) -> anyhow::Result<HttpResponse> {
        anyhow::bail!("offline observer HTTP")
    }
}
struct OfflineTransport;
#[whatsapp_rust::async_trait]
impl TransportFactory for OfflineTransport {
    async fn create_transport(
        &self,
    ) -> anyhow::Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        anyhow::bail!("offline observer transport")
    }
}
async fn client() -> Arc<Client> {
    let pm = Arc::new(
        PersistenceManager::new(Arc::new(InMemoryBackend::new()))
            .await
            .unwrap(),
    );
    let (client, receiver) = Client::builder()
        .with_persistence_manager(pm)
        .with_runtime(TokioRuntime)
        .with_http_client(OfflineHttp)
        .with_transport_factory(OfflineTransport)
        .with_version_override((2, 3000, 1))
        .build()
        .await
        .unwrap()
        .into_parts();
    assert_eq!(receiver.receiver_count(), 1);
    client
}
async fn dispatch(client: &Arc<Client>, state: &'static str, media: Option<&str>) {
    let mut child = NodeBuilder::new(state);
    if let Some(media) = media {
        child = child.attr("media", media);
    }
    let node = NodeBuilder::new("chatstate")
        .attr("from", "120363000001@g.us")
        .attr("participant", "12025550111@s.whatsapp.net")
        .children([child.build()])
        .build();
    let packed = wacore_binary::marshal::marshal(&node).unwrap();
    let bytes = wacore_binary::util::unpack(&packed).unwrap();
    let owned = Arc::new(whatsapp_rust::OwnedNodeRef::new(bytes.into_owned()).unwrap());
    assert!(
        ChatstateHandler
            .handle(client.clone(), owned, &mut false)
            .await
    );
}
async fn receive<T>(receiver: &async_channel::Receiver<T>) -> T {
    tokio::time::timeout(Duration::from_secs(5), receiver.recv())
        .await
        .expect("observer receive deadline")
        .expect("observer channel open")
}

async fn eventually(mut predicate: impl FnMut() -> bool) {
    tokio::time::timeout(Duration::from_secs(5), async {
        while !predicate() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn async_observer_is_filtered_and_subscription_is_raii() {
    let client = client().await;
    let (tx, rx) = async_channel::unbounded();
    let handler = CallbackEventHandler::from_callback(
        &client,
        EventInterest::of(&[EventKind::ChatPresence]),
        EventDelivery::Ordered { capacity: 4 },
        move |event, _| {
            let tx = tx.clone();
            async move {
                tx.send(event).await.unwrap();
            }
        },
    );
    let subscription = client.subscribe_handler(handler.clone());
    dispatch(&client, "composing", Some("audio")).await;
    let observed = receive(&rx).await;
    assert!(
        matches!(&*observed, Event::ChatPresence(update) if update.source.chat.to_string() == "120363000001@g.us" && update.source.sender.to_string() == "12025550111@s.whatsapp.net")
    );
    drop(subscription);
    dispatch(&client, "paused", None).await;
    assert_eq!(handler.stats().accepted, 1);
    assert!(rx.try_recv().is_err());
    handler.cancel();
    client.shutdown().await;
}

#[tokio::test]
async fn chatstate_compatibility_view_uses_the_same_bus_fact() {
    let client = client().await;
    let (tx, rx) = async_channel::unbounded();
    let subscription = client.subscribe_chatstate_handler(Arc::new(move |event| {
        tx.try_send(event).unwrap();
    }));
    let (bus, events) = ChannelEventHandler::with_capacity(8);
    let bus_subscription =
        client.subscribe(EventInterest::of(&[EventKind::ChatPresence]), bus.clone());
    for (state, media, expected) in [
        ("composing", None, ChatActivity::Typing),
        ("composing", Some("audio"), ChatActivity::RecordingAudio),
        ("paused", None, ChatActivity::Idle),
    ] {
        dispatch(&client, state, media).await;
        let observed = receive(&rx).await;
        assert_eq!(observed.chat.to_string(), "120363000001@g.us");
        assert_eq!(
            observed.participant.as_ref().unwrap().to_string(),
            "12025550111@s.whatsapp.net"
        );
        assert_eq!(observed.state, expected);
        let fact = receive(&events).await;
        let Event::ChatPresence(presence) = &*fact else {
            panic!("shared chat presence fact")
        };
        let projected = whatsapp_rust::ChatStateEvent::from_presence(presence);
        assert_eq!(observed.chat, projected.chat);
        assert_eq!(observed.participant, projected.participant);
        assert_eq!(observed.state, projected.state);
        assert!(rx.try_recv().is_err(), "one typed view per registration");
        assert!(events.try_recv().is_err(), "one bus fact per registration");
    }
    assert_eq!(bus.stats().enqueued, 3);
    drop(subscription);
    dispatch(&client, "composing", None).await;
    assert!(rx.try_recv().is_err());
    assert_eq!(bus.stats().enqueued, 4, "independent bus observer remains");
    drop(bus_subscription);
    dispatch(&client, "paused", None).await;
    assert_eq!(bus.stats().enqueued, 4);
    client.shutdown().await;
}

#[tokio::test]
async fn bounded_pool_drops_newest_without_stalling_protocol_handler() {
    let client = client().await;
    let (tx, rx) = async_channel::unbounded();
    let handler = CallbackEventHandler::from_callback(
        &client,
        EventInterest::of(&[EventKind::ChatPresence]),
        EventDelivery::BoundedConcurrent {
            capacity: 2,
            max_concurrency: 2,
        },
        move |_, client| {
            let tx = tx.clone();
            async move {
                tx.send(()).await.unwrap();
                std::future::pending::<()>().await;
                drop(client);
            }
        },
    );
    let _subscription = client.subscribe_handler(handler.clone());
    for _ in 0..2 {
        dispatch(&client, "composing", None).await;
        receive(&rx).await;
    }
    for _ in 0..12 {
        dispatch(&client, "composing", None).await;
    }
    assert_eq!(handler.stats().callbacks_active, 2);
    assert_eq!(handler.stats().dropped_full, 10);
    assert_eq!(client.stats().events_dropped, 10);
    client.shutdown().await;
    eventually(|| handler.stats().callbacks_active == 0 && handler.stats().discarded == 2).await;
    assert_eq!(handler.stats().callbacks_cancelled, 2);
}

#[tokio::test]
async fn ordered_delivery_preserves_accepted_order_and_drops_newest() {
    let client = client().await;
    let (started_tx, started) = async_channel::unbounded();
    let (release, permits) = async_channel::bounded::<()>(1);
    let handler = CallbackEventHandler::from_callback(
        &client,
        EventInterest::of(&[EventKind::ChatPresence]),
        EventDelivery::Ordered { capacity: 2 },
        move |event, _| {
            let started = started_tx.clone();
            let permits = permits.clone();
            async move {
                let Event::ChatPresence(presence) = &*event else {
                    panic!("chat presence")
                };
                started
                    .send(whatsapp_rust::ChatStateEvent::from_presence(presence).state)
                    .await
                    .unwrap();
                permits.recv().await.unwrap();
            }
        },
    );
    let subscription = client.subscribe_handler(handler.clone());
    dispatch(&client, "composing", None).await;
    assert_eq!(receive(&started).await, ChatActivity::Typing);
    dispatch(&client, "composing", Some("audio")).await;
    dispatch(&client, "paused", None).await;
    dispatch(&client, "composing", None).await; // newest is rejected
    assert_eq!(handler.stats().accepted, 3);
    assert_eq!(handler.stats().dropped_full, 1);
    assert_eq!(handler.stats().callbacks_active, 1);
    assert!(started.try_recv().is_err());
    for state in [ChatActivity::RecordingAudio, ChatActivity::Idle] {
        release.send(()).await.unwrap();
        assert_eq!(receive(&started).await, state);
        assert_eq!(handler.stats().callbacks_active, 1);
    }
    release.send(()).await.unwrap();
    eventually(|| handler.stats().callbacks_completed == 3).await;
    assert!(started.try_recv().is_err());
    assert_eq!(client.stats().events_dropped, 1);
    drop(subscription);
    dispatch(&client, "paused", None).await;
    assert_eq!(handler.stats().accepted, 3);
    handler.cancel();
    client.shutdown().await;
}

#[tokio::test]
async fn defaults_zero_and_reentrant_terminal_cancellation_are_public() {
    assert!(matches!(
        EventDelivery::default(),
        EventDelivery::BoundedConcurrent {
            capacity: 256,
            max_concurrency: 16
        }
    ));
    let (_, bounded) = ChannelEventHandler::new();
    assert_eq!(bounded.capacity(), Some(256));
    let (_, unlimited) = ChannelEventHandler::unbounded();
    assert_eq!(unlimited.capacity(), None);
    let (_, zero) = ChannelEventHandler::with_capacity(0);
    assert_eq!(zero.capacity(), Some(1));
    let client = client().await;
    let weak = Arc::downgrade(&client);
    let handler = CallbackEventHandler::from_callback(
        &client,
        EventInterest::ALL,
        EventDelivery::Ordered { capacity: 0 },
        |_, client| async move {
            client.shutdown().await;
        },
    );
    let subscription = client.subscribe_handler(handler.clone());
    dispatch(&client, "composing", None).await;
    eventually(|| client.shutdown_signal().is_fired() && handler.stats().callbacks_active == 0)
        .await;
    assert_eq!(handler.stats().callbacks_started, 1);
    drop(subscription);
    drop(client);
    eventually(|| weak.upgrade().is_none()).await;
    // Terminal intake remains closed even when the adapter is kept by the host.
    let connected = Event::Connected(whatsapp_rust::types::events::Connected::builder().build());
    handler.handle_event(Arc::new(connected));
    assert_eq!(handler.stats().closed, 1);
}

struct CallbackProbe {
    started: async_channel::Receiver<EventKind>,
    released: async_channel::Receiver<()>,
    captures: Weak<()>,
    runtime: Arc<TrackingRuntime>,
    task_drops: async_channel::Receiver<usize>,
}

struct TrackingRuntime {
    next_id: AtomicUsize,
    dropped: async_channel::Sender<usize>,
}
struct TaskLease {
    id: usize,
    dropped: async_channel::Sender<usize>,
}
impl Drop for TaskLease {
    fn drop(&mut self) {
        let _ = self.dropped.try_send(self.id);
    }
}
impl Runtime for TrackingRuntime {
    fn spawn(&self, future: BoxFuture<'static, ()>) -> AbortHandle {
        let lease = TaskLease {
            id: self.next_id.fetch_add(1, Ordering::Relaxed),
            dropped: self.dropped.clone(),
        };
        TokioRuntime.spawn(Box::pin(async move {
            let _lease = lease;
            future.await;
        }))
    }
    fn sleep(&self, duration: Duration) -> BoxFuture<'static, ()> {
        TokioRuntime.sleep(duration)
    }
    fn spawn_blocking(&self, task: Box<dyn FnOnce() + Send + 'static>) -> BoxFuture<'static, ()> {
        TokioRuntime.spawn_blocking(task)
    }
    fn yield_now(&self) -> Option<BoxFuture<'static, ()>> {
        TokioRuntime.yield_now()
    }
}

// These references must live in the suspended user future, not just its factory.
struct CallbackLease {
    _client: Arc<Client>,
    _capture: Arc<()>,
    released: async_channel::Sender<()>,
}
impl Drop for CallbackLease {
    fn drop(&mut self) {
        let _ = self.released.try_send(());
    }
}

async fn pending_bot(policy: EventDelivery) -> (Bot, CallbackProbe) {
    let (started_tx, started) = async_channel::unbounded();
    let (released_tx, released) = async_channel::unbounded();
    let capture = Arc::new(());
    let captures = Arc::downgrade(&capture);
    let (dropped, task_drops) = async_channel::unbounded();
    let runtime = Arc::new(TrackingRuntime {
        next_id: AtomicUsize::new(0),
        dropped,
    });
    let bot = Bot::builder()
        .with_backend(InMemoryBackend::new())
        .with_runtime_arc(runtime.clone())
        .with_http_client(OfflineHttp)
        .with_transport_factory(OfflineTransport)
        .with_version((2, 3000, 1))
        .with_event_delivery(policy)
        .on_event_for(&[EventKind::ChatPresence], move |event, client| {
            let started = started_tx.clone();
            let lease = CallbackLease {
                _client: client,
                _capture: capture.clone(),
                released: released_tx.clone(),
            };
            async move {
                started.try_send(event.kind()).unwrap();
                std::future::pending::<()>().await;
                drop(lease);
            }
        })
        .build()
        .await
        .unwrap();
    (
        bot,
        CallbackProbe {
            started,
            released,
            captures,
            runtime,
            task_drops,
        },
    )
}

async fn observe_driver_drop(probe: &CallbackProbe, driver_id: usize) {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if probe.task_drops.recv().await.unwrap() == driver_id {
                break;
            }
        }
    })
    .await
    .expect("native executor dropped the actual driver future");
}

async fn observe_driver_cancellation(client: &Arc<Client>, probe: &CallbackProbe) -> bool {
    let released = tokio::time::timeout(Duration::from_secs(5), probe.released.recv())
        .await
        .is_ok_and(|result| result.is_ok());
    assert!(
        !client.shutdown_signal().is_fired(),
        "terminal shutdown must not mask driver cancellation"
    );
    if !released {
        assert!(
            probe.captures.upgrade().is_some(),
            "pending callback capture remains live"
        );
        // Preserve the unmasked observation, then use the separately proven
        // terminal path only to clean up a failing regression's pending work.
        client.shutdown().await;
        receive(&probe.released).await;
    }
    released
}

async fn assert_callback_ownership_released(client: Arc<Client>, probe: CallbackProbe) {
    eventually(|| probe.captures.upgrade().is_none()).await;
    let weak = Arc::downgrade(&client);
    drop(client);
    eventually(|| weak.upgrade().is_none()).await;
}

fn callback_policies() -> [EventDelivery; 4] {
    [
        EventDelivery::Ordered { capacity: 1 },
        EventDelivery::BoundedConcurrent {
            capacity: 1,
            max_concurrency: 2,
        },
        EventDelivery::default(),
        EventDelivery::ConcurrentUnbounded,
    ]
}

async fn poll_paused_driver(driver: std::pin::Pin<&mut impl Future>) {
    let mut driver = driver;
    std::future::poll_fn(|cx| {
        assert!(driver.as_mut().poll(cx).is_pending());
        std::task::Poll::Ready(())
    })
    .await;
}

#[tokio::test]
async fn bot_handle_abort_cancels_pending_builder_callback_without_terminal_shutdown() {
    for policy in callback_policies() {
        let (bot, probe) = pending_bot(policy).await;
        let client = bot.client();
        client.pause().await;
        let handle = bot.spawn();
        let driver_id = probe.runtime.next_id.load(Ordering::Relaxed) - 1;
        dispatch(&client, "composing", None).await;
        assert_eq!(receive(&probe.started).await, EventKind::ChatPresence);
        handle.abort();
        observe_driver_drop(&probe, driver_id).await;
        // Observe while the public handle is still alive: Drop must not mask abort.
        let released = observe_driver_cancellation(&client, &probe).await;
        drop(handle);
        assert_callback_ownership_released(client, probe).await;
        assert!(released, "driver abort left callback pending: {policy:?}");
    }
}

#[tokio::test]
async fn bot_handle_drop_cancels_pending_builder_callback_without_terminal_shutdown() {
    for policy in callback_policies() {
        let (bot, probe) = pending_bot(policy).await;
        let client = bot.client();
        client.pause().await;
        let handle = bot.spawn();
        let driver_id = probe.runtime.next_id.load(Ordering::Relaxed) - 1;
        dispatch(&client, "composing", None).await;
        assert_eq!(receive(&probe.started).await, EventKind::ChatPresence);
        drop(handle);
        observe_driver_drop(&probe, driver_id).await;
        let released = observe_driver_cancellation(&client, &probe).await;
        assert_callback_ownership_released(client, probe).await;
        assert!(released, "handle Drop left callback pending: {policy:?}");
    }
}

#[tokio::test]
async fn foreground_driver_cancellation_releases_callback_without_terminal_shutdown() {
    for policy in callback_policies() {
        let (bot, probe) = pending_bot(policy).await;
        let client = bot.client();
        client.pause().await;
        let mut driver = Box::pin(bot.run());
        poll_paused_driver(driver.as_mut()).await;
        dispatch(&client, "composing", None).await;
        assert_eq!(receive(&probe.started).await, EventKind::ChatPresence);
        drop(driver);
        let released = observe_driver_cancellation(&client, &probe).await;
        assert_callback_ownership_released(client, probe).await;
        assert!(
            released,
            "foreground cancellation left callback pending: {policy:?}"
        );
    }
}

#[tokio::test]
async fn unpolled_foreground_driver_drops_callback_factory_without_shutdown() {
    let (bot, probe) = pending_bot(EventDelivery::Ordered { capacity: 1 }).await;
    let client = bot.client();
    let driver = Box::pin(bot.run());
    drop(driver);
    eventually(|| probe.captures.upgrade().is_none()).await;
    assert!(probe.started.try_recv().is_err());
    assert!(probe.released.try_recv().is_err());
    assert!(!client.shutdown_signal().is_fired());
    assert_callback_ownership_released(client, probe).await;
}

#[tokio::test]
async fn completed_background_driver_cancels_callbacks_while_handle_is_retained() {
    for policy in callback_policies() {
        let (bot, probe) = pending_bot(policy).await;
        let client = bot.client();
        client.pause().await;
        let mut handle = bot.spawn();
        dispatch(&client, "composing", None).await;
        assert_eq!(receive(&probe.started).await, EventKind::ChatPresence);
        client.set_auto_reconnect(false);
        client.resume();
        let verdict = tokio::time::timeout(Duration::from_secs(5), &mut handle)
            .await
            .unwrap();
        assert!(matches!(
            verdict,
            BotRunOutcome::Completed(RunCompletionReason::AutoReconnectDisabled {
                connect_error: Some(_),
                ..
            })
        ));
        let released = observe_driver_cancellation(&client, &probe).await;
        drop(handle);
        assert_callback_ownership_released(client, probe).await;
        assert!(
            released,
            "normal completion left callback pending: {policy:?}"
        );
    }
}

#[tokio::test]
async fn completed_foreground_driver_cancels_callbacks_while_future_is_retained() {
    for policy in callback_policies() {
        let (bot, probe) = pending_bot(policy).await;
        let client = bot.client();
        client.pause().await;
        let mut driver = Box::pin(bot.run());
        poll_paused_driver(driver.as_mut()).await;
        dispatch(&client, "composing", None).await;
        assert_eq!(receive(&probe.started).await, EventKind::ChatPresence);
        client.set_auto_reconnect(false);
        client.resume();
        let verdict = tokio::time::timeout(Duration::from_secs(5), driver.as_mut())
            .await
            .unwrap();
        assert!(matches!(
            verdict,
            RunCompletionReason::AutoReconnectDisabled {
                connect_error: Some(_),
                ..
            }
        ));
        let released = observe_driver_cancellation(&client, &probe).await;
        drop(driver);
        assert_callback_ownership_released(client, probe).await;
        assert!(
            released,
            "normal foreground completion left callback pending: {policy:?}"
        );
    }
}

#[tokio::test]
async fn builder_callbacks_deliver_normally_until_driver_scope_ends() {
    for policy in callback_policies() {
        let (tx, rx) = async_channel::unbounded();
        let bot = Bot::builder()
            .with_backend(InMemoryBackend::new())
            .with_runtime(TokioRuntime)
            .with_http_client(OfflineHttp)
            .with_transport_factory(OfflineTransport)
            .with_version((2, 3000, 1))
            .with_event_delivery(policy)
            .on_event_for(&[EventKind::ChatPresence], move |event, _| {
                let tx = tx.clone();
                async move {
                    tx.try_send(event).unwrap();
                }
            })
            .build()
            .await
            .unwrap();
        let client = bot.client();
        client.pause().await;
        let handle = bot.spawn();
        for state in ["composing", "paused", "composing"] {
            dispatch(&client, state, None).await;
            let observed = receive(&rx).await;
            assert!(matches!(&*observed, Event::ChatPresence(update)
                if update.source.chat.to_string() == "120363000001@g.us"
                    && update.source.sender.to_string() == "12025550111@s.whatsapp.net"));
        }
        assert_eq!(client.stats().events_dropped, 0);
        handle.abort();
        dispatch(&client, "paused", None).await;
        assert!(rx.try_recv().is_err());
        assert!(!client.shutdown_signal().is_fired());
        drop(handle);
        let weak = Arc::downgrade(&client);
        drop(client);
        eventually(|| weak.upgrade().is_none()).await;
    }
}

#[tokio::test]
async fn bot_driver_abort_preserves_independent_client_subscription() {
    let (bot, probe) = pending_bot(EventDelivery::Ordered { capacity: 1 }).await;
    let client = bot.client();
    client.pause().await;
    let (tx, rx) = async_channel::unbounded();
    let subscription = client.subscribe_chatstate_handler(Arc::new(move |event| {
        tx.try_send(event.state).unwrap();
    }));
    let handle = bot.spawn();
    dispatch(&client, "composing", None).await;
    receive(&probe.started).await;
    assert_eq!(receive(&rx).await, ChatActivity::Typing);
    handle.abort();
    assert!(observe_driver_cancellation(&client, &probe).await);
    dispatch(&client, "paused", None).await;
    assert_eq!(receive(&rx).await, ChatActivity::Idle);
    assert!(probe.started.try_recv().is_err());
    drop(subscription);
    drop(handle);
    assert_callback_ownership_released(client, probe).await;
}
