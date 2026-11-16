//! `waproto/src/whatsapp.proto`: the upstream schema plus the messages this
//! repository keeps after WA dropped them from the public bundle.
//!
//! Local *fields* belong in `waproto/build.rs`'s `LOCAL_FIELDS`, which splices
//! them into the descriptor. A whole message has no such hook, so it is declared
//! here and spliced into the `.proto` text instead. Either way the addition
//! survives a sync, and a sync that starts declaring the same name fails the
//! build rather than quietly taking over a type we already persist.

use anyhow::{Result, bail, ensure};
use std::collections::BTreeMap;

/// A message this repository declares that the upstream bundle no longer does.
struct LocalBlock {
    /// Spliced in after this top-level message's closing brace, so the block
    /// lands beside the message it belongs to instead of at the end of the file.
    after_message: &'static str,
    text: &'static str,
}

const LOCAL_BLOCKS: &[LocalBlock] = &[
    LocalBlock {
        after_message: "LIDMigrationMappingSyncMessage",
        text: "\
// Retained locally: WA dropped these from the public JS bundle, but the wire
// still carries them as the protobuf-encoded `encodedMappingPayload` above.
message LIDMigrationMapping {
  required uint64 pn = 1;
  required uint64 assignedLid = 2;
  optional uint64 latestLid = 3;
}

message LIDMigrationMappingSyncPayload {
  repeated LIDMigrationMapping pnToLidMappings = 1;
  optional uint64 chatDbMigrationTimestamp = 2;
}
",
    },
    LocalBlock {
        after_message: "WebNotificationsInfo",
        // A locked capture is a sample of client code, not an authority to delete API.
        text: include_str!("retained.proto"),
    },
];

pub fn generate(upstream: &str) -> Result<String> {
    let reviewed = serde_json::from_str(include_str!("../../fixtures/reclaimed-proto.json"))?;
    generate_with_review(upstream, &reviewed)
}

fn generate_with_review(upstream: &str, reviewed: &BTreeMap<String, String>) -> Result<String> {
    let mut out = reconcile_reclaimed(upstream, reviewed)?;
    for block in LOCAL_BLOCKS {
        for name in block.text.lines().filter_map(declaration_name) {
            if out
                .lines()
                .filter_map(declaration_name)
                .any(|upstream| upstream == name)
            {
                bail!(
                    "upstream now declares `{name}`; drop the local block anchored at \
                     `{}` in the proto emitter instead of shadowing it",
                    block.after_message
                );
            }
        }
        let at = end_of_message(&out, block.after_message).ok_or_else(|| {
            anyhow::anyhow!(
                "anchor `message {}` is gone from the upstream proto; re-anchor the local block \
                 declaring retained types",
                block.after_message,
            )
        })?;
        out.insert_str(at, &format!("\n{}", block.text));
    }
    Ok(out)
}

fn reconcile_reclaimed(upstream: &str, reviewed: &BTreeMap<String, String>) -> Result<String> {
    for (name, digest) in reviewed {
        let start = upstream
            .find(&format!("\nmessage {name} {{\n"))
            .map(|at| at + 1);
        // Synthetic inputs and the first declaration need no leading newline.
        let start = start.or_else(|| {
            upstream
                .starts_with(&format!("message {name} {{\n"))
                .then_some(0)
        });
        let start = start.ok_or_else(|| anyhow::anyhow!("reclaimed message {name} missing"))?;
        let end = end_of_message(&upstream[start..], name)
            .ok_or_else(|| anyhow::anyhow!("reclaimed message {name} incomplete"))?;
        ensure!(
            crate::source::digest(upstream[start..start + end].as_bytes()) == *digest,
            "reclaimed message {name} changed; review its wire contract"
        );
        ensure!(
            !upstream
                .lines()
                .any(|line| line == format!("message Web{name} {{")),
            "reclaimed message Web{name} already exists"
        );
    }
    // These Labyrinth names now denote incompatible wire contracts. Rewrite
    // current references together; historical declarations are appended later.
    let mut out = String::with_capacity(upstream.len());
    let bytes = upstream.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        if bytes[at] == b'"' || bytes[at] == b'\'' {
            let quote = bytes[at];
            at += 1;
            while at < bytes.len() {
                let byte = bytes[at];
                at += 1;
                if byte == b'\\' {
                    at = (at + 1).min(bytes.len());
                } else if byte == quote {
                    break;
                }
            }
        } else if bytes[at..].starts_with(b"//") {
            at += upstream[at..].find('\n').unwrap_or(bytes.len() - at);
        } else if bytes[at..].starts_with(b"/*") {
            at += upstream[at + 2..]
                .find("*/")
                .map_or(bytes.len() - at, |end| end + 4);
        } else if bytes[at].is_ascii_alphabetic() || bytes[at] == b'_' {
            at += 1;
            while at < bytes.len() && (bytes[at].is_ascii_alphanumeric() || bytes[at] == b'_') {
                at += 1;
            }
            if reviewed.contains_key(&upstream[start..at]) {
                out.push_str("Web");
            }
        } else {
            at += upstream[at..].chars().next().map_or(1, char::len_utf8);
        }
        out.push_str(&upstream[start..at]);
    }
    Ok(out)
}

fn declaration_name(line: &str) -> Option<&str> {
    line.strip_prefix("message ")
        .or_else(|| line.strip_prefix("enum "))?
        .split_whitespace()
        .next()
}

/// Byte offset just past the closing brace line of a top-level message.
///
/// Top-level declarations sit at column zero and nested ones are indented, so a
/// line that is exactly `}` closes the message the scan is inside.
fn end_of_message(proto: &str, name: &str) -> Option<usize> {
    let open = format!("message {name} {{");
    let mut offset = 0usize;
    let mut inside = false;
    for line in proto.split_inclusive('\n') {
        let trimmed = line.trim_end_matches(['\n', '\r']);
        if inside && trimmed == "}" {
            return Some(offset + line.len());
        }
        if trimmed == open {
            inside = true;
        }
        offset += line.len();
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn generate(upstream: &str) -> Result<String> {
        generate_with_review(upstream, &BTreeMap::new())
    }

    #[test]
    fn reclaimed_contracts_keep_current_references_separate_and_preserve_literals() {
        let message = "message Legacy {\n  required int32 id = 1;\n}\n";
        let source = format!(
            "{message}// Legacy\n/* Legacy */\nmessage Holder {{\n  optional Legacy child = 1;\n  optional string text = 2 [default = \"Legacy\"];\n}}\n"
        );
        let reviewed =
            BTreeMap::from([("Legacy".into(), crate::source::digest(message.as_bytes()))]);
        let out = reconcile_reclaimed(&source, &reviewed).unwrap();
        assert!(out.contains("message WebLegacy {"));
        assert!(out.contains("optional WebLegacy child = 1;"));
        assert!(out.contains("// Legacy\n/* Legacy */"));
        assert!(out.contains("[default = \"Legacy\"]"));
        assert!(reconcile_reclaimed(&source.replace("int32", "bytes"), &reviewed).is_err());
        assert!(reconcile_reclaimed("", &reviewed).is_err());
        assert!(
            reconcile_reclaimed(&format!("{source}message WebLegacy {{\n}}\n"), &reviewed).is_err()
        );
    }

    const UPSTREAM: &str = "\
syntax = \"proto2\";
package whatsapp;

message LIDMigrationMappingSyncMessage {
  optional bytes encodedMappingPayload = 1;
}

message NextMessage {
  oneof commandInput {
    CreateBackupInput createBackupInput = 1;
  }
}

message WebNotificationsInfo {
}
";

    #[test]
    fn splices_the_local_block_after_its_anchor() {
        let out = generate(UPSTREAM).expect("proto");
        let anchor = out
            .find("optional bytes encodedMappingPayload")
            .expect("anchor");
        let local = out
            .find("message LIDMigrationMapping {")
            .expect("local block");
        let next = out.find("message NextMessage").expect("next message");
        assert!(anchor < local && local < next, "{out}");
        assert!(out.contains("message LIDMigrationMappingSyncPayload {"));
        // The upstream text is otherwise untouched.
        assert!(out.starts_with("syntax = \"proto2\";\n"));
        assert!(out.ends_with("}\n"));
    }

    #[test]
    fn a_lost_anchor_fails_instead_of_dropping_the_block() {
        let err = generate("syntax = \"proto2\";\npackage whatsapp;\n").expect_err("must fail");
        assert!(err.to_string().contains("re-anchor"), "{err}");
    }

    #[test]
    fn retained_messages_and_enums_cannot_be_silently_reclaimed() {
        for declaration in ["message MinosCommand {}", "enum FUTURE_PROOF_BEHAVIOR {}"] {
            let err = generate(&format!("{UPSTREAM}\n{declaration}\n")).expect_err("collision");
            assert!(err.to_string().contains("drop the local block"), "{err}");
        }
    }

    #[test]
    fn upstream_reclaiming_the_name_fails_instead_of_duplicating_it() {
        let upstream =
            format!("{UPSTREAM}\nmessage LIDMigrationMapping {{\n  required uint64 pn = 1;\n}}\n");
        let err = generate(&upstream).expect_err("must fail");
        assert!(err.to_string().contains("drop the local block"), "{err}");
    }

    #[test]
    fn a_nested_closing_brace_does_not_end_the_anchor_early() {
        let upstream = "\
message LIDMigrationMappingSyncMessage {
  message Inner {
    optional bytes a = 1;
  }
  optional bytes encodedMappingPayload = 1;
}
message WebNotificationsInfo {
}
";
        let out = generate(upstream).expect("proto");
        let payload = out.find("encodedMappingPayload").expect("field");
        let local = out.find("message LIDMigrationMapping {").expect("local");
        assert!(payload < local, "{out}");
    }
}
