//! `wacore/binary/src/tokens.json`: the binary-protocol token dictionaries.
//!
//! The IR is already canonicalized to the wire-indexable form (index 0 of the
//! single-byte table is the reserved empty token), so this is a re-serialization
//! under the two keys `wacore-binary`'s build script reads.

use anyhow::{Result, ensure};
use serde::Serialize;

use crate::ir::TokensIr;

/// Serialized in declaration order, so the file reads single-byte table first.
/// `serde_json::json!` would key it by a sorted map and put the dictionaries on
/// top, which is a whole-file diff for no gain.
#[derive(Serialize)]
struct Tokens<'a> {
    single_byte: &'a [String],
    double_byte: &'a [Vec<String>],
}

pub fn generate(ir: &TokensIr) -> Result<String> {
    // The decoder indexes these tables directly by wire tag. A table that lost
    // its reserved leading slot would shift every token by one and corrupt every
    // frame, so refuse to write it rather than let the build succeed.
    ensure!(
        ir.single_byte.first().is_some_and(|t| t.is_empty()),
        "single-byte token table does not start with the reserved empty token"
    );
    ensure!(
        !ir.double_byte.is_empty(),
        "token IR carries no double-byte dictionaries"
    );
    let doc = Tokens {
        single_byte: &ir.single_byte,
        double_byte: &ir.double_byte,
    };
    Ok(serde_json::to_string_pretty(&doc)? + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ir(single: &[&str], double: &[&[&str]]) -> TokensIr {
        TokensIr {
            single_byte: single.iter().map(|s| s.to_string()).collect(),
            double_byte: double
                .iter()
                .map(|d| d.iter().map(|s| s.to_string()).collect())
                .collect(),
        }
    }

    #[test]
    fn emits_the_two_keys_the_build_script_reads() {
        let out =
            generate(&ir(&["", "xmlstreamstart"], &[&["read-self"], &["reject"]])).expect("tokens");
        let v: serde_json::Value = serde_json::from_str(&out).expect("json");
        assert_eq!(v["single_byte"][0], "");
        assert_eq!(v["single_byte"][1], "xmlstreamstart");
        assert_eq!(v["double_byte"][0][0], "read-self");
        assert_eq!(v["double_byte"][1][0], "reject");
        assert!(out.ends_with('\n'));
    }

    #[test]
    fn a_table_missing_the_reserved_slot_is_rejected() {
        let err = generate(&ir(&["xmlstreamstart"], &[&["read-self"]]));
        assert!(err.is_err(), "off-by-one table must not be written");
    }

    #[test]
    fn a_table_with_no_dictionaries_is_rejected() {
        assert!(generate(&ir(&[""], &[])).is_err());
    }
}
