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

fn start(bytes: &[u8]) -> Result<Runtime> {
    for _ in 0..8 {
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
    bail!("initVoipStack never returned 0")
}

/// Decode one recorded stanza, skipping the stream-flag leading byte.
fn decode(call: &SignalingCall) -> Option<Node> {
    call.stanza
        .get(1..)
        .map(marshal::unmarshal_ref)
        .transpose()
        .ok()
        .flatten()
        .map(|node| node.to_owned())
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

/// Field-level diff of the vendor offer against our video-offer builder.
/// Returns the first divergence, or `None` when the deterministic fields agree.
/// Random per-call bytes (`<enc>` ciphertext, `<privacy>`) are excluded: the
/// engine mints fresh keys every run, so byte equality there is unachievable.
fn diff_offer(vendor: &Node, rust: &Node) -> Option<String> {
    let v_tags = child_tags(vendor);
    let r_tags = child_tags(rust);
    if v_tags != r_tags {
        return Some(format!("child order {v_tags:?} vs {r_tags:?}"));
    }
    let attr = |node: &Node, tag: &str, name: &str| {
        children_of(node)
            .iter()
            .find(|c| c.tag == tag)
            .and_then(|n| {
                n.attrs()
                    .optional_string(name)
                    .as_deref()
                    .map(str::to_owned)
            })
    };
    for name in [
        "enc",
        "dec",
        "device_orientation",
        "screen_width",
        "screen_height",
    ] {
        if attr(vendor, "video", name) != attr(rust, "video", name) {
            return Some(format!(
                "video attr {name}: {:?} vs {:?}",
                attr(vendor, "video", name),
                attr(rust, "video", name)
            ));
        }
    }
    let cap = |node: &Node| {
        children_of(node)
            .iter()
            .find(|c| c.tag == "capability")
            .and_then(|n| match &n.content {
                Some(NodeContent::Bytes(bytes)) => Some(bytes.clone()),
                _ => None,
            })
    };
    if cap(vendor) != cap(rust) {
        return Some("capability bytes differ".to_owned());
    }
    None
}

/// Side A: place a video call, return the emitted `<offer>` node.
fn side_a_initiator(bytes: &[u8]) -> Result<(Node, String)> {
    let mut r = start(bytes)?;
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
    let ret = outcome.as_ref().ok().and_then(|v| v.as_int());
    let stanzas = r.signaling()?;
    let total: usize = stanzas.iter().map(|s| s.stanza.len()).sum();
    let offer = stanzas
        .iter()
        .filter_map(decode)
        .find(|n| n.tag == "offer")
        .context("side A emitted no <offer>")?;
    let info = r.call_embind("getCallInfo", &[]).ok();
    r.refuel();
    println!("side A: startVoipCall -> {ret:?}, offer {total} bytes");
    println!("side A: offer children {:?}", child_tags(&offer));
    Ok((offer, format!("{info:?}")))
}

/// Side B: deliver one inbound `<call>` body, attempt `acceptCall`, and report
/// what the engine emitted plus whether the offer parsed.
fn side_b_answerer(bytes: &[u8], body: Vec<Node>, label: &str) -> Result<Vec<Node>> {
    let mut r = start(bytes)?;
    let caller = Jid::new("11223344556677", Server::Lid);
    let now = r.virtual_unix_time();
    let wrapper = NodeBuilder::new("call")
        .attr("from", caller.clone())
        .attr("call-id", CALL_ID)
        .attr("call-creator", caller.with_device(1))
        .attr("t", now.to_string())
        .children(body)
        .build();
    let payload = base64::engine::general_purpose::STANDARD.encode(marshal::marshal(&wrapper)?);
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
    // No settle here: the virtual clock advances per observation, and settling
    // ages the call past `caller_timeout`, tearing it down as missed before
    // anything can accept it (see `signaling_census`).
    let parsed = r.engine_log().iter().any(|l| l.contains("!Offer from:"));
    // State immediately after delivery, before accept: is the call EVER active,
    // even transiently, or is it born torn down?
    let immediate = r.call_embind("getCallInfo", &[]).ok();
    r.refuel();
    let immediate_state = format!("{immediate:?}");
    println!(
        "side B [{label}]: immediate getCallInfo: {}",
        &immediate_state[..immediate_state.len().min(400)]
    );
    let accepted = r.call_embind("acceptCall", &[Value::Bool(true), Value::Bool(true)]);
    r.refuel();
    r.settle(std::time::Duration::from_secs(5));
    r.refuel();
    let emitted: Vec<Node> = r.signaling()?.iter().filter_map(decode).collect();
    println!(
        "side B [{label}]: offer parsed={parsed}, acceptCall -> {accepted:?}, emitted={}",
        emitted.len()
    );
    for line in r.engine_log().iter().rev().take(12).rev() {
        println!("    log: {}", line.trim());
    }
    Ok(emitted)
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

    // Initiator differential: vendor offer vs our builder on deterministic fields.
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
    match diff_offer(&offer, &rust_inner) {
        None => println!("VERDICT initiator: MATCH on deterministic fields"),
        Some(d) => println!("VERDICT initiator: DIVERGENCE: {d}"),
    }

    // Side B, probe 1: the vendor's own offer bytes (real ciphertext, no key).
    let emitted_raw = side_b_answerer(
        &bytes,
        vec![offer.clone(), voip_settings_sibling()],
        "raw vendor offer",
    )?;
    println!(
        "VERDICT answerer/raw: emitted {} stanza(s)",
        emitted_raw.len()
    );

    // Side B, probe 2: census shape with the vendor's own <video> child.
    let emitted = side_b_answerer(
        &bytes,
        vec![census_video_offer(&video), voip_settings_sibling()],
        "census video offer",
    )?;

    // Answerer differential, if the engine emitted an accept. The vendor value
    // is the inner `<accept>` node while `build_accept` returns the outer
    // `<call>` wrapper, so descend into the wrapper first, as the offer
    // comparison does; comparing wrapper children would report the protocol
    // fields against `["accept"]`.
    match emitted.iter().find(|n| n.tag == "accept") {
        Some(vendor_accept) => {
            let rust_wrapper = build_accept(&AcceptParams {
                call_id: CALL_ID,
                to: &peer,
                id: "2",
                call_creator: &creator,
                audio_rates: &["8000", "16000"],
                relay_te: None,
                rte: None,
                voip_settings: None,
                capability: Some(&CAPABILITY_VIDEO_OFFER),
                video: true,
                peer_abtest_bucket: None,
                peer_abtest_bucket_id_list: None,
            });
            let rust_accept = children_of(&rust_wrapper)
                .iter()
                .find(|c| c.tag == "accept")
                .cloned()
                .context("rust accept wrapper holds no <accept>")?;
            println!(
                "VERDICT answerer: vendor accept children {:?} vs rust {:?}",
                child_tags(vendor_accept),
                child_tags(&rust_accept)
            );
        }
        None => println!("VERDICT answerer: STALL, no <accept> emitted; nothing to compare"),
    }

    println!("side A call state: {}", &a_state[..a_state.len().min(300)]);
    Ok(())
}
