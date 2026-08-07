use std::cell::RefCell;
use std::io;
use zlib_rs::{Inflate, InflateError, InflateFlush, Status};

/// zlib inflate wants a zlib header and the 32 KB LZ77 window.
const ZLIB_HEADER: bool = true;
const WINDOW_BITS: u8 = 15;

thread_local! {
    static DECOMPRESSOR: RefCell<(Inflate, Vec<u8>)> = RefCell::new((
        Inflate::new(ZLIB_HEADER, WINDOW_BITS),
        Vec::with_capacity(4096),
    ));

    // Free-list of inflate state (~46 KB each). A connection's bootstrap history
    // sync decompresses several blobs sequentially, each via a fresh
    // `InflateReader`; reusing the state avoids re-initializing zlib per blob.
    //
    // The output window is deliberately not part of the entry. It is the larger
    // allocation of the two and costs one malloc to recreate, where zlib state
    // costs an order of magnitude more, so retaining it bought little and left
    // 64 KB alive per thread for the process's lifetime after a single sync.
    static INFLATE_POOL: RefCell<Vec<Inflate>> = const { RefCell::new(Vec::new()) };
}

/// Inflate straight into the vector's spare capacity, then extend its length by
/// the produced count. Unlike `flate2::Decompress::decompress_vec`, this never
/// zero-initializes the spare region first: flate2's zlib-rs backend doesn't
/// override `decompress_uninit`, so it memsets the whole output window before
/// every call — pure waste, since inflate overwrites exactly those bytes.
fn inflate_into_spare(
    inflate: &mut Inflate,
    input: &[u8],
    out: &mut Vec<u8>,
    flush: InflateFlush,
) -> Result<Status, InflateError> {
    let before = inflate.total_out();
    let status = inflate.decompress_uninit(input, out.spare_capacity_mut(), flush)?;
    let produced = (inflate.total_out() - before) as usize;
    // SAFETY: `decompress_uninit` wrote exactly `produced` bytes (per total_out)
    // into the spare capacity, so that prefix is now initialized and in-bounds.
    unsafe { out.set_len(out.len() + produced) };
    Ok(status)
}

/// Streaming zlib reader: decompresses `input` incrementally into a small
/// accumulation buffer, so a caller can parse length-delimited records as they
/// become available and discard consumed bytes — peak memory stays ~the largest
/// single record being buffered, not the whole decompressed blob.
///
/// Usage: `ensure(n)` to make ≥ n bytes available, read from `available()`, then
/// `consume(k)`. The buffer is compacted (consumed prefix dropped) as it grows.
pub struct InflateReader<'a> {
    input: &'a [u8],
    in_pos: usize,
    // `Option` so `Drop` can move the state back into the pool (Inflate has no
    // cheap throwaway value to swap in). Always `Some` until dropped.
    decomp: Option<Inflate>,
    buf: Vec<u8>,
    cursor: usize,
    total_out: u64,
    max: u64,
    eof: bool,
    stream_end: bool,
}

impl<'a> InflateReader<'a> {
    /// Output decompress window per pump; also the compaction threshold. One
    /// window is the steady state: the pump compacts a consumed prefix before
    /// inflating, so the buffer only grows past `CHUNK` when one individual
    /// record genuinely exceeds it. Allocated per reader, never retained.
    const CHUNK: usize = 64 * 1024;
    /// Cap on retained free-list entries, so concurrently-alive readers on one
    /// thread don't grow the pool unbounded. Sequential readers are the norm and
    /// leave one entry parked; the cap only binds when readers nest.
    const POOL_MAX: usize = 4;

    pub fn new(input: &'a [u8], max: u64) -> Self {
        let decomp = INFLATE_POOL.with(|p| p.borrow_mut().pop()).map_or_else(
            || Inflate::new(ZLIB_HEADER, WINDOW_BITS),
            |mut decomp| {
                decomp.reset(ZLIB_HEADER);
                decomp
            },
        );
        Self {
            input,
            in_pos: 0,
            decomp: Some(decomp),
            buf: Vec::with_capacity(Self::CHUNK),
            cursor: 0,
            total_out: 0,
            max,
            eof: false,
            stream_end: false,
        }
    }

    /// Unparsed decompressed bytes currently buffered.
    #[inline]
    pub fn available(&self) -> &[u8] {
        &self.buf[self.cursor..]
    }

    /// Mark `n` already-read bytes as consumed.
    #[inline]
    pub fn consume(&mut self, n: usize) {
        self.cursor = (self.cursor + n).min(self.buf.len());
    }

    /// Ensure at least `need` unparsed bytes are buffered, decompressing more as
    /// required. Returns `Ok(false)` if the stream ends before reaching `need`.
    pub fn ensure(&mut self, need: usize) -> io::Result<bool> {
        while self.buf.len() - self.cursor < need {
            if self.eof {
                return Ok(false);
            }
            self.pump()?;
        }
        Ok(true)
    }

    /// True once the stream is fully decompressed and all bytes consumed.
    pub fn is_done(&self) -> bool {
        self.eof && self.cursor >= self.buf.len()
    }

    /// Total decompressed bytes produced so far. After the stream ends this is
    /// the blob's exact inflated size.
    pub fn total_out(&self) -> u64 {
        self.total_out
    }

    /// Compressed input consumed so far plus the input's full length. The
    /// ratio lets callers extrapolate totals (e.g. record counts) from a
    /// prefix without a second pass over the blob.
    #[inline]
    pub fn compressed_progress(&self) -> (usize, usize) {
        (self.in_pos, self.input.len())
    }

    /// Whether zlib reported a proper stream end (terminator + adler32
    /// checksum). An EOF (`ensure` returning false) without this means the
    /// input was truncated, not finished.
    pub fn stream_ended(&self) -> bool {
        self.stream_end
    }

    fn pump(&mut self) -> io::Result<()> {
        // Reclaim the consumed prefix before inflating. Reserving a full output
        // window on top of a small read-ahead suffix made ordinary framed
        // streams jump from 64 to 128 KiB even though no record needed it.
        // Compacting here keeps those streams in one window; a record larger
        // than the window still grows normally below.
        if self.cursor != 0 {
            let remaining = self.buf.len() - self.cursor;
            self.buf.copy_within(self.cursor.., 0);
            self.buf.truncate(remaining);
            self.cursor = 0;
        }

        // `decomp` is `Some` for the reader's whole lifetime (only `Drop` takes it),
        // so this is unreachable in practice; surface it as an error rather than panic.
        let decomp = self
            .decomp
            .as_mut()
            .ok_or_else(|| io::Error::other("InflateReader used after pool return"))?;
        // Inflate straight into the window's spare capacity: a stack chunk +
        // extend_from_slice would copy every decompressed byte a second time
        // (~10% of a history-sync extraction).
        if self.buf.len() == self.buf.capacity() {
            self.buf.reserve(Self::CHUNK);
        }
        let prev_in = decomp.total_in();
        let prev_out = decomp.total_out();
        let status = inflate_into_spare(
            decomp,
            &self.input[self.in_pos..],
            &mut self.buf,
            InflateFlush::NoFlush,
        )
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.as_str()))?;
        let new_in = decomp.total_in();
        let produced = (decomp.total_out() - prev_out) as usize;
        self.in_pos += (new_in - prev_in) as usize;
        self.total_out += produced as u64;
        if self.total_out > self.max {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("decompressed payload exceeds {} bytes", self.max),
            ));
        }

        match status {
            Status::StreamEnd => {
                self.eof = true;
                self.stream_end = true;
            }
            // No output produced and not at stream end: distinguish a truncated
            // tail (no input left → treat as end, with `stream_end` left false
            // so callers can tell it apart from a real terminator) from a
            // stalled/corrupt stream (input remains but the decompressor
            // consumed none → error, instead of spinning forever since 64 KB of
            // output is always available).
            // Mirrors the no-progress guard in `decompress_zlib_pooled`.
            _ if produced == 0 => {
                if self.in_pos >= self.input.len() {
                    self.eof = true;
                } else if new_in == prev_in {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "zlib stream stalled (no progress)",
                    ));
                }
            }
            _ => {}
        }
        Ok(())
    }
}

impl Drop for InflateReader<'_> {
    fn drop(&mut self) {
        // Return the decompressor to the per-thread free-list for reuse; `reset`
        // on the next checkout makes prior stream state (incl. errors) moot. The
        // window goes back to the allocator with the rest of the reader.
        if let Some(decomp) = self.decomp.take() {
            INFLATE_POOL.with(|p| {
                let mut pool = p.borrow_mut();
                if pool.len() < Self::POOL_MAX {
                    pool.push(decomp);
                }
            });
        }
    }
}

/// Grow the output buffer by projecting the decompressed size from the
/// expansion ratio observed so far, instead of blind capacity doubling. A
/// high-ratio stream (the up-front `2x compressed` guess undershot) then
/// converges in one or two reallocations sized near the real total, rather
/// than a doubling chain whose copies and final overshoot dominate both the
/// allocated-bytes count and the peak.
fn grow_by_observed_ratio(
    scratch: &mut Vec<u8>,
    decompressor: &Inflate,
    compressed_len: usize,
    cap: usize,
) {
    let consumed = decompressor.total_in() as usize;
    let produced = decompressor.total_out() as usize;
    let remaining_in = compressed_len.saturating_sub(consumed) as u64;
    let projected = if consumed > 0 && produced > 0 {
        // 9/8 margin: early bytes compress worse than the warmed-up tail, so
        // the observed ratio slightly underestimates the remainder.
        ((produced as u64).saturating_mul(remaining_in) / consumed as u64).saturating_mul(9) / 8
    } else {
        0
    };
    // Floor at the doubling step: small payloads (protocol nodes) keep their
    // old growth exactly; the projection only ever grows MORE, for the
    // high-ratio multi-MB streams it exists for.
    let min_grow = scratch.capacity().max(4096);
    let want = (projected.min(usize::MAX as u64) as usize)
        .max(min_grow)
        .min(cap - scratch.len());
    scratch.reserve(want);
}

/// Decompress zlib data using a pooled decompressor.
///
/// Reuses the per-thread `zlib_rs::Inflate` internal state (~48 KB) across
/// calls. The output buffer is taken by the caller (zero-copy), so it is sized
/// up-front from the compressed length to avoid repeated doubling reallocations
/// while it grows to the decompressed size.
pub fn decompress_zlib_pooled(compressed: &[u8], max_size: u64) -> io::Result<Vec<u8>> {
    DECOMPRESSOR.with(|cell| {
        let (decompressor, scratch) = &mut *cell.borrow_mut();
        decompressor.reset(ZLIB_HEADER);
        scratch.clear();

        // Cap output growth to max_size + 1 so we detect oversized payloads
        // without allocating unbounded memory from a compressed bomb.
        let cap = (max_size as usize).saturating_add(1);

        // Pre-size the output near the likely decompressed size to avoid the
        // repeated doubling reallocations the old 64 KB upper clamp forced for
        // every multi-MB history-sync chunk. 2x the compressed length is a
        // conservative first guess (zlib here compresses ~2-5x): it rarely
        // overshoots the real size, so it cuts reallocations without inflating
        // peak memory. Bounded by `cap` so a bad guess can't exceed the limit;
        // the floor also bows to `cap` because callers now pass exact (possibly
        // tiny) decompressed sizes as the limit, where a fixed 4096 floor would
        // invert the clamp and panic.
        let floor = 4096.min(cap);
        let estimated = compressed.len().saturating_mul(2).clamp(floor, cap);
        if scratch.capacity() < estimated {
            scratch.reserve(estimated - scratch.capacity());
        }

        let mut input_offset = 0;
        loop {
            // Enforce cap before we grow the buffer for the next inflate call
            if scratch.len() >= cap {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("decompressed payload exceeds {max_size} bytes"),
                ));
            }

            let prev_in = decompressor.total_in();
            let prev_out = decompressor.total_out();

            let status = inflate_into_spare(
                decompressor,
                &compressed[input_offset..],
                scratch,
                InflateFlush::Finish,
            )
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e.as_str()))?;

            input_offset = decompressor.total_in() as usize;

            if scratch.len() as u64 > max_size {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("decompressed payload exceeds {max_size} bytes"),
                ));
            }

            match status {
                Status::StreamEnd => break,
                Status::Ok => {
                    grow_by_observed_ratio(scratch, decompressor, compressed.len(), cap);
                }
                Status::BufError => {
                    if decompressor.total_in() == prev_in && decompressor.total_out() == prev_out {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "zlib stream truncated (no progress)",
                        ));
                    }
                    grow_by_observed_ratio(scratch, decompressor, compressed.len(), cap);
                }
            }
        }

        // Move the Vec out (zero-copy), then restore scratch with fresh capacity.
        // Callers (unpack_bytes, history_sync) wrap in Bytes::from() which takes
        // ownership of the Vec's allocation, so no extra copy occurs.
        let result = std::mem::take(scratch);
        // Pre-allocate for next call so the first decompress_vec doesn't start at 0
        scratch.reserve(4096);
        Ok(result)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::write::ZlibEncoder;
    use std::io::Write;

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = ZlibEncoder::new(Vec::new(), Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    fn varied(n: usize) -> Vec<u8> {
        let mut s: u64 = 0x9e37_79b9_7f4a_7c15;
        (0..n)
            .map(|_| {
                s ^= s << 13;
                s ^= s >> 7;
                s ^= s << 17;
                (s >> 24) as u8
            })
            .collect()
    }

    /// A zlib stream carrying `data` verbatim in one stored (uncompressed)
    /// deflate block, hand-built so the fixture costs no compressor.
    ///
    /// `zlib()` above cannot be used under Miri: zlib-rs 0.6.6's *deflate* state
    /// frees its buffers from `deflate::end` while a `&mut` into them is still
    /// protected, which Miri rejects. That is the compression half, which this
    /// crate never runs — inflate is the whole production path — so the fixture
    /// side steps around it rather than the test being dropped.
    fn stored_zlib(data: &[u8]) -> Vec<u8> {
        assert!(data.len() <= u16::MAX as usize, "one stored block only");
        // 0x78 0x01: deflate, 32 KB window, and (0x78 << 8 | 0x01) % 31 == 0 as
        // the header check requires.
        let mut out = vec![0x78, 0x01];
        let len = data.len() as u16;
        // BFINAL=1, BTYPE=00 (stored), then the byte-aligned LEN/!LEN pair.
        out.push(0x01);
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&(!len).to_le_bytes());
        out.extend_from_slice(data);

        let (mut a, mut b) = (1u32, 0u32);
        for &byte in data {
            a = (a + byte as u32) % 65521;
            b = (b + a) % 65521;
        }
        out.extend_from_slice(&(((b << 16) | a).to_be_bytes()));
        out
    }

    // Tests sized in hundreds of KB to MB reach window refill and the growth
    // projection, which puts a full inflate cycle hours out of reach of Miri's
    // interpreter, so those are `#[cfg_attr(miri, ignore)]`. This one and the
    // truncated/corrupt pair keep the `set_len` in `inflate_into_spare` under
    // Miri on fixtures it can finish.
    #[test]
    fn pooled_roundtrip_small_input() {
        let original = varied(1024);
        let compressed = stored_zlib(&original);
        assert_eq!(
            decompress_zlib_pooled(&compressed, 64 * 1024).unwrap(),
            original
        );
        assert_eq!(drain_reader(&compressed, original.len()), original);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn inflate_reader_roundtrip_across_chunks() {
        // >128 KB so the stream spans multiple 64 KB decompress windows, and read
        // it back in tiny odd steps to exercise refill + compaction.
        let original = varied(200 * 1024);
        let compressed = zlib(&original);
        let mut r = InflateReader::new(&compressed, 64 * 1024 * 1024);
        let mut out = Vec::with_capacity(original.len());
        while r.ensure(1).unwrap() {
            let n = r.available().len().min(7);
            out.extend_from_slice(&r.available()[..n]);
            r.consume(n);
        }
        assert!(r.is_done());
        assert_eq!(out, original);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn inflate_reader_ensure_larger_than_chunk() {
        // A single record bigger than the 64 KB window must be fully buffered.
        let original: Vec<u8> = (0..150 * 1024).map(|i| (i % 256) as u8).collect();
        let compressed = zlib(&original);
        let mut r = InflateReader::new(&compressed, 64 * 1024 * 1024);
        assert!(r.ensure(150 * 1024).unwrap());
        assert_eq!(&r.available()[..150 * 1024], &original[..]);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn inflate_reader_keeps_one_window_for_smaller_records() {
        const RECORD: usize = 30 * 1024;
        let original = varied(RECORD * 8);
        let compressed = zlib(&original);
        let mut r = InflateReader::new(&compressed, 64 * 1024 * 1024);

        for expected in original.chunks(RECORD) {
            assert!(r.ensure(expected.len()).unwrap());
            assert_eq!(&r.available()[..expected.len()], expected);
            r.consume(expected.len());
            assert!(
                r.buf.capacity() <= InflateReader::CHUNK,
                "sub-window records grew the inflate buffer to {} bytes",
                r.buf.capacity()
            );
        }
        assert!(!r.ensure(1).unwrap());
        assert!(r.is_done());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn inflate_reader_enforces_max() {
        let original = vec![0u8; 1024 * 1024];
        let compressed = zlib(&original);
        let mut r = InflateReader::new(&compressed, 4096);
        assert!(r.ensure(1024 * 1024).is_err());
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn pooled_high_ratio_stream_roundtrips() {
        // ~50x expansion: the 2x up-front guess undershoots badly, so this
        // exercises the ratio-projected growth path end to end.
        let original: Vec<u8> = (0..4_000_000u32).map(|i| ((i / 1024) % 7) as u8).collect();
        let compressed = zlib(&original);
        assert!(
            compressed.len() < original.len() / 20,
            "fixture not high-ratio"
        );
        let out = decompress_zlib_pooled(&compressed, 64 * 1024 * 1024).unwrap();
        assert_eq!(out, original);
        // The projection should land near the real size, not at a doubling
        // overshoot far past it.
        assert!(
            out.capacity() < original.len() * 2,
            "capacity {} vs data {}",
            out.capacity(),
            original.len()
        );
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn pooled_oneshot_matches_streaming() {
        let original = varied(100_000);
        let compressed = zlib(&original);
        let one_shot = decompress_zlib_pooled(&compressed, 64 * 1024 * 1024).unwrap();
        assert_eq!(one_shot, original);
    }

    fn drain_reader(compressed: &[u8], n: usize) -> Vec<u8> {
        let mut r = InflateReader::new(compressed, 64 * 1024 * 1024);
        let mut out = Vec::with_capacity(n);
        while r.ensure(1).unwrap() {
            let take = r.available().len();
            out.extend_from_slice(r.available());
            r.consume(take);
        }
        assert!(r.is_done());
        out
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn inflate_reader_reuses_pool_state_correctly() {
        // Back-to-back readers each checkout the pooled Decompress and reset it, so
        // no state may carry over between streams. Verify several sizes in sequence.
        for n in [10_000usize, 250_000, 1, 80_000] {
            let original = varied(n);
            assert_eq!(drain_reader(&zlib(&original), n), original, "size {n}");
        }
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn inflate_reader_reuse_after_error() {
        // A reader aborted mid-stream (max exceeded) returns partial zlib state to
        // the pool; the next checkout must reset it and decompress a full stream.
        {
            let compressed = zlib(&varied(500_000));
            let mut r = InflateReader::new(&compressed, 4096);
            assert!(r.ensure(500_000).is_err());
        }
        let original = varied(120_000);
        assert_eq!(drain_reader(&zlib(&original), 120_000), original);
    }

    #[test]
    #[cfg_attr(miri, ignore)]
    fn oversized_window_is_not_retained_for_the_thread() {
        // Buffering a large record grows `buf` to many MB. The window is not part
        // of a pool entry, so the next reader starts back at one window instead of
        // inheriting the grown allocation.
        let big = varied(2 * 1024 * 1024);
        let compressed = zlib(&big);
        {
            let mut r = InflateReader::new(&compressed, 64 * 1024 * 1024);
            assert!(r.ensure(big.len()).unwrap());
            assert!(r.buf.capacity() >= big.len(), "buf should grow while alive");
        }
        let next = InflateReader::new(&compressed, 64 * 1024 * 1024);
        assert!(
            next.buf.capacity() <= InflateReader::CHUNK,
            "fresh reader inherited a {} byte window",
            next.buf.capacity()
        );
    }

    /// Truncation is reported as EOF without a terminator, not as an error, and
    /// the reader that picks the pooled state up next still decodes a full stream.
    #[test]
    fn truncated_stream_reports_missing_terminator() {
        let original = varied(1024);
        let compressed = stored_zlib(&original);
        let truncated = &compressed[..compressed.len() - 300];

        let mut r = InflateReader::new(truncated, 64 * 1024);
        while r.ensure(1).unwrap() {
            let n = r.available().len();
            r.consume(n);
        }
        assert!(!r.stream_ended(), "truncated input reported a terminator");
        assert!(r.is_done());
        drop(r);

        assert_eq!(drain_reader(&compressed, original.len()), original);
    }

    /// A corrupt stream fails with `InvalidData` and returns its half-used state
    /// to the pool; the next checkout must reset it rather than inherit the fault.
    #[test]
    fn corrupt_stream_errors_without_poisoning_the_pool() {
        let original = varied(1024);
        let mut compressed = stored_zlib(&original);
        // Break the stored block's LEN/!LEN complement pair, which inflate
        // rejects outright rather than mis-decoding.
        compressed[5] ^= 0xff;

        let mut r = InflateReader::new(&compressed, 64 * 1024);
        let err = r.ensure(1).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidData);
        drop(r);

        let good = stored_zlib(&original);
        assert_eq!(drain_reader(&good, original.len()), original);
    }
}
