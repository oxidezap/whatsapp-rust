//! End-to-end device-list response baseline, from decrypted packed bytes to
//! owned output. Fixture construction and compression are outside measurement.
//! The core CodSpeed shard records CPU and allocation metrics for these rows.

use divan::black_box;
use std::sync::OnceLock;

#[path = "support/usync_response.rs"]
mod response;
use response::Fixture;

const WIDTHS: [usize; 4] = [1, 32, 128, 512];

fn fixture(width: usize) -> &'static Fixture {
    static FIXTURES: OnceLock<Vec<Fixture>> = OnceLock::new();
    let fixtures = FIXTURES.get_or_init(|| WIDTHS.into_iter().map(Fixture::new).collect());
    &fixtures[WIDTHS
        .iter()
        .position(|&n| n == width)
        .expect("known width")]
}

#[divan::bench(args = WIDTHS)]
fn tree_plain(bencher: divan::Bencher, width: usize) {
    let fixture = fixture(width);
    bencher.bench(|| black_box(fixture.receive(black_box(&fixture.plain))));
}

#[divan::bench(args = WIDTHS)]
fn tree_compressed(bencher: divan::Bencher, width: usize) {
    let fixture = fixture(width);
    bencher.bench(|| black_box(fixture.receive(black_box(&fixture.compressed))));
}

fn main() {
    divan::main();
}
