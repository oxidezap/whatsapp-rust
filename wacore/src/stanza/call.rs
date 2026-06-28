//! Parser and builders for `<call>` stanzas. Returns `Ok(None)` on unknown action
//! children so future server additions don't break the handler.
//!
//! Wire shapes (tags, attrs, child order) follow the wacrg spec: call-offer (SIG-01),
//! call-preaccept (SIG-04), call-accept (SIG-03), call-reject (SIG-08),
//! call-terminate (SIG-14), call-ack (SIG-07). The `<offer>` child order is
//! server-enforced; a mis-ordered offer is rejected with error 439.

use anyhow::{Result, anyhow};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Node, NodeRef};

use crate::time::from_secs;
use crate::types::call::{CallAction, CallAudioCodec, IncomingCall};

const KNOWN_ACTIONS: &[&str] = &[
    "offer",
    "offer_notice",
    "preaccept",
    "accept",
    "reject",
    "terminate",
    "transport",
    "relaylatency",
];

pub fn parse_call_stanza(node: &NodeRef<'_>) -> Result<Option<IncomingCall>> {
    if node.tag != "call" {
        return Err(anyhow!("expected <call>, got <{}>", node.tag));
    }

    // Find a known action child first so unknown/future actions short-circuit
    // before attr validation (forward-compat, even if stanza attrs also shift).
    let Some(child) = node
        .children()
        .and_then(|cs| cs.iter().find(|c| KNOWN_ACTIONS.contains(&c.tag.as_ref())))
    else {
        return Ok(None);
    };

    let mut attrs = node.attrs();
    let from = attrs
        .optional_jid("from")
        .ok_or_else(|| anyhow!("<call> missing 'from' attribute"))?;
    // whatsmeow doesn't require an id on <call>, and the signaling-only actions (transport,
    // relaylatency) can arrive without one. Only the offer-ack consumes stanza_id and real offers
    // always carry it, so default a missing id to empty rather than rejecting the whole stanza --
    // which would drop these actions right after we added them to KNOWN_ACTIONS to surface them.
    let stanza_id = attrs
        .optional_string("id")
        .map(|s| s.into_owned())
        .unwrap_or_default();
    let notify = attrs
        .optional_string("notify")
        .and_then(|s| (!s.is_empty()).then(|| s.into_owned()));
    let platform = attrs.optional_string("platform").map(|s| s.into_owned());
    let version = attrs.optional_string("version").map(|s| s.into_owned());
    let ts = attrs
        .optional_unix_time("t")
        .ok_or_else(|| anyhow!("<call> missing or invalid 't' attribute"))?;
    let timestamp = from_secs(ts).ok_or_else(|| anyhow!("<call> 't'={ts} out of range"))?;
    // Server-set presence flag marking an offline-queue replay (WA Web `hasAttr("offline")`), the
    // same idiom we already use for messages/receipts. NOT the `e` attr, which is an offer timestamp.
    let offline = attrs.optional_string("offline").is_some();

    attrs.finish().map_err(|e| anyhow!("<call> attrs: {e}"))?;

    let action = parse_action(child)?;

    Ok(Some(IncomingCall {
        from,
        stanza_id,
        notify,
        platform,
        version,
        timestamp,
        offline,
        action,
        // The media facade (decrypt callKey + connect relay) needs the offer's <enc>/<relay>;
        // capture them only on an <offer> and only when `voip` is on (RelayData lives there).
        #[cfg(feature = "voip")]
        media: (child.tag.as_ref() == "offer")
            .then(|| parse_media_offer(node, child))
            .flatten()
            .map(Box::new),
    }))
}

/// Extract the media material from an `<offer>`: the `<enc>` addressed to us (direct child or under
/// `<destination><to><enc>`) and the `<relay>` block (searched anywhere in the `<call>` subtree, as
/// the example does). Returns `None` when the offer carries no `<enc>` for us (nothing to decrypt).
#[cfg(feature = "voip")]
fn parse_media_offer(
    call: &NodeRef<'_>,
    offer: &NodeRef<'_>,
) -> Option<crate::types::call::MediaOffer> {
    use crate::types::call::{MediaOffer, OfferRecipientEnc};

    let mut encs = Vec::new();
    if let Some(enc_node) = offer.get_optional_child("enc") {
        // Single-device: a bare <enc> child addressed to us directly.
        if let Some(enc) = parse_offer_enc(enc_node) {
            encs.push(OfferRecipientEnc { to: None, enc });
        }
    } else if let Some(dest) = offer.get_optional_child("destination") {
        // Multi-device: one <to jid><enc> per recipient device. Keep every entry with its jid so the
        // facade decrypts the one for THIS device instead of whichever happens to be first.
        for to in dest
            .children()
            .unwrap_or_default()
            .iter()
            .filter(|c| c.tag.as_ref() == "to")
        {
            // A <to> destination must carry a parseable jid; skip a malformed one rather than pushing
            // `to: None`, which is the bare-<enc> (directly-addressed) sentinel. Read the TYPED wire
            // JID via `to_jid`, never `as_str().parse()`: a LID arrives as an AD-JID whose agent byte
            // the string form suppresses (`renders_agent(Lid)` is false), so reparsing the string
            // drops it to agent 0 and the `<to>` would never equal our own (agent-carrying) LID.
            if let Some(enc_node) = to.get_optional_child("enc")
                && let Some(enc) = parse_offer_enc(enc_node)
                && let Some(to_jid) = to.get_attr("jid").and_then(|v| v.to_jid())
            {
                encs.push(OfferRecipientEnc {
                    to: Some(to_jid),
                    enc,
                });
            }
        }
    }
    if encs.is_empty() {
        return None;
    }

    let relay = find_relay(call).and_then(crate::voip::relay_parse::parse_relay_data);
    Some(MediaOffer { encs, relay })
}

/// Parse one `<enc>` node into the ciphertext plus the wire `type`/`v` needed to decrypt the callKey.
#[cfg(feature = "voip")]
fn parse_offer_enc(enc_node: &NodeRef<'_>) -> Option<crate::types::call::OfferEnc> {
    use crate::types::call::OfferEnc;
    let ciphertext = enc_node.content_bytes()?.to_vec();
    let enc_type = enc_node
        .get_attr("type")
        .map(|v| v.as_str().to_string())
        .unwrap_or_else(|| "pkmsg".into());
    let version = enc_node
        .get_attr("v")
        .and_then(|v| v.as_str().parse::<u8>().ok())
        .unwrap_or(2);
    Some(OfferEnc {
        enc_type,
        version,
        ciphertext,
    })
}

/// Find the first `<relay>` node anywhere in the subtree (the offer's relay may sit under `<call>`
/// or `<offer>` depending on server framing).
#[cfg(feature = "voip")]
pub fn find_relay<'a, 'b>(nr: &'b NodeRef<'a>) -> Option<&'b NodeRef<'a>> {
    if nr.tag.as_ref() == "relay" {
        return Some(nr);
    }
    nr.children().and_then(|cs| cs.iter().find_map(find_relay))
}

fn parse_audio_codec(node: &NodeRef<'_>) -> Result<CallAudioCodec> {
    let mut a = node.attrs();
    let enc = a
        .required_string("enc")
        .map_err(|e| anyhow!("<audio> missing 'enc': {e}"))?
        .into_owned();
    let rate_raw = a
        .optional_u64("rate")
        .ok_or_else(|| anyhow!("<audio enc={enc}> missing or invalid 'rate'"))?;
    let rate = u32::try_from(rate_raw)
        .map_err(|_| anyhow!("<audio enc={enc}> 'rate'={rate_raw} overflows u32"))?;
    a.finish().map_err(|e| anyhow!("<audio> attrs: {e}"))?;
    Ok(CallAudioCodec { enc, rate })
}

fn parse_action(node: &NodeRef<'_>) -> Result<CallAction> {
    let mut attrs = node.attrs();
    let call_id = attrs
        .required_string("call-id")
        .map_err(|e| anyhow!("<{}> missing 'call-id': {e}", node.tag))?
        .into_owned();
    let call_creator = attrs
        .optional_jid("call-creator")
        .ok_or_else(|| anyhow!("<{}> missing 'call-creator'", node.tag))?;

    Ok(match node.tag.as_ref() {
        "offer" => {
            let caller_pn = attrs.optional_jid("caller_pn");
            let caller_country_code = attrs
                .optional_string("caller_country_code")
                .map(|s| s.into_owned());
            let device_class = attrs
                .optional_string("device_class")
                .map(|s| s.into_owned());
            let joinable = attrs
                .optional_string("joinable")
                .map(|s| s == "1")
                .unwrap_or(false);
            let group_jid = attrs.optional_jid("group-jid");

            attrs.finish().map_err(|e| anyhow!("<offer> attrs: {e}"))?;

            let children = node.children().unwrap_or_default();
            let is_video = children.iter().any(|c| c.tag == "video");
            let audio = children
                .iter()
                .filter(|c| c.tag == "audio")
                .map(parse_audio_codec)
                .collect::<Result<Vec<_>>>()?;

            CallAction::Offer {
                call_id,
                call_creator,
                caller_pn,
                caller_country_code,
                device_class,
                joinable,
                is_video,
                audio,
                group_jid,
            }
        }
        "offer_notice" => {
            let is_video = attrs.optional_string("media").is_some_and(|s| s == "video");
            let is_group = attrs.optional_string("type").is_some_and(|s| s == "group");
            attrs
                .finish()
                .map_err(|e| anyhow!("<offer_notice> attrs: {e}"))?;
            CallAction::OfferNotice {
                call_id,
                call_creator,
                is_video,
                is_group,
            }
        }
        "preaccept" => {
            attrs
                .finish()
                .map_err(|e| anyhow!("<preaccept> attrs: {e}"))?;
            CallAction::PreAccept {
                call_id,
                call_creator,
            }
        }
        "transport" => {
            let p2p_cand_round = attrs
                .optional_string("p2p-cand-round")
                .map(|s| s.into_owned());
            let transport_message_type = attrs
                .optional_string("transport-message-type")
                .map(|s| s.into_owned());
            attrs
                .finish()
                .map_err(|e| anyhow!("<transport> attrs: {e}"))?;
            CallAction::Transport {
                call_id,
                call_creator,
                p2p_cand_round,
                transport_message_type,
            }
        }
        "relaylatency" => {
            attrs
                .finish()
                .map_err(|e| anyhow!("<relaylatency> attrs: {e}"))?;
            CallAction::RelayLatency {
                call_id,
                call_creator,
            }
        }
        "accept" => {
            attrs.finish().map_err(|e| anyhow!("<accept> attrs: {e}"))?;
            CallAction::Accept {
                call_id,
                call_creator,
            }
        }
        "reject" => {
            attrs.finish().map_err(|e| anyhow!("<reject> attrs: {e}"))?;
            CallAction::Reject {
                call_id,
                call_creator,
            }
        }
        "terminate" => {
            // WA Web gates the call-log outcome on `reason` (missed vs accepted/rejected-elsewhere),
            // so surface it instead of dropping it.
            let reason = attrs.optional_string("reason").map(|c| c.into_owned());
            let duration = attrs
                .optional_u64("duration")
                .and_then(|v| u32::try_from(v).ok());
            let audio_duration = attrs
                .optional_u64("audio_duration")
                .and_then(|v| u32::try_from(v).ok());
            attrs
                .finish()
                .map_err(|e| anyhow!("<terminate> attrs: {e}"))?;
            CallAction::Terminate {
                call_id,
                call_creator,
                reason,
                duration,
                audio_duration,
            }
        }
        other => return Err(anyhow!("unreachable: unknown action <{other}>")),
    })
}

/// Build `<receipt to=caller id=stanza_id [from=own_ad]><offer call-id call-creator/></receipt>`
/// for acknowledging an incoming `<offer>`. Pure so it can be unit-tested
/// without a live socket.
pub fn build_offer_ack_receipt(call: &IncomingCall, own_ad: Option<&Jid>) -> Option<Node> {
    let CallAction::Offer {
        call_id,
        call_creator,
        ..
    } = &call.action
    else {
        return None;
    };

    let mut receipt = NodeBuilder::new("receipt")
        .attr("to", &call.from)
        .attr("id", call.stanza_id.as_str());
    if let Some(jid) = own_ad {
        receipt = receipt.attr("from", jid);
    }

    let offer = NodeBuilder::new("offer")
        .attr("call-id", call_id.as_str())
        .attr("call-creator", call_creator)
        .build();

    Some(receipt.children([offer]).build())
}

// --- Outbound call-signaling builders ---
//
// Pure `Node` builders for offer/accept/preaccept/transport/relaylatency/heartbeat/
// terminate/mute/reject. They need no codec/crypto, so they live in core (not behind
// the `voip` feature): a consumer can reject/terminate/answer signaling without the
// mlow codec. The `<offer>` child order is load-bearing (server returns 439 if wrong).
//
// Stanza ids generated from random bytes (heartbeat, preaccept) are passed in so the
// builders stay pure; the I/O layer supplies them.

/// Capability blob for `<offer>`/`<accept>` (ver=1).
pub const CAPABILITY_OFFER: [u8; 7] = [0x01, 0x05, 0xf7, 0x09, 0xe4, 0xbb, 0x13];
/// Capability blob for `<preaccept>` (ver=1).
pub const CAPABILITY_PREACCEPT: [u8; 7] = [0x01, 0x05, 0xf7, 0x09, 0xe4, 0xbb, 0x07];

/// `<terminate reason>` wire tokens. The caller-driven sibling dismiss SENDS the `*_ELSEWHERE`
/// ones; the receive path maps every reason to a call-log outcome (see WA Web's
/// `ActionWebHandleIncomingSignalingMessage`): `TIMEOUT`/`GROUP_CALL_ENDED`/absent -> missed.
pub const TERMINATE_REASON_ACCEPTED_ELSEWHERE: &str = "accepted_elsewhere";
pub const TERMINATE_REASON_REJECTED_ELSEWHERE: &str = "rejected_elsewhere";
pub const TERMINATE_REASON_TIMEOUT: &str = "timeout";
pub const TERMINATE_REASON_GROUP_CALL_ENDED: &str = "group_call_ended";

/// Relay latency wire encoding: `0x2000000 + rtt_ms`.
pub fn encode_latency(rtt_ms: u32) -> String {
    (0x0200_0000u32.wrapping_add(rtt_ms)).to_string()
}

/// One per-device encrypted callKey entry inside `<offer>`.
pub struct OfferDeviceKey {
    pub device_jid: Jid,
    pub ciphertext: Vec<u8>,
    /// Signal message type: `pkmsg` or `msg`.
    pub enc_type: String,
}

pub struct OfferParams<'a> {
    pub call_id: &'a str,
    pub to: &'a Jid,
    pub call_creator: &'a Jid,
    pub device_keys: &'a [OfferDeviceKey],
    pub privacy_token: Option<&'a [u8]>,
    pub capability: Option<&'a [u8]>,
    pub device_identity: Option<&'a [u8]>,
    /// Stanza `id` on the `<call>` wrapper. Required for the server to ack-correlate the offer; the
    /// initiator's relay arrives in that `<ack type=offer>` reply, so without an id there is no ack
    /// to wait on. Pure builder, so the I/O layer supplies the generated id.
    pub id: Option<&'a str>,
    /// True when the callee resolved to more than one device, even if only one survived encryption:
    /// keep the `<destination><to jid>` shape so the surviving device stays explicitly addressed (a
    /// bare `<enc>` drops its jid and could misroute to the primary device).
    pub multi_device: bool,
}

/// `<call to=peer><offer call-id call-creator>…</offer></call>` with the mandatory
/// child order: privacy → audio(8k) → audio(16k) → net → capability → destination|enc →
/// encopt → device-identity.
pub fn build_offer(p: &OfferParams<'_>) -> Node {
    let mut children: Vec<Node> = Vec::new();
    if let Some(privacy) = p.privacy_token {
        children.push(NodeBuilder::new("privacy").bytes(privacy.to_vec()).build());
    }
    children.push(
        NodeBuilder::new("audio")
            .attr("enc", "opus")
            .attr("rate", "8000")
            .build(),
    );
    children.push(
        NodeBuilder::new("audio")
            .attr("enc", "opus")
            .attr("rate", "16000")
            .build(),
    );
    children.push(NodeBuilder::new("net").attr("medium", "3").build());
    if let Some(cap) = p.capability {
        children.push(capability_node(cap));
    }

    if p.device_keys.len() > 1 || p.multi_device {
        let to_nodes: Vec<Node> = p
            .device_keys
            .iter()
            .map(|dk| {
                NodeBuilder::new("to")
                    .attr("jid", &dk.device_jid)
                    .children([enc_node(dk)])
                    .build()
            })
            .collect();
        children.push(NodeBuilder::new("destination").children(to_nodes).build());
    } else if let Some(dk) = p.device_keys.first() {
        children.push(enc_node(dk));
    }

    children.push(encopt_node());
    if let Some(di) = p.device_identity {
        children.push(
            NodeBuilder::new("device-identity")
                .bytes(di.to_vec())
                .build(),
        );
    }

    call_wrap(
        p.to,
        p.id,
        offer_action("offer", p.call_id, p.call_creator, children),
    )
}

fn enc_node(dk: &OfferDeviceKey) -> Node {
    NodeBuilder::new("enc")
        .attr("v", "2")
        .attr("type", dk.enc_type.clone())
        .attr("count", "0")
        .bytes(dk.ciphertext.clone())
        .build()
}

pub struct AcceptParams<'a> {
    pub call_id: &'a str,
    pub to: &'a Jid,
    /// The `<call>` wrapper id. Required, not optional: the server only relays/ack-correlates a call
    /// stanza that carries one, so an idless `<accept>` is silently dropped and never reaches the
    /// caller (which then never marks the call accepted, leaving siblings ringing and timing it out).
    pub id: &'a str,
    pub call_creator: &'a Jid,
    /// Advertised `<audio enc=opus rate=…>` formats, in preference order. Selecting only `8000`
    /// is the lever to steer the caller off Meta's 16 kHz mlow codec onto RFC Opus NB.
    pub audio_rates: &'a [&'a str],
    pub relay_te: Option<&'a [u8]>,
    pub rte: Option<&'a [u8]>,
    pub voip_settings: Option<&'a [u8]>,
    pub capability: Option<&'a [u8]>,
}

/// `<accept>`: audio → [te priority=2] → net medium=2 → encopt → [capability] → [rte] → [voip_settings].
pub fn build_accept(p: &AcceptParams<'_>) -> Node {
    let mut children: Vec<Node> = p.audio_rates.iter().map(|rate| audio_opus(rate)).collect();
    if let Some(te) = p.relay_te {
        children.push(
            NodeBuilder::new("te")
                .attr("priority", "2")
                .bytes(te.to_vec())
                .build(),
        );
    }
    children.push(NodeBuilder::new("net").attr("medium", "2").build());
    children.push(encopt_node());
    if let Some(cap) = p.capability {
        children.push(capability_node(cap));
    }
    if let Some(rte) = p.rte {
        children.push(NodeBuilder::new("rte").bytes(rte.to_vec()).build());
    }
    if let Some(vs) = p.voip_settings {
        children.push(
            NodeBuilder::new("voip_settings")
                .attr("uncompressed", "1")
                .bytes(vs.to_vec())
                .build(),
        );
    }
    call_wrap(
        p.to,
        Some(p.id),
        offer_action("accept", p.call_id, p.call_creator, children),
    )
}

/// One `<audio enc=opus rate=…>` advertisement child.
fn audio_opus(rate: &str) -> Node {
    NodeBuilder::new("audio")
        .attr("enc", "opus")
        .attr("rate", rate)
        .build()
}

/// `<encopt keygen=2>`: selects the v2 SRTP key path. Shared by offer/accept/preaccept.
fn encopt_node() -> Node {
    NodeBuilder::new("encopt").attr("keygen", "2").build()
}

/// `<capability ver=1>` carrying its fixed blob. Shared by offer/accept/preaccept.
fn capability_node(blob: &[u8]) -> Node {
    NodeBuilder::new("capability")
        .attr("ver", "1")
        .bytes(blob.to_vec())
        .build()
}

/// `<preaccept>`: audio → encopt → capability(preaccept blob). `id` is the random call-wrapper id.
pub fn build_preaccept(
    call_id: &str,
    to: &Jid,
    call_creator: &Jid,
    wrapper_id: &str,
    audio_rates: &[&str],
) -> Node {
    let mut children: Vec<Node> = audio_rates.iter().map(|rate| audio_opus(rate)).collect();
    children.push(encopt_node());
    children.push(capability_node(&CAPABILITY_PREACCEPT));
    call_wrap(
        to,
        Some(wrapper_id),
        offer_action("preaccept", call_id, call_creator, children),
    )
}

pub struct TransportParams<'a> {
    pub call_id: &'a str,
    pub to: &'a Jid,
    pub call_creator: &'a Jid,
    pub p2p_cand_round: Option<&'a str>,
    pub transport_message_type: Option<&'a str>,
    pub relay_te: Option<&'a [u8]>,
}

/// `<transport>`: optional `<te priority=1>` then `<net medium=2 [protocol=0]>`.
pub fn build_transport(p: &TransportParams<'_>) -> Node {
    let mut action = NodeBuilder::new("transport")
        .attr("call-id", p.call_id)
        .attr("call-creator", p.call_creator);
    if let Some(round) = p.p2p_cand_round {
        action = action.attr("p2p-cand-round", round.to_string());
    }
    if let Some(mt) = p.transport_message_type {
        action = action.attr("transport-message-type", mt.to_string());
    }

    let mut children: Vec<Node> = Vec::new();
    if let Some(te) = p.relay_te {
        children.push(
            NodeBuilder::new("te")
                .attr("priority", "1")
                .bytes(te.to_vec())
                .build(),
        );
    }
    let mut net = NodeBuilder::new("net").attr("medium", "2");
    if p.transport_message_type != Some("9") {
        net = net.attr("protocol", "0");
    }
    children.push(net.build());

    call_wrap(p.to, None, action.children(children).build())
}

pub struct RelayLatencyParams<'a> {
    pub call_id: &'a str,
    pub to: &'a Jid,
    pub call_creator: &'a Jid,
    pub latency_ms: u32,
    pub relay_name: &'a str,
    pub address_bytes: &'a [u8],
    /// Peer devices; omit for inbound callee.
    pub devices: &'a [Jid],
}

/// `<relaylatency>` with a `<te latency relay_name>` and optional `<destination>`.
pub fn build_relay_latency(p: &RelayLatencyParams<'_>) -> Node {
    let mut children: Vec<Node> = vec![
        NodeBuilder::new("te")
            .attr("latency", encode_latency(p.latency_ms))
            .attr("relay_name", p.relay_name.to_string())
            .bytes(p.address_bytes.to_vec())
            .build(),
    ];
    if !p.devices.is_empty() {
        children.push(destination_to(p.devices));
    }
    call_wrap(
        p.to,
        None,
        offer_action("relaylatency", p.call_id, p.call_creator, children),
    )
}

/// `<call to={call_id}@call id=…><heartbeat call-id call-creator/></call>`.
pub fn build_heartbeat(call_id: &str, call_creator: &Jid, wrapper_id: &str) -> Node {
    let action = NodeBuilder::new("heartbeat")
        .attr("call-id", call_id)
        .attr("call-creator", call_creator)
        .build();
    NodeBuilder::new("call")
        .attr("to", format!("{call_id}@call"))
        .attr("id", wrapper_id.to_string())
        .children([action])
        .build()
}

pub struct TerminateParams<'a> {
    pub call_id: &'a str,
    /// The device the terminate is addressed to. WhatsApp routes call signaling per device, so a
    /// sibling dismiss (`accepted_elsewhere`) sends one stanza per device JID, NOT a single
    /// `<destination>` block (that fan-out is gated to `offer`/`enc_rekey` in WA Web).
    pub to: &'a Jid,
    /// The `<call>` wrapper id. WA Web sets a generated id on every call stanza.
    pub id: Option<&'a str>,
    pub call_creator: &'a Jid,
    pub reason: Option<&'a str>,
}

pub fn build_terminate(p: &TerminateParams<'_>) -> Node {
    let mut action = NodeBuilder::new("terminate")
        .attr("call-id", p.call_id)
        .attr("call-creator", p.call_creator);
    if let Some(reason) = p.reason {
        action = action.attr("reason", reason.to_string());
    }
    call_wrap(p.to, p.id, action.build())
}

pub fn build_mute_v2(call_id: &str, to: &Jid, call_creator: &Jid, mute_state: &str) -> Node {
    let action = NodeBuilder::new("mute_v2")
        .attr("call-id", call_id)
        .attr("call-creator", call_creator)
        .attr("mute-state", mute_state.to_string())
        .build();
    call_wrap(to, None, action)
}

/// `<call to=peer id=wrapper_id><reject call-id call-creator count="0"/></call>`.
/// `count="0"` and the wrapper `id` match WA Web's reject wire shape.
pub fn build_reject(call_id: &str, to: &Jid, call_creator: &Jid, wrapper_id: &str) -> Node {
    call_wrap(
        to,
        Some(wrapper_id),
        NodeBuilder::new("reject")
            .attr("call-id", call_id)
            .attr("call-creator", call_creator)
            .attr("count", "0")
            .build(),
    )
}

fn offer_action(tag: &'static str, call_id: &str, call_creator: &Jid, children: Vec<Node>) -> Node {
    NodeBuilder::new(tag)
        .attr("call-id", call_id)
        .attr("call-creator", call_creator)
        .children(children)
        .build()
}

fn destination_to(devices: &[Jid]) -> Node {
    let tos: Vec<Node> = devices
        .iter()
        .map(|jid| NodeBuilder::new("to").attr("jid", jid).build())
        .collect();
    NodeBuilder::new("destination").children(tos).build()
}

fn call_wrap(to: &Jid, id: Option<&str>, action: Node) -> Node {
    let mut call = NodeBuilder::new("call").attr("to", to);
    if let Some(id) = id {
        call = call.attr("id", id.to_string());
    }
    call.children([action]).build()
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::{Jid, Server};

    fn fake_caller_lid() -> Jid {
        Jid::new("111111111111111", Server::Lid)
    }

    fn fake_caller_pn() -> Jid {
        Jid::new("15555550100", Server::Pn)
    }

    fn base_call_builder() -> NodeBuilder {
        NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "STANZA-ID-0001")
            .attr("version", "2.25.37.76")
            .attr("platform", "android")
            .attr("notify", "Test Caller")
            .attr("t", "1766847151")
            .attr("e", "0")
    }

    fn offer_builder_base() -> NodeBuilder {
        NodeBuilder::new("offer")
            .attr("call-creator", fake_caller_lid())
            .attr("call-id", "CALL-ID-0001")
    }

    fn as_ref<'a>(n: &'a wacore_binary::Node) -> NodeRef<'a> {
        n.as_node_ref()
    }

    // An offer carrying an <enc> (the encrypted callKey) and a <relay> must surface both on
    // IncomingCall.media so the media facade can decrypt the callKey and connect the relay without
    // re-walking the raw stanza. Covers the bare-<enc> form; the <destination><to><enc> form is the
    // multi-device variant the parser also accepts.
    #[cfg(feature = "voip")]
    #[test]
    fn offer_captures_enc_and_relay_for_media() {
        let relay = NodeBuilder::new("relay")
            .children([
                NodeBuilder::new("warp_mi_tag_len")
                    .bytes(b"4".to_vec())
                    .build(),
                NodeBuilder::new("token")
                    .attr("id", "0")
                    .bytes(vec![0xaa, 0xbb])
                    .build(),
                NodeBuilder::new("te2")
                    .attr("relay_id", "1")
                    .attr("relay_name", "gru1c02")
                    .attr("token_id", "0")
                    .attr("auth_token_id", "1")
                    .bytes(vec![157, 240, 226, 133, 0x0d, 0x96])
                    .build(),
            ])
            .build();
        let node = base_call_builder()
            .children([
                offer_builder_base()
                    .children([NodeBuilder::new("enc")
                        .attr("v", "2")
                        .attr("type", "pkmsg")
                        .bytes(vec![1, 2, 3, 4])
                        .build()])
                    .build(),
                relay,
            ])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        let media = call.media.expect("offer with <enc> must capture media");
        let enc = media
            .enc_for(None)
            .expect("a bare <enc> is addressed to us");
        assert_eq!(enc.enc_type, "pkmsg");
        assert_eq!(enc.version, 2);
        assert_eq!(enc.ciphertext, vec![1, 2, 3, 4]);
        let rd = media.relay.expect("the <relay> must be parsed");
        assert_eq!(rd.warp_mi_tag_len, Some(4));
        assert_eq!(rd.relay_tokens[0], vec![0xaa, 0xbb]);
        assert_eq!(rd.endpoints[0].relay_name, "gru1c02");
    }

    // An offer with no <enc> for us (e.g. a different device's destination) yields media=None: there
    // is nothing to decrypt, so the media facade has nothing to drive.
    #[cfg(feature = "voip")]
    #[test]
    fn offer_without_enc_has_no_media() {
        let node = base_call_builder()
            .children([offer_builder_base()
                .children([NodeBuilder::new("audio")
                    .attr("enc", "opus")
                    .attr("rate", "16000")
                    .build()])
                .build()])
            .build();
        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        assert!(call.media.is_none());
    }

    // A multi-device offer lists one <to jid><enc> per recipient device. The parser keeps every
    // entry, and enc_for selects by OUR device jid, not by child order, so a linked (non-first)
    // device decrypts its own callKey instead of another device's.
    #[cfg(feature = "voip")]
    #[test]
    fn offer_multi_device_selects_enc_for_our_device() {
        let dev1: Jid = "111111111111111:3@lid".parse().unwrap();
        let dev2: Jid = "111111111111111:7@lid".parse().unwrap();
        let to1 = NodeBuilder::new("to")
            .attr("jid", &dev1)
            .children([NodeBuilder::new("enc")
                .attr("v", "2")
                .attr("type", "pkmsg")
                .bytes(vec![0xA1])
                .build()])
            .build();
        let to2 = NodeBuilder::new("to")
            .attr("jid", &dev2)
            .children([NodeBuilder::new("enc")
                .attr("v", "2")
                .attr("type", "msg")
                .bytes(vec![0xB2])
                .build()])
            .build();
        let node = base_call_builder()
            .children([offer_builder_base()
                .children([NodeBuilder::new("destination").children([to1, to2]).build()])
                .build()])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        let media = call.media.expect("multi-device offer captures media");
        // Selected by device jid, not by child order: dev2 is second but resolves to its own enc.
        assert_eq!(media.enc_for(Some(&dev2)).unwrap().ciphertext, vec![0xB2]);
        assert_eq!(media.enc_for(Some(&dev2)).unwrap().enc_type, "msg");
        assert_eq!(media.enc_for(Some(&dev1)).unwrap().ciphertext, vec![0xA1]);
        // A device not listed gets nothing, rather than silently decrypting the wrong key.
        let other: Jid = "222222222222222:1@lid".parse().unwrap();
        assert!(media.enc_for(Some(&other)).is_none());
    }

    // Regression: a LID `<to jid>` decoded from the wire is an AD-JID carrying agent=1 (the Lid domain
    // byte); our own LID is agent=1 too. The parser must read the TYPED jid (`to_jid`), never
    // stringify+reparse it (which drops the agent to 0, since `renders_agent(Lid)` is false), or a
    // multi-device callee never matches its `<to>` and silently fails "offer carried no callKey",
    // even against the real server. The typed Jid value with agent=1 here is exactly what
    // `as_node_ref`/the decoder carries for an AD-JID.
    #[cfg(feature = "voip")]
    #[test]
    fn offer_to_jid_keeps_lid_agent_from_typed_jid() {
        let wire_to = Jid {
            user: "111111111111111".into(),
            server: Server::Lid,
            agent: 1, // the AD-JID domain byte; the string form suppresses it
            device: 7,
            integrator: 0,
        };
        let to = NodeBuilder::new("to")
            .attr("jid", &wire_to)
            .children([NodeBuilder::new("enc")
                .attr("v", "2")
                .attr("type", "msg")
                .bytes(vec![0xB2])
                .build()])
            .build();
        let node = base_call_builder()
            .children([offer_builder_base()
                .children([NodeBuilder::new("destination").children([to]).build()])
                .build()])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        let media = call.media.expect("offer captures media");

        // Our own device LID as get_lid() yields it: agent=1.
        assert_eq!(
            media
                .enc_for(Some(&wire_to))
                .expect("callKey for our device")
                .ciphertext,
            vec![0xB2],
        );
    }

    #[test]
    fn offer_audio_only() {
        let node = base_call_builder()
            .children([offer_builder_base()
                .attr("caller_pn", fake_caller_pn())
                .attr("device_class", "2016")
                .attr("joinable", "1")
                .attr("caller_country_code", "BR")
                .children([
                    NodeBuilder::new("audio")
                        .attr("enc", "opus")
                        .attr("rate", "16000")
                        .build(),
                    NodeBuilder::new("audio")
                        .attr("enc", "opus")
                        .attr("rate", "8000")
                        .build(),
                ])
                .build()])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        assert_eq!(call.stanza_id, "STANZA-ID-0001");
        assert_eq!(call.from, fake_caller_lid());
        assert_eq!(call.timestamp.timestamp(), 1766847151);
        assert!(!call.offline);
        assert_eq!(call.notify.as_deref(), Some("Test Caller"));
        assert_eq!(call.platform.as_deref(), Some("android"));

        match call.action {
            CallAction::Offer {
                call_id,
                call_creator,
                caller_pn,
                caller_country_code,
                device_class,
                joinable,
                is_video,
                audio,
                group_jid,
            } => {
                assert_eq!(call_id, "CALL-ID-0001");
                assert_eq!(call_creator, fake_caller_lid());
                assert_eq!(caller_pn, Some(fake_caller_pn()));
                assert_eq!(caller_country_code.as_deref(), Some("BR"));
                assert_eq!(device_class.as_deref(), Some("2016"));
                assert!(joinable);
                assert!(!is_video);
                assert_eq!(audio.len(), 2);
                assert_eq!(audio[0].enc, "opus");
                assert_eq!(audio[0].rate, 16000);
                assert_eq!(audio[1].rate, 8000);
                assert_eq!(group_jid, None);
            }
            other => panic!("expected Offer, got {other:?}"),
        }
    }

    #[test]
    fn offer_video() {
        let node = base_call_builder()
            .children([offer_builder_base()
                .children([
                    NodeBuilder::new("audio")
                        .attr("enc", "opus")
                        .attr("rate", "16000")
                        .build(),
                    NodeBuilder::new("video").build(),
                ])
                .build()])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        match call.action {
            CallAction::Offer {
                is_video, audio, ..
            } => {
                assert!(is_video);
                assert_eq!(audio.len(), 1);
            }
            other => panic!("expected Offer, got {other:?}"),
        }
    }

    #[test]
    fn offer_minimum_attrs() {
        let node = NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "STANZA-ID-0001")
            .attr("t", "1766847151")
            .children([offer_builder_base().build()])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        assert_eq!(call.notify, None);
        assert_eq!(call.platform, None);
        assert_eq!(call.version, None);
        match call.action {
            CallAction::Offer {
                caller_pn,
                caller_country_code,
                device_class,
                joinable,
                is_video,
                audio,
                ..
            } => {
                assert_eq!(caller_pn, None);
                assert_eq!(caller_country_code, None);
                assert_eq!(device_class, None);
                assert!(!joinable);
                assert!(!is_video);
                assert!(audio.is_empty());
            }
            other => panic!("expected Offer, got {other:?}"),
        }
    }

    #[test]
    fn offer_with_group_jid() {
        let group_jid = Jid::new("123456789", Server::Group);
        let node = base_call_builder()
            .children([offer_builder_base()
                .attr("group-jid", group_jid.clone())
                .children([NodeBuilder::new("audio")
                    .attr("enc", "opus")
                    .attr("rate", "16000")
                    .build()])
                .build()])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        match call.action {
            CallAction::Offer {
                group_jid: parsed_group,
                ..
            } => {
                assert_eq!(parsed_group, Some(group_jid));
            }
            other => panic!("expected Offer, got {other:?}"),
        }
    }

    #[test]
    fn offer_notice_group_audio_call() {
        let node = NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "STANZA-ID-GROUP")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("offer_notice")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "GROUP-CALL-ID")
                .attr("media", "audio")
                .attr("type", "group")
                .build()])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        match call.action {
            CallAction::OfferNotice {
                call_id,
                call_creator,
                is_video,
                is_group,
            } => {
                assert_eq!(call_id, "GROUP-CALL-ID");
                assert_eq!(call_creator, fake_caller_lid());
                assert!(!is_video);
                assert!(is_group);
            }
            other => panic!("expected OfferNotice, got {other:?}"),
        }
    }

    #[test]
    fn offer_notice_video_flag() {
        let node = NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "STANZA-ID-GROUP")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("offer_notice")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "GROUP-CALL-ID")
                .attr("media", "video")
                .attr("type", "group")
                .build()])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        match call.action {
            CallAction::OfferNotice {
                is_video, is_group, ..
            } => {
                assert!(is_video);
                assert!(is_group);
            }
            other => panic!("expected OfferNotice, got {other:?}"),
        }
    }

    #[test]
    fn preaccept_accept_reject_variants() {
        for (tag, expected_variant) in [
            ("preaccept", "pre_accept"),
            ("accept", "accept"),
            ("reject", "reject"),
        ] {
            let node = base_call_builder()
                .children([NodeBuilder::new(tag)
                    .attr("call-creator", fake_caller_lid())
                    .attr("call-id", "CID")
                    .build()])
                .build();

            let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
            assert_eq!(call.action.call_id(), "CID");
            let name = match call.action {
                CallAction::PreAccept { .. } => "pre_accept",
                CallAction::Accept { .. } => "accept",
                CallAction::Reject { .. } => "reject",
                _ => "other",
            };
            assert_eq!(name, expected_variant);
        }
    }

    #[test]
    fn terminate_with_duration() {
        let node = base_call_builder()
            .children([NodeBuilder::new("terminate")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CID")
                .attr("reason", "timeout")
                .attr("duration", "3670")
                .attr("audio_duration", "3670")
                .build()])
            .build();

        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        match call.action {
            CallAction::Terminate {
                reason,
                duration,
                audio_duration,
                ..
            } => {
                assert_eq!(reason.as_deref(), Some("timeout"));
                assert_eq!(duration, Some(3670));
                assert_eq!(audio_duration, Some(3670));
            }
            other => panic!("expected Terminate, got {other:?}"),
        }
    }

    #[test]
    fn transport_and_relaylatency_are_parsed_not_dropped() {
        // Regression: these were missing from KNOWN_ACTIONS and silently dropped (Ok(None)).
        let transport = base_call_builder()
            .children([NodeBuilder::new("transport")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CID")
                .attr("p2p-cand-round", "1")
                .attr("transport-message-type", "3")
                .children([NodeBuilder::new("net").attr("medium", "2").build()])
                .build()])
            .build();
        let call = parse_call_stanza(&as_ref(&transport)).unwrap().unwrap();
        match call.action {
            CallAction::Transport {
                call_id,
                p2p_cand_round,
                transport_message_type,
                ..
            } => {
                assert_eq!(call_id, "CID");
                assert_eq!(p2p_cand_round.as_deref(), Some("1"));
                assert_eq!(transport_message_type.as_deref(), Some("3"));
            }
            other => panic!("expected Transport, got {other:?}"),
        }

        let relaylatency = base_call_builder()
            .children([NodeBuilder::new("relaylatency")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CID")
                .build()])
            .build();
        let call = parse_call_stanza(&as_ref(&relaylatency)).unwrap().unwrap();
        assert!(matches!(call.action, CallAction::RelayLatency { .. }));
    }

    #[test]
    fn idless_call_stanza_parses() {
        // whatsmeow tolerates a <call> with no 'id'; transport/relaylatency can arrive that way.
        // Rejecting it would drop the action, so an absent id parses to an empty stanza_id instead.
        let transport = NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("t", "1766847151")
            .children([NodeBuilder::new("transport")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CID")
                .children([NodeBuilder::new("net").attr("medium", "2").build()])
                .build()])
            .build();
        let call = parse_call_stanza(&as_ref(&transport)).unwrap().unwrap();
        assert_eq!(call.stanza_id, "");
        assert!(matches!(call.action, CallAction::Transport { .. }));
    }

    #[test]
    fn transport_malformed_call_creator_errors() {
        // Malformed attrs on these arms must fail-loud, not produce a defaulted variant.
        let node = base_call_builder()
            .children([NodeBuilder::new("transport")
                .attr("call-creator", "@@not-a-jid@@")
                .attr("call-id", "CID")
                .build()])
            .build();
        assert!(parse_call_stanza(&as_ref(&node)).is_err());
    }

    #[test]
    fn relaylatency_malformed_call_creator_errors() {
        let node = base_call_builder()
            .children([NodeBuilder::new("relaylatency")
                .attr("call-creator", "@@not-a-jid@@")
                .attr("call-id", "CID")
                .build()])
            .build();
        assert!(parse_call_stanza(&as_ref(&node)).is_err());
    }

    #[test]
    fn unknown_action_returns_none() {
        let node = base_call_builder()
            .children([NodeBuilder::new("surprise").build()])
            .build();
        assert!(parse_call_stanza(&as_ref(&node)).unwrap().is_none());
    }

    #[test]
    fn unknown_action_short_circuits_before_attr_validation() {
        // No `t` attr, but unknown action means we never validate it.
        let node = NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "S")
            .children([NodeBuilder::new("surprise").build()])
            .build();
        assert!(parse_call_stanza(&as_ref(&node)).unwrap().is_none());
    }

    #[test]
    fn malformed_audio_missing_enc_errors() {
        let node = base_call_builder()
            .children([offer_builder_base()
                .children([NodeBuilder::new("audio").attr("rate", "16000").build()])
                .build()])
            .build();

        assert!(parse_call_stanza(&as_ref(&node)).is_err());
    }

    #[test]
    fn malformed_audio_missing_rate_errors() {
        let node = base_call_builder()
            .children([offer_builder_base()
                .children([NodeBuilder::new("audio").attr("enc", "opus").build()])
                .build()])
            .build();

        assert!(parse_call_stanza(&as_ref(&node)).is_err());
    }

    #[test]
    fn malformed_audio_rate_overflow_errors() {
        let node = base_call_builder()
            .children([offer_builder_base()
                .children([NodeBuilder::new("audio")
                    .attr("enc", "opus")
                    .attr("rate", "4294967296") // u32::MAX + 1
                    .build()])
                .build()])
            .build();

        assert!(parse_call_stanza(&as_ref(&node)).is_err());
    }

    #[test]
    fn malformed_missing_t_errors() {
        let node = NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "STANZA-ID-0001")
            .children([offer_builder_base().build()])
            .build();

        assert!(parse_call_stanza(&as_ref(&node)).is_err());
    }

    #[test]
    fn offline_delivery_flag() {
        // Offline-queue replay is marked by the PRESENCE of the `offline` attribute (WA Web
        // `hasAttr("offline")`), regardless of value.
        let offline_node = NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "S")
            .attr("t", "1766847151")
            .attr("offline", "1")
            .children([offer_builder_base().build()])
            .build();
        assert!(
            parse_call_stanza(&as_ref(&offline_node))
                .unwrap()
                .unwrap()
                .offline
        );

        // A live offer has no `offline` attr. The `e` attr (an offer timestamp) must NOT be mistaken
        // for the offline flag -- regression guard against the old `optional_string("e")` bug.
        let online_node = NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "S")
            .attr("t", "1766847151")
            .attr("e", "1766847151")
            .children([offer_builder_base().build()])
            .build();
        assert!(
            !parse_call_stanza(&as_ref(&online_node))
                .unwrap()
                .unwrap()
                .offline
        );
    }

    #[test]
    fn build_offer_ack_receipt_matches_wa_web_shape() {
        let node = base_call_builder()
            .children([offer_builder_base().build()])
            .build();
        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        let own = Jid::new("222222222222222", Server::Lid).with_device(42);

        let receipt = build_offer_ack_receipt(&call, Some(&own)).unwrap();
        assert_eq!(receipt.tag.as_ref(), "receipt");

        let mut a = receipt.attrs();
        assert_eq!(
            a.required_string("to").unwrap(),
            fake_caller_lid().to_string()
        );
        assert_eq!(a.required_string("id").unwrap(), "STANZA-ID-0001");
        assert_eq!(a.required_string("from").unwrap(), own.to_string());

        let offer = receipt.get_optional_child("offer").unwrap();
        let mut oa = offer.attrs();
        assert_eq!(oa.required_string("call-id").unwrap(), "CALL-ID-0001");
        assert_eq!(
            oa.required_string("call-creator").unwrap(),
            fake_caller_lid().to_string()
        );
    }

    #[test]
    fn build_offer_ack_receipt_returns_none_for_non_offer() {
        let node = base_call_builder()
            .children([NodeBuilder::new("reject")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "X")
                .build()])
            .build();
        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        assert!(build_offer_ack_receipt(&call, None).is_none());
    }

    #[test]
    fn build_offer_ack_receipt_omits_from_when_own_ad_missing() {
        let node = base_call_builder()
            .children([offer_builder_base().build()])
            .build();
        let call = parse_call_stanza(&as_ref(&node)).unwrap().unwrap();
        let receipt = build_offer_ack_receipt(&call, None).unwrap();
        let mut a = receipt.attrs();
        assert!(a.optional_string("from").is_none());
    }

    // --- Outbound signaling builder tests ---

    fn peer() -> Jid {
        Jid::new("111111111111111", Server::Lid)
    }
    fn creator() -> Jid {
        Jid::new("222222222222222", Server::Lid).with_device(19)
    }

    fn child_tags(call: &Node) -> Vec<String> {
        let r: NodeRef<'_> = call.as_node_ref();
        let action = &r.children().unwrap()[0];
        action
            .children()
            .unwrap()
            .iter()
            .map(|c| c.tag.as_ref().to_string())
            .collect()
    }

    // Conformance gate for wacrg call-offer (SIG-01): the `<offer>` child order
    // privacy → audio(8000) → audio(16000) → net → capability → (enc|destination)
    // → encopt → device-identity is server-enforced (a mis-ordered offer is rejected
    // with error 439), so this asserts the exact normative order.
    #[test]
    fn offer_child_order_is_load_bearing() {
        let peer = peer();
        let creator = creator();
        let dk = OfferDeviceKey {
            device_jid: peer.clone(),
            ciphertext: vec![1, 2, 3],
            enc_type: "pkmsg".into(),
        };
        let call = build_offer(&OfferParams {
            call_id: "CID",
            to: &peer,
            call_creator: &creator,
            device_keys: std::slice::from_ref(&dk),
            privacy_token: Some(&[0xaa, 0xbb]),
            capability: Some(&CAPABILITY_OFFER),
            device_identity: Some(&[0xcc]),
            id: Some("OFFER-STANZA-ID"),
            multi_device: false,
        });
        // Single device key → bare <enc> (not <destination>).
        assert_eq!(
            child_tags(&call),
            [
                "privacy",
                "audio",
                "audio",
                "net",
                "capability",
                "enc",
                "encopt",
                "device-identity"
            ]
        );
        let r = call.as_node_ref();
        assert_eq!(r.tag.as_ref(), "call");
        // The stanza id lands on the <call> wrapper so the server can ack-correlate the offer.
        assert_eq!(
            r.attrs().optional_string("id").as_deref(),
            Some("OFFER-STANZA-ID")
        );
        let offer = &r.children().unwrap()[0];
        assert_eq!(offer.tag.as_ref(), "offer");
        assert_eq!(
            offer.attrs().optional_string("call-id").as_deref(),
            Some("CID")
        );
    }

    #[test]
    fn offer_multi_device_uses_destination() {
        let peer = peer();
        let creator = creator();
        let keys = vec![
            OfferDeviceKey {
                device_jid: peer.clone(),
                ciphertext: vec![1],
                enc_type: "pkmsg".into(),
            },
            OfferDeviceKey {
                device_jid: creator.clone(),
                ciphertext: vec![2],
                enc_type: "msg".into(),
            },
        ];
        let call = build_offer(&OfferParams {
            call_id: "CID",
            to: &peer,
            call_creator: &creator,
            device_keys: &keys,
            privacy_token: None,
            capability: None,
            device_identity: None,
            id: None,
            multi_device: false,
        });
        let tags = child_tags(&call);
        assert!(tags.contains(&"destination".to_string()));
        assert!(!tags.contains(&"enc".to_string()));
    }

    // A multi-device callee whose encryption left a single survivor must keep the addressed
    // `<destination><to jid>` shape, not collapse to a bare `<enc>` that drops the device jid.
    #[test]
    fn offer_multi_device_single_survivor_keeps_destination() {
        let peer = peer();
        let creator = creator();
        let keys = vec![OfferDeviceKey {
            device_jid: creator.clone(),
            ciphertext: vec![2],
            enc_type: "msg".into(),
        }];
        let call = build_offer(&OfferParams {
            call_id: "CID",
            to: &peer,
            call_creator: &creator,
            device_keys: &keys,
            privacy_token: None,
            capability: None,
            device_identity: None,
            id: None,
            multi_device: true,
        });
        let tags = child_tags(&call);
        assert!(tags.contains(&"destination".to_string()));
        assert!(!tags.contains(&"enc".to_string()));
    }

    // Conformance gate for wacrg call-accept (SIG-03) and call-preaccept (SIG-04):
    // accept order audio… → [te] → net(medium=2) → encopt → [capability]; preaccept
    // order audio… → encopt → capability. Both echo call-id/call-creator from the offer.
    #[test]
    fn accept_and_preaccept_shape() {
        let peer = peer();
        let creator = creator();
        let accept = build_accept(&AcceptParams {
            call_id: "CID",
            to: &peer,
            id: "ACCEPT-STANZA-ID",
            call_creator: &creator,
            audio_rates: &["16000"],
            relay_te: Some(&[0u8; 6]),
            rte: None,
            voip_settings: None,
            capability: Some(&CAPABILITY_OFFER),
        });
        assert_eq!(
            child_tags(&accept),
            ["audio", "te", "net", "encopt", "capability"]
        );
        // The wrapper id is load-bearing: the server drops an idless <accept> instead of relaying it
        // to the caller, so the call is never marked accepted (siblings keep ringing, 45s timeout).
        assert_eq!(
            accept
                .as_node_ref()
                .attrs()
                .optional_string("id")
                .as_deref(),
            Some("ACCEPT-STANZA-ID")
        );

        let pre = build_preaccept("CID", &peer, &creator, "abcd1234", &["8000", "16000"]);
        assert_eq!(child_tags(&pre), ["audio", "audio", "encopt", "capability"]);
        assert_eq!(
            pre.as_node_ref().attrs().optional_string("id").as_deref(),
            Some("abcd1234")
        );
    }

    #[test]
    fn transport_net_protocol_rule() {
        let peer = peer();
        let creator = creator();
        // type != 9 → net has protocol=0
        let t1 = build_transport(&TransportParams {
            call_id: "CID",
            to: &peer,
            call_creator: &creator,
            p2p_cand_round: Some("1"),
            transport_message_type: Some("1"),
            relay_te: Some(&[9u8; 6]),
        });
        let r = t1.as_node_ref();
        let action = &r.children().unwrap()[0];
        assert_eq!(
            action
                .attrs()
                .optional_string("transport-message-type")
                .as_deref(),
            Some("1")
        );
        let net = action.get_optional_child("net").unwrap();
        assert_eq!(
            net.attrs().optional_string("protocol").as_deref(),
            Some("0")
        );

        // type == 9 → no protocol attr
        let t9 = build_transport(&TransportParams {
            call_id: "CID",
            to: &peer,
            call_creator: &creator,
            p2p_cand_round: None,
            transport_message_type: Some("9"),
            relay_te: None,
        });
        let r9 = t9.as_node_ref();
        let net9 = r9.children().unwrap()[0].get_optional_child("net").unwrap();
        assert!(net9.attrs().optional_string("protocol").is_none());
    }

    #[test]
    fn relaylatency_encoding_and_heartbeat() {
        let peer = peer();
        let creator = creator();
        assert_eq!(encode_latency(45), "33554477");
        let rl = build_relay_latency(&RelayLatencyParams {
            call_id: "CID",
            to: &peer,
            call_creator: &creator,
            latency_ms: 45,
            relay_name: "gru1c02",
            address_bytes: &[1, 2, 3, 4, 5, 6],
            devices: std::slice::from_ref(&peer),
        });
        let r = rl.as_node_ref();
        let action = &r.children().unwrap()[0];
        let te = action.get_optional_child("te").unwrap();
        assert_eq!(
            te.attrs().optional_string("latency").as_deref(),
            Some("33554477")
        );
        assert_eq!(
            te.attrs().optional_string("relay_name").as_deref(),
            Some("gru1c02")
        );
        assert!(action.get_optional_child("destination").is_some());

        let hb = build_heartbeat("CALLID", &creator, "DEADBEEF");
        assert_eq!(
            hb.as_node_ref().attrs().optional_string("to").as_deref(),
            Some("CALLID@call")
        );
        assert_eq!(
            hb.as_node_ref().attrs().optional_string("id").as_deref(),
            Some("DEADBEEF")
        );
    }

    // The sibling-dismiss terminate is addressed PER device (to the device JID), carries a wrapper
    // `id`, and never uses a `<destination>` block (that fan-out is gated to offer/enc_rekey).
    #[test]
    fn terminate_is_per_device_with_id_and_no_destination() {
        let dev = peer().with_device(3);
        let creator = creator();
        let term = build_terminate(&TerminateParams {
            call_id: "CID",
            to: &dev,
            id: Some("term-1"),
            call_creator: &creator,
            reason: Some("accepted_elsewhere"),
        });
        let r = term.as_node_ref();
        assert_eq!(
            r.attrs().optional_string("to").as_deref(),
            Some(dev.to_string().as_str()),
            "addressed to the device JID, not the bare peer"
        );
        assert_eq!(r.attrs().optional_string("id").as_deref(), Some("term-1"));
        let action = &r.children().unwrap()[0];
        assert_eq!(action.tag, "terminate");
        assert_eq!(
            action.attrs().optional_string("reason").as_deref(),
            Some("accepted_elsewhere")
        );
        assert!(
            action.get_optional_child("destination").is_none(),
            "terminate must not use a <destination> block"
        );
    }
}
