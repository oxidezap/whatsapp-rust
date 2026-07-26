use crate::socket::error::{EncryptSendError, EncryptSendErrorKind, Result, SocketError};
use crate::transport::Transport;
use async_channel;
use bytes::BytesMut;
use futures::channel::oneshot;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use wacore::handshake::{NoiseCipher, NoiseError};
use wacore::runtime::{AbortHandle, Runtime};

const INLINE_ENCRYPT_THRESHOLD: usize = 16 * 1024;

/// Result type for send operations.
type SendResult = std::result::Result<(), EncryptSendError>;

/// A job sent to the dedicated sender task.
struct SendJob {
    plaintext: bytes::Bytes,
    response_tx: oneshot::Sender<SendResult>,
}

pub struct NoiseSocket {
    read_key: Arc<NoiseCipher>,
    read_counter: Arc<AtomicU32>,
    /// Channel to send jobs to the dedicated sender task.
    /// Using a channel instead of a mutex avoids blocking callers while
    /// the current send is in progress - they can enqueue their work and
    /// await the result without holding a lock.
    send_job_tx: async_channel::Sender<SendJob>,
    /// Handle to the sender task. Aborted on drop to prevent resource leaks
    /// if the task is stuck on a slow/hanging network operation.
    _sender_task_handle: AbortHandle,
}

impl NoiseSocket {
    pub fn new(
        runtime: Arc<dyn Runtime>,
        transport: Arc<dyn Transport>,
        write_key: NoiseCipher,
        read_key: NoiseCipher,
    ) -> Self {
        Self::with_stats(runtime, transport, write_key, read_key, None)
    }

    /// Like [`Self::new`], recording sent frames into `stats` (the main WA
    /// session socket passes the client's [`SessionStats`](wacore::stats::SessionStats); VoIP relay
    /// sockets and tests pass `None`).
    pub fn with_stats(
        runtime: Arc<dyn Runtime>,
        transport: Arc<dyn Transport>,
        write_key: NoiseCipher,
        read_key: NoiseCipher,
        stats: Option<Arc<wacore::stats::SessionStats>>,
    ) -> Self {
        let write_key = Arc::new(write_key);
        let read_key = Arc::new(read_key);

        // Small buffer matched to typical steady-state throughput; the sender
        // task is network-bound (awaits `transport.send`), so a transient
        // WebSocket stall will backpressure producers here rather than queue.
        let (send_job_tx, send_job_rx) = async_channel::bounded::<SendJob>(8);

        // Spawn the dedicated sender task
        let transport_clone = transport.clone();
        let write_key_clone = write_key.clone();
        let rt_clone = runtime.clone();
        let sender_task_handle = runtime.spawn(Box::pin(Self::sender_task(
            rt_clone,
            transport_clone,
            write_key_clone,
            send_job_rx,
            stats,
        )));

        Self {
            read_key,
            read_counter: Arc::new(AtomicU32::new(0)),
            send_job_tx,
            _sender_task_handle: sender_task_handle,
        }
    }

    /// Dedicated sender task that processes send jobs sequentially.
    /// This ensures frames are sent in counter order without requiring a mutex.
    /// The task owns the write counter and processes jobs one at a time.
    async fn sender_task(
        runtime: Arc<dyn Runtime>,
        transport: Arc<dyn Transport>,
        write_key: Arc<NoiseCipher>,
        send_job_rx: async_channel::Receiver<SendJob>,
        stats: Option<Arc<wacore::stats::SessionStats>>,
    ) {
        let mut write_counter: u32 = 0;
        let mut enc_buf = Vec::with_capacity(4096);
        // BytesMut: split().freeze() yields a zero-copy Bytes while retaining
        // the underlying allocation for the next frame.
        let mut out_buf = BytesMut::with_capacity(4096);
        // A failed transport write says nothing about how much of the frame the
        // peer received, so the counter that frame consumed can neither be
        // reused (nonce reuse under the same write key) nor confidently skipped
        // (the peer's read counter would desync). Both outcomes are unrecoverable
        // in-band, so the whole sender goes out of service and the connection
        // must be re-established with a fresh handshake key.
        let mut poisoned = false;

        while let Ok(job) = send_job_rx.recv().await {
            let result = if poisoned {
                Err(EncryptSendError::poisoned())
            } else {
                let outcome = Self::process_send_job(
                    &runtime,
                    &transport,
                    &write_key,
                    &mut write_counter,
                    job.plaintext,
                    &mut enc_buf,
                    &mut out_buf,
                    stats.as_deref(),
                )
                .await;
                // Crypto and framing failures are rejected before any byte
                // reaches the wire and leave the counter untouched, so they do
                // not compromise the keystream. Only a transport failure is
                // ambiguous.
                if let Err(err) = &outcome
                    && matches!(err.kind, EncryptSendErrorKind::Transport)
                {
                    poisoned = true;
                    // Poisoning only stops this half. A write can fail while
                    // the read half stays open (half-open socket, or a
                    // Transport that reports Err without emitting
                    // Disconnected), and then nothing else would notice: the
                    // read loop keeps running and the client reports itself
                    // connected while every send fails forever. Closing the
                    // transport makes the existing disconnect path observe the
                    // drop and reconnect with a fresh handshake key, which is
                    // the only way this sender becomes usable again.
                    transport.disconnect().await;
                }
                outcome
            };

            let _ = job.response_tx.send(result);
        }
    }

    /// Process a single send job: encrypt and send the message.
    #[allow(clippy::too_many_arguments)]
    async fn process_send_job(
        runtime: &Arc<dyn Runtime>,
        transport: &Arc<dyn Transport>,
        write_key: &Arc<NoiseCipher>,
        write_counter: &mut u32,
        plaintext: bytes::Bytes,
        enc_buf: &mut Vec<u8>,
        out_buf: &mut BytesMut,
        stats: Option<&wacore::stats::SessionStats>,
    ) -> SendResult {
        let counter = *write_counter;
        // Refuse to wrap the per-direction frame counter: reusing an AES-GCM nonce
        // under the same key is catastrophic. Mirrors NoiseState::post_increment_counter
        // (which errors on exhaustion). 2^32 frames per connection is unreachable in
        // practice, so erroring here forces a reconnect rather than a silent nonce reuse.
        if counter == u32::MAX {
            return Err(EncryptSendError::crypto(NoiseError::CounterExhausted));
        }

        if plaintext.len() <= INLINE_ENCRYPT_THRESHOLD {
            enc_buf.clear();
            enc_buf.extend_from_slice(&plaintext);
            if let Err(e) = write_key.encrypt_in_place_with_counter(counter, enc_buf) {
                return Err(EncryptSendError::crypto(e));
            }

            if let Err(e) = wacore::framing::encode_frame_into(enc_buf, None, out_buf) {
                return Err(EncryptSendError::framing(e));
            }
        } else {
            let write_key = write_key.clone();
            // `Bytes` is Send + 'static: move it into the blocking task (a refcount
            // bump) instead of copying the whole >16KB plaintext with `to_vec()`.
            let encrypt_result = wacore::runtime::blocking(&**runtime, move || {
                write_key.encrypt_with_counter(counter, &plaintext)
            })
            .await;

            let ciphertext = match encrypt_result {
                Ok(c) => c,
                Err(e) => {
                    return Err(EncryptSendError::crypto(e));
                }
            };

            if let Err(e) = wacore::framing::encode_frame_into(&ciphertext, None, out_buf) {
                return Err(EncryptSendError::framing(e));
            }
        }

        // Zero-copy: split() moves the written data into a new BytesMut,
        // freeze() converts it to Bytes. The original out_buf retains its
        // allocated capacity for the next frame.
        let frame = out_buf.split().freeze();
        let wire_bytes = frame.len();
        // Burn the counter at the moment the nonce leaves this function, not
        // when the write is confirmed: a `transport.send` that returns Err may
        // still have put those bytes on the wire, and a second frame derived
        // from the same counter would reuse the AES-GCM nonce. The counter is
        // known to be below u32::MAX from the guard above, so `+ 1` is exact.
        *write_counter = counter + 1;
        if let Err(e) = transport.send(frame).await {
            return Err(EncryptSendError::transport(e));
        }
        if let Some(stats) = stats {
            stats.record_frame_sent(wire_bytes);
        }

        Ok(())
    }

    pub async fn encrypt_and_send(&self, plaintext: bytes::Bytes) -> SendResult {
        let (response_tx, response_rx) = oneshot::channel();

        let job = SendJob {
            plaintext,
            response_tx,
        };

        // Send job to the sender task. If channel is closed, sender task has stopped.
        if let Err(_send_err) = self.send_job_tx.send(job).await {
            return Err(EncryptSendError::channel_closed());
        }

        // Wait for the sender task to process our job and return the result
        match response_rx.await {
            Ok(result) => result,
            Err(_) => {
                // Sender task dropped without sending a response
                Err(EncryptSendError::channel_closed())
            }
        }
    }

    pub fn decrypt_frame(&self, mut ciphertext: BytesMut) -> Result<BytesMut> {
        // Checked increment: error instead of wrapping the read counter (AES-GCM
        // nonce reuse). fetch_update returns the pre-increment counter to use, or
        // Err when it would overflow u32. Mirrors the write side.
        let counter = self
            .read_counter
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |c| c.checked_add(1))
            .map_err(|_| SocketError::Cipher(NoiseError::CounterExhausted))?;
        self.read_key
            .decrypt_in_place_with_counter(counter, &mut ciphertext)
            .map_err(SocketError::Cipher)?;
        Ok(ciphertext)
    }
}

// AbortHandle aborts the sender task on drop automatically, so no manual
// Drop impl is needed — the `sender_task_handle` field's own Drop does the work.

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use wacore::framing::FRAME_LENGTH_SIZE;

    #[tokio::test]
    async fn test_encrypt_and_send_succeeds() {
        let transport = Arc::new(crate::transport::mock::MockTransport);

        let key = [0u8; 32];
        let write_key = NoiseCipher::new(&key).expect("32-byte key should be valid");
        let read_key = NoiseCipher::new(&key).expect("32-byte key should be valid");

        let socket = NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport,
            write_key,
            read_key,
        );

        let result = socket.encrypt_and_send(bytes::Bytes::new()).await;
        assert!(result.is_ok(), "encrypt_and_send should succeed");
    }

    #[tokio::test]
    async fn decrypt_frame_errors_on_counter_exhaustion() {
        let key = [0u8; 32];
        let socket = NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(crate::transport::mock::MockTransport),
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
        );
        // At u32::MAX the next read would wrap the counter to 0 and reuse a nonce;
        // the counter check fires before decryption, so the bytes don't matter.
        socket.read_counter.store(u32::MAX, Ordering::SeqCst);
        let err = socket
            .decrypt_frame(BytesMut::from(&b"ignored"[..]))
            .expect_err("exhausted read counter must error, not wrap");
        assert!(matches!(
            err,
            SocketError::Cipher(NoiseError::CounterExhausted)
        ));
    }

    /// Frames above INLINE_ENCRYPT_THRESHOLD take the blocking path that now moves
    /// the `Bytes` plaintext (refcount) instead of `to_vec()`-copying it. Verify
    /// both a small (inline) and a large (>16KB) frame still encrypt to ciphertext
    /// that decrypts back to the exact original.
    #[tokio::test]
    async fn test_large_frame_round_trips_via_bytes_path() {
        use async_lock::Mutex;
        use async_trait::async_trait;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicU32, Ordering};

        struct CapturingTransport {
            captured: Arc<Mutex<Vec<Vec<u8>>>>,
            read_key: NoiseCipher,
            counter: AtomicU32,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl Transport for CapturingTransport {
            async fn send(&self, data: bytes::Bytes) -> std::result::Result<(), anyhow::Error> {
                let mut data = data.to_vec();
                data.drain(..3); // strip the 3-byte frame length prefix
                let counter = self.counter.fetch_add(1, Ordering::SeqCst);
                self.read_key
                    .decrypt_in_place_with_counter(counter, &mut data)
                    .expect("frame should decrypt");
                self.captured.lock().await.push(data);
                Ok(())
            }
            async fn disconnect(&self) {}
        }

        let captured = Arc::new(Mutex::new(Vec::new()));
        let key = [7u8; 32];
        let transport = Arc::new(CapturingTransport {
            captured: captured.clone(),
            read_key: NoiseCipher::new(&key).expect("32-byte key"),
            counter: AtomicU32::new(0),
        });
        let socket = NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport,
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
        );

        let small: Vec<u8> = (0..1_000u32).map(|i| i as u8).collect();
        let large: Vec<u8> = (0..40_000u32).map(|i| (i % 251) as u8).collect();
        assert!(small.len() <= INLINE_ENCRYPT_THRESHOLD);
        assert!(large.len() > INLINE_ENCRYPT_THRESHOLD);

        socket
            .encrypt_and_send(bytes::Bytes::from(small.clone()))
            .await
            .expect("small frame send");
        socket
            .encrypt_and_send(bytes::Bytes::from(large.clone()))
            .await
            .expect("large frame send");

        let got = captured.lock().await;
        assert_eq!(got.len(), 2);
        assert_eq!(got[0], small, "inline (<=16KB) frame must round-trip");
        assert_eq!(
            got[1], large,
            "large (>16KB) frame must round-trip via the moved-Bytes path"
        );
    }

    /// A transport that accepts (and records) the frame and *then* reports
    /// failure: the ambiguous case where the peer may well have consumed the
    /// frame, so its read counter has already advanced.
    struct AcceptThenFailTransport {
        sent: std::sync::Mutex<Vec<bytes::Bytes>>,
        fail_from: usize,
        disconnected: AtomicBool,
    }

    impl AcceptThenFailTransport {
        fn new(fail_from: usize) -> Self {
            Self {
                sent: std::sync::Mutex::new(Vec::new()),
                fail_from,
                disconnected: AtomicBool::new(false),
            }
        }

        fn sent(&self) -> Vec<bytes::Bytes> {
            self.sent.lock().expect("send mutex").clone()
        }

        fn disconnected(&self) -> bool {
            self.disconnected.load(Ordering::SeqCst)
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl Transport for AcceptThenFailTransport {
        async fn send(&self, data: bytes::Bytes) -> std::result::Result<(), anyhow::Error> {
            let mut sent = self.sent.lock().expect("send mutex");
            sent.push(data);
            if sent.len() > self.fail_from {
                return Err(anyhow::anyhow!(
                    "injected failure after accepting the frame"
                ));
            }
            Ok(())
        }
        async fn disconnect(&self) {
            self.disconnected.store(true, Ordering::SeqCst);
        }
    }

    fn test_socket(transport: Arc<dyn Transport>) -> NoiseSocket {
        let key = [0x11u8; 32];
        NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport,
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
        )
    }

    /// Transport failure *before* anything is written: every later send on the
    /// same connection must be refused, so no second frame can be encrypted
    /// under the counter the failed frame consumed.
    #[tokio::test]
    async fn send_error_before_write_poisons_the_sender() {
        let transport = Arc::new(crate::transport::mock::CapturingMockTransport::new());
        transport.fail_next_sends(1);
        let socket = test_socket(transport.clone());

        let first = socket
            .encrypt_and_send(bytes::Bytes::from_static(b"first"))
            .await
            .expect_err("injected transport failure");
        assert!(matches!(first.kind, EncryptSendErrorKind::Transport));

        for attempt in 0..3 {
            let err = socket
                .encrypt_and_send(bytes::Bytes::from_static(b"later"))
                .await
                .expect_err("sends after a transport failure must be refused");
            assert!(
                matches!(err.kind, EncryptSendErrorKind::Poisoned),
                "attempt {attempt} should be rejected as poisoned, got {err:?}"
            );
            assert!(err.is_transport_unavailable(), "must force a reconnect");
        }

        assert_eq!(
            transport.sent_count(),
            0,
            "no frame may reach the wire after the sender is poisoned"
        );
        assert_eq!(transport.failed_sends(), 1, "only the first send was tried");
    }

    /// Transport failure *after* the frame was accepted (the ambiguous case:
    /// the peer may have decrypted it and advanced its read counter). The
    /// sender must still refuse everything that follows.
    #[tokio::test]
    async fn ambiguous_send_error_poisons_the_sender() {
        let transport = Arc::new(AcceptThenFailTransport::new(0));
        let socket = test_socket(transport.clone());

        let first = socket
            .encrypt_and_send(bytes::Bytes::from_static(b"first"))
            .await
            .expect_err("transport reported failure after accepting the frame");
        assert!(matches!(first.kind, EncryptSendErrorKind::Transport));

        let second = socket
            .encrypt_and_send(bytes::Bytes::from_static(b"second"))
            .await
            .expect_err("sends after an ambiguous failure must be refused");
        assert!(matches!(second.kind, EncryptSendErrorKind::Poisoned));

        assert_eq!(
            transport.sent().len(),
            1,
            "exactly the one ambiguous frame reached the transport"
        );
    }

    /// Poisoning the sender is only half a recovery: a write can fail while the
    /// read half stays open, and then nothing tears the connection down. The
    /// sender must close the transport so the existing disconnect path
    /// reconnects, instead of leaving a client that looks connected and cannot
    /// send.
    #[tokio::test]
    async fn poisoning_the_sender_closes_the_transport() {
        let transport: Arc<AcceptThenFailTransport> = Arc::new(AcceptThenFailTransport::new(0));
        let socket = test_socket(transport.clone());

        let first = socket
            .encrypt_and_send(bytes::Bytes::from_static(b"first"))
            .await;
        assert!(first.is_err(), "the injected failure must surface");
        assert!(
            transport.disconnected(),
            "the first transport error must close the transport so the client reconnects"
        );
    }

    /// Drives the per-frame primitive directly against an always-failing
    /// transport: even without the sender-task poison flag, a counter value is
    /// never handed to the cipher twice. Proven by decrypting the two captured
    /// frames with counters 0 and 1 - reuse would make the second decrypt fail.
    #[tokio::test]
    async fn write_counter_is_never_reused_after_a_send_error() {
        let key = [0x33u8; 32];
        let transport: Arc<AcceptThenFailTransport> = Arc::new(AcceptThenFailTransport::new(0));
        let transport_dyn: Arc<dyn Transport> = transport.clone();
        let runtime: Arc<dyn Runtime> = Arc::new(crate::runtime_impl::TokioRuntime);
        let write_key = Arc::new(NoiseCipher::new(&key).expect("32-byte key"));

        let mut write_counter: u32 = 0;
        let mut enc_buf = Vec::new();
        let mut out_buf = BytesMut::new();

        for expected_counter in 0..2u32 {
            assert_eq!(write_counter, expected_counter);
            let err = NoiseSocket::process_send_job(
                &runtime,
                &transport_dyn,
                &write_key,
                &mut write_counter,
                bytes::Bytes::from(vec![expected_counter as u8; 32]),
                &mut enc_buf,
                &mut out_buf,
                None,
            )
            .await
            .expect_err("transport always fails");
            assert!(matches!(err.kind, EncryptSendErrorKind::Transport));
            assert_eq!(
                write_counter,
                expected_counter + 1,
                "a failed send must still consume its counter"
            );
        }

        let read_key = NoiseCipher::new(&key).expect("32-byte key");
        for (counter, frame) in transport.sent().into_iter().enumerate() {
            let mut body = frame[FRAME_LENGTH_SIZE..].to_vec();
            read_key
                .decrypt_in_place_with_counter(counter as u32, &mut body)
                .expect("each frame must decrypt under its own distinct counter");
            assert_eq!(body, vec![counter as u8; 32]);
        }
    }

    /// A framing failure is detected before any byte reaches the wire, so it
    /// must not disable the connection the way a transport failure does.
    #[tokio::test]
    async fn framing_error_does_not_poison_the_sender() {
        let transport = Arc::new(crate::transport::mock::CapturingMockTransport::new());
        let socket = test_socket(transport.clone());

        // Ciphertext = payload + 16-byte tag, so this is the smallest payload
        // whose frame no longer fits the 24-bit length prefix.
        let oversize = bytes::Bytes::from(vec![0u8; wacore::framing::FRAME_MAX_SIZE - 16]);
        let err = socket
            .encrypt_and_send(oversize)
            .await
            .expect_err("frame exceeds the 24-bit length prefix");
        assert!(matches!(err.kind, EncryptSendErrorKind::Framing));

        socket
            .encrypt_and_send(bytes::Bytes::from_static(b"still usable"))
            .await
            .expect("a rejected oversize frame must leave the connection usable");
        assert_eq!(transport.sent_count(), 1);
    }

    #[tokio::test]
    async fn test_concurrent_sends_maintain_order() {
        use async_lock::Mutex;
        use async_trait::async_trait;
        use std::sync::Arc;

        // Create a mock transport that records the order of sends by decrypting
        // the first byte (which contains the task index)
        struct RecordingTransport {
            recorded_order: Arc<Mutex<Vec<u8>>>,
            read_key: NoiseCipher,
            counter: AtomicU32,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl Transport for RecordingTransport {
            async fn send(&self, data: bytes::Bytes) -> std::result::Result<(), anyhow::Error> {
                if data.len() > 16 {
                    let mut data = data.to_vec();
                    // Strip the 3-byte frame header, then decrypt in place
                    data.drain(..3);
                    let counter = self.counter.fetch_add(1, Ordering::SeqCst);

                    if self
                        .read_key
                        .decrypt_in_place_with_counter(counter, &mut data)
                        .is_ok()
                        && !data.is_empty()
                    {
                        let index = data[0];
                        let mut order = self.recorded_order.lock().await;
                        order.push(index);
                    }
                }
                Ok(())
            }

            async fn disconnect(&self) {}
        }

        let recorded_order = Arc::new(Mutex::new(Vec::new()));
        let key = [0u8; 32];
        let write_key = NoiseCipher::new(&key).expect("32-byte key should be valid");
        let read_key = NoiseCipher::new(&key).expect("32-byte key should be valid");

        let transport = Arc::new(RecordingTransport {
            recorded_order: recorded_order.clone(),
            read_key: NoiseCipher::new(&key).expect("32-byte key should be valid"),
            counter: AtomicU32::new(0),
        });

        let socket = Arc::new(NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport,
            write_key,
            read_key,
        ));

        // Spawn multiple concurrent sends with their indices
        let mut handles = Vec::new();
        for i in 0..10 {
            let socket = socket.clone();
            handles.push(tokio::spawn(async move {
                // Use index as the first byte of plaintext to identify this send
                let mut plaintext = vec![i as u8];
                plaintext.extend_from_slice(&[0u8; 99]);
                socket.encrypt_and_send(bytes::Bytes::from(plaintext)).await
            }));
        }

        // Wait for all sends to complete
        for handle in handles {
            let result = handle.await.expect("task should complete");
            assert!(result.is_ok(), "All sends should succeed");
        }

        // Verify all sends completed in FIFO order (0, 1, 2, ..., 9)
        let order = recorded_order.lock().await;
        let expected: Vec<u8> = (0..10).collect();
        assert_eq!(*order, expected, "Sends should maintain FIFO order");
    }

    /// Tests that the encrypted buffer sizing formula (plaintext.len() + 32) is sufficient.
    /// This verifies the optimization in client.rs that sizes the buffer based on payload.
    #[tokio::test]
    async fn test_encrypted_buffer_sizing_is_sufficient() {
        use async_trait::async_trait;
        use std::sync::Arc;
        use std::sync::atomic::{AtomicUsize, Ordering};

        // Transport that records the actual encrypted data size
        struct SizeRecordingTransport {
            last_size: Arc<AtomicUsize>,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl Transport for SizeRecordingTransport {
            async fn send(&self, data: bytes::Bytes) -> std::result::Result<(), anyhow::Error> {
                self.last_size.store(data.len(), Ordering::SeqCst);
                Ok(())
            }
            async fn disconnect(&self) {}
        }

        let last_size = Arc::new(AtomicUsize::new(0));
        let transport = Arc::new(SizeRecordingTransport {
            last_size: last_size.clone(),
        });

        let key = [0u8; 32];
        let write_key = NoiseCipher::new(&key).expect("32-byte key should be valid");
        let read_key = NoiseCipher::new(&key).expect("32-byte key should be valid");

        let socket = NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport,
            write_key,
            read_key,
        );

        // Test various payload sizes: tiny, small, medium, large, very large
        let test_sizes = [0, 1, 50, 100, 500, 1000, 1024, 2000, 5000, 16384, 20000];

        for size in test_sizes {
            let plaintext = vec![0xABu8; size];
            let result = socket
                .encrypt_and_send(bytes::Bytes::from(plaintext.clone()))
                .await;

            assert!(
                result.is_ok(),
                "encrypt_and_send should succeed for payload size {}",
                size
            );

            let actual_encrypted_size = last_size.load(Ordering::SeqCst);

            // Verify the actual encrypted size fits within our allocated capacity
            // Encrypted size = plaintext + 16 (AES-GCM tag) + 3 (frame header) = plaintext + 19
            let expected_max = size + 19;
            assert_eq!(
                actual_encrypted_size, expected_max,
                "Encrypted size for {} byte payload should be {} (got {})",
                size, expected_max, actual_encrypted_size
            );
        }
    }

    /// Locks the SessionStats wire accounting to the transport truth: bytes
    /// counted must equal the frames the transport actually saw.
    #[tokio::test]
    async fn session_stats_match_transport_bytes() {
        let factory = crate::transport::mock::CapturingMockTransportFactory::new();
        let transport = factory.transport();
        let key = [0u8; 32];
        let stats = Arc::new(wacore::stats::SessionStats::new());

        let socket = NoiseSocket::with_stats(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport.clone(),
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
            Some(stats.clone()),
        );

        for size in [0usize, 100, 5000] {
            socket
                .encrypt_and_send(bytes::Bytes::from(vec![0u8; size]))
                .await
                .expect("send");
        }

        let sent = transport.sent();
        let wire_total: usize = sent.iter().map(|f| f.len()).sum();
        let snap = stats.snapshot();
        assert_eq!(snap.frames_sent, sent.len() as u64);
        assert_eq!(snap.bytes_sent, wire_total as u64);
        assert!(stats.first_send_since_recv_ms() > 0);
    }

    /// Tests edge cases for buffer sizing
    #[tokio::test]
    async fn test_encrypted_buffer_sizing_edge_cases() {
        use async_trait::async_trait;
        use std::sync::Arc;

        struct NoOpTransport;

        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl Transport for NoOpTransport {
            async fn send(&self, _data: bytes::Bytes) -> std::result::Result<(), anyhow::Error> {
                Ok(())
            }
            async fn disconnect(&self) {}
        }

        let transport = Arc::new(NoOpTransport);
        let key = [0u8; 32];
        let write_key = NoiseCipher::new(&key).expect("32-byte key should be valid");
        let read_key = NoiseCipher::new(&key).expect("32-byte key should be valid");

        let socket = NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport,
            write_key,
            read_key,
        );

        // Test empty payload
        let result = socket.encrypt_and_send(bytes::Bytes::new()).await;
        assert!(result.is_ok(), "Empty payload should encrypt successfully");

        // Test payload at inline threshold boundary (16KB)
        let at_threshold = bytes::Bytes::from(vec![0u8; 16 * 1024]);
        let result = socket.encrypt_and_send(at_threshold).await;
        assert!(
            result.is_ok(),
            "Payload at inline threshold should encrypt successfully"
        );

        // Test payload just above inline threshold
        let above_threshold = bytes::Bytes::from(vec![0u8; 16 * 1024 + 1]);
        let result = socket.encrypt_and_send(above_threshold).await;
        assert!(
            result.is_ok(),
            "Payload above inline threshold should encrypt successfully"
        );
    }
}
