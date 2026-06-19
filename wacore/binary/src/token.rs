pub const DICT_VERSION: u8 = 3;

// --- Public Constants for Special Tags ---
pub const LIST_EMPTY: u8 = 0;
pub const DICTIONARY_0: u8 = 236;
pub const DICTIONARY_1: u8 = 237;
pub const DICTIONARY_2: u8 = 238;
pub const DICTIONARY_3: u8 = 239;

pub const JID_PAIR: u8 = 250;
pub const HEX_8: u8 = 251;
pub const BINARY_8: u8 = 252;
pub const BINARY_20: u8 = 253;
pub const BINARY_32: u8 = 254;
pub const NIBBLE_8: u8 = 255;
pub const INTEROP_JID: u8 = 245;
pub const FB_JID: u8 = 246;
pub const AD_JID: u8 = 247;
pub const LIST_8: u8 = 248;
pub const LIST_16: u8 = 249;

pub const PACKED_MAX: u8 = 127;
pub const SINGLE_BYTE_MAX: u16 = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TokenKind {
    Single(u8),
    Double(u8, u8),
}

include!(concat!(env!("OUT_DIR"), "/token_maps.rs"));

pub fn index_of_token(token: &str) -> Option<TokenKind> {
    hashify_lookup(token.as_bytes())
}

pub fn get_single_token(index: u8) -> Option<&'static str> {
    SINGLE_BYTE_TOKENS.get(index as usize).copied()
}

pub fn get_double_token(dict: u8, index: u8) -> Option<&'static str> {
    DOUBLE_BYTE_TOKENS
        .get(dict as usize)
        .and_then(|d| d.get(index as usize))
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_single_byte_token_roundtrip() {
        for i in 1u8..=235 {
            if let Some(token) = get_single_token(i) {
                let result = index_of_token(token);
                assert!(
                    matches!(result, Some(TokenKind::Single(idx)) if idx == i),
                    "Token '{}' at index {} doesn't round-trip",
                    token,
                    i,
                );
            }
        }
    }

    #[test]
    fn test_double_byte_token_roundtrip() {
        for dict in 0..4u8 {
            for idx in 0..255u8 {
                if let Some(token) = get_double_token(dict, idx) {
                    let result = index_of_token(token);
                    assert!(
                        matches!(result, Some(TokenKind::Double(d, i)) if d == dict && i == idx),
                        "Token '{}' at dict {} index {} doesn't round-trip",
                        token,
                        dict,
                        idx,
                    );
                }
            }
        }
    }

    #[test]
    fn test_unknown_string_returns_none() {
        assert!(index_of_token("xyzzy_not_a_token_12345").is_none());
    }

    #[test]
    fn test_empty_string_returns_none() {
        assert!(index_of_token("").is_none());
    }

    #[test]
    fn test_token_boundary_indices() {
        let token_0 = get_single_token(0);
        assert_eq!(token_0, Some(""), "Index 0 should be empty string token");
        assert!(get_single_token(LIST_8).is_none());
        assert!(get_single_token(LIST_16).is_none());
        assert!(get_single_token(JID_PAIR).is_none());
        assert!(get_single_token(HEX_8).is_none());
        assert!(get_single_token(BINARY_8).is_none());
        assert!(get_single_token(BINARY_20).is_none());
        assert!(get_single_token(BINARY_32).is_none());
        assert!(get_single_token(NIBBLE_8).is_none());
    }

    #[test]
    fn test_almost_matching_strings() {
        let token = get_single_token(1).expect("single token at index 1 must exist");
        assert!(index_of_token(&format!("{}_modified", token)).is_none());
        assert!(index_of_token(&format!("prefix_{}", token)).is_none());
        assert!(index_of_token(&format!("{}!", token)).is_none());
    }

    #[test]
    fn test_out_of_bounds_dictionary() {
        assert!(get_double_token(4, 0).is_none());
        assert!(get_double_token(5, 100).is_none());
        assert!(get_double_token(255, 0).is_none());
    }

    /// Exhaustive equivalence guard: against a reference map built from the token
    /// tables, every one-byte input and every single-byte mutation (all 256
    /// values) of every token must resolve to the reference value (the token or
    /// `None`). Probes the length-bucketed lookup for any discriminator collision
    /// that would silently map a non-token (a JID/id) onto a token and corrupt the
    /// wire, and stays durable against future token-set edits.
    #[test]
    fn lookup_matches_reference_under_byte_mutation() {
        use std::collections::HashMap;

        let mut reference: HashMap<Vec<u8>, TokenKind> = HashMap::new();
        for i in 0u8..=235 {
            if let Some(t) = get_single_token(i) {
                reference
                    .entry(t.as_bytes().to_vec())
                    .or_insert(TokenKind::Single(i));
            }
        }
        for dict in 0..4u8 {
            for idx in 0..=255u8 {
                if let Some(t) = get_double_token(dict, idx) {
                    reference
                        .entry(t.as_bytes().to_vec())
                        .or_insert(TokenKind::Double(dict, idx));
                }
            }
        }

        let check = |bytes: &[u8]| {
            assert_eq!(
                hashify_lookup(bytes),
                reference.get(bytes).copied(),
                "lookup disagrees with reference for {bytes:?}",
            );
        };

        for b in 0u8..=255 {
            check(&[b]);
        }

        let keys: Vec<Vec<u8>> = reference.keys().cloned().collect();
        for key in &keys {
            let mut m = key.clone();
            for pos in 0..m.len() {
                let orig = m[pos];
                for b in 0u8..=255 {
                    m[pos] = b;
                    check(&m);
                }
                m[pos] = orig;
            }
        }
    }
}
