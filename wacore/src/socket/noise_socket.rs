use crate::handshake::{NoiseCipher, NoiseError};
use crate::libsignal::crypto::GcmInPlaceBuffer;
use crate::net::Transport;
use crate::runtime::{AbortHandle, Runtime};
use crate::socket::error::{EncryptSendError, Result, SocketError};
use async_channel;
use bytes::BytesMut;
use futures::channel::oneshot;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

pub const INLINE_ENCRYPT_THRESHOLD: usize = 16 * 1024;

/// AES-GCM tag length. A frame's wire size is a fixed function of its plaintext
/// length, which is what lets the length prefix be written before the ciphertext
/// exists.
pub const TAG_LEN: usize = 16;

/// The region of the batch buffer one frame's ciphertext occupies, exposed to
/// AES-GCM as if it were a buffer of its own.
pub struct FrameBody<'a> {
    pub out: &'a mut BytesMut,
    /// Offset in `out` where this frame's ciphertext starts, i.e. just past its
    /// length prefix. Held as an offset rather than a slice so the AEAD can grow
    /// the buffer by the tag through the same view.
    pub base: usize,
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
pub const MAX_BATCH_FRAMES: usize = 16;
pub const MAX_BATCH_WIRE_BYTES: usize = 64 * 1024;

/// What the batch buffer holds between bursts: a few small stanzas coalesced.
pub const OUT_BUF_IDLE_CAPACITY: usize = 4096;

/// Consecutive small batches that mark a burst as finished. Only then is the
/// grown buffer released, so a burst spread over several batches is never
/// interrupted to reallocate mid-flight.
pub const SMALL_BATCHES_BEFORE_SHRINK: usize = 32;

/// Whether the batch buffer should be swapped for an idle-sized one, advancing
/// the burst-tracking state.
pub fn should_release_batch_buffer(
    batch_wire_len: usize,
    buffer_capacity: usize,
    queue_drained: bool,
    grown: &mut bool,
    small_batches: &mut usize,
) -> bool {
    if batch_wire_len > OUT_BUF_IDLE_CAPACITY || buffer_capacity > OUT_BUF_IDLE_CAPACITY {
        *grown = true;
    }
    // Checked ahead of the size of the batch just written: one large frame
    // followed by silence is precisely the case to release on, and testing the
    // size first would send it down the "burst still running" path forever.
    if *grown && queue_drained {
        *grown = false;
        *small_batches = 0;
        return true;
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
pub type SendResult = std::result::Result<(), EncryptSendError>;

/// Wire size a plaintext will occupy once encrypted and framed: the AES-GCM tag
/// plus the length prefix. Used to test a queued frame against the batch ceiling
/// before paying to encrypt it.
pub fn frame_wire_len(plaintext_len: usize) -> usize {
    plaintext_len + TAG_LEN + crate::framing::FRAME_LENGTH_SIZE
}

/// One batched write's failure, handed to every waiter in that batch.
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
pub struct SendJob {
    pub plaintext: bytes::Bytes,
    pub response_tx: oneshot::Sender<SendResult>,
}

/// Observer for plaintext frames sent over the wire before encryption.
pub trait FrameTap: Send + Sync + 'static {
    /// Whether the tap is currently active.
    fn enabled(&self) -> bool {
        true
    }

    /// Receives a copy of the plaintext frame that was just sent.
    fn publish(&self, plaintext: bytes::Bytes);
}

/// What a socket reports its sends to. Both halves belong to the caller; a
/// light client or VoIP relay socket passes [`Default`], reporting to neither.
#[derive(Default, Clone)]
pub struct SendObservers {
    /// Wire-byte accounting, recorded after the transport write.
    stats: Option<Arc<crate::stats::SessionStats>>,
    /// Publisher for the plaintext of each frame that reached the transport.
    sent_frames: Option<Arc<dyn FrameTap>>,
}

impl SendObservers {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Report wire bytes into `stats` and nothing else.
    #[must_use]
    pub fn with_stats(stats: Arc<crate::stats::SessionStats>) -> Self {
        Self {
            stats: Some(stats),
            sent_frames: None,
        }
    }

    #[must_use]
    pub fn with_sent_frames(mut self, tap: Arc<dyn FrameTap>) -> Self {
        self.sent_frames = Some(tap);
        self
    }

    pub fn stats(&self) -> Option<&Arc<crate::stats::SessionStats>> {
        self.stats.as_ref()
    }

    pub fn sent_frames(&self) -> Option<&Arc<dyn FrameTap>> {
        self.sent_frames.as_ref()
    }
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
    /// if the task is stuck on a slow/hanging network operation, and
    /// explicitly by [`NoiseSocket::abort_sender`] when teardown cannot wait
    /// for the last `Arc` to go.
    sender_task_handle: AbortHandle,
}

impl NoiseSocket {
    pub fn new(
        runtime: Arc<dyn Runtime>,
        transport: Arc<dyn Transport>,
        write_key: NoiseCipher,
        read_key: NoiseCipher,
    ) -> Self {
        Self::with_observers(
            runtime,
            transport,
            write_key,
            read_key,
            SendObservers::default(),
        )
    }

    /// Like [`Self::new`], reporting each send to `observers` (the main WA
    /// session socket passes the client's; VoIP relay sockets and light clients
    /// report to nothing).
    pub fn with_observers(
        runtime: Arc<dyn Runtime>,
        transport: Arc<dyn Transport>,
        write_key: NoiseCipher,
        read_key: NoiseCipher,
        observers: SendObservers,
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
            observers,
        )));

        Self {
            read_key,
            read_counter: Arc::new(AtomicU32::new(0)),
            send_job_tx,
            sender_task_handle,
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
        observers: SendObservers,
    ) {
        let SendObservers { stats, sent_frames } = observers;
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

        // Plaintexts staged for the sent-frame tap, emptied into `tap.publish`
        // only once the transport write succeeds. Kept in a single Vec across
        // batches to avoid per-batch allocation.
        let mut observed: Vec<bytes::Bytes> = Vec::new();

        loop {
            let first_job = match carry_over.take() {
                Some(job) => job,
                None => match send_job_rx.recv().await {
                    Ok(job) => job,
                    Err(_) => break, // Channel closed, exit task
                },
            };

            // Poison check before touching any frame in this batch.
            if poisoned {
                let _ = first_job
                    .response_tx
                    .send(Err(EncryptSendError::poisoned()));
                continue;
            }

            waiters.clear();
            observed.clear();
            out_buf.clear();

            let mut batch_wire_len = 0usize;
            let mut encrypt_failure: Option<(oneshot::Sender<SendResult>, EncryptSendError)> = None;

            let mut job = first_job;
            loop {
                let response_tx = job.response_tx;
                // Cloned before the plaintext is consumed, dropped again if the
                // frame never makes it into the buffer.
                let to_observe = match sent_frames.as_deref() {
                    Some(tap) if tap.enabled() => Some(job.plaintext.clone()),
                    _ => None,
                };
                match Self::encrypt_frame_into(
                    &runtime,
                    &write_key,
                    &mut write_counter,
                    job.plaintext,
                    &mut out_buf,
                )
                .await
                {
                    Ok(wire_len) => {
                        batch_wire_len += wire_len;
                        waiters.push((response_tx, wire_len));
                        if let Some(plaintext) = to_observe {
                            observed.push(plaintext);
                        }
                    }
                    Err(err) => {
                        // The failing frame wrote nothing into `out_buf`, so
                        // any earlier frames in the batch are intact and can
                        // still be sent. Stop packing more work in, deliver
                        // the error to this caller, and flush what we have.
                        encrypt_failure = Some((response_tx, err));
                        break;
                    }
                }

                // Batch ceiling: flush now rather than growing indefinitely.
                if waiters.len() >= MAX_BATCH_FRAMES || batch_wire_len >= MAX_BATCH_WIRE_BYTES {
                    break;
                }

                // Opportunistically drain any frames that arrived while we were
                // encrypting. Non-blocking: we never wait for a batch to fill.
                match send_job_rx.try_recv() {
                    Ok(next) => {
                        // If appending this next frame would cross the byte
                        // ceiling, hold it over to open the next batch rather
                        // than encrypting it into this one.
                        let estimated_wire = frame_wire_len(next.plaintext.len());
                        if !waiters.is_empty()
                            && batch_wire_len + estimated_wire > MAX_BATCH_WIRE_BYTES
                        {
                            carry_over = Some(next);
                            break;
                        }
                        job = next;
                    }
                    Err(_) => break,
                }
            }

            // Flush the batch to the transport.
            let transport_result = if !out_buf.is_empty() {
                // split().freeze() yields an immutable Bytes over the packed
                // frames without copying, leaving `out_buf` empty with its
                // capacity intact for the next batch.
                let payload = out_buf.split().freeze();
                transport.send(payload).await
            } else {
                Ok(())
            };

            match transport_result {
                Ok(()) => {
                    if let Some(stats) = &stats {
                        for (_, wire_len) in &waiters {
                            stats.record_frame_sent(*wire_len);
                        }
                    }
                    for (response_tx, _) in waiters.drain(..) {
                        let _ = response_tx.send(Ok(()));
                    }
                    // Re-read the gate rather than trusting the read at
                    // capture time, so a batch that outlived its last lease
                    // stays quiet.
                    if let Some(tap) = sent_frames.as_deref()
                        && tap.enabled()
                    {
                        for plaintext in observed.drain(..) {
                            tap.publish(plaintext);
                        }
                    }
                }
                Err(e) => {
                    // One write failed; mark the sender poisoned so later
                    // jobs on this connection are rejected promptly without
                    // risking nonce reuse or counter desync.
                    poisoned = true;
                    transport.disconnect().await;
                    let err = EncryptSendError::transport(e);
                    if waiters.len() == 1 {
                        let (response_tx, _) = waiters.drain(..).next().expect("length checked");
                        let _ = response_tx.send(Err(err));
                    } else {
                        let shared = Arc::new(err);
                        for (response_tx, _) in waiters.drain(..) {
                            let _ = response_tx.send(Err(EncryptSendError::transport(
                                SharedSendFailure(shared.clone()),
                            )));
                        }
                    }
                }
            }
            if let Some((response_tx, err)) = encrypt_failure {
                let _ = response_tx.send(Err(err));
            }

            let queue_drained = carry_over.is_none() && send_job_rx.is_empty();
            if should_release_batch_buffer(
                batch_wire_len,
                out_buf.capacity(),
                queue_drained,
                &mut out_buf_grown,
                &mut small_batches,
            ) {
                out_buf = BytesMut::with_capacity(OUT_BUF_IDLE_CAPACITY);
            }
        }
    }

    /// Encrypt one plaintext and append the framed result to `out_buf`,
    /// returning its wire size.
    pub async fn encrypt_frame_into(
        runtime: &Arc<dyn Runtime>,
        write_key: &Arc<NoiseCipher>,
        write_counter: &mut u32,
        plaintext: bytes::Bytes,
        out_buf: &mut BytesMut,
    ) -> std::result::Result<usize, EncryptSendError> {
        let counter = *write_counter;
        if counter == u32::MAX {
            return Err(EncryptSendError::crypto(NoiseError::CounterExhausted));
        }
        let before = out_buf.len();

        if plaintext.len() <= INLINE_ENCRYPT_THRESHOLD {
            let body_len = plaintext.len() + TAG_LEN;
            if let Err(e) = crate::framing::append_frame_header_into(body_len, None, out_buf) {
                return Err(EncryptSendError::framing(e));
            }
            let base = out_buf.len();
            out_buf.extend_from_slice(&plaintext);
            if let Err(e) = write_key
                .encrypt_in_place_with_counter(counter, &mut FrameBody { out: out_buf, base })
            {
                out_buf.truncate(before);
                return Err(EncryptSendError::crypto(e));
            }
            if out_buf.len() - base != body_len {
                out_buf.truncate(before);
                return Err(EncryptSendError::crypto(NoiseError::Encrypt(
                    crate::libsignal::crypto::CryptoProviderError::BackendFailed,
                )));
            }
            *write_counter = counter + 1;
            Ok(out_buf.len() - before)
        } else {
            let write_key = write_key.clone();
            let encrypt_result = crate::runtime::blocking(&**runtime, move || {
                write_key.encrypt_with_counter(counter, &plaintext)
            })
            .await;
            let ciphertext = match encrypt_result {
                Ok(c) => c,
                Err(e) => return Err(EncryptSendError::crypto(e)),
            };

            if let Err(e) = crate::framing::append_frame_into(&ciphertext, None, out_buf) {
                return Err(EncryptSendError::framing(e));
            }
            *write_counter = counter + 1;
            Ok(out_buf.len() - before)
        }
    }

    /// Enqueues a plaintext frame for encryption and send.
    pub async fn enqueue_send(
        &self,
        plaintext: bytes::Bytes,
    ) -> std::result::Result<oneshot::Receiver<SendResult>, EncryptSendError> {
        let (response_tx, response_rx) = oneshot::channel();

        let job = SendJob {
            plaintext,
            response_tx,
        };

        if let Err(_send_err) = self.send_job_tx.send(job).await {
            return Err(EncryptSendError::channel_closed());
        }

        Ok(response_rx)
    }

    /// Awaits a receiver handed out by [`Self::enqueue_send`].
    pub async fn await_send(receiver: oneshot::Receiver<SendResult>) -> SendResult {
        match receiver.await {
            Ok(result) => result,
            Err(_) => Err(EncryptSendError::channel_closed()),
        }
    }

    /// Encrypts and sends a plaintext frame over the transport.
    pub async fn encrypt_and_send(&self, plaintext: bytes::Bytes) -> SendResult {
        let receiver = self.enqueue_send(plaintext).await?;
        Self::await_send(receiver).await
    }

    /// Marshals and encrypts a binary protocol node, sending it through the dedicated sender task.
    pub async fn send_node(&self, node: &wacore_binary::Node) -> SendResult {
        let plaintext = wacore_binary::marshal(node)
            .map_err(|e| EncryptSendError::framing(SocketError::Marshal(e)))?;
        self.encrypt_and_send(plaintext.into()).await
    }

    /// Stop the sender task now, without waiting for the last `Arc` to drop.
    pub fn abort_sender(&self) {
        self.send_job_tx.close();
        self.sender_task_handle.abort();
    }

    /// Decrypts an incoming frame in-place using the connection's read cipher and counter.
    ///
    /// Checks for counter exhaustion before decrypting and commits the counter increment
    /// only after authentication succeeds, preventing corrupted frames from desynchronizing
    /// future valid frames.
    pub fn decrypt_frame(&self, mut ciphertext: BytesMut) -> Result<BytesMut> {
        let counter = self.read_counter.load(Ordering::SeqCst);
        if counter == u32::MAX {
            return Err(SocketError::Cipher(NoiseError::CounterExhausted));
        }
        self.read_key
            .decrypt_in_place_with_counter(counter, &mut ciphertext)
            .map_err(SocketError::Cipher)?;
        self.read_counter
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |c| c.checked_add(1))
            .map_err(|_| SocketError::Cipher(NoiseError::CounterExhausted))?;
        Ok(ciphertext)
    }

    /// Read-only snapshot of the current read frame counter.
    pub fn read_counter(&self) -> u32 {
        self.read_counter.load(Ordering::SeqCst)
    }

    /// Explicitly sets the read counter for testing counter exhaustion.
    #[doc(hidden)]
    pub fn set_read_counter_for_test(&self, val: u32) {
        self.read_counter.store(val, Ordering::SeqCst);
    }
}
