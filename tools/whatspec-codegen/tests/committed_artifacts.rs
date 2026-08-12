//! The committed artifacts all describe one WhatsApp build.
//!
//! This is the offline half of `--check`: it needs no whatspec checkout, so it
//! runs in the ordinary test matrix and catches the failure that actually
//! happened before this tool existed, which is a single domain being refreshed
//! on its own. `--check` covers the rest, byte for byte, against the pinned IR.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("the crate lives at <root>/tools/whatspec-codegen")
        .to_path_buf()
}

fn read(rel: &str) -> String {
    let path = repo_root().join(rel);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

/// The version out of `(WhatsApp <version>)` in a generated file's first line.
fn stamped_in_header(text: &str) -> Option<&str> {
    let line = text.lines().next()?;
    let start = line.find("(WhatsApp ")? + "(WhatsApp ".len();
    let rest = &line[start..];
    Some(&rest[..rest.find(')')?])
}

/// The version the lock pins, read without a JSON dependency in the test.
fn locked_version() -> String {
    let lock = read("tools/whatspec-codegen/whatspec.lock.json");
    let key = "\"waVersion\":";
    let after = lock.split(key).nth(1).expect("lock has a waVersion");
    after
        .split('"')
        .nth(1)
        .expect("waVersion is a string")
        .to_string()
}

#[test]
fn every_generated_rust_file_stamps_the_locked_version() {
    let want = locked_version();
    for rel in [
        "wacore/src/version/generated.rs",
        "wacore/src/iq/abprops.rs",
        "wacore/src/iq/mex_operations.rs",
        "wacore/appstate/src/schemas.rs",
    ] {
        let text = read(rel);
        let got = stamped_in_header(&text)
            .unwrap_or_else(|| panic!("{rel} has no `(WhatsApp <version>)` header"));
        assert_eq!(
            got, want,
            "{rel} was generated from WhatsApp {got} but the lock pins {want}; \
             regenerate the whole set with `cargo run -p whatspec-codegen`"
        );
    }
}

#[test]
fn the_proto_and_the_version_const_agree_with_the_lock() {
    let want = locked_version();
    let proto = read("waproto/src/whatsapp.proto");
    let stamped = proto
        .lines()
        .find_map(|l| l.strip_prefix("/// WhatsApp Version: "))
        .expect("the proto carries its version");
    assert_eq!(
        stamped, want,
        "waproto/src/whatsapp.proto is from another build"
    );

    let generated = read("wacore/src/version/generated.rs");
    assert!(
        generated.contains(&format!("WA_WEB_VERSION_STR: &str = \"{want}\"")),
        "the version const does not spell {want}"
    );
    let triple: Vec<&str> = want.split('.').collect();
    assert!(
        generated.contains(&format!(
            "WA_WEB_VERSION: (u32, u32, u32) = ({}, {}, {})",
            triple[0], triple[1], triple[2]
        )),
        "the version triple does not match {want}"
    );
}

#[test]
fn a_header_from_another_build_is_detected() {
    // The assertion above is only worth anything if the extraction actually
    // reads the version rather than returning None and passing vacuously.
    let good = "//! Auto-generated x (WhatsApp 2.3000.7). DO NOT EDIT.\n//!\n";
    assert_eq!(stamped_in_header(good), Some("2.3000.7"));
    assert_eq!(stamped_in_header("//! hand written\n"), None);
    assert_eq!(stamped_in_header(""), None);
    assert_eq!(
        stamped_in_header("//! (WhatsApp 2.3000.7 unterminated\n"),
        None
    );
}

#[test]
fn the_token_tables_stay_wire_indexable() {
    // tokens.json carries no version stamp, so this is what the other tests are
    // for it: index 0 of the single-byte table is the reserved empty token, and
    // a regeneration that lost it would shift every token by one.
    let tokens = read("wacore/binary/src/tokens.json");
    let single = tokens
        .split("\"single_byte\": [")
        .nth(1)
        .expect("single_byte table");
    assert!(
        single.trim_start().starts_with("\"\","),
        "single_byte no longer starts with the reserved empty token"
    );
    assert!(tokens.contains("\"double_byte\": ["));
}
