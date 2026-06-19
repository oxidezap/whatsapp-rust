//! App-state sync hot paths: the commutative LTHash over upload-scale MAC
//! batches (the SIMD lane math), the per-keyId HKDF expansion, and the full
//! inbound patch processing pass with MAC validation on a realistic
//! 50-mutation patch.

use divan::black_box;
use std::collections::HashMap;
use std::sync::Arc;
use wacore_appstate::{
    WAPATCH_INTEGRITY, decode_record, encode_record, expand_app_state_keys, hash::HashState,
    process_patch,
};
use waproto::whatsapp as wa;

fn main() {
    divan::main();
}

/// 812 MACs = one prekey-upload-sized batch; also the order of magnitude of a
/// large app-state patch apply.
#[divan::bench]
fn bench_lthash_subtract_then_add_812(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let base = [0u8; 128];
            let macs: Vec<[u8; 32]> = (0..812u32)
                .map(|i| {
                    let mut m = [0u8; 32];
                    m[..4].copy_from_slice(&i.to_le_bytes());
                    m
                })
                .collect();
            (base, macs)
        })
        .bench_refs(|(base, macs)| {
            const EMPTY: &[Vec<u8>] = &[];
            black_box(WAPATCH_INTEGRITY.subtract_then_add(black_box(&*base), EMPTY, macs))
        });
}

/// HKDF expansion of a sync key: runs once per key id per collection sync.
#[divan::bench]
fn bench_expand_app_state_keys() {
    let master = [0x42u8; 32];
    black_box(expand_app_state_keys(black_box(&master)));
}

struct PatchFixture {
    patch: wa::SyncdPatch,
    keys: Arc<wacore_appstate::ExpandedAppStateKeys>,
    prev_macs: HashMap<Vec<u8>, Vec<u8>>,
}

/// A realistic inbound patch: 50 SET mutations with valid index/value MACs,
/// half of them overwriting indices whose previous value lives in the store.
fn setup_patch(n: usize) -> PatchFixture {
    let master = [0x07u8; 32];
    let keys = expand_app_state_keys(&master);
    let key_id = b"AAAA".to_vec();

    let mut prev_macs = HashMap::new();
    let mut mutations = Vec::with_capacity(n);
    for i in 0..n {
        let index = format!("[\"star\",\"5511{i:09}@s.whatsapp.net\"]");
        let value = wa::SyncActionValue {
            timestamp: Some(1_700_000_000 + i as i64),
            star_action: Some(wa::sync_action_value::StarAction {
                starred: Some(i % 2 == 0),
            }),
            ..Default::default()
        };
        let iv = [i as u8; 16];
        let (mutation, _value_mac) = encode_record(
            wa::syncd_mutation::SyncdOperation::Set,
            index.as_bytes(),
            &value,
            &keys,
            &key_id,
            &iv,
            1,
        );
        // Half the indices have a stored previous value the prev-lookup hits.
        if i % 2 == 0
            && let Some(rec) = &mutation.record
            && let Some(idx) = rec.index.as_ref().and_then(|x| x.blob.clone())
        {
            prev_macs.insert(idx, vec![0x55u8; 32]);
        }
        mutations.push(mutation);
    }

    let mut state = HashState::default();
    // Compute the post-patch hash so the embedded snapshot MAC validates.
    let mut probe = state.clone();
    probe.version = 1;
    let (_, res) = probe.update_hash(&mutations, |idx, _| Ok(prev_macs.get(idx).cloned()));
    res.unwrap();
    let snapshot_mac = probe.generate_snapshot_mac("regular", &keys.snapshot_mac);
    state.version = 0;

    let mut patch = wa::SyncdPatch {
        version: Some(wa::SyncdVersion { version: Some(1) }),
        mutations,
        key_id: Some(wa::KeyId {
            id: Some(key_id.clone()),
        }),
        snapshot_mac: Some(snapshot_mac),
        ..Default::default()
    };
    patch.patch_mac = Some(wacore_appstate::hash::generate_patch_mac(
        &patch,
        "regular",
        &keys.patch_mac,
        1,
    ));

    PatchFixture {
        patch,
        keys: Arc::new(keys),
        prev_macs,
    }
}

#[divan::bench]
fn bench_process_patch_50_validated(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| setup_patch(50))
        .bench_refs(|fixture| {
            let keys = Arc::clone(&fixture.keys);
            let mut state = HashState::default();
            black_box(
                process_patch(
                    &fixture.patch,
                    &mut state,
                    |_| Ok(Arc::clone(&keys)),
                    |idx| Ok(fixture.prev_macs.get(idx).cloned()),
                    true,
                    "regular",
                )
                .unwrap(),
            );
        });
}

/// One encrypted app-state record built the way the server delivers it, so
/// `decode_record` runs its full inbound cost: AES-256-CBC decrypt, the content
/// HMAC-SHA512 and index HMAC-SHA256 validation, the prost decode of the action
/// value, and the serde_json parse of the index array. This runs once per
/// mutation, up to ~1000× per resume patch; `process_patch` covers the whole
/// loop but never isolates this inner per-record cost.
fn setup_record(
    shape: &str,
) -> (
    wa::SyncdRecord,
    wacore_appstate::ExpandedAppStateKeys,
    Vec<u8>,
) {
    let keys = expand_app_state_keys(&[0x07u8; 32]);
    let key_id = b"AAAA".to_vec();
    let (index, value) = match shape {
        // A starred-message mutation: 5-part index (the real STAR schema),
        // tiny action value.
        "star" => (
            "[\"star\",\"5511000000000@s.whatsapp.net\",\"3EB0123456789ABCDEF01234\",\"1\",\"0\"]"
                .to_string(),
            wa::SyncActionValue {
                timestamp: Some(1_700_000_000),
                star_action: Some(wa::sync_action_value::StarAction {
                    starred: Some(true),
                }),
                ..Default::default()
            },
        ),
        // A contact mutation: longer index plus a name-carrying value, the
        // larger-payload end of the AES/HMAC cost.
        "contact" => (
            "[\"contact\",\"5511999998888@s.whatsapp.net\"]".to_string(),
            wa::SyncActionValue {
                timestamp: Some(1_700_000_000),
                contact_action: Some(wa::sync_action_value::ContactAction {
                    full_name: Some("Benchmark Contact Full Name".to_string()),
                    first_name: Some("Benchmark".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            },
        ),
        other => unreachable!("unknown shape {other}"),
    };
    let (mutation, _value_mac) = encode_record(
        wa::syncd_mutation::SyncdOperation::Set,
        index.as_bytes(),
        &value,
        &keys,
        &key_id,
        &[0x11u8; 16],
        1,
    );
    let record = mutation.record.expect("encoded record");
    (record, keys, key_id)
}

#[divan::bench(args = ["star", "contact"])]
fn bench_decode_record(bencher: divan::Bencher, shape: &str) {
    bencher
        .with_inputs(|| setup_record(shape))
        .bench_refs(|(record, keys, key_id)| {
            black_box(
                decode_record(
                    wa::syncd_mutation::SyncdOperation::Set,
                    black_box(record),
                    black_box(keys),
                    black_box(key_id),
                    true,
                )
                .unwrap(),
            )
        });
}
