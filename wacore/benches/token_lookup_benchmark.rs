//! Token-table probe benchmarks: `index_of_token` runs once per string in
//! every outgoing stanza (tags, attr names, attr values), and most probes
//! miss. Hit and miss workloads are pinned separately so a lookup-structure
//! change can't trade one for the other unnoticed.

use divan::black_box;
use wacore_binary::token::{get_double_token, get_single_token, index_of_token};

fn main() {
    divan::main();
}

/// Every dictionary token once — the exhaustive hit workload.
fn all_tokens() -> Vec<&'static str> {
    let mut toks = Vec::new();
    for i in 0..=u8::MAX {
        if let Some(t) = get_single_token(i)
            && !t.is_empty()
        {
            toks.push(t);
        }
    }
    for dict in 0..4u8 {
        for i in 0..=u8::MAX {
            if let Some(t) = get_double_token(dict, i)
                && !t.is_empty()
            {
                toks.push(t);
            }
        }
    }
    toks
}

/// Strings shaped like real stanza values that are not tokens: JID users,
/// message ids, hex/base64 fragments, short numerics. These dominate the
/// probe traffic on the encode path.
fn miss_strings() -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..64u64 {
        out.push(format!("15551{:07}", i * 7919)); // phone-number users
        out.push(format!("3EB0{:012X}", i.wrapping_mul(0x9E37_79B9))); // msg ids
        out.push(format!("{}", i * 37)); // short numerics
        out.push(format!("{}", i * 32 + 1)); // in-bucket numerics (the fat 1-4 byte groups)
        out.push(format!("abcdef{:02x}u", i)); // hex-ish short strings
    }
    // A few short numerics ("0", "37", "407", ...) are real dictionary
    // tokens; drop them so this stays a pure miss metric.
    out.retain(|s| index_of_token(s).is_none());
    out
}

#[divan::bench]
fn token_lookup_hits(bencher: divan::Bencher) {
    let toks = all_tokens();
    bencher.bench_local(|| {
        let mut found = 0u32;
        for t in &toks {
            if index_of_token(black_box(t)).is_some() {
                found += 1;
            }
        }
        black_box(found)
    });
}

#[divan::bench]
fn token_lookup_misses(bencher: divan::Bencher) {
    let strs = miss_strings();
    bencher.bench_local(|| {
        let mut found = 0u32;
        for s in &strs {
            if index_of_token(black_box(s)).is_some() {
                found += 1;
            }
        }
        black_box(found)
    });
}
