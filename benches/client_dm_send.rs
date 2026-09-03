//! Client-level 1:1 send, swept across the recipient's device count.
//!
//! `wacore`'s `bench_dm_send` covers the stanza preparation with sessions
//! already in hand; this is everything the client does around it on a warm
//! repeat DM: the wire-namespace decision, the memoized device resolution,
//! the LID/PN mapping lookups on the way to each device's Signal address,
//! the session pre-check, the per-device lock keys, and the marshal and noise
//! encrypt before the sink.
//!
//! Two addressing shapes, because they are two per-device code paths (see
//! `DmAddressing`): a migrated account sending to a LID, where no device
//! needs a mapping lookup, and an unmigrated account sending to a PN with a
//! known LID, where every device's address is looked up at each resolution
//! point. A repeat DM to the same recipient is the common case, so the sweep
//! is over the recipient's device count (1 = a phone, 5 = the maximum with
//! four companions) at a fixed one companion of our own.
//!
//! Not covered, so no number here is over-read: the SQLite backend (the
//! fixture stores in memory), first contact (the fixture is warm), and the
//! socket write (the transport drops the frame).

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};
use whatsapp_rust::bench_support::{DmAddressing, DmSendHarness};

fn main() {
    divan::main();
}

/// The recipient's device count: a phone alone, a phone with one linked
/// device, and the ceiling of four linked devices.
const PEER_DEVICES: &[usize] = &[1, 2, 5];

/// Every measured send advances each pairwise ratchet by one and re-arms the
/// coalesced signal flush; neither crosses a threshold, so the budget only
/// keeps the run short.
const SAMPLE_COUNT: u32 = 20;
const SAMPLE_SIZE: u32 = 20;

/// One fixture per (benchmark, addressing, device count), leaked for the
/// process lifetime, for the reasons `client_group_send` gives: a fixture is
/// several acknowledged X3DH exchanges and two warm-up sends, and a shared one
/// would carry ratchet state and an armed flush worker between benchmarks.
fn shared(
    label: &'static str,
    addressing: DmAddressing,
    peer_devices: usize,
) -> &'static DmSendHarness {
    type Key = (&'static str, DmAddressing, usize);
    static FIXTURES: OnceLock<Mutex<HashMap<Key, &'static DmSendHarness>>> = OnceLock::new();
    let mut map = FIXTURES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .expect("fixture registry");
    map.entry((label, addressing, peer_devices))
        .or_insert_with(|| Box::leak(Box::new(DmSendHarness::new(peer_devices, addressing))))
}

/// A migrated account's repeat DM to a LID recipient.
#[divan::bench(args = PEER_DEVICES, sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn warm_dm_send_lid(bencher: divan::Bencher, peer_devices: usize) {
    let harness = shared("warm_dm_send_lid", DmAddressing::Lid, peer_devices);
    bencher.bench(|| harness.warm_send());
}

/// An unmigrated account's repeat DM to a PN whose LID it knows: the wire
/// stays PN, every device's Signal address upgrades to LID.
#[divan::bench(args = PEER_DEVICES, sample_count = SAMPLE_COUNT, sample_size = SAMPLE_SIZE)]
fn warm_dm_send_pn_with_known_lid(bencher: divan::Bencher, peer_devices: usize) {
    let harness = shared(
        "warm_dm_send_pn_with_known_lid",
        DmAddressing::PnWithKnownLid,
        peer_devices,
    );
    bencher.bench(|| harness.warm_send());
}
