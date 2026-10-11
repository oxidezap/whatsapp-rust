#![allow(dead_code, unused_imports)] // Test oracle helpers are intentionally shared with this fixture bench.
// Fixture-only allocator diagnostics; production benchmark allocation setup is unchanged.
#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();
include!("../tests/consumer.rs");
fn main() { divan::main(); }
