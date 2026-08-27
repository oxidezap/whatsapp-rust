//! Encoder cost of the group sender-key distribution fan-out, swept across
//! recipient counts.
//!
//! Its own target rather than a section of `binary_benchmark`: the fixture
//! below instantiates the typed-JID attribute path, and adding it to that
//! crate root changed inlining enough to cost `create_attr_node`'s builder an
//! extra `SmallVec` reallocation — ~300 instructions on `bench_attr_parser`,
//! a benchmark with no connection to this one. A separate crate root keeps a
//! new fixture from moving an unrelated baseline.

use divan::black_box;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Jid;
use wacore_binary::marshal::marshal_exact;
use wacore_binary::node::Node;

fn main() {
    divan::main();
}

/// The `<participants>` shape a sender-key distribution puts on the wire: one
/// `<to jid=…>` per recipient device, each wrapping its own `<enc>`. This is
/// the only group stanza whose encode cost is proportional to the recipient
/// count — a steady-state group send distributes to our own companions only
/// and carries nothing per member (pinned by
/// `warm_group_stanza_size_tracks_own_devices_not_group_size` in `wacore`),
/// which is why the width is swept here rather than assumed.
///
/// Recipients are a typed [`Jid`] per device, as `build_participant_node`
/// passes them, and are spread over distinct users with a handful of devices
/// each, the way a real fanout resolves. Both details decide the encoding:
/// a typed JID skips the string classifier production never runs here, and a
/// device id must stay within `u8` to take the `AD_JID` path — a single user
/// numbered up to 511 would silently encode half the sweep as `JID_PAIR` and
/// measure two wire shapes at once.
///
/// The ciphertexts are `type="msg"`, the shape a redistribution to devices that
/// already hold a pairwise session emits — a membership change or a rotation.
/// **This sweep does not characterize a first-contact fan-out.** Those get
/// `type="pkmsg"`, whose `PreKeySignalMessage` carries an identity key, a base
/// key and the registration id on top of the same inner message, and that
/// larger payload is paid once *per recipient* — `marshal_exact` copies every
/// payload through the writer — so it raises the slope, not the intercept. Read
/// this sweep as a lower bound there, or measure a `pkmsg` payload separately;
/// do not extrapolate the cold cost from these numbers.
fn create_skdm_fanout_node(width: usize) -> Node {
    const DEVICES_PER_USER: usize = 4;
    let recipients: Vec<Node> = (0..width)
        .map(|i| {
            let user = 5511999990000u64 + (i / DEVICES_PER_USER) as u64;
            let device = (i % DEVICES_PER_USER) as u16;
            NodeBuilder::new("to")
                .attr("jid", Jid::pn_device(user.to_string(), device))
                .children(vec![
                    NodeBuilder::new("enc")
                        .attr("v", "2")
                        .attr("type", "msg")
                        .bytes(vec![0xAB; 128])
                        .build(),
                ])
                .build()
        })
        .collect();
    NodeBuilder::new("message")
        .attr("to", "120363000000000001@g.us")
        .attr("id", "3EB0A1B2C3D4E5F60718")
        .attr("type", "text")
        .children(vec![
            NodeBuilder::new("participants")
                .children(recipients)
                .build(),
        ])
        .build()
}

// Group sender-key distribution, swept across the recipient count reported for
// real groups. Marshalling is linear in the fan-out width, so this is what a
// redistribution — a membership change, or a rotation — pays in the encoder.
// A first-contact fan-out pays a steeper per-recipient term over the larger
// `pkmsg` payload (see the fixture), so it is not what these numbers measure.
// The steady-state send that follows carries no `<participants>` at all.
// Keeping both facts measurable is what tells a group-size regression ("the
// warm stanza grew a per-participant node") apart from a group that is merely
// redistributing.
//
// `marshal_exact`, not `marshal_auto`: every outbound stanza goes through
// `Client::marshal_node_for_send`, which picks the two-pass exact strategy.
// The two differ in exactly what this sweep is measuring — one-pass reserves
// and grows, two-pass plans the size first and replays a hint tape — so the
// wrong one would track a path no group send takes.
#[divan::bench(args = [8, 32, 128, 512])]
fn bench_marshal_exact_group_fanout(bencher: divan::Bencher, width: usize) {
    bencher
        .with_inputs(|| create_skdm_fanout_node(width))
        .bench_refs(|node| black_box(marshal_exact(black_box(node)).unwrap()));
}
