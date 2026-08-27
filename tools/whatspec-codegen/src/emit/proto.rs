//! `waproto/src/whatsapp.proto`: the upstream schema plus the messages this
//! repository keeps after WA dropped them from the public bundle.
//!
//! Local *fields* belong in `waproto/build.rs`'s `LOCAL_FIELDS`, which splices
//! them into the descriptor. A whole message has no such hook, so it is declared
//! here and spliced into the `.proto` text instead. Either way the addition
//! survives a sync, and a sync that starts declaring the same name fails the
//! build rather than quietly taking over a type we already persist.

use anyhow::{Result, bail};

/// A message this repository declares that the upstream bundle no longer does.
struct LocalBlock {
    /// Spliced in after this top-level message's closing brace, so the block
    /// lands beside the message it belongs to instead of at the end of the file.
    after_message: &'static str,
    /// Type names the block declares. Upstream declaring one of these means the
    /// block is obsolete and must be deleted, not duplicated.
    declares: &'static [&'static str],
    text: &'static str,
}

const LOCAL_BLOCKS: &[LocalBlock] = &[LocalBlock {
    after_message: "LIDMigrationMappingSyncMessage",
    declares: &["LIDMigrationMapping", "LIDMigrationMappingSyncPayload"],
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
}];

pub fn generate(upstream: &str) -> Result<String> {
    let mut out = upstream.to_string();
    for block in LOCAL_BLOCKS {
        for name in block.declares {
            if declares_message(upstream, name) {
                bail!(
                    "upstream now declares `message {name}`; drop the local block anchored at \
                     `{}` in the proto emitter instead of shadowing it",
                    block.after_message
                );
            }
        }
        let at = end_of_message(&out, block.after_message).ok_or_else(|| {
            anyhow::anyhow!(
                "anchor `message {}` is gone from the upstream proto; re-anchor the local block \
                 declaring {:?}",
                block.after_message,
                block.declares
            )
        })?;
        out.insert_str(at, &format!("\n{}", block.text));
    }
    Ok(out)
}

/// Whether the file declares a top-level `message <name>`.
fn declares_message(proto: &str, name: &str) -> bool {
    proto
        .lines()
        .any(|l| l == format!("message {name} {{") || l == format!("message {name} {{}}"))
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

    const UPSTREAM: &str = "\
syntax = \"proto2\";
package whatsapp;

message LIDMigrationMappingSyncMessage {
  optional bytes encodedMappingPayload = 1;
}

message LabyrinthWaCommand {
  oneof commandInput {
    CreateBackupInput createBackupInput = 1;
  }
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
        let next = out
            .find("message LabyrinthWaCommand")
            .expect("next message");
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
";
        let out = generate(upstream).expect("proto");
        let payload = out.find("encodedMappingPayload").expect("field");
        let local = out.find("message LIDMigrationMapping {").expect("local");
        assert!(payload < local, "{out}");
    }
}
