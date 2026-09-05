//! Where does a from-scratch client's offer differ from the vendor's own?
//!
//! Two implementations now produce the same artifact and neither has seen the
//! other's source: WhatsApp's VoIP engine, running as wasm, emits an `<offer>`
//! through `sendSignalingXMPP_js_sync`; whatsapp-rust's
//! `wacore::stanza::call::build_offer` builds one from the protocol as it was
//! reverse-engineered. This puts them side by side.
//!
//! **This is the measurement the whole "run the vendor's own logic" argument
//! rests on.** A re-implemented client drifts from the real one in ways nobody
//! wrote down, and every such difference is a bit of behavioural fingerprint.
//! Until now that claim had no instance attached to it. Each row below where
//! the two disagree is one.
//!
//! What it is not: a verdict on which is *right*. The engine is the vendor's,
//! so where they differ the engine is the reference by construction — but the
//! JS layer above it edits the stanza before it reaches the wire, so a
//! difference can also be work the browser does that the engine leaves undone.
//!
//! ## The result
//!
//! **Child order identical. Six of seven children identical. One difference.**
//!
//! ```text
//! privacy → audio(8000) → audio(16000) → net → capability → enc → encopt
//!
//!  ==  <privacy> [32 bytes]
//!  ==  <audio enc="opus" rate="8000">
//!  ==  <audio enc="opus" rate="16000">
//!  ==  <net medium="3">
//!  ==  <capability ver="1"> [7 bytes]
//!  !=  engine        <enc count="0"> [32 bytes]
//!      whatsapp-rust <enc count="0" type="pkmsg" v="2"> [32 bytes]
//!  ==  <encopt keygen="2">
//! ```
//!
//! That is a strong result for the re-implementation: the tag set, the ordering,
//! the attribute names and values, and the payload lengths all agree, on a
//! stanza neither side copied from the other.
//!
//! And the one difference is very likely **not** a defect. `<enc>` is the field
//! the JS layer mediates in both directions, so the engine handling it in the
//! clear is the division of labour rather than an omission:
//!
//!   * inbound is **confirmed** in the bundle — `WAWebVoipValidateAndDecryptEnc`
//!     Signal-decrypts the `<enc>` child and calls
//!     `unsafeSetNodeContent(decrypted)` before the stanza reaches the engine,
//!     under an `AppTrackerType.VoipOfferDecrypt` mark;
//!   * outbound is the **symmetric inference**: the engine puts the 32-byte call
//!     key in `<enc>` unencrypted and without `v`/`type`, and the JS encrypts it
//!     and stamps the Signal message type — which is exactly what
//!     whatsapp-rust, having no JS layer, does itself.
//!
//! Stated as an inference because it is one: the outbound half was not traced to
//! its site in the bundle, only reasoned from the inbound half plus the shape of
//! what the engine emits.
//!
//! So the honest reading is that on this stanza the re-implemented client is
//! **byte-shape faithful**, and the sole divergence is a layer boundary rather
//! than drift. A useful negative result for the fingerprint argument: it says
//! the drift this project exists to eliminate is not visible *here*, and points
//! the search at the paths nobody has compared yet.
//!
//! ```sh
//! cargo run --release --example offer_differential
//! ```
use anyhow::{Result, bail};
use oracle_core::{Catalog, Runtime, ThreadPolicy, Value};
use std::collections::BTreeMap;
use wacore::stanza::call::{OfferDeviceKey, OfferParams, build_offer};
use wacore_binary::jid::Server;
use wacore_binary::node::{NodeContent, NodeContentRef};
use wacore_binary::{Jid, Node, NodeRef, marshal};

const SELF: &str = "15550002222@c.us";
const SELF_DEVICE: &str = "15550002222:0@c.us";
const SELF_LID: &str = "99887766554433:0@lid";
const PEER_LID: &str = "11223344556677@lid";
const PEER_LID_DEVICE: &str = "11223344556677:0@lid";
const CALL_ID: &str = "0011223344556677";
/// What this hands `startVoipCall`, and what the engine puts in `<privacy>`.
const TC_TOKEN: [u8; 32] = [0xA5; 32];

/// Drive the engine to the point where it emits an offer, and return it.
fn engine_offer(bytes: &[u8]) -> Result<(String, Vec<u8>)> {
    const ATTEMPTS: usize = 8;
    for _ in 0..ATTEMPTS {
        let mut r = Runtime::instantiate(bytes)?;
        r.set_thread_policy(ThreadPolicy::Spawn);
        r.set_main_thread_registration(true);
        r.run_ctors()?;
        let init = r.call_embind(
            "initVoipStack",
            &[
                Value::Str(SELF.into()),
                Value::Str(SELF_DEVICE.into()),
                Value::Str(SELF_LID.into()),
            ],
        );
        r.refuel();
        if init.as_ref().ok().and_then(|v| v.as_int()) != Some(0) {
            continue;
        }

        r.call_embind(
            "startVoipCall",
            &[
                Value::Str(PEER_LID.into()),
                Value::StringList(vec![PEER_LID_DEVICE.into()]),
                Value::Str(CALL_ID.into()),
                Value::Bool(false),
                Value::Str(PEER_LID.into()),
                Value::Bool(false),
                Value::Bytes(TC_TOKEN.to_vec()),
            ],
        )
        .ok();
        r.refuel();
        r.settle(std::time::Duration::from_secs(8));

        if let Some(call) = r.signaling().into_iter().next() {
            return Ok((call.peer_jid, call.stanza));
        }
    }
    bail!("the engine emitted no signaling in {ATTEMPTS} attempts")
}

/// The same call, as whatsapp-rust would build it.
///
/// Filled to match what the engine was given, so a difference is a difference
/// in the *shape* rather than in the inputs. `audio_rates` is the pair its own
/// documentation names as WhatsApp's, and the ciphertext stands in for the
/// Signal message a real client would have encrypted.
fn rust_offer() -> Node {
    let peer: Jid = Jid::new("11223344556677", Server::Lid);
    let device: Jid = peer.clone().with_device(0);
    let creator: Jid = Jid::new("99887766554433", Server::Lid);
    build_offer(&OfferParams {
        call_id: CALL_ID,
        to: &peer,
        call_creator: &creator,
        device_keys: &[OfferDeviceKey {
            device_jid: device,
            ciphertext: vec![0x42; 32],
            enc_type: "pkmsg".to_string(),
        }],
        privacy_token: Some(&TC_TOKEN),
        capability: Some(&[0x01, 0x05, 0xf7, 0x09, 0xe0, 0xbb, 0x5b]),
        device_identity: None,
        id: Some("1"),
        multi_device: false,
        video: false,
        audio_rates: &["8000", "16000"],
    })
}

/// One child of the offer, reduced to what a comparison can be made over.
#[derive(Debug, PartialEq, Eq)]
struct Child {
    tag: String,
    attrs: BTreeMap<String, String>,
    bytes: Option<usize>,
}

fn children_of_ref(node: &NodeRef<'_>) -> Vec<Child> {
    let Some(NodeContentRef::Nodes(nodes)) = &node.content else {
        return Vec::new();
    };
    nodes
        .iter()
        .map(|c| Child {
            tag: c.tag.to_string(),
            attrs: c
                .attrs
                .as_slice()
                .iter()
                .map(|(k, v)| (k.to_string(), format!("{v:?}")))
                .collect(),
            bytes: match &c.content {
                Some(NodeContentRef::Bytes(b)) => Some(b.len()),
                _ => None,
            },
        })
        .collect()
}

fn children_of(node: &Node) -> Vec<Child> {
    match &node.content {
        Some(NodeContent::Nodes(nodes)) => nodes
            .iter()
            .map(|c| Child {
                tag: c.tag.to_string(),
                attrs: c
                    .attrs
                    .iter()
                    .map(|(k, v)| (k.to_string(), format!("{v:?}")))
                    .collect(),
                bytes: match &c.content {
                    Some(NodeContent::Bytes(b)) => Some(b.len()),
                    _ => None,
                },
            })
            .collect(),
        _ => Vec::new(),
    }
}

fn show(child: &Child) -> String {
    let attrs: Vec<String> = child
        .attrs
        .iter()
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    let body = child
        .bytes
        .map(|n| format!(" [{n} bytes]"))
        .unwrap_or_default();
    format!("<{} {}>{body}", child.tag, attrs.join(" "))
}

fn main() -> Result<()> {
    let which = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "S_ivh1PriOA".into());
    let catalog = Catalog::discover()?;
    let entry = catalog.resolve(&which)?;
    let bytes = std::fs::read(&entry.path)?;

    let (peer, stanza) = engine_offer(&bytes)?;
    // The leading byte is the stream flag the transport puts in front of a node.
    let engine_node = marshal::unmarshal_ref(&stanza[1..])?;
    println!(
        "engine  {} -> <{}> to {peer}, {} bytes on the wire\n",
        entry.id,
        engine_node.tag,
        stanza.len()
    );

    // whatsapp-rust wraps its offer in `<call>`; the engine emits the inner
    // action alone, because the JS layer adds the wrapper. Compare like with
    // like by descending to the `<offer>`.
    let rust_call = rust_offer();
    let rust_offer_node = match &rust_call.content {
        Some(NodeContent::Nodes(nodes)) => nodes
            .iter()
            .find(|n| n.tag == "offer")
            .cloned()
            .expect("build_offer wraps an <offer>"),
        _ => bail!("build_offer produced no children"),
    };

    let engine_children = children_of_ref(&engine_node);
    let rust_children = children_of(&rust_offer_node);

    println!("child order");
    println!(
        "  engine        {}",
        engine_children
            .iter()
            .map(|c| c.tag.as_str())
            .collect::<Vec<_>>()
            .join(" → ")
    );
    println!(
        "  whatsapp-rust {}",
        rust_children
            .iter()
            .map(|c| c.tag.as_str())
            .collect::<Vec<_>>()
            .join(" → ")
    );
    let order_matches = engine_children
        .iter()
        .map(|c| &c.tag)
        .eq(rust_children.iter().map(|c| &c.tag));
    println!(
        "  -> {}\n",
        if order_matches {
            "identical"
        } else {
            "THEY DIFFER"
        }
    );

    println!("child by child");
    let mut differences = 0;
    for (i, engine_child) in engine_children.iter().enumerate() {
        match rust_children.get(i) {
            Some(rust_child) if rust_child == engine_child => {
                println!("  ==  {}", show(engine_child));
            }
            Some(rust_child) => {
                differences += 1;
                println!("  !=  engine        {}", show(engine_child));
                println!("      whatsapp-rust {}", show(rust_child));
            }
            None => {
                differences += 1;
                println!("  --  engine only   {}", show(engine_child));
            }
        }
    }
    for rust_child in rust_children.iter().skip(engine_children.len()) {
        differences += 1;
        println!("  ++  rust only     {}", show(rust_child));
    }

    println!("\n{differences} difference(s).");
    Ok(())
}
