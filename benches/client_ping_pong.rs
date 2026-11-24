use whatsapp_rust::bench_support::connected_idle::ping_pong::{PingPong, ROUNDS};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    let _ = env_logger::try_init();
    divan::main();
}

#[divan::bench(sample_count = 12, sample_size = 1)]
fn warm_dm_ping_pong(bencher: divan::Bencher) {
    bencher
        .counter(divan::counter::ItemsCount::new(ROUNDS))
        .with_inputs(|| PingPong::new().expect("warm ping/pong fixture"))
        .bench_local_refs(|fixture| fixture.run().expect("complete ping/pong sample"));
}
