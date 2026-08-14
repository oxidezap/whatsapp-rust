use std::sync::Arc;

use async_trait::async_trait;
use log::{debug, warn};
#[cfg(feature = "voip-runtime")]
use wacore::message_processing::EncType;
#[cfg(feature = "voip-runtime")]
use wacore::messages::MessageUtils;
#[cfg(feature = "voip-runtime")]
use wacore::stanza::call::{
    REJECT_REASON_BUSY, TERMINATE_REASON_ACCEPTED_ELSEWHERE, TERMINATE_REASON_GROUP_CALL_ENDED,
    TERMINATE_REASON_REJECTED_ELSEWHERE, TERMINATE_REASON_TIMEOUT, TerminateParams,
    VideoStateParams, build_call_video_ack, build_terminate, build_video_state,
};
use wacore::stanza::call::{build_offer_ack_receipt, parse_call_stanza};
use wacore::stanza::group_call::build_call_control_ack;
use wacore::types::call::{CallAction, CallActionTag, IncomingCall, MissedCall, MissedReason};
#[cfg(feature = "voip-runtime")]
use wacore::types::call::{CallEndedElsewhere, ElsewhereOutcome, VideoState};
use wacore::types::events::Event;
#[cfg(feature = "voip-runtime")]
use wacore::types::group_call::{GroupCallDevice, GroupCallEncRekey, ScreenShareState};
#[cfg(feature = "voip-runtime")]
use wacore::voip::GroupStateApply;
#[cfg(feature = "voip-runtime")]
use wacore::voip::{CallEvent, PeerVideoTransition, VideoControl};
#[cfg(feature = "voip-runtime")]
use wacore_binary::Jid;
use wacore_binary::{OwnedNodeRef, Server};
#[cfg(feature = "voip-runtime")]
use zeroize::Zeroizing;

#[cfg(feature = "voip-runtime")]
use crate::client::CallError;
use crate::client::Client;

use super::traits::StanzaHandler;

/// Router sends the generic `<ack>` via `should_ack`, so this handler only
/// parses and dispatches. On `Offer` it also emits the `<receipt><offer/></receipt>`
/// ack-of-offer so the caller's signaling layer knows the device received the ring.
#[derive(Default)]
pub struct CallHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for CallHandler {
    fn tag(&self) -> &'static str {
        "call"
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.call", level = "debug", skip_all)
    )]
    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<OwnedNodeRef>,
        cancelled: &mut bool,
    ) -> bool {
        let nr = node.get();
        let typed_control_type = nr
            .children()
            .and_then(|children| {
                children.iter().find(|child| {
                    matches!(
                        CallActionTag::try_from(child.tag.as_ref()),
                        Ok(CallActionTag::WaitingRoomUpdate)
                            | Ok(CallActionTag::RaiseHand)
                            | Ok(CallActionTag::ScreenShare)
                    )
                })
            })
            .map(|child| child.tag.as_ref());
        let typed_control_ack =
            typed_control_type.and_then(|action_type| build_call_control_ack(nr, action_type));
        if typed_control_type.is_some() {
            // These actions require a typed ACK. Never let the router send its generic ACK,
            // including when parsing fails or the typed send itself fails.
            *cancelled = true;
        }
        match parse_call_stanza(nr) {
            Ok(Some(call)) => {
                #[cfg(feature = "voip-runtime")]
                let mut call = call;
                #[cfg(feature = "voip-runtime")]
                if matches!(
                    &call.action,
                    CallAction::GroupUpdate { update } if update.rekey_requested
                ) {
                    // This is a transport receipt, independent of whether the authoritative
                    // transition later commits. Send it before Signal fanout can block for every
                    // participant, and suppress the router's deferred generic ACK.
                    *cancelled = true;
                    if let Err(error) = client.send_ack_for(nr).await {
                        warn!(
                            "call: failed to acknowledge group update before epoch fanout: {error}"
                        );
                    }
                }
                #[cfg(feature = "voip-runtime")]
                let is_terminate = matches!(&call.action, CallAction::Terminate { .. });
                #[cfg(feature = "voip-runtime")]
                let is_group_transition = matches!(
                    &call.action,
                    CallAction::GroupUpdate { .. }
                        | CallAction::EncRekey { .. }
                        | CallAction::WaitingRoomUpdate { .. }
                        | CallAction::RaiseHand { .. }
                        | CallAction::ScreenShare { .. }
                ) || is_terminate;
                #[cfg(feature = "voip-runtime")]
                let group_transition = if is_group_transition {
                    client
                        .call_registry()
                        .current_group_transition(call.action.call_id())
                } else {
                    None
                };
                #[cfg(feature = "voip-runtime")]
                let group_transition_generation =
                    group_transition.as_ref().map(|(generation, _)| *generation);
                #[cfg(feature = "voip-runtime")]
                let _group_transition_guard = if let Some((_, lock)) = group_transition.as_ref() {
                    Some(lock.lock().await)
                } else {
                    None
                };
                if let Some(action_type) = typed_control_type {
                    let Some(ack) = typed_control_ack else {
                        warn!(
                            "call: {action_type} stanza has no routable id; leaving it uncommitted"
                        );
                        return true;
                    };
                    if let Err(error) = send_node_boxed(&client, ack).await {
                        warn!("call: failed to send typed {action_type} ack: {error}");
                        return true;
                    }
                }
                #[cfg(feature = "voip-runtime")]
                if let Some(generation) = group_transition_generation
                    && !client
                        .call_registry()
                        .is_current(call.action.call_id(), generation)
                {
                    debug!(
                        "call: discarded stale {} after call generation changed for {}",
                        call.action.wire_tag(),
                        call.action.call_id()
                    );
                    return true;
                }
                #[cfg(feature = "voip-runtime")]
                let is_group_terminate = is_terminate
                    && group_transition_generation.is_some_and(|generation| {
                        client
                            .call_registry()
                            .is_group_call_if_current(call.action.call_id(), generation)
                    });
                #[cfg(feature = "voip-runtime")]
                if is_group_terminate
                    && !group_transition_generation.is_some_and(|generation| {
                        client.call_registry().group_creator_authorized_if_current(
                            call.action.call_id(),
                            generation,
                            call.action.call_creator(),
                            &routed_call_sender(&call),
                        )
                    })
                {
                    warn!(
                        "call: rejected group terminate from non-creator sender for {}",
                        call.action.call_id()
                    );
                    return true;
                }
                // Diagnostic: every recognized <call> action we receive (offer/accept/reject/
                // terminate/transport/relaylatency...). Lets us see whether the caller actually gets a
                // peer device's <accept> (which drives the sibling dismiss).
                debug!(
                    "call: received {} for {} from {}",
                    call.action.wire_tag(),
                    call.action.call_id(),
                    call.from.observe()
                );
                let is_offer = matches!(call.action, CallAction::Offer { .. });
                let is_offer_notice = matches!(call.action, CallAction::OfferNotice { .. });
                if (is_offer || is_offer_notice) && call.offline {
                    // Offline-queue replay (an offer or a group-call offer_notice): the call is long
                    // dead (no relay, not connectable). Don't ack or ring it -- surface a non-ringing
                    // missed-call so a consumer can't auto-accept it (WA Web drops the stale notice
                    // rather than ringing; its cancel_call + missed_call for offerReceivedWhileOffline).
                    client
                        .core
                        .event_bus
                        .dispatch(Event::MissedCall(MissedCall::new(
                            call.from.clone(),
                            call.action.call_id().to_string(),
                            call.timestamp,
                            MissedReason::Offline,
                        )));
                } else {
                    // Track an incoming offer as ringing so only an UNANSWERED <terminate> later
                    // surfaces a missed call; an answered, outgoing, or duplicate terminate must not.
                    // Mirrors WA Web's _ringingCalls. The offline branch above already surfaced its
                    // own missed-offline, so it is intentionally not marked here. Mark BEFORE the
                    // offer-ack await: <call> stanzas are processed concurrently, so a fast <terminate>
                    // for this offer racing the await must see the ringing flag (else its missed-call
                    // is lost and we'd set a stale flag after the call already ended).
                    #[cfg(feature = "voip-runtime")]
                    let mut duplicate_active_group_offer = false;
                    #[cfg(feature = "voip-runtime")]
                    let mut buffered_initial_group_controls = Vec::new();
                    #[cfg(feature = "voip-runtime")]
                    if is_offer {
                        if let Some(group) = call.group.as_deref() {
                            let mut session = wacore::voip::CallSession::new_incoming(
                                call.action.call_id(),
                                call.from.clone(),
                                call.action.call_creator().clone(),
                            );
                            session.is_video =
                                matches!(&call.action, CallAction::Offer { is_video: true, .. });
                            session.group = Some(group.clone());
                            duplicate_active_group_offer = match client
                                .call_registry()
                                .insert_ringing_group_if_inactive(session)
                            {
                                Ok(Some(generation)) => {
                                    call.set_ringing_generation(generation);
                                    buffered_initial_group_controls =
                                        client.call_registry().take_initial_group_controls(
                                            call.action.call_id(),
                                            call.action.call_creator(),
                                        );
                                    false
                                }
                                Ok(None) => true,
                                Err(reason) => {
                                    warn!(
                                        "call: rejected invalid initial group snapshot for {}: {reason:?}",
                                        call.action.call_id()
                                    );
                                    true
                                }
                            };
                        } else {
                            client
                                .call_registry()
                                .mark_incoming_ringing(call.action.call_id());
                        }
                    }
                    if is_offer && let Err(e) = send_offer_ack_receipt(&client, &call).await {
                        warn!("call: failed to send offer ack receipt: {e}");
                    }
                    #[cfg(feature = "voip-runtime")]
                    if let CallAction::PreAccept { audio, .. } | CallAction::Accept { audio, .. } =
                        &call.action
                        && !audio.is_empty()
                        && let Some(expected) = client
                            .call_registry()
                            .snapshot(call.action.call_id())
                            .and_then(|session| session.audio_format)
                        && !audio.iter().any(|codec| {
                            codec.enc.eq_ignore_ascii_case("opus")
                                && codec.rate == expected.signaling_rate
                        })
                    {
                        let received_rates =
                            audio.iter().map(|codec| codec.rate).collect::<Vec<_>>();
                        warn!(
                            "call: peer selected audio rates {received_rates:?}, expected {}",
                            expected.signaling_rate
                        );
                        dismiss_outgoing_siblings(&client, &call).await;
                        client.call_registry().send_call_event(
                            call.action.call_id(),
                            CallEvent::AudioFormatMismatch {
                                expected_rate: expected.signaling_rate,
                                received_rates,
                            },
                        );
                        let registry = client.call_registry();
                        let is_group = registry.generation_of(call.action.call_id()).is_some_and(
                            |generation| {
                                registry.is_group_call_if_current(call.action.call_id(), generation)
                            },
                        );
                        if is_group {
                            dismiss_incompatible_group_invitee(&client, &call).await;
                        } else if let Err(e) = client
                            .voip()
                            .terminate(
                                call.action.call_id(),
                                &call.from,
                                call.action.call_creator(),
                            )
                            .await
                        {
                            warn!("call: failed to terminate incompatible audio profile: {e}");
                        }
                        return true;
                    }
                    #[cfg(feature = "voip-runtime")]
                    if matches!(
                        &call.action,
                        CallAction::PreAccept { .. } | CallAction::Accept { .. }
                    ) && let Some(generation) =
                        client.call_registry().generation_of(call.action.call_id())
                    {
                        let capability = nr
                            .children()
                            .and_then(|children| {
                                children.iter().find(|action| {
                                    CallActionTag::try_from(action.tag.as_ref()).is_ok()
                                })
                            })
                            .and_then(|action| action.get_optional_child("capability"));
                        let device = if let Some(capability) = capability
                            && let Some(bytes) =
                                capability.content_bytes().filter(|bytes| !bytes.is_empty())
                        {
                            let version = match capability.get_attr("ver") {
                                None => Some(1),
                                Some(version) => version.as_str().parse::<u32>().ok(),
                            };
                            if let Some(version) = version {
                                Some(
                                    GroupCallDevice::new(routed_call_sender(&call))
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
                            if let Some(device) = device {
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
                                        &routed_call_sender(&call),
                                    );
                            }
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
                    #[cfg(feature = "voip-runtime")]
                    if let CallAction::Accept { .. } = &call.action {
                        let sender = routed_call_sender(&call);
                        // Record the device that answered so a later <terminate> targets it (call
                        // signaling is addressed per device, not to the bare peer the offer rang).
                        client
                            .call_registry()
                            .set_answering_device(call.action.call_id(), sender.clone());
                        client
                            .call_registry()
                            .send_rekey(call.action.call_id(), sender.to_string());
                        if let Some(generation) =
                            client.call_registry().generation_of(call.action.call_id())
                        {
                            announce_orientation_on_accept(
                                &client,
                                call.action.call_id(),
                                generation,
                                &sender,
                            )
                            .await;
                        }
                    }
                    // Caller-side multi-device dismiss: when one of the callee's devices accepts or
                    // rejects an outbound call of ours, tell the rest to stop ringing.
                    #[cfg(feature = "voip-runtime")]
                    dismiss_outgoing_siblings(&client, &call).await;
                    // A <terminate> for a call that was still ringing (an incoming offer we never
                    // answered) gets a terminal outcome. We mirror WA Web's
                    // ActionWebHandleIncomingSignalingMessage, which maps it from the `reason`:
                    // timeout / group_call_ended / absent -> missed; accepted_elsewhere /
                    // rejected_elsewhere -> answered/declined on another of our devices (NOT missed);
                    // any other reason -> no outcome. take_ringing is one-shot and false for an
                    // answered, outgoing, or already-terminated call, so we never misfire for our own
                    // outgoing call or a duplicate terminate; it still consumes the flag on any reason.
                    // Decided BEFORE terminate_call below.
                    #[cfg(feature = "voip-runtime")]
                    if let CallAction::Terminate {
                        call_id,
                        call_creator,
                        ..
                    } = &call.action
                        && group_transition_generation.is_none()
                    {
                        // The ACK can reveal a call-link id before its join task publishes the
                        // generation. Share its registration lane: either retain the terminal
                        // control first, or claim and remove the generation registration won.
                        let sender = routed_call_sender(&call);
                        let _ = client
                            .retain_or_apply_pending_call_link_terminate(
                                call_id,
                                call_creator,
                                &sender,
                            )
                            .await;
                    }
                    #[cfg(feature = "voip-runtime")]
                    if let CallAction::Terminate { reason, .. } = &call.action
                        && client.call_registry().take_ringing(call.action.call_id())
                    {
                        // Shared provenance for whichever outcome this reason maps to.
                        let from = call.from.clone();
                        let cid = call.action.call_id().to_string();
                        let ts = call.timestamp;
                        let outcome = match reason.as_deref() {
                            None
                            | Some(TERMINATE_REASON_TIMEOUT)
                            | Some(TERMINATE_REASON_GROUP_CALL_ENDED) => Some(Event::MissedCall(
                                MissedCall::new(from, cid, ts, MissedReason::Remote),
                            )),
                            Some(TERMINATE_REASON_ACCEPTED_ELSEWHERE) => {
                                Some(Event::CallEndedElsewhere(CallEndedElsewhere::new(
                                    from,
                                    cid,
                                    ts,
                                    ElsewhereOutcome::Accepted,
                                )))
                            }
                            Some(TERMINATE_REASON_REJECTED_ELSEWHERE) => {
                                Some(Event::CallEndedElsewhere(CallEndedElsewhere::new(
                                    from,
                                    cid,
                                    ts,
                                    ElsewhereOutcome::Rejected,
                                )))
                            }
                            _ => None,
                        };
                        if let Some(outcome) = outcome {
                            client.core.event_bus.dispatch(outcome);
                        }
                    }
                    // A `busy` reject speaks for one device, and a group reject speaks for one
                    // invited participant. Neither tears down the registered call; authoritative
                    // timeout/terminate or the group roster owns the corresponding final state.
                    #[cfg(feature = "voip-runtime")]
                    if let CallAction::Terminate { .. } = &call.action
                        && let Some(generation) = group_transition_generation
                    {
                        crate::voip::facade::terminate_call_if_current(
                            &client,
                            call.action.call_id(),
                            generation,
                        );
                    } else if matches!(&call.action, CallAction::Reject { .. }
                            if !reject_is_device_busy(&call.action)
                                && !client
                                    .call_registry()
                                    .is_group_call(call.action.call_id()))
                    {
                        crate::voip::facade::terminate_call(&client, call.action.call_id());
                    }
                    #[cfg(feature = "voip-runtime")]
                    let mut dispatch_call = !duplicate_active_group_offer;
                    #[cfg(not(feature = "voip-runtime"))]
                    let dispatch_call = true;
                    #[cfg(feature = "voip-runtime")]
                    match &call.action {
                        CallAction::GroupUpdate { update }
                            if client
                                .buffer_pending_call_link_update(update, &routed_call_sender(&call))
                                .suppresses_dispatch() =>
                        {
                            // A call-link admission can overtake the join task after its ACK wakes.
                            // Registration consumes it atomically, or rejects a saturated join.
                            dispatch_call = false;
                        }
                        CallAction::EncRekey { rekey }
                            if group_transition_generation.is_none()
                                && client.pending_call_link_control_candidate(
                                    &rekey.call_id,
                                    &rekey.call_creator,
                                    &routed_call_sender(&call),
                                ) =>
                        {
                            let sender = routed_call_sender(&call);
                            dispatch_call = match decrypt_group_epoch(&client, rekey, &sender).await
                            {
                                Ok(raw_epoch) => {
                                    let buffered = client.buffer_pending_call_link_epoch(
                                        &rekey.call_id,
                                        &rekey.call_creator,
                                        &sender,
                                        rekey.transaction_id,
                                        &raw_epoch,
                                    );
                                    if buffered.suppresses_dispatch() {
                                        false
                                    } else {
                                        apply_current_decrypted_group_epoch(
                                            &client, rekey, &sender, &raw_epoch,
                                        )
                                        .await;
                                        false
                                    }
                                }
                                Err(error) => {
                                    warn!(
                                        "call: rejected encrypted group epoch for {}: {error}",
                                        rekey.call_id
                                    );
                                    false
                                }
                            };
                        }
                        CallAction::GroupUpdate { .. } | CallAction::EncRekey { .. }
                            if group_transition_generation.is_none() =>
                        {
                            let registry = client.call_registry();
                            dispatch_call = if registry.buffer_initial_group_control(call.clone()) {
                                false
                            } else {
                                apply_current_group_control(&client, &call).await
                            };
                        }
                        CallAction::GroupUpdate { .. } | CallAction::EncRekey { .. } => {
                            dispatch_call = apply_group_control(
                                &client,
                                &call,
                                group_transition_generation
                                    .expect("matched group transition has a generation"),
                            )
                            .await;
                        }
                        CallAction::WaitingRoomUpdate { room }
                            if client
                                .buffer_pending_call_link_waiting_room(
                                    room,
                                    &routed_call_sender(&call),
                                )
                                .suppresses_dispatch() =>
                        {
                            // The ACK can wake the join task before it publishes its generation.
                            // Registration replays it in order, or rejects a saturated join.
                            dispatch_call = false;
                        }
                        CallAction::WaitingRoomUpdate { room }
                            if group_transition_generation.is_none() =>
                        {
                            dispatch_call =
                                apply_current_waiting_room_update(&client, room, &call).await;
                        }
                        CallAction::WaitingRoomUpdate { room } => {
                            dispatch_call = apply_waiting_room_update(
                                &client,
                                room,
                                &call,
                                group_transition_generation
                                    .expect("matched waiting-room update has a generation"),
                            );
                        }
                        CallAction::RaiseHand {
                            call_id,
                            call_creator,
                            raised,
                        } => {
                            let sender = routed_call_sender(&call);
                            if let Some((generation, participant)) = group_transition_generation
                                .and_then(|generation| {
                                    client
                                        .call_registry()
                                        .canonical_group_participant_if_current(
                                            call_id,
                                            generation,
                                            call_creator,
                                            &sender,
                                        )
                                        .map(|participant| (generation, participant))
                                })
                            {
                                client.call_registry().set_raised_hand_if_current(
                                    call_id,
                                    generation,
                                    &participant,
                                    *raised,
                                );
                                client.call_registry().send_call_event_if_current(
                                    call_id,
                                    generation,
                                    CallEvent::HandRaised {
                                        participant,
                                        raised: *raised,
                                    },
                                );
                            } else {
                                warn!(
                                    "call: rejected hand state from unauthorized sender for {call_id}"
                                );
                                dispatch_call = false;
                            }
                        }
                        CallAction::ScreenShare {
                            call_id,
                            call_creator,
                            screen_share,
                        } => {
                            let sender = routed_call_sender(&call);
                            if let Some((generation, participant)) = group_transition_generation
                                .and_then(|generation| {
                                    client
                                        .call_registry()
                                        .canonical_group_participant_if_current(
                                            call_id,
                                            generation,
                                            call_creator,
                                            &sender,
                                        )
                                        .map(|participant| (generation, participant))
                                })
                            {
                                let registry = client.call_registry();
                                let media_allows_share = screen_share.state
                                    != ScreenShareState::Started
                                    || registry
                                        .group_state_if_current(call_id, generation)
                                        .and_then(|state| {
                                            state
                                                .snapshot()
                                                .map(|snapshot| snapshot.media == "video")
                                        })
                                        .unwrap_or(false);
                                if !media_allows_share {
                                    warn!(
                                        "call: rejected screen-share start for audio-only group {call_id}"
                                    );
                                    dispatch_call = false;
                                } else if registry.set_screen_share_if_current(
                                    call_id,
                                    generation,
                                    &participant,
                                    screen_share.clone(),
                                ) {
                                    registry.send_call_event_if_current(
                                        call_id,
                                        generation,
                                        CallEvent::ScreenShareChanged {
                                            participant,
                                            screen_share: screen_share.clone(),
                                        },
                                    );
                                } else {
                                    dispatch_call = false;
                                }
                            } else {
                                warn!(
                                    "call: rejected screen-share state from unauthorized sender for {call_id}"
                                );
                                dispatch_call = false;
                            }
                        }
                        _ => {}
                    }
                    // In-call <video state> signaling: send the TYPED ack (`type="video"`) and
                    // suppress the router's generic one — an untyped ack makes the upgrade
                    // requester assume failure and revert after ~5s. Serialize event publication,
                    // then expose the state only after that ack commits it.
                    #[cfg(feature = "voip-runtime")]
                    if let CallAction::VideoState {
                        state, orientation, ..
                    } = &call.action
                    {
                        let call_id = call.action.call_id();
                        let registry = client.call_registry();
                        let Some((generation, transition_lock)) =
                            registry.current_video_transition(call_id)
                        else {
                            warn!(
                                "call: video state has no active call; leaving the transition unacknowledged"
                            );
                            return true;
                        };
                        let _transition_guard = transition_lock.lock().await;
                        let group_sender = if registry
                            .group_state_if_current(call_id, generation)
                            .is_some()
                        {
                            let sender = routed_call_sender(&call);
                            if registry
                                .canonical_group_participant_if_current(
                                    call_id,
                                    generation,
                                    call.action.call_creator(),
                                    &sender,
                                )
                                .is_none()
                            {
                                warn!(
                                    "call: rejected video state from unauthorized group sender for {call_id}"
                                );
                                return true;
                            }
                            Some(sender)
                        } else {
                            None
                        };
                        let event_permit = group_sender
                            .is_none()
                            .then(|| {
                                registry
                                    .is_current(call_id, generation)
                                    .then(|| registry.reserve_call_event(call_id))
                                    .flatten()
                                    .filter(|permit| permit.generation() == generation)
                            })
                            .flatten();
                        if group_sender.is_none() && event_permit.is_none() {
                            warn!(
                                "call: video state event queue is unavailable; leaving the transition unacknowledged"
                            );
                        }
                        let acked = if group_sender.is_some() || event_permit.is_some() {
                            match build_call_video_ack(&call, nr) {
                                Some(ack) => {
                                    *cancelled = true;
                                    match client.send_node(ack).await {
                                        Ok(()) => true,
                                        Err(e) => {
                                            warn!("call: failed to send typed video ack: {e}");
                                            false
                                        }
                                    }
                                }
                                None => {
                                    warn!(
                                        "call: video stanza has no id; cannot send the typed ack"
                                    );
                                    false
                                }
                            }
                        } else {
                            false
                        };
                        if let Some(sender) = group_sender {
                            // Sending the ACK crosses a transport await. Reauthorize the sender
                            // against the current roster before publishing participant-scoped state.
                            let participant = acked
                                .then(|| {
                                    registry.canonical_group_participant_if_current(
                                        call_id,
                                        generation,
                                        call.action.call_creator(),
                                        &sender,
                                    )
                                })
                                .flatten();
                            dispatch_call = participant.is_some();
                            if let (Some(participant), Some(orientation)) =
                                (participant, orientation)
                            {
                                let orientation_key = (sender.device != 0)
                                    .then(|| {
                                        registry.canonical_group_device_if_current(
                                            call_id,
                                            generation,
                                            call.action.call_creator(),
                                            &sender,
                                        )
                                    })
                                    .flatten()
                                    .unwrap_or(participant);
                                registry.send_video_ctl(
                                    call_id,
                                    generation,
                                    VideoControl::SetParticipantOrientation {
                                        participant: orientation_key,
                                        orientation: *orientation,
                                    },
                                );
                            }
                            // A group participant's `<video>` state describes only that sender.
                            // The authoritative roster owns group media mode; never feed this into
                            // the 1:1 negotiation state machine or tear down the local plane.
                            drop(event_permit);
                            drop(_transition_guard);
                            if dispatch_call {
                                client.core.event_bus.dispatch(Event::IncomingCall(call));
                            }
                            replay_initial_group_controls(&client, buffered_initial_group_controls)
                                .await;
                            return true;
                        }
                        let mut transition_current =
                            acked && registry.is_current(call_id, generation);
                        let transition = if transition_current
                            || matches!(
                                state,
                                VideoState::UpgradeReject
                                    | VideoState::UpgradeRejectByTimeout
                                    | VideoState::UpgradeCancel
                                    | VideoState::UpgradeCancelByTimeout
                                    | VideoState::Disabled
                                    | VideoState::Error
                            ) {
                            registry.apply_peer_video_state(call_id, generation, *state)
                        } else {
                            PeerVideoTransition::Ignored
                        };

                        let mut upgrade_token = None;
                        match transition {
                            PeerVideoTransition::Ignored => {
                                transition_current = false;
                            }
                            PeerVideoTransition::UpgradeRequested(token) => {
                                upgrade_token = Some(token);
                            }
                            PeerVideoTransition::Applied {
                                enable_plane,
                                teardown_local,
                                answer_upgrade,
                            } => {
                                if teardown_local
                                    && !registry.run_video_teardown(call_id, generation)
                                {
                                    registry.send_video_ctl(
                                        call_id,
                                        generation,
                                        VideoControl::Disable,
                                    );
                                }
                                if transition_current
                                    && (answer_upgrade || *state == VideoState::UpgradeAccept)
                                {
                                    if answer_upgrade {
                                        let accept = build_video_state(&VideoStateParams {
                                            call_id,
                                            to: &call.from,
                                            id: &client.generate_request_id(),
                                            call_creator: call.action.call_creator(),
                                            state: VideoState::UpgradeAccept,
                                            dec: Some("H264,AV1"),
                                            // Ours, not the peer's: accepting an
                                            // upgrade announces a direction, so it
                                            // carries the rotation the app set.
                                            device_orientation: Some(
                                                registry
                                                    .local_video_orientation(call_id, generation)
                                                    .unwrap_or(0),
                                            ),
                                        });
                                        if let Err(e) = client.send_node(accept).await {
                                            warn!(
                                                "call: failed to answer concurrent video upgrade: {e}"
                                            );
                                            transition_current = false;
                                        }
                                    }
                                    if transition_current {
                                        let enabled = build_video_state(&VideoStateParams {
                                            call_id,
                                            to: &call.from,
                                            id: &client.generate_request_id(),
                                            call_creator: call.action.call_creator(),
                                            state: VideoState::Enabled,
                                            dec: Some("H264"),
                                            device_orientation: Some(
                                                registry
                                                    .local_video_orientation(call_id, generation)
                                                    .unwrap_or(0),
                                            ),
                                        });
                                        if let Err(e) = client.send_node(enabled).await {
                                            warn!(
                                                "call: failed to announce accepted video upgrade: {e}"
                                            );
                                            transition_current = false;
                                        }
                                    }
                                    if !transition_current {
                                        if !registry.run_video_teardown(call_id, generation) {
                                            registry.send_video_ctl(
                                                call_id,
                                                generation,
                                                VideoControl::Disable,
                                            );
                                        }
                                        registry.reset_video(call_id, generation);
                                    }
                                }
                                if enable_plane && transition_current {
                                    registry.send_video_ctl(
                                        call_id,
                                        generation,
                                        VideoControl::Enable,
                                    );
                                }
                            }
                            _ => {
                                transition_current = false;
                            }
                        }

                        transition_current &= registry.is_current(call_id, generation);
                        if transition_current {
                            if let Some(orientation) = orientation {
                                registry.send_video_ctl(
                                    call_id,
                                    generation,
                                    VideoControl::SetOrientation(*orientation),
                                );
                            }
                            let event_delivered = event_permit.as_ref().is_some_and(|permit| {
                                permit.send(CallEvent::VideoStateChanged {
                                    state: *state,
                                    orientation: *orientation,
                                    upgrade_token,
                                })
                            });
                            if !event_delivered {
                                warn!("call: video state event receiver closed after typed ack");
                            }
                        }
                        dispatch_call = transition_current;
                        // Keep the transition serialized through every committed side effect.
                        drop(event_permit);
                    }
                    if dispatch_call {
                        client.core.event_bus.dispatch(Event::IncomingCall(call));
                    }
                    #[cfg(feature = "voip-runtime")]
                    replay_initial_group_controls(&client, buffered_initial_group_controls).await;
                }
            }
            Ok(None) => {
                if let Some(ack) = typed_control_ack
                    && let Err(error) = send_node_boxed(&client, ack).await
                {
                    warn!("call: failed to acknowledge unrecognized typed control: {error}");
                }
                debug!("call: ignoring unrecognized action (forward-compat)");
            }
            Err(e) => {
                if let Some(ack) = typed_control_ack
                    && let Err(error) = send_node_boxed(&client, ack).await
                {
                    warn!("call: failed to acknowledge malformed typed control: {error}");
                }
                warn!("call: failed to parse stanza: {e}");
            }
        }
        true
    }
}

/// Erase the rare typed-ACK send future so all three handler outcomes share one poll/drop path.
fn send_node_boxed(
    client: &Client,
    node: wacore_binary::Node,
) -> wacore::runtime::BoxFuture<'_, Result<(), crate::client::ClientError>> {
    Box::pin(client.send_node(node))
}

#[cfg(feature = "voip-runtime")]
async fn apply_current_group_control(client: &Client, call: &IncomingCall) -> bool {
    let registry = client.call_registry();
    let call_id = call.action.call_id();
    let Some((generation, lock)) = registry.current_group_transition(call_id) else {
        warn!(
            "call: rejected {} without a matching group offer for {call_id}",
            call.action.wire_tag()
        );
        return false;
    };
    let _guard = lock.lock().await;
    if !registry.is_current(call_id, generation) {
        return false;
    }
    apply_group_control(client, call, generation).await
}

#[cfg(feature = "voip-runtime")]
/// Announce a rotation the app set while the call was still ringing.
///
/// A video-from-start offer carries `device_orientation="0"`, and its caller
/// sends no further `<video>` of its own, so a rotation set between `start()`
/// and the answer would never reach the peer: while ringing there is no call
/// entry on their side to apply one against. Upright needs no stanza — that is
/// what the offer already said.
#[cfg(feature = "voip-runtime")]
async fn announce_orientation_on_accept(
    client: &Arc<Client>,
    call_id: &str,
    generation: u64,
    to: &Jid,
) {
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

#[cfg(feature = "voip-runtime")]
async fn apply_current_decrypted_group_epoch(
    client: &Client,
    rekey: &GroupCallEncRekey,
    sender: &Jid,
    raw_epoch: &[u8],
) {
    let registry = client.call_registry();
    let Some((generation, lock)) = registry.current_group_transition(&rekey.call_id) else {
        warn!(
            "call: rejected enc_rekey without a matching group offer for {}",
            rekey.call_id
        );
        return;
    };
    let _guard = lock.lock().await;
    if !registry.is_current(&rekey.call_id, generation) {
        return;
    }
    if !registry.group_sender_authorized_if_current(
        &rekey.call_id,
        generation,
        &rekey.call_creator,
        sender,
    ) {
        warn!(
            "call: rejected group epoch from unauthorized sender for {}",
            rekey.call_id
        );
        return;
    }
    commit_decrypted_group_epoch(client, rekey, generation, raw_epoch);
}

#[cfg(feature = "voip-runtime")]
async fn apply_current_waiting_room_update(
    client: &Client,
    room: &wacore::types::group_call::WaitingRoom,
    call: &IncomingCall,
) -> bool {
    let registry = client.call_registry();
    let Some((generation, lock)) = registry.current_group_transition(&room.call_id) else {
        warn!(
            "call: rejected waiting-room snapshot without a matching link join for {}",
            room.call_id
        );
        return false;
    };
    let _guard = lock.lock().await;
    apply_waiting_room_update(client, room, call, generation)
}

#[cfg(feature = "voip-runtime")]
fn apply_waiting_room_update(
    client: &Client,
    room: &wacore::types::group_call::WaitingRoom,
    call: &IncomingCall,
    generation: u64,
) -> bool {
    let registry = client.call_registry();
    if !registry.group_creator_authorized_if_current(
        &room.call_id,
        generation,
        &room.call_creator,
        &routed_call_sender(call),
    ) {
        warn!(
            "call: rejected waiting-room snapshot from unauthorized sender for {}",
            room.call_id
        );
        return false;
    }
    match registry.apply_waiting_room_if_current(room.clone(), generation) {
        GroupStateApply::Applied => {
            registry.send_call_event_if_current(
                &room.call_id,
                generation,
                CallEvent::WaitingRoomUpdated(Box::new(room.clone())),
            );
            true
        }
        GroupStateApply::Stale | GroupStateApply::UnknownCall => false,
        GroupStateApply::IdentityMismatch | GroupStateApply::InvalidSnapshot => {
            warn!(
                "call: rejected invalid waiting-room snapshot for {}",
                room.call_id
            );
            false
        }
        _ => false,
    }
}

#[cfg(feature = "voip-runtime")]
async fn replay_initial_group_controls(client: &Client, controls: Vec<IncomingCall>) {
    for call in controls {
        if apply_current_group_control(client, &call).await {
            client.core.event_bus.dispatch(Event::IncomingCall(call));
        }
    }
}

#[cfg(feature = "voip-runtime")]
async fn apply_group_control(client: &Client, call: &IncomingCall, generation: u64) -> bool {
    let registry = client.call_registry();
    let sender = routed_call_sender(call);
    match &call.action {
        CallAction::GroupUpdate { update } => {
            if !registry.group_creator_authorized_if_current(
                &update.call_id,
                generation,
                &update.call_creator,
                &sender,
            ) {
                warn!(
                    "call: rejected group snapshot from non-creator sender for {}",
                    update.call_id
                );
                return false;
            }
            match registry.apply_group_update_if_current(update.as_ref().clone(), generation) {
                GroupStateApply::Applied => {
                    if !registry.send_group_update_if_current(
                        &update.call_id,
                        generation,
                        update.as_ref().clone(),
                    ) {
                        warn!(
                            "call: terminating {} after its committed group snapshot could not reach media",
                            update.call_id
                        );
                        registry.remove_if_current(&update.call_id, generation);
                        return false;
                    }
                    if update.rekey_requested {
                        match crate::voip::facade::fanout_group_epoch(client, update).await {
                            Ok(fanout) => {
                                let fanout_generation = fanout.generation();
                                if let Err(error) = fanout.commit(|epoch| {
                                    fanout_generation
                                        .is_some_and(|generation| {
                                            client.call_registry().send_group_epoch_if_current(
                                                &update.call_id,
                                                generation,
                                                update.transaction_id,
                                                epoch.to_vec(),
                                            )
                                        })
                                        .then_some(())
                                        .ok_or(CallError::Media(
                                            "local group epoch consumer closed",
                                        ))
                                }) {
                                    warn!(
                                        "call: failed to commit requested group epoch for {}: {error}",
                                        update.call_id
                                    );
                                    client.call_registry().send_call_event_if_current(
                                        &update.call_id,
                                        generation,
                                        CallEvent::GroupRekeyFailed,
                                    );
                                }
                            }
                            Err(error) => {
                                warn!(
                                    "call: failed to distribute requested group epoch for {}: {error}",
                                    update.call_id
                                );
                                client.call_registry().send_call_event_if_current(
                                    &update.call_id,
                                    generation,
                                    CallEvent::GroupRekeyFailed,
                                );
                            }
                        }
                    }
                    if let Some(committed) = client
                        .call_registry()
                        .group_state_if_current(&update.call_id, generation)
                        .and_then(|group| group.snapshot().cloned())
                    {
                        client.call_registry().send_call_event_if_current(
                            &update.call_id,
                            generation,
                            CallEvent::GroupUpdated(Box::new(committed)),
                        );
                    }
                    true
                }
                GroupStateApply::Stale | GroupStateApply::UnknownCall => false,
                GroupStateApply::IdentityMismatch | GroupStateApply::InvalidSnapshot => {
                    warn!(
                        "call: rejected invalid group snapshot for {}",
                        update.call_id
                    );
                    false
                }
                _ => false,
            }
        }
        CallAction::EncRekey { rekey } => {
            if !registry.group_sender_authorized_if_current(
                &rekey.call_id,
                generation,
                &rekey.call_creator,
                &sender,
            ) {
                warn!(
                    "call: rejected group epoch from unauthorized sender for {}",
                    rekey.call_id
                );
                return false;
            }
            match decrypt_group_epoch(client, rekey, &sender).await {
                Ok(raw_epoch) => {
                    commit_decrypted_group_epoch(client, rekey, generation, &raw_epoch)
                }
                Err(error) => {
                    warn!(
                        "call: rejected encrypted group epoch for {}: {error}",
                        rekey.call_id
                    );
                }
            }
            false
        }
        _ => false,
    }
}

#[cfg(feature = "voip-runtime")]
fn commit_decrypted_group_epoch(
    client: &Client,
    rekey: &GroupCallEncRekey,
    generation: u64,
    raw_epoch: &[u8],
) {
    let registry = client.call_registry();
    if !registry.send_group_epoch_if_current(
        &rekey.call_id,
        generation,
        rekey.transaction_id,
        raw_epoch.to_vec(),
    ) {
        debug!(
            "call: group epoch for {} has no active media consumer",
            rekey.call_id
        );
    }
}

#[cfg(feature = "voip-runtime")]
async fn decrypt_group_epoch(
    client: &Client,
    rekey: &GroupCallEncRekey,
    sender: &Jid,
) -> anyhow::Result<Zeroizing<Vec<u8>>> {
    let enc_type = EncType::from_wire(&rekey.encryption_type)
        .ok_or_else(|| anyhow::anyhow!("unsupported Signal envelope type"))?;
    let plaintext = Zeroizing::new(
        client
            .signal()
            .decrypt_message(sender, enc_type, &rekey.ciphertext)
            .await
            .map_err(|error| anyhow::anyhow!("Signal decrypt failed: {error}"))?,
    );
    let decoded = MessageUtils::unpad_message_ref(
        &plaintext,
        u8::try_from(rekey.encryption_version)
            .map_err(|_| anyhow::anyhow!("unsupported padding version"))?,
    )
    .map_err(|error| anyhow::anyhow!("message unpad failed: {error}"))
    .and_then(|unpadded| {
        waproto::codec::message_decode(unpadded)
            .map_err(|error| anyhow::anyhow!("message decode failed: {error}"))
    })?;
    let raw_epoch = Zeroizing::new(
        decoded
            .call
            .into_option()
            .and_then(|call| call.call_key)
            .ok_or_else(|| anyhow::anyhow!("message has no call key"))?,
    );
    validate_group_epoch_key(&raw_epoch)?;
    Ok(raw_epoch)
}

#[cfg(feature = "voip-runtime")]
fn validate_group_epoch_key(raw_epoch: &[u8]) -> anyhow::Result<()> {
    if raw_epoch.len() != 32 {
        anyhow::bail!("call key must contain exactly 32 bytes");
    }
    Ok(())
}

#[cfg(feature = "voip-runtime")]
fn routed_call_sender(call: &IncomingCall) -> Jid {
    call.participant.as_ref().unwrap_or(&call.from).clone()
}

#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.call_offer_ack", level = "debug", skip_all, fields(peer = %call.from.observe()), err(Debug)))]
async fn send_offer_ack_receipt(client: &Client, call: &IncomingCall) -> anyhow::Result<()> {
    let own_from = match call.from.server {
        Server::Lid => client.lid(),
        _ => client.pn(),
    };

    let Some(receipt) = build_offer_ack_receipt(call, own_from.as_ref()) else {
        return Ok(());
    };

    client.send_node(receipt).await.map_err(anyhow::Error::from)
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
/// callee's other devices go on ringing and may still answer. Any other reason - including none -
/// is the callee's own decision and ends the call.
#[cfg(feature = "voip-runtime")]
fn reject_is_device_busy(action: &CallAction) -> bool {
    matches!(
        action,
        CallAction::Reject { reason, .. } if reason.as_deref() == Some(REJECT_REASON_BUSY)
    )
}

#[cfg(feature = "voip-runtime")]
async fn dismiss_outgoing_siblings(client: &Client, call: &IncomingCall) {
    let reason = match &call.action {
        CallAction::Accept { .. } => TERMINATE_REASON_ACCEPTED_ELSEWHERE,
        // A `busy` device has not decided anything for the callee, so its siblings must keep
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

#[cfg(feature = "voip-runtime")]
async fn dismiss_incompatible_group_invitee(client: &Client, call: &IncomingCall) {
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
#[cfg(feature = "voip-runtime")]
fn same_device(a: &Jid, b: &Jid) -> bool {
    a.user == b.user && a.server == b.server && a.device == b.device
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::node_to_owned_ref;
    use std::sync::Arc;
    use wacore::types::events::{ChannelEventHandler, Event};
    #[cfg(feature = "voip-runtime")]
    use wacore::types::group_call::{GroupCallParticipant, GroupCallUpdate};
    #[cfg(feature = "voip-runtime")]
    use wacore::voip::video_control_channel;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::{Jid, Server};

    fn fake_caller_lid() -> Jid {
        Jid::new("111111111111111", Server::Lid)
    }

    fn offer_stanza() -> wacore_binary::Node {
        NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "STANZA-ID-0001")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("offer")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CALL-ID-0001")
                .children([NodeBuilder::new("audio")
                    .attr("enc", "opus")
                    .attr("rate", "16000")
                    .build()])
                .build()])
            .build()
    }

    #[cfg(feature = "voip-runtime")]
    #[test]
    fn routed_call_sender_prefers_participant_metadata() {
        let wrapper = Jid::new("GROUP-CALL", Server::Call);
        let participant = Jid::new("222222222222222", Server::Lid).with_device(2);
        let mut call = IncomingCall::new_for_test(
            wrapper.clone(),
            "STANZA-GROUP-CONTROL".to_string(),
            wacore::time::from_secs(1_766_847_151_i64).expect("valid ts"),
            CallAction::RaiseHand {
                call_id: "GROUP-CALL".to_string(),
                call_creator: fake_caller_lid(),
                raised: true,
            },
        );
        assert_eq!(routed_call_sender(&call), wrapper);
        call.participant = Some(participant.clone());
        assert_eq!(routed_call_sender(&call), participant);
    }

    #[cfg(feature = "voip-runtime")]
    #[test]
    fn group_epoch_keys_require_the_exact_protocol_length() {
        assert!(validate_group_epoch_key(&[0; 32]).is_ok());
        assert!(validate_group_epoch_key(&[0; 31]).is_err());
        assert!(validate_group_epoch_key(&[0; 33]).is_err());
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn decrypted_epoch_is_committed_when_registration_overtakes_pending_buffering() {
        use wacore::types::group_call::GroupCallEncRekey;
        use wacore::voip::CallSession;

        let client = make_client().await;
        let creator = fake_caller_lid();
        let sender = creator.clone().with_device(1);
        let call_id = "CALL-LINK-EPOCH-RACE";
        let generation = client
            .call_registry()
            .insert_call_link_checked(CallSession::new_outgoing(
                call_id,
                Jid::new(call_id, Server::Call),
                creator.clone(),
            ))
            .expect("registration won while Signal decryption was in flight");
        let rekey = GroupCallEncRekey::builder()
            .call_id(call_id.to_string())
            .call_creator(creator)
            .transaction_id(7)
            .key_generation(2)
            .encryption_type("msg".to_string())
            .encryption_version(2)
            .ciphertext(vec![0xff])
            .build();
        let raw_epoch = [7; 32];

        assert!(
            !client
                .buffer_pending_call_link_epoch(
                    call_id,
                    &rekey.call_creator,
                    &sender,
                    rekey.transaction_id,
                    &raw_epoch,
                )
                .suppresses_dispatch(),
            "the published generation must win before the pending buffer can retain the key"
        );
        apply_current_decrypted_group_epoch(&client, &rekey, &sender, &raw_epoch).await;

        assert_eq!(
            client
                .call_registry()
                .pending_group_epoch_transaction_if_current(call_id, generation),
            Some(7),
            "the already decrypted key must reach the generation without reprocessing ciphertext"
        );
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn waiting_room_update_rechecks_a_generation_published_during_registration() {
        let client = make_sending_client().await;
        let creator = fake_caller_lid();
        let call_id = "CALL-LINK-RACE";
        let registry = client.call_registry();
        let generation = registry
            .insert_call_link_checked(wacore::voip::CallSession::new_outgoing(
                call_id,
                Jid::new(call_id, Server::Call),
                creator.clone(),
            ))
            .expect("valid call-link session");
        let room = wacore::types::group_call::WaitingRoom::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .link_token("TEST-CALL-LINK".to_string())
            .media(wacore::types::group_call::CallLinkMedia::Audio)
            .enabled(true)
            .is_admin(true)
            .transaction_id(2)
            .users(Vec::new())
            .build();
        let mut call = IncomingCall::new_for_test(
            Jid::new(call_id, Server::Call),
            "WAITING-ROOM-RACE".to_string(),
            wacore::time::from_secs(1_766_847_151_i64).expect("valid ts"),
            CallAction::WaitingRoomUpdate {
                room: Box::new(room.clone()),
            },
        );
        call.participant = Some(creator.with_device(1));

        assert!(apply_current_waiting_room_update(&client, &room, &call).await);
        assert!(
            registry
                .group_state_if_current(call_id, generation)
                .and_then(|state| state.waiting_room().cloned())
                .is_some_and(|current| { current.transaction_id == Some(2) && current.is_admin }),
            "the update must apply to the generation that appeared after the initial lookup"
        );
        registry.remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn typed_control_after_unknown_child_suppresses_the_generic_ack() {
        let client = make_sending_client().await;
        let creator = fake_caller_lid();
        let stanza = NodeBuilder::new("call")
            .attr("from", creator.clone())
            .attr("id", "FORWARD-COMPAT-CONTROL")
            .attr("t", "1766847151")
            .children([
                NodeBuilder::new("future_call_action").build(),
                NodeBuilder::new("user_action")
                    .attr("call-id", "GROUP-CALL")
                    .attr("call-creator", creator)
                    .attr("action", "raise_hand")
                    .children([NodeBuilder::new("raise_hand")
                        .attr("raise-hand-state", "1")
                        .build()])
                    .build(),
            ])
            .build();
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("ack"));
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client, node_to_owned_ref(&stanza), &mut cancelled)
                .await
        );
        assert!(
            cancelled,
            "a known typed control after a future child must suppress the generic ACK"
        );
        let ack = waiter.await.expect("typed control ACK");
        assert_eq!(
            ack.as_node_ref().attrs().optional_string("type").as_deref(),
            Some("user_action")
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn routed_group_control_rejects_sender_outside_authoritative_roster() {
        use wacore::types::group_call::{GroupCallParticipant, GroupCallUpdate};
        use wacore::voip::{CallSession, GroupStateApply};

        let client = make_sending_client().await;
        let creator = fake_caller_lid();
        let registry = client.call_registry();
        let mut session = CallSession::new_outgoing(
            "GROUP-CALL",
            Jid::new("GROUP-CALL", Server::Call),
            creator.clone(),
        );
        session.is_video = true;
        let generation = registry.insert(session);
        let update = GroupCallUpdate::builder()
            .call_id("GROUP-CALL".to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("video".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![GroupCallParticipant::new(
                creator.clone(),
                vec![GroupCallDevice::new(creator.clone().with_device(1))],
            )])
            .build();
        assert_eq!(
            registry.apply_group_update(update),
            GroupStateApply::Applied
        );
        let (handler, global_rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();
        let outsider = Jid::new("999999999999999", Server::Lid).with_device(9);
        let stanza = NodeBuilder::new("call")
            .attr("from", Jid::new("GROUP-CALL", Server::Call))
            .attr("participant", outsider.clone())
            .attr("id", "UNAUTHORIZED-CONTROL")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("user_action")
                .attr("call-id", "GROUP-CALL")
                .attr("call-creator", creator.clone())
                .attr("action", "raise_hand")
                .children([NodeBuilder::new("raise_hand")
                    .attr("raise-hand-state", "1")
                    .build()])
                .build()])
            .build();
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&stanza), &mut cancelled)
                .await
        );
        assert!(
            cancelled,
            "the typed control must still receive its typed ACK"
        );
        assert!(
            registry
                .group_state("GROUP-CALL")
                .expect("group state")
                .raised_hands()
                .is_empty()
        );
        assert!(
            global_rx.try_recv().is_err(),
            "an unauthorized control must not reach the public event bus"
        );

        let video = NodeBuilder::new("call")
            .attr("from", Jid::new("GROUP-CALL", Server::Call))
            .attr("participant", outsider)
            .attr("id", "UNAUTHORIZED-VIDEO")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("video")
                .attr("call-id", "GROUP-CALL")
                .attr("call-creator", creator)
                .attr("state", "0")
                .build()])
            .build();
        cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&video), &mut cancelled)
                .await
        );
        assert_eq!(
            registry.video_states("GROUP-CALL", generation),
            Some((VideoState::Enabled, VideoState::Enabled)),
            "an outsider must not disable the group video plane"
        );
        assert!(
            global_rx.try_recv().is_err(),
            "unauthorized video state must not reach the public event bus"
        );
        registry.remove_if_current("GROUP-CALL", generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn remote_screen_share_start_is_rejected_for_an_audio_only_group() {
        use wacore::types::group_call::{GroupCallParticipant, GroupCallUpdate};
        use wacore::voip::{CallSession, GroupStateApply};

        let client = make_sending_client().await;
        let creator = fake_caller_lid();
        let participant = Jid::new("222222222222222", Server::Lid);
        let participant_device = participant.clone().with_device(2);
        let call_id = "AUDIO-GROUP-CALL";
        let registry = client.call_registry();
        let generation = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        ));
        let mut creator_roster = GroupCallParticipant::new(
            creator.clone(),
            vec![GroupCallDevice::new(creator.clone().with_device(1))],
        );
        creator_roster.state = Some("connected".to_string());
        let mut participant_roster = GroupCallParticipant::new(
            participant,
            vec![GroupCallDevice::new(participant_device.clone())],
        );
        participant_roster.state = Some("connected".to_string());
        assert_eq!(
            registry.apply_group_update(
                GroupCallUpdate::builder()
                    .call_id(call_id.to_string())
                    .call_creator(creator.clone())
                    .transaction_id(1)
                    .media("audio".to_string())
                    .connected_limit(32)
                    .joinable(true)
                    .av_upgradable(true)
                    .rekey_requested(false)
                    .participants(vec![creator_roster, participant_roster])
                    .build()
            ),
            GroupStateApply::Applied
        );
        let (handler, global_rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();
        let stanza = NodeBuilder::new("call")
            .attr("from", Jid::new(call_id, Server::Call))
            .attr("participant", participant_device)
            .attr("id", "AUDIO-GROUP-SCREEN-SHARE")
            .attr("t", "1766847151")
            .children([NodeBuilder::new(CallActionTag::ScreenShare.as_str())
                .attr("call-id", call_id)
                .attr("call-creator", creator)
                .attr("screenshare_state", "1")
                .attr("version", "2")
                .attr("screen_share_id", "7")
                .build()])
            .build();
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client, node_to_owned_ref(&stanza), &mut cancelled)
                .await
        );
        assert!(cancelled, "the typed control must still receive its ACK");
        assert!(
            registry
                .group_state_if_current(call_id, generation)
                .expect("group state")
                .screen_shares()
                .is_empty(),
            "an audio-only roster must not retain a remote screen-share start"
        );
        assert!(
            global_rx.try_recv().is_err(),
            "the rejected screen-share start must not reach the public event bus"
        );
        registry.remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn routed_group_video_is_participant_scoped_and_canonicalizes_a_pn_alias() {
        use wacore::types::group_call::{GroupCallParticipant, GroupCallUpdate};
        use wacore::voip::{CallEvent, CallSession, GroupStateApply, VideoControl};

        let client = make_sending_client().await;
        let creator = fake_caller_lid();
        let participant = Jid::new("222222222222222", Server::Lid);
        let participant_pn = Jid::new("12025550112", Server::Pn);
        let registry = client.call_registry();
        let mut session = CallSession::new_outgoing(
            "GROUP-CALL",
            Jid::new("GROUP-CALL", Server::Call),
            creator.clone(),
        );
        session.is_video = true;
        let generation = registry.insert(session);
        let mut peer = GroupCallParticipant::new(
            participant.clone(),
            vec![
                GroupCallDevice::new(participant.clone().with_device(2)),
                GroupCallDevice::new(participant.clone().with_device(3)),
            ],
        );
        peer.pn = Some(participant_pn.clone());
        peer.state = Some("connected".to_string());
        assert_eq!(
            registry.apply_group_update(
                GroupCallUpdate::builder()
                    .call_id("GROUP-CALL".to_string())
                    .call_creator(creator.clone())
                    .transaction_id(1)
                    .media("video".to_string())
                    .connected_limit(32)
                    .joinable(true)
                    .av_upgradable(true)
                    .rekey_requested(false)
                    .participants(vec![
                        GroupCallParticipant::new(
                            creator.clone(),
                            vec![GroupCallDevice::new(creator.clone().with_device(1))],
                        ),
                        peer,
                    ])
                    .build(),
            ),
            GroupStateApply::Applied
        );
        let (event_tx, event_rx) = async_channel::unbounded::<CallEvent>();
        let (control_tx, control_rx) = video_control_channel();
        registry.set_video_channels(
            "GROUP-CALL",
            generation,
            event_tx,
            control_tx,
            Box::new(|| {}),
        );
        let stanza = NodeBuilder::new("call")
            .attr("from", Jid::new("GROUP-CALL", Server::Call))
            .attr("participant", participant_pn)
            .attr("id", "PN-ORIENTATION")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("video")
                .attr("call-id", "GROUP-CALL")
                .attr("call-creator", creator)
                .attr("state", "0")
                .attr("device_orientation", "3")
                .build()])
            .build();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&stanza), &mut cancelled)
                .await
        );
        assert!(cancelled);
        assert_eq!(
            registry.video_states("GROUP-CALL", generation),
            Some((VideoState::Enabled, VideoState::Enabled)),
            "one participant's disabled state must not downgrade the whole group"
        );
        assert!(
            registry
                .snapshot("GROUP-CALL")
                .is_some_and(|session| session.is_video),
            "participant signaling must not tear down the local group video plane"
        );
        assert!(
            event_rx.try_recv().is_err(),
            "the 1:1 video event lacks participant identity and must stay unused for group peers"
        );
        let controls = std::iter::from_fn(|| control_rx.try_recv().ok()).collect::<Vec<_>>();
        assert!(
            controls.iter().any(|control| matches!(
                control,
                VideoControl::SetParticipantOrientation {
                    participant: canonical,
                    orientation: 3,
                } if canonical == &participant
            )),
            "video orientation must be keyed by the roster LID, not the routed PN alias"
        );
        assert!(
            !controls
                .iter()
                .any(|control| matches!(control, VideoControl::Disable)),
            "participant signaling cannot disable the call-wide video plane"
        );

        let routed_device = participant.with_device(3);
        let stanza = NodeBuilder::new("call")
            .attr("from", Jid::new("GROUP-CALL", Server::Call))
            .attr("participant", routed_device.clone())
            .attr("id", "DEVICE-ORIENTATION")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("video")
                .attr("call-id", "GROUP-CALL")
                .attr("call-creator", fake_caller_lid())
                .attr("state", "0")
                .attr("device_orientation", "1")
                .build()])
            .build();
        assert!(
            CallHandler
                .handle(client, node_to_owned_ref(&stanza), &mut cancelled)
                .await
        );
        let device_controls = std::iter::from_fn(|| control_rx.try_recv().ok()).collect::<Vec<_>>();
        assert!(
            device_controls.iter().any(|control| matches!(
                control,
                VideoControl::SetParticipantOrientation {
                    participant,
                    orientation: 1,
                } if participant == &routed_device
            )),
            "a routed device control must not overwrite its sibling's orientation: {device_controls:?}"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn group_video_reauthorizes_the_sender_after_the_typed_ack() {
        use wacore::types::group_call::{GroupCallParticipant, GroupCallUpdate};
        use wacore::voip::{CallEvent, CallSession, GroupStateApply, VideoControl};

        let (client, send_started, release_send) = make_blocking_sending_client().await;
        let (handler, global_rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();
        let creator = fake_caller_lid();
        let participant = Jid::new("222222222222222", Server::Lid).with_device(2);
        let registry = client.call_registry();
        let mut session = CallSession::new_outgoing(
            "GROUP-CALL",
            Jid::new("GROUP-CALL", Server::Call),
            creator.clone(),
        );
        session.is_video = true;
        let generation = registry.insert(session);
        let mut peer = GroupCallParticipant::new(
            participant.to_non_ad(),
            vec![GroupCallDevice::new(participant.clone())],
        );
        peer.state = Some("connected".to_string());
        let mut creator_member = GroupCallParticipant::new(
            creator.clone(),
            vec![GroupCallDevice::new(creator.clone().with_device(1))],
        );
        creator_member.state = Some("connected".to_string());
        assert_eq!(
            registry.apply_group_update(
                GroupCallUpdate::builder()
                    .call_id("GROUP-CALL".to_string())
                    .call_creator(creator.clone())
                    .transaction_id(1)
                    .media("video".to_string())
                    .connected_limit(32)
                    .joinable(true)
                    .av_upgradable(true)
                    .rekey_requested(false)
                    .participants(vec![creator_member.clone(), peer])
                    .build(),
            ),
            GroupStateApply::Applied
        );
        let (event_tx, _event_rx) = async_channel::unbounded::<CallEvent>();
        let (control_tx, control_rx) = video_control_channel();
        registry.set_video_channels(
            "GROUP-CALL",
            generation,
            event_tx,
            control_tx,
            Box::new(|| {}),
        );
        let stanza = NodeBuilder::new("call")
            .attr("from", Jid::new("GROUP-CALL", Server::Call))
            .attr("participant", participant)
            .attr("id", "REMOVED-DURING-VIDEO-ACK")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("video")
                .attr("call-id", "GROUP-CALL")
                .attr("call-creator", creator.clone())
                .attr("state", "0")
                .attr("device_orientation", "3")
                .build()])
            .build();

        let mut cancelled = false;
        let handled = {
            let handling = CallHandler.handle(client, node_to_owned_ref(&stanza), &mut cancelled);
            tokio::pin!(handling);
            tokio::select! {
                started = send_started.recv() => started.expect("typed video ack started"),
                _ = &mut handling => panic!("handler completed before the typed ack send"),
            }
            assert_eq!(
                registry.apply_group_update(
                    GroupCallUpdate::builder()
                        .call_id("GROUP-CALL".to_string())
                        .call_creator(creator)
                        .transaction_id(2)
                        .media("video".to_string())
                        .connected_limit(32)
                        .joinable(true)
                        .av_upgradable(true)
                        .rekey_requested(false)
                        .participants(vec![creator_member])
                        .build(),
                ),
                GroupStateApply::Applied
            );
            release_send
                .send(())
                .await
                .expect("release typed video ack");
            handling.await
        };

        assert!(handled);
        assert!(cancelled);
        assert!(
            std::iter::from_fn(|| control_rx.try_recv().ok())
                .all(|control| !matches!(control, VideoControl::SetParticipantOrientation { .. })),
            "a sender removed while the ACK is in flight cannot publish video controls"
        );
        assert!(
            global_rx.try_recv().is_err(),
            "a removed sender cannot reach the public event bus after the ACK"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn routed_group_snapshots_reject_non_creator_roster_writes_and_outsiders() {
        use wacore::types::group_call::{
            CallLinkMedia, GroupCallParticipant, GroupCallUpdate, WaitingRoom,
        };
        use wacore::voip::{CallSession, GroupStateApply};

        let client = make_sending_client().await;
        let creator = fake_caller_lid();
        let registry = client.call_registry();
        let generation = registry
            .insert_call_link_checked(CallSession::new_outgoing(
                "GROUP-CALL",
                Jid::new("GROUP-CALL", Server::Call),
                creator.clone(),
            ))
            .expect("valid call-link session");
        let participant = Jid::new("222222222222222", Server::Lid).with_device(2);
        let update = GroupCallUpdate::builder()
            .call_id("GROUP-CALL".to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![
                GroupCallParticipant::new(
                    creator.clone(),
                    vec![GroupCallDevice::new(creator.clone().with_device(1))],
                ),
                GroupCallParticipant::new(
                    participant.to_non_ad(),
                    vec![GroupCallDevice::new(participant.clone())],
                ),
            ])
            .build();
        assert_eq!(
            registry.apply_group_update(update),
            GroupStateApply::Applied
        );
        assert_eq!(
            registry.apply_waiting_room(
                WaitingRoom::builder()
                    .call_id("GROUP-CALL".to_string())
                    .call_creator(creator.clone())
                    .link_token("TEST-CALL-LINK".to_string())
                    .media(CallLinkMedia::Audio)
                    .enabled(true)
                    .is_admin(true)
                    .transaction_id(1)
                    .users(Vec::new())
                    .build(),
            ),
            GroupStateApply::Applied
        );

        let (handler, global_rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();
        let outsider = Jid::new("999999999999999", Server::Lid).with_device(9);
        let group_update = NodeBuilder::new("call")
            .attr("from", Jid::new("GROUP-CALL", Server::Call))
            .attr("participant", participant.clone())
            .attr("id", "UNAUTHORIZED-GROUP-SNAPSHOT")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("group_update")
                .attr("call-id", "GROUP-CALL")
                .attr("call-creator", creator.clone())
                .children([NodeBuilder::new("group_info")
                    .attr("transaction-id", "2")
                    .attr("media", "audio")
                    .attr("connected-limit", "32")
                    .attr("joinable", "1")
                    .children([NodeBuilder::new("user")
                        .attr("jid", outsider.to_non_ad())
                        .attr("state", "connected")
                        .children([NodeBuilder::new("device")
                            .attr("jid", outsider.clone())
                            .attr("pid", "9")
                            .build()])
                        .build()])
                    .build()])
                .build()])
            .build();
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&group_update),
                    &mut cancelled,
                )
                .await
        );
        assert_eq!(
            registry
                .group_state("GROUP-CALL")
                .and_then(|state| state.snapshot().map(|snapshot| snapshot.transaction_id)),
            Some(1),
            "a connected participant must not replace the creator-authoritative roster"
        );

        let waiting_room = NodeBuilder::new("call")
            .attr("from", Jid::new("GROUP-CALL", Server::Call))
            .attr("participant", participant)
            .attr("id", "UNAUTHORIZED-WAITING-SNAPSHOT")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("waiting_room_update")
                .attr("call-id", "GROUP-CALL")
                .attr("call-creator", creator.clone())
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "GROUP-CALL")
                    .attr("call-creator", creator)
                    .attr("link-token", "TEST-CALL-LINK")
                    .attr("media", "audio")
                    .attr("enabled", "1")
                    .attr("is_admin", "1")
                    .attr("transaction-id", "2")
                    .build()])
                .build()])
            .build();
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&waiting_room),
                    &mut cancelled,
                )
                .await
        );
        assert_eq!(
            registry
                .group_state("GROUP-CALL")
                .and_then(|state| state.waiting_room().and_then(|room| room.transaction_id)),
            Some(1),
            "a connected non-creator must not replace the waiting-room state"
        );
        assert!(
            global_rx.try_recv().is_err(),
            "unauthorized snapshots must not reach the public event bus"
        );
        registry.remove_if_current("GROUP-CALL", generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn queued_group_transition_cannot_commit_to_a_replacement_generation() {
        use wacore::voip::CallSession;

        let client = make_client().await;
        let call_id = "GROUP-LOCK-GENERATION";
        let creator = fake_caller_lid();
        let registry = client.call_registry();
        let stale_generation = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        ));
        let stale_lock = registry
            .group_transition_lock(call_id, stale_generation)
            .expect("stale generation lock");
        let stale_guard = stale_lock.lock().await;
        let update = NodeBuilder::new("call")
            .attr("from", creator.clone())
            .attr("id", "STALE-GROUP-UPDATE")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("group_update")
                .attr("call-id", call_id)
                .attr("call-creator", creator.clone())
                .children([NodeBuilder::new("group_info")
                    .attr("transaction-id", "2")
                    .attr("media", "audio")
                    .attr("connected-limit", "32")
                    .children([NodeBuilder::new("user")
                        .attr("jid", creator.clone())
                        .attr("state", "connected")
                        .children([NodeBuilder::new("device")
                            .attr("jid", creator.clone().with_device(1))
                            .attr("pid", "1")
                            .build()])
                        .build()])
                    .build()])
                .build()])
            .build();
        let handler_client = client.clone();
        let handler = tokio::spawn(async move {
            let mut cancelled = false;
            CallHandler
                .handle(handler_client, node_to_owned_ref(&update), &mut cancelled)
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(2), async {
            while Arc::strong_count(&stale_lock) < 3 {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("handler must retain the stale transition lock");

        let replacement = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator,
        ));
        assert_ne!(replacement, stale_generation);
        drop(stale_guard);
        assert!(handler.await.expect("handler task"));
        assert!(
            registry
                .group_state_if_current(call_id, replacement)
                .is_none(),
            "a transition queued on the removed lock must not mutate the replacement"
        );
        registry.remove_if_current(call_id, replacement);
    }

    #[cfg(feature = "voip-runtime")]
    fn active_group_offer_stanza() -> wacore_binary::Node {
        active_group_offer_stanza_with_limit("32")
    }

    #[cfg(feature = "voip-runtime")]
    fn active_group_offer_stanza_with_limit(connected_limit: &str) -> wacore_binary::Node {
        let caller = fake_caller_lid();
        NodeBuilder::new("call")
            .attr("from", &caller)
            .attr("id", "STANZA-GROUP")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("offer")
                .attr("call-creator", &caller)
                .attr("call-id", "GROUP-CALL")
                .attr("joinable", "1")
                .children([
                    NodeBuilder::new("audio")
                        .attr("enc", "opus")
                        .attr("rate", "16000")
                        .build(),
                    NodeBuilder::new("net").attr("medium", "2").build(),
                    NodeBuilder::new("group_info")
                        .attr("transaction-id", "7")
                        .attr("connected-limit", connected_limit)
                        .attr("media", "audio")
                        .attr("joinable", "1")
                        .attr("rekey", "0")
                        .children([NodeBuilder::new("user")
                            .attr("jid", &caller)
                            .attr("state", "connected")
                            .children([NodeBuilder::new("device")
                                .attr("jid", caller.with_device(1))
                                .attr("pid", "4")
                                .build()])
                            .build()])
                        .build(),
                ])
                .build()])
            .build()
    }

    #[cfg(feature = "voip-runtime")]
    fn active_group_update_stanza(transaction_id: u32) -> wacore_binary::Node {
        let caller = fake_caller_lid();
        NodeBuilder::new("call")
            .attr("from", &caller)
            .attr("id", format!("STANZA-GROUP-UPDATE-{transaction_id}"))
            .attr("t", "1766847151")
            .children([NodeBuilder::new("group_update")
                .attr("call-id", "GROUP-CALL")
                .attr("call-creator", &caller)
                .children([NodeBuilder::new("group_info")
                    .attr("transaction-id", transaction_id.to_string())
                    .attr("connected-limit", "32")
                    .attr("media", "audio")
                    .attr("joinable", "1")
                    .attr("rekey", "0")
                    .children([NodeBuilder::new("user")
                        .attr("jid", &caller)
                        .attr("state", "connected")
                        .children([NodeBuilder::new("device")
                            .attr("jid", caller.with_device(1))
                            .attr("pid", "4")
                            .build()])
                        .build()])
                    .build()])
                .build()])
            .build()
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn rekey_group_update_is_acknowledged_before_epoch_fanout() {
        use wacore::types::group_call::GroupCallUpdate;
        use wacore::voip::{CallSession, GroupStateApply};

        let (client, send_started, release_send) = make_blocking_sending_client().await;
        client.set_connected_for_test(true);
        let creator = fake_caller_lid();
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                creator.clone(),
            )))
            .await;
        let call_id = "GROUP-REKEY-ACK";
        let registry = client.call_registry();
        let generation = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        ));
        assert_eq!(
            registry.apply_group_update(
                GroupCallUpdate::builder()
                    .call_id(call_id.to_string())
                    .call_creator(creator.clone())
                    .transaction_id(1)
                    .media("audio".to_string())
                    .connected_limit(32)
                    .joinable(true)
                    .av_upgradable(true)
                    .rekey_requested(false)
                    .participants(Vec::new())
                    .build(),
            ),
            GroupStateApply::Applied
        );
        let stanza = NodeBuilder::new("call")
            .attr("from", creator.clone())
            .attr("id", "GROUP-REKEY-ACK-STANZA")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("group_update")
                .attr("call-id", call_id)
                .attr("call-creator", creator)
                .children([NodeBuilder::new("group_info")
                    .attr("transaction-id", "2")
                    .attr("connected-limit", "32")
                    .attr("media", "audio")
                    .attr("joinable", "1")
                    .attr("rekey", "1")
                    .build()])
                .build()])
            .build();
        let node = node_to_owned_ref(&stanza);
        let mut cancelled = false;
        let handled = {
            let handling = CallHandler.handle(client.clone(), node, &mut cancelled);
            tokio::pin!(handling);
            tokio::select! {
                started = send_started.recv() => started.expect("generic transport ACK send started"),
                _ = &mut handling => panic!("handler completed before the rekey ACK send"),
            }
            assert_eq!(
                registry
                    .group_state_if_current(call_id, generation)
                    .and_then(|state| state.snapshot().map(|snapshot| snapshot.transaction_id)),
                Some(1),
                "the transport ACK must start before the roster commit and epoch fanout"
            );
            release_send.send(()).await.expect("release ACK send");
            handling.await
        };

        assert!(handled);
        assert!(
            cancelled,
            "the router must not send a duplicate generic ACK"
        );
        assert_eq!(
            registry
                .group_state_if_current(call_id, generation)
                .and_then(|state| state.snapshot().map(|snapshot| snapshot.transaction_id)),
            Some(2)
        );
        assert_eq!(
            registry.pending_group_epoch_transaction_if_current(call_id, generation),
            Some(2),
            "the requested epoch must still commit after the early transport receipt"
        );
        registry.remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn initial_group_control_waits_in_a_bounded_buffer_for_its_offer() {
        let client = make_sending_client().await;
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&active_group_update_stanza(8)),
                    &mut cancelled,
                )
                .await
        );
        assert!(
            client.call_registry().generation_of("GROUP-CALL").is_none(),
            "a control cannot fabricate a call before its offer"
        );
        assert_eq!(
            client.call_registry().memory_stats().entries,
            1,
            "the control must be retained as bounded registry state, not a waiting task"
        );

        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&active_group_offer_stanza()),
                    &mut cancelled,
                )
                .await
        );
        assert_eq!(
            client
                .call_registry()
                .group_state("GROUP-CALL")
                .and_then(|state| state.snapshot().map(|snapshot| snapshot.transaction_id)),
            Some(8),
            "the control that overtook registration must apply after the matching offer"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn group_updated_event_carries_the_committed_inherited_snapshot() {
        let client = make_sending_client().await;
        let registry = client.call_registry();
        let call_id = "COMMITTED-GROUP-EVENT";
        let creator = fake_caller_lid();
        let group_jid = Jid::new("120363000000001", Server::Group);
        let mut creator_participant = GroupCallParticipant::new(
            creator.to_non_ad(),
            vec![GroupCallDevice::new(creator.clone().with_device(1))],
        );
        creator_participant.state = Some("connected".to_string());
        let relay = wacore::types::group_call::GroupCallRelay::builder()
            .transaction_id(1)
            .self_pid(1)
            .uuid("relay".to_string())
            .participant_uuid("participant".to_string())
            .attribute_padding(false)
            .key(b"relay-key".to_vec())
            .tokens(vec![vec![1]])
            .auth_tokens(vec![vec![2]])
            .endpoints(vec![
                wacore::types::group_call::GroupCallRelayEndpoint::builder()
                    .relay_id(1)
                    .token_id(0)
                    .auth_token_id(0)
                    .relay_name("relay-1".to_string())
                    .is_fna(false)
                    .ipv4("203.0.113.7".to_string())
                    .port(3480)
                    .build(),
            ])
            .build();
        let initial = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .group_jid(group_jid.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![creator_participant.clone()])
            .relay(relay.clone())
            .build();
        let mut session =
            wacore::voip::CallSession::new_outgoing(call_id, creator.clone(), creator.clone());
        session.group = Some(initial);
        let generation = registry
            .insert_group_checked(session)
            .expect("group session");
        let (control_tx, _control_rx) = async_channel::bounded(1);
        assert!(registry.set_group_control_sender(call_id, generation, Some(4), control_tx));
        let (event_tx, event_rx) = async_channel::unbounded();
        let (video_tx, _video_rx) = video_control_channel();
        registry.set_video_channels(call_id, generation, event_tx, video_tx, Box::new(|| {}));

        let update = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(2)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![creator_participant])
            .build();
        let mut call = IncomingCall::new_for_test(
            creator.clone(),
            "COMMITTED-GROUP-EVENT-STANZA".to_string(),
            wacore::time::from_secs(1_766_847_151_i64).expect("valid ts"),
            CallAction::GroupUpdate {
                update: Box::new(update),
            },
        );
        call.participant = Some(creator.with_device(1));

        assert!(apply_group_control(&client, &call, generation).await);
        let CallEvent::GroupUpdated(committed) = event_rx.try_recv().expect("group update event")
        else {
            panic!("expected a group update event");
        };
        assert_eq!(committed.group_jid, Some(group_jid));
        assert_eq!(committed.relay, Some(relay));
        registry.remove_if_current(call_id, generation);
    }

    async fn make_client() -> Arc<Client> {
        crate::test_utils::create_test_client().await
    }

    /// A client whose sends land on a counting transport (so ack sends succeed and can be counted).
    #[cfg(feature = "voip-runtime")]
    async fn make_sending_client() -> Arc<Client> {
        make_sending_client_with_failure_after(None).await.0
    }

    #[cfg(feature = "voip-runtime")]
    async fn make_sending_client_with_failure_after(
        failure_after: Option<usize>,
    ) -> (Arc<Client>, Arc<std::sync::atomic::AtomicUsize>) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wacore::handshake::NoiseCipher;

        struct CountingTransport {
            sends: Arc<AtomicUsize>,
            failure_after: Option<usize>,
        }
        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl crate::transport::Transport for CountingTransport {
            async fn send(&self, _data: bytes::Bytes) -> Result<(), anyhow::Error> {
                let attempt = self.sends.fetch_add(1, Ordering::SeqCst);
                if self.failure_after.is_some_and(|limit| attempt >= limit) {
                    return Err(anyhow::anyhow!("injected transport send failure"));
                }
                Ok(())
            }
            async fn disconnect(&self) {}
        }

        let client = make_client().await;
        let sends = Arc::new(AtomicUsize::new(0));
        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(CountingTransport {
                sends: sends.clone(),
                failure_after,
            }) as Arc<dyn crate::transport::Transport>,
            NoiseCipher::new(&key).expect("valid key"),
            NoiseCipher::new(&key).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
        (client, sends)
    }

    #[cfg(feature = "voip-runtime")]
    async fn make_blocking_sending_client() -> (
        Arc<Client>,
        async_channel::Receiver<()>,
        async_channel::Sender<()>,
    ) {
        use wacore::handshake::NoiseCipher;

        struct BlockingTransport {
            started: async_channel::Sender<()>,
            release: async_channel::Receiver<()>,
        }
        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl crate::transport::Transport for BlockingTransport {
            async fn send(&self, _data: bytes::Bytes) -> Result<(), anyhow::Error> {
                let _ = self.started.try_send(());
                let _ = self.release.recv().await;
                Ok(())
            }
            async fn disconnect(&self) {}
        }

        let (started_tx, started_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let client = make_client().await;
        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(BlockingTransport {
                started: started_tx,
                release: release_rx,
            }) as Arc<dyn crate::transport::Transport>,
            NoiseCipher::new(&key).expect("valid key"),
            NoiseCipher::new(&key).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
        (client, started_rx, release_tx)
    }

    #[cfg(feature = "voip-runtime")]
    fn video_stanza(state: &str) -> wacore_binary::Node {
        NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "STANZA-ID-0002")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("video")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CALL-ID-0001")
                .attr("state", state.to_string())
                .attr("device_orientation", "1")
                .build()])
            .build()
    }

    #[cfg(feature = "voip-runtime")]
    fn audio_selection_stanza(rate: &str) -> wacore_binary::Node {
        audio_selection_stanza_with_leading_children(rate, Vec::new())
    }

    #[cfg(feature = "voip-runtime")]
    fn audio_selection_stanza_with_leading_children(
        rate: &str,
        mut leading: Vec<wacore_binary::Node>,
    ) -> wacore_binary::Node {
        leading.push(
            NodeBuilder::new("accept")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CALL-ID-0001")
                .children([
                    NodeBuilder::new("audio")
                        .attr("enc", "opus")
                        .attr("rate", rate.to_string())
                        .build(),
                    NodeBuilder::new("capability")
                        .attr("ver", "1")
                        .bytes(wacore::stanza::call::CAPABILITY_OFFER.to_vec())
                        .build(),
                ])
                .build(),
        );
        NodeBuilder::new("call")
            .attr("from", fake_caller_lid())
            .attr("id", "STANZA-ID-AUDIO")
            .attr("t", "1766847151")
            .children(leading)
            .build()
    }

    #[cfg(feature = "voip-runtime")]
    fn register_native_opus_call(
        client: &Client,
        ring_devices: Vec<Jid>,
    ) -> (async_channel::Receiver<CallEvent>, u64) {
        use wacore::voip::{AudioFormat, CallEvent};

        let registry = client.call_registry();
        let mut session = wacore::voip::CallSession::new_outgoing(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        );
        session.audio_format = Some(AudioFormat::OPUS_16KHZ_60MS);
        session.ring_devices = ring_devices;
        let generation = registry.insert(session);
        assert!(
            registry.set_group_invite_self_device(
                "CALL-ID-0001",
                generation,
                GroupCallDevice::new(Jid::new("111111111111111", Server::Lid).with_device(1))
                    .with_capability(1, wacore::stanza::call::CAPABILITY_OFFER)
            )
        );
        let (event_tx, event_rx) = async_channel::unbounded::<CallEvent>();
        let (control_tx, _control_rx) = video_control_channel();
        registry.set_video_channels(
            "CALL-ID-0001",
            generation,
            event_tx,
            control_tx,
            Box::new(|| {}),
        );
        (event_rx, generation)
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn incompatible_audio_selection_emits_event_and_terminates_call() {
        use wacore::voip::CallEvent;

        let (client, sends) = make_sending_client_with_failure_after(None).await;
        let accepting = fake_caller_lid();
        let (event_rx, _generation) =
            register_native_opus_call(&client, vec![accepting.clone(), accepting.with_device(1)]);

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&audio_selection_stanza("8000")),
                    &mut cancelled,
                )
                .await
        );

        assert_eq!(
            event_rx.try_recv(),
            Ok(CallEvent::AudioFormatMismatch {
                expected_rate: 16_000,
                received_rates: vec![8_000],
            })
        );
        assert_eq!(sends.load(std::sync::atomic::Ordering::SeqCst), 2);
        assert!(client.call_registry().snapshot("CALL-ID-0001").is_none());
        assert!(!cancelled);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn incompatible_group_invitee_does_not_terminate_the_active_call() {
        use wacore::types::group_call::{GroupCallParticipant, GroupCallUpdate};
        use wacore::voip::{CallEvent, GroupStateApply};

        let (client, sends) = make_sending_client_with_failure_after(None).await;
        let (event_rx, generation) = register_native_opus_call(&client, Vec::new());
        let creator = fake_caller_lid();
        assert_eq!(
            client.call_registry().apply_group_update(
                GroupCallUpdate::builder()
                    .call_id("CALL-ID-0001".to_string())
                    .call_creator(creator.clone())
                    .transaction_id(1)
                    .media("audio".to_string())
                    .connected_limit(32)
                    .joinable(true)
                    .av_upgradable(true)
                    .rekey_requested(false)
                    .participants(vec![GroupCallParticipant::new(
                        creator.clone(),
                        vec![GroupCallDevice::new(creator.with_device(1))],
                    )])
                    .build(),
            ),
            GroupStateApply::Applied
        );

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&audio_selection_stanza("8000")),
                    &mut cancelled,
                )
                .await
        );

        assert_eq!(
            event_rx.try_recv(),
            Ok(CallEvent::AudioFormatMismatch {
                expected_rate: 16_000,
                received_rates: vec![8_000],
            })
        );
        assert!(
            client
                .call_registry()
                .is_current("CALL-ID-0001", generation),
            "one incompatible invitee cannot tear down shared group media"
        );
        assert_eq!(
            sends.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "only the incompatible invitee should receive a terminal stanza"
        );

        let unrouted = NodeBuilder::new("call")
            .attr("from", Jid::new("CALL-ID-0001", Server::Call))
            .attr("id", "UNROUTED-CODEC-MISMATCH")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("accept")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CALL-ID-0001")
                .children([NodeBuilder::new("audio")
                    .attr("enc", "opus")
                    .attr("rate", "8000")
                    .build()])
                .build()])
            .build();
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&unrouted), &mut cancelled)
                .await
        );
        assert_eq!(
            sends.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "an unrouted sender must not receive a call-scoped terminate"
        );
        assert!(
            client
                .call_registry()
                .is_current("CALL-ID-0001", generation),
            "an unrouted mismatch cannot end the active group generation"
        );
        client
            .call_registry()
            .remove_if_current("CALL-ID-0001", generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn compatible_audio_selection_keeps_call_active() {
        let (client, sends) = make_sending_client_with_failure_after(None).await;
        let (event_rx, generation) = register_native_opus_call(&client, Vec::new());
        let (handler, global_rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&audio_selection_stanza("16000")),
                    &mut cancelled,
                )
                .await
        );

        assert!(event_rx.try_recv().is_err());
        assert!(client.call_registry().snapshot("CALL-ID-0001").is_some());
        let fallback = client
            .call_registry()
            .group_invite_fallback_roster("CALL-ID-0001", generation)
            .expect("accepted 1:1 call retains both active device capabilities");
        assert_eq!(fallback.len(), 2);
        assert_eq!(fallback[1].devices[0].jid, fake_caller_lid());
        assert_eq!(sends.load(std::sync::atomic::Ordering::SeqCst), 0);
        assert!(matches!(
            global_rx.try_recv().as_deref(),
            Ok(Event::IncomingCall(IncomingCall {
                action: CallAction::Accept { .. },
                ..
            }))
        ));
        assert!(!cancelled);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn accept_capability_follows_the_forward_compatible_typed_action() {
        let client = make_sending_client().await;
        let (_event_rx, generation) = register_native_opus_call(&client, Vec::new());
        let stanza = audio_selection_stanza_with_leading_children(
            "16000",
            vec![NodeBuilder::new("future_call_action").build()],
        );

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&stanza), &mut cancelled)
                .await
        );
        let fallback = client
            .call_registry()
            .group_invite_fallback_roster("CALL-ID-0001", generation)
            .expect("the accepted device keeps its parsed capability");
        assert_eq!(
            fallback[1].devices[0].capability_version,
            Some(1),
            "a leading unknown child cannot hide the typed accept capability"
        );
        client
            .call_registry()
            .remove_if_current("CALL-ID-0001", generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn capabilityless_accept_retains_the_same_devices_preaccept_capability() {
        let client = make_sending_client().await;
        let (_event_rx, generation) = register_native_opus_call(&client, Vec::new());
        let peer = fake_caller_lid().with_device(3);
        let signaling = |tag: &'static str, capability: bool, stanza_id: &str| {
            let mut children = vec![
                NodeBuilder::new("audio")
                    .attr("enc", "opus")
                    .attr("rate", "16000")
                    .build(),
            ];
            if capability {
                children.push(
                    NodeBuilder::new("capability")
                        .attr("ver", "7")
                        .bytes(vec![7, 8, 9])
                        .build(),
                );
            }
            NodeBuilder::new("call")
                .attr("from", peer.clone())
                .attr("id", stanza_id.to_string())
                .attr("t", "1766847151")
                .children([NodeBuilder::new(tag)
                    .attr("call-creator", fake_caller_lid())
                    .attr("call-id", "CALL-ID-0001")
                    .children(children)
                    .build()])
                .build()
        };

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&signaling("preaccept", true, "PREACCEPT-CAPABILITY")),
                    &mut cancelled,
                )
                .await
        );
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&signaling("accept", false, "ACCEPT-NO-CAPABILITY")),
                    &mut cancelled,
                )
                .await
        );

        let fallback = client
            .call_registry()
            .group_invite_fallback_roster("CALL-ID-0001", generation)
            .expect("same-device preaccept capability remains available");
        assert_eq!(fallback[1].devices[0].jid.user, peer.user);
        assert_eq!(fallback[1].devices[0].jid.device, peer.device);
        assert_eq!(fallback[1].devices[0].capability_version, Some(7));
        client
            .call_registry()
            .remove_if_current("CALL-ID-0001", generation);
    }

    #[cfg(feature = "voip-runtime")]
    /// A rotation set while the call was ringing has to reach the device that
    /// answered. `call_creator` is our own LID on an outgoing call, so a stanza
    /// addressed there comes straight back to us and the callee stays upright.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn the_deferred_orientation_goes_to_the_device_that_answered() {
        let client = make_sending_client().await;
        let (_event_rx, generation) = register_native_opus_call(&client, Vec::new());
        assert!(
            client
                .call_registry()
                .set_is_video("CALL-ID-0001", generation, true)
        );
        assert!(
            client
                .call_registry()
                .set_local_video_orientation("CALL-ID-0001", generation, 3)
        );

        let answering_device = fake_caller_lid().with_device(4);
        let accept = NodeBuilder::new("call")
            .attr("from", Jid::new("CALL-ID-0001", Server::Call))
            .attr("participant", answering_device.clone())
            .attr("id", "STANZA-ID-ORIENTATION-ACCEPT")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("accept")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CALL-ID-0001")
                .children([NodeBuilder::new("audio")
                    .attr("enc", "opus")
                    .attr("rate", "16000")
                    .build()])
                .build()])
            .build();

        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&accept), &mut cancelled)
                .await
        );
        let sent = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
            .await
            .expect("the deferred rotation must be announced on accept")
            .expect("waiter");

        let r = sent.as_node_ref();
        assert_eq!(
            r.attrs().optional_string("to").as_deref(),
            Some(answering_device.to_string().as_str()),
            "the rotation must be addressed to the device that answered"
        );
        let action = r
            .children()
            .into_iter()
            .flatten()
            .find(|child| child.tag == "video")
            .expect("a <video> carrying the rotation");
        assert_eq!(
            action
                .attrs()
                .optional_string("device_orientation")
                .as_deref(),
            Some("3")
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn call_scoped_accept_records_the_routed_participant_device() {
        let client = make_sending_client().await;
        let (_event_rx, generation) = register_native_opus_call(&client, Vec::new());
        let (rekey_tx, rekey_rx) = async_channel::bounded(1);
        client
            .call_registry()
            .set_rekey_sender("CALL-ID-0001", generation, rekey_tx);
        let participant = fake_caller_lid().with_device(4);
        let accept = NodeBuilder::new("call")
            .attr("from", Jid::new("CALL-ID-0001", Server::Call))
            .attr("participant", participant.clone())
            .attr("id", "STANZA-ID-GROUP-ACCEPT")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("accept")
                .attr("call-creator", fake_caller_lid())
                .attr("call-id", "CALL-ID-0001")
                .children([
                    NodeBuilder::new("audio")
                        .attr("enc", "opus")
                        .attr("rate", "16000")
                        .build(),
                    NodeBuilder::new("capability")
                        .attr("ver", "1")
                        .bytes(wacore::stanza::call::CAPABILITY_OFFER.to_vec())
                        .build(),
                ])
                .build()])
            .build();
        let accept = node_to_owned_ref(&accept);
        let routed_participant = parse_call_stanza(accept.get())
            .expect("valid accept")
            .expect("known action")
            .participant
            .expect("participant metadata");

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), accept, &mut cancelled,)
                .await
        );
        let fallback = client
            .call_registry()
            .group_invite_fallback_roster("CALL-ID-0001", generation)
            .expect("accepted call retains both active device capabilities");
        assert_eq!(fallback[1].devices[0].jid, routed_participant);
        assert_eq!(
            client
                .call_registry()
                .answering_device_if_current("CALL-ID-0001", generation),
            Some(routed_participant.clone())
        );
        assert_eq!(
            rekey_rx.try_recv(),
            Ok(routed_participant.to_string()),
            "the receive pipeline must rekey to the actual answering device"
        );
        client
            .call_registry()
            .remove_if_current("CALL-ID-0001", generation);
    }

    #[cfg(feature = "voip-runtime")]
    async fn begin_local_upgrade(registry: &wacore::voip::CallRegistry, generation: u64) {
        let transition_lock = registry
            .video_transition_lock("CALL-ID-0001", generation)
            .expect("active call");
        let _guard = transition_lock.lock().await;
        assert!(
            registry
                .begin_local_video_request("CALL-ID-0001", generation)
                .is_some()
        );
    }

    // A <call><video> must be acked with the TYPED <ack class="call" type="video"> and the
    // generic router ack suppressed — an untyped ack makes the upgrade requester revert ~5s in.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn video_state_sends_typed_ack_and_cancels_generic() {
        use wacore::voip::CallEvent;

        let client = make_sending_client().await;
        let registry = client.call_registry();
        let generation = registry.insert(wacore::voip::CallSession::new_incoming(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        ));
        let (event_tx, _event_rx) = async_channel::unbounded::<CallEvent>();
        let (control_tx, _control_rx) = video_control_channel();
        registry.set_video_channels(
            "CALL-ID-0001",
            generation,
            event_tx,
            control_tx,
            Box::new(|| {}),
        );
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("ack"));

        let node = node_to_owned_ref(&video_stanza("11"));
        let mut cancelled = false;
        assert!(CallHandler.handle(client, node, &mut cancelled).await);
        assert!(cancelled, "the generic (untyped) ack must be suppressed");

        let ack = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
            .await
            .expect("typed ack must be sent")
            .expect("waiter");
        let r = ack.as_node_ref();
        assert_eq!(r.attrs().optional_string("class").as_deref(), Some("call"));
        assert_eq!(r.attrs().optional_string("type").as_deref(), Some("video"));
        assert_eq!(
            r.attrs().optional_string("id").as_deref(),
            Some("STANZA-ID-0002")
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn video_state_is_not_visible_before_typed_ack_completes() {
        use wacore::voip::CallEvent;

        let (client, send_started, release_send) = make_blocking_sending_client().await;
        let registry = client.call_registry();
        let generation = registry.insert(wacore::voip::CallSession::new_incoming(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        ));
        let (event_tx, event_rx) = async_channel::bounded::<CallEvent>(1);
        let (control_tx, _control_rx) = video_control_channel();
        registry.set_video_channels(
            "CALL-ID-0001",
            generation,
            event_tx.clone(),
            control_tx,
            Box::new(|| {}),
        );

        let node = node_to_owned_ref(&video_stanza("11"));
        let mut cancelled = false;
        let handled = {
            let handling = CallHandler.handle(client, node, &mut cancelled);
            tokio::pin!(handling);
            tokio::select! {
                started = send_started.recv() => started.expect("typed ack send started"),
                _ = &mut handling => panic!("handler completed before the blocked ack send"),
            }
            assert!(
                event_rx.is_empty(),
                "the application must not observe an uncommitted video state"
            );
            event_tx
                .try_send(CallEvent::RelayAllocated)
                .expect("fill the queue while the typed ack is in flight");

            release_send.send(()).await.expect("release ack send");
            handling.await
        };
        assert!(handled);
        assert!(cancelled);
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::VideoStateChanged { .. })
        ));
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn cancelled_peer_request_invalidates_its_acceptance_token() {
        use wacore::voip::CallEvent;

        let client = make_sending_client().await;
        let registry = client.call_registry();
        let generation = registry.insert(wacore::voip::CallSession::new_incoming(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        ));
        let (event_tx, event_rx) = async_channel::unbounded::<CallEvent>();
        let (control_tx, _control_rx) = video_control_channel();
        registry.set_video_channels(
            "CALL-ID-0001",
            generation,
            event_tx,
            control_tx,
            Box::new(|| {}),
        );

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&video_stanza("11")),
                    &mut cancelled,
                )
                .await
        );
        let token = match event_rx.try_recv().expect("upgrade request event") {
            CallEvent::VideoStateChanged {
                state: VideoState::UpgradeRequestV2,
                upgrade_token: Some(token),
                ..
            } => token,
            event => panic!("unexpected event: {event:?}"),
        };
        assert!(registry.peer_video_request_is_current("CALL-ID-0001", token));

        cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client,
                    node_to_owned_ref(&video_stanza("0")),
                    &mut cancelled,
                )
                .await
        );
        assert!(!registry.peer_video_request_is_current("CALL-ID-0001", token));
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::VideoStateChanged {
                state: VideoState::Disabled,
                upgrade_token: None,
                ..
            })
        ));
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn video_transition_stays_serialized_through_enabled_announcement() {
        use wacore::voip::CallEvent;

        let (client, send_started, release_send) = make_blocking_sending_client().await;
        let registry = client.call_registry();
        let generation = registry.insert(wacore::voip::CallSession::new_incoming(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        ));
        let (event_tx, _event_rx) = async_channel::bounded::<CallEvent>(1);
        let (control_tx, _control_rx) = video_control_channel();
        registry.set_video_channels(
            "CALL-ID-0001",
            generation,
            event_tx,
            control_tx,
            Box::new(|| {}),
        );
        begin_local_upgrade(&registry, generation).await;

        let node = node_to_owned_ref(&video_stanza("4"));
        let mut cancelled = false;
        let handled = {
            let handling = CallHandler.handle(client, node, &mut cancelled);
            tokio::pin!(handling);

            tokio::select! {
                started = send_started.recv() => started.expect("typed ack send started"),
                _ = &mut handling => panic!("handler completed before the typed ack send"),
            }
            release_send.send(()).await.expect("release typed ack");
            tokio::select! {
                started = send_started.recv() => started.expect("Enabled announcement started"),
                _ = &mut handling => panic!("handler completed before the Enabled send"),
            }
            assert!(
                registry.reserve_call_event("CALL-ID-0001").is_none(),
                "the next transition must not overtake committed effects"
            );
            release_send.send(()).await.expect("release Enabled send");
            handling.await
        };
        assert!(handled);
        assert!(cancelled);
        assert!(registry.reserve_call_event("CALL-ID-0001").is_some());
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn failed_enabled_announcement_rolls_back_the_video_upgrade() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wacore::voip::{CallEvent, VideoControl};

        let (client, sends) = make_sending_client_with_failure_after(Some(1)).await;
        let (global_handler, global_rx) = ChannelEventHandler::new();
        client.subscribe_handler(global_handler).detach();
        let registry = client.call_registry();
        let generation = registry.insert(wacore::voip::CallSession::new_outgoing(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        ));
        begin_local_upgrade(&registry, generation).await;
        let (event_tx, event_rx) = async_channel::unbounded::<CallEvent>();
        let (control_tx, control_rx) = video_control_channel();
        let teardown_control = control_tx.clone();
        let teardowns = Arc::new(AtomicUsize::new(0));
        registry.set_video_channels("CALL-ID-0001", generation, event_tx, control_tx, {
            let teardowns = teardowns.clone();
            Box::new(move || {
                teardowns.fetch_add(1, Ordering::SeqCst);
                let _ = teardown_control.send(VideoControl::Disable);
            })
        });

        let node = node_to_owned_ref(&video_stanza("4"));
        let mut cancelled = false;
        assert!(CallHandler.handle(client, node, &mut cancelled).await);

        assert!(
            cancelled,
            "the successful typed ack suppresses the generic ack"
        );
        assert_eq!(sends.load(Ordering::SeqCst), 2);
        assert_eq!(teardowns.load(Ordering::SeqCst), 1);
        assert!(event_rx.try_recv().is_err());
        assert_eq!(control_rx.try_recv(), Ok(VideoControl::Disable));
        assert!(control_rx.try_recv().is_err());
        assert!(!registry.snapshot("CALL-ID-0001").expect("session").is_video);
        assert!(global_rx.try_recv().is_err());
        assert!(registry.reserve_call_event("CALL-ID-0001").is_some());
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn video_state_does_not_mutate_a_same_id_replacement_after_ack_await() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wacore::voip::CallEvent;

        let (client, send_started, release_send) = make_blocking_sending_client().await;
        let (global_handler, global_rx) = ChannelEventHandler::new();
        client.subscribe_handler(global_handler).detach();
        let registry = client.call_registry();
        let stale_generation = registry.insert(wacore::voip::CallSession::new_incoming(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        ));
        let (stale_event_tx, stale_event_rx) = async_channel::unbounded::<CallEvent>();
        let (stale_control_tx, _stale_control_rx) = video_control_channel();
        registry.set_video_channels(
            "CALL-ID-0001",
            stale_generation,
            stale_event_tx,
            stale_control_tx,
            Box::new(|| {}),
        );
        begin_local_upgrade(&registry, stale_generation).await;

        let node = node_to_owned_ref(&video_stanza("4"));
        let mut cancelled = false;
        let replacement_teardowns = Arc::new(AtomicUsize::new(0));
        let replacement_control_rx = {
            let handling = CallHandler.handle(client, node, &mut cancelled);
            tokio::pin!(handling);
            tokio::select! {
                started = send_started.recv() => started.expect("typed ack send started"),
                _ = &mut handling => panic!("handler completed before the typed ack send"),
            }

            let replacement = registry.insert(wacore::voip::CallSession::new_incoming(
                "CALL-ID-0001",
                fake_caller_lid(),
                fake_caller_lid(),
            ));
            let (replacement_event_tx, _replacement_event_rx) =
                async_channel::unbounded::<CallEvent>();
            let (replacement_control_tx, replacement_control_rx) = video_control_channel();
            registry.set_video_channels(
                "CALL-ID-0001",
                replacement,
                replacement_event_tx,
                replacement_control_tx,
                {
                    let replacement_teardowns = replacement_teardowns.clone();
                    Box::new(move || {
                        replacement_teardowns.fetch_add(1, Ordering::SeqCst);
                    })
                },
            );

            release_send.send(()).await.expect("release typed ack");
            assert!(
                tokio::time::timeout(std::time::Duration::from_secs(2), &mut handling)
                    .await
                    .expect("stale handler must not start an Enabled send")
            );
            replacement_control_rx
        };
        assert!(cancelled);
        assert!(stale_event_rx.try_recv().is_err());
        assert!(replacement_control_rx.try_recv().is_err());
        assert!(
            !registry
                .snapshot("CALL-ID-0001")
                .expect("replacement")
                .is_video
        );
        assert_eq!(replacement_teardowns.load(Ordering::SeqCst), 0);
        assert!(global_rx.try_recv().is_err());
    }

    // Non-video call actions keep the generic ack: `cancelled` must stay false.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn non_video_actions_keep_the_generic_ack() {
        let client = make_sending_client().await;
        let node = node_to_owned_ref(&offer_stanza());
        let mut cancelled = false;
        assert!(CallHandler.handle(client, node, &mut cancelled).await);
        assert!(!cancelled, "an offer must not cancel the generic ack");
    }

    // The video state must reach the call's event stream (via the registry hook) with the
    // orientation, steer the plane (Enable on accept), and update the session's is_video flag.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn video_state_forwards_event_and_steers_the_plane() {
        use wacore::types::call::VideoState;
        use wacore::voip::{CallEvent, VideoControl};

        let client = make_sending_client().await;
        let (global_handler, global_rx) = ChannelEventHandler::new();
        client.subscribe_handler(global_handler).detach();
        let registry = client.call_registry();
        let session = wacore::voip::CallSession::new_incoming(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        );
        let generation = registry.insert(session);
        let (ev_tx, ev_rx) = async_channel::unbounded::<CallEvent>();
        let (ctl_tx, ctl_rx) = video_control_channel();
        registry.set_video_channels("CALL-ID-0001", generation, ev_tx, ctl_tx, Box::new(|| {}));
        begin_local_upgrade(&registry, generation).await;
        let enabled_waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));

        let node = node_to_owned_ref(&video_stanza("4")); // UpgradeAccept
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node, &mut cancelled)
                .await
        );

        let ev = ev_rx.try_recv().expect("event must be forwarded");
        assert!(matches!(
            ev,
            CallEvent::VideoStateChanged {
                state: VideoState::UpgradeAccept,
                orientation: Some(1),
                ..
            }
        ));
        let ctls: Vec<VideoControl> = std::iter::from_fn(|| ctl_rx.try_recv().ok()).collect();
        assert!(ctls.contains(&VideoControl::SetOrientation(1)));
        assert!(
            ctls.contains(&VideoControl::Enable),
            "an accept must enable our plane so the peer's video decodes"
        );
        assert!(
            registry.snapshot("CALL-ID-0001").expect("session").is_video,
            "UpgradeAccept marks the call as video"
        );
        assert!(matches!(
            global_rx.try_recv().as_deref(),
            Ok(Event::IncomingCall(IncomingCall {
                action: CallAction::VideoState { .. },
                ..
            }))
        ));
        let enabled = tokio::time::timeout(std::time::Duration::from_secs(2), enabled_waiter)
            .await
            .expect("Enabled stanza must be sent")
            .expect("waiter");
        let enabled_ref = enabled.as_node_ref();
        let children = enabled_ref.children().expect("video child");
        let video = &children[0];
        assert_eq!(video.tag, "video");
        assert_eq!(video.attrs().optional_string("state").as_deref(), Some("1"));
        assert_eq!(
            video.attrs().optional_string("dec").as_deref(),
            Some("H264")
        );
        assert_eq!(
            video
                .attrs()
                .optional_string("device_orientation")
                .as_deref(),
            Some("0")
        );

        // The peer stopping its direction does NOT stop ours or disable the shared plane.
        let node = node_to_owned_ref(&video_stanza("6"));
        let mut cancelled = false;
        assert!(CallHandler.handle(client, node, &mut cancelled).await);
        assert!(
            registry.snapshot("CALL-ID-0001").expect("session").is_video,
            "peer Stopped must preserve our enabled video direction"
        );
        let ctls: Vec<VideoControl> = std::iter::from_fn(|| ctl_rx.try_recv().ok()).collect();
        assert!(
            !ctls.contains(&VideoControl::Disable),
            "the peer stopping ITS video must not tear our plane down"
        );
    }

    // A peer that REJECTS or CANCELS an upgrade we started must fully release our local video: the
    // registered teardown hook runs (so the camera feed stops) and the session flag clears.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn refused_upgrade_runs_local_video_teardown() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wacore::voip::CallEvent;

        for state in ["5", "8"] {
            // 5 = UpgradeReject, 8 = UpgradeCancel.
            let client = make_sending_client().await;
            let registry = client.call_registry();
            let session = wacore::voip::CallSession::new_outgoing(
                "CALL-ID-0001",
                fake_caller_lid(),
                fake_caller_lid(),
            );
            let generation = registry.insert(session);
            begin_local_upgrade(&registry, generation).await;
            let torn = Arc::new(AtomicUsize::new(0));
            let (ev_tx, _ev_rx) = async_channel::unbounded::<CallEvent>();
            let (ctl_tx, _ctl_rx) = video_control_channel();
            registry.set_video_channels("CALL-ID-0001", generation, ev_tx, ctl_tx, {
                let torn = torn.clone();
                Box::new(move || {
                    torn.fetch_add(1, Ordering::SeqCst);
                })
            });

            let node = node_to_owned_ref(&video_stanza(state));
            let mut cancelled = false;
            assert!(CallHandler.handle(client, node, &mut cancelled).await);
            assert_eq!(
                torn.load(Ordering::SeqCst),
                1,
                "state {state}: a refused upgrade must run the local video teardown"
            );
            assert!(
                !registry.snapshot("CALL-ID-0001").expect("session").is_video,
                "state {state}: a refused upgrade must clear the video flag"
            );
        }
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn refused_upgrade_tears_down_when_typed_ack_send_fails() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wacore::voip::CallEvent;

        let client = make_client().await;
        let registry = client.call_registry();
        let session = wacore::voip::CallSession::new_outgoing(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        );
        let generation = registry.insert(session);
        begin_local_upgrade(&registry, generation).await;
        let torn = Arc::new(AtomicUsize::new(0));
        let (ev_tx, _ev_rx) = async_channel::unbounded::<CallEvent>();
        let (ctl_tx, _ctl_rx) = video_control_channel();
        registry.set_video_channels("CALL-ID-0001", generation, ev_tx, ctl_tx, {
            let torn = torn.clone();
            Box::new(move || {
                torn.fetch_add(1, Ordering::SeqCst);
            })
        });

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client,
                    node_to_owned_ref(&video_stanza("5")),
                    &mut cancelled,
                )
                .await
        );
        assert!(cancelled, "a built typed ack suppresses the generic ack");
        assert_eq!(torn.load(Ordering::SeqCst), 1);
        assert!(!registry.snapshot("CALL-ID-0001").expect("session").is_video);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn unacked_video_state_is_not_dispatched_globally() {
        use wacore::voip::CallEvent;

        let client = make_client().await;
        let (global_handler, global_rx) = ChannelEventHandler::new();
        client.subscribe_handler(global_handler).detach();
        let registry = client.call_registry();
        let generation = registry.insert(wacore::voip::CallSession::new_incoming(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        ));
        let (ev_tx, _ev_rx) = async_channel::unbounded::<CallEvent>();
        let (ctl_tx, ctl_rx) = video_control_channel();
        registry.set_video_channels("CALL-ID-0001", generation, ev_tx, ctl_tx, Box::new(|| {}));
        begin_local_upgrade(&registry, generation).await;

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client,
                    node_to_owned_ref(&video_stanza("4")),
                    &mut cancelled,
                )
                .await
        );
        assert!(
            cancelled,
            "the attempted typed ack suppresses the generic ack"
        );
        assert!(global_rx.try_recv().is_err());
        assert!(ctl_rx.try_recv().is_err());
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn committed_video_state_supersedes_diagnostics_when_event_queue_is_full() {
        use wacore::voip::{CallEvent, VideoControl};

        let client = make_sending_client().await;
        let registry = client.call_registry();
        let generation = registry.insert(wacore::voip::CallSession::new_incoming(
            "CALL-ID-0001",
            fake_caller_lid(),
            fake_caller_lid(),
        ));
        let (ev_tx, ev_rx) = async_channel::bounded::<CallEvent>(1);
        ev_tx.try_send(CallEvent::RelayAllocated).expect("prefill");
        let (ctl_tx, ctl_rx) = video_control_channel();
        registry.set_video_channels("CALL-ID-0001", generation, ev_tx, ctl_tx, Box::new(|| {}));
        begin_local_upgrade(&registry, generation).await;

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client,
                    node_to_owned_ref(&video_stanza("4")),
                    &mut cancelled,
                )
                .await
        );
        assert!(cancelled, "the typed ack must suppress the generic ack");
        assert!(registry.snapshot("CALL-ID-0001").expect("session").is_video);
        assert!(matches!(
            ev_rx.try_recv(),
            Ok(CallEvent::VideoStateChanged { .. })
        ));
        assert!(
            std::iter::from_fn(|| ctl_rx.try_recv().ok())
                .any(|ctl| matches!(ctl, VideoControl::Enable)),
            "the committed transition must steer the local plane"
        );
    }

    #[tokio::test]
    async fn offer_dispatches_event() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let node = node_to_owned_ref(&offer_stanza());
        let mut cancelled = false;
        assert!(CallHandler.handle(client, node, &mut cancelled).await);

        let mut seen = false;
        while let Ok(ev) = rx.try_recv() {
            if matches!(&*ev, Event::IncomingCall(call) if call.action.call_id() == "CALL-ID-0001")
            {
                seen = true;
                break;
            }
        }
        assert!(seen, "IncomingCall event must be dispatched");
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn active_group_offer_registers_ringing_session_before_dispatch() {
        use std::sync::atomic::Ordering;
        use wacore::voip::CallPhase;

        let (client, sends) = make_sending_client_with_failure_after(None).await;
        let (event_handler, event_rx) = ChannelEventHandler::new();
        client.subscribe_handler(event_handler).detach();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&active_group_offer_stanza()),
                    &mut cancelled,
                )
                .await
        );

        let session = client
            .call_registry()
            .snapshot("GROUP-CALL")
            .expect("active group offer must register a session");
        let generation = client
            .call_registry()
            .generation_of("GROUP-CALL")
            .expect("registered generation");
        assert_eq!(session.phase(), CallPhase::Ringing);
        assert_eq!(
            session.group.as_ref().map(|group| group.transaction_id),
            Some(7)
        );
        assert!(
            client.call_registry().take_ringing("GROUP-CALL"),
            "the offer must remain unanswered after its delivery receipt"
        );
        assert_eq!(sends.load(Ordering::SeqCst), 1);
        assert!(!cancelled);
        let event = event_rx.try_recv().expect("incoming group offer event");
        let Event::IncomingCall(incoming) = event.as_ref() else {
            panic!("expected incoming group offer event");
        };
        assert_eq!(
            incoming.group.as_deref().map(|group| group.transaction_id),
            Some(7)
        );
        assert_eq!(
            incoming.ringing_generation(),
            Some(generation),
            "the dispatched event must retain the exact eagerly registered generation"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn invalid_initial_group_offer_is_not_registered_or_dispatched() {
        let client = make_sending_client().await;
        let (event_handler, event_rx) = ChannelEventHandler::new();
        client.subscribe_handler(event_handler).detach();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&active_group_offer_stanza_with_limit("0")),
                    &mut cancelled,
                )
                .await
        );

        assert!(
            client.call_registry().generation_of("GROUP-CALL").is_none(),
            "an invalid initial roster must not create a generation"
        );
        assert!(
            !std::iter::from_fn(|| event_rx.try_recv().ok())
                .any(|event| matches!(&*event, Event::IncomingCall(_))),
            "an invalid initial roster must not reach application fallback"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn duplicate_group_offer_preserves_an_active_generation() {
        use wacore::voip::CallPhase;

        let client = make_sending_client().await;
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&active_group_offer_stanza()),
                    &mut cancelled,
                )
                .await
        );
        let registry = client.call_registry();
        let generation = registry
            .generation_of("GROUP-CALL")
            .expect("first offer generation");
        assert!(registry.transition("GROUP-CALL", CallPhase::Connecting));
        assert!(registry.take_ringing("GROUP-CALL"));

        let (event_handler, event_rx) = ChannelEventHandler::new();
        client.subscribe_handler(event_handler).detach();
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&active_group_offer_stanza()),
                    &mut cancelled,
                )
                .await
        );

        assert_eq!(registry.generation_of("GROUP-CALL"), Some(generation));
        assert_eq!(
            registry
                .snapshot("GROUP-CALL")
                .map(|session| session.phase()),
            Some(CallPhase::Connecting)
        );
        assert!(
            !registry.take_ringing("GROUP-CALL"),
            "a redelivery must not make an accepted call ring again"
        );
        assert!(
            !std::iter::from_fn(|| event_rx.try_recv().ok())
                .any(|event| matches!(&*event, Event::IncomingCall(_))),
            "a redelivery for the active generation must not publish a second offer"
        );
    }

    #[tokio::test]
    async fn unrecognized_action_does_not_dispatch() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let node = node_to_owned_ref(
            &NodeBuilder::new("call")
                .attr("from", fake_caller_lid())
                .attr("id", "S")
                .attr("t", "1766847151")
                .children([NodeBuilder::new("surprise").build()])
                .build(),
        );
        let mut cancelled = false;
        assert!(CallHandler.handle(client, node, &mut cancelled).await);

        while let Ok(ev) = rx.try_recv() {
            assert!(
                !matches!(&*ev, Event::IncomingCall(_)),
                "must not dispatch IncomingCall for unknown action"
            );
        }
    }

    /// Drives the handler end-to-end with a real `NoiseSocket` wired to a
    /// counting transport so the offer-ack send path is exercised. Without
    /// this, a regression that removes `send_offer_ack_receipt` from the
    /// handler would go unnoticed by the event-dispatch test alone.
    #[tokio::test]
    async fn offer_triggers_outbound_send() {
        use async_trait::async_trait;
        use bytes::Bytes;
        use std::sync::atomic::{AtomicUsize, Ordering};
        use wacore::handshake::NoiseCipher;

        struct CountingTransport {
            count: Arc<AtomicUsize>,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl crate::transport::Transport for CountingTransport {
            async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
                self.count.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
            async fn disconnect(&self) {}
        }

        let client = make_client().await;
        let count = Arc::new(AtomicUsize::new(0));
        let transport: Arc<dyn crate::transport::Transport> = Arc::new(CountingTransport {
            count: count.clone(),
        });
        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport,
            NoiseCipher::new(&key).expect("valid key"),
            NoiseCipher::new(&key).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));

        let node = node_to_owned_ref(&offer_stanza());
        let mut cancelled = false;
        assert!(CallHandler.handle(client, node, &mut cancelled).await);

        assert!(
            count.load(Ordering::SeqCst) >= 1,
            "handler must invoke the outbound send path for offer ack receipts"
        );
    }

    #[tokio::test]
    async fn malformed_stanza_does_not_error_or_dispatch() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let node = node_to_owned_ref(
            &NodeBuilder::new("call")
                .attr("from", fake_caller_lid())
                .attr("id", "S")
                .children([NodeBuilder::new("offer")
                    .attr("call-creator", fake_caller_lid())
                    .attr("call-id", "X")
                    .build()])
                .build(),
        );
        let mut cancelled = false;
        assert!(CallHandler.handle(client, node, &mut cancelled).await);
        while let Ok(ev) = rx.try_recv() {
            assert!(!matches!(&*ev, Event::IncomingCall(_)));
        }
    }

    // Caller-side sibling dismiss: when one callee device accepts our outbound call, the OTHER rung
    // device gets a per-device `<call to=DEVICE_JID id=..><terminate reason="accepted_elsewhere">`
    // (no <destination> block), and the rung set is consumed one-shot. A two-device callee keeps the
    // assertion to a single dismiss stanza the waiter can capture in full.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn accept_dismisses_other_callee_device() {
        let (client, sends) = make_sending_client_with_failure_after(None).await;
        let peer = Jid::new("222222222222222", Server::Lid);
        let creator = Jid::new("111111111111111", Server::Lid);
        let (sibling, accepting) = (peer.with_device(1), peer.with_device(2));

        // Register the outbound call with its rung device set on the session (as place_call does).
        let mut session =
            wacore::voip::CallSession::new_outgoing("CALL-ID-0001", peer.clone(), creator.clone());
        session.ring_devices = vec![sibling.clone(), accepting.clone()];
        client.call_registry().insert(session);

        // The call-scoped wrapper routes the acceptance through `participant`; `from` is not the
        // answering device and must never be used to choose the sibling dismiss targets.
        let accept = NodeBuilder::new("call")
            .attr("from", Jid::new("CALL-ID-0001", Server::Call))
            .attr("participant", accepting.clone())
            .attr("id", "STANZA-ACCEPT")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("accept")
                .attr("call-creator", creator.clone())
                .attr("call-id", "CALL-ID-0001")
                .build()])
            .build();

        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&accept), &mut cancelled)
                .await
        );

        let sent = waiter.await.expect("a dismiss <terminate> must be sent");
        let r = sent.as_node_ref();
        // Addressed to the SIBLING device JID (not the bare peer, not the accepting device), with an id.
        assert_eq!(
            r.attrs().optional_string("to").as_deref(),
            Some(sibling.to_string().as_str())
        );
        assert!(
            r.attrs().optional_string("id").is_some(),
            "wrapper needs an id"
        );
        let term = &r.children().unwrap()[0];
        assert_eq!(term.tag, "terminate");
        assert_eq!(
            term.attrs().optional_string("reason").as_deref(),
            Some("accepted_elsewhere")
        );
        assert!(
            term.get_optional_child("destination").is_none(),
            "terminate must not use a <destination> block"
        );
        assert!(
            client
                .call_registry()
                .take_dismiss_targets("CALL-ID-0001")
                .is_none(),
            "the rung device set must be consumed one-shot"
        );
        assert_eq!(
            sends.load(std::sync::atomic::Ordering::SeqCst),
            1,
            "the routed answerer must not receive accepted_elsewhere"
        );
    }

    // A `busy` reject is ONE DEVICE saying it cannot take the call, not the callee declining. It
    // must neither tear the call down nor dismiss the siblings, or the primary phone's <preaccept>
    // (observed landing 190ms later) arrives at a call we already ended. Critically the one-shot
    // rung set must SURVIVE, so a later genuine accept still has siblings to dismiss.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn busy_reject_keeps_the_call_and_the_rung_set() {
        let client = make_client().await;
        let peer = Jid::new("222222222222222", Server::Lid);
        let creator = Jid::new("111111111111111", Server::Lid);
        let (busy_device, other) = (peer.with_device(1), peer.with_device(2));

        let mut session =
            wacore::voip::CallSession::new_outgoing("CALL-ID-0001", peer.clone(), creator.clone());
        session.ring_devices = vec![busy_device.clone(), other.clone()];
        client.call_registry().insert(session);

        let reject = NodeBuilder::new("call")
            .attr("from", busy_device.clone())
            .attr("id", "STANZA-BUSY")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("reject")
                .attr("call-creator", creator.clone())
                .attr("call-id", "CALL-ID-0001")
                .attr("count", "0")
                .attr("reason", "busy")
                .build()])
            .build();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&reject), &mut cancelled)
                .await
        );

        assert!(
            client
                .call_registry()
                .generation_of("CALL-ID-0001")
                .is_some(),
            "a busy device must not end the call for the others"
        );
        assert!(
            client
                .call_registry()
                .take_dismiss_targets("CALL-ID-0001")
                .is_some(),
            "the one-shot rung set must survive a busy reject, or a later genuine accept has \
             nothing to dismiss and the sibling rings until the call times out"
        );
    }

    // The failure case for the above: a reject WITHOUT `reason="busy"` is the callee declining, and
    // must still tear the call down exactly as before.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn reject_without_busy_still_ends_the_call() {
        let client = make_client().await;
        let peer = Jid::new("222222222222222", Server::Lid);
        let creator = Jid::new("111111111111111", Server::Lid);
        let declining = peer.with_device(1);

        let session =
            wacore::voip::CallSession::new_outgoing("CALL-ID-0001", peer.clone(), creator.clone());
        client.call_registry().insert(session);

        let reject = NodeBuilder::new("call")
            .attr("from", declining)
            .attr("id", "STANZA-DECLINE")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("reject")
                .attr("call-creator", creator.clone())
                .attr("call-id", "CALL-ID-0001")
                .build()])
            .build();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&reject), &mut cancelled)
                .await
        );

        assert!(
            client
                .call_registry()
                .generation_of("CALL-ID-0001")
                .is_none(),
            "an explicit decline must still end the call"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn participant_reject_keeps_the_active_group_call() {
        let client = make_client().await;
        let creator = Jid::new("111111111111111", Server::Lid);
        let participant = Jid::new("222222222222222", Server::Lid);
        let call_id = "GROUP-CALL";
        let mut session = wacore::voip::CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        );
        session.group = Some(
            GroupCallUpdate::builder()
                .call_id(call_id.to_string())
                .call_creator(creator.clone())
                .transaction_id(1)
                .media("audio".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(Vec::new())
                .build(),
        );
        let generation = client.call_registry().insert(session);
        let reject = NodeBuilder::new("call")
            .attr("from", participant)
            .attr("id", "STANZA-GROUP-DECLINE")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("reject")
                .attr("call-creator", creator)
                .attr("call-id", call_id)
                .build()])
            .build();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&reject), &mut cancelled)
                .await
        );
        assert_eq!(
            client.call_registry().generation_of(call_id),
            Some(generation),
            "one participant declining an invite must not terminate group media"
        );
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn participant_reject_keeps_a_pre_ack_outgoing_group_call() {
        let client = make_client().await;
        let creator = Jid::new("111111111111111", Server::Lid);
        let participant = Jid::new("222222222222222", Server::Lid);
        let call_id = "PRE-ACK-GROUP-CALL";
        let session = wacore::voip::CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        );
        let generation = client.call_registry().insert_group(session);
        let reject = NodeBuilder::new("call")
            .attr("from", participant)
            .attr("id", "STANZA-PRE-ACK-GROUP-DECLINE")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("reject")
                .attr("call-creator", creator)
                .attr("call-id", call_id)
                .build()])
            .build();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(client.clone(), node_to_owned_ref(&reject), &mut cancelled)
                .await
        );
        assert_eq!(
            client.call_registry().generation_of(call_id),
            Some(generation),
            "the explicit group marker must protect the pre-ACK generation"
        );
        client
            .call_registry()
            .remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn only_the_creator_can_terminate_an_active_group_call() {
        let client = make_client().await;
        let creator = Jid::new("111111111111111", Server::Lid);
        let participant = Jid::new("222222222222222", Server::Lid).with_device(2);
        let call_id = "GROUP-CALL";
        let mut session = wacore::voip::CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        );
        session.group = Some(
            GroupCallUpdate::builder()
                .call_id(call_id.to_string())
                .call_creator(creator.clone())
                .transaction_id(1)
                .media("audio".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(Vec::new())
                .build(),
        );
        let generation = client.call_registry().insert(session);

        let participant_terminate = NodeBuilder::new("call")
            .attr("from", Jid::new(call_id, Server::Call))
            .attr("participant", participant)
            .attr("id", "STANZA-PARTICIPANT-TERM")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("terminate")
                .attr("call-creator", creator.clone())
                .attr("call-id", call_id)
                .build()])
            .build();
        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&participant_terminate),
                    &mut cancelled,
                )
                .await
        );
        assert_eq!(
            client.call_registry().generation_of(call_id),
            Some(generation),
            "one participant must not terminate the whole group call"
        );

        let creator_terminate = NodeBuilder::new("call")
            .attr("from", Jid::new(call_id, Server::Call))
            .attr("participant", creator.clone().with_device(1))
            .attr("id", "STANZA-CREATOR-TERM")
            .attr("t", "1766847152")
            .children([NodeBuilder::new("terminate")
                .attr("call-creator", creator)
                .attr("call-id", call_id)
                .build()])
            .build();
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&creator_terminate),
                    &mut cancelled,
                )
                .await
        );
        assert!(
            client.call_registry().generation_of(call_id).is_none(),
            "the creator's terminate must still end the group call"
        );
    }

    // A peer <terminate> for our call tears it down: the registry entry (and with it the media task)
    // is removed so CallHandle::wait_ended() resolves, instead of leaking until a relay timeout.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn terminate_tears_down_the_call() {
        let client = make_client().await;
        let peer = Jid::new("222222222222222", Server::Lid);
        let creator = Jid::new("111111111111111", Server::Lid);
        let session =
            wacore::voip::CallSession::new_outgoing("CALL-ID-0001", peer.clone(), creator.clone());
        client.call_registry().insert(session);
        assert!(
            client
                .call_registry()
                .generation_of("CALL-ID-0001")
                .is_some(),
            "precondition: the call is registered"
        );

        let terminate = NodeBuilder::new("call")
            .attr("from", peer.with_device(1))
            .attr("id", "STANZA-TERM")
            .attr("t", "1766847151")
            .children([NodeBuilder::new("terminate")
                .attr("call-creator", creator.clone())
                .attr("call-id", "CALL-ID-0001")
                .build()])
            .build();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&terminate),
                    &mut cancelled
                )
                .await
        );

        assert!(
            client
                .call_registry()
                .generation_of("CALL-ID-0001")
                .is_none(),
            "a peer <terminate> must remove the call from the registry"
        );
    }

    #[cfg(feature = "voip-runtime")]
    fn terminate_stanza(from: Jid, call_creator: Jid, call_id: &str) -> wacore_binary::Node {
        terminate_stanza_reason(from, call_creator, call_id, None)
    }

    #[cfg(feature = "voip-runtime")]
    fn terminate_stanza_reason(
        from: Jid,
        call_creator: Jid,
        call_id: &str,
        reason: Option<&str>,
    ) -> wacore_binary::Node {
        let mut term = NodeBuilder::new("terminate")
            .attr("call-creator", call_creator)
            .attr("call-id", call_id);
        if let Some(r) = reason {
            term = term.attr("reason", r);
        }
        NodeBuilder::new("call")
            .attr("from", from)
            .attr("id", "STANZA-TERM")
            .attr("t", "1766847151")
            .children([term.build()])
            .build()
    }

    #[cfg(feature = "voip-runtime")]
    fn count_missed(rx: &async_channel::Receiver<Arc<Event>>, call_id: &str) -> usize {
        let mut n = 0;
        while let Ok(ev) = rx.try_recv() {
            if let Event::MissedCall(m) = &*ev
                && m.call_id == call_id
                && matches!(m.reason, MissedReason::Remote)
            {
                n += 1;
            }
        }
        n
    }

    // An incoming offer that rings and is never answered, then a peer <terminate>, surfaces exactly
    // one MissedCall(Remote) -- WA Web's missed-call outcome. The offer is what marks the call ringing;
    // a terminate with no preceding offer is an ended call, not a missed one.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn unanswered_incoming_terminate_surfaces_missed_call() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let mut cancelled = false;
        // The offer rings (marks the call ringing).
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&offer_stanza()),
                    &mut cancelled
                )
                .await
        );
        // The peer gives up before we answer.
        let terminate = terminate_stanza(fake_caller_lid(), fake_caller_lid(), "CALL-ID-0001");
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&terminate),
                    &mut cancelled
                )
                .await
        );
        assert_eq!(
            count_missed(&rx, "CALL-ID-0001"),
            1,
            "an unanswered incoming <terminate> must surface exactly one MissedCall(Remote)"
        );
    }

    // A duplicate <terminate> for the same unanswered call must NOT re-fire a missed call: the ringing
    // flag is consumed one-shot by the first terminate.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn duplicate_terminate_does_not_refire_missed_call() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&offer_stanza()),
                    &mut cancelled
                )
                .await
        );
        let terminate = terminate_stanza(fake_caller_lid(), fake_caller_lid(), "CALL-ID-0001");
        for _ in 0..2 {
            assert!(
                CallHandler
                    .handle(
                        client.clone(),
                        node_to_owned_ref(&terminate),
                        &mut cancelled
                    )
                    .await
            );
        }
        assert_eq!(
            count_missed(&rx, "CALL-ID-0001"),
            1,
            "a duplicate <terminate> must not surface a second MissedCall"
        );
    }

    // A <terminate> for an OUTGOING call we placed must NOT surface a missed call: we never rang for
    // it, so it is an ended call, not a missed one. This is the regression the registry-absence gate
    // produced -- our own call's teardown looked identical to an unanswered incoming call.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn outgoing_call_terminate_does_not_surface_missed_call() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let peer = Jid::new("222222222222222", Server::Lid);
        let creator = Jid::new("111111111111111", Server::Lid); // us, the caller
        client
            .call_registry()
            .insert(wacore::voip::CallSession::new_outgoing(
                "CALL-ID-OUT",
                peer.clone(),
                creator.clone(),
            ));

        let mut cancelled = false;
        // The peer terminates our outgoing call; even a second terminate must stay silent.
        let terminate = terminate_stanza(peer.with_device(1), creator, "CALL-ID-OUT");
        for _ in 0..2 {
            assert!(
                CallHandler
                    .handle(
                        client.clone(),
                        node_to_owned_ref(&terminate),
                        &mut cancelled
                    )
                    .await
            );
        }
        assert_eq!(
            count_missed(&rx, "CALL-ID-OUT"),
            0,
            "an outgoing call's <terminate> must never surface a MissedCall(Remote)"
        );
    }

    // WA Web (ActionWebHandleIncomingSignalingMessage) maps a <terminate reason=...> to a call-log
    // outcome: accepted_elsewhere -> AcceptedElsewhere and rejected_elsewhere -> Rejected, meaning
    // another of our devices took the call. A companion device that rang then receives the caller's
    // elsewhere-dismiss must surface CallEndedElsewhere with the matching outcome, NOT a MissedCall.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn elsewhere_terminate_surfaces_call_ended_elsewhere_not_missed() {
        for (reason, expected) in [
            ("accepted_elsewhere", ElsewhereOutcome::Accepted),
            ("rejected_elsewhere", ElsewhereOutcome::Rejected),
        ] {
            let client = make_client().await;
            let (handler, rx) = ChannelEventHandler::new();
            client.subscribe_handler(handler).detach();

            let mut cancelled = false;
            assert!(
                CallHandler
                    .handle(
                        client.clone(),
                        node_to_owned_ref(&offer_stanza()),
                        &mut cancelled
                    )
                    .await
            );
            let terminate = terminate_stanza_reason(
                fake_caller_lid(),
                fake_caller_lid(),
                "CALL-ID-0001",
                Some(reason),
            );
            assert!(
                CallHandler
                    .handle(
                        client.clone(),
                        node_to_owned_ref(&terminate),
                        &mut cancelled
                    )
                    .await
            );
            // Single drain: assert the elsewhere outcome is present and no MissedCall slipped through.
            let mut missed = 0;
            let mut elsewhere = Vec::new();
            while let Ok(ev) = rx.try_recv() {
                match &*ev {
                    Event::MissedCall(m) if m.call_id == "CALL-ID-0001" => missed += 1,
                    Event::CallEndedElsewhere(e) if e.call_id == "CALL-ID-0001" => {
                        elsewhere.push(e.outcome)
                    }
                    _ => {}
                }
            }
            assert_eq!(
                missed, 0,
                "a <terminate reason=\"{reason}\"> must not surface a MissedCall"
            );
            assert_eq!(
                elsewhere,
                vec![expected],
                "a <terminate reason=\"{reason}\"> must surface CallEndedElsewhere({expected:?})"
            );
        }
    }

    // A reason that IS a missed outcome (timeout) on a still-ringing call surfaces the missed call,
    // confirming the reason gate excludes only the elsewhere outcomes, not every reason.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn timeout_terminate_surfaces_missed_call() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let mut cancelled = false;
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&offer_stanza()),
                    &mut cancelled
                )
                .await
        );
        let terminate = terminate_stanza_reason(
            fake_caller_lid(),
            fake_caller_lid(),
            "CALL-ID-0001",
            Some("timeout"),
        );
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&terminate),
                    &mut cancelled
                )
                .await
        );
        assert_eq!(
            count_missed(&rx, "CALL-ID-0001"),
            1,
            "a <terminate reason=\"timeout\"> on a ringing call must surface a MissedCall"
        );
    }

    // A call we locally declined must not later be recorded as missed: reject() consumes the ringing
    // flag, so a caller <terminate> that follows our <reject> reads as ended, not missed.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn local_reject_then_caller_terminate_is_not_missed() {
        use async_trait::async_trait;
        use bytes::Bytes;
        use wacore::handshake::NoiseCipher;

        struct OkTransport;
        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl crate::transport::Transport for OkTransport {
            async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
                Ok(())
            }
            async fn disconnect(&self) {}
        }

        let client = make_client().await;
        // reject()/offer-ack need a socket to send through.
        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(OkTransport) as Arc<dyn crate::transport::Transport>,
            NoiseCipher::new(&key).expect("valid key"),
            NoiseCipher::new(&key).expect("valid key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));

        let (handler, rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let mut cancelled = false;
        // The offer rings.
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&offer_stanza()),
                    &mut cancelled
                )
                .await
        );

        // We decline it.
        let owned = node_to_owned_ref(&offer_stanza());
        let incoming = parse_call_stanza(owned.get())
            .expect("offer parses")
            .expect("offer is a recognized call");
        client
            .voip()
            .reject(&incoming)
            .await
            .expect("reject sends the <reject>");

        // The caller then terminates; the declined call must not surface a missed call.
        let terminate = terminate_stanza(fake_caller_lid(), fake_caller_lid(), "CALL-ID-0001");
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&terminate),
                    &mut cancelled
                )
                .await
        );
        assert_eq!(
            count_missed(&rx, "CALL-ID-0001"),
            0,
            "a locally-declined call must not be recorded as a missed call"
        );
    }

    // A call we answered must not later be recorded as missed: accepting registers the call (as
    // accept().start() -> spawn_call -> registry.insert does), which consumes the ringing flag, so a
    // caller <terminate> after we picked up reads as ended, not missed.
    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn answered_call_then_caller_terminate_is_not_missed() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.subscribe_handler(handler).detach();

        let mut cancelled = false;
        // The offer rings.
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&offer_stanza()),
                    &mut cancelled
                )
                .await
        );

        // We answer it: registering the call (what spawn_call does) consumes the ringing flag.
        let peer = Jid::new("222222222222222", Server::Lid);
        let creator = fake_caller_lid();
        client
            .call_registry()
            .insert(wacore::voip::CallSession::new_incoming(
                "CALL-ID-0001",
                peer,
                creator.clone(),
            ));

        // The caller later hangs up; the answered call must not surface a missed call.
        let terminate = terminate_stanza(creator.clone(), creator, "CALL-ID-0001");
        assert!(
            CallHandler
                .handle(
                    client.clone(),
                    node_to_owned_ref(&terminate),
                    &mut cancelled
                )
                .await
        );
        assert_eq!(
            count_missed(&rx, "CALL-ID-0001"),
            0,
            "an answered call must not be recorded as a missed call"
        );
    }
}
