//! Which setup step, if any, makes the engine's offer reach the host?
//!
//! The question this was opened to answer: `outbound_after_settings` reported
//! the engine building an outgoing offer that no host call carried, and
//! `StackInterfaceWeb.js` performs two setup steps no harness here did — an
//! SCTP ring buffer and a JS worker thread (`JsWorkerThread.js` is
//! `startJsWorkerThread()` → `getJsWorkerPThreadId()` → a message port, and
//! `SctpDataChannelThread.js` is built on the same thing). So: run the same
//! origination under each combination and see which one makes the offer leave.
//!
//! The arms are cumulative on purpose. If none of them changes the outcome, the
//! missing piece is not a setup call at all.
//!
//! **The answer turned out to be that nothing was missing.** All five arms are
//! identical and all five *send*: `startVoipCall` returns 0, the engine builds
//! an offer, and `sendSignalingXMPP_js_sync` is called once, with
//!
//! ```text
//! sendSignalingXMPP_js_sync(peer_jid, call_id, stanza, len)
//!   arg0 -> "11223344556677@lid"
//!   arg1 -> "0011223344556677"
//!   arg2 -> a 179-byte buffer
//!   arg3 =  179
//! ```
//!
//! The outbound signaling path works headless, on a bare engine, with no glue
//! and none of the setup steps above.
//!
//! ## The measurement error that hid it
//!
//! Every earlier run reported `sent=0`, and every one of them was wrong. Three
//! separate holes, each of which reads as a confident negative:
//!
//!   * **`all_calls_to` reads the recorded-call *list*, which stops growing at
//!     `MAX_TRACE` (8192).** Bringing the engine up makes ~39 *million* host
//!     calls, so the list has been full since long before anything interesting
//!     happens and every query answers zero. Use `shared().hot_calls()`, which
//!     reads counters, or `clear_trace()` immediately before the stretch being
//!     measured. This example does both: counters for the verdict, a cleared
//!     list for the arguments.
//!   * **`watch_markers` has to be armed** before an instrumented copy records
//!     anything, so "the marker never fired" and "the marker was never watched"
//!     are indistinguishable. Only a control marker on a function known to run
//!     (#12871, which logs `send_offer_msg`) exposed it.
//!   * **`stubs_called` only lists imports that got a stub**, so an import with
//!     a real implementation is invisible to it whatever it does.
//!
//! The conclusion those holes produced — "#855 never executes, the dispatcher
//! is missing, the glue is the gap" — was false in every part.
//! `call_the_sender` calls #855 directly through table slot 464 and it forwards
//! exactly as its body reads; this example shows the engine reaching it on its
//! own during an ordinary origination.
//!
//! ## The stanza
//!
//! `arg2` cannot be read after the fact: #855 frees the three pointers as soon
//! as the import returns and the allocator hands the memory straight back out,
//! so a later read shows whatever landed there. `sendSignalingXMPP_js_sync` is
//! therefore implemented rather than stubbed, and copies the bytes while they
//! exist — `Runtime::signaling()` returns them.
//!
//! Decoding those 179 bytes with **whatsapp-rust's** parser, which shares no
//! lineage with the engine that produced them, gives a stanza:
//!
//! ```xml
//! <offer call-id="0011223344556677" call-creator="99887766554433@lid">
//!   <privacy>a5 a5 … 32 bytes</privacy>   <!-- the tcToken passed in -->
//!   <audio enc="opus" rate="8000"/>
//!   <audio enc="opus" rate="16000"/>
//!   <net medium="3"/>
//!   <capability ver="1">01 05 f7 09 e0 bb 5b</capability>
//!   <enc count="0">32 bytes</enc>
//!   <encopt keygen="2"/>
//! </offer>
//! ```
//!
//! The first byte is the stream flag the transport puts in front of a node, so
//! the node starts at +1. The `<privacy>` content is the 32-byte tcToken this
//! example passes to `startVoipCall`, which is what makes the path end-to-end
//! rather than merely plausible: an argument handed in at the embind surface
//! comes back out as a field of a stanza a second implementation can parse.
//!
//! ```sh
//! cargo run --release --example outbound_setup_matrix
//! cargo run --release --example outbound_setup_matrix -- JgwtTQVeWPm
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
const PEER_LID: &str = "11223344556677@lid";
const PEER_LID_DEVICE: &str = "11223344556677:0@lid";
const OUTGOING_CALL_ID: &str = "0011223344556677";

/// What `WAWebVoipStackInterfaceWebHelpers` forwards, by wasm key. Three of the
/// 28 are renamed from the property's own name; these are the keys the engine
/// is given. Values are neutral rather than WhatsApp's, which the A/B registry
/// would supply.
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

/// One cumulative arm.
#[derive(Clone, Copy)]
struct Arm {
    label: &'static str,
    ab_props: bool,
    settings: bool,
    sctp: bool,
    worker: bool,
}

const ARMS: &[Arm] = &[
    Arm {
        label: "bare",
        ab_props: false,
        settings: false,
        sctp: false,
        worker: false,
    },
    Arm {
        label: "+ab props",
        ab_props: true,
        settings: false,
        sctp: false,
        worker: false,
    },
    Arm {
        label: "+settings",
        ab_props: true,
        settings: true,
        sctp: false,
        worker: false,
    },
    Arm {
        label: "+sctp ring",
        ab_props: true,
        settings: true,
        sctp: true,
        worker: false,
    },
    Arm {
        label: "+js worker",
        ab_props: true,
        settings: true,
        sctp: true,
        worker: true,
    },
];

fn offer_stanza(caller: &Jid, now: u64) -> Node {
    NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("call-id", "0102030405060708")
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

fn engine(bytes: &[u8]) -> anyhow::Result<Runtime> {
    const ATTEMPTS: usize = 8;
    for _ in 0..ATTEMPTS {
        let mut r = Runtime::instantiate(bytes)?;
        r.set_thread_policy(ThreadPolicy::Spawn);
        r.set_main_thread_registration(true);
        r.run_ctors()?;
        r.attach_log_ring(4 << 20)?;
        // Arm the marker mirror. `oracle instrument` splices calls to this
        // import, and until the sink is named nothing is recorded — so an
        // instrumented run reads exactly like an uninstrumented one, and a
        // marker that never fires looks the same as a marker never watched.
        // That mistake cost one round of this experiment.
        r.shared().watch_markers("env::on_call_event_js_sync");
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
    // Clear the incoming call, or the origination fails on an initialised call
    // context for a reason that has nothing to do with the arm being tested.
    r.call_embind("rejectCall", &[]).ok();
    r.refuel();
    r.settle(std::time::Duration::from_secs(3));
    Ok(())
}

/// The buffer the engine's SCTP data path writes through.
fn init_sctp(r: &mut Runtime) -> String {
    const SIZE: u32 = 1 << 20;
    let Ok(ptr) = r.malloc(SIZE) else {
        return "malloc failed".into();
    };
    let set_up = r.call_embind(
        "initSctpRingBuffer",
        &[Value::Int(i64::from(ptr)), Value::Int(i64::from(SIZE))],
    );
    r.refuel();
    let now = r.call_embind("isSctpRingBufferInitialized", &[]);
    r.refuel();
    format!("init={set_up:?} initialized={now:?}")
}

/// `JsWorkerThread.js`'s wrapper: start the thread, then read its pthread id.
fn start_worker(r: &mut Runtime) -> String {
    let worker = r.call_embind("startJsWorkerThread", &[]);
    r.refuel();
    r.settle(std::time::Duration::from_secs(2));
    let mut out = format!("start={worker:?}");
    if let Ok(handle) = &worker
        && let Some(id) = handle.as_int()
    {
        let pthread = r.call_embind("getJsWorkerPThreadId", &[Value::Int(id)]);
        r.refuel();
        out.push_str(&format!(" pthread={pthread:?}"));
    }
    out
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

    for arm in ARMS {
        println!("=== {}", arm.label);
        let mut r = engine(&bytes)?;

        if arm.ab_props {
            println!(
                "  ab props     {} of {}",
                set_ab_props(&mut r),
                AB_PROPS.len()
            );
        }
        if arm.settings {
            load_settings(&mut r)?;
            println!("  settings     loaded");
        }
        if arm.sctp {
            println!("  sctp ring    {}", init_sctp(&mut r));
        }
        if arm.worker {
            println!("  js worker    {}", start_worker(&mut r));
        }

        let mark = r.engine_log().len();
        // Empty the recorded-call list so the origination's own host calls fit
        // inside MAX_TRACE and their arguments survive. Startup alone makes
        // tens of millions, which is why the list is useless without this.
        r.shared().clear_trace();
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
        r.settle(std::time::Duration::from_secs(8));

        // Read the *counters*, not the recorded-call list. `all_calls_to` stops
        // growing at MAX_TRACE (8192) and this run makes tens of millions of
        // host calls, so a zero from it means "not in the first 8192" and
        // nothing more. Every earlier `sent=0` in this investigation was that
        // artefact.
        let counts = r.shared().hot_calls();
        let count = |symbol: &str| -> u64 {
            counts
                .iter()
                .find(|(s, _)| s == symbol)
                .map(|(_, n)| *n)
                .unwrap_or(0)
        };
        let sent = count("env::sendSignalingXMPP_js_sync");
        let sendto = count("env::call_sendto");
        let events = count("env::on_call_event_js_sync");
        let markers: Vec<i32> = r.shared().markers().iter().map(|(id, _)| *id).collect();
        if !markers.is_empty() {
            println!("  markers      {markers:?}");
        }

        // What the engine handed the host, captured inside the host call —
        // the only moment the stanza exists, since #855 frees the buffer on
        // return. Reading the recorded arguments afterwards shows whatever the
        // allocator handed out next, which is what an earlier version of this
        // example printed.
        for call in r.signaling() {
            println!(
                "  -> {} / {} : {} bytes",
                call.peer_jid,
                call.call_id,
                call.stanza.len()
            );
            let head: Vec<String> = call
                .stanza
                .iter()
                .take(32)
                .map(|b| format!("{b:02x}"))
                .collect();
            println!("     {}", head.join(" "));

            // Decode it with **whatsapp-rust's** parser — an implementation
            // that shares no lineage with the engine that produced these bytes.
            // If it parses, the engine's output is a stanza in the sense the
            // rest of the ecosystem means, and this is RFC-0005's cross-check
            // for free: two independent implementations agreeing on one wire
            // format.
            //
            // The leading byte is the stream flag WhatsApp's transport puts in
            // front of a node, so the node itself starts at +1; both forms are
            // tried rather than assuming which one the engine hands out.
            for (label, slice) in [
                ("as given", call.stanza.as_slice()),
                (
                    "skipping the flag byte",
                    &call.stanza[1.min(call.stanza.len())..],
                ),
            ] {
                match marshal::unmarshal_ref(slice) {
                    Ok(node) => {
                        println!("     wacore parses it ({label}): {node:?}");
                        break;
                    }
                    Err(e) => println!("     wacore ({label}): {e}"),
                }
            }
        }

        let lines = r.engine_log_from(mark);
        println!(
            "  startVoipCall -> {}   sent={sent} sendto={sendto} events={events} lines={}",
            match &result {
                Ok(v) => format!("{v:?}"),
                Err(_) => "trap".into(),
            },
            lines.len()
        );
        for line in lines
            .iter()
            .filter(|l| l.contains("offer") || l.contains("Offer") || l.contains("transport"))
        {
            println!("    {}", line.trim());
        }
        println!();
    }

    Ok(())
}
