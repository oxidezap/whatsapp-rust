use divan::black_box;
use wacore::sync_engine::{
    app_state::{BootstrapState, RetryBudget},
    history::HistoryActivity,
};

#[global_allocator]
static ALLOC: divan::AllocProfiler = divan::AllocProfiler::system();

fn main() {
    divan::main();
}

// These reference operations are the SDK accounting from the PR base. Keep
// them together with the core cases so both use the same compiler and process.
#[derive(Default)]
struct LegacyActivity {
    generation: usize,
    tasks: usize,
    tasks_peak: usize,
    bytes: usize,
    bytes_peak: usize,
}

#[divan::bench(args = [64, 4096])]
fn legacy_history_activity(bencher: divan::Bencher, count: usize) {
    bencher.bench(|| {
        let mut state = LegacyActivity::default();
        for i in 0..count {
            let bytes = black_box(i);
            let generation = state.generation;
            state.tasks = state.tasks.saturating_add(1);
            state.tasks_peak = state.tasks_peak.max(state.tasks);
            state.bytes = state.bytes.saturating_add(bytes);
            state.bytes_peak = state.bytes_peak.max(state.bytes);
            let resized = black_box(bytes + 1);
            if state.generation == generation {
                state.bytes = state.bytes.saturating_sub(bytes).saturating_add(resized);
                state.bytes_peak = state.bytes_peak.max(state.bytes);
            }
            if state.generation == generation {
                state.bytes = state.bytes.saturating_sub(resized);
                state.tasks = state.tasks.saturating_sub(1);
                black_box(state.tasks == 0);
            }
            if black_box(i % 16 == 0) {
                state.generation = state.generation.wrapping_add(1);
                state.tasks = 0;
                state.bytes = 0;
            }
        }
        black_box((state.tasks, state.tasks_peak, state.bytes, state.bytes_peak))
    });
}

#[divan::bench(args = [64, 4096])]
fn core_history_activity(bencher: divan::Bencher, count: usize) {
    bencher.bench(|| {
        let mut state = HistoryActivity::default();
        for i in 0..count {
            let bytes = black_box(i);
            let generation = state.begin(bytes);
            let resized = black_box(bytes + 1);
            state.resize(generation, bytes, resized);
            black_box(state.release(generation, resized));
            if black_box(i % 16 == 0) {
                state.reset();
            }
        }
        let result = state.snapshot();
        black_box((
            result.tasks,
            result.tasks_peak,
            result.payload_bytes,
            result.payload_bytes_peak,
        ))
    });
}

#[divan::bench(args = [false, true])]
fn legacy_retry_budget(bencher: divan::Bencher, reaches_server: bool) {
    bencher.bench(|| {
        let mut attempts = 0u32;
        for _ in 0..32 {
            if attempts >= 8 {
                break;
            }
            black_box(
                std::time::Duration::from_secs(1)
                    .saturating_mul(2u32.saturating_pow(attempts))
                    .min(std::time::Duration::from_secs(3600)),
            );
            if black_box(reaches_server) {
                attempts += 1;
            }
        }
        black_box(attempts)
    });
}

#[divan::bench(args = [false, true])]
fn core_retry_budget(bencher: divan::Bencher, reaches_server: bool) {
    bencher.bench(|| {
        let mut budget = RetryBudget::default();
        while let Some(delay) = budget.next_delay() {
            black_box(delay);
            if black_box(reaches_server) {
                budget.record_attempt();
            }
        }
        black_box(budget.attempts())
    });
}

#[divan::bench]
fn legacy_bootstrap(bencher: divan::Bencher) {
    bencher.bench(|| {
        let mut word = black_box(1u64);
        for generation in 1..64u64 {
            let next = (word >> 1).max(black_box(generation)).saturating_add(1);
            word = (next << 1) | 1;
            let writer = black_box(generation + 1);
            if word >> 1 <= writer {
                word = writer << 1;
            }
        }
        black_box(word)
    });
}

#[divan::bench]
fn core_bootstrap(bencher: divan::Bencher) {
    bencher.bench(|| {
        let mut state = BootstrapState::from_word(black_box(1));
        for generation in 1..64u64 {
            state = state.arm_for_pairing(black_box(generation));
            if let Some(next) = state.settle(black_box(generation + 1), false) {
                state = next;
            }
        }
        black_box(state.word())
    });
}
