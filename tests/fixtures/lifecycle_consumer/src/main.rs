//! Portable downstream lifecycle graph: the same source builds on base/head.
//! No runtime is driven and no network or persistence operation is executed.
use std::sync::Arc;

use whatsapp_rust::wacore::runtime::BoxFuture;
use whatsapp_rust::{Client, ClientBuilder, ClientBuilderError};

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
trait HostLifecycle {
    async fn finish(&self);
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl HostLifecycle for Arc<Client> {
    async fn finish(&self) {
        let _ = self.logout().await;
        let _ = self.shutdown().await;
    }
}

// Keeping this function pointer observable links the real public graph rather
// than comparing rlibs or a probe whose lifecycle code was optimized away.
fn lifecycle_graph(builder: ClientBuilder) -> BoxFuture<'static, Result<(), ClientBuilderError>> {
    Box::pin(async move {
        let (client, receiver) = builder.build().await?.into_parts();
        drop(receiver);
        let _ = client.is_socket_ready();
        let _ = client.is_session_ready();
        client.finish().await;
        let _ = client.run().await;
        Ok(())
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn lifecycle_graph_anchor() -> usize {
    lifecycle_graph as *const () as usize
}

fn main() {
    std::hint::black_box(lifecycle_graph_anchor());
}
