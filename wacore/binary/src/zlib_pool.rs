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

    // Free-list of streaming-reader state (inflate state ~48 KB + 64 KB buf). A
    // connection's bootstrap history sync decompresses several blobs sequentially,
    // each via a fresh `InflateReader`; reusing the state avoids re-initializing
    // zlib and re-allocating the buffer per blob.
    static INFLATE_POOL: RefCell<Vec<(Inflate, Vec<u8>)>> = const { RefCell::new(Vec::new()) };
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
    /// Output decompress window per pump; also the compaction threshold.
    const CHUNK: usize = 64 * 1024;
    /// Cap on retained free-list entries, so concurrently-alive readers on one
    /// thread don't grow the pool unbounded.
    const POOL_MAX: usize = 4;

    pub fn new(input: &'a [u8], max: u64) -> Self {
        let (decomp, buf) = INFLATE_POOL.with(|p| p.borrow_mut().pop()).map_or_else(
            || {
                (
                    Inflate::new(ZLIB_HEADER, WINDOW_BITS),
                    Vec::with_capacity(Self::CHUNK),
                )
            },
            |(mut decomp, mut buf)| {
                decomp.reset(ZLIB_HEADER);
                buf.clear();
                (decomp, buf)
            },
        );
        Self {
            input,
            in_pos: 0,
            decomp: Some(decomp),
            buf,
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

    /// Whether zlib reported a proper stream end (terminator + adler32
    /// checksum). An EOF (`ensure` returning false) without this means the
    /// input was truncated, not finished.
    pub fn stream_ended(&self) -> bool {
        self.stream_end
    }

    fn pump(&mut self) -> io::Result<()> {
        // Drop the consumed prefix before growing, so the buffer holds roughly
        // just the record currently being accumulated.
        if self.cursor >= Self::CHUNK || self.cursor == self.buf.len() {
            self.buf.drain(..self.cursor);
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
        self.buf.reserve(Self::CHUNK);
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
        // Return the decompressor + buffer to the per-thread free-list for reuse.
        // `reset` on the next checkout makes prior stream state (incl. errors) moot.
        if let Some(decomp) = self.decomp.take() {
            let mut buf = std::mem::take(&mut self.buf);
            // A large top-level record (e.g. a big conversation, up to `max`) can
            // grow `buf` to many MB; don't retain that allocation in the pool for
            // the thread's lifetime. Shrink back toward the normal working size.
            buf.clear();
            buf.shrink_to(Self::CHUNK);
            INFLATE_POOL.with(|p| {
                let mut pool = p.borrow_mut();
                if pool.len() < Self::POOL_MAX {
                    pool.push((decomp, buf));
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

    #[test]
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
    fn inflate_reader_ensure_larger_than_chunk() {
        // A single record bigger than the 64 KB window must be fully buffered.
        let original: Vec<u8> = (0..150 * 1024).map(|i| (i % 256) as u8).collect();
        let compressed = zlib(&original);
        let mut r = InflateReader::new(&compressed, 64 * 1024 * 1024);
        assert!(r.ensure(150 * 1024).unwrap());
        assert_eq!(&r.available()[..150 * 1024], &original[..]);
    }

    #[test]
    fn inflate_reader_enforces_max() {
        let original = vec![0u8; 1024 * 1024];
        let compressed = zlib(&original);
        let mut r = InflateReader::new(&compressed, 4096);
        assert!(r.ensure(1024 * 1024).is_err());
    }

    #[test]
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
    fn inflate_reader_reuses_pool_state_correctly() {
        // Back-to-back readers each checkout the pooled Decompress and reset it, so
        // no state may carry over between streams. Verify several sizes in sequence.
        for n in [10_000usize, 250_000, 1, 80_000] {
            let original = varied(n);
            assert_eq!(drain_reader(&zlib(&original), n), original, "size {n}");
        }
    }

    #[test]
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
    fn drop_shrinks_oversized_buffer_before_pooling() {
        // Buffering a large record grows `buf` to many MB; on return to the pool it
        // must be shrunk back toward CHUNK, not parked at full size for the thread.
        INFLATE_POOL.with(|p| p.borrow_mut().clear());
        let big = varied(2 * 1024 * 1024);
        let compressed = zlib(&big);
        {
            let mut r = InflateReader::new(&compressed, 64 * 1024 * 1024);
            assert!(r.ensure(big.len()).unwrap());
            assert!(r.buf.capacity() >= big.len(), "buf should grow while alive");
        }
        let pooled = INFLATE_POOL.with(|p| p.borrow().last().map(|(_, b)| b.capacity()));
        assert!(
            matches!(pooled, Some(cap) if cap <= InflateReader::CHUNK * 2),
            "pooled buffer not shrunk: {pooled:?}"
        );
    }
}
