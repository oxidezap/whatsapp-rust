//! Probe benchmarks for the `ProtocolAddress` key path.
//!
//! `SignalStoreCache` keys its session and identity maps by
//! `address.as_str()`, at 53 call sites, so a session-heavy send renders an
//! address far more often than the whole-pipeline benches show. There it
//! happens a handful of times per iteration and disappears under SHA-256:
//! `bench_dm_send` spends 43% of its time in `sha2::compress256` and only
//! ~0.36% rendering addresses, which is under the threshold at which a
//! benchmark reads as changed at all.
//!
//! These workloads probe a populated cache at the rate production reaches, so
//! a change to how the address renders or compares shows up as a number
//! instead of a rounding error.

use divan::black_box;
use std::collections::HashMap;
use wacore::store::SignalStoreCache;
use wacore::types::jid::JidExt;
use wacore_binary::jid::{Jid, Server};
use wacore_libsignal::protocol::ProtocolAddress;

fn main() {
    divan::main();
}

/// SipHash with fixed keys: the default `RandomState` seeds per process, so
/// bucket layout — and with it the cache behavior these probes measure —
/// would differ between runs.
type DetState = std::hash::BuildHasherDefault<std::hash::DefaultHasher>;
type DetHashMap<K, V> = HashMap<K, V, DetState>;

/// Device counts spanning what a real fan-out touches: a small chat's device
/// set, and a busy account whose cache holds many peers at once.
const FANOUTS: &[usize] = &[8, 64];

/// Addresses shaped like the ones a fan-out touches: PN and LID peers, each
/// with the companion devices a paired account carries. Numbers are
/// fictitious.
fn addresses(count: usize, salt: u64) -> Vec<ProtocolAddress> {
    (0..count)
        .map(|i| {
            let i = i as u64;
            let jid = if i.is_multiple_of(2) {
                Jid::new(
                    format!("5511{:09}", 900_000_000 + salt + i * 7919),
                    Server::Pn,
                )
            } else {
                Jid::new(
                    format!("1005{:011}", 15_017_610_342u64 + salt + i * 104_729),
                    Server::Lid,
                )
            };
            jid.with_device((i % 4) as u16).to_protocol_address()
        })
        .collect()
}

/// The identity read every device in a fan-out performs. `try_get_identity`
/// renders the address and probes the map, which is the whole of the work
/// once the entry is warm — exactly the shape the 53 call sites multiply.
#[divan::bench(args = FANOUTS)]
fn identity_probe_hits(bencher: divan::Bencher, fanout: usize) {
    let cache = SignalStoreCache::new();
    let addrs = addresses(fanout, 0);
    for addr in &addrs {
        assert!(
            cache.try_put_identity(addr, b"0123456789abcdef0123456789abcdef"),
            "uncontended put must succeed"
        );
    }

    bencher.bench_local(|| {
        let mut found = 0u32;
        for addr in &addrs {
            if cache.try_get_identity(black_box(addr)).is_some() {
                found += 1;
            }
        }
        black_box(found)
    });
}

/// The same probe against addresses the cache has never seen. A miss still
/// pays the full render before the lookup fails, so it must not regress while
/// the hit path improves.
#[divan::bench(args = FANOUTS)]
fn identity_probe_misses(bencher: divan::Bencher, fanout: usize) {
    let cache = SignalStoreCache::new();
    for addr in &addresses(fanout, 0) {
        cache.try_put_identity(addr, b"0123456789abcdef0123456789abcdef");
    }
    let absent = addresses(fanout, 1_000_000);

    bencher.bench_local(|| {
        let mut misses = 0u32;
        for addr in &absent {
            if cache.try_get_identity(black_box(addr)).is_none() {
                misses += 1;
            }
        }
        black_box(misses)
    });
}

/// A map keyed by `ProtocolAddress` rather than by its rendered string, which
/// is how the in-memory stores the pipeline benches use are shaped. This
/// routes through `Hash` and `Eq`, both of which read `as_str()`, so it pins
/// the other way the render is reached.
#[divan::bench(args = FANOUTS)]
fn address_keyed_map_probe(bencher: divan::Bencher, fanout: usize) {
    let addrs = addresses(fanout, 0);
    let map: DetHashMap<ProtocolAddress, u32> = addrs
        .iter()
        .enumerate()
        .map(|(i, addr)| (addr.clone(), i as u32))
        .collect();

    bencher.bench_local(|| {
        let mut sum = 0u32;
        for addr in &addrs {
            sum += map.get(black_box(addr)).copied().unwrap_or_default();
        }
        black_box(sum)
    });
}
