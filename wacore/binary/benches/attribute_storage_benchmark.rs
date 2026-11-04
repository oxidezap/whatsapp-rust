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

fn main() {
    divan::main();
}
