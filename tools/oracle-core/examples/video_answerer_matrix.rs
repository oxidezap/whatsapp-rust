//! Which seeded state, if any, lets the pinned engine keep an inbound video
//! offer alive long enough to accept it?
//!
//! Each probe feeds one inbound offer to a fresh engine, reads `getCallInfo`
//! immediately (empty means the call never existed, not even transiently),
//! attempts `acceptCall`, and reports what the engine emitted. Probes vary
//! three axes: the A/B properties `setABPropsOnWasm` forwards (`AB_PROPS`,
//! neutral values, copied from `outbound_after_settings.rs`), the offer shape
//! (settings-only vs video with a clear call key), and the accept arguments.
//!
//! All identities are fictitious. Execute in release.
//!
//! ```sh
//! cargo run --release -p oracle-core --example video_answerer_matrix
//! ```
//!
//! `ENGINE` overrides the pinned module (default `JgwtTQVeWPm`).

mod common;

use anyhow::Result;
use base64::Engine as _;
use oracle_core::{Runtime, Value};
use sha2::{Digest, Sha256};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Server;
use wacore_binary::{Jid, Node, marshal};

const ENGINE: &str = "JgwtTQVeWPm";
const ENGINE_SHA: &str = "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db";

const SELF: [&str; 3] = [
    "15550002222@c.us",
    "15550002222:0@c.us",
    "99887766554433:0@lid",
];
const CALL_ID: &str = "0011223344556677";
const CALL_KEY: [u8; 32] = [0x5A; 32];
const SETTINGS: &[u8] =
    br#"{"encode":{"use_mlow_codec_v1":"false"},"options":{"enable_48khz_rtp_clock":"false","caller_timeout":"45"}}"#;

/// Neutral A/B values, as `WAWebVoipStackInterfaceWebHelpers` would forward
/// them. Copied from `outbound_after_settings.rs`; the question is the same:
/// does configuring the engine at all change what it does?
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

/// The vendor's own video child, read back from a `startVoipCall` offer so the
/// inbound shape carries no invented bytes.
const VIDEO_ATTRS: &[(&str, &str)] = &[
    ("enc", "h.264"),
    ("dec", "H264"),
    ("device_orientation", "0"),
    ("screen_width", "0"),
    ("screen_height", "0"),
];

fn video_child() -> Node {
    let mut builder = NodeBuilder::new("video");
    for (name, value) in VIDEO_ATTRS {
        builder = builder.attr(name, (*value).to_owned());
    }
    builder.build()
}

fn census_video_offer_at(now: Option<u64>) -> Node {
    let mut offer = NodeBuilder::new("offer");
    if let Some(now) = now {
        offer = offer.attr("t", now.to_string());
    }
    offer
        .children([
            NodeBuilder::new("audio")
                .attr("enc", "opus")
                .attr("rate", "16000")
                .build(),
            video_child(),
            NodeBuilder::new("net").attr("medium", "3").build(),
            NodeBuilder::new("enc")
                .attr("count", "0")
                .bytes(CALL_KEY.to_vec())
                .build(),
            NodeBuilder::new("encopt").attr("keygen", "2").build(),
        ])
        .build()
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

struct Probe {
    label: &'static str,
    ab_props: bool,
    video: bool,
    accept: (bool, bool),
    /// Expiry (`e`) on the `<call>` wrapper: the pinned IR lists it as a known
    /// inbound attr, and the engine tears the offer down as expired before
    /// accept, so a missing expiry is a suspect.
    expiry: bool,
    /// Send `t` in milliseconds rather than seconds: if the engine reads the
    /// offer timestamp in ms, a seconds `t` sits 1000x in the past and the
    /// offer arrives already expired.
    t_millis: bool,
    /// Second numeric argument to `handleIncomingSignalingOffer` as an offset
    /// from now (`None` repeats now, as every harness does). If that slot is
    /// an expiry rather than a second timestamp, `now` expires the offer on
    /// arrival, which fits the born-torn-down verdict exactly.
    second_numeric_offset: Option<u64>,
    /// Stamp `t` on the inner `<offer>` as well as the `<call>` wrapper.
    inner_t: bool,
    /// Shift the offer timestamp by this many seconds (wheels the wrapper `t`
    /// and the `t` arg together). Diagnostic: if `call_offer_elapsed_t`
    /// follows the shift, the engine reads our timestamp and the miss comes
    /// from elsewhere; if it stays at the clock epoch, the engine never
    /// finds it.
    t_offset: i64,
    /// Device suffix on the caller JID handed to the engine (untested axis:
    /// every harness passes bare user form).
    caller_device: Option<u16>,
    /// Call-creator device suffix (`None` = bare user form, as the repo
    /// suite sends; the matrix defaultресс is device 1).
    creator_device: Option<u16>,
}

fn run_probe(bytes: &[u8], probe: &Probe) -> Result<()> {
    let mut r = common::engine(
        bytes,
        common::Startup {
            identity: SELF,
            attempts: 8,
            // NO_MAIN_THREAD=1: skip main-thread registration. With it on, a
            // synchronous `handleIncomingSignalingOffer` can spin on work
            // queued for the blocking main thread (see `state.rs`), and the
            // scheduler's timeout escape may then surface partial processing
            // as a verdict. Registration off trades away outbound-signaling
            // collection (nothing to collect here) for a clean inbound path.
            register_main: std::env::var("NO_MAIN_THREAD").is_err(),
            log_bytes: 4 << 20,
            marker_sink: None,
        },
    )?;
    // LOG_LEVEL=N raises the engine log threshold before delivery: the default
    // level hides subsystem diagnostics, and the miss decision may explain
    // itself one level up.
    if let Ok(level) = std::env::var("LOG_LEVEL")
        && let Ok(level) = level.parse::<i32>()
    {
        match r.set_engine_log_level(level) {
            Ok(previous) => println!("PROBE {}: log level {previous} -> {level}", probe.label),
            Err(e) => println!("PROBE {}: set log level failed: {e}", probe.label),
        }
        r.refuel();
    }
    // The event thread must be up before delivery: an offer into the startup
    // gap lands on a half-started engine and reads as a refusal.
    // MINIMAL=1 skips the gate and all seeding: every observation burns the
    // monotonic clock the guest advances per read, so the earliest possible
    // feed discriminates clock-burn from unreadiness.
    let minimal = std::env::var("MINIMAL").is_ok();
    if !minimal {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
        let mut gated = false;
        while std::time::Instant::now() < deadline {
            if r.engine_log()
                .iter()
                .any(|l| l.contains("call_event_proc resumed"))
            {
                gated = true;
                break;
            }
            r.process_queued_calls();
            r.refuel();
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if !gated {
            println!(
                "PROBE {}: event thread never announced itself; results suspect",
                probe.label
            );
        }
    }
    // REPLICATE_LOAD=1: exact `load_settings` sequence from
    // `outbound_after_settings.rs` (settings offer, settle, rejectCall to
    // clear, second start). That flow reports the second start failing with
    // "already initialized", i.e. a surviving context; if it does not
    // replicate here, the observation is harness-conditional.
    if std::env::var("REPLICATE_LOAD").is_ok() {
        let caller = Jid::new("11223344556677", Server::Lid);
        let now = r.virtual_unix_time();
        let payload = base64::engine::general_purpose::STANDARD.encode(marshal::marshal(
            &common::settings_offer(&caller, now, "0102030405060708", SETTINGS),
        )?);
        if probe.ab_props && !minimal {
            let set = set_ab_props(&mut r);
            println!("  ab props accepted {set} of {}", AB_PROPS.len());
        }
        r.call_embind(
            "handleIncomingSignalingOffer",
            &[
                Value::Str(payload.clone()),
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
        r.refuel();
        let rejected = r.call_embind("rejectCall", &[]);
        r.refuel();
        r.settle(std::time::Duration::from_secs(3));
        r.refuel();
        let second = r.call_embind(
            "startVoipCall",
            &[
                Value::Str("11223344556677@lid".into()),
                Value::StringList(vec!["11223344556677:0@lid".into()]),
                Value::Str("0011223344556677".into()),
                Value::Bool(false),
                Value::Str("11223344556677@lid".into()),
                Value::Bool(false),
                Value::Bytes(Vec::new()),
            ],
        );
        r.refuel();
        println!("REPLICA: rejectCall -> {rejected:?}, second start -> {second:?}");
        for line in r.engine_log().iter().rev().take(8).rev() {
            println!("    log: {}", line.trim());
        }
        return Ok(());
    }
    if probe.ab_props && !minimal {
        let set = set_ab_props(&mut r);
        println!("  ab props accepted {set} of {}", AB_PROPS.len());
    }
    let mut caller = Jid::new("11223344556677", Server::Lid);
    if let Some(device) = probe.caller_device {
        caller = caller.with_device(device);
    }
    let now = r.virtual_unix_time();
    let shifted: u64 = (now as i64 + probe.t_offset).max(0) as u64;
    // MS_PAIR=1: millisecond-consistent timestamps on every channel (wrapper
    // `t` and `e`, inner `t`, args 4/5), expiry 45s after offer. Overrides the
    // per-probe timestamp flags: earlier t-ms probes kept `e` in seconds
    // (expiry before offer), so they could not test millisecond reading.
    let ms_pair = std::env::var("MS_PAIR").is_ok();
    let t_all: u64 = if ms_pair {
        now * 1000
    } else if probe.t_millis {
        shifted * 1000
    } else {
        shifted
    };
    let e_all: u64 = if ms_pair {
        now * 1000 + 45000
    } else {
        shifted + if probe.expiry { 45 } else { 0 }
    };
    let offer = if probe.video {
        census_video_offer_at(probe.inner_t.then_some(t_all))
    } else {
        match common::settings_offer(&caller, shifted, CALL_ID, SETTINGS)
            .content
            .clone()
        {
            Some(wacore_binary::node::NodeContent::Nodes(children)) => children
                .iter()
                .find(|c| c.tag == "offer")
                .cloned()
                .expect("settings carrier holds an <offer>"),
            _ => anyhow::bail!("settings carrier has no children"),
        }
    };
    // The settings sibling rides inside <call>, next to <offer>, on the wire.
    // The wrapper carries a stanza `id`: the engine files the record under it
    // (an idless record keeps transaction_id -1), and the pinned IR requires
    // it on inbound `<call>`.
    let mut wrapper = NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("id", "1")
        .attr("call-id", CALL_ID)
        .attr(
            "call-creator",
            match probe.creator_device {
                Some(device) => caller.with_device(device),
                None => caller.clone(),
            },
        )
        .attr("t", t_all.to_string());
    if probe.expiry || ms_pair {
        wrapper = wrapper.attr("e", e_all.to_string());
    }
    let wrapper = wrapper
        .children([
            offer,
            NodeBuilder::new("voip_settings")
                .attr("uncompressed", "1")
                .bytes(SETTINGS.to_vec())
                .build(),
        ])
        .build();
    let second_numeric = match probe.second_numeric_offset {
        Some(offset) => shifted + offset,
        None => shifted,
    };
    // Arguments 4 and 5 are the stanza's `e` and `t` timestamps, read with
    // stoull (see `tests/signaling.rs::deliver`): the probe axes ride here,
    // not only on the stanza attributes, because the engine reads the header
    // inputs. `second_numeric_offset` shifts `t` into the future.
    let (e_arg, t_arg) = if ms_pair {
        (e_all, t_all)
    } else {
        (
            shifted + if probe.expiry { 45 } else { 0 },
            if probe.t_millis {
                second_numeric * 1000
            } else {
                second_numeric
            },
        )
    };
    let payload = base64::engine::general_purpose::STANDARD.encode(marshal::marshal(&wrapper)?);
    // Preserve the delivery outcome: a trap here must read as a delivery
    // failure, never as "the engine processed and rejected the offer".
    // DELIVER_VIA=message routes through the generic `handleIncomingSignalingMessage`
    // entry (payload, platform, version, e, t, bool, caller, bytes) instead of
    // the offer-specific one: same bytes, different router.
    let via_message = std::env::var("DELIVER_VIA").is_ok_and(|v| v == "message");
    // DELIVER_TWICE=1 feeds the same offer again on the same engine: the
    // message-buffer scan decides 14/15/27 by what it finds buffered, so a
    // second delivery meeting the first one's record must behave differently
    // if buffer state is the gate.
    let twice = std::env::var("DELIVER_TWICE").is_ok();
    let rounds = if twice { 2 } else { 1 };
    let mut delivered = String::new();
    for round in 0..rounds {
        let outcome = if via_message {
            r.call_embind(
                "handleIncomingSignalingMessage",
                &[
                    Value::Str(payload.clone()),
                    Value::Str("web".into()),
                    Value::Str("2.3000.0".into()),
                    Value::Str(e_arg.to_string()),
                    Value::Str(t_arg.to_string()),
                    Value::Bool(false),
                    Value::Str(caller.to_string()),
                    Value::Bytes(Vec::new()),
                ],
            )
        } else {
            r.call_embind(
                "handleIncomingSignalingOffer",
                &[
                    Value::Str(payload.clone()),
                    Value::Str("web".into()),
                    Value::Str("2.3000.0".into()),
                    Value::Str(e_arg.to_string()),
                    Value::Str(t_arg.to_string()),
                    Value::Bool(false),
                    Value::Bool(true),
                    Value::Str(caller.to_string()),
                    Value::Bytes(Vec::new()),
                ],
            )
        };
        r.refuel();
        delivered = match &outcome {
            Ok(value) => format!("{value:?}"),
            Err(_) => "trap".to_owned(),
        };
        if twice {
            println!("PROBE {}: delivery round {round}: {delivered}", probe.label);
        }
    }
    // Collect queued main-thread work to observable quiescence before reading
    // state: a fixed pass count can sample while activation is still pending,
    // and processing a callback can enqueue more work. Each pass yields briefly
    // so guest workers are actually scheduled between observations. A `false`
    // here marks the probe suspect, not conclusive.
    let mut stable = 0;
    let (mut last_signaling, mut last_log) = (usize::MAX, usize::MAX);
    let mut quiesced = false;
    for _ in 0..20 {
        r.process_queued_calls();
        r.refuel();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let (signaling, log) = (
            r.signaling().map(|s| s.len()).unwrap_or(usize::MAX),
            r.engine_log().len(),
        );
        if signaling == last_signaling && log == last_log {
            stable += 1;
            if stable >= 3 {
                quiesced = true;
                break;
            }
        } else {
            stable = 0;
            (last_signaling, last_log) = (signaling, log);
        }
    }
    // Did the inbound blob land in the applied settings store? The offer
    // carries caller_timeout=45; an empty read-back means the expiry path
    // computes against an unapplied (zero) window and every offer is
    // instantly stale no matter what timestamps ride it.
    let applied_timeout = match r.call_embind(
        "getVoipParam",
        &[Value::Str("options.caller_timeout".into())],
    ) {
        Ok(Value::Str(s)) if s.is_empty() => "empty".to_owned(),
        Ok(Value::Str(s)) => s,
        other => format!("{other:?}"),
    };
    r.refuel();
    println!(
        "PROBE {}: applied caller_timeout={applied_timeout}",
        probe.label
    );
    if std::env::var("LOG_SPAN").is_ok() {
        let lines = r.engine_log();
        if let Some(start) = lines.iter().position(|l| l.contains("!Offer from:")) {
            for line in lines.iter().skip(start).take(60) {
                println!("    span: {}", line.trim());
            }
        } else {
            println!("    span: no !Offer line; last 10 lines:");
            for line in lines.iter().rev().take(10).rev() {
                println!("    span: {}", line.trim());
            }
        }
        for line in r
            .logs()
            .iter()
            .filter(|l| l.contains("host stat:"))
            .take(10)
        {
            println!("    {line}");
        }
        return Ok(());
    }
    let immediate = r.call_embind("getCallInfo", &[]);
    r.refuel();
    // A trapped state query is a host failure, not a dead call: it is a
    // distinct `unknown` outcome, never folded into dead.
    let alive: Option<bool> = match &immediate {
        Err(e) => {
            println!("PROBE {}: getCallInfo trapped: {e}", probe.label);
            None
        }
        Ok(Value::Str(s)) => Some(!s.is_empty()),
        Ok(other) => {
            println!(
                "PROBE {}: unexpected getCallInfo shape: {other:?}",
                probe.label
            );
            Some(true)
        }
    };
    // Was it already torn down before accept, or still pending? Snapshot the
    // teardown markers now: if `missed by the user` is logged pre-accept, the
    // call was born torn down (timestamp/parse issue); if absent, accept raced
    // an activation that never completes (missing-state issue).
    let torn_down_pre_accept = r
        .engine_log()
        .iter()
        .any(|l| l.contains("missed by the user") || l.contains("call_term_reason"));
    // SKIP_ACCEPT=1: do not race activation. Feed, settle like `load_settings`
    // does, then read state: if the call is alive here, the premature accept
    // is what kills it, and the fix is sequencing (wait for active).
    if std::env::var("SKIP_ACCEPT").is_ok() {
        r.settle(std::time::Duration::from_secs(3));
        r.refuel();
        let alive_later: Option<bool> = match r.call_embind("getCallInfo", &[]) {
            Err(e) => {
                println!("PROBE {}: late getCallInfo trapped: {e}", probe.label);
                None
            }
            Ok(Value::Str(s)) => Some(!s.is_empty()),
            Ok(_) => Some(true),
        };
        r.refuel();
        let emitted = r.signaling()?.len();
        let missed = r
            .engine_log()
            .iter()
            .any(|l| l.contains("missed by the user"));
        println!(
            "PROBE {}: delivered={delivered} quiesced={quiesced} alive={alive:?} preaccept_torn_down={torn_down_pre_accept} no-accept: alive_later={alive_later:?} emitted={emitted} missed={missed}",
            probe.label,
        );
        return Ok(());
    }
    // CHECK_CONTEXT=1: replicate the `load_settings` observation. A second
    // `startVoipCall` reporting already-initialized proves a call context
    // object exists even when `getCallInfo` reads empty, separating
    // state (missed) from existence.
    if std::env::var("CHECK_CONTEXT").is_ok() {
        let second = r.call_embind(
            "startVoipCall",
            &[
                Value::Str("11223344556677@lid".into()),
                Value::StringList(vec!["11223344556677:0@lid".into()]),
                Value::Str("probe-second-start".into()),
                Value::Bool(true),
                Value::Str("11223344556677@lid".into()),
                Value::Bool(false),
                Value::Bytes(vec![0xA5; 32]),
            ],
        );
        r.refuel();
        println!("PROBE {}: second start -> {second:?}", probe.label,);
        for line in r.engine_log().iter().rev().take(6).rev() {
            println!("    log: {}", line.trim());
        }
        return Ok(());
    }
    let accepted = r.call_embind(
        "acceptCall",
        &[Value::Bool(probe.accept.0), Value::Bool(probe.accept.1)],
    );
    r.refuel();
    // `settle` drains the proxy queue each tick, but a `false` return means it
    // never quiesced: reporting a stall then turns harness timing into a
    // protocol verdict, so quiescence is printed, not discarded.
    let settled = r.settle(std::time::Duration::from_secs(5));
    r.refuel();
    let emitted = r.signaling()?.len();
    let term_reason = r
        .engine_log()
        .iter()
        .rev()
        .find(|l| l.contains("call_term_reason"))
        .map(|l| l.trim().to_owned())
        .unwrap_or_default();
    let missed = r
        .engine_log()
        .iter()
        .any(|l| l.contains("missed by the user"));
    // Report the elapsed stat the engine logged: it discriminates "reads our
    // timestamp" (follows the shift) from "never finds it" (stays at epoch).
    let elapsed = r
        .engine_log()
        .iter()
        .rev()
        .find(|l| l.contains("call_offer_elapsed_t"))
        .map(|l| l.trim().to_owned())
        .unwrap_or_default();
    println!(
        "PROBE {}: delivered={delivered} quiesced={quiesced} alive={alive:?} preaccept_torn_down={torn_down_pre_accept} accept={accepted:?} settled={settled} emitted={emitted} missed={missed} {term_reason} | {elapsed}",
        probe.label,
    );
    Ok(())
}

fn main() -> Result<()> {
    let which = std::env::var("ENGINE").unwrap_or_else(|_| ENGINE.into());
    let catalog = oracle_core::Catalog::discover()?;
    let entry = catalog.resolve(&which)?;
    let bytes = std::fs::read(&entry.path)?;
    if which == ENGINE {
        assert_eq!(hex::encode(Sha256::digest(&bytes)), ENGINE_SHA);
    }
    println!("engine: {which}");

    for probe in [
        Probe {
            label: "settings-only",
            ab_props: false,
            video: false,
            accept: (true, true),
            expiry: false,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "settings-only+ab",
            ab_props: true,
            video: false,
            accept: (true, true),
            expiry: false,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab",
            ab_props: true,
            video: true,
            accept: (true, true),
            expiry: false,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab,accept-ff",
            ab_props: true,
            video: true,
            accept: (false, false),
            expiry: false,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab+expiry",
            ab_props: true,
            video: true,
            accept: (true, true),
            expiry: true,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "settings-only+expiry",
            ab_props: false,
            video: false,
            accept: (true, true),
            expiry: true,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "settings-only+t-ms",
            ab_props: false,
            video: false,
            accept: (true, true),
            expiry: false,
            t_millis: true,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab+t-ms",
            ab_props: true,
            video: true,
            accept: (true, true),
            expiry: false,
            t_millis: true,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab+future-t",
            ab_props: true,
            video: true,
            accept: (true, true),
            expiry: false,
            t_millis: false,
            second_numeric_offset: Some(45),
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab+inner-t",
            ab_props: true,
            video: true,
            accept: (true, true),
            expiry: false,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: true,
            t_offset: 0,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab+t+100k",
            ab_props: true,
            video: true,
            accept: (true, true),
            expiry: false,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 100000,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab+t-100k",
            ab_props: true,
            video: true,
            accept: (true, true),
            expiry: false,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: -100000,
            caller_device: None,
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab+caller-dev0",
            ab_props: true,
            video: true,
            accept: (true, true),
            expiry: false,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: Some(0),
            creator_device: Some(1),
        },
        Probe {
            label: "video+ab+creator-bare",
            ab_props: true,
            video: true,
            accept: (true, true),
            expiry: false,
            t_millis: false,
            second_numeric_offset: None,
            inner_t: false,
            t_offset: 0,
            caller_device: None,
            creator_device: None,
        },
    ] {
        // PROBE=<label> runs a single probe (default: the whole matrix).
        if let Ok(only) = std::env::var("PROBE")
            && only != probe.label
        {
            continue;
        }
        println!("=== {}", probe.label);
        run_probe(&bytes, &probe)?;
    }
    Ok(())
}
