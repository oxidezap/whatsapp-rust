//! Feeding the VoIP engine a real signaling stanza.
//!
//! The stanza is built with whatsapp-rust's own binary codec and handed to
//! WhatsApp Web's engine. The engine's internal log is what makes this a test
//! rather than a smoke check: it names the argument it read, the parser error it
//! hit, and the subsystem that failed, so the assertions are about behaviour
//! instead of about "the call returned void".
//!
//! Argument order and encoding both come from WhatsApp Web's own bridge:
//!
//! ```js
//! handleIncomingSignalingOffer(serializeVoipWapNode(node), ...)
//! // Web bridge = base64(encodeStanza(node)), including the flag byte
//! ```
//!
//! All identities are fictitious.
//!
//! # Why these are `#[ignore]`d
//!
//! Because they are slow. Each brings up PJSIP's worker pool under host-driven
//! scheduling, and the file takes about fifteen minutes:
//!
//! ```sh
//! cargo test --release --test signaling -- --ignored --test-threads 1
//! ```
//!
//! `--test-threads 1` matters: several engines at once still contend for cores.
//!
//! The slow gate runs each scenario in a fresh process through nextest. Engine
//! startup, traps, missing captures and incomplete logs are failures. No retry
//! can turn a failed observation into conformance evidence.
//!
mod common;

use base64::Engine as _;
use std::time::Duration;

use common::evidence::{self, Failure};
use oracle_core::{Runtime, ThreadPolicy, Value};
use sha2::{Digest, Sha256};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Server;
use wacore_binary::{Jid, Node, marshal};

const VOIP: &str = "JgwtTQVeWPm";
const CALL_ID: &str = "0102030405060708";
const CALLER: &str = "15550001111";
const CALLEE: &str = "15550002222";
/// Large enough for a whole call setup. Starting a call now negotiates media
/// per participant and per stream, which is thousands of lines; at 64 KiB the
/// ring wrapped and the earliest lines — the ones saying the call started —
/// were the ones lost.
const LOG_RING: u32 = 4 << 20;

/// Longest to wait for the engine to finish reacting to an operation.
const REACT_TIMEOUT: Duration = Duration::from_secs(15);

/// How long the log must stay unchanged before the reaction counts as over.
const QUIET: Duration = Duration::from_millis(300);

/// Start one fresh engine. Startup failure is infrastructure evidence, not a retry.
fn engine() -> Runtime {
    engine_with(ThreadPolicy::Spawn)
}

fn engine_with(policy: ThreadPolicy) -> Runtime {
    match try_engine(policy) {
        Ok(runtime) => runtime,
        Err(EngineError::Unavailable) => evidence::fail(Failure::MissingCapture, VOIP),
        Err(EngineError::Broken(why) | EngineError::InitFailed(why)) => {
            evidence::fail(Failure::Infrastructure, why)
        }
    }
}

enum EngineError {
    /// The capture directory or module is missing.
    Unavailable,
    Broken(String),
    InitFailed(String),
}

fn try_engine(policy: ThreadPolicy) -> Result<Runtime, EngineError> {
    let Some(bytes) =
        common::capture(VOIP).map_err(|error| EngineError::Broken(error.to_string()))?
    else {
        return Err(EngineError::Unavailable);
    };

    eprintln!(
        "oracle capture={VOIP} capture_sha256={} bytes={} source=wasm.lock.json",
        hex::encode(Sha256::digest(&bytes)),
        bytes.len()
    );
    let mut runtime = Runtime::instantiate(&bytes)
        .map_err(|error| EngineError::Broken(format!("instantiate: {error:#}")))?;
    runtime.set_thread_policy(policy);
    // Register as the main runtime thread with `can_block = 0`, as the browser
    // does — the futex the engine compiles reaches `memory.atomic.wait32` with
    // `1`, and off the main browser thread that blocks rather than throwing.
    //
    // It only works alongside the draining below: registration is what lets the
    // engine hand work to this thread, and work handed over and never collected
    // reads as an engine that did nothing.
    runtime.set_main_thread_registration(true);
    runtime
        .run_ctors()
        .map_err(|error| EngineError::Broken(format!("ctors: {error:#}")))?;
    runtime
        .attach_log_ring(LOG_RING)
        .map_err(|error| EngineError::Broken(format!("log ring: {error:#}")))?;
    // The media stack must actually come up: with it down the parser never
    // reaches the call stack and every assertion here would be about a dead
    // engine. `0` is success; `120011` is PJSIP being denied a thread.
    let init = common::evidence::call(
        &mut runtime,
        "initVoipStack",
        &[
            Value::Str(format!("{CALLEE}@c.us")),
            Value::Str(format!("{CALLEE}:0@c.us")),
            Value::Str("99887766554433:0@lid".to_owned()),
        ],
    );
    runtime.refuel();
    if policy == ThreadPolicy::Spawn
        && init.as_ref().ok().and_then(|value| value.as_int()) != Some(0)
    {
        return Err(EngineError::InitFailed(format!(
            "init={init:?} log={:?}",
            runtime.engine_log()
        )));
    }
    // The event worker must announce readiness before delivering a scenario.
    if policy == ThreadPolicy::Spawn && !wait_for_line(&mut runtime, "call_event_proc resumed") {
        return Err(EngineError::InitFailed(format!(
            "the event thread never announced itself: {:?}",
            runtime.engine_log()
        )));
    }
    Ok(runtime)
}

/// Waits for a line to appear in the engine's log, draining as it goes.
///
/// Matched on a distinctive substring, and `call_event_proc resumed` is one —
/// but see the module docs for why that is worth checking every time: the
/// obvious needle for the call-stack banner turned out to be a substring of an
/// unrelated line, and the test passed on a stanza that never parsed.
fn wait_for_line(runtime: &mut Runtime, needle: &str) -> bool {
    let deadline = std::time::Instant::now() + REACT_TIMEOUT;
    while std::time::Instant::now() < deadline {
        if runtime
            .engine_log()
            .iter()
            .any(|line| line.contains(needle))
        {
            return true;
        }
        runtime.try_process_queued_calls().unwrap_or_else(|error| {
            common::evidence::fail(common::evidence::Failure::Infrastructure, error)
        });
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

/// Serialises the tests that start real guest threads.
///
/// Each one brings up a PJSIP worker pool, and cargo runs tests in parallel by
/// default. Several engines competing for cores makes them miss their own
/// deadlines, which surfaces as an unrelated-looking failure rather than as
/// contention. The lock is poison-tolerant: a panicking test must not cascade
/// into every test after it.
use common::threaded_guard;

fn jid(user: &str) -> Jid {
    Jid::new(user, Server::Pn)
}

/// Native input is the offer child selected by WAWebHandleVoipCall.callParser,
/// not its enclosing call stanza. The Web bridge serializes that child intact.
/// Keep call-id/call-creator and settings on the offer, as in the preserved
/// dispatcher and WAWebVoipStackInterfaceWeb. See README.md for source pins.
fn offer_stanza() -> Node {
    NodeBuilder::new("offer")
        .attr("call-id", CALL_ID)
        .attr("call-creator", jid(CALLER))
        .children([
            NodeBuilder::new("audio")
                .attr("enc", "opus")
                .attr("rate", "16000")
                .build(),
            NodeBuilder::new("net").attr("medium", "3").build(),
            NodeBuilder::new("encopt").attr("keygen", "2").build(),
            NodeBuilder::new("voip_settings")
                .attr("uncompressed", "1")
                .bytes(VOIP_SETTINGS.to_vec())
                .build(),
        ])
        .build()
}

/// The settings blob, in whatsapp-rust's standard-Opus form.
/// Adding `caller_timeout`/`callee_timeout` under `options` was tried, on the
/// theory that an empty ring window would explain the immediate miss —
/// `getVoipParam("options.caller_timeout")` does read back empty. It changes
/// nothing: the offer still ends `term_reason 27`. Kept in whatsapp-rust's
/// standard form.
const VOIP_SETTINGS: &[u8] =
    br#"{"encode":{"use_mlow_codec_v1":"false"},"options":{"enable_48khz_rtp_clock":"false"}}"#;

/// Web serializes the full stanza. The flagless form is a negative fixture.
/// See the preserved Web bridge evidence in README.md.
fn serialize(node: &Node, keep_flag: bool) -> String {
    let encoded = marshal::marshal(node).expect("marshal");
    let payload = if keep_flag {
        &encoded[..]
    } else {
        &encoded[1..]
    };
    base64::engine::general_purpose::STANDARD.encode(payload)
}

fn deliver(runtime: &mut Runtime, stanza: String) -> Vec<String> {
    let args = offer_arguments(stanza, runtime.virtual_unix_time());
    deliver_arguments(runtime, args)
}

fn deliver_arguments(runtime: &mut Runtime, args: [Value; 9]) -> Vec<String> {
    let mark = runtime.engine_log().len();
    runtime.clear_calls();

    let stanza = args[0].as_str().expect("serialized offer");
    let input_hash = hex::encode(Sha256::digest(stanza.as_bytes()));
    eprintln!(
        "oracle input_base64={stanza} bridge_arguments={:?}",
        &args[1..]
    );
    let outcome = common::evidence::call(runtime, "handleIncomingSignalingOffer", &args);
    runtime.refuel();
    let settled = wait_for_reaction(runtime, mark);
    common::evidence::record(runtime);
    let lines = runtime.engine_log_from(mark);
    let overflowed = runtime
        .engine_log_overflowed()
        .unwrap_or_else(|error| evidence::fail(Failure::Infrastructure, error));
    eprintln!(
        "oracle input_sha256={input_hash} capture={VOIP} outcome={outcome:?} settled={settled} overflowed={overflowed} log={lines:?}"
    );
    evidence::check_log(overflowed, runtime.memory_view_is_coherent(), settled)
        .unwrap_or_else(|kind| evidence::fail(kind, "incomplete offer evidence"));
    if let Err(error) = outcome {
        evidence::fail(
            Failure::Infrastructure,
            format!("offer call trapped: {error:#}"),
        );
    }
    if lines
        .iter()
        .any(|line| line.contains("check_locking_order"))
    {
        evidence::fail(
            Failure::Infrastructure,
            "engine lock watchdog interrupted the scenario",
        );
    }
    lines
}

/// WID strings belong to the native ABI. Wire JIDs remain canonical. In the
/// preserved Web bridge, argument 8 is peer_jid.toString(), whose PN suffix is
/// c.us. Using Rust Jid::to_string() here changes the input being tested.
fn offer_arguments(stanza: String, now: u64) -> [Value; 9] {
    [
        Value::Str(stanza),
        Value::Str("web".to_owned()),
        Value::Str("2.3000.0".to_owned()),
        Value::Str(now.to_string()),
        Value::Str(now.to_string()),
        Value::Bool(false),
        Value::Bool(false),
        Value::Str(format!("{CALLER}@c.us")),
        Value::Bytes(Vec::new()),
    ]
}

#[test]
fn native_offer_arguments_preserve_the_web_wid_abi() {
    let args = offer_arguments("fixture".to_owned(), 1_609_459_200);
    assert_eq!(args[0], Value::Str("fixture".to_owned()));
    assert_eq!(args[3], Value::Str("1609459200".to_owned()));
    assert_eq!(args[4], args[3]);
    assert_eq!(args[7], Value::Str("15550001111@c.us".to_owned()));
    assert_eq!(jid(CALLER).to_string(), "15550001111@s.whatsapp.net");
}

/// The original failing fixture passed the call envelope instead of voipNode.
#[test]
#[ignore = "real threads; see the module docs"]
fn an_enclosing_call_is_rejected_as_a_native_offer() {
    let _serial = threaded_guard();
    let mut runtime = engine();
    let envelope = NodeBuilder::new("call")
        .attr("from", jid(CALLER))
        .children([offer_stanza()])
        .build();
    let payload = serialize(&envelope, true);
    let lines = deliver(&mut runtime, payload);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("conversion_result FAILED")),
        "the native bridge should reject the envelope: {lines:?}"
    );
    assert!(!reached_call_stack(&lines));
}

/// Waits until the engine stops writing about what it was just asked to do.
///
/// The engine hands offer processing to its event thread, so the call returning
/// says nothing about whether it finished. A fixed sleep is wrong in both
/// directions — too short under load, wasted otherwise — so this waits for the
/// log to grow and then go quiet. Returns early on the common path.
fn wait_for_reaction(runtime: &mut Runtime, mark: usize) -> bool {
    let deadline = std::time::Instant::now() + REACT_TIMEOUT;
    let mut last_len = runtime.engine_log().len();
    let mut unchanged_since = std::time::Instant::now();

    while std::time::Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(20));
        // Drain what the engine parked for this thread.
        //
        // A quiet log is not a finished reaction: with the main thread
        // registered, work the engine hands to it waits in the proxy queue, and
        // only the host takes it out. Waiting for quiet without draining reports
        // "the engine did nothing" about a run in which it queued the answer and
        // nobody collected it.
        runtime.try_process_queued_calls().unwrap_or_else(|error| {
            common::evidence::fail(common::evidence::Failure::Infrastructure, error)
        });
        let len = runtime.engine_log().len();

        if len != last_len {
            last_len = len;
            unchanged_since = std::time::Instant::now();
            continue;
        }
        // Something was written, and nothing has been for a while.
        if len > mark && unchanged_since.elapsed() >= QUIET {
            return true;
        }
    }
    false
}

/// The engine writes diagnostics into a host-supplied buffer. Everything else
/// here depends on that working.
#[test]
#[ignore = "real threads; see the module docs"]
fn the_engine_log_is_readable() {
    let _serial = threaded_guard();
    let runtime = engine();
    let log = runtime.engine_log();

    assert!(!log.is_empty(), "engine produced no log output");
    assert!(
        log.iter().any(|line| line.contains("initVoipStack called")),
        "expected the init line: {log:?}"
    );
}

/// Pins the argument order, which is not derivable from the embind signature —
/// it starts with five consecutive strings. The engine echoes two of them by
/// name, so its own log is the evidence.
#[test]
#[ignore = "real threads; see the module docs"]
fn arguments_are_in_the_documented_order() {
    let _serial = threaded_guard();
    let mut runtime = engine();
    let lines = {
        let payload = serialize(&offer_stanza(), true);
        deliver(&mut runtime, payload)
    };

    let echoed = lines
        .iter()
        .find(|line| line.contains("handleIncomingSignalingOffer from platform"))
        .unwrap_or_else(|| panic!("engine did not echo its arguments: {lines:?}"));

    assert!(
        echoed.contains("platform web") && echoed.contains("version 2.3000.0"),
        "arguments 2 and 3 are platform and version, got: {echoed}"
    );
}

/// Whether the offer reached the call stack.
///
/// Matched on the call-stack banner, not on "Offer from": the entry-point line
/// (`handleIncomingSignalingOffer from platform ...`) contains that as a
/// substring, so the looser test reports success for a stanza that never
/// parsed.
fn reached_call_stack(lines: &[String]) -> bool {
    lines
        .iter()
        .any(|line| line.contains("VOIP STAC") && line.contains(CALL_ID))
}

/// The Web bridge includes the flag byte; its removal is a negative control.
#[test]
#[ignore = "real threads; see the module docs"]
fn engine_accepts_the_framed_encoding() {
    let _serial = threaded_guard();

    let framed = {
        let mut runtime = engine();
        let payload = serialize(&offer_stanza(), true);
        deliver(&mut runtime, payload)
    };
    assert!(
        reached_call_stack(&framed),
        "the framed encoding should reach the call stack: {framed:?}"
    );

    let unframed = {
        let mut runtime = engine();
        let payload = serialize(&offer_stanza(), false);
        deliver(&mut runtime, payload)
    };
    assert!(
        !reached_call_stack(&unframed),
        "dropping the flag byte should not reach the call stack: {unframed:?}"
    );
    assert!(
        unframed
            .iter()
            .any(|line| line.contains("conversion_result FAILED")
                || line.contains("invalid list size")),
        "the negative control needs an explicit parser rejection: {unframed:?}"
    );
}

/// Malformed input must not take the *host* down, which is what makes the
/// oracle safe to fuzz against.
///
/// Deliberately not "the engine keeps working": a stanza truncated mid-blob
/// leaves the engine unresponsive to later calls, and that is the engine's
/// behaviour to report rather than a property to assert away. What has to hold
/// is that the harness stays usable — the call returns instead of hanging or
/// panicking, and a fresh instance still works.
///
/// Each case gets its own engine. Now that a well-formed offer is accepted, the
/// engine carries call state forward, so reusing one instance would test the
/// accumulation rather than the input.
#[test]
#[ignore = "real threads; see the module docs"]
fn engine_survives_malformed_offers() {
    let _serial = threaded_guard();
    /// Builds one malformed payload, given a live engine to size it against.
    type BuildPayload = Box<dyn Fn(&Runtime) -> String>;

    let cases: Vec<(&str, BuildPayload)> = vec![
        ("empty", Box::new(|_: &Runtime| String::new())),
        (
            "not base64",
            Box::new(|_: &Runtime| "!!! not a stanza !!!".to_owned()),
        ),
        (
            "base64 of garbage",
            Box::new(|_: &Runtime| base64::engine::general_purpose::STANDARD.encode([0xffu8; 32])),
        ),
        (
            "truncated stanza",
            Box::new(|_: &Runtime| {
                let full = serialize(&offer_stanza(), true);
                full[..full.len() / 2].to_owned()
            }),
        ),
    ];

    for (label, build) in cases {
        let mut runtime = engine();
        let payload = build(&runtime);
        let _ = deliver(&mut runtime, payload);

        // The call has to come back one way or the other. Whether the engine
        // still answers afterwards is its business; whether the host survives
        // is ours.
        runtime.refuel();
        let probe = common::evidence::call(&mut runtime, "getWebP2PVirtualIpv4", &[]);
        match probe {
            Ok(value) => assert_eq!(
                value,
                Value::Str("192.0.2.1".to_owned()),
                "engine answered a {label} offer with the wrong value"
            ),
            // Recorded, not asserted away: a truncated stanza does leave the
            // engine unable to answer.
            Err(_) => eprintln!("note: engine stopped answering after a {label} offer"),
        }
    }

    // The host is still able to bring up a working engine afterwards.
    let mut fresh = engine();
    assert_eq!(
        fresh.call_embind("getWebP2PVirtualIpv4", &[]).ok(),
        Some(Value::Str("192.0.2.1".to_owned()))
    );
}

/// Same input, same outcome.
///
/// What is compared is the set of engine *events* — the lines naming what it
/// did — not the transcript. Under threads the transcript is not reproducible
/// by construction: the virtual clock advances once per observation, so the
/// engine's own timestamps shift by a tick depending on how the workers were
/// scheduled, and worker output interleaves differently. Asserting the raw text
/// would be asserting the scheduler, which is precisely the guarantee threading
/// gives up.
#[test]
#[ignore = "real threads; see the module docs"]
fn offer_handling_is_deterministic() {
    let _serial = threaded_guard();
    let observe = || {
        let mut runtime = engine();
        let stanza = serialize(&offer_stanza(), true);
        let lines = deliver(&mut runtime, stanza);
        assert!(
            lines
                .iter()
                .any(|line| line.contains("wa_call_handle_incoming_xmpp_offer() status 0")),
            "determinism requires two accepted offers: {lines:?}"
        );
        events(lines)
    };
    let a = observe();
    let b = observe();

    assert_eq!(a, b, "two runs reached different engine events");
    assert!(!a.is_empty(), "no events recorded at all");
}

/// The lines that say what the engine decided, with everything timing-derived
/// dropped.
///
/// Overflow is rejected before comparing events. The engine's severity marker
/// is excluded from this comparison of decisions.
fn events(lines: Vec<String>) -> Vec<String> {
    const MARKERS: [&str; 5] = [
        "handleIncomingSignalingOffer from",
        "Offer from",
        "EVENT:",
        "call_term_reason",
        "wa_call_handle_incoming_xmpp_offer",
    ];

    let mut found: Vec<String> = lines
        .into_iter()
        .filter(|line| MARKERS.iter().any(|marker| line.contains(marker)))
        .map(|line| line.replace('!', " "))
        .collect();
    found.sort();
    found
}

/// Refusing threads makes the media stack fail, and the log says why.
///
/// `120011` is `PJ_ERRNO_START_SYS + EAGAIN`: PJSIP asking for the thread it
/// was denied. `ThreadPolicy::Spawn` is what fixes it — see
/// `tests/threading.rs`.
#[test]
#[ignore = "real threads; see the module docs"]
fn media_stack_reports_its_failure() {
    let _serial = threaded_guard();
    let runtime = engine_with(ThreadPolicy::Refuse);
    let log = runtime.engine_log();

    let failure = log
        .iter()
        .find(|line| line.contains("wa_call_init failed"))
        .unwrap_or_else(|| panic!("expected the media init to report a failure: {log:?}"));
    assert!(
        failure.contains("120011"),
        "unexpected media failure code: {failure}"
    );
}

/// The log reader must read the ring's header, not scan the buffer.
///
/// The earlier version searched the whole allocation for printable runs, so
/// unrelated bytes past the end of the log came back as extra lines and the
/// transcript changed depending on what else the heap happened to hold. That is
/// what made two probes of the same offer disagree.
#[test]
#[ignore = "real threads; see the module docs"]
fn the_log_reader_returns_only_the_log() {
    let _serial = threaded_guard();
    let runtime = engine();
    let lines = runtime.engine_log();

    assert!(!lines.is_empty(), "engine produced no log");
    // Every line the engine writes carries its thread tag; anything else came
    // from somewhere other than the log.
    for line in &lines {
        assert!(
            line.starts_with("[0]") || line.starts_with('['),
            "not a log line: {line:?}"
        );
    }
}

/// Guest threads are **not** serialised, and this pins that down.
///
/// `schedule.rs` is supposed to run one guest thread at a time, and several
/// safety arguments in this codebase lean on it — `HostState::read`'s SAFETY
/// note among them. It does not hold: a thread acquires the turn once around
/// its whole routine, and `yield_point` hands it on only while some other
/// thread is blocked in its own first `acquire`, so once every worker has
/// forced past `TURN_TIMEOUT` nothing waits and nothing yields.
///
/// Asserted the way it actually is, rather than the way it should be, because a
/// permanently red test teaches nobody anything and a silent one hides this
/// entirely. **If this test starts failing, the scheduler has been fixed** —
/// which is good news, and the thing to do is update it along with the notes in
/// `AGENTS.md`, `threads.rs` and `examples/ring_corruption.rs` that all say
/// otherwise.
#[test]
#[ignore = "real threads; see the module docs"]
fn guest_threads_are_not_serialised() {
    let _serial = threaded_guard();
    let runtime = engine();

    let peak = runtime.max_threads_in_wasm();
    eprintln!("most guest threads executing at once: {peak}");
    assert!(
        peak > 1,
        "the scheduler now serialises guest threads — see this test's docs, \
         several comments claim the opposite and need updating"
    );
}

/// The memory watch fires, and says which thread and which guest stack.
///
/// This exists because the watch's *silence* was briefly taken as evidence.
/// The first version checked only inside `host_func`, and `emscripten.rs`
/// defines most of its functions with `func_wrap`, so it stayed quiet through a
/// round that destroyed ten megabytes of guest memory — reported as "nothing
/// wrote to the watched span", meaning "nothing looked at it".
///
/// A watch that cannot be seen to fire is worth nothing, so this makes it fire:
/// arm it, change the span, then let the engine's workers make host calls.
#[test]
#[ignore = "real threads; see the module docs"]
fn the_memory_watch_names_the_moment() {
    let _serial = threaded_guard();
    let mut runtime = engine();

    assert!(runtime.watch_memory(), "no static data to watch");
    let (at, len) = runtime.coherence_witness().expect("witness");
    runtime
        .write_bytes_at(at, &vec![0xFF; len as usize])
        .expect("clobber the watched span");

    // The workers poll, so they reach a host call within milliseconds; this is
    // waiting for the report rather than for anything to happen.
    runtime
        .try_settle(Duration::from_secs(2))
        .unwrap_or_else(|error| {
            common::evidence::fail(common::evidence::Failure::Infrastructure, error)
        });

    // From the watch, not from the host log: the log refuses new lines once
    // full, and this report was being written past that point and discarded.
    let report = runtime
        .watch_report()
        .into_iter()
        .next()
        .expect("the watch never fired on a span that was definitely changed");
    assert!(
        report.contains("guest stack:"),
        "the report must carry the guest stack, which is the whole point of it: {report}"
    );
}

/// The log is withheld when the host's view of guest memory has gone wrong.
///
/// About one run in four, an incoming offer followed by an outgoing call leaves
/// the host reading entirely different bytes from the guest, and the ring comes
/// back as hundreds of lines of high-entropy noise. `engine_log` refuses in
/// that case — but waiting for a 1-in-4 fault to demonstrate the refusal is not
/// a test, it is a hope. Five consecutive runs of
/// `settings_from_an_incoming_offer_do_not_unblock_an_outgoing_call` came back
/// healthy and said nothing about whether the refusal works.
///
/// So the fault is induced instead: overwrite the witness and the log must go
/// away; put it back and the log must return. The second half is what makes
/// this a test of a live check rather than of a latch that trips once.
#[test]
#[ignore = "real threads; see the module docs"]
fn an_incoherent_memory_view_withholds_the_log() {
    let _serial = threaded_guard();
    let mut runtime = engine();

    let (at, len) = runtime
        .coherence_witness()
        .expect("no witness: the module placed no static data to watch");
    let original = runtime.read(at, len).expect("read witness");

    assert_eq!(runtime.memory_view_is_coherent(), Some(true));
    assert!(
        !runtime.engine_log().is_empty(),
        "engine produced no log to withhold"
    );

    runtime
        .write_bytes_at(at, &vec![0xFF; original.len()])
        .expect("clobber witness");

    assert_eq!(runtime.memory_view_is_coherent(), Some(false));
    assert!(
        runtime.engine_log().is_empty(),
        "the log was returned from a view known to be wrong"
    );
    assert!(
        runtime
            .logs()
            .iter()
            .any(|line| line.contains("engine log withheld")),
        "the refusal was silent; a caller has no way to tell it apart from an empty log"
    );

    // Static data is written once and read forever, so restoring it leaves the
    // guest exactly as it was — which is also why it is the right thing to
    // watch.
    runtime.write_bytes_at(at, &original).expect("restore");

    assert_eq!(runtime.memory_view_is_coherent(), Some(true));
    assert!(
        !runtime.engine_log().is_empty(),
        "the log did not come back once the view was good again"
    );
}

/// A module with no ring attached has no log, rather than whatever its heap
/// happens to contain.
#[test]
#[ignore = "real threads; see the module docs"]
fn no_ring_means_no_log() {
    let _serial = threaded_guard();
    let Some(bytes) =
        common::capture(VOIP).unwrap_or_else(|error| panic!("loading {VOIP}: {error:#}"))
    else {
        evidence::fail(Failure::MissingCapture, VOIP);
    };
    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.run_ctors().expect("ctors");

    assert!(
        runtime.engine_log().is_empty(),
        "log should be empty until a ring is attached"
    );
}

/// Overflow has to be reportable: once the ring wraps, an index taken from an
/// earlier read no longer points at the same line, so a diff across it is
/// meaningless rather than merely incomplete.
#[test]
#[ignore = "real threads; see the module docs"]
fn overflow_is_observable() {
    let _serial = threaded_guard();
    let mut runtime = engine();
    // A ring this size does not wrap during startup, so this pins the
    // "no overflow" reading; the accessor existing is what lets a caller with a
    // smaller ring notice the opposite.
    assert!(!runtime.engine_log_overflowed().expect("overflow probe"));
}

/// The offer reaches the call stack, rather than being rejected by the parser.
///
/// The engine names the call by the id from the stanza, which is only possible
/// once the shape is right — everything before this point failed in
/// `parse_xmpp_offer` and never got here.
#[test]
#[ignore = "real threads; see the module docs"]
fn a_well_formed_offer_reaches_the_call_stack() {
    let _serial = threaded_guard();
    let mut runtime = engine();
    let lines = {
        let payload = serialize(&offer_stanza(), true);
        deliver(&mut runtime, payload)
    };

    assert!(
        !lines.iter().any(|l| l.contains("parse_xmpp_offer")),
        "parser rejected the offer: {lines:?}"
    );
    assert!(
        reached_call_stack(&lines),
        "engine did not record the offer with its call id: {lines:?}"
    );
}

/// Argument 4 and 5 must be numeric.
///
/// The signature is five consecutive strings, so nothing distinguishes them;
/// the engine reports `stoull: no conversion` when they are not timestamps.
#[test]
#[ignore = "real threads; see the module docs"]
fn non_numeric_timestamps_are_rejected() {
    let _serial = threaded_guard();
    let mut runtime = engine();
    let mark = runtime.engine_log().len();

    let mut args = offer_arguments(
        serialize(&offer_stanza(), true),
        runtime.virtual_unix_time(),
    );
    args[3] = Value::Str("not-a-number".to_owned());
    args[4] = Value::Str("also-not".to_owned());
    let _ = common::evidence::call(&mut runtime, "handleIncomingSignalingOffer", &args);
    runtime.refuel();

    common::evidence::record(&runtime);
    let lines = runtime.engine_log_from(mark);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("msgEStr or msgTStr") || line.contains("stoull")),
        "expected the engine to report the parse failure: {lines:?}"
    );
}

/// The native offer requires its own call-id. Removing only that attribute
/// must produce a rejection, rather than an accepted anonymous call.
#[test]
#[ignore = "real threads; see the module docs"]
fn a_native_offer_without_a_call_id_is_rejected() {
    let _serial = threaded_guard();
    let mut runtime = engine();
    let mut missing = offer_stanza();
    missing.attrs.0.retain(|(key, _)| key != "call-id");
    let lines = deliver(&mut runtime, serialize(&missing, true));
    assert!(lines.iter().any(|line| line.contains("empty call-id") || line.contains("conversion_result FAILED")),
        "the native bridge must reject the missing call-id: {lines:?}");
}

/// The engine accepts the offer outright: status 0, no parser complaint.
///
/// This is the difference `uncompressed="1"` makes. Without it the same stanza
/// comes back as `70004` with `unexpected compressed voip params`, because the
/// engine reads the settings blob as compressed data.
#[test]
#[ignore = "real threads; see the module docs"]
fn a_well_formed_offer_is_accepted() {
    let _serial = threaded_guard();
    let mut runtime = engine();
    let payload = serialize(&offer_stanza(), true);
    let lines = deliver(&mut runtime, payload);

    // A missing line proves nothing once the ring has wrapped: the reader can
    // only return what is still in the buffer. Asserting through an overflow
    // reports "the engine refused" for a run in which it may well have
    // accepted, which is the kind of wrong answer an oracle must not give.
    if runtime.engine_log_overflowed().expect("overflow probe") {
        evidence::fail(Failure::Overflow, "engine log wrapped");
    }

    assert!(
        lines
            .iter()
            .any(|line| line.contains("wa_call_handle_incoming_xmpp_offer() status 0")),
        "engine did not accept the offer: {lines:?}"
    );
    assert!(
        !lines.iter().any(|line| line.contains("voip params")),
        "settings blob was rejected: {lines:?}"
    );
}

/// Does an incoming offer leave settings behind that an outgoing call can use?
///
/// An outgoing call fails in `make_and_cache_offer` at `offer.cc:430`, whose
/// guard reads two bytes of the call context — `+3856` and `+662166` — and both
/// are zero. Nothing in the module carries the `options.*` names that
/// `getVoipParam` answers to, so those come from the server's
/// `<voip_settings>` blob; on the incoming path the engine says as much, with
/// `Application settings not loaded`.
///
/// An incoming offer *does* carry that blob. If handling one caches settings,
/// an outgoing call made afterwards should get past the guard. This is the
/// cheapest test of "both blockers are the same missing configuration".
#[test]
#[ignore = "real threads; see the module docs"]
fn settings_from_an_incoming_offer_do_not_unblock_an_outgoing_call() {
    let _serial = common::engine_lock();
    let mut runtime = engine();

    let payload = serialize(&offer_stanza(), true);
    let _ = deliver(&mut runtime, payload);

    let mark = runtime.engine_log().len();
    let _ = common::evidence::call(
        &mut runtime,
        "startVoipCall",
        &[
            Value::Str(PEER_LID.to_owned()),
            Value::StringList(vec![PEER_LID_DEVICE.to_owned()]),
            Value::Str("0011223344556677".to_owned()),
            Value::Bool(false),
            Value::Str(PEER_LID.to_owned()),
            Value::Bool(false),
            Value::Bytes(Vec::new()),
        ],
    );
    runtime.refuel();
    runtime
        .try_settle(Duration::from_secs(5))
        .unwrap_or_else(|error| {
            common::evidence::fail(common::evidence::Failure::Infrastructure, error)
        });

    common::evidence::record(&runtime);
    let lines = runtime.engine_log_from(mark);

    // What this used to assert, and what it took to get here.
    //
    // About one run in four the ring came back holding 880 lines of
    // high-entropy noise — `"E'8da(R#"`, `"8+bb=BX+"` — and this test read that
    // as the engine's state being corrupted by the sequence. The noise was real
    // and the reading was wrong twice over: it was not the ring, it was all of
    // linear memory; and it was not the engine, it was **this host**.
    //
    // `env::get_random_bytes_js` takes `(len, buf)`, and the host had it as
    // `(buf, len)`. The module's only caller of it is the crypto callback that
    // `generate_raw_e2e_keys` dispatches through, which asks for 32 bytes; with
    // the arguments swapped that became fifteen megabytes of the host's own
    // PRNG written from address 32. The bytes really were key material, and the
    // host was the one writing them.
    let structured = lines
        .iter()
        .filter(|line| line.contains(".c") || line.contains("EVENT") || line.contains("call"))
        .count();
    eprintln!(
        "after an incoming offer, an outgoing call yields {} lines, {structured} of them structured; overflowed={}",
        lines.len(),
        runtime.engine_log_overflowed().expect("overflow probe")
    );
    for line in lines.iter().take(6) {
        eprintln!("  SAMPLE {line:?}");
    }

    // This is the regression guard for the swapped `get_random_bytes_js`
    // arguments, and it is an assertion now rather than an `eprintln!`.
    //
    // For as long as the host read that import as `(buf, len)` instead of
    // `(len, buf)`, this sequence destroyed the whole of linear memory about
    // one run in four — a request for 32 bytes at `0xf00000` became fifteen
    // megabytes of PRNG output written from address 32. The test tolerated it,
    // because the cause was unknown and a red suite that could not be fixed
    // teaches nobody anything. The cause is known, so the tolerance goes.
    assert_ne!(
        runtime.memory_view_is_coherent(),
        Some(false),
        "the host's view of guest memory went incoherent during offer-then-call — \
         the corruption this used to tolerate is back; see `get_random_bytes_js` in \
         emscripten.rs and examples/ring_corruption.rs"
    );

    // And anything the engine did write has the shape of engine output.
    assert!(
        !lines.is_empty() && structured > 0,
        "the log has lines but none of them are engine output: {lines:?}"
    );
}

/// Does the *incoming* path end up with a self participant?
///
/// The outgoing path does not: `make_and_cache_offer` fails because the group's
/// cached self pointer is null, even though `getCallInfo` marks a participant
/// `is_self`. Whether the same is true after an accepted incoming offer is the
/// cheapest way to tell a systemic gap from something specific to originating a
/// call — and the two paths build the call through different code.
///
/// This asserts what the engine reports rather than a desired outcome; it is a
/// probe kept honest by being a test.
#[test]
#[ignore = "real threads; see the module docs"]
fn an_incoming_offer_reports_its_participants() {
    let _serial = threaded_guard();
    let mut runtime = engine();
    let payload = serialize(&offer_stanza(), true);
    let lines = deliver(&mut runtime, payload);
    for line in &lines {
        eprintln!("  | {}", line.trim());
    }

    let info = common::evidence::call(&mut runtime, "getCallInfo", &[]);
    runtime.refuel();
    let Some(json) = info.as_ref().ok().and_then(|value| value.as_str()) else {
        evidence::fail(Failure::Infrastructure, format!("getCallInfo: {info:?}"));
    };
    if json.is_empty() {
        evidence::fail(
            Failure::Mismatch,
            "engine holds no call state after the offer",
        );
    }

    let has_self = json.contains("\"is_self\": true");
    let uuid_set = !json.contains("\"self_participant_uuid\": \"\"");
    eprintln!("incoming path: is_self={has_self} self_participant_uuid_set={uuid_set}");

    // The engine must at least be able to say who we are; without that the
    // incoming path cannot answer a call either.
    assert!(
        has_self,
        "no participant marked as ourselves after an accepted offer: {json}"
    );
}

/// Marking the blob compressed when it is not gets the offer rejected.
/// Pinned in both directions so the attribute's meaning is unambiguous.
#[test]
#[ignore = "real threads; see the module docs"]
fn an_unmarked_settings_blob_is_rejected() {
    let _serial = threaded_guard();
    let mut runtime = engine();

    let mut unmarked = offer_stanza();
    let Some(wacore_binary::node::NodeContent::Nodes(children)) = &mut unmarked.content else {
        panic!("offer fixture has no children");
    };
    children
        .iter_mut()
        .find(|node| node.tag == "voip_settings")
        .expect("settings fixture")
        .attrs = Default::default();

    let payload = serialize(&unmarked, true);
    let lines = deliver(&mut runtime, payload);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("unexpected compressed voip params")),
        "expected the engine to read the blob as compressed: {lines:?}"
    );
}

/// The engine takes the caller from argument 8, not from the stanza.
///
/// This is the shape WhatsApp Web uses — `t.peer_jid.toString()` in
/// `WAWeb/Handle/VoipCallOffer.js` — and getting it wrong is silent: the offer
/// is still accepted, and the engine simply attributes it to nobody. The log
/// redacts a JID to its last four digits, so that is what this looks for.
#[test]
#[ignore = "real threads; see the module docs"]
fn the_caller_comes_from_the_argument_not_the_stanza() {
    let _serial = threaded_guard();
    let mut runtime = engine();
    let payload = serialize(&offer_stanza(), true);
    let mut args = offer_arguments(payload, runtime.virtual_unix_time());
    // Deliberately different from the stanza's CALLER.
    args[7] = Value::Str("15550003333@c.us".to_owned());
    let lines = deliver_arguments(&mut runtime, args);

    if runtime.engine_log_overflowed().expect("overflow probe") {
        evidence::fail(Failure::Overflow, "engine log wrapped");
    }

    let offer_line = lines
        .iter()
        .find(|line| line.contains("Offer from:"))
        .unwrap_or_else(|| panic!("engine logged no offer line: {lines:?}"));

    assert!(
        offer_line.contains("3333"),
        "the caller should reach the engine, got {offer_line:?}"
    );
    // The failure this pins: an empty argument 8 reads as user zero.
    assert!(
        !offer_line.contains("from::0@"),
        "the engine read no caller at all: {offer_line:?}"
    );
}

/// The engine will start a call, given the arguments WhatsApp Web passes.
///
/// Two of them were wrong for a long time and neither failed loudly:
///
/// - `initVoipStack` takes three legacy-form JIDs — the user, the user's
///   device, and the user's **device LID** (`WAWeb/Voip/Init.js` passes
///   `getMaybeMeDeviceLid()`). The third was being given a settings blob, and
///   the engine reported it only as `Failed to fetch typed self lid device jid`
///   much later, inside a call attempt.
/// - `startVoipCall`'s second argument is the peer's list of *device* JIDs.
///   Empty, `create_participant_jid` fails and the call returns -1 having
///   written nothing at all to the log.
///
/// With both right the engine gets as far as `ACTION start_precall`, which is
/// the outbound path working. Pinned because it is the only end-to-end evidence
/// that the engine can be driven into starting a call at all.
#[test]
#[ignore = "real threads; see the module docs"]
fn the_engine_starts_an_outgoing_call() {
    let _serial = threaded_guard();

    let mut runtime = engine();

    let mark = runtime.engine_log().len();
    let outcome = common::evidence::call(
        &mut runtime,
        "startVoipCall",
        &[
            // Every JID in LID form: the engine enforces it outright —
            // `start_precall peer_participant_jids must be LID, enforce LID
            // for all calls` — and a phone-number JID fails with 70008.
            Value::Str(PEER_LID.to_owned()),
            // The peer's devices, in device-LID form. Empty, this returns
            // -1 with nothing written to the log at all.
            Value::StringList(vec![PEER_LID_DEVICE.to_owned()]),
            Value::Str("0011223344556677".to_owned()),
            Value::Bool(false),
            Value::Str(PEER_LID.to_owned()),
            Value::Bool(false),
            Value::Bytes(Vec::new()),
        ],
    );
    runtime.refuel();
    runtime
        .try_settle(std::time::Duration::from_secs(5))
        .unwrap_or_else(|error| {
            common::evidence::fail(common::evidence::Failure::Infrastructure, error)
        });

    common::evidence::record(&runtime);
    let lines = runtime.engine_log_from(mark);
    if runtime.engine_log_overflowed().expect("overflow probe") {
        evidence::fail(Failure::Overflow, "engine log wrapped");
    }

    let started = lines.iter().any(|line| line.contains("Call start"));
    assert!(
        started,
        "the engine should start the call: \
             {outcome:?} {lines:?}"
    );

    assert!(
        lines
            .iter()
            .any(|line| line.contains("start_precall begin")),
        "and reach precall setup: {lines:?}"
    );
    // The call is genuinely being built by this point: media is negotiated
    // per participant, per stream. This is the deepest the engine has been
    // driven.
    assert!(
        lines
            .iter()
            .any(|line| line.contains("call_generate_ssrc_for_participant")),
        "media SSRCs should be generated for the participants: {lines:?}"
    );
    for stream in ["audio", "video", "screen_share"] {
        assert!(
            lines
                .iter()
                .any(|line| line.contains(&format!("{stream} stream"))),
            "an SSRC should be generated for the {stream} stream: {lines:?}"
        );
    }
    assert!(
        !lines
            .iter()
            .any(|line| line.contains("peer_participant_jids must be LID")),
        "the JIDs should satisfy the LID requirement: {lines:?}"
    );
    // The call reaches the state a ringing outbound call is in. Everything
    // before this is setup; this is the engine committing to the call.
    assert!(
        lines.iter().any(|line| line.contains("[None -> Calling]")),
        "the call should enter the Calling state: {lines:?}"
    );

    // And then it stops. This pins the blocker rather than endorsing it:
    // `make_and_cache_offer` (offer.cc) returns 70008.
    //
    // 70008 is shared by several guards. This test observes the failure
    // and lack of a send, but does not locate the guard in the pinned capture.
    // Read `make_and_cache_offer` in that capture and mark its call sites
    // with `oracle instrument --calls-in FUNC`; a body marker cannot tell
    // which caller ran. Do not carry old-capture indices into this module.
    //
    // When this assertion starts failing, the blocker is gone: replace it
    // with one that expects an offer on the wire.
    assert!(
        lines
            .iter()
            .any(|line| line.contains("make_and_cache_offer failed: 70008")),
        "the known blocker should still be the one that stops the call; if this \
             fails, the offer path may now work — check for outgoing signaling: {lines:?}"
    );
}

/// The init sequence WhatsApp Web actually performs, and the handles it gets.
///
/// `WAWeb/Voip/Init.js` runs `voipInit` and `setHideMyIp` together and then
/// starts network-medium monitoring, which reaches the engine as
/// `updateNetworkMedium(medium, 0)`. Neither of the latter two was ever called
/// here, and `startJsWorkerThread` — the thing `SctpDataChannelThread.js` is
/// built on, so the base of the P2P data path — had never been exercised at
/// all.
///
/// None of this unblocks an outgoing offer. It is pinned because the surface it
/// covers only became reachable once the worker threads stopped dying, and a
/// regression there would silently take it away again.
#[test]
#[ignore = "brings up the media stack; slow"]
fn the_engine_accepts_the_init_sequence_whatsapp_web_performs() {
    let _serial = common::engine_lock();
    let mut runtime = engine();

    let hide_ip = common::evidence::call(&mut runtime, "setHideMyIp", &[Value::Bool(false)]);
    runtime.refuel();
    assert!(
        hide_ip.is_ok(),
        "setHideMyIp should be accepted: {hide_ip:?}"
    );

    let medium = common::evidence::call(
        &mut runtime,
        "updateNetworkMedium",
        &[Value::Int(1), Value::Int(0)],
    );
    runtime.refuel();
    assert_eq!(
        medium.as_ref().ok().and_then(|value| value.as_int()),
        Some(0),
        "updateNetworkMedium should report success: {medium:?}"
    );

    // The JS worker thread the engine hands back a handle for, and the pthread
    // id behind it — `JsWorkerThread.js` needs both to build its message port.
    let worker = common::evidence::call(&mut runtime, "startJsWorkerThread", &[]);
    runtime.refuel();
    let Some(handle) = worker.as_ref().ok().and_then(|value| value.as_int()) else {
        panic!("startJsWorkerThread should return a handle: {worker:?}");
    };
    assert!(handle != 0, "the handle should be a real pointer");

    let pthread =
        common::evidence::call(&mut runtime, "getJsWorkerPThreadId", &[Value::Int(handle)]);
    runtime.refuel();
    assert!(
        pthread
            .as_ref()
            .ok()
            .and_then(|value| value.as_int())
            .is_some_and(|id| id != 0),
        "the worker should have a pthread id: {pthread:?}"
    );

    // Config the server would have supplied. Empty is the honest answer with no
    // server attached, and worth pinning: it is why `elected_relay_idx` is -1
    // and why no offer can carry a relay.
    let param = common::evidence::call(
        &mut runtime,
        "getVoipParam",
        &[Value::Str("options.caller_timeout".to_owned())],
    );
    runtime.refuel();
    assert_eq!(
        param.as_ref().ok().and_then(|value| value.as_str()),
        Some(""),
        "with no server there is no config to read: {param:?}"
    );
}

/// `getCallInfo` is the only window into the engine's own view of a call.
///
/// The call context reaches the guest through a wasm global, and the module
/// exports no globals, so the host cannot read it. This function takes no
/// arguments and reads that context itself, which makes it the whole of the
/// available observability — worth pinning, because most of this investigation
/// was spent inferring from disassembly what one call reports directly.
///
/// What it reports is also the current puzzle: the participant list *does*
/// carry a `is_self: true` entry for our own LID, yet `make_and_cache_offer`
/// fails because the group's cached self pointer is null.
#[test]
#[ignore = "brings up the media stack; slow"]
fn the_engine_reports_a_self_participant_for_an_outgoing_call() {
    let _serial = common::engine_lock();
    let mut runtime = engine();

    // Nothing before the call: this is what makes the post-call dump evidence
    // about *this* call rather than pre-existing state.
    let before = common::evidence::call(&mut runtime, "getCallInfo", &[]);
    runtime.refuel();
    assert_eq!(
        before.as_ref().ok().and_then(|value| value.as_str()),
        Some(""),
        "with no call in flight the engine should report nothing: {before:?}"
    );

    let _ = common::evidence::call(
        &mut runtime,
        "startVoipCall",
        &[
            Value::Str(PEER_LID.to_owned()),
            Value::StringList(vec![PEER_LID_DEVICE.to_owned()]),
            Value::Str("0011223344556677".to_owned()),
            Value::Bool(false),
            Value::Str(PEER_LID.to_owned()),
            Value::Bool(false),
            Value::Bytes(Vec::new()),
        ],
    );
    runtime.refuel();
    runtime
        .try_settle(std::time::Duration::from_secs(5))
        .unwrap_or_else(|error| {
            common::evidence::fail(common::evidence::Failure::Infrastructure, error)
        });

    let info = common::evidence::call(&mut runtime, "getCallInfo", &[]);
    runtime.refuel();
    let Some(json) = info.as_ref().ok().and_then(|value| value.as_str()) else {
        evidence::fail(Failure::Infrastructure, format!("getCallInfo: {info:?}"));
    };

    assert!(
        json.contains("\"is_self\": true"),
        "the engine should mark one participant as ourselves: {json}"
    );
    assert!(
        json.contains("\"participant_count\": 2"),
        "caller and callee should both be present: {json}"
    );
}

/// The peer's LID. Fictitious, like every identity in these tests.
const PEER_LID: &str = "11223344556677@lid";
const PEER_LID_DEVICE: &str = "11223344556677:0@lid";

/// This build refuses a phone-number peer: calls must be placed to LIDs.
///
/// It matters beyond this repository. `~/projects/meowmeow-node-wasm` places
/// calls against an older module and passes `…@s.whatsapp.net` as the peer, so
/// anyone porting that recipe here will hit this and have no idea why. The
/// engine says it plainly — `peer_participant_jids must be LID, enforce LID for
/// all calls` — and that sentence is the ground truth whatsapp-rust has to
/// match.
#[test]
#[ignore = "real threads; see the module docs"]
fn a_phone_number_peer_is_refused_because_this_build_enforces_lid() {
    let _serial = common::engine_lock();
    let mut runtime = engine();

    let mark = runtime.engine_log().len();
    let outcome = common::evidence::call(
        &mut runtime,
        "startVoipCall",
        &[
            Value::Str("11223344556677@s.whatsapp.net".to_owned()),
            Value::StringList(vec!["11223344556677:0@c.us".to_owned()]),
            Value::Str("0011223344556677".to_owned()),
            Value::Bool(false),
            Value::Str("11223344556677@s.whatsapp.net".to_owned()),
            Value::Bool(false),
            Value::Bytes(Vec::new()),
        ],
    );
    runtime.refuel();
    runtime
        .try_settle(Duration::from_secs(5))
        .unwrap_or_else(|error| {
            common::evidence::fail(common::evidence::Failure::Infrastructure, error)
        });

    common::evidence::record(&runtime);
    let lines = runtime.engine_log_from(mark);
    if runtime.engine_log_overflowed().expect("overflow probe") {
        evidence::fail(Failure::Overflow, "engine log wrapped");
    }

    assert!(
        lines.iter().any(|line| line.contains("must be LID")),
        "the engine should refuse a phone-number peer and say so: {outcome:?} {lines:?}"
    );
}

/// Does the `<voip_settings>` blob in an offer end up where `getVoipParam` reads?
///
/// An outgoing call is blocked by configuration this build no longer takes at
/// init — `getVoipParam("options.*")` answers empty, and an older module got
/// the same values as `initVoipStack` arguments. An incoming offer does carry a
/// settings blob, and the engine parses it (mislabelling it compressed gets the
/// whole offer rejected), so the question is whether handling one leaves those
/// values readable.
///
/// If it does, feeding a synthetic offer is a way to configure the engine
/// without a server. If it does not, settings arrive by some other channel and
/// that is worth knowing before building anything on this idea.
#[test]
#[ignore = "real threads; see the module docs"]
fn an_offers_settings_blob_becomes_readable_through_get_voip_param() {
    let _serial = common::engine_lock();
    let mut runtime = engine();

    let read = |runtime: &mut Runtime, name: &str| -> Option<String> {
        let value = common::evidence::call(runtime, "getVoipParam", &[Value::Str(name.to_owned())]);
        runtime.refuel();
        value
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
    };

    // The blob this test sends sets `enable_48khz_rtp_clock` under `options`,
    // so that is the name to look for — a key we know is in there.
    let name = "options.enable_48khz_rtp_clock";
    let before = read(&mut runtime, name);

    let payload = serialize(&offer_stanza(), true);
    let _ = deliver(&mut runtime, payload);

    let after = read(&mut runtime, name);
    eprintln!("getVoipParam({name}): before={before:?} after={after:?}");

    // It does. An incoming offer configures the engine, and the values it
    // carries are readable afterwards — so a synthetic offer is a way to set
    // voip params without a server, which is what this build otherwise lacks.
    assert_eq!(
        before.as_deref(),
        Some(""),
        "nothing configured before the offer"
    );
    assert_eq!(
        after.as_deref(),
        Some("false"),
        "the offer's settings blob should be readable through getVoipParam"
    );
}

/// The current capture's offer guard is count != 0 || ctx[1568] != 0.
/// Function 12564, body bytes 5385915..5389659, reads that flag at
/// 0x522fef..0x522ff8 and returns 70008 on the zero/zero branch at 0x52300a.
/// Its only direct caller, 11745 at 0x491ec9, passes the global context from
/// *(u32*)1719816 and a destination count assembled at 0x491e21..0x491e3b.
/// The old ctx[3856]/ctx[662166] probe described a different binary and object.
/// Pin both functions and the context getter before reading the current flag.
#[test]
#[ignore = "real threads; see the module docs"]
fn incoming_settings_preserve_the_current_offer_guard_flag() {
    let _serial = threaded_guard();
    let bytes = common::capture(VOIP)
        .expect("capture integrity")
        .unwrap_or_else(|| evidence::fail(Failure::MissingCapture, VOIP));
    for (index, expected) in [
        (
            12564,
            "655f4f886b7614da939113787adcd4570746a8fb953b22c7adf2429ee9d52af5",
        ),
        (
            11745,
            "7c80c7d76bce45a5bcc332d69bdb3ad7f4bd37c77eeeead8504e1db39f758e82",
        ),
        (
            11591,
            "b2957dabb3d7bccbcbb238acef86ac12ca4bde09ae8c360081e30fd37cffa9ae",
        ),
    ] {
        assert_eq!(
            oracle_core::derive::function_body_sha256(&bytes, index).expect("guard selector"),
            expected,
            "capture changed; re-derive the guard and context address together"
        );
    }
    let mut runtime = engine();
    let flag = |runtime: &Runtime| {
        let pointer = runtime
            .read(1_719_816, 4)
            .unwrap_or_else(|error| evidence::fail(Failure::Infrastructure, error));
        let ctx = u32::from_le_bytes(pointer.try_into().expect("four-byte context pointer"));
        if ctx < 0x10000 {
            evidence::fail(Failure::Infrastructure, "no call context");
        }
        runtime
            .read(ctx.checked_add(1568).expect("guard address"), 1)
            .unwrap_or_else(|error| evidence::fail(Failure::Infrastructure, error))[0]
    };
    let before = flag(&runtime);
    let lines = deliver(&mut runtime, serialize(&offer_stanza(), true));
    assert!(
        lines
            .iter()
            .any(|line| line.contains("wa_call_handle_incoming_xmpp_offer() status 0")),
        "the settings observation requires an accepted offer: {lines:?}"
    );
    let after = flag(&runtime);
    eprintln!("oracle guard_offset=1568 before={before} after={after}");
    assert_eq!(before, 0);
    assert_eq!(
        after, before,
        "incoming settings changed the pinned offer guard flag"
    );
}
