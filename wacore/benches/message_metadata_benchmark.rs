//! Allocation and CPU cost of the content-only metadata rule (no encryption/I/O).

use wacore::send::message_meta_from_message;
use waproto::whatsapp as wa;

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

#[divan::bench(args = [false, true])]
fn image_metadata(bencher: divan::Bencher, view_once: bool) {
    let message = wa::Message {
        image_message: Some(wa::message::ImageMessage {
            view_once: Some(view_once),
            ..Default::default()
        })
        .into(),
        ..Default::default()
    };
    bencher.bench(|| message_meta_from_message(std::hint::black_box(&message)));
}

// An allocating control makes a no-allocation row observable rather than an
// assumption that the profiler happened to be disabled.
#[divan::bench]
fn allocation_control() -> Box<[u8]> {
    vec![0; 4096].into_boxed_slice()
}

#[divan::bench]
fn ordinary_text_metadata(bencher: divan::Bencher) {
    let message = wa::Message {
        conversation: Some("synthetic".into()),
        ..Default::default()
    };
    bencher.bench(|| message_meta_from_message(std::hint::black_box(&message)));
}
