//! Direct-call ingress shared by the native dispatcher and external socket adapters.

use super::direct::DirectCallHost;
use crate::client::CallError;
use log::{debug, warn};
use std::sync::Arc;
use wacore::stanza::call::{
    CAPABILITY_INDEX_MLOW_V1, CapabilityBit, REJECT_REASON_BUSY, REJECT_REASON_ENC,
    TERMINATE_REASON_ACCEPTED_ELSEWHERE, TERMINATE_REASON_REJECTED_ELSEWHERE, TerminateParams,
    VideoStateParams, build_offer_ack_receipt, build_terminate, build_video_state, capability_bit,
    parse_call_stanza,
};
use wacore::types::call::{CallAction, CallActionTag, IncomingCall, VideoState};
use wacore::types::group_call::GroupCallDevice;
use wacore::voip_control::CallEvent;
use wacore_binary::{Jid, NodeRef, Server};

pub(crate) type AuthorizedAnswer = (u64, Jid, Jid);

pub(crate) enum AnswerAuthorization {
    Continue {
        answer: Option<AuthorizedAnswer>,
        alias: Option<Jid>,
    },
    Observe,
    Ignore,
}

pub(crate) async fn authorize_answer(
    client: &dyn DirectCallHost,
    call: &mut IncomingCall,
) -> AnswerAuthorization {
    let mut direct_accept_alias = None;
    let accepting = matches!(&call.action, CallAction::Accept { .. });
    let rejecting = matches!(&call.action, CallAction::Reject { .. });
    let terminating = matches!(&call.action, CallAction::Terminate { .. });
    let preaccepting = matches!(&call.action, CallAction::PreAccept { .. });
    let answer = if accepting || rejecting || terminating || preaccepting {
        let registry = client.call_registry();
        match registry.generation_of(call.action.call_id()) {
            Some(generation)
                if registry.is_group_call_if_current(call.action.call_id(), generation) =>
            {
                None
            }
            Some(generation) => {
                let Some(session) = registry.snapshot_if_current(call.action.call_id(), generation)
                else {
                    return AnswerAuthorization::Ignore;
                };
                if accepting
                    && (session.direction != wacore::voip_control::CallDirection::Outgoing
                        || session.answering_device.is_some())
                {
                    return AnswerAuthorization::Ignore;
                }
                // Participant metadata is authoritative only inside this call's
                // service envelope, never on a peer-authored or foreign-call stanza.
                if call.participant.is_some()
                    && !is_call_service_sender(&call.from, call.action.call_id())
                {
                    return AnswerAuthorization::Ignore;
                }
                let observed = routed_call_sender(call);
                // Preserve the native trusted-service authority model for terminal events.
                // This narrow compatibility exception is not an observed direct wire fixture:
                // peer-authored and participant-routed traffic still requires peer identity.
                let service_termination = terminating
                    && call.participant.is_none()
                    && is_call_service_sender(&call.from, call.action.call_id());
                let sender = if service_termination {
                    Some(observed.clone())
                } else {
                    match canonical_call_identity(client, &observed, &session.peer_jid).await {
                        Some(peer) => Some(peer),
                        // A linked sibling may decline the same call for our account. Its identity
                        // must still be a trusted alias of ours, never the stanza's caller_pn hint.
                        None if rejecting || terminating => {
                            if let Some(own) = client.own_lid() {
                                canonical_call_identity(client, &observed, &own).await
                            } else {
                                None
                            }
                        }
                        None => None,
                    }
                };
                let Some(sender) = sender else {
                    return AnswerAuthorization::Ignore;
                };
                let Some(creator) = canonical_call_identity(
                    client,
                    call.action.call_creator(),
                    &session.call_creator,
                )
                .await
                else {
                    return AnswerAuthorization::Ignore;
                };
                if accepting {
                    direct_accept_alias = client.namespace_alias(&sender).await;
                }
                if !registry.is_current(call.action.call_id(), generation) {
                    return AnswerAuthorization::Ignore;
                }
                if call.participant.is_some() {
                    call.participant = Some(sender.clone());
                } else {
                    call.from = sender.clone();
                }
                Some((generation, creator, sender))
            }
            None => {
                return AnswerAuthorization::Observe;
            }
        }
    } else {
        None
    };
    AnswerAuthorization::Continue {
        answer,
        alias: direct_accept_alias,
    }
}

pub(crate) async fn apply_answer(
    client: &Arc<dyn DirectCallHost>,
    call: &IncomingCall,
    nr: &NodeRef<'_>,
    direct_accept: Option<&AuthorizedAnswer>,
    direct_accept_alias: Option<&Jid>,
) -> bool {
    let epoch = client.call_state().epoch();
    let initial_generation = client.call_registry().generation_of(call.action.call_id());
    let current = || operation_current(&**client, call.action.call_id(), epoch, initial_generation);
    if direct_accept.is_some_and(|(generation, _, _)| initial_generation != Some(*generation)) {
        return false;
    }
    if let CallAction::PreAccept { audio, .. } | CallAction::Accept { audio, .. } = &call.action
        && !audio.is_empty()
        && let Some(expected) = client
            .call_registry()
            .snapshot(call.action.call_id())
            .and_then(|session| session.audio_format)
        && !audio.iter().any(|codec| {
            codec.enc.eq_ignore_ascii_case("opus") && codec.rate == expected.signaling_rate
        })
    {
        let received_rates = audio.iter().map(|codec| codec.rate).collect::<Vec<_>>();
        warn!(
            "call: peer selected audio rates {received_rates:?}, expected {}",
            expected.signaling_rate
        );
        dismiss_outgoing_siblings(&**client, call, epoch, initial_generation).await;
        if !current() {
            return false;
        }
        client.call_registry().send_call_event(
            call.action.call_id(),
            CallEvent::AudioFormatMismatch {
                expected_rate: expected.signaling_rate,
                received_rates,
            },
        );
        let registry = client.call_registry();
        let is_group = registry
            .generation_of(call.action.call_id())
            .is_some_and(|generation| {
                registry.is_group_call_if_current(call.action.call_id(), generation)
            });
        if is_group {
            dismiss_incompatible_group_invitee(&**client, call, epoch, initial_generation).await;
        } else if let Some(generation) = registry.generation_of(call.action.call_id()) {
            let _transition = client.lock_answer_transition(call.action.call_id()).await;
            if !current() || !registry.is_current(call.action.call_id(), generation) {
                return false;
            }
            let delivery = super::direct::terminate_for_generation(
                &**client,
                call.action.call_id(),
                std::slice::from_ref(&routed_call_sender(call)),
                call.action.call_creator(),
                generation,
            )
            .await;
            if let Some(error) = delivery.failure {
                warn!("call: failed to terminate incompatible audio profile: {error}");
            }
        }
        return false;
    }
    // Defaults to Unknown, which is "the peer said nothing" and resets nothing:
    // a `<preaccept>`/`<accept>` we never parsed must not look like a refusal.
    let mut peer_mlow_bit = CapabilityBit::Unknown;
    let mut accepting_device_capability = None;
    let mut caller_transport: Option<(Jid, Jid)> = None;
    if matches!(
        &call.action,
        CallAction::PreAccept { .. } | CallAction::Accept { .. }
    ) && let Some(generation) = direct_accept
        .map(|(generation, _, _)| *generation)
        .or_else(|| client.call_registry().generation_of(call.action.call_id()))
    {
        let capability = nr
            .children()
            .and_then(|children| {
                children
                    .iter()
                    .find(|action| CallActionTag::try_from(action.tag.as_ref()).is_ok())
            })
            .and_then(|action| action.get_optional_child("capability"));
        // Only an ABSENT node is `Unknown`; a present one reads through
        // `capability_bit`, which owns what an unreadable `ver` means.
        peer_mlow_bit = capability.map_or(CapabilityBit::Unknown, |capability| {
            let version = capability
                .get_attr("ver")
                .and_then(|version| version.as_str().parse::<u32>().ok());
            let bytes = capability.content_bytes().unwrap_or_default();
            capability_bit(version, bytes, CAPABILITY_INDEX_MLOW_V1)
        });
        let mut capability_sender = routed_call_sender(&call);
        if matches!(&call.action, CallAction::PreAccept { .. })
            && !client
                .call_registry()
                .is_group_call_if_current(call.action.call_id(), generation)
            && let Some(session) = client
                .call_registry()
                .snapshot_if_current(call.action.call_id(), generation)
            && let Some(canonical) =
                canonical_call_identity(&**client, &capability_sender, &session.peer_jid).await
        {
            // The accept authenticates in the session's namespace. Retain earlier
            // codec statements under that same trusted device identity.
            capability_sender = canonical;
        }
        if !current() {
            return false;
        }
        // A device states its codec capability in the `<preaccept>` and its
        // `<accept>` need not repeat it -- a video answer omits the child by
        // construction. Retained per device so the accept below reads what THIS
        // device said rather than treating the omission as "the peer said nothing",
        // which would leave MLow on against a peer outside the rollout.
        client.call_registry().note_peer_capability(
            call.action.call_id(),
            generation,
            &capability_sender,
            peer_mlow_bit,
        );
        let device = if let Some(capability) = capability
            && let Some(bytes) = capability.content_bytes().filter(|bytes| !bytes.is_empty())
        {
            let version = match capability.get_attr("ver") {
                None => Some(1),
                Some(version) => version.as_str().parse::<u32>().ok(),
            };
            if let Some(version) = version {
                Some(
                    GroupCallDevice::new(capability_sender)
                        .with_capability(version, bytes.to_vec()),
                )
            } else {
                warn!(
                    "call: ignored invalid peer capability version for {}",
                    call.action.call_id()
                );
                None
            }
        } else {
            None
        };
        if matches!(&call.action, CallAction::Accept { .. }) {
            accepting_device_capability = device;
        } else if let Some(device) = device {
            client.call_registry().set_group_invite_peer_device(
                call.action.call_id(),
                generation,
                device,
            );
        }
    }
    // Caller-side: key our recv path to the device that actually answered. We dial the
    // base callee LID, but a companion answers from `:N` and encrypts under its own
    // device id; without this every inbound frame decrypts to garbage. One-shot, and a
    // no-op for an incoming call or a call we aren't the caller of (no sender registered).
    if let CallAction::Accept { .. } = &call.action {
        let sender = routed_call_sender(&call);
        // Record the device that answered so a later <terminate> targets it (call
        // signaling is addressed per device, not to the bare peer the offer rang).
        if direct_accept.is_none() {
            client
                .call_registry()
                .set_answering_device(call.action.call_id(), sender.clone());
        }
        // The peer's capability and the answering device land in the same stanza
        // and both have to be applied before the first inbound packet, so they
        // travel as one message rather than racing.
        let mut peer_mlow_bit = client.call_registry().resolve_peer_capability(
            call.action.call_id(),
            &sender,
            peer_mlow_bit,
        );
        if peer_mlow_bit == CapabilityBit::Unknown
            && let Some(alias) = direct_accept_alias
        {
            // The mapping may have arrived after preaccept. Never borrow another
            // device's rollout bit: this trusted alias preserves the device id.
            peer_mlow_bit = client.call_registry().resolve_peer_capability(
                call.action.call_id(),
                alias,
                CapabilityBit::Unknown,
            );
        }
        let audio_codec = client
            .call_registry()
            .peer_selected_audio_codec(call.action.call_id(), peer_mlow_bit);
        let answer = wacore::voip_control::control::PeerAnswer::builder()
            .answering_lid(sender.to_string())
            .maybe_audio_codec(audio_codec)
            .build();
        if let Some((generation, creator, device)) = direct_accept {
            if !client
                .call_registry()
                .accept_direct_if_current(
                    call.action.call_id(),
                    *generation,
                    creator,
                    device.clone(),
                    answer,
                )
                .await
            {
                return false;
            }
            if !current() {
                return false;
            }
            // #1098: answer the callee's `<accept>` with our `<transport>`, as the
            // phone does when it is the caller. Sent after the accept's own
            // stanzas (rotation, sibling dismiss) below.
            caller_transport = Some((sender.clone(), creator.clone()));
        } else {
            client
                .call_registry()
                .send_rekey(call.action.call_id(), answer);
        }
        // Only the answer that reached media may select promotion capabilities.
        // A refused or concurrent sibling answer must not latch a different device.
        if let Some(generation) = direct_accept
            .map(|(generation, _, _)| *generation)
            .or_else(|| client.call_registry().generation_of(call.action.call_id()))
        {
            if let Some(device) = accepting_device_capability {
                client.call_registry().select_group_invite_peer_device(
                    call.action.call_id(),
                    generation,
                    Some(device),
                );
            } else {
                client
                    .call_registry()
                    .select_group_invite_peer_without_capability(
                        call.action.call_id(),
                        generation,
                        &sender,
                    );
            }
        }
        // The answering device's camera rotation, announced on
        // the `<accept>`'s `<video>` and nowhere else until it
        // turns. After `set_answering_device`, so the registry
        // can tell a winning answer from a late sibling's.
        if let Some(orientation) = call.video_orientation
            && let Some(generation) = direct_accept
                .map(|(generation, _, _)| *generation)
                .or_else(|| client.call_registry().generation_of(call.action.call_id()))
        {
            client.call_registry().set_peer_video_orientation(
                call.action.call_id(),
                generation,
                &sender,
                orientation,
            );
        }
        if let Some(generation) = direct_accept
            .map(|(generation, _, _)| *generation)
            .or_else(|| client.call_registry().generation_of(call.action.call_id()))
        {
            announce_orientation_on_accept(
                &**client,
                call.action.call_id(),
                generation,
                &sender,
                epoch,
            )
            .await;
            if !current() {
                return false;
            }
        }
    }
    // Caller-side multi-device dismiss: when one of the callee's devices accepts or
    // rejects an outbound call of ours, tell the rest to stop ringing.
    dismiss_outgoing_siblings(&**client, call, epoch, initial_generation).await;
    if !current() {
        return false;
    }
    if let Some((to, creator)) = caller_transport.take() {
        let transport =
            wacore::stanza::call::build_transport(&wacore::stanza::call::TransportParams {
                call_id: call.action.call_id(),
                to: &to,
                call_creator: &creator,
                p2p_cand_round: Some("1"),
                transport_message_type: Some("1"),
                relay_te: None,
            });
        crate::voip::facade::send_caller_call_signal_host(
            client,
            transport,
            "transport",
            call.action.call_id(),
        )
        .await;
    }
    current()
}

fn operation_current(
    host: &dyn DirectCallHost,
    id: &str,
    epoch: u64,
    generation: Option<u64>,
) -> bool {
    host.call_state().ensure_epoch(epoch).is_ok()
        && host.call_registry().generation_of(id) == generation
}

pub(crate) fn is_call_service_sender(sender: &Jid, call_id: &str) -> bool {
    sender.server == Server::Call
        && sender.device == 0
        && sender.agent == 0
        && sender.integrator == 0
        && sender.user.as_str() == call_id
}

pub(crate) fn routed_call_sender(call: &IncomingCall) -> Jid {
    call.participant.as_ref().unwrap_or(&call.from).clone()
}

/// Reads existing trusted PN/LID identity mappings; a call stanza never teaches this lookup.
/// Unknown or ambiguous aliases cannot authorize an answer to the current direct call.
pub(crate) async fn canonical_call_identity(
    client: &dyn DirectCallHost,
    observed: &Jid,
    expected: &Jid,
) -> Option<Jid> {
    let canonical = if observed.server == expected.server {
        observed.clone()
    } else {
        client.namespace_alias(observed).await?
    };
    (canonical.to_non_ad() == expected.to_non_ad()).then_some(canonical)
}

/// Caller-side sibling dismiss. When a callee device accepts/rejects one of OUR outbound calls, the
/// caller (us) tells the callee's OTHER devices to stop ringing via `<terminate reason=...>` -- the
/// dismiss is caller-driven (the callee never dismisses its own siblings; verified vs WA Web + APK).
/// The rung device set lives on the registry session (`take_dismiss_targets`), consumed one-shot so a
/// duplicate accept/reject can't re-dismiss. No-op for any other action, or a call we aren't the
/// caller of (inbound call, single-device callee, or one already dismissed). A `Terminate` needs no
/// handling here: the call ends, its registry entry (and the device set with it) goes away.
/// Whether a `<reject>` says the DEVICE is unavailable rather than that the callee declined.
///
/// `busy` is a per-device statement (already in a call, or a companion with no voice support); the
/// callee's other devices go on ringing and may still answer. `enc` is the same shape: the device
/// could not decrypt the offer (its registration changed device-side, observed with registration
/// bytes on the reject), so it decided nothing for the callee either. Any other reason - including
/// none - is the callee's own decision and ends the call.
pub(crate) fn reject_is_device_busy(action: &CallAction) -> bool {
    matches!(
        action,
        CallAction::Reject { reason, .. }
            if reason.as_deref() == Some(REJECT_REASON_BUSY)
                || reason.as_deref() == Some(REJECT_REASON_ENC)
    )
}

async fn dismiss_outgoing_siblings(
    client: &dyn DirectCallHost,
    call: &IncomingCall,
    epoch: u64,
    generation: Option<u64>,
) {
    if !operation_current(client, call.action.call_id(), epoch, generation) {
        return;
    }
    let reason = match &call.action {
        CallAction::Accept { .. } => TERMINATE_REASON_ACCEPTED_ELSEWHERE,
        // A `busy`/`enc` device has not decided anything for the callee, so its siblings must keep
        // ringing. Returning BEFORE take_dismiss_targets matters: that take is one-shot, and
        // consuming the rung set here would leave a later genuine accept with nothing to dismiss.
        CallAction::Reject { .. } if reject_is_device_busy(&call.action) => return,
        CallAction::Reject { .. } => TERMINATE_REASON_REJECTED_ELSEWHERE,
        _ => return,
    };
    let call_id = call.action.call_id();

    let Some((call_creator, devices)) = client.call_registry().take_dismiss_targets(call_id) else {
        // Either not our outgoing call, the call already deregistered, the rung set was already
        // consumed (a duplicate accept), or it rang a single device. If a multi-device callee's
        // sibling is still ringing and we land here, the rung set wasn't there to dismiss from.
        debug!("call: {reason} for {call_id}: no sibling-dismiss targets tracked");
        return;
    };

    // Send ONE <terminate> per sibling device, addressed to that DEVICE JID with a generated wrapper
    // id -- the WA Web/APK form. (A single stanza with a <destination> block to the bare peer is NOT
    // it: WA Web gates the destination fan-out to offer/enc_rekey, and the server routes call
    // signaling per device.) Skip the device that accepted/rejected: compare on device identity
    // (user + server + device), not full Jid equality, since the usync device-list and the accept's
    // `from` can carry a different `agent` for the same physical device.
    let answerer = routed_call_sender(call);
    let others: Vec<Jid> = devices
        .into_iter()
        .filter(|device| !same_device(device, &answerer))
        .collect();
    debug!(
        "call: {reason} from {} for {call_id}: dismissing {} sibling device(s)",
        answerer.observe(),
        others.len()
    );
    for dev in &others {
        if !operation_current(client, call_id, epoch, generation) {
            return;
        }
        let id = client.generate_request_id();
        let node = build_terminate(&TerminateParams {
            call_id,
            to: dev,
            id: Some(&id),
            call_creator: &call_creator,
            reason: Some(reason),
        });
        match client.send_node(node).await {
            Ok(()) => debug!(
                "call: dismissed sibling device {} ({reason}) for {call_id}",
                dev.observe()
            ),
            Err(e) => warn!(
                "call: failed to dismiss sibling device {}: {e}",
                dev.observe()
            ),
        }
    }
}

async fn dismiss_incompatible_group_invitee(
    client: &dyn DirectCallHost,
    call: &IncomingCall,
    epoch: u64,
    generation: Option<u64>,
) {
    if !operation_current(client, call.action.call_id(), epoch, generation) {
        return;
    }
    let target = routed_call_sender(call);
    if target.server == Server::Call {
        warn!(
            "call: ignored codec mismatch without a routed group invitee for {}",
            call.action.call_id()
        );
        return;
    }
    let id = client.generate_request_id();
    let node = build_terminate(&TerminateParams {
        call_id: call.action.call_id(),
        to: &target,
        id: Some(&id),
        call_creator: call.action.call_creator(),
        reason: None,
    });
    if let Err(error) = client.send_node(node).await {
        warn!(
            "call: failed to dismiss codec-incompatible group invitee {}: {error}",
            target.observe()
        );
    }
}

/// Whether two JIDs name the same device: user + server + device id. Excludes `agent`/`integrator`,
/// representation details that can differ between the usync device-list and a stanza's `from`.
fn same_device(a: &Jid, b: &Jid) -> bool {
    a.user == b.user && a.server == b.server && a.device == b.device
}

async fn announce_orientation_on_accept(
    client: &dyn DirectCallHost,
    call_id: &str,
    generation: u64,
    to: &Jid,
    epoch: u64,
) {
    if !operation_current(client, call_id, epoch, Some(generation)) {
        return;
    }
    let registry = client.call_registry();
    let orientation = registry
        .local_video_orientation(call_id, generation)
        .unwrap_or(0);
    if orientation == 0 {
        return;
    }
    let sending_video = registry
        .video_states(call_id, generation)
        .is_some_and(|(self_state, _)| self_state == VideoState::Enabled);
    if !sending_video {
        return;
    }
    let Some(session) = registry.snapshot_if_current(call_id, generation) else {
        return;
    };
    let stanza = build_video_state(&VideoStateParams {
        call_id,
        // The device that answered, not `call_creator`: on an outgoing call that
        // is our own LID, and the stanza would come straight back to us.
        to,
        id: &client.generate_request_id(),
        call_creator: &session.call_creator,
        state: VideoState::Enabled,
        dec: Some("H264"),
        device_orientation: Some(orientation),
    });
    if let Err(e) = client.send_node(stanza).await {
        warn!("call: failed to announce video orientation after accept: {e}");
    }
}

/// The host dispatcher owns generic ACKs. This function sends only call-specific receipts.
pub(crate) async fn send_offer_receipt(
    host: &dyn DirectCallHost,
    call: &IncomingCall,
) -> Result<(), CallError> {
    let own_from = if call.from.server == Server::Lid {
        host.own_lid()
    } else {
        host.own_pn()
    };
    if let Some(receipt) = build_offer_ack_receipt(call, own_from.as_ref()) {
        host.send_node(receipt).await?;
    }
    Ok(())
}

/// Only the offer's authenticated routing envelope supplies a candidate mapping. Acceptance never
/// teaches an alias that could authorize itself; the host retains conflict and durability policy.
pub(crate) async fn learn_offer_identity(
    host: &dyn DirectCallHost,
    call: &IncomingCall,
) -> Result<(), CallError> {
    let CallAction::Offer {
        caller_pn: Some(pn),
        ..
    } = &call.action
    else {
        return Ok(());
    };
    let peer = &call.from;
    if !peer.server.is_lid_family()
        || !pn.server.is_pn_family()
        || peer.user.is_empty()
        || pn.user.is_empty()
    {
        return Ok(());
    }
    let has_username = call
        .caller_username
        .as_deref()
        .is_some_and(|username| !username.is_empty());
    host.learn_identity(peer, pn, call.offline, has_username)
        .await
}

pub(crate) struct PendingOffer {
    registry: Arc<wacore::voip_control::registry::CallRegistry>,
    id: String,
    token: u64,
    retained: bool,
}

impl PendingOffer {
    pub(crate) fn new(
        registry: Arc<wacore::voip_control::registry::CallRegistry>,
        id: &str,
        token: u64,
    ) -> Self {
        Self {
            registry,
            id: id.to_owned(),
            token,
            retained: false,
        }
    }

    pub(crate) fn is_current(&self) -> bool {
        self.registry.is_current_ringing_offer(&self.id, self.token)
    }

    pub(crate) fn retain(&mut self) {
        self.retained = true;
    }
}

impl Drop for PendingOffer {
    fn drop(&mut self) {
        if !self.retained {
            self.registry
                .take_current_ringing_offer(&self.id, self.token);
        }
    }
}

/// Apply a parsed 1:1 stanza without taking ownership of the host's generic transport ACK.
pub async fn ingest_direct_call(
    host: Arc<dyn DirectCallHost>,
    node: &NodeRef<'_>,
) -> Result<Option<IncomingCall>, CallError> {
    if !host.is_connected() {
        return Err(CallError::Connect("host is disconnected".into()));
    }
    let epoch = host.call_state().epoch();
    let Some(mut call) =
        parse_call_stanza(node).map_err(|error| CallError::Setup(error.to_string()))?
    else {
        return Ok(None);
    };
    if call.group.is_some()
        || host.call_registry().is_group_call(call.action.call_id())
        || !matches!(
            &call.action,
            CallAction::Offer {
                is_video: false,
                group_jid: None,
                ..
            } | CallAction::PreAccept { .. }
                | CallAction::Accept { .. }
                | CallAction::Reject { .. }
                | CallAction::Terminate { .. }
                | CallAction::Transport { .. }
                | CallAction::RelayLatency { .. }
        )
    {
        return Err(CallError::Media(
            "external call control currently supports 1:1 audio only",
        ));
    }
    let terminal_generation = host.call_registry().generation_of(call.action.call_id());
    let (answer, alias) = match authorize_answer(&*host, &mut call).await {
        AnswerAuthorization::Continue { answer, alias } => (answer, alias),
        AnswerAuthorization::Observe => return Ok(Some(call)),
        AnswerAuthorization::Ignore => return Ok(None),
    };
    if host.call_state().ensure_epoch(epoch).is_err() {
        return Ok(None);
    }
    let mut offer = if matches!(&call.action, CallAction::Offer { .. }) {
        if call.offline {
            return Ok(None);
        }
        let Some(token) = host
            .call_registry()
            .admit_incoming_offer(call.action.call_id())
        else {
            return Ok(None);
        };
        call.set_ringing_generation(token);
        let pending = PendingOffer::new(host.call_registry(), call.action.call_id(), token);
        if let Err(error) = send_offer_receipt(&*host, &call).await {
            warn!("call: failed to send offer ack receipt: {error}");
        }
        Some(pending)
    } else {
        None
    };
    learn_offer_identity(&*host, &call).await?;
    if !host.is_connected() || host.call_state().ensure_epoch(epoch).is_err() {
        return Ok(None);
    }
    if !apply_answer(&host, &call, node, answer.as_ref(), alias.as_ref()).await
        || host.call_state().ensure_epoch(epoch).is_err()
    {
        return Ok(None);
    }
    if matches!(&call.action, CallAction::Terminate { .. }) {
        host.call_registry().take_ringing(call.action.call_id());
    }
    if (matches!(&call.action, CallAction::Terminate { .. })
        || matches!(&call.action, CallAction::Reject { .. } if !reject_is_device_busy(&call.action)))
        && let Some(generation) = terminal_generation
    {
        super::direct::terminate_local_if_current(&*host, call.action.call_id(), generation);
    }
    if offer.as_ref().is_some_and(|pending| !pending.is_current()) {
        return Ok(None);
    }
    if let Some(pending) = &mut offer {
        pending.retain();
    }
    Ok(Some(call))
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::voip::direct::{CallAckFuture, DirectCallKeys, DirectCallState};
    use async_trait::async_trait;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicBool, Ordering};
    use wacore::message_processing::EncType;
    use wacore::runtime::Runtime;
    use wacore::voip_control::{CallSession, registry::CallRegistry};
    use wacore_binary::{Node, builder::NodeBuilder};

    struct Gate {
        entered: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
    }

    struct Host {
        state: DirectCallState,
        connected: AtomicBool,
        sent: Mutex<Vec<Node>>,
        aliases: Mutex<HashMap<Jid, Jid>>,
        learned: Mutex<Vec<(Jid, Jid, bool, bool)>>,
        receipt_gate: Mutex<Option<Gate>>,
        alias_gate: Mutex<Option<Gate>>,
    }

    impl Host {
        fn new() -> Arc<Self> {
            Arc::new(Self {
                state: DirectCallState::new(Arc::new(CallRegistry::new())),
                connected: AtomicBool::new(true),
                sent: Mutex::new(Vec::new()),
                aliases: Mutex::new(HashMap::new()),
                learned: Mutex::new(Vec::new()),
                receipt_gate: Mutex::new(None),
                alias_gate: Mutex::new(None),
            })
        }
    }

    #[async_trait]
    impl DirectCallHost for Host {
        fn call_state(&self) -> &DirectCallState {
            &self.state
        }
        fn runtime(&self) -> Arc<dyn Runtime> {
            Arc::new(crate::TokioRuntime)
        }
        fn own_lid(&self) -> Option<Jid> {
            Some(own())
        }
        fn own_pn(&self) -> Option<Jid> {
            Some(Jid::pn("15550000001").with_device(1))
        }
        fn is_connected(&self) -> bool {
            self.connected.load(Ordering::Acquire)
        }
        fn generate_request_id(&self) -> String {
            "INGRESS-OUT".into()
        }
        async fn learn_identity(
            &self,
            lid: &Jid,
            pn: &Jid,
            offline: bool,
            has_username: bool,
        ) -> Result<(), CallError> {
            self.learned.lock().expect("learned").push((
                lid.clone(),
                pn.clone(),
                offline,
                has_username,
            ));
            Ok(())
        }
        async fn namespace_alias(&self, jid: &Jid) -> Option<Jid> {
            let gate = self.alias_gate.lock().expect("gate").take();
            if let Some(gate) = gate {
                gate.entered.send(()).await.expect("observe lookup");
                gate.release.recv().await.expect("release lookup");
            }
            self.aliases.lock().expect("aliases").get(jid).cloned()
        }
        async fn resolve_peer(&self, _: &Jid) -> Result<Jid, CallError> {
            panic!("ingress must not discover peers")
        }
        async fn prepare_keys(&self, _: &Jid, _: &[u8]) -> Result<DirectCallKeys, CallError> {
            panic!("ingress must not encrypt offers")
        }
        async fn decrypt(&self, _: &Jid, _: EncType, _: &[u8]) -> Result<Vec<u8>, CallError> {
            panic!("acceptance must not mutate ratchets")
        }
        async fn read_token(&self, _: &Jid) -> Result<Option<Vec<u8>>, CallError> {
            panic!("ingress must not read tokens")
        }
        async fn issue_token(&self, _: &Jid) {
            panic!("ingress must not issue tokens")
        }
        async fn send_node(&self, node: Node) -> Result<(), CallError> {
            self.sent.lock().expect("sent").push(node);
            let gate = self.receipt_gate.lock().expect("gate").take();
            if let Some(gate) = gate {
                gate.entered.send(()).await.expect("observe receipt");
                gate.release.recv().await.expect("release receipt");
            }
            Ok(())
        }
        fn watch_ack(&self, _: &str) -> Result<CallAckFuture, CallError> {
            panic!("these ingress cases do not send caller transport")
        }
    }

    fn own() -> Jid {
        Jid::lid("100000000000001").with_device(1)
    }
    fn peer() -> Jid {
        Jid::lid("100000000000002")
    }

    fn stanza(tag: &'static str, from: Jid, creator: Jid, extra: &[(&'static str, &str)]) -> Node {
        let mut action = NodeBuilder::new(tag)
            .attr("call-id", "DIRECT-INGRESS")
            .attr("call-creator", creator);
        for (key, value) in extra {
            action = action.attr(*key, *value);
        }
        NodeBuilder::new("call")
            .attr("id", "DIRECT-IN")
            .attr("from", from)
            .attr("t", "1766847151")
            .children([action.build()])
            .build()
    }

    fn register(host: &Host) -> u64 {
        host.call_registry()
            .insert(CallSession::new_outgoing("DIRECT-INGRESS", peer(), own()))
    }

    #[tokio::test]
    async fn service_authority_is_limited_to_matching_unrouted_termination() {
        for (tag, service_id, creator, participant, accepted) in [
            ("terminate", "DIRECT-INGRESS", own(), None, true),
            ("terminate", "FOREIGN", own(), None, false),
            ("terminate", "DIRECT-INGRESS", peer(), None, false),
            (
                "terminate",
                "DIRECT-INGRESS",
                own(),
                Some(Jid::new("100000000000099", Server::Lid)),
                false,
            ),
            ("reject", "DIRECT-INGRESS", own(), None, false),
            ("preaccept", "DIRECT-INGRESS", own(), None, false),
        ] {
            let host = Host::new();
            let generation = register(&host);
            let mut node = stanza(tag, Jid::new(service_id, Server::Call), creator, &[]);
            if let Some(participant) = participant {
                node.attrs.insert("participant", participant);
            }
            let result = ingest_direct_call(host.clone(), &node.as_node_ref())
                .await
                .unwrap();
            assert_eq!(result.is_some(), accepted, "{tag}/{service_id}");
            assert_eq!(
                host.call_registry()
                    .is_current("DIRECT-INGRESS", generation),
                !accepted
            );
            assert!(host.sent.lock().unwrap().is_empty());
        }
    }

    #[tokio::test]
    async fn forged_terminal_and_preaccept_stanzas_cannot_publish_or_mutate() {
        for tag in ["reject", "terminate", "preaccept"] {
            for forged_creator in [false, true] {
                let host = Host::new();
                let generation = register(&host);
                let stranger = Jid::new("100000000000099", Server::Lid);
                let node = stanza(
                    tag,
                    if forged_creator {
                        peer().with_device(2)
                    } else {
                        stranger.clone()
                    },
                    if forged_creator { stranger } else { own() },
                    &[],
                );
                assert!(
                    ingest_direct_call(host.clone(), &node.as_node_ref())
                        .await
                        .unwrap()
                        .is_none()
                );
                assert!(
                    host.call_registry()
                        .is_current("DIRECT-INGRESS", generation)
                );
                assert!(host.sent.lock().unwrap().is_empty());
            }
        }
    }

    #[tokio::test]
    async fn codec_mismatch_termination_targets_the_service_envelope_participant() {
        let host = Host::new();
        let mut session = CallSession::new_outgoing("DIRECT-INGRESS", peer(), own());
        session.audio_format = Some(wacore::voip_control::MediaAudioFormat::OPUS_16KHZ_60MS);
        host.call_registry().insert(session);
        let participant = peer().with_device(2);
        let node = NodeBuilder::new("call")
            .attr("id", "ROUTED-MISMATCH")
            .attr("from", Jid::new("DIRECT-INGRESS", Server::Call))
            .attr("participant", participant.clone())
            .attr("t", "1766847151")
            .children([NodeBuilder::new("preaccept")
                .attr("call-id", "DIRECT-INGRESS")
                .attr("call-creator", own())
                .children([NodeBuilder::new("audio")
                    .attr("enc", "opus")
                    .attr("rate", "48000")
                    .build()])
                .build()])
            .build();
        assert!(
            ingest_direct_call(host.clone(), &node.as_node_ref())
                .await
                .unwrap()
                .is_none()
        );
        let sent = host.sent.lock().unwrap();
        assert!(sent.iter().any(|node| {
            node.as_node_ref().get_optional_child("terminate").is_some()
                && node
                    .as_node_ref()
                    .get_attr("to")
                    .map(|value| value.as_str().into_owned())
                    == Some(participant.to_string())
        }));
    }

    #[tokio::test]
    async fn rejects_accept_trusted_peer_aliases_and_our_linked_siblings() {
        for sibling in [false, true] {
            let host = Host::new();
            register(&host);
            let phone = Jid::pn(if sibling {
                "15550000001"
            } else {
                "15550000002"
            })
            .with_device(2);
            let expected = if sibling { own() } else { peer() }.with_device(2);
            host.aliases.lock().unwrap().insert(phone.clone(), expected);
            let node = stanza("reject", phone, own(), &[("reason", "decline")]);
            assert!(
                ingest_direct_call(host.clone(), &node.as_node_ref())
                    .await
                    .unwrap()
                    .is_some()
            );
            assert!(host.call_registry().snapshot("DIRECT-INGRESS").is_none());
        }
    }

    #[tokio::test]
    async fn offer_sends_one_distinct_receipt_and_leaves_generic_ack_with_host() {
        let host = Host::new();
        let node = stanza("offer", peer().with_device(2), peer(), &[]);
        let event = ingest_direct_call(host.clone(), &node.as_node_ref())
            .await
            .expect("offer")
            .expect("event");
        assert!(matches!(event.action, CallAction::Offer { .. }));
        assert!(host.call_registry().is_ringing("DIRECT-INGRESS"));
        let sent = host.sent.lock().expect("sent");
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0].tag.as_ref(), "receipt");
        assert_eq!(
            sent[0]
                .as_node_ref()
                .get_attr("id")
                .map(|v| v.as_str().into_owned()),
            Some("DIRECT-IN".to_owned())
        );
        assert!(sent[0].as_node_ref().get_optional_child("offer").is_some());
    }

    #[tokio::test]
    async fn offline_offer_never_rings_or_sends_a_receipt() {
        let host = Host::new();
        let node = NodeBuilder::new("call")
            .attr("id", "OFFLINE")
            .attr("from", peer())
            .attr("offline", "1")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("offer")
                .attr("call-id", "DIRECT-INGRESS")
                .attr("call-creator", peer())
                .build()])
            .build();
        assert!(
            ingest_direct_call(host.clone(), &node.as_node_ref())
                .await
                .expect("offline")
                .is_none()
        );
        assert!(!host.call_registry().is_ringing("DIRECT-INGRESS"));
        assert!(host.sent.lock().expect("sent").is_empty());
    }

    #[tokio::test]
    async fn termination_during_receipt_send_suppresses_the_late_offer_event() {
        let host = Host::new();
        let (entered_tx, entered) = async_channel::bounded(1);
        let (release, release_rx) = async_channel::bounded(1);
        *host.receipt_gate.lock().expect("gate") = Some(Gate {
            entered: entered_tx,
            release: release_rx,
        });
        let node = stanza("offer", peer().with_device(2), peer(), &[]);
        let offer_host = host.clone();
        let offer =
            tokio::spawn(async move { ingest_direct_call(offer_host, &node.as_node_ref()).await });
        entered.recv().await.expect("receipt entered");
        let end = stanza("terminate", peer().with_device(2), peer(), &[]);
        assert!(
            ingest_direct_call(host.clone(), &end.as_node_ref())
                .await
                .expect("terminate")
                .is_some()
        );
        release.send(()).await.expect("release");
        assert!(offer.await.expect("task").expect("offer").is_none());
        assert!(!host.call_registry().is_ringing("DIRECT-INGRESS"));
    }

    #[tokio::test]
    async fn a_same_id_redelivery_supersedes_a_suspended_offer_without_stealing_its_marker() {
        let host = Host::new();
        let (entered_tx, entered) = async_channel::bounded(1);
        let (release, release_rx) = async_channel::bounded(1);
        *host.receipt_gate.lock().expect("gate") = Some(Gate {
            entered: entered_tx,
            release: release_rx,
        });
        let node = stanza("offer", peer().with_device(2), peer(), &[]);
        let first_host = host.clone();
        let first =
            tokio::spawn(async move { ingest_direct_call(first_host, &node.as_node_ref()).await });
        entered.recv().await.expect("first receipt entered");
        let newer = stanza("offer", peer().with_device(3), peer(), &[]);
        let accepted = ingest_direct_call(host.clone(), &newer.as_node_ref())
            .await
            .expect("new offer")
            .expect("new event");
        assert_eq!(accepted.from.device, 3);
        release.send(()).await.expect("release first");
        assert!(first.await.expect("task").expect("stale offer").is_none());
        assert!(host.call_registry().is_ringing("DIRECT-INGRESS"));
    }

    #[tokio::test]
    async fn cancelling_a_pending_offer_releases_only_its_own_ringing_marker() {
        let host = Host::new();
        let (entered_tx, entered) = async_channel::bounded(1);
        let (_release, release_rx) = async_channel::bounded(1);
        *host.receipt_gate.lock().expect("gate") = Some(Gate {
            entered: entered_tx,
            release: release_rx,
        });
        let node = stanza("offer", peer().with_device(2), peer(), &[]);
        let first_host = host.clone();
        let first =
            tokio::spawn(async move { ingest_direct_call(first_host, &node.as_node_ref()).await });
        entered.recv().await.expect("receipt entered");
        assert!(host.call_registry().is_ringing("DIRECT-INGRESS"));
        first.abort();
        assert!(first.await.expect_err("cancelled").is_cancelled());
        assert!(!host.call_registry().is_ringing("DIRECT-INGRESS"));
    }

    #[tokio::test]
    async fn full_offer_admission_never_sends_a_receipt_or_publishes_an_offer() {
        let host = Host::new();
        for index in 0..256 {
            assert!(
                host.call_registry()
                    .try_mark_incoming_ringing(&format!("RING-{index}"))
            );
        }
        let node = stanza("offer", peer().with_device(2), peer(), &[]);
        assert!(
            ingest_direct_call(host.clone(), &node.as_node_ref())
                .await
                .expect("overflow")
                .is_none()
        );
        assert!(!host.call_registry().is_ringing("DIRECT-INGRESS"));
        assert!(host.sent.lock().expect("sent").is_empty());
        assert!(host.learned.lock().expect("learned").is_empty());
    }

    #[tokio::test]
    async fn identity_candidate_comes_from_offer_sender_and_never_an_accept() {
        let host = Host::new();
        let phone = Jid::pn("15550000002").with_device(7);
        let sender = peer().with_device(2);
        let creator = Jid::lid("100000000000003");
        let node = stanza(
            "offer",
            sender.clone(),
            creator,
            &[
                ("caller_pn", &phone.to_string()),
                ("username", "synthetic-name"),
            ],
        );
        ingest_direct_call(host.clone(), &node.as_node_ref())
            .await
            .expect("offer");
        assert_eq!(
            *host.learned.lock().expect("learned"),
            vec![(sender, phone, false, true)]
        );
        let accepted = stanza(
            "accept",
            peer().with_device(2),
            own(),
            &[("caller_pn", "15550000003@s.whatsapp.net")],
        );
        ingest_direct_call(host.clone(), &accepted.as_node_ref())
            .await
            .expect("unmanaged accept");
        assert_eq!(host.learned.lock().expect("learned").len(), 1);
    }

    #[tokio::test]
    async fn disconnect_during_receipt_wait_does_not_publish_or_retain_an_offer() {
        let host = Host::new();
        let (entered_tx, entered) = async_channel::bounded(1);
        let (release, release_rx) = async_channel::bounded(1);
        *host.receipt_gate.lock().expect("gate") = Some(Gate {
            entered: entered_tx,
            release: release_rx,
        });
        let node = stanza("offer", peer().with_device(2), peer(), &[]);
        let first_host = host.clone();
        let first =
            tokio::spawn(async move { ingest_direct_call(first_host, &node.as_node_ref()).await });
        entered.recv().await.expect("receipt entered");
        host.connected.store(false, Ordering::Release);
        release.send(()).await.expect("release receipt");
        assert!(
            first
                .await
                .expect("task")
                .expect("disconnected offer")
                .is_none()
        );
        assert!(!host.call_registry().is_ringing("DIRECT-INGRESS"));
        let node = stanza("offer", peer(), peer(), &[]);
        assert!(matches!(
            ingest_direct_call(host.clone(), &node.as_node_ref()).await,
            Err(CallError::Connect(_))
        ));
        assert_eq!(host.sent.lock().expect("sent").len(), 1);
    }

    #[tokio::test]
    async fn a_foreign_accept_cannot_choose_the_answering_device() {
        let host = Host::new();
        let generation = register(&host);
        let fake = Jid::lid("100000000000003").with_device(7);
        let node = stanza("accept", fake, own(), &[]);
        assert!(
            ingest_direct_call(host.clone(), &node.as_node_ref())
                .await
                .expect("refused")
                .is_none()
        );
        let session = host
            .call_registry()
            .snapshot_if_current("DIRECT-INGRESS", generation)
            .expect("same call");
        assert!(session.answering_device.is_none());
        assert!(host.sent.lock().expect("sent").is_empty());
    }

    #[tokio::test]
    async fn trusted_namespace_lookup_preserves_the_device_and_rechecks_generation() {
        let host = Host::new();
        register(&host);
        let phone = Jid::pn("15550000002").with_device(7);
        host.aliases
            .lock()
            .expect("aliases")
            .insert(phone.clone(), peer().with_device(7));
        let (entered_tx, entered) = async_channel::bounded(1);
        let (release, release_rx) = async_channel::bounded(1);
        *host.alias_gate.lock().expect("gate") = Some(Gate {
            entered: entered_tx,
            release: release_rx,
        });
        let node = stanza("accept", phone, own(), &[]);
        let accept_host = host.clone();
        let accepting =
            tokio::spawn(async move { ingest_direct_call(accept_host, &node.as_node_ref()).await });
        entered.recv().await.expect("identity lookup entered");
        let replacement = register(&host);
        release.send(()).await.expect("release lookup");
        assert!(
            accepting
                .await
                .expect("task")
                .expect("stale answer")
                .is_none()
        );
        let current = host
            .call_registry()
            .snapshot_if_current("DIRECT-INGRESS", replacement)
            .expect("replacement");
        assert!(current.answering_device.is_none());
        assert!(host.sent.lock().expect("sent").is_empty());
    }

    #[tokio::test]
    async fn namespace_lookup_from_a_previous_connection_cannot_publish_a_preaccept() {
        let host = Host::new();
        register(&host);
        let phone = Jid::pn("15550000002").with_device(2);
        host.aliases
            .lock()
            .expect("aliases")
            .insert(phone.clone(), peer().with_device(2));
        let (entered_tx, entered) = async_channel::bounded(1);
        let (release, release_rx) = async_channel::bounded(1);
        *host.alias_gate.lock().expect("gate") = Some(Gate {
            entered: entered_tx,
            release: release_rx,
        });
        let node = stanza("preaccept", phone, own(), &[]);
        let previous_host = host.clone();
        let previous =
            tokio::spawn(
                async move { ingest_direct_call(previous_host, &node.as_node_ref()).await },
            );
        entered.recv().await.expect("lookup entered");
        crate::voip::direct::disconnect(&*host);
        let replacement = register(&host);
        release.send(()).await.expect("release lookup");
        assert!(
            previous
                .await
                .expect("task")
                .expect("previous connection")
                .is_none()
        );
        assert!(
            host.call_registry()
                .is_current("DIRECT-INGRESS", replacement)
        );
        assert!(host.sent.lock().expect("sent").is_empty());
    }

    #[tokio::test]
    async fn sibling_dismiss_stops_before_a_second_write_after_reconnect() {
        let host = Host::new();
        let mut outgoing = CallSession::new_outgoing("DIRECT-INGRESS", peer(), own());
        outgoing.ring_devices = vec![peer(), peer().with_device(2), peer().with_device(3)];
        host.call_registry().insert(outgoing);
        let (entered_tx, entered) = async_channel::bounded(1);
        let (release, release_rx) = async_channel::bounded(1);
        *host.receipt_gate.lock().expect("gate") = Some(Gate {
            entered: entered_tx,
            release: release_rx,
        });
        let node = stanza("reject", peer(), own(), &[]);
        let previous_host = host.clone();
        let previous =
            tokio::spawn(
                async move { ingest_direct_call(previous_host, &node.as_node_ref()).await },
            );
        entered.recv().await.expect("first sibling write entered");
        crate::voip::direct::disconnect(&*host);
        let replacement = register(&host);
        release.send(()).await.expect("release sibling write");
        assert!(
            previous
                .await
                .expect("task")
                .expect("previous connection")
                .is_none()
        );
        assert_eq!(host.sent.lock().expect("sent").len(), 1);
        assert!(
            host.call_registry()
                .is_current("DIRECT-INGRESS", replacement)
        );
    }

    #[tokio::test]
    async fn device_busy_keeps_the_call_but_decline_retires_it() {
        let host = Host::new();
        let generation = register(&host);
        let busy = stanza(
            "reject",
            peer().with_device(2),
            own(),
            &[("reason", "busy")],
        );
        ingest_direct_call(host.clone(), &busy.as_node_ref())
            .await
            .expect("device busy");
        assert!(
            host.call_registry()
                .is_current("DIRECT-INGRESS", generation)
        );
        let declined = stanza("reject", peer().with_device(2), own(), &[]);
        ingest_direct_call(host.clone(), &declined.as_node_ref())
            .await
            .expect("decline");
        assert!(
            host.call_registry()
                .generation_of("DIRECT-INGRESS")
                .is_none()
        );
    }

    #[tokio::test]
    async fn typed_video_and_group_offers_are_rejected_without_ack_or_state_mutation() {
        let host = Host::new();
        let generation = register(&host);
        let node = stanza(
            "video",
            peer().with_device(2),
            own(),
            &[("state", "11"), ("dec", "H264")],
        );
        let group = stanza(
            "offer",
            peer(),
            peer(),
            &[("group-jid", "100000000000001-100000000000002@g.us")],
        );
        for unsupported in [node, group] {
            assert!(matches!(
                ingest_direct_call(host.clone(), &unsupported.as_node_ref()).await,
                Err(CallError::Media(_))
            ));
        }
        assert!(
            host.call_registry()
                .is_current("DIRECT-INGRESS", generation)
        );
        assert!(host.sent.lock().expect("sent").is_empty());
    }
}
