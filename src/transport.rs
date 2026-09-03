// Re-export transport types from wacore
pub use wacore::net::{DisconnectReason, Transport, TransportEvent, TransportFactory};

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

    /// A transport that accepts calls and never answers them.
    ///
    /// Stands in for the socket a teardown cannot rely on: a connection whose
    /// peer has gone away without a FIN is not refused, it is retried by the
    /// kernel until `tcp_retries2` gives up (~15 minutes on Linux), and both
    /// `send` and the close queue behind that. Anything that awaits either of
    /// them unbounded is parked for the same quarter of an hour.
    pub struct StallingMockTransport {
        sends_started: std::sync::atomic::AtomicUsize,
        disconnects_started: std::sync::atomic::AtomicUsize,
    }

    impl StallingMockTransport {
        pub fn new() -> Self {
            Self {
                sends_started: std::sync::atomic::AtomicUsize::new(0),
                disconnects_started: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        /// Sends that entered the transport and never came back out.
        pub fn sends_started(&self) -> usize {
            self.sends_started
                .load(std::sync::atomic::Ordering::Acquire)
        }

        /// Closes that entered the transport and never came back out.
        pub fn disconnects_started(&self) -> usize {
            self.disconnects_started
                .load(std::sync::atomic::Ordering::Acquire)
        }
    }

    impl Default for StallingMockTransport {
        fn default() -> Self {
            Self::new()
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl Transport for StallingMockTransport {
        async fn send(&self, _data: bytes::Bytes) -> Result<(), anyhow::Error> {
            self.sends_started
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            std::future::pending::<()>().await;
            unreachable!("a stalled send never resolves")
        }

        async fn disconnect(&self) {
            self.disconnects_started
                .fetch_add(1, std::sync::atomic::Ordering::AcqRel);
            std::future::pending::<()>().await;
        }
    }

    /// Splits one transport write into the length-prefixed frames it carries,
    /// each returned with its 3-byte prefix intact so callers keep seeing what
    /// a single-frame write used to look like. A trailing partial frame (which
    /// this sender never produces) is returned as-is rather than dropped.
    fn split_framed(write: &bytes::Bytes) -> Vec<bytes::Bytes> {
        const PREFIX: usize = 3;
        let mut frames = Vec::new();
        let mut offset = 0usize;
        while offset + PREFIX <= write.len() {
            let len = ((write[offset] as usize) << 16)
                | ((write[offset + 1] as usize) << 8)
                | (write[offset + 2] as usize);
            let end = offset + PREFIX + len;
            if end > write.len() {
                break;
            }
            frames.push(write.slice(offset..end));
            offset = end;
        }
        if offset < write.len() {
            frames.push(write.slice(offset..));
        }
        frames
    }

    /// Records every `send()` payload so a unit test can assert what the
    /// client wrote to the wire.
    pub struct CapturingMockTransport {
        sent: std::sync::Mutex<Vec<bytes::Bytes>>,
        remaining_failures: std::sync::atomic::AtomicUsize,
        failed_sends: std::sync::atomic::AtomicUsize,
    }

    impl CapturingMockTransport {
        pub fn new() -> Self {
            Self {
                sent: std::sync::Mutex::new(Vec::new()),
                remaining_failures: std::sync::atomic::AtomicUsize::new(0),
                failed_sends: std::sync::atomic::AtomicUsize::new(0),
            }
        }

        /// Every frame written, one entry each, in write-counter order.
        ///
        /// The noise sender coalesces queued frames into a single `send()`, so
        /// a captured write is not necessarily one frame. Splitting here keeps
        /// the whole assertion surface ("the Nth frame is ...", decrypted under
        /// counter N) valid whether or not a batch happened to form.
        pub fn sent(&self) -> Vec<bytes::Bytes> {
            self.sent_writes().iter().flat_map(split_framed).collect()
        }

        /// The raw `send()` payloads, batches included. Use this to assert on
        /// transport-level behaviour (how many writes, how large); use
        /// [`Self::sent`] to assert on frames.
        pub fn sent_writes(&self) -> Vec<bytes::Bytes> {
            self.sent.lock().expect("capturing mutex").clone()
        }

        pub fn sent_count(&self) -> usize {
            self.sent().len()
        }

        /// Number of `send()` calls, as opposed to frames.
        pub fn write_count(&self) -> usize {
            self.sent.lock().expect("capturing mutex").len()
        }

        pub fn fail_next_sends(&self, count: usize) {
            self.remaining_failures
                .store(count, std::sync::atomic::Ordering::Release);
        }

        pub fn failed_sends(&self) -> usize {
            self.failed_sends.load(std::sync::atomic::Ordering::Acquire)
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
            if self
                .remaining_failures
                .fetch_update(
                    std::sync::atomic::Ordering::AcqRel,
                    std::sync::atomic::Ordering::Acquire,
                    |remaining| remaining.checked_sub(1),
                )
                .is_ok()
            {
                self.failed_sends
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                return Err(anyhow::anyhow!("injected transport failure"));
            }
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
