//! Full token decoding, storage construction/drop and selected attribute reads.
//! These fixtures are synthetic; they do not represent a traffic distribution.
use divan::black_box;
use wacore_binary::bench_internals as decoder_pool;
pub use wacore_binary::*;

#[divan::bench(args = [0, 1, 32, 128])]
fn production_tree(bencher: divan::Bencher, width: usize) {
    let data = decoder_pool::fixture(width);
    bencher.bench_local(|| {
        black_box(marshal::unmarshal_ref(black_box(&data)).expect("decode"));
    });
}

#[divan::bench(args = [0, 1, 32, 128])]
fn parser_control(bencher: divan::Bencher, width: usize) {
    let data = decoder_pool::fixture(width);
    bencher.bench_local(|| {
        black_box(decoder_pool::baseline(black_box(&data)).expect("decode"));
    });
}

#[divan::bench(args = [0, 1, 32, 128])]
fn chunked(bencher: divan::Bencher, width: usize) {
    let data = decoder_pool::fixture(width);
    bencher.bench_local(|| {
        black_box(decoder_pool::decode(black_box(&data), 128, false).expect("decode"));
    });
}

#[divan::bench(args = [0, 1, 32, 128])]
fn two_pass(bencher: divan::Bencher, width: usize) {
    let data = decoder_pool::fixture(width);
    bencher.bench_local(|| {
        black_box(decoder_pool::decode(black_box(&data), 0, true).expect("decode"));
    });
}

#[divan::bench(args = [0, 1, 32, 128])]
fn tree_attribute_consumer(bencher: divan::Bencher, width: usize) {
    let data = decoder_pool::fixture(width);
    bencher.bench_local(|| {
        let tree = decoder_pool::baseline(black_box(&data)).expect("decode");
        black_box(decoder_pool::consume_tree(&tree).expect("attributes"));
    });
}

#[divan::bench(args = [0, 1, 32, 128])]
fn chunked_attribute_consumer(bencher: divan::Bencher, width: usize) {
    let data = decoder_pool::fixture(width);
    bencher.bench_local(|| {
        let frame = decoder_pool::decode(black_box(&data), 128, false).expect("decode");
        black_box(frame.consume().expect("attributes"));
    });
}

#[divan::bench(args = [0, 1, 32, 128])]
fn two_pass_attribute_consumer(bencher: divan::Bencher, width: usize) {
    let data = decoder_pool::fixture(width);
    bencher.bench_local(|| {
        let frame = decoder_pool::decode(black_box(&data), 0, true).expect("decode");
        black_box(frame.consume().expect("attributes"));
    });
}

fn main() {
    divan::main();
}

#[divan::bench(args = [0, 1, 32, 128])]
fn owned_tree_attribute_consumer(bencher: divan::Bencher, width: usize) {
    let data = bytes::Bytes::from(decoder_pool::fixture(width));
    bencher.bench_local(|| {
        let frame = OwnedNodeRef::new(black_box(data.clone())).expect("decode");
        black_box(decoder_pool::consume_tree(frame.get()).expect("attributes"));
    });
}

#[divan::bench(args = [0, 1, 32, 128])]
fn owned_chunked_attribute_consumer(bencher: divan::Bencher, width: usize) {
    let data = bytes::Bytes::from(decoder_pool::fixture(width));
    bencher.bench_local(|| {
        let frame =
            decoder_pool::OwnedFrame::new(black_box(data.clone()), 128, false).expect("decode");
        black_box(frame.get().consume().expect("attributes"));
    });
}

#[divan::bench(args = [0, 1, 32, 128])]
fn owned_two_pass_attribute_consumer(bencher: divan::Bencher, width: usize) {
    let data = bytes::Bytes::from(decoder_pool::fixture(width));
    bencher.bench_local(|| {
        let frame =
            decoder_pool::OwnedFrame::new(black_box(data.clone()), 0, true).expect("decode");
        black_box(frame.get().consume().expect("attributes"));
    });
}
