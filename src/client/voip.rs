//! Call-control accessor. Reject/terminate are always available since their stanza builders live in
//! core; the high-level call/accept flows, including their signaling, need the `voip` feature.

#[cfg(feature = "voip-runtime")]
use std::sync::Arc;
#[cfg(feature = "voip-runtime")]
use std::time::Duration;

use wacore::stanza::call::{TerminateParams, build_reject, build_terminate};
#[cfg(feature = "voip-runtime")]
use wacore::stanza::group_call::{
    build_active_group_accept, build_active_group_preaccept, build_call_link_create,
    build_call_link_join_with_capability, build_call_link_query, build_raise_hand,
    build_screen_share, build_waiting_room_admit, build_waiting_room_deny,
    build_waiting_room_heartbeat, build_waiting_room_toggle, parse_call_link_create_ack,
    parse_call_link_join_ack, parse_call_link_query_ack, parse_waiting_room_admit_ack,
    parse_waiting_room_deny_ack, parse_waiting_room_toggle_ack,
};
#[cfg(feature = "voip-runtime")]
use wacore::types::call::CallAction;
use wacore::types::call::IncomingCall;
#[cfg(feature = "voip-runtime")]
use wacore::types::group_call::{
    CallLink, CallLinkJoin, CallLinkMedia, CallLinkPreview, ScreenShare, ScreenShareState,
};
#[cfg(feature = "voip-runtime")]
use wacore::voip::{AudioFormat, CallEvent, CallPhase, CallSession, VideoControl};
use wacore_binary::Jid;
#[cfg(feature = "voip-runtime")]
use wacore_binary::Node;
#[cfg(feature = "voip-runtime")]
use wacore_binary::Server;

#[cfg(feature = "voip-runtime")]
use super::ResponseWaiter;
use super::{Client, ClientError};

/// Opaque call-control handle obtained via [`Client::voip`]. Borrows the client;
/// kept as a newtype so the surface can grow without breaking callers.
pub struct Voip<'a> {
    client: &'a Client,
}

#[cfg(feature = "voip-runtime")]
struct CallLinkRegistrationGuard {
    registry: Arc<wacore::voip::CallRegistry>,
    call_id: String,
    generation: u64,
    armed: bool,
}

#[cfg(feature = "voip-runtime")]
impl CallLinkRegistrationGuard {
    fn new(registry: Arc<wacore::voip::CallRegistry>, call_id: &str, generation: u64) -> Self {
        Self {
            registry,
            call_id: call_id.to_string(),
            generation,
            armed: true,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

#[cfg(feature = "voip-runtime")]
impl Drop for CallLinkRegistrationGuard {
    fn drop(&mut self) {
        if self.armed {
            self.registry
                .remove_if_current(&self.call_id, self.generation);
        }
    }
}

impl Client {
    /// Call control: reject/terminate are always available; media (call/accept)
    /// needs the `voip` feature.
    pub fn voip(&self) -> Voip<'_> {
        Voip { client: self }
    }

    /// The per-call media registry the `voip` facade registers active calls in. `pub(crate)` so the
    /// facade and the connection-cleanup teardown share one instance.
    #[cfg(feature = "voip-runtime")]
    pub(crate) fn call_registry(&self) -> Arc<wacore::voip::CallRegistry> {
        self.call_registry.clone()
    }

    /// Lock the striped answer-transition lane for `call_id`. Incoming answer registration and
    /// answer teardown both use this, preventing a replacement generation from being installed
    /// after the old one is claimed but before its terminal stanza reaches the wire.
    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn lock_answer_transition(
        &self,
        call_id: &str,
    ) -> async_lock::MutexGuardArc<()> {
        use std::hash::{Hash, Hasher};

        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        call_id.hash(&mut hasher);
        let lane = hasher.finish() as usize % self.answer_transition_locks.len();
        self.answer_transition_locks[lane].clone().lock_arc().await
    }
}

/// Errors from call-control operations. `#[non_exhaustive]` so new variants stay
/// non-breaking after 1.0.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CallError {
    #[error("{0}")]
    Send(#[from] ClientError),
    #[error("call_id cannot be empty")]
    EmptyCallId,
    /// `accept` was called with an `IncomingCall` that is not an `<offer>` (nothing to answer).
    #[cfg(feature = "voip-runtime")]
    #[error("not an incoming call offer")]
    NotAnOffer,
    /// `accept().start()` was called without PCM or encoded audio endpoints.
    #[cfg(feature = "voip-runtime")]
    #[error("accept() requires audio(...) or encoded_audio(...) before start()")]
    MissingAudio,
    /// The selected media profile was not present in the incoming offer.
    #[cfg(feature = "voip-runtime")]
    #[error("incoming offer does not advertise the selected audio rate {0}")]
    AudioFormatNotOffered(u32),
    /// Video endpoints were supplied for an offer that only advertised audio.
    #[cfg(feature = "voip-runtime")]
    #[error("incoming offer did not advertise video; use start_video() after answering")]
    VideoNotOffered,
    /// The peer ended or superseded the call while the answer was being prepared.
    #[cfg(feature = "voip-runtime")]
    #[error("call ended during answer setup")]
    CallEndedDuringSetup,
    /// Decrypting the offer's encrypted callKey failed.
    #[cfg(feature = "voip-runtime")]
    #[error("callKey decrypt failed: {0}")]
    Decrypt(String),
    /// Assembling the call config from the offer's relay block failed.
    #[cfg(feature = "voip-runtime")]
    #[error("call setup failed: {0}")]
    Setup(String),
    /// Connecting the relay media transport (UDP/DTLS/SCTP) failed.
    #[cfg(feature = "voip-runtime")]
    #[error("relay connect failed: {0}")]
    Connect(String),
    /// The offer was missing media material (no `<enc>`/`<relay>`, no callKey, no own LID, etc.).
    #[cfg(feature = "voip-runtime")]
    #[error("media offer error: {0}")]
    Media(&'static str),
    /// The peer cancelled or replaced the upgrade before its video source became ready.
    #[cfg(feature = "voip-runtime")]
    #[error("video upgrade request is no longer current")]
    VideoUpgradeExpired,
    /// `call(peer)` resolved zero devices for the peer (nothing to address an offer to).
    #[cfg(feature = "voip-runtime")]
    #[error("peer has no resolvable devices")]
    NoDevices,
    /// An outgoing offer would emit a pkmsg `<enc>` but we hold no ADV account, so the peer could
    /// not validate the pre-key message. Refused before send to avoid advancing the sender chain
    /// (mirrors the peer-send path's `<device-identity>` requirement).
    #[cfg(feature = "voip-runtime")]
    #[error("offer pkmsg requires <device-identity> (account is None)")]
    MissingDeviceIdentity,
    /// A call-service response was malformed or rejected.
    #[cfg(feature = "voip-runtime")]
    #[error("call service response failed: {0}")]
    Response(String),
    /// The call service did not answer within its bounded request window.
    #[cfg(feature = "voip-runtime")]
    #[error("call service request timed out")]
    ResponseTimeout,
}

impl Voip<'_> {
    /// Reject an incoming call. Fire-and-forget — no server response is expected.
    pub async fn reject(&self, incoming: &IncomingCall) -> Result<(), CallError> {
        self.reject_call(
            incoming.action.call_id(),
            &incoming.from,
            incoming.action.call_creator(),
        )
        .await
    }

    /// Reject a call when its signaling identifiers are already available.
    /// `peer` is the outer `<call to>` target, while `call_creator` is the
    /// action's `call-creator` attribute; preserve them separately because
    /// they may differ for companion-device signaling.
    /// Fire-and-forget — no server response is expected.
    pub async fn reject_call(
        &self,
        call_id: &str,
        peer: &Jid,
        call_creator: &Jid,
    ) -> Result<(), CallError> {
        if call_id.is_empty() {
            return Err(CallError::EmptyCallId);
        }
        let id = self.client.generate_request_id();
        let stanza = build_reject(call_id, peer, call_creator, &id);
        // Consume the ringing flag BEFORE the async send: a caller <terminate> processed while we await
        // the send would otherwise hit take_ringing first and surface a phantom missed call for a call
        // we already declined (WA Web deletes it from _ringingCalls on reject). No-op if never ringing.
        #[cfg(feature = "voip-runtime")]
        self.client.call_registry().take_ringing(call_id);
        self.client.send_node(stanza).await?;
        Ok(())
    }

    /// Begin answering an incoming call: returns a builder; call `.audio(source, sink)` then
    /// `.start().await` to send `<preaccept>`, decrypt the callKey, send `<accept>`, connect the relay,
    /// and drive the call, yielding a [`CallHandle`](crate::voip::CallHandle). Requires
    /// `voip-runtime` or a profile that enables it: `voip`, `voip-encoded`, `voip-mlow`, or
    /// `voip-libopus`.
    #[cfg(feature = "voip-runtime")]
    pub fn accept<'b>(&'b self, incoming: &'b IncomingCall) -> crate::voip::AcceptCall<'b> {
        crate::voip::facade::AcceptCall::new(self.client, incoming)
    }

    /// Begin placing an outgoing 1:1 call to `peer`: returns a builder; call `.audio(source, sink)`
    /// then `.start().await` to generate the callKey, encrypt it per peer device, send the `<offer>`,
    /// and register the call, yielding a [`CallHandle`](crate::voip::CallHandle). The media engine
    /// only attaches once the server hands back the relay for our call-id (live), so the returned
    /// handle is dormant until then. Requires `voip-runtime` or a profile that enables it: `voip`,
    /// `voip-encoded`, `voip-mlow`, or `voip-libopus`.
    #[cfg(feature = "voip-runtime")]
    pub fn call<'b>(&'b self, peer: &'b Jid) -> crate::voip::OutgoingCall<'b> {
        crate::voip::facade::OutgoingCall::new(self.client, peer)
    }

    /// Begin a native group call to two or more selected users.
    #[cfg(feature = "voip-runtime")]
    pub fn group_call<'b>(&'b self, targets: &'b [Jid]) -> crate::voip::OutgoingGroupCall<'b> {
        crate::voip::facade::OutgoingGroupCall::new(self.client, targets)
    }

    /// Begin a native call bound to an existing group. The current roster is resolved at
    /// [`start`](crate::voip::GroupBoundCall::start), with this account excluded automatically.
    #[cfg(feature = "voip-runtime")]
    pub fn group_call_by_id<'b>(&'b self, group_jid: &'b Jid) -> crate::voip::GroupBoundCall<'b> {
        crate::voip::facade::GroupBoundCall::new(self.client, group_jid)
    }

    /// Join a reusable call link and attach group media after admission.
    #[cfg(feature = "voip-runtime")]
    pub fn call_link<'b>(
        &'b self,
        token_or_url: &'b str,
        media: CallLinkMedia,
    ) -> crate::voip::CallLinkCall<'b> {
        crate::voip::facade::CallLinkCall::new(self.client, token_or_url, media)
    }

    /// Send the eager preparation response for an active group-call invitation.
    #[cfg(feature = "voip-runtime")]
    pub async fn preaccept_group_invite(&self, incoming: &IncomingCall) -> Result<(), CallError> {
        let CallAction::Offer {
            call_id,
            call_creator,
            is_video,
            ..
        } = &incoming.action
        else {
            return Err(CallError::NotAnOffer);
        };
        if incoming.group.is_none() {
            return Err(CallError::Media("offer is not an active group invitation"));
        }
        let node = build_active_group_preaccept(
            call_id,
            call_creator,
            &self.client.generate_request_id(),
            *is_video,
        )
        .map_err(|error| CallError::Response(error.to_string()))?;
        self.client.send_node(node).await?;
        Ok(())
    }

    /// Immediately accept an active group-call invitation using call-scoped signaling.
    #[cfg(feature = "voip-runtime")]
    pub async fn accept_group_invite(&self, incoming: &IncomingCall) -> Result<(), CallError> {
        let CallAction::Offer {
            call_id,
            call_creator,
            ..
        } = &incoming.action
        else {
            return Err(CallError::NotAnOffer);
        };
        if incoming.group.is_none() {
            return Err(CallError::Media("offer is not an active group invitation"));
        }
        let node =
            build_active_group_accept(call_id, call_creator, &self.client.generate_request_id())
                .map_err(|error| CallError::Response(error.to_string()))?;
        self.client.send_node(node).await?;
        self.client.call_registry().take_ringing(call_id);
        self.client
            .call_registry()
            .transition(call_id, CallPhase::Connecting);
        Ok(())
    }

    /// Create a reusable audio or video call link.
    #[cfg(feature = "voip-runtime")]
    pub async fn create_call_link(&self, media: CallLinkMedia) -> Result<CallLink, CallError> {
        let request_id = self.client.generate_request_id();
        let request = build_call_link_create(media, &request_id)
            .map_err(|error| CallError::Response(error.to_string()))?;
        execute_call_service_request(
            self.client,
            &request_id,
            request,
            parse_call_link_create_ack,
        )
        .await
    }

    /// Inspect a call link without joining it.
    #[cfg(feature = "voip-runtime")]
    pub async fn preview_call_link(
        &self,
        token_or_url: &str,
        media: CallLinkMedia,
    ) -> Result<CallLinkPreview, CallError> {
        let token = normalize_call_link_token(token_or_url, media)?;
        let request_id = self.client.generate_request_id();
        let request = build_call_link_query(&token, media, &request_id)
            .map_err(|error| CallError::Response(error.to_string()))?;
        execute_call_service_request(self.client, &request_id, request, parse_call_link_query_ack)
            .await
    }

    /// Join a call link. The result explicitly reports whether this endpoint was admitted or placed
    /// in the waiting room; media starts only after an admitted authoritative group snapshot.
    #[cfg(feature = "voip-runtime")]
    pub async fn join_call_link(
        &self,
        token_or_url: &str,
        media: CallLinkMedia,
    ) -> Result<CallLinkJoin, CallError> {
        self.join_call_link_with_audio(token_or_url, media, AudioFormat::MLOW_16KHZ_60MS)
            .await
    }

    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn join_call_link_with_audio(
        &self,
        token_or_url: &str,
        media: CallLinkMedia,
        audio_format: AudioFormat,
    ) -> Result<CallLinkJoin, CallError> {
        let own_lid = self.client.lid().ok_or(CallError::Media("no own LID"))?;
        let token = normalize_call_link_token(token_or_url, media)?;
        let request_id = self.client.generate_request_id();
        let capability = crate::voip::facade::offer_capability(false, audio_format);
        let request = build_call_link_join_with_capability(&token, media, &request_id, capability)
            .map_err(|error| CallError::Response(error.to_string()))?;
        let mut join = execute_call_service_request(
            self.client,
            &request_id,
            request,
            parse_call_link_join_ack,
        )
        .await?;
        if join.token.is_empty() {
            join.token.clone_from(&token);
        }
        if join.media != media {
            return Err(CallError::Response(
                "call-link response changed the requested media mode".to_string(),
            ));
        }

        let mut session = CallSession::new_outgoing(
            &join.call_id,
            Jid::new(&join.call_id, Server::Call),
            join.call_creator.clone(),
        );
        session.audio_format = Some(audio_format);
        session.is_video = media == CallLinkMedia::Video;
        session.group = join.group.clone();
        let _ = session.transition_to(CallPhase::Calling);
        let _ = session.transition_to(if join.in_waiting_room {
            CallPhase::WaitingRoom
        } else {
            CallPhase::Connecting
        });
        let registry = self.client.call_registry();
        let generation = registry.insert(session);
        let mut registration =
            CallLinkRegistrationGuard::new(registry.clone(), &join.call_id, generation);

        if join.in_waiting_room {
            let Some(room) = join.pending_waiting_room() else {
                registry.remove_if_current(&join.call_id, generation);
                return Err(CallError::Response(
                    "call-link join omitted its waiting-room state".to_string(),
                ));
            };
            if registry.apply_waiting_room(room) != wacore::voip::GroupStateApply::Applied {
                registry.remove_if_current(&join.call_id, generation);
                return Err(CallError::Response(
                    "call-link waiting-room identity was rejected".to_string(),
                ));
            }
            if let Err(error) = self
                .waiting_room_heartbeat(&join.call_id, &join.call_creator)
                .await
            {
                registry.remove_if_current(&join.call_id, generation);
                return Err(error);
            }
            self.start_waiting_room_heartbeat(
                join.call_id.clone(),
                join.call_creator.clone(),
                generation,
            );
        } else if let Some(update) = join.group.as_ref()
            && update.rekey_requested
        {
            let raw_epoch = match crate::voip::facade::fanout_group_epoch(self.client, update).await
            {
                Ok(raw_epoch) => raw_epoch,
                Err(error) => {
                    registry.remove_if_current(&join.call_id, generation);
                    return Err(error);
                }
            };
            if !registry.send_group_epoch(&join.call_id, update.transaction_id, raw_epoch) {
                registry.remove_if_current(&join.call_id, generation);
                return Err(CallError::Media(
                    "call-link group epoch could not be retained",
                ));
            }
        }

        registry.set_group_invite_self_device(
            &join.call_id,
            generation,
            wacore::types::group_call::GroupCallDevice::new(own_lid).with_capability(1, capability),
        );
        registration.disarm();
        Ok(join)
    }

    /// Enable or disable approval for a live call-link waiting room.
    #[cfg(feature = "voip-runtime")]
    pub async fn set_approval_required(
        &self,
        call_id: &str,
        call_creator: &Jid,
        enabled: bool,
    ) -> Result<(), CallError> {
        self.ensure_waiting_room_admin(call_id)?;
        let request_id = self.client.generate_request_id();
        execute_call_service_request(
            self.client,
            &request_id,
            build_waiting_room_toggle(call_id, call_creator, enabled, &request_id),
            parse_waiting_room_toggle_ack,
        )
        .await?;
        self.client
            .call_registry()
            .set_waiting_room_enabled(call_id, enabled);
        Ok(())
    }

    /// Keep a pending call-link admission alive.
    #[cfg(feature = "voip-runtime")]
    pub async fn waiting_room_heartbeat(
        &self,
        call_id: &str,
        call_creator: &Jid,
    ) -> Result<(), CallError> {
        self.send_group_control(
            call_id,
            build_waiting_room_heartbeat(call_id, call_creator, &self.client.generate_request_id()),
        )
        .await
    }

    /// Admit one user from a call-link waiting room.
    #[cfg(feature = "voip-runtime")]
    pub async fn admit_waiting_user(
        &self,
        call_id: &str,
        call_creator: &Jid,
        user: &Jid,
    ) -> Result<(), CallError> {
        self.ensure_waiting_room_admin(call_id)?;
        let request_id = self.client.generate_request_id();
        execute_call_service_request(
            self.client,
            &request_id,
            build_waiting_room_admit(call_id, call_creator, user, &request_id),
            parse_waiting_room_admit_ack,
        )
        .await
    }

    /// Deny one user from a call-link waiting room.
    #[cfg(feature = "voip-runtime")]
    pub async fn deny_waiting_user(
        &self,
        call_id: &str,
        call_creator: &Jid,
        user: &Jid,
    ) -> Result<(), CallError> {
        self.ensure_waiting_room_admin(call_id)?;
        let request_id = self.client.generate_request_id();
        execute_call_service_request(
            self.client,
            &request_id,
            build_waiting_room_deny(call_id, call_creator, user, &request_id),
            parse_waiting_room_deny_ack,
        )
        .await
    }

    /// Publish the local persistent raise/lower-hand state.
    #[cfg(feature = "voip-runtime")]
    pub async fn set_hand_raised(
        &self,
        call_id: &str,
        call_creator: &Jid,
        raised: bool,
    ) -> Result<(), CallError> {
        let participant = self
            .client
            .lid()
            .ok_or(CallError::Media("no own LID"))?
            .to_non_ad();
        let target = Jid::new(call_id, Server::Call);
        self.send_group_control(
            call_id,
            build_raise_hand(
                call_id,
                &target,
                call_creator,
                &self.client.generate_request_id(),
                raised,
            ),
        )
        .await?;
        let registry = self.client.call_registry();
        if registry.set_raised_hand(call_id, &participant, raised) {
            registry.send_call_event(
                call_id,
                CallEvent::HandRaised {
                    participant,
                    raised,
                },
            );
        }
        Ok(())
    }

    /// Publish a screen-share start/stop transition.
    #[cfg(feature = "voip-runtime")]
    pub async fn set_screen_share(
        &self,
        call_id: &str,
        call_creator: &Jid,
        state: ScreenShareState,
        screen_share_id: Option<u32>,
    ) -> Result<(), CallError> {
        let participant = self
            .client
            .lid()
            .ok_or(CallError::Media("no own LID"))?
            .to_non_ad();
        let target = Jid::new(call_id, Server::Call);
        self.send_group_control(
            call_id,
            build_screen_share(
                call_id,
                &target,
                call_creator,
                &self.client.generate_request_id(),
                state,
                screen_share_id,
            ),
        )
        .await?;
        let screen_share = ScreenShare::new(state, screen_share_id);
        let registry = self.client.call_registry();
        if registry.set_screen_share(call_id, &participant, screen_share.clone()) {
            registry.send_call_event(
                call_id,
                CallEvent::ScreenShareChanged {
                    participant,
                    screen_share,
                },
            );
        }
        if state == ScreenShareState::Started
            && let Some(generation) = registry.generation_of(call_id)
        {
            registry.send_video_ctl(call_id, generation, VideoControl::RequireKeyframe);
        }
        Ok(())
    }

    #[cfg(feature = "voip-runtime")]
    async fn send_group_control(&self, call_id: &str, node: Node) -> Result<(), CallError> {
        if call_id.is_empty() {
            return Err(CallError::EmptyCallId);
        }
        self.client.send_node(node).await?;
        Ok(())
    }

    #[cfg(feature = "voip-runtime")]
    fn ensure_waiting_room_admin(&self, call_id: &str) -> Result<(), CallError> {
        let room = self
            .client
            .call_registry()
            .group_state(call_id)
            .and_then(|state| state.waiting_room().cloned())
            .ok_or(CallError::Media("call has no waiting-room state"))?;
        if !room.is_admin {
            return Err(CallError::Media(
                "waiting-room control requires an administrator",
            ));
        }
        Ok(())
    }

    #[cfg(feature = "voip-runtime")]
    fn start_waiting_room_heartbeat(&self, call_id: String, call_creator: Jid, generation: u64) {
        let weak_client = self.client.self_weak.get().cloned().unwrap_or_default();
        let runtime = self.client.runtime.clone();
        let sleeper = runtime.clone();
        let heartbeat_call_id = call_id.clone();
        let task = runtime.spawn(Box::pin(async move {
            loop {
                sleeper.sleep(Duration::from_secs(10)).await;
                let Some(client) = weak_client.upgrade() else {
                    break;
                };
                if client.call_registry().phase(&heartbeat_call_id) != Some(CallPhase::WaitingRoom)
                {
                    break;
                }
                let request_id = client.generate_request_id();
                if client
                    .send_node(build_waiting_room_heartbeat(
                        &heartbeat_call_id,
                        &call_creator,
                        &request_id,
                    ))
                    .await
                    .is_err()
                {
                    break;
                }
            }
        }));
        self.client
            .call_registry()
            .set_waiting_room_task(&call_id, generation, task);
    }

    /// Terminate an active call.
    pub async fn terminate(
        &self,
        call_id: &str,
        peer: &Jid,
        call_creator: &Jid,
    ) -> Result<(), CallError> {
        if call_id.is_empty() {
            return Err(CallError::EmptyCallId);
        }
        let id = self.client.generate_request_id();
        let stanza = build_terminate(&TerminateParams {
            call_id,
            to: peer,
            id: Some(&id),
            call_creator,
            reason: None,
        });
        let sent = self.client.send_node(stanza).await;
        // Tear the local call down regardless of whether the stanza reached the peer: the app asked to
        // hang up, and a failed signaling send must not leave the media task capturing/sending (or a
        // dormant outgoing call free to attach on a late relay ack). Reuse the same teardown the peer's
        // `<terminate>` triggers so the public hangup actually ends our side too.
        #[cfg(feature = "voip-runtime")]
        crate::voip::facade::terminate_call(self.client, call_id);
        sent?;
        Ok(())
    }
}

#[cfg(feature = "voip-runtime")]
fn normalize_call_link_token(
    token_or_url: &str,
    expected_media: CallLinkMedia,
) -> Result<String, CallError> {
    let value = token_or_url.trim();
    if value.is_empty() {
        return Err(CallError::Response(
            "call-link token is required".to_string(),
        ));
    }
    const PREFIX: &str = "https://call.whatsapp.com/";
    if let Some(path) = value.strip_prefix(PREFIX) {
        let mut parts = path.split('/');
        let media = parts.next();
        let token = parts.next();
        if parts.next().is_some()
            || token.is_none_or(str::is_empty)
            || media != Some(expected_media.as_str())
        {
            return Err(CallError::Response(
                "invalid call-link URL or media mode".to_string(),
            ));
        }
        return Ok(token.unwrap_or_default().to_string());
    }
    if value.contains("://") || value.contains('/') {
        return Err(CallError::Response("invalid call-link token".to_string()));
    }
    Ok(value.to_string())
}

#[cfg(feature = "voip-runtime")]
async fn execute_call_service_request<T>(
    client: &Client,
    request_id: &str,
    request: Node,
    parse: fn(&wacore_binary::NodeRef<'_>) -> anyhow::Result<T>,
) -> Result<T, CallError> {
    let (tx, response) = futures::channel::oneshot::channel();
    let cleanup_generation = client
        .response_waiters_guard()
        .try_insert_guarded(request_id.to_string(), ResponseWaiter::Iq(tx))
        .ok_or_else(|| CallError::Response("duplicate call-service request id".to_string()))?;
    let _waiter_guard = crate::request::ResponseWaiterGuard::new(
        client.response_waiters.clone(),
        request_id.to_string(),
        cleanup_generation,
    );
    if let Err(error) = client.send_node(request).await {
        return Err(error.into());
    }
    let response =
        match wacore::runtime::timeout(&*client.runtime, Duration::from_secs(10), response).await {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => return Err(CallError::Response("response channel closed".to_string())),
            Err(_) => return Err(CallError::ResponseTimeout),
        };
    parse(response.get()).map_err(|error| CallError::Response(error.to_string()))
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[cfg(feature = "voip-runtime")]
    use std::time::Duration;

    use async_trait::async_trait;
    use bytes::Bytes;
    use wacore::handshake::NoiseCipher;
    use wacore::types::call::{CallAction, IncomingCall};
    #[cfg(feature = "voip-runtime")]
    use wacore::types::group_call::{CallLinkMedia, ScreenShareState};
    #[cfg(feature = "voip-runtime")]
    use wacore::voip::{
        AudioFormat, CallEvent, CallPhase, CallSession, VideoControl, video_control_channel,
    };
    #[cfg(feature = "voip-runtime")]
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::{Jid, Server};

    use crate::client::Client;

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

    async fn make_client_with_count() -> (Arc<Client>, Arc<AtomicUsize>) {
        let client = crate::test_utils::create_test_client().await;

        let count = Arc::new(AtomicUsize::new(0));
        let socket_transport: Arc<dyn crate::transport::Transport> = Arc::new(CountingTransport {
            count: count.clone(),
        });
        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            socket_transport,
            NoiseCipher::new(&key).expect("valid key"),
            NoiseCipher::new(&key).expect("valid key"),
        );
        *client.noise_socket.lock().await = Some(Arc::new(noise_socket));
        (client, count)
    }

    #[cfg(feature = "voip-runtime")]
    struct FailingTransport;

    #[cfg(feature = "voip-runtime")]
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl crate::transport::Transport for FailingTransport {
        async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
            Err(anyhow::anyhow!("transport down"))
        }
        async fn disconnect(&self) {}
    }

    #[cfg(feature = "voip-runtime")]
    async fn make_client_failing() -> Arc<Client> {
        let client = crate::test_utils::create_test_client().await;
        let socket_transport: Arc<dyn crate::transport::Transport> = Arc::new(FailingTransport);
        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            socket_transport,
            NoiseCipher::new(&key).expect("valid key"),
            NoiseCipher::new(&key).expect("valid key"),
        );
        *client.noise_socket.lock().await = Some(Arc::new(noise_socket));
        client
    }

    fn caller() -> Jid {
        Jid::new("111111111111111", Server::Lid)
    }

    fn call_creator() -> Jid {
        Jid::new("222222222222222", Server::Lid)
    }

    fn incoming_reject() -> IncomingCall {
        IncomingCall::new_for_test(
            caller(),
            "STANZA-ID-0001".into(),
            wacore::time::from_secs(1_766_847_151_i64).expect("valid ts"),
            CallAction::Offer {
                call_id: "CALL-ID-0001".into(),
                call_creator: caller(),
                caller_pn: None,
                caller_country_code: None,
                device_class: None,
                joinable: false,
                is_video: false,
                audio: Vec::new(),
                group_jid: None,
            },
        )
    }

    #[tokio::test]
    async fn reject_sends_stanza() {
        let (client, count) = make_client_with_count().await;
        client
            .voip()
            .reject(&incoming_reject())
            .await
            .expect("reject should send");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn reject_call_sends_stanza_without_event_context() {
        let (client, count) = make_client_with_count().await;
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let peer = caller();
        let creator = call_creator();
        client
            .voip()
            .reject_call("CALL-ID-0001", &peer, &creator)
            .await
            .expect("reject should send");
        assert_eq!(count.load(Ordering::SeqCst), 1);

        let sent = waiter.await.expect("reject stanza should be observable");
        let call = sent.as_node_ref();
        assert_eq!(
            call.attrs().optional_string("to").as_deref(),
            Some(peer.to_string().as_str())
        );
        let reject = &call.children().expect("call action")[0];
        assert_eq!(reject.tag, "reject");
        assert_eq!(
            reject.attrs().optional_string("call-id").as_deref(),
            Some("CALL-ID-0001")
        );
        assert_eq!(
            reject.attrs().optional_string("call-creator").as_deref(),
            Some(creator.to_string().as_str())
        );
        assert_eq!(
            reject.attrs().optional_string("count").as_deref(),
            Some("0")
        );
    }

    #[tokio::test]
    async fn terminate_sends_stanza() {
        let (client, count) = make_client_with_count().await;
        client
            .voip()
            .terminate("CALL-ID-0001", &caller(), &caller())
            .await
            .expect("terminate should send");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn terminate_aborts_the_local_call() {
        use wacore::voip::CallSession;
        let (client, _count) = make_client_with_count().await;
        let reg = client.call_registry();
        reg.insert(CallSession::new_outgoing(
            "CALL-ID-0001",
            caller(),
            caller(),
        ));
        assert_eq!(reg.active_count(), 1);
        client
            .voip()
            .terminate("CALL-ID-0001", &caller(), &caller())
            .await
            .expect("terminate should send");
        assert_eq!(
            reg.active_count(),
            0,
            "terminate must tear the local call down, not just signal the peer"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn terminate_tears_down_local_even_when_send_fails() {
        use wacore::voip::CallSession;
        let client = make_client_failing().await;
        let reg = client.call_registry();
        reg.insert(CallSession::new_outgoing(
            "CALL-ID-0001",
            caller(),
            caller(),
        ));
        assert_eq!(reg.active_count(), 1);
        let res = client
            .voip()
            .terminate("CALL-ID-0001", &caller(), &caller())
            .await;
        assert!(
            res.is_err(),
            "a failed signaling send must surface the error"
        );
        assert_eq!(
            reg.active_count(),
            0,
            "a failed signaling send must still tear the local media task down"
        );
    }

    #[tokio::test]
    async fn reject_empty_call_id_errors() {
        let (client, _count) = make_client_with_count().await;
        let mut call = incoming_reject();
        call.action = CallAction::Reject {
            call_id: String::new(),
            call_creator: caller(),
            reason: None,
        };
        assert!(client.voip().reject(&call).await.is_err());
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn local_group_controls_commit_state_events_and_screen_keyframe_gate() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let own_device = Jid::new("111111111111111", Server::Lid).with_device(1);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_device,
            )))
            .await;
        let participant = Jid::new("111111111111111", Server::Lid);
        let creator = participant.clone();
        let call_id = "TEST-GROUP-CONTROLS";
        let registry = client.call_registry();
        let generation = registry.insert(CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        ));
        let (event_tx, event_rx) = async_channel::bounded(4);
        let (video_tx, video_rx) = video_control_channel();
        registry.set_video_channels(call_id, generation, event_tx, video_tx, Box::new(|| {}));

        client
            .voip()
            .set_hand_raised(call_id, &creator, true)
            .await
            .expect("raise hand");
        assert!(
            registry
                .group_state(call_id)
                .expect("group state")
                .raised_hands()
                .contains(&participant)
        );
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::HandRaised {
                participant: event_participant,
                raised: true,
            }) if event_participant == participant
        ));

        client
            .voip()
            .set_screen_share(call_id, &creator, ScreenShareState::Started, Some(7))
            .await
            .expect("start screen share");
        let share = registry
            .group_state(call_id)
            .expect("group state")
            .screen_shares()
            .get(&participant)
            .cloned()
            .expect("local screen share");
        assert_eq!(share.state, ScreenShareState::Started);
        assert_eq!(share.version, 2);
        assert_eq!(share.screen_share_id, Some(7));
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::ScreenShareChanged {
                participant: event_participant,
                screen_share,
            }) if event_participant == participant && screen_share == share
        ));
        assert_eq!(
            video_rx.try_recv(),
            Ok(VideoControl::RequireKeyframe),
            "starting a replacement screen source must re-arm the H.264 recovery gate"
        );

        client
            .voip()
            .set_screen_share(call_id, &creator, ScreenShareState::Stopped, None)
            .await
            .expect("stop screen share");
        assert!(
            registry
                .group_state(call_id)
                .expect("group state")
                .screen_shares()
                .is_empty()
        );
        assert!(matches!(
            event_rx.try_recv(),
            Ok(CallEvent::ScreenShareChanged {
                participant: event_participant,
                screen_share,
            }) if event_participant == participant
                && screen_share.state == ScreenShareState::Stopped
        ));
        assert_eq!(video_rx.try_recv(), Err(async_channel::TryRecvError::Empty));
        assert_eq!(transport.sent_count(), 3);
        registry.remove_if_current(call_id, generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test(start_paused = true)]
    async fn call_link_requests_round_trip_through_bounded_response_waiters() {
        async fn wait_for_frames(
            transport: &crate::transport::mock::CapturingMockTransport,
            expected: usize,
        ) {
            for _ in 0..10_000 {
                if transport.sent_count() >= expected {
                    return;
                }
                tokio::task::yield_now().await;
            }
            panic!("timed out waiting for {expected} captured call frames");
        }

        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let create_client = client.clone();
        let create = tokio::spawn(async move {
            create_client
                .voip()
                .create_call_link(CallLinkMedia::Video)
                .await
        });
        let request = sent.await.expect("link_create request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_create")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("link_create")
                    .attr("token", "TEST-CALL-LINK")
                    .attr("media", "video")
                    .build()])
                .build(),
        )
        .await;
        let link = create.await.expect("create task").expect("create response");
        assert_eq!(link.token, "TEST-CALL-LINK");
        assert_eq!(link.media, CallLinkMedia::Video);

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let preview_client = client.clone();
        let preview = tokio::spawn(async move {
            preview_client
                .voip()
                .preview_call_link("TEST-CALL-LINK", CallLinkMedia::Video)
                .await
        });
        let request = sent.await.expect("link_query request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_query")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("link_query")
                    .attr("token", "TEST-CALL-LINK")
                    .attr("media", "video")
                    .attr("link_creator", creator.clone())
                    .children([NodeBuilder::new("waiting_room")
                        .attr("enabled", "1")
                        .attr("is_admin", "0")
                        .build()])
                    .build()])
                .build(),
        )
        .await;
        let preview = preview
            .await
            .expect("preview task")
            .expect("preview response");
        assert_eq!(preview.creator, creator);
        assert!(preview.waiting_room_enabled);
        assert!(!preview.is_admin);

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let join_client = client.clone();
        let join = tokio::spawn(async move {
            join_client
                .voip()
                .join_call_link_with_audio(
                    "TEST-CALL-LINK",
                    CallLinkMedia::Video,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let request = sent.await.expect("link_join request");
        let request_ref = request.as_node_ref();
        let action = &request_ref.children().expect("join action children")[0];
        assert_eq!(
            action
                .get_optional_child("capability")
                .expect("join capability")
                .content_bytes(),
            Some(wacore::stanza::call::CAPABILITY_STANDARD_OPUS_OFFER.as_slice())
        );
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "TEST-CALL-ID")
                    .attr("call-creator", creator.clone())
                    .attr("link-token", "TEST-CALL-LINK")
                    .attr("media", "video")
                    .attr("enabled", "1")
                    .attr("is_admin", "0")
                    .attr("transaction-id", "7")
                    .children([NodeBuilder::new("user")
                        .attr("jid", Jid::new("444444444444444", Server::Lid))
                        .attr("state", "pending")
                        .build()])
                    .build()])
                .build(),
        )
        .await;
        let join = join.await.expect("join task").expect("join response");
        assert!(join.in_waiting_room);
        assert!(join.waiting_room_enabled);
        assert_eq!(join.call_id, "TEST-CALL-ID");
        assert!(join.group.is_none());
        assert_eq!(
            client.call_registry().phase("TEST-CALL-ID"),
            Some(CallPhase::WaitingRoom)
        );
        let room = client
            .call_registry()
            .group_state("TEST-CALL-ID")
            .and_then(|state| state.waiting_room().cloned())
            .expect("waiting-room state retained");
        assert_eq!(room.transaction_id, Some(7));
        assert_eq!(room.users.len(), 1);

        wait_for_frames(&transport, 4).await;
        let immediate = crate::test_utils::decode_sent_iq(&transport, 3).await;
        let heartbeat = &immediate.get().children().expect("heartbeat action")[0];
        assert_eq!(heartbeat.tag, "heartbeat");
        assert_eq!(
            heartbeat.attrs().optional_string("type").as_deref(),
            Some("waiting_room")
        );

        tokio::time::advance(Duration::from_secs(10)).await;
        wait_for_frames(&transport, 5).await;
        let scheduled = crate::test_utils::decode_sent_iq(&transport, 4).await;
        assert_eq!(
            scheduled.get().children().expect("heartbeat action")[0].tag,
            "heartbeat"
        );

        let admitted = NodeBuilder::new("group_update")
            .attr("call-id", "TEST-CALL-ID")
            .attr("call-creator", creator)
            .children([NodeBuilder::new("group_info")
                .attr("transaction-id", "8")
                .attr("connected-limit", "32")
                .attr("media", "video")
                .build()])
            .build();
        let update = wacore::stanza::group_call::parse_group_update(&admitted.as_node_ref())
            .expect("admitted group snapshot");
        assert_eq!(
            client.call_registry().apply_group_update(update),
            wacore::voip::GroupStateApply::Applied
        );
        assert_eq!(
            client.call_registry().phase("TEST-CALL-ID"),
            Some(CallPhase::Connecting)
        );
        let heartbeat_count = transport.sent_count();
        tokio::time::advance(Duration::from_secs(20)).await;
        tokio::task::yield_now().await;
        assert_eq!(
            transport.sent_count(),
            heartbeat_count,
            "admission must cancel the repeating heartbeat"
        );
        let generation = client
            .call_registry()
            .generation_of("TEST-CALL-ID")
            .expect("registered call-link generation");
        client
            .call_registry()
            .remove_if_current("TEST-CALL-ID", generation);
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn cancelling_call_link_request_removes_response_waiter() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let request_client = client.clone();
        let request = tokio::spawn(async move {
            request_client
                .voip()
                .create_call_link(CallLinkMedia::Audio)
                .await
        });
        let node = sent.await.expect("link_create request");
        let request_id = node
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        assert!(
            client.response_waiters_guard().contains_key(&request_id),
            "the request must register its ACK waiter before sending"
        );

        request.abort();
        assert!(
            request
                .await
                .expect_err("request should be cancelled")
                .is_cancelled()
        );
        tokio::task::yield_now().await;
        assert!(
            !client.response_waiters_guard().contains_key(&request_id),
            "cancelling a call-service request must not leak its waiter"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn cancelling_registered_call_link_join_removes_its_generation() {
        use wacore::handshake::NoiseCipher;

        struct BlockingTransport {
            started: async_channel::Sender<()>,
        }

        #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
        impl crate::transport::Transport for BlockingTransport {
            async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
                let _ = self.started.try_send(());
                futures::future::pending().await
            }

            async fn disconnect(&self) {}
        }

        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let join_sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let join_client = client.clone();
        let join = tokio::spawn(async move {
            join_client
                .voip()
                .join_call_link_with_audio(
                    "CANCELLED-CALL-LINK",
                    CallLinkMedia::Audio,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let request = join_sent.await.expect("link_join request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        let (started_tx, started_rx) = async_channel::bounded(1);
        let blocking_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(BlockingTransport {
                started: started_tx,
            }),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
            NoiseCipher::new(&[0u8; 32]).expect("valid key"),
        );
        *client.noise_socket.lock().await = Some(Arc::new(blocking_socket));
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("waiting_room")
                    .attr("call-id", "CANCELLED-CALL-ID")
                    .attr("call-creator", creator)
                    .attr("link-token", "CANCELLED-CALL-LINK")
                    .attr("media", "audio")
                    .attr("enabled", "1")
                    .attr("is_admin", "0")
                    .attr("transaction-id", "1")
                    .build()])
                .build(),
        )
        .await;
        started_rx.recv().await.expect("heartbeat send must start");
        assert!(
            client
                .call_registry()
                .generation_of("CANCELLED-CALL-ID")
                .is_some(),
            "the join must register before its heartbeat completes"
        );

        join.abort();
        assert!(
            join.await
                .expect_err("join should be cancelled")
                .is_cancelled()
        );
        tokio::task::yield_now().await;
        assert_eq!(
            client.call_registry().generation_of("CANCELLED-CALL-ID"),
            None,
            "cancelling after registration must reap only that generation"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn immediately_admitted_call_link_preserves_requested_token() {
        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let join_client = client.clone();
        let join = tokio::spawn(async move {
            join_client
                .voip()
                .join_call_link_with_audio(
                    "REQUESTED-CALL-LINK",
                    CallLinkMedia::Video,
                    AudioFormat::OPUS_16KHZ_60MS,
                )
                .await
        });
        let request = sent.await.expect("link_join request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", request_id.as_str())
                .children([NodeBuilder::new("group_info")
                    .attr("call-id", "ADMITTED-CALL-ID")
                    .attr("call-creator", creator)
                    .attr("transaction-id", "1")
                    .attr("connected-limit", "32")
                    .attr("media", "video")
                    .build()])
                .build(),
        )
        .await;

        let admitted = join.await.expect("join task").expect("join response");
        assert_eq!(admitted.token, "REQUESTED-CALL-LINK");
        assert!(!admitted.in_waiting_room);
        let generation = client
            .call_registry()
            .generation_of("ADMITTED-CALL-ID")
            .expect("registered admitted call");
        client
            .call_registry()
            .remove_if_current("ADMITTED-CALL-ID", generation);
    }
}
