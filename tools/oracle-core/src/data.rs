//! Data-segment extraction.
//!
//! Running `strings` over a whole `.wasm` mostly returns noise, because dense
//! integer opcodes decode as printable ASCII by accident. Restricting the scan
//! to data segments is what makes the output identify a minified module: string
//! literals, format strings and lookup tables live there and nowhere else.

use anyhow::{Context, Result};
use wasmparser::{Parser, Payload};

/// One data segment, as the module declares it.
#[derive(Debug, Clone)]
pub struct DataSegment {
    /// Its position in the data section, which is how `strings` refers to it.
    pub index: usize,
    /// Linear-memory offset, when the segment is active with a constant offset.
    pub memory_offset: Option<u64>,
    /// Length of the segment's contents, in bytes.
    pub len: usize,
}

/// A printable run found inside a data segment.
///
/// Segments only: dense wasm opcodes decode as printable ASCII by accident, so
/// scanning the whole file the way `strings(1)` does reports code as text.
#[derive(Debug, Clone)]
pub struct DataString {
    /// Index of the segment it was found in.
    pub segment: usize,
    /// Byte offset within the segment.
    pub offset: usize,
    /// The run itself.
    pub value: String,
}

/// What [`extract`] found.
#[derive(Debug, Clone, Default)]
pub struct DataReport {
    /// Every data segment, in declaration order.
    pub segments: Vec<DataSegment>,
    /// The printable runs, in the order they appear.
    pub strings: Vec<DataString>,
    /// Total size of all segment contents.
    pub total_bytes: usize,
}

/// Extracts data segments and the printable runs inside them.
pub fn extract(bytes: &[u8], min_run: usize) -> Result<DataReport> {
    let mut report = DataReport::default();

    for payload in Parser::new(0).parse_all(bytes) {
        let Payload::DataSection(reader) = payload.context("parsing wasm sections")? else {
            continue;
        };

        for (index, data) in reader.into_iter().enumerate() {
            let data = data.context("reading data segment")?;
            report.segments.push(DataSegment {
                index,
                memory_offset: const_offset(&data.kind),
                len: data.data.len(),
            });
            report.total_bytes += data.data.len();
            collect_strings(index, data.data, min_run, &mut report.strings);
        }
    }

    Ok(report)
}

/// Only constant-offset active segments have a statically known address; a
/// segment placed by a computed expression does not, and reporting a guess
/// would be worse than reporting nothing.
fn const_offset(kind: &wasmparser::DataKind<'_>) -> Option<u64> {
    let wasmparser::DataKind::Active { offset_expr, .. } = kind else {
        return None;
    };

    let mut reader = offset_expr.get_operators_reader();
    let operator = reader.read().ok()?;
    match operator {
        wasmparser::Operator::I32Const { value } => Some(value as u64),
        wasmparser::Operator::I64Const { value } => Some(value as u64),
        _ => None,
    }
}

fn collect_strings(segment: usize, data: &[u8], min_run: usize, out: &mut Vec<DataString>) {
    let mut current = String::new();
    let mut start = 0usize;

    for (offset, &byte) in data.iter().enumerate() {
        let printable = byte.is_ascii_graphic() || byte == b' ';
        if printable {
            if current.is_empty() {
                start = offset;
            }
            current.push(byte as char);
            continue;
        }
        if current.len() >= min_run {
            out.push(DataString {
                segment,
                offset: start,
                value: std::mem::take(&mut current),
            });
        } else {
            current.clear();
        }
    }

    if current.len() >= min_run {
        out.push(DataString {
            segment,
            offset: start,
            value: current,
        });
    }
}
