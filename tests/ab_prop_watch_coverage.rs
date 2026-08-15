//! Every A/B gate this crate reads must be watched.
//!
//! `AbPropsCache::apply_props` keeps only codes in its interest set, seeded from
//! `iq::props::WATCHED`. A prop read without being watched is therefore not a
//! prop that reads stale -- it is one whose server value was thrown away during
//! parsing, so the read returns the registry default now and on every future
//! connect. Nothing errors and nothing logs.
//!
//! That is worth a scan rather than a runtime check alone. The cache does
//! `debug_assert` on read, but only a test that actually exercises the gated
//! path can trip it, and a gate is usually added precisely because the path is
//! hard to reach. Three shipped gates were dead this way before anyone noticed:
//! `receipt_mode_bitmask_enabled`, `enable_spam_report_iq_with_privacy_token`
//! and `profile_scraping_privacy_token_in_about_usync`.
//!
//! The scan is textual on purpose. Resolving these paths properly would mean
//! running the compiler, and the failure being guarded is a name appearing in
//! one file and not another -- exactly what text sees.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use wacore::iq::props::WATCHED;

fn manifest_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Every `.rs` file under `src/`, as (display path, contents).
fn sources() -> Vec<(String, String)> {
    fn walk(dir: &Path, out: &mut Vec<(String, String)>) {
        let entries = std::fs::read_dir(dir).unwrap_or_else(|e| panic!("read {dir:?}: {e}"));
        for entry in entries {
            let path = entry.expect("dir entry").path();
            if path.is_dir() {
                walk(&path, out);
            } else if path.extension().is_some_and(|e| e == "rs") {
                let text =
                    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {path:?}: {e}"));
                out.push((path.display().to_string(), text));
            }
        }
    }
    let mut out = Vec::new();
    walk(&manifest_path("src"), &mut out);
    assert!(!out.is_empty(), "no sources found under src/");
    out
}

/// Screaming-snake identifiers qualified by a prop registry module, as
/// (identifier, file). The emitter names each constant after the flag it
/// carries, so `web::FOO_ENABLED` is the constant for `foo_enabled`.
fn referenced_props(sources: &[(String, String)]) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (file, text) in sources {
        for module in ["web::", "stale::"] {
            let mut rest = text.as_str();
            while let Some(at) = rest.find(module) {
                rest = &rest[at + module.len()..];
                let ident: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_uppercase() || c.is_ascii_digit() || *c == '_')
                    .collect();
                // Lowercase or mixed means this was some other `web::` path,
                // not a flag constant.
                if ident.len() > 1
                    && !rest[ident.len()..].starts_with(|c: char| c.is_alphanumeric())
                {
                    found.push((ident, file.clone()));
                }
            }
        }
    }
    found
}

#[test]
fn every_ab_prop_this_crate_reads_is_watched() {
    let watched: HashMap<String, u32> = WATCHED
        .iter()
        .map(|p| (p.name.to_uppercase(), p.code))
        .collect();

    let sources = sources();
    let referenced = referenced_props(&sources);
    assert!(
        !referenced.is_empty(),
        "the scan found no prop constants at all, so it is no longer guarding anything"
    );

    let mut unwatched: Vec<String> = referenced
        .iter()
        .filter(|(ident, _)| !watched.contains_key(ident))
        .map(|(ident, file)| format!("{ident} (read in {file})"))
        .collect();
    unwatched.sort();
    unwatched.dedup();

    assert!(
        unwatched.is_empty(),
        "these A/B props are read but absent from `WATCHED` in \
         wacore/src/iq/props.rs, so the server's value is discarded and each \
         read yields the registry default forever:\n  {}",
        unwatched.join("\n  "),
    );
}
