//! Where the thread-status profiler flag sits, and whether it has moved.
//!
//! `f12302` — emscripten's `emscripten_conditional_set_current_thread_status`,
//! which every engine worker reaches from the futex wait path — returns
//! immediately when the byte at `0x14B958` is zero. Nothing in this module ever
//! writes it: the only writer, `emscripten_thread_profiler_enable`, has no call
//! sites and no table slots. It should be zero for the whole run, and it was
//! not: `cargo xt oracle neutralize-thread-profiler` exists because forcing the test
//! to fail took a run from eleven traps to none.
//!
//! **Where it sits.** This example prints the two numbers that frame it:
//!
//! ```text
//! main stack: base 0x24cf60 end 0x14cf60 ...
//! ```
//!
//! `0x14B958` is `0x1608` bytes below `end`. Static data begins where the stack
//! region stops and the flag is the first interesting byte under it, so a stack
//! running past its own low bound writes exactly there — and every guest thread
//! starts from `0x24cf60`, so they share that 1 MiB region.
//!
//! That is geometry, not proof, and this run does not reproduce the problem:
//! the flag reads `0x00` at instantiation, after the constructors, after
//! `initVoipStack` and after `startVoipCall`, with the engine log ring attached
//! at level 9 and the soft-assert gate open, and no worker traps. So it is a
//! guard as much as a probe — a non-zero flag here means the corruption is
//! back, and the trap count says whether it mattered.
//!
//! It is also the smallest run that starts a call, which makes it the A/B
//! harness for anything touching `threads.rs`. See "Giving each thread its own
//! stack works, and is still not the fix" in agent_docs/voip_oracle_status.md for the numbers it
//! produced there.
//!
//! ```sh
//! cargo run --release --example profiler_flag [--verbose-engine] [--enable-asserts]
//! ```
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};
use wasmtime::Val;

/// The profiler flag. `0x14B958`.
const PROFILER_FLAG: u32 = 1_358_168;

/// Reads the flag, or says why it could not.
fn flag(runtime: &Runtime) -> String {
    match runtime.read(PROFILER_FLAG, 1) {
        Ok(byte) => format!("{:#04x}", byte.first().copied().unwrap_or_default()),
        Err(error) => format!("unreadable: {error}"),
    }
}

/// Calls a no-argument export returning one i32, for the stack accessors.
fn stack_value(runtime: &mut Runtime, name: &str) -> String {
    match runtime.call(name, &[]) {
        Ok(values) => match values.first() {
            Some(Val::I32(value)) => format!("{value:#x}"),
            other => format!("{other:?}"),
        },
        Err(error) => format!("unavailable: {error}"),
    }
}

fn main() -> anyhow::Result<()> {
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve("JgwtTQVeWPm")?;
    let bytes = std::fs::read(&entry.path)?;

    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.set_thread_policy(ThreadPolicy::Spawn);

    println!("profiler flag at instantiation: {}", flag(&runtime));
    runtime.run_ctors()?;
    println!("profiler flag after ctors:      {}", flag(&runtime));

    // `examples/outgoing_call.rs` does both of these and still sees workers
    // trap, so they are the variables to hold up against a run that does not.
    // Off by default: this example's job is the flag, not the log.
    if std::env::args().any(|arg| arg == "--verbose-engine") {
        runtime.attach_log_ring(4 << 20)?;
        println!("engine log level was {:?}", runtime.set_engine_log_level(9));
        println!("profiler flag after log setup:  {}", flag(&runtime));
    }
    // The soft-assert gate `outgoing_call` opens. `f8502` returns immediately
    // while this byte is zero, so setting it turns on a body that every
    // `file.cc:line` in the engine runs through.
    if std::env::args().any(|arg| arg == "--enable-asserts") {
        runtime.write_bytes_at(1_351_084, &[1])?;
        println!("profiler flag after assert gate: {}", flag(&runtime));
    }

    println!(
        "main stack: base {} end {} current {} free {}",
        stack_value(&mut runtime, "emscripten_stack_get_base"),
        stack_value(&mut runtime, "emscripten_stack_get_end"),
        stack_value(&mut runtime, "emscripten_stack_get_current"),
        stack_value(&mut runtime, "emscripten_stack_get_free"),
    );

    let started = runtime.call_embind(
        "initVoipStack",
        &[
            Value::Str("15550002222@c.us".into()),
            Value::Str("0".into()),
            Value::Str("{}".into()),
        ],
    );
    println!("initVoipStack -> {started:?}");
    println!("profiler flag after init:       {}", flag(&runtime));
    println!(
        "main stack after init: current {} free {}",
        stack_value(&mut runtime, "emscripten_stack_get_current"),
        stack_value(&mut runtime, "emscripten_stack_get_free"),
    );

    // Starting a call is what drives the workers deep enough to matter: init
    // alone leaves them parked in the futex wait.
    let call = runtime.call_embind(
        "startVoipCall",
        &[
            Value::Str("11223344556677@lid".into()),
            Value::StringList(vec!["11223344556677:0@lid".into()]),
            Value::Str("0011223344556677".into()),
            Value::Bool(false),
            Value::Str("11223344556677@lid".into()),
            Value::Bool(false),
            Value::Bytes(vec![0xA5; 32]),
        ],
    );
    runtime.refuel();
    runtime.settle(std::time::Duration::from_secs(5));
    println!("startVoipCall -> {call:?}");
    println!("profiler flag after the call:   {}", flag(&runtime));
    println!(
        "main stack after the call: current {} free {}",
        stack_value(&mut runtime, "emscripten_stack_get_current"),
        stack_value(&mut runtime, "emscripten_stack_get_free"),
    );

    let traps = runtime
        .logs()
        .into_iter()
        .filter(|line| line.contains("stopped:"))
        .count();
    println!("worker threads that stopped on an error: {traps}");

    Ok(())
}
