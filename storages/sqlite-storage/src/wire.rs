//! On-disk encoding for the BLOB columns this backend persists (server cert
//! chain, app-state sync keys, app-state hash state).
//!
//! Modeled as protobuf via `prost` derive macros (no `.proto` file), reusing the
//! `prost` dependency the workspace already pulls in for the wire protocol. The
//! format is field-tagged, so reordering or adding a field doesn't corrupt old
//! rows the way a positional codec would. Domain types in `wacore` stay
//! untouched; conversion happens only at this boundary.

use std::collections::HashMap;

use prost::Message;
use wacore::appstate::hash::HashState;
use wacore::store::device::{CachedNoiseCert, CachedServerCertChain};
use wacore::store::error::StoreError;
use wacore::store::traits::AppStateSyncKey;

/// X25519 public key length in `CachedNoiseCert`.
const NOISE_KEY_LEN: usize = 32;
/// App-state hash length in `HashState`.
const HASH_STATE_LEN: usize = 128;
/// App-state master key length (the HKDF input for `expand_app_state_keys`).
const APP_STATE_KEY_LEN: usize = 32;

#[derive(Clone, PartialEq, prost::Message)]
struct NoiseCert {
    #[prost(bytes = "vec", tag = "1")]
    key: Vec<u8>,
    #[prost(int64, tag = "2")]
    not_before: i64,
    #[prost(int64, tag = "3")]
    not_after: i64,
}

#[derive(Clone, PartialEq, prost::Message)]
struct ServerCertChain {
    #[prost(message, optional, tag = "1")]
    intermediate: Option<NoiseCert>,
    #[prost(message, optional, tag = "2")]
    leaf: Option<NoiseCert>,
}

#[derive(Clone, PartialEq, prost::Message)]
struct AppStateSyncKeyWire {
    #[prost(bytes = "vec", tag = "1")]
    key_data: Vec<u8>,
    #[prost(bytes = "vec", tag = "2")]
    fingerprint: Vec<u8>,
    #[prost(int64, tag = "3")]
    timestamp: i64,
}

#[derive(Clone, PartialEq, prost::Message)]
struct HashStateWire {
    #[prost(uint64, tag = "1")]
    version: u64,
    #[prost(bytes = "vec", tag = "2")]
    hash: Vec<u8>,
    #[prost(map = "string, bytes", tag = "3")]
    index_value_map: HashMap<String, Vec<u8>>,
}

fn bad_len(field: &str, expected: usize, got: usize) -> StoreError {
    StoreError::Serialization(format!("{field}: expected {expected} bytes, got {got}").into())
}

fn decode_err(e: prost::DecodeError) -> StoreError {
    StoreError::Serialization(Box::new(e))
}

// --- server cert chain ---

impl From<&CachedNoiseCert> for NoiseCert {
    fn from(c: &CachedNoiseCert) -> Self {
        Self {
            key: c.key.to_vec(),
            not_before: c.not_before,
            not_after: c.not_after,
        }
    }
}

fn noise_cert_from_wire(w: NoiseCert) -> Result<CachedNoiseCert, StoreError> {
    let got = w.key.len();
    let key: [u8; NOISE_KEY_LEN] = w
        .key
        .try_into()
        .map_err(|_| bad_len("noise_cert.key", NOISE_KEY_LEN, got))?;
    Ok(CachedNoiseCert {
        key,
        not_before: w.not_before,
        not_after: w.not_after,
    })
}

pub(crate) fn encode_server_cert_chain(c: &CachedServerCertChain) -> Vec<u8> {
    ServerCertChain {
        intermediate: Some((&c.intermediate).into()),
        leaf: Some((&c.leaf).into()),
    }
    .encode_to_vec()
}

pub(crate) fn decode_server_cert_chain(bytes: &[u8]) -> Result<CachedServerCertChain, StoreError> {
    let w = ServerCertChain::decode(bytes).map_err(decode_err)?;
    let intermediate = w.intermediate.ok_or_else(|| {
        StoreError::Serialization("server_cert_chain.intermediate missing".into())
    })?;
    let leaf = w
        .leaf
        .ok_or_else(|| StoreError::Serialization("server_cert_chain.leaf missing".into()))?;
    Ok(CachedServerCertChain {
        intermediate: noise_cert_from_wire(intermediate)?,
        leaf: noise_cert_from_wire(leaf)?,
    })
}

// --- app-state sync key ---

pub(crate) fn encode_app_state_sync_key(k: &AppStateSyncKey) -> Vec<u8> {
    AppStateSyncKeyWire {
        key_data: k.key_data.clone(),
        fingerprint: k.fingerprint.clone(),
        timestamp: k.timestamp,
    }
    .encode_to_vec()
}

pub(crate) fn decode_app_state_sync_key(bytes: &[u8]) -> Result<AppStateSyncKey, StoreError> {
    let w = AppStateSyncKeyWire::decode(bytes).map_err(decode_err)?;
    // An old bincode row (or a corrupt blob) can occasionally parse as protobuf
    // with garbage key material. Reject anything that isn't a 32-byte master
    // key so the caller treats it as absent and re-requests it, rather than
    // deriving bad sub-keys that later fail with MAC/decrypt errors.
    if w.key_data.len() != APP_STATE_KEY_LEN {
        return Err(bad_len(
            "app_state_sync_key.key_data",
            APP_STATE_KEY_LEN,
            w.key_data.len(),
        ));
    }
    Ok(AppStateSyncKey {
        key_data: w.key_data,
        fingerprint: w.fingerprint,
        timestamp: w.timestamp,
    })
}

// --- app-state hash state ---

pub(crate) fn encode_hash_state(s: &HashState) -> Vec<u8> {
    HashStateWire {
        version: s.version,
        hash: s.hash.to_vec(),
        index_value_map: s.index_value_map.clone(),
    }
    .encode_to_vec()
}

pub(crate) fn decode_hash_state(bytes: &[u8]) -> Result<HashState, StoreError> {
    let w = HashStateWire::decode(bytes).map_err(decode_err)?;
    let got = w.hash.len();
    let hash: [u8; HASH_STATE_LEN] = w
        .hash
        .try_into()
        .map_err(|_| bad_len("hash_state.hash", HASH_STATE_LEN, got))?;
    Ok(HashState {
        version: w.version,
        hash,
        index_value_map: w.index_value_map,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn server_cert_chain_roundtrips() {
        let chain = CachedServerCertChain {
            intermediate: CachedNoiseCert {
                key: [0xAB; 32],
                not_before: 1_700_000_000,
                not_after: 1_900_000_000,
            },
            leaf: CachedNoiseCert {
                key: [0xCD; 32],
                not_before: 1_700_000_500,
                not_after: 1_899_999_500,
            },
        };
        let decoded = decode_server_cert_chain(&encode_server_cert_chain(&chain)).unwrap();
        assert_eq!(decoded, chain);
    }

    #[test]
    fn server_cert_chain_rejects_wrong_key_len() {
        // A wire blob whose key is not 32 bytes must error, not silently truncate.
        let bytes = ServerCertChain {
            intermediate: Some(NoiseCert {
                key: vec![0u8; 5],
                not_before: 1,
                not_after: 2,
            }),
            leaf: Some(NoiseCert {
                key: vec![0u8; 32],
                not_before: 1,
                not_after: 2,
            }),
        }
        .encode_to_vec();
        assert!(decode_server_cert_chain(&bytes).is_err());
    }

    #[test]
    fn app_state_sync_key_roundtrips() {
        let key = AppStateSyncKey {
            key_data: vec![7u8; 32],
            fingerprint: vec![9, 8, 7],
            timestamp: 1_700_000_123,
        };
        let decoded = decode_app_state_sync_key(&encode_app_state_sync_key(&key)).unwrap();
        assert_eq!(decoded.key_data, key.key_data);
        assert_eq!(decoded.fingerprint, key.fingerprint);
        assert_eq!(decoded.timestamp, key.timestamp);
    }

    #[test]
    fn app_state_sync_key_rejects_wrong_key_len() {
        // An old bincode row can parse as protobuf with garbage key material;
        // non-32-byte key data must error so it is treated as absent and
        // re-requested, not used to derive bad sub-keys.
        let bytes = AppStateSyncKeyWire {
            key_data: vec![0u8; 16],
            fingerprint: vec![1, 2, 3],
            timestamp: 1,
        }
        .encode_to_vec();
        assert!(decode_app_state_sync_key(&bytes).is_err());
    }

    #[test]
    fn hash_state_roundtrips() {
        let mut index_value_map = HashMap::new();
        index_value_map.insert("idx-a".to_string(), vec![1, 2, 3]);
        index_value_map.insert("idx-b".to_string(), vec![]);
        let mut hash = [0u8; 128];
        hash[0] = 0xFF;
        hash[127] = 0x11;
        let state = HashState {
            version: 42,
            hash,
            index_value_map: index_value_map.clone(),
        };
        let decoded = decode_hash_state(&encode_hash_state(&state)).unwrap();
        assert_eq!(decoded.version, 42);
        assert_eq!(decoded.hash, hash);
        assert_eq!(decoded.index_value_map, index_value_map);
    }

    #[test]
    fn hash_state_default_roundtrips() {
        let state = HashState::default();
        let decoded = decode_hash_state(&encode_hash_state(&state)).unwrap();
        assert_eq!(decoded.version, 0);
        assert_eq!(decoded.hash, [0u8; 128]);
        assert!(decoded.index_value_map.is_empty());
    }

    #[test]
    fn hash_state_rejects_wrong_hash_len() {
        // A wire blob whose hash is not 128 bytes must error, not silently truncate.
        let bytes = HashStateWire {
            version: 1,
            hash: vec![0u8; 64],
            index_value_map: HashMap::new(),
        }
        .encode_to_vec();
        assert!(decode_hash_state(&bytes).is_err());
    }
}
