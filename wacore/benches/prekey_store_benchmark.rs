//! Allocation accounting for the prekey batch write on the connect path.
//!
//! `upload_pre_keys_pass` generates `DEFAULT_WANTED_PRE_KEY_COUNT` (812) one-time
//! prekeys and hands them to `store_prekeys_batch` as one call. `divan::AllocProfiler`
//! is wired as the global allocator so each row reports allocation count and bytes
//! next to wall time -- the count is the signal here, since the map's growth is the
//! only thing this path allocates: the records themselves are `Bytes` slices of one
//! shared buffer the caller already owns, so they cost refcount bumps, not copies.
//!
//! The `populated` row is the second upload pass: the same batch size arriving at a
//! map that already holds a window. It exists so a reservation that over-grows an
//! already-populated table would show up as bytes here rather than silently.

use bytes::Bytes;
use divan::{Bencher, black_box};
use futures::executor::block_on;
use wacore::store::in_memory::InMemoryBackend;
use wacore::store::traits::SignalStore;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

/// `DEFAULT_WANTED_PRE_KEY_COUNT` in `src/prekeys.rs`, mirroring WA Web's
/// UPLOAD_KEYS_COUNT. The small row is there to show the growth is what scales.
const CONNECT_BATCH: usize = 812;

/// Upper bound on one encoded `PreKeyRecordStructure`, the same figure
/// `upload_pre_keys_pass` sizes its shared buffer with.
const RECORD_LEN: usize = 74;

/// The batch as the upload path actually hands it over: every record is a slice of
/// one contiguous buffer, so building it allocates once regardless of `count`.
fn batch(first_id: u32, count: usize) -> Vec<(u32, Bytes)> {
    let shared = Bytes::from(vec![7u8; count * RECORD_LEN]);
    (0..count)
        .map(|i| {
            (
                first_id + i as u32,
                shared.slice(i * RECORD_LEN..(i + 1) * RECORD_LEN),
            )
        })
        .collect()
}

#[divan::bench(args = [64, CONNECT_BATCH])]
fn store_prekeys_batch(bencher: Bencher, count: usize) {
    bencher
        .with_inputs(|| (InMemoryBackend::new(), batch(1, count)))
        .bench_refs(|(backend, keys)| {
            block_on(backend.store_prekeys_batch(keys, false)).unwrap();
            black_box(&backend);
        });
}

/// Prekey ids are minted from the monotonic NEXT_PK_ID counter, so a second pass
/// carries ids past the first window's -- every row is new, none overwrites.
#[divan::bench(name = "store_prekeys_batch/populated")]
fn store_prekeys_batch_populated(bencher: Bencher) {
    bencher
        .with_inputs(|| {
            let backend = InMemoryBackend::new();
            block_on(backend.store_prekeys_batch(&batch(1, CONNECT_BATCH), true)).unwrap();
            let next = batch(CONNECT_BATCH as u32 + 1, CONNECT_BATCH);
            (backend, next)
        })
        .bench_refs(|(backend, keys)| {
            block_on(backend.store_prekeys_batch(keys, false)).unwrap();
            black_box(&backend);
        });
}
