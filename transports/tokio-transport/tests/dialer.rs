//! Public API consumer: no access to factory internals or upstream upgrade code.
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use wacore::net::TransportFactory;
use whatsapp_rust_tokio_transport::{BoxedStream, StreamDialer, TokioWebSocketTransportFactory};

struct RecordingDialer(Arc<Mutex<Vec<(String, u16)>>>);

#[async_trait]
impl StreamDialer for RecordingDialer {
    async fn dial(&self, host: &str, port: u16) -> anyhow::Result<BoxedStream> {
        self.0.lock().unwrap().push((host.into(), port));
        Err(std::io::Error::new(std::io::ErrorKind::ConnectionRefused, "dial sentinel").into())
    }
}

#[tokio::test]
async fn url_destinations_and_typed_dial_errors() {
    for (url, expected) in [
        ("ws://relay.example/ws/chat?ED=AQ==", ("relay.example", 80)),
        ("wss://relay.example/ws/chat", ("relay.example", 443)),
        ("wss://relay.example:5222/ws/chat", ("relay.example", 5222)),
        ("ws://[::1]:8080/ws/chat", ("::1", 8080)),
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let factory: Box<dyn TransportFactory> = Box::new(
            TokioWebSocketTransportFactory::new()
                .with_url(url)
                .with_dialer(RecordingDialer(calls.clone())),
        );
        let error = factory.create_transport().await.err().unwrap();
        assert_eq!(
            error.downcast_ref::<std::io::Error>().unwrap().kind(),
            std::io::ErrorKind::ConnectionRefused
        );
        assert_eq!(
            *calls.lock().unwrap(),
            vec![(expected.0.into(), expected.1)]
        );
    }
}

#[tokio::test]
async fn invalid_urls_and_origin_never_invoke_the_dialer() {
    for url in [
        "/ws/chat",
        "wss://",
        "ws://:80/",
        "ws://[]/",
        "ws://host:65536/",
        "ws://host:bad/",
        "ws://host:+443/",
        "ws://host:-1/",
        "ftp://host/",
        "not a uri",
    ] {
        let calls = Arc::new(Mutex::new(Vec::new()));
        let factory = TokioWebSocketTransportFactory::new()
            .with_url(url)
            .with_dialer(RecordingDialer(calls.clone()));
        assert!(factory.create_transport().await.is_err(), "{url}");
        assert!(calls.lock().unwrap().is_empty(), "{url}");
    }
    let calls = Arc::new(Mutex::new(Vec::new()));
    let factory = TokioWebSocketTransportFactory::new()
        .with_origin("bad\r\nheader")
        .with_dialer(RecordingDialer(calls.clone()));
    assert!(factory.create_transport().await.is_err());
    assert!(calls.lock().unwrap().is_empty());
}

struct LocalDialer(std::net::SocketAddr);

#[async_trait]
impl StreamDialer for LocalDialer {
    async fn dial(&self, host: &str, port: u16) -> anyhow::Result<BoxedStream> {
        assert_eq!(host, "relay.example");
        assert_eq!(port, self.0.port());
        Ok(Box::new(tokio::net::TcpStream::connect(self.0).await?))
    }
}

#[tokio::test]
async fn cancelling_upgrade_closes_the_supplied_stream() {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let factory = TokioWebSocketTransportFactory::new()
            .with_url(format!("ws://relay.example:{}/ws/chat", addr.port()))
            .with_dialer(LocalDialer(addr));
        let mut connection = Box::pin(factory.create_transport());
        let peer = async {
            let (mut tcp, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(tcp.read_u8().await.unwrap());
            }
            assert!(
                String::from_utf8(request)
                    .unwrap()
                    .contains(&format!("Host: relay.example:{}\r\n", addr.port()))
            );
            tcp
        };
        let mut peer = tokio::select! {
            _ = &mut connection => panic!("upgrade finished without response"),
            peer = peer => peer,
        };
        drop(connection);
        assert_eq!(peer.read(&mut [0; 1]).await.unwrap(), 0);
    })
    .await
    .unwrap();
}

struct WaitingDialer {
    addr: std::net::SocketAddr,
    ready: Arc<tokio::sync::Notify>,
    dropped: Arc<std::sync::atomic::AtomicBool>,
}
struct DropFlag(Arc<std::sync::atomic::AtomicBool>);
impl Drop for DropFlag {
    fn drop(&mut self) {
        self.0.store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[async_trait]
impl StreamDialer for WaitingDialer {
    async fn dial(&self, _: &str, _: u16) -> anyhow::Result<BoxedStream> {
        let _stream = tokio::net::TcpStream::connect(self.addr).await?;
        let _flag = DropFlag(self.dropped.clone());
        self.ready.notify_one();
        std::future::pending().await
    }
}

#[tokio::test]
async fn transport_timeout_cancels_an_in_progress_dial() {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let dropped = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let ready = Arc::new(tokio::sync::Notify::new());
        let factory = TokioWebSocketTransportFactory::new().with_dialer(WaitingDialer {
            addr: listener.local_addr().unwrap(),
            ready: ready.clone(),
            dropped: dropped.clone(),
        });
        let mut connection = Box::pin(factory.create_transport());
        tokio::select! {
            _ = &mut connection => panic!("pending dial completed"),
            _ = ready.notified() => {},
        }
        assert!(
            tokio::time::timeout(std::time::Duration::ZERO, connection)
                .await
                .is_err()
        );
        assert!(dropped.load(std::sync::atomic::Ordering::SeqCst));
        let (mut peer, _) = listener.accept().await.unwrap();
        assert_eq!(peer.read(&mut [0; 1]).await.unwrap(), 0);
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn failed_upgrade_preserves_source_and_closes_the_stream() {
    tokio::time::timeout(std::time::Duration::from_secs(5), async {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let factory = TokioWebSocketTransportFactory::new()
            .with_url(format!("ws://relay.example:{}/ws/chat", addr.port()))
            .with_dialer(LocalDialer(addr));
        let server = async {
            let (mut peer, _) = listener.accept().await.unwrap();
            let mut request = Vec::new();
            while !request.ends_with(b"\r\n\r\n") {
                request.push(peer.read_u8().await.unwrap());
            }
            peer.write_all(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 0\r\n\r\n")
                .await
                .unwrap();
            assert_eq!(peer.read(&mut [0; 1]).await.unwrap(), 0);
        };
        let client = async {
            let error = factory.create_transport().await.err().unwrap();
            assert!(error.downcast_ref::<tokio_websockets::Error>().is_some());
        };
        tokio::join!(server, client);
    })
    .await
    .unwrap();
}
