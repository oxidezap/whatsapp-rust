//! Byte accounting for media readers and writers, independent of an HTTP runtime.
use std::io::{self, Read, Write};
use std::sync::{Arc, Mutex};

/// Per-download application policy, independent of declared sizes and headers.
///
/// `Default::default()` adds no media-layer byte cap. The examined Web full-download
/// path does not apply its upload-size gates; HTTP adapters may have separate caps.
/// [`Self::new`] bounds wire bytes, including padding and MAC, and plaintext bytes.
/// Both totals include failed attempts. This optional application policy prevents
/// failover from multiplying the requested budget; it is not a Web retry rule.
/// Bounded downloads require a streaming HTTP adapter and fail before HTTP when
/// unavailable. Dropping the future stops active streaming processing at
/// cooperative I/O checkpoints. It cannot interrupt a blocked adapter/writer
/// call; already-scheduled failure cleanup may continue. Sink cleanup and
/// publication safety still require the writer contract on error/cancellation.
/// Limits count bytes consumed by the media layer and emitted to the sink, not
/// adapter/TLS buffers, allocator capacity, or RSS. A small budget therefore also
/// needs a bounded streaming adapter and a destination suited to the platform.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DownloadLimits {
    bounds: Option<(u64, u64)>,
}

impl DownloadLimits {
    pub const fn new(max_wire_bytes: u64, max_plaintext_bytes: u64) -> Self {
        Self {
            bounds: Some((max_wire_bytes, max_plaintext_bytes)),
        }
    }

    pub(super) const fn unlimited() -> Self {
        Self { bounds: None }
    }

    pub(super) const fn is_bounded(self) -> bool {
        self.bounds.is_some()
    }

    pub(super) fn plaintext_capacity(self, declared: u64) -> u64 {
        // CBC decryption and plaintext copying cannot expand their input.
        self.bounds.map_or(declared, |(wire, plaintext)| {
            declared.min(wire).min(plaintext)
        })
    }
}

/// The byte domain whose application budget was exhausted.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MediaByteDomain {
    Wire,
    Plaintext,
}

/// A download policy decision. SDK-origin decisions stop host failover;
/// adapter and writer errors retain their I/O origin even if they contain this type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum DownloadPolicyError {
    #[error("media {domain:?} byte budget exceeded ({limit} bytes)")]
    LimitExceeded { domain: MediaByteDomain, limit: u64 },
    #[error("bounded media downloads require a streaming HTTP adapter")]
    StreamingRequired,
    #[error("media download cancelled")]
    Cancelled,
}

// The public policy type can also appear in third-party I/O errors. Only this
// private marker proves that the active SDK budget produced the failure.
#[derive(Debug, thiserror::Error)]
#[error("{0}")]
pub(super) struct BudgetPolicyCause(#[source] pub(super) DownloadPolicyError);

fn policy_io(error: DownloadPolicyError) -> io::Error {
    io::Error::other(BudgetPolicyCause(error))
}

#[derive(Default)]
struct State {
    wire: u64,
    plaintext: u64,
    cancelled: bool,
}

/// One operation's accounting, shared across readers, writers and retries.
/// No file-sized allocation or 64-bit atomic is needed. The caller owns cleanup
/// of unverified output, including on cancellation. Use one reader and one
/// writer at a time; clones retain accounting across sequential attempts.
/// Reads allow a one-byte overflow probe, which is never forwarded. Cancellation
/// takes effect before the next I/O call and cannot interrupt a blocked read.
#[derive(Clone)]
pub(super) struct DownloadBudget {
    limits: DownloadLimits,
    state: Option<Arc<Mutex<State>>>,
}

impl DownloadBudget {
    pub(super) fn new(limits: DownloadLimits) -> Self {
        Self {
            limits,
            state: limits
                .is_bounded()
                .then(|| Arc::new(Mutex::new(State::default()))),
        }
    }

    /// Cancel a bounded operation at its next I/O boundary. Unlimited budgets
    /// have no shared state and do not support cancellation.
    pub(super) fn cancel(&self) {
        let Some(state) = &self.state else {
            return;
        };
        state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .cancelled = true;
    }

    /// Check before starting other blocking I/O owned by this operation.
    pub(super) fn check_cancelled(&self) -> Result<(), DownloadPolicyError> {
        if let Some(state) = &self.state
            && state
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .cancelled
        {
            return Err(DownloadPolicyError::Cancelled);
        }
        Ok(())
    }

    pub(super) fn reader<R: Read>(&self, inner: R) -> impl Read + use<R> {
        BudgetReader {
            inner,
            budget: self.clone(),
        }
    }

    pub(super) fn writer<W: Write>(&self, inner: W) -> impl Write + use<W> {
        BudgetWriter {
            inner,
            budget: self.clone(),
        }
    }

    fn remaining(&self, domain: MediaByteDomain) -> io::Result<u64> {
        let Some(state) = &self.state else {
            return Ok(u64::MAX);
        };
        let state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.cancelled {
            return Err(policy_io(DownloadPolicyError::Cancelled));
        }
        Ok(match (self.limits.bounds, domain) {
            (Some((limit, _)), MediaByteDomain::Wire) => limit.saturating_sub(state.wire),
            (Some((_, limit)), MediaByteDomain::Plaintext) => limit.saturating_sub(state.plaintext),
            (None, _) => u64::MAX,
        })
    }

    fn charge(&self, domain: MediaByteDomain, n: usize) {
        let Some(state) = &self.state else {
            return;
        };
        let mut state = state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let count = match domain {
            MediaByteDomain::Wire => &mut state.wire,
            MediaByteDomain::Plaintext => &mut state.plaintext,
        };
        *count = count.saturating_add(n as u64);
    }

    fn exceeded(&self, domain: MediaByteDomain) -> io::Error {
        let limit = self
            .limits
            .bounds
            .map_or(u64::MAX, |(wire, plain)| match domain {
                MediaByteDomain::Wire => wire,
                MediaByteDomain::Plaintext => plain,
            });
        policy_io(DownloadPolicyError::LimitExceeded { domain, limit })
    }
}

struct BudgetReader<R> {
    inner: R,
    budget: DownloadBudget,
}

impl<R: Read> Read for BudgetReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if buf.is_empty() {
            return Ok(0);
        }
        let remaining = self.budget.remaining(MediaByteDomain::Wire)?;
        if remaining == 0 {
            // Probe one byte to distinguish exact-boundary EOF from overflow.
            // Never synthesize EOF: that could authenticate a truncated prefix.
            let n = self.inner.read(&mut [0u8; 1])?;
            return if n == 0 {
                Ok(0)
            } else {
                Err(self.budget.exceeded(MediaByteDomain::Wire))
            };
        }
        let len = buf
            .len()
            .min(usize::try_from(remaining).unwrap_or(usize::MAX));
        let n = self.inner.read(&mut buf[..len])?;
        self.budget.charge(MediaByteDomain::Wire, n);
        Ok(n)
    }
}

struct BudgetWriter<W> {
    inner: W,
    budget: DownloadBudget,
}

impl<W: Write> Write for BudgetWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let remaining = self.budget.remaining(MediaByteDomain::Plaintext)?;
        if buf.len() as u64 > remaining {
            return Err(self.budget.exceeded(MediaByteDomain::Plaintext));
        }
        let n = self.inner.write(buf)?;
        self.budget.charge(MediaByteDomain::Plaintext, n);
        Ok(n)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.budget.check_cancelled().map_err(policy_io)?;
        self.inner.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn policy(error: &io::Error) -> DownloadPolicyError {
        error
            .get_ref()
            .unwrap()
            .downcast_ref::<BudgetPolicyCause>()
            .unwrap()
            .0
    }

    #[test]
    fn wire_boundary_requires_real_eof() {
        for len in [0, 1, 8192] {
            let budget = DownloadBudget::new(DownloadLimits::new(len, len));
            let mut reader = budget.reader(Cursor::new(vec![7; len as usize]));
            let mut out = Vec::new();
            reader.read_to_end(&mut out).unwrap();
            assert_eq!(out.len(), len as usize);
            let budget = DownloadBudget::new(DownloadLimits::new(len, len));
            let mut reader = budget.reader(Cursor::new(vec![7; len as usize + 1]));
            assert_eq!(
                policy(&reader.read_to_end(&mut Vec::new()).unwrap_err()),
                DownloadPolicyError::LimitExceeded {
                    domain: MediaByteDomain::Wire,
                    limit: len
                }
            );
        }
    }

    #[test]
    fn retry_accounting_and_cancellation_survive_clones() {
        let budget = DownloadBudget::new(DownloadLimits::new(4, 4));
        budget
            .reader(Cursor::new(b"ab"))
            .read_exact(&mut [0; 2])
            .unwrap();
        let mut retry = budget.clone().reader(Cursor::new(b"cde"));
        retry.read_exact(&mut [0; 2]).unwrap();
        assert!(matches!(
            policy(&retry.read(&mut [0]).unwrap_err()),
            DownloadPolicyError::LimitExceeded { .. }
        ));
        let mut sink = Vec::new();
        budget.writer(&mut sink).write_all(b"abcd").unwrap();
        assert!(budget.writer(&mut sink).write_all(b"e").is_err());
        assert_eq!(sink, b"abcd");
        budget.cancel();
        assert_eq!(
            policy(&budget.reader(io::empty()).read(&mut [0]).unwrap_err()),
            DownloadPolicyError::Cancelled
        );
        assert_eq!(
            policy(&budget.writer(io::sink()).write(b"x").unwrap_err()),
            DownloadPolicyError::Cancelled
        );
    }

    #[test]
    fn cancelled_flush_does_not_touch_the_sink() {
        struct FlushSink(bool);
        impl Write for FlushSink {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                Ok(bytes.len())
            }
            fn flush(&mut self) -> io::Result<()> {
                self.0 = true;
                Ok(())
            }
        }
        let budget = DownloadBudget::new(DownloadLimits::new(4, 4));
        let mut sink = FlushSink(false);
        budget.cancel();
        assert_eq!(
            policy(&budget.writer(&mut sink).flush().unwrap_err()),
            DownloadPolicyError::Cancelled
        );
        assert!(!sink.0);
    }

    #[test]
    fn endless_source_stops_after_limit_plus_one_without_growing_output() {
        struct Endless(usize);
        impl Read for Endless {
            fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
                self.0 += bytes.len();
                bytes.fill(1);
                Ok(bytes.len())
            }
        }
        let mut source = Endless(0);
        let budget = DownloadBudget::new(DownloadLimits::new(19, 19));
        assert!(io::copy(&mut budget.reader(&mut source), &mut io::sink()).is_err());
        assert_eq!(source.0, 20);
    }
}
