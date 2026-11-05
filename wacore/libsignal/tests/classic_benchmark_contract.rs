// SPDX-License-Identifier: MIT
#[path = "../benches/classic_fixture/mod.rs"]
mod classic_fixture;
#[test]
#[allow(clippy::print_stderr)]
fn public_port_benchmark_scenarios_complete_successfully() {
    for &case in classic_fixture::CASES {
        let mut fixture = classic_fixture::Fixture::new(case);
        if let Some((length, digest)) = fixture.encoded_input_digest() {
            eprintln!("synthetic-input {case} {length} {digest}");
        }
        if let Some(digest) = fixture.encoded_group_body_digest(case) {
            eprintln!("synthetic-group-body {case} {digest}");
        }
        fixture.exercise(case);
        #[cfg(feature = "bench-internals")]
        fixture.check_cached_key_postconditions(case);
    }
}
