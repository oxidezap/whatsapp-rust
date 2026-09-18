//! The neutral control-plane contract must not reach into the media engine.
//!
//! `wacore::voip_control` is the boundary a media backend implements. Its whole value is that it
//! compiles with `voip` off: a build can enable `voip-control` and carry the call contract without
//! the engine. The one exception is `engine_bridge.rs`, which is `#[cfg(feature = "voip")]` and is
//! documented as the single place the two meet.
//!
//! A future edit that reaches for a `crate::voip::` path from an ungated file in the contract would
//! not fail the normal workspace build -- the engine is on there -- but it would silently break the
//! byte cut. This scans the code and fails on the reintroduction, the same way `subsystem_boundary`
//! guards the subsystem budget.
//!
//! It scans code lines only. Prose that names an engine type to explain what the contract mirrors
//! is documentation, not coupling; a `use` or a call is the thing that compiles the engine in.

use std::path::{Path, PathBuf};

/// Files allowed to name the engine, because they are gated on `voip` and are where the two meet.
const ALLOWED: &[&str] = &["engine_bridge.rs"];

/// Engine roots that must not appear in an ungated contract file. `crate::voip::` catches the
/// engine and the signaling half alike; the contract now owns its own copy of everything it needs.
const FORBIDDEN: &[&str] = &["crate::voip::", "super::voip::"];

/// Whether a line is documentation rather than code. Doc comments are prose; the engine a contract
/// file *compiles in* is what matters.
fn is_doc_or_comment(line: &str) -> bool {
    let line = line.trim_start();
    line.starts_with("//")
}

/// Code lines that belong to a `#[cfg(test)]` module are ignored: the byte cut is about what a
/// shipping build compiles, and test scaffolding is never linked into it. A test may reach for the
/// engine's KAT vectors without coupling the contract.
fn non_test_code_lines(source: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let mut test_depth: Option<i32> = None;
    let mut pending_test = false;
    let mut depth: i32 = 0;
    for (index, line) in source.lines().enumerate() {
        let trimmed = line.trim_start();
        if test_depth.is_none() && trimmed.starts_with("#[cfg(test)]") {
            pending_test = true;
        }
        // Track brace depth so a `#[cfg(test)] mod tests {` region can be skipped whole.
        if pending_test && trimmed.starts_with("mod ") {
            test_depth = Some(depth);
            pending_test = false;
        }
        if test_depth.is_none() {
            out.push((index + 1, line));
        }
        depth += line.matches('{').count() as i32;
        depth -= line.matches('}').count() as i32;
        if let Some(start) = test_depth
            && depth <= start
        {
            test_depth = None;
        }
    }
    out
}

fn contract_dirs() -> Vec<PathBuf> {
    // `CARGO_MANIFEST_DIR` is this crate's root (`wacore/`).
    vec![PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/voip_control")]
}

fn rust_sources(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {}: {e}", dir.display())) {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            rust_sources(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

#[test]
fn the_contract_never_names_the_engine() {
    let mut sources = Vec::new();
    for dir in contract_dirs() {
        rust_sources(&dir, &mut sources);
    }
    assert!(
        !sources.is_empty(),
        "the contract directory was not found; the guard would pass vacuously"
    );

    let mut offenders = Vec::new();
    for path in &sources {
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if ALLOWED.contains(&name) {
            continue;
        }
        let source = std::fs::read_to_string(path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
        for (line_number, line) in non_test_code_lines(&source) {
            if is_doc_or_comment(line) {
                continue;
            }
            if FORBIDDEN.iter().any(|needle| line.contains(needle)) {
                offenders.push(format!(
                    "{}:{} {}",
                    path.display(),
                    line_number,
                    line.trim()
                ));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "the neutral contract reached into the engine:\n{}\n\n\
         Move the type into `voip_control`, or gate the file on `voip` and document why in \
         `agent_docs/subsystem_boundary.md`.",
        offenders.join("\n"),
    );
}
