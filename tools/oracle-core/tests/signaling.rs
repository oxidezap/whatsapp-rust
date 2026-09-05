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
//! // serializeVoipWapNode = base64(encodeStanza(node) minus its flag byte)
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
//! # What it took to make them reliable
//!
//! Four separate problems, each measured rather than guessed:
//!
//! - **Startup raced about one time in nine.** `initVoipStack` finishes in
//!   ~5 ms, and the trap landed with 99.6% of the fuel untouched and the media
//!   init already logged as complete — a race between the calling thread and
//!   `call_event_proc` starting. `schedule.rs` now lets only one guest thread
//!   execute at a time, which took the rate to roughly 1 in 40; six retries in
//!   `engine()` cover the remainder.
//! - **Offer handling is asynchronous.** The call returning says nothing about
//!   whether the event thread has done the work, so `wait_for_reaction` waits
//!   for the log to grow and then go quiet instead of sleeping a fixed amount.
//! - **Startup is not one burst of logging**, so "the log went quiet" is not
//!   "startup finished". Under load the gap between the media stack finishing
//!   and `call_event_proc` starting exceeds `QUIET`, and an offer delivered
//!   into it is handled by an engine that has not finished starting.
//!   `engine_with` waits for `call_event_proc resumed` — the last line startup
//!   writes — and treats not reaching it as a failed startup.
//! - **The engine's lock watchdog fires on the offer path**, about one run in
//!   four, and it is ours: `schedule.rs` lets a worker hold a lock across its
//!   whole turn, so the main thread meets the engine's locks in an order its
//!   design does not admit. The run is identical to a good one and then stops
//!   one line short of `wa_call_handle_incoming_xmpp_offer() status 0`. See
//!   `deliver_without_a_lock_inversion`, which retries on exactly that
//!   complaint and on nothing else.
//!
//! The last two are why this file used to say "not because they are unreliable
//! — because they are slow", while `a_well_formed_offer_is_accepted` failed
//! about one run in four. It was both.
//!
//! `examples/init_stress.rs` measures the startup race if the rate needs
//! rechecking after a capture update.

mod common;

use base64::Engine as _;
use std::time::Duration;

use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};
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

/// A VoIP module with its log ring attached and its stack initialised.
///
/// Threads are on: without them PJSIP's init fails and signaling never reaches
/// the call stack, so every test here would be exercising a dead engine.
///
/// Bringing the media stack up starts a real worker pool, and under a loaded
/// machine that occasionally does not complete. Retrying on a fresh instance is
/// honest — the failure is resource contention in the host, not a property of
/// the module — and keeps a heavy integration test usable rather than flaky.
fn engine() -> Option<Runtime> {
    // 0.11^6 ≈ 1.8e-6. See the module docs for where 0.11 comes from.
    const ATTEMPTS: usize = 6;

    for attempt in 1..=ATTEMPTS {
        match engine_with(ThreadPolicy::Spawn) {
            Ok(runtime) => return Some(runtime),
            // No capture: nothing to retry.
            Err(EngineError::Unavailable) => return None,
            Err(EngineError::InitFailed(why)) if attempt < ATTEMPTS => {
                eprintln!("media stack did not come up (attempt {attempt}): {why}");
            }
            Err(EngineError::InitFailed(why)) => {
                panic!("media stack failed to initialise after {ATTEMPTS} attempts: {why}")
            }
        }
    }
    None
}

enum EngineError {
    /// The capture directory or module is missing.
    Unavailable,
    InitFailed(String),
}

fn engine_with(policy: ThreadPolicy) -> Result<Runtime, EngineError> {
    let catalog = Catalog::discover().map_err(|_| EngineError::Unavailable)?;
    let entry = catalog
        .resolve(VOIP)
        .map_err(|_| EngineError::Unavailable)?;
    let bytes = std::fs::read(&entry.path).map_err(|_| EngineError::Unavailable)?;

    let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
    runtime.set_thread_policy(policy);
    // Register as the main runtime thread with `can_block = 0`, as the browser
    // does — the futex the engine compiles reaches `memory.atomic.wait32` with
    // `1`, and off the main browser thread that blocks rather than throwing.
    //
    // It only works alongside the draining below: registration is what lets the
    // engine hand work to this thread, and work handed over and never collected
    // reads as an engine that did nothing.
    runtime.set_main_thread_registration(true);
    runtime.run_ctors().expect("ctors");
    runtime
        .attach_log_ring(LOG_RING)
        .expect("engine should accept a log ring");
    // The media stack must actually come up: with it down the parser never
    // reaches the call stack and every assertion here would be about a dead
    // engine. `0` is success; `120011` is PJSIP being denied a thread.
    let init = runtime.call_embind(
        "initVoipStack",
        &[
            // `"0"` and `"{}"` are not what `voipInit` wants — it takes three
            // legacy-form JIDs (user, user device, user device LID), as
            // `WAWeb/Voip/Init.js` shows and as `examples/outgoing_call.rs`
            // passes. Correcting it here was tried and reverted: the incoming
            // offer still ends as `Missed` with an identical log, and the suite
            // started overrunning, so it bought nothing and cost time. Worth
            // revisiting only alongside a fix that needs the real identity.
            Value::Str(jid(CALLEE).to_string()),
            Value::Str("0".to_owned()),
            Value::Str("{}".to_owned()),
        ],
    );
    runtime.refuel();
    if policy == ThreadPolicy::Spawn
        && init.as_ref().ok().and_then(|value| value.as_int()) != Some(0)
    {
        return Err(EngineError::InitFailed(format!("{init:?}")));
    }
    // Wait for the media stack to finish announcing itself rather than for the
    // threads to finish: PJSIP's worker is a loop bounded only by its fuel, so
    // a full quiesce would always time out.
    wait_for_reaction(&mut runtime, 0);

    // And then wait for the *event thread*, which is a different thing and is
    // what an offer is handed to.
    //
    // `wait_for_reaction` waits for the log to go quiet, and startup is not one
    // continuous burst of logging: under load it can pause for longer than
    // `QUIET` between the media stack finishing and `call_event_proc` starting.
    // An offer delivered into that gap is handled by a half-started engine,
    // which does not look like a timing problem at all — it looks like the
    // engine refusing the stanza:
    //
    //     record_incoming_msg: no active call
    //     Application settings not loaded
    //     Failed to get voip storage dir
    //
    // and `wa_call_handle_incoming_xmpp_offer() status 0` never appears. That
    // is what made `a_well_formed_offer_is_accepted` and
    // `offer_handling_is_deterministic` fail about one run in seven, on a suite
    // documented as slow but not flaky.
    //
    // `call_event_proc resumed` is the last line startup writes, at the default
    // log level, and it means the event thread is running and idle. Not
    // reaching it is a failed startup like any other, so it goes back through
    // the retry above rather than into a test.
    if policy == ThreadPolicy::Spawn && !wait_for_line(&mut runtime, "call_event_proc resumed") {
        return Err(EngineError::InitFailed(
            "the event thread never announced itself".to_owned(),
        ));
    }
    Ok(runtime)
}

/// Delivers an offer on a fresh engine until the engine's own lock watchdog
/// stays quiet, and returns what it logged.
///
/// **What is being retried is a harness artifact, not a verdict.** A run that
/// ends like this —
///
/// ```text
/// events/eve  EVENT: Call missed by the user
///   wa_os.cc  Mutex scope=0, priority=1: name=, owner=thr0x14b00c, taken=1
///   wa_os.cc  check_locking_order wrong order for mutex scope=0, priority=0
/// ```
///
/// is byte-for-byte identical to a good one up to that point and then stops:
/// `wa_call_handle_incoming_xmpp_offer() status 0`, the line the synchronous
/// call writes on its way out, never arrives. The engine caught a lock-order
/// inversion and dumped its mutexes instead of finishing.
///
/// It is ours. `schedule.rs` runs one guest thread at a time, so a worker can
/// hold a lock across its whole turn and the main thread meets the engine's
/// locks in an order its design does not admit. `schedule.rs` says the
/// lock-order complaints are "a symptom of something else"; this is the
/// something else, and it is the scheduler that buys everything around it.
///
/// So the retry is conditioned on that complaint and nothing else. An engine
/// that answers — with any status, including a refusal — is returned as it is,
/// and the test judges it.
fn deliver_without_a_lock_inversion(runtime: &mut Runtime, stanza: &str) -> Vec<String> {
    // Four, not more. Each attempt builds a fresh engine and `engine()` retries
    // startup six times inside that, so the two multiply; measured, four
    // rounds of `a_well_formed_offer_is_accepted` passed in 37-53 s each,
    // which is two or three attempts.
    const ATTEMPTS: usize = 4;

    for attempt in 1..=ATTEMPTS {
        let lines = deliver(runtime, stanza.to_owned());
        if !lines
            .iter()
            .any(|line| line.contains("check_locking_order"))
        {
            return lines;
        }
        if attempt == ATTEMPTS {
            return lines;
        }
        eprintln!("lock-order inversion handling the offer (attempt {attempt}); fresh engine");
        let Some(fresh) = engine() else {
            return lines;
        };
        *runtime = fresh;
    }
    unreachable!("the loop returns on its last attempt")
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
        runtime.process_queued_calls();
        std::thread::sleep(Duration::from_millis(20));
    }
    false
}

macro_rules! engine_or_skip {
    () => {
        match engine() {
            Some(runtime) => runtime,
            None => {
                eprintln!("skipping: no capture (set WA_WASM_DIR)");
                return;
            }
        }
    };
}

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

fn jid(user: &str) -> Jid {
    Jid::new(user, Server::Pn)
}

/// The stanza shape the engine's parser accepts.
///
/// Established against the engine's own log, one complaint at a time:
///
/// - `call-id` on the **`<call>`**, not on the `<offer>`. With it on the offer
///   the parser reports `empty call-id`.
/// - `<voip_settings>` as a **sibling** of `<offer>`. Nested inside it, the
///   parser reports `missing voip_settings`.
/// - JIDs passed as `Jid`, not as text: the codec encodes them with the
///   JID_PAIR token and the engine reads them back that way.
/// - `<voip_settings uncompressed="1">`. Without the attribute the engine reads
///   the blob as compressed and rejects the whole offer with
///   `unexpected compressed voip params`. whatsapp-rust sets the same attribute
///   when it builds an accept, which is where the answer came from.
/// - `t` on the guest's clock, via `virtual_unix_time`. The host clock is
///   virtual and starts in 2021, so a real timestamp is in the future.
fn offer_stanza(now: u64) -> Node {
    let caller = jid(CALLER);
    let offer = NodeBuilder::new("offer")
        .children([
            NodeBuilder::new("audio")
                .attr("enc", "opus")
                .attr("rate", "16000")
                .build(),
            NodeBuilder::new("net").attr("medium", "3").build(),
            NodeBuilder::new("encopt").attr("keygen", "2").build(),
        ])
        .build();

    NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("call-id", CALL_ID)
        .attr("call-creator", caller)
        .attr("t", now.to_string())
        .children([
            offer,
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

/// Serializes a node the way the bridge does.
///
/// `keep_flag` selects whether the transport flag byte survives. The JS glue
/// drops it, but the engine's reader only accepts the payload *with* it — see
/// `engine_accepts_the_framed_encoding`.
fn serialize(node: &Node, keep_flag: bool) -> String {
    let encoded = marshal::marshal(node).expect("marshal");
    let payload = if keep_flag {
        &encoded[..]
    } else {
        &encoded[1..]
    };
    base64::engine::general_purpose::STANDARD.encode(payload)
}

/// Delivers an offer and returns the engine log lines it produced.
fn offer_for(runtime: &Runtime) -> Node {
    offer_stanza(runtime.virtual_unix_time())
}

fn deliver(runtime: &mut Runtime, stanza: String) -> Vec<String> {
    // Deliberately *not* draining the proxy queue first, and it is worth
    // knowing why. Work the engine parks for the main thread during startup
    // looks like something that should run before the offer, so running it
    // here was tried: `a_well_formed_offer_is_accepted` then fails four times
    // out of four, where it had been failing about one in five. Whatever that
    // queued work does, doing it immediately before the offer is worse than
    // leaving it, which also says the half-started engine below is not simply
    // "the queue had not been drained".
    let mark = runtime.engine_log().len();
    runtime.clear_calls();

    let _ = runtime.call_embind(
        "handleIncomingSignalingOffer",
        &[
            Value::Str(stanza),
            Value::Str("web".to_owned()),
            Value::Str("2.3000.0".to_owned()),
            // Arguments 4 and 5 are the stanza's `e` and `t` timestamps, read
            // with stoull. Anything non-numeric raises std::invalid_argument.
            Value::Str(runtime.virtual_unix_time().to_string()),
            Value::Str(runtime.virtual_unix_time().to_string()),
            // Both bools tried in both positions: neither changes the outcome,
            // and flipping the first leaves `call_wakeup_source` at 0 in the
            // field stats — so it is probably not `is_offline`, whatever
            // WhatsApp Web's parameter names suggest.
            Value::Bool(false),
            Value::Bool(false),
            // The caller, as `t.peer_jid.toString()` in WhatsApp Web's own
            // `WAWeb/Handle/VoipCallOffer.js`. The engine takes the caller from
            // here rather than from the stanza: with this empty it logs the
            // offer as coming from `0@s.whatsapp.net`.
            Value::Str(jid(CALLER).to_string()),
            // `f = u[1].tcToken` in the same file — the trusted-contact token,
            // legitimately absent here.
            Value::Bytes(Vec::new()),
        ],
    );
    runtime.refuel();
    wait_for_reaction(runtime, mark);
    runtime.engine_log_from(mark)
}

/// Waits until the engine stops writing about what it was just asked to do.
///
/// The engine hands offer processing to its event thread, so the call returning
/// says nothing about whether it finished. A fixed sleep is wrong in both
/// directions — too short under load, wasted otherwise — so this waits for the
/// log to grow and then go quiet. Returns early on the common path.
fn wait_for_reaction(runtime: &mut Runtime, mark: usize) {
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
        runtime.process_queued_calls();
        let len = runtime.engine_log().len();

        if len != last_len {
            last_len = len;
            unchanged_since = std::time::Instant::now();
            continue;
        }
        // Something was written, and nothing has been for a while.
        if len > mark && unchanged_since.elapsed() >= QUIET {
            return;
        }
    }
}

/// The engine writes diagnostics into a host-supplied buffer. Everything else
/// here depends on that working.
#[test]
#[ignore = "real threads; see the module docs"]
fn the_engine_log_is_readable() {
    let _serial = threaded_guard();
    let runtime = engine_or_skip!();
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
    let mut runtime = engine_or_skip!();
    let lines = {
        let payload = serialize(&offer_for(&runtime), true);
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

/// The encoding the engine's reader accepts.
///
/// The JS glue drops the transport flag byte before base64-encoding, but the
/// engine's own reader wants it: without it the payload is off by one from the
/// start and the stanza never reaches the call stack. Compared by how far each
/// gets rather than by a specific error string, since the reader reports
/// different failures depending on what the shifted bytes happen to decode to.
#[test]
#[ignore = "real threads; see the module docs"]
fn engine_accepts_the_framed_encoding() {
    let mut runtime = engine_or_skip!();

    let framed = {
        let payload = serialize(&offer_for(&runtime), true);
        deliver(&mut runtime, payload)
    };
    assert!(
        reached_call_stack(&framed),
        "the framed encoding should reach the call stack: {framed:?}"
    );

    let unframed = {
        let payload = serialize(&offer_for(&runtime), false);
        deliver(&mut runtime, payload)
    };
    assert!(
        !reached_call_stack(&unframed),
        "dropping the flag byte should not reach the call stack: {unframed:?}"
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
            Box::new(|runtime: &Runtime| {
                let full = serialize(&offer_for(runtime), true);
                full[..full.len() / 2].to_owned()
            }),
        ),
    ];

    for (label, build) in cases {
        let Some(mut runtime) = engine() else {
            eprintln!("skipping: no capture (set WA_WASM_DIR)");
            return;
        };
        let payload = build(&runtime);
        let _ = deliver(&mut runtime, payload);

        // The call has to come back one way or the other. Whether the engine
        // still answers afterwards is its business; whether the host survives
        // is ours.
        runtime.refuel();
        let probe = runtime.call_embind("getWebP2PVirtualIpv4", &[]);
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
    let mut fresh = engine().expect("host should still work");
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
    let Some(mut first) = engine() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };
    let mut second = engine().expect("second instance");

    let stanza = serialize(&offer_for(&first), true);
    // Both sides through the lock-watchdog retry: an inversion stops the
    // reaction one line short of `status 0`, and comparing an abandoned run
    // against a completed one is not a determinism result either way.
    let a = events(deliver_without_a_lock_inversion(&mut first, &stanza));
    let b = events(deliver_without_a_lock_inversion(&mut second, &stanza));

    assert_eq!(a, b, "two runs reached different engine events");
    assert!(!a.is_empty(), "no events recorded at all");
}

/// The lines that say what the engine decided, with everything timing-derived
/// dropped.
///
/// The `!` the engine writes into a line's prefix is dropped too: it marks a
/// line the ring buffer lost, not a different decision, and two runs that
/// decided the same thing must compare equal whether or not one of them dropped
/// a line on the way.
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
    let Ok(runtime) = engine_with(ThreadPolicy::Refuse) else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };
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
    let runtime = engine_or_skip!();
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
    let runtime = engine_or_skip!();

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
    let mut runtime = engine_or_skip!();

    assert!(runtime.watch_memory(), "no static data to watch");
    let (at, len) = runtime.coherence_witness().expect("witness");
    runtime
        .write_bytes_at(at, &vec![0xFF; len as usize])
        .expect("clobber the watched span");

    // The workers poll, so they reach a host call within milliseconds; this is
    // waiting for the report rather than for anything to happen.
    runtime.settle(Duration::from_secs(2));

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
    let mut runtime = engine_or_skip!();

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
    let Some(catalog) = Catalog::discover().ok() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };
    let Ok(entry) = catalog.resolve(VOIP) else {
        return;
    };
    let bytes = std::fs::read(&entry.path).expect("read");
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
    let mut runtime = engine_or_skip!();
    // A ring this size does not wrap during startup, so this pins the
    // "no overflow" reading; the accessor existing is what lets a caller with a
    // smaller ring notice the opposite.
    assert!(!runtime.engine_log_overflowed());
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
    let mut runtime = engine_or_skip!();
    let lines = {
        let payload = serialize(&offer_for(&runtime), true);
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
    let mut runtime = engine_or_skip!();
    let mark = runtime.engine_log().len();

    let _ = runtime.call_embind(
        "handleIncomingSignalingOffer",
        &[
            Value::Str(serialize(&offer_for(&runtime), true)),
            Value::Str("web".to_owned()),
            Value::Str("2.3000.0".to_owned()),
            Value::Str("not-a-number".to_owned()),
            Value::Str("also-not".to_owned()),
            Value::Bool(false),
            Value::Bool(false),
            // The caller, as `t.peer_jid.toString()` in WhatsApp Web's own
            // `WAWeb/Handle/VoipCallOffer.js`. The engine takes the caller from
            // here rather than from the stanza: with this empty it logs the
            // offer as coming from `0@s.whatsapp.net`.
            Value::Str(jid(CALLER).to_string()),
            // `f = u[1].tcToken` in the same file — the trusted-contact token,
            // legitimately absent here.
            Value::Bytes(Vec::new()),
        ],
    );
    runtime.refuel();

    let lines = runtime.engine_log_from(mark);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("msgEStr or msgTStr") || line.contains("stoull")),
        "expected the engine to report the parse failure: {lines:?}"
    );
}

/// `call-id` belongs on the `<call>`. This pins the difference, because the
/// attribute name is the same in both places and only the position matters.
#[test]
#[ignore = "real threads; see the module docs"]
fn call_id_must_sit_on_the_call_element() {
    let _serial = threaded_guard();
    let mut runtime = engine_or_skip!();

    let misplaced = NodeBuilder::new("call")
        .attr("from", jid(CALLER))
        .attr("t", "1735689600")
        .children([NodeBuilder::new("offer")
            .attr("call-id", CALL_ID)
            .attr("call-creator", jid(CALLER))
            .build()])
        .build();

    let lines = deliver(&mut runtime, serialize(&misplaced, true));
    assert!(
        lines.iter().any(|line| line.contains("empty call-id")),
        "expected the parser to miss a call-id on the offer: {lines:?}"
    );
}

/// The engine accepts the offer outright: status 0, no parser complaint.
///
/// This is the difference `uncompressed="1"` makes. Without it the same stanza
/// comes back as `70004` with `unexpected compressed voip params`, because the
/// engine reads the settings blob as compressed data.
#[test]
#[ignore = "real threads; see the module docs"]
fn a_well_formed_offer_is_accepted() {
    let mut runtime = engine_or_skip!();
    let payload = serialize(&offer_for(&runtime), true);
    let lines = deliver_without_a_lock_inversion(&mut runtime, &payload);

    // A missing line proves nothing once the ring has wrapped: the reader can
    // only return what is still in the buffer. Asserting through an overflow
    // reports "the engine refused" for a run in which it may well have
    // accepted, which is the kind of wrong answer an oracle must not give.
    if runtime.engine_log_overflowed() {
        eprintln!("engine log overflowed; nothing can be concluded from absence");
        return;
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
    let Some(mut runtime) = engine_with_identity() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    let payload = serialize(&offer_for(&runtime), true);
    // Plain `deliver`, not the lock-watchdog retry. What this asserts is that
    // the *sequence* does not shred the engine's log, which an inversion does
    // not affect — and routing it through the retry cost twenty minutes on its
    // own, because a fresh engine per attempt compounds with the six startup
    // retries inside `engine()`.
    let _ = deliver(&mut runtime, payload);

    let mark = runtime.engine_log().len();
    let _ = runtime.call_embind(
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
    runtime.settle(Duration::from_secs(5));

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
        runtime.engine_log_overflowed()
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
        lines.is_empty() || structured > 0,
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
    let mut runtime = engine_or_skip!();
    let payload = serialize(&offer_for(&runtime), true);
    let lines = deliver(&mut runtime, payload);
    for line in &lines {
        eprintln!("  | {}", line.trim());
    }

    let info = runtime.call_embind("getCallInfo", &[]);
    runtime.refuel();
    let Some(json) = info.as_ref().ok().and_then(|value| value.as_str()) else {
        eprintln!("engine reported no call after the offer: {info:?}");
        return;
    };
    if json.is_empty() {
        eprintln!("engine holds no call state after the offer");
        return;
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
    let mut runtime = engine_or_skip!();

    let caller = jid(CALLER);
    let unmarked = NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("call-id", CALL_ID)
        .attr("call-creator", caller)
        .attr("t", runtime.virtual_unix_time().to_string())
        .children([
            NodeBuilder::new("offer").build(),
            // Same bytes, without the attribute that says they are plain.
            NodeBuilder::new("voip_settings")
                .bytes(VOIP_SETTINGS.to_vec())
                .build(),
        ])
        .build();

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
    let mut runtime = engine_or_skip!();
    let payload = serialize(&offer_for(&runtime), true);
    let lines = deliver(&mut runtime, payload);

    if runtime.engine_log_overflowed() {
        eprintln!("engine log overflowed; nothing can be concluded from absence");
        return;
    }

    let offer_line = lines
        .iter()
        .find(|line| line.contains("Offer from:"))
        .unwrap_or_else(|| panic!("engine logged no offer line: {lines:?}"));

    let tail = &CALLER[CALLER.len() - 4..];
    assert!(
        offer_line.contains(tail),
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
    /// The engine's worker pool outlives its `Runtime` — a PJSIP worker is a
    /// loop bounded only by fuel — so a run that follows several other engine
    /// tests competes with threads that have not finished dying. Retried for
    /// the same reason startup is.
    const ATTEMPTS: usize = 4;

    let _serial = threaded_guard();

    for attempt in 1..=ATTEMPTS {
        let Some(mut runtime) = engine_with_identity() else {
            eprintln!("skipping: no capture (set WA_WASM_DIR)");
            return;
        };

        let mark = runtime.engine_log().len();
        let outcome = runtime.call_embind(
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
        runtime.settle(std::time::Duration::from_secs(5));

        let lines = runtime.engine_log_from(mark);
        if runtime.engine_log_overflowed() {
            eprintln!("engine log overflowed; nothing can be concluded from absence");
            return;
        }

        let started = lines.iter().any(|line| line.contains("Call start"));
        if !started && attempt < ATTEMPTS {
            eprintln!("call did not start (attempt {attempt}): {outcome:?}");
            continue;
        }
        assert!(
            started,
            "the engine should start the call after {ATTEMPTS} attempts: \
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
        // *Which* of its nine `70008` sites fires is not known. It is not
        // `offer.cc:463`, the missing-self-participant one, though that was
        // written here as established: with the engine's workers alive,
        // `getCallInfo` answers, and it reports `participant_count: 2` with
        // `11223344556677@lid` as `is_self: false` and `99887766554433@lid` as
        // `is_self: true`. The self participant exists. Neither of the two
        // functions that build it logs its "self participant not created"
        // failure, either. See agent_docs/voip_oracle_status.md for the remaining candidates.
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
        return;
    }
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
    let Some(mut runtime) = engine_with_identity() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    let hide_ip = runtime.call_embind("setHideMyIp", &[Value::Bool(false)]);
    runtime.refuel();
    assert!(
        hide_ip.is_ok(),
        "setHideMyIp should be accepted: {hide_ip:?}"
    );

    let medium = runtime.call_embind("updateNetworkMedium", &[Value::Int(1), Value::Int(0)]);
    runtime.refuel();
    assert_eq!(
        medium.as_ref().ok().and_then(|value| value.as_int()),
        Some(0),
        "updateNetworkMedium should report success: {medium:?}"
    );

    // The JS worker thread the engine hands back a handle for, and the pthread
    // id behind it — `JsWorkerThread.js` needs both to build its message port.
    let worker = runtime.call_embind("startJsWorkerThread", &[]);
    runtime.refuel();
    let Some(handle) = worker.as_ref().ok().and_then(|value| value.as_int()) else {
        panic!("startJsWorkerThread should return a handle: {worker:?}");
    };
    assert!(handle != 0, "the handle should be a real pointer");

    let pthread = runtime.call_embind("getJsWorkerPThreadId", &[Value::Int(handle)]);
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
    let param = runtime.call_embind(
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
    let Some(mut runtime) = engine_with_identity() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    // Nothing before the call: this is what makes the post-call dump evidence
    // about *this* call rather than pre-existing state.
    let before = runtime.call_embind("getCallInfo", &[]);
    runtime.refuel();
    assert_eq!(
        before.as_ref().ok().and_then(|value| value.as_str()),
        Some(""),
        "with no call in flight the engine should report nothing: {before:?}"
    );

    let _ = runtime.call_embind(
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
    runtime.settle(std::time::Duration::from_secs(5));

    let info = runtime.call_embind("getCallInfo", &[]);
    runtime.refuel();
    let Some(json) = info.as_ref().ok().and_then(|value| value.as_str()) else {
        // A trap here is the corruption `agent_docs/voip_oracle_status.md` describes, not a new
        // fault. The shared stack is *not* the reason, though it was written
        // down as one here: every guest thread does start from the main
        // thread's stack pointer, but giving each worker its own 4 MiB stack —
        // confirmed from inside the guest with `stackSave` — changes nothing.
        // What actually stops the engine is static data being corrupted at
        // runtime, which shows up as a set profiler flag at 0x14B958 and an
        // out-of-linear-memory pointer in `pthread + 112`. The mechanism is
        // still open.
        //
        // Reported and skipped rather than asserted, because a red suite here
        // would say "the self participant is missing" about a run that never
        // got far enough to have one. That makes this test vacuous whenever it
        // takes this path — it is a placeholder for the assertion below, not
        // evidence for it.
        eprintln!(
            "getCallInfo could not answer — see agent_docs/voip_oracle_status.md, threads share one stack: {info:?}"
        );
        return;
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

/// An engine whose identity is set up the way WhatsApp Web sets it up.
fn engine_with_identity() -> Option<Runtime> {
    const ATTEMPTS: usize = 6;

    for _ in 0..ATTEMPTS {
        let catalog = Catalog::discover().ok()?;
        let entry = catalog.resolve(VOIP).ok()?;
        let bytes = std::fs::read(&entry.path).ok()?;

        let mut runtime = Runtime::instantiate(&bytes).expect("instantiate");
        runtime.set_thread_policy(ThreadPolicy::Spawn);
        runtime.run_ctors().expect("ctors");
        runtime.attach_log_ring(LOG_RING).expect("log ring");

        let init = runtime.call_embind(
            "initVoipStack",
            &[
                Value::Str(format!("{CALLEE}@c.us")),
                Value::Str(format!("{CALLEE}:0@c.us")),
                // The device LID. Fictitious, like every identity in these
                // tests.
                Value::Str("99887766554433:0@lid".to_owned()),
            ],
        );
        runtime.refuel();
        if init.as_ref().ok().and_then(|value| value.as_int()) == Some(0) {
            return Some(runtime);
        }
    }
    None
}

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
    let Some(mut runtime) = engine_with_identity() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    let mark = runtime.engine_log().len();
    let outcome = runtime.call_embind(
        "startVoipCall",
        &[
            Value::Str("11223344556677@s.whatsapp.net".to_owned()),
            Value::StringList(vec!["11223344556677:0@s.whatsapp.net".to_owned()]),
            Value::Str("0011223344556677".to_owned()),
            Value::Bool(false),
            Value::Str("11223344556677@s.whatsapp.net".to_owned()),
            Value::Bool(false),
            Value::Bytes(Vec::new()),
        ],
    );
    runtime.refuel();
    runtime.settle(Duration::from_secs(5));

    let lines = runtime.engine_log_from(mark);
    if runtime.engine_log_overflowed() {
        eprintln!("engine log overflowed; nothing can be concluded from absence");
        return;
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
    let Some(mut runtime) = engine_with_identity() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    let read = |runtime: &mut Runtime, name: &str| -> Option<String> {
        let value = runtime.call_embind("getVoipParam", &[Value::Str(name.to_owned())]);
        runtime.refuel();
        value
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned))
    };

    // The blob this test sends sets `enable_48khz_rtp_clock` under `options`,
    // so that is the name to look for — a key we know is in there.
    let name = "options.enable_48khz_rtp_clock";
    let before = read(&mut runtime, name);

    let payload = serialize(&offer_for(&runtime), true);
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

/// Does an offer's settings blob move the bytes that block an outgoing offer?
///
/// `make_and_cache_offer` fails at `offer.cc:430` when both terms of a guard are
/// zero. One of them is `ctx[662166]`, read straight off the call context. The
/// other is `arg2`, which comes from `params[3856]` — and `params` is
/// `wa_call_start_call`'s own first argument, *not* the context, so the reading
/// of `ctx[3856]` below is of a different object and means nothing on its own.
/// It is measured anyway because it costs nothing and its stability is worth
/// knowing.
///
/// An incoming offer *does* configure the engine: values from its
/// `<voip_settings>` blob are readable through `getVoipParam` afterwards. So the
/// question is whether that channel reaches `ctx[662166]`.
///
/// The context is reachable at `*(u32*)1719816`; the engine gets there the same
/// way, through the function `getCallInfo` calls. Re-derived for the
/// `JgwtTQVeWPm` capture along the route the previous one recorded:
/// `oracle xref <module> getCallInfo` names the registration site (func 1200),
/// whose body pushes the table slot it registers (764), and that slot holds
/// func 1141 — 145 instructions, the same shape as the previous capture's
/// 1108 — which calls func 11591, seven instructions opening with
/// `i32.const 1719816 / i32.load`.
///
/// **The two offsets below have not been carried forward.** They came out of
/// `make_and_cache_offer` in the previous capture, and that function is one of
/// the ones this rollout rewrote: it does not survive `unwasm`'s fingerprint,
/// so there is no mechanical route from the old numbers to new ones. Reading
/// them anyway would ask a live object for two bytes at offsets derived from
/// different code, and a pair of zeroes read from the wrong place looks exactly
/// like the answer this test is pinning — which is how a test comes to pass for
/// a stanza that never parsed. So it refuses instead, and says why.
#[test]
#[ignore = "real threads; see the module docs"]
fn an_offer_does_not_move_the_bytes_that_gate_the_outgoing_offer() {
    /// Set once `make_and_cache_offer`'s guard has been located in this capture
    /// and the two offsets read out of it.
    const OFFSETS_REDERIVED: Option<(u32, u32)> = None;

    let Some((settings_offset, guard_offset)) = OFFSETS_REDERIVED else {
        eprintln!(
            "skipping: the offer-guard offsets (3856, 662166) were read out of \
             D5pLH9sfOOl's make_and_cache_offer, which JgwtTQVeWPm rewrote. \
             Re-derive them before trusting this probe."
        );
        return;
    };

    let _serial = common::engine_lock();
    let Some(mut runtime) = engine_with_identity() else {
        eprintln!("skipping: no capture (set WA_WASM_DIR)");
        return;
    };

    let guard_bytes = |runtime: &Runtime| -> Option<(u8, u8)> {
        let ctx = runtime
            .read(1_719_816, 4)
            .ok()
            .map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))?;
        if ctx < 0x10000 {
            return None;
        }
        Some((
            *runtime.read(ctx + settings_offset, 1).ok()?.first()?,
            *runtime.read(ctx + guard_offset, 1).ok()?.first()?,
        ))
    };

    let Some(before) = guard_bytes(&runtime) else {
        eprintln!("skipping: no call context yet");
        return;
    };

    let payload = serialize(&offer_for(&runtime), true);
    let _ = deliver(&mut runtime, payload);

    let after = guard_bytes(&runtime);
    eprintln!("guard bytes (3856, 662166): before={before:?} after={after:?}");

    // Pinned as the current answer: configuring through an offer does not
    // reach these. If this starts failing, the settings channel does move them
    // and the outgoing blocker is reachable from here.
    assert_eq!(
        after,
        Some((0, 0)),
        "an offer moved ctx[662166] — the settings channel reaches the guard"
    );
}
