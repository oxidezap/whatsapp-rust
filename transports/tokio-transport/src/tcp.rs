//! Raw byte-stream transport for mobile clients and independently keyed mocks.
//!
//! Framing and Noise remain in the existing protocol layer. This adapter neither
//! adds TLS nor performs an HTTP upgrade, even when the selected port is 443.

use std::sync::Arc;

use anyhow::{Context, bail};
use async_trait::async_trait;
use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{Mutex, watch};
use wacore::net::{DisconnectReason, Transport, TransportEvent, TransportFactory};

use crate::StreamDialer;

const READ_CAPACITY: usize = 16 * 1024;
const EVENT_CAPACITY: usize = 64;

/// TCP factory with an explicit destination and no fallback to production.
///
/// Dial cancellation and timeouts remain owned by the caller, as with the
/// WebSocket factory. A custom dialer may provide a proxy tunnel or test stream.
#[derive(Clone)]
pub struct TokioTcpTransportFactory {
    host: String,
    port: u16,
    dialer: Option<Arc<dyn StreamDialer>>,
}

impl TokioTcpTransportFactory {
    pub fn new(host: impl Into<String>, port: u16) -> anyhow::Result<Self> {
        let host = host.into();
        // Portable configuration may retain URI brackets, while stream dialers
        // and Tokio's (host, port) resolver expect an unbracketed IPv6 literal.
        let host = if host.starts_with('[') || host.ends_with(']') {
            host.strip_prefix('[')
                .and_then(|value| value.strip_suffix(']'))
                .and_then(|value| value.parse::<std::net::Ipv6Addr>().ok())
                .context("invalid bracketed IPv6 TCP host")?
                .to_string()
        } else {
            host
        };
        if host.is_empty()
            || host.chars().any(|c| {
                c.is_whitespace() || c.is_control() || matches!(c, '/' | '\\' | '@' | '?' | '#')
            })
            || (host.contains(':') && host.parse::<std::net::Ipv6Addr>().is_err())
            || port == 0
        {
            bail!("TCP destination requires a host without URL syntax and a nonzero port");
        }
        Ok(Self {
            host,
            port,
            dialer: None,
        })
    }

    pub fn with_dialer(mut self, dialer: impl StreamDialer + 'static) -> Self {
        self.dialer = Some(Arc::new(dialer));
        self
    }
}

#[async_trait]
impl TransportFactory for TokioTcpTransportFactory {
    async fn create_transport(
        &self,
    ) -> anyhow::Result<(Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)> {
        if let Some(dialer) = &self.dialer {
            return Ok(from_byte_stream(dialer.dial(&self.host, self.port).await?));
        }
        let stream = tokio::net::TcpStream::connect((self.host.as_str(), self.port))
            .await
            .context("TCP connection failed")?;
        stream.set_nodelay(true).context("setting TCP_NODELAY")?;
        Ok(from_byte_stream(stream))
    }
}

struct ByteStreamTransport<W> {
    writer: Mutex<Option<W>>,
    shutdown: watch::Sender<bool>,
}

impl<W> Drop for ByteStreamTransport<W> {
    fn drop(&mut self) {
        let _ = self.shutdown.send(true);
    }
}

// A cancelled write may have sent a prefix. Retire the entire connection so a
// subsequent caller cannot append bytes to that incomplete encrypted frame.
struct WriteRetirement<'a> {
    shutdown: &'a watch::Sender<bool>,
    armed: bool,
}

impl Drop for WriteRetirement<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.shutdown.send(true);
        }
    }
}

#[async_trait]
impl<W: AsyncWrite + Unpin + Send> Transport for ByteStreamTransport<W> {
    async fn send(&self, data: Bytes) -> anyhow::Result<()> {
        let mut shutdown = self.shutdown.subscribe();
        let mut guard = tokio::select! {
            biased;
            _ = shutdown.changed() => bail!("TCP transport is closed"),
            guard = self.writer.lock() => guard,
        };
        if *shutdown.borrow() {
            bail!("TCP transport is closed");
        }
        let writer = guard.as_mut().context("TCP transport is closed")?;
        let mut retirement = WriteRetirement {
            shutdown: &self.shutdown,
            armed: true,
        };
        tokio::select! {
            biased;
            _ = shutdown.changed() => bail!("TCP transport closed during write"),
            result = async {
                writer.write_all(&data).await.context("TCP write failed")?;
                // Dialers may return buffered streams. A completed send must
                // reach the underlying stream before retirement is disarmed.
                writer.flush().await.context("TCP flush failed")
            } => result?,
        }
        retirement.armed = false;
        Ok(())
    }

    async fn disconnect(&self) {
        let _ = self.shutdown.send(true);
        // Closing the write half, rather than waiting for a peer-driven shutdown,
        // also releases pending writers when the peer stops reading.
        self.writer.lock().await.take();
    }
}

fn handoff_read(buffer: &mut BytesMut) -> Bytes {
    if buffer.len() < crate::ADOPT_THRESHOLD {
        // A one-byte TCP fragment must not retain a whole 16 KiB read allocation
        // in the event queue. Keep that allocation reusable for the next read.
        let data = Bytes::copy_from_slice(buffer);
        buffer.clear();
        data
    } else {
        buffer.split().freeze()
    }
}

/// Adapts an owned stream without adding framing, TLS or certificate bypasses.
///
/// Reads are bounded and delivered through a bounded event queue. Disconnect,
/// dropped transport ownership and a dropped receiver release the read task,
/// including while that queue is full. The existing Noise layer validates trust.
pub fn from_byte_stream<S>(
    stream: S,
) -> (Arc<dyn Transport>, async_channel::Receiver<TransportEvent>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let (mut reader, writer) = tokio::io::split(stream);
    let (tx, rx) = async_channel::bounded(EVENT_CAPACITY);
    let (shutdown, mut stop) = watch::channel(false);
    let transport = Arc::new(ByteStreamTransport {
        writer: Mutex::new(Some(writer)),
        shutdown,
    });
    let signal = transport.shutdown.clone();
    let weak_transport = Arc::downgrade(&transport);
    let _ = tx.try_send(TransportEvent::Connected);
    tokio::spawn(async move {
        let mut buffer = BytesMut::with_capacity(READ_CAPACITY);
        let reason = loop {
            // Bound each read independently of AsyncRead's allocation strategy.
            let mut limited_reader = (&mut reader).take(READ_CAPACITY as u64);
            let result = tokio::select! {
                biased;
                _ = stop.changed() => break DisconnectReason::Unknown,
                _ = tx.closed() => break DisconnectReason::Unknown,
                result = limited_reader.read_buf(&mut buffer) => result,
            };
            match result {
                Ok(0) => break DisconnectReason::StreamEnded,
                Ok(_) => {
                    let data = handoff_read(&mut buffer);
                    tokio::select! {
                        biased;
                        _ = stop.changed() => break DisconnectReason::Unknown,
                        result = tx.send(TransportEvent::DataReceived(data)) => {
                            if result.is_err() {
                                break DisconnectReason::Unknown;
                            }
                        }
                    }
                    buffer.reserve(READ_CAPACITY);
                }
                Err(error) => break DisconnectReason::ReadError(error.to_string()),
            }
        };
        drop(reader);
        let _ = signal.send(true);
        // Retirement must release both halves even when the caller retains the
        // transport. Signalling first interrupts a writer waiting on the peer.
        if let Some(transport) = weak_transport.upgrade() {
            transport.writer.lock().await.take();
        }
        // A full, unconsumed queue must not keep the task or socket alive.
        // The eventual channel closure also reports termination to the owner.
        let _ = tx.try_send(TransportEvent::Disconnected(reason));
    });
    (transport, rx)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{Duration, timeout};

    const DEADLINE: Duration = Duration::from_secs(2);

    #[tokio::test]
    async fn preserves_bytes_across_partial_reads_and_reports_eof() {
        let (stream, mut peer) = tokio::io::duplex(8);
        let (transport, events) = from_byte_stream(stream);
        assert!(matches!(
            events.recv().await.unwrap(),
            TransportEvent::Connected
        ));
        let peer_task = tokio::spawn(async move {
            let mut sent = [0; 5];
            peer.read_exact(&mut sent).await.unwrap();
            assert_eq!(&sent, b"hello");
            peer.write_all(b"fragmented stream").await.unwrap();
        });
        transport.send(Bytes::from_static(b"hello")).await.unwrap();
        let mut received = Vec::new();
        timeout(DEADLINE, async {
            loop {
                match events.recv().await.unwrap() {
                    TransportEvent::DataReceived(data) => received.extend_from_slice(&data),
                    TransportEvent::Disconnected(DisconnectReason::StreamEnded) => break,
                    event => panic!("unexpected event: {event:?}"),
                }
            }
        })
        .await
        .unwrap();
        assert_eq!(received, b"fragmented stream");
        peer_task.await.unwrap();
        assert!(
            transport
                .send(Bytes::from_static(b"after EOF"))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn cancelled_partial_write_retires_connection() {
        let (stream, mut actual_peer) = tokio::io::duplex(1);
        let (transport, _events) = from_byte_stream(stream);
        let mut pending = Box::pin(transport.send(Bytes::from_static(b"ab")));
        tokio::select! {
            biased;
            result = &mut pending => panic!("write unexpectedly completed: {result:?}"),
            result = actual_peer.read_u8() => assert_eq!(result.unwrap(), b'a'),
        }
        drop(pending);
        assert!(transport.send(Bytes::from_static(b"c")).await.is_err());
        assert_eq!(
            timeout(DEADLINE, actual_peer.read_u8())
                .await
                .unwrap()
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::UnexpectedEof
        );
    }

    #[tokio::test]
    async fn disconnect_interrupts_a_stalled_write() {
        let (stream, mut peer) = tokio::io::duplex(1);
        let (transport, _events) = from_byte_stream(stream);
        let send_transport = transport.clone();
        let pending =
            tokio::spawn(async move { send_transport.send(Bytes::from(vec![0; 1024])).await });
        assert_eq!(timeout(DEADLINE, peer.read_u8()).await.unwrap().unwrap(), 0);
        timeout(DEADLINE, transport.disconnect()).await.unwrap();
        assert!(timeout(DEADLINE, pending).await.unwrap().unwrap().is_err());
    }

    #[tokio::test]
    async fn write_and_flush_errors_release_stream_without_explicit_disconnect() {
        use std::{
            pin::Pin,
            task::{Context, Poll},
        };
        struct FailingStream {
            on_drop: Option<tokio::sync::oneshot::Sender<()>>,
            fail_flush: bool,
        }
        impl Drop for FailingStream {
            fn drop(&mut self) {
                if let Some(sender) = self.on_drop.take() {
                    let _ = sender.send(());
                }
            }
        }
        impl AsyncRead for FailingStream {
            fn poll_read(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
                _: &mut tokio::io::ReadBuf<'_>,
            ) -> Poll<std::io::Result<()>> {
                Poll::Pending
            }
        }
        impl AsyncWrite for FailingStream {
            fn poll_write(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
                bytes: &[u8],
            ) -> Poll<std::io::Result<usize>> {
                if self.fail_flush {
                    return Poll::Ready(Ok(bytes.len()));
                }
                Poll::Ready(Err(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "synthetic write failure",
                )))
            }
            fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<std::io::Result<()>> {
                if self.fail_flush {
                    return Poll::Ready(Err(std::io::Error::new(
                        std::io::ErrorKind::BrokenPipe,
                        "synthetic flush failure",
                    )));
                }
                Poll::Ready(Ok(()))
            }
            fn poll_shutdown(
                self: Pin<&mut Self>,
                _: &mut Context<'_>,
            ) -> Poll<std::io::Result<()>> {
                Poll::Ready(Ok(()))
            }
        }
        for fail_flush in [false, true] {
            let (dropped, observed) = tokio::sync::oneshot::channel();
            let (transport, _events) = from_byte_stream(FailingStream {
                on_drop: Some(dropped),
                fail_flush,
            });
            let expected = if fail_flush {
                "TCP flush failed"
            } else {
                "TCP write failed"
            };
            assert!(
                transport
                    .send(Bytes::from_static(b"fail"))
                    .await
                    .unwrap_err()
                    .to_string()
                    .contains(expected)
            );
            timeout(DEADLINE, observed).await.unwrap().unwrap();
            assert!(
                transport
                    .send(Bytes::from_static(b"retired"))
                    .await
                    .is_err()
            );
        }
    }

    #[tokio::test]
    async fn buffered_stream_sends_are_flushed_before_success() {
        let (stream, mut peer) = tokio::io::duplex(1024);
        let (transport, _events) = from_byte_stream(tokio::io::BufStream::new(stream));
        transport.send(Bytes::from_static(b"hello")).await.unwrap();
        let mut bytes = [0; 5];
        timeout(DEADLINE, peer.read_exact(&mut bytes))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&bytes, b"hello");
    }

    #[tokio::test]
    async fn cancelled_buffer_flush_retires_connection() {
        let (stream, mut peer) = tokio::io::duplex(1);
        let (transport, _events) = from_byte_stream(tokio::io::BufStream::new(stream));
        let mut send = Box::pin(transport.send(Bytes::from_static(b"ab")));
        tokio::select! {
            biased;
            result = &mut send => panic!("flush unexpectedly completed: {result:?}"),
            result = peer.read_u8() => assert_eq!(result.unwrap(), b'a'),
        }
        drop(send);
        assert_eq!(
            timeout(DEADLINE, peer.read_u8())
                .await
                .unwrap()
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::UnexpectedEof
        );
        assert!(
            transport
                .send(Bytes::from_static(b"retired"))
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn dropping_transport_stops_a_backpressured_reader() {
        let (stream, mut peer) = tokio::io::duplex(1);
        let (transport, events) = from_byte_stream(stream);
        assert!(matches!(
            events.recv().await.unwrap(),
            TransportEvent::Connected
        ));
        let (filled_tx, filled_rx) = tokio::sync::oneshot::channel();
        let filling = tokio::spawn(async move {
            // One byte per read: a full queue, one read waiting for queue space,
            // and one byte left in the duplex stream, without timing assumptions.
            peer.write_all(&[0; EVENT_CAPACITY + 2]).await.unwrap();
            filled_tx.send(()).unwrap();
            assert_eq!(
                peer.read_u8().await.unwrap_err().kind(),
                std::io::ErrorKind::UnexpectedEof
            );
        });
        timeout(DEADLINE, filled_rx).await.unwrap().unwrap();
        assert_eq!(events.len(), EVENT_CAPACITY);
        drop(transport);
        timeout(DEADLINE, filling).await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn dropping_receiver_closes_stream_while_transport_is_retained() {
        let (stream, mut peer) = tokio::io::duplex(1);
        let (transport, events) = from_byte_stream(stream);
        drop(events);
        assert_eq!(
            timeout(DEADLINE, peer.read_u8())
                .await
                .unwrap()
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::UnexpectedEof
        );
        assert!(transport.send(Bytes::from_static(b"closed")).await.is_err());
    }

    #[tokio::test]
    async fn factory_connects_to_only_its_configured_destination() {
        let listener = tokio::net::TcpListener::bind(("127.0.0.1", 0))
            .await
            .unwrap();
        let addr = listener.local_addr().unwrap();
        let factory = TokioTcpTransportFactory::new("127.0.0.1", addr.port()).unwrap();
        let (transport, events) = timeout(DEADLINE, factory.create_transport())
            .await
            .unwrap()
            .unwrap();
        let (mut peer, _) = timeout(DEADLINE, listener.accept()).await.unwrap().unwrap();
        assert!(matches!(
            events.recv().await.unwrap(),
            TransportEvent::Connected
        ));
        transport.send(Bytes::from_static(b"raw")).await.unwrap();
        let mut bytes = [0; 3];
        timeout(DEADLINE, peer.read_exact(&mut bytes))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&bytes, b"raw");
        transport.disconnect().await;
        assert_eq!(
            timeout(DEADLINE, peer.read_u8())
                .await
                .unwrap()
                .unwrap_err()
                .kind(),
            std::io::ErrorKind::UnexpectedEof
        );
    }

    #[test]
    fn short_reads_reuse_the_read_allocation_without_retaining_it_in_events() {
        let mut buffer = BytesMut::with_capacity(READ_CAPACITY);
        let allocation = buffer.as_ptr();
        buffer.extend_from_slice(b"a");
        let first = handoff_read(&mut buffer);
        buffer.reserve(READ_CAPACITY);
        assert_eq!(buffer.as_ptr(), allocation);
        buffer.extend_from_slice(b"b");
        let second = handoff_read(&mut buffer);
        assert_eq!(buffer.as_ptr(), allocation);
        assert_eq!(buffer.capacity(), READ_CAPACITY);
        assert_eq!(&first[..], b"a");
        assert_eq!(&second[..], b"b");
    }

    #[test]
    fn normalizes_ipv6_for_native_and_custom_dialers() {
        assert_eq!(
            TokioTcpTransportFactory::new("[::1]", 443).unwrap().host,
            "::1"
        );
        assert_eq!(
            TokioTcpTransportFactory::new("::1", 443).unwrap().host,
            "::1"
        );
        assert!(TokioTcpTransportFactory::new("127.0.0.1:443", 443).is_err());
        assert!(TokioTcpTransportFactory::new("host\0", 443).is_err());
    }

    #[tokio::test]
    async fn custom_dialer_receives_destination_and_errors_never_fall_back() {
        struct RejectDialer;
        #[async_trait]
        impl StreamDialer for RejectDialer {
            async fn dial(&self, host: &str, port: u16) -> anyhow::Result<crate::BoxedStream> {
                assert_eq!((host, port), ("::1", 1234));
                bail!("synthetic dial failure")
            }
        }
        let factory = TokioTcpTransportFactory::new("[::1]", 1234)
            .unwrap()
            .with_dialer(RejectDialer);
        let result = factory.create_transport().await;
        assert!(matches!(result, Err(error) if error.to_string() == "synthetic dial failure"));
    }

    #[test]
    fn rejects_invalid_destinations() {
        for (host, port) in [
            ("", 443),
            ("https://localhost", 443),
            ("host/path", 443),
            ("host", 0),
            ("host\n", 443),
        ] {
            assert!(TokioTcpTransportFactory::new(host, port).is_err());
        }
    }
}
