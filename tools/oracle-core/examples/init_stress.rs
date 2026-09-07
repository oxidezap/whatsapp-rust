//! Measures how reliably the VoIP stack initialises.
//!
//! The startup race this exists to quantify is described in
//! `tests/signaling.rs`; run it after touching threading or scheduling.
//!
//! ```sh
//! cargo run --release --example init_stress -- [rounds] [--register-main-thread]
//! ```
//!
//! `--register-main-thread` is the second thing this measures, and it is the
//! blocker for outgoing signaling rather than for startup. The engine queues
//! its outbound stanzas on the main-thread proxy queue, and every drain of that
//! queue fails while `emscripten_is_main_runtime_thread()` is false — see
//! `state.rs`. Turning it on was measured as *worse*, `initVoipStack` trapping
//! instead, which is why it is off. That measurement was taken while every
//! guest thread was running on the main thread's stack, so it is worth
//! retaking: this is the switch that would let an answer reach
//! `sendSignalingXMPP_js_sync`.
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let register_main_thread = args.iter().any(|arg| arg == "--register-main-thread");
    let rounds: usize = args.iter().find_map(|arg| arg.parse().ok()).unwrap_or(20);
    // Which capture. Taking it as an argument is what makes this a control:
    // the same host against the previous capture says whether a failure is the
    // module's or ours, and that question came up on the first bump.
    let module = args
        .iter()
        .find(|arg| !arg.starts_with("--") && arg.parse::<usize>().is_err())
        .cloned()
        .unwrap_or_else(|| "JgwtTQVeWPm".to_owned());

    let catalog = Catalog::discover()?;
    let entry = catalog.resolve(&module)?;
    let bytes = std::fs::read(&entry.path)?;

    println!("{module}: {rounds} rounds, register_main_thread={register_main_thread}");

    let (mut ok, mut forced) = (0usize, 0u64);
    for round in 1..=rounds {
        let mut r = Runtime::instantiate(&bytes)?;
        r.set_thread_policy(ThreadPolicy::Spawn);
        r.set_main_thread_registration(register_main_thread);
        r.run_ctors()?;

        let started = std::time::Instant::now();
        let result = r.call_embind(
            "initVoipStack",
            &[
                Value::Str("15550002222@s.whatsapp.net".into()),
                Value::Str("0".into()),
                Value::Str("{}".into()),
            ],
        );
        let elapsed = started.elapsed();
        forced += r.forced_turns();

        match &result {
            Ok(Value::Int(0)) => ok += 1,
            other => println!(
                "{round:>3}: {:?} in {elapsed:?}\n     host log: {:?}",
                other
                    .as_ref()
                    .err()
                    .map(|_| "trap")
                    .unwrap_or("unexpected value"),
                r.logs()
                    .into_iter()
                    .filter(|line| {
                        // "C++ exception" is what `cxa.rs` logs when it can name
                        // what was thrown, and it is the line worth having:
                        // without it a failure reads as an anonymous trap inside
                        // `std::terminate` and says nothing about the cause.
                        line.contains("C++ exception")
                            || line.contains("caught in")
                            || line.contains("export absent")
                            || line.contains("failed")
                    })
                    // The *first* few, not the last. Once a noexcept wrapper
                    // has reached `std::terminate` every later line is the loop
                    // reporting itself; the error that started it is at the top.
                    .take(4)
                    .map(|line| line.chars().take(110).collect::<String>())
                    .collect::<Vec<_>>()
            ),
        }
    }

    println!("{ok}/{rounds} succeeded, {forced} forced turns");
    Ok(())
}
