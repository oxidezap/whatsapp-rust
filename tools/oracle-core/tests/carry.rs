//! Carrying an index between captures.
//!
//! This is the one place the two halves of this repository share code, so it is
//! the one place a test has to show the sharing earns its keep. What it must
//! not do is answer confidently when it cannot: an index carried to the wrong
//! function reads exactly like a correct one, because the reads still succeed.

use oracle_core::Catalog;
use oracle_core::carry::{Captures, Carried};

fn module(id: &str) -> Option<Vec<u8>> {
    let catalog = Catalog::discover().ok()?;
    let entry = catalog.resolve(id).ok()?;
    std::fs::read(&entry.path).ok()
}

/// A module carried against itself: every fingerprintable function must find
/// itself, and nothing else.
///
/// The strongest available check that does not need two captures on disk — and
/// it is not trivial, because a shape shared by two helpers is ambiguous even
/// here, which is precisely the case that must not be answered.
#[test]
fn a_capture_carried_against_itself_finds_every_function_where_it_already_is() {
    let Some(bytes) = module("COs9e0Kj0ic") else {
        eprintln!("skipping: COs9e0Kj0ic unavailable (set WA_WASM_DIR)");
        return;
    };

    let captures = Captures::new(&bytes, &bytes).expect("decode");
    let coverage = captures.coverage();
    assert_eq!(
        coverage.old_functions, coverage.new_functions,
        "the same module has the same function count"
    );
    assert!(
        coverage.carried > 0,
        "some functions should be uniquely fingerprintable"
    );

    // Every unique answer must be the identity. A different index would mean
    // the hash collided across two distinct bodies.
    let mut unique = 0;
    let first = coverage.new_first_defined;
    for index in first..first + coverage.new_functions as u32 {
        if let Carried::One(found) = captures.carry(index) {
            assert_eq!(found, index, "func {index} carried to {found}");
            unique += 1;
        }
    }
    assert_eq!(
        unique, coverage.carried,
        "coverage should count exactly the unique answers"
    );
}

/// The two real captures, when both are present.
#[test]
fn the_voip_bump_is_a_renumbering_rather_than_a_rewrite() {
    let (Some(old), Some(new)) = (module("D5pLH9sfOOl"), module("JgwtTQVeWPm")) else {
        eprintln!("skipping: needs both VoIP captures; the lock pins only the current one");
        return;
    };

    let captures = Captures::new(&old, &new).expect("decode");
    let coverage = captures.coverage();

    // Measured at 6561 when this was written. Asserted as a floor rather than
    // an equality: the point is that most of the module survives the bump, and
    // pinning the exact number would make this a test about the hash.
    assert!(
        coverage.carried > 5_000,
        "most of the engine should carry forward, got {}",
        coverage.carried
    );

    // The log dispatcher, which is how `LOG_LEVEL` was re-derived.
    assert_eq!(captures.carry(13_445), Carried::One(14_839));

    // And the code WhatsApp actually edited does not carry. This is the half of
    // the answer that matters: a tool that reported something here would have
    // sent the offer-guard offsets to a function that no longer exists.
    assert_eq!(
        captures.carry(11_198),
        Carried::Changed,
        "make_and_cache_offer was rewritten in this rollout"
    );
}

/// A body too short to distinguish is refused, not guessed.
#[test]
fn a_short_body_is_refused_rather_than_matched() {
    let Some(bytes) = module("COs9e0Kj0ic") else {
        eprintln!("skipping: COs9e0Kj0ic unavailable (set WA_WASM_DIR)");
        return;
    };
    let captures = Captures::new(&bytes, &bytes).expect("decode");

    // Index 0 is an import in every captured module here, so it has no body at
    // all — the clearest case of "there is nothing to fingerprint".
    assert_eq!(captures.carry(0), Carried::NotFingerprintable);
    assert_eq!(captures.carry(9_999_999), Carried::NotFingerprintable);
}
