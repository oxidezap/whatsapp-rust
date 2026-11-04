// Isolate container construction/drop and node stride from token decoding.
// A separate crate root preserves the existing decoder benchmark code layout.
// Each batch retains 128 node-shaped records. Argument 9 mixes 0..=8
// attributes; all other arguments are uniform counts.

use divan::black_box;
use smallvec::SmallVec;
use wacore_binary::node::{AttrsRef, NodeContentRef, NodeRef, NodeStr, ValueRef};

const NODES: usize = 128;

struct StoredNode<A> {
    _tag: NodeStr<'static>,
    _attrs: A,
    _content: Option<NodeContentRef<'static>>,
}

fn entries(count: usize) -> impl ExactSizeIterator<Item = (NodeStr<'static>, ValueRef<'static>)> {
    const KEYS: [&str; 8] = ["id", "type", "t", "count", "offline", "v", "edit", "phash"];
    (0..count).map(|i| {
        (
            NodeStr::Borrowed(KEYS[i]),
            ValueRef::String(NodeStr::Owned("123456789012345".into())),
        )
    })
}

fn measure<A>(bencher: divan::Bencher, count: usize, build: impl Fn(usize) -> A) {
    bencher.bench_local(|| {
        let nodes: Vec<_> = (0..NODES)
            .map(|i| {
                let count = if count == 9 { i % 9 } else { count };
                StoredNode {
                    _tag: NodeStr::Borrowed("message"),
                    _attrs: build(black_box(count)),
                    _content: None,
                }
            })
            .collect();
        black_box(&nodes);
    });
}

#[divan::bench(args = [0, 1, 2, 3, 4, 8, 9])]
fn boxed(bencher: divan::Bencher, count: usize) {
    assert_eq!(
        size_of::<StoredNode<AttrsRef<'static>>>(),
        size_of::<NodeRef<'static>>(),
    );
    measure(bencher, count, |n| {
        entries(n).collect::<AttrsRef<'static>>()
    });
}

#[divan::bench(args = [0, 1, 2, 3, 4, 8, 9])]
fn inline_one(bencher: divan::Bencher, count: usize) {
    measure(bencher, count, |n| {
        entries(n).collect::<SmallVec<[(NodeStr<'static>, ValueRef<'static>); 1]>>()
    });
}

#[divan::bench(args = [0, 1, 2, 3, 4, 8, 9])]
fn inline_three(bencher: divan::Bencher, count: usize) {
    measure(bencher, count, |n| {
        entries(n).collect::<SmallVec<[(NodeStr<'static>, ValueRef<'static>); 3]>>()
    });
}

// The decoder knows the count before reading pairs. Match its preallocated
// push loop as well as FromIterator: SmallVec's collect uses reserve, which
// rounds spilled capacity up, whereas with_capacity reserves the exact count.
#[divan::bench(args = [0, 1, 2, 3, 4, 8, 9])]
fn boxed_known_count(bencher: divan::Bencher, count: usize) {
    measure(bencher, count, |n| {
        let mut attrs = Vec::with_capacity(n);
        for entry in entries(n) {
            attrs.push(entry);
        }
        AttrsRef::from_vec(attrs)
    });
}

#[divan::bench(args = [0, 1, 2, 3, 4, 8, 9])]
fn inline_one_known_count(bencher: divan::Bencher, count: usize) {
    measure(bencher, count, |n| {
        let mut attrs = SmallVec::<[(NodeStr<'static>, ValueRef<'static>); 1]>::with_capacity(n);
        for entry in entries(n) {
            attrs.push(entry);
        }
        attrs
    });
}

#[divan::bench(args = [0, 1, 2, 3, 4, 8, 9])]
fn inline_three_known_count(bencher: divan::Bencher, count: usize) {
    measure(bencher, count, |n| {
        let mut attrs = SmallVec::<[(NodeStr<'static>, ValueRef<'static>); 3]>::with_capacity(n);
        for entry in entries(n) {
            attrs.push(entry);
        }
        attrs
    });
}

// One attribute pool per decoded tree: nodes keep a (start, len) range into a
// shared Vec instead of owning a per-node allocation. The pool grows
// geometrically because the decoder does not know the total count up front.
// This removes per-node malloc/free and shrinks the record (8 B range vs 16 B
// boxed slice), at the cost of amortized pool reallocation.
#[divan::bench(args = [0, 1, 2, 3, 4, 8, 9])]
fn pooled(bencher: divan::Bencher, count: usize) {
    measure_pooled(bencher, count, 0);
}

// Upper bound for a chunked arena (e.g. bumpalo) that never copies on growth:
// the pool is sized once for the whole batch.
#[divan::bench(args = [0, 1, 2, 3, 4, 8, 9])]
fn pooled_presized(bencher: divan::Bencher, count: usize) {
    let total = (0..NODES)
        .map(|i| if count == 9 { i % 9 } else { count })
        .sum();
    measure_pooled(bencher, count, total);
}

fn measure_pooled(bencher: divan::Bencher, count: usize, capacity: usize) {
    bencher.bench_local(|| {
        let mut pool: Vec<(NodeStr<'static>, ValueRef<'static>)> =
            Vec::with_capacity(black_box(capacity));
        let nodes: Vec<_> = (0..NODES)
            .map(|i| {
                let count = black_box(if count == 9 { i % 9 } else { count });
                let start = pool.len() as u32;
                pool.extend(entries(count));
                StoredNode {
                    _tag: NodeStr::Borrowed("message"),
                    _attrs: (start, count as u32),
                    _content: None,
                }
            })
            .collect();
        black_box((&nodes, &pool));
    });
}

fn main() {
    divan::main();
}
