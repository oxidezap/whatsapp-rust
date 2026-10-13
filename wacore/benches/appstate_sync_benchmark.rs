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
            {
                let mut proto = wa::SyncdMutation::default();
                proto.operation = Some(wa::syncd_mutation::SyncdOperation::Set.into());
                proto.record = buffa::MessageField::some({
                    let mut proto = wa::SyncdRecord::default();
                    proto.index = buffa::MessageField::some({
                        let mut proto = wa::SyncdIndex::default();
                        proto.blob = Some(index_mac);
                        proto
                    });
                    proto.value = buffa::MessageField::some({
                        let mut proto = wa::SyncdValue::default();
                        proto.blob = Some(vec![0x5A; 48]);
                        proto
                    });
                    proto.key_id = buffa::MessageField::some({
                        let mut proto = wa::KeyId::default();
                        proto.id = Some(b"AAAA".to_vec());
                        proto
                    });
                    proto
                });
                proto
            }
        })
        .collect()
}

/// 10 and 1000 straddle `MAC_DEDUP_SCAN_LIMIT` (64): 10 takes the linear
/// scan — distinct indices are its worst case, a full compare per element —
/// while 1000 takes the sort+dedup path, the resume-sync upper bound.
#[divan::bench(args = [10, 1000])]
fn bench_collect_unique_index_macs(bencher: divan::Bencher, n: usize) {
    bencher
        .with_inputs(|| setup_mutations(n))
        .bench_refs(|mutations| black_box(collect_unique_index_macs(black_box(mutations))));
}

/// Same widths over one repeated index: the dedup early-out on the scan path
/// (10) and an already-sorted input on the sort path (1000). The delta
/// against the distinct rows above is the best-vs-worst spread a swap
/// (HashSet, sort) must beat on both ends before it lands.
fn setup_duplicate_mutations(n: usize) -> Vec<wa::SyncdMutation> {
    let one = setup_mutations(1).pop().expect("one mutation");
    vec![one; n]
}

#[divan::bench(args = [10, 1000])]
fn bench_collect_unique_index_macs_duplicates(bencher: divan::Bencher, n: usize) {
    bencher
        .with_inputs(|| setup_duplicate_mutations(n))
        .bench_refs(|mutations| black_box(collect_unique_index_macs(black_box(mutations))));
}
