use flate2::{Decompress, FlushDecompress, Status};
use std::cell::RefCell;
use std::io;

thread_local! {
    static DECOMPRESSOR: RefCell<(Decompress, Vec<u8>)> = RefCell::new((
        Decompress::new(true),
        Vec::with_capacity(4096),
    ));
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
    decomp: Decompress,
    buf: Vec<u8>,
    cursor: usize,
    total_out: u64,
    max: u64,
    eof: bool,
}

impl<'a> InflateReader<'a> {
    /// Output decompress window per pump; also the compaction threshold.
    const CHUNK: usize = 64 * 1024;

    pub fn new(input: &'a [u8], max: u64) -> Self {
        Self {
            input,
            in_pos: 0,
            decomp: Decompress::new(true),
            buf: Vec::with_capacity(Self::CHUNK),
            cursor: 0,
            total_out: 0,
            max,
            eof: false,
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

    fn pump(&mut self) -> io::Result<()> {
        // Drop the consumed prefix before growing, so the buffer holds roughly
        // just the record currently being accumulated.
        if self.cursor >= Self::CHUNK || self.cursor == self.buf.len() {
            self.buf.drain(..self.cursor);
            self.cursor = 0;
        }

        let mut chunk = [0u8; Self::CHUNK];
        let prev_in = self.decomp.total_in();
        let prev_out = self.decomp.total_out();
        let status = self
            .decomp
            .decompress(
                &self.input[self.in_pos..],
                &mut chunk,
                FlushDecompress::None,
            )
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        let produced = (self.decomp.total_out() - prev_out) as usize;
        self.in_pos += (self.decomp.total_in() - prev_in) as usize;
        self.total_out += produced as u64;
        if self.total_out > self.max {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!("decompressed payload exceeds {} bytes", self.max),
            ));
        }
        self.buf.extend_from_slice(&chunk[..produced]);

        match status {
            Status::StreamEnd => self.eof = true,
            // No output produced and not at stream end: distinguish a truncated
            // tail (no input left → treat as end) from a stalled/corrupt stream
            // (input remains but the decompressor consumed none → error, instead
            // of spinning forever since 64 KB of output is always available).
            // Mirrors the no-progress guard in `decompress_zlib_pooled`.
            _ if produced == 0 => {
                if self.in_pos >= self.input.len() {
                    self.eof = true;
                } else if self.decomp.total_in() == prev_in {
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

/// Decompress zlib data using a pooled decompressor.
///
/// Reuses the per-thread `flate2::Decompress` internal state (~48 KB) across
/// calls. The output buffer is taken by the caller (zero-copy), so it is sized
/// up-front from the compressed length to avoid repeated doubling reallocations
/// while it grows to the decompressed size.
pub fn decompress_zlib_pooled(compressed: &[u8], max_size: u64) -> io::Result<Vec<u8>> {
    DECOMPRESSOR.with(|cell| {
        let (decompressor, scratch) = &mut *cell.borrow_mut();
        decompressor.reset(true);
        scratch.clear();

        // Cap output growth to max_size + 1 so we detect oversized payloads
        // without allocating unbounded memory from a compressed bomb.
        let cap = (max_size as usize).saturating_add(1);

        // Pre-size the output near the likely decompressed size to avoid the
        // repeated doubling reallocations the old 64 KB upper clamp forced for
        // every multi-MB history-sync chunk. 2x the compressed length is a
        // conservative first guess (zlib here compresses ~2-5x): it rarely
        // overshoots the real size, so it cuts reallocations without inflating
        // peak memory. Bounded by `cap` so a bad guess can't exceed the limit.
        let estimated = compressed.len().saturating_mul(2).clamp(4096, cap);
        if scratch.capacity() < estimated {
            scratch.reserve(estimated - scratch.capacity());
        }

        let mut input_offset = 0;
        loop {
            // Enforce cap before decompress_vec can grow the buffer
            if scratch.len() >= cap {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("decompressed payload exceeds {max_size} bytes"),
                ));
            }

            let prev_in = decompressor.total_in();
            let prev_out = decompressor.total_out();

            let status = decompressor
                .decompress_vec(
                    &compressed[input_offset..],
                    scratch,
                    FlushDecompress::Finish,
                )
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

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
                    // Grow but never past the cap
                    let want = scratch.capacity().max(4096).min(cap - scratch.len());
                    scratch.reserve(want);
                }
                Status::BufError => {
                    if decompressor.total_in() == prev_in && decompressor.total_out() == prev_out {
                        return Err(io::Error::new(
                            io::ErrorKind::InvalidData,
                            "zlib stream truncated (no progress)",
                        ));
                    }
                    let want = scratch.capacity().max(4096).min(cap - scratch.len());
                    scratch.reserve(want);
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
    fn pooled_oneshot_matches_streaming() {
        let original = varied(100_000);
        let compressed = zlib(&original);
        let one_shot = decompress_zlib_pooled(&compressed, 64 * 1024 * 1024).unwrap();
        assert_eq!(one_shot, original);
    }
}
