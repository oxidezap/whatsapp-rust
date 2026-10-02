//! Linked, stable-API consumer for matched base/head native and WASM sizes.
//! The WASM artifact is measured, not executed as a browser/server probe.
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use whatsapp_rust::store::persistence_manager::PersistenceManager;
use whatsapp_rust::wacore::store::in_memory::InMemoryBackend;
use whatsapp_rust::wacore::store::traits::DeviceStore;

fn ready<T>(future: impl Future<Output = T>) -> T {
    struct NoWake;
    impl Wake for NoWake {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(NoWake));
    let mut context = Context::from_waker(&waker);
    match std::pin::pin!(future).as_mut().poll(&mut context) {
        Poll::Ready(value) => value,
        Poll::Pending => panic!("uncontended in-memory operation suspended"),
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = Arc::new(InMemoryBackend::new());
    let pm = ready(PersistenceManager::new(backend.clone()))?;
    let before = pm.get_device_snapshot();
    ready(pm.modify_device(|device| device.push_name = "probe".into()));
    assert_eq!(before.push_name, "");
    assert_eq!(pm.get_device_snapshot().push_name, "probe");
    assert!(Arc::ptr_eq(&before.backend, &pm.backend()));
    ready(pm.flush())?;
    let saved = ready(backend.load())?.ok_or("saved device absent")?;
    assert_eq!(saved.push_name, "probe");
    Ok(())
}
