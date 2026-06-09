use bytes::{Buf, Bytes, BytesMut};
use log::trace;

pub const FRAME_LENGTH_SIZE: usize = 3;
/// WA Web: `if (t >= 1 << 24)` in WAFrameSocket.$8
pub const FRAME_MAX_SIZE: usize = 1 << 24;

/// Trait for buffers that can receive framed output.
/// Implemented for `Vec<u8>` and `BytesMut`.
pub trait FrameBuf {
    fn clear(&mut self);
    fn reserve(&mut self, additional: usize);
    fn extend_from_slice(&mut self, src: &[u8]);
}

impl FrameBuf for Vec<u8> {
    #[inline]
    fn clear(&mut self) {
        self.clear();
    }
    #[inline]
    fn reserve(&mut self, additional: usize) {
        self.reserve(additional);
    }
    #[inline]
    fn extend_from_slice(&mut self, src: &[u8]) {
        self.extend_from_slice(src);
    }
}

impl FrameBuf for BytesMut {
    #[inline]
    fn clear(&mut self) {
        self.clear();
    }
    #[inline]
    fn reserve(&mut self, additional: usize) {
        self.reserve(additional);
    }
    #[inline]
    fn extend_from_slice(&mut self, src: &[u8]) {
        self.extend_from_slice(src);
    }
}

/// Encodes a payload into a WhatsApp frame, writing directly into `out`.
/// The `out` buffer is cleared before use, allowing buffer reuse.
/// Works with both `Vec<u8>` and `BytesMut`.
pub fn encode_frame_into(
    payload: &[u8],
    header: Option<&[u8]>,
    out: &mut impl FrameBuf,
) -> Result<(), anyhow::Error> {
    let payload_len = payload.len();

    if payload_len >= FRAME_MAX_SIZE {
        return Err(anyhow::anyhow!(
            "Frame is too large (max: {}, got: {})",
            FRAME_MAX_SIZE,
            payload_len
        ));
    }

    let header_len = header.map(|h| h.len()).unwrap_or(0);
    let prefix_len = header_len + FRAME_LENGTH_SIZE;
    let total_len = prefix_len + payload_len;

    out.clear();
    out.reserve(total_len);

    if let Some(header_data) = header {
        out.extend_from_slice(header_data);
    }

    let len_bytes = u32::to_be_bytes(payload_len as u32);
    out.extend_from_slice(&len_bytes[1..]);
    out.extend_from_slice(payload);

    Ok(())
}

/// Encodes a payload into a WhatsApp frame with length prefix.
/// For performance-critical paths, prefer `encode_frame_into`.
pub fn encode_frame(payload: &[u8], header: Option<&[u8]>) -> Result<Vec<u8>, anyhow::Error> {
    let header_len = header.map(|h| h.len()).unwrap_or(0);
    let prefix_len = header_len + FRAME_LENGTH_SIZE;
    let mut data = Vec::with_capacity(prefix_len + payload.len());
    encode_frame_into(payload, header, &mut data)?;
    Ok(data)
}

/// A frame decoder that buffers incoming data and extracts complete frames.
pub struct FrameDecoder {
    buffer: BytesMut,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::new(),
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Feed an owned payload, adopting its allocation when possible.
    ///
    /// In steady state each transport message starts on a frame boundary
    /// (`buffer` is empty) and the payload has no other references, so
    /// `try_into_mut` succeeds and the bytes are adopted wholesale — the one
    /// remaining full copy of inbound traffic on the receive path is skipped.
    /// A pending partial frame or a shared payload falls back to [`feed`].
    ///
    /// [`feed`]: Self::feed
    pub fn feed_bytes(&mut self, data: Bytes) {
        if self.buffer.is_empty() {
            match data.try_into_mut() {
                Ok(owned) => self.buffer = owned,
                Err(shared) => self.buffer.extend_from_slice(&shared),
            }
        } else {
            self.buffer.extend_from_slice(&data);
        }
    }

    pub fn decode_frame(&mut self) -> Option<BytesMut> {
        if self.buffer.len() < FRAME_LENGTH_SIZE {
            return None;
        }

        // The 3-byte length caps frame_len at 0xFFFFFF (= FRAME_MAX_SIZE - 1), so an
        // oversize read is structurally impossible; the size limit is enforced on
        // send (encode_frame_into) and WA Web's convertBufferedToFrames likewise
        // trusts the decoded length. A previous `frame_len > FRAME_MAX_SIZE` branch
        // here was dead, and its handling (advancing only the 3 length bytes, leaving
        // the payload to be misread as the next length) would have desynced the stream.
        let frame_len = ((self.buffer[0] as usize) << 16)
            | ((self.buffer[1] as usize) << 8)
            | (self.buffer[2] as usize);

        if self.buffer.len() >= FRAME_LENGTH_SIZE + frame_len {
            self.buffer.advance(FRAME_LENGTH_SIZE);
            let frame_data = self.buffer.split_to(frame_len);
            trace!("<-- Decoded frame: {} bytes", frame_data.len());
            Some(frame_data)
        } else {
            None
        }
    }

    /// Returns the number of bytes currently buffered waiting for more data.
    pub fn buffered_len(&self) -> usize {
        self.buffer.len()
    }

    /// Clears the internal buffer.
    pub fn clear(&mut self) {
        self.buffer.clear();
    }
}

impl Default for FrameDecoder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encode_frame_no_header() {
        let payload = vec![1, 2, 3, 4, 5];
        let encoded = encode_frame(&payload, None).expect("frame operation should succeed");

        assert_eq!(encoded[0], 0);
        assert_eq!(encoded[1], 0);
        assert_eq!(encoded[2], 5);
        assert_eq!(&encoded[3..], &payload[..]);
    }

    #[test]
    fn test_encode_frame_with_header() {
        let payload = vec![1, 2, 3];
        let header = vec![0xAA, 0xBB];
        let encoded =
            encode_frame(&payload, Some(&header)).expect("frame operation should succeed");

        assert_eq!(&encoded[0..2], &header[..]);
        assert_eq!(encoded[2], 0);
        assert_eq!(encoded[3], 0);
        assert_eq!(encoded[4], 3);
        assert_eq!(&encoded[5..], &payload[..]);
    }

    #[test]
    fn test_frame_decoder() {
        let mut decoder = FrameDecoder::new();

        decoder.feed(&[0, 0, 5, 1, 2]);
        assert!(decoder.decode_frame().is_none());

        decoder.feed(&[3, 4, 5]);
        let frame = decoder
            .decode_frame()
            .expect("frame operation should succeed");
        assert_eq!(&frame[..], &[1, 2, 3, 4, 5]);

        assert!(decoder.decode_frame().is_none());
    }

    #[test]
    fn test_frame_decoder_multiple_frames() {
        let mut decoder = FrameDecoder::new();

        decoder.feed(&[0, 0, 2, 0xAA, 0xBB, 0, 0, 3, 0xCC, 0xDD, 0xEE]);

        let frame1 = decoder
            .decode_frame()
            .expect("frame operation should succeed");
        assert_eq!(&frame1[..], &[0xAA, 0xBB]);

        let frame2 = decoder
            .decode_frame()
            .expect("frame operation should succeed");
        assert_eq!(&frame2[..], &[0xCC, 0xDD, 0xEE]);

        assert!(decoder.decode_frame().is_none());
    }

    #[test]
    fn test_feed_bytes_adopts_unique_payload_without_copy() {
        let mut data = vec![0, 0, 5];
        data.extend_from_slice(&[1, 2, 3, 4, 5]);
        let payload_addr = data.as_ptr() as usize;

        let mut decoder = FrameDecoder::new();
        decoder.feed_bytes(Bytes::from(data));

        let frame = decoder.decode_frame().expect("complete frame");
        assert_eq!(&frame[..], &[1, 2, 3, 4, 5]);
        // Zero-copy: the frame points into the original allocation, offset by
        // the 3-byte length prefix.
        assert_eq!(frame.as_ptr() as usize, payload_addr + FRAME_LENGTH_SIZE);
    }

    #[test]
    fn test_feed_bytes_shared_payload_falls_back_to_copy() {
        let mut data = vec![0, 0, 2];
        data.extend_from_slice(&[0xAA, 0xBB]);
        let shared = Bytes::from(data);
        let keep_alive = shared.clone(); // refcount > 1 → try_into_mut fails

        let mut decoder = FrameDecoder::new();
        decoder.feed_bytes(shared);

        let frame = decoder.decode_frame().expect("complete frame");
        assert_eq!(&frame[..], &[0xAA, 0xBB]);
        assert_eq!(&keep_alive[3..], &[0xAA, 0xBB]);
    }

    #[test]
    fn test_feed_bytes_partial_frame_accumulates() {
        let mut decoder = FrameDecoder::new();

        decoder.feed_bytes(Bytes::from(vec![0, 0, 4, 1, 2]));
        assert!(decoder.decode_frame().is_none());

        // Buffer non-empty → append path; the frame completes across feeds.
        decoder.feed_bytes(Bytes::from(vec![3, 4]));
        let frame = decoder.decode_frame().expect("complete frame");
        assert_eq!(&frame[..], &[1, 2, 3, 4]);

        // Drained again → the next unique payload is adopted zero-copy.
        let mut data = vec![0, 0, 1];
        data.push(0xCC);
        let payload_addr = data.as_ptr() as usize;
        decoder.feed_bytes(Bytes::from(data));
        let frame = decoder.decode_frame().expect("complete frame");
        assert_eq!(frame.as_ptr() as usize, payload_addr + FRAME_LENGTH_SIZE);
    }

    #[test]
    fn test_feed_bytes_multiple_frames_single_payload() {
        let mut decoder = FrameDecoder::new();
        decoder.feed_bytes(Bytes::from(vec![
            0, 0, 2, 0xAA, 0xBB, 0, 0, 3, 0xCC, 0xDD, 0xEE,
        ]));

        let frame1 = decoder.decode_frame().expect("first frame");
        assert_eq!(&frame1[..], &[0xAA, 0xBB]);
        let frame2 = decoder.decode_frame().expect("second frame");
        assert_eq!(&frame2[..], &[0xCC, 0xDD, 0xEE]);
        assert!(decoder.decode_frame().is_none());
    }

    #[test]
    fn test_encode_frame_too_large() {
        let large_payload = vec![0u8; FRAME_MAX_SIZE];
        let result = encode_frame(&large_payload, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_encode_frame_into_reuses_buffer() {
        let mut buffer = Vec::with_capacity(100);
        let original_ptr = buffer.as_ptr();

        let payload1 = vec![1, 2, 3, 4, 5];
        encode_frame_into(&payload1, None, &mut buffer).expect("frame operation should succeed");
        assert_eq!(&buffer[3..], &payload1[..]);

        let payload2 = vec![6, 7, 8];
        encode_frame_into(&payload2, None, &mut buffer).expect("frame operation should succeed");
        assert_eq!(&buffer[3..], &payload2[..]);

        assert_eq!(buffer.as_ptr(), original_ptr);
    }

    #[test]
    fn test_encode_frame_into_with_header() {
        let mut buffer = Vec::new();
        let payload = vec![1, 2, 3];
        let header = vec![0xAA, 0xBB];
        encode_frame_into(&payload, Some(&header), &mut buffer)
            .expect("frame operation should succeed");

        assert_eq!(&buffer[0..2], &header[..]);
        assert_eq!(buffer[2], 0);
        assert_eq!(buffer[3], 0);
        assert_eq!(buffer[4], 3);
        assert_eq!(&buffer[5..], &payload[..]);
    }
}
