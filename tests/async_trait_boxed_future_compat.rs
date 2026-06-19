//! Compile-time regression guard for issue #825.
//!
//! Public async methods whose future type embeds a closure returning
//! references (e.g. an iterator adapter passed to a generic helper) fail with
//! "implementation of `FnOnce` is not general enough" ONLY when a downstream
//! caller boxes the future, which is exactly what `#[async_trait]` does. The
//! library's own tests never box that way, so this file reproduces the
//! consumer shape: if it compiles, the guard passes.

use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use wacore_binary::Jid;
use whatsapp_rust::client::Client;

#[async_trait]
pub trait WhatsAppGateway: Send + Sync {
    async fn check_phones(&self, phones: Vec<String>) -> Result<Vec<bool>, String>;
    async fn statuses(&self, phones: Vec<String>) -> Result<Vec<Option<String>>, String>;
}

pub struct GatewayImpl {
    client: RwLock<Option<Arc<Client>>>,
}

#[async_trait]
impl WhatsAppGateway for GatewayImpl {
    async fn check_phones(&self, phones: Vec<String>) -> Result<Vec<bool>, String> {
        let guard = self.client.read().await;
        let client = guard.as_ref().expect("client").clone();
        drop(guard);

        let jids: Vec<Jid> = phones.iter().map(|p| Jid::pn(p.as_str())).collect();
        let results = client
            .contacts()
            .is_on_whatsapp(&jids)
            .await
            .map_err(|e| e.to_string())?;
        Ok(results.into_iter().map(|r| r.is_registered).collect())
    }

    async fn statuses(&self, phones: Vec<String>) -> Result<Vec<Option<String>>, String> {
        let guard = self.client.read().await;
        let client = guard.as_ref().expect("client").clone();
        drop(guard);

        let jids: Vec<Jid> = phones.iter().map(|p| Jid::pn(p.as_str())).collect();
        let info = client
            .contacts()
            .get_user_info(&jids)
            .await
            .map_err(|e| e.to_string())?;
        Ok(jids
            .iter()
            .map(|jid| info.get(jid).and_then(|i| i.status.clone()))
            .collect())
    }
}

/// The guard is the compilation itself; this just keeps the test binary
/// non-empty and the trait impl reachable.
#[test]
fn async_trait_consumers_compile() {
    fn assert_impl<T: WhatsAppGateway>() {}
    assert_impl::<GatewayImpl>();
}
