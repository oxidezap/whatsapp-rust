//! Group-call, call-link, waiting-room, and participant-control stanzas.
//!
//! The shapes here come from WhatsApp Web call signaling. Media state is
//! transaction ordered: callers must reject snapshots older than the last
//! accepted `transaction-id`.

use anyhow::{Result, anyhow, bail};
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Node, NodeRef, Server};

use crate::types::group_call::{
    CallLink, CallLinkJoin, CallLinkMedia, CallLinkPreview, GROUP_CALL_MAX_PARTICIPANTS,
    GroupCallDevice, GroupCallEncRekey, GroupCallParticipant, GroupCallRelay,
    GroupCallRelayEndpoint, GroupCallUpdate, ScreenShare, ScreenShareState, WaitingRoom,
    WaitingRoomUser,
};

use super::call::{
    AcceptParams, CAPABILITY_OFFER, CAPABILITY_STANDARD_OPUS_OFFER,
    CAPABILITY_STANDARD_OPUS_VIDEO_OFFER, CAPABILITY_VIDEO_OFFER, OfferDeviceKey, build_accept,
    build_preaccept,
};

const MAX_RELAY_TOKENS: usize = 64;

/// Initial ad-hoc or group-bound call offer.
pub struct InitialGroupOfferParams<'a> {
    pub call_id: &'a str,
    pub id: &'a str,
    pub call_creator: &'a Jid,
    pub group_jid: Option<&'a Jid>,
    pub participants: &'a [GroupCallParticipant],
    pub audio_rate: u32,
    pub video: bool,
}

/// Active-call invitation of one new or disconnected participant.
pub struct GroupInviteOfferParams<'a> {
    pub call_id: &'a str,
    pub id: &'a str,
    pub to: &'a Jid,
    pub call_creator: &'a Jid,
    pub target_devices: &'a [Jid],
    pub participants: &'a [GroupCallParticipant],
    pub video: bool,
}

/// Direct delivery of one shared keygen-v2 epoch.
pub struct GroupEncRekeyParams<'a> {
    pub call_id: &'a str,
    pub id: &'a str,
    pub to: &'a Jid,
    pub call_creator: &'a Jid,
    pub transaction_id: u32,
    pub device_key: &'a OfferDeviceKey,
    pub device_identity: Option<&'a [u8]>,
}

/// Build the eager preparation response for an invitation into an active group call.
pub fn build_active_group_preaccept(
    call_id: &str,
    call_creator: &Jid,
    request_id: &str,
    video: bool,
) -> Result<Node> {
    validate_call_identity(call_id, request_id, call_creator)?;
    Ok(build_preaccept(
        call_id,
        &Jid::new(call_id, Server::Call),
        call_creator,
        request_id,
        &["16000"],
        video,
    ))
}

/// Build the immediate acceptance of an invitation into an active group call.
pub fn build_active_group_accept(
    call_id: &str,
    call_creator: &Jid,
    request_id: &str,
    video: bool,
) -> Result<Node> {
    validate_call_identity(call_id, request_id, call_creator)?;
    Ok(build_accept(&AcceptParams {
        call_id,
        to: &Jid::new(call_id, Server::Call),
        id: request_id,
        call_creator,
        audio_rates: &["16000"],
        relay_te: None,
        rte: None,
        voip_settings: None,
        capability: None,
        video,
        peer_abtest_bucket: None,
        peer_abtest_bucket_id_list: None,
    }))
}

/// Build an initial group offer in the server-enforced child order.
pub fn build_initial_group_offer(params: &InitialGroupOfferParams<'_>) -> Result<Node> {
    validate_call_identity(params.call_id, params.id, params.call_creator)?;
    if params.participants.len() < 3 {
        bail!("group offer requires self and at least two remote participants");
    }
    if params.participants.len() > GROUP_CALL_MAX_PARTICIPANTS {
        bail!(
            "group offer has {} participants, maximum is {GROUP_CALL_MAX_PARTICIPANTS}",
            params.participants.len()
        );
    }
    if params.audio_rate == 0 {
        bail!("group offer audio rate must be non-zero");
    }

    let users = build_group_users(params.participants, Some(params.call_creator), params.video)?;
    let audio_rate = params.audio_rate.to_string();
    let mut children = vec![audio_opus(&audio_rate)];
    if params.video {
        children.push(video_offer_node());
    }
    children.push(NodeBuilder::new("net").attr("medium", "3").build());
    children.push(NodeBuilder::new("group_info").children(users).build());

    let mut action = call_action("offer", params.call_id, params.call_creator, children);
    if let Some(group_jid) = params.group_jid {
        if group_jid.server != Server::Group {
            bail!("group offer group_jid must use @g.us");
        }
        action.attrs.insert("group-jid", group_jid);
    }
    Ok(call_wrap(
        &Jid::new(params.call_id, Server::Call),
        params.id,
        action,
    ))
}

/// Build a singular offer for a participant joining an already-active call.
pub fn build_group_invite_offer(params: &GroupInviteOfferParams<'_>) -> Result<Node> {
    validate_call_identity(params.call_id, params.id, params.call_creator)?;
    if params.to.user.is_empty() {
        bail!("group invite target is required");
    }
    if params.target_devices.is_empty() {
        bail!("group invite requires at least one target device");
    }
    if params.participants.is_empty() {
        bail!("group invite requires an active roster");
    }

    let mut children = vec![audio_opus("16000")];
    if params.video {
        children.push(video_offer_node());
    }
    children.push(NodeBuilder::new("net").attr("medium", "2").build());
    children.push(destination_to(params.target_devices));
    children.push(
        NodeBuilder::new("group_info")
            // Preserve the server-authoritative roster verbatim: a video invitation advertises
            // video separately and must not upgrade capability blobs owned by other participants.
            .children(build_group_users(params.participants, None, false)?)
            .build(),
    );
    Ok(call_wrap(
        params.to,
        params.id,
        call_action("offer", params.call_id, params.call_creator, children),
    ))
}

fn validate_call_identity(call_id: &str, id: &str, creator: &Jid) -> Result<()> {
    if call_id.is_empty() {
        bail!("call id is required");
    }
    if id.is_empty() {
        bail!("call stanza id is required");
    }
    if creator.user.is_empty() {
        bail!("call creator is required");
    }
    Ok(())
}

fn build_group_users(
    participants: &[GroupCallParticipant],
    video_creator: Option<&Jid>,
    video: bool,
) -> Result<Vec<Node>> {
    participants
        .iter()
        .enumerate()
        .map(|(participant_index, participant)| {
            if participant.jid.user.is_empty() {
                bail!("participant {participant_index} has no jid");
            }
            if participant.devices.is_empty() {
                bail!("participant {participant_index} has no devices");
            }
            let devices = participant
                .devices
                .iter()
                .enumerate()
                .map(|(device_index, device)| {
                    if device.jid.user.is_empty() {
                        bail!("participant {participant_index} device {device_index} has no jid");
                    }
                    let mut builder = NodeBuilder::new("device").attr("jid", &device.jid);
                    if !device.capability.is_empty() {
                        let capability = if video
                            && video_creator.is_some_and(|creator| {
                                creator.to_non_ad() == device.jid.to_non_ad()
                            }) {
                            if device.capability == CAPABILITY_OFFER {
                                CAPABILITY_VIDEO_OFFER.to_vec()
                            } else if device.capability == CAPABILITY_STANDARD_OPUS_OFFER {
                                CAPABILITY_STANDARD_OPUS_VIDEO_OFFER.to_vec()
                            } else {
                                device.capability.clone()
                            }
                        } else {
                            device.capability.clone()
                        };
                        let mut cap = NodeBuilder::new("capability");
                        if let Some(version) = device.capability_version {
                            cap = cap.attr("ver", version.to_string());
                        }
                        builder = builder.children([cap.bytes(capability).build()]);
                    }
                    Ok(builder.build())
                })
                .collect::<Result<Vec<_>>>()?;
            let mut builder = NodeBuilder::new("user").attr("jid", &participant.jid);
            if let Some(state) = participant.state.as_deref() {
                builder = builder.attr("state", state);
            }
            Ok(builder.children(devices).build())
        })
        .collect()
}

/// Parse the group snapshot embedded in an active-call invitation.
pub fn parse_group_invite_snapshot(offer: &NodeRef<'_>) -> Result<Option<GroupCallUpdate>> {
    if offer.tag != "offer" {
        bail!("expected <offer>, got <{}>", offer.tag);
    }
    let Some(group_info) = offer.get_optional_child("group_info") else {
        return Ok(None);
    };
    let mut attrs = offer.attrs();
    let call_id = required_string(&mut attrs, "call-id", "offer")?;
    let call_creator = required_jid(&mut attrs, "call-creator", "offer")?;
    let joinable = attrs.optional_string("joinable").as_deref() == Some("1");

    let mut update = parse_group_info(group_info, call_id, call_creator)?;
    let mut group_attrs = group_info.attrs();
    if let Some(group_call_id) = group_attrs.optional_string("call-id")
        && group_call_id != update.call_id
    {
        bail!("group_info call-id does not match offer");
    }
    if let Some(group_creator) = group_attrs.optional_jid("call-creator")
        && group_creator != update.call_creator
    {
        bail!("group_info call-creator does not match offer");
    }
    update.joinable = joinable;
    Ok(Some(update))
}

/// Parse an authoritative `<group_update>` action.
pub fn parse_group_update(node: &NodeRef<'_>) -> Result<GroupCallUpdate> {
    if node.tag != "group_update" {
        bail!("expected <group_update>, got <{}>", node.tag);
    }
    let mut attrs = node.attrs();
    let call_id = required_string(&mut attrs, "call-id", "group_update")?;
    let creator = required_jid(&mut attrs, "call-creator", "group_update")?;
    attrs
        .finish()
        .map_err(|error| anyhow!("<group_update> attrs: {error}"))?;
    let group_info = node
        .get_optional_child("group_info")
        .ok_or_else(|| anyhow!("<group_update> missing <group_info>"))?;
    let mut update = parse_group_info(group_info, call_id, creator)?;
    update.av_upgradable = node.get_optional_child("av_upgrade").is_some_and(|child| {
        child.attrs().optional_string("av-upgradable").as_deref() == Some("1")
    });
    update.relay = node
        .get_optional_child("relay")
        .map(parse_group_relay)
        .transpose()?;
    Ok(update)
}

/// Parse the group snapshot returned by an initial offer or link join.
pub fn parse_initial_group_call_ack(node: &NodeRef<'_>) -> Result<Option<GroupCallUpdate>> {
    if node.tag != "ack" {
        bail!("expected <ack>, got <{}>", node.tag);
    }
    let Some(group_info) = node.get_optional_child("group_info") else {
        return Ok(None);
    };
    let mut attrs = group_info.attrs();
    let call_id = required_string(&mut attrs, "call-id", "group_info")?;
    let creator = required_jid(&mut attrs, "call-creator", "group_info")?;
    let mut update = parse_group_info(group_info, call_id, creator)?;
    update.relay = node
        .get_optional_child("relay")
        .map(parse_group_relay)
        .transpose()?;
    Ok(Some(update))
}

fn parse_group_info(
    node: &NodeRef<'_>,
    call_id: String,
    call_creator: Jid,
) -> Result<GroupCallUpdate> {
    let mut attrs = node.attrs();
    let group_jid = attrs.optional_jid("group-jid");
    let media = required_string(&mut attrs, "media", "group_info")?;
    let transaction_id = required_u32(&mut attrs, "transaction-id", "group_info")?;
    let connected_limit = required_u32(&mut attrs, "connected-limit", "group_info")?;
    let joinable = attrs.optional_string("joinable").as_deref() == Some("1");
    let rekey_requested = attrs.optional_string("rekey").as_deref() == Some("1");
    let participants = node
        .children()
        .unwrap_or_default()
        .iter()
        .filter(|child| child.tag.as_ref() == "user")
        .map(parse_group_participant)
        .collect::<Result<Vec<_>>>()?;
    Ok(GroupCallUpdate {
        call_id,
        call_creator,
        group_jid,
        transaction_id,
        media,
        connected_limit,
        joinable,
        av_upgradable: false,
        rekey_requested,
        participants,
        relay: None,
    })
}

fn parse_group_participant(node: &NodeRef<'_>) -> Result<GroupCallParticipant> {
    let mut attrs = node.attrs();
    let jid = required_jid(&mut attrs, "jid", "user")?;
    let pn = attrs.optional_jid("user_pn");
    let state = Some(required_string(&mut attrs, "state", "user")?);
    let participant_type = attrs
        .optional_string("type")
        .map(|value| value.into_owned());
    let devices = node
        .children()
        .unwrap_or_default()
        .iter()
        .filter(|child| child.tag.as_ref() == "device")
        .map(parse_group_device)
        .collect::<Result<Vec<_>>>()?;
    Ok(GroupCallParticipant {
        jid,
        pn,
        state,
        participant_type,
        devices,
    })
}

fn parse_group_device(node: &NodeRef<'_>) -> Result<GroupCallDevice> {
    let mut attrs = node.attrs();
    let jid = required_jid(&mut attrs, "jid", "device")?;
    let platform = attrs
        .optional_string("platform")
        .map(|value| value.into_owned());
    let pid = optional_u32(&mut attrs, "pid", "device")?;
    let (capability_version, capability) = match node.get_optional_child("capability") {
        Some(capability) => {
            let mut cap_attrs = capability.attrs();
            (
                optional_u32(&mut cap_attrs, "ver", "capability")?,
                capability.content_bytes().unwrap_or_default().to_vec(),
            )
        }
        None => (None, Vec::new()),
    };
    Ok(GroupCallDevice {
        jid,
        platform,
        pid,
        capability_version,
        capability,
    })
}

fn parse_group_relay(node: &NodeRef<'_>) -> Result<GroupCallRelay> {
    let mut attrs = node.attrs();
    let transaction_id = optional_u32(&mut attrs, "transaction-id", "relay")?;
    let self_pid = optional_u32(&mut attrs, "self_pid", "relay")?;
    let uuid = required_string(&mut attrs, "uuid", "relay")?;
    let participant_uuid = required_string(&mut attrs, "participant_uuid", "relay")?;
    let attribute_padding = attrs.optional_string("attribute_padding").as_deref() == Some("1");
    let warp_mi_tag_len = optional_u32(&mut attrs, "warp_mi_tag_len", "relay")?;
    let children = node.children().unwrap_or_default();
    let content = |tag: &str| {
        children
            .iter()
            .find(|child| child.tag.as_ref() == tag)
            .and_then(NodeRef::content_bytes)
            .unwrap_or_default()
            .to_vec()
    };
    let endpoints = children
        .iter()
        .filter(|child| child.tag.as_ref() == "te2")
        .map(parse_group_relay_endpoint)
        .collect::<Result<Vec<_>>>()?;
    Ok(GroupCallRelay {
        transaction_id,
        self_pid,
        uuid,
        participant_uuid,
        attribute_padding,
        warp_mi_tag_len,
        key: content("key"),
        hbh_key: content("hbh_key"),
        tokens: parse_indexed_tokens(children, "token"),
        auth_tokens: parse_indexed_tokens(children, "auth_token"),
        endpoints,
    })
}

fn parse_group_relay_endpoint(node: &NodeRef<'_>) -> Result<GroupCallRelayEndpoint> {
    let mut attrs = node.attrs();
    let relay_id = optional_u32(&mut attrs, "relay_id", "te2")?.unwrap_or_default();
    let token_id = optional_u32(&mut attrs, "token_id", "te2")?.unwrap_or_default();
    let auth_token_id = optional_u32(&mut attrs, "auth_token_id", "te2")?.unwrap_or_default();
    let relay_name = required_string(&mut attrs, "relay_name", "te2")?;
    let domain_name = attrs
        .optional_string("domain_name")
        .map(|value| value.into_owned());
    let rtt_ms = optional_u32(&mut attrs, "c2r_rtt", "te2")?;
    let is_fna = attrs.optional_string("is_fna").as_deref() == Some("1");
    let address = node.content_bytes().unwrap_or_default().to_vec();
    let (ipv4, port) = if let [a, b, c, d, high, low] = address.as_slice() {
        (
            Some(format!("{a}.{b}.{c}.{d}")),
            Some(u16::from_be_bytes([*high, *low])),
        )
    } else {
        (None, None)
    };
    Ok(GroupCallRelayEndpoint {
        relay_id,
        token_id,
        auth_token_id,
        relay_name,
        domain_name,
        rtt_ms,
        is_fna,
        address,
        ipv4,
        port,
    })
}

fn parse_indexed_tokens(children: &[NodeRef<'_>], tag: &str) -> Vec<Vec<u8>> {
    let mut values = Vec::new();
    for child in children.iter().filter(|child| child.tag.as_ref() == tag) {
        let Some(value) = child.content_bytes() else {
            continue;
        };
        let index = child
            .attrs()
            .optional_string("id")
            .and_then(|raw| raw.parse::<usize>().ok())
            .unwrap_or(values.len());
        if index >= MAX_RELAY_TOKENS {
            continue;
        }
        if values.len() <= index {
            values.resize_with(index + 1, Vec::new);
        }
        values[index] = value.to_vec();
    }
    values
}

/// Build one keygen-v2 group epoch delivery.
pub fn build_group_enc_rekey(params: &GroupEncRekeyParams<'_>) -> Result<Node> {
    validate_call_identity(params.call_id, params.id, params.call_creator)?;
    if params.transaction_id == 0 {
        bail!("group epoch transaction id must be non-zero");
    }
    if params.device_key.device_jid != *params.to {
        bail!("group epoch ciphertext target does not match stanza target");
    }
    if params.device_key.ciphertext.is_empty() {
        bail!("group epoch ciphertext is empty");
    }
    if !matches!(params.device_key.enc_type.as_str(), "msg" | "pkmsg") {
        bail!("unsupported group epoch encryption type");
    }
    if params.device_key.enc_type == "pkmsg" && params.device_identity.is_none() {
        bail!("pre-key group epoch requires device identity");
    }
    let mut children = vec![
        NodeBuilder::new("encopt").attr("keygen", "2").build(),
        NodeBuilder::new("enc")
            .attr("v", "2")
            .attr("type", &params.device_key.enc_type)
            .attr("count", "0")
            .bytes(params.device_key.ciphertext.clone())
            .build(),
    ];
    if let Some(identity) = params.device_identity {
        children.push(
            NodeBuilder::new("device-identity")
                .bytes(identity.to_vec())
                .build(),
        );
    }
    let action = NodeBuilder::new("enc_rekey")
        .attr("call-id", params.call_id)
        .attr("call-creator", params.call_creator)
        .attr("transaction-id", params.transaction_id.to_string())
        .children(children)
        .build();
    Ok(call_wrap(params.to, params.id, action))
}

/// Parse one keygen-v2 group epoch delivery.
pub fn parse_group_enc_rekey(node: &NodeRef<'_>) -> Result<GroupCallEncRekey> {
    if node.tag != "enc_rekey" {
        bail!("expected <enc_rekey>, got <{}>", node.tag);
    }
    let mut attrs = node.attrs();
    let call_id = required_string(&mut attrs, "call-id", "enc_rekey")?;
    let call_creator = required_jid(&mut attrs, "call-creator", "enc_rekey")?;
    let transaction_id = required_u32(&mut attrs, "transaction-id", "enc_rekey")?;
    attrs
        .finish()
        .map_err(|error| anyhow!("<enc_rekey> attrs: {error}"))?;
    let encopt = node
        .get_optional_child("encopt")
        .ok_or_else(|| anyhow!("<enc_rekey> missing <encopt>"))?;
    let enc = node
        .get_optional_child("enc")
        .ok_or_else(|| anyhow!("<enc_rekey> missing <enc>"))?;
    let mut encopt_attrs = encopt.attrs();
    let key_generation = required_u32(&mut encopt_attrs, "keygen", "encopt")?;
    let mut enc_attrs = enc.attrs();
    let encryption_type = required_string(&mut enc_attrs, "type", "enc")?;
    let encryption_version = required_u32(&mut enc_attrs, "v", "enc")?;
    if key_generation != 2
        || encryption_version != 2
        || !matches!(encryption_type.as_str(), "msg" | "pkmsg")
    {
        bail!("unsupported group epoch encryption");
    }
    let ciphertext = enc
        .content_bytes()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow!("<enc_rekey><enc> has no ciphertext"))?
        .to_vec();
    Ok(GroupCallEncRekey {
        call_id,
        call_creator,
        transaction_id,
        key_generation,
        encryption_type,
        encryption_version,
        ciphertext,
    })
}

/// Build a call-link creation request.
pub fn build_call_link_create(media: CallLinkMedia, request_id: &str) -> Result<Node> {
    build_call_service_request(
        request_id,
        NodeBuilder::new("link_create")
            .attr("media", media.as_str())
            .build(),
    )
}

/// Build a call-link preview request.
pub fn build_call_link_query(token: &str, media: CallLinkMedia, request_id: &str) -> Result<Node> {
    if token.trim().is_empty() {
        bail!("call-link token is required");
    }
    build_call_service_request(
        request_id,
        NodeBuilder::new("link_query")
            .attr("token", token)
            .attr("media", media.as_str())
            .build(),
    )
}

/// Build a call-link join request with the media capabilities of this client.
pub fn build_call_link_join(token: &str, media: CallLinkMedia, request_id: &str) -> Result<Node> {
    build_call_link_join_with_capability(token, media, request_id, &CAPABILITY_OFFER)
}

/// Build a call-link join request with an explicit version-1 audio capability.
pub fn build_call_link_join_with_capability(
    token: &str,
    media: CallLinkMedia,
    request_id: &str,
    capability: &[u8],
) -> Result<Node> {
    if token.trim().is_empty() {
        bail!("call-link token is required");
    }
    if capability.is_empty() {
        bail!("call-link capability is required");
    }
    let mut children = vec![audio_opus("16000")];
    if media == CallLinkMedia::Video {
        children.push(
            NodeBuilder::new("video")
                .attr("dec", "H264")
                .attr("device_orientation", "0")
                .build(),
        );
    }
    children.extend([
        NodeBuilder::new("net").attr("medium", "2").build(),
        NodeBuilder::new("capability")
            .attr("ver", "1")
            .bytes(capability.to_vec())
            .build(),
    ]);
    build_call_service_request(
        request_id,
        NodeBuilder::new("link_join")
            .attr("token", token)
            .attr("media", media.as_str())
            .children(children)
            .build(),
    )
}

fn build_call_service_request(request_id: &str, action: Node) -> Result<Node> {
    if request_id.is_empty() {
        bail!("call request id is required");
    }
    Ok(call_wrap(&Jid::new("", Server::Call), request_id, action))
}

/// Parse a link-creation response.
pub fn parse_call_link_create_ack(node: &NodeRef<'_>) -> Result<CallLink> {
    let child = validated_call_ack_child(node, "link_create")?;
    let mut attrs = child.attrs();
    let token = required_string(&mut attrs, "token", "link_create")?;
    let media = parse_call_link_media(required_string(&mut attrs, "media", "link_create")?)?;
    Ok(CallLink { token, media })
}

/// Parse a link-preview response.
pub fn parse_call_link_query_ack(node: &NodeRef<'_>) -> Result<CallLinkPreview> {
    let child = validated_call_ack_child(node, "link_query")?;
    let mut attrs = child.attrs();
    let token = required_string(&mut attrs, "token", "link_query")?;
    let media = parse_call_link_media(required_string(&mut attrs, "media", "link_query")?)?;
    let creator = required_jid(&mut attrs, "link_creator", "link_query")?;
    let creator_pn = attrs.optional_jid("link_creator_pn");
    let waiting = child.get_optional_child("waiting_room");
    let (waiting_room_enabled, is_admin) = waiting.map_or((false, false), |waiting| {
        let mut attrs = waiting.attrs();
        (
            bool_attr(&mut attrs, "enabled"),
            bool_attr(&mut attrs, "is_admin"),
        )
    });
    Ok(CallLinkPreview {
        token,
        media,
        creator,
        creator_pn,
        waiting_room_enabled,
        is_admin,
    })
}

/// Parse a link join that either admitted this endpoint or placed it in a waiting room.
pub fn parse_call_link_join_ack(node: &NodeRef<'_>) -> Result<CallLinkJoin> {
    validate_call_ack(node, "link_join")?;
    let waiting = node
        .get_optional_child("waiting_room")
        .map(parse_waiting_room)
        .transpose()?;
    let group = parse_initial_group_call_ack(node)?;
    let (token, media, call_id, call_creator, waiting_room_enabled, is_admin) =
        match (&waiting, &group) {
            (Some(waiting), _) => (
                waiting.link_token.clone(),
                waiting.media,
                waiting.call_id.clone(),
                waiting.call_creator.clone(),
                waiting.enabled,
                waiting.is_admin,
            ),
            (None, Some(group)) => (
                String::new(),
                parse_call_link_media(group.media.clone())?,
                group.call_id.clone(),
                group.call_creator.clone(),
                false,
                false,
            ),
            (None, None) => bail!("<link_join> ack has neither waiting_room nor group_info"),
        };
    if group.is_none() && !waiting_room_enabled {
        bail!("<link_join> waiting-room-only ack is not enabled");
    }
    if let Some(group) = &group
        && (group.call_id != call_id || group.call_creator != call_creator)
    {
        bail!("waiting-room and admitted group identities differ");
    }
    Ok(CallLinkJoin {
        token,
        media,
        call_id,
        call_creator,
        waiting_room_enabled,
        in_waiting_room: waiting.is_some() && waiting_room_enabled && group.is_none(),
        is_admin,
        waiting_room: waiting,
        group,
    })
}

/// Parse one authoritative waiting-room update action.
pub fn parse_waiting_room_update(node: &NodeRef<'_>) -> Result<WaitingRoom> {
    if node.tag != "waiting_room_update" {
        bail!("expected <waiting_room_update>, got <{}>", node.tag);
    }
    let mut attrs = node.attrs();
    let call_id = required_string(&mut attrs, "call-id", "waiting_room_update")?;
    let creator = required_jid(&mut attrs, "call-creator", "waiting_room_update")?;
    attrs
        .finish()
        .map_err(|error| anyhow!("<waiting_room_update> attrs: {error}"))?;
    let waiting = node
        .get_optional_child("waiting_room")
        .ok_or_else(|| anyhow!("<waiting_room_update> missing <waiting_room>"))?;
    let room = parse_waiting_room(waiting)?;
    if room.call_id != call_id || room.call_creator != creator {
        bail!("waiting-room update identity mismatch");
    }
    Ok(room)
}

fn parse_waiting_room(node: &NodeRef<'_>) -> Result<WaitingRoom> {
    if node.tag != "waiting_room" {
        bail!("expected <waiting_room>, got <{}>", node.tag);
    }
    let mut attrs = node.attrs();
    let call_id = required_string(&mut attrs, "call-id", "waiting_room")?;
    let call_creator = required_jid(&mut attrs, "call-creator", "waiting_room")?;
    let link_token = required_string(&mut attrs, "link-token", "waiting_room")?;
    let media = parse_call_link_media(required_string(&mut attrs, "media", "waiting_room")?)?;
    let enabled = bool_attr(&mut attrs, "enabled");
    let is_admin = bool_attr(&mut attrs, "is_admin");
    let transaction_id = optional_u32(&mut attrs, "transaction-id", "waiting_room")?;
    let users = node
        .children()
        .unwrap_or_default()
        .iter()
        .filter(|child| child.tag.as_ref() == "user")
        .map(|child| {
            let mut attrs = child.attrs();
            Ok(WaitingRoomUser {
                jid: required_jid(&mut attrs, "jid", "waiting_room user")?,
                pn: attrs.optional_jid("user_pn"),
                state: required_string(&mut attrs, "state", "waiting_room user")?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(WaitingRoom {
        call_id,
        call_creator,
        link_token,
        media,
        enabled,
        is_admin,
        transaction_id,
        users,
    })
}

/// Toggle approval requirements for a live link call.
pub fn build_waiting_room_toggle(
    call_id: &str,
    creator: &Jid,
    enabled: bool,
    request_id: &str,
) -> Result<Node> {
    validate_call_identity(call_id, request_id, creator)?;
    Ok(build_waiting_room_request(
        "waiting_room_toggle",
        call_id,
        creator,
        request_id,
        [("enabled", if enabled { "1" } else { "0" })],
        Vec::new(),
    ))
}

/// Keep a pending call-link join alive.
pub fn build_waiting_room_heartbeat(
    call_id: &str,
    creator: &Jid,
    request_id: &str,
) -> Result<Node> {
    validate_call_identity(call_id, request_id, creator)?;
    Ok(build_waiting_room_request(
        "heartbeat",
        call_id,
        creator,
        request_id,
        [("type", "waiting_room")],
        Vec::new(),
    ))
}

/// Admit one pending waiting-room user.
pub fn build_waiting_room_admit(
    call_id: &str,
    creator: &Jid,
    user: &Jid,
    request_id: &str,
) -> Result<Node> {
    validate_call_identity(call_id, request_id, creator)?;
    build_waiting_room_user_action("waiting_room_admit", call_id, creator, user, request_id)
}

/// Deny one pending waiting-room user.
pub fn build_waiting_room_deny(
    call_id: &str,
    creator: &Jid,
    user: &Jid,
    request_id: &str,
) -> Result<Node> {
    validate_call_identity(call_id, request_id, creator)?;
    build_waiting_room_user_action("waiting_room_deny", call_id, creator, user, request_id)
}

/// Validate the service acknowledgement for a waiting-room approval toggle.
pub fn parse_waiting_room_toggle_ack(node: &NodeRef<'_>) -> Result<()> {
    validate_call_ack(node, "waiting_room_toggle")
}

/// Validate the service acknowledgement for admitting one waiting user.
pub fn parse_waiting_room_admit_ack(node: &NodeRef<'_>) -> Result<()> {
    validate_call_ack(node, "waiting_room_admit")
}

/// Validate the service acknowledgement for denying one waiting user.
pub fn parse_waiting_room_deny_ack(node: &NodeRef<'_>) -> Result<()> {
    validate_call_ack(node, "waiting_room_deny")
}

fn build_waiting_room_user_action(
    tag: &'static str,
    call_id: &str,
    creator: &Jid,
    user: &Jid,
    request_id: &str,
) -> Result<Node> {
    if user.user.is_empty() {
        bail!("waiting-room user is required");
    }
    Ok(build_waiting_room_request(
        tag,
        call_id,
        creator,
        request_id,
        [],
        vec![NodeBuilder::new("user").attr("jid", user).build()],
    ))
}

fn build_waiting_room_request<const N: usize>(
    tag: &'static str,
    call_id: &str,
    creator: &Jid,
    request_id: &str,
    extra_attrs: [(&'static str, &'static str); N],
    children: Vec<Node>,
) -> Node {
    let mut action = NodeBuilder::new(tag)
        .attr("call-id", call_id)
        .attr("call-creator", creator);
    for (name, value) in extra_attrs {
        action = action.attr(name, value);
    }
    if !children.is_empty() {
        action = action.children(children);
    }
    call_wrap(&Jid::new(call_id, Server::Call), request_id, action.build())
}

/// Build a persistent raise/lower-hand transition.
pub fn build_raise_hand(
    call_id: &str,
    to: &Jid,
    creator: &Jid,
    request_id: &str,
    raised: bool,
) -> Result<Node> {
    validate_call_identity(call_id, request_id, creator)?;
    if to.user.is_empty() {
        bail!("raise-hand target is required");
    }
    Ok(call_wrap(
        to,
        request_id,
        NodeBuilder::new("user_action")
            .attr("call-id", call_id)
            .attr("call-creator", creator)
            .attr("action", "raise_hand")
            .children([NodeBuilder::new("raise_hand")
                .attr("raise-hand-state", if raised { "1" } else { "0" })
                .build()])
            .build(),
    ))
}

/// Parse a persistent raise/lower-hand transition.
pub fn parse_raise_hand(node: &NodeRef<'_>) -> Result<bool> {
    if node.tag != "user_action" {
        bail!("expected <user_action>, got <{}>", node.tag);
    }
    let mut attrs = node.attrs();
    let action = required_string(&mut attrs, "action", "user_action")?;
    let _ = required_string(&mut attrs, "call-id", "user_action")?;
    let _ = required_jid(&mut attrs, "call-creator", "user_action")?;
    attrs
        .finish()
        .map_err(|error| anyhow!("<user_action> attrs: {error}"))?;
    if action != "raise_hand" {
        bail!("unsupported user action {action:?}");
    }
    let state = node
        .get_optional_child("raise_hand")
        .ok_or_else(|| anyhow!("<user_action> missing <raise_hand>"))?
        .attrs()
        .optional_string("raise-hand-state");
    match state.as_deref() {
        Some("1") => Ok(true),
        Some("0") => Ok(false),
        _ => bail!("invalid raise-hand-state"),
    }
}

/// Build one version-2 screen-share transition.
pub fn build_screen_share(
    call_id: &str,
    to: &Jid,
    creator: &Jid,
    request_id: &str,
    state: ScreenShareState,
    screen_share_id: Option<u32>,
) -> Result<Node> {
    validate_call_identity(call_id, request_id, creator)?;
    if to.user.is_empty() {
        bail!("screen-share target is required");
    }
    let mut action = NodeBuilder::new("screen_share")
        .attr("call-id", call_id)
        .attr("call-creator", creator)
        .attr("screenshare_state", state.code().to_string())
        .attr("version", "2");
    if let Some(id) = screen_share_id {
        action = action.attr("screen_share_id", id.to_string());
    }
    Ok(call_wrap(to, request_id, action.build()))
}

/// Parse one versioned screen-share transition.
pub fn parse_screen_share(node: &NodeRef<'_>) -> Result<ScreenShare> {
    if node.tag != "screen_share" {
        bail!("expected <screen_share>, got <{}>", node.tag);
    }
    let mut attrs = node.attrs();
    let _ = required_string(&mut attrs, "call-id", "screen_share")?;
    let _ = required_jid(&mut attrs, "call-creator", "screen_share")?;
    let state_value = required_u32(&mut attrs, "screenshare_state", "screen_share")?;
    let state_code = i32::try_from(state_value)
        .map_err(|_| anyhow!("unsupported screen-share state {state_value}"))?;
    let state = ScreenShareState::try_from(state_code)
        .map_err(|_| anyhow!("unsupported screen-share state {state_value}"))?;
    let version = required_u32(&mut attrs, "version", "screen_share")?;
    let screen_share_id = optional_u32(&mut attrs, "screen_share_id", "screen_share")?;
    attrs
        .finish()
        .map_err(|error| anyhow!("<screen_share> attrs: {error}"))?;
    Ok(ScreenShare {
        state,
        version,
        screen_share_id,
    })
}

/// Typed ACK for a received group-call control action.
pub fn build_call_control_ack(original: &NodeRef<'_>, action_type: &str) -> Option<Node> {
    if original.tag != "call" || action_type.is_empty() {
        return None;
    }
    let id = original.get_attr("id")?.to_node_value();
    let from = original.get_attr("from")?.to_node_value();
    let mut ack = NodeBuilder::new("ack")
        .attr("class", "call")
        .attr("id", id)
        .attr("to", from)
        .attr("type", action_type);
    if let Some(participant) = original.get_attr("participant")
        && original
            .get_attr("from")
            .is_none_or(|from| participant.as_str() != from.as_str())
    {
        ack = ack.attr("participant", participant.to_node_value());
    }
    if let Some(recipient) = original.get_attr("recipient") {
        ack = ack.attr("recipient", recipient.to_node_value());
    }
    Some(ack.build())
}

fn validate_call_ack(node: &NodeRef<'_>, expected_type: &str) -> Result<()> {
    let class = node.get_attr("class").map(|value| value.as_str());
    let response_type = node.get_attr("type").map(|value| value.as_str());
    if node.tag != "ack"
        || class.as_deref() != Some("call")
        || response_type.as_deref() != Some(expected_type)
    {
        bail!("unexpected {expected_type} ack envelope");
    }
    if let Some(error) = node
        .get_attr("error")
        .map(|value| value.as_str())
        .filter(|error| !error.is_empty())
    {
        bail!("{expected_type} rejected with error {error}");
    }
    Ok(())
}

fn validated_call_ack_child<'a, 'b>(
    node: &'b NodeRef<'a>,
    expected_type: &str,
) -> Result<&'b NodeRef<'a>> {
    validate_call_ack(node, expected_type)?;
    node.get_optional_child(expected_type)
        .ok_or_else(|| anyhow!("<ack type={expected_type}> missing payload"))
}

fn parse_call_link_media(value: String) -> Result<CallLinkMedia> {
    CallLinkMedia::try_from(value.as_str())
        .map_err(|_| anyhow!("unsupported call-link media {value:?}"))
}

fn bool_attr(attrs: &mut wacore_binary::AttrParserRef<'_>, name: &str) -> bool {
    attrs.optional_string(name).as_deref() == Some("1")
}

fn required_string(
    attrs: &mut wacore_binary::AttrParserRef<'_>,
    name: &str,
    tag: &str,
) -> Result<String> {
    attrs
        .optional_string(name)
        .filter(|value| !value.is_empty())
        .map(|value| value.into_owned())
        .ok_or_else(|| anyhow!("<{tag}> missing {name:?} attribute"))
}

fn required_jid(
    attrs: &mut wacore_binary::AttrParserRef<'_>,
    name: &str,
    tag: &str,
) -> Result<Jid> {
    attrs
        .optional_jid(name)
        .ok_or_else(|| anyhow!("<{tag}> missing or invalid {name:?} jid"))
}

fn required_u32(
    attrs: &mut wacore_binary::AttrParserRef<'_>,
    name: &str,
    tag: &str,
) -> Result<u32> {
    optional_u32(attrs, name, tag)?.ok_or_else(|| anyhow!("<{tag}> missing {name:?} attribute"))
}

fn optional_u32(
    attrs: &mut wacore_binary::AttrParserRef<'_>,
    name: &str,
    tag: &str,
) -> Result<Option<u32>> {
    let Some(raw) = attrs.optional_string(name) else {
        return Ok(None);
    };
    raw.parse::<u32>()
        .map(Some)
        .map_err(|error| anyhow!("<{tag}> invalid {name:?}={raw:?}: {error}"))
}

fn audio_opus(rate: &str) -> Node {
    NodeBuilder::new("audio")
        .attr("enc", "opus")
        .attr("rate", rate)
        .build()
}

fn video_offer_node() -> Node {
    NodeBuilder::new("video")
        .attr("enc", "h264")
        .attr("dec", "h264")
        .attr("orientation", "0")
        .attr("screen_width", "1920")
        .attr("screen_height", "1080")
        .attr("device_orientation", "0")
        .build()
}

fn destination_to(devices: &[Jid]) -> Node {
    NodeBuilder::new("destination")
        .children(
            devices
                .iter()
                .map(|jid| NodeBuilder::new("to").attr("jid", jid).build()),
        )
        .build()
}

fn call_action(tag: &'static str, call_id: &str, creator: &Jid, children: Vec<Node>) -> Node {
    NodeBuilder::new(tag)
        .attr("call-id", call_id)
        .attr("call-creator", creator)
        .children(children)
        .build()
}

fn call_wrap(to: &Jid, id: &str, action: Node) -> Node {
    NodeBuilder::new("call")
        .attr("to", to)
        .attr("id", id)
        .children([action])
        .build()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jid(user: &str, device: u8) -> Jid {
        Jid::new(user, Server::Lid).with_device(device as u16)
    }

    fn participant(user: &str, device: u8, capability: &[u8]) -> GroupCallParticipant {
        GroupCallParticipant {
            jid: Jid::new(user, Server::Lid),
            pn: None,
            state: None,
            participant_type: None,
            devices: vec![GroupCallDevice {
                jid: jid(user, device),
                platform: None,
                pid: None,
                capability_version: Some(1),
                capability: capability.to_vec(),
            }],
        }
    }

    fn child_tags(node: &Node) -> Vec<String> {
        let node_ref = node.as_node_ref();
        node_ref
            .children()
            .expect("children")
            .iter()
            .map(|child| child.tag.to_string())
            .collect()
    }

    fn action(node: &Node) -> &Node {
        match node.content.as_ref() {
            Some(wacore_binary::NodeContent::Nodes(children)) => {
                children.first().expect("one call action")
            }
            _ => panic!("one call action"),
        }
    }

    #[test]
    fn initial_group_offer_has_exact_route_order_and_video_capability() {
        let creator = jid("100001", 1);
        let participants = [
            participant("100001", 1, &CAPABILITY_OFFER),
            participant("200002", 2, &CAPABILITY_OFFER),
            participant("300003", 3, &CAPABILITY_OFFER),
        ];
        let group = Jid::new("1234567890-1111111111", Server::Group);
        let node = build_initial_group_offer(&InitialGroupOfferParams {
            call_id: "00aabbccddeeff001122334455667788",
            id: "REQ-1",
            call_creator: &creator,
            group_jid: Some(&group),
            participants: &participants,
            audio_rate: 16_000,
            video: true,
        })
        .expect("valid offer");

        assert_eq!(
            node.attrs.get("to").and_then(|value| value.to_jid()),
            Some(Jid::new("00aabbccddeeff001122334455667788", Server::Call))
        );
        assert_eq!(
            child_tags(action(&node)),
            ["audio", "video", "net", "group_info"]
        );
        let action = action(&node);
        let action_ref = action.as_node_ref();
        let action_children = action_ref.children().expect("group offer children");
        let audio = &action_children[0];
        assert_eq!(audio.attrs().optional_u64("rate"), Some(16_000));
        assert_eq!(
            action
                .attrs
                .get("group-jid")
                .and_then(|value| value.to_jid()),
            Some(group)
        );
        let action_ref = action.as_node_ref();
        let group_info = action_ref
            .get_optional_child("group_info")
            .expect("group_info");
        let group_users = group_info.children().expect("group users");
        assert!(
            group_users[0].attrs().optional_string("state").is_none(),
            "outbound offer participants must omit absent state instead of using a sentinel"
        );
        let creator_capability = group_users[0].children().unwrap()[0]
            .get_optional_child("capability")
            .expect("creator capability");
        assert_eq!(
            creator_capability.content_bytes(),
            Some(CAPABILITY_VIDEO_OFFER.as_slice())
        );
    }

    #[test]
    fn initial_group_video_offer_preserves_standard_opus_capability_family() {
        let creator = jid("100001", 0);
        let participants = [
            participant("100001", 1, &CAPABILITY_STANDARD_OPUS_OFFER),
            participant("200002", 2, &CAPABILITY_OFFER),
            participant("300003", 3, &CAPABILITY_OFFER),
        ];
        let node = build_initial_group_offer(&InitialGroupOfferParams {
            call_id: "00aabbccddeeff001122334455667788",
            id: "REQ-1",
            call_creator: &creator,
            group_jid: None,
            participants: &participants,
            audio_rate: 16_000,
            video: true,
        })
        .expect("valid offer");
        let action_ref = action(&node).as_node_ref();
        let capability = action_ref
            .get_optional_child("group_info")
            .expect("group_info")
            .children()
            .expect("users")[0]
            .children()
            .expect("devices")[0]
            .get_optional_child("capability")
            .expect("creator capability");
        assert_eq!(
            capability.content_bytes(),
            Some(CAPABILITY_STANDARD_OPUS_VIDEO_OFFER.as_slice())
        );
    }

    #[test]
    fn initial_group_offer_validates_membership_and_group_jid() {
        let creator = jid("100001", 1);
        let too_small = [
            participant("100001", 1, &CAPABILITY_OFFER),
            participant("200002", 2, &CAPABILITY_OFFER),
        ];
        assert!(
            build_initial_group_offer(&InitialGroupOfferParams {
                call_id: "CID",
                id: "REQ",
                call_creator: &creator,
                group_jid: None,
                participants: &too_small,
                audio_rate: 16_000,
                video: false,
            })
            .is_err()
        );
        let enough = [
            participant("100001", 1, &CAPABILITY_OFFER),
            participant("200002", 2, &CAPABILITY_OFFER),
            participant("300003", 3, &CAPABILITY_OFFER),
        ];
        assert!(
            build_initial_group_offer(&InitialGroupOfferParams {
                call_id: "CID",
                id: "REQ",
                call_creator: &creator,
                group_jid: Some(&Jid::new("not-a-group", Server::Lid)),
                participants: &enough,
                audio_rate: 16_000,
                video: false,
            })
            .is_err()
        );
    }

    #[test]
    fn participant_invite_keeps_destination_before_roster() {
        let creator = jid("100001", 1);
        let target = Jid::new("400004", Server::Lid);
        let target_devices = [jid("400004", 4), jid("400004", 5)];
        let roster = [
            participant("100001", 1, &CAPABILITY_OFFER),
            participant("200002", 2, &CAPABILITY_OFFER),
        ];
        let node = build_group_invite_offer(&GroupInviteOfferParams {
            call_id: "CID",
            id: "REQ",
            to: &target,
            call_creator: &creator,
            target_devices: &target_devices,
            participants: &roster,
            video: true,
        })
        .expect("valid invite");
        assert_eq!(
            child_tags(action(&node)),
            ["audio", "video", "net", "destination", "group_info"]
        );
        let action_node = action(&node);
        let action_ref = action_node.as_node_ref();
        let destination = action_ref.get_optional_child("destination").unwrap();
        let addressed = destination
            .children()
            .unwrap()
            .iter()
            .filter_map(|child| child.get_attr("jid").and_then(|value| value.to_jid()))
            .collect::<Vec<_>>();
        assert_eq!(addressed, target_devices);
    }

    #[test]
    fn active_group_preaccept_and_accept_use_call_scoped_routing() {
        let creator = jid("111111111111111", 0);
        let preaccept =
            build_active_group_preaccept("ACTIVE-CALL", &creator, "PRE-ID", true).unwrap();
        assert_eq!(
            preaccept.as_node_ref().attrs().optional_jid("to").unwrap(),
            Jid::new("ACTIVE-CALL", Server::Call)
        );
        assert_eq!(
            child_tags(action(&preaccept)),
            ["audio", "video", "encopt", "capability"]
        );

        let accept = build_active_group_accept("ACTIVE-CALL", &creator, "ACCEPT-ID", true).unwrap();
        assert_eq!(
            accept.as_node_ref().attrs().optional_jid("to").unwrap(),
            Jid::new("ACTIVE-CALL", Server::Call)
        );
        assert_eq!(
            child_tags(action(&accept)),
            ["audio", "video", "net", "encopt"]
        );
        assert!(
            action(&accept)
                .as_node_ref()
                .get_optional_child("metadata")
                .is_none()
        );
    }

    #[test]
    fn group_update_preserves_transaction_roster_relay_and_rekey() {
        let creator = jid("100001", 1);
        let device = jid("200002", 2);
        let node = NodeBuilder::new("group_update")
            .attr("call-id", "CID")
            .attr("call-creator", &creator)
            .children([
                NodeBuilder::new("group_info")
                    .attr("transaction-id", "7")
                    .attr("media", "video")
                    .attr("connected-limit", "8")
                    .attr("joinable", "1")
                    .attr("rekey", "1")
                    .children([NodeBuilder::new("user")
                        .attr("jid", Jid::new("200002", Server::Lid))
                        .attr("state", "connected")
                        .children([NodeBuilder::new("device")
                            .attr("jid", &device)
                            .attr("pid", "22")
                            .attr("platform", "web")
                            .build()])
                        .build()])
                    .build(),
                NodeBuilder::new("av_upgrade")
                    .attr("av-upgradable", "1")
                    .build(),
                NodeBuilder::new("relay")
                    .attr("transaction-id", "7")
                    .attr("self_pid", "11")
                    .attr("uuid", "relay-uuid")
                    .attr("participant_uuid", "participant-uuid")
                    .attr("warp_mi_tag_len", "8")
                    .children([
                        NodeBuilder::new("key").bytes(b"relay-key".to_vec()).build(),
                        NodeBuilder::new("hbh_key")
                            .bytes(b"hbh-key".to_vec())
                            .build(),
                        NodeBuilder::new("token")
                            .attr("id", "0")
                            .bytes(b"token".to_vec())
                            .build(),
                        NodeBuilder::new("auth_token")
                            .attr("id", "0")
                            .bytes(b"auth".to_vec())
                            .build(),
                        NodeBuilder::new("te2")
                            .attr("relay_id", "3")
                            .attr("token_id", "0")
                            .attr("auth_token_id", "0")
                            .attr("relay_name", "zrh1c01")
                            .attr("c2r_rtt", "24")
                            .attr("is_fna", "0")
                            .bytes(vec![1, 2, 3, 4, 0x1f, 0x90])
                            .build(),
                    ])
                    .build(),
            ])
            .build();

        let update = parse_group_update(&node.as_node_ref()).expect("valid update");
        assert_eq!(update.transaction_id, 7);
        assert_eq!(update.media, "video");
        assert!(update.joinable);
        assert!(update.av_upgradable);
        assert!(update.rekey_requested);
        assert_eq!(update.participants[0].devices[0].pid, Some(22));
        let relay = update.relay.expect("relay");
        assert_eq!(relay.self_pid, Some(11));
        assert_eq!(relay.key, b"relay-key");
        assert_eq!(relay.tokens, [b"token".to_vec()]);
        assert_eq!(relay.endpoints[0].ipv4.as_deref(), Some("1.2.3.4"));
        assert_eq!(relay.endpoints[0].port, Some(8080));
    }

    #[test]
    fn group_update_rejects_missing_and_invalid_transaction_fields() {
        let creator = jid("100001", 1);
        for transaction in [None, Some("not-a-number")] {
            let mut info = NodeBuilder::new("group_info")
                .attr("media", "audio")
                .attr("connected-limit", "8");
            if let Some(transaction) = transaction {
                info = info.attr("transaction-id", transaction);
            }
            let node = NodeBuilder::new("group_update")
                .attr("call-id", "CID")
                .attr("call-creator", &creator)
                .children([info.build()])
                .build();
            assert!(parse_group_update(&node.as_node_ref()).is_err());
        }
    }

    #[test]
    fn group_epoch_round_trips_and_rejects_wrong_target() {
        let creator = jid("100001", 1);
        let target = jid("200002", 2);
        let key = OfferDeviceKey {
            device_jid: target.clone(),
            ciphertext: vec![1, 2, 3],
            enc_type: "msg".to_string(),
        };
        let node = build_group_enc_rekey(&GroupEncRekeyParams {
            call_id: "CID",
            id: "REQ",
            to: &target,
            call_creator: &creator,
            transaction_id: 9,
            device_key: &key,
            device_identity: None,
        })
        .expect("valid group rekey");
        let parsed = parse_group_enc_rekey(&action(&node).as_node_ref()).expect("parse rekey");
        assert_eq!(parsed.transaction_id, 9);
        assert_eq!(parsed.key_generation, 2);
        assert_eq!(parsed.encryption_type, "msg");
        assert_eq!(parsed.ciphertext, [1, 2, 3]);

        let wrong = jid("300003", 3);
        assert!(
            build_group_enc_rekey(&GroupEncRekeyParams {
                call_id: "CID",
                id: "REQ",
                to: &wrong,
                call_creator: &creator,
                transaction_id: 9,
                device_key: &key,
                device_identity: None,
            })
            .is_err()
        );
    }

    #[test]
    fn prekey_group_epoch_includes_device_identity() {
        let creator = jid("100001", 1);
        let target = jid("200002", 2);
        let key = OfferDeviceKey {
            device_jid: target.clone(),
            ciphertext: vec![4, 5, 6],
            enc_type: "pkmsg".to_string(),
        };
        assert!(
            build_group_enc_rekey(&GroupEncRekeyParams {
                call_id: "CID",
                id: "REQ",
                to: &target,
                call_creator: &creator,
                transaction_id: 10,
                device_key: &key,
                device_identity: None,
            })
            .is_err(),
            "a pre-key envelope is unusable without the sender's ADV identity"
        );

        let identity = [7, 8, 9];
        let node = build_group_enc_rekey(&GroupEncRekeyParams {
            call_id: "CID",
            id: "REQ",
            to: &target,
            call_creator: &creator,
            transaction_id: 10,
            device_key: &key,
            device_identity: Some(&identity),
        })
        .expect("pre-key rekey with identity");
        let rekey = action(&node);
        assert_eq!(child_tags(rekey), ["encopt", "enc", "device-identity"]);
        assert_eq!(
            rekey
                .as_node_ref()
                .get_optional_child("device-identity")
                .and_then(|child| child.content_bytes()),
            Some(identity.as_slice())
        );
    }

    #[test]
    fn call_link_builders_match_service_and_call_scoped_routes() {
        let create = build_call_link_create(CallLinkMedia::Video, "REQ-1").unwrap();
        assert_eq!(
            create.attrs.get("to").and_then(|value| value.to_jid()),
            Some(Jid::new("", Server::Call))
        );
        assert_eq!(
            action(&create)
                .attrs
                .get("media")
                .map(|value| value.as_str().into_owned())
                .as_deref(),
            Some("video")
        );

        let join = build_call_link_join("TOKEN", CallLinkMedia::Video, "REQ-2").unwrap();
        assert_eq!(
            child_tags(action(&join)),
            ["audio", "video", "net", "capability"]
        );
        let query = build_call_link_query("TOKEN", CallLinkMedia::Audio, "REQ-QUERY").unwrap();
        assert_eq!(
            action(&query)
                .attrs
                .get("token")
                .map(|value| value.as_str().into_owned())
                .as_deref(),
            Some("TOKEN")
        );
        let standard_join = build_call_link_join_with_capability(
            "TOKEN",
            CallLinkMedia::Audio,
            "REQ-2B",
            &CAPABILITY_STANDARD_OPUS_OFFER,
        )
        .unwrap();
        assert_eq!(
            action(&standard_join)
                .as_node_ref()
                .get_optional_child("capability")
                .expect("capability")
                .content_bytes(),
            Some(CAPABILITY_STANDARD_OPUS_OFFER.as_slice())
        );

        let creator = jid("100001", 1);
        let user = Jid::new("200002", Server::Lid);
        for node in [
            build_waiting_room_toggle("CID", &creator, true, "REQ-3"),
            build_waiting_room_admit("CID", &creator, &user, "REQ-4"),
            build_waiting_room_deny("CID", &creator, &user, "REQ-5"),
        ] {
            let node = node.expect("valid waiting-room request");
            assert_eq!(
                node.attrs.get("to").and_then(|value| value.to_jid()),
                Some(Jid::new("CID", Server::Call))
            );
        }
        assert!(build_waiting_room_heartbeat("", &creator, "REQ-6").is_err());
        assert!(
            build_raise_hand("CID", &Jid::new("", Server::Call), &creator, "REQ-7", true,).is_err()
        );
        assert!(
            build_screen_share(
                "CID",
                &Jid::new("CID", Server::Call),
                &creator,
                "",
                ScreenShareState::Started,
                None,
            )
            .is_err()
        );
    }

    #[test]
    fn call_link_query_ack_allows_missing_waiting_room() {
        let creator = jid("100001", 1);
        let ack = NodeBuilder::new("ack")
            .attr("class", "call")
            .attr("type", "link_query")
            .children([NodeBuilder::new("link_query")
                .attr("token", "TOKEN")
                .attr("media", "audio")
                .attr("link_creator", &creator)
                .build()])
            .build();
        let preview = parse_call_link_query_ack(&ack.as_node_ref()).expect("valid query ack");
        assert!(!preview.waiting_room_enabled);
        assert!(!preview.is_admin);
    }

    #[test]
    fn relay_tokens_preserve_out_of_order_entries() {
        let children = [
            NodeBuilder::new("token")
                .attr("id", "2")
                .bytes(b"two".to_vec())
                .build(),
            NodeBuilder::new("token")
                .attr("id", "0")
                .bytes(b"zero".to_vec())
                .build(),
        ];
        let refs = children.iter().map(Node::as_node_ref).collect::<Vec<_>>();
        assert_eq!(
            parse_indexed_tokens(&refs, "token"),
            [b"zero".to_vec(), Vec::new(), b"two".to_vec()]
        );
    }

    #[test]
    fn call_link_join_distinguishes_waiting_and_admitted() {
        let creator = jid("100001", 1);
        let waiting = NodeBuilder::new("ack")
            .attr("class", "call")
            .attr("type", "link_join")
            .children([NodeBuilder::new("waiting_room")
                .attr("call-id", "CID")
                .attr("call-creator", &creator)
                .attr("link-token", "TOKEN")
                .attr("media", "video")
                .attr("enabled", "1")
                .attr("is_admin", "0")
                .attr("transaction-id", "4")
                .build()])
            .build();
        let result = parse_call_link_join_ack(&waiting.as_node_ref()).unwrap();
        assert!(result.in_waiting_room);
        assert!(result.group.is_none());

        let admitted = NodeBuilder::new("ack")
            .attr("class", "call")
            .attr("type", "link_join")
            .children([NodeBuilder::new("group_info")
                .attr("call-id", "CID")
                .attr("call-creator", &creator)
                .attr("transaction-id", "5")
                .attr("media", "video")
                .attr("connected-limit", "8")
                .build()])
            .build();
        let result = parse_call_link_join_ack(&admitted.as_node_ref()).unwrap();
        assert!(!result.in_waiting_room);
        assert_eq!(result.group.unwrap().transaction_id, 5);

        let disabled_waiting_only = NodeBuilder::new("ack")
            .attr("class", "call")
            .attr("type", "link_join")
            .children([NodeBuilder::new("waiting_room")
                .attr("call-id", "CID")
                .attr("call-creator", &creator)
                .attr("link-token", "TOKEN")
                .attr("media", "video")
                .attr("enabled", "0")
                .attr("is_admin", "0")
                .build()])
            .build();
        assert!(parse_call_link_join_ack(&disabled_waiting_only.as_node_ref()).is_err());
    }

    #[test]
    fn waiting_room_control_acks_reject_wrong_type_and_server_errors() {
        let ok = NodeBuilder::new("ack")
            .attr("class", "call")
            .attr("type", "waiting_room_toggle")
            .build();
        assert!(parse_waiting_room_toggle_ack(&ok.as_node_ref()).is_ok());
        assert!(parse_waiting_room_admit_ack(&ok.as_node_ref()).is_err());

        let rejected = NodeBuilder::new("ack")
            .attr("class", "call")
            .attr("type", "waiting_room_deny")
            .attr("error", "403")
            .build();
        assert!(parse_waiting_room_deny_ack(&rejected.as_node_ref()).is_err());
    }

    #[test]
    fn participant_controls_round_trip_and_ack_preserves_routing() {
        let creator = jid("100001", 1);
        let target = Jid::new("CID", Server::Call);
        let raised = build_raise_hand("CID", &target, &creator, "REQ-1", true).unwrap();
        assert!(parse_raise_hand(&action(&raised).as_node_ref()).unwrap());

        let share = build_screen_share(
            "CID",
            &target,
            &creator,
            "REQ-2",
            ScreenShareState::Started,
            Some(7),
        )
        .unwrap();
        let parsed = parse_screen_share(&action(&share).as_node_ref()).unwrap();
        assert_eq!(parsed.state, ScreenShareState::Started);
        assert_eq!(parsed.version, 2);
        assert_eq!(parsed.screen_share_id, Some(7));
        let overflow = NodeBuilder::new("screen_share")
            .attr("call-id", "CID")
            .attr("call-creator", &creator)
            .attr("screenshare_state", u32::MAX.to_string())
            .attr("version", "2")
            .build();
        assert!(parse_screen_share(&overflow.as_node_ref()).is_err());

        let from = jid("100001", 1);
        let participant = jid("100001", 2);
        let recipient = jid("200002", 3);
        let original = NodeBuilder::new("call")
            .attr("id", "REQ")
            .attr("from", &from)
            .attr("participant", &participant)
            .attr("recipient", &recipient)
            .build();
        let ack =
            build_call_control_ack(&original.as_node_ref(), "screen_share").expect("typed ack");
        assert_eq!(
            ack.attrs.get("to").and_then(|value| value.to_jid()),
            Some(from)
        );
        assert_eq!(
            ack.attrs
                .get("participant")
                .and_then(|value| value.to_jid()),
            Some(participant)
        );
        assert_eq!(
            ack.attrs.get("recipient").and_then(|value| value.to_jid()),
            Some(recipient)
        );
    }
}
