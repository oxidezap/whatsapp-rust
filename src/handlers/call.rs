use std::sync::Arc;

use async_trait::async_trait;
use log::{debug, warn};
#[cfg(feature = "voip")]
use wacore::stanza::call::{
    TERMINATE_REASON_ACCEPTED_ELSEWHERE, TERMINATE_REASON_GROUP_CALL_ENDED,
    TERMINATE_REASON_REJECTED_ELSEWHERE, TERMINATE_REASON_TIMEOUT, TerminateParams,
    build_terminate,
};
use wacore::stanza::call::{build_offer_ack_receipt, parse_call_stanza};
use wacore::types::call::{CallAction, IncomingCall, MissedCall, MissedReason};
#[cfg(feature = "voip")]
use wacore::types::call::{CallEndedElsewhere, ElsewhereOutcome};
use wacore::types::events::Event;
#[cfg(feature = "voip")]
use wacore_binary::Jid;
use wacore_binary::{OwnedNodeRef, Server};

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
        _cancelled: &mut bool,
    ) -> bool {
        let nr = node.get();
        match parse_call_stanza(nr) {
            Ok(Some(call)) => {
                // Diagnostic: every recognized <call> action we receive (offer/accept/reject/
                // terminate/transport/relaylatency...). Lets us see whether the caller actually gets a
                // peer device's <accept> (which drives the sibling dismiss).
                debug!(
                    "call: received {} for {} from {}",
                    call.action.action_kind(),
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
                    #[cfg(feature = "voip")]
                    if is_offer {
                        client
                            .call_registry()
                            .mark_incoming_ringing(call.action.call_id());
                    }
                    if is_offer && let Err(e) = send_offer_ack_receipt(&client, &call).await {
                        warn!("call: failed to send offer ack receipt: {e}");
                    }
                    // Caller-side: key our recv path to the device that actually answered. We dial the
                    // base callee LID, but a companion answers from `:N` and encrypts under its own
                    // device id; without this every inbound frame decrypts to garbage. One-shot, and a
                    // no-op for an incoming call or a call we aren't the caller of (no sender registered).
                    #[cfg(feature = "voip")]
                    if let CallAction::Accept { .. } = &call.action {
                        // Record the device that answered so a later <terminate> targets it (call
                        // signaling is addressed per device, not to the bare peer the offer rang).
                        client
                            .call_registry()
                            .set_answering_device(call.action.call_id(), call.from.clone());
                        client
                            .call_registry()
                            .send_rekey(call.action.call_id(), call.from.to_string());
                    }
                    // Caller-side multi-device dismiss: when one of the callee's devices accepts or
                    // rejects an outbound call of ours, tell the rest to stop ringing.
                    #[cfg(feature = "voip")]
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
                    #[cfg(feature = "voip")]
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
                    #[cfg(feature = "voip")]
                    if matches!(
                        &call.action,
                        CallAction::Reject { .. } | CallAction::Terminate { .. }
                    ) {
                        crate::voip::facade::terminate_call(&client, call.action.call_id());
                    }
                    client.core.event_bus.dispatch(Event::IncomingCall(call));
                }
            }
            Ok(None) => {
                debug!("call: ignoring unrecognized action (forward-compat)");
            }
            Err(e) => {
                warn!("call: failed to parse stanza: {e}");
            }
        }
        true
    }
}

#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.recv.call_offer_ack", level = "debug", skip_all, fields(peer = %call.from.observe()), err(Debug)))]
async fn send_offer_ack_receipt(client: &Client, call: &IncomingCall) -> anyhow::Result<()> {
    let own_from = match call.from.server {
        Server::Lid => client.get_lid(),
        _ => client.get_pn(),
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
#[cfg(feature = "voip")]
async fn dismiss_outgoing_siblings(client: &Client, call: &IncomingCall) {
    let reason = match &call.action {
        CallAction::Accept { .. } => TERMINATE_REASON_ACCEPTED_ELSEWHERE,
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
    let others: Vec<Jid> = devices
        .into_iter()
        .filter(|d| !same_device(d, &call.from))
        .collect();
    debug!(
        "call: {reason} from {} for {call_id}: dismissing {} sibling device(s)",
        call.from.observe(),
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

/// Whether two JIDs name the same device: user + server + device id. Excludes `agent`/`integrator`,
/// representation details that can differ between the usync device-list and a stanza's `from`.
#[cfg(feature = "voip")]
fn same_device(a: &Jid, b: &Jid) -> bool {
    a.user == b.user && a.server == b.server && a.device == b.device
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::node_to_owned_ref;
    use std::sync::Arc;
    use wacore::types::events::{ChannelEventHandler, Event};
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

    async fn make_client() -> Arc<Client> {
        crate::test_utils::create_test_client().await
    }

    #[tokio::test]
    async fn offer_dispatches_event() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.register_handler(handler);

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

    #[tokio::test]
    async fn unrecognized_action_does_not_dispatch() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.register_handler(handler);

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
        *client.noise_socket.lock().await = Some(Arc::new(noise_socket));

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
        client.register_handler(handler);

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
    #[cfg(feature = "voip")]
    #[tokio::test]
    async fn accept_dismisses_other_callee_device() {
        let client = make_client().await;
        let peer = Jid::new("222222222222222", Server::Lid);
        let creator = Jid::new("111111111111111", Server::Lid);
        let (sibling, accepting) = (peer.with_device(1), peer.with_device(2));

        // Register the outbound call with its rung device set on the session (as place_call does).
        let mut session =
            wacore::voip::CallSession::new_outgoing("CALL-ID-0001", peer.clone(), creator.clone());
        session.ring_devices = vec![sibling.clone(), accepting.clone()];
        client.call_registry().insert(session);

        // The `accepting` device accepts.
        let accept = NodeBuilder::new("call")
            .attr("from", accepting.clone())
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
    }

    // A peer <terminate> for our call tears it down: the registry entry (and with it the media task)
    // is removed so CallHandle::wait_ended() resolves, instead of leaking until a relay timeout.
    #[cfg(feature = "voip")]
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

    #[cfg(feature = "voip")]
    fn terminate_stanza(from: Jid, call_creator: Jid, call_id: &str) -> wacore_binary::Node {
        terminate_stanza_reason(from, call_creator, call_id, None)
    }

    #[cfg(feature = "voip")]
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

    #[cfg(feature = "voip")]
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
    #[cfg(feature = "voip")]
    #[tokio::test]
    async fn unanswered_incoming_terminate_surfaces_missed_call() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.register_handler(handler);

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
    #[cfg(feature = "voip")]
    #[tokio::test]
    async fn duplicate_terminate_does_not_refire_missed_call() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.register_handler(handler);

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
    #[cfg(feature = "voip")]
    #[tokio::test]
    async fn outgoing_call_terminate_does_not_surface_missed_call() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.register_handler(handler);

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
    #[cfg(feature = "voip")]
    #[tokio::test]
    async fn elsewhere_terminate_surfaces_call_ended_elsewhere_not_missed() {
        for (reason, expected) in [
            ("accepted_elsewhere", ElsewhereOutcome::Accepted),
            ("rejected_elsewhere", ElsewhereOutcome::Rejected),
        ] {
            let client = make_client().await;
            let (handler, rx) = ChannelEventHandler::new();
            client.register_handler(handler);

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
    #[cfg(feature = "voip")]
    #[tokio::test]
    async fn timeout_terminate_surfaces_missed_call() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.register_handler(handler);

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
    #[cfg(feature = "voip")]
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
        *client.noise_socket.lock().await = Some(Arc::new(noise_socket));

        let (handler, rx) = ChannelEventHandler::new();
        client.register_handler(handler);

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
    #[cfg(feature = "voip")]
    #[tokio::test]
    async fn answered_call_then_caller_terminate_is_not_missed() {
        let client = make_client().await;
        let (handler, rx) = ChannelEventHandler::new();
        client.register_handler(handler);

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
