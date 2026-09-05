//! Read-only guest memory capture at instrumented instruction boundaries.

use std::collections::BTreeMap;
use std::sync::Mutex;

use anyhow::{Context, Result, anyhow};

#[derive(Debug)]
pub(crate) struct Span {
    pub at: u32,
    pub scalar: bool,
    pub len: u32,
    pub count: usize,
    pub out: String,
}

#[derive(Debug)]
struct Captured {
    records: BTreeMap<i32, Vec<Vec<u8>>>,
    bytes: usize,
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
            anyhow::ensure!(
                captured.bytes + span.len as usize <= 64 * 1024 * 1024,
                "snapshot budget exceeds 64 MiB"
            );
            anyhow::ensure!(
                captured.records.get(&(*id as i32)).map_or(0, Vec::len) < span.count,
                "too many hits for {}",
                span.out
            );
            if span.scalar {
                return Ok((*base as u32).to_le_bytes().to_vec());
            }
            let ptr = (*base as u32)
                .checked_add(span.at)
                .context("snapshot address overflow")?;
            state.read(ptr, span.len)
        })();
        match result {
            Ok(bytes) => {
                captured.bytes += bytes.len();
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
                outputs.push((format!("{}_{i:04}.bin", span.out), bytes));
            }
        }
        Ok(outputs)
    }
}
