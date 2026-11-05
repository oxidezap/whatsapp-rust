// SPDX-License-Identifier: MIT
//! Public-port workloads shared with functional tests and immutable-revision comparisons.
// The standalone integration test calls audit helpers; this target has no test harness.
#[cfg_attr(test, allow(dead_code))]
mod classic_fixture;
use classic_fixture::{CASES, Case, Fixture};
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();
fn main() {
    divan::main();
}
#[divan::bench(args = CASES)]
fn classic(bencher: divan::Bencher, case: &Case) {
    bencher
        .with_inputs(|| Fixture::new(*case))
        .bench_refs(|fixture| fixture.exercise(*case));
}
