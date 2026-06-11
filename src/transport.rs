// Re-export transport types from wacore
pub use wacore::net::{Transport, TransportEvent, TransportFactory};

#[cfg(feature = "tokio-transport")]
pub use whatsapp_rust_tokio_transport::{
    Connector, TokioWebSocketTransportFactory, default_tls_connector, from_websocket,
};

#[cfg(test)]
pub mod mock {
    use super::*;
    use async_trait::async_trait;
    use std::sync::Arc;

    /// A mock transport that does nothing, for testing purposes
    pub struct MockTransport;

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl Transport for MockTransport {
        async fn send(&self, _data: bytes::Bytes) -> Result<(), anyhow::Error> {
            Ok(())
        }

        async fn disconnect(&self) {}
    }

    /// A mock transport factory for testing
    #[derive(Default)]
    pub struct MockTransportFactory;

    impl MockTransportFactory {
        pub fn new() -> Self {
            Self
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl TransportFactory for MockTransportFactory {
        async fn create_transport(
            &self,
        ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), anyhow::Error>
        {
            let (_tx, rx) = async_channel::bounded(1);
            Ok((Arc::new(MockTransport), rx))
        }
    }

    /// Records every `send()` payload so a unit test can assert what the
    /// client wrote to the wire.
    pub struct CapturingMockTransport {
        sent: std::sync::Mutex<Vec<bytes::Bytes>>,
    }

    impl CapturingMockTransport {
        pub fn new() -> Self {
            Self {
                sent: std::sync::Mutex::new(Vec::new()),
            }
        }

        pub fn sent(&self) -> Vec<bytes::Bytes> {
            self.sent.lock().expect("capturing mutex").clone()
        }
    }

    impl Default for CapturingMockTransport {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl Transport for CapturingMockTransport {
        async fn send(&self, data: bytes::Bytes) -> Result<(), anyhow::Error> {
            self.sent.lock().expect("capturing mutex").push(data);
            Ok(())
        }

        async fn disconnect(&self) {}
    }

    /// Factory variant for [`CapturingMockTransport`].
    pub struct CapturingMockTransportFactory {
        transport: Arc<CapturingMockTransport>,
    }

    impl CapturingMockTransportFactory {
        pub fn new() -> Self {
            Self {
                transport: Arc::new(CapturingMockTransport::new()),
            }
        }

        pub fn transport(&self) -> Arc<CapturingMockTransport> {
            self.transport.clone()
        }
    }

    impl Default for CapturingMockTransportFactory {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl TransportFactory for CapturingMockTransportFactory {
        async fn create_transport(
            &self,
        ) -> Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>), anyhow::Error>
        {
            let (_tx, rx) = async_channel::bounded(1);
            Ok((self.transport.clone(), rx))
        }
    }
}
