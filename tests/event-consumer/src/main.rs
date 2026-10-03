//! Small matched native/WASM consumer of host-owned synchronous delivery.
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use whatsapp_rust::types::events::{
    ChannelEventHandler, Connected, CoreEventBus, Event, EventHandler, EventInterest, EventKind,
};

struct HostHandler(Arc<AtomicUsize>);
impl EventHandler for HostHandler {
    fn handle_event(&self, _: Arc<Event>) {
        self.0.fetch_add(1, Ordering::Relaxed);
    }
    fn interest(&self) -> EventInterest {
        EventInterest::of(&[EventKind::Connected])
    }
}

// Export keeps the host capability exercised in the linked WASM artifact too.
// SAFETY: This standalone executable owns `event_host_probe`; it is defined once
// and no linked dependency declares that symbol. This fixture is not a library
// exposing the symbol for linkage into another host's namespace.
#[unsafe(no_mangle)]
pub extern "C" fn event_host_probe() -> usize {
    let bus = CoreEventBus::new();
    let calls = Arc::new(AtomicUsize::new(0));
    let owned = bus.subscribe_handler(Arc::new(HostHandler(calls.clone())));
    let (bounded, rx) = ChannelEventHandler::new();
    let channel = bus.subscribe_handler(bounded.clone());
    let (unbounded, unlimited) = ChannelEventHandler::unbounded();
    let explicit_unbounded = bus.subscribe_handler(unbounded);
    bus.dispatch(Event::Connected(Connected::builder().build()));
    assert_eq!(calls.load(Ordering::Relaxed), 1, "synchronous dispatch");
    assert_eq!(rx.len(), 1);
    assert_eq!(unlimited.len(), 1);
    assert!(owned.unsubscribe());
    drop(channel);
    drop(explicit_unbounded);
    bus.dispatch(Event::Connected(Connected::builder().build()));
    assert_eq!(rx.len(), 1);
    assert_eq!(unlimited.len(), 1);
    calls.load(Ordering::Relaxed) + bounded.stats().enqueued as usize
}

fn main() {
    assert_eq!(event_host_probe(), 2);
}

#[test]
fn host_owns_delivery_and_subscription() {
    main();
}
