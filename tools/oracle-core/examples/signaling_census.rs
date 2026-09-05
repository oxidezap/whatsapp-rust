//! Which stanzas does the engine emit, across the call lifecycle?
//!
//! `offer_differential` compares one stanza because one was all that had been
//! captured. Extending that comparison needs the other side of it first: what
//! the engine emits when a call is accepted, rejected or ended, and under which
//! tags. This drives each path on its own instance and prints every stanza that
//! reaches the host, decoded.
//!
//! Each scenario gets a fresh engine because the engine keeps call state and a
//! second start on a used one reports `Call context has already been
//! initialized` — a failure about the harness rather than about the path.
//!
//! ## The answer: exactly one
//!
//! ```text
//! incoming offer alone       nothing
//! accept an incoming call    nothing   (acceptCall -> Void)
//! reject an incoming call    nothing   (rejectCall -> Void)
//! end an outgoing call       nothing   (endCall -> trap; the <offer> came earlier)
//! ```
//!
//! **`<offer>` is the only stanza this engine can currently be made to emit**, so
//! the differential in `offer_differential` cannot be extended to `<accept>`,
//! `<reject>`, `<terminate>` or `<transport>` yet: there is nothing on the
//! engine's side to compare whatsapp-rust's builders against.
//!
//! The reason is upstream of all three. An incoming offer is parsed —
//! `!Offer from:6677:0@lid call_id:0102030405060708`, and
//! `wa_call_handle_incoming_xmpp_offer() status 0` — and then torn down
//! immediately:
//!
//! ```text
//! record_incoming_msg: no active call
//! Application settings not loaded
//! fs call_offer_elapsed_t : 1609459200      <-- an absolute timestamp, not a delta
//! fs call_term_reason     : 27
//! EVENT: Call missed by the user
//! ```
//!
//! So no call is ever active, and `acceptCall`/`rejectCall` have nothing to
//! answer: `Failed accept (call not active)`, status `670007`.
//!
//! Ruled out, each by measurement:
//!
//!   * **Ageing.** The virtual clock advances per observation, so settling after
//!     the offer could have aged it past `caller_timeout`. Removing the settle
//!     entirely changes nothing (`SETTLE_AFTER_OFFER=n` puts it back).
//!   * **The missing call key.** WA Web replaces the `<enc>` ciphertext with the
//!     32-byte `callKey` before the engine sees it, and this harness sent no
//!     `<enc>` at all. Adding one changes nothing.
//!   * **Sending only the inner child.** Notes elsewhere say the base64 carries
//!     the `<offer>` alone rather than the `<call>` wrapper. Measured, that is
//!     *worse*: the engine then logs no `!Offer` line at all, so it parses
//!     nothing. The wrapper is required here (`SEND_INNER_ONLY=1` to compare).
//!
//! `call_offer_elapsed_t` reading as `1609459200` — the epoch this harness starts
//! its virtual clock at — is the sharpest remaining lead: the engine is computing
//! an elapsed time against a zero base, which is what an offer timestamp it never
//! found would produce. Where it looks for that timestamp is the next thing to
//! find; the argument list is the obvious candidate, since this passes the same
//! value for both of the two unnamed numeric arguments.
//!
//! ```sh
//! cargo run --release --example signaling_census
//! SEND_INNER_ONLY=1 cargo run --release --example signaling_census
//! SETTLE_AFTER_OFFER=4 cargo run --release --example signaling_census
//! ```
use anyhow::{Result, bail};
use base64::Engine as _;
use oracle_core::{Catalog, Runtime, SignalingCall, ThreadPolicy, Value};
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
const INCOMING_CALL_ID: &str = "0102030405060708";
const TC_TOKEN: [u8; 32] = [0xA5; 32];
/// The 32-byte call key the JS layer leaves in `<enc>` for the engine.
const CALL_KEY: [u8; 32] = [0x5A; 32];

/// An incoming offer, in the shape the engine's parser accepts: LID identities
/// throughout, and `<voip_settings uncompressed="1">` as a sibling of `<offer>`.
fn incoming_offer(caller: &Jid, now: u64) -> Node {
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
                    // The call key, in the clear.
                    //
                    // On the wire this child holds a Signal ciphertext; WA Web's
                    // `WAWebVoipValidateAndDecryptEnc` decrypts it, pulls the
                    // 32-byte `callKey` out of the `Message.Call` protobuf and
                    // calls `unsafeSetNodeContent(callKey)` before handing the
                    // stanza to the engine. So the engine expects the key, not
                    // the ciphertext, and an offer without this child is one it
                    // cannot establish a call from — which is why it was being
                    // torn down immediately as `Call missed by the user`.
                    NodeBuilder::new("enc")
                        .attr("count", "0")
                        .bytes(CALL_KEY.to_vec())
                        .build(),
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

fn engine(bytes: &[u8]) -> Result<Runtime> {
    const ATTEMPTS: usize = 8;
    for _ in 0..ATTEMPTS {
        let mut r = Runtime::instantiate(bytes)?;
        r.set_thread_policy(ThreadPolicy::Spawn);
        r.set_main_thread_registration(true);
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
    bail!("initVoipStack never returned 0 in {ATTEMPTS} attempts")
}

/// Hand the engine an incoming offer, so the paths that need an active call
/// have one.
fn feed_offer(r: &mut Runtime) -> Result<()> {
    let caller = Jid::new("11223344556677", Server::Lid);
    let now = r.virtual_unix_time();
    // The `<call>` wrapper is sent, not just the inner `<offer>`.
    //
    // Notes elsewhere say WA Web encodes only the inner child —
    // `encodeB64(encodeStanza(parsedNode.node()))` — with the wrapper's
    // attributes reaching the engine through the argument list. Measured here,
    // that is **worse**: with only the `<offer>` the engine never even logs
    // `!Offer from:… call_id:…`, so it parses nothing, while with the wrapper it
    // reads both and reports `status 0`. `SEND_INNER_ONLY=1` reproduces the
    // comparison.
    let wrapper = incoming_offer(&caller, now);
    let node = if std::env::var("SEND_INNER_ONLY").is_ok() {
        match &wrapper.content {
            Some(wacore_binary::node::NodeContent::Nodes(children)) => children
                .iter()
                .find(|c| c.tag == "offer")
                .cloned()
                .expect("the wrapper holds an <offer>"),
            _ => wrapper,
        }
    } else {
        wrapper
    };
    let payload = base64::engine::general_purpose::STANDARD.encode(marshal::marshal(&node)?);
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
    // **Do not settle here.** The virtual clock advances per observation, and
    // the engine makes millions of host calls: settling for a few real seconds
    // ages the call past `caller_timeout` and it is torn down as
    // `Call missed by the user` (`call_term_reason: 27`) before anything can
    // accept it. That is what made the incoming path look broken.
    //
    // Overridable so the effect itself is measurable rather than asserted.
    if let Ok(secs) = std::env::var("SETTLE_AFTER_OFFER")
        && let Ok(secs) = secs.parse::<u64>()
    {
        r.settle(std::time::Duration::from_secs(secs));
    }
    Ok(())
}

fn originate(r: &mut Runtime) {
    r.call_embind(
        "startVoipCall",
        &[
            Value::Str(PEER_LID.into()),
            Value::StringList(vec![PEER_LID_DEVICE.into()]),
            Value::Str(OUTGOING_CALL_ID.into()),
            Value::Bool(false),
            Value::Str(PEER_LID.into()),
            Value::Bool(false),
            Value::Bytes(TC_TOKEN.to_vec()),
        ],
    )
    .ok();
    r.refuel();
    r.settle(std::time::Duration::from_secs(8));
}

/// Print every stanza the host was handed, decoded.
fn report(label: &str, calls: &[SignalingCall]) {
    if calls.is_empty() {
        println!("  (nothing)");
        return;
    }
    for call in calls {
        // +1 skips the stream flag the transport puts in front of a node.
        let decoded = call
            .stanza
            .get(1..)
            .map(marshal::unmarshal_ref)
            .transpose()
            .ok()
            .flatten();
        match decoded {
            Some(node) => println!(
                "  <{}> to {} / {} — {} bytes\n      {node:?}",
                node.tag,
                call.peer_jid,
                call.call_id,
                call.stanza.len()
            ),
            None => println!(
                "  UNDECODABLE to {} / {} — {} bytes",
                call.peer_jid,
                call.call_id,
                call.stanza.len()
            ),
        }
    }
    let _ = label;
}

fn main() -> Result<()> {
    let which = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "S_ivh1PriOA".into());
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve(&which)?;
    let bytes = std::fs::read(&entry.path)?;
    println!("{}\n", entry.id);

    // Each scenario: what to do, and whether it needs an incoming call first.
    // `acceptCall` and `rejectCall` act on the *incoming* call; `endCall` is
    // driven on an outgoing one so both directions are covered.
    let scenarios: &[(&str, bool, &str, Vec<Value>)] = &[
        ("incoming offer alone", true, "", vec![]),
        (
            "accept an incoming call",
            true,
            "acceptCall",
            vec![Value::Bool(false), Value::Bool(false)],
        ),
        ("reject an incoming call", true, "rejectCall", vec![]),
        (
            "end an outgoing call",
            false,
            "endCall",
            vec![Value::Int(0), Value::Bool(false)],
        ),
    ];

    for (label, incoming, method, args) in scenarios {
        println!("=== {label}");
        let mut r = engine(&bytes)?;
        if *incoming {
            feed_offer(&mut r)?;
        } else {
            originate(&mut r);
        }

        let before = r.signaling().len();
        if !method.is_empty() {
            let outcome = r.call_embind(method, args);
            r.refuel();
            r.settle(std::time::Duration::from_secs(6));
            println!(
                "  {method} -> {}",
                match &outcome {
                    Ok(v) => format!("{v:?}"),
                    Err(_) => "trap".into(),
                }
            );
        }

        for line in r.engine_log().iter().rev().take(26).rev() {
            println!("      log: {}", line.trim());
        }
        let all = r.signaling();
        if before > 0 {
            println!("  before this step:");
            report(label, &all[..before]);
        }
        println!("  from this step:");
        report(label, &all[before..]);
        println!();
    }

    Ok(())
}
