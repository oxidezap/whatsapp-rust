use bytes::{Buf, BytesMut};
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
    out.clear();
    append_frame_into(payload, header, out)
}

/// Appends a frame's prefix for a payload of `payload_len` and reserves room for
/// the payload itself, without needing the payload's bytes.
///
/// Exists for producers that write the payload straight into `out` afterwards
/// (encrypting in place where the bytes will already be on the wire) instead of
/// building it elsewhere and copying it in. The length prefix is derivable ahead
/// of the payload because AEAD ciphertext length is a fixed function of the
/// plaintext length.
///
/// On error nothing is written, so `out` is left exactly as it was.
pub fn append_frame_header_into(
    payload_len: usize,
    header: Option<&[u8]>,
    out: &mut impl FrameBuf,
) -> Result<(), anyhow::Error> {
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

    out.reserve(total_len);

    if let Some(header_data) = header {
        out.extend_from_slice(header_data);
    }

    let len_bytes = u32::to_be_bytes(payload_len as u32);
    out.extend_from_slice(&len_bytes[1..]);

    Ok(())
}

/// Like [`encode_frame_into`], but appends to `out` instead of clearing it, so
/// several frames can be laid out back to back in one buffer and handed to the
/// transport as a single write.
pub fn append_frame_into(
    payload: &[u8],
    header: Option<&[u8]>,
    out: &mut impl FrameBuf,
) -> Result<(), anyhow::Error> {
    append_frame_header_into(payload.len(), header, out)?;
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

/// Headroom claimed whenever the accumulation buffer runs out of room.
///
/// Every frame split out of a chunk keeps that whole chunk alive for as long as
/// the frame lives, so the chunk size is also the memory floor of a retained
/// inbound node (a chat lane queue, or an offline/history-sync backlog of ~1000
/// stanzas). 1 KiB bounds such a backlog at ~1 MB; 8 KiB would cost ~8 MB to
/// save a further ~0.16 allocations per frame, which is not the trade this
/// receive path wants.
const CHUNK_SIZE: usize = 1024;

/// Capacity a drained accumulation buffer may keep before it is released.
///
/// `BytesMut::reserve` reclaims an oversized unique allocation instead of
/// freeing it, so without this a single multi-megabyte history-sync frame would
/// pin its buffer for the rest of the connection.
const MAX_IDLE_CAPACITY: usize = 64 * 1024;

/// The most the decoder will reserve for a frame whose bytes have not arrived.
///
/// The length prefix is the peer's claim, not evidence. Reserving the whole
/// declared frame lets anyone who sends three bytes announcing 16 MiB, and then
/// nothing, pin 16 MiB until the connection dies. Capping the eager reserve
/// keeps the growth of a genuinely large frame to a handful of steps while
/// making that claim cost only what has actually been received.
const MAX_EAGER_RESERVE: usize = 64 * 1024;

/// A frame decoder that buffers incoming data and extracts complete frames.
///
/// Transport reads accumulate in one long-lived buffer and each frame is split
/// out of it, the shape `tokio_util::codec::Decoder` uses. Two `bytes`
/// properties make that cheaper than handing the buffer itself downstream per
/// frame: the accumulation allocation amortizes over every frame that fits in a
/// chunk, and a buffer that has been split is already reference-counted, so the
/// `freeze()` further down the receive path is a pointer move rather than a
/// fresh `Shared` allocation.
pub struct FrameDecoder {
    buffer: BytesMut,
    /// Whether the buffer has ever grown past [`MAX_IDLE_CAPACITY`].
    ///
    /// Splitting a frame out hands it the capacity, so a frame that consumed
    /// the whole buffer leaves `capacity() == 0` behind and the size check
    /// alone cannot see that a multi-megabyte allocation is still underneath.
    /// Once the frame is dropped the decoder is its sole owner and the next
    /// `reserve` reclaims it rather than freeing it, which is the leak this
    /// remembers to prevent.
    grew_oversized: bool,
}

impl FrameDecoder {
    pub fn new() -> Self {
        Self {
            // `with_capacity`, not `new`: `bytes` remembers the initial capacity
            // and uses it as the floor when it has to allocate a fresh buffer
            // because a live frame still references the current one, which is
            // the steady-state path here.
            buffer: BytesMut::with_capacity(CHUNK_SIZE),
            grew_oversized: false,
        }
    }

    pub fn feed(&mut self, data: &[u8]) {
        self.reserve_for(data.len());
        self.buffer.extend_from_slice(data);
    }

    /// Makes room for `incoming` more bytes.
    ///
    /// Growing only once the bytes genuinely do not fit uses up the tail of the
    /// current chunk first. A read bigger than a chunk asks for exactly what it
    /// needs, so a megabyte-sized history-sync frame is not rounded up to the
    /// next growth step.
    fn reserve_for(&mut self, incoming: usize) {
        if self.buffer.capacity() - self.buffer.len() < incoming {
            self.buffer.reserve(incoming.max(CHUNK_SIZE));
            self.grew_oversized |= self.buffer.capacity() > MAX_IDLE_CAPACITY;
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

        if self.buffer.len() < FRAME_LENGTH_SIZE + frame_len {
            // The length prefix is known before the payload is, so a large frame
            // sizes the buffer ahead instead of growing geometrically across the
            // reads that carry it, capped per [`MAX_EAGER_RESERVE`].
            let missing = FRAME_LENGTH_SIZE + frame_len - self.buffer.len();
            self.reserve_for(missing.min(MAX_EAGER_RESERVE));
            return None;
        }

        self.buffer.advance(FRAME_LENGTH_SIZE);
        let frame_data = self.buffer.split_to(frame_len);
        trace!("<-- Decoded frame: {} bytes", frame_data.len());

        // A frame rarely ends exactly on a read boundary, so what is left behind
        // is usually the head of the next one, and that residue keeps the whole
        // oversized allocation alive however few bytes it is. Copying a short
        // residue into a fresh chunk lets the large allocation die with the
        // frame. A long one is a frame still arriving that would have to grow
        // the buffer straight back, so it is left where it already is.
        if (self.grew_oversized || self.buffer.capacity() > MAX_IDLE_CAPACITY)
            && self.buffer.len() <= CHUNK_SIZE
        {
            let mut fresh = BytesMut::with_capacity(CHUNK_SIZE);
            fresh.extend_from_slice(&self.buffer);
            self.buffer = fresh;
            self.grew_oversized = false;
        }

        Some(frame_data)
    }

    /// The accumulation buffer's capacity, for retention assertions.
    #[cfg(test)]
    pub fn capacity_for_test(&self) -> usize {
        self.buffer.capacity()
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
    use crate::state::NoiseCipher;

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

    /// Payload size for the four tests below that need the buffer to go past
    /// [`MAX_IDLE_CAPACITY`], which is the only property of it that matters.
    ///
    /// Cut under Miri because interpretation is ~100x native and `payload`
    /// builds its bytes one at a time. It must stay *above* 64 KiB: below that
    /// `grew_oversized` never latches, the release path is never entered, and
    /// all four assertions hold without testing anything. Each of them asserts
    /// the latch, so shrinking this too far fails loudly instead.
    const OVERSIZED_PAYLOAD: usize = if cfg!(miri) { 80 * 1024 } else { 200 * 1024 };

    /// Deterministic stand-in for a payload of `len` bytes.
    fn payload(len: usize, seed: u8) -> Vec<u8> {
        (0..len)
            .map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed))
            .collect()
    }

    /// The whole point of the accumulation buffer: consecutive frames are cut
    /// out of one allocation rather than each carrying its own. Their addresses
    /// prove it: frame 2 begins exactly one length prefix past the end of frame
    /// 1, in the same chunk, even though they arrived in separate feeds.
    #[test]
    fn consecutive_frames_are_split_out_of_one_buffer() {
        let mut decoder = FrameDecoder::new();

        decoder.feed(&encode_frame(&payload(40, 1), None).expect("encode"));
        let first = decoder.decode_frame().expect("first frame");

        decoder.feed(&encode_frame(&payload(40, 2), None).expect("encode"));
        let second = decoder.decode_frame().expect("second frame");

        assert_eq!(&first[..], &payload(40, 1)[..]);
        assert_eq!(&second[..], &payload(40, 2)[..]);
        assert_eq!(
            second.as_ptr() as usize,
            first.as_ptr() as usize + first.len() + FRAME_LENGTH_SIZE,
            "the second frame must be cut from the chunk the first one left behind"
        );
    }

    /// A buffer whose chunk has run out claims a whole new one rather than just
    /// the bytes in hand, which is what amortizes the allocation over the frames
    /// that follow. Every frame is kept alive so the exhausted chunk cannot be
    /// reclaimed and the buffer is forced to grow.
    #[test]
    fn growing_the_buffer_claims_a_whole_chunk() {
        let frame = encode_frame(&payload(60, 0x88), None).expect("encode");
        let mut decoder = FrameDecoder::new();
        let mut retained = Vec::new();

        while decoder.buffer.capacity() >= frame.len() {
            decoder.feed(&frame);
            retained.push(decoder.decode_frame().expect("frame"));
        }

        decoder.feed(&frame);
        assert!(
            decoder.buffer.capacity() >= CHUNK_SIZE,
            "a grown buffer holds {} bytes, less than one chunk",
            decoder.buffer.capacity()
        );
        assert_eq!(
            &decoder.decode_frame().expect("frame after growth")[..],
            &payload(60, 0x88)[..]
        );
    }

    /// A frame that outgrows a chunk still decodes byte for byte, whether it
    /// arrives whole or spread over reads that each fit in a chunk.
    #[test]
    fn frame_larger_than_a_chunk_round_trips() {
        let expected = payload(CHUNK_SIZE * 3 + 7, 0x5A);
        let encoded = encode_frame(&expected, None).expect("encode");

        let mut decoder = FrameDecoder::new();
        decoder.feed(&encoded);
        let whole = decoder.decode_frame().expect("oversize frame in one feed");
        assert_eq!(&whole[..], &expected[..]);

        let mut decoder = FrameDecoder::new();
        let mut pieces = encoded.chunks(600);
        decoder.feed(pieces.next().expect("at least one piece"));
        assert!(decoder.decode_frame().is_none());
        assert!(
            decoder.buffer.capacity() >= encoded.len(),
            "the length prefix must size the buffer for the whole frame in one step"
        );
        for piece in pieces {
            decoder.feed(piece);
        }
        let pieced = decoder.decode_frame().expect("oversize frame across feeds");
        assert_eq!(&pieced[..], &expected[..]);
        assert!(decoder.decode_frame().is_none());
    }

    /// The chunk boundary itself: a frame whose payload exactly fills a chunk
    /// leaves no room behind it, so the frame after it must still decode.
    #[test]
    fn frame_exactly_one_chunk_long_round_trips() {
        let first = payload(CHUNK_SIZE, 0x11);
        let second = payload(9, 0x22);

        let mut decoder = FrameDecoder::new();
        decoder.feed(&encode_frame(&first, None).expect("encode"));
        assert_eq!(
            &decoder.decode_frame().expect("chunk-sized frame")[..],
            &first[..]
        );

        decoder.feed(&encode_frame(&second, None).expect("encode"));
        assert_eq!(
            &decoder.decode_frame().expect("following frame")[..],
            &second[..]
        );
    }

    /// The retention case: a decoded frame is held (a queued node) while later
    /// frames keep arriving and reusing the buffer. Holding it must not let the
    /// later traffic overwrite its bytes, and must not corrupt the later frames
    /// either.
    #[test]
    fn a_retained_frame_is_not_disturbed_by_later_frames() {
        let mut decoder = FrameDecoder::new();
        let retained_payload = payload(64, 0x77);

        decoder.feed(&encode_frame(&retained_payload, None).expect("encode"));
        let retained = decoder.decode_frame().expect("retained frame");

        let mut still_live = Vec::new();
        for i in 0..200u16 {
            let expected = payload(50 + (i as usize % 40), i as u8);
            decoder.feed(&encode_frame(&expected, None).expect("encode"));
            let frame = decoder.decode_frame().expect("later frame");
            assert_eq!(&frame[..], &expected[..], "frame {i} decoded wrong");
            // Keep a window of frames alive so the buffer keeps being split
            // while several of its chunks are still referenced.
            still_live.push(frame);
            if still_live.len() > 16 {
                still_live.remove(0);
            }
        }

        assert_eq!(&retained[..], &retained_payload[..]);
    }

    /// A frame big enough to have grown the buffer must not leave that
    /// allocation attached to the decoder for the rest of the connection.
    /// A length prefix is a claim, not evidence. Three bytes announcing a
    /// near-16 MiB frame, with nothing behind them, must not make the decoder
    /// reserve 16 MiB: that turns one packet into a memory-exhaustion lever
    /// held open until the connection dies.
    #[test]
    fn a_declared_frame_cannot_reserve_more_than_it_has_sent() {
        let mut decoder = FrameDecoder::new();

        // 0xFFFFFF, the largest length the 3-byte prefix can express.
        decoder.feed(&[0xFF, 0xFF, 0xFF]);
        assert!(
            decoder.decode_frame().is_none(),
            "the payload has not arrived, so no frame can come out"
        );

        assert!(
            decoder.buffered_len() < 16 * 1024 * 1024,
            "precondition: nothing was actually received"
        );
        assert!(
            decoder.capacity_for_test() <= MAX_EAGER_RESERVE + CHUNK_SIZE,
            "a declared but unsent frame reserved {} bytes",
            decoder.capacity_for_test()
        );
    }

    /// A frame that consumes the whole buffer hands its capacity away, so the
    /// decoder is left reading `capacity() == 0` while a multi-megabyte
    /// allocation still sits underneath it. Once the frame drops, the next
    /// `reserve` reclaims that allocation rather than freeing it, and the
    /// decoder holds it for the rest of the connection.
    #[test]
    fn an_exactly_consumed_oversized_buffer_is_still_released() {
        let mut decoder = FrameDecoder::new();
        let payload_len = OVERSIZED_PAYLOAD;

        let mut wire = Vec::with_capacity(FRAME_LENGTH_SIZE + payload_len);
        wire.push((payload_len >> 16) as u8);
        wire.push((payload_len >> 8) as u8);
        wire.push(payload_len as u8);
        wire.extend(std::iter::repeat_n(0xAB, payload_len));

        decoder.feed(&wire);
        assert!(
            decoder.grew_oversized,
            "fixture never crossed MAX_IDLE_CAPACITY, so the release path below is not reached"
        );
        let frame = decoder.decode_frame().expect("the whole frame arrived");
        assert_eq!(frame.len(), payload_len);
        drop(frame);

        // The next read must not re-adopt the large allocation.
        decoder.feed(&[0x00, 0x00, 0x01, 0x7F]);
        assert!(
            decoder.capacity_for_test() <= MAX_IDLE_CAPACITY,
            "decoder still holds {} bytes after an exactly consumed frame",
            decoder.capacity_for_test()
        );
    }

    /// A read almost never ends on a frame boundary, so the realistic shape of
    /// the case above is an oversized frame trailed by the first bytes of the
    /// next one. Those few bytes are a live view into the large allocation and
    /// keep all of it alive, for the rest of the connection if the rest of that
    /// frame never arrives.
    #[test]
    fn an_oversized_buffer_is_released_when_a_short_tail_remains() {
        let big = payload(OVERSIZED_PAYLOAD, 0x55);
        let mut decoder = FrameDecoder::new();

        let mut wire = encode_frame(&big, None).expect("encode");
        // Two bytes of the next frame's length prefix: not enough to decode, so
        // the decoder parks holding them.
        wire.extend_from_slice(&[0x00, 0x00]);
        decoder.feed(&wire);
        assert!(
            decoder.grew_oversized,
            "fixture never crossed MAX_IDLE_CAPACITY, so the release path below is not reached"
        );

        let frame = decoder.decode_frame().expect("big frame");
        assert_eq!(&frame[..], &big[..]);

        // `capacity()` cannot see this: the residue is a two-byte view, so it
        // reports two bytes whether or not 200 KiB sits underneath it. Where it
        // points is the observable fact. A residue sitting right where the frame
        // ends is still the tail of that same allocation, so this asserts the
        // two are *not* adjacent, i.e. the residue was moved out of it. The
        // sibling test below asserts the opposite for a residue too long to move.
        assert_ne!(
            decoder.buffer.as_ptr() as usize,
            frame.as_ptr() as usize + frame.len(),
            "the residue was left as a view into the oversized allocation"
        );
        drop(frame);

        // And with the frame gone the decoder is sole owner of whatever it kept,
        // so a reserve here would reclaim the large one rather than free it.
        decoder.feed(&[0x01, 0x7F]);
        assert!(
            decoder.capacity_for_test() <= MAX_IDLE_CAPACITY,
            "decoder reclaimed {} bytes behind a 2-byte tail",
            decoder.capacity_for_test()
        );

        // The tail has to survive the move, or the stream desyncs.
        let next = decoder.decode_frame().expect("the tailed frame completes");
        assert_eq!(&next[..], &[0x7F]);
    }

    /// The counterpart: a residue that is itself a frame in flight is left
    /// alone. Copying it would be paid per frame through a history sync, and
    /// the buffer it was copied out of would have to be grown straight back.
    #[test]
    fn a_long_residue_is_not_copied_out_of_its_buffer() {
        let big = payload(OVERSIZED_PAYLOAD, 0x66);
        let mut decoder = FrameDecoder::new();

        let mut wire = encode_frame(&big, None).expect("encode");
        wire.extend_from_slice(&encode_frame(&payload(8 * 1024, 0x77), None).expect("encode")[..]);
        decoder.feed(&wire);
        assert!(
            decoder.grew_oversized,
            "fixture never crossed MAX_IDLE_CAPACITY, so there is no release decision to skip"
        );

        let frame = decoder.decode_frame().expect("big frame");
        assert_eq!(
            decoder.buffer.as_ptr() as usize,
            frame.as_ptr() as usize + frame.len(),
            "an 8 KiB frame in flight was copied out of the buffer it arrived in"
        );
    }

    #[test]
    fn an_oversized_buffer_is_released_once_drained() {
        let big = payload(OVERSIZED_PAYLOAD, 0x33);
        let mut decoder = FrameDecoder::new();
        decoder.feed(&encode_frame(&big, None).expect("encode"));
        assert!(
            decoder.grew_oversized,
            "fixture never crossed MAX_IDLE_CAPACITY, so the release path below is not reached"
        );

        let frame = decoder.decode_frame().expect("big frame");
        assert_eq!(&frame[..], &big[..]);
        // Dropping it makes the decoder the sole owner, which is the case where
        // `reserve` would otherwise reclaim the oversized allocation forever.
        drop(frame);

        let small = payload(5, 0x44);
        decoder.feed(&encode_frame(&small, None).expect("encode"));
        assert_eq!(
            &decoder.decode_frame().expect("small frame")[..],
            &small[..]
        );

        assert!(
            decoder.buffer.capacity() <= MAX_IDLE_CAPACITY,
            "decoder still holds {} bytes after a large frame drained",
            decoder.buffer.capacity()
        );
    }

    /// Decoded frames go straight into in-place AEAD decryption, which writes
    /// through the buffer it is handed. A frame that is a split-off view of a
    /// shared chunk must therefore still round trip, and must not touch the
    /// neighbouring frame that shares its allocation.
    #[test]
    fn a_split_frame_decrypts_in_place() {
        let cipher = NoiseCipher::new(&[0x2A; 32]).expect("32-byte key");
        let plaintext = payload(300, 0x66);

        let mut decoder = FrameDecoder::new();
        decoder.feed(
            &encode_frame(
                &cipher.encrypt_with_counter(0, &plaintext).expect("encrypt"),
                None,
            )
            .expect("encode"),
        );
        let neighbour_payload = payload(70, 0x99);
        decoder.feed(&encode_frame(&neighbour_payload, None).expect("encode"));

        let mut frame = decoder.decode_frame().expect("encrypted frame");
        let neighbour = decoder.decode_frame().expect("neighbouring frame");

        cipher
            .decrypt_in_place_with_counter(0, &mut frame)
            .expect("the tag must verify over a split-off frame");
        assert_eq!(&frame[..], &plaintext[..]);
        assert_eq!(&neighbour[..], &neighbour_payload[..]);
    }

    /// A frame whose length prefix is corrupted upwards never completes, and the
    /// decoder must keep waiting instead of handing out short or foreign bytes.
    #[test]
    fn a_frame_shorter_than_its_length_prefix_is_withheld() {
        let mut decoder = FrameDecoder::new();
        decoder.feed(&[0, 0, 8, 1, 2, 3]);
        assert!(decoder.decode_frame().is_none());
        assert_eq!(decoder.buffered_len(), 6);

        // Not even a following frame's bytes may be borrowed to complete it.
        decoder.feed(&[4, 5]);
        assert!(decoder.decode_frame().is_none());

        decoder.feed(&[6, 7, 8]);
        let frame = decoder.decode_frame().expect("now complete");
        assert_eq!(&frame[..], &[1, 2, 3, 4, 5, 6, 7, 8]);
    }

    /// The header-only form has to lay down exactly what `append_frame_into`
    /// lays down before the payload, or a caller that writes its payload in
    /// place afterwards produces a frame the peer cannot split. Asserted
    /// against the literal big-endian bytes, not against `append_frame_into`:
    /// that one delegates here, so comparing the two would agree with itself
    /// no matter what either wrote.
    #[test]
    fn append_frame_header_writes_the_length_big_endian() {
        let mut out = vec![0x5Au8];
        append_frame_header_into(0x012345, None, &mut out).expect("header only");
        assert_eq!(
            out,
            vec![0x5A, 0x01, 0x23, 0x45],
            "the header must append 3 big-endian length bytes after what was staged"
        );

        // A length that only fills the low byte must still occupy all three.
        let mut short = Vec::new();
        append_frame_header_into(5, None, &mut short).expect("header only");
        assert_eq!(short, vec![0x00, 0x00, 0x05]);
        assert_eq!(short.len(), FRAME_LENGTH_SIZE);
    }

    /// Reserving for the payload is the header call's job: the caller writes the
    /// payload straight into `out` and must not have to grow it again.
    #[test]
    fn append_frame_header_reserves_room_for_the_payload() {
        let mut out = Vec::new();
        append_frame_header_into(1000, None, &mut out).expect("header only");
        assert!(out.capacity() >= FRAME_LENGTH_SIZE + 1000);
    }

    /// An oversize frame is rejected before a single byte is written, so a
    /// buffer that is already staging other frames survives the rejection.
    #[test]
    fn append_frame_header_rejects_oversize_without_writing() {
        let mut out = vec![0xEE; 7];
        let err = append_frame_header_into(FRAME_MAX_SIZE, None, &mut out);
        assert!(err.is_err());
        assert_eq!(out, vec![0xEE; 7]);
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
