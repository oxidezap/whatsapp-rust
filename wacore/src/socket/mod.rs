pub mod error;
pub mod noise_socket;

pub use error::{EncryptSendError, EncryptSendErrorKind, EncryptSendResult, Result, SocketError};
pub use noise_socket::{
    FrameBody, FrameTap, INLINE_ENCRYPT_THRESHOLD, MAX_BATCH_FRAMES, MAX_BATCH_WIRE_BYTES,
    NoiseSocket, OUT_BUF_IDLE_CAPACITY, SMALL_BATCHES_BEFORE_SHRINK, SendJob, SendObservers,
    SendResult, TAG_LEN, frame_wire_len, should_release_batch_buffer,
};
