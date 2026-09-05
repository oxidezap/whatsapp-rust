//! Everything the engine says while rejecting an offer.
//!
//! `a_well_formed_offer_is_accepted` answers with two lines on the
//! `JgwtTQVeWPm` capture:
//!
//! ```text
//! VoipSignaling.cpp:767 handleIncomingSignalingOffer from platform web version 2.3000.0
//! VoipSignaling.cpp:826 convertToXmppMsg() conversion_result FAILED
//! ```
//!
//! Two lines is what the default threshold allows, and a rejection that says
//! only "FAILED" names nothing to fix. This raises the threshold first — the
//! level lives at a plain word in memory, `Runtime::set_engine_log_level` — and
//! then delivers the same offer, so the reason is on the record rather than
//! inferred from the disassembly.
//!
//! ```sh
//! cargo run --release --example offer_probe
//! ```

use base64::Engine as _;
use oracle_core::patch::{self, Plan};
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::marshal;

const CALLEE: &str = "15550002222";
const CALL_ID: &str = "probe-call-0001";

/// whatsapp-rust's standard-Opus settings blob.
const VOIP_SETTINGS: &[u8] =
    br#"{"encode":{"use_mlow_codec_v1":"false"},"options":{"enable_48khz_rtp_clock":"false"}}"#;

fn jid(number: &str) -> String {
    format!("{number}@s.whatsapp.net")
}

/// The JID shapes worth trying for `from` / `call-creator`.
///
/// This build answers `invalid domain for device JID` to the one the previous
/// capture accepted, so which shapes it *does* accept is the question, and
/// asking it is cheaper than reading `wa_call_device_jid_from_string`.
const SHAPES: &[(&str, &str)] = &[
    // The four domain values `wa_call_device_jid_from_string` and
    // `wa_call_device_jid_create` both accept, read off the bitmask they share:
    // `1 << domain & 2600`, so 3, 5, 9 and 11. The mapping comes from func
    // 11430 — `call`=3, `lid`=5, 112348=9, `hosted.lid`=11 — and
    // `s.whatsapp.net`=0 is the one the previous capture used.
    ("call:0", "15550001111:0@call"),
    ("lid:0", "15550001111:0@lid"),
    ("hosted.lid:0", "15550001111:0@hosted.lid"),
    ("call", "15550001111@call"),
    ("hosted.lid", "15550001111@hosted.lid"),
];

fn main() -> anyhow::Result<()> {
    let catalog = Catalog::discover()?;
    // Which capture. The comparison that matters is against the previous one:
    // this offer was worked out there, one engine complaint at a time, and the
    // question is what this rollout changed rather than what it dislikes.
    let module = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "JgwtTQVeWPm".to_owned());
    let entry = catalog.resolve(&module)?;
    println!("capture: {module}");
    let bytes = std::fs::read(&entry.path)?;

    // Mark every argument of `wa_call_device_jid_create`, the function that
    // answers "invalid domain for device JID". Its guard is
    // `a0==0 || a1==0 || a2==0 || a4 > 11`, and which of those is the one
    // failing cannot be read off the bytecode — three of the four are plausible.
    let bytes = if std::env::var("PROBE_JID_CREATE").is_ok() {
        // `from_string`, not `create`: the marked run showed `create` is never
        // reached, so the rejection happens entirely inside the parser. Marking
        // its call sites says which branch it bails on — the question a body
        // patch cannot answer, since several of them call the same helper.
        // Walk up from `create`: only func 11466 calls it, and only 11839 and
        // 11848 call *that*. Marking both entries says which of the two is on
        // the path that carries domain 0.
        // `wa_call_jid_clone`'s source JID. Marking the pointer is what turns
        // "which of the 45 call sites" into one reading: the struct it clones
        // carries its domain in the word at offset 0, so the value can be
        // looked up in guest memory afterwards instead of searched for.
        let plan = Plan {
            value_entry: vec![(11_447, 2), (11_438, 4)],
            id_base: patch::DEFAULT_ID_BASE,
            ..Plan::default()
        };
        let (rewritten, map) = patch::instrument(&bytes, &plan)?;
        println!("instrumented the create chain via {}", map.via_symbol);
        for marker in &map.markers {
            println!(
                "  {} = {}",
                marker.id - patch::DEFAULT_ID_BASE,
                marker.detail
            );
        }
        rewritten
    } else {
        bytes
    };

    let mut runtime = Runtime::instantiate(&bytes)?;
    runtime.shared().watch_markers("env::on_call_event_js_sync");
    runtime.set_thread_policy(ThreadPolicy::Spawn);
    // After the constructors: `initLogRingBuffer` is an embind function, and
    // embind registers its API *in* the constructors.
    runtime.run_ctors()?;
    runtime.attach_log_ring(1 << 20)?;

    let started = runtime.call_embind(
        "initVoipStack",
        &[
            // The self JID. `agent_docs/voip_oracle_status.md` records this build wanting a bare
            // LID here where the previous one took a phone number, and the
            // creator check may well be relative to it.
            Value::Str(std::env::var("PROBE_SELF").unwrap_or_else(|_| jid(CALLEE))),
            Value::Str("0".into()),
            Value::Str("{}".into()),
        ],
    )?;
    println!("initVoipStack -> {started:?}");
    runtime.quiesce(std::time::Duration::from_secs(10));

    // Everything the engine is willing to say, not just the two lines its
    // default threshold allows.
    let previous = runtime.set_engine_log_level(9)?;
    println!("log level {previous} -> 9");

    for (name, shape) in SHAPES {
        let before = runtime.engine_log().len();
        deliver(&mut runtime, shape);
        let said: Vec<String> = runtime
            .engine_log()
            .into_iter()
            .skip(before)
            .filter(|line| {
                line.contains("invalid")
                    || line.contains("status")
                    || line.contains("accepted")
                    || line.contains("FAILED")
            })
            .collect();
        println!("--- {name}: {shape}");
        for line in said {
            println!("      {line}");
        }
        let markers = runtime.shared().markers();
        // Read each cloned JID back out of guest memory. Offset 0 is the
        // domain enum; the bytes after it are the user part, and a `std::string`
        // that is short enough lives inline, which is why this prints raw.
        for (id, value) in markers.iter().rev().take(6) {
            if *id - patch::DEFAULT_ID_BASE != 0 {
                continue;
            }
            let Ok(ptr) = u32::try_from(*value) else {
                continue;
            };
            if ptr < 0x10000 {
                continue;
            }
            if let Ok(raw) = runtime.read(ptr, 48) {
                let domain = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);
                let text: String = raw[4..]
                    .iter()
                    .map(|b| {
                        if b.is_ascii_graphic() {
                            *b as char
                        } else {
                            '.'
                        }
                    })
                    .collect();
                println!("      clone src {ptr:#x} domain={domain} bytes={text}");
            }
        }
        if !markers.is_empty() {
            let tail: Vec<String> = markers
                .iter()
                .rev()
                .take(20)
                .map(|(id, value)| format!("{}={value} ({value:#x})", id - patch::DEFAULT_ID_BASE))
                .collect();
            println!("      args (newest first): {}", tail.join("  "));
        }
    }
    Ok(())
}

/// Delivers one offer built around the given creator JID.
fn deliver(runtime: &mut Runtime, creator: &str) {
    let now = runtime.virtual_unix_time();
    // `from` and `call-creator` are separate attributes and only one of them is
    // named in the rejection, so they are varied apart: `PROBE_FROM` overrides
    // the sender while `creator` stays the thing under test.
    let from = std::env::var("PROBE_FROM").unwrap_or_else(|_| creator.to_owned());
    let caller = creator.to_owned();
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
    // `caller_pn` is read by this build's `parse_xmpp_offer` and by nothing in
    // the previous one — it appears in the function's string constants next to
    // `call-creator`, `call-id` and `joinable`. Set it when asked to.
    let mut builder = NodeBuilder::new("call")
        .attr("from", from)
        .attr("call-id", CALL_ID)
        .attr("call-creator", caller);
    if let Ok(pn) = std::env::var("PROBE_CALLER_PN") {
        builder = builder.attr("caller_pn", pn);
    }
    // `fill_common_header_from_incoming_stanza` builds a device JID out of a
    // *user* JID, and the failing one carries domain 0 — which is both
    // `s.whatsapp.net` and the zero a struct is initialised to. An attribute
    // that is simply absent would look exactly like that.
    if let Ok(to) = std::env::var("PROBE_TO") {
        builder = builder.attr("to", to);
    }
    // Arbitrary extra attributes, as `name=value,name=value`. The header's JID
    // is cloned from something that arrives entirely zeroed, so the thing to
    // look for is an attribute that is *absent* rather than wrong, and the
    // module's own strings name the candidates.
    // Leaked on purpose: `attr` wants a `&'static str` for the name, and this
    // is a probe that exits straight after.
    if let Ok(extra) = std::env::var("PROBE_ATTRS") {
        for pair in extra.split(',').filter(|p| !p.is_empty()) {
            if let Some((name, value)) = pair.split_once('=') {
                let name: &'static str = Box::leak(name.to_owned().into_boxed_str());
                builder = builder.attr(name, value.to_owned());
            }
        }
    }
    let node = builder
        .attr("t", now.to_string())
        .children([
            offer,
            NodeBuilder::new("voip_settings")
                .attr("uncompressed", "1")
                .bytes(VOIP_SETTINGS.to_vec())
                .build(),
        ])
        .build();

    // With the transport flag byte, which is what the engine's reader accepts.
    let encoded = marshal::marshal(&node).expect("marshal");
    let payload = base64::engine::general_purpose::STANDARD.encode(&encoded[..]);

    let _outcome = runtime.call_embind(
        "handleIncomingSignalingOffer",
        &[
            Value::Str(payload),
            Value::Str("web".into()),
            Value::Str("2.3000.0".into()),
            Value::Str(now.to_string()),
            Value::Str(now.to_string()),
            Value::Bool(false),
            Value::Bool(false),
            Value::Str(String::new()),
            Value::Bytes(Vec::new()),
        ],
    );
    runtime.quiesce(std::time::Duration::from_secs(6));
}
