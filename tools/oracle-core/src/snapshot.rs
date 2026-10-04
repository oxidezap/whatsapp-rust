//! Read-only guest memory capture at instrumented instruction boundaries.

use std::collections::BTreeMap;
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};

/// Independent bound on captured records. Zero-length spans never grow
/// `bytes`, so the byte budget alone cannot stop a looping guest from
/// exhausting host memory with heap-backed entries.
pub(crate) const MAX_SNAPSHOT_RECORDS: usize = 65_536;

#[derive(Debug)]
pub(crate) struct Span {
    pub at: u32,
    pub scalar: bool,
    pub nullable: bool,
    pub len: u32,
    pub count: usize,
    pub out: String,
}

#[derive(Debug)]
struct Captured {
    records: BTreeMap<i32, Vec<Vec<u8>>>,
    bytes: usize,
    total: usize,
    error: Option<String>,
}

#[derive(Debug)]
pub(crate) struct Recorder {
    symbol: String,
    spans: BTreeMap<i32, Span>,
    captured: Mutex<Captured>,
}

impl Recorder {
    pub fn new(symbol: String, spans: BTreeMap<i32, Span>) -> Self {
        Self {
            symbol,
            spans,
            captured: Mutex::new(Captured {
                records: BTreeMap::new(),
                bytes: 0,
                total: 0,
                error: None,
            }),
        }
    }

    pub fn record(&self, state: &crate::state::HostState, module: &str, name: &str, args: &[i64]) {
        if self.symbol != format!("{module}::{name}") {
            return;
        }
        let [id, base, ..] = args else {
            return;
        };
        let Some(span) = self.spans.get(&(*id as i32)) else {
            return;
        };
        let mut captured = self.captured.lock().unwrap_or_else(|e| e.into_inner());
        if captured.error.is_some() {
            return;
        }
        let result = (|| -> Result<Vec<u8>> {
            let absent = !span.scalar && span.nullable && *base as u32 == 0;
            let len = if absent { 0 } else { span.len as usize };
            anyhow::ensure!(
                captured.bytes + len <= 64 * 1024 * 1024,
                "snapshot budget exceeds 64 MiB"
            );
            anyhow::ensure!(
                captured.total < MAX_SNAPSHOT_RECORDS,
                "snapshot record budget exceeded"
            );
            anyhow::ensure!(
                captured.records.get(&(*id as i32)).map_or(0, Vec::len) < span.count,
                "too many hits for {}",
                span.out
            );
            if span.scalar {
                return Ok((*base as u32).to_le_bytes().to_vec());
            }
            if absent {
                return Ok(Vec::new());
            }
            let ptr = (*base as u32)
                .checked_add(span.at)
                .context("snapshot address overflow")?;
            state.read(ptr, span.len)
        })();
        match result {
            Ok(bytes) => {
                captured.bytes += bytes.len();
                captured.total += 1;
                captured.records.entry(*id as i32).or_default().push(bytes);
            }
            Err(error) => captured.error = Some(format!("snapshot {}: {error:#}", span.out)),
        }
    }

    pub fn finish(&self) -> Result<Vec<(String, Vec<u8>)>> {
        let mut captured = self.captured.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(error) = &captured.error {
            return Err(anyhow!(error.clone()));
        }
        let mut outputs = Vec::new();
        for (id, span) in &self.spans {
            let records = captured.records.remove(id).unwrap_or_default();
            anyhow::ensure!(
                records.len() == span.count,
                "snapshot {}: expected {} hits, got {}",
                span.out,
                span.count,
                records.len()
            );
            for (i, bytes) in records.into_iter().enumerate() {
                outputs.push((output_name(&span.out, i), bytes));
            }
        }
        Ok(outputs)
    }
}

pub(crate) fn output_name(prefix: &str, index: usize) -> String {
    format!("{prefix}_{index:04}.bin")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn memory_recorder(nullable: bool, at: u32, count: usize) -> Recorder {
        Recorder::new(
            "probe::hit".to_owned(),
            BTreeMap::from([(
                0,
                Span {
                    at,
                    scalar: false,
                    nullable,
                    len: 4,
                    count,
                    out: "span".to_owned(),
                },
            )]),
        )
    }

    #[test]
    fn nullable_null_records_absence_without_reading_or_offsetting() {
        // No memory is mapped; even `at` is outside any possible span.
        let state = crate::state::HostState::default();
        let recorder = memory_recorder(true, u32::MAX, 1);
        recorder.record(&state, "probe", "hit", &[0, 0]);
        assert_eq!(
            recorder.finish().unwrap(),
            vec![("span_0000.bin".to_owned(), vec![])]
        );
        let required = memory_recorder(false, 0, 1);
        required.record(&state, "probe", "hit", &[0, 0]);
        assert!(required.finish().is_err());
    }

    #[test]
    fn nullable_capture_preserves_present_memory_and_raw_zero_address_reads() {
        let (_local, _cross_process) = crate::test_common::threaded_guard();
        let engine = crate::host::build_engine().unwrap();
        let memory =
            wasmtime::SharedMemory::new(&engine, wasmtime::MemoryType::shared(1, 1)).unwrap();
        let mut state =
            crate::state::HostState::for_thread(Default::default(), 0, Default::default());
        state.memory = Some(memory);
        state.write(0, b"NULL").unwrap();
        state.write(20, b"data").unwrap();
        let recorder = memory_recorder(true, 4, 2);
        recorder.record(&state, "probe", "hit", &[0, 0]);
        recorder.record(&state, "probe", "hit", &[0, 16]);
        assert_eq!(
            recorder.finish().unwrap(),
            vec![
                ("span_0000.bin".to_owned(), vec![]),
                ("span_0001.bin".to_owned(), b"data".to_vec()),
            ]
        );
        assert_eq!(state.read(0, 4).unwrap(), b"NULL");
        assert_eq!(state.read(20, 4).unwrap(), b"data");
        let raw = memory_recorder(false, 0, 1);
        raw.record(&state, "probe", "hit", &[0, 0]);
        assert_eq!(raw.finish().unwrap()[0].1, b"NULL");
        let invalid = memory_recorder(true, 0, 1);
        invalid.record(&state, "probe", "hit", &[0, 65_535]);
        assert!(invalid.finish().is_err(), "nonnull OOB remains an error");
        let overflow = memory_recorder(true, u32::MAX, 1);
        overflow.record(&state, "probe", "hit", &[0, 1]);
        assert!(
            overflow.finish().is_err(),
            "nonnull address overflow remains an error"
        );
    }

    #[test]
    fn absent_records_still_require_exact_hits_and_obey_the_record_budget() {
        let state = crate::state::HostState::default();
        let missing = memory_recorder(true, 0, 2);
        missing.record(&state, "probe", "hit", &[0, 0]);
        assert!(missing.finish().is_err());
        let extra = memory_recorder(true, 0, 1);
        for _ in 0..2 {
            extra.record(&state, "probe", "hit", &[0, 0]);
        }
        assert!(extra.finish().is_err());
        let bounded = memory_recorder(true, 0, MAX_SNAPSHOT_RECORDS + 1);
        for _ in 0..=MAX_SNAPSHOT_RECORDS {
            bounded.record(&state, "probe", "hit", &[0, 0]);
        }
        assert!(bounded.finish().is_err());
    }
    #[test]
    fn records_are_bounded_independently_of_bytes() {
        // 70_000 scalar hits fit the 64 MiB byte budget and the per-span
        // count, so only an independent record budget can stop them.
        let recorder = Recorder::new(
            "probe::hit".to_owned(),
            BTreeMap::from([(
                0,
                Span {
                    at: 0,
                    scalar: true,
                    nullable: false,
                    len: 0,
                    count: 70_000,
                    out: "span".to_owned(),
                },
            )]),
        );
        let state = crate::state::HostState::default();
        for _ in 0..70_000 {
            recorder.record(&state, "probe", "hit", &[0, 0]);
        }
        assert!(
            recorder.finish().is_err(),
            "record count must be bounded independently of bytes"
        );
    }
}
