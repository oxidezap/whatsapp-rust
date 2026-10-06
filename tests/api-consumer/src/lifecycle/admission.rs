//! Admission works for a standalone host, including a local-only wasm policy.
use std::{sync::Arc, time::Duration};
use whatsapp_rust::{Client, ConnectAdmission, RunCompletionReason, async_trait};

#[cfg(target_arch = "wasm32")]
pub struct Policy(std::rc::Rc<std::cell::Cell<usize>>);
#[cfg(not(target_arch = "wasm32"))]
pub struct Policy(std::sync::atomic::AtomicUsize);

impl ConnectAdmission for Policy {
    fn delay(&self) -> Duration {
        #[cfg(target_arch = "wasm32")]
        self.0.set(self.0.get() + 1);
        #[cfg(not(target_arch = "wasm32"))]
        self.0.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Duration::ZERO
    }
}

pub struct RecheckingPolicy(pub Policy);
impl ConnectAdmission for RecheckingPolicy {
    fn delay(&self) -> Duration {
        self.0.delay()
    }
    fn recheck(&self) -> Duration {
        // Reading the same host state remains valid with wasm's local Rc.
        #[cfg(target_arch = "wasm32")]
        let calls = self.0.0.get();
        #[cfg(not(target_arch = "wasm32"))]
        let calls = self.0.0.load(std::sync::atomic::Ordering::Relaxed);
        Duration::from_millis(calls as u64)
    }
}

pub fn builder(policy: Arc<dyn ConnectAdmission>) -> whatsapp_rust::ClientBuilder {
    Client::builder().with_connect_admission_arc(policy)
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait Driver {
    async fn run(&self) -> RunCompletionReason;
}
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl Driver for Arc<Client> {
    async fn run(&self) -> RunCompletionReason {
        Client::run(self).await
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn shared_policy_is_object_safe() {
    let policy = Policy(std::sync::atomic::AtomicUsize::new(0));
    let boxed: Box<dyn ConnectAdmission> = Box::new(policy);
    assert_eq!(boxed.delay(), Duration::ZERO);
    assert_eq!(boxed.recheck(), Duration::ZERO);
    let _ = builder(Arc::from(boxed));
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
fn rechecking_policy_is_object_safe() {
    let policy = Policy(std::sync::atomic::AtomicUsize::new(0));
    let shared: Arc<dyn ConnectAdmission> = Arc::new(RecheckingPolicy(policy));
    assert_eq!(shared.delay(), Duration::ZERO);
    assert_eq!(shared.recheck(), Duration::from_millis(1));
    assert_eq!(shared.recheck(), Duration::from_millis(1));
    let _ = builder(shared);
}
