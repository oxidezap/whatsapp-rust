//! What a function was handed on the run where it trapped.
//!
//! `initVoipStack` fails about two runs in five on the `JgwtTQVeWPm` capture,
//! and the first thing to go wrong is `wasm trap: out of bounds memory access`
//! inside func 606 — which is `free`. Everything after that is the host's
//! `invoke_*` reporting the trap as a C++ throw, a `noexcept` wrapper answering
//! that with `std::terminate`, and terminate looping; none of it says anything
//! about the cause.
//!
//! This marks a function's entry with one of its parameters, so the value it
//! died on is on the record. A pointer inside the guest stack region says heap
//! metadata was overwritten by a frame; a wild one says the pointer was never
//! valid.
//!
//! ```sh
//! cargo run --release --example free_trap -- [rounds] [--func N] [--local N]
//! ```
//!
//! **Do not point this at `free` itself.** The host trace holds 8192 calls and
//! the engine frees far more than that during startup, so the tail — the only
//! part that matters — is the part that gets dropped. Mark the caller on the
//! path instead: func 13894 is the deleter the failing backtrace goes through,
//! and it reaches `free` in one place.

use oracle_core::patch::{self, Plan};
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};

/// The deleter on the failing path: `if (13895(arg2)) 14376(arg0) else free(arg0)`.
const DEFAULT_FUNC: u32 = 13894;

fn main() -> anyhow::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flag = |name: &str, fallback: u32| -> u32 {
        args.iter()
            .position(|arg| arg == name)
            .and_then(|at| args.get(at + 1))
            .and_then(|value| value.parse().ok())
            .unwrap_or(fallback)
    };
    let func = flag("--func", DEFAULT_FUNC);
    let local = flag("--local", 0);
    let rounds: usize = args
        .iter()
        .take_while(|arg| !arg.starts_with("--"))
        .find_map(|arg| arg.parse().ok())
        .unwrap_or(6);

    let catalog = Catalog::discover()?;
    let entry = catalog.resolve("JgwtTQVeWPm")?;
    let bytes = std::fs::read(&entry.path)?;

    let plan = Plan {
        value_entry: vec![(func, local)],
        id_base: patch::DEFAULT_ID_BASE,
        ..Plan::default()
    };
    let (instrumented, map) = patch::instrument(&bytes, &plan)?;
    println!(
        "marking local {local} at entry of func {func}, via {} ({} -> {} bytes)",
        map.via_symbol,
        bytes.len(),
        instrumented.len()
    );

    let (mut ok, mut trapped) = (0usize, 0usize);
    for round in 1..=rounds {
        let mut runtime = Runtime::instantiate(&instrumented)?;
        // Mirror the markers into their own ring. The host call trace fills up
        // with the engine's ordinary traffic long before startup fails, so the
        // tail — the only part that answers anything — is what it drops.
        runtime.shared().watch_markers(&map.via_symbol);
        runtime.set_thread_policy(ThreadPolicy::Spawn);
        runtime.run_ctors()?;

        let result = runtime.call_embind(
            "initVoipStack",
            &[
                Value::Str("15550002222@s.whatsapp.net".into()),
                Value::Str("0".into()),
                Value::Str("{}".into()),
            ],
        );

        let fired = runtime.shared().markers();

        let outcome = match &result {
            Ok(Value::Int(0)) => {
                ok += 1;
                "ok".to_owned()
            }
            Ok(other) => format!("{other:?}"),
            Err(_) => {
                trapped += 1;
                "trap".to_owned()
            }
        };

        // The tail is what matters on a trapping round: the last value is the
        // one the failing call carried.
        let tail: Vec<String> = fired
            .iter()
            .rev()
            .take(4)
            .map(|(_, value)| format!("{value:#x}"))
            .collect();

        println!(
            "{round:>3}: {outcome:<5} {} hits, last: {}",
            fired.len(),
            tail.join(" ")
        );
    }

    println!("{ok} ok, {trapped} trapped, of {rounds}");
    Ok(())
}
