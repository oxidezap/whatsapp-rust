//! Does loading voip params unblock the outgoing call?
//!
//! `settings_probe` establishes that `getVoipParam` is answered from the
//! `<voip_settings>` blob on an incoming call stanza, and from nowhere else —
//! the embind surface has a `getVoipParam` and no setter, so a host cannot
//! configure the engine at all. `agent_docs/voip_oracle_status.md` records `startVoipCall`
//! failing with `make_and_cache_offer failed: 70008`, and the obvious causal
//! story is that the two are one fact.
//!
//! **They are not, and the premise is stale.** Measured on `S_ivh1PriOA` and
//! `JgwtTQVeWPm`, with a LID peer, a filled device list and a 32-byte tcToken:
//!
//!   * `startVoipCall` returns `0`, reaches `[None -> Calling]`, and the engine
//!     builds and hands off an offer — `send_offer_msg`, `EVENT: Call offer
//!     sent`. No 70008 anywhere. Whatever produced it on `D5pLH9sfOOl` is fixed
//!     or was an artefact of the arguments that capture was driven with.
//!   * Loading settings first changes **nothing** about that: both arms are
//!     identical, 39 log lines each, same states, same offer.
//!
//! **WARNING — this example's `sent=` column is not to be trusted.** It reads
//! `all_calls_to`, which returns the recorded-call *list*; that list stops
//! growing at `MAX_TRACE` (8192) and engine startup alone makes ~39 **million**
//! host calls, so it answers zero for everything regardless of what happened.
//! Every `sent=0` this example printed was that artefact.
//!
//! `outbound_setup_matrix` measures the same thing correctly, through
//! `shared().hot_calls()`, and finds the offer **is** delivered:
//! `sendSignalingXMPP_js_sync(peer_jid, call_id, stanza, 179)`, on a bare
//! engine, in every arrangement. Read that one instead; this is kept for the
//! settings A/B, which is unaffected because it reads `getVoipParam` directly.
//!
//! ```sh
//! cargo run --release --example outbound_after_settings
//! NO_MAIN_THREAD=1 cargo run --release --example outbound_after_settings
//! ```
use base64::Engine as _;
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Server;
use wacore_binary::{Jid, Node, marshal};

const SETTINGS: &[u8] =
    br#"{"encode":{"use_mlow_codec_v1":"false"},"options":{"enable_48khz_rtp_clock":"false","caller_timeout":"45"}}"#;

const SELF: &str = "15550002222@c.us";
const SELF_DEVICE: &str = "15550002222:0@c.us";
const SELF_LID: &str = "99887766554433:0@lid";
/// The engine enforces LID for every call: `peer_participant_jids must be LID`.
const PEER_LID: &str = "11223344556677@lid";
const PEER_LID_DEVICE: &str = "11223344556677:0@lid";
const OUTGOING_CALL_ID: &str = "0011223344556677";
const INCOMING_CALL_ID: &str = "0102030405060708";

/// An incoming offer whose only job is to carry the settings blob.
fn offer_stanza(caller: &Jid, now: u64) -> Node {
    NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("call-id", INCOMING_CALL_ID)
        .attr("call-creator", caller.with_device(1))
        .attr("t", now.to_string())
        .children([
            NodeBuilder::new("offer")
                .children([
                    NodeBuilder::new("audio")
                        .attr("enc", "opus")
                        .attr("rate", "16000")
                        .build(),
                    NodeBuilder::new("net").attr("medium", "3").build(),
                    NodeBuilder::new("encopt").attr("keygen", "2").build(),
                ])
                .build(),
            NodeBuilder::new("voip_settings")
                .attr("uncompressed", "1")
                .bytes(SETTINGS.to_vec())
                .build(),
        ])
        .build()
}

/// `initVoipStack` fails intermittently, so a single attempt reads as "the arm
/// was not measured" when it is really "the engine did not start". Same retry
/// `outgoing_call` and `tests/signaling.rs` carry.
fn engine(bytes: &[u8]) -> anyhow::Result<Runtime> {
    const ATTEMPTS: usize = 8;
    for _ in 0..ATTEMPTS {
        let mut r = Runtime::instantiate(bytes)?;
        r.set_thread_policy(ThreadPolicy::Spawn);
        // Register as emscripten's main runtime thread, which is what gives the
        // proxy queue a thread that may drain it — and the outbound signaling
        // callback is dispatched through that queue. Without it, "nothing was
        // dispatched" would be a property of the harness rather than of the
        // engine. `outgoing_call` documents the trade: it makes this path
        // deterministic and breaks two `tests/signaling.rs` cases.
        r.set_main_thread_registration(std::env::var("NO_MAIN_THREAD").is_err());
        r.run_ctors()?;
        r.attach_log_ring(4 << 20)?;
        let init = r.call_embind(
            "initVoipStack",
            &[
                Value::Str(SELF.into()),
                Value::Str(SELF_DEVICE.into()),
                Value::Str(SELF_LID.into()),
            ],
        );
        r.refuel();
        if init.as_ref().ok().and_then(|v| v.as_int()) == Some(0) {
            return Ok(r);
        }
    }
    anyhow::bail!("initVoipStack never returned 0 in {ATTEMPTS} attempts")
}

fn caller_timeout(r: &mut Runtime) -> String {
    let answer = r.call_embind(
        "getVoipParam",
        &[Value::Str("options.caller_timeout".into())],
    );
    r.refuel();
    match answer {
        Ok(Value::Str(s)) if s.is_empty() => "empty".into(),
        Ok(Value::Str(s)) => s,
        other => format!("{other:?}"),
    }
}

/// Every A/B property `WAWebVoipStackInterfaceWebHelpers` forwards into the
/// engine, as `(wasmKey, kind)`, read out of `WAWebVoipABPropConfig` in the
/// 2.3000.1044659339 bundle. Three are renamed on the way in — the key here is
/// the one the wasm is given, not the property's own name.
///
/// **The values are not WhatsApp's.** Real ones come from the A/B registry the
/// server answers for; these are the neutral choices a client with no registry
/// would have to invent, which is exactly the question being asked: does
/// configuring the engine at all change what it does?
const AB_PROPS: &[(&str, &str)] = &[
    ("aigc_version", "int"),
    ("app_exit_reason_version", "int"),
    ("attach_transport_rtx", "bool"),
    ("audio_level_speaking_threshold", "int"),
    ("call_admin_version", "int"),
    ("calling_rust_migration_bitmap", "int"),
    ("calling_rust_migration_incoming_stanza_bitmap", "int"),
    ("calling_screen_share_milestone_version", "int"),
    ("default_endpoint_thread_poll_timeout", "int"),
    ("enable_av_downgrade", "bool"),
    ("enable_init_bwe_for_group_call", "bool"),
    (
        "enable_new_user_action_stanza_for_raise_hand_sender",
        "bool",
    ),
    ("enable_offer_v2_upgrade", "bool"),
    ("enable_ring_for_gc_on_offer_expire", "bool"),
    ("enable_silent_offer", "bool"),
    ("enable_waiting_room_logging", "bool"),
    ("enable_webcodec_video_encode", "bool"),
    ("enable_web_voip_audio_driver_lifetime_fix", "bool"),
    ("heartbeat_interval_s", "int"),
    ("ignore_joinable_terminate_on_expired_offer", "bool"),
    ("lobby_timeout_min", "int"),
    ("max_group_size_for_long_ringtone", "int"),
    ("max_num_participants_for_ss", "int"),
    ("allow_reporting_call_replayer_id", "bool"),
    ("vid_stream_pause_resume_jb_reset_threshold_ms", "int"),
    ("voice_ai_conversation_starter_latency_tracking", "bool"),
    ("voip_stack_incoming_message_ownership_transfer", "bool"),
    ("log_level", "int"),
];

/// Configure the engine the way `setABPropsOnWasm` does.
fn set_ab_props(r: &mut Runtime) -> usize {
    let mut set = 0;
    for (key, kind) in AB_PROPS {
        let call = match *kind {
            "bool" => r.call_embind(
                "setABPropBool",
                &[Value::Str((*key).into()), Value::Bool(true)],
            ),
            _ => r.call_embind(
                "setABPropInt",
                &[
                    Value::Str((*key).into()),
                    // `log_level` is the one where a specific value is known to
                    // mean something; the rest get zero rather than a guess.
                    Value::Int(if *key == "log_level" { 9 } else { 0 }),
                ],
            ),
        };
        r.refuel();
        if call.is_ok() {
            set += 1;
        }
    }
    set
}

/// Feed the engine a settings blob through the only door it has.
fn load_settings(r: &mut Runtime) -> anyhow::Result<()> {
    let caller = Jid::new("11223344556677", Server::Lid);
    let now = r.virtual_unix_time();
    let payload = base64::engine::general_purpose::STANDARD
        .encode(marshal::marshal(&offer_stanza(&caller, now))?);

    r.call_embind(
        "handleIncomingSignalingOffer",
        &[
            Value::Str(payload),
            Value::Str("web".into()),
            Value::Str("2.3000.0".into()),
            Value::Str(now.to_string()),
            Value::Str(now.to_string()),
            Value::Bool(false),
            Value::Bool(true),
            Value::Str(caller.to_string()),
            Value::Bytes(Vec::new()),
        ],
    )
    .ok();
    r.refuel();
    r.settle(std::time::Duration::from_secs(3));

    // Clear the incoming call before originating one. Without this the second
    // start reports `Call context has already been initialized` and fails with
    // 70008 for a reason that has nothing to do with settings — which would
    // make the comparison meaningless in the direction that flatters it.
    r.call_embind("rejectCall", &[]).ok();
    r.refuel();
    r.settle(std::time::Duration::from_secs(3));
    Ok(())
}

fn originate(r: &mut Runtime) -> (String, Vec<String>, usize) {
    let mark = r.engine_log().len();
    let result = r.call_embind(
        "startVoipCall",
        &[
            Value::Str(PEER_LID.into()),
            Value::StringList(vec![PEER_LID_DEVICE.into()]),
            Value::Str(OUTGOING_CALL_ID.into()),
            Value::Bool(false),
            Value::Str(PEER_LID.into()),
            Value::Bool(false),
            Value::Bytes(vec![0xA5; 32]),
        ],
    );
    r.refuel();
    r.settle(std::time::Duration::from_secs(5));

    let shown = match &result {
        Ok(value) => format!("{value:?}"),
        Err(_) => "trap".into(),
    };

    // Drain what the engine queued for the main thread.
    //
    // The offer is built on a worker (`[1743564]` in the log) and
    // `sendSignalingXMPP_js_sync` is a `_js_sync` callback, so emscripten
    // proxies it: the worker queues the call and the *main thread* runs it. In
    // a browser that second half is the JS glue reacting to a postMessage. This
    // harness has no event loop, so nobody ever ran it — which is why the
    // engine could report `Call offer sent` with no host call to show for it.
    //
    // Both entry points are module exports, so the host can do the glue's job
    // by calling them. Errors are reported rather than swallowed: "the drain
    // trapped" and "the drain found nothing" are different answers.
    for drain in [
        "emscripten_main_thread_process_queued_calls",
        "_emscripten_check_mailbox",
    ] {
        match r.call(drain, &[]) {
            Ok(_) => {}
            Err(e) => println!("    {drain} -> error: {e}"),
        }
        r.refuel();
        r.settle(std::time::Duration::from_secs(2));
    }

    // Give the worker that built the offer room to finish. The engine logs
    // `Call offer sent` from a worker thread, so "nothing was sent" could be a
    // thread that had not got there yet rather than one that never would.
    println!("    live guest threads: {}", r.live_threads());
    r.settle(std::time::Duration::from_secs(15));
    println!(
        "    live guest threads after settling: {}",
        r.live_threads()
    );

    let sent = r.all_calls_to("env::sendSignalingXMPP_js_sync").len();
    (shown, r.engine_log_from(mark), sent)
}

fn main() -> anyhow::Result<()> {
    let which = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "S_ivh1PriOA".into());
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve(&which)?;
    let bytes = std::fs::read(&entry.path)?;
    println!(
        "{}\n",
        entry.path.file_name().unwrap_or_default().to_string_lossy()
    );

    for load in [false, true] {
        println!(
            "=== outgoing call {} settings loaded first",
            if load { "WITH" } else { "WITHOUT" }
        );
        let mut r = engine(&bytes)?;
        if load {
            println!(
                "  setABProp* accepted {} of {}",
                set_ab_props(&mut r),
                AB_PROPS.len()
            );
            load_settings(&mut r)?;
        }
        println!("  options.caller_timeout = {}", caller_timeout(&mut r));

        let (result, lines, sent) = originate(&mut r);
        println!(
            "  startVoipCall -> {result}, {} log lines, {sent} stanzas sent",
            lines.len()
        );

        // Where the offer goes once the engine has built it.
        //
        // `sendSignalingXMPP_js_sync` is not an import: it is a JS callback the
        // browser's glue registers, reached from a worker thread through
        // emscripten's proxy queue — queue the call, dispatch it on the main
        // thread through `emscripten_receive_on_main_thread_js`, which calls
        // through `__indirect_function_table`. Counting only the import is
        // therefore blind to whether the engine tried; counting the dispatch is
        // not.
        for symbol in [
            "env::emscripten_receive_on_main_thread_js",
            "env::_emscripten_notify_mailbox_postmessage",
            "env::sendSignalingXMPP_js_sync",
            "env::call_sendto",
            "env::on_call_event_js_sync",
        ] {
            println!("    {symbol:<48} {} call(s)", r.all_calls_to(symbol).len());
        }
        println!(
            "    sender has a recording stub: {}",
            r.stubbed_imports()
                .contains("env::sendSignalingXMPP_js_sync")
        );
        // Every import the guest reached that has no real implementation. If
        // the offer is being handed to something, this is where it shows up —
        // and a name here is a piece of glue to write, in priority order.
        let mut called = r.stubs_called();
        called.sort_by_key(|entry| std::cmp::Reverse(entry.1));
        println!("    stubs the guest called ({}):", called.len());
        for (name, count) in called.iter().take(14) {
            println!("      {count:>5}  {name}");
        }

        // The lines this experiment is about, quoted rather than summarised: a
        // status code that changed is the whole result. `agent_docs/voip_oracle_status.md` was
        // written when `make_and_cache_offer` failed with 70008, so its absence
        // has to be visible rather than inferred from a return value.
        for line in lines.iter().filter(|l| {
            l.contains("make_and_cache_offer")
                || l.contains("Call start")
                || l.contains("send_offer_msg")
                || l.contains("Call offer")
                || l.contains("change_call_state")
        }) {
            println!("    {}", line.trim());
        }
        println!();
    }

    Ok(())
}
