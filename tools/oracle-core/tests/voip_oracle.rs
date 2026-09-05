//! The VoIP module as a behavioural oracle.
//!
//! These tests call the real WhatsApp Web calling engine and assert on what it
//! returns. They exist to be compared against whatsapp-rust: any constant or
//! behaviour asserted here is the ground truth the Rust implementation has to
//! match, taken from the shipped artifact rather than from a decompilation.
//!
//! They skip when the capture directory is absent. A skipped run is not a
//! passing run — check for `skipping:` in the output.

use oracle_core::{Catalog, Runtime, Value};

const VOIP_MODULE: &str = "JgwtTQVeWPm";

/// Loads the VoIP module with its constructors already run, or `None` when the
/// capture is not available.
fn voip() -> Option<Runtime> {
    let catalog = Catalog::discover().ok()?;
    let module = catalog.resolve(VOIP_MODULE).ok()?;
    let bytes = std::fs::read(&module.path).ok()?;

    let mut runtime = Runtime::instantiate(&bytes).expect("VoIP module should instantiate");
    runtime
        .run_ctors()
        .expect("VoIP constructors should run to completion");
    runtime.clear_calls();
    Some(runtime)
}

macro_rules! voip_or_skip {
    () => {
        match voip() {
            Some(runtime) => runtime,
            None => {
                eprintln!("skipping: no VoIP capture (set WA_WASM_DIR)");
                return;
            }
        }
    };
}

#[test]
fn registers_its_signaling_api() {
    let mut runtime = voip_or_skip!();
    let registry = runtime.embind();

    // The entry points that carry protocol behaviour. If a capture stops
    // exporting one of these, the oracle can no longer answer for it, and that
    // should fail loudly rather than silently narrow the test surface.
    for expected in [
        "handleIncomingSignalingOffer",
        "handleIncomingSignalingMessage",
        "handleIncomingSignalingAck",
        "handleIncomingSignalingReceipt",
        "startVoipCall",
        "acceptCall",
        "rejectCall",
        "endCall",
    ] {
        assert!(
            registry
                .functions
                .iter()
                .any(|function| function.name == expected),
            "VoIP module should register `{expected}`"
        );
    }

    assert!(
        registry.functions.len() > 90,
        "expected the full API, got {} functions",
        registry.functions.len()
    );
}

#[test]
fn reports_its_p2p_virtual_endpoint() {
    let mut runtime = voip_or_skip!();

    // Documentation addresses (RFC 5737 / RFC 3849). The engine uses them as
    // placeholders for the P2P path rather than real routable addresses.
    assert_eq!(
        runtime.call_embind("getWebP2PVirtualIpv4", &[]).unwrap(),
        Value::Str("192.0.2.1".to_owned())
    );
    assert_eq!(
        runtime.call_embind("getWebP2PVirtualIpv6", &[]).unwrap(),
        Value::Str("2001:db8::1".to_owned())
    );
    assert_eq!(
        runtime.call_embind("getWebP2PVirtualPort", &[]).unwrap(),
        Value::Int(9999)
    );
}

#[test]
fn reports_its_sctp_ice_event_ids() {
    let mut runtime = voip_or_skip!();

    // Wire constants: these ids go into telemetry events, so a Rust
    // implementation that logs them must use the same numbers.
    //
    // They are the engine's, not the protocol's, and they move: the previous
    // capture answered 199/200/201 and this one answers 203/204/205 — the whole
    // block shifted by four, which is what a renumbering looks like next to a
    // corrupt read. Ask the module rather than carrying the old numbers over.
    for (function, expected) in [
        ("getTsLoggerEventIdWebSctpIceConnectionStart", 203),
        ("getTsLoggerEventIdWebSctpIceConnectionComplete", 204),
        ("getTsLoggerEventIdWebSctpIceConnectionFailed", 205),
    ] {
        assert_eq!(
            runtime.call_embind(function, &[]).unwrap(),
            Value::Int(expected),
            "{function}"
        );
    }
}

/// The property the whole harness rests on: identical input, identical output.
/// Without it, a difference between this and whatsapp-rust could never be
/// attributed to the implementation rather than to the run.
#[test]
fn is_deterministic_across_instances() {
    let Some(mut first) = voip() else {
        eprintln!("skipping: no VoIP capture (set WA_WASM_DIR)");
        return;
    };
    let mut second = voip().expect("second instance");

    let probes: [(&str, Vec<Value>); 5] = [
        ("getWebP2PVirtualIpv4", vec![]),
        ("getWebP2PVirtualIpv6", vec![]),
        ("getWebP2PVirtualPort", vec![]),
        ("getCallInfo", vec![]),
        ("getEncodedVideoPortMask", vec![]),
    ];

    for (function, args) in &probes {
        let a = first.call_embind(function, args);
        let b = second.call_embind(function, args);
        assert_eq!(
            a.as_ref().ok(),
            b.as_ref().ok(),
            "`{function}` differed between two runs"
        );
    }
}

/// Calling the same function repeatedly in one instance must also be stable —
/// a test that compares against whatsapp-rust will do exactly this.
#[test]
fn is_deterministic_across_repeated_calls() {
    let mut runtime = voip_or_skip!();

    let first = runtime.call_embind("getWebP2PVirtualIpv4", &[]).unwrap();
    for _ in 0..8 {
        assert_eq!(
            runtime.call_embind("getWebP2PVirtualIpv4", &[]).unwrap(),
            first
        );
    }
}

#[test]
fn accepts_abprop_overrides() {
    let mut runtime = voip_or_skip!();

    // Property setters are the documented way to steer engine behaviour, and
    // they must not throw for an unknown key.
    assert_eq!(
        runtime
            .call_embind(
                "setABPropBool",
                &[Value::Str("test_prop".to_owned()), Value::Bool(true)]
            )
            .unwrap(),
        Value::Void
    );
    assert_eq!(
        runtime
            .call_embind(
                "setABPropInt",
                &[Value::Str("test_int".to_owned()), Value::Int(42)]
            )
            .unwrap(),
        Value::Void
    );
}

/// The call-entry signatures, pinned because a sibling project's do not match.
///
/// `~/projects/meowmeow-node-wasm` drives a WhatsApp Web VoIP module from Node
/// and does place calls, which makes it tempting to copy its argument lists.
/// Its module is a different build — 82 embind functions against this one's
/// 206 — and its entry points take more arguments:
///
/// ```text
///                  meowmeow's build                          this build
/// initVoipStack    (str, str, str, bool, u32, u32, u32, u32)  (str, str, str)
/// startVoipCall    (str, List, str, bool, str, bool, bool, …) (str, List, str, bool, str, bool, …)
/// ```
///
/// The extra `initVoipStack` arguments are configuration — `voipParamsVersion`,
/// `maxParticipants`, `maxGroupSize`. This build does not accept them, which is
/// consistent with what the engine says at runtime: `getVoipParam("options.*")`
/// answers empty and the incoming path reports `Application settings not
/// loaded`. Configuration moved out of init, so it has to arrive some other
/// way, and passing more arguments cannot substitute for that.
///
/// If a capture update changes these, the comparison above needs redoing before
/// any of it is trusted.
#[test]
fn the_call_entry_points_take_the_arguments_this_build_declares() {
    let mut runtime = voip_or_skip!();
    let registry = runtime.embind();

    // embind puts the return type first in `arg_types`, so the parameter count
    // is one less than its length.
    let arity = |name: &str| -> Option<usize> {
        registry
            .functions
            .iter()
            .find(|function| function.name == name)
            .map(|function| function.arg_types.len() - 1)
    };

    assert_eq!(
        arity("initVoipStack"),
        Some(3),
        "initVoipStack takes three JIDs here; meowmeow's build takes eight arguments"
    );
    assert_eq!(
        arity("startVoipCall"),
        Some(7),
        "startVoipCall takes seven arguments here; meowmeow's build takes eight"
    );
}

/// What the engine will log, before anything asks it to log more.
///
/// Every line is gated by one threshold compare, and the wrappers pick the
/// level: 3 for an ordinary line, 4 for the one a subsystem writes when it
/// *finishes*. Whether a missing line is evidence depends entirely on this
/// number — below 4 it says nothing, at 4 it means the code did not run. That
/// distinction decided a diagnosis here, so it is pinned rather than assumed.
#[test]
fn the_engine_starts_willing_to_log_its_completions() {
    let mut runtime = voip_or_skip!();

    let previous = runtime
        .set_engine_log_level(9)
        .expect("the threshold is a plain word in memory");
    assert_eq!(
        previous, 4,
        "level-4 lines are on by default, so their absence is evidence"
    );

    let restored = runtime.set_engine_log_level(previous).expect("set back");
    assert_eq!(restored, 9, "the write took effect");
}
