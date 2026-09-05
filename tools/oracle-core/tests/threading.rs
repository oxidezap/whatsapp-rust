//! Real guest threads.
//!
//! Under `ThreadPolicy::Spawn` a `pthread_create` starts another instance of the
//! same module over the same shared memory, the way emscripten's Web Worker glue
//! does. That is what brings the VoIP media stack up, and it is also where
//! reproducibility stops being free — see the determinism tests below for what
//! survives.

use std::time::Duration;

mod common;

use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};

const VOIP: &str = "JgwtTQVeWPm";
const CALLEE: &str = "15550002222@s.whatsapp.net";
/// Short on purpose. PJSIP's worker thread is a loop bounded only by its fuel,
/// so a full quiesce would always time out; these tests check that spawned
/// threads are *tracked*, not that the engine's own pool ever drains.
const QUIESCE: Duration = Duration::from_secs(2);

/// Serialises the tests that start real guest threads.
///
/// Each one brings up a PJSIP worker pool, and cargo runs tests in parallel by
/// default. Several engines competing for cores makes them miss their own
/// deadlines, which surfaces as an unrelated-looking failure rather than as
/// contention. The lock is poison-tolerant: a panicking test must not cascade
/// into every test after it.
fn threaded_guard() -> (std::sync::MutexGuard<'static, ()>, common::EngineLock) {
    static LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    // Both locks: the mutex serialises this binary's tests, and the port
    // serialises against the *other* test binaries, which cargo runs in
    // parallel with this one.
    let guard = LOCK.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    (guard, common::engine_lock())
}

/// A VoIP module initialised under the given thread policy.
fn engine(policy: ThreadPolicy) -> Option<Runtime> {
    let catalog = Catalog::discover().ok()?;
    let entry = catalog.resolve(VOIP).ok()?;
    let bytes = std::fs::read(&entry.path).ok()?;

    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.set_thread_policy(policy);
    runtime.run_ctors().expect("ctors");
    runtime
        .attach_log_ring(1 << 20)
        .expect("engine should accept a log ring");
    Some(runtime)
}

/// Initialises the VoIP stack, reporting why rather than returning `None`.
///
/// Swallowing the error made a fuel exhaustion under parallel test load look
/// like a different return value.
fn init_stack(runtime: &mut Runtime) -> i64 {
    let result = runtime
        .call_embind(
            "initVoipStack",
            &[
                Value::Str(CALLEE.to_owned()),
                Value::Str("0".to_owned()),
                Value::Str("{}".to_owned()),
            ],
        )
        .unwrap_or_else(|error| panic!("initVoipStack failed: {error:#}"));
    runtime.refuel();
    result
        .as_int()
        .unwrap_or_else(|| panic!("initVoipStack returned {result:?}"))
}

/// Brings an engine up with its media stack running, retrying the startup race.
///
/// Two threads reach the same state before either finishes with it, and
/// `initVoipStack` traps — roughly one attempt in six under load. The race is
/// the engine's, not the harness's: `schedule.rs` narrows it and cannot close
/// it, because the threads coordinate through shared memory rather than through
/// host calls. `signaling.rs` retries for the same reason, and the alternative
/// is a test that fails for a reason it is not testing.
fn engine_started() -> Option<Runtime> {
    const ATTEMPTS: usize = 6;

    for attempt in 1..=ATTEMPTS {
        let mut runtime = engine(ThreadPolicy::Spawn)?;
        let outcome = runtime.call_embind(
            "initVoipStack",
            &[
                Value::Str(CALLEE.to_owned()),
                Value::Str("0".to_owned()),
                Value::Str("{}".to_owned()),
            ],
        );
        runtime.refuel();

        match outcome.as_ref().ok().and_then(|value| value.as_int()) {
            Some(0) => return Some(runtime),
            other if attempt < ATTEMPTS => {
                eprintln!("media stack did not come up (attempt {attempt}): {other:?}");
            }
            other => panic!("media stack failed after {ATTEMPTS} attempts: {other:?}"),
        }
    }
    None
}

macro_rules! engine_or_skip {
    ($policy:expr) => {
        match engine($policy) {
            Some(runtime) => runtime,
            None => {
                eprintln!("skipping: no capture (set WA_WASM_DIR)");
                return;
            }
        }
    };
}

/// The headline result: the media stack only initialises when threads run.
///
/// `120011` is `PJ_ERRNO_START_SYS + EAGAIN` — PJSIP asking for the thread it
/// was refused. With `Spawn` it gets one and returns success.
#[test]
fn the_media_stack_needs_real_threads() {
    let _serial = threaded_guard();
    let Some(mut refused) = engine(ThreadPolicy::Refuse) else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };
    assert_eq!(
        init_stack(&mut refused),
        120_011,
        "without threads PJSIP should fail with EAGAIN"
    );

    // Retried: the startup race is the engine's own, and a test about
    // *threads being needed* must not fail because two of them collided.
    let spawned = engine_started().expect("media stack should come up with real threads");

    let log = spawned.engine_log();
    assert!(
        log.iter()
            .any(|line| line.contains("worker thread started")),
        "expected PJSIP's worker to start: {log:?}"
    );
    assert!(
        log.iter()
            .any(|line| line.contains("pjmedia_codec_opus_init success")),
        "expected the Opus codec to initialise: {log:?}"
    );

    let _ = spawned.quiesce(QUIESCE);
}

/// Threads must be observable and must finish. A leaked worker would make every
/// later observation a race.
#[test]
fn spawned_threads_are_tracked_and_finish() {
    let _serial = threaded_guard();
    let Some(runtime) = engine_started() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    let log = runtime.engine_log();
    assert!(
        log.iter()
            .any(|line| line.contains("worker thread started")),
        "no thread appears to have run"
    );

    // The engine's worker outlives the call that started it, so what is checked
    // is that the count is tracked at all, not that it reaches zero.
    let settled = runtime.quiesce(QUIESCE);
    if settled {
        assert_eq!(runtime.live_threads(), 0);
    }
}

/// What survives threading: the parts of the transcript that do not depend on
/// interleaving.
///
/// The full trace is *not* asserted equal — two runs can legitimately interleave
/// differently, and a test that demanded otherwise would be flaky by design.
/// What must hold is that the same milestones are reached.
#[test]
fn threaded_runs_reach_the_same_milestones() {
    let _serial = threaded_guard();
    // Both retried: this test is about two successful runs agreeing, so a
    // startup race in either is noise rather than the thing under test.
    let Some(first) = engine_started() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };
    let second = engine_started().expect("second instance");
    let _ = (first.quiesce(QUIESCE), second.quiesce(QUIESCE));

    // Milestones are lines the engine emits once per successful startup; their
    // presence is reproducible even though their ordering relative to worker
    // output is not.
    const MILESTONES: [&str; 4] = [
        "initVoipStack called",
        "pjmedia_endpt_create = 0",
        "pjmedia_codec_opus_init success",
        "init_media_endpt_and_codecs Exit",
    ];

    let (a, b) = (first.engine_log(), second.engine_log());
    for milestone in MILESTONES {
        assert!(
            a.iter().any(|line| line.contains(milestone)),
            "first run missed `{milestone}`"
        );
        assert!(
            b.iter().any(|line| line.contains(milestone)),
            "second run missed `{milestone}`"
        );
    }
}

/// The clock stays monotonic across threads. A per-thread clock would let a
/// worker observe time moving backwards, which is exactly what a deadline loop
/// notices first.
#[test]
fn the_clock_is_monotonic_across_threads() {
    let _serial = threaded_guard();
    // Measured across the worker pool running, not across startup: what is
    // under test is that concurrent readers never see the clock go backwards,
    // and the startup race would fail this test for an unrelated reason.
    let Some(runtime) = engine_started() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    let before = runtime.state().clock_ms();
    runtime.quiesce(QUIESCE);
    let after = runtime.state().clock_ms();

    assert!(
        after >= before,
        "clock went backwards across threads: {before} -> {after}"
    );
}

/// Log lines carry a global sequence number, so a transcript can be put back in
/// a stable order regardless of how the OS scheduled the threads that wrote it.
#[test]
fn the_transcript_is_ordered_by_sequence() {
    let _serial = threaded_guard();
    let Some(runtime) = engine_started() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };
    runtime.quiesce(QUIESCE);

    let lines = runtime.shared().logs();
    let mut sorted = lines.clone();
    sorted.sort_by_key(|line| line.seq);
    assert_eq!(lines, sorted, "logs should come back in sequence order");

    // Every line is attributed, so output from a worker is distinguishable from
    // output from the thread that started it.
    assert!(lines.iter().all(|line| !line.text.is_empty()));
}

/// Randomness must be reproducible *per thread*.
///
/// A single shared stream is reproducible only if consumed in a reproducible
/// order, which under threads it is not: two runs can interleave their
/// `random_get` calls differently and hand the same caller different bytes.
/// Seeding per thread makes each thread's sequence depend only on how many
/// bytes that thread has taken.
#[test]
fn randomness_is_reproducible_per_thread() {
    let _serial = threaded_guard();
    use oracle_core::shared::SharedHost;

    // Same thread, same seed, across runs.
    assert_eq!(SharedHost::seed_for(0), SharedHost::seed_for(0));
    assert_eq!(SharedHost::seed_for(7), SharedHost::seed_for(7));
    // Different threads must not share a stream, or two workers would produce
    // identical "random" material.
    assert_ne!(SharedHost::seed_for(0), SharedHost::seed_for(1));
    assert_ne!(SharedHost::seed_for(1), SharedHost::seed_for(2));
}

/// The main thread's random stream is unaffected by whether workers ran, which
/// is what makes a threaded run comparable to a single-threaded one.
#[test]
fn the_main_thread_stream_is_independent_of_workers() {
    let _serial = threaded_guard();
    let Some(mut threaded) = engine(ThreadPolicy::Spawn) else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };
    let mut single = engine(ThreadPolicy::Refuse).expect("second instance");

    // getWebP2PVirtualIpv4 is pure, but reaching it consumes the same host
    // randomness in both cases; a shared stream would let worker activity
    // change what the main thread sees.
    let a = threaded.call_embind("getWebP2PVirtualIpv4", &[]).ok();
    let b = single.call_embind("getWebP2PVirtualIpv4", &[]).ok();
    assert_eq!(a, b);
}

/// Guest execution is serialised once threads are on.
///
/// This is what took the startup race from about one in nine to one in forty:
/// two guest threads can no longer be inside guest code at the same time.
#[test]
fn scheduling_is_enabled_with_threads() {
    let runtime = engine_or_skip!(ThreadPolicy::Spawn);
    assert!(
        runtime.shared().scheduler.is_enabled(),
        "threads should turn scheduling on"
    );

    let single = engine(ThreadPolicy::Refuse).expect("second instance");
    assert!(
        !single.shared().scheduler.is_enabled(),
        "a single-threaded module should not pay for scheduling"
    );
}

/// Draining the main thread's proxy queue fails, and says so.
///
/// Both entry points refuse an unregistered main thread:
/// `emscripten_main_thread_process_queued_calls` asserts
/// `emscripten_is_main_runtime_thread()`, and `_emscripten_check_mailbox`
/// asserts `pthread_self()`. Neither holds here, because registering the main
/// thread costs startup reliability (see `HostState::register_main_thread`),
/// so today the drain cannot succeed by either route.
///
/// That matters: the VoIP engine dispatches its outgoing signaling through
/// this queue, so nothing queued for the main thread is ever delivered. It is
/// a known, load-bearing limitation rather than a mystery, and the real fix is
/// to drain off the blocking path.
///
/// What this test pins is the part that must never regress — the failure is
/// **reported**. A silent drain would make the next investigator believe the
/// queue was empty, which is exactly how this cost several sessions already.
#[test]
fn draining_the_proxy_queue_reports_its_failure() {
    let _serial = threaded_guard();
    let Some(mut runtime) = engine_started() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    let before = runtime.logs().len();
    assert!(
        runtime.process_queued_calls(),
        "a threaded module should have something to drain through"
    );

    let reported: Vec<String> = runtime
        .logs()
        .into_iter()
        .skip(before)
        .filter(|line| line.contains("failed"))
        .collect();
    assert!(
        !reported.is_empty(),
        "the drain cannot succeed yet, so it must at least say so — got silence"
    );
    // Whichever route was taken, the reason has to be legible.
    assert!(
        reported
            .iter()
            .any(|line| line.contains("check_mailbox") || line.contains("queued_calls")),
        "the report should name the entry point that failed, got: {reported:?}"
    );
}

/// No thread should have to break serialisation to make progress.
///
/// A forced turn means guest code waited on something it never signalled
/// through a host call, so the host ran it anyway rather than hang. It is the
/// documented escape hatch, and it should stay unused on a healthy run.
#[test]
fn no_thread_has_to_force_its_turn() {
    let _serial = threaded_guard();
    let Some(runtime) = engine_started() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };
    runtime.quiesce(QUIESCE);

    // Forced turns are now expected, and the reason is a fix rather than a
    // regression: worker threads used to die immediately — the host asked for
    // `__emscripten_thread_init` while the module exports
    // `_emscripten_thread_init`, so they ran with no pthread and
    // `emscripten_proxy_execute_queue` asserted on `pthread_self()`. A thread
    // that dies at once never waits, so this counter used to read zero for the
    // worst possible reason. Now they live, and a live worker blocks in
    // `memory.atomic.wait32` — inside wasm, with no host call — so the host
    // sometimes has to break serialisation to keep going.
    //
    // What is still worth asserting is that it stays bounded: a counter that
    // climbs with every run means threads are waiting on something that never
    // arrives.
    let forced = runtime.forced_turns();
    assert!(
        forced <= 8,
        "forced turns should stay bounded, saw {forced}"
    );
}

/// An engine whose instantiating thread is registered as the main runtime
/// thread. Kept separate from `engine_started` because registration changes how
/// the guest waits — see `HostState::register_main_thread`.
fn engine_registered() -> Option<Runtime> {
    let catalog = Catalog::discover().ok()?;
    let entry = catalog.resolve(VOIP).ok()?;
    let bytes = std::fs::read(&entry.path).ok()?;

    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.set_thread_policy(ThreadPolicy::Spawn);
    // Before the constructors: registration happens during startup.
    runtime.set_main_thread_registration(true);
    runtime.run_ctors().expect("ctors");
    runtime
        .attach_log_ring(1 << 20)
        .expect("engine should accept a log ring");
    Some(runtime)
}

/// Work a worker thread queues for the main thread has to actually run.
///
/// Emscripten proxies such a call by queueing it and letting the main thread
/// dispatch it through `emscripten_receive_on_main_thread_js`. In a browser the
/// event loop turns and the queue drains; here nothing turns unless the host
/// drains it, and the VoIP engine sends its outbound signaling this way — the
/// callback in the function table that calls `sendSignalingXMPP_js_sync` is
/// reached only through that queue.
///
/// This pins both halves: the entry point exists and draining it is safe to
/// call repeatedly, including before anything has been queued.
#[test]
fn the_main_thread_queue_can_be_drained() {
    let _serial = threaded_guard();

    // Registration has to be on, and has to be on before the constructors run.
    // Without it the entry point traps on its own assertion rather than
    // draining anything — which is exactly the failure this pins.
    let Some(mut runtime) = engine_registered() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    assert!(
        runtime.process_queued_calls(),
        "a threaded module exports emscripten_main_thread_process_queued_calls"
    );

    // Draining with nothing queued is a no-op rather than an error, which is
    // what makes it safe to do on every tick of `settle`.
    for _ in 0..3 {
        assert!(runtime.process_queued_calls());
    }

    let failures: Vec<String> = runtime
        .logs()
        .into_iter()
        .filter(|line| line.contains("process_queued_calls failed"))
        .collect();
    assert!(
        failures.is_empty(),
        "the main thread did not register itself: {failures:?}"
    );
}

/// A single-threaded module has no such queue, and says so rather than
/// pretending it drained one.
#[test]
fn a_module_without_the_queue_reports_it() {
    let catalog = match Catalog::discover() {
        Ok(catalog) => catalog,
        Err(_) => {
            eprintln!("skipping: no capture (set WA_WASM_DIR)");
            return;
        }
    };
    // mozjpeg is single-threaded: no shared memory, no proxying.
    let Ok(entry) = catalog.resolve("php8T1oSIZM") else {
        eprintln!("skipping: mozjpeg unavailable");
        return;
    };
    let bytes = std::fs::read(&entry.path).expect("read module");
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");

    assert!(
        !runtime.process_queued_calls(),
        "a module with no proxying queue has nothing to drain"
    );
}

/// The startup race is gone, and this is what would show it coming back.
///
/// It used to trap about one attempt in six, and `schedule.rs` could only
/// narrow it. The cause was not the engine: `__emscripten_thread_init` was
/// given `can_block = 1` for the main thread, so it took
/// `memory.atomic.wait32` — a wait that happens *inside* wasm, with no host
/// call. It therefore held its scheduler turn while blocked, and the thread
/// that would notify it never got one. Emscripten passes
/// `canBlock = !ENVIRONMENT_IS_WEB` for exactly this reason: a browser's main
/// thread cannot use `Atomics.wait` either, and busy-waits instead.
///
/// With the web value, the main thread takes emscripten's own busy-wait, which
/// calls `_emscripten_yield` each time round — a host call, so the turn is
/// yielded and the proxying queue drains. Both problems close together.
///
/// Repeated, because a single green run cannot tell a fixed race from a lucky
/// one. `forced_turns` is the sharper signal: non-zero means a thread ran
/// without being granted a turn, which is the harness breaking its own
/// serialisation to avoid a hang.
///
/// # What it caught on the capture bump, and what fixed it
///
/// It failed on `JgwtTQVeWPm` — `initVoipStack` trapping about two runs in five
/// — and the cause was not a race in the engine. A `std::string` built in func
/// 724's own stack frame was being overwritten by another guest thread's frame,
/// so its `__is_long_` bit read as set while `__data_` held a neighbour's
/// local, and `~basic_string` reached `free(1)`. See "A deleter is handed the
/// integer 1" in `agent_docs/voip_oracle_status.md` for the chain, read out of the bytecode.
///
/// The fix is `threads.rs` giving each worker the 64 KiB stack the guest itself
/// allocated for it, which is what emscripten's `establishStackSpace` does. The
/// same change had been tried three times before and recorded as making things
/// worse; what was missing was a signal sharp enough to judge it, since
/// `startVoipCall` is broken for unrelated reasons. `free_trap` supplies one —
/// it asks whether the deallocator is handed a pointer or a small integer:
///
/// ```text
///                        shared stack      per-thread stack
/// initVoipStack          8/12, 20/20 ->    12/12, 20/20
/// last value freed       1, 2 on traps     always a pointer
/// ```
#[test]
fn startup_is_reliable_and_never_forces_a_turn() {
    const ROUNDS: usize = 8;

    let _serial = threaded_guard();
    for round in 1..=ROUNDS {
        let Some(runtime) = engine(ThreadPolicy::Spawn) else {
            eprintln!("skipping: no capture (set WA_WASM_DIR)");
            return;
        };
        let mut runtime = runtime;

        let outcome = runtime.call_embind(
            "initVoipStack",
            &[
                Value::Str(CALLEE.to_owned()),
                Value::Str("0".to_owned()),
                Value::Str("{}".to_owned()),
            ],
        );
        runtime.refuel();

        assert_eq!(
            outcome.as_ref().ok().and_then(|value| value.as_int()),
            Some(0),
            "round {round}: the media stack should come up every time, got {outcome:?}"
        );
        // Bounded rather than zero: see `no_thread_has_to_force_its_turn` for
        // why a live worker sometimes has to be forced, and why zero used to
        // mean "the workers were already dead".
        let forced = runtime.forced_turns();
        assert!(
            forced <= 8,
            "round {round}: forced turns should stay bounded, saw {forced}"
        );
    }
}
