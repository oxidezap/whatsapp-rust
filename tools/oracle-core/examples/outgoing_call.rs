//! Does the engine's outbound signaling channel work at all?
//!
//! An incoming offer is accepted and then dropped as `Missed`, and nothing
//! reaches `sendSignalingXMPP_js_sync`. That leaves two very different
//! explanations: the engine decided not to answer, or the channel itself never
//! carries anything. Originating a call separates them — an outgoing call
//! *must* put an offer on the wire, so if this produces one the channel is
//! fine and the incoming path is the problem.
//!
//! Arguments follow WhatsApp Web's own call, in `WAWeb/Voip/StackInterfaceWeb.js`:
//!
//! ```js
//! s.startVoipCall(e.toString({legacy: true}), u /* StringList */, n, r, a, i, c)
//! ```
//!
//! and `WAWeb/Voip/StartCall.js` supplies `r` as the video flag, `a` as a second
//! legacy JID, and the call id as `"00" + randomHex(16).substr(2)`. Note
//! `legacy: true` — the `@c.us` form, not `@s.whatsapp.net`.
//!
//! The participant list is the peer's *devices* — `pe(R, "callStart")` filters
//! companion devices out of a device list — and an empty one is why this used
//! to return `-1` with no log at all. Filled in, the engine starts the call:
//! `Call start, call_role 1, num_peers 1` and `ACTION start_precall`.
//!
//! Each variant gets a fresh instance: the engine keeps call state, and a
//! second start on a used one reports `init_local_state - Call context has
//! already been initialized` and fails with 70008, which answers a different
//! question.
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};

/// `WAWeb/Voip/Init.js` calls `voipInit` with three legacy-form JIDs: the
/// user, the user's device, and the user's **device LID**. The third was being
/// passed as `"{}"` — a settings blob it never was — which is what
/// `get_app_jids: wa_call_device_jid_from_string failed` and `Failed to fetch
/// typed self lid device jid` were reporting.
/// Where the bytecode probes write. Above anything the engine uses, because
/// the host grows memory out to it — which only works since `grow_memory`
/// learned to grow an *imported* memory. Confirmed by reading it back.
const SCRATCH: u32 = 0x2FF_0000;

const SELF: &str = "15550002222@c.us";
const SELF_DEVICE: &str = "15550002222:0@c.us";
/// A LID is a separate identity namespace; the device form carries `:<device>`.
const SELF_LID: &str = "99887766554433:0@lid";
/// The peer's LID. The engine enforces LID for every call:
/// `start_precall peer_participant_jids must be LID`.
const PEER_LID: &str = "11223344556677@lid";
const PEER_LID_DEVICE: &str = "11223344556677:0@lid";
/// Sixteen hex characters, the shape WhatsApp Web generates.
const CALL_ID: &str = "0011223344556677";

/// Brings the engine up, retrying a startup that fails.
///
/// `initVoipStack` fails intermittently — the same flakiness `engine_with_identity`
/// in `tests/signaling.rs` has always retried around. Without this, one bad
/// startup aborts the whole example and every shape after it goes unmeasured,
/// which reads as "the shape was not tried" rather than "the engine did not
/// start".
fn engine(bytes: &[u8]) -> anyhow::Result<Runtime> {
    const ATTEMPTS: usize = 6;

    let mut last = None;
    for attempt in 1..=ATTEMPTS {
        match engine_once(bytes) {
            Ok(runtime) => return Ok(runtime),
            Err(error) => {
                println!("   engine startup attempt {attempt}/{ATTEMPTS} failed: {error:#}");
                last = Some(error);
            }
        }
    }
    Err(last.unwrap_or_else(|| anyhow::anyhow!("engine startup failed")))
}

fn engine_once(bytes: &[u8]) -> anyhow::Result<Runtime> {
    let mut runtime = Runtime::instantiate(bytes)?;
    runtime.set_thread_policy(ThreadPolicy::Spawn);
    // Register as emscripten's main runtime thread, the way the browser does.
    //
    // This was off because turning it on was measured breaking startup — but
    // that predates passing `can_block = 0`, which is the value WhatsApp Web
    // itself uses off the main browser thread. Sigilo, which runs WA's own JS
    // glue under Bun, documents why: with `can_block = 1` the compiled futex
    // reaches `memory.atomic.wait32`, and outside a browser that blocks forever
    // instead of throwing.
    //
    // With it on the run is *deterministic* — 167 engine-log lines every time,
    // against a baseline that wandered between 24 and 202 — and the proxy queue
    // has a thread that may drain it, which is where the engine dispatches
    // outgoing signaling. The offer still fails; this is not that fix.
    //
    // It is a trade, not a free win: turning it on in `tests/signaling.rs` made
    // `a_well_formed_offer_is_accepted` and
    // `the_engine_reports_a_self_participant_for_an_outgoing_call` trap on the
    // `getCallInfo` path, so that was reverted. Whatever registration changes
    // for the better here, it changes something else for the worse there.
    runtime.set_main_thread_registration(true);
    while runtime.memory_size() < (SCRATCH as usize + 0x1_0000) {
        if runtime.grow_memory(256).is_err() {
            break;
        }
    }
    runtime.run_ctors()?;
    runtime.attach_log_ring(4 << 20)?;

    // Unlock the engine's diagnostic logging.
    //
    // Function 8502 — the one every `file.cc:line` soft-assert goes through —
    // opens with `i32.const 1351084 / i32.load8_u / i32eqz / br_if`, so a zero
    // byte there makes it return without emitting anything. That gate is why
    // whole paths looked like they never ran: "no log line" proved nothing
    // about them. Setting it makes the engine explain itself.
    //
    // Read before writing: setting it and seeing no change proves nothing if it
    // was already set, and that mistake made the first run of this experiment
    // worthless.
    const ASSERT_LOG_ENABLE: u32 = 1_351_084;
    println!(
        "assert-log gate BEFORE any write: {:?}",
        runtime.read(ASSERT_LOG_ENABLE, 1)
    );
    runtime.write_bytes_at(ASSERT_LOG_ENABLE, &[1])?;

    // Same lesson one level up: that gate only governs 8502. Ordinary lines go
    // through a threshold instead, which admits level 3 while the lines a
    // subsystem writes on *success* are level 4. Leaving it alone makes a
    // function that completed look like a function that gave up.
    println!("engine log level was {:?}", runtime.set_engine_log_level(9));

    // Not done here, and worth knowing why: the soft-assert path needs a *second*
    // thing, an indirect call through a slot at 1351212 that nothing fills, so
    // every assert increments a counter and emits nothing. Whole exit paths in
    // `wa_call_group_create_participant` are invisible for that reason.
    //
    // Pointing it at slot 3770 — a three-argument logger, the same arity the
    // callback is invoked with — does not work: three runs came back at 94
    // engine-log lines each against a ~200-line baseline, deterministically
    // worse, with no assert text to show for it. Matching arity is not matching
    // meaning. Anything here has to be a function that treats its arguments as
    // `(file, function, line)`, and the callback also only fires for the *first*
    // assert of each severity, so it cannot enumerate exit paths anyway.

    // What WhatsApp Web does immediately before `initVoipStack` and we never
    // did: `setABPropsOnWasm` walks `WAWeb/Voip/ABPropConfig.js` and pushes
    // every entry through `setABPropBool` / `setABPropInt`. Without it the
    // engine logs "Application settings not loaded" and every
    // `getVoipParam("options.*")` comes back empty.
    //
    // The values a real client uses come from the server, so these are the
    // types' own defaults — the point is whether the engine behaves differently
    // when the properties exist at all, not to reproduce anyone's rollout.
    const AB_BOOL: [&str; 12] = [
        "enable_av_downgrade",
        "enable_new_user_action_stanza_for_raise_hand_sender",
        "enable_webcodec_video_encode",
        "enable_init_bwe_for_group_call",
        "enable_ring_for_gc_on_offer_expire",
        "allow_reporting_call_replayer_id",
        "enable_offer_v2_upgrade",
        "enable_silent_offer",
        "voice_ai_conversation_starter_latency_tracking",
        "enable_waiting_room_logging",
        "attach_transport_rtx",
        "ignore_joinable_terminate_on_expired_offer",
    ];
    const AB_INT: [&str; 15] = [
        "heartbeat_interval_s",
        "lobby_timeout_min",
        "max_num_participants_for_ss",
        "calling_screen_share_milestone_version",
        "max_group_size_for_long_ringtone",
        "app_exit_reason_version",
        "log_level",
        "audio_level_speaking_threshold",
        "calling_rust_migration_bitmap",
        "calling_rust_migration_incoming_stanza_bitmap",
        "default_endpoint_thread_poll_timeout",
        "aigc_version",
        "call_admin_version",
        "vid_stream_pause_resume_jb_reset_threshold_ms",
        "voip_stack_incoming_message_ownership_transfer",
    ];
    let mut ab_set = 0usize;
    for name in AB_BOOL {
        if runtime
            .call_embind(
                "setABPropBool",
                &[Value::Str(name.to_owned()), Value::Bool(false)],
            )
            .is_ok()
        {
            ab_set += 1;
        }
        runtime.refuel();
    }
    // Deliberately not the ints. Zero is a sane default for a feature flag and
    // is not one for `heartbeat_interval_s` or
    // `default_endpoint_thread_poll_timeout`: pushing all 27 as zero registers
    // fine — "Application settings not loaded" stops appearing — and takes the
    // run from 167 log lines to 93. A real client gets these from the server.
    let _ = AB_INT;
    println!("AB props (bool) accepted: {ab_set}/{}", AB_BOOL.len());

    let init_mark = runtime.engine_log().len();
    let init = runtime.call_embind(
        "initVoipStack",
        &[
            Value::Str(SELF.into()),
            Value::Str(SELF_DEVICE.into()),
            Value::Str(SELF_LID.into()),
        ],
    )?;
    runtime.refuel();
    println!("initVoipStack -> {init:?}");
    let lines = runtime.engine_log_from(init_mark);
    println!("--- init log ({} lines) ---", lines.len());
    for line in &lines {
        println!("   {}", line.trim());
    }
    println!("--- end init log ---");
    // Two calls WhatsApp Web makes at init that we never did.
    // `WAWeb/Voip/Init.js` runs `voipInit` and `setHideMyIp` together, then
    // starts network-medium monitoring, which reaches the engine through
    // `updateNetworkMedium(medium, 0)` (`StackInterfaceWeb.js:952`).
    //
    // The second one matches a complaint in our own log word for word:
    // `wa_tp.cc get network medium: unknown peer id`.
    let hide_ip = runtime.call_embind("setHideMyIp", &[Value::Bool(false)]);
    runtime.refuel();
    println!("setHideMyIp -> {hide_ip:?}");
    let medium = runtime.call_embind("updateNetworkMedium", &[Value::Int(1), Value::Int(0)]);
    runtime.refuel();
    println!("updateNetworkMedium(1, 0) -> {medium:?}");

    // `getVoipParam` reads engine config by dotted name; the only name visible
    // in the captured JS is `options.caller_timeout`
    // (`HandleNativeCallEvent.js:130`). Worth knowing whether the window works
    // at all, since it would expose configuration nothing else reaches.
    for name in ["options.caller_timeout", "options.callee_timeout"] {
        let value = runtime.call_embind("getVoipParam", &[Value::Str(name.into())]);
        runtime.refuel();
        println!("getVoipParam({name}) -> {value:?}");
    }

    println!("live threads right after init: {}", runtime.live_threads());
    // Again, after init: the first write happens before `initVoipStack`, which
    // brings the engine's own logging config up and can put the gate back.
    runtime.write_bytes_at(ASSERT_LOG_ENABLE, &[1])?;
    println!(
        "assert-log gate now: {:?}",
        runtime.read(ASSERT_LOG_ENABLE, 1)
    );
    println!();
    Ok(runtime)
}

fn main() -> anyhow::Result<()> {
    let catalog = Catalog::discover()?;
    let bytes = std::fs::read(&catalog.resolve("JgwtTQVeWPm")?.path)?;

    // "start_precall peer_participant_jids must be LID, enforce LID for all
    // calls" — the engine says it outright once `_localtime_js` is real enough
    // for it to get that far.
    // Every JID in LID form, not just the list: the engine enforces LID for
    // "all calls", and the peer arguments are JIDs too.
    // The shape that works: a bare LID for the peer argument and a *device*
    // LID in the participant list. Both LID — the engine enforces it:
    // `start_precall peer_participant_jids must be LID, enforce LID for all
    // calls`.
    // `make_and_cache_offer` fails at `offer.cc:485`, not at 463 as this
    // comment used to say: `get_participant` walks the call's group and no jid
    // comparison matches. The self participant is present — `getCallInfo`
    // reports it — so adding our own LID to the list is not the missing piece.
    // See agent_docs/voip_oracle_status.md.
    // The fifth argument is a *legacy-form* JID in WhatsApp Web, not a LID:
    // `StartCall.js` passes `(g ?? h).toString({legacy: true})`. Passing
    // `11223344556677@c.us` there was tried and changes nothing — see the
    // excluded-variants table in agent_docs/voip_oracle_status.md — so both shapes below use the
    // LID and vary only what the participant list holds.
    let shapes: [(&str, &str, &str, Vec<String>); 2] = [
        (
            "device in the list",
            PEER_LID,
            PEER_LID,
            vec![PEER_LID_DEVICE.to_owned()],
        ),
        (
            "bare in the list",
            PEER_LID,
            PEER_LID,
            vec![PEER_LID.to_owned()],
        ),
    ];

    for (label, peer, alt_jid, devices) in shapes {
        for hold in [false] {
            let mut runtime = engine(&bytes)?;
            let mark = runtime.engine_log().len();

            // `hold`: build the vector ourselves and pass it as an already-made
            // object, which the call machinery does not release afterwards.
            // `startVoipCall` traps inside `std::vector`'s destructor (table
            // slot 375 → `free`), so the question is whether the engine has
            // taken the buffer and our release is a second free.
            let devices_arg = if hold {
                let registry = runtime.embind();
                let class_type = registry
                    .classes
                    .iter()
                    .find(|(_, class)| class.name == "StringList")
                    .map(|(type_id, _)| *type_id)
                    .expect("StringList should be registered");
                let handle = runtime
                    .build_vector(class_type, &[], &devices)
                    .expect("build StringList");
                Value::Object(handle)
            } else {
                Value::StringList(devices.clone())
            };

            // The engine exposes an SCTP ring buffer and a predicate for
            // whether it is initialised. Nothing here ever set it up, and
            // outbound data may well go through it.
            let initialised = runtime.call_embind("isSctpRingBufferInitialized", &[]);
            runtime.refuel();
            println!("   isSctpRingBufferInitialized (before) -> {initialised:?}");

            if let Ok(ptr) = runtime.malloc(1 << 20) {
                let set_up = runtime.call_embind(
                    "initSctpRingBuffer",
                    &[Value::Int(i64::from(ptr)), Value::Int(1 << 20)],
                );
                runtime.refuel();
                println!("   initSctpRingBuffer -> {set_up:?}");
                let now = runtime.call_embind("isSctpRingBufferInitialized", &[]);
                runtime.refuel();
                println!("   isSctpRingBufferInitialized (after) -> {now:?}");
            }

            // The sender is only ever reached through table slot 436, and the
            // dispatcher that invokes it sits in 437. If either is empty at
            // runtime the channel cannot carry anything, whatever the engine
            // decides.
            // Is the sender even instrumented? If something other than the
            // stub defines it, it would be called without being recorded — and
            // every "0 sent" in this investigation would be a blind spot
            // rather than a fact.
            let stubbed = runtime.stubbed_imports();
            for name in [
                "env::sendSignalingXMPP_js_sync",
                "env::on_call_event_js_sync",
                "env::call_sendto",
            ] {
                println!("   {name} stubbed(=recorded): {}", stubbed.contains(name));
            }

            for slot in [433, 434, 435, 436, 437] {
                println!(
                    "   table[{slot}] populated: {}",
                    runtime.table_entry_exists(slot)
                );
            }

            // `notifyWebP2PChannelReady(true, false)` used to be called here, on
            // the theory that the outbound channel needed to be told a transport
            // existed. It does not work that way, and the engine says so:
            // `[WebP2P] wa_call_notify_web_p2p_channel_ready failed: 70004`.
            //
            // In `StackInterfaceWeb.js` that call is made only from the
            // DataChannel state-change handler, once a *real* WebRTC channel has
            // opened, and after `initP2PVirtualAddresses`. Announcing a channel
            // that was never set up is a lie the engine is right to reject, so
            // the call is gone until there is a bridge behind it.

            // A setup step WhatsApp Web performs and we never did.
            // `WAWeb/Voip/JsWorkerThread.js` builds its wrapper out of
            // `startJsWorkerThread()` → `getJsWorkerPThreadId()` → a message
            // port, and `SctpDataChannelThread.js` is built on the same thing —
            // so this is the base of the P2P data path.
            let worker = runtime.call_embind("startJsWorkerThread", &[]);
            runtime.refuel();
            println!("   startJsWorkerThread -> {worker:?}");
            if let Ok(handle) = &worker
                && let Some(id) = handle.as_int()
            {
                let pthread = runtime.call_embind("getJsWorkerPThreadId", &[Value::Int(id)]);
                runtime.refuel();
                println!("   getJsWorkerPThreadId -> {pthread:?}");
            }

            // Before the call, for comparison: the post-failure dump shows a
            // participant with `is_self: true`, but it is read after the engine
            // has torn the call down, so on its own it cannot say whether that
            // participant existed *during* the attempt.
            let before_info = runtime.call_embind("getCallInfo", &[]);
            runtime.refuel();
            println!(
                "   getCallInfo BEFORE start -> {}",
                match &before_info {
                    Ok(value) => format!("{} chars", format!("{value:?}").len()),
                    Err(_) => "Err".to_owned(),
                }
            );

            let outcome = runtime.call_embind(
                "startVoipCall",
                &[
                    Value::Str(peer.into()),
                    devices_arg,
                    Value::Str(CALL_ID.into()),
                    Value::Bool(false),
                    Value::Str(alt_jid.into()),
                    Value::Bool(false),
                    // The last argument is the tcToken: `StartCall.js` fetches
                    // it (`getTcToken`) and passes it as `L`, which the stack
                    // interface turns into this Uint8List. We had been sending
                    // an empty one, and a WhatsApp offer carries the token.
                    Value::Bytes(vec![0xA5; 32]),
                ],
            );
            // The engine does the work on its own threads. One dying mid-call
            // would stop it just like a trap would.
            println!("   live threads before: {}", runtime.live_threads());
            // Is the trap simply the fuel running out? Traps at arbitrary
            // points are exactly what that looks like.
            println!(
                "   fuel left after the call: {:?}",
                runtime.fuel_remaining()
            );
            runtime.refuel();
            runtime.settle(std::time::Duration::from_secs(5));

            // The probes store `value + 1`, so 0 means "did not run" and 1
            // means "ran, and the value was zero" — two things a naive probe
            // reports identically.
            match runtime.read_u32_at(SCRATCH) {
                Ok(0) => println!("   probe: did not run (unpatched capture?)"),
                Ok(raw) => println!("   probe: {:#x}", raw - 1),
                Err(error) => println!("   probe unreadable: {error:#}"),
            }
            println!("   live threads after: {}", runtime.live_threads());
            for note in runtime.logs().iter().filter(|line| line.contains("thread")) {
                println!("   >>> {note}");
            }
            // `getCallInfo` takes no arguments and reads the call context
            // itself, through the same global the host cannot read: no global
            // is exported, so this is the only way to ask the engine what state
            // it believes the call is in.
            // Two more diagnostic windows, never used before. They take no
            // arguments and read the call context themselves, like getCallInfo.
            for probe in ["getShortStatisticString", "getDebugStatisticString"] {
                let out = runtime.call_embind(probe, &[]);
                runtime.refuel();
                let text = out
                    .as_ref()
                    .ok()
                    .and_then(|value| value.as_str())
                    .unwrap_or("<err>")
                    .to_owned();
                println!(
                    "   {probe} -> {} chars: {}",
                    text.len(),
                    &text[..text.len().min(400)]
                );
            }

            let info = runtime.call_embind("getCallInfo", &[]);
            runtime.refuel();
            println!("   getCallInfo -> {info:?}");

            let overflowed = runtime.engine_log_overflowed();
            let dropped = runtime
                .call_embind("getLogRingBufferOverflowCount", &[])
                .ok()
                .and_then(|value| value.as_int())
                .unwrap_or(-1);
            runtime.refuel();
            let lines = runtime.engine_log_from(mark);
            // Counters, not the recorded-call list. That list stops growing at
            // `MAX_TRACE` (8192) and startup alone makes tens of millions of
            // host calls, so `all_calls_to` answers zero for everything by the
            // time this runs — which is why every "0 sent" below was an
            // artefact rather than a finding. See `outbound_setup_matrix`.
            let counts = runtime.shared().hot_calls();
            let count = |symbol: &str| -> u64 {
                counts
                    .iter()
                    .find(|(s, _)| s == symbol)
                    .map(|(_, n)| *n)
                    .unwrap_or(0)
            };
            let sent = count("env::sendSignalingXMPP_js_sync");
            let events = count("env::on_call_event_js_sync");
            let verdict = match &outcome {
                Ok(value) => format!("{value:?}"),
                Err(error) => {
                    let report = format!("{error:#}");
                    let frame = report
                        .lines()
                        .find(|line| line.contains("wasm function"))
                        .unwrap_or("?")
                        .trim();
                    format!("trap: {frame}")
                }
            };
            println!(
                "=== {label:22} hold={hold} -> {verdict} | {} lines \
                 (overflowed={overflowed}, engine dropped {dropped}), \
                 {sent} sent, {events} events",
                lines.len(),
            );
            for line in lines.iter().take(140) {
                println!("      {}", line.trim());
            }
            // The tail is where the failure is. The head shows a call starting
            // normally every time, which is why watching only it kept this
            // looking like "the engine does nothing".
            println!("      ... last lines before it stops ...");
            for line in lines.iter().skip(lines.len().saturating_sub(40)) {
                println!("      | {}", line.trim());
            }
            // A call that is up should be endable, and ending it has to tell
            // the peer — so this is a second, independent chance for the
            // outbound channel to show it works.
            // Both of these must tell the peer, so they are the other two
            // chances to see the outbound channel work. Counted through the
            // counters — the comment that used to sit here concluded "the
            // outbound channel is blocked for everything" from a `len()` on a
            // list that had been full since startup.
            let sent_now = |runtime: &Runtime| -> u64 {
                runtime
                    .shared()
                    .hot_calls()
                    .iter()
                    .find(|(s, _)| s == "env::sendSignalingXMPP_js_sync")
                    .map(|(_, n)| *n)
                    .unwrap_or(0)
            };
            for (name, args) in [
                ("endCall", vec![Value::Int(0), Value::Bool(false)]),
                ("rejectCall", vec![]),
            ] {
                let before = sent_now(&runtime);
                let outcome = runtime.call_embind(name, &args);
                runtime.refuel();
                runtime.settle(std::time::Duration::from_secs(3));
                println!(
                    "   {name} -> {} | sent {} -> {}",
                    match &outcome {
                        Ok(value) => format!("{value:?}"),
                        Err(_) => "trap".to_owned(),
                    },
                    before,
                    sent_now(&runtime)
                );
            }

            // Arguments still come from the recorded list, which only has them
            // if `clear_trace` was called before the stretch above.
            for call in runtime
                .all_calls_to("env::sendSignalingXMPP_js_sync")
                .iter()
                .take(2)
            {
                println!("   >>> SENT: args {:?}", call.args);
            }
            // The log ends at `Creating field_stats_manager` and the landing
            // pad calls `__resumeException`, so something throws there.
            // libc++abi writes the uncaught exception's message to stderr
            // before aborting: `libc++abi: <what()>`.
            let stderr = runtime.wasi().stderr_text();
            if !stderr.trim().is_empty() {
                println!("   GUEST STDERR: {stderr}");
            }
            println!("   --- stubs the guest actually called ---");
            for (symbol, count) in runtime.stubs_called() {
                println!("   ??? {symbol}: {count} call(s)");
            }
            for note in runtime
                .logs()
                .iter()
                .filter(|line| {
                    line.contains("C++ exception")
                        || line.contains("abort")
                        || line.contains("assertion")
                        || line.contains("unreachable")
                })
                .map(|line| {
                    line.replace('\n', " | ")
                        .chars()
                        .take(400)
                        .collect::<String>()
                })
                .rev()
                .take(4)
            {
                println!("   !!! {note}");
            }
        }
    }

    Ok(())
}
