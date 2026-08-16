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
//!
//! What it looks for is the *read*, not the name: the argument to a cache
//! accessor. Keying on the name alone is too loose -- the registry has
//! thousands of flags, and `GROUP_CALL_MAX_PARTICIPANTS` (a `usize` derived
//! from a flag) and the `PLACEHOLDER_MESSAGE_RESEND` proto enum variant both
//! spell one without reading anything. Keying on a `web::` prefix is too
//! tight, since a grouped `use ...::web::{FOO}` leaves the call site saying
//! only `FOO`. The accessor argument is the thing that actually consults the
//! cache, so it is neither.
//!
//! The one form that escapes is a prop bound to a differently-named local
//! constant first. Nothing does that today, and the runtime `debug_assert`
//! still covers it if anything ever does.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use wacore::iq::abprops::{self, AbProp};
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

/// Every flag the registries declare, keyed by the constant that carries it.
/// The emitter names each constant after its flag, so `foo_enabled` is
/// `FOO_ENABLED`; `props::stale` is hand-written to the same convention.
fn registry() -> HashMap<String, AbProp> {
    let mut out = HashMap::new();
    for module in abprops::ALL {
        for prop in *module {
            out.insert(prop.name.to_uppercase(), *prop);
        }
    }
    // `stale` holds flags the bundle no longer builds, so they are in no
    // generated registry but are still read and still have to be watched.
    for prop in WATCHED {
        out.insert(prop.name.to_uppercase(), *prop);
    }
    out
}

/// The accessors that consult the cache for one flag. `watch`/`watch_many`
/// are not reads, and a prop passed to them is registered by that very call.
const ACCESSORS: &[&str] = &["is_enabled(", "get_int(", ".get("];

/// Flags passed to a cache accessor, as (identifier, file).
///
/// Takes the argument's last `::` segment, so a qualified path and a bare name
/// reduce to the same constant. Anything that is not a screaming-snake registry
/// name -- a local, an expression -- is not a flag and is skipped.
fn referenced_props(
    sources: &[(String, String)],
    registry: &HashMap<String, AbProp>,
) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for (file, text) in sources {
        for accessor in ACCESSORS {
            let mut rest = text.as_str();
            while let Some(at) = rest.find(accessor) {
                rest = &rest[at + accessor.len()..];
                let Some(end) = rest.find(')') else { continue };
                let arg = rest[..end].trim().trim_end_matches(',').trim();
                let ident = arg.rsplit("::").next().unwrap_or(arg).trim();
                if registry.contains_key(ident) {
                    found.push((ident.to_string(), file.clone()));
                }
            }
        }
    }
    found
}

#[test]
fn every_ab_prop_this_crate_reads_is_watched() {
    let registry = registry();
    let watched: HashSet<String> = WATCHED.iter().map(|p| p.name.to_uppercase()).collect();

    let sources = sources();
    let referenced = referenced_props(&sources, &registry);
    assert!(
        !referenced.is_empty(),
        "the scan found no prop constants at all, so it is no longer guarding anything"
    );

    let mut unwatched: Vec<String> = referenced
        .iter()
        .filter(|(ident, _)| !watched.contains(ident))
        .map(|(ident, file)| {
            let code = registry[ident].code;
            format!("{ident} (code {code}, read in {file})")
        })
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

/// The scan is only worth having if it sees a name that no `web::` prefix
/// introduces, which is the form the previous version missed.
#[test]
fn a_grouped_import_is_still_seen() {
    let registry = registry();
    let watched_name = WATCHED[0].name.to_uppercase();
    let source = vec![(
        "fake.rs".to_string(),
        format!(
            "use wacore::iq::abprops::web::{{{watched_name}}};\n\
             let on = cache.is_enabled({watched_name}).await;\n"
        ),
    )];
    // Named without reading, which the scan must not treat as a read.
    let decoy = vec![(
        "decoy.rs".to_string(),
        "let n = GROUP_CALL_MAX_PARTICIPANTS;\n\
         let t = wa::message::PeerDataOperationRequestType::PLACEHOLDER_MESSAGE_RESEND;\n"
            .to_string(),
    )];
    assert!(
        referenced_props(&decoy, &registry).is_empty(),
        "the scan counted a flag name that no accessor reads"
    );

    let found = referenced_props(&source, &registry);
    assert!(
        found.iter().any(|(ident, _)| *ident == watched_name),
        "the scan missed {watched_name} brought in by a grouped import"
    );
}
