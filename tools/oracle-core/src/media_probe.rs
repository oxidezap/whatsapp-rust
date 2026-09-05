//! Bounded capture and exact comparison of audio/video callbacks.
//!
//! A media oracle needs the bytes crossing the guest/host boundary, not only
//! the fact that a callback ran. Watches describe which callback arguments
//! hold a payload pointer, length and optional RTP metadata. The host copies
//! those bytes while the callback is active and keeps the resulting records in
//! deterministic call order.

use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::sync::Mutex;

use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MAX_MEDIA_RECORDS: usize = 4096;
const MAX_MEDIA_PAYLOAD_BYTES: u32 = 16 * 1024 * 1024;
const MAX_MEDIA_TOTAL_BYTES: usize = 256 * 1024 * 1024;

/// Logical stream observed at a wasm callback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaStream {
    /// Encoded audio, decoded PCM or an audio transport packet.
    Audio,
    /// An encoded video access unit, fragment or video transport packet.
    Video,
}

/// A validated description of one guest-to-host media callback.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaWatch {
    symbol: String,
    stream: MediaStream,
    pointer_arg: usize,
    length_arg: usize,
    payload_offset: u32,
    sequence_arg: Option<usize>,
    timestamp_arg: Option<usize>,
}

impl MediaWatch {
    /// Describe a callback whose `pointer_arg` and `length_arg` identify its payload.
    pub fn new(
        module: &str,
        name: &str,
        stream: MediaStream,
        pointer_arg: usize,
        length_arg: usize,
    ) -> Result<Self> {
        ensure!(
            !module.is_empty()
                && !name.is_empty()
                && !module.contains("::")
                && !name.contains("::"),
            "media callback needs nonempty module and name components"
        );
        ensure!(
            pointer_arg != length_arg,
            "media callback pointer and length cannot use the same argument"
        );
        Ok(Self {
            symbol: format!("{module}::{name}"),
            stream,
            pointer_arg,
            length_arg,
            payload_offset: 0,
            sequence_arg: None,
            timestamp_arg: None,
        })
    }

    /// Add a byte offset to the callback's base pointer.
    #[must_use]
    pub fn with_payload_offset(mut self, offset: u32) -> Self {
        self.payload_offset = offset;
        self
    }

    /// Record the callback argument carrying a packet sequence number.
    #[must_use]
    pub fn with_sequence_arg(mut self, index: usize) -> Self {
        self.sequence_arg = Some(index);
        self
    }

    /// Record the callback argument carrying an RTP/media timestamp.
    #[must_use]
    pub fn with_timestamp_arg(mut self, index: usize) -> Self {
        self.timestamp_arg = Some(index);
        self
    }

    /// Fully qualified `module::name` callback symbol.
    #[must_use]
    pub fn symbol(&self) -> &str {
        &self.symbol
    }

    fn matches(&self, module: &str, name: &str) -> bool {
        self.symbol.len() == module.len() + name.len() + 2
            && self.symbol.starts_with(module)
            && self.symbol[module.len()..].starts_with("::")
            && self.symbol.ends_with(name)
    }

    fn validate(&self) -> Result<()> {
        let (module, name) = self
            .symbol
            .split_once("::")
            .context("media callback symbol has no module separator")?;
        ensure!(
            !module.is_empty() && !name.is_empty() && !name.contains("::"),
            "media callback symbol must be exactly module::name"
        );
        ensure!(
            self.pointer_arg != self.length_arg,
            "media callback pointer and length cannot use the same argument"
        );
        let required = [self.pointer_arg, self.length_arg]
            .into_iter()
            .chain(self.sequence_arg)
            .chain(self.timestamp_arg)
            .collect::<Vec<_>>();
        let distinct = required.iter().copied().collect::<BTreeSet<_>>();
        ensure!(
            distinct.len() == required.len(),
            "media callback argument roles must use distinct indices"
        );
        Ok(())
    }
}

/// One payload copied from a watched callback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaObservation {
    /// Audio or video.
    pub stream: MediaStream,
    /// Fully qualified callback symbol.
    pub symbol: String,
    /// Global callback order within this probe.
    pub ordinal: usize,
    /// Packet sequence number when the watch names one.
    pub sequence: Option<i64>,
    /// RTP/media timestamp when the watch names one.
    pub timestamp: Option<i64>,
    /// Exact bytes copied from guest memory.
    pub payload: Vec<u8>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MediaManifest {
    version: u32,
    records: Vec<MediaManifestRecord>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MediaManifestRecord {
    stream: MediaStream,
    symbol: String,
    ordinal: usize,
    sequence: Option<i64>,
    timestamp: Option<i64>,
    bytes: usize,
    sha256: String,
}

#[derive(Debug, Default)]
struct ProbeState {
    observations: BTreeMap<usize, MediaObservation>,
    next_ordinal: usize,
    bytes: usize,
    error: Option<String>,
}

/// Shared recorder behind [`crate::Runtime::watch_media`].
#[derive(Debug)]
pub(crate) struct MediaProbe {
    watches: Vec<MediaWatch>,
    state: Mutex<ProbeState>,
}

impl MediaProbe {
    pub fn new(watches: Vec<MediaWatch>) -> Result<Self> {
        ensure!(!watches.is_empty(), "media probe needs at least one watch");
        let mut symbols = BTreeSet::new();
        for watch in &watches {
            watch.validate()?;
            ensure!(
                symbols.insert(watch.symbol().to_owned()),
                "duplicate media watch for {}",
                watch.symbol()
            );
        }
        Ok(Self {
            watches,
            state: Mutex::new(ProbeState::default()),
        })
    }

    pub fn record(&self, host: &crate::state::HostState, module: &str, name: &str, args: &[i64]) {
        let Some(watch) = self
            .watches
            .iter()
            .find(|watch| watch.matches(module, name))
        else {
            return;
        };
        let symbol = watch.symbol.clone();
        let ordinal = {
            let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
            if state.error.is_some() {
                return;
            }
            if state.next_ordinal == MAX_MEDIA_RECORDS {
                state.error = Some(format!(
                    "media callback count exceeds {MAX_MEDIA_RECORDS} records"
                ));
                return;
            }
            let ordinal = state.next_ordinal;
            state.next_ordinal += 1;
            ordinal
        };

        let captured = (|| -> Result<MediaObservation> {
            let pointer = u32::try_from(
                *args
                    .get(watch.pointer_arg)
                    .context("media callback has no pointer argument")?,
            )
            .context("media callback pointer is negative or wider than wasm32")?;
            let length = u32::try_from(
                *args
                    .get(watch.length_arg)
                    .context("media callback has no length argument")?,
            )
            .context("media callback length is negative or wider than wasm32")?;
            ensure!(
                length <= MAX_MEDIA_PAYLOAD_BYTES,
                "media callback payload exceeds {MAX_MEDIA_PAYLOAD_BYTES} bytes"
            );
            let pointer = pointer
                .checked_add(watch.payload_offset)
                .context("media callback payload address overflow")?;
            let payload = host.read(pointer, length)?;
            Ok(MediaObservation {
                stream: watch.stream,
                symbol: symbol.clone(),
                ordinal,
                sequence: match watch.sequence_arg {
                    Some(index) => Some(
                        *args
                            .get(index)
                            .context("media callback has no sequence argument")?,
                    ),
                    None => None,
                },
                timestamp: match watch.timestamp_arg {
                    Some(index) => Some(
                        *args
                            .get(index)
                            .context("media callback has no timestamp argument")?,
                    ),
                    None => None,
                },
                payload,
            })
        })();

        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if state.error.is_some() {
            return;
        }
        match captured {
            Ok(observation) => {
                if state.bytes + observation.payload.len() > MAX_MEDIA_TOTAL_BYTES {
                    state.error = Some(format!(
                        "media callback payloads exceed {MAX_MEDIA_TOTAL_BYTES} bytes"
                    ));
                    return;
                }
                state.bytes += observation.payload.len();
                state.observations.insert(ordinal, observation);
            }
            Err(error) => state.error = Some(format!("capture {symbol}: {error:#}")),
        }
    }

    pub fn take(&self) -> Result<Vec<MediaObservation>> {
        let mut state = self.state.lock().unwrap_or_else(|error| error.into_inner());
        if let Some(error) = state.error.take() {
            state.observations.clear();
            state.bytes = 0;
            state.next_ordinal = 0;
            anyhow::bail!(error);
        }
        ensure!(
            state.observations.len() == state.next_ordinal,
            "media callbacks are still being captured"
        );
        state.bytes = 0;
        state.next_ordinal = 0;
        Ok(std::mem::take(&mut state.observations)
            .into_values()
            .collect())
    }
}

/// Compare normalized wasm and Rust observations byte for byte.
///
/// Callers decide which callback arguments are meaningful when creating each
/// side. This function deliberately applies no timestamp tolerance, packet
/// reordering or codec-specific normalization that could hide a defect.
pub fn compare_media(expected: &[MediaObservation], actual: &[MediaObservation]) -> Result<()> {
    ensure!(
        expected.len() == actual.len(),
        "media observation count differs: expected {}, got {}",
        expected.len(),
        actual.len()
    );
    for (index, (expected, actual)) in expected.iter().zip(actual).enumerate() {
        ensure!(
            expected.stream == actual.stream
                && expected.sequence == actual.sequence
                && expected.timestamp == actual.timestamp,
            "media metadata differs at record {index}: expected {expected:?}, got {actual:?}"
        );
        if expected.payload != actual.payload {
            let expected_hash = hex::encode(Sha256::digest(&expected.payload));
            let actual_hash = hex::encode(Sha256::digest(&actual.payload));
            anyhow::bail!(
                "media payload differs at record {index}: expected {} bytes/{expected_hash}, got {} bytes/{actual_hash}",
                expected.payload.len(),
                actual.payload.len()
            );
        }
    }
    Ok(())
}

/// Persist a media trace as a small JSON manifest plus one binary file per record.
///
/// The manifest is written last, so a partial update cannot validate against
/// payloads whose size or hash does not match it.
pub fn write_media_trace(directory: &Path, observations: &[MediaObservation]) -> Result<()> {
    ensure!(
        observations.len() <= MAX_MEDIA_RECORDS,
        "media trace exceeds {MAX_MEDIA_RECORDS} records"
    );
    std::fs::create_dir_all(directory)?;
    let mut total = 0usize;
    let mut records = Vec::with_capacity(observations.len());
    for (index, observation) in observations.iter().enumerate() {
        ensure!(
            observation.ordinal == index,
            "media trace ordinal {} is not canonical index {index}",
            observation.ordinal
        );
        ensure!(
            observation.payload.len() <= MAX_MEDIA_PAYLOAD_BYTES as usize,
            "media trace record {index} exceeds {MAX_MEDIA_PAYLOAD_BYTES} bytes"
        );
        total = total
            .checked_add(observation.payload.len())
            .context("media trace size overflow")?;
        ensure!(
            total <= MAX_MEDIA_TOTAL_BYTES,
            "media trace exceeds {MAX_MEDIA_TOTAL_BYTES} bytes"
        );
        persist_bytes(
            directory,
            &trace_payload_path(directory, index),
            &observation.payload,
        )?;
        records.push(MediaManifestRecord {
            stream: observation.stream,
            symbol: observation.symbol.clone(),
            ordinal: index,
            sequence: observation.sequence,
            timestamp: observation.timestamp,
            bytes: observation.payload.len(),
            sha256: hex::encode(Sha256::digest(&observation.payload)),
        });
    }
    for entry in std::fs::read_dir(directory)? {
        let path = entry?.path();
        if path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with("record-") && name.ends_with(".bin"))
            && !records
                .iter()
                .any(|record| trace_payload_path(directory, record.ordinal) == path)
        {
            std::fs::remove_file(path)?;
        }
    }
    let mut manifest = serde_json::to_vec_pretty(&MediaManifest {
        version: 1,
        records,
    })?;
    manifest.push(b'\n');
    persist_bytes(directory, &directory.join("media-trace.json"), &manifest)?;
    Ok(())
}

/// Read and verify every payload in a persisted media trace.
pub fn read_media_trace(directory: &Path) -> Result<Vec<MediaObservation>> {
    const MAX_MANIFEST_BYTES: u64 = 4 * 1024 * 1024;
    let manifest_path = directory.join("media-trace.json");
    let metadata = std::fs::symlink_metadata(&manifest_path)?;
    ensure!(
        metadata.file_type().is_file() && !metadata.file_type().is_symlink(),
        "media trace manifest is not a regular file"
    );
    ensure!(
        metadata.len() <= MAX_MANIFEST_BYTES,
        "media trace manifest exceeds {MAX_MANIFEST_BYTES} bytes"
    );
    let manifest: MediaManifest = serde_json::from_slice(&std::fs::read(&manifest_path)?)?;
    ensure!(
        manifest.version == 1,
        "unsupported media trace version {}",
        manifest.version
    );
    ensure!(
        manifest.records.len() <= MAX_MEDIA_RECORDS,
        "media trace exceeds {MAX_MEDIA_RECORDS} records"
    );
    let mut total = 0usize;
    let mut observations = Vec::with_capacity(manifest.records.len());
    for (index, record) in manifest.records.into_iter().enumerate() {
        ensure!(
            record.ordinal == index,
            "media trace ordinal {} is not canonical index {index}",
            record.ordinal
        );
        ensure!(
            record.bytes <= MAX_MEDIA_PAYLOAD_BYTES as usize,
            "media trace record {index} exceeds {MAX_MEDIA_PAYLOAD_BYTES} bytes"
        );
        total = total
            .checked_add(record.bytes)
            .context("media trace size overflow")?;
        ensure!(
            total <= MAX_MEDIA_TOTAL_BYTES,
            "media trace exceeds {MAX_MEDIA_TOTAL_BYTES} bytes"
        );
        let path = trace_payload_path(directory, index);
        let payload_metadata = std::fs::symlink_metadata(&path)?;
        ensure!(
            payload_metadata.file_type().is_file() && !payload_metadata.file_type().is_symlink(),
            "media trace record {index} is not a regular file"
        );
        ensure!(
            payload_metadata.len() == u64::try_from(record.bytes)?,
            "media trace record {index} size differs: manifest {}, file {}",
            record.bytes,
            payload_metadata.len()
        );
        let payload = std::fs::read(path)?;
        let sha256 = hex::encode(Sha256::digest(&payload));
        ensure!(
            sha256 == record.sha256,
            "media trace record {index} hashes to {sha256}; manifest requires {}",
            record.sha256
        );
        observations.push(MediaObservation {
            stream: record.stream,
            symbol: record.symbol,
            ordinal: index,
            sequence: record.sequence,
            timestamp: record.timestamp,
            payload,
        });
    }
    Ok(observations)
}

fn trace_payload_path(directory: &Path, ordinal: usize) -> std::path::PathBuf {
    directory.join(format!("record-{ordinal:04}.bin"))
}

fn persist_bytes(directory: &Path, path: &Path, bytes: &[u8]) -> Result<()> {
    let mut temporary = tempfile::NamedTempFile::new_in(directory)?;
    temporary.write_all(bytes)?;
    temporary
        .persist(path)
        .with_context(|| format!("write {}", path.display()))?;
    Ok(())
}
