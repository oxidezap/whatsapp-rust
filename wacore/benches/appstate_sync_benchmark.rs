//! App-state patch-list hot paths in the `wacore` orchestration layer: the
//! index-MAC dedup that feeds the batched previous-value-MAC lookup, run once
//! per inbound patch and per outbound build_patch. The linear scan is O(N²)
//! over distinct indices; HashSet measured slower at small N in this codebase,
//! so both ends are pinned here before any swap.

use divan::black_box;
use wacore::appstate_sync::collect_unique_index_macs;
use waproto::whatsapp as wa;

fn main() {
    divan::main();
}

/// N SET mutations with distinct 32-byte index MACs — distinct indices are the
/// realistic patch shape and the scan's worst case (full compare per element).
fn setup_mutations(n: usize) -> Vec<wa::SyncdMutation> {
    (0..n as u64)
        .map(|i| {
            let mut index_mac = vec![0u8; 32];
            index_mac[..8].copy_from_slice(&i.to_le_bytes());
            wa::SyncdMutation {
                operation: Some(wa::syncd_mutation::SyncdOperation::Set.into()),
                record: buffa::MessageField::some(wa::SyncdRecord {
                    index: buffa::MessageField::some(wa::SyncdIndex {
                        blob: Some(index_mac),
                    }),
                    value: buffa::MessageField::some(wa::SyncdValue {
                        blob: Some(vec![0x5A; 48]),
                    }),
                    key_id: buffa::MessageField::some(wa::KeyId {
                        id: Some(b"AAAA".to_vec()),
                    }),
                }),
            }
        })
        .collect()
}

/// 10 = a typical incremental patch; 1000 = the resume-sync upper bound, where
/// the quadratic scan does ~500k Vec<u8> compares.
#[divan::bench(args = [10, 1000])]
fn bench_collect_unique_index_macs(bencher: divan::Bencher, n: usize) {
    bencher
        .with_inputs(|| setup_mutations(n))
        .bench_refs(|mutations| black_box(collect_unique_index_macs(black_box(mutations))));
}
