//! A complete video call through the pinned engine, both sides in one run.
//!
//! Side A (initiator) places a video call with `startVoipCall` and the emitted
//! `<offer>` is captured. Side B (answerer) is fed an offer through
//! `handleIncomingSignalingOffer`, then `acceptCall` is attempted and whatever
//! the engine emits is captured. Each emitted stanza is diffed field by field
//! against the matching whatsapp-rust builder; the run ends with a VERDICTS
//! block naming the first divergence, or the stall point with engine-log
//! evidence when a side emits nothing.
//!
//! All identities are fictitious. Execute in release: debug Cranelift
//! compilation is too slow for meaningful engine deadlines.
//!
//! ```sh
//! cargo run --release -p oracle-core --example video_call_both_sides
//! ```
//!
//! `ENGINE` overrides the pinned module (default `JgwtTQVeWPm`).

use anyhow::{Context, Result, bail};
use base64::Engine as _;
use oracle_core::{Catalog, Runtime, SignalingCall, ThreadPolicy, Value};
use sha2::{Digest, Sha256};
use wacore::stanza::call::{
    AcceptParams, CAPABILITY_VIDEO_OFFER, OfferDeviceKey, OfferParams, build_accept, build_offer,
};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::jid::Server;
use wacore_binary::node::NodeContent;
use wacore_binary::{Jid, Node, marshal};

const ENGINE: &str = "JgwtTQVeWPm";
const ENGINE_SHA: &str = "97259423aea19cc30c1771478e035105cb0d0e64ab4b0297741b62d01deac8db";

const SELF: &str = "15550002222@c.us";
const SELF_DEVICE: &str = "15550002222:0@c.us";
const SELF_LID: &str = "99887766554433:0@lid";
const PEER_LID: &str = "11223344556677@lid";
const PEER_DEVICE: &str = "11223344556677:0@lid";
const CALL_ID: &str = "0011223344556677";
const TC_TOKEN: [u8; 32] = [0xA5; 32];
/// The 32-byte call key the JS layer leaves in `<enc>` for the engine, in the
/// clear: on the wire the child holds a Signal ciphertext that
/// `WAWebVoipValidateAndDecryptEnc` replaces before the engine sees it.
const CALL_KEY: [u8; 32] = [0x5A; 32];
const SETTINGS: &[u8] =
    br#"{"encode":{"use_mlow_codec_v1":"false"},"options":{"enable_48khz_rtp_clock":"false","caller_timeout":"45"}}"#;

fn load_engine() -> Result<Vec<u8>> {
    let which = std::env::var("ENGINE").unwrap_or_else(|_| ENGINE.into());
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve(&which)?;
    let bytes = std::fs::read(&entry.path)?;
    if which == ENGINE {
        assert_eq!(hex::encode(Sha256::digest(&bytes)), ENGINE_SHA);
    }
    Ok(bytes)
}

fn start(bytes: &[u8], identity: [&str; 3]) -> Result<Runtime> {
    for _ in 0..8 {
        let mut r = Runtime::instantiate(bytes)?;
        r.set_thread_policy(ThreadPolicy::Spawn);
        r.set_main_thread_registration(true);
        r.run_ctors()?;
        r.attach_log_ring(4 << 20)?;
        let args = identity.map(|value| Value::Str(value.to_owned()));
        let init = r.call_embind("initVoipStack", &args);
        r.refuel();
        if init.as_ref().ok().and_then(|v| v.as_int()) == Some(0) {
            return Ok(r);
        }
    }
    bail!("initVoipStack never returned 0")
}

/// Wait for the event thread before delivering anything: an offer into the
/// startup gap lands on a half-started engine and reads as a refusal. A
/// missing startup marker is a startup failure, not protocol evidence, so a
/// blown deadline fails loudly instead of letting later verdicts blame the
/// signaling.
fn await_event_thread(r: &mut Runtime, label: &str) -> Result<()> {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(60);
    while std::time::Instant::now() < deadline {
        if r.engine_log()
            .iter()
            .any(|l| l.contains("call_event_proc resumed"))
        {
            return Ok(());
        }
        r.process_queued_calls();
        r.refuel();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    bail!("{label}: event thread never announced itself")
}

/// Drain main-thread work the engine queued, until observable quiescence or a
/// bound: with the main thread registered, answers wait in the proxy queue and
/// only the host takes them out. A fixed pass count can sample state while
/// activation is still pending (processing a callback can enqueue more work),
/// so quiescence — signaling count and log length unchanged across passes — is
/// awaited and reported. Each pass yields briefly so guest workers are actually
/// scheduled between observations; back-to-back passes with no worker progress
/// would declare a false quiescence. Returns whether it was reached.
fn drain(r: &mut Runtime) -> bool {
    const PASSES: usize = 20;
    const STABLE: usize = 3;
    let mut stable = 0;
    let (mut last_signaling, mut last_log) = (usize::MAX, usize::MAX);
    for _ in 0..PASSES {
        r.process_queued_calls();
        r.refuel();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let (signaling, log) = (
            r.signaling().map(|s| s.len()).unwrap_or(usize::MAX),
            r.engine_log().len(),
        );
        if signaling == last_signaling && log == last_log {
            stable += 1;
            if stable >= STABLE {
                return true;
            }
        } else {
            stable = 0;
            (last_signaling, last_log) = (signaling, log);
        }
    }
    false
}

/// Decode one recorded stanza, skipping the stream-flag leading byte.
///
/// Loud on failure: a corrupt or truncated capture must surface as a decode
/// error, never as "the engine emitted nothing" (which would misreport a
/// broken host path as a signaling stall).
fn decode(call: &SignalingCall) -> Result<Node> {
    let body = call
        .stanza
        .get(1..)
        .context("signaling stanza shorter than the stream flag")?;
    Ok(marshal::unmarshal_ref(body)
        .with_context(|| format!("undecodable signaling stanza ({} bytes)", body.len()))?
        .to_owned())
}

/// Truncate at a character boundary: engine Debug output may hold multi-byte
/// characters, and byte-index slicing panics mid-character.
fn clip(s: &str, max_chars: usize) -> &str {
    match s.char_indices().nth(max_chars) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

fn children_of(node: &Node) -> Vec<Node> {
    match &node.content {
        Some(NodeContent::Nodes(children)) => children.clone(),
        _ => vec![],
    }
}

fn child_tags(node: &Node) -> Vec<String> {
    children_of(node)
        .iter()
        .map(|c| c.tag.to_string())
        .collect()
}

/// Field-level diff of two same-level stanzas: child order, every child's
/// attributes, and every child's bytes.
///
/// Two exclusions, both stage mismatches rather than drift. `<enc>` carries
/// fresh ciphertext per run, so its bytes can never agree. And the engine
/// emits the pre-fanout skeleton (bare `count`), while our builder emits the
/// post-fanout per-device form (`v`/`type` added by the JS fan-out the engine
/// never performs) — so only `count` participates for `<enc>`. Per-call
/// `<privacy>` is deterministic inside this harness (both sides are handed the
/// same token), so it compares in full.
fn diff_children(vendor: &Node, rust: &Node) -> Option<String> {
    // Attribute order is not a wire fact (XML attributes are unordered), so
    // signatures sort attributes by name; only the set of pairs matters.
    fn attr_set(node: &Node) -> Vec<(String, String)> {
        let mut attrs: Vec<(String, String)> = node
            .attrs
            .iter()
            .filter(|(k, _)| node.tag != "enc" || k.as_ref() as &str == "count")
            .map(|(k, v)| (k.to_string(), format!("{v:?}")))
            .collect();
        attrs.sort();
        attrs
    }
    fn signature(node: &Node) -> String {
        match &node.content {
            Some(NodeContent::Bytes(_)) if node.tag == "enc" => {
                format!("{} {:?}", node.tag, attr_set(node))
            }
            _ => format!("{} {:?} {:?}", node.tag, attr_set(node), node.content),
        }
    }
    // The action nodes themselves carry `call-id`/`call-creator`: an identifier
    // drift must fail here, not hide behind matching children. (`<enc>`
    // bytes/`v`/`type` stay excluded per the stage rule above.)
    if vendor.tag != rust.tag {
        return Some(format!("action tag {} vs {}", vendor.tag, rust.tag));
    }
    if attr_set(vendor) != attr_set(rust) {
        return Some(format!(
            "action attrs {:?} vs {:?}",
            attr_set(vendor),
            attr_set(rust)
        ));
    }
    let (v_children, r_children) = (children_of(vendor), children_of(rust));
    if child_tags(vendor) != child_tags(rust) {
        return Some(format!(
            "child order {:?} vs {:?}",
            child_tags(vendor),
            child_tags(rust)
        ));
    }
    v_children
        .iter()
        .zip(r_children.iter())
        .enumerate()
        .find_map(|(i, (v, r))| {
            (signature(v) != signature(r)).then(|| {
                format!(
                    "child {i} <{}> differs:\n  vendor {v:?}\n  rust   {r:?}",
                    v.tag
                )
            })
        })
}

/// Side A: place a video call, return the emitted `<offer>` node. A trapped
/// or nonzero start is a host failure, not an empty offer: fail loudly
/// instead of judging the signaling that never ran.
fn side_a_initiator(bytes: &[u8]) -> Result<(Node, String)> {
    let mut r = start(bytes, [SELF, SELF_DEVICE, SELF_LID])?;
    await_event_thread(&mut r, "side A")?;
    let outcome = r.call_embind(
        "startVoipCall",
        &[
            Value::Str(PEER_LID.into()),
            Value::StringList(vec![PEER_DEVICE.into()]),
            Value::Str(CALL_ID.into()),
            Value::Bool(true),
            Value::Str(PEER_LID.into()),
            Value::Bool(false),
            Value::Bytes(TC_TOKEN.to_vec()),
        ],
    );
    r.refuel();
    r.settle(std::time::Duration::from_secs(8));
    r.refuel();
    match &outcome {
        Ok(value) if value.as_int() == Some(0) => {}
        other => bail!("side A startVoipCall failed: {other:?}"),
    }
    let stanzas = r.signaling()?;
    let decoded: Vec<(usize, Node)> = stanzas
        .iter()
        .map(|call| decode(call).map(|node| (call.stanza.len(), node)))
        .collect::<Result<_>>()?;
    let (offer_len, offer) = decoded
        .into_iter()
        .find(|(_, n)| n.tag == "offer")
        .context("side A emitted no <offer>")?;
    let info = r.call_embind("getCallInfo", &[]).ok();
    r.refuel();
    println!("side A: startVoipCall -> 0, offer {offer_len} bytes");
    println!("side A: offer children {:?}", child_tags(&offer));
    Ok((offer, format!("{info:?}")))
}

/// Side B: deliver one inbound `<call>` body, attempt `acceptCall`, and report
/// what the engine emitted plus whether the offer parsed. Returns the emitted
/// stanzas and whether the run quiesced throughout: without quiescence a
/// missing accept is inconclusive host scheduling, not a protocol stall.
/// The third element reports whether activation was ever unobservable
/// (trapped or misshapen state queries), which is likewise inconclusive.
///
/// `identity` is the device under test: the raw probe runs the engine as the
/// peer (the offer's true recipient), the census probe as self.
fn side_b_answerer(
    bytes: &[u8],
    identity: [&str; 3],
    caller: Jid,
    body: Vec<Node>,
    label: &str,
) -> Result<(Vec<Node>, bool, bool)> {
    let mut r = start(bytes, identity)?;
    await_event_thread(&mut r, label)?;
    let now = r.virtual_unix_time();
    let wrapper = NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("id", "1")
        .attr("call-id", CALL_ID)
        .attr("call-creator", caller.with_device(1))
        .attr("t", now.to_string())
        .children(body)
        .build();
    let payload = base64::engine::general_purpose::STANDARD.encode(marshal::marshal(&wrapper)?);
    // A trapped delivery is a host failure, not answerer behavior: bail
    // inconclusive instead of draining, accepting, and reporting a stall on
    // an offer that never arrived.
    let delivered = r
        .call_embind(
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
        .with_context(|| format!("side B [{label}]: offer delivery trapped"))?;
    r.refuel();
    println!("side B [{label}]: delivered={delivered:?}");
    // No settle here: the virtual clock advances per observation, and settling
    // ages the call past `caller_timeout`, tearing it down as missed before
    // anything can accept it (see `signaling_census`). Drain to observable
    // quiescence first so queued main-thread work is collected before state
    // is read; a `false` here marks the verdict suspect, not conclusive.
    let quiesced = drain(&mut r);
    println!("side B [{label}]: quiesced={quiesced}");
    let parsed = r.engine_log().iter().any(|l| l.contains("!Offer from:"));
    // State immediately after delivery, before accept: is the call EVER active,
    // even transiently, or is it born torn down?
    let immediate = r.call_embind("getCallInfo", &[]).ok();
    r.refuel();
    let immediate_state = format!("{immediate:?}");
    println!(
        "side B [{label}]: immediate getCallInfo: {}",
        clip(&immediate_state, 400)
    );
    // Do not race activation: poll for a live call before accepting, bounded
    // so a never-activating offer still terminates the run with a verdict.
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    let mut activated = false;
    // A trapped or misshapen state query is not an empty state: it means
    // activation was never successfully observed, which the final verdict
    // must carry as unknown rather than a clean stall.
    let mut activation_unknown = false;
    while std::time::Instant::now() < deadline {
        match r.call_embind("getCallInfo", &[]) {
            Ok(Value::Str(s)) if !s.is_empty() => {
                activated = true;
                break;
            }
            Ok(Value::Str(_)) => {}
            Ok(other) => {
                println!("side B [{label}]: unexpected getCallInfo shape: {other:?}");
                activation_unknown = true;
            }
            Err(e) => {
                println!("side B [{label}]: getCallInfo trapped: {e}");
                activation_unknown = true;
            }
        }
        r.process_queued_calls();
        r.refuel();
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    println!("side B [{label}]: activated={activated}");
    // A trapped accept is a host failure, not answerer behavior: bail
    // inconclusive instead of reporting a stall on signaling that never ran.
    let accepted = r
        .call_embind("acceptCall", &[Value::Bool(true), Value::Bool(true)])
        .context("side B acceptCall trapped")?;
    r.refuel();
    let settled = r.settle(std::time::Duration::from_secs(5));
    r.refuel();
    let emitted: Vec<Node> = r.signaling()?.iter().map(decode).collect::<Result<_>>()?;
    println!(
        "side B [{label}]: offer parsed={parsed}, acceptCall -> {accepted:?}, settled={settled}, emitted={}",
        emitted.len()
    );
    for line in r.engine_log().iter().rev().take(12).rev() {
        println!("    log: {}", line.trim());
    }
    Ok((emitted, quiesced && settled, activation_unknown))
}

fn voip_settings_sibling() -> Node {
    NodeBuilder::new("voip_settings")
        .attr("uncompressed", "1")
        .bytes(SETTINGS.to_vec())
        .build()
}

/// The census-shaped inbound video offer: clear call key, the vendor's own
/// `<video>` child, and the settings sibling the parser requires.
fn census_video_offer(video: &Node) -> Node {
    NodeBuilder::new("offer")
        .children([
            NodeBuilder::new("audio")
                .attr("enc", "opus")
                .attr("rate", "16000")
                .build(),
            video.clone(),
            NodeBuilder::new("net").attr("medium", "3").build(),
            NodeBuilder::new("enc")
                .attr("count", "0")
                .bytes(CALL_KEY.to_vec())
                .build(),
            NodeBuilder::new("encopt").attr("keygen", "2").build(),
        ])
        .build()
}

fn main() -> Result<()> {
    let bytes = load_engine()?;

    // Side A: initiator.
    let (offer, a_state) = side_a_initiator(&bytes)?;
    let video = children_of(&offer)
        .iter()
        .find(|c| c.tag == "video")
        .cloned()
        .context("side A offer has no <video> child")?;

    // Initiator differential: vendor offer vs our builder, every deterministic
    // field (`<enc>` bytes excluded: fresh ciphertext per run).
    let peer = Jid::new("11223344556677", Server::Lid);
    let creator = Jid::new("99887766554433", Server::Lid);
    let rust_offer = build_offer(&OfferParams {
        call_id: CALL_ID,
        to: &peer,
        call_creator: &creator,
        device_keys: &[OfferDeviceKey {
            device_jid: peer.clone().with_device(0),
            ciphertext: vec![0x42; 32],
            enc_type: "pkmsg".to_string(),
        }],
        privacy_token: Some(&[0xa5; 32]),
        capability: Some(&CAPABILITY_VIDEO_OFFER),
        device_identity: None,
        id: Some("1"),
        multi_device: false,
        video: true,
        audio_rates: &["8000", "16000"],
    });
    let rust_inner = children_of(&rust_offer)
        .iter()
        .find(|c| c.tag == "offer")
        .cloned()
        .context("rust offer wrapper holds no <offer>")?;
    match diff_children(&offer, &rust_inner) {
        None => println!("VERDICT initiator: MATCH on all compared fields"),
        Some(d) => println!("VERDICT initiator: DIVERGENCE: {d}"),
    }

    // Side B, probe 1: the vendor's own offer bytes (real ciphertext, no key),
    // delivered to an engine running as the peer — the offer's true recipient —
    // with Side A as the incoming caller.
    let peer_caller = Jid::new("99887766554433", Server::Lid);
    let (emitted_raw, raw_quiet, _raw_unknown) = side_b_answerer(
        &bytes,
        [
            "11223344556677@c.us",
            "11223344556677:0@c.us",
            "11223344556677:0@lid",
        ],
        peer_caller,
        vec![offer.clone(), voip_settings_sibling()],
        "raw vendor offer",
    )?;
    println!(
        "VERDICT answerer/raw: emitted {} stanza(s), quiesced={raw_quiet}",
        emitted_raw.len()
    );

    // Side B, probe 2: census shape with the vendor's own <video> child,
    // delivered to an engine running as self with the peer as caller.
    let census_caller = Jid::new("11223344556677", Server::Lid);
    let (emitted, quiet, activation_unknown) = side_b_answerer(
        &bytes,
        [SELF, SELF_DEVICE, SELF_LID],
        census_caller.clone(),
        vec![census_video_offer(&video), voip_settings_sibling()],
        "census video offer",
    )?;

    // Answerer differential, if the engine emitted an accept. The vendor value
    // is the inner `<accept>` node while `build_accept` returns the outer
    // `<call>` wrapper, so descend into the wrapper first, as the offer
    // comparison does; comparing wrapper children would report the protocol
    // fields against `["accept"]`.
    //
    // The expectation mirrors the production answer path (`answer_with_ids`):
    // creator and target are the incoming caller, the audio set is exactly the
    // offered rate (a conforming accept can select only what was offered), and
    // a from-start video accept carries no capability child.
    match emitted.iter().find(|n| n.tag == "accept") {
        Some(vendor_accept) => {
            let rust_wrapper = build_accept(&AcceptParams {
                call_id: CALL_ID,
                to: &census_caller.clone().with_device(1),
                id: "2",
                call_creator: &census_caller.clone().with_device(1),
                audio_rates: &["16000"],
                relay_te: None,
                rte: None,
                voip_settings: None,
                capability: None,
                video: true,
                peer_abtest_bucket: None,
                peer_abtest_bucket_id_list: None,
            });
            let rust_accept = children_of(&rust_wrapper)
                .iter()
                .find(|c| c.tag == "accept")
                .cloned()
                .context("rust accept wrapper holds no <accept>")?;
            match diff_children(vendor_accept, &rust_accept) {
                None => println!("VERDICT answerer: MATCH on all compared fields"),
                Some(d) => println!("VERDICT answerer: DIVERGENCE: {d}"),
            }
        }
        None if quiet && !activation_unknown => {
            println!("VERDICT answerer: STALL, no <accept> emitted; nothing to compare")
        }
        None => println!(
            "VERDICT answerer: INCONCLUSIVE, no <accept> emitted without clean activation observation; not a protocol stall"
        ),
    }

    println!("side A call state: {}", clip(&a_state, 300));
    Ok(())
}
