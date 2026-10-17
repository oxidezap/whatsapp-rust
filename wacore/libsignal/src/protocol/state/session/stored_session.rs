use super::{SessionState, SkippedKey};
use buffa::encoding::{Tag, WireType, decode_varint, encode_varint, skip_field_depth, varint_len};
use buffa::{Message, SizeCache};
use waproto::tags::session_structure::{RECEIVER_CHAINS, chain::MESSAGE_KEYS};

pub(super) struct StoredSession<'a> {
    state: &'a SessionState,
    base: Option<Vec<u8>>,
    len: usize,
}

impl<'a> StoredSession<'a> {
    #[allow(clippy::disallowed_methods)]
    pub(super) fn new(state: &'a SessionState, cache: &mut SizeCache) -> Self {
        if !state.has_skipped_keys() {
            return Self {
                state,
                base: None,
                len: state.session.compute_size(cache) as usize,
            };
        }
        // Encode the small generated tree once, then splice the borrowed backlog.
        // This preserves unrelated generated fields without cloning SessionStructure
        // or rebuilding an array of heap-backed protobuf message keys.
        let base = state.session.encode_to_vec();
        let extra = state
            .session
            .receiver_chains
            .iter()
            .zip(&state.skipped)
            .map(|(chain, keys)| {
                if keys.is_some() {
                    assert!(
                        chain.message_keys.is_empty(),
                        "skipped keys must have a single owner"
                    );
                }
                let keys_len = keys.as_ref().map_or(0, |keys| encoded_keys_len(keys));
                let base_len = chain.encoded_len() as usize;
                keys_len + varint_len((base_len + keys_len) as u64) - varint_len(base_len as u64)
            })
            .sum::<usize>();
        let len = base.len() + extra;
        assert!(
            len <= buffa::MAX_MESSAGE_BYTES as usize,
            "session exceeds protobuf size limit"
        );
        Self {
            state,
            base: Some(base),
            len,
        }
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }

    #[allow(clippy::disallowed_methods)]
    pub(super) fn write_to(&self, cache: &mut SizeCache, out: &mut Vec<u8>) {
        let Some(base) = &self.base else {
            self.state.session.write_to(cache, out);
            return;
        };
        let start = out.len();
        let mut remaining = base.as_slice();
        let mut chain_index = 0;
        while !remaining.is_empty() {
            let field = remaining;
            // Input is the generated encoder's own output, never network bytes.
            let tag = Tag::decode(&mut remaining).expect("generated session tag");
            if tag.field_number() == RECEIVER_CHAINS && tag.wire_type() == WireType::LengthDelimited
            {
                let len = decode_varint(&mut remaining).expect("generated receiver chain length")
                    as usize;
                let (chain, rest) = remaining.split_at(len);
                remaining = rest;
                let keys = self
                    .state
                    .skipped
                    .get(chain_index)
                    .and_then(Option::as_deref);
                chain_index += 1;
                let keys_len = keys.map_or(0, |keys| encoded_keys_len(keys));
                tag.encode(out);
                encode_varint((len + keys_len) as u64, out);
                out.extend_from_slice(chain);
                if let Some(keys) = keys {
                    for key in keys.iter() {
                        Tag::new(MESSAGE_KEYS, WireType::LengthDelimited).encode(out);
                        encode_varint(key_len(key) as u64, out);
                        match key {
                            SkippedKey::Seed { index, seed } => {
                                use waproto::tags::session_structure::chain::message_key::{
                                    INDEX, SEED,
                                };
                                Tag::new(INDEX, WireType::Varint).encode(out);
                                encode_varint(u64::from(*index), out);
                                Tag::new(SEED, WireType::LengthDelimited).encode(out);
                                encode_varint(seed.len() as u64, out);
                                out.extend_from_slice(seed);
                            }
                            SkippedKey::Legacy(key) => key.encode(out),
                        }
                    }
                }
            } else {
                skip_field_depth(tag, &mut remaining, buffa::RECURSION_LIMIT)
                    .expect("generated session field");
                out.extend_from_slice(&field[..field.len() - remaining.len()]);
            }
        }
        debug_assert_eq!(out.len() - start, self.len);
    }
}

#[allow(clippy::disallowed_methods)]
fn key_len(key: &SkippedKey) -> usize {
    match key {
        SkippedKey::Seed { index, .. } => {
            use waproto::tags::session_structure::chain::message_key::{INDEX, SEED};
            varint_len(u64::from(INDEX) << 3)
                + varint_len(u64::from(*index))
                + length_delimited_len(SEED, 32)
        }
        SkippedKey::Legacy(key) => key.encoded_len() as usize,
    }
}

fn encoded_keys_len(keys: &[SkippedKey]) -> usize {
    keys.iter()
        .map(|key| {
            let len = key_len(key);
            length_delimited_len(MESSAGE_KEYS, len)
        })
        .sum()
}

fn length_delimited_len(field: u32, len: usize) -> usize {
    varint_len((u64::from(field) << 3) | 2) + varint_len(len as u64) + len
}
