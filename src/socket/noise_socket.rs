use crate::socket::error::{EncryptSendError, EncryptSendErrorKind, Result, SocketError};
use crate::transport::Transport;
use async_channel;
use bytes::BytesMut;
use futures::channel::oneshot;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use wacore::handshake::{NoiseCipher, NoiseError};
use wacore::libsignal::crypto::GcmInPlaceBuffer;
use wacore::runtime::{AbortHandle, Runtime};

const INLINE_ENCRYPT_THRESHOLD: usize = 16 * 1024;

/// AES-GCM tag length. A frame's wire size is a fixed function of its plaintext
/// length, which is what lets the length prefix be written before the ciphertext
/// exists.
const TAG_LEN: usize = 16;

/// The region of the batch buffer one frame's ciphertext occupies, exposed to
/// AES-GCM as if it were a buffer of its own.
///
/// Sealing through this view puts the ciphertext and its tag straight where the
/// transport will read them. The alternative, sealing into scratch space and
/// copying the result in, costs a second full pass over every byte sent, which
/// is the copy comparable stacks are built to avoid: quinn seals with
/// `PacketKey::encrypt(&self, packet, buf, header_len)` directly in the datagram
/// buffer, and rustls encrypts each fragment into the record it will send.
struct FrameBody<'a> {
    out: &'a mut BytesMut,
    /// Offset in `out` where this frame's ciphertext starts, i.e. just past its
    /// length prefix. Held as an offset rather than a slice so the AEAD can grow
    /// the buffer by the tag through the same view.
    base: usize,
}

impl GcmInPlaceBuffer for FrameBody<'_> {
    fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.out[self.base..]
    }

    fn as_slice(&self) -> &[u8] {
        &self.out[self.base..]
    }

    fn resize(&mut self, new_len: usize, value: u8) {
        self.out.resize(self.base + new_len, value);
    }

    fn truncate(&mut self, len: usize) {
        self.out.truncate(self.base + len);
    }
}

/// Ceilings on one batched write. They bound how much is buffered before the
/// first frame reaches the socket; the batch never waits for work, so these
/// only matter when a burst is already queued.
const MAX_BATCH_FRAMES: usize = 16;
const MAX_BATCH_WIRE_BYTES: usize = 64 * 1024;

/// What the batch buffer holds between bursts: a few small stanzas coalesced.
///
/// One large stanza grows the buffer to its own size, and the allocation then
/// outlives the burst that needed it, so a single media-sized send costs the
/// session tens of KiB for the rest of the connection. Measured at 60 KiB
/// resident per socket after one 60 KiB frame, against 8 KiB for a socket that
/// only ever sends small ones.
const OUT_BUF_IDLE_CAPACITY: usize = 4096;

/// Consecutive small batches that mark a burst as finished. Only then is the
/// grown buffer released, so a burst spread over several batches is never
/// interrupted to reallocate mid-flight.
const SMALL_BATCHES_BEFORE_SHRINK: usize = 32;

/// Whether the batch buffer should be swapped for an idle-sized one, advancing
/// the burst-tracking state.
///
/// A free function because the buffer lives inside the sender task, where no
/// test can reach it: the decision is only checkable if it is separable from
/// the loop that acts on it.
///
/// `buffer_capacity` is read as well as the wire length because a frame that
/// fails mid-encrypt is truncated back out of the buffer, leaving the
/// allocation it grew with no wire bytes to notice it by. Only large traffic
/// counts as the burst still running, though — a merely large buffer must not
/// keep resetting the countdown that releases it.
fn should_release_batch_buffer(
    batch_wire_len: usize,
    buffer_capacity: usize,
    grown: &mut bool,
    small_batches: &mut usize,
) -> bool {
    if batch_wire_len > OUT_BUF_IDLE_CAPACITY || buffer_capacity > OUT_BUF_IDLE_CAPACITY {
        *grown = true;
    }
    if batch_wire_len > OUT_BUF_IDLE_CAPACITY {
        *small_batches = 0;
        return false;
    }
    // An empty batch wrote nothing, so it is not evidence of anything.
    if !*grown || batch_wire_len == 0 {
        return false;
    }
    *small_batches += 1;
    if *small_batches < SMALL_BATCHES_BEFORE_SHRINK {
        return false;
    }
    *grown = false;
    *small_batches = 0;
    true
}

/// Result type for send operations.
type SendResult = std::result::Result<(), EncryptSendError>;

/// Wire size a plaintext will occupy once encrypted and framed: the AES-GCM tag
/// plus the length prefix. Used to test a queued frame against the batch ceiling
/// before paying to encrypt it.
fn frame_wire_len(plaintext_len: usize) -> usize {
    plaintext_len + TAG_LEN + wacore::framing::FRAME_LENGTH_SIZE
}

/// One batched write's failure, handed to every waiter in that batch.
///
/// `anyhow::Error` is not `Clone`, so a shared reference is what lets all the
/// callers see the real cause instead of a re-worded copy.
#[derive(Debug)]
struct SharedSendFailure(Arc<EncryptSendError>);

impl std::fmt::Display for SharedSendFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for SharedSendFailure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        std::error::Error::source(self.0.as_ref())
    }
}

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
        // BytesMut: split().freeze() yields a zero-copy Bytes while retaining
        // the underlying allocation for the next frame.
        let mut out_buf = BytesMut::with_capacity(OUT_BUF_IDLE_CAPACITY);
        // Whether `out_buf` is still holding an allocation a burst grew, and how
        // many small batches have gone out since.
        let mut out_buf_grown = false;
        let mut small_batches: usize = 0;
        // A failed transport write says nothing about how much of the frame the
        // peer received, so the counter that frame consumed can neither be
        // reused (nonce reuse under the same write key) nor confidently skipped
        // (the peer's read counter would desync). Both outcomes are unrecoverable
        // in-band, so the whole sender goes out of service and the connection
        // must be re-established with a fresh handshake key.
        let mut poisoned = false;
        // Reused across batches: one allocation for the life of the connection
        // instead of one per batch.
        let mut waiters: Vec<(oneshot::Sender<SendResult>, usize)> = Vec::new();
        // A job pulled off the channel that would have overflowed the byte
        // ceiling, held over to open the next batch. Dropping it (on shutdown)
        // drops its response channel, which the caller sees as a closed sender:
        // a held-over job can be lost, but it can never hang its caller.
        let mut carry_over: Option<SendJob> = None;

        loop {
            let job = match carry_over.take() {
                Some(job) => job,
                None => match send_job_rx.recv().await {
                    Ok(job) => job,
                    Err(_) => break,
                },
            };
            if poisoned {
                let _ = job.response_tx.send(Err(EncryptSendError::poisoned()));
                continue;
            }

            // Encrypt everything already queued into one buffer and write it
            // once. Three independent producers answer a single inbound message
            // (the reply, the delivery receipt and the stanza ack), so a write
            // per frame turned into a syscall, a TLS record and a WebSocket
            // message per frame. Only frames that are ALREADY waiting are taken:
            // never block for more, or this trades syscalls for latency.
            waiters.clear();
            let mut encrypt_failure: Option<(oneshot::Sender<SendResult>, EncryptSendError)> = None;
            let mut job = job;
            loop {
                let response_tx = job.response_tx;
                match Self::encrypt_frame_into(
                    &runtime,
                    &write_key,
                    &mut write_counter,
                    job.plaintext,
                    &mut out_buf,
                )
                .await
                {
                    Ok(wire_bytes) => waiters.push((response_tx, wire_bytes)),
                    Err(e) => {
                        // The counter is untouched on this frame, and every
                        // frame already in the buffer must still go out so the
                        // peer's counters stay contiguous.
                        encrypt_failure = Some((response_tx, e));
                        break;
                    }
                }
                if out_buf.len() >= MAX_BATCH_WIRE_BYTES || waiters.len() >= MAX_BATCH_FRAMES {
                    break;
                }
                match send_job_rx.try_recv() {
                    Ok(next) => {
                        // Check the ceiling before appending, not after, or a
                        // nearly-full batch overshoots it by a whole frame. A
                        // frame that cannot fit any batch still goes alone
                        // rather than deadlocking against the ceiling.
                        let projected = out_buf.len() + frame_wire_len(next.plaintext.len());
                        if projected > MAX_BATCH_WIRE_BYTES {
                            carry_over = Some(next);
                            break;
                        }
                        job = next;
                    }
                    Err(_) => break,
                }
            }

            let mut batch_wire_len = 0usize;
            let outcome = if out_buf.is_empty() {
                Ok(())
            } else {
                // Zero-copy: split() hands the written bytes over and out_buf
                // keeps its capacity for the next batch.
                let wire = out_buf.split().freeze();
                batch_wire_len = wire.len();
                if waiters.len() > 1 {
                    // The only externally visible sign that a batch happened.
                    // Without it, "does the peer accept several frames in one
                    // WebSocket message?" cannot be answered from a live run.
                    log::debug!(
                        "noise: coalesced {} frames into one {}-byte write",
                        waiters.len(),
                        wire.len()
                    );
                }
                match transport.send(wire).await {
                    Ok(()) => {
                        if let Some(stats) = stats.as_deref() {
                            for (_, wire_bytes) in &waiters {
                                stats.record_frame_sent(*wire_bytes);
                            }
                        }
                        Ok(())
                    }
                    Err(e) => Err(EncryptSendError::transport(e)),
                }
            };

            {
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
            }

            // Every frame in this batch shares the fate of the single write.
            match outcome {
                Ok(()) => {
                    for (response_tx, _) in waiters.drain(..) {
                        let _ = response_tx.send(Ok(()));
                    }
                }
                // One waiter owns the failure outright. This is the overwhelmingly
                // common case, and handing over the error untouched is what keeps
                // `err.source.downcast_ref::<MyTransportError>()` working for a
                // caller with its own Transport: wrapping would bury the typed
                // error one level down for no benefit, since there is nobody to
                // share it with.
                Err(err) if waiters.len() == 1 => {
                    let (response_tx, _) = waiters.drain(..).next().expect("length checked");
                    let _ = response_tx.send(Err(err));
                }
                // Several waiters, and EncryptSendError is not Clone: they share
                // one Arc. Display renders only the kind, so re-wording per waiter
                // would hand each caller "transport error" with the cause gone;
                // sharing keeps the whole chain reachable for a refcount bump each.
                Err(err) => {
                    let shared = Arc::new(err);
                    for (response_tx, _) in waiters.drain(..) {
                        let _ = response_tx.send(Err(EncryptSendError::transport(
                            SharedSendFailure(shared.clone()),
                        )));
                    }
                }
            }
            if let Some((response_tx, err)) = encrypt_failure {
                let _ = response_tx.send(Err(err));
            }

            // Release the grown allocation once the burst is over. Nothing above
            // reads `out_buf` across iterations — `split()` left it empty — so
            // replacing it here cannot affect a batching decision.
            if should_release_batch_buffer(
                batch_wire_len,
                out_buf.capacity(),
                &mut out_buf_grown,
                &mut small_batches,
            ) {
                out_buf = BytesMut::with_capacity(OUT_BUF_IDLE_CAPACITY);
            }
        }
    }

    /// Encrypt one plaintext and append the framed result to `out_buf`,
    /// returning its wire size. The counter is burned once the framed ciphertext
    /// is committed to `out_buf`, whether or not the write that carries it
    /// succeeds. Every error path leaves `out_buf` exactly as it found it, which
    /// is the only reason leaving the counter unburned there is sound: a change
    /// that keeps partial output must burn the counter too, or the next frame
    /// reuses its nonce.
    async fn encrypt_frame_into(
        runtime: &Arc<dyn Runtime>,
        write_key: &Arc<NoiseCipher>,
        write_counter: &mut u32,
        plaintext: bytes::Bytes,
        out_buf: &mut BytesMut,
    ) -> std::result::Result<usize, EncryptSendError> {
        let counter = *write_counter;
        // Refuse to wrap the per-direction frame counter: reusing an AES-GCM
        // nonce under the same key is catastrophic. 2^32 frames per connection
        // is unreachable in practice, so erroring here forces a reconnect
        // rather than a silent nonce reuse.
        if counter == u32::MAX {
            return Err(EncryptSendError::crypto(NoiseError::CounterExhausted));
        }
        let before = out_buf.len();

        if plaintext.len() <= INLINE_ENCRYPT_THRESHOLD {
            // Ciphertext is exactly the plaintext plus the tag, so the length
            // prefix is known before the bytes it counts exist and the frame can
            // be sealed where it already sits in the batch.
            let body_len = plaintext.len() + TAG_LEN;
            if let Err(e) = wacore::framing::append_frame_header_into(body_len, None, out_buf) {
                return Err(EncryptSendError::framing(e));
            }
            let base = out_buf.len();
            out_buf.extend_from_slice(&plaintext);
            if let Err(e) = write_key
                .encrypt_in_place_with_counter(counter, &mut FrameBody { out: out_buf, base })
            {
                // Unlike the paths above, this one has already appended the
                // prefix and the plaintext. Rolling both back is what keeps the
                // rest of the batch, which still has to go out, contiguous, and
                // what keeps this counter safe to hand to the next frame. The
                // default AEAD cannot fail on a fixed-size key and nonce, so
                // only a `set_crypto_provider` backend reaches this: it is the
                // contract for those, not dead code.
                out_buf.truncate(before);
                return Err(EncryptSendError::crypto(e));
            }
            // The length prefix was written from `plaintext.len() + TAG_LEN`
            // before the ciphertext existed, which is sound only because
            // `TransportAead` is AES-256-GCM by contract. A `set_crypto_provider`
            // backend that grows the buffer by anything else would put a frame
            // on the wire whose prefix disagrees with its body, desyncing the
            // peer's parser for the rest of the connection. Checking costs one
            // comparison and turns that into a refused send.
            if out_buf.len() - base != body_len {
                out_buf.truncate(before);
                return Err(EncryptSendError::crypto(NoiseError::Encrypt(
                    wacore::libsignal::crypto::CryptoProviderError::BackendFailed,
                )));
            }
        } else {
            let write_key = write_key.clone();
            // `Bytes` is Send + 'static: move it into the blocking task (a
            // refcount bump) instead of copying the whole >16KB plaintext.
            let encrypt_result = wacore::runtime::blocking(&**runtime, move || {
                write_key.encrypt_with_counter(counter, &plaintext)
            })
            .await;
            let ciphertext = match encrypt_result {
                Ok(c) => c,
                Err(e) => return Err(EncryptSendError::crypto(e)),
            };
            if let Err(e) = wacore::framing::append_frame_into(&ciphertext, None, out_buf) {
                return Err(EncryptSendError::framing(e));
            }
        }

        *write_counter = counter + 1;
        Ok(out_buf.len() - before)
    }

    /// Hands `plaintext` to the sender task and returns the channel its result
    /// will arrive on, without waiting for it.
    ///
    /// Split out of [`Self::encrypt_and_send`] so a burst can enqueue every
    /// frame before awaiting any of them. The sender coalesces whatever is
    /// already queued into one transport write, so a caller that awaited each
    /// frame before enqueueing the next would hand them over one completion
    /// apart and get one write per frame.
    ///
    /// The returned receiver must be awaited, or the result is dropped and the
    /// caller cannot tell a delivered frame from a failed one.
    pub(crate) async fn enqueue_send(
        &self,
        plaintext: bytes::Bytes,
    ) -> std::result::Result<oneshot::Receiver<SendResult>, EncryptSendError> {
        let (response_tx, response_rx) = oneshot::channel();

        let job = SendJob {
            plaintext,
            response_tx,
        };

        // Send job to the sender task. If channel is closed, sender task has stopped.
        if let Err(_send_err) = self.send_job_tx.send(job).await {
            return Err(EncryptSendError::channel_closed());
        }

        Ok(response_rx)
    }

    /// Awaits a receiver handed out by [`Self::enqueue_send`].
    pub(crate) async fn await_send(receiver: oneshot::Receiver<SendResult>) -> SendResult {
        match receiver.await {
            Ok(result) => result,
            Err(_) => {
                // Sender task dropped without sending a response
                Err(EncryptSendError::channel_closed())
            }
        }
    }

    pub async fn encrypt_and_send(&self, plaintext: bytes::Bytes) -> SendResult {
        let receiver = self.enqueue_send(plaintext).await?;
        Self::await_send(receiver).await
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
    use std::future::Future;
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

    /// Drives the per-frame primitive directly: every frame burns exactly one
    /// counter at encrypt time, whether or not the write that carries it ever
    /// succeeds. Proven by decrypting the two frames with counters 0 and 1 -
    /// reuse would make the second decrypt fail.
    #[tokio::test]
    async fn every_encrypted_frame_burns_its_own_counter() {
        let key = [0x33u8; 32];
        let runtime: Arc<dyn Runtime> = Arc::new(crate::runtime_impl::TokioRuntime);
        let write_key = Arc::new(NoiseCipher::new(&key).expect("32-byte key"));

        let mut write_counter: u32 = 0;
        let mut out_buf = BytesMut::new();

        for expected_counter in 0..2u32 {
            assert_eq!(write_counter, expected_counter);
            NoiseSocket::encrypt_frame_into(
                &runtime,
                &write_key,
                &mut write_counter,
                bytes::Bytes::from(vec![expected_counter as u8; 32]),
                &mut out_buf,
            )
            .await
            .expect("encrypt must succeed");
            assert_eq!(
                write_counter,
                expected_counter + 1,
                "each frame must consume its counter at encrypt time"
            );
        }

        let read_key = NoiseCipher::new(&key).expect("32-byte key");
        for (counter, frame) in split_frames(&out_buf).into_iter().enumerate() {
            let mut body = frame;
            read_key
                .decrypt_in_place_with_counter(counter as u32, &mut body)
                .expect("each frame must decrypt under its own distinct counter");
            assert_eq!(body, vec![counter as u8; 32]);
        }
    }

    /// The frame is sealed straight into the batch buffer, so the offset it is
    /// sealed at is load-bearing in a way a staging copy never was: too low and
    /// AES-GCM overwrites the length prefix or the frame before it, too high and
    /// the plaintext leaks past the ciphertext. Pinned by encrypting a second
    /// frame behind a first and checking the first is untouched, the header
    /// counts exactly the ciphertext, and the body decrypts under its counter.
    #[tokio::test]
    async fn a_frame_is_sealed_in_place_behind_the_one_before_it() {
        let key = [0x21u8; 32];
        let runtime: Arc<dyn Runtime> = Arc::new(crate::runtime_impl::TokioRuntime);
        let write_key = Arc::new(NoiseCipher::new(&key).expect("32-byte key"));
        let mut write_counter: u32 = 0;
        let mut out_buf = BytesMut::new();

        let first = bytes::Bytes::from(vec![0xA1u8; 40]);
        NoiseSocket::encrypt_frame_into(
            &runtime,
            &write_key,
            &mut write_counter,
            first,
            &mut out_buf,
        )
        .await
        .expect("first frame");
        let first_frame = out_buf.to_vec();

        let second_plain = vec![0xB2u8; 77];
        let wire_len = NoiseSocket::encrypt_frame_into(
            &runtime,
            &write_key,
            &mut write_counter,
            bytes::Bytes::from(second_plain.clone()),
            &mut out_buf,
        )
        .await
        .expect("second frame");

        assert_eq!(
            &out_buf[..first_frame.len()],
            &first_frame[..],
            "sealing the second frame must not reach back into the first"
        );
        assert_eq!(wire_len, frame_wire_len(second_plain.len()));
        assert_eq!(out_buf.len(), first_frame.len() + wire_len);

        let second = &out_buf[first_frame.len()..];
        let declared =
            ((second[0] as usize) << 16) | ((second[1] as usize) << 8) | second[2] as usize;
        assert_eq!(
            declared,
            second_plain.len() + TAG_LEN,
            "the header must count the ciphertext that was sealed after it"
        );

        let read_key = NoiseCipher::new(&key).expect("32-byte key");
        let mut body = BytesMut::from(&second[FRAME_LENGTH_SIZE..]);
        read_key
            .decrypt_in_place_with_counter(1, &mut body)
            .expect("the sealed body must authenticate under its own counter");
        assert_eq!(&body[..], &second_plain[..]);
    }

    /// The AEAD grows and shrinks the buffer through this view, so every one of
    /// its operations has to be relative to the frame's own start. An absolute
    /// `resize` or `truncate` here would silently eat the frames already staged
    /// for the same write.
    #[test]
    fn the_frame_body_view_never_reaches_before_its_own_frame() {
        let mut out = BytesMut::from(&b"earlier-frame"[..]);
        let base = out.len();
        out.extend_from_slice(b"body");

        let mut view = FrameBody {
            out: &mut out,
            base,
        };
        assert_eq!(view.as_slice(), b"body");
        assert_eq!(view.len(), 4);
        view.as_mut_slice()[0] = b'B';

        // Growing by a tag-sized amount, the way sealing does.
        view.resize(4 + TAG_LEN, 0);
        assert_eq!(view.len(), 4 + TAG_LEN);
        view.truncate(4);
        assert_eq!(view.as_slice(), b"Body");

        assert_eq!(
            &out[..base],
            &b"earlier-frame"[..],
            "no view operation may touch the bytes staged before this frame"
        );
    }

    /// A frame that cannot be encrypted must leave the batch buffer byte for byte
    /// as it found it: the frames already in it still have to reach the wire, and
    /// the counter it declined to burn is handed to whoever comes next. Counter
    /// exhaustion is the failure that is reachable without swapping the process
    /// wide crypto provider.
    #[tokio::test]
    async fn a_failed_frame_leaves_the_batch_buffer_byte_identical() {
        let key = [0x22u8; 32];
        let runtime: Arc<dyn Runtime> = Arc::new(crate::runtime_impl::TokioRuntime);
        let write_key = Arc::new(NoiseCipher::new(&key).expect("32-byte key"));
        let mut out_buf = BytesMut::new();

        // One frame already staged, then the counter runs out mid-batch.
        let mut write_counter: u32 = u32::MAX - 1;
        NoiseSocket::encrypt_frame_into(
            &runtime,
            &write_key,
            &mut write_counter,
            bytes::Bytes::from(vec![0xC3u8; 24]),
            &mut out_buf,
        )
        .await
        .expect("the last usable counter must still encrypt");
        let staged = out_buf.to_vec();
        assert_eq!(write_counter, u32::MAX);

        let err = NoiseSocket::encrypt_frame_into(
            &runtime,
            &write_key,
            &mut write_counter,
            bytes::Bytes::from(vec![0xD4u8; 24]),
            &mut out_buf,
        )
        .await
        .expect_err("an exhausted counter must not wrap");
        assert!(matches!(err.kind, EncryptSendErrorKind::Crypto));
        assert_eq!(
            out_buf.to_vec(),
            staged,
            "the rejected frame must not leave a header or a plaintext behind"
        );
        assert_eq!(write_counter, u32::MAX, "a rejected frame burns no counter");

        // The staged frame is intact and complete, not just the right length.
        let read_key = NoiseCipher::new(&key).expect("32-byte key");
        let mut body = BytesMut::from(&staged[FRAME_LENGTH_SIZE..]);
        read_key
            .decrypt_in_place_with_counter(u32::MAX - 1, &mut body)
            .expect("the frame staged before the failure must still be sendable");
        assert_eq!(&body[..], &[0xC3u8; 24][..]);
    }

    /// Order must survive a full job channel, not just an empty one.
    ///
    /// A burst larger than the channel leaves some sends parked waiting for a
    /// slot, and the whole ordering guarantee (`send_raw_bytes_burst` promises
    /// arrival order, and the ack worker relies on it) then rests on those
    /// parked senders being woken in the order they queued. Frame N decrypts
    /// only under counter N, so any reordering fails here.
    #[tokio::test]
    async fn order_survives_a_full_job_channel() {
        let key = [0x88u8; 32];
        let transport = GatedTransport::closed();
        let runtime: Arc<dyn Runtime> = Arc::new(crate::runtime_impl::TokioRuntime);
        let socket = Arc::new(NoiseSocket::new(
            runtime,
            transport.clone(),
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
        ));

        // Comfortably past the channel's capacity, so later sends must park.
        const FRAMES: usize = 20;
        let sends: Vec<BoxSend> = (0..FRAMES)
            .map(|i| {
                let socket = socket.clone();
                Box::pin(async move {
                    socket
                        .encrypt_and_send(bytes::Bytes::from(vec![i as u8; 32]))
                        .await
                }) as BoxSend
            })
            .collect();
        let mut joined = futures::future::join_all(sends);
        assert!(
            futures::FutureExt::now_or_never(&mut joined).is_none(),
            "the gate is closed, so nothing can have completed"
        );

        transport.gate.add_permits(FRAMES);
        for result in joined.await {
            result.expect("send must succeed");
        }

        let read_key = NoiseCipher::new(&key).expect("32-byte key");
        let bodies: Vec<Vec<u8>> = transport
            .writes()
            .iter()
            .flat_map(|w| split_frames(w))
            .collect();
        assert_eq!(bodies.len(), FRAMES, "every frame must reach the wire");
        for (counter, mut body) in bodies.into_iter().enumerate() {
            read_key
                .decrypt_in_place_with_counter(counter as u32, &mut body)
                .expect("a frame written out of counter order cannot authenticate");
            // Decrypting alone would not catch a reorder: jobs that woke out of
            // FIFO order would be encrypted in that order too, so their
            // counters would still line up. The payload is what pins it -
            // unlike the concurrent-producer test, these sends are polled in
            // order by one joined future, so submission order is deterministic.
            assert_eq!(
                body,
                vec![counter as u8; 32],
                "frame {counter} must carry the payload submitted at position {counter}"
            );
        }
    }

    /// A single-frame send hands its caller the transport's own error, not a
    /// wrapper. Callers with a custom `Transport` downcast to their own error
    /// type to decide whether a failure is retryable, and `downcast_ref` looks
    /// at the concrete type rather than walking the chain, so wrapping the
    /// common case would silently break that.
    #[tokio::test]
    async fn a_lone_waiter_gets_the_transport_error_untouched() {
        #[derive(Debug)]
        struct TypedTransportError;
        impl std::fmt::Display for TypedTransportError {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "typed transport error")
            }
        }
        impl std::error::Error for TypedTransportError {}

        struct TypedFailTransport;

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl Transport for TypedFailTransport {
            async fn send(&self, _data: bytes::Bytes) -> std::result::Result<(), anyhow::Error> {
                Err(anyhow::Error::new(TypedTransportError))
            }
            async fn disconnect(&self) {}
        }

        let key = [0x77u8; 32];
        let runtime: Arc<dyn Runtime> = Arc::new(crate::runtime_impl::TokioRuntime);
        let socket = NoiseSocket::new(
            runtime,
            Arc::new(TypedFailTransport),
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
        );

        let err = socket
            .encrypt_and_send(bytes::Bytes::from(vec![9u8; 32]))
            .await
            .expect_err("the transport always fails");

        assert!(matches!(err.kind, EncryptSendErrorKind::Transport));
        assert!(
            err.source.downcast_ref::<TypedTransportError>().is_some(),
            "a lone waiter must receive the transport's own error type, got: {:?}",
            err.source
        );
    }

    /// The byte ceiling must hold across a burst. Checking it after appending
    /// would let a nearly-full batch overshoot by a whole frame, which for a
    /// large stanza is the difference between a bounded buffer and an unbounded
    /// one.
    #[tokio::test]
    async fn a_batch_never_overshoots_the_byte_ceiling() {
        let key = [0x66u8; 32];
        let transport = GatedTransport::closed();
        let runtime: Arc<dyn Runtime> = Arc::new(crate::runtime_impl::TokioRuntime);
        let socket = Arc::new(NoiseSocket::new(
            runtime,
            transport.clone(),
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
        ));

        // Sized so three fit under the ceiling and the fourth cannot: the batch
        // has to stop and hold it over rather than append it.
        const FRAME_BYTES: usize = 20 * 1024;
        const FRAMES: usize = 5;
        let mut sends = queue_all(
            &socket,
            (0..FRAMES).map(|i| bytes::Bytes::from(vec![i as u8; FRAME_BYTES])),
        );
        transport.gate.add_permits(FRAMES);
        for result in (&mut sends).await {
            result.expect("send must succeed");
        }

        let writes = transport.writes();
        for write in &writes {
            let frames = split_frames(write);
            assert!(
                frames.len() == 1 || write.len() <= MAX_BATCH_WIRE_BYTES,
                "a multi-frame write must respect the ceiling: {} bytes in {} frames",
                write.len(),
                frames.len()
            );
        }
        assert!(
            writes.iter().any(|w| split_frames(w).len() > 1),
            "the burst must still coalesce, otherwise this proves nothing"
        );

        let read_key = NoiseCipher::new(&key).expect("32-byte key");
        let bodies: Vec<Vec<u8>> = writes.iter().flat_map(|w| split_frames(w)).collect();
        assert_eq!(bodies.len(), FRAMES, "a held-over frame must still be sent");
        for (counter, mut body) in bodies.into_iter().enumerate() {
            read_key
                .decrypt_in_place_with_counter(counter as u32, &mut body)
                .expect("holding a frame over must not disturb counter order");
        }
    }

    /// The transport's own error must survive the hop to every caller. It cannot
    /// be cloned, and `EncryptSendError`'s Display renders only the kind, so a
    /// naive rebuild silently degrades "connection reset by peer" into
    /// "transport error" and the caller loses the only diagnostic there was.
    #[tokio::test]
    async fn the_transport_cause_reaches_the_caller() {
        let key = [0x55u8; 32];
        let transport: Arc<AcceptThenFailTransport> = Arc::new(AcceptThenFailTransport::new(0));
        let runtime: Arc<dyn Runtime> = Arc::new(crate::runtime_impl::TokioRuntime);
        let socket = NoiseSocket::new(
            runtime,
            transport,
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
        );

        let err = socket
            .encrypt_and_send(bytes::Bytes::from(vec![7u8; 32]))
            .await
            .expect_err("the transport always fails");

        assert!(matches!(err.kind, EncryptSendErrorKind::Transport));
        // `{:#}` walks the anyhow chain; the injected message must still be in it.
        let chain = format!("{:#}", err.source);
        assert!(
            chain.contains("injected failure after accepting the frame"),
            "the transport's cause was lost on the way to the caller: {chain}"
        );
    }

    /// A transport whose writes block until permits are handed out, so a test
    /// can pile jobs into the sender's channel and then release them: the state
    /// batching exists for.
    struct GatedTransport {
        writes: std::sync::Mutex<Vec<bytes::Bytes>>,
        gate: tokio::sync::Semaphore,
    }

    impl GatedTransport {
        fn closed() -> Arc<Self> {
            Arc::new(Self {
                writes: std::sync::Mutex::new(Vec::new()),
                gate: tokio::sync::Semaphore::new(0),
            })
        }

        fn writes(&self) -> Vec<bytes::Bytes> {
            self.writes.lock().expect("writes mutex").clone()
        }
    }

    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl Transport for GatedTransport {
        async fn send(&self, data: bytes::Bytes) -> std::result::Result<(), anyhow::Error> {
            let permit = self.gate.acquire().await.expect("gate open");
            permit.forget();
            self.writes.lock().expect("writes mutex").push(data);
            Ok(())
        }
        async fn disconnect(&self) {}
    }

    /// The wire-level test below cannot see the buffer, so the release decision
    /// is checked here directly: without this, deleting the whole shrink would
    /// leave every other assertion in this file green.
    mod releasing_the_batch_buffer {
        use super::super::{
            OUT_BUF_IDLE_CAPACITY, SMALL_BATCHES_BEFORE_SHRINK, should_release_batch_buffer,
        };

        const BIG: usize = OUT_BUF_IDLE_CAPACITY + 1;
        const SMALL: usize = 64;
        /// A capacity a burst plausibly grew the buffer to, tied to the
        /// threshold so it cannot drift under it.
        const GROWN_CAPACITY: usize = OUT_BUF_IDLE_CAPACITY * 16;

        /// Drives `count` small batches and returns how many asked for a release.
        fn quiet(count: usize, capacity: usize, grown: &mut bool, small: &mut usize) -> usize {
            (0..count)
                .filter(|_| should_release_batch_buffer(SMALL, capacity, grown, small))
                .count()
        }

        #[test]
        fn a_grown_buffer_is_released_once_the_burst_has_been_quiet() {
            let (mut grown, mut small) = (false, 0);
            assert!(!should_release_batch_buffer(BIG, 0, &mut grown, &mut small));
            assert!(grown, "a large batch must mark the buffer grown");

            assert_eq!(
                quiet(SMALL_BATCHES_BEFORE_SHRINK - 1, 0, &mut grown, &mut small),
                0,
                "released before the burst was over"
            );
            assert!(should_release_batch_buffer(
                SMALL, 0, &mut grown, &mut small
            ));
            assert!(!grown, "the release must clear the grown flag");
        }

        /// The countdown is consecutive: traffic that is still large restarts it.
        #[test]
        fn a_large_batch_restarts_the_countdown() {
            let (mut grown, mut small) = (false, 0);
            should_release_batch_buffer(BIG, 0, &mut grown, &mut small);
            quiet(SMALL_BATCHES_BEFORE_SHRINK - 1, 0, &mut grown, &mut small);

            should_release_batch_buffer(BIG, 0, &mut grown, &mut small);

            assert_eq!(
                quiet(SMALL_BATCHES_BEFORE_SHRINK - 1, 0, &mut grown, &mut small),
                0,
                "the countdown carried over across a large batch"
            );
        }

        /// A frame that fails mid-encrypt is truncated out of the buffer, so the
        /// allocation it grew is left with no wire bytes naming it. Capacity is
        /// the only remaining evidence, and it must not itself keep resetting
        /// the countdown or the buffer would never be released at all.
        #[test]
        fn a_buffer_grown_by_a_truncated_frame_is_still_released() {
            let (mut grown, mut small) = (false, 0);
            assert!(!should_release_batch_buffer(
                0,
                GROWN_CAPACITY,
                &mut grown,
                &mut small
            ));
            assert!(grown, "capacity alone must mark the buffer grown");

            assert_eq!(
                quiet(
                    SMALL_BATCHES_BEFORE_SHRINK - 1,
                    GROWN_CAPACITY,
                    &mut grown,
                    &mut small
                ),
                0
            );
            assert!(should_release_batch_buffer(
                SMALL,
                GROWN_CAPACITY,
                &mut grown,
                &mut small
            ));
        }

        /// A socket that never sent anything large must never pay a realloc.
        #[test]
        fn a_buffer_that_never_grew_is_never_released() {
            let (mut grown, mut small) = (false, 0);
            assert_eq!(
                quiet(
                    SMALL_BATCHES_BEFORE_SHRINK * 4,
                    OUT_BUF_IDLE_CAPACITY,
                    &mut grown,
                    &mut small
                ),
                0
            );
            assert!(!grown);
        }
    }

    /// Releasing the grown buffer must be invisible on the wire: a burst after
    /// the shrink has to regrow, coalesce and decrypt exactly as the first one
    /// did, with counter order intact across the whole connection.
    #[tokio::test]
    async fn a_burst_after_the_buffer_shrinks_still_batches_and_round_trips() {
        let key = [0x5au8; 32];
        let transport = GatedTransport::closed();
        let socket = Arc::new(NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport.clone(),
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
        ));

        // A frame big enough to grow the buffer well past its idle capacity.
        const BIG: usize = 40 * 1024;
        const SMALL: usize = 64;
        let mut expected: Vec<Vec<u8>> = Vec::new();

        // Grow, then quieten past the shrink threshold. Sent one at a time so
        // each is its own batch, which is what the threshold counts — and so
        // the order is the send order, which concurrent sends past the
        // channel's capacity would not guarantee.
        transport.gate.add_permits(SMALL_BATCHES_BEFORE_SHRINK + 1);
        for payload in std::iter::once(vec![0xAAu8; BIG])
            .chain((0..SMALL_BATCHES_BEFORE_SHRINK).map(|i| vec![i as u8; SMALL]))
        {
            expected.push(payload.clone());
            socket
                .encrypt_and_send(bytes::Bytes::from(payload))
                .await
                .expect("warm-up send must succeed");
        }

        // The buffer is back to idle size; this burst has to regrow it.
        const BURST: usize = 5;
        const BURST_BYTES: usize = 20 * 1024;
        let payloads: Vec<Vec<u8>> = (0..BURST)
            .map(|i| vec![0xB0 | i as u8; BURST_BYTES])
            .collect();
        expected.extend(payloads.iter().cloned());
        let mut sends = queue_all(&socket, payloads.into_iter().map(bytes::Bytes::from));
        transport.gate.add_permits(BURST);
        for result in (&mut sends).await {
            result.expect("post-shrink send must succeed");
        }

        let writes = transport.writes();
        for write in &writes {
            let frames = split_frames(write);
            assert!(
                frames.len() == 1 || write.len() <= MAX_BATCH_WIRE_BYTES,
                "the ceiling must still hold after a shrink: {} bytes in {} frames",
                write.len(),
                frames.len()
            );
        }
        assert!(
            writes
                .iter()
                .skip_while(|w| split_frames(w).len() <= 1)
                .any(|w| split_frames(w).len() > 1),
            "the regrown buffer must still coalesce, otherwise the shrink cost batching"
        );

        let read_key = NoiseCipher::new(&key).expect("32-byte key");
        let bodies: Vec<Vec<u8>> = writes.iter().flat_map(|w| split_frames(w)).collect();
        assert_eq!(
            bodies.len(),
            expected.len(),
            "every frame must reach the wire"
        );
        for (counter, (mut body, want)) in bodies.into_iter().zip(expected).enumerate() {
            read_key
                .decrypt_in_place_with_counter(counter as u32, &mut body)
                .expect("a shrink must not disturb counter order");
            assert_eq!(body, want, "frame {counter} did not round-trip");
        }
    }

    /// Queues every payload on `socket` and returns the joined sends, still
    /// pending.
    ///
    /// This is the batching tests' precondition: all N frames sitting in the
    /// sender's channel at once. It holds by construction rather than by
    /// waiting - `send` on a channel with room resolves on its first poll, so
    /// polling the joined future once has queued every job - which is why these
    /// tests do not spin on `yield_now` and hope the scheduler cooperated.
    fn queue_all(
        socket: &Arc<NoiseSocket>,
        payloads: impl Iterator<Item = bytes::Bytes>,
    ) -> futures::future::JoinAll<BoxSend> {
        let sends: Vec<BoxSend> = payloads
            .map(|payload| {
                let socket = socket.clone();
                Box::pin(async move { socket.encrypt_and_send(payload).await }) as BoxSend
            })
            .collect();
        let mut joined = futures::future::join_all(sends);
        let queued = futures::FutureExt::now_or_never(&mut joined);
        assert!(
            queued.is_none(),
            "the sends must still be in flight: the transport gate is closed"
        );
        joined
    }

    type BoxSend = std::pin::Pin<Box<dyn Future<Output = SendResult> + Send>>;

    /// Splits a concatenated run of length-prefixed frames into their bodies.
    fn split_frames(mut wire: &[u8]) -> Vec<Vec<u8>> {
        let mut frames = Vec::new();
        while !wire.is_empty() {
            let mut len = 0usize;
            for byte in &wire[..FRAME_LENGTH_SIZE] {
                len = (len << 8) | *byte as usize;
            }
            let body = &wire[FRAME_LENGTH_SIZE..FRAME_LENGTH_SIZE + len];
            frames.push(body.to_vec());
            wire = &wire[FRAME_LENGTH_SIZE + len..];
        }
        frames
    }

    /// Frames queued while a write is in flight leave together in one write, in
    /// counter order, and every caller is answered. Batching is only sound if
    /// all three hold: a lost waiter hangs a send forever, and reordering would
    /// desync the peer's read counter.
    #[tokio::test]
    async fn queued_frames_leave_in_one_write_in_counter_order() {
        let key = [0x44u8; 32];
        let transport = GatedTransport::closed();
        let runtime: Arc<dyn Runtime> = Arc::new(crate::runtime_impl::TokioRuntime);
        let socket = Arc::new(NoiseSocket::new(
            runtime,
            transport.clone(),
            NoiseCipher::new(&key).expect("32-byte key"),
            NoiseCipher::new(&key).expect("32-byte key"),
        ));

        const FRAMES: usize = 5;
        let mut sends = queue_all(
            &socket,
            (0..FRAMES).map(|i| bytes::Bytes::from(vec![i as u8; 32])),
        );
        transport.gate.add_permits(FRAMES);
        for result in (&mut sends).await {
            result.expect("send must succeed");
        }

        let writes = transport.writes();
        assert!(
            writes.len() < FRAMES,
            "queued frames must coalesce, got {} writes for {FRAMES} frames",
            writes.len()
        );

        let read_key = NoiseCipher::new(&key).expect("32-byte key");
        let bodies: Vec<Vec<u8>> = writes.iter().flat_map(|w| split_frames(w)).collect();
        assert_eq!(bodies.len(), FRAMES, "every frame must reach the wire");

        // Decrypting frame N under counter N is the order proof: the counter is
        // the AES-GCM nonce, so a frame written out of order fails to
        // authenticate here.
        let mut payloads = Vec::new();
        for (counter, body) in bodies.into_iter().enumerate() {
            let mut body = body;
            read_key
                .decrypt_in_place_with_counter(counter as u32, &mut body)
                .expect("frames must be written in counter order");
            assert_eq!(body, vec![body[0]; 32], "frame body must survive intact");
            payloads.push(body[0]);
        }

        // Which producer wins which counter is not fixed - the senders race into
        // the channel - so the invariant is that each one's payload is on the
        // wire exactly once, none dropped and none duplicated.
        payloads.sort_unstable();
        let expected: Vec<u8> = (0..FRAMES as u8).collect();
        assert_eq!(
            payloads, expected,
            "each producer's payload must appear exactly once"
        );
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
                // One write can carry several frames: the sender coalesces
                // whatever is already queued, so each write is unpacked frame by
                // frame before decrypting.
                for mut frame in split_frames(&data) {
                    let counter = self.counter.fetch_add(1, Ordering::SeqCst);

                    if self
                        .read_key
                        .decrypt_in_place_with_counter(counter, &mut frame)
                        .is_ok()
                        && !frame.is_empty()
                    {
                        let index = frame[0];
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
