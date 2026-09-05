//! Test-only reader for losslessly packed oracle fixtures.

use std::io::Read;

pub(super) fn decode<T: serde::de::DeserializeOwned>(
    packed: &[u8],
) -> Result<T, Box<dyn std::error::Error>> {
    let bytes = inflate(packed)?;
    let mut reader = bytes.as_slice();
    let value = ciborium::de::from_reader(&mut reader)?;
    if !reader.is_empty() {
        return Err("trailing bytes after the fixture CBOR value".into());
    }
    Ok(value)
}

pub(super) fn inflate(packed: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut decoder = ruzstd::decoding::StreamingDecoder::new(packed)?;
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes)?;
    Ok(bytes)
}
