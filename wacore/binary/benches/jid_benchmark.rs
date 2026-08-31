//! Jid parse/format hot paths: every stanza attribute that carries an
//! address goes through these, several times per message.

use divan::black_box;
use wacore_binary::jid::Jid;

fn main() {
    divan::main();
}

const PN: &str = "5511999990000@s.whatsapp.net";
const LID: &str = "123456789012345@lid";
const AD_DEVICE: &str = "5511999990000.0:7@s.whatsapp.net";
const GROUP: &str = "120363012345678901@g.us";

#[divan::bench(args = [PN, LID, AD_DEVICE, GROUP])]
fn bench_jid_parse(input: &str) -> Jid {
    black_box(black_box(input).parse().unwrap())
}

#[divan::bench]
fn bench_jid_to_string(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let mut jid = Jid::lid("123456789012345");
            jid.device = 7;
            jid
        })
        .bench_refs(|jid| black_box(jid.to_string()));
}

#[divan::bench]
fn bench_jid_to_non_ad_string(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let mut jid = Jid::pn("5511999990000");
            jid.device = 3;
            jid
        })
        .bench_refs(|jid| black_box(jid.to_non_ad_string()));
}

/// The per-recipient fan-out formatter: writes the AD form into a reused
/// buffer instead of allocating a String per device.
#[divan::bench]
fn bench_jid_push_phash_form(bencher: divan::Bencher) {
    bencher
        .with_inputs(|| {
            let mut jid = Jid::lid("123456789012345");
            jid.device = 7;
            (jid, String::with_capacity(64))
        })
        .bench_refs(|(jid, buf)| {
            jid.push_phash_form_to(buf);
            // black-box the contents, not just the length: observing only
            // `len` lets LLVM elide the actual formatting writes.
            black_box(buf.as_bytes());
        });
}

// ---------------------------------------------------------------------------
// Identity: `PartialEq`, `Hash`, and the maps keyed by them.
//
// Every incoming message looks a `Jid` up in at least one hash map (the chat
// lane, the session lock, the device memo), and every fan-out compares JIDs
// pairwise while deduplicating. Neither operation had a bench, so a change to
// `identity_agent` -- which both go through -- had no way to be costed: the
// send/receive benches bury it under crypto and marshalling, and the binary
// size gate only moves on dependency deltas.
//
// Two things about the shape of these, both forced by how small the operation
// is relative to the instruments measuring it:
//
// - **Every fixture is built once and leaked.** `with_inputs` runs per
//   iteration and, while divan excludes it from the wall clock, an
//   instruction-count profile still counts it -- and building a `Jid` costs
//   more than comparing two, which would leave the profile measuring setup.
// - **Each iteration works a whole batch**, reported through `ItemsCount`. A
//   single comparison is a few nanoseconds, well under this repository's
//   documented wall-clock noise on cloud hardware, and under callgrind divan's
//   own per-iteration bookkeeping costs ~335k instructions. Batching amortises
//   both to nothing, so `ns/item` and `Ir/item` are both readable.
// ---------------------------------------------------------------------------

use divan::counter::ItemsCount;
use std::collections::HashMap;
use std::hash::{BuildHasher, RandomState};
use std::sync::OnceLock;
use wacore_binary::jid::Server;

/// Items per measured iteration. Large enough that the per-iteration overhead
/// of either instrument is below the per-item cost being read.
const BATCH: usize = 4096;

/// Fan-out sizes worth distinguishing: a DM's own devices, a small group, and
/// a large one. Below the first the map cost is noise; above the last the
/// server splits the fan-out anyway.
const FANOUT_SIZES: [usize; 3] = [8, 64, 512];

fn pn_device(user: &str, device: u16) -> Jid {
    let mut jid = Jid::pn(user);
    jid.device = device;
    jid
}

fn interop_agent(agent: u8) -> Jid {
    Jid {
        user: "5511999990000".into(),
        server: Server::Interop,
        agent,
        device: 0,
        integrator: 0,
    }
}

/// The five comparisons `==` resolves differently: an equal pair (the full field
/// walk), a mismatch caught on the first field, a mismatch caught only on
/// `server`, and the two shapes that miss the raw equal-agents shortcut and go
/// through `identity_agent`. Those two are separate because the normalisation
/// does different work in each: on `@interop` it renders the agent, so it
/// confirms a real difference, while on the phone namespace it suppresses it, so
/// it is what makes two JIDs equal that the raw compare would have split. The
/// second is the case the function exists for.
const EQ_CASES: [&str; 5] = [
    "equal",
    "user_differs",
    "server_differs",
    "agent_nonzero",
    "agent_normalised",
];

fn eq_pair(case: &str) -> &'static (Jid, Jid) {
    static PAIRS: OnceLock<HashMap<&'static str, (Jid, Jid)>> = OnceLock::new();
    &PAIRS.get_or_init(|| {
        HashMap::from([
            (
                "equal",
                (pn_device("5511999990000", 7), pn_device("5511999990000", 7)),
            ),
            (
                "user_differs",
                (pn_device("5511999990000", 7), pn_device("5511999990001", 7)),
            ),
            ("server_differs", {
                let mut right = Jid::lid("5511999990000");
                right.device = 7;
                (pn_device("5511999990000", 7), right)
            }),
            ("agent_nonzero", (interop_agent(3), interop_agent(4))),
            // Different raw agents, equal identity: the phone namespace does
            // not render the agent, so both normalise to 0 and the pair is one
            // device.
            (
                "agent_normalised",
                (
                    pn_device("5511999990000", 7),
                    Jid {
                        agent: 1,
                        ..pn_device("5511999990000", 7)
                    },
                ),
            ),
        ])
    })[case]
}

#[divan::bench(args = EQ_CASES)]
fn bench_jid_eq(bencher: divan::Bencher, case: &str) {
    let (left, right) = eq_pair(case);
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut equal = 0usize;
        for _ in 0..BATCH {
            equal += (black_box(left) == black_box(right)) as usize;
        }
        black_box(equal)
    });
}

#[divan::bench]
fn bench_jid_hash(bencher: divan::Bencher) {
    static STATE: OnceLock<(RandomState, Jid)> = OnceLock::new();
    let (state, jid) = STATE.get_or_init(|| (RandomState::new(), pn_device("5511999990000", 7)));
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut acc = 0u64;
        for _ in 0..BATCH {
            acc ^= state.hash_one(black_box(jid));
        }
        black_box(acc)
    });
}

fn fanout(size: usize) -> &'static [Jid] {
    static SETS: OnceLock<Vec<Vec<Jid>>> = OnceLock::new();
    let sets = SETS.get_or_init(|| {
        FANOUT_SIZES
            .iter()
            .map(|&n| {
                (0..n)
                    .map(|i| pn_device(&format!("5511999{i:06}"), (i % 8) as u16))
                    .collect()
            })
            .collect()
    });
    &sets[FANOUT_SIZES
        .iter()
        .position(|&n| n == size)
        .expect("known fan-out size")]
}

/// Building the map from scratch: the cost a cold fan-out pays once per send.
/// One item is one inserted device, so the arms are comparable across sizes.
#[divan::bench(args = FANOUT_SIZES)]
fn bench_jid_hashmap_insert(bencher: divan::Bencher, size: usize) {
    let jids = fanout(size);
    bencher.counter(ItemsCount::new(size)).bench(|| {
        let mut map = HashMap::with_capacity(jids.len());
        for jid in jids {
            map.insert(jid.clone(), ());
        }
        black_box(map.len())
    });
}

/// A warm map plus a full set of probes for each outcome.
struct Probed {
    map: HashMap<Jid, ()>,
    hits: Vec<Jid>,
    misses: Vec<Jid>,
}

/// The per-message shape: one lookup into a warm map. Hit and miss are separate
/// because a miss stops at the hash and a hit pays the `==` on top of it.
///
/// Each batch cycles through every key rather than repeating one. `HashMap`'s
/// default hasher is seeded per process, so a single probe's bucket and
/// collision chain are drawn fresh on every run: repeating it would let the
/// same code measure differently between a baseline and a PR, and batching
/// would only amplify whichever path that one draw happened to pick. Averaging
/// over the whole key set makes the layout wash out instead.
#[divan::bench(args = FANOUT_SIZES, consts = [true, false])]
fn bench_jid_hashmap_get<const HIT: bool>(bencher: divan::Bencher, size: usize) {
    static MAPS: OnceLock<Vec<Probed>> = OnceLock::new();
    let built = MAPS.get_or_init(|| {
        FANOUT_SIZES
            .iter()
            .map(|&n| Probed {
                map: fanout(n).iter().map(|j| (j.clone(), ())).collect(),
                hits: fanout(n).to_vec(),
                // Same shape and width as the hits, so a miss differs only in
                // being absent.
                misses: (0..n)
                    .map(|i| pn_device(&format!("5511000{i:06}"), (i % 8) as u16))
                    .collect(),
            })
            .collect()
    });
    let probed = &built[FANOUT_SIZES
        .iter()
        .position(|&n| n == size)
        .expect("known fan-out size")];
    let map = &probed.map;
    let probes: &[Jid] = if HIT { &probed.hits } else { &probed.misses };
    bencher.counter(ItemsCount::new(BATCH)).bench(|| {
        let mut found = 0usize;
        for i in 0..BATCH {
            found += map.get(black_box(&probes[i % probes.len()])).is_some() as usize;
        }
        black_box(found)
    });
}
