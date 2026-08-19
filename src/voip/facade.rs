//! The incoming-call facade: `client.voip().accept(&incoming).audio(src, sink).start()` ->
//! [`CallHandle`]. It internalizes the preaccept -> offer-decrypt -> accept -> relay-connect ->
//! engine-spawn orchestration the example drove by hand, so a consumer never touches call-signaling
//! builders, the relay socket, the Signal session, or the sans-IO engine directly. Behind the `voip`
//! feature; reject/terminate stay feature-free in `super`.

use std::future::Future;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use bytes::Bytes;
use log::warn;
use wacore::message_processing::EncType;
use wacore::messages::MessageUtils;
use wacore::stanza::call::{
    AcceptParams, CAPABILITY_OFFER, CAPABILITY_PREACCEPT, CAPABILITY_STANDARD_OPUS_OFFER,
    CAPABILITY_STANDARD_OPUS_PREACCEPT, CAPABILITY_STANDARD_OPUS_VIDEO_OFFER,
    CAPABILITY_VIDEO_OFFER, OfferDeviceKey, OfferParams, TerminateParams, VideoStateParams,
    build_accept, build_offer, build_preaccept_with_capability, build_terminate, build_video_state,
    standard_opus_voip_settings,
};
use wacore::stanza::group_call::{
    GroupInviteOfferParams, InitialGroupOfferParams, build_group_invite_offer,
    build_initial_group_offer, parse_initial_group_call_ack,
};
use wacore::stats::UnkeyableDevice;
use wacore::types::call::{CallAction, IncomingCall, VideoState};
use wacore::types::group_call::{
    CallLinkMedia, GROUP_CALL_MAX_PARTICIPANTS, GROUP_CALL_MAX_REMOTE_PARTICIPANTS,
    GroupCallDevice, GroupCallParticipant, GroupCallUpdate, ScreenShareState,
};
use wacore::voip::relay_parse::RelayData;
use wacore::voip::transport::RelayTransportFactory;
use wacore::voip::{
    AudioConfig, AudioFormat, AudioRtpProfile, CallChannels, CallConfig, CallDirection, CallEngine,
    CallEvent, CallPhase, EncodedAudioFrame, GroupEngineConfig, VideoControl, VideoControlReceiver,
    VideoControlSender, VideoFrame, VideoUpgradeToken, video_control_channel,
};
use wacore_binary::{Jid, JidExt as _, Server};
use waproto::whatsapp as wa;
use zeroize::{Zeroize, Zeroizing};

use crate::client::{CallError, Client, ResponseWaiter};
use crate::voip::audio::{
    AudioSink, AudioSource, EncodedAudioSink, EncodedAudioSource, WA_FRAME_SAMPLES,
};
use crate::voip::driver::{RandTxIds, run_call_tokio};
use crate::voip::transport::RelayMediaChannelFactory;
use crate::voip::video::{VideoSink, VideoSource};

enum AudioEndpoints {
    Pcm {
        source: Arc<dyn AudioSource>,
        sink: Arc<dyn AudioSink>,
    },
    Encoded {
        format: AudioFormat,
        source: Arc<dyn EncodedAudioSource>,
        sink: Arc<dyn EncodedAudioSink>,
    },
}

impl AudioEndpoints {
    fn config(&self) -> AudioConfig {
        match self {
            Self::Pcm { .. } => AudioConfig::MLOW_PCM,
            Self::Encoded { format, .. } => AudioConfig::encoded(*format),
        }
    }

    fn signaling_rate(&self) -> u32 {
        self.config().format.signaling_rate
    }
}

macro_rules! impl_media_builder_methods {
    () => {
        /// Provide the microphone source and speaker sink for the call. Channel senders and
        /// receivers implementing the corresponding endpoint traits can be passed directly.
        pub fn audio<S, K>(mut self, source: S, sink: K) -> Self
        where
            S: AudioSource,
            K: AudioSink,
        {
            self.audio = Some(AudioEndpoints::Pcm {
                source: Arc::new(source),
                sink: Arc::new(sink),
            });
            self
        }

        /// Send and receive complete codec payloads instead of using the built-in PCM adapter.
        pub fn encoded_audio<S, K>(mut self, format: AudioFormat, source: S, sink: K) -> Self
        where
            S: EncodedAudioSource,
            K: EncodedAudioSink,
        {
            self.audio = Some(AudioEndpoints::Encoded {
                format,
                source: Arc::new(source),
                sink: Arc::new(sink),
            });
            self
        }

        /// Enable video media with an encoded H.264 source and sink.
        pub fn video<S, K>(mut self, source: S, sink: K) -> Self
        where
            S: VideoSource,
            K: VideoSink,
        {
            self.video = Some(VideoEndpoints::new(source, sink));
            self
        }
    };
}

/// Builder returned by [`Voip::accept`](crate::Voip::accept). Holds the offer and, once
/// [`audio`](Self::audio) is called, the source/sink; [`start`](Self::start) prepares the engine,
/// sends the callee's `<preaccept>` then `<accept>`, and starts the media plane. Borrows the
/// client so it can't outlive it.
pub struct AcceptCall<'a> {
    pub(crate) client: &'a Client,
    pub(crate) incoming: &'a IncomingCall,
    audio: Option<AudioEndpoints>,
    video: Option<VideoEndpoints>,
}

async fn wait_for_group_relay(
    registry: &wacore::voip::CallRegistry,
    call_id: &str,
    generation: u64,
) -> Result<GroupCallUpdate, CallError> {
    loop {
        let listener = registry
            .listen_group_update(call_id, generation)
            .ok_or(CallError::CallEndedDuringSetup)?;
        if let Some(update) = registry
            .group_state_if_current(call_id, generation)
            .and_then(|state| state.snapshot().cloned())
            && update.relay.is_some()
        {
            return Ok(update);
        }
        listener.await;
    }
}

impl<'a> AcceptCall<'a> {
    pub(crate) fn new(client: &'a Client, incoming: &'a IncomingCall) -> Self {
        Self {
            client,
            incoming,
            audio: None,
            video: None,
        }
    }

    impl_media_builder_methods!();

    /// Send `<preaccept>`, decrypt the callKey, send `<accept>`, then connect the relay and spawn and
    /// register the call driver. The returned [`CallHandle`] controls the live call. The
    /// relay path needs a live DTLS/SCTP endpoint; engine and stanza construction remain offline
    /// testable.
    // Lifecycle span over accept/start. PII-safe: the caller JID goes through `observe()`.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.voip.accept_start",
            level = "debug",
            skip_all,
            fields(peer = %self.incoming.from.observe()),
            err(Debug)
        )
    )]
    pub async fn start(mut self) -> Result<CallHandle, CallError> {
        // Take the audio endpoints out first; the offline setup (decrypt + config + addr) only
        // borrows `&self`, so move the fields before those borrows to avoid a partial-move clash.
        let audio = self.audio.take().ok_or(CallError::MissingAudio)?;
        let audio_config = audio.config();
        if let CallAction::Offer { audio, .. } = &self.incoming.action
            && !audio.is_empty()
            && !audio.iter().any(|codec| {
                codec.enc.eq_ignore_ascii_case("opus")
                    && codec.rate == audio_config.format.signaling_rate
            })
        {
            return Err(CallError::AudioFormatNotOffered(
                audio_config.format.signaling_rate,
            ));
        }
        let CallAction::Offer {
            call_id,
            call_creator,
            is_video: offered_video,
            ..
        } = &self.incoming.action
        else {
            return Err(CallError::NotAnOffer);
        };
        if call_id.is_empty() {
            return Err(CallError::EmptyCallId);
        }
        let video = self.video.take();
        let peer_invite_device = self
            .incoming
            .media()
            .and_then(|media| media.peer_device.clone());
        let has_video = video.is_some();
        if video.as_ref().is_some_and(|v| !v.has_valid_timing()) {
            return Err(CallError::Media(
                "video RTP timestamp stride must be non-zero",
            ));
        }
        if video.is_some() && !offered_video {
            return Err(CallError::VideoNotOffered);
        }
        let registry = self.client.call_registry();
        let group_generation = if self.incoming.group.is_some() {
            let generation = self
                .incoming
                .ringing_generation()
                .ok_or(CallError::CallEndedDuringSetup)?;
            if registry.ringing_group_generation(call_id, call_creator) != Some(generation) {
                return Err(CallError::CallEndedDuringSetup);
            }
            Some(generation)
        } else {
            if registry.is_group_call(call_id) {
                // The application retained a direct offer past a same-id group replacement. It
                // must not borrow that newer roster or replace its eagerly registered generation.
                return Err(CallError::CallEndedDuringSetup);
            }
            None
        };
        let mut group = match group_generation {
            Some(generation) => Some(
                registry
                    .group_state_if_current(call_id, generation)
                    .and_then(|state| state.snapshot().cloned())
                    .ok_or(CallError::CallEndedDuringSetup)?,
            ),
            None => None,
        };
        if self.incoming.group.is_some()
            && self.incoming.media().is_none()
            && group
                .as_ref()
                .and_then(|update| update.relay.as_ref())
                .is_none()
            && let Some(generation) = group_generation
        {
            group = Some(
                match wacore::runtime::timeout(
                    &*self.client.runtime,
                    OFFER_ACK_RELAY_TIMEOUT,
                    wait_for_group_relay(&registry, call_id, generation),
                )
                .await
                {
                    Ok(result) => result?,
                    Err(_) => return Err(CallError::ResponseTimeout),
                },
            );
        }
        if group
            .as_ref()
            .is_some_and(|group| (group.media == "video") != *offered_video)
        {
            return Err(CallError::Response(
                "group offer signaling and roster media modes differ".to_string(),
            ));
        }
        let is_group = group.is_some();
        if self.incoming.media().is_none()
            && group
                .as_ref()
                .and_then(|group| group.relay.as_ref())
                .is_none()
        {
            return Err(CallError::Media("offer carried no media block"));
        }
        let peer_jid = if is_group {
            Jid::new(call_id, Server::Call)
        } else {
            self.incoming.from.clone()
        };
        let mut session =
            wacore::voip::CallSession::new_incoming(call_id, peer_jid, call_creator.clone());
        session.audio_format = Some(audio_config.format);
        session.is_video = has_video;
        session.group = group.clone();
        // Register BEFORE the decrypt await. A peer <terminate> can now reap this generation during
        // setup, instead of falling through terminate_call as an unknown call and letting us accept
        // a call that has already ended.
        let mut registration = if let Some(generation) = group_generation {
            let _answer_transition = self.client.lock_answer_transition(call_id).await;
            if !registry.promote_ringing_group_if_current(session, generation) {
                return Err(CallError::CallEndedDuringSetup);
            }
            RegisteredCall::from_existing(self.client, call_id, generation)?
        } else {
            RegisteredCall::new(self.client, session).await
        };
        let mut teardown = AnswerTeardown::new(self.client, &registration);
        let preaccept_id = self.client.generate_request_id();
        let accept_id = self.client.generate_request_id();
        let (preaccept, accept) = build_answer_signaling(
            self.incoming,
            audio_config.format,
            has_video,
            is_group,
            &preaccept_id,
            &accept_id,
        )?;
        let (engine, built_call_id, addr) = send_preaccept_then_prepare(
            self.client,
            &registration,
            &mut teardown,
            preaccept,
            self.build_engine(has_video, audio_config, group),
        )
        .await?;
        debug_assert_eq!(built_call_id, registration.call_id);
        // The decrypt above may await on the network (prekey fetch). If the connection dropped
        // meanwhile, cleanup_connection_state reaped the pending registration. Preserve the
        // connection-specific error before checking the generation.
        if !self.client.is_connected() {
            return Err(CallError::Connect(ERR_DISCONNECTED_DURING_SETUP.into()));
        }
        registration.ensure_current()?;
        let factory = RelayMediaChannelFactory::new(addr, self.client.runtime.clone());
        // Final acceptance waits until media setup succeeded and the registered generation is still
        // current; only then may the caller apply the participant keys and enter the call.
        send_answer_node(self.client, &registration, &mut teardown, accept).await?;
        let handle = spawn_answered_call(
            self.client,
            &mut registration,
            teardown,
            engine,
            &factory,
            audio,
            video,
        )
        .await?;
        if !is_group && let Some(own_lid) = self.client.lid() {
            self.client.call_registry().set_group_invite_self_device(
                &handle.call_id,
                handle.generation,
                GroupCallDevice::new(own_lid)
                    .with_capability(1, offer_capability(has_video, audio_config.format)),
            );
            if let Some(peer_device) = peer_invite_device {
                self.client.call_registry().set_group_invite_peer_device(
                    &handle.call_id,
                    handle.generation,
                    peer_device,
                );
            }
        }
        Ok(handle)
    }

    /// Build the [`CallEngine`] from the offer: decrypt the callKey over the Signal session, then
    /// assemble the incoming-call config from the parsed relay. No network I/O beyond the Signal
    /// session the decrypt needs.
    async fn build_engine(
        &self,
        enable_video: bool,
        audio: AudioConfig,
        group: Option<GroupCallUpdate>,
    ) -> Result<(CallEngine, String, SocketAddr), CallError> {
        let CallAction::Offer {
            call_id,
            call_creator,
            ..
        } = &self.incoming.action
        else {
            return Err(CallError::NotAnOffer);
        };
        if call_id.is_empty() {
            return Err(CallError::EmptyCallId);
        }

        // Our own device LID: used both to pick the callKey enc for THIS device (a multi-device
        // offer lists one per `<destination><to jid>`) and as the send-side SRTP participant id.
        let own_lid = self.client.lid().ok_or(CallError::Media("no own LID"))?;
        if let Some(group) = group.as_ref()
            && let Some(relay) = group.relay.as_ref()
        {
            let self_lid = own_lid.to_string();
            let mut config = CallConfig::for_group(
                CallDirection::Incoming,
                call_id,
                &self_lid,
                &call_creator.to_string(),
                relay,
            )
            .map_err(|error| CallError::Setup(error.to_string()))?;
            config.audio = audio;
            config.enable_video = enable_video;
            let addr = socket_addr_from_config(&config)?;
            let mut engine = CallEngine::new(config, Box::new(RandTxIds))
                .map_err(|error| CallError::Setup(error.to_string()))?;
            engine
                .configure_group(GroupEngineConfig {
                    call_creator: call_creator.clone(),
                    self_jid: own_lid,
                    initial_update: group.clone(),
                    direct_peer: None,
                })
                .map_err(|error| CallError::Setup(error.to_string()))?;
            return Ok((engine, call_id.clone(), addr));
        }

        let media = self
            .incoming
            .media()
            .ok_or(CallError::Media("offer carried no media block"))?;
        let enc = media
            .enc_for(Some(&own_lid))
            .ok_or(CallError::Media("offer carried no callKey for this device"))?;

        let enc_type =
            EncType::from_wire(&enc.enc_type).ok_or(CallError::Media("unknown enc type"))?;
        let plaintext = self
            .client
            .signal()
            .decrypt_message(call_creator, enc_type, &enc.ciphertext)
            .await
            .map_err(|e| CallError::Decrypt(e.to_string()))?;
        let unpadded = MessageUtils::unpad_message_ref(&plaintext, enc.version)
            .map_err(|e| CallError::Decrypt(e.to_string()))?;
        let msg = waproto::codec::message_decode(unpadded)
            .map_err(|e| CallError::Decrypt(format!("decode call message: {e}")))?;
        let call_key = msg
            .call
            .into_option()
            .and_then(|c| c.call_key)
            .ok_or(CallError::Media("offer carried no callKey"))?;

        // E2E SRTP keys derive from the participant LIDs: ours for send, the peer's for recv. Each
        // crypto layer normalizes the JID with its own rule, so pass the raw JIDs through.
        let self_lid = own_lid.to_string();
        let peer_lid = call_creator.to_string();
        let relay = media
            .relay
            .as_ref()
            .ok_or(CallError::Media("offer carried no <relay>"))?;

        let mut config = CallConfig::for_incoming(call_id, &self_lid, &peer_lid, call_key, relay)
            .map_err(|e| CallError::Setup(e.to_string()))?;
        config.audio = audio;
        config.enable_video = enable_video;
        // Read the dial addr off the config before CallEngine::new consumes it (no second relay walk).
        let addr = socket_addr_from_config(&config)?;
        let engine = CallEngine::new(config, Box::new(RandTxIds))
            .map_err(|e| CallError::Setup(e.to_string()))?;
        Ok((engine, call_id.clone(), addr))
    }
}

/// Builder returned by [`Voip::call`](crate::Voip::call). Mirrors [`AcceptCall`]:
/// holds the peer and, once [`audio`](Self::audio) is called, the source/sink, then [`start`](Self::start)
/// generates the callKey, encrypts it per peer device, sends the `<offer>`, and registers the call.
/// Borrows the client so it can't outlive it.
pub struct OutgoingCall<'a> {
    pub(crate) client: &'a Client,
    pub(crate) peer: &'a Jid,
    audio: Option<AudioEndpoints>,
    video: Option<VideoEndpoints>,
}

impl<'a> OutgoingCall<'a> {
    pub(crate) fn new(client: &'a Client, peer: &'a Jid) -> Self {
        Self {
            client,
            peer,
            audio: None,
            video: None,
        }
    }

    impl_media_builder_methods!();

    /// Resolve a PN callee to its LID, querying the server when the local cache misses so a first-ever
    /// call to a never-messaged contact still works. The cache-only `get_current_lid` returns `None`
    /// for an unknown PN; a usync device-list query side-effect-learns the LID↔PN mapping (warming the
    /// cache synchronously), so we retry the cache after it. A persisting miss means the server has no
    /// LID for this PN, which is unrecoverable for key derivation, so reject.
    async fn resolve_callee_lid(
        &self,
        pn: &Jid,
    ) -> Result<wacore_binary::CompactString, CallError> {
        if let Some(lid) = self.client.lid_pn_cache.get_current_lid(&pn.user).await {
            return Ok(lid);
        }
        // The query's device records are not used here; it is the LID-PN learning side effect we want.
        self.client
            .signal()
            .get_user_devices(std::slice::from_ref(pn))
            .await
            .map_err(|e| CallError::Setup(e.to_string()))?;
        self.client
            .lid_pn_cache
            .get_current_lid(&pn.user)
            .await
            .ok_or(CallError::Media(
                "no known LID for the PN callee; cannot derive media keys",
            ))
    }

    /// Generate the callKey, encrypt it per peer device, send the `<offer>`, register the outgoing
    /// call, and return a dormant [`CallHandle`]. The initiator's relay is NOT in the offer; it
    /// arrives in the server's `<ack type=offer>` reply, so the media engine attaches later via
    /// `attach_outgoing_relay`. Everything here (device resolution, encrypt, offer build + send) is
    /// offline testable; the relay attach + media connect need a real server.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.voip.call_start",
            level = "debug",
            skip_all,
            fields(peer = %self.peer.observe()),
            err(Debug)
        )
    )]
    pub async fn start(mut self) -> Result<CallHandle, CallError> {
        let audio = self.audio.take().ok_or(CallError::MissingAudio)?;
        let video = self.video.take();
        if video.as_ref().is_some_and(|v| !v.has_valid_timing()) {
            return Err(CallError::Media(
                "video RTP timestamp stride must be non-zero",
            ));
        }

        // WA Web `_e()`: call-id is "00" + 15 random bytes as lowercase hex (32 hex chars total).
        let call_id = gen_call_id();

        // Our own LID is the send-side SRTP participant id; required for E2E key derivation.
        let own_lid = self.client.lid().ok_or(CallError::Media("no own LID"))?;

        // The media keys (SFrame/SRTP) derive from the peer's LID, so a PN callee must be resolved to
        // its LID first; without a known LID we would derive non-matching keys, so reject. The offer
        // is then LID-addressed end to end.
        let peer = match self.peer.server {
            Server::Lid => self.peer.clone(),
            _ => {
                let lid_user = self.resolve_callee_lid(self.peer).await?;
                Jid::new(lid_user.as_str(), Server::Lid)
            }
        };
        let call_creator = own_lid.clone();

        // Resolve the peer's devices and make sure we hold a Signal session for each. Drop hosted
        // (Cloud-API) companions, same as the DM fan-out: the rest of the client never establishes
        // sessions for them, so encrypting a callKey to one would fail or emit an unusable <to>.
        let fetched = self
            .client
            .signal()
            .get_user_devices(std::slice::from_ref(&peer))
            .await
            .map_err(|e| CallError::Setup(e.to_string()))?;
        let mut devices = drop_hosted_devices(fetched);
        if devices.is_empty() {
            return Err(CallError::NoDevices);
        }
        // The FULL callee device set the server will ring, captured BEFORE the no-account pkmsg filter
        // (or a per-device encrypt skip) shrinks it to the encryptable survivors. The server rings
        // EVERY device of the callee -- including ones we don't encrypt a callKey for (e.g. the primary
        // phone we drop as pkmsg without an ADV account) -- so the sibling-dismiss target set must be
        // this full set, or an un-dismissed device keeps ringing and times the call out. Also keeps the
        // addressed `<destination>` offer shape when only one device survives encryption.
        let ring_devices = devices.clone();

        // A device whose encrypt would emit pkmsg needs a <device-identity> from our ADV account.
        // Without the account we can't supply it, so offer ONLY to devices that encrypt as plain msg
        // and drop the rest before assert_sessions/encrypt, rather than failing the whole call
        // (consistent with the per-device encrypt skip below). This reuses the send path's pkmsg
        // pre-flight, so a session-present-but-unacked device is correctly treated as pkmsg, not msg
        // (a bare contains_session check would wrongly keep it and let a no-identity pkmsg <enc> out).
        if self
            .client
            .persistence_manager()
            .get_device_snapshot()
            .account
            .is_none()
        {
            // would_emit_pkmsg does a load_session + store_session round-trip (a redundant write-back
            // in the shared pre-flight); hold the per-device session locks place_call's encrypt also
            // takes, so it can't clobber a concurrent send advancing the same session.
            let lock_jids = self.client.build_session_lock_keys(&devices).await;
            let session_guards = self.client.session_guards_for(&lock_jids).await;

            let mut would_pkmsg = Vec::with_capacity(devices.len());
            for d in &devices {
                would_pkmsg.push(
                    self.client
                        .would_emit_pkmsg(d)
                        .await
                        .map_err(|e| CallError::Setup(e.to_string()))?,
                );
            }
            drop(session_guards);
            devices = keep_non_pkmsg_devices(devices, &would_pkmsg)?;
        }

        self.client
            .signal()
            .assert_sessions(&devices)
            .await
            .map_err(|e| CallError::Setup(e.to_string()))?;

        place_call(
            self.client,
            call_id,
            &peer,
            &call_creator,
            &own_lid,
            &devices,
            &ring_devices,
            audio,
            video,
        )
        .await
    }
}

/// Builder for a native ad-hoc or group-bound call with at least two remote users.
pub struct OutgoingGroupCall<'a> {
    client: &'a Client,
    targets: &'a [Jid],
    group_jid: Option<Jid>,
    exclude_local_user: bool,
    audio: Option<AudioEndpoints>,
    video: Option<VideoEndpoints>,
}

impl<'a> OutgoingGroupCall<'a> {
    pub(crate) fn new(client: &'a Client, targets: &'a [Jid]) -> Self {
        Self {
            client,
            targets,
            group_jid: None,
            exclude_local_user: false,
            audio: None,
            video: None,
        }
    }

    /// Bind the call to an existing WhatsApp group.
    pub fn group(mut self, group_jid: Jid) -> Self {
        self.group_jid = Some(group_jid);
        self
    }

    impl_media_builder_methods!();

    /// Resolve all selected users/devices, send one call-scoped offer, wait for the authoritative
    /// relay snapshot, install a requested shared epoch, and start the group media driver.
    pub async fn start(mut self) -> Result<CallHandle, CallError> {
        if self
            .group_jid
            .as_ref()
            .is_some_and(|jid| jid.server != Server::Group || jid.user.is_empty())
        {
            return Err(CallError::Setup(
                "group-bound call requires a valid group JID".to_string(),
            ));
        }
        let audio = self.audio.take().ok_or(CallError::MissingAudio)?;
        let video = self.video.take();
        if video
            .as_ref()
            .is_some_and(|video| !video.has_valid_timing())
        {
            return Err(CallError::Media(
                "video RTP timestamp stride must be non-zero",
            ));
        }
        let own_lid = self.client.lid().ok_or(CallError::Media("no own LID"))?;
        let own_user = own_lid.to_non_ad();
        let own_pn = self.client.pn().map(|jid| jid.to_non_ad());
        let mut candidates = Vec::with_capacity(self.targets.len());
        for target in self.targets {
            let target = target.to_non_ad();
            if target == own_user || own_pn.as_ref() == Some(&target) {
                if self.exclude_local_user {
                    continue;
                }
                return Err(CallError::Setup(
                    "group call target cannot be the local user".to_string(),
                ));
            }
            candidates.push(target);
        }
        if candidates.len() > GROUP_CALL_MAX_REMOTE_PARTICIPANTS {
            return Err(CallError::Setup(format!(
                "group call requires 2..={GROUP_CALL_MAX_REMOTE_PARTICIPANTS} remote users"
            )));
        }
        let resolved = futures::future::join_all(candidates.into_iter().map(|target| async move {
            match target.server {
                Server::Lid => Some(target),
                _ => self
                    .client
                    .resolve_recipient_to_lid(&target)
                    .await
                    .map(|jid| jid.to_non_ad()),
            }
        }))
        .await;
        let mut seen = std::collections::HashSet::new();
        let mut targets = Vec::with_capacity(resolved.len());
        for resolved in resolved {
            let resolved = resolved.ok_or(CallError::NoDevices)?;
            if resolved == own_user {
                if self.exclude_local_user {
                    continue;
                }
                return Err(CallError::Setup(
                    "group call target cannot be the local user".to_string(),
                ));
            }
            if !seen.insert(resolved.clone()) {
                return Err(CallError::Setup(
                    "group call targets must be unique".to_string(),
                ));
            }
            targets.push(resolved);
            if targets.len() > GROUP_CALL_MAX_REMOTE_PARTICIPANTS {
                return Err(CallError::Setup(format!(
                    "group call requires 2..={GROUP_CALL_MAX_REMOTE_PARTICIPANTS} remote users"
                )));
            }
        }
        if !(2..=GROUP_CALL_MAX_REMOTE_PARTICIPANTS).contains(&targets.len()) {
            return Err(CallError::Setup(format!(
                "group call requires 2..={GROUP_CALL_MAX_REMOTE_PARTICIPANTS} remote users"
            )));
        }

        let devices = drop_hosted_devices(
            self.client
                .signal()
                .get_user_devices(&targets)
                .await
                .map_err(|error| CallError::Setup(error.to_string()))?,
        );
        let mut participants = Vec::with_capacity(targets.len() + 1);
        let self_device = GroupCallDevice::new(own_lid.clone())
            .with_capability(1, offer_capability(video.is_some(), audio.config().format));
        participants.push(GroupCallParticipant::new(own_user, vec![self_device]));
        for target in &targets {
            let target_devices = devices
                .iter()
                .filter(|device| device.to_non_ad() == *target)
                .cloned()
                .collect::<Vec<_>>();
            if target_devices.is_empty() {
                return Err(CallError::NoDevices);
            }
            participants.push(GroupCallParticipant::new(
                target.clone(),
                target_devices
                    .into_iter()
                    .map(GroupCallDevice::new)
                    .collect(),
            ));
        }

        let call_id = gen_call_id();
        let request_id = self.client.generate_request_id();
        let offer = build_initial_group_offer(&InitialGroupOfferParams {
            call_id: &call_id,
            id: &request_id,
            call_creator: &own_lid,
            group_jid: self.group_jid.as_ref(),
            participants: &participants,
            audio_rate: audio.config().format.signaling_rate,
            video: video.is_some(),
        })
        .map_err(|error| CallError::Response(error.to_string()))?;
        let (tx, response) = futures::channel::oneshot::channel();
        let cleanup_generation = self
            .client
            .response_waiters_guard()
            .try_insert_guarded(request_id.clone(), ResponseWaiter::Iq(tx))
            .ok_or_else(|| CallError::Response("duplicate group-offer request id".to_string()))?;
        let _waiter_guard = crate::request::ResponseWaiterGuard::new(
            self.client.response_waiters.clone(),
            request_id.clone(),
            cleanup_generation,
        );
        let mut session = wacore::voip::CallSession::new_outgoing(
            &call_id,
            Jid::new(&call_id, Server::Call),
            own_lid.clone(),
        );
        session.audio_format = Some(audio.config().format);
        session.is_video = video.is_some();
        let _ = session.transition_to(CallPhase::Calling);
        // Register before the offer reaches the wire. A creator-authenticated group update can
        // overtake the ACK on the independent call-stanza path, and must have a generation where
        // its roster and first epoch can be retained.
        let mut registration = RegisteredCall::new_group(self.client, session).await;
        // Sending is delivery-ambiguous: cancellation can drop this future after the server has
        // accepted the bytes but before send_node returns. Arm cleanup before crossing that await.
        let mut teardown = GroupOfferTeardown::new(self.client, &mut registration);
        if let Err(error) = self.client.send_node(offer).await {
            return Err(error.into());
        }
        let response = match wacore::runtime::timeout(
            &*self.client.runtime,
            OFFER_ACK_RELAY_TIMEOUT,
            response,
        )
        .await
        {
            Ok(Ok(response)) => response,
            Ok(Err(_)) => {
                return Err(CallError::Response(
                    "group offer response channel closed".to_string(),
                ));
            }
            Err(_) => return Err(CallError::ResponseTimeout),
        };
        let ack_update = parse_initial_group_call_ack(response.get())
            .map_err(|error| CallError::Response(error.to_string()))?
            .ok_or_else(|| {
                CallError::Response("group offer ack has no group snapshot".to_string())
            })?;
        if ack_update.call_id != call_id || ack_update.call_creator != own_lid {
            return Err(CallError::Response(
                "group offer ack identity mismatch".to_string(),
            ));
        }
        ensure_group_offer_media(&ack_update, video.is_some())?;
        let registry = self.client.call_registry();
        let transition_lock = registry
            .group_transition_lock(&call_id, registration.generation)
            .ok_or(CallError::CallEndedDuringSetup)?;
        let transition_guard = transition_lock.lock().await;
        registration.ensure_current()?;
        let applied =
            registry.apply_group_update_if_current(ack_update.clone(), registration.generation);
        if !matches!(
            applied,
            wacore::voip::GroupStateApply::Applied | wacore::voip::GroupStateApply::Stale
        ) {
            return Err(CallError::Response(
                "group offer ack snapshot was rejected".to_string(),
            ));
        }
        let update = registry
            .group_state_if_current(&call_id, registration.generation)
            .and_then(|state| state.snapshot().cloned())
            .ok_or(CallError::CallEndedDuringSetup)?;
        ensure_group_offer_media(&update, video.is_some())?;
        if !registry.transition_if_current(&call_id, registration.generation, CallPhase::Connecting)
        {
            return Err(CallError::CallEndedDuringSetup);
        }
        let relay = update
            .relay
            .as_ref()
            .or(ack_update.relay.as_ref())
            .ok_or(CallError::Media("group offer ack has no relay"))?;
        let mut config = CallConfig::for_group(
            CallDirection::Outgoing,
            &call_id,
            &own_lid.to_string(),
            &own_lid.to_string(),
            relay,
        )
        .map_err(|error| CallError::Setup(error.to_string()))?;
        config.audio = audio.config();
        config.enable_video = video.is_some();
        let addr = socket_addr_from_config(&config)?;
        let mut engine = CallEngine::new(config, Box::new(RandTxIds))
            .map_err(|error| CallError::Setup(error.to_string()))?;
        engine
            .configure_group(GroupEngineConfig {
                call_creator: own_lid.clone(),
                self_jid: own_lid.clone(),
                initial_update: update.clone(),
                direct_peer: None,
            })
            .map_err(|error| CallError::Setup(error.to_string()))?;
        // A newer pre-ACK update may have overtaken this ACK without requesting its own rekey.
        // Honor the ACK request unless the serialized signaling handler retained an equal/newer
        // epoch; fan out against the current roster so newly arrived participants receive it too.
        let retained_epoch =
            registry.pending_group_epoch_transaction_if_current(&call_id, registration.generation);
        if let Some(rekey_update) = group_offer_epoch_update(&ack_update, &update, retained_epoch) {
            fanout_group_epoch(self.client, rekey_update)
                .await?
                .commit(|epoch| {
                    engine
                        .apply_group_raw_epoch(rekey_update.transaction_id, epoch)
                        .map_err(|error| CallError::Setup(error.to_string()))
                })?;
        }
        drop(transition_guard);

        if !self.client.is_connected() {
            return Err(CallError::Connect(ERR_DISCONNECTED_DURING_SETUP.into()));
        }
        let factory = RelayMediaChannelFactory::new(addr, self.client.runtime.clone());
        let handle =
            spawn_registered_call(self.client, &registration, engine, &factory, audio, video)
                .await?;
        registration.disarm();
        teardown.disarm();
        Ok(handle)
    }
}

/// Builder for a call to the current remote roster of an existing WhatsApp group.
pub struct GroupBoundCall<'a> {
    client: &'a Client,
    group_jid: &'a Jid,
    audio: Option<AudioEndpoints>,
    video: Option<VideoEndpoints>,
}

impl<'a> GroupBoundCall<'a> {
    pub(crate) fn new(client: &'a Client, group_jid: &'a Jid) -> Self {
        Self {
            client,
            group_jid,
            audio: None,
            video: None,
        }
    }

    impl_media_builder_methods!();

    /// Resolve the current group roster, exclude this account, and place one group-bound call.
    pub async fn start(mut self) -> Result<CallHandle, CallError> {
        if self.audio.is_none() {
            return Err(CallError::MissingAudio);
        }
        if self.group_jid.server != Server::Group || self.group_jid.user.is_empty() {
            return Err(CallError::Setup(
                "group-bound call requires a valid group JID".to_string(),
            ));
        }
        let info = self
            .client
            .groups()
            .query_info(self.group_jid)
            .await
            .map_err(|error| CallError::Setup(error.to_string()))?;
        OutgoingGroupCall {
            client: self.client,
            targets: &info.participants,
            group_jid: Some(self.group_jid.to_non_ad()),
            exclude_local_user: true,
            audio: self.audio.take(),
            video: self.video.take(),
        }
        .start()
        .await
    }
}

/// Builder for joining reusable call-link group media.
///
/// A pending admission keeps its captured heartbeat alive while `start()` waits. Once an
/// authoritative group snapshot with relay material arrives, the same registered generation is
/// promoted into the media driver without losing roster or key updates received in the meantime.
pub struct CallLinkCall<'a> {
    client: &'a Client,
    token_or_url: &'a str,
    media: CallLinkMedia,
    audio: Option<AudioEndpoints>,
    video: Option<VideoEndpoints>,
}

impl<'a> CallLinkCall<'a> {
    pub(crate) fn new(client: &'a Client, token_or_url: &'a str, media: CallLinkMedia) -> Self {
        Self {
            client,
            token_or_url,
            media,
            audio: None,
            video: None,
        }
    }

    impl_media_builder_methods!();

    /// Join, wait for admission when required, and attach the shared relay media engine.
    pub async fn start(mut self) -> Result<CallHandle, CallError> {
        let audio = self.audio.take().ok_or(CallError::MissingAudio)?;
        if audio.signaling_rate() != 16_000 {
            return Err(CallError::AudioFormatNotOffered(audio.signaling_rate()));
        }
        let video = self.video.take();
        if video
            .as_ref()
            .is_some_and(|video| !video.has_valid_timing())
        {
            return Err(CallError::Media(
                "video RTP timestamp stride must be non-zero",
            ));
        }
        match (self.media, video.is_some()) {
            (CallLinkMedia::Video, false) => {
                return Err(CallError::Media(
                    "video call link requires video source and sink",
                ));
            }
            (CallLinkMedia::Audio, true) => {
                return Err(CallError::Media(
                    "audio call link cannot attach video endpoints",
                ));
            }
            _ => {}
        }

        let join_registration = self
            .client
            .voip()
            .join_call_link_registration_with_audio(
                self.token_or_url,
                self.media,
                audio.config().format,
            )
            .await?;
        let join = join_registration.join;
        let generation = join_registration.generation;
        let registry = self.client.call_registry();
        let mut registration =
            RegisteredCall::from_existing(self.client, &join.call_id, generation)?;
        // Transfer cleanup ownership before admission can race with cancellation. The teardown
        // atomically inspects the claimed generation's phase: a waiting-room cancellation is local,
        // while an already admitted endpoint must also receive a terminal stanza.
        let mut teardown = GroupOfferTeardown::new_call_link(self.client, &mut registration);

        let update = loop {
            let listener = registry
                .listen_group_update(&join.call_id, generation)
                .ok_or(CallError::Media("call-link ended before admission"))?;
            if let Some(update) = registry
                .group_state_if_current(&join.call_id, generation)
                .and_then(|state| state.snapshot().cloned())
            {
                ensure_call_link_admitted_snapshot(&update, &join.call_creator, self.media)?;
                if update.relay.is_some() {
                    break update;
                }
                if registry.phase_if_current(&join.call_id, generation)
                    == Some(CallPhase::Connecting)
                {
                    let update = match wacore::runtime::timeout(
                        &*self.client.runtime,
                        OFFER_ACK_RELAY_TIMEOUT,
                        wait_for_group_relay(&registry, &join.call_id, generation),
                    )
                    .await
                    {
                        Ok(result) => result?,
                        Err(_) => return Err(CallError::ResponseTimeout),
                    };
                    ensure_call_link_admitted_snapshot(&update, &join.call_creator, self.media)?;
                    break update;
                }
            }
            listener.await;
        };
        let relay = update
            .relay
            .as_ref()
            .ok_or(CallError::Media("call-link group snapshot has no relay"))?;
        let own_lid = self.client.lid().ok_or(CallError::Media("no own LID"))?;
        let mut config = CallConfig::for_group(
            CallDirection::Outgoing,
            &join.call_id,
            &own_lid.to_string(),
            &join.call_creator.to_string(),
            relay,
        )
        .map_err(|error| CallError::Setup(error.to_string()))?;
        config.audio = audio.config();
        config.enable_video = video.is_some();
        let addr = socket_addr_from_config(&config)?;
        let mut engine = CallEngine::new(config, Box::new(RandTxIds))
            .map_err(|error| CallError::Setup(error.to_string()))?;
        engine
            .configure_group(GroupEngineConfig {
                call_creator: join.call_creator.clone(),
                self_jid: own_lid,
                initial_update: update.clone(),
                direct_peer: None,
            })
            .map_err(|error| CallError::Setup(error.to_string()))?;

        if !self.client.is_connected() {
            return Err(CallError::Connect(ERR_DISCONNECTED_DURING_SETUP.into()));
        }
        let factory = RelayMediaChannelFactory::new(addr, self.client.runtime.clone());
        let handle =
            spawn_registered_call(self.client, &registration, engine, &factory, audio, video)
                .await?;
        registration.disarm();
        teardown.disarm();
        Ok(handle)
    }
}

/// The video source/sink pair a builder's `.video()` provided.
struct VideoEndpoints {
    source: Arc<dyn VideoSource>,
    sink: Arc<dyn VideoSink>,
}

impl VideoEndpoints {
    fn new<S: VideoSource, K: VideoSink>(source: S, sink: K) -> Self {
        Self {
            source: Arc::new(source),
            sink: Arc::new(sink),
        }
    }

    fn has_valid_timing(&self) -> bool {
        self.source.rtp_timestamp_stride() != 0
    }
}

/// Drop hosted (Cloud-API) companions from the offer's device set: the client never establishes
/// Signal sessions for them, so a callKey `<enc>` to one would fail or emit an unusable `<to>`.
fn drop_hosted_devices(mut devices: Vec<Jid>) -> Vec<Jid> {
    devices.retain(|d| !d.is_hosted());
    devices
}

/// Keep only the devices whose encrypt stays a plain `msg` (not pkmsg), given `would_pkmsg` flags
/// parallel to `devices`. Used when we hold no ADV account and so can't attach the `<device-identity>`
/// a pkmsg `<enc>` requires. Errors `MissingDeviceIdentity` when every device would be pkmsg (nothing
/// left to offer), rather than letting an unvalidatable pkmsg out or failing the whole call silently.
fn keep_non_pkmsg_devices(devices: Vec<Jid>, would_pkmsg: &[bool]) -> Result<Vec<Jid>, CallError> {
    debug_assert_eq!(devices.len(), would_pkmsg.len());
    let kept: Vec<Jid> = devices
        .into_iter()
        .zip(would_pkmsg)
        .filter_map(|(d, &pkmsg)| (!pkmsg).then_some(d))
        .collect();
    if kept.is_empty() {
        return Err(CallError::MissingDeviceIdentity);
    }
    Ok(kept)
}

pub(crate) fn offer_capability(video: bool, audio: AudioFormat) -> &'static [u8] {
    let standard_opus = matches!(audio.rtp_profile, AudioRtpProfile::StandardOpus);
    match (video, standard_opus) {
        (true, true) => &CAPABILITY_STANDARD_OPUS_VIDEO_OFFER,
        (false, true) => &CAPABILITY_STANDARD_OPUS_OFFER,
        (true, false) => &CAPABILITY_VIDEO_OFFER,
        (false, false) => &CAPABILITY_OFFER,
    }
}

fn ensure_group_offer_media(
    update: &GroupCallUpdate,
    requested_video: bool,
) -> Result<(), CallError> {
    let requested = if requested_video { "video" } else { "audio" };
    if update.media == requested {
        Ok(())
    } else {
        Err(CallError::Response(
            "group offer ack changed the requested media mode".to_string(),
        ))
    }
}

fn group_offer_epoch_update<'a>(
    ack_update: &GroupCallUpdate,
    current_update: &'a GroupCallUpdate,
    retained_epoch: Option<u32>,
) -> Option<&'a GroupCallUpdate> {
    (ack_update.rekey_requested
        && retained_epoch.is_none_or(|transaction| transaction < ack_update.transaction_id))
    .then_some(current_update)
}

fn ensure_call_link_admitted_media(
    update: &GroupCallUpdate,
    requested: CallLinkMedia,
) -> Result<(), CallError> {
    if update.media == requested.as_str() {
        Ok(())
    } else {
        Err(CallError::Response(
            "call-link admitted snapshot changed the requested media mode".to_string(),
        ))
    }
}

fn ensure_call_link_admitted_snapshot(
    update: &GroupCallUpdate,
    expected_creator: &Jid,
    requested: CallLinkMedia,
) -> Result<(), CallError> {
    if update.call_creator != *expected_creator {
        return Err(CallError::Response(
            "call-link admitted snapshot changed call identity".to_string(),
        ));
    }
    ensure_call_link_admitted_media(update, requested)
}

fn answer_preaccept_capability(video: bool, audio: AudioFormat) -> &'static [u8] {
    let standard_opus = matches!(audio.rtp_profile, AudioRtpProfile::StandardOpus);
    match (video, standard_opus) {
        (true, true) => &CAPABILITY_STANDARD_OPUS_OFFER,
        (false, true) => &CAPABILITY_STANDARD_OPUS_PREACCEPT,
        (true, false) => &CAPABILITY_OFFER,
        (false, false) => &CAPABILITY_PREACCEPT,
    }
}

fn build_answer_signaling(
    incoming: &IncomingCall,
    audio: AudioFormat,
    video: bool,
    is_group: bool,
    preaccept_id: &str,
    accept_id: &str,
) -> Result<(wacore_binary::Node, wacore_binary::Node), CallError> {
    let CallAction::Offer {
        call_id,
        call_creator,
        ..
    } = &incoming.action
    else {
        return Err(CallError::NotAnOffer);
    };
    let audio_rate = audio.signaling_rate.to_string();
    let audio_rates = [audio_rate.as_str()];
    let standard_opus = matches!(audio.rtp_profile, AudioRtpProfile::StandardOpus);
    let target = if is_group {
        Jid::new(call_id, Server::Call)
    } else {
        incoming.from.clone()
    };
    let preaccept = build_preaccept_with_capability(
        call_id,
        &target,
        call_creator,
        preaccept_id,
        &audio_rates,
        answer_preaccept_capability(video, audio),
        video,
    );
    // Captured video accepts mirror the offer's peer experiment metadata; audio accepts omit it.
    let metadata = if video { incoming.media() } else { None };
    let accept = build_accept(&AcceptParams {
        call_id,
        to: &target,
        id: accept_id,
        call_creator,
        audio_rates: &audio_rates,
        relay_te: None,
        rte: None,
        voip_settings: standard_opus
            .then(|| standard_opus_voip_settings(audio.rtp_clock_rate == 48_000)),
        // A captured from-start video accept carries no capability child; audio answers advertise
        // the selected MLOW/standard-Opus capability.
        capability: if video {
            None
        } else if standard_opus {
            Some(&CAPABILITY_STANDARD_OPUS_OFFER)
        } else {
            Some(&CAPABILITY_OFFER)
        },
        video,
        peer_abtest_bucket: metadata.and_then(|offer| offer.peer_abtest_bucket.as_deref()),
        peer_abtest_bucket_id_list: metadata
            .and_then(|offer| offer.peer_abtest_bucket_id_list.as_deref()),
    });
    Ok((preaccept, accept))
}

#[cfg(test)]
async fn send_answer_signaling_with_ids(
    client: &Client,
    incoming: &IncomingCall,
    audio: AudioFormat,
    video: bool,
    registration: &RegisteredCall,
    teardown: &mut AnswerTeardown,
    ids: (&str, &str),
) -> Result<(), CallError> {
    let (preaccept, accept) = build_answer_signaling(
        incoming,
        audio,
        video,
        incoming.group.is_some(),
        ids.0,
        ids.1,
    )?;
    send_answer_node(client, registration, teardown, preaccept).await?;
    send_answer_node(client, registration, teardown, accept).await
}

async fn send_answer_node(
    client: &Client,
    registration: &RegisteredCall,
    teardown: &mut AnswerTeardown,
    node: wacore_binary::Node,
) -> Result<(), CallError> {
    registration.ensure_current()?;
    // A failed transport write is delivery-ambiguous. Arm before the await so both an error and
    // cancellation explicitly end this generation; a peer-ended generation fails the guarded claim.
    teardown.arm();
    if let Err(error) = client.send_node(node).await {
        teardown.terminate(client).await;
        return Err(error.into());
    }
    registration.ensure_current()?;
    Ok(())
}

async fn send_preaccept_then_prepare<T>(
    client: &Client,
    registration: &RegisteredCall,
    teardown: &mut AnswerTeardown,
    preaccept: wacore_binary::Node,
    prepare: impl Future<Output = Result<T, CallError>>,
) -> Result<T, CallError> {
    // Preaccept is the early device-selection response: send it before Signal decrypt/prekey I/O
    // so the caller stops ringing sibling devices while this answer prepares its media.
    send_answer_node(client, registration, teardown, preaccept).await?;
    match prepare.await {
        Ok(prepared) => Ok(prepared),
        Err(error) => {
            teardown.terminate(client).await;
            Err(error)
        }
    }
}

/// Generate one shared group epoch, Signal-encrypt it independently to every connected remote
/// PID-bearing device, and send every direct `enc_rekey`. The caller installs the returned root
/// locally only after the complete fan-out succeeds.
pub(crate) struct GroupEpochFanout {
    raw_epoch: Zeroizing<Vec<u8>>,
    teardown: GroupRekeyTeardown,
}

impl GroupEpochFanout {
    pub(crate) fn generation(&self) -> Option<u64> {
        self.teardown.generation
    }

    /// Commit the local epoch before disarming partial-fanout teardown. If `apply` fails or this
    /// future's caller is cancelled before calling `commit`, dropping the guard terminates the call.
    pub(crate) fn commit<T>(
        mut self,
        apply: impl FnOnce(&[u8]) -> Result<T, CallError>,
    ) -> Result<T, CallError> {
        let result = apply(&self.raw_epoch)?;
        self.teardown.disarm();
        Ok(result)
    }
}

pub(crate) async fn fanout_group_epoch(
    client: &Client,
    update: &GroupCallUpdate,
) -> Result<GroupEpochFanout, CallError> {
    let generation = client.call_registry().generation_of(&update.call_id);
    fanout_group_epoch_for_generation(client, update, generation).await
}

fn ensure_group_rekey_generation(
    client: &Client,
    call_id: &str,
    generation: Option<u64>,
) -> Result<(), CallError> {
    if generation
        .is_some_and(|generation| client.call_registry().generation_of(call_id) != Some(generation))
    {
        Err(CallError::CallEndedDuringSetup)
    } else {
        Ok(())
    }
}

async fn fanout_group_epoch_for_generation(
    client: &Client,
    update: &GroupCallUpdate,
    generation: Option<u64>,
) -> Result<GroupEpochFanout, CallError> {
    ensure_group_rekey_generation(client, &update.call_id, generation)?;
    // The authoritative roster transaction is already committed before this fan-out starts. Any
    // preparation failure would make an identical redelivery stale, so own teardown from the first
    // fallible step instead of leaving the generation alive without its requested epoch.
    let teardown = GroupRekeyTeardown::new(client, update, generation, true);
    let own_lid = client.lid().ok_or(CallError::Media("no own LID"))?;
    let own_pn = client.pn();
    let mut seen = std::collections::HashSet::new();
    let recipients = update
        .participants
        .iter()
        .filter(|participant| participant.state.as_deref() == Some("connected"))
        .flat_map(|participant| participant.devices.iter())
        .filter(|device| {
            device.pid.is_some()
                && !same_device_identity(&device.jid, &own_lid)
                && !own_pn
                    .as_ref()
                    .is_some_and(|own_pn| same_device_identity(&device.jid, own_pn))
                && seen.insert(device.jid.clone())
        })
        .map(|device| device.jid.clone())
        .collect::<Vec<_>>();
    if !recipients.is_empty() {
        client
            .signal()
            .assert_sessions(&recipients)
            .await
            .map_err(|error| CallError::Setup(error.to_string()))?;
    }
    ensure_group_rekey_generation(client, &update.call_id, generation)?;

    let raw_epoch = Zeroizing::new(rand::random::<[u8; 32]>().to_vec());
    let mut message = wa::Message {
        call: buffa::MessageField::some(wa::message::Call {
            call_key: Some(raw_epoch.to_vec()),
            ..Default::default()
        }),
        ..Default::default()
    };
    let padded = Zeroizing::new(MessageUtils::encode_and_pad(&message));
    if let Some(call) = message.call.as_option_mut()
        && let Some(call_key) = call.call_key.as_mut()
    {
        call_key.zeroize();
    }
    if recipients.is_empty() {
        return Ok(GroupEpochFanout {
            raw_epoch,
            teardown,
        });
    }
    let device = client.persistence_manager().get_device_snapshot();
    let encrypted = {
        let lock_jids = client.build_session_lock_keys(&recipients).await;
        let session_guards = client.session_guards_for(&lock_jids).await;
        if device.account.is_none() {
            for recipient in &recipients {
                if client
                    .would_emit_pkmsg(recipient)
                    .await
                    .map_err(|error| CallError::Setup(error.to_string()))?
                {
                    return Err(CallError::MissingDeviceIdentity);
                }
            }
        }
        let plan = wacore::send::SessionPlan::assume_ready(recipients.len());
        let mut adapter = client.signal_adapter();
        let mut stores = adapter.as_signal_stores();
        let encrypted = wacore::send::encrypt_for_devices_with_sessions_raw(
            &*client.runtime,
            &mut stores,
            &recipients,
            &padded,
            plan,
        )
        .await
        .map_err(|error| CallError::Setup(error.to_string()));
        drop(session_guards);
        encrypted
    };
    let encrypted = encrypted?;
    // This path asserts sessions upstream instead of establishing them, so
    // nothing else has counted a device the fan-out could not encrypt for.
    // Recorded before the checks below, which can return early.
    client
        .stats
        .record_unkeyable_devices(UnkeyableDevice::Encrypt, encrypted.unkeyed_at_encrypt);
    ensure_group_rekey_generation(client, &update.call_id, generation)?;
    let device_identity = wacore::send::needs_device_identity(
        encrypted.includes_prekey_message,
        device.account.as_deref(),
    )
    .map_err(|_| CallError::MissingDeviceIdentity)?;
    client
        .persist_signal_state_pre_wire()
        .await
        .map_err(|error| CallError::Setup(error.to_string()))?;
    ensure_group_rekey_generation(client, &update.call_id, generation)?;
    publish_group_epoch_ciphertexts(
        client,
        update,
        generation,
        &recipients,
        &encrypted,
        device_identity.as_deref(),
    )
    .await?;
    Ok(GroupEpochFanout {
        raw_epoch,
        teardown,
    })
}

fn same_device_identity(left: &Jid, right: &Jid) -> bool {
    left.user == right.user && left.server == right.server && left.device == right.device
}

async fn publish_group_epoch_ciphertexts(
    client: &Client,
    update: &GroupCallUpdate,
    generation: Option<u64>,
    recipients: &[Jid],
    encrypted: &wacore::send::EncryptForDevicesRaw,
    device_identity: Option<&[u8]>,
) -> Result<(), CallError> {
    let complete = encrypted.devices.len() == recipients.len()
        && recipients.iter().all(|recipient| {
            encrypted
                .devices
                .iter()
                .any(|encrypted| encrypted.device_jid == *recipient)
        });
    let mut nodes = Vec::with_capacity(recipients.len());
    for recipient in recipients {
        let Some(encrypted) = encrypted
            .devices
            .iter()
            .find(|encrypted| encrypted.device_jid == *recipient)
        else {
            continue;
        };
        nodes.push(
            wacore::stanza::group_call::build_group_enc_rekey(
                &wacore::stanza::group_call::GroupEncRekeyParams {
                    call_id: &update.call_id,
                    id: &client.generate_request_id(),
                    to: recipient,
                    call_creator: &update.call_creator,
                    transaction_id: update.transaction_id,
                    device_key: &OfferDeviceKey {
                        device_jid: encrypted.device_jid.clone(),
                        ciphertext: encrypted.ciphertext.clone(),
                        enc_type: encrypted.enc_type.to_string(),
                    },
                    device_identity,
                },
            )
            .map_err(|error| CallError::Response(error.to_string()))?,
        );
    }
    for node in nodes {
        ensure_group_rekey_generation(client, &update.call_id, generation)?;
        client.send_node(node).await?;
        ensure_group_rekey_generation(client, &update.call_id, generation)?;
    }
    if !complete {
        return Err(CallError::Media(
            "group epoch encryption did not cover every recipient",
        ));
    }
    Ok(())
}

/// Drain every dormant outgoing call (relay never arrived) and notify each one's `ended` so any
/// parked `wait_ended()` resolves. The relay socket and signaling are connection-scoped, so a dormant
/// outgoing call can't survive a disconnect; the registry's `abort_all` already covers attached calls,
/// but those have no media-task drop-guard yet, so this is their only end-notify path. Called from the
/// client's connection teardown.
pub(crate) fn drain_pending_outgoing_on_disconnect(client: &Client) {
    let drained: Vec<PendingOutgoing> = {
        let mut map = client
            .voip_state()
            .pending_outgoing_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.drain().map(|(_, p)| p).collect()
    };
    for pending in drained {
        pending.ended.notify();
    }
}

/// Generate the callKey, encrypt it per device, send the `<offer>`, register the outgoing call, and
/// park the relay-attach material. Split from device resolution (usync) so it is offline testable
/// with a seeded device list. The relay attach is the only live-only piece.
#[allow(clippy::too_many_arguments)]
async fn place_call(
    client: &Client,
    call_id: String,
    peer: &Jid,
    call_creator: &Jid,
    own_lid: &Jid,
    // The devices we encrypt the callKey for (the offer's `<enc>` recipients).
    devices: &[Jid],
    // The FULL callee device set the server rings (a superset of `devices` -- it includes devices we
    // can't encrypt for). Drives the sibling-dismiss target set and the addressed offer shape.
    ring_devices: &[Jid],
    audio: AudioEndpoints,
    video: Option<VideoEndpoints>,
) -> Result<CallHandle, CallError> {
    // The offer keeps the addressed `<destination><to jid>` shape whenever the callee is multi-device
    // (computed before any pkmsg/encrypt filtering), even if one encryptable survivor remains.
    let multi_device = ring_devices.len() > 1;
    // Diagnostic: the resolved callee device set drives sibling-dismiss. If a multi-device callee
    // shows only one here, a device-list resolution gap (e.g. the primary missing) is why a sibling
    // keeps ringing -- the dismiss is gated on `multi_device`.
    log::debug!(
        "voip: call {call_id} resolved {} callee device(s) (sibling-dismiss {}): [{}]",
        ring_devices.len(),
        if multi_device { "armed" } else { "off" },
        // Observed (PII-safe) device list, matching the rest of the call paths; the args only format
        // when debug logging is enabled, so the join is free otherwise.
        ring_devices
            .iter()
            .map(|j| j.observe().to_string())
            .collect::<Vec<_>>()
            .join(", "),
    );
    // The callKey we generate is what the engine and SFrame key from; the peer learns it by
    // decrypting the per-device <enc> we send below.
    let call_key = rand::random::<[u8; 32]>();
    let padded = MessageUtils::encode_and_pad(&wa::Message {
        call: buffa::MessageField::some(wa::message::Call {
            call_key: Some(call_key.to_vec()),
            ..Default::default()
        }),
        ..Default::default()
    });

    // Encrypt the callKey for each peer device, reusing the message send path's per-device encrypt
    // core (`encrypt_for_devices_with_sessions_raw`): same parallel fan-out + skip-on-fail contract.
    // A per-device failure (no session, identity/backend error) only SKIPS that device (mirrors the
    // message send fan-out's log+skip): aborting the whole offer would strand the already-encrypted
    // devices with an advanced Signal chain for a ciphertext they never receive, making a later send
    // undecryptable. The offer is built from the survivors.
    //
    // Take the per-device session locks once (the send path's batch model) instead of a per-device
    // lock; concurrent ratchet mutations would corrupt session state.
    let raw = {
        let lock_jids = client.build_session_lock_keys(devices).await;
        let _session_guards = client.session_guards_for(&lock_jids).await;

        // Sessions were asserted upstream (`OutgoingCall::start`), so skip the network session-ensure and
        // encrypt against the existing sessions directly: a device whose session is somehow still
        // missing fails its encrypt and is skipped, exactly as the old per-device loop did.
        let plan = wacore::send::SessionPlan::assume_ready(devices.len());
        let mut adapter = client.signal_adapter();
        let mut stores = adapter.as_signal_stores();
        let raw = wacore::send::encrypt_for_devices_with_sessions_raw(
            &*client.runtime,
            &mut stores,
            devices,
            &padded,
            plan,
        )
        .await
        .map_err(|e| CallError::Setup(e.to_string()))?;
        // Before the persist below, which can bail with `?`: the devices this
        // fan-out dropped are dropped whether or not the offer goes out.
        client
            .stats
            .record_unkeyable_devices(UnkeyableDevice::Encrypt, raw.unkeyed_at_encrypt);
        drop(_session_guards);
        client
            .persist_signal_state_pre_wire()
            .await
            .map_err(|e| CallError::Setup(e.to_string()))?;
        raw
    };

    // The raw fan-out yields survivors in completion order; re-order to the input `devices` order so
    // the offer addresses devices deterministically (the offer's `<destination><to>` order).
    let mut device_keys: Vec<OfferDeviceKey> = Vec::with_capacity(raw.devices.len());
    for device in devices {
        if let Some(one) = raw.devices.iter().find(|d| &d.device_jid == device) {
            device_keys.push(OfferDeviceKey {
                device_jid: one.device_jid.clone(),
                ciphertext: one.ciphertext.clone(),
                enc_type: one.enc_type.to_string(),
            });
        }
    }
    // Every device failed to encrypt: there is no one to address the offer to.
    if device_keys.is_empty() {
        return Err(CallError::NoDevices);
    }

    // A pkmsg device must be able to validate our identity, so attach the encoded ADV account
    // identity (same blob the message send path attaches alongside a pkmsg). A pkmsg without a
    // <device-identity> advances our sender chain while the peer can't validate or consume the pre-key
    // message, so `needs_device_identity` refuses here before any registration/send.
    let account = client
        .persistence_manager()
        .get_device_snapshot()
        .account
        .clone();
    let device_identity = match wacore::send::needs_device_identity(
        raw.includes_prekey_message,
        account.as_deref(),
    ) {
        Ok(bytes) => bytes,
        Err(_) => return Err(CallError::MissingDeviceIdentity),
    };

    // A privacy-restricted callee rejects the offer unless it carries our stored token for them.
    let privacy_token = client.lookup_tc_token_for_jid(peer).await;

    // The offer needs a stanza id so the server can ack-correlate it: the initiator's relay rides
    // back on the `<ack type=offer>` reply to THIS id, not on a later <call>.
    let offer_stanza_id = client.generate_request_id();
    let audio_rate = audio.signaling_rate().to_string();
    let audio_rates = [audio_rate.as_str()];
    let offer = build_offer(&OfferParams {
        call_id: &call_id,
        to: peer,
        call_creator,
        device_keys: &device_keys,
        privacy_token: privacy_token.as_deref(),
        capability: Some(offer_capability(video.is_some(), audio.config().format)),
        device_identity: device_identity.as_deref(),
        id: Some(&offer_stanza_id),
        // Keep the addressed `<destination>` shape for a multi-device callee even if encryption
        // failures left a single surviving key, so that key stays tied to its device.
        multi_device,
        video: video.is_some(),
        audio_rates: &audio_rates,
    });

    // Register the ack-waiter for the offer's stanza id BEFORE send_node so a fast server reply can't
    // arrive before we are listening. The ack carries the relay; the spawned task below attaches the
    // engine when it resolves.
    let ack_rx = client.register_ack_waiter(&offer_stanza_id);

    // Register the outgoing call AND park the relay-attach material BEFORE send_node, mirroring the
    // incoming register-before-connect ordering. The handle starts dormant; the ack-waiter task
    // attaches the media engine once the relay arrives.
    let registry = client.call_registry();
    let mut session =
        wacore::voip::CallSession::new_outgoing(&call_id, peer.clone(), call_creator.clone());
    session.audio_format = Some(audio.config().format);
    session.is_video = video.is_some();
    // The rung device set lives on the session so an inbound <accept>/<reject> from one callee device
    // can dismiss the rest (caller-driven accepted_elsewhere); it is dropped automatically whenever the
    // call deregisters (every end path removes the registry entry). Use the FULL server-rung set, NOT
    // the encrypted `device_keys` subset: a device we couldn't encrypt for (e.g. the primary phone,
    // dropped as pkmsg without an ADV account) still rings and must be dismissed, or it times the call
    // out. Only when the callee is multi-device -- a single-device callee has no sibling.
    if multi_device {
        session.ring_devices = ring_devices.to_vec();
    }
    let _ = session.transition_to(CallPhase::Calling);
    let generation = registry.insert(session);
    registry.set_group_invite_self_device(
        &call_id,
        generation,
        GroupCallDevice::new(own_lid.clone())
            .with_capability(1, offer_capability(video.is_some(), audio.config().format)),
    );

    let muted = Arc::new(AtomicBool::new(false));
    let ended = Arc::new(EndedFlag::default());
    // Wake wait_ended() whenever this registry entry is removed -- including a terminal stanza or a
    // disconnect that lands while we're still dialing the relay (no media task yet to carry the notify).
    registry.set_ended_notify(&call_id, generation, {
        let ended = ended.clone();
        move || ended.notify()
    });
    let (ev_tx, ev_rx) = async_channel::bounded::<CallEvent>(CALL_EVENT_CHANNEL_CAPACITY);

    // Recv-rekey channel, created now (not at engine build) so a `<accept>` that races ahead of the
    // relay still lands: the sender lives on the registry from this point; the receiver is parked on
    // the pending entry and handed to the drive loop when the relay arrives (the bounded(1) buffers a
    // pre-engine rekey). One slot is enough — the rekey is one-shot (first answerer wins).
    let (rekey_tx, rekey_rx) = async_channel::bounded::<String>(1);
    registry.set_rekey_sender(&call_id, generation, rekey_tx);

    // Video plumbing exists for EVERY call (idle channels cost nothing): the signaling handler and
    // CallHandle::start_video need the senders even when the call starts audio-only.
    let video_shared = Arc::new(VideoShared::new());
    registry.set_video_channels(
        &call_id,
        generation,
        ev_tx.clone(),
        video_shared.ctl_tx.clone(),
        video_teardown_hook(&video_shared),
    );

    // Park the material needed to spawn the engine once the relay arrives. Keyed by call-id.
    client
        .voip_state()
        .pending_outgoing_calls
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .insert(
            call_id.clone(),
            PendingOutgoing {
                generation,
                self_lid: own_lid.to_string(),
                peer_lid: peer.to_string(),
                call_key: call_key.to_vec(),
                audio,
                video,
                video_shared: video_shared.clone(),
                muted: muted.clone(),
                ended: ended.clone(),
                ev_tx,
                rekey_rx,
            },
        );

    // If the send fails, undo the registration: drop the (generation-guarded) pending entry, reap our
    // registry generation, drop the dangling ack-waiter, and wake any wait_ended() waiter, then
    // propagate. Guarded so a same-call-id replacement that already superseded us isn't evicted.
    if let Err(e) = client.send_node(offer).await {
        let removed = take_pending_if_current(
            &client.voip_state().pending_outgoing_calls,
            &call_id,
            generation,
        );
        registry.remove_if_current(&call_id, generation);
        // No ack will ever arrive for the failed offer; drop the waiter so it can't leak.
        client.response_waiters_guard().remove(&offer_stanza_id);
        if removed.is_some() {
            ended.notify();
        }
        return Err(e.into());
    }

    // Prevents 463 nacks on later offers to this contact (WA Web's post-offer sendTcToken).
    spawn_call_tc_token_issuance(client, peer);

    // The relay arrives in the `<ack type=offer>` reply to the offer's stanza id (live-only, needs a
    // real server). Wait on the ack-waiter with a bounded timeout; attach the engine when the relay
    // lands, else fail the call so a parked wait_ended() resolves.
    spawn_outgoing_relay_waiter(client, call_id.clone(), generation, offer_stanza_id, ack_rx);

    Ok(CallHandle {
        call_id,
        generation,
        peer_jid: peer.clone(),
        call_creator: call_creator.clone(),
        client_registry: registry,
        pending_outgoing_calls: client.voip_state().pending_outgoing_calls.clone(),
        client: client_weak(client),
        muted,
        video: video_shared,
        events: ev_rx,
        ended,
    })
}

/// The client's own `Weak`, for handles that need to reach back (send a `<video>` stanza) without
/// keeping the client alive.
fn client_weak(client: &Client) -> std::sync::Weak<Client> {
    client
        .self_weak
        .get()
        .cloned()
        .unwrap_or_else(std::sync::Weak::new)
}

/// Time to wait for the server's `<ack type=offer>` carrying the relay before giving up.
const OFFER_ACK_RELAY_TIMEOUT: Duration = Duration::from_secs(10);

/// Three 60 ms frames absorb scheduling jitter without building a long capture delay.
const MIC_CHANNEL_CAPACITY: usize = 3;

/// Bound on the consumer-facing `CallEvent` queue. The driver posts with `try_send`, so once a slow
/// or absent consumer lets it fill, further diagnostics drop instead of growing without bound.
/// Lifecycle events (RelayAllocated/Failed/TimedOut) are emitted before media flows, so they are
/// never dropped, and call teardown is driven by the `ended` flag, not this channel.
const CALL_EVENT_CHANNEL_CAPACITY: usize = 64;
/// Signaling controls are rare, but the queue stays bounded against a stalled media task.
const GROUP_CONTROL_CHANNEL_CAPACITY: usize = 64;

/// Returned by `CallError::Connect` when the socket drops mid-setup, before the engine is attached.
const ERR_DISCONNECTED_DURING_SETUP: &str = "connection dropped during call setup";

/// Spawn the task that turns the offer's `<ack>` into a connected media engine: await the ack-waiter
/// (bounded), and on a node carrying a `<relay>` attach the engine via [`attach_outgoing_relay`]
/// (reusing its for_outgoing + generation handling). On timeout / no relay / closed channel the call
/// failed to connect, so drain its pending entry and notify `ended` to resolve wait_ended().
fn spawn_outgoing_relay_waiter(
    client: &Client,
    call_id: String,
    generation: u64,
    offer_stanza_id: String,
    ack_rx: futures::channel::oneshot::Receiver<Arc<wacore_binary::OwnedNodeRef>>,
) {
    // Upgrade the client's self-weak so the task owns an Arc<Client> for the duration of the wait.
    let Some(client) = client.self_weak.get().and_then(|w| w.upgrade()) else {
        return;
    };
    let runtime = client.runtime.clone();
    runtime
        .clone()
        .spawn(Box::pin(async move {
            let relay =
                match wacore::runtime::timeout(&*runtime, OFFER_ACK_RELAY_TIMEOUT, ack_rx).await {
                    // The ack node re-encoded as OwnedNodeRef; find its <relay> child and parse it (same
                    // path the incoming offer uses). handle_ack_response already removed our waiter entry.
                    Ok(Ok(ack)) => wacore::stanza::call::find_relay(ack.get())
                        .and_then(wacore::voip::relay_parse::parse_relay_data),
                    // Sender dropped (disconnect cleared the waiter map) or the timeout elapsed:
                    // handle_ack_response never ran, so our still-registered waiter entry must be dropped
                    // here or send_keepalive suppresses pings for the life of the connection.
                    Ok(Err(_)) | Err(_) => {
                        client.response_waiters_guard().remove(&offer_stanza_id);
                        None
                    }
                };

            match relay {
                Some(relay) => {
                    if let Err(e) = attach_outgoing_relay(&client, &call_id, &relay).await {
                        warn!("voip: failed to attach outgoing relay for {call_id}: {e}");
                        fail_pending_outgoing(&client, &call_id, generation);
                    }
                }
                None => {
                    warn!(
                        "voip: no relay in offer ack for {call_id} (timeout or absent); call failed"
                    );
                    fail_pending_outgoing(&client, &call_id, generation);
                }
            }
        }))
        .detach();
}

/// Fire-and-forget issuance of our trusted-contact token to the callee after an outgoing offer,
/// mirroring WA Web's `sendTcToken` in StartCall.js. Rate-limited by the sender bucket so repeat
/// calls to the same contact within a window don't re-issue. Best-effort: a failed issuance only
/// risks a later re-issue, never the call itself.
fn spawn_call_tc_token_issuance(client: &Client, peer: &Jid) {
    let Some(client) = client.self_weak.get().and_then(|w| w.upgrade()) else {
        return;
    };
    let runtime = client.runtime.clone();
    let peer = peer.clone();
    runtime
        .spawn(Box::pin(async move {
            if client.should_issue_tc_token(&peer).await {
                client.issue_tc_token_after_send(&peer).await;
            }
        }))
        .detach();
}

/// Tear down a still-dormant outgoing call that never got its relay: drop the (generation-guarded)
/// pending entry, reap the registry generation, and notify its `ended` so wait_ended() resolves. A
/// no-op if the call was already hung up or superseded.
/// Remove the pending-outgoing entry for `call_id` only if it is still on `generation`. A same-call-id
/// replacement that already superseded it keeps its entry. The generation guard must stay in lockstep
/// across every removal site, so it lives here rather than being re-inlined.
fn take_pending_if_current(
    pending: &std::sync::Mutex<std::collections::HashMap<String, PendingOutgoing>>,
    call_id: &str,
    generation: u64,
) -> Option<PendingOutgoing> {
    let mut map = pending.lock().unwrap_or_else(|e| e.into_inner());
    if map.get(call_id).is_some_and(|p| p.generation == generation) {
        map.remove(call_id)
    } else {
        None
    }
}

fn fail_pending_outgoing(client: &Client, call_id: &str, generation: u64) {
    let pending = take_pending_if_current(
        &client.voip_state().pending_outgoing_calls,
        call_id,
        generation,
    );
    client
        .call_registry()
        .remove_if_current(call_id, generation);
    if let Some(pending) = pending {
        pending.ended.notify();
    }
}

/// Tear a call down on a peer terminal stanza (`<reject>` / `<terminate>`): aborts the active media
/// task, drains a still-dormant pending-outgoing entry, and notifies `ended` so
/// `CallHandle::wait_ended()` resolves instead of hanging until an unrelated relay timeout. A no-op
/// for a call_id we don't track.
pub(crate) fn terminate_call(client: &Client, call_id: &str) {
    if let Some(generation) = client.call_registry().generation_of(call_id) {
        fail_pending_outgoing(client, call_id, generation);
    }
}

/// Tear down only the generation whose terminal stanza was authorized.
pub(crate) fn terminate_call_if_current(client: &Client, call_id: &str, generation: u64) {
    fail_pending_outgoing(client, call_id, generation);
}

/// "00" + 15 random bytes as lowercase hex (WA Web `_e()`): 32 hex chars total.
fn gen_call_id() -> String {
    format!("00{}", hex::encode(rand::random::<[u8; 15]>()))
}

/// Everything `voip().call()` parks until the initiator's relay arrives from the server. Holds the
/// already-registered generation and the already-handed-out handle's shared state so the engine
/// drives the SAME [`CallHandle`].
pub(crate) struct PendingOutgoing {
    generation: u64,
    self_lid: String,
    peer_lid: String,
    call_key: Vec<u8>,
    audio: AudioEndpoints,
    /// `.video()` endpoints for a video-from-the-start call; `None` for audio-only.
    video: Option<VideoEndpoints>,
    /// The handle's video plumbing (created at place time so `start_video` works while dormant).
    video_shared: Arc<VideoShared>,
    muted: Arc<AtomicBool>,
    ended: Arc<EndedFlag>,
    ev_tx: async_channel::Sender<CallEvent>,
    /// Receiver half of the one-shot recv-rekey channel (sender lives on the registry). Handed to the
    /// drive loop when the relay arrives so a `<accept>` that beat the relay is still applied (buffered).
    rekey_rx: async_channel::Receiver<String>,
}

/// The relay socket address to dial, read off a built config's already-parsed endpoint (avoids
/// re-walking the relay block, which `CallConfig::for_*` already did into `relay_ip`/`relay_port`).
fn socket_addr_from_config(config: &CallConfig) -> Result<SocketAddr, CallError> {
    format!("{}:{}", config.relay_ip, config.relay_port)
        .parse()
        .map_err(|_| CallError::Media("relay address is not a valid socket addr"))
}

/// Build the engine from a relay that arrived for a pending OUTGOING call and start the driver,
/// reusing the dormant handle's shared state. Returns `Ok(false)` when no pending outgoing call
/// matches `call_id` (so the caller can fall through to normal handling).
///
/// The relay rides back in the server's `<ack type=offer>` reply to the offer's stanza id; the
/// ack-waiter task captures it and calls this with the `<relay>` parsed by the same
/// `find_relay`/`parse_relay_data` the incoming offer uses.
pub(crate) async fn attach_outgoing_relay(
    client: &Client,
    call_id: &str,
    relay: &RelayData,
) -> Result<bool, CallError> {
    // Remove-on-match: a second relay for the same call-id is ignored (the engine is already up).
    let pending = {
        let mut map = client
            .voip_state()
            .pending_outgoing_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        map.remove(call_id)
    };
    let Some(pending) = pending else {
        return Ok(false);
    };

    // Hung up / superseded during the ack race: the registry entry is gone, so don't resurrect the
    // call. No engine will attach, and hangup raced us for the pending entry so it couldn't notify;
    // wake any wait_ended() here.
    if client.call_registry().generation_of(call_id) != Some(pending.generation) {
        pending.ended.notify();
        return Ok(true);
    }

    // The pending entry is already removed above. The setup below (config/engine build + addr parse)
    // runs BEFORE attach_engine takes over registry/ended ownership, so on any of these errors the
    // call would otherwise leak its registry generation and a parked wait_ended() would hang forever
    // (no pending entry left for a later hangup to drain). Build everything in a fallible block and, on
    // any early-return error in this window, reap the generation and notify `ended` before propagating.
    let build = (|| {
        let mut config = CallConfig::for_outgoing(
            call_id,
            &pending.self_lid,
            &pending.peer_lid,
            pending.call_key.clone(),
            relay,
        )
        .map_err(|e| CallError::Setup(e.to_string()))?;
        config.audio = pending.audio.config();
        config.enable_video = pending.video.is_some();
        // Read the dial addr off the config before CallEngine::new consumes it (no second relay walk).
        let addr = socket_addr_from_config(&config)?;
        let engine = CallEngine::new(config, Box::new(RandTxIds))
            .map_err(|e| CallError::Setup(e.to_string()))?;
        Ok::<_, CallError>((
            engine,
            RelayMediaChannelFactory::new(addr, client.runtime.clone()),
        ))
    })();
    let (engine, factory) = match build {
        Ok(pair) => pair,
        Err(e) => {
            client
                .call_registry()
                .remove_if_current(call_id, pending.generation);
            pending.ended.notify();
            return Err(e);
        }
    };

    attach_engine(
        client,
        call_id,
        pending.generation,
        FailureCleanup::Here,
        engine,
        &factory,
        pending.audio,
        pending.video,
        pending.video_shared,
        pending.muted,
        pending.ended,
        pending.ev_tx,
        // Outgoing: hand the drive loop the recv-rekey receiver so a callee `<accept>` rekeys recv to
        // the answering device (buffered if the accept beat this relay).
        Some(pending.rekey_rx),
    )
    .await?;
    Ok(true)
}

/// A call generation registered before any fallible answer-side work. Until disarmed, dropping this
/// guard removes only its own generation, so setup/signaling errors cannot leak a task-less registry
/// entry or reap a same-call-id replacement.
struct RegisteredCall {
    registry: Arc<wacore::voip::CallRegistry>,
    call_id: String,
    peer_jid: Jid,
    call_creator: Jid,
    generation: u64,
    ended: Arc<EndedFlag>,
    armed: bool,
}

impl RegisteredCall {
    async fn new(client: &Client, session: wacore::voip::CallSession) -> Self {
        Self::new_inner(client, session, false).await
    }

    async fn new_group(client: &Client, session: wacore::voip::CallSession) -> Self {
        Self::new_inner(client, session, true).await
    }

    async fn new_inner(
        client: &Client,
        session: wacore::voip::CallSession,
        force_group: bool,
    ) -> Self {
        let registry = client.call_registry();
        let call_id = session.call_id.clone();
        let peer_jid = session.peer_jid.clone();
        let call_creator = session.call_creator.clone();
        // Serialize this insertion with a teardown for the same call-id. In particular, a failed
        // generation keeps the lane across its terminal send, closing the remove-before-send window
        // in which a re-offer could otherwise become current and then be killed by the stale stanza.
        let _transition = client.lock_answer_transition(&call_id).await;
        let generation = if session.group.is_some() {
            registry
                .promote_ringing_group(session.clone())
                .unwrap_or_else(|| registry.insert_group(session))
        } else if force_group {
            registry.insert_group(session)
        } else {
            registry.insert(session)
        };
        let ended = Arc::new(EndedFlag::default());
        // Wake setup/connect and any eventual CallHandle whenever this generation is removed,
        // including before a media task exists.
        registry.set_ended_notify(&call_id, generation, {
            let ended = ended.clone();
            move || ended.notify()
        });
        Self {
            registry,
            call_id,
            peer_jid,
            call_creator,
            generation,
            ended,
            armed: true,
        }
    }

    fn from_existing(client: &Client, call_id: &str, generation: u64) -> Result<Self, CallError> {
        let registry = client.call_registry();
        let session = registry
            .snapshot_if_current(call_id, generation)
            .ok_or(CallError::CallEndedDuringSetup)?;
        let ended = Arc::new(EndedFlag::default());
        registry.set_ended_notify(call_id, generation, {
            let ended = ended.clone();
            move || ended.notify()
        });
        Ok(Self {
            registry,
            call_id: call_id.to_string(),
            peer_jid: session.peer_jid,
            call_creator: session.call_creator,
            generation,
            ended,
            armed: true,
        })
    }

    fn ensure_current(&self) -> Result<(), CallError> {
        if self.registry.generation_of(&self.call_id) == Some(self.generation) {
            Ok(())
        } else {
            Err(CallError::CallEndedDuringSetup)
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for RegisteredCall {
    fn drop(&mut self) {
        if self.armed {
            self.registry
                .remove_if_current(&self.call_id, self.generation);
        }
    }
}

/// Owns peer cleanup once answer signaling begins. Claiming removes only this registration
/// generation while holding the call-id transition lane through the async terminal send, so a
/// superseding same-call-id offer cannot be installed in the removal-before-send window.
struct AnswerTeardown {
    client: std::sync::Weak<Client>,
    registry: Arc<wacore::voip::CallRegistry>,
    call_id: String,
    peer_jid: Jid,
    call_creator: Jid,
    generation: u64,
    armed: bool,
    claimed: bool,
    transition: Option<async_lock::MutexGuardArc<()>>,
}

struct GroupOfferTeardown {
    client: std::sync::Weak<Client>,
    registry: Arc<wacore::voip::CallRegistry>,
    call_id: String,
    call_creator: Jid,
    generation: u64,
    terminate_only_if_admitted: bool,
    armed: bool,
}

struct GroupRekeyTeardown {
    client: std::sync::Weak<Client>,
    call_id: String,
    call_creator: Jid,
    generation: Option<u64>,
    armed: bool,
}

impl GroupRekeyTeardown {
    fn new(
        client: &Client,
        update: &GroupCallUpdate,
        generation: Option<u64>,
        armed: bool,
    ) -> Self {
        Self {
            client: client_weak(client),
            call_id: update.call_id.clone(),
            call_creator: update.call_creator.clone(),
            generation,
            armed,
        }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

fn claim_group_rekey_generation(client: &Client, call_id: &str, generation: Option<u64>) -> bool {
    generation.is_none_or(|generation| {
        let pending = take_pending_if_current(
            &client.voip_state().pending_outgoing_calls,
            call_id,
            generation,
        );
        let removed = client
            .call_registry()
            .remove_if_current(call_id, generation);
        if let Some(pending) = pending {
            pending.ended.notify();
        }
        removed
    })
}

impl Drop for GroupRekeyTeardown {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Some(client) = self.client.upgrade() else {
            return;
        };
        let call_id = self.call_id.clone();
        let call_creator = self.call_creator.clone();
        let generation = self.generation;
        let transition = client.answer_transition_lock(&call_id);
        let runtime = client.runtime.clone();
        if let Some(transition) = transition.try_lock_arc() {
            if !claim_group_rekey_generation(&client, &call_id, generation) {
                return;
            }
            runtime
                .spawn(Box::pin(async move {
                    let _transition = transition;
                    let target = Jid::new(&call_id, Server::Call);
                    send_answer_terminate(&client, &call_id, &target, &call_creator).await;
                }))
                .detach();
            return;
        }
        runtime
            .spawn(Box::pin(async move {
                let _transition = transition.lock_arc().await;
                if !claim_group_rekey_generation(&client, &call_id, generation) {
                    return;
                }
                let target = Jid::new(&call_id, Server::Call);
                send_answer_terminate(&client, &call_id, &target, &call_creator).await;
            }))
            .detach();
    }
}

impl GroupOfferTeardown {
    fn new(client: &Client, registration: &mut RegisteredCall) -> Self {
        let teardown = Self {
            client: client_weak(client),
            registry: registration.registry.clone(),
            call_id: registration.call_id.clone(),
            call_creator: registration.call_creator.clone(),
            generation: registration.generation,
            terminate_only_if_admitted: false,
            armed: true,
        };
        // Transfer setup cleanup ownership before this guard can be dropped. Its async teardown
        // must remain able to claim the generation after the surrounding future is cancelled.
        registration.disarm();
        teardown
    }

    fn new_call_link(client: &Client, registration: &mut RegisteredCall) -> Self {
        let mut teardown = Self::new(client, registration);
        teardown.terminate_only_if_admitted = true;
        teardown
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for GroupOfferTeardown {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Some(client) = self.client.upgrade() else {
            return;
        };
        let registry = self.registry.clone();
        let call_id = self.call_id.clone();
        let call_creator = self.call_creator.clone();
        let generation = self.generation;
        let terminate_only_if_admitted = self.terminate_only_if_admitted;
        let runtime = client.runtime.clone();
        runtime
            .spawn(Box::pin(async move {
                // Claim this exact setup while holding the call-id transition lane through the
                // terminal send. A replacement either wins first (and suppresses this stale
                // terminate) or waits until the old call-scoped terminate is on the wire.
                let _transition = client.lock_answer_transition(&call_id).await;
                let Some(phase) = registry.remove_if_current_with_phase(&call_id, generation)
                else {
                    return;
                };
                if terminate_only_if_admitted && phase == CallPhase::WaitingRoom {
                    return;
                }
                let target = Jid::new(&call_id, Server::Call);
                send_answer_terminate(&client, &call_id, &target, &call_creator).await;
            }))
            .detach();
    }
}

impl AnswerTeardown {
    fn new(client: &Client, registration: &RegisteredCall) -> Self {
        Self {
            client: client_weak(client),
            registry: registration.registry.clone(),
            call_id: registration.call_id.clone(),
            peer_jid: registration.peer_jid.clone(),
            call_creator: registration.call_creator.clone(),
            generation: registration.generation,
            armed: false,
            claimed: false,
            transition: None,
        }
    }

    fn arm(&mut self) {
        self.armed = true;
    }

    fn disarm(&mut self) {
        self.armed = false;
        self.claimed = false;
        self.transition = None;
    }

    async fn terminate(&mut self, client: &Client) {
        if !self.armed {
            return;
        }
        // Store the owned guard in `self`: if this future is cancelled during the terminal write,
        // Drop can move the still-held lane into its detached retry without reopening the race.
        self.transition = Some(client.lock_answer_transition(&self.call_id).await);
        if self
            .registry
            .remove_if_current(&self.call_id, self.generation)
        {
            self.claimed = true;
            send_answer_terminate(client, &self.call_id, &self.peer_jid, &self.call_creator).await;
        }
        self.disarm();
    }
}

impl Drop for AnswerTeardown {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let Some(client) = self.client.upgrade() else {
            return;
        };
        let call_id = self.call_id.clone();
        let peer_jid = self.peer_jid.clone();
        let call_creator = self.call_creator.clone();
        let registry = self.registry.clone();
        let generation = self.generation;
        let claimed = self.claimed;
        let transition = self.transition.take();
        let runtime = client.runtime.clone();
        runtime
            .spawn(Box::pin(async move {
                let _transition = match transition {
                    Some(transition) => transition,
                    None => client.lock_answer_transition(&call_id).await,
                };
                if claimed || registry.remove_if_current(&call_id, generation) {
                    send_answer_terminate(&client, &call_id, &peer_jid, &call_creator).await;
                }
            }))
            .detach();
    }
}

pub(crate) async fn send_answer_terminate(
    client: &Client,
    call_id: &str,
    peer_jid: &Jid,
    call_creator: &Jid,
) {
    let id = client.generate_request_id();
    let stanza = build_terminate(&TerminateParams {
        call_id,
        to: peer_jid,
        id: Some(&id),
        call_creator,
        reason: None,
    });
    if let Err(error) = client.send_node(stanza).await {
        warn!("voip: failed to terminate peer after answer failed call_id={call_id}: {error}");
    }
}

/// Spawn the call driver over `factory` after registering it. Generic over the relay factory so a
/// test can inject an in-memory transport instead of the real DTLS/SCTP dialer.
#[cfg(test)]
async fn spawn_call(
    client: &Client,
    session: wacore::voip::CallSession,
    engine: CallEngine,
    factory: &dyn RelayTransportFactory,
    audio: AudioEndpoints,
    video: Option<VideoEndpoints>,
) -> Result<CallHandle, CallError> {
    let mut registration = RegisteredCall::new(client, session).await;
    let result = spawn_registered_call(client, &registration, engine, factory, audio, video).await;
    if result.is_ok() {
        registration.disarm();
    }
    result
}

/// Attach media to a generation that is already registered. Answering uses this after registering
/// before call-key decryption; the generic spawn wrapper above uses the same path for tests.
async fn spawn_registered_call(
    client: &Client,
    registration: &RegisteredCall,
    engine: CallEngine,
    factory: &dyn RelayTransportFactory,
    audio: AudioEndpoints,
    video: Option<VideoEndpoints>,
) -> Result<CallHandle, CallError> {
    registration.ensure_current()?;
    let registry = &registration.registry;
    let muted = Arc::new(AtomicBool::new(false));
    let (ev_tx, ev_rx) = async_channel::bounded::<CallEvent>(CALL_EVENT_CHANNEL_CAPACITY);
    let video_shared = Arc::new(VideoShared::new());
    registry.set_video_channels(
        &registration.call_id,
        registration.generation,
        ev_tx.clone(),
        video_shared.ctl_tx.clone(),
        video_teardown_hook(&video_shared),
    );
    attach_engine(
        client,
        &registration.call_id,
        registration.generation,
        FailureCleanup::Guard,
        engine,
        factory,
        audio,
        video,
        video_shared.clone(),
        muted.clone(),
        registration.ended.clone(),
        ev_tx,
        // Incoming (callee): no recv-rekey — the callee already keys recv on its own self LID.
        None,
    )
    .await?;
    Ok(CallHandle {
        call_id: registration.call_id.clone(),
        generation: registration.generation,
        peer_jid: registration.peer_jid.clone(),
        call_creator: registration.call_creator.clone(),
        client_registry: client.call_registry(),
        pending_outgoing_calls: client.voip_state().pending_outgoing_calls.clone(),
        client: client_weak(client),
        muted,
        video: video_shared,
        events: ev_rx,
        ended: registration.ended.clone(),
    })
}

/// Finish an answer after the peer has received `<accept>`. The teardown guard explicitly ends a
/// locally failed or cancelled startup, but its generation claim no-ops after peer termination or
/// same-call-id supersession.
async fn spawn_answered_call(
    client: &Client,
    registration: &mut RegisteredCall,
    mut teardown: AnswerTeardown,
    engine: CallEngine,
    factory: &dyn RelayTransportFactory,
    audio: AudioEndpoints,
    video: Option<VideoEndpoints>,
) -> Result<CallHandle, CallError> {
    match spawn_registered_call(client, registration, engine, factory, audio, video).await {
        Ok(handle) => {
            teardown.disarm();
            registration.disarm();
            Ok(handle)
        }
        Err(error) => {
            teardown.terminate(client).await;
            Err(error)
        }
    }
}

/// Connect the relay and spawn the driver task against pre-built shared handle state (mute flag,
/// ended flag, event sender). Shared so the outgoing relay-arrival path can drive the same
/// already-handed-out [`CallHandle`]. The registry entry under `generation` must already exist.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FailureCleanup {
    /// No outer registration guard remains, so attach_engine reaps failures itself.
    Here,
    /// RegisteredCall/AnswerTeardown owns generation-aware cleanup.
    Guard,
}

#[allow(clippy::too_many_arguments)]
async fn attach_engine(
    client: &Client,
    call_id: &str,
    generation: u64,
    failure_cleanup: FailureCleanup,
    engine: CallEngine,
    factory: &dyn RelayTransportFactory,
    audio: AudioEndpoints,
    video: Option<VideoEndpoints>,
    video_shared: Arc<VideoShared>,
    muted: Arc<AtomicBool>,
    ended: Arc<EndedFlag>,
    ev_tx: async_channel::Sender<CallEvent>,
    // Caller-only recv-rekey receiver; `None` for an incoming call (the callee keys recv on its own
    // self LID and never rekeys).
    rekey_rx: Option<async_channel::Receiver<String>>,
) -> Result<(), CallError> {
    let (group_tx, group_rx) = async_channel::bounded(GROUP_CONTROL_CHANNEL_CAPACITY);
    if !client.call_registry().set_group_control_sender(
        call_id,
        generation,
        engine.media_warp_mi_tag_len(),
        group_tx,
    ) {
        if failure_cleanup == FailureCleanup::Here {
            client
                .call_registry()
                .remove_if_current(call_id, generation);
        }
        ended.notify();
        return Err(CallError::Setup(
            "group relay WARP tag length changed during media attachment".to_string(),
        ));
    }
    let group_ctl = Some(group_rx);
    // The registry entry already exists. Re-check is_connected NOW (after insert, before connect) so a
    // disconnect that clears is_connected before abort_all can't slip through the gap between an
    // earlier guard's load and the insert: either we inserted before abort_all (it catches us) or this
    // sees !is_connected and the direct caller / outer registration guard cleans up. Wake
    // wait_ended() before bailing so a parked waiter resolves.
    if !client.is_connected() {
        if failure_cleanup == FailureCleanup::Here {
            client
                .call_registry()
                .remove_if_current(call_id, generation);
        }
        ended.notify();
        return Err(CallError::Connect(ERR_DISCONNECTED_DURING_SETUP.into()));
    }

    // Connect failure leaves the call already visible (registry entry inserted before connect; for an
    // outgoing call the PendingOutgoing was already removed by attach_outgoing_relay). Direct callers
    // reap here; guarded callers retain the generation so answer teardown can claim it atomically.
    // Either way, wake wait_ended() before propagating.
    //
    // Race the dial against the call ending. A hangup, a peer <terminate>, or a disconnect landing in
    // this window all remove our registry entry, and its `on_terminal` hook notifies `ended` even
    // though no media task exists yet -- so selecting on `ended` drops the in-flight connect future to
    // abort the unwanted DTLS/SCTP dial instead of letting it run to success or the 12s timeout while
    // wait_ended() stays parked.
    let dial = factory.connect();
    let (transport, relay_events) =
        match futures::future::select(dial, std::pin::pin!(ended.wait())).await {
            futures::future::Either::Left((Ok(pair), _)) => pair,
            futures::future::Either::Left((Err(e), _)) => {
                if failure_cleanup == FailureCleanup::Here {
                    client
                        .call_registry()
                        .remove_if_current(call_id, generation);
                }
                ended.notify();
                return Err(CallError::Connect(e.to_string()));
            }
            // Ended mid-dial: the loser `dial` future drops here, aborting the connect. The generation
            // was already reaped by whoever ended us; reap defensively and stop.
            futures::future::Either::Right(((), _dial)) => {
                client
                    .call_registry()
                    .remove_if_current(call_id, generation);
                return Err(CallError::Connect("call ended during relay connect".into()));
            }
        };

    // Only the selected I/O pair stays open. Closed inactive channels make their driver select arms
    // retire immediately without per-frame branching or idle tasks.
    let (mic_rx, speaker, encoded_audio_in, encoded_audio_out, audio_feed) = match audio {
        AudioEndpoints::Pcm { source, sink } => {
            let (mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(MIC_CHANNEL_CAPACITY);
            let mute_feed = MuteFeed {
                src: source.frames(),
                out: mic_tx,
                muted,
            };
            let feed = client.runtime.spawn(Box::pin(mute_feed.run()));
            let (_encoded_tx, encoded_audio_in) = async_channel::bounded::<Bytes>(1);
            let (encoded_audio_out, _encoded_rx) = async_channel::bounded::<EncodedAudioFrame>(1);
            (
                mic_rx,
                sink.playout(),
                encoded_audio_in,
                encoded_audio_out,
                Some(feed),
            )
        }
        AudioEndpoints::Encoded { source, sink, .. } => {
            let (_mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(1);
            let (speaker, _speaker_rx) = async_channel::bounded::<Vec<i16>>(1);
            (mic_rx, speaker, source.frames(), sink.frames(), None)
        }
    };

    // Video plumbing: the drive loop always gets the channels; the endpoints attach now (a
    // `.video()` call) or later (an upgrade via CallHandle::start_video/accept_video).
    let (video_in_rx, video_ctl_rx) = video_shared.take_receivers();
    let (video_out_tx, video_out_rx) = async_channel::bounded::<VideoFrame>(VIDEO_OUT_CHANNEL_CAP);
    // Forwarder from the drive loop to whatever sink is CURRENTLY attached (swappable mid-call).
    // Ends when the drive loop drops its video_out sender; moved into the media task like mic_feed.
    let sink_slot = video_shared.sink_slot.clone();
    let video_out_feed = client.runtime.spawn(Box::pin(async move {
        while let Ok(frame) = video_out_rx.recv().await {
            let tx = sink_slot.lock().unwrap_or_else(|e| e.into_inner()).clone();
            if let Some(tx) = tx {
                // Loss tolerant, like the speaker: a stalled sink sheds frames.
                let _ = tx.try_send(frame);
            }
        }
    }));
    if let Some(v) = &video {
        video_shared.attach_endpoints(client, &v.source, &v.sink, ended.clone());
        // From-start video: the engine was built with enable_video, but a same-path Enable is
        // idempotent and keeps one code path for both entries.
        video_shared.send_control(VideoControl::Enable);
    }

    let channels = CallChannels {
        mic: mic_rx,
        speaker,
        encoded_audio_in,
        encoded_audio_out,
        events: ev_tx,
        rekey: rekey_rx,
        video_in: video_in_rx,
        video_out: video_out_tx,
        video_ctl: video_ctl_rx,
        group_ctl,
    };

    let registry = client.call_registry();
    let registry_for_task = registry.clone();
    let cid = call_id.to_string();
    // Build the notify-on-drop guard OUTSIDE the future and move it in. A captured value is dropped
    // with the future even if the task is aborted before its first poll; a value `let`-bound inside
    // the body is only constructed on poll, so it would be skipped on an abort-before-poll and leave
    // a parked wait_ended() waiter asleep forever.
    let ended_guard = scopeguard::guard(ended, |e| {
        e.notify();
    });
    let task = client.runtime.spawn(Box::pin(async move {
        // All are captured (moved in), so any teardown -- even an abort before the first poll --
        // drops them: the feeds are aborted and `ended` is notified.
        let _ended_guard = ended_guard;
        let _audio_feed = audio_feed;
        let _video_out_feed = video_out_feed;
        run_call_tokio(transport, relay_events, channels, engine).await;
        // A locally-ended call gets no <terminate>; drop our own entry so the registry doesn't grow.
        // The call's `ring_devices` live on the session, so this also drops the sibling-dismiss
        // tracking -- no separate map to clean up.
        registry_for_task.remove_if_current(&cid, generation);
    }));
    registry.set_media_task(call_id, generation, task);
    Ok(())
}

/// Outbound video AU backlog before the source feed back-pressures (AUs are large; keep it short).
const VIDEO_IN_CHANNEL_CAP: usize = 4;
/// Inbound video AU backlog between the drive loop and the sink forwarder.
const VIDEO_OUT_CHANNEL_CAP: usize = 8;
/// `dec` we advertise on an upgrade request (what we can decode).
const VIDEO_DEC_REQUEST: &str = "H264";
/// `dec` WA Web advertises on an UpgradeAccept.
const VIDEO_DEC_ACCEPT: &str = "H264,AV1";
/// WA's native upgrade timer downgrades an unanswered request after five seconds.
const VIDEO_UPGRADE_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(Clone, Copy)]
enum VideoUpgradeRole {
    Initiate,
    Accept(VideoUpgradeToken),
}

/// Per-call video plumbing shared between the [`CallHandle`], the registry (signaling handler),
/// and the drive loop. Created for EVERY call at registration time — idle channels cost nothing,
/// and a mid-call upgrade must not have to rebuild the media task.
pub(crate) struct VideoShared {
    /// Mid-call video-plane control into the drive loop (Enable/Disable/SetOrientation).
    ctl_tx: VideoControlSender,
    /// Outbound AUs into the drive loop; a `VideoFeed` pumps the attached source into it.
    in_tx: async_channel::Sender<Vec<u8>>,
    /// The CURRENTLY attached sink (swappable: upgrade attaches, downgrade clears). The
    /// out-forwarder task reads it per frame.
    sink_slot: Arc<std::sync::Mutex<Option<async_channel::Sender<VideoFrame>>>>,
    /// The live source feed, aborted on downgrade/replacement.
    feed: std::sync::Mutex<Option<wacore::runtime::AbortHandle>>,
    /// Receiver halves parked until `attach_engine` hands them to the drive loop.
    receivers: std::sync::Mutex<Option<VideoReceivers>>,
}

/// The drive-loop halves of the video channels: (outbound AUs, plane control).
type VideoReceivers = (async_channel::Receiver<Vec<u8>>, VideoControlReceiver);

impl VideoShared {
    fn new() -> Self {
        let (ctl_tx, ctl_rx) = video_control_channel();
        let (in_tx, in_rx) = async_channel::bounded::<Vec<u8>>(VIDEO_IN_CHANNEL_CAP);
        Self {
            ctl_tx,
            in_tx,
            sink_slot: Arc::new(std::sync::Mutex::new(None)),
            feed: std::sync::Mutex::new(None),
            receivers: std::sync::Mutex::new(Some((in_rx, ctl_rx))),
        }
    }

    /// The drive-loop halves. After the one real take (attach_engine), fresh closed channels are
    /// returned defensively — their arms disable themselves in the driver.
    fn take_receivers(&self) -> VideoReceivers {
        self.receivers
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .unwrap_or_else(|| {
                let (_ctl_tx, ctl_rx) = video_control_channel();
                (async_channel::bounded(1).1, ctl_rx)
            })
    }

    fn send_control(&self, control: VideoControl) {
        let _ = self.ctl_tx.send(control);
    }

    /// Attach (or replace) the consumer's endpoints: the sink becomes the forwarder's target and a
    /// fresh `VideoFeed` pumps the source; a previous feed is aborted (its source is released).
    fn attach_endpoints(
        &self,
        client: &Client,
        source: &Arc<dyn VideoSource>,
        sink: &Arc<dyn VideoSink>,
        ended: Arc<EndedFlag>,
    ) {
        // Queue timing before the feed can make an AU ready. The driver's control arm is biased
        // ahead of media, so the first RTP timestamp already uses the source's cadence.
        self.send_control(VideoControl::SetTimestampStride(
            source.rtp_timestamp_stride(),
        ));
        *self.sink_slot.lock().unwrap_or_else(|e| e.into_inner()) = Some(sink.playout());
        let feed = VideoFeed {
            source: source.clone(),
            src: source.frames(),
            out: self.in_tx.clone(),
            ended,
        };
        let handle = client.runtime.spawn(Box::pin(feed.run()));
        // Abort outside the guard: edition 2024 keeps an `if let` scrutinee temporary
        // alive for the whole matching arm, and a `Runtime` that cancels synchronously
        // would drop the task inline and re-enter this non-reentrant mutex.
        let old = self
            .feed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .replace(handle);
        if let Some(old) = old {
            old.abort();
        }
    }

    /// Release the endpoints (downgrade / refused upgrade): the source feed is aborted, the sink is
    /// dropped, and the drive loop's video plane is disabled so it stops emitting/decoding video.
    fn detach_endpoints(&self) {
        *self.sink_slot.lock().unwrap_or_else(|e| e.into_inner()) = None;
        let feed = self.feed.lock().unwrap_or_else(|e| e.into_inner()).take();
        if let Some(feed) = feed {
            feed.abort();
        }
        self.send_control(VideoControl::Disable);
    }
}

/// A registry teardown hook that releases codec endpoints on refusal or call termination.
fn video_teardown_hook(video: &Arc<VideoShared>) -> Box<dyn Fn() + Send + Sync> {
    let video = video.clone();
    Box::new(move || video.detach_endpoints())
}

fn release_video_endpoints(
    registry: &wacore::voip::CallRegistry,
    pending_outgoing_calls: &std::sync::Mutex<std::collections::HashMap<String, PendingOutgoing>>,
    video: &VideoShared,
    call_id: &str,
    generation: u64,
) {
    let pending_video = {
        // This synchronous map is shared with non-async setup paths; its lock covers only one
        // lookup/take and is never held across an await.
        let mut pending = pending_outgoing_calls
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        pending
            .get_mut(call_id)
            .filter(|entry| entry.generation == generation)
            .and_then(|entry| entry.video.take())
    };
    drop(pending_video);
    if !registry.run_video_teardown(call_id, generation) {
        video.detach_endpoints();
    }
}

/// Forwards encoded AUs from the consumer's source into the drive loop. Bounded on the call's
/// `ended` flag as well as the channels, so a source that stops producing after the call ends
/// can't park this task on `recv()` forever.
struct VideoFeed {
    source: Arc<dyn VideoSource>,
    src: async_channel::Receiver<Vec<u8>>,
    out: async_channel::Sender<Vec<u8>>,
    ended: Arc<EndedFlag>,
}

impl VideoFeed {
    async fn run(self) {
        use futures::FutureExt;
        let _source = self.source;
        loop {
            let ended = self.ended.wait().fuse();
            let recv = self.src.recv().fuse();
            futures::pin_mut!(ended, recv);
            let au = futures::select_biased! {
                _ = ended => break,
                au = recv => match au {
                    Ok(au) => au,
                    Err(_) => break,
                },
            };
            // Await the send (the bounded channel back-pressures a source that outruns the wire),
            // but race it against `ended` too: a dormant call whose 4-AU queue filled before
            // `attach_engine` drained it would otherwise park here forever past teardown.
            let ended = self.ended.wait().fuse();
            let send = self.out.send(au).fuse();
            futures::pin_mut!(ended, send);
            futures::select_biased! {
                _ = ended => break,
                res = send => {
                    if res.is_err() {
                        break;
                    }
                }
            }
        }
    }
}

/// Forwards mic frames to the engine, zeroing them while muted. Zeroing (vs. dropping) keeps the
/// media stream fed: the engine turns an exact-zero frame into a one-byte DTX comfort-noise packet,
/// so the relay's consent-freshness timer never sees a gap (a gap makes the peer re-negotiate).
struct MuteFeed {
    src: async_channel::Receiver<Vec<i16>>,
    out: async_channel::Sender<Vec<i16>>,
    muted: Arc<AtomicBool>,
}

impl MuteFeed {
    async fn run(self) {
        while let Ok(mut frame) = self.src.recv().await {
            if self.muted.load(Ordering::Relaxed) && frame.len() == WA_FRAME_SAMPLES {
                // Exact-zero in place: the engine's mute fast-path keys on an all-zero 960 frame.
                frame.fill(0);
            }
            // A dropped receiver (call torn down) stops the feed.
            if self.out.send(frame).await.is_err() {
                break;
            }
        }
    }
}

/// A sticky one-shot "call ended" signal. Unlike a bare edge-triggered `Event`, a `wait()` that
/// arrives AFTER the notification still returns at once (the flag stays set), so a stale handle whose
/// task already ended -- e.g. one superseded by a same-call-id replacement -- never parks forever.
#[derive(Default)]
struct EndedFlag {
    done: AtomicBool,
    event: event_listener::Event,
}

impl EndedFlag {
    fn notify(&self) {
        self.done.store(true, Ordering::SeqCst);
        self.event.notify(usize::MAX);
    }

    async fn wait(&self) {
        // Listen BEFORE the flag check so a notify in the gap still wakes us.
        let listener = self.event.listen();
        if self.done.load(Ordering::SeqCst) {
            return;
        }
        listener.await;
    }
}

/// Opaque handle to a live call. Drop does NOT end the call (the driver task owns its own lifetime);
/// call [`hangup`](Self::hangup) to tear it down. No public fields, so the surface can grow without
/// breaking callers. `Clone` is cheap (shared `Arc` state); every clone controls the SAME live call.
#[derive(Clone)]
pub struct CallHandle {
    call_id: String,
    /// The registry generation this handle owns, so hangup only tears down THIS call and not a
    /// same-call-id replacement (glare/retry) that superseded it.
    generation: u64,
    /// The call's peer and creator, kept so a consumer can drive `voip().terminate(..)` straight off
    /// the handle without separately tracking the signaling metadata. `peer_jid` is the bare LID the
    /// offer rang; `peer_jid()` upgrades it to the answering device once an `<accept>` arrives.
    peer_jid: Jid,
    call_creator: Jid,
    client_registry: Arc<wacore::voip::CallRegistry>,
    /// The same map `voip().call()` parked this call's relay-attach material in. A dormant outgoing
    /// hangup (engine not yet attached) must drop its entry here AND notify `ended` itself, since no
    /// engine task exists yet to fire the drop-guard.
    pending_outgoing_calls:
        Arc<std::sync::Mutex<std::collections::HashMap<String, PendingOutgoing>>>,
    /// Weak so a lingering handle can't keep the client alive; upgraded to send `<video>` stanzas.
    client: std::sync::Weak<Client>,
    muted: Arc<AtomicBool>,
    video: Arc<VideoShared>,
    events: async_channel::Receiver<CallEvent>,
    ended: Arc<EndedFlag>,
}

fn ensure_group_invite_capacity(
    snapshot: &GroupCallUpdate,
    existing_only: bool,
) -> Result<(), CallError> {
    if !existing_only && snapshot.participants.len() >= GROUP_CALL_MAX_PARTICIPANTS {
        return Err(CallError::Media("group participant limit reached"));
    }
    let connected = snapshot
        .participants
        .iter()
        .filter(|participant| participant.state.as_deref() == Some("connected"))
        .count();
    if snapshot.connected_limit != 0 && connected >= snapshot.connected_limit as usize {
        return Err(CallError::Media(
            "group connected-participant limit reached",
        ));
    }
    Ok(())
}

fn group_invite_target_matches(
    participant: &GroupCallParticipant,
    target: &Jid,
    requested_target: &Jid,
) -> bool {
    let matches = |candidate: &Jid| {
        [target, requested_target]
            .iter()
            .any(|identity| candidate.user == identity.user && candidate.server == identity.server)
    };
    matches(&participant.jid)
        || participant.pn.as_ref().is_some_and(matches)
        || participant
            .devices
            .iter()
            .any(|device| matches(&device.jid))
}

fn current_group_invite_offer_context(
    registry: &wacore::voip::CallRegistry,
    call_id: &str,
    generation: u64,
    target: &Jid,
    requested_target: &Jid,
    existing_only: bool,
) -> Result<(Vec<GroupCallParticipant>, bool), CallError> {
    if let Some(state) = registry.group_state_if_current(call_id, generation)
        && let Some(snapshot) = state.snapshot()
    {
        let member = snapshot
            .participants
            .iter()
            .find(|participant| group_invite_target_matches(participant, target, requested_target));
        ensure_group_invite_capacity(snapshot, existing_only)?;
        let participants = if existing_only {
            let member =
                member.ok_or(CallError::Media("ring target does not belong to the call"))?;
            if member.state.as_deref() == Some("connected") {
                return Err(CallError::Media("ring target is already connected"));
            }
            snapshot
                .participants
                .iter()
                .filter(|participant| {
                    participant.state.as_deref() == Some("connected")
                        && !group_invite_target_matches(participant, target, requested_target)
                })
                .cloned()
                .collect::<Vec<_>>()
        } else {
            if member.is_some() {
                return Err(CallError::Media(
                    "invite target already belongs to the call",
                ));
            }
            snapshot
                .participants
                .iter()
                .filter(|participant| participant.state.as_deref() == Some("connected"))
                .cloned()
                .collect()
        };
        return Ok((participants, snapshot.media == "video"));
    }
    if existing_only {
        return Err(CallError::Media(
            "ring target does not belong to an authoritative group roster",
        ));
    }
    let session = registry
        .snapshot_if_current(call_id, generation)
        .ok_or(CallError::Media("call is no longer active"))?;
    let participants = registry
        .group_invite_fallback_roster(call_id, generation)
        .ok_or(CallError::Media(
            "active device capabilities are not ready for group promotion",
        ))?;
    Ok((participants, session.is_video))
}

fn group_video_upgrade_allowed(group: &wacore::voip::GroupCallState) -> bool {
    !group
        .waiting_room()
        .is_some_and(|room| room.media == CallLinkMedia::Audio)
        && group
            .snapshot()
            .is_some_and(|snapshot| snapshot.media == "video")
}

impl CallHandle {
    /// The call-id this handle controls.
    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    /// The peer this call is with, as the `<terminate>` target. For an outgoing call this is the
    /// call-scoped JID after an ad-hoc group promotion, otherwise the callee device that answered
    /// (learned from the inbound `<accept>`) once one has, since direct-call signaling is addressed
    /// per device. Before either transition it is the bare peer the offer rang.
    pub fn peer_jid(&self) -> Jid {
        if self
            .client_registry
            .group_state_if_current(&self.call_id, self.generation)
            .is_some()
        {
            return Jid::new(&self.call_id, Server::Call);
        }
        self.client_registry
            .answering_device_if_current(&self.call_id, self.generation)
            .unwrap_or_else(|| self.peer_jid.clone())
    }

    /// The call's creator JID, as carried in the signaling (needed by `voip().terminate(..)`).
    pub fn call_creator(&self) -> &Jid {
        &self.call_creator
    }

    /// Latest transaction-ordered group state, including waiting-room and participant controls.
    pub fn group_state(&self) -> Option<wacore::voip::GroupCallState> {
        self.client_registry
            .group_state_if_current(&self.call_id, self.generation)
    }

    /// Invite a user who does not yet belong to the current authoritative roster.
    pub async fn invite_participant(&self, target: &Jid) -> Result<(), CallError> {
        self.send_participant_invite(target, false).await
    }

    /// Ring a disconnected user already present in the current authoritative roster.
    pub async fn ring_participant(&self, target: &Jid) -> Result<(), CallError> {
        self.send_participant_invite(target, true).await
    }

    async fn send_participant_invite(
        &self,
        target: &Jid,
        existing_only: bool,
    ) -> Result<(), CallError> {
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        let requested_target = target.to_non_ad();
        let target = client
            .resolve_recipient_to_lid(target)
            .await
            .ok_or(CallError::NoDevices)?
            .to_non_ad();
        let (initial_participants, _) = current_group_invite_offer_context(
            &self.client_registry,
            &self.call_id,
            self.generation,
            &target,
            &requested_target,
            existing_only,
        )?;
        if initial_participants.is_empty() {
            return Err(CallError::Media("call has no connected invite roster"));
        }
        let devices = drop_hosted_devices(
            client
                .get_user_devices(std::slice::from_ref(&target))
                .await
                .map_err(|error| CallError::Setup(error.to_string()))?,
        );
        if devices.is_empty() {
            return Err(CallError::NoDevices);
        }
        let transition_lock = self
            .client_registry
            .group_transition_lock(&self.call_id, self.generation)
            .ok_or(CallError::Media("call is no longer active"))?;
        let _transition_guard = transition_lock.lock().await;
        self.ensure_current()?;
        let (participants, video) = current_group_invite_offer_context(
            &self.client_registry,
            &self.call_id,
            self.generation,
            &target,
            &requested_target,
            existing_only,
        )?;
        if participants.is_empty() {
            return Err(CallError::Media("call has no connected invite roster"));
        }
        let request_id = client.generate_request_id();
        let node = build_group_invite_offer(&GroupInviteOfferParams {
            call_id: &self.call_id,
            id: &request_id,
            to: &target,
            call_creator: &self.call_creator,
            target_devices: &devices,
            participants: &participants,
            video,
            device_orientation: self.local_video_orientation(),
        })
        .map_err(|error| CallError::Response(error.to_string()))?;
        self.ensure_current()?;
        client.send_node(node).await?;
        Ok(())
    }

    /// Publish the local persistent raise/lower-hand state.
    pub async fn set_hand_raised(&self, raised: bool) -> Result<(), CallError> {
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        client
            .voip()
            .set_hand_raised_for_generation(
                &self.call_id,
                &self.call_creator,
                self.generation,
                raised,
            )
            .await
    }

    /// Send an RTC reaction; an empty string removes the local participant's previous reaction.
    pub fn send_reaction(&self, emoji: impl Into<String>) -> Result<(), CallError> {
        self.ensure_current()?;
        if self.client_registry.send_group_reaction_if_current(
            &self.call_id,
            self.generation,
            emoji.into(),
        ) {
            Ok(())
        } else {
            Err(CallError::Media("group app-data stream is unavailable"))
        }
    }

    /// Switch the local video source role to screen sharing.
    pub async fn start_screen_share(&self, screen_share_id: Option<u32>) -> Result<(), CallError> {
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        client
            .voip()
            .set_screen_share_for_generation(
                &self.call_id,
                &self.call_creator,
                self.generation,
                ScreenShareState::Started,
                screen_share_id,
            )
            .await
    }

    /// Return the local video source role to the camera.
    pub async fn stop_screen_share(&self) -> Result<(), CallError> {
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        client
            .voip()
            .set_screen_share_for_generation(
                &self.call_id,
                &self.call_creator,
                self.generation,
                ScreenShareState::Stopped,
                None,
            )
            .await
    }

    /// Enable or disable approval for this call-link call.
    pub async fn set_approval_required(&self, enabled: bool) -> Result<(), CallError> {
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        client
            .voip()
            .set_approval_required_for_generation(
                &self.call_id,
                &self.call_creator,
                self.generation,
                enabled,
            )
            .await
    }

    /// Admit one waiting-room user.
    pub async fn admit_waiting_user(&self, user: &Jid) -> Result<(), CallError> {
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        client
            .voip()
            .admit_waiting_user_for_generation(
                &self.call_id,
                &self.call_creator,
                self.generation,
                user,
            )
            .await
    }

    /// Deny one waiting-room user.
    pub async fn deny_waiting_user(&self, user: &Jid) -> Result<(), CallError> {
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        client
            .voip()
            .deny_waiting_user_for_generation(
                &self.call_id,
                &self.call_creator,
                self.generation,
                user,
            )
            .await
    }

    /// Mute or unmute the local microphone. While muted the engine sends DTX comfort-noise (the
    /// stream stays fed); it does not gap, so the peer doesn't re-negotiate the transport.
    /// This affects PCM audio only; encoded-audio callers must mute their external source.
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    /// Whether the microphone is currently muted.
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// The rotation to stamp on a `<video>` we are about to send. `0` for a call
    /// that has already gone: the stanza is on its way out either way, and an
    /// upright default is the one that cannot describe a rotation that never
    /// happened.
    fn local_video_orientation(&self) -> u8 {
        self.client_registry
            .local_video_orientation(&self.call_id, self.generation)
            .unwrap_or(0)
    }

    /// Announce this side's camera rotation to the peer, as quarter turns in
    /// `0..=3`. See `CallEntry::self_video_orientation` for what the value is
    /// and what it is not.
    ///
    /// Every `<video>` this call sends from now on carries it. While video is
    /// running the change is announced at once, as a `<video state=1>` that
    /// re-states the current state: that is the shape the peer's own rotations
    /// arrive in, and a no-op there beyond the new orientation. A call still
    /// ringing has nobody to tell — the peer has no call entry to apply it
    /// against — so the value waits and is announced when they answer.
    ///
    /// `Err` when the call is over, when the value is out of range, or when the
    /// stanza could not be sent. A value rejected for range is not stored.
    pub async fn set_video_orientation(&self, orientation: u8) -> Result<(), CallError> {
        self.ensure_current()?;
        // Range first: rejecting it costs nothing and does not need the lock.
        if orientation > 3 {
            return Err(CallError::Media(
                "video orientation must be quarter turns in 0..=3",
            ));
        }

        // Store and announce under the same lock the state transitions take. Two
        // handles rotating at once would otherwise interleave — one storing `1`
        // and pausing while the other stores and sends `2` — and the first would
        // then send its stale `1`, leaving the peer at a rotation the registry
        // does not record. The lock also keeps a rotation from putting a
        // `state=1` on the wire after `stop_video`'s `state=6`.
        let transition_lock = self
            .client_registry
            .video_transition_lock(&self.call_id, self.generation)
            .ok_or(CallError::Media("call no longer active"))?;
        let _transition_guard = transition_lock.lock().await;
        self.ensure_current()?;

        if !self.client_registry.set_local_video_orientation(
            &self.call_id,
            self.generation,
            orientation,
        ) {
            return Err(CallError::Media("call no longer active"));
        }

        // Nothing to announce until there is a direction to announce it for, and
        // nobody to announce it to until the call is answered: a video-from-start
        // offer sets `self_state` to Enabled while the peer is still ringing, and
        // it has no call entry to apply a `<video>` against, so one sent now is
        // dropped. The value is kept either way — `announce_orientation_on_accept`
        // sends it when the peer answers, and the stanza that enables video
        // carries it for every other path.
        let announceable = self
            .client_registry
            .video_states(&self.call_id, self.generation)
            .is_some_and(|(self_state, _)| self_state == VideoState::Enabled)
            && self
                .client_registry
                .phase_if_current(&self.call_id, self.generation)
                .is_some_and(|phase| phase != CallPhase::Ringing);
        if !announceable {
            return Ok(());
        }

        let client = self.upgrade_client()?;
        let stanza = build_video_state(&VideoStateParams {
            call_id: &self.call_id,
            to: &self.peer_jid(),
            id: &client.generate_request_id(),
            call_creator: &self.call_creator,
            state: VideoState::Enabled,
            dec: Some(VIDEO_DEC_REQUEST),
            // Read back rather than reusing the argument: under the lock they
            // agree, and reading makes that the invariant rather than a habit.
            device_orientation: Some(self.local_video_orientation()),
        });
        client.send_node(stanza).await?;
        Ok(())
    }

    /// UPGRADE the call to video (we initiate): attaches the endpoints, enables the media plane,
    /// and sends `<video state=11 dec="H264" device_orientation="0" voip_settings="video">`.
    /// The peer answers with
    /// `UpgradeAccept`/`Enabled` (surfaced as [`CallEvent::VideoStateChanged`]); media flows as
    /// soon as both planes are up. Also the way to start media on a `.video()` call whose builder
    /// endpoints you skipped.
    pub async fn start_video<S, K>(&self, source: S, sink: K) -> Result<(), CallError>
    where
        S: VideoSource,
        K: VideoSink,
    {
        self.begin_video(Arc::new(source), Arc::new(sink), VideoUpgradeRole::Initiate)
            .await
    }

    /// ACCEPT the peer's video upgrade request (a `VideoStateChanged { state: UpgradeRequestV2 }`
    /// event): the token binds this action to that exact request, then the method attaches the
    /// endpoints, enables the media plane, and answers
    /// `<video state=4 dec="H264,AV1" device_orientation="0">` followed by
    /// `<video state=1 dec="H264" device_orientation="0">` (Enabled).
    pub async fn accept_video<S, K>(
        &self,
        request: VideoUpgradeToken,
        source: S,
        sink: K,
    ) -> Result<(), CallError>
    where
        S: VideoSource,
        K: VideoSink,
    {
        self.begin_video(
            Arc::new(source),
            Arc::new(sink),
            VideoUpgradeRole::Accept(request),
        )
        .await
    }

    /// Send the standalone `<video state=1 dec="H264" device_orientation="0">` used after a
    /// mid-call video upgrade. Captured video-from-start callees do not need this extra stanza.
    pub async fn announce_video_enabled(&self) -> Result<(), CallError> {
        self.ensure_current()?;
        let transition_lock = self
            .client_registry
            .video_transition_lock(&self.call_id, self.generation)
            .ok_or(CallError::Media("call no longer active"))?;
        let _transition_guard = transition_lock.lock().await;
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        let stanza = build_video_state(&VideoStateParams {
            call_id: &self.call_id,
            to: &self.peer_jid(),
            id: &client.generate_request_id(),
            call_creator: &self.call_creator,
            state: VideoState::Enabled,
            dec: Some(VIDEO_DEC_REQUEST),
            device_orientation: Some(self.local_video_orientation()),
        });
        client.send_node(stanza).await?;
        Ok(())
    }

    /// Stop our video direction: sends `<video state=6>` (Stopped, no marker), tears the local
    /// video plane down, and releases the source/sink. The peer may keep sending its direction;
    /// audio is untouched. Idempotent.
    pub async fn stop_video(&self) -> Result<(), CallError> {
        self.ensure_current()?;
        let transition_lock = self
            .client_registry
            .video_transition_lock(&self.call_id, self.generation)
            .ok_or(CallError::Media("call no longer active"))?;
        let _transition_guard = transition_lock.lock().await;
        self.ensure_current()?;
        // Tear local media down FIRST, matching `Voip::terminate`: the app asked to stop video, so
        // a failed signaling send must NOT leave the camera streaming. If the peer misses the
        // Stopped it keeps sending video, but our plane is disabled so those PT-97 packets drop.
        self.release_local_video();
        self.client_registry
            .stop_local_video(&self.call_id, self.generation);
        let client = self.upgrade_client()?;
        let stanza = build_video_state(&VideoStateParams {
            call_id: &self.call_id,
            to: &self.peer_jid(),
            id: &client.generate_request_id(),
            call_creator: &self.call_creator,
            state: VideoState::Stopped,
            dec: None,
            // No direction left for a rotation to describe. `None` omits the
            // attribute rather than asserting `0`, which is a real rotation.
            device_orientation: None,
        });
        client.send_node(stanza).await?;
        Ok(())
    }

    /// Release local codec endpoints without changing the directional signaling state.
    fn release_local_video(&self) {
        release_video_endpoints(
            &self.client_registry,
            &self.pending_outgoing_calls,
            &self.video,
            &self.call_id,
            self.generation,
        );
    }

    /// Shared upgrade/accept body: attach endpoints, enable the plane, send the handshake stanzas.
    async fn begin_video(
        &self,
        source: Arc<dyn VideoSource>,
        sink: Arc<dyn VideoSink>,
        role: VideoUpgradeRole,
    ) -> Result<(), CallError> {
        // A stale handle (superseded by a same-call-id glare/retry) must not attach endpoints or
        // send `<video>` for a call it no longer owns — that would mutate the replacement's state.
        self.ensure_current()?;
        if source.rtp_timestamp_stride() == 0 {
            return Err(CallError::Media(
                "video RTP timestamp stride must be non-zero",
            ));
        }
        let client = self.upgrade_client()?;
        // Group downgrades also reset video negotiation and release its endpoints. Take their lane
        // first so eligibility, request publication, endpoint attachment, and teardown-hook
        // replacement are one transition relative to an authoritative audio-only snapshot.
        let group_transition_lock = self
            .client_registry
            .group_transition_lock(&self.call_id, self.generation)
            .ok_or(CallError::Media("call no longer active"))?;
        let _group_transition_guard = group_transition_lock.lock().await;
        self.ensure_current()?;
        let transition_lock = self
            .client_registry
            .video_transition_lock(&self.call_id, self.generation)
            .ok_or(CallError::Media("call no longer active"))?;
        let transition_guard = transition_lock.lock().await;
        self.ensure_current()?;
        if matches!(role, VideoUpgradeRole::Initiate)
            && let Some(group) = self
                .client_registry
                .group_state_if_current(&self.call_id, self.generation)
            && !group_video_upgrade_allowed(&group)
        {
            return Err(CallError::Media(
                "group media mode does not allow a video upgrade",
            ));
        }
        let request_epoch = match role {
            VideoUpgradeRole::Initiate => Some(
                self.client_registry
                    .begin_local_video_request(&self.call_id, self.generation)
                    .ok_or(CallError::Media("video transition already in progress"))?,
            ),
            VideoUpgradeRole::Accept(request) => {
                if request.generation() != self.generation
                    || !self
                        .client_registry
                        .peer_video_request_is_current(&self.call_id, request)
                {
                    return Err(CallError::VideoUpgradeExpired);
                }
                None
            }
        };
        self.video
            .attach_endpoints(&client, &source, &sink, self.ended.clone());
        if !self.client_registry.set_video_teardown(
            &self.call_id,
            self.generation,
            video_teardown_hook(&self.video),
        ) {
            self.video.detach_endpoints();
            if let Some(epoch) = request_epoch {
                self.client_registry
                    .end_local_video_request(&self.call_id, self.generation, epoch);
            }
            return Err(CallError::Media("call no longer active"));
        }
        // The upgrade INITIATOR holds outbound video until the peer accepts (the handler ungates on
        // the peer's UpgradeAccept/Enabled). The acceptor's peer already requested, so it may send
        // immediately.
        let ctl = match role {
            VideoUpgradeRole::Initiate => VideoControl::EnableAwaitingAccept,
            VideoUpgradeRole::Accept(_) => VideoControl::Enable,
        };
        self.video.send_control(ctl);

        let peer = self.peer_jid();
        let send_state = |state: VideoState, dec: Option<&'static str>| {
            // Each call stanza gets its own wrapper id so the peer's typed ack correlates.
            let stanza = build_video_state(&VideoStateParams {
                call_id: &self.call_id,
                to: &peer,
                id: &client.generate_request_id(),
                call_creator: &self.call_creator,
                state,
                dec,
                // The rotation rides every stanza that announces a direction, so
                // one set before video started is announced by the stanza that
                // starts it rather than waiting for the next rotation. Read per
                // stanza, so a rotation between the two an upgrade sends lands on
                // the second.
                device_orientation: Some(self.local_video_orientation()),
            });
            let client = client.clone();
            async move { client.send_node(stanza).await }
        };
        let send_result = match role {
            VideoUpgradeRole::Initiate => {
                if let Err(e) =
                    send_state(VideoState::UpgradeRequestV2, Some(VIDEO_DEC_REQUEST)).await
                {
                    Err(e)
                } else {
                    Ok(())
                }
            }
            VideoUpgradeRole::Accept(_) => {
                if let Err(e) = send_state(VideoState::UpgradeAccept, Some(VIDEO_DEC_ACCEPT)).await
                {
                    Err(e)
                } else if let Err(e) =
                    send_state(VideoState::Enabled, Some(VIDEO_DEC_REQUEST)).await
                {
                    // The peer times out an incomplete handshake; keep local media aligned with it.
                    warn!(
                        "voip: video upgrade handshake failed call_id={} phase=enabled_after_accept error={e}",
                        self.call_id
                    );
                    Err(e)
                } else {
                    Ok(())
                }
            }
        };
        if let Err(e) = send_result {
            self.release_local_video();
            if let Some(epoch) = request_epoch {
                self.client_registry
                    .end_local_video_request(&self.call_id, self.generation, epoch);
            } else {
                self.client_registry
                    .reset_video(&self.call_id, self.generation);
            }
            return Err(e.into());
        }

        match role {
            VideoUpgradeRole::Initiate => {
                drop(transition_guard);
                if let Some(epoch) = request_epoch {
                    self.spawn_video_upgrade_timeout(epoch, client);
                }
            }
            VideoUpgradeRole::Accept(request) => {
                if !self
                    .client_registry
                    .complete_peer_video_request(&self.call_id, request)
                {
                    self.release_local_video();
                    self.client_registry
                        .reset_video(&self.call_id, self.generation);
                    return Err(CallError::VideoUpgradeExpired);
                }
            }
        }
        Ok(())
    }

    fn spawn_video_upgrade_timeout(&self, epoch: u64, client: Arc<Client>) {
        let runtime = client.runtime.clone();
        let sleeper = runtime.clone();
        let registry = self.client_registry.clone();
        let pending = self.pending_outgoing_calls.clone();
        let video = self.video.clone();
        let weak_client = Arc::downgrade(&client);
        let call_id = self.call_id.clone();
        let generation = self.generation;
        let peer = self.peer_jid();
        let call_creator = self.call_creator.clone();
        runtime
            .spawn(Box::pin(async move {
                sleeper.sleep(VIDEO_UPGRADE_TIMEOUT).await;
                let Some(transition_lock) = registry.video_transition_lock(&call_id, generation)
                else {
                    return;
                };
                let _transition_guard = transition_lock.lock().await;
                if !registry.end_local_video_request(&call_id, generation, epoch) {
                    return;
                }
                release_video_endpoints(&registry, &pending, &video, &call_id, generation);
                let Some(client) = weak_client.upgrade() else {
                    return;
                };
                let stanza = build_video_state(&VideoStateParams {
                    call_id: &call_id,
                    to: &peer,
                    id: &client.generate_request_id(),
                    call_creator: &call_creator,
                    state: VideoState::UpgradeCancelByTimeout,
                    dec: None,
                    device_orientation: None,
                });
                if let Err(e) = client.send_node(stanza).await {
                    warn!("voip: failed to announce video upgrade timeout call_id={call_id}: {e}");
                }
            }))
            .detach();
    }

    fn upgrade_client(&self) -> Result<Arc<Client>, CallError> {
        self.client
            .upgrade()
            .ok_or(CallError::Media("client dropped"))
    }

    /// Reject a video op from a handle that no longer owns its call-id (superseded by a same-call-id
    /// replacement), so it can't drive the replacement's video state.
    fn ensure_current(&self) -> Result<(), CallError> {
        if self.client_registry.generation_of(&self.call_id) == Some(self.generation) {
            Ok(())
        } else {
            Err(CallError::Media("call no longer active"))
        }
    }

    /// Tear the call down: abort the media task (which closes the relay and the audio channels).
    /// Idempotent. Signaling `<terminate>` is a separate concern; send it via
    /// [`Voip::terminate`](crate::Voip::terminate) if the peer must be told.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.voip.hangup",
            level = "debug",
            skip_all,
            fields(call_id = %self.call_id)
        )
    )]
    pub async fn hangup(&self) {
        // Generation-guarded: if a same-call-id glare/retry replaced this call under a newer
        // generation, this no-ops instead of aborting the replacement. For an ATTACHED call this
        // aborts the media task, whose drop-guard notifies `ended`. The bool reports whether we
        // owned and removed the current registration.
        let removed_registry = self
            .client_registry
            .remove_if_current(&self.call_id, self.generation);

        // A DORMANT outgoing call (relay not yet arrived, no engine task) still has its relay-attach
        // material parked here. Drop it so it can't leak or later resurrect, guarded by generation so
        // a superseded handle doesn't evict a live replacement's pending entry.
        let removed_pending =
            take_pending_if_current(&self.pending_outgoing_calls, &self.call_id, self.generation);

        // The call's `ring_devices` (sibling-dismiss tracking) live on the registry session, so the
        // `remove_if_current` above already dropped them -- no separate map to clear.
        //
        // Notify `ended` whenever we actually removed our own registration. For an attached call this
        // is redundant with the task drop-guard; it's load-bearing in the window where the relay dial
        // (attach_engine's connect) is still in flight -- no media task exists yet to fire a drop-guard
        // and the PendingOutgoing was already consumed, so nothing else would wake wait_ended() or stop
        // the dial. A superseded/already-gone handle removed nothing and stays quiet.
        if removed_registry || removed_pending.is_some() {
            self.ended.notify();
        }
    }

    /// Subscribe to call lifecycle and media diagnostics.
    ///
    /// All receivers returned here (and across cloned handles) share ONE queue: each event is
    /// delivered to exactly one receiver, competitively. Drive a single consumer loop per call;
    /// polling two receivers concurrently splits the events between them rather than broadcasting.
    pub fn events(&self) -> async_channel::Receiver<CallEvent> {
        self.events.clone()
    }

    /// Resolve once the call's media task has finished (relay disconnect, send failure, or hangup).
    pub async fn wait_ended(&self) {
        // Sticky: returns at once if the call already ended, so a stale handle (superseded by a
        // same-call-id replacement, whose task already ended and set the flag) never parks on a
        // one-shot notification that already fired.
        self.ended.wait().await;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use bytes::Bytes;
    use std::sync::Mutex;
    use std::sync::atomic::AtomicUsize;
    use wacore::voip::relay_parse::{RelayAddress, RelayData, RelayEndpoint};
    use wacore::voip::transport::{RelayDisconnectReason, RelayTransport, RelayTransportEvent};
    use wacore_binary::{Jid, Server};

    use crate::store::persistence_manager::PersistenceManager;
    use crate::store::traits::Backend;
    use crate::test_utils::{MockHttpClient, create_test_backend, seed_peer_session};

    async fn make_client() -> Arc<Client> {
        let client = crate::test_utils::create_test_client().await;
        // The facade's connect path gates on is_connected; mark connected for the unit tests.
        client.set_connected_for_test(true);
        client
    }

    async fn install_noise_transport(
        client: &Client,
        transport: Arc<dyn crate::transport::Transport>,
    ) {
        use wacore::handshake::NoiseCipher;

        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            transport,
            NoiseCipher::new(&key).expect("key"),
            NoiseCipher::new(&key).expect("key"),
        );
        *client.noise_socket.lock().unwrap() = Some(Arc::new(noise_socket));
    }

    struct GatedSendTransport {
        gate_attempt: usize,
        fail_gated_send: bool,
        attempts: AtomicUsize,
        entered: async_channel::Sender<()>,
        release: async_channel::Receiver<()>,
    }

    #[async_trait]
    impl crate::transport::Transport for GatedSendTransport {
        async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
            if self.attempts.fetch_add(1, Ordering::SeqCst) != self.gate_attempt {
                return Ok(());
            }
            self.entered
                .send(())
                .await
                .map_err(|_| anyhow::anyhow!("send-gate observer closed"))?;
            self.release
                .recv()
                .await
                .map_err(|_| anyhow::anyhow!("send gate closed"))?;
            if self.fail_gated_send {
                Err(anyhow::anyhow!("injected gated send failure"))
            } else {
                Ok(())
            }
        }

        async fn disconnect(&self) {}
    }

    fn gated_send_transport(
        gate_attempt: usize,
        fail_gated_send: bool,
    ) -> (
        Arc<dyn crate::transport::Transport>,
        async_channel::Receiver<()>,
        async_channel::Sender<()>,
    ) {
        let (entered_tx, entered_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        (
            Arc::new(GatedSendTransport {
                gate_attempt,
                fail_gated_send,
                attempts: AtomicUsize::new(0),
                entered: entered_tx,
                release: release_rx,
            }),
            entered_rx,
            release_tx,
        )
    }

    fn caller() -> Jid {
        Jid::new("222222222222222", Server::Lid)
    }

    fn sample_relay() -> RelayData {
        RelayData {
            relay_key_ascii: Some(b"relay-key".to_vec()),
            warp_mi_tag_len: Some(4),
            relay_tokens: vec![vec![0xAB; 16]],
            endpoints: vec![RelayEndpoint {
                relay_id: 1,
                relay_name: "gru1c02".into(),
                token_id: 0,
                auth_token_id: 1,
                addresses: vec![RelayAddress {
                    protocol: 0,
                    ipv4: Some("203.0.113.7".into()),
                    ipv6: None,
                    port: 3478,
                }],
                ..Default::default()
            }],
            ..Default::default()
        }
    }

    fn sample_group_relay(transaction_id: u32) -> wacore::types::group_call::GroupCallRelay {
        use wacore::types::group_call::{GroupCallRelay, GroupCallRelayEndpoint};

        GroupCallRelay::builder()
            .transaction_id(transaction_id)
            .self_pid(1)
            .uuid("TEST-RELAY".to_string())
            .participant_uuid("TEST-PARTICIPANT".to_string())
            .attribute_padding(false)
            .warp_mi_tag_len(4)
            .key(vec![7; 32])
            .tokens(vec![vec![9; 16]])
            .endpoints(vec![
                GroupCallRelayEndpoint::builder()
                    .relay_id(1)
                    .token_id(0)
                    .auth_token_id(0)
                    .relay_name("test-relay".to_string())
                    .is_fna(false)
                    .ipv4("203.0.113.7".to_string())
                    .port(3478)
                    .build(),
            ])
            .build()
    }

    fn mk_session() -> wacore::voip::CallSession {
        wacore::voip::CallSession::new_incoming("CID-FACADE", caller(), caller())
    }

    fn rekey_update(client: &Client, recipients: &[Jid]) -> GroupCallUpdate {
        let own_lid = client.lid().expect("own lid");
        let mut own_device = GroupCallDevice::new(own_lid.clone());
        own_device.pid = Some(1);
        let mut participants = vec![GroupCallParticipant::new(
            own_lid.to_non_ad(),
            vec![own_device],
        )];
        participants[0].state = Some("connected".to_string());
        participants.extend(recipients.iter().enumerate().map(|(index, recipient)| {
            let mut device = GroupCallDevice::new(recipient.clone());
            device.pid = Some(index as u32 + 2);
            let mut participant = GroupCallParticipant::new(recipient.to_non_ad(), vec![device]);
            participant.state = Some("connected".to_string());
            participant
        }));
        GroupCallUpdate::builder()
            .call_id("00abcdef0123456789abcdef01234567".to_string())
            .call_creator(own_lid)
            .transaction_id(7)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(true)
            .participants(participants)
            .build()
    }

    fn register_group_update(client: &Client, update: &GroupCallUpdate) -> u64 {
        let mut session = wacore::voip::CallSession::new_incoming(
            &update.call_id,
            update.call_creator.clone(),
            update.call_creator.clone(),
        );
        session.group = Some(update.clone());
        client.call_registry().insert(session)
    }

    fn pcm_audio(source: Arc<dyn AudioSource>, sink: Arc<dyn AudioSink>) -> AudioEndpoints {
        AudioEndpoints::Pcm { source, sink }
    }

    fn encoded_audio(format: AudioFormat) -> AudioEndpoints {
        let (_source_tx, source_rx) = async_channel::unbounded::<Bytes>();
        let (sink_tx, _sink_rx) = async_channel::unbounded::<EncodedAudioFrame>();
        AudioEndpoints::Encoded {
            format,
            source: Arc::new(source_rx),
            sink: Arc::new(sink_tx),
        }
    }

    #[tokio::test]
    async fn group_call_by_id_uses_cached_roster_and_excludes_every_local_identity() {
        use wacore::client::context::GroupInfo;
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};
        use wacore::types::message::AddressingMode;

        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let own_pn = Jid::new("12025550111", Server::Pn);
        let own_lid = Jid::new("111111111111111", Server::Lid).with_device(1);
        for command in [
            crate::store::commands::DeviceCommand::SetId(Some(own_pn.clone())),
            crate::store::commands::DeviceCommand::SetLid(Some(own_lid.clone())),
        ] {
            client.persistence_manager().process_command(command).await;
        }
        let peer_a = Jid::new("222222222222222", Server::Lid);
        let peer_b = Jid::new("333333333333333", Server::Lid);
        for peer in [&peer_a, &peer_b] {
            client
                .update_device_list(DeviceListRecord {
                    user: peer.user.to_string(),
                    devices: vec![DeviceInfo::new(0, None)],
                    timestamp: wacore::time::now_secs(),
                    phash: None,
                    raw_id: None,
                })
                .await
                .expect("seed device list");
        }
        let group = Jid::new("120363000000000001", Server::Group);
        client
            .get_group_cache()
            .insert(
                group.clone(),
                Arc::new(GroupInfo::new(
                    vec![own_pn, own_lid.to_non_ad(), peer_a.clone(), peer_b.clone()],
                    AddressingMode::Lid,
                )),
            )
            .await;

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let start_client = client.clone();
        let start_group = group.clone();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (speaker_tx, _speaker_rx) = async_channel::unbounded::<Vec<i16>>();
        let start = tokio::spawn(async move {
            start_client
                .voip()
                .group_call_by_id(&start_group)
                .audio(mic_rx, speaker_tx)
                .start()
                .await
        });
        let node = sent.await.expect("group offer");
        let node_ref = node.as_node_ref();
        let offer = &node_ref.children().expect("call action")[0];
        assert_eq!(
            offer.attrs().optional_jid("group-jid"),
            Some(group),
            "the roster-derived call must remain bound to its source group"
        );
        let users = offer
            .get_optional_child("group_info")
            .expect("group info")
            .children()
            .expect("group users")
            .iter()
            .map(|user| user.attrs().optional_jid("jid").expect("user jid"))
            .collect::<Vec<_>>();
        assert_eq!(
            users,
            [own_lid.to_non_ad(), peer_a, peer_b],
            "the own PN and LID roster entries must collapse into the single local participant"
        );

        start.abort();
        let _ = start.await;
    }

    #[tokio::test]
    async fn group_call_by_id_rejects_non_group_jid_before_roster_lookup() {
        let client = make_client().await;
        let peer = Jid::new("222222222222222", Server::Lid);
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (speaker_tx, _speaker_rx) = async_channel::unbounded::<Vec<i16>>();
        assert!(matches!(
            client
                .voip()
                .group_call_by_id(&peer)
                .audio(mic_rx, speaker_tx)
                .start()
                .await,
            Err(CallError::Setup(message))
                if message == "group-bound call requires a valid group JID"
        ));
    }

    #[tokio::test]
    async fn oversized_group_targets_are_rejected_before_recipient_resolution() {
        let client = make_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;
        let targets = (0..=GROUP_CALL_MAX_REMOTE_PARTICIPANTS)
            .map(|index| Jid::new(format!("1202555{:04}", 100 + index), Server::Pn))
            .collect::<Vec<_>>();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (speaker_tx, _speaker_rx) = async_channel::unbounded::<Vec<i16>>();

        let result = tokio::time::timeout(
            Duration::from_secs(1),
            client
                .voip()
                .group_call(&targets)
                .audio(mic_rx, speaker_tx)
                .start(),
        )
        .await
        .expect("oversized input must fail without waiting for recipient IQs");
        assert!(matches!(
            result,
            Err(CallError::Setup(message))
                if message
                    == format!(
                        "group call requires 2..={GROUP_CALL_MAX_REMOTE_PARTICIPANTS} remote users"
                    )
        ));
    }

    #[test]
    fn group_invites_enforce_membership_and_connected_limits() {
        let creator = Jid::new("111111111111111", Server::Lid);
        let participant = |index: usize, state: &str| {
            let mut participant = GroupCallParticipant::new(
                Jid::new(format!("200000000000{index:03}"), Server::Lid),
                Vec::new(),
            );
            participant.state = Some(state.to_string());
            participant
        };
        let mut snapshot = GroupCallUpdate::builder()
            .call_id("GROUP-CALL".to_string())
            .call_creator(creator)
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(
                (0..GROUP_CALL_MAX_PARTICIPANTS)
                    .map(|index| participant(index, "disconnected"))
                    .collect(),
            )
            .build();
        assert!(matches!(
            ensure_group_invite_capacity(&snapshot, false),
            Err(CallError::Media("group participant limit reached"))
        ));
        assert!(
            ensure_group_invite_capacity(&snapshot, true).is_ok(),
            "ringing an existing member does not add a membership slot"
        );

        snapshot.participants.truncate(2);
        for participant in &mut snapshot.participants {
            participant.state = Some("connected".to_string());
        }
        snapshot.connected_limit = 2;
        assert!(matches!(
            ensure_group_invite_capacity(&snapshot, false),
            Err(CallError::Media(
                "group connected-participant limit reached"
            ))
        ));
        snapshot.connected_limit = 0;
        assert!(ensure_group_invite_capacity(&snapshot, false).is_ok());
    }

    #[test]
    fn group_invite_context_revalidates_the_latest_roster_and_media() {
        let registry = wacore::voip::CallRegistry::new();
        let creator = Jid::new("111111111111111", Server::Lid);
        let target = Jid::new("222222222222222", Server::Lid);
        let connected = Jid::new("333333333333333", Server::Lid);
        let generation = registry.insert_group(wacore::voip::CallSession::new_outgoing(
            "GROUP-CALL",
            Jid::new("GROUP-CALL", Server::Call),
            creator.clone(),
        ));
        let participant = |jid: Jid, state: &str| {
            let mut participant = GroupCallParticipant::new(jid, Vec::new());
            participant.state = Some(state.to_string());
            participant
        };
        let update = |transaction_id, media: &str, target_state: &str, connected_limit| {
            GroupCallUpdate::builder()
                .call_id("GROUP-CALL".to_string())
                .call_creator(creator.clone())
                .transaction_id(transaction_id)
                .media(media.to_string())
                .connected_limit(connected_limit)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(vec![
                    participant(connected.clone(), "connected"),
                    participant(target.clone(), target_state),
                ])
                .build()
        };

        assert_eq!(
            registry
                .apply_group_update_if_current(update(1, "audio", "disconnected", 32), generation,),
            wacore::voip::GroupStateApply::Applied
        );
        assert!(
            !current_group_invite_offer_context(
                &registry,
                "GROUP-CALL",
                generation,
                &target,
                &target,
                true,
            )
            .expect("initial disconnected target")
            .1
        );

        assert_eq!(
            registry
                .apply_group_update_if_current(update(2, "video", "disconnected", 32), generation,),
            wacore::voip::GroupStateApply::Applied
        );
        assert!(
            current_group_invite_offer_context(
                &registry,
                "GROUP-CALL",
                generation,
                &target,
                &target,
                true,
            )
            .expect("latest disconnected target")
            .1,
            "a newer roster's video mode must replace the pre-await offer mode"
        );

        assert_eq!(
            registry
                .apply_group_update_if_current(update(3, "video", "connected", 32), generation,),
            wacore::voip::GroupStateApply::Applied
        );
        assert!(matches!(
            current_group_invite_offer_context(
                &registry,
                "GROUP-CALL",
                generation,
                &target,
                &target,
                true,
            ),
            Err(CallError::Media("ring target is already connected"))
        ));

        assert_eq!(
            registry
                .apply_group_update_if_current(update(4, "video", "disconnected", 1), generation,),
            wacore::voip::GroupStateApply::Applied
        );
        assert!(matches!(
            current_group_invite_offer_context(
                &registry,
                "GROUP-CALL",
                generation,
                &Jid::new("444444444444444", Server::Lid),
                &Jid::new("444444444444444", Server::Lid),
                false,
            ),
            Err(CallError::Media(
                "group connected-participant limit reached"
            ))
        ));
        registry.remove_if_current("GROUP-CALL", generation);
    }

    #[test]
    fn new_group_invite_context_excludes_disconnected_members() {
        let registry = wacore::voip::CallRegistry::new();
        let creator = Jid::new("111111111111111", Server::Lid);
        let connected = Jid::new("222222222222222", Server::Lid);
        let disconnected = Jid::new("333333333333333", Server::Lid);
        let target = Jid::new("444444444444444", Server::Lid);
        let generation = registry.insert_group(wacore::voip::CallSession::new_outgoing(
            "GROUP-CALL",
            Jid::new("GROUP-CALL", Server::Call),
            creator.clone(),
        ));
        let participant = |jid: Jid, state: &str| {
            let mut participant = GroupCallParticipant::new(jid, Vec::new());
            participant.state = Some(state.to_string());
            participant
        };
        let snapshot = GroupCallUpdate::builder()
            .call_id("GROUP-CALL".to_string())
            .call_creator(creator)
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![
                participant(connected.clone(), "connected"),
                participant(disconnected, "disconnected"),
            ])
            .build();
        assert_eq!(
            registry.apply_group_update_if_current(snapshot, generation),
            wacore::voip::GroupStateApply::Applied
        );

        let (participants, video) = current_group_invite_offer_context(
            &registry,
            "GROUP-CALL",
            generation,
            &target,
            &target,
            false,
        )
        .expect("new target can be invited");
        assert!(!video);
        assert_eq!(participants.len(), 1);
        assert_eq!(participants[0].jid, connected);
    }

    #[test]
    fn group_invite_context_matches_pn_and_lid_roster_aliases() {
        let registry = wacore::voip::CallRegistry::new();
        let creator = Jid::new("111111111111111", Server::Lid);
        let connected = Jid::new("222222222222222", Server::Lid);
        let target_lid = Jid::new("333333333333333", Server::Lid);
        let target_pn = Jid::new("12025550123", Server::Pn);
        let generation = registry.insert_group(wacore::voip::CallSession::new_outgoing(
            "GROUP-CALL",
            Jid::new("GROUP-CALL", Server::Call),
            creator.clone(),
        ));
        let snapshot = GroupCallUpdate::builder()
            .call_id("GROUP-CALL".to_string())
            .call_creator(creator)
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![
                GroupCallParticipant::builder()
                    .jid(connected.clone())
                    .state("connected".to_string())
                    .devices(Vec::new())
                    .build(),
                GroupCallParticipant::builder()
                    .jid(target_pn.clone())
                    .state("disconnected".to_string())
                    .devices(Vec::new())
                    .build(),
            ])
            .build();
        assert_eq!(
            registry.apply_group_update_if_current(snapshot, generation),
            wacore::voip::GroupStateApply::Applied
        );

        let (participants, video) = current_group_invite_offer_context(
            &registry,
            "GROUP-CALL",
            generation,
            &target_lid,
            &target_pn,
            true,
        )
        .expect("the PN request and resolved LID identify the same disconnected member");
        assert!(!video);
        assert_eq!(
            participants
                .iter()
                .map(|participant| participant.jid.clone())
                .collect::<Vec<_>>(),
            vec![connected]
        );
        assert!(matches!(
            current_group_invite_offer_context(
                &registry,
                "GROUP-CALL",
                generation,
                &target_lid,
                &target_pn,
                false,
            ),
            Err(CallError::Media(
                "invite target already belongs to the call"
            ))
        ));
    }

    #[test]
    fn initial_group_offer_ack_must_preserve_requested_media() {
        let update = |media: &str| {
            GroupCallUpdate::builder()
                .call_id("GROUP-CALL".to_string())
                .call_creator(Jid::new("111111111111111", Server::Lid))
                .transaction_id(1)
                .media(media.to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(Vec::new())
                .build()
        };

        assert!(ensure_group_offer_media(&update("audio"), false).is_ok());
        assert!(ensure_group_offer_media(&update("video"), true).is_ok());
        assert!(ensure_group_offer_media(&update("audio"), true).is_err());
        assert!(ensure_group_offer_media(&update("video"), false).is_err());
    }

    #[test]
    fn overtaken_group_offer_ack_keeps_its_unfulfilled_rekey_request() {
        let ack = GroupCallUpdate::builder()
            .call_id("GROUP-CALL".to_string())
            .call_creator(Jid::new("111111111111111", Server::Lid))
            .transaction_id(5)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(true)
            .participants(Vec::new())
            .build();
        let mut current = ack.clone();
        current.transaction_id = 6;
        current.rekey_requested = false;

        assert_eq!(
            group_offer_epoch_update(&ack, &current, None).map(|update| update.transaction_id),
            Some(6),
            "the ACK request must fan out against the authoritative overtaking roster"
        );
        assert!(group_offer_epoch_update(&ack, &current, Some(4)).is_some());
        assert!(group_offer_epoch_update(&ack, &current, Some(5)).is_none());
        assert!(group_offer_epoch_update(&ack, &current, Some(6)).is_none());
    }

    #[test]
    fn delayed_call_link_admission_must_preserve_identity_and_requested_media() {
        let creator = Jid::new("111111111111111", Server::Lid);
        let update = |media: &str| {
            GroupCallUpdate::builder()
                .call_id("GROUP-CALL".to_string())
                .call_creator(creator.clone())
                .transaction_id(1)
                .media(media.to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(Vec::new())
                .build()
        };

        assert!(ensure_call_link_admitted_media(&update("audio"), CallLinkMedia::Audio).is_ok());
        assert!(ensure_call_link_admitted_media(&update("video"), CallLinkMedia::Video).is_ok());
        assert!(ensure_call_link_admitted_media(&update("audio"), CallLinkMedia::Video).is_err());
        assert!(ensure_call_link_admitted_media(&update("video"), CallLinkMedia::Audio).is_err());
        assert!(
            ensure_call_link_admitted_snapshot(&update("audio"), &creator, CallLinkMedia::Audio)
                .is_ok()
        );
        assert!(
            ensure_call_link_admitted_snapshot(
                &update("audio"),
                &Jid::new("222222222222222", Server::Lid),
                CallLinkMedia::Audio,
            )
            .is_err()
        );
    }

    #[tokio::test]
    async fn accept_rejects_an_audio_profile_the_peer_did_not_offer() {
        let client = make_client().await;
        let incoming = IncomingCall::new_for_test(
            caller(),
            "STANZA-AUDIO-MISMATCH".into(),
            wacore::time::from_secs(1_700_000_000).expect("timestamp"),
            CallAction::Offer {
                call_id: "CALL-AUDIO-MISMATCH".into(),
                call_creator: caller(),
                caller_pn: None,
                caller_country_code: None,
                device_class: None,
                joinable: false,
                is_video: false,
                audio: vec![wacore::types::call::CallAudioCodec {
                    enc: "opus".into(),
                    rate: 8_000,
                }],
                group_jid: None,
            },
        );
        let (_source_tx, source_rx) = async_channel::unbounded::<Bytes>();
        let (sink_tx, _sink_rx) = async_channel::unbounded::<EncodedAudioFrame>();

        let result = client
            .voip()
            .accept(&incoming)
            .encoded_audio(AudioFormat::OPUS_16KHZ_60MS, source_rx, sink_tx)
            .start()
            .await;

        assert!(matches!(
            result,
            Err(CallError::AudioFormatNotOffered(16_000))
        ));
    }

    fn incoming_offer(video: bool) -> IncomingCall {
        IncomingCall::new_for_test(
            caller(),
            "STANZA-ANSWER-SIGNALING".into(),
            wacore::time::from_secs(1_700_000_000).expect("timestamp"),
            CallAction::Offer {
                call_id: "CALL-ANSWER-SIGNALING".into(),
                call_creator: caller(),
                caller_pn: None,
                caller_country_code: None,
                device_class: None,
                joinable: false,
                is_video: video,
                audio: vec![wacore::types::call::CallAudioCodec {
                    enc: "opus".into(),
                    rate: 16_000,
                }],
                group_jid: None,
            },
        )
    }

    async fn register_answer(client: &Client, incoming: &IncomingCall) -> RegisteredCall {
        let mut session = wacore::voip::CallSession::new_incoming(
            incoming.action.call_id(),
            incoming.from.clone(),
            incoming.action.call_creator().clone(),
        );
        session.audio_format = Some(AudioFormat::MLOW_16KHZ_60MS);
        RegisteredCall::new(client, session).await
    }

    #[tokio::test]
    async fn audio_offer_rejects_from_start_video_endpoints() {
        let client = make_client().await;
        let incoming = incoming_offer(false);
        let (_audio_tx, audio_rx) = async_channel::unbounded::<Bytes>();
        let (audio_out, _audio_out_rx) = async_channel::unbounded::<EncodedAudioFrame>();
        let (_video_tx, video_rx) = async_channel::unbounded::<Vec<u8>>();
        let (video_out, _video_out_rx) = async_channel::unbounded::<VideoFrame>();

        let result = client
            .voip()
            .accept(&incoming)
            .encoded_audio(AudioFormat::OPUS_16KHZ_60MS, audio_rx, audio_out)
            .video(video_rx, video_out)
            .start()
            .await;

        assert!(matches!(result, Err(CallError::VideoNotOffered)));
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "invalid video configuration must fail before registration"
        );
    }

    #[tokio::test]
    async fn incoming_group_offer_rejects_conflicting_roster_media() {
        let client = make_client().await;
        let mut incoming = incoming_offer(true);
        let call_id = incoming.action.call_id().to_string();
        let mut group = GroupCallUpdate::builder()
            .call_id(call_id)
            .call_creator(caller())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        group.relay = Some(sample_group_relay(1));
        incoming.group = Some(Box::new(group));
        let mut session = wacore::voip::CallSession::new_incoming(
            incoming.action.call_id(),
            incoming.from.clone(),
            incoming.action.call_creator().clone(),
        );
        session.is_video = true;
        session.group = incoming.group.as_deref().cloned();
        let generation = client
            .call_registry()
            .insert_ringing_group_if_inactive(session)
            .expect("valid group snapshot")
            .expect("ringing group generation");
        incoming.set_ringing_generation(generation);
        let (_source_tx, source_rx) = async_channel::unbounded::<Bytes>();
        let (sink_tx, _sink_rx) = async_channel::unbounded::<EncodedAudioFrame>();

        let result = client
            .voip()
            .accept(&incoming)
            .encoded_audio(AudioFormat::MLOW_16KHZ_60MS, source_rx, sink_tx)
            .start()
            .await;

        assert!(matches!(
            result,
            Err(CallError::Response(message))
                if message == "group offer signaling and roster media modes differ"
        ));
        assert!(
            client
                .call_registry()
                .is_current(incoming.action.call_id(), generation),
            "conflicting media must fail before accepting or replacing the ringing generation"
        );
    }

    #[tokio::test]
    async fn stale_direct_accept_cannot_borrow_or_replace_a_group_offer() {
        let client = make_client().await;
        let incoming = incoming_offer(false);
        let call_id = incoming.action.call_id().to_string();
        let group_creator = Jid::new("15550003333", Server::Lid);
        let mut group_session = wacore::voip::CallSession::new_incoming(
            &call_id,
            group_creator.clone(),
            group_creator.clone(),
        );
        group_session.group = Some(
            GroupCallUpdate::builder()
                .call_id(call_id.clone())
                .call_creator(group_creator)
                .transaction_id(1)
                .media("audio".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(Vec::new())
                .build(),
        );
        let generation = client
            .call_registry()
            .insert_ringing_group_if_inactive(group_session)
            .expect("valid group snapshot")
            .expect("ringing group generation");
        let (_source_tx, source_rx) = async_channel::unbounded::<Bytes>();
        let (sink_tx, _sink_rx) = async_channel::unbounded::<EncodedAudioFrame>();

        let result = client
            .voip()
            .accept(&incoming)
            .encoded_audio(AudioFormat::MLOW_16KHZ_60MS, source_rx, sink_tx)
            .start()
            .await;

        assert!(matches!(result, Err(CallError::CallEndedDuringSetup)));
        assert!(
            client.call_registry().is_current(&call_id, generation),
            "the newer ringing group generation must survive a stale direct accept"
        );
    }

    #[tokio::test]
    async fn stale_group_accept_cannot_borrow_the_replacement_generation() {
        let client = make_client().await;
        let mut incoming = incoming_offer(false);
        let call_id = incoming.action.call_id().to_string();
        let creator = incoming.action.call_creator().clone();
        let mut update = GroupCallUpdate::builder()
            .call_id(call_id.clone())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        update.relay = Some(sample_group_relay(1));
        incoming.group = Some(Box::new(update.clone()));

        let mut stale_session =
            wacore::voip::CallSession::new_incoming(&call_id, creator.clone(), creator.clone());
        stale_session.group = Some(update.clone());
        let stale = client
            .call_registry()
            .insert_ringing_group_if_inactive(stale_session)
            .expect("valid group snapshot")
            .expect("ringing group generation");
        incoming.set_ringing_generation(stale);

        let mut replacement_session =
            wacore::voip::CallSession::new_incoming(&call_id, creator.clone(), creator);
        replacement_session.group = Some(update);
        let replacement = client
            .call_registry()
            .insert_ringing_group(replacement_session);
        let (_source_tx, source_rx) = async_channel::unbounded::<Bytes>();
        let (sink_tx, _sink_rx) = async_channel::unbounded::<EncodedAudioFrame>();

        let result = client
            .voip()
            .accept(&incoming)
            .encoded_audio(AudioFormat::MLOW_16KHZ_60MS, source_rx, sink_tx)
            .start()
            .await;

        assert!(matches!(result, Err(CallError::CallEndedDuringSetup)));
        assert!(
            client.call_registry().is_current(&call_id, replacement),
            "a retained offer event must not claim a newer same-id group generation"
        );
    }

    #[tokio::test]
    async fn active_group_invite_waits_for_its_usable_relay_snapshot() {
        let registry = wacore::voip::CallRegistry::new();
        let call_id = "ACTIVE-GROUP-INVITE";
        let creator = caller();
        let update = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        let mut session =
            wacore::voip::CallSession::new_incoming(call_id, creator.clone(), creator);
        session.group = Some(update.clone());
        let generation = registry
            .insert_ringing_group_if_inactive(session)
            .expect("valid group snapshot")
            .expect("ringing generation");
        let mut wait = Box::pin(wait_for_group_relay(&registry, call_id, generation));

        assert!(
            tokio::time::timeout(Duration::from_millis(20), wait.as_mut())
                .await
                .is_err(),
            "media attachment must not reject while the invitation relay is still racing in"
        );
        let mut admitted = update;
        admitted.transaction_id = 2;
        admitted.relay = Some(sample_group_relay(2));
        assert_eq!(
            registry.apply_group_update_if_current(admitted, generation),
            wacore::voip::GroupStateApply::Applied
        );
        assert!(
            wait.await.expect("relay update").relay.is_some(),
            "the waiter must return the newly committed usable relay"
        );
    }

    #[tokio::test]
    async fn offer_without_media_reports_media_error() {
        let client = make_client().await;
        let incoming = incoming_offer(false);
        let (_source_tx, source_rx) = async_channel::unbounded::<Bytes>();
        let (sink_tx, _sink_rx) = async_channel::unbounded::<EncodedAudioFrame>();

        let result = client
            .voip()
            .accept(&incoming)
            .encoded_audio(AudioFormat::MLOW_16KHZ_60MS, source_rx, sink_tx)
            .start()
            .await;

        assert!(matches!(
            result,
            Err(CallError::Media("offer carried no media block"))
        ));
    }

    #[tokio::test]
    async fn peer_terminate_before_preaccept_reaps_pending_answer() {
        let (client, sent_count) = make_sending_client().await;
        let incoming = incoming_offer(false);
        client
            .call_registry()
            .mark_incoming_ringing(incoming.action.call_id());
        let registration = register_answer(&client, &incoming).await;
        let mut teardown = AnswerTeardown::new(&client, &registration);

        // A terminal stanza may land after registration but before the first answer send.
        terminate_call(&client, incoming.action.call_id());
        let result = send_answer_signaling_with_ids(
            &client,
            &incoming,
            AudioFormat::MLOW_16KHZ_60MS,
            false,
            &registration,
            &mut teardown,
            ("PREACCEPT-TERMINATED-ID", "ACCEPT-TERMINATED-ID"),
        )
        .await;

        assert!(matches!(result, Err(CallError::CallEndedDuringSetup)));
        assert_eq!(
            sent_count.load(Ordering::SeqCst),
            0,
            "neither preaccept nor accept may be sent after the caller terminates"
        );
        assert_eq!(client.call_registry().active_count(), 0);
    }

    #[test]
    fn answer_signaling_builds_selected_audio_nodes() {
        let (preaccept, accept) = build_answer_signaling(
            &incoming_offer(false),
            AudioFormat::MLOW_16KHZ_60MS,
            false,
            false,
            "PREACCEPT-ID",
            "ACCEPT-ID",
        )
        .expect("answer signaling");

        let preaccept_ref = preaccept.as_node_ref();
        assert_eq!(
            preaccept_ref.attrs().optional_string("id").as_deref(),
            Some("PREACCEPT-ID")
        );
        let preaccept_action = &preaccept_ref.children().expect("preaccept action")[0];
        assert_eq!(preaccept_action.tag.as_ref(), "preaccept");
        assert_eq!(
            preaccept_action
                .get_optional_child("audio")
                .and_then(|audio| audio.attrs().optional_string("rate"))
                .as_deref(),
            Some("16000")
        );
        assert_eq!(
            preaccept_action
                .get_optional_child("capability")
                .and_then(|capability| capability.content_bytes()),
            Some(CAPABILITY_PREACCEPT.as_slice())
        );

        let accept_ref = accept.as_node_ref();
        assert_eq!(
            accept_ref.attrs().optional_string("id").as_deref(),
            Some("ACCEPT-ID")
        );
        let accept_action = &accept_ref.children().expect("accept action")[0];
        assert_eq!(accept_action.tag.as_ref(), "accept");
        assert_eq!(
            accept_action
                .get_optional_child("capability")
                .and_then(|capability| capability.content_bytes()),
            Some(CAPABILITY_OFFER.as_slice())
        );
    }

    #[test]
    fn group_answer_signaling_targets_the_call_scope() {
        let mut incoming = incoming_offer(false);
        let call_id = incoming.action.call_id().to_string();
        incoming.group = Some(Box::new(
            GroupCallUpdate::builder()
                .call_id(call_id.clone())
                .call_creator(caller())
                .transaction_id(1)
                .media("audio".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(Vec::new())
                .build(),
        ));
        let (preaccept, accept) = build_answer_signaling(
            &incoming,
            AudioFormat::MLOW_16KHZ_60MS,
            false,
            true,
            "PREACCEPT-GROUP-ID",
            "ACCEPT-GROUP-ID",
        )
        .expect("group answer signaling");
        let expected = Jid::new(call_id, Server::Call);
        for node in [preaccept, accept] {
            assert_eq!(
                node.as_node_ref().attrs().optional_jid("to"),
                Some(expected.clone())
            );
        }
    }

    #[tokio::test]
    async fn answer_signaling_sends_preaccept_before_accept() {
        let client = make_client().await;
        let (transport, entered_rx, release_tx) = gated_send_transport(0, false);
        install_noise_transport(&client, transport).await;
        let incoming = incoming_offer(false);
        let registration = register_answer(&client, &incoming).await;
        let mut teardown = AnswerTeardown::new(&client, &registration);
        let preaccept_waiter = client.wait_for_sent_node(
            crate::client::NodeFilter::tag("call").attr("id", "PREACCEPT-ORDER-ID"),
        );
        let mut accept_waiter = client.wait_for_sent_node(
            crate::client::NodeFilter::tag("call").attr("id", "ACCEPT-ORDER-ID"),
        );

        let send = send_answer_signaling_with_ids(
            &client,
            &incoming,
            AudioFormat::MLOW_16KHZ_60MS,
            false,
            &registration,
            &mut teardown,
            ("PREACCEPT-ORDER-ID", "ACCEPT-ORDER-ID"),
        );
        let observe = async {
            let preaccept = tokio::time::timeout(Duration::from_secs(2), preaccept_waiter)
                .await
                .expect("preaccept must reach send_node")
                .expect("preaccept waiter");
            entered_rx
                .recv()
                .await
                .expect("preaccept transport attempt");
            assert_eq!(
                preaccept
                    .as_node_ref()
                    .children()
                    .expect("preaccept action")[0]
                    .tag
                    .as_ref(),
                "preaccept"
            );
            assert!(
                tokio::time::timeout(Duration::from_millis(20), &mut accept_waiter)
                    .await
                    .is_err(),
                "accept must not be emitted while the preaccept transport write is blocked"
            );

            release_tx.send(()).await.expect("release first send");
            let accept = tokio::time::timeout(Duration::from_secs(2), accept_waiter)
                .await
                .expect("accept must follow preaccept")
                .expect("accept waiter");
            assert_eq!(
                accept.as_node_ref().children().expect("accept action")[0]
                    .tag
                    .as_ref(),
                "accept"
            );
        };

        let (result, ()) = tokio::join!(send, observe);
        result.expect("answer signaling");
        teardown.disarm();
    }

    #[tokio::test]
    async fn preaccept_precedes_slow_answer_preparation() {
        let (client, _sent_count) = make_sending_client().await;
        let incoming = incoming_offer(false);
        let registration = register_answer(&client, &incoming).await;
        let mut teardown = AnswerTeardown::new(&client, &registration);
        let (preaccept, _accept) = build_answer_signaling(
            &incoming,
            AudioFormat::MLOW_16KHZ_60MS,
            false,
            false,
            "PREACCEPT-EARLY-ID",
            "ACCEPT-AFTER-PREPARE-ID",
        )
        .expect("answer signaling");
        let preaccept_waiter = client.wait_for_sent_node(
            crate::client::NodeFilter::tag("call").attr("id", "PREACCEPT-EARLY-ID"),
        );
        let (prepare_tx, prepare_rx) = async_channel::bounded(1);
        let prepared = Arc::new(AtomicBool::new(false));
        let prepare = {
            let prepared = prepared.clone();
            async move {
                prepare_rx.recv().await.expect("release preparation");
                prepared.store(true, Ordering::SeqCst);
                Ok::<_, CallError>(())
            }
        };

        let answer =
            send_preaccept_then_prepare(&client, &registration, &mut teardown, preaccept, prepare);
        let observe = async {
            preaccept_waiter.await.expect("preaccept waiter");
            assert!(
                !prepared.load(Ordering::SeqCst),
                "preaccept must be sent while answer preparation is still pending"
            );
            prepare_tx.send(()).await.expect("finish preparation");
        };

        let (result, ()) = tokio::join!(answer, observe);
        result.expect("preaccept then prepare");
        teardown.disarm();
    }

    #[tokio::test]
    async fn peer_terminate_during_preparation_stops_before_accept() {
        let (client, sent_count) = make_sending_client().await;
        let incoming = incoming_offer(false);
        let registration = register_answer(&client, &incoming).await;
        let mut teardown = AnswerTeardown::new(&client, &registration);
        let (preaccept, accept) = build_answer_signaling(
            &incoming,
            AudioFormat::MLOW_16KHZ_60MS,
            false,
            false,
            "PREACCEPT-PEER-END-ID",
            "ACCEPT-PEER-END-ID",
        )
        .expect("answer signaling");
        let preaccept_waiter = client.wait_for_sent_node(
            crate::client::NodeFilter::tag("call").attr("id", "PREACCEPT-PEER-END-ID"),
        );
        let (prepare_tx, prepare_rx) = async_channel::bounded(1);

        let answer = async {
            send_preaccept_then_prepare(&client, &registration, &mut teardown, preaccept, async {
                prepare_rx.recv().await.expect("release preparation");
                Ok::<_, CallError>(())
            })
            .await?;
            registration.ensure_current()?;
            send_answer_node(&client, &registration, &mut teardown, accept).await
        };
        let end_peer = async {
            preaccept_waiter.await.expect("preaccept waiter");
            terminate_call(&client, incoming.action.call_id());
            prepare_tx.send(()).await.expect("finish preparation");
        };

        let (result, ()) = tokio::join!(answer, end_peer);
        assert!(matches!(result, Err(CallError::CallEndedDuringSetup)));
        assert_eq!(
            sent_count.load(Ordering::SeqCst),
            1,
            "peer termination after preaccept must suppress the final accept"
        );
    }

    #[tokio::test]
    async fn accept_send_failure_terminates_after_preaccept() {
        let client = make_client().await;
        let (transport, entered_rx, release_tx) = gated_send_transport(1, true);
        install_noise_transport(&client, transport).await;
        let incoming = incoming_offer(false);
        let registration = register_answer(&client, &incoming).await;
        let mut teardown = AnswerTeardown::new(&client, &registration);
        let preaccept_waiter = client.wait_for_sent_node(
            crate::client::NodeFilter::tag("call").attr("id", "PREACCEPT-FAIL-ID"),
        );
        let accept_waiter = client.wait_for_sent_node(
            crate::client::NodeFilter::tag("call").attr("id", "ACCEPT-FAIL-ID"),
        );

        let send = send_answer_signaling_with_ids(
            &client,
            &incoming,
            AudioFormat::MLOW_16KHZ_60MS,
            false,
            &registration,
            &mut teardown,
            ("PREACCEPT-FAIL-ID", "ACCEPT-FAIL-ID"),
        );
        let observe = async {
            preaccept_waiter.await.expect("preaccept waiter");
            accept_waiter.await.expect("accept waiter");
            entered_rx.recv().await.expect("accept transport attempt");
            let terminate_waiter =
                client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
            release_tx.send(()).await.expect("release accept send");
            let terminate = tokio::time::timeout(Duration::from_secs(2), terminate_waiter)
                .await
                .expect("terminate must follow the failed accept")
                .expect("terminate waiter");
            assert_eq!(
                terminate
                    .as_node_ref()
                    .children()
                    .expect("terminate action")[0]
                    .tag
                    .as_ref(),
                "terminate"
            );
        };

        let (result, ()) = tokio::join!(send, observe);
        assert!(matches!(result, Err(CallError::Send(_))));
    }

    #[test]
    fn standard_opus_video_answer_keeps_signaling_profile_aligned() {
        let (preaccept, accept) = build_answer_signaling(
            &incoming_offer(true),
            AudioFormat::OPUS_RFC7587_16KHZ_60MS,
            true,
            false,
            "PREACCEPT-VIDEO-ID",
            "ACCEPT-VIDEO-ID",
        )
        .expect("video answer signaling");

        let preaccept_ref = preaccept.as_node_ref();
        let preaccept_action = &preaccept_ref.children().expect("preaccept action")[0];
        assert!(preaccept_action.get_optional_child("video").is_some());
        assert_eq!(
            preaccept_action
                .get_optional_child("capability")
                .and_then(|capability| capability.content_bytes()),
            Some(CAPABILITY_STANDARD_OPUS_OFFER.as_slice())
        );

        let accept_ref = accept.as_node_ref();
        let accept_action = &accept_ref.children().expect("accept action")[0];
        assert!(accept_action.get_optional_child("video").is_some());
        assert!(
            accept_action.get_optional_child("capability").is_none(),
            "captured video accepts omit capability"
        );
        assert_eq!(
            accept_action
                .get_optional_child("voip_settings")
                .and_then(|settings| settings.content_bytes()),
            Some(standard_opus_voip_settings(true))
        );
    }

    // peer_jid() is the <terminate> target: bare/direct device until promotion installs group state,
    // then the call-scoped address used by every subsequent group signaling transition.
    #[tokio::test]
    async fn peer_jid_upgrades_to_the_answering_device() {
        let client = make_client().await;
        let generation = client.call_registry().insert(mk_session());
        let (_ev_tx, ev_rx) = async_channel::unbounded::<CallEvent>();
        let handle = CallHandle {
            call_id: "CID-FACADE".into(),
            generation,
            peer_jid: caller(),
            call_creator: caller(),
            client_registry: client.call_registry(),
            pending_outgoing_calls: client.voip_state().pending_outgoing_calls.clone(),
            client: std::sync::Weak::new(),
            muted: Arc::new(AtomicBool::new(false)),
            video: Arc::new(VideoShared::new()),
            events: ev_rx,
            ended: Arc::new(EndedFlag::default()),
        };
        assert_eq!(handle.peer_jid(), caller(), "bare peer before any accept");
        let device = caller().with_device(2);
        client
            .call_registry()
            .set_answering_device("CID-FACADE", device.clone());
        assert_eq!(
            handle.peer_jid(),
            device,
            "after the accept the terminate target is the answering device"
        );

        let update = GroupCallUpdate::builder()
            .call_id("CID-FACADE".to_string())
            .call_creator(caller())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        assert_eq!(
            client.call_registry().apply_group_update(update),
            wacore::voip::GroupStateApply::Applied
        );
        assert_eq!(
            handle.peer_jid(),
            Jid::new("CID-FACADE", Server::Call),
            "promotion must retarget later signaling to the call scope"
        );
    }

    fn engine() -> CallEngine {
        let cfg = CallConfig::for_incoming(
            "CID-FACADE",
            "111111111111111:0@lid",
            "222222222222222:0@lid",
            (0u8..32).collect(),
            &sample_relay(),
        )
        .expect("config");
        CallEngine::new(cfg, Box::new(RandTxIds)).expect("engine")
    }

    /// In-memory relay factory: returns a transport that records sends and a channel the test feeds
    /// inbound events through. Lets `spawn_call` be exercised without a real DTLS/SCTP dialer.
    struct MockFactory {
        sent: Arc<Mutex<Vec<Bytes>>>,
        relay_rx: Mutex<Option<async_channel::Receiver<RelayTransportEvent>>>,
        connects: Arc<AtomicUsize>,
    }
    struct MockTransport {
        sent: Arc<Mutex<Vec<Bytes>>>,
    }
    #[async_trait]
    impl RelayTransport for MockTransport {
        async fn send(&self, data: Bytes) -> anyhow::Result<()> {
            self.sent.lock().unwrap().push(data);
            Ok(())
        }
        async fn disconnect(&self) {}
    }
    #[async_trait]
    impl RelayTransportFactory for MockFactory {
        async fn connect(
            &self,
        ) -> anyhow::Result<(
            Arc<dyn RelayTransport>,
            async_channel::Receiver<RelayTransportEvent>,
        )> {
            self.connects.fetch_add(1, Ordering::SeqCst);
            let rx = self.relay_rx.lock().unwrap().take().expect("connect once");
            Ok((
                Arc::new(MockTransport {
                    sent: self.sent.clone(),
                }),
                rx,
            ))
        }
    }

    // spawn_call: connects via the injected factory, registers the call, emits the STUN allocate,
    // and tears down (registry empties, handle.wait_ended resolves) when the relay disconnects.
    #[tokio::test]
    async fn spawn_call_registers_drives_and_tears_down() {
        let client = make_client().await;
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let sent = Arc::new(Mutex::new(Vec::new()));
        let connects = Arc::new(AtomicUsize::new(0));
        let factory = MockFactory {
            sent: sent.clone(),
            relay_rx: Mutex::new(Some(relay_rx)),
            connects: connects.clone(),
        };
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        let handle = spawn_call(
            &client,
            mk_session(),
            engine(),
            &factory,
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await
        .expect("spawn_call");

        assert_eq!(connects.load(Ordering::SeqCst), 1, "factory connected once");
        assert_eq!(handle.call_id(), "CID-FACADE");
        assert_eq!(
            client.call_registry().active_count(),
            1,
            "the call is registered while live"
        );

        // The driver started: it emitted the initial STUN allocate over the transport.
        for _ in 0..50 {
            if !sent.lock().unwrap().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            !sent.lock().unwrap().is_empty(),
            "start must emit the STUN allocate"
        );

        // Relay disconnect ends the driver; the call deregisters and wait_ended resolves.
        relay_tx
            .send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), handle.wait_ended())
            .await
            .expect("wait_ended must resolve after the relay disconnects");
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "a locally-ended call deregisters itself"
        );
    }

    // An attached call that ends via the media layer (relay disconnect, no <terminate> stanza) drops
    // its sibling-dismiss tracking automatically: the rung devices live on the registry session, which
    // the media task's completion path removes. No separate per-call map to leak.
    #[tokio::test]
    async fn media_task_end_drops_ring_devices() {
        let client = make_client().await;
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let factory = MockFactory {
            sent: Arc::new(Mutex::new(Vec::new())),
            relay_rx: Mutex::new(Some(relay_rx)),
            connects: Arc::new(AtomicUsize::new(0)),
        };
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        let mut session = wacore::voip::CallSession::new_outgoing("CID-FACADE", caller(), caller());
        session.ring_devices = vec![caller().with_device(1), caller().with_device(2)];

        let handle = spawn_call(
            &client,
            session,
            engine(),
            &factory,
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await
        .expect("spawn_call");
        assert!(
            client
                .call_registry()
                .snapshot("CID-FACADE")
                .is_some_and(|s| !s.ring_devices.is_empty()),
            "the rung devices are tracked while the call is live"
        );

        // Relay disconnect ends the driver; the media task's completion path removes the registry
        // entry, taking the rung device set with it.
        relay_tx
            .send(RelayTransportEvent::Disconnected(
                RelayDisconnectReason::Closed,
            ))
            .await
            .unwrap();
        tokio::time::timeout(Duration::from_secs(2), handle.wait_ended())
            .await
            .expect("wait_ended must resolve after the relay disconnects");

        assert!(
            client
                .call_registry()
                .take_dismiss_targets("CID-FACADE")
                .is_none(),
            "media-task completion must drop the rung device set with the registry entry"
        );
    }

    // hangup() aborts the media task and deregisters the call.
    #[tokio::test]
    async fn hangup_tears_down_the_call() {
        let client = make_client().await;
        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let sent = Arc::new(Mutex::new(Vec::new()));
        let factory = MockFactory {
            sent: sent.clone(),
            relay_rx: Mutex::new(Some(relay_rx)),
            connects: Arc::new(AtomicUsize::new(0)),
        };
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let handle = spawn_call(
            &client,
            mk_session(),
            engine(),
            &factory,
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await
        .expect("spawn_call");
        assert_eq!(client.call_registry().active_count(), 1);
        handle.hangup().await;
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "hangup deregisters the call"
        );
    }

    // A stale handle (superseded by a same-call-id glare/retry) must not tear down the replacement:
    // hangup is generation-guarded.
    #[tokio::test]
    async fn stale_handle_hangup_spares_the_replacement() {
        let client = make_client().await;
        let spawn = |_client: &Client| {
            let (_relay_tx, relay_rx) = async_channel::unbounded();
            let factory = MockFactory {
                sent: Arc::new(Mutex::new(Vec::new())),
                relay_rx: Mutex::new(Some(relay_rx)),
                connects: Arc::new(AtomicUsize::new(0)),
            };
            let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
            let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
            (factory, Arc::new(mic_rx), Arc::new(spk_tx))
        };

        let (f1, mic1, spk1) = spawn(&client);
        let stale = spawn_call(
            &client,
            mk_session(),
            engine(),
            &f1,
            pcm_audio(mic1, spk1),
            None,
        )
        .await
        .expect("first spawn_call");
        // A same-call-id re-offer replaces the first (new generation, aborts its task).
        let (f2, mic2, spk2) = spawn(&client);
        let live = spawn_call(
            &client,
            mk_session(),
            engine(),
            &f2,
            pcm_audio(mic2, spk2),
            None,
        )
        .await
        .expect("replacement spawn_call");
        assert_eq!(
            client.call_registry().active_count(),
            1,
            "same call-id replaced, not duplicated"
        );

        // The stale handle hangs up: it must NOT abort the replacement.
        stale.hangup().await;
        assert_eq!(
            client.call_registry().active_count(),
            1,
            "stale hangup must leave the live replacement registered"
        );
        // The live handle still tears it down.
        live.hangup().await;
        assert_eq!(client.call_registry().active_count(), 0);
    }

    #[tokio::test]
    async fn stale_peer_terminate_spares_the_replacement_generation() {
        let client = make_client().await;
        let stale = client.call_registry().insert(mk_session());
        let current = client.call_registry().insert(mk_session());

        terminate_call_if_current(&client, "CID-FACADE", stale);
        assert_eq!(
            client.call_registry().generation_of("CID-FACADE"),
            Some(current),
            "a terminal stanza authorized for an old generation must spare its replacement"
        );

        terminate_call_if_current(&client, "CID-FACADE", current);
    }

    // A stale handle's wait_ended() must resolve (not hang) once a same-call-id replacement aborted
    // its media task: the ended flag is sticky, so the already-fired notification is not missed.
    #[tokio::test]
    async fn stale_handle_wait_ended_resolves_via_sticky_flag() {
        let client = make_client().await;
        let spawn = |_client: &Client| {
            let (_relay_tx, relay_rx) = async_channel::unbounded();
            let factory = MockFactory {
                sent: Arc::new(Mutex::new(Vec::new())),
                relay_rx: Mutex::new(Some(relay_rx)),
                connects: Arc::new(AtomicUsize::new(0)),
            };
            let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
            let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
            (factory, Arc::new(mic_rx), Arc::new(spk_tx))
        };

        let (f1, mic1, spk1) = spawn(&client);
        let stale = spawn_call(
            &client,
            mk_session(),
            engine(),
            &f1,
            pcm_audio(mic1, spk1),
            None,
        )
        .await
        .expect("first spawn_call");
        let (f2, mic2, spk2) = spawn(&client);
        let _live = spawn_call(
            &client,
            mk_session(),
            engine(),
            &f2,
            pcm_audio(mic2, spk2),
            None,
        )
        .await
        .expect("replacement spawn_call");

        // The replacement aborted the stale task; its wait_ended must still resolve.
        tokio::time::timeout(Duration::from_secs(2), stale.wait_ended())
            .await
            .expect("stale handle wait_ended must resolve, not hang");
    }

    // A wait_ended() already parked on the listener must wake when hangup() aborts the media task:
    // the abort drops the future, the drop guard notifies `ended`. Without the guard the waiter would
    // sleep forever (the clean run_call_tokio continuation never runs under abort).
    #[tokio::test]
    async fn wait_ended_wakes_when_hangup_aborts_the_task() {
        let client = make_client().await;
        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let sent = Arc::new(Mutex::new(Vec::new()));
        let factory = MockFactory {
            sent: sent.clone(),
            relay_rx: Mutex::new(Some(relay_rx)),
            connects: Arc::new(AtomicUsize::new(0)),
        };
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let handle = Arc::new(
            spawn_call(
                &client,
                mk_session(),
                engine(),
                &factory,
                pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
                None,
            )
            .await
            .expect("spawn_call"),
        );

        let waiter = {
            let h = handle.clone();
            tokio::spawn(async move { h.wait_ended().await })
        };
        // Let the waiter register its listener and pass the still-present phase check, so it is truly
        // parked on `listener.await` (the path the guard must cover), not the early return.
        tokio::time::sleep(Duration::from_millis(20)).await;
        handle.hangup().await;
        tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("wait_ended must resolve after hangup aborts the task")
            .expect("waiter task");
    }

    // The mute gate: a muted 960-frame is zeroed in place (the engine's DTX fast-path), an unmuted
    // frame passes through untouched, and a wrong-length frame is never zeroed (left for the engine
    // to drop).
    #[tokio::test]
    async fn mute_feed_zeroes_muted_frames_only() {
        let (src_tx, src_rx) = async_channel::unbounded::<Vec<i16>>();
        let (out_tx, out_rx) = async_channel::unbounded::<Vec<i16>>();
        let muted = Arc::new(AtomicBool::new(false));
        let feed = MuteFeed {
            src: src_rx,
            out: out_tx,
            muted: muted.clone(),
        };
        let task = tokio::spawn(feed.run());

        // Unmuted: passes through.
        src_tx.send(vec![5i16; WA_FRAME_SAMPLES]).await.unwrap();
        assert!(
            out_rx.recv().await.unwrap().iter().all(|&s| s == 5),
            "unmuted frame passes through untouched"
        );

        // Muted: a 960-frame is zeroed.
        muted.store(true, Ordering::Relaxed);
        src_tx.send(vec![5i16; WA_FRAME_SAMPLES]).await.unwrap();
        assert!(
            out_rx.recv().await.unwrap().iter().all(|&s| s == 0),
            "a muted 960-frame must be zeroed for the engine's DTX fast-path"
        );

        // Muted but wrong length: not zeroed (the engine drops it; we don't fake a 960 frame).
        src_tx.send(vec![5i16; 480]).await.unwrap();
        let short = out_rx.recv().await.unwrap();
        assert_eq!(short.len(), 480);
        assert!(
            short.iter().all(|&s| s == 5),
            "a wrong-length frame is forwarded unchanged"
        );

        drop(src_tx);
        task.await.unwrap();
    }

    // call-id is "00" + 15 random bytes as lowercase hex: 32 hex chars, always "00"-prefixed.
    #[test]
    fn gen_call_id_shape() {
        for _ in 0..32 {
            let id = gen_call_id();
            assert_eq!(id.len(), 32, "call-id must be 32 hex chars");
            assert!(id.starts_with("00"), "call-id must start with 00");
            assert!(
                id.bytes()
                    .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()),
                "call-id must be lowercase hex"
            );
        }
    }

    fn peer_lid() -> Jid {
        // Fictitious peer LID device (no real PII).
        Jid::new("333333333333333", Server::Lid).with_device(0)
    }

    /// A test client with a real NoiseSocket over a counting transport, so the offer send path
    /// (`send_node`) is exercised, plus the own LID set so `place_call` can derive call_creator.
    async fn make_sending_client() -> (Arc<Client>, Arc<AtomicUsize>) {
        make_sending_client_with_failure_after(None).await
    }

    async fn make_sending_client_with_failure_after(
        failure_after: Option<usize>,
    ) -> (Arc<Client>, Arc<AtomicUsize>) {
        let backend = create_test_backend().await;
        make_sending_client_with_backend(backend, failure_after).await
    }

    async fn make_sending_client_with_backend(
        backend: Arc<dyn Backend>,
        failure_after: Option<usize>,
    ) -> (Arc<Client>, Arc<AtomicUsize>) {
        let pm = PersistenceManager::new(backend).await.expect("pm");
        // Set our own LID so lid() resolves (the send-side participant id).
        pm.process_command(crate::store::commands::DeviceCommand::SetLid(Some(
            Jid::new("111111111111111", Server::Lid),
        )))
        .await;
        // Set the ADV account so a pkmsg offer attaches a <device-identity> (as the send path does).
        pm.process_command(crate::store::commands::DeviceCommand::SetAccount(Some(
            wa::ADVSignedDeviceIdentity {
                details: Some(vec![0u8; 32]),
                account_signature_key: Some(vec![0u8; 32]),
                account_signature: Some(vec![0u8; 64]),
                device_signature: Some(vec![0u8; 64]),
            },
        )))
        .await;
        let transport = Arc::new(crate::transport::mock::MockTransportFactory::new());
        let (client, _rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(pm),
            transport,
            Arc::new(MockHttpClient),
            None,
        )
        .await;

        let count = Arc::new(AtomicUsize::new(0));
        struct CountingTransport {
            count: Arc<AtomicUsize>,
            failure_after: Option<usize>,
        }
        #[async_trait]
        impl crate::transport::Transport for CountingTransport {
            async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
                let attempt = self.count.fetch_add(1, Ordering::SeqCst);
                if self.failure_after.is_some_and(|limit| attempt >= limit) {
                    return Err(anyhow::anyhow!("injected transport send failure"));
                }
                Ok(())
            }
            async fn disconnect(&self) {}
        }
        let socket_transport: Arc<dyn crate::transport::Transport> = Arc::new(CountingTransport {
            count: count.clone(),
            failure_after,
        });
        install_noise_transport(&client, socket_transport).await;
        client.set_connected_for_test(true);
        client.enter_live_mode_for_tests();
        (client, count)
    }

    // The offer-build path for an outgoing call: a fresh callKey is generated, encrypted per device
    // (a fresh session yields pkmsg), and the sent <offer> carries the load-bearing child order. The
    // call is registered Outgoing and the relay-attach material is parked, dormant until the relay.
    #[tokio::test]
    async fn place_call_builds_and_sends_offer() {
        let (client, sent_count) = make_sending_client().await;
        let peer_user = Jid::new("333333333333333", Server::Lid);
        let device = peer_lid();
        seed_peer_session(&client, &device).await;

        let own_lid = client.lid().expect("own lid");
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        // Capture the sent <offer> node for child-order assertions.
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));

        let handle = place_call(
            &client,
            "00abcdef0123456789abcdef01234567".into(),
            &peer_user,
            &own_lid,
            &own_lid,
            std::slice::from_ref(&device),
            std::slice::from_ref(&device),
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await
        .expect("place_call");

        assert_eq!(handle.call_id(), "00abcdef0123456789abcdef01234567");
        assert!(
            sent_count.load(Ordering::SeqCst) >= 1,
            "the offer must be sent"
        );
        assert_eq!(
            client.call_registry().active_count(),
            1,
            "the outgoing call is registered"
        );
        assert!(
            client
                .voip_state()
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .contains_key("00abcdef0123456789abcdef01234567"),
            "the relay-attach material must be parked pending the relay"
        );

        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("offer must be sent")
            .expect("waiter");
        let r = node.as_node_ref();
        assert_eq!(r.tag.as_ref(), "call");
        // The <call> wrapper must carry a stanza id so the server can ack-correlate the offer; the
        // initiator's relay rides back on that ack.
        assert!(
            r.attrs()
                .optional_string("id")
                .is_some_and(|id| !id.is_empty()),
            "the offer <call> must carry a stanza id for ack correlation"
        );
        let offer = &r.children().unwrap()[0];
        assert_eq!(offer.tag.as_ref(), "offer");
        assert_eq!(
            offer.attrs().optional_string("call-id").as_deref(),
            Some("00abcdef0123456789abcdef01234567")
        );
        // call-creator is our own LID (for a LID peer), the send-side SRTP participant id.
        assert_eq!(
            offer.attrs().optional_string("call-creator").as_deref(),
            Some(own_lid.to_string().as_str())
        );
        let tags: Vec<String> = offer
            .children()
            .unwrap()
            .iter()
            .map(|c| c.tag.as_ref().to_string())
            .collect();
        // Single device → bare <enc>; a fresh session is pkmsg, so a <device-identity> follows.
        assert_eq!(
            tags,
            [
                "audio",
                "net",
                "capability",
                "enc",
                "encopt",
                "device-identity"
            ]
        );
        let enc = offer.get_optional_child("enc").unwrap();
        assert_eq!(
            offer
                .get_optional_child("audio")
                .and_then(|audio| audio.attrs().optional_string("rate"))
                .as_deref(),
            Some("16000")
        );
        assert_eq!(
            enc.attrs().optional_string("type").as_deref(),
            Some("pkmsg")
        );
        assert!(
            !enc.content_bytes().unwrap_or_default().is_empty(),
            "the per-device <enc> must carry the encrypted callKey"
        );
    }

    #[tokio::test]
    async fn warm_call_key_fanout_reuses_durable_session_leases() {
        use wacore::store::in_memory::InMemoryBackend;

        let backend = Arc::new(InMemoryBackend::new());
        let (client, _) =
            make_sending_client_with_backend(backend.clone() as Arc<dyn Backend>, None).await;
        let peer = Jid::new("333333333333333", Server::Lid);
        let devices = [peer.clone().with_device(0), peer.clone().with_device(1)];

        for device in &devices {
            seed_peer_session(&client, device).await;
            client
                .signal()
                .encrypt_message(device, b"warm lease")
                .await
                .expect("lease warmup");
        }
        let writes_before = backend.session_batch_write_count();

        client
            .signal_flush_test_block
            .store(true, Ordering::Release);
        let own_lid = client.lid().expect("own lid");
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let _handle = place_call(
            &client,
            "001234567890abcdef1234567890abcd".into(),
            &peer,
            &own_lid,
            &own_lid,
            &devices,
            &devices,
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await
        .expect("warm multi-device call");

        assert_eq!(
            backend.session_batch_write_count(),
            writes_before,
            "a warm call-key fanout must not flush durable leases synchronously"
        );
        client
            .signal_flush_test_block
            .store(false, Ordering::Release);
    }

    #[tokio::test]
    async fn empty_group_rekey_fanout_still_commits_a_local_epoch() {
        let (client, sent) = make_sending_client().await;
        let update = rekey_update(&client, &[]);

        let epoch_len = fanout_group_epoch(&client, &update)
            .await
            .expect("empty fanout")
            .commit(|epoch| Ok(epoch.len()))
            .expect("local epoch commit");

        assert_eq!(epoch_len, 32);
        assert_eq!(
            sent.load(Ordering::SeqCst),
            0,
            "an empty recipient set must initialize only the local epoch"
        );
    }

    #[tokio::test]
    async fn group_rekey_fanout_excludes_the_local_pn_device_alias() {
        let (client, sent) = make_sending_client().await;
        let own_pn = Jid::new("12025550111", Server::Pn);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetId(Some(
                own_pn.clone(),
            )))
            .await;
        let mut update = rekey_update(&client, &[]);
        update.participants[0].devices[0].jid = own_pn.with_device(0);

        let epoch_len = fanout_group_epoch(&client, &update)
            .await
            .expect("local PN alias must not require a Signal session")
            .commit(|epoch| Ok(epoch.len()))
            .expect("local epoch commit");

        assert_eq!(epoch_len, 32);
        assert_eq!(
            sent.load(Ordering::SeqCst),
            0,
            "the local PN device alias must be excluded from remote epoch recipients"
        );
    }

    #[tokio::test]
    async fn group_rekey_fanout_keeps_a_sibling_device_of_the_local_account() {
        let (client, sent) = make_sending_client().await;
        let own_pn = Jid::new("12025550111", Server::Pn);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetId(Some(
                own_pn.clone(),
            )))
            .await;
        let local_device = own_pn.clone().with_device(0);
        let sibling = own_pn.with_device(2);
        seed_peer_session(&client, &sibling).await;
        let mut update = rekey_update(&client, &[]);
        update.participants[0].pn = Some(local_device.to_non_ad());
        update.participants[0].devices[0].jid = local_device;
        let mut sibling_device = GroupCallDevice::new(sibling.clone());
        sibling_device.pid = Some(2);
        update.participants[0].devices.push(sibling_device);

        fanout_group_epoch(&client, &update)
            .await
            .expect("the local account's sibling remains a remote recipient")
            .commit(|_| Ok(()))
            .expect("local epoch commit");

        assert_eq!(
            sent.load(Ordering::SeqCst),
            1,
            "only the exact local endpoint is excluded from epoch fanout"
        );
    }

    #[tokio::test]
    async fn prepublication_group_rekey_failure_terminates_committed_generation() {
        let backend = Arc::new(wacore::store::in_memory::InMemoryBackend::new());
        let (client, _sent) = make_sending_client_with_backend(backend.clone(), None).await;
        let recipient = peer_lid();
        seed_peer_session(&client, &recipient).await;
        let update = rekey_update(&client, &[recipient]);
        register_group_update(&client, &update);
        backend.set_fail_session_writes(true);

        assert!(
            fanout_group_epoch(&client, &update).await.is_err(),
            "the pre-wire durability failure must abort epoch publication"
        );
        assert!(
            client.call_registry().snapshot(&update.call_id).is_none(),
            "a committed rekey request that cannot be retried must terminate its generation"
        );
    }

    #[tokio::test]
    async fn missing_group_identity_is_rejected_before_advancing_fresh_sessions() {
        let (client, _sent) = make_sending_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetAccount(None))
            .await;
        let recipient = peer_lid();
        seed_peer_session(&client, &recipient).await;
        assert!(
            client
                .would_emit_pkmsg(&recipient)
                .await
                .expect("fresh session inspection"),
            "the seeded recipient must require a pre-key message"
        );
        let update = rekey_update(&client, std::slice::from_ref(&recipient));
        register_group_update(&client, &update);

        assert!(matches!(
            fanout_group_epoch(&client, &update).await,
            Err(CallError::MissingDeviceIdentity)
        ));
        assert!(
            client
                .would_emit_pkmsg(&recipient)
                .await
                .expect("session inspection after rejected fanout"),
            "identity preflight must reject before advancing the fresh Signal session"
        );
    }

    #[tokio::test]
    async fn partial_group_rekey_encryption_publishes_every_successful_ciphertext() {
        let (client, sent) = make_sending_client().await;
        let recipients = [
            peer_lid(),
            Jid::new("444444444444444", Server::Lid).with_device(0),
        ];
        let update = rekey_update(&client, &recipients);
        let generation = register_group_update(&client, &update);
        let encrypted = wacore::send::EncryptForDevicesRaw {
            devices: vec![wacore::send::EncryptedDevice {
                device_jid: recipients[0].clone(),
                enc_type: "msg",
                is_prekey: false,
                ciphertext: vec![7; 32],
            }],
            includes_prekey_message: false,
            had_unregistered_device: false,
            rejected_devices: Vec::new(),
            // One of the two recipients has no ciphertext, which is the drop
            // this fixture is about.
            unkeyed_at_encrypt: 1,
        };

        assert!(matches!(
            publish_group_epoch_ciphertexts(
                &client,
                &update,
                Some(generation),
                &recipients,
                &encrypted,
                None,
            )
            .await,
            Err(CallError::Media(
                "group epoch encryption did not cover every recipient"
            ))
        ));
        assert_eq!(
            sent.load(Ordering::SeqCst),
            1,
            "the successfully encrypted recipient must be published before reporting partial coverage"
        );
        client
            .call_registry()
            .remove_if_current(&update.call_id, generation);
    }

    #[tokio::test]
    async fn partial_group_rekey_send_terminates_instead_of_splitting_the_epoch() {
        let (client, sent) = make_sending_client_with_failure_after(Some(1)).await;
        let recipients = [
            peer_lid(),
            Jid::new("444444444444444", Server::Lid).with_device(0),
        ];
        for recipient in &recipients {
            seed_peer_session(&client, recipient).await;
        }
        let update = rekey_update(&client, &recipients);
        register_group_update(&client, &update);

        assert!(fanout_group_epoch(&client, &update).await.is_err());
        assert_eq!(
            sent.load(Ordering::SeqCst),
            2,
            "the second direct rekey send must exercise the partial-fanout path"
        );
        assert!(
            client.call_registry().snapshot(&update.call_id).is_none(),
            "a partial fanout must tear down the local generation"
        );
    }

    #[tokio::test]
    async fn stale_group_rekey_generation_sends_nothing_and_spares_replacement() {
        let (client, sent) = make_sending_client().await;
        let recipient = peer_lid();
        seed_peer_session(&client, &recipient).await;
        let update = rekey_update(&client, &[recipient]);
        let stale_generation = register_group_update(&client, &update);
        let mut replacement = wacore::voip::CallSession::new_incoming(
            &update.call_id,
            update.call_creator.clone(),
            update.call_creator.clone(),
        );
        replacement.group = Some(update.clone());
        let replacement_generation = client.call_registry().insert(replacement);

        assert!(matches!(
            fanout_group_epoch_for_generation(&client, &update, Some(stale_generation)).await,
            Err(CallError::CallEndedDuringSetup)
        ));
        assert_eq!(
            sent.load(Ordering::SeqCst),
            0,
            "a superseded generation must publish no stale rekey stanza"
        );
        assert_eq!(
            client.call_registry().generation_of(&update.call_id),
            Some(replacement_generation),
            "stale fanout validation must spare the replacement generation"
        );
    }

    #[tokio::test]
    async fn cancelled_group_rekey_fanout_terminates_the_local_generation() {
        let (client, _) = make_sending_client().await;
        let (transport, entered, _release) = gated_send_transport(1, false);
        install_noise_transport(&client, transport).await;
        let recipients = [
            peer_lid(),
            Jid::new("444444444444444", Server::Lid).with_device(0),
        ];
        for recipient in &recipients {
            seed_peer_session(&client, recipient).await;
        }
        let update = rekey_update(&client, &recipients);
        register_group_update(&client, &update);

        let task = tokio::spawn({
            let client = client.clone();
            let update = update.clone();
            async move { fanout_group_epoch(&client, &update).await }
        });
        tokio::time::timeout(Duration::from_secs(2), entered.recv())
            .await
            .expect("second rekey send must block")
            .expect("send gate");
        task.abort();
        assert!(matches!(task.await, Err(error) if error.is_cancelled()));
        assert!(
            client.call_registry().snapshot(&update.call_id).is_none(),
            "cancelling after one direct send must tear down the local generation"
        );
    }

    #[tokio::test]
    async fn group_rekey_teardown_does_not_hold_the_answer_lane_during_fanout() {
        let (client, _) = make_sending_client().await;
        let update = rekey_update(&client, &[]);
        let generation = register_group_update(&client, &update);
        let mut teardown = GroupRekeyTeardown::new(&client, &update, Some(generation), true);

        let transition = tokio::time::timeout(
            Duration::from_millis(50),
            client.lock_answer_transition(&update.call_id),
        )
        .await
        .expect("fanout preparation must leave the answer lane available");
        drop(transition);
        teardown.disarm();
        assert!(
            client
                .call_registry()
                .remove_if_current(&update.call_id, generation)
        );
    }

    #[tokio::test]
    async fn group_rekey_teardown_serializes_replacement_until_terminate_is_sent() {
        let (client, _) = make_sending_client().await;
        let (transport, entered, release) = gated_send_transport(0, false);
        install_noise_transport(&client, transport).await;
        let update = rekey_update(&client, &[]);
        let stale_generation = register_group_update(&client, &update);
        drop(GroupRekeyTeardown::new(
            &client,
            &update,
            Some(stale_generation),
            true,
        ));

        tokio::time::timeout(Duration::from_secs(2), entered.recv())
            .await
            .expect("terminate transport attempt")
            .expect("send gate");
        assert_eq!(
            client.call_registry().generation_of(&update.call_id),
            None,
            "the failed generation is claimed before its terminal send"
        );

        let mut replacement_session = wacore::voip::CallSession::new_incoming(
            &update.call_id,
            update.call_creator.clone(),
            update.call_creator.clone(),
        );
        replacement_session.group = Some(update.clone());
        let mut replacement = Box::pin(RegisteredCall::new(&client, replacement_session));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), replacement.as_mut())
                .await
                .is_err(),
            "a same-call-id replacement must wait while rekey termination is in flight"
        );

        release.send(()).await.expect("release terminate send");
        let replacement = replacement.await;
        assert_ne!(replacement.generation, stale_generation);
        assert_eq!(
            client.call_registry().generation_of(&update.call_id),
            Some(replacement.generation)
        );
    }

    #[tokio::test]
    async fn cancelled_stale_group_rekey_fanout_spares_a_replacement_generation() {
        let (client, _) = make_sending_client().await;
        let (transport, entered, _release) = gated_send_transport(1, false);
        install_noise_transport(&client, transport).await;
        let recipients = [
            peer_lid(),
            Jid::new("444444444444444", Server::Lid).with_device(0),
        ];
        for recipient in &recipients {
            seed_peer_session(&client, recipient).await;
        }
        let update = rekey_update(&client, &recipients);
        register_group_update(&client, &update);

        let task = tokio::spawn({
            let client = client.clone();
            let update = update.clone();
            async move { fanout_group_epoch(&client, &update).await }
        });
        tokio::time::timeout(Duration::from_secs(2), entered.recv())
            .await
            .expect("second rekey send must block")
            .expect("send gate");
        let replacement_generation = {
            let mut replacement = wacore::voip::CallSession::new_incoming(
                &update.call_id,
                update.call_creator.clone(),
                update.call_creator.clone(),
            );
            replacement.group = Some(update.clone());
            client.call_registry().insert(replacement)
        };

        task.abort();
        assert!(matches!(task.await, Err(error) if error.is_cancelled()));
        assert_eq!(
            client.call_registry().generation_of(&update.call_id),
            Some(replacement_generation),
            "a stale rekey guard must not reap a same-call-id replacement"
        );
    }

    #[tokio::test]
    async fn encoded_opus_offer_advertises_only_its_selected_profile() {
        let (client, _sent_count) = make_sending_client().await;
        let peer_user = Jid::new("333333333333333", Server::Lid);
        let device = peer_lid();
        seed_peer_session(&client, &device).await;
        let own_lid = client.lid().expect("own lid");
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));

        let handle = place_call(
            &client,
            "00abcdef0123456789abcdef00c0dec0".into(),
            &peer_user,
            &own_lid,
            &own_lid,
            std::slice::from_ref(&device),
            std::slice::from_ref(&device),
            encoded_audio(AudioFormat::OPUS_16KHZ_60MS),
            None,
        )
        .await
        .expect("place encoded call");

        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("offer must be sent")
            .expect("waiter");
        let node_ref = node.as_node_ref();
        let offer = &node_ref.children().unwrap()[0];
        let audio = offer
            .children()
            .unwrap()
            .iter()
            .filter(|child| child.tag.as_ref() == "audio")
            .collect::<Vec<_>>();
        assert_eq!(audio.len(), 1);
        assert_eq!(
            audio[0].attrs().optional_string("rate").as_deref(),
            Some("16000")
        );
        assert_eq!(
            client
                .call_registry()
                .snapshot(handle.call_id())
                .and_then(|session| session.audio_format),
            Some(AudioFormat::OPUS_16KHZ_60MS)
        );
    }

    // A stored token must ride on the offer as the leading `<privacy>` child, or a
    // privacy-restricted callee rejects the call (the no-token case is covered above).
    #[tokio::test]
    async fn place_call_attaches_stored_tctoken_as_privacy_node() {
        use wacore::store::traits::TcTokenEntry;

        let (client, _sent_count) = make_sending_client().await;
        let peer_user = Jid::new("333333333333333", Server::Lid);
        let device = peer_lid();
        seed_peer_session(&client, &device).await;

        // Keyed by the callee's account LID user (resolve_tc_token_key for a LID peer).
        client
            .persistence_manager
            .backend()
            .put_tc_token(
                "333333333333333",
                &TcTokenEntry {
                    token: vec![0xAB, 0xCD, 0xEF],
                    token_timestamp: wacore::time::now_secs(),
                    sender_timestamp: None,
                },
            )
            .await
            .unwrap();

        let own_lid = client.lid().expect("own lid");
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));

        place_call(
            &client,
            "00abcdef0123456789abcdef01234567".into(),
            &peer_user,
            &own_lid,
            &own_lid,
            std::slice::from_ref(&device),
            std::slice::from_ref(&device),
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await
        .expect("place_call");

        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("offer must be sent")
            .expect("waiter");
        let r = node.as_node_ref();
        let offer = &r.children().unwrap()[0];
        let children = offer.children().unwrap();
        // `<privacy>` is the load-bearing first child of the offer.
        assert_eq!(
            children[0].tag.as_ref(),
            "privacy",
            "the stored tctoken must ride as the leading <privacy> child"
        );
        assert_eq!(
            children[0].content_bytes(),
            Some([0xAB, 0xCD, 0xEF].as_slice())
        );
    }

    // Finding S: a per-device encrypt failure must SKIP that device, not abort the whole offer (which
    // would strand the already-encrypted devices with an advanced chain for a ciphertext they never
    // receive). Two seeded devices and one with no session: the offer still goes out, addressed to the
    // two survivors only (the multi-device <destination><to> shape, since >1 device encrypted).
    #[tokio::test]
    async fn place_call_skips_undecryptable_device_and_offers_the_rest() {
        let (client, sent_count) = make_sending_client().await;
        let peer_user = Jid::new("333333333333333", Server::Lid);
        // Seed sessions for devices 0 and 1; device 2 has no session, so message_encrypt errors for it.
        let good0 = peer_lid();
        let good1 = Jid::new("333333333333333", Server::Lid).with_device(1);
        let bad = Jid::new("333333333333333", Server::Lid).with_device(2);
        seed_peer_session(&client, &good0).await;
        seed_peer_session(&client, &good1).await;

        let own_lid = client.lid().expect("own lid");
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));

        let handle = place_call(
            &client,
            "00abcdef0123456789abcdef0123c0de".into(),
            &peer_user,
            &own_lid,
            &own_lid,
            &[good0.clone(), good1.clone(), bad.clone()],
            &[good0.clone(), good1.clone(), bad.clone()],
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await
        .expect("place_call must succeed with surviving devices");

        assert_eq!(handle.call_id(), "00abcdef0123456789abcdef0123c0de");
        assert!(
            sent_count.load(Ordering::SeqCst) >= 1,
            "the offer for the surviving devices must be sent"
        );

        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("offer must be sent")
            .expect("waiter");
        let r = node.as_node_ref();
        let offer = &r.children().unwrap()[0];
        // >1 device encrypted → a <destination> wrapping a <to> per survivor; the failed device is absent.
        let destination = offer
            .get_optional_child("destination")
            .expect("multi-device offer carries a <destination>");
        let addressed: Vec<String> = destination
            .children()
            .unwrap()
            .iter()
            .filter(|c| c.tag.as_ref() == "to")
            .filter_map(|c| c.attrs().optional_string("jid").map(|j| j.into_owned()))
            .collect();
        assert_eq!(
            addressed,
            [good0.to_string(), good1.to_string()],
            "only the devices with a session are addressed; the undecryptable one is skipped"
        );

        // Regression guard for the device gap: the dismiss target set (ring_devices) is the FULL rung
        // set, INCLUDING the undecryptable `bad` device the server still rings -- so a sibling we can't
        // encrypt for is still dismissed on accept (else it rings on and times the call out at ~45s).
        let session = client
            .call_registry()
            .snapshot(handle.call_id())
            .expect("the outgoing call is registered");
        let mut ring: Vec<String> = session.ring_devices.iter().map(|d| d.to_string()).collect();
        ring.sort();
        let mut expected = [good0.to_string(), good1.to_string(), bad.to_string()];
        expected.sort();
        assert_eq!(
            ring, expected,
            "ring_devices must be the full rung set (incl. the undecryptable device), not just the encrypted offer recipients"
        );
    }

    // Finding S: if EVERY device fails to encrypt (none has a session), there is no one to address the
    // offer to, so place_call returns NoDevices and registers/sends nothing.
    #[tokio::test]
    async fn place_call_all_devices_fail_returns_no_devices() {
        let (client, sent_count) = make_sending_client().await;
        let peer_user = Jid::new("333333333333333", Server::Lid);
        // No session seeded for the device, so its encrypt errors and it is skipped.
        let device = peer_lid();
        let own_lid = client.lid().expect("own lid");
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        let res = place_call(
            &client,
            "00abcdef0123456789abcdef0123ba1d".into(),
            &peer_user,
            &own_lid,
            &own_lid,
            std::slice::from_ref(&device),
            std::slice::from_ref(&device),
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await;
        assert!(
            matches!(res, Err(CallError::NoDevices)),
            "every device failing to encrypt must surface as NoDevices"
        );
        assert_eq!(
            sent_count.load(Ordering::SeqCst),
            0,
            "no offer is sent when no device could be encrypted for"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "an unsendable offer must not register the call"
        );
        assert!(
            client
                .voip_state()
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .is_empty(),
            "an unsendable offer must not park a pending entry"
        );
    }

    /// Place a dormant outgoing call (offer sent, relay not yet arrived) and return its handle. Shares
    /// the place_call machinery the offer-send test uses; the call lands in pending_outgoing_calls.
    async fn place_dormant_outgoing(client: &Arc<Client>) -> (CallHandle, String) {
        place_dormant_outgoing_with_video(client, None).await
    }

    async fn place_dormant_outgoing_with_video(
        client: &Arc<Client>,
        video: Option<VideoEndpoints>,
    ) -> (CallHandle, String) {
        let peer_user = Jid::new("333333333333333", Server::Lid);
        let device = peer_lid();
        seed_peer_session(client, &device).await;
        let own_lid = client.lid().expect("own lid");
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let call_id = "00abcdef0123456789abcdef0123beef".to_string();
        let handle = place_call(
            client,
            call_id.clone(),
            &peer_user,
            &own_lid,
            &own_lid,
            std::slice::from_ref(&device),
            std::slice::from_ref(&device),
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            video,
        )
        .await
        .expect("place_call");
        (handle, call_id)
    }

    // A dormant outgoing call (relay never arrived, no engine task) that is hung up must: drop its
    // pending_outgoing_calls entry (no leak / no resurrection) AND resolve wait_ended() itself, since
    // no media task exists to fire the drop-guard.
    #[tokio::test]
    async fn dormant_outgoing_hangup_drops_pending_and_resolves_wait_ended() {
        let (client, _count) = make_sending_client().await;
        let (handle, call_id) = place_dormant_outgoing(&client).await;
        assert!(
            client
                .voip_state()
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .contains_key(&call_id),
            "the dormant call is parked pending the relay"
        );

        handle.hangup().await;

        assert!(
            client
                .voip_state()
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .is_empty(),
            "hangup must drop the dormant pending entry"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "hangup must deregister the dormant call"
        );
        tokio::time::timeout(Duration::from_secs(2), handle.wait_ended())
            .await
            .expect("dormant hangup must resolve wait_ended (no engine task to notify it)");
    }

    // A disconnect must tear down dormant outgoing calls: drain pending_outgoing_calls and notify each
    // `ended`, so a parked wait_ended() resolves instead of hanging across the reconnect.
    #[tokio::test]
    async fn disconnect_drains_dormant_outgoing_and_resolves_wait_ended() {
        let (client, _count) = make_sending_client().await;
        let (handle, _call_id) = place_dormant_outgoing(&client).await;

        drain_pending_outgoing_on_disconnect(&client);

        assert!(
            client
                .voip_state()
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .is_empty(),
            "disconnect must drain dormant outgoing calls"
        );
        tokio::time::timeout(Duration::from_secs(2), handle.wait_ended())
            .await
            .expect("disconnect must resolve a dormant call's wait_ended");
    }

    /// A factory whose connect() parks until `gate` is released, so a test can run cleanup mid-connect
    /// and exercise the register-before-connect ordering: the entry exists before connect, cleanup
    /// removes it during the gap, and set_media_task aborts the just-spawned task.
    struct GatedFactory {
        gate: async_channel::Receiver<()>,
        relay_rx: Mutex<Option<async_channel::Receiver<RelayTransportEvent>>>,
        sent: Arc<Mutex<Vec<Bytes>>>,
    }
    #[async_trait]
    impl RelayTransportFactory for GatedFactory {
        async fn connect(
            &self,
        ) -> anyhow::Result<(
            Arc<dyn RelayTransport>,
            async_channel::Receiver<RelayTransportEvent>,
        )> {
            // Park until the test releases the gate (after it ran cleanup).
            let _ = self.gate.recv().await;
            let rx = self.relay_rx.lock().unwrap().take().expect("connect once");
            Ok((
                Arc::new(MockTransport {
                    sent: self.sent.clone(),
                }),
                rx,
            ))
        }
    }

    // Finding 1: the call is registered BEFORE the relay connect().await. If cleanup_connection_state
    // (abort_all) runs during that connect gap, the entry is gone by the time connect returns, so
    // set_media_task aborts the just-spawned media task immediately. The call must not survive as a
    // stale entry, and wait_ended() must resolve (the aborted task's drop-guard fires `ended`).
    #[tokio::test]
    async fn cleanup_during_connect_gap_aborts_the_spawned_task() {
        let client = make_client().await;
        let (gate_tx, gate_rx) = async_channel::bounded::<()>(1);
        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let factory = GatedFactory {
            gate: gate_rx,
            relay_rx: Mutex::new(Some(relay_rx)),
            sent: Arc::new(Mutex::new(Vec::new())),
        };
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        // spawn_call registers the entry, then parks inside the gated connect().
        let spawn = tokio::spawn({
            let client = client.clone();
            async move {
                spawn_call(
                    &client,
                    mk_session(),
                    engine(),
                    &factory,
                    pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
                    None,
                )
                .await
            }
        });

        // Wait until the entry is registered (proves register-before-connect), then simulate the
        // disconnect that removes it while connect is still parked.
        for _ in 0..100 {
            if client.call_registry().active_count() == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert_eq!(
            client.call_registry().active_count(),
            1,
            "the call must be registered before the relay connect"
        );
        assert_eq!(client.call_registry().abort_all(), 1, "cleanup removes it");

        // Release the gate so connect returns; set_media_task now finds no entry and aborts the task.
        gate_tx.send(()).await.unwrap();
        let handle = spawn.await.expect("spawn task").expect("spawn_call");

        assert_eq!(
            client.call_registry().active_count(),
            0,
            "the spawned task must not resurrect a stale entry after cleanup"
        );
        tokio::time::timeout(Duration::from_secs(2), handle.wait_ended())
            .await
            .expect("an aborted-before-poll task must still notify ended via the drop-guard");
    }

    // Finding T: the pre-connect re-check inside attach_engine. The entry is registered, then a
    // disconnect clears is_connected while the gated connect is parked. attach_engine re-checks
    // is_connected AFTER the insert but BEFORE connect returns -- here the gate is never released, so
    // the connect would block forever; the re-check must instead self-clean (remove the just-registered
    // entry, notify `ended`) and return a Connect error, so the entry can't leak and wait_ended resolves.
    #[tokio::test]
    async fn cleanup_before_connect_self_cleans_via_preconnect_recheck() {
        let client = make_client().await;
        // Gate is never released: if the re-check didn't fire, connect would park forever and the test
        // would time out instead of asserting.
        let (_gate_tx, gate_rx) = async_channel::bounded::<()>(1);
        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let factory = GatedFactory {
            gate: gate_rx,
            relay_rx: Mutex::new(Some(relay_rx)),
            sent: Arc::new(Mutex::new(Vec::new())),
        };
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        // Disconnect clears is_connected before the connect path runs.
        client.set_connected_for_test(false);

        let res = spawn_call(
            &client,
            mk_session(),
            engine(),
            &factory,
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await;
        assert!(
            matches!(res, Err(CallError::Connect(_))),
            "the pre-connect re-check must surface a Connect error when is_connected is false"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "the pre-connect re-check must reap the just-registered entry (no leak)"
        );
    }

    // Codex window: hangup landing while attach_engine is parked in the relay dial -- registry entry
    // present, media task not registered yet, PendingOutgoing already consumed -- must wake
    // wait_ended() promptly and abort the dial, not park until the dial succeeds or times out. The
    // gate is NEVER released, so a passing test proves the dial was aborted (not awaited).
    #[tokio::test]
    async fn hangup_during_connect_window_resolves_wait_ended_and_aborts_dial() {
        let client = make_client().await;
        let (_gate_tx, gate_rx) = async_channel::bounded::<()>(1);
        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let factory = Arc::new(GatedFactory {
            gate: gate_rx,
            relay_rx: Mutex::new(Some(relay_rx)),
            sent: Arc::new(Mutex::new(Vec::new())),
        });
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        // The registry entry exists (as it would by the time the relay arrives); the handle is already
        // out (as for an outgoing call), sharing the engine's `ended`/`muted`/events state.
        let generation = client.call_registry().insert(mk_session());
        let muted = Arc::new(AtomicBool::new(false));
        let ended = Arc::new(EndedFlag::default());
        let (ev_tx, ev_rx) = async_channel::unbounded::<CallEvent>();
        let handle = CallHandle {
            call_id: "CID-FACADE".into(),
            generation,
            peer_jid: caller(),
            call_creator: caller(),
            client_registry: client.call_registry(),
            pending_outgoing_calls: client.voip_state().pending_outgoing_calls.clone(),
            client: std::sync::Weak::new(),
            muted: muted.clone(),
            video: Arc::new(VideoShared::new()),
            events: ev_rx,
            ended: ended.clone(),
        };

        // Drive attach_engine in the background; it parks in the gated connect.
        let attach = tokio::spawn({
            let client = client.clone();
            let factory = factory.clone();
            async move {
                attach_engine(
                    &client,
                    "CID-FACADE",
                    generation,
                    FailureCleanup::Here,
                    engine(),
                    &*factory,
                    pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
                    None,
                    Arc::new(VideoShared::new()),
                    muted,
                    ended,
                    ev_tx,
                    None,
                )
                .await
            }
        });
        // Let attach_engine reach the gated connect before hanging up.
        tokio::time::sleep(Duration::from_millis(30)).await;

        handle.hangup().await;

        tokio::time::timeout(Duration::from_secs(2), handle.wait_ended())
            .await
            .expect(
                "hangup in the connect window must wake wait_ended without the dial completing",
            );

        let res = tokio::time::timeout(Duration::from_secs(2), attach)
            .await
            .expect("attach_engine must return once hangup aborts the dial")
            .expect("attach task");
        assert!(
            matches!(res, Err(CallError::Connect(_))),
            "an aborted dial surfaces a Connect error"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "hangup must leave no stale registry entry"
        );
    }

    // A disconnect during the connect window must abort the dial and resolve wait_ended(). The
    // task-less registry entry is cleared by abort_all() without touching `ended`, so the dial is
    // raced against the per-connection shutdown signal; without that arm wait_ended() would park
    // until connect() hit its own timeout.
    #[tokio::test]
    async fn disconnect_during_connect_window_resolves_wait_ended_and_aborts_dial() {
        let client = make_client().await;
        let (_gate_tx, gate_rx) = async_channel::bounded::<()>(1);
        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let factory = Arc::new(GatedFactory {
            gate: gate_rx,
            relay_rx: Mutex::new(Some(relay_rx)),
            sent: Arc::new(Mutex::new(Vec::new())),
        });
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        let generation = client.call_registry().insert(mk_session());
        let muted = Arc::new(AtomicBool::new(false));
        let ended = Arc::new(EndedFlag::default());
        // place_call/spawn_call wire this hook; replicate it so removing the entry wakes `ended`.
        client
            .call_registry()
            .set_ended_notify("CID-FACADE", generation, {
                let ended = ended.clone();
                move || ended.notify()
            });
        let (ev_tx, _ev_rx) = async_channel::unbounded::<CallEvent>();

        let attach = tokio::spawn({
            let client = client.clone();
            let factory = factory.clone();
            let ended = ended.clone();
            async move {
                attach_engine(
                    &client,
                    "CID-FACADE",
                    generation,
                    FailureCleanup::Here,
                    engine(),
                    &*factory,
                    pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
                    None,
                    Arc::new(VideoShared::new()),
                    muted,
                    ended,
                    ev_tx,
                    None,
                )
                .await
            }
        });
        // Let attach_engine park in the gated connect.
        tokio::time::sleep(Duration::from_millis(30)).await;

        // A disconnect clears the task-less registry entry, whose on_terminal hook wakes `ended`.
        client.call_registry().abort_all();

        tokio::time::timeout(Duration::from_secs(2), ended.wait())
            .await
            .expect(
                "a disconnect in the connect window must wake `ended` without the dial completing",
            );

        let res = tokio::time::timeout(Duration::from_secs(2), attach)
            .await
            .expect("attach_engine must return once the disconnect aborts the dial")
            .expect("attach task");
        assert!(
            matches!(res, Err(CallError::Connect(_))),
            "an aborted dial surfaces a Connect error"
        );
    }

    // A peer <terminate>/<reject> during the connect window removes the task-less registry entry via
    // terminate_call; its on_terminal hook must wake `ended` (no pending entry to drain, no media task
    // to abort), aborting the dial instead of parking wait_ended() until the connect timeout.
    #[tokio::test]
    async fn peer_terminate_during_connect_window_resolves_wait_ended_and_aborts_dial() {
        let client = make_client().await;
        let (_gate_tx, gate_rx) = async_channel::bounded::<()>(1);
        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let factory = Arc::new(GatedFactory {
            gate: gate_rx,
            relay_rx: Mutex::new(Some(relay_rx)),
            sent: Arc::new(Mutex::new(Vec::new())),
        });
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        let generation = client.call_registry().insert(mk_session());
        let muted = Arc::new(AtomicBool::new(false));
        let ended = Arc::new(EndedFlag::default());
        client
            .call_registry()
            .set_ended_notify("CID-FACADE", generation, {
                let ended = ended.clone();
                move || ended.notify()
            });
        let (ev_tx, _ev_rx) = async_channel::unbounded::<CallEvent>();

        let attach = tokio::spawn({
            let client = client.clone();
            let factory = factory.clone();
            let ended = ended.clone();
            async move {
                attach_engine(
                    &client,
                    "CID-FACADE",
                    generation,
                    FailureCleanup::Here,
                    engine(),
                    &*factory,
                    pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
                    None,
                    Arc::new(VideoShared::new()),
                    muted,
                    ended,
                    ev_tx,
                    None,
                )
                .await
            }
        });
        tokio::time::sleep(Duration::from_millis(30)).await;

        // The peer terminal-stanza path (no pending entry; entry has no media task yet).
        terminate_call(&client, "CID-FACADE");

        tokio::time::timeout(Duration::from_secs(2), ended.wait())
            .await
            .expect("a peer terminate in the connect window must wake `ended`");

        let res = tokio::time::timeout(Duration::from_secs(2), attach)
            .await
            .expect("attach_engine must return once the terminate aborts the dial")
            .expect("attach task");
        assert!(matches!(res, Err(CallError::Connect(_))));
        assert_eq!(client.call_registry().active_count(), 0);
    }

    // A pending-outgoing call with no matching call-id leaves attach_outgoing_relay a no-op (returns
    // false), so an unrelated <call> doesn't spuriously spawn an engine.
    #[tokio::test]
    async fn attach_outgoing_relay_ignores_unknown_call_id() {
        let client = make_client().await;
        let attached = attach_outgoing_relay(&client, "NOT-PENDING", &sample_relay())
            .await
            .expect("attach must not error on an unknown call-id");
        assert!(!attached, "no pending call → no attach");
    }

    /// A factory whose connect() always fails, to exercise attach_engine's connect-error cleanup.
    struct FailingFactory;
    #[async_trait]
    impl RelayTransportFactory for FailingFactory {
        async fn connect(
            &self,
        ) -> anyhow::Result<(
            Arc<dyn RelayTransport>,
            async_channel::Receiver<RelayTransportEvent>,
        )> {
            Err(anyhow::anyhow!("relay handshake timeout"))
        }
    }

    // Finding G: a relay connect() failure must reap the (already-registered) call and wake any
    // wait_ended() waiter. spawn_call inserts the registry entry before connect, so without cleanup the
    // call would leak in the registry and an outgoing handle's wait_ended() would hang. Driven here via
    // the incoming spawn_call path (registry-insert before connect); the outgoing path shares the same
    // attach_engine.
    #[tokio::test]
    async fn connect_failure_reaps_registry_and_resolves_wait_ended() {
        let client = make_client().await;
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        // CallHandle has no Debug, so match on the Result rather than expect_err.
        let res = spawn_call(
            &client,
            mk_session(),
            engine(),
            &FailingFactory,
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await;
        assert!(
            matches!(res, Err(CallError::Connect(_))),
            "a connect failure must surface as a Connect error"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "a connect failure must not leak the registry entry"
        );
    }

    #[tokio::test]
    async fn answered_call_connect_failure_terminates_the_peer() {
        let (client, sent_count) = make_sending_client().await;
        let mut registration = RegisteredCall::new(&client, mk_session()).await;
        let mut teardown = AnswerTeardown::new(&client, &registration);
        teardown.arm();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        let result = spawn_answered_call(
            &client,
            &mut registration,
            teardown,
            engine(),
            &FailingFactory,
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await;

        assert!(matches!(result, Err(CallError::Connect(_))));
        assert_eq!(
            sent_count.load(Ordering::SeqCst),
            1,
            "a post-accept relay failure must send one terminate stanza"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "the failed answer must not leak its registered generation"
        );
    }

    #[tokio::test]
    async fn failed_group_offer_startup_terminates_the_call_scope() {
        let (client, _sent_count) = make_sending_client().await;
        let call_id = "GROUP-OFFER-FAILED";
        let creator = Jid::new("111111111111111", Server::Lid);
        let mut session = wacore::voip::CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator,
        );
        let _ = session.transition_to(CallPhase::Calling);
        let mut registration = RegisteredCall::new(&client, session).await;
        let terminate_waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        drop(GroupOfferTeardown::new(&client, &mut registration));

        let terminate = tokio::time::timeout(Duration::from_secs(2), terminate_waiter)
            .await
            .expect("group startup failure must send terminate")
            .expect("terminate waiter");
        let node = terminate.as_node_ref();
        assert_eq!(
            node.attrs().optional_jid("to"),
            Some(Jid::new(call_id, Server::Call))
        );
        let action = &node.children().expect("terminate action")[0];
        assert_eq!(action.tag.as_ref(), "terminate");
        assert_eq!(
            action.attrs().optional_string("call-id").as_deref(),
            Some(call_id)
        );
    }

    #[tokio::test]
    async fn group_offer_teardown_serializes_reoffer_until_terminate_is_sent() {
        let client = make_client().await;
        let (transport, entered_rx, release_tx) = gated_send_transport(0, false);
        install_noise_transport(&client, transport).await;
        let mut registration = RegisteredCall::new(&client, mk_session()).await;
        let stale_generation = registration.generation;
        drop(GroupOfferTeardown::new(&client, &mut registration));

        tokio::time::timeout(Duration::from_secs(2), entered_rx.recv())
            .await
            .expect("terminate transport attempt")
            .expect("send gate");
        assert_eq!(
            client.call_registry().generation_of("CID-FACADE"),
            None,
            "the failed generation is claimed before its terminal send"
        );

        let mut replacement = Box::pin(RegisteredCall::new(&client, mk_session()));
        assert!(
            tokio::time::timeout(Duration::from_millis(20), replacement.as_mut())
                .await
                .is_err(),
            "a same-call-id re-offer must wait while group termination is in flight"
        );
        release_tx.send(()).await.expect("release terminate send");
        let replacement = replacement.await;
        assert_ne!(replacement.generation, stale_generation);
        assert_eq!(
            client.call_registry().generation_of("CID-FACADE"),
            Some(replacement.generation)
        );
    }

    async fn start_waiting_room_call_link(
        call_id: &'static str,
    ) -> (
        Arc<Client>,
        Arc<crate::transport::mock::CapturingMockTransport>,
        tokio::task::JoinHandle<Result<CallHandle, CallError>>,
        Jid,
        Jid,
        u64,
    ) {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let own_lid = Jid::new("111111111111111", Server::Lid).with_device(1);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;
        let creator = Jid::new("333333333333333", Server::Lid);
        let join_sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let start_client = client.clone();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (speaker_tx, _speaker_rx) = async_channel::unbounded::<Vec<i16>>();
        let start = tokio::spawn(async move {
            start_client
                .voip()
                .call_link("TEST-CALL-LINK", CallLinkMedia::Audio)
                .audio(mic_rx, speaker_tx)
                .start()
                .await
        });
        let request = join_sent.await.expect("call-link join request");
        let request_id = request
            .as_node_ref()
            .attrs()
            .optional_string("id")
            .expect("request id")
            .into_owned();
        crate::test_utils::answer_iq(
            &client,
            &request_id,
            &wacore_binary::builder::NodeBuilder::new("ack")
                .attr("class", "call")
                .attr("type", "link_join")
                .attr("id", request_id.as_str())
                .children([wacore_binary::builder::NodeBuilder::new("waiting_room")
                    .attr("call-id", call_id)
                    .attr("call-creator", creator.clone())
                    .attr("link-token", "TEST-CALL-LINK")
                    .attr("media", "audio")
                    .attr("enabled", "1")
                    .attr("is_admin", "0")
                    .attr("transaction-id", "1")
                    .build()])
                .build(),
        )
        .await;

        let generation = loop {
            if let Some(generation) = client.call_registry().generation_of(call_id) {
                break generation;
            }
            tokio::task::yield_now().await;
        };
        assert_eq!(
            client.call_registry().phase_if_current(call_id, generation),
            Some(CallPhase::WaitingRoom)
        );
        (client, transport, start, own_lid, creator, generation)
    }

    #[tokio::test]
    async fn cancelling_call_link_in_waiting_room_sends_no_terminate() {
        let (client, transport, start, _own_lid, _creator, _generation) =
            start_waiting_room_call_link("WAITING-CANCELLATION").await;

        start.abort();
        match start.await {
            Err(error) => assert!(error.is_cancelled()),
            Ok(_) => panic!("call-link start must cancel"),
        }
        for _ in 0..20 {
            if client
                .call_registry()
                .generation_of("WAITING-CANCELLATION")
                .is_none()
            {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(
            client.call_registry().generation_of("WAITING-CANCELLATION"),
            None,
            "cancellation must reap the exact waiting-room generation"
        );
        for index in 0..transport.sent_count() {
            let node = crate::test_utils::decode_sent_iq(&transport, index).await;
            let node = node.get();
            let sent_terminate = node.tag == "call"
                && node.children().is_some_and(|children| {
                    children.iter().any(|child| {
                        child.tag == "terminate"
                            && child.attrs().optional_string("call-id").as_deref()
                                == Some("WAITING-CANCELLATION")
                    })
                });
            assert!(
                !sent_terminate,
                "leaving a waiting room is local and must not terminate a call scope"
            );
        }
    }

    #[tokio::test]
    async fn cancelling_call_link_after_relayless_admission_terminates_the_call_scope() {
        let call_id = "RELAYLESS-ADMISSION";
        let (client, _transport, start, own_lid, creator, generation) =
            start_waiting_room_call_link(call_id).await;
        let mut participant =
            GroupCallParticipant::new(own_lid.to_non_ad(), vec![GroupCallDevice::new(own_lid)]);
        participant.state = Some("connected".to_string());
        assert_eq!(
            client.call_registry().apply_group_update(
                GroupCallUpdate::builder()
                    .call_id(call_id.to_string())
                    .call_creator(creator.clone())
                    .transaction_id(2)
                    .media("audio".to_string())
                    .connected_limit(32)
                    .joinable(true)
                    .av_upgradable(true)
                    .rekey_requested(false)
                    .participants(vec![participant])
                    .build(),
            ),
            wacore::voip::GroupStateApply::Applied
        );
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.call_registry().phase_if_current(call_id, generation)
                != Some(CallPhase::Connecting)
            {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the admitted snapshot must drive the call into Connecting");

        let terminate_sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        start.abort();
        match start.await {
            Err(error) => assert!(error.is_cancelled()),
            Ok(_) => panic!("call-link start must be cancelled"),
        }
        let terminate = tokio::time::timeout(Duration::from_secs(2), terminate_sent)
            .await
            .expect("admitted cancellation must send terminate")
            .expect("terminate stanza");
        let terminate_ref = terminate.as_node_ref();
        let action = &terminate_ref.children().expect("terminate action")[0];
        assert_eq!(action.tag.as_ref(), "terminate");
        assert_eq!(
            action.attrs().optional_string("call-id").as_deref(),
            Some(call_id)
        );
        assert_eq!(client.call_registry().generation_of(call_id), None);
    }

    #[tokio::test(start_paused = true)]
    async fn relayless_call_link_admission_times_out_and_terminates() {
        let call_id = "RELAYLESS-TIMEOUT";
        let (client, _transport, start, own_lid, creator, generation) =
            start_waiting_room_call_link(call_id).await;
        let mut participant =
            GroupCallParticipant::new(own_lid.to_non_ad(), vec![GroupCallDevice::new(own_lid)]);
        participant.state = Some("connected".to_string());
        assert_eq!(
            client.call_registry().apply_group_update(
                GroupCallUpdate::builder()
                    .call_id(call_id.to_string())
                    .call_creator(creator)
                    .transaction_id(2)
                    .media("audio".to_string())
                    .connected_limit(32)
                    .joinable(true)
                    .av_upgradable(true)
                    .rekey_requested(false)
                    .participants(vec![participant])
                    .build(),
            ),
            wacore::voip::GroupStateApply::Applied
        );
        assert_eq!(
            client.call_registry().phase_if_current(call_id, generation),
            Some(CallPhase::Connecting)
        );
        let terminate_sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let deadline =
            tokio::time::Instant::now() + OFFER_ACK_RELAY_TIMEOUT + OFFER_ACK_RELAY_TIMEOUT;
        while !start.is_finished() && tokio::time::Instant::now() < deadline {
            tokio::time::advance(Duration::from_millis(100)).await;
            tokio::task::yield_now().await;
        }
        assert!(
            start.is_finished(),
            "the bounded relay setup timeout must complete the call-link task"
        );

        let result = start.await.expect("call-link setup task");
        assert!(matches!(result, Err(CallError::ResponseTimeout)));
        let terminate = terminate_sent.await.expect("timeout terminate stanza");
        let terminate = terminate.as_node_ref();
        let action = &terminate.children().expect("terminate action")[0];
        assert_eq!(action.tag.as_ref(), "terminate");
        assert_eq!(
            action.attrs().optional_string("call-id").as_deref(),
            Some(call_id)
        );
        assert_eq!(client.call_registry().generation_of(call_id), None);
    }

    #[tokio::test]
    async fn cancelling_group_offer_during_send_terminates_the_call_scope() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        struct CancelAwareTransport {
            attempts: AtomicUsize,
            first_started: async_channel::Sender<()>,
            first_release: async_channel::Receiver<()>,
            cleanup_sent: async_channel::Sender<()>,
        }

        #[async_trait]
        impl crate::transport::Transport for CancelAwareTransport {
            async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
                if self.attempts.fetch_add(1, Ordering::SeqCst) == 0 {
                    let _ = self.first_started.try_send(());
                    self.first_release
                        .recv()
                        .await
                        .map_err(|_| anyhow::anyhow!("offer release closed"))?;
                    return Ok(());
                }
                let _ = self.cleanup_sent.try_send(());
                Ok(())
            }

            async fn disconnect(&self) {}
        }

        let client = make_client().await;
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                Jid::new("111111111111111", Server::Lid).with_device(1),
            )))
            .await;
        let targets = [
            Jid::new("222222222222222", Server::Lid),
            Jid::new("333333333333333", Server::Lid),
        ];
        for target in &targets {
            client
                .update_device_list(DeviceListRecord {
                    user: target.user.to_string(),
                    devices: vec![DeviceInfo::new(0, None)],
                    timestamp: wacore::time::now_secs(),
                    phash: None,
                    raw_id: None,
                })
                .await
                .expect("seed target device");
        }
        let (first_tx, first_rx) = async_channel::bounded(1);
        let (release_tx, release_rx) = async_channel::bounded(1);
        let (cleanup_tx, cleanup_rx) = async_channel::bounded(1);
        install_noise_transport(
            &client,
            Arc::new(CancelAwareTransport {
                attempts: AtomicUsize::new(0),
                first_started: first_tx,
                first_release: release_rx,
                cleanup_sent: cleanup_tx,
            }),
        )
        .await;

        let start_client = client.clone();
        let start_targets = targets.clone();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (speaker_tx, _speaker_rx) = async_channel::unbounded::<Vec<i16>>();
        let start = tokio::spawn(async move {
            start_client
                .voip()
                .group_call(&start_targets)
                .audio(mic_rx, speaker_tx)
                .start()
                .await
        });
        tokio::time::timeout(Duration::from_secs(2), first_rx.recv())
            .await
            .expect("group offer send must start")
            .expect("offer observer");
        start.abort();
        match start.await {
            Err(error) => assert!(error.is_cancelled()),
            Ok(_) => panic!("start must be cancelled"),
        }
        release_tx.send(()).await.expect("release ambiguous send");
        tokio::time::timeout(Duration::from_secs(2), cleanup_rx.recv())
            .await
            .expect("cancellation must schedule a terminate send")
            .expect("terminate observer");
    }

    #[tokio::test]
    async fn outgoing_group_offer_registers_before_its_ack_can_be_overtaken() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};

        let (client, _transport) = crate::test_utils::create_iq_test_client().await;
        let own_lid = Jid::new("111111111111111", Server::Lid).with_device(1);
        client
            .persistence_manager()
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own_lid.clone(),
            )))
            .await;
        let targets = [
            Jid::new("222222222222222", Server::Lid),
            Jid::new("333333333333333", Server::Lid),
        ];
        for target in &targets {
            client
                .update_device_list(DeviceListRecord {
                    user: target.user.to_string(),
                    devices: vec![DeviceInfo::new(0, None)],
                    timestamp: wacore::time::now_secs(),
                    phash: None,
                    raw_id: None,
                })
                .await
                .expect("seed target device");
        }

        let sent = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        let start_client = client.clone();
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (speaker_tx, _speaker_rx) = async_channel::unbounded::<Vec<i16>>();
        let start = tokio::spawn(async move {
            start_client
                .voip()
                .group_call(&targets)
                .audio(mic_rx, speaker_tx)
                .start()
                .await
        });
        let offer = sent.await.expect("initial group offer");
        let offer_ref = offer.as_node_ref();
        let action = &offer_ref.children().expect("offer action")[0];
        let call_id = action
            .attrs()
            .optional_string("call-id")
            .expect("call id")
            .into_owned();
        let generation = client
            .call_registry()
            .generation_of(&call_id)
            .expect("group call must be registered before awaiting its ACK");
        assert!(
            client.call_registry().is_group_call(&call_id),
            "the pre-ACK generation must already reject direct-call teardown semantics"
        );
        let overtaking = GroupCallUpdate::builder()
            .call_id(call_id.clone())
            .call_creator(own_lid)
            .transaction_id(2)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        assert_eq!(
            client
                .call_registry()
                .apply_group_update_if_current(overtaking, generation,),
            wacore::voip::GroupStateApply::Applied,
            "a group update that overtakes the ACK must be retained"
        );

        start.abort();
        match start.await {
            Err(error) => assert!(error.is_cancelled()),
            Ok(_) => panic!("start should be cancelled"),
        }
        tokio::time::timeout(Duration::from_secs(2), async {
            while client.call_registry().generation_of(&call_id).is_some() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("cancellation must reap the pre-ACK registration");
    }

    #[tokio::test]
    async fn registered_outgoing_group_retains_roster_before_media_attach() {
        use wacore::voip::{GroupControl, GroupStateApply};

        let client = make_client().await;
        let call_id = "OUTGOING-GROUP-SETUP";
        let creator = Jid::new("111111111111111", Server::Lid);
        let update = |transaction_id| {
            GroupCallUpdate::builder()
                .call_id(call_id.to_string())
                .call_creator(creator.clone())
                .transaction_id(transaction_id)
                .media("audio".to_string())
                .connected_limit(32)
                .joinable(true)
                .av_upgradable(true)
                .rekey_requested(false)
                .participants(Vec::new())
                .build()
        };
        let mut session = wacore::voip::CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator.clone(),
        );
        let _ = session.transition_to(CallPhase::Calling);
        let registration = RegisteredCall::new(&client, session).await;

        assert_eq!(
            client.call_registry().apply_group_update(update(2)),
            GroupStateApply::Applied,
            "an update arriving during epoch fan-out must find the early registration"
        );
        assert_eq!(
            client
                .call_registry()
                .apply_group_update_if_current(update(1), registration.generation),
            GroupStateApply::Stale,
            "the older ACK snapshot must not replace an overtaking group update"
        );
        let (tx, rx) = async_channel::bounded(4);
        client.call_registry().set_group_control_sender(
            call_id,
            registration.generation,
            Some(4),
            tx,
        );
        match rx.try_recv().expect("latest roster reaches attached media") {
            GroupControl::Update(update) => assert_eq!(update.transaction_id, 2),
            _ => panic!("expected the retained roster"),
        }
    }

    #[tokio::test]
    async fn call_link_media_attachment_preserves_registered_generation_and_epoch() {
        use wacore::voip::{GroupControl, GroupStateApply};

        let client = make_client().await;
        let call_id = "CALL-LINK-ATTACH";
        let creator = Jid::new("111111111111111", Server::Lid);
        let update = GroupCallUpdate::builder()
            .call_id(call_id.to_string())
            .call_creator(creator.clone())
            .transaction_id(7)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(true)
            .participants(Vec::new())
            .build();
        let mut session = wacore::voip::CallSession::new_outgoing(
            call_id,
            Jid::new(call_id, Server::Call),
            creator,
        );
        session.group = Some(update);
        let registry = client.call_registry();
        let generation = registry
            .insert_call_link_checked(session)
            .expect("valid call-link session");
        assert_eq!(
            registry.apply_waiting_room(
                wacore::types::group_call::WaitingRoom::builder()
                    .call_id(call_id.to_string())
                    .call_creator(Jid::new("111111111111111", Server::Lid))
                    .link_token("TEST-CALL-LINK".to_string())
                    .media(CallLinkMedia::Audio)
                    .enabled(false)
                    .is_admin(true)
                    .transaction_id(7)
                    .users(Vec::new())
                    .build(),
            ),
            GroupStateApply::Applied
        );
        assert!(registry.send_group_epoch_if_current(call_id, generation, 7, vec![7; 32]));

        let mut registration =
            RegisteredCall::from_existing(&client, call_id, generation).expect("existing call");
        assert_eq!(
            registry.generation_of(call_id),
            Some(generation),
            "media attachment must not replace the admitted call-link generation"
        );
        assert!(
            registry
                .group_state_if_current(call_id, generation)
                .and_then(|state| state.waiting_room().cloned())
                .is_some_and(|room| room.is_admin),
            "the retained waiting-room/admin snapshot must survive media attachment"
        );

        let (tx, rx) = async_channel::bounded(4);
        registry.set_group_control_sender(call_id, generation, Some(4), tx);
        assert!(matches!(
            rx.try_recv(),
            Ok(GroupControl::Transition { update, epoch })
                if update.transaction_id == 7 && epoch.transaction_id == 7
        ));
        assert!(rx.try_recv().is_err());
        registration.disarm();
        registry.remove_if_current(call_id, generation);
    }

    #[tokio::test]
    async fn answered_call_supersession_does_not_terminate_replacement() {
        let (client, sent_count) = make_sending_client().await;
        let mut registration = RegisteredCall::new(&client, mk_session()).await;
        let stale_generation = registration.generation;
        let mut teardown = AnswerTeardown::new(&client, &registration);
        teardown.arm();
        let (_gate_tx, gate_rx) = async_channel::bounded::<()>(1);
        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let factory = GatedFactory {
            gate: gate_rx,
            relay_rx: Mutex::new(Some(relay_rx)),
            sent: Arc::new(Mutex::new(Vec::new())),
        };
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();

        let spawn = spawn_answered_call(
            &client,
            &mut registration,
            teardown,
            engine(),
            &factory,
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        );
        let replace = async {
            tokio::time::sleep(Duration::from_millis(20)).await;
            RegisteredCall::new(&client, mk_session()).await
        };
        let (result, replacement) = tokio::join!(spawn, replace);

        assert!(matches!(result, Err(CallError::Connect(_))));
        assert_ne!(replacement.generation, stale_generation);
        assert_eq!(
            client.call_registry().generation_of("CID-FACADE"),
            Some(replacement.generation),
            "the replacement generation must remain registered"
        );
        assert_eq!(
            sent_count.load(Ordering::SeqCst),
            0,
            "supersession must not send a terminate for the replacement call"
        );
    }

    #[tokio::test]
    async fn answer_teardown_serializes_reoffer_until_terminate_is_sent() {
        let client = make_client().await;
        let (transport, entered_rx, release_tx) = gated_send_transport(0, false);
        install_noise_transport(&client, transport).await;
        let registration = RegisteredCall::new(&client, mk_session()).await;
        let stale_generation = registration.generation;
        let mut teardown = AnswerTeardown::new(&client, &registration);
        teardown.arm();

        let terminate = teardown.terminate(&client);
        let replace = async {
            entered_rx
                .recv()
                .await
                .expect("terminate transport attempt");
            assert_eq!(
                client.call_registry().generation_of("CID-FACADE"),
                None,
                "the failed generation is claimed before its terminal send"
            );

            let mut replacement = Box::pin(RegisteredCall::new(&client, mk_session()));
            assert!(
                tokio::time::timeout(Duration::from_millis(20), replacement.as_mut())
                    .await
                    .is_err(),
                "a same-call-id re-offer must wait while terminate is in flight"
            );
            assert_eq!(
                client.call_registry().generation_of("CID-FACADE"),
                None,
                "the replacement must not enter the registry before terminate is written"
            );

            release_tx.send(()).await.expect("release terminate send");
            replacement.await
        };

        let ((), replacement) = tokio::join!(terminate, replace);
        assert_ne!(replacement.generation, stale_generation);
        assert_eq!(
            client.call_registry().generation_of("CID-FACADE"),
            Some(replacement.generation),
            "the replacement may register only after the stale terminal send completes"
        );
    }

    #[tokio::test]
    async fn cancelling_answered_call_startup_terminates_the_peer() {
        let (client, _sent_count) = make_sending_client().await;
        let mut registration = RegisteredCall::new(&client, mk_session()).await;
        let mut teardown = AnswerTeardown::new(&client, &registration);
        teardown.arm();
        let (_gate_tx, gate_rx) = async_channel::bounded::<()>(1);
        let (_relay_tx, relay_rx) = async_channel::unbounded();
        let factory = GatedFactory {
            gate: gate_rx,
            relay_rx: Mutex::new(Some(relay_rx)),
            sent: Arc::new(Mutex::new(Vec::new())),
        };
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let terminate_waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));

        let result = tokio::time::timeout(
            Duration::from_millis(20),
            spawn_answered_call(
                &client,
                &mut registration,
                teardown,
                engine(),
                &factory,
                pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
                None,
            ),
        )
        .await;
        assert!(result.is_err(), "relay startup must still be pending");

        let terminate = tokio::time::timeout(Duration::from_secs(2), terminate_waiter)
            .await
            .expect("cancelled startup must send terminate")
            .expect("terminate waiter");
        assert_eq!(
            terminate
                .as_node_ref()
                .children()
                .expect("terminate action")[0]
                .tag
                .as_ref(),
            "terminate"
        );
        assert_eq!(client.call_registry().active_count(), 0);
    }

    /// A sending client with NO noise socket set, so send_node fails (get_noise_socket errors). The
    /// signal encrypt path is independent of the noise socket, so place_call still builds the offer and
    /// only the send fails, exercising the send-failure cleanup.
    async fn make_failing_send_client() -> Arc<Client> {
        let backend = create_test_backend().await;
        let pm = PersistenceManager::new(backend).await.expect("pm");
        pm.process_command(crate::store::commands::DeviceCommand::SetLid(Some(
            Jid::new("111111111111111", Server::Lid),
        )))
        .await;
        pm.process_command(crate::store::commands::DeviceCommand::SetAccount(Some(
            wa::ADVSignedDeviceIdentity {
                details: Some(vec![0u8; 32]),
                account_signature_key: Some(vec![0u8; 32]),
                account_signature: Some(vec![0u8; 64]),
                device_signature: Some(vec![0u8; 64]),
            },
        )))
        .await;
        let transport = Arc::new(crate::transport::mock::MockTransportFactory::new());
        let (client, _rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(pm),
            transport,
            Arc::new(MockHttpClient),
            None,
        )
        .await;
        // Intentionally leave noise_socket unset so send_node errors.
        client.set_connected_for_test(true);
        client
    }

    // Finding H: place_call registers the outgoing call and parks the pending entry BEFORE send_node,
    // so a fast server answer's relay can be attached even while the send is in flight. If the send then
    // fails, the registration must be undone: the pending entry dropped, the registry generation reaped,
    // and (since a pending entry existed) wait_ended() woken. The call must not leak.
    #[tokio::test]
    async fn place_call_send_failure_cleans_up_registration() {
        let client = make_failing_send_client().await;
        let peer_user = Jid::new("333333333333333", Server::Lid);
        let device = peer_lid();
        seed_peer_session(&client, &device).await;
        let own_lid = client.lid().expect("own lid");
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let call_id = "00abcdef0123456789abcdef0123dead".to_string();

        // CallHandle has no Debug, so match on the Result rather than expect_err. The error must come
        // from the send (Send), not the offer build (Setup/Media).
        let res = place_call(
            &client,
            call_id.clone(),
            &peer_user,
            &own_lid,
            &own_lid,
            std::slice::from_ref(&device),
            std::slice::from_ref(&device),
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await;
        assert!(
            matches!(res, Err(CallError::Send(_))),
            "a send failure must surface as a Send error"
        );
        assert!(
            client
                .voip_state()
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .is_empty(),
            "a send failure must drop the parked pending entry"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "a send failure must reap the registry generation"
        );
    }

    // Finding I: attach_outgoing_relay removes the pending entry FIRST, then builds the engine. If that
    // build fails (here: a relay with an out-of-range warp_mi_tag_len makes CallEngine::new error), the
    // generation must still be reaped and `ended` notified, else the handed-out handle's wait_ended()
    // hangs forever with no pending entry left for a later hangup to drain.
    #[tokio::test]
    async fn attach_outgoing_relay_setup_error_reaps_and_resolves_wait_ended() {
        let (client, _count) = make_sending_client().await;
        let (handle, call_id) = place_dormant_outgoing(&client).await;
        assert_eq!(client.call_registry().active_count(), 1);

        // A relay whose warp_mi_tag_len is out of the 1..=20 range drives MediaPipeline::new (via
        // CallEngine::new) to None, so attach_outgoing_relay errors in the setup window.
        let mut relay = sample_relay();
        relay.warp_mi_tag_len = Some(99);
        let res = attach_outgoing_relay(&client, &call_id, &relay).await;
        assert!(
            matches!(res, Err(CallError::Setup(_))),
            "an out-of-range warp_mi_tag_len must surface as a Setup error"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "a setup error in attach_outgoing_relay must reap the registry generation"
        );
        tokio::time::timeout(Duration::from_secs(2), handle.wait_ended())
            .await
            .expect("a setup error must resolve the handle's wait_ended, not hang it");
    }

    // Finding M: if hangup() races the relay ack -- removing the registry entry while
    // attach_outgoing_relay already holds the pending entry it just removed -- the generation-mismatch
    // branch must notify `ended`, else the handle's wait_ended() hangs (hangup found no pending to
    // notify).
    #[tokio::test]
    async fn attach_outgoing_relay_superseded_resolves_wait_ended() {
        let (client, _count) = make_sending_client().await;
        let (handle, call_id) = place_dormant_outgoing(&client).await;
        assert_eq!(client.call_registry().active_count(), 1);

        // Simulate a hangup landing between attach's pending-remove and its generation check: the
        // registry entry is gone but the pending entry is still present for attach to consume.
        client.call_registry().remove(&call_id);
        let res = attach_outgoing_relay(&client, &call_id, &sample_relay()).await;
        assert!(
            matches!(res, Ok(true)),
            "a superseded attach returns Ok(true)"
        );
        tokio::time::timeout(Duration::from_secs(2), handle.wait_ended())
            .await
            .expect("a superseded attach must resolve the handle's wait_ended, not hang it");
    }

    // Finding J: the relay-waiter, on the timeout / closed-channel (no-ack) paths, must remove the
    // ack-waiter it registered in response_waiters, else send_keepalive suppresses pings for the life of
    // the connection. Drive it via the closed-channel branch (drop the sender) and assert the offer's
    // stanza-id waiter is gone.
    #[tokio::test]
    async fn relay_waiter_no_ack_removes_response_waiter() {
        // make_client's Client::new already sets self_weak, so spawn_outgoing_relay_waiter can upgrade
        // an owned Arc<Client>.
        let client = make_client().await;

        let offer_stanza_id = "OFFER-STANZA-J".to_string();
        let ack_rx = client.register_ack_waiter(&offer_stanza_id);
        assert!(
            client
                .response_waiters_guard()
                .contains_key(&offer_stanza_id),
            "the ack-waiter must be registered"
        );
        // Drop the sender (re-register a NEW waiter under the same id) so ack_rx closes immediately and
        // the task takes the no-ack closed-channel branch without waiting out the full timeout. The new
        // waiter is what the cleanup must then remove.
        let _shadow_rx = client.register_ack_waiter(&offer_stanza_id);

        // No matching pending entry: the task's attach is a harmless no-op, but the response_waiters
        // cleanup must still run.
        spawn_outgoing_relay_waiter(
            &client,
            "00absent00absent00absent00absent".into(),
            0,
            offer_stanza_id.clone(),
            ack_rx,
        );

        // The spawned task drops the now-dangling waiter on the no-ack path.
        for _ in 0..200 {
            if !client
                .response_waiters_guard()
                .contains_key(&offer_stanza_id)
            {
                break;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        assert!(
            !client
                .response_waiters_guard()
                .contains_key(&offer_stanza_id),
            "a no-ack relay-waiter must drop its response_waiters entry so keepalive isn't suppressed"
        );
    }

    /// A sending client with an own LID but NO ADV account, so an outgoing pkmsg offer has no
    /// <device-identity> to attach. Mirrors make_sending_client minus the SetAccount.
    async fn make_no_account_client() -> Arc<Client> {
        let backend = create_test_backend().await;
        let pm = PersistenceManager::new(backend).await.expect("pm");
        pm.process_command(crate::store::commands::DeviceCommand::SetLid(Some(
            Jid::new("111111111111111", Server::Lid),
        )))
        .await;
        let transport = Arc::new(crate::transport::mock::MockTransportFactory::new());
        let (client, _rx) = Client::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            Arc::new(pm),
            transport,
            Arc::new(MockHttpClient),
            None,
        )
        .await;
        client.set_connected_for_test(true);
        client
    }

    // Finding K: a fresh session yields a pkmsg <enc>; if we hold no ADV account the peer can't validate
    // the pre-key message, so place_call must refuse BEFORE any registration/send (mirroring the
    // peer-send path). No registry/pending entry may leak and nothing is sent.
    #[tokio::test]
    async fn place_call_pkmsg_without_account_refuses() {
        let client = make_no_account_client().await;
        let peer_user = Jid::new("333333333333333", Server::Lid);
        let device = peer_lid();
        seed_peer_session(&client, &device).await;
        let own_lid = client.lid().expect("own lid");
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let call_id = "00abcdef0123456789abcdef0123feed".to_string();

        let res = place_call(
            &client,
            call_id.clone(),
            &peer_user,
            &own_lid,
            &own_lid,
            std::slice::from_ref(&device),
            std::slice::from_ref(&device),
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await;
        assert!(
            matches!(res, Err(CallError::MissingDeviceIdentity)),
            "a pkmsg offer with no ADV account must refuse before send"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "a refused offer must not register the call"
        );
        assert!(
            client
                .voip_state()
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .is_empty(),
            "a refused offer must not park a pending entry"
        );
    }

    // Finding P: media keys derive from the peer's LID, so a PN callee whose LID we can't resolve must
    // be rejected rather than deriving non-matching keys from the PN string. On a cache miss the facade
    // now attempts an active usync resolve first; here the test client is not running, so the usync
    // fails fast (Setup) instead of returning a no-LID (Media). Either way the call is rejected and no
    // key is derived off the raw PN.
    #[tokio::test]
    async fn call_pn_callee_without_known_lid_is_rejected() {
        let (client, _count) = make_sending_client().await;
        let pn_peer = Jid::new("559900000000", Server::Pn);
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let res = client
            .voip()
            .call(&pn_peer)
            .audio(mic_rx, spk_tx)
            .start()
            .await;
        assert!(
            matches!(res, Err(CallError::Media(_) | CallError::Setup(_))),
            "a PN callee with no resolvable LID must be rejected, not keyed off the raw PN"
        );
    }

    // Item 2: the outgoing-offer device pre-filter, exercised as pure helpers (the async would_pkmsg
    // I/O is split out). Hosted (Cloud-API) companions are dropped; with no ADV account only the
    // plain-msg devices survive and an all-pkmsg set yields MissingDeviceIdentity.
    #[test]
    fn drop_hosted_devices_removes_cloud_api_companions() {
        let regular0 = Jid::new("333333333333333", Server::Lid);
        let hosted_dev99 = Jid::new("333333333333333", Server::Lid).with_device(99);
        let hosted_server = Jid::new("444444444444444", Server::Hosted);

        let kept = drop_hosted_devices(vec![regular0.clone(), hosted_dev99, hosted_server]);
        assert_eq!(
            kept,
            vec![regular0],
            "device 99 and @hosted companions must be dropped, the regular device kept"
        );
    }

    #[test]
    fn keep_non_pkmsg_devices_filters_and_errors() {
        let d0 = Jid::new("333333333333333", Server::Lid);
        let d1 = Jid::new("333333333333333", Server::Lid).with_device(1);

        // Mixed: the msg device is kept, the pkmsg device dropped (no identity to attach).
        let kept = keep_non_pkmsg_devices(vec![d0.clone(), d1.clone()], &[false, true])
            .expect("a non-pkmsg device survives");
        assert_eq!(kept, vec![d0.clone()]);

        // Every device would be pkmsg: nothing to offer without a <device-identity>.
        let err = keep_non_pkmsg_devices(vec![d0, d1], &[true, true]);
        assert!(matches!(err, Err(CallError::MissingDeviceIdentity)));
    }

    #[test]
    fn outgoing_offer_capability_matches_audio_profile() {
        assert_eq!(
            offer_capability(false, AudioFormat::MLOW_16KHZ_60MS),
            CAPABILITY_OFFER
        );
        assert_eq!(
            offer_capability(true, AudioFormat::MLOW_16KHZ_60MS),
            CAPABILITY_VIDEO_OFFER
        );
        assert_eq!(
            offer_capability(false, AudioFormat::OPUS_16KHZ_60MS),
            CAPABILITY_STANDARD_OPUS_OFFER
        );
        assert_eq!(
            offer_capability(true, AudioFormat::OPUS_16KHZ_60MS),
            CAPABILITY_STANDARD_OPUS_VIDEO_OFFER
        );
    }

    /// A live handle over a mock relay + a sending client, for the video handshake tests. The
    /// returned relay sender is a keepalive: dropping it disconnects the relay and ends the call.
    async fn sending_handle() -> (
        Arc<Client>,
        Arc<AtomicUsize>,
        CallHandle,
        async_channel::Sender<RelayTransportEvent>,
    ) {
        let (client, sent_count) = make_sending_client().await;
        let (relay_tx, relay_rx) = async_channel::unbounded();
        let factory = MockFactory {
            sent: Arc::new(Mutex::new(Vec::new())),
            relay_rx: Mutex::new(Some(relay_rx)),
            connects: Arc::new(AtomicUsize::new(0)),
        };
        let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
        let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
        let handle = spawn_call(
            &client,
            mk_session(),
            engine(),
            &factory,
            pcm_audio(Arc::new(mic_rx), Arc::new(spk_tx)),
            None,
        )
        .await
        .expect("spawn_call");
        (client, sent_count, handle, relay_tx)
    }

    fn video_endpoints() -> (
        async_channel::Receiver<Vec<u8>>,
        async_channel::Sender<VideoFrame>,
    ) {
        let (_vin_tx, vin_rx) = async_channel::unbounded::<Vec<u8>>();
        let (vout_tx, _vout_rx) = async_channel::unbounded::<VideoFrame>();
        // Keep the unused halves alive long enough for the test by leaking them into the channels'
        // refcounts via clone-on-return (the closed-channel behavior is covered elsewhere).
        std::mem::forget(_vin_tx);
        std::mem::forget(_vout_rx);
        (vin_rx, vout_tx)
    }

    async fn peer_upgrade_request(handle: &CallHandle) -> VideoUpgradeToken {
        let transition_lock = handle
            .client_registry
            .video_transition_lock(&handle.call_id, handle.generation)
            .expect("active call");
        let _guard = transition_lock.lock().await;
        match handle.client_registry.apply_peer_video_state(
            &handle.call_id,
            handle.generation,
            VideoState::UpgradeRequestV2,
        ) {
            wacore::voip::PeerVideoTransition::UpgradeRequested(token) => token,
            transition => panic!("unexpected transition: {transition:?}"),
        }
    }

    struct TimedVideoSource {
        frames: async_channel::Receiver<Vec<u8>>,
        timestamp_stride: u32,
    }

    impl VideoSource for TimedVideoSource {
        fn frames(&self) -> async_channel::Receiver<Vec<u8>> {
            self.frames.clone()
        }

        fn rtp_timestamp_stride(&self) -> u32 {
            self.timestamp_stride
        }
    }

    struct DropTrackedVideoSource {
        frames: async_channel::Receiver<Vec<u8>>,
        drops: Arc<AtomicUsize>,
    }

    impl VideoSource for DropTrackedVideoSource {
        fn frames(&self) -> async_channel::Receiver<Vec<u8>> {
            self.frames.clone()
        }
    }

    impl Drop for DropTrackedVideoSource {
        fn drop(&mut self) {
            self.drops.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// The action child of a sent `<call>` node.
    fn call_action_of(node: &wacore_binary::Node) -> wacore_binary::Node {
        node.as_node_ref().children().unwrap()[0].to_owned()
    }

    /// The rotation rides the stanza that announces a direction, so one set
    /// before video starts is announced by the stanza that starts it rather than
    /// waiting for the app to rotate again.
    #[tokio::test]
    async fn orientation_set_before_video_rides_the_upgrade_request() {
        let (client, _sent, handle, _relay_keepalive) = sending_handle().await;
        handle
            .set_video_orientation(3)
            .await
            .expect("no direction yet, so nothing to announce");

        let (vsrc, vsink) = video_endpoints();
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        handle.start_video(vsrc, vsink).await.expect("start_video");
        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("upgrade request must be sent")
            .expect("waiter");
        let action = call_action_of(&node);
        assert_eq!(
            action
                .as_node_ref()
                .attrs()
                .optional_string("device_orientation")
                .as_deref(),
            Some("3"),
            "the stanza that starts video must carry the rotation already set"
        );
        handle.hangup().await;
    }

    /// A rotation while video is running is announced on its own, as a `state=1`
    /// that re-states the direction — which is how the peer's own rotations
    /// arrive, and a no-op there beyond the new orientation.
    #[tokio::test]
    async fn rotating_mid_call_announces_the_new_orientation() {
        let (client, _sent, handle, _relay_keepalive) = sending_handle().await;
        let (vsrc, vsink) = video_endpoints();
        handle.start_video(vsrc, vsink).await.expect("start_video");
        client.call_registry().apply_peer_video_state(
            "CID-FACADE",
            handle.generation,
            VideoState::UpgradeAccept,
        );
        // Answered: a call still ringing has no entry on the peer's side to
        // apply a `<video>` against, and deliberately sends none.
        assert!(client.call_registry().transition_if_current(
            "CID-FACADE",
            handle.generation,
            CallPhase::Connecting
        ));

        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        handle.set_video_orientation(1).await.expect("rotate");
        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("a rotation with video running must reach the peer")
            .expect("waiter");
        let action = call_action_of(&node);
        let ar = action.as_node_ref();
        assert_eq!(ar.tag, "video");
        assert_eq!(ar.attrs().optional_string("state").as_deref(), Some("1"));
        assert_eq!(
            ar.attrs().optional_string("device_orientation").as_deref(),
            Some("1")
        );
        handle.hangup().await;
    }

    /// Every mutating method on `CallHandle` refuses a handle a replacement has
    /// superseded, and a rotation is no exception: it would otherwise write the
    /// live call's orientation from a handle that no longer owns it.
    #[tokio::test]
    async fn a_stale_handle_cannot_rotate_the_replacement() {
        let client = make_client().await;
        let spawn = |_client: &Client| {
            let (_relay_tx, relay_rx) = async_channel::unbounded();
            let factory = MockFactory {
                sent: Arc::new(Mutex::new(Vec::new())),
                relay_rx: Mutex::new(Some(relay_rx)),
                connects: Arc::new(AtomicUsize::new(0)),
            };
            let (_mic_tx, mic_rx) = async_channel::unbounded::<Vec<i16>>();
            let (spk_tx, _spk_rx) = async_channel::unbounded::<Vec<i16>>();
            (factory, Arc::new(mic_rx), Arc::new(spk_tx))
        };

        let (f1, mic1, spk1) = spawn(&client);
        let stale = spawn_call(
            &client,
            mk_session(),
            engine(),
            &f1,
            pcm_audio(mic1, spk1),
            None,
        )
        .await
        .expect("first spawn_call");
        let (f2, mic2, spk2) = spawn(&client);
        let live = spawn_call(
            &client,
            mk_session(),
            engine(),
            &f2,
            pcm_audio(mic2, spk2),
            None,
        )
        .await
        .expect("replacement spawn_call");

        stale
            .set_video_orientation(3)
            .await
            .expect_err("a superseded handle must not rotate the call");
        assert_eq!(
            client
                .call_registry()
                .local_video_orientation(&stale.call_id, live.generation),
            Some(0),
            "the live call keeps the rotation it had"
        );
        live.hangup().await;
    }

    /// A camera does not un-rotate because a negotiation restarted. Six paths
    /// rebuild `VideoNegotiation` — a group downgrade among them — and a rotation
    /// stored before one of them has to still be there for the `<video>` that
    /// starts video again.
    #[tokio::test]
    async fn a_rotation_survives_a_video_negotiation_rebuild() {
        let (client, _sent, handle, _relay_keepalive) = sending_handle().await;
        let (vsrc, vsink) = video_endpoints();
        handle.start_video(vsrc, vsink).await.expect("start_video");
        handle.set_video_orientation(3).await.expect("rotate");
        handle.stop_video().await.expect("stop_video");

        assert_eq!(
            client
                .call_registry()
                .local_video_orientation("CID-FACADE", handle.generation),
            Some(3),
            "the device is still rotated, whatever the negotiation did"
        );

        let (vsrc, vsink) = video_endpoints();
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        handle.start_video(vsrc, vsink).await.expect("restart");
        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("restart must be sent")
            .expect("waiter");
        assert_eq!(
            call_action_of(&node)
                .as_node_ref()
                .attrs()
                .optional_string("device_orientation")
                .as_deref(),
            Some("3"),
            "restarting video must announce the rotation still in force"
        );
        handle.hangup().await;
    }

    /// A video-from-start offer leaves `self_state` Enabled while the peer is
    /// still ringing, so the direction check alone would send a `<video>` the
    /// peer has no call entry to apply — dropped, and never resent, because such
    /// a caller sends no further video stanza of its own. The value waits for
    /// the answer instead.
    #[tokio::test]
    async fn a_ringing_call_stores_the_rotation_without_announcing_it() {
        let (client, sent, handle, _relay_keepalive) = sending_handle().await;
        // What a video-from-start offer leaves behind: the direction is already
        // Enabled, from the offer, while the call is still ringing. The direction
        // check alone would take that for a call that can be told about a
        // rotation.
        assert!(
            client
                .call_registry()
                .set_is_video("CID-FACADE", handle.generation, true)
        );
        assert_eq!(
            client
                .call_registry()
                .video_states("CID-FACADE", handle.generation)
                .map(|(self_state, _)| self_state),
            Some(VideoState::Enabled)
        );
        assert_eq!(
            client
                .call_registry()
                .phase_if_current("CID-FACADE", handle.generation),
            Some(CallPhase::Ringing),
            "and still ringing, which is the case under test"
        );

        let before = sent.load(Ordering::SeqCst);
        handle.set_video_orientation(2).await.expect("store it");
        assert_eq!(
            sent.load(Ordering::SeqCst),
            before,
            "a ringing call has nobody to announce to"
        );
        assert_eq!(
            client
                .call_registry()
                .local_video_orientation("CID-FACADE", handle.generation),
            Some(2),
            "and the rotation is kept for the answer"
        );
        handle.hangup().await;
    }

    /// Out of range is refused rather than folded in: `4` is a caller counting
    /// something other than quarter turns, and answering it with `0` would leave
    /// the call upright and wrong. The stored value must survive the attempt.
    #[tokio::test]
    async fn an_out_of_range_orientation_is_refused_and_does_not_replace_the_stored_one() {
        let (client, _sent, handle, _relay_keepalive) = sending_handle().await;
        handle.set_video_orientation(2).await.expect("in range");

        for rejected in [4u8, 255] {
            handle
                .set_video_orientation(rejected)
                .await
                .expect_err("out of range must not be accepted");
        }
        assert_eq!(
            client
                .call_registry()
                .local_video_orientation("CID-FACADE", handle.generation),
            Some(2),
            "a rejected value must not have replaced the one in force"
        );
        handle.hangup().await;
    }

    /// Stopping leaves no direction for a rotation to describe, so the stanza
    /// omits the attribute rather than asserting `0`, which is a real rotation.
    #[tokio::test]
    async fn stopping_video_announces_no_orientation() {
        let (client, _sent, handle, _relay_keepalive) = sending_handle().await;
        let (vsrc, vsink) = video_endpoints();
        handle.start_video(vsrc, vsink).await.expect("start_video");
        handle.set_video_orientation(3).await.expect("rotate");

        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        handle.stop_video().await.expect("stop_video");
        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("stop must be sent")
            .expect("waiter");
        let action = call_action_of(&node);
        let ar = action.as_node_ref();
        assert_eq!(ar.attrs().optional_string("state").as_deref(), Some("6"));
        assert_eq!(
            ar.attrs().optional_string("device_orientation"),
            None,
            "a stopped direction has no rotation to announce"
        );
        handle.hangup().await;
    }

    #[tokio::test]
    async fn start_video_sends_upgrade_request_with_marker() {
        let (client, _sent, handle, _relay_keepalive) = sending_handle().await;
        let (vsrc, vsink) = video_endpoints();
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        handle.start_video(vsrc, vsink).await.expect("start_video");
        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("upgrade request must be sent")
            .expect("waiter");
        let r = node.as_node_ref();
        assert!(
            r.attrs()
                .optional_string("id")
                .is_some_and(|id| !id.is_empty()),
            "the <call> wrapper needs an id so the peer's typed video ack correlates"
        );
        let action = call_action_of(&node);
        let ar = action.as_node_ref();
        assert_eq!(ar.tag, "video");
        assert_eq!(ar.attrs().optional_string("state").as_deref(), Some("11"));
        assert_eq!(ar.attrs().optional_string("dec").as_deref(), Some("H264"));
        assert_eq!(
            ar.attrs().optional_string("device_orientation").as_deref(),
            Some("0")
        );
        assert_eq!(
            ar.attrs().optional_string("voip_settings").as_deref(),
            Some("video"),
            "the upgrade request must carry the marker attr"
        );
        assert!(
            client
                .call_registry()
                .snapshot("CID-FACADE")
                .expect("session")
                .is_video,
            "start_video must mark the session as video"
        );
        handle.hangup().await;
    }

    #[tokio::test]
    async fn start_video_rejects_zero_timestamp_stride() {
        let (_client, _sent, handle, _relay_keepalive) = sending_handle().await;
        let (frames, sink) = video_endpoints();
        let source = TimedVideoSource {
            frames,
            timestamp_stride: 0,
        };

        assert!(matches!(
            handle.start_video(source, sink).await,
            Err(CallError::Media(
                "video RTP timestamp stride must be non-zero"
            ))
        ));
        assert!(
            handle.video.sink_slot.lock().unwrap().is_none(),
            "invalid timing must not attach endpoints"
        );
        handle.hangup().await;
    }

    #[tokio::test]
    async fn start_video_rejects_audio_group_before_attaching() {
        let (client, sent_count, handle, _relay_keepalive) = sending_handle().await;
        let update = GroupCallUpdate::builder()
            .call_id("CID-FACADE".to_string())
            .call_creator(caller())
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        assert_eq!(
            client.call_registry().apply_group_update(update),
            wacore::voip::GroupStateApply::Applied
        );
        let sent_before = sent_count.load(Ordering::SeqCst);
        let (source, sink) = video_endpoints();

        assert!(matches!(
            handle.start_video(source, sink).await,
            Err(CallError::Media(
                "group media mode does not allow a video upgrade"
            ))
        ));
        assert_eq!(
            sent_count.load(Ordering::SeqCst),
            sent_before,
            "an ineligible group call must not send video signaling"
        );
        assert!(
            handle.video.sink_slot.lock().unwrap().is_none(),
            "an ineligible group call must not attach video endpoints"
        );
        handle.hangup().await;
    }

    #[tokio::test]
    async fn group_downgrade_serializes_with_video_setup() {
        let (client, sent_count, handle, _relay_keepalive) = sending_handle().await;
        let update = GroupCallUpdate::builder()
            .call_id("CID-FACADE".to_string())
            .call_creator(caller())
            .transaction_id(1)
            .media("video".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(Vec::new())
            .build();
        assert_eq!(
            client.call_registry().apply_group_update(update.clone()),
            wacore::voip::GroupStateApply::Applied
        );
        let group_transition_lock = client
            .call_registry()
            .group_transition_lock("CID-FACADE", handle.generation)
            .expect("current group transition");
        let group_transition_guard = group_transition_lock.lock().await;
        let sent_before = sent_count.load(Ordering::SeqCst);
        let video_handle = handle.clone();
        let (source, sink) = video_endpoints();
        let upgrade = tokio::spawn(async move { video_handle.start_video(source, sink).await });
        for _ in 0..3 {
            tokio::task::yield_now().await;
        }
        assert!(
            handle.video.sink_slot.lock().unwrap().is_none(),
            "video setup must wait behind an authoritative group transition"
        );
        assert_eq!(sent_count.load(Ordering::SeqCst), sent_before);

        let mut downgrade = update;
        downgrade.transaction_id = 2;
        downgrade.media = "audio".to_string();
        assert_eq!(
            client.call_registry().apply_group_update(downgrade),
            wacore::voip::GroupStateApply::Applied
        );
        drop(group_transition_guard);

        assert!(matches!(
            upgrade.await.expect("video task"),
            Err(CallError::Media(
                "group media mode does not allow a video upgrade"
            ))
        ));
        assert_eq!(
            sent_count.load(Ordering::SeqCst),
            sent_before,
            "the overtaken upgrade must not send video signaling"
        );
        assert!(
            handle.video.sink_slot.lock().unwrap().is_none(),
            "the overtaken upgrade must not attach video endpoints"
        );
        handle.hangup().await;
    }

    #[test]
    fn audio_call_links_are_not_video_upgradable() {
        let mut group = wacore::voip::GroupCallState::new("CALL-LINK", caller());
        assert_eq!(
            group.apply_waiting_room(
                wacore::types::group_call::WaitingRoom::builder()
                    .call_id("CALL-LINK".to_string())
                    .call_creator(caller())
                    .link_token("TEST-CALL-LINK".to_string())
                    .media(CallLinkMedia::Audio)
                    .enabled(true)
                    .is_admin(false)
                    .transaction_id(1)
                    .users(Vec::new())
                    .build(),
            ),
            wacore::voip::GroupStateApply::Applied
        );
        assert!(!group_video_upgrade_allowed(&group));
    }

    // A signaling send failure during an upgrade must roll the local video setup back: endpoints
    // detached and the session flag cleared, so the source isn't left encoding into a call the peer
    // never transitioned.
    #[tokio::test]
    async fn start_video_send_failure_rolls_back_local_setup() {
        let client = make_failing_send_client().await; // send_node errors (no noise socket)
        let registry = client.call_registry();
        let generation = registry.insert(mk_session());
        let video_shared = Arc::new(VideoShared::new());
        let (ev_tx, ev_rx) = async_channel::unbounded::<CallEvent>();
        registry.set_video_channels(
            "CID-FACADE",
            generation,
            ev_tx,
            video_shared.ctl_tx.clone(),
            video_teardown_hook(&video_shared),
        );
        let handle = CallHandle {
            call_id: "CID-FACADE".into(),
            generation,
            peer_jid: caller(),
            call_creator: caller(),
            client_registry: registry.clone(),
            pending_outgoing_calls: client.voip_state().pending_outgoing_calls.clone(),
            client: Arc::downgrade(&client),
            muted: Arc::new(AtomicBool::new(false)),
            video: video_shared.clone(),
            events: ev_rx,
            ended: Arc::new(EndedFlag::default()),
        };

        let (vsrc, vsink) = video_endpoints();
        let res = handle.start_video(vsrc, vsink).await;
        assert!(res.is_err(), "a failed upgrade send must surface an error");
        assert!(
            video_shared.sink_slot.lock().unwrap().is_none(),
            "the video endpoints must be detached after a failed upgrade"
        );
        assert!(
            !registry.snapshot("CID-FACADE").expect("session").is_video,
            "a failed upgrade must clear the video flag"
        );
    }

    #[tokio::test]
    async fn accept_video_sends_accept_then_enabled() {
        let (client, sent_count, handle, _relay_keepalive) = sending_handle().await;
        let request = peer_upgrade_request(&handle).await;
        let (vsrc, vsink) = video_endpoints();
        let before = sent_count.load(Ordering::SeqCst);
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        handle
            .accept_video(request, vsrc, vsink)
            .await
            .expect("accept_video");
        assert_eq!(
            sent_count.load(Ordering::SeqCst) - before,
            2,
            "accept_video sends UpgradeAccept then Enabled"
        );
        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("accept must be sent")
            .expect("waiter");
        let action = call_action_of(&node);
        let ar = action.as_node_ref();
        assert_eq!(ar.attrs().optional_string("state").as_deref(), Some("4"));
        assert_eq!(
            ar.attrs().optional_string("dec").as_deref(),
            Some("H264,AV1")
        );
        assert_eq!(
            ar.attrs().optional_string("device_orientation").as_deref(),
            Some("0")
        );
        assert_eq!(
            ar.attrs().optional_string("voip_settings"),
            None,
            "an accept must not carry the upgrade marker"
        );
        handle.hangup().await;
    }

    #[tokio::test]
    async fn accept_video_enabled_failure_rolls_back_local_plane() {
        let (client, sent) = make_sending_client_with_failure_after(Some(1)).await;
        let registry = client.call_registry();
        let generation = registry.insert(mk_session());
        let video = Arc::new(VideoShared::new());
        let (event_tx, events) = async_channel::unbounded::<CallEvent>();
        registry.set_video_channels(
            "CID-FACADE",
            generation,
            event_tx,
            video.ctl_tx.clone(),
            video_teardown_hook(&video),
        );
        let handle = CallHandle {
            call_id: "CID-FACADE".into(),
            generation,
            peer_jid: caller(),
            call_creator: caller(),
            client_registry: registry.clone(),
            pending_outgoing_calls: client.voip_state().pending_outgoing_calls.clone(),
            client: Arc::downgrade(&client),
            muted: Arc::new(AtomicBool::new(false)),
            video: video.clone(),
            events,
            ended: Arc::new(EndedFlag::default()),
        };

        let request = peer_upgrade_request(&handle).await;
        let (source, sink) = video_endpoints();
        assert!(handle.accept_video(request, source, sink).await.is_err());
        assert_eq!(sent.load(Ordering::SeqCst), 2);
        assert!(video.sink_slot.lock().unwrap().is_none());
        assert!(!registry.snapshot("CID-FACADE").unwrap().is_video);
    }

    #[tokio::test]
    async fn stale_peer_request_cannot_accept_a_newer_request() {
        let (_client, _sent, handle, _relay_keepalive) = sending_handle().await;
        let first = peer_upgrade_request(&handle).await;
        let transition_lock = handle
            .client_registry
            .video_transition_lock(&handle.call_id, handle.generation)
            .expect("active call");
        let second = {
            let _guard = transition_lock.lock().await;
            assert!(matches!(
                handle.client_registry.apply_peer_video_state(
                    &handle.call_id,
                    handle.generation,
                    VideoState::UpgradeCancel,
                ),
                wacore::voip::PeerVideoTransition::Applied { .. }
            ));
            match handle.client_registry.apply_peer_video_state(
                &handle.call_id,
                handle.generation,
                VideoState::UpgradeRequestV2,
            ) {
                wacore::voip::PeerVideoTransition::UpgradeRequested(token) => token,
                transition => panic!("unexpected transition: {transition:?}"),
            }
        };
        assert_ne!(first, second);

        let drops = Arc::new(AtomicUsize::new(0));
        let (frames_tx, frames) = async_channel::unbounded();
        let source = DropTrackedVideoSource {
            frames,
            drops: drops.clone(),
        };
        let (_sink_rx, sink) = {
            let (sink, receiver) = async_channel::unbounded::<VideoFrame>();
            (receiver, sink)
        };
        assert!(matches!(
            handle.accept_video(first, source, sink).await,
            Err(CallError::VideoUpgradeExpired)
        ));
        drop(frames_tx);
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert!(handle.video.sink_slot.lock().unwrap().is_none());
        assert!(
            handle
                .client_registry
                .peer_video_request_is_current(&handle.call_id, second)
        );
        handle.hangup().await;
    }

    #[tokio::test(start_paused = true)]
    async fn unanswered_local_upgrade_times_out_and_releases_source() {
        let (client, sends) = make_sending_client().await;
        let registry = client.call_registry();
        let generation = registry.insert(mk_session());
        let video = Arc::new(VideoShared::new());
        let (event_tx, events) = async_channel::unbounded::<CallEvent>();
        registry.set_video_channels(
            "CID-FACADE",
            generation,
            event_tx,
            video.ctl_tx.clone(),
            video_teardown_hook(&video),
        );
        let handle = CallHandle {
            call_id: "CID-FACADE".into(),
            generation,
            peer_jid: caller(),
            call_creator: caller(),
            client_registry: registry.clone(),
            pending_outgoing_calls: client.voip_state().pending_outgoing_calls.clone(),
            client: Arc::downgrade(&client),
            muted: Arc::new(AtomicBool::new(false)),
            video: video.clone(),
            events,
            ended: Arc::new(EndedFlag::default()),
        };

        let drops = Arc::new(AtomicUsize::new(0));
        let (_frames_tx, frames) = async_channel::unbounded();
        let source = DropTrackedVideoSource {
            frames,
            drops: drops.clone(),
        };
        let (sink, _sink_rx) = async_channel::unbounded::<VideoFrame>();
        handle
            .start_video(source, sink)
            .await
            .expect("request sent");
        assert_eq!(sends.load(Ordering::SeqCst), 1);

        let timeout_waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        tokio::task::yield_now().await;
        tokio::time::advance(VIDEO_UPGRADE_TIMEOUT).await;
        for _ in 0..10 {
            if drops.load(Ordering::SeqCst) == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
        assert_eq!(drops.load(Ordering::SeqCst), 1);
        assert_eq!(sends.load(Ordering::SeqCst), 2);
        let timeout_node = timeout_waiter.await.expect("timeout stanza");
        let timeout_action = call_action_of(&timeout_node);
        assert_eq!(
            timeout_action
                .as_node_ref()
                .attrs()
                .optional_string("state")
                .as_deref(),
            Some("9")
        );
        assert!(!registry.snapshot("CID-FACADE").unwrap().is_video);
        assert_eq!(
            registry.video_states("CID-FACADE", generation),
            Some((VideoState::Disabled, VideoState::Disabled))
        );
        handle.hangup().await;
    }

    #[tokio::test]
    async fn stop_video_sends_stopped_and_releases_endpoints() {
        let (client, _sent, handle, _relay_keepalive) = sending_handle().await;
        let (vsrc, vsink) = video_endpoints();
        handle.start_video(vsrc, vsink).await.expect("start_video");
        assert!(
            handle.video.sink_slot.lock().unwrap().is_some(),
            "start_video attaches the sink"
        );

        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        handle.stop_video().await.expect("stop_video");
        let node = tokio::time::timeout(Duration::from_secs(2), waiter)
            .await
            .expect("downgrade must be sent")
            .expect("waiter");
        let action = call_action_of(&node);
        let ar = action.as_node_ref();
        assert_eq!(ar.tag, "video");
        assert_eq!(ar.attrs().optional_string("state").as_deref(), Some("6"));
        assert_eq!(
            ar.attrs().optional_string("voip_settings"),
            None,
            "a downgrade must NOT carry the marker (it re-arms the peer's video)"
        );
        assert!(
            handle.video.sink_slot.lock().unwrap().is_none(),
            "stop_video releases the sink"
        );
        assert!(
            !client
                .call_registry()
                .snapshot("CID-FACADE")
                .expect("session")
                .is_video,
            "stop_video must clear the session's video flag"
        );

        let (vsrc, vsink) = video_endpoints();
        handle
            .start_video(vsrc, vsink)
            .await
            .expect("restart_video");
        assert!(handle.video.sink_slot.lock().unwrap().is_some());
        handle.hangup().await;
        assert!(
            handle.video.sink_slot.lock().unwrap().is_none(),
            "a restarted video plane must rearm terminal teardown"
        );
    }

    #[tokio::test]
    async fn dormant_stop_video_clears_pending_endpoints() {
        let (client, _sent) = make_sending_client().await;
        let (source, sink) = video_endpoints();
        let (handle, call_id) =
            place_dormant_outgoing_with_video(&client, Some(VideoEndpoints::new(source, sink)))
                .await;
        assert!(
            client
                .voip_state()
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .get(&call_id)
                .is_some_and(|pending| pending.video.is_some())
        );

        handle.stop_video().await.expect("dormant downgrade");
        assert!(
            client
                .voip_state()
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .get(&call_id)
                .is_some_and(|pending| pending.video.is_none()),
            "relay attach must not resurrect endpoints removed before its ack"
        );
        let registry = client.call_registry();
        assert_eq!(
            registry.video_states(&call_id, handle.generation),
            Some((VideoState::Stopped, VideoState::Enabled))
        );
        assert!(registry.snapshot(&call_id).unwrap().is_video);
        handle.hangup().await;
    }

    // The VideoFeed pumps the consumer's source into the drive loop's channel and dies on the
    // call's ended flag (not only on channel closure), so a silent source can't park it forever.
    #[tokio::test]
    async fn video_feed_forwards_and_stops_on_ended() {
        let client = make_client().await;
        let shared = VideoShared::new();
        let (in_rx, ctl_rx) = shared.take_receivers();
        let (src_tx, src_rx) = async_channel::unbounded::<Vec<u8>>();
        let sink: Arc<dyn VideoSink> = {
            let (vout_tx, _vout_rx) = async_channel::unbounded::<VideoFrame>();
            std::mem::forget(_vout_rx);
            Arc::new(vout_tx)
        };
        let source: Arc<dyn VideoSource> = Arc::new(TimedVideoSource {
            frames: src_rx,
            timestamp_stride: 4500,
        });
        let ended = Arc::new(EndedFlag::default());
        shared.attach_endpoints(&client, &source, &sink, ended.clone());

        assert_eq!(
            ctl_rx.recv().await.unwrap(),
            VideoControl::SetTimestampStride(4500)
        );

        src_tx.send(vec![1, 2, 3]).await.expect("feed source");
        let got = tokio::time::timeout(Duration::from_secs(2), in_rx.recv())
            .await
            .expect("AU must be forwarded")
            .expect("channel open");
        assert_eq!(got, vec![1, 2, 3]);

        // Ending the call stops the feed even though the source channel stays open: a later AU
        // must NOT be forwarded. (The channel itself stays open — VideoShared retains a sender —
        // so absence of traffic, not closure, is the observable.)
        ended.notify();
        tokio::task::yield_now().await;
        src_tx.send(vec![9, 9, 9]).await.expect("source still open");
        let after = tokio::time::timeout(Duration::from_millis(300), in_rx.recv()).await;
        assert!(
            after.is_err(),
            "an AU sent after ended must not be forwarded (feed stopped)"
        );
    }

    /// Runs a caller-supplied hook inline on `abort()`, standing in for a `Runtime` that
    /// finishes cancellation synchronously and therefore drops the task on the spot.
    #[derive(Clone, Default)]
    struct AbortHook(Arc<std::sync::OnceLock<Box<dyn Fn() + Send + Sync>>>);

    struct ReentrantAbortRuntime {
        handle: tokio::runtime::Handle,
        hook: AbortHook,
    }

    impl wacore::runtime::Runtime for ReentrantAbortRuntime {
        fn spawn(
            &self,
            future: std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ) -> wacore::runtime::AbortHandle {
            let task = self.handle.spawn(future);
            let hook = self.hook.clone();
            wacore::runtime::AbortHandle::new(move || {
                task.abort();
                if let Some(reenter) = hook.0.get() {
                    reenter();
                }
            })
        }

        fn spawn_detached(
            &self,
            future: std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ) {
            self.handle.spawn(future);
        }

        fn sleep(&self, duration: Duration) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(tokio::time::sleep(duration))
        }

        fn spawn_blocking(
            &self,
            f: Box<dyn FnOnce() + Send + 'static>,
        ) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(async {
                let _ = tokio::task::spawn_blocking(f).await;
            })
        }

        fn yield_now(&self) -> Option<std::pin::Pin<Box<dyn Future<Output = ()> + Send>>> {
            None
        }
    }

    /// The old feed must be aborted with the `feed` mutex already released: cancellation runs
    /// runtime-supplied code, and on a runtime that cancels synchronously that code runs the
    /// task's drop inline, re-entering this non-reentrant mutex.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn feed_swap_does_not_abort_while_holding_the_feed_lock() {
        let hook = AbortHook::default();
        let persistence_manager = Arc::new(
            PersistenceManager::new(create_test_backend().await)
                .await
                .expect("persistence manager"),
        );
        let client = Client::builder()
            .with_runtime_arc(Arc::new(ReentrantAbortRuntime {
                handle: tokio::runtime::Handle::current(),
                hook: hook.clone(),
            }))
            .with_persistence_manager(persistence_manager)
            .with_transport_factory(crate::transport::mock::MockTransportFactory::new())
            .with_http_client(MockHttpClient)
            .build()
            .await
            .expect("client build")
            .into_client();

        let shared = Arc::new(VideoShared::new());
        let _receivers = shared.take_receivers();
        hook.0
            .set({
                let shared = Arc::clone(&shared);
                Box::new(move || {
                    let _guard = shared.feed.lock().unwrap_or_else(|e| e.into_inner());
                })
            })
            .ok()
            .expect("the hook is installed once");

        let (src_rx, vout_tx) = video_endpoints();
        let source: Arc<dyn VideoSource> = Arc::new(src_rx);
        let sink: Arc<dyn VideoSink> = Arc::new(vout_tx);
        let ended = Arc::new(EndedFlag::default());

        // A re-entrant lock parks its thread for good, so drive the sequence on a thread of its
        // own and let the timeout report the deadlock instead of hanging the suite.
        let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
        std::thread::spawn(move || {
            shared.attach_endpoints(&client, &source, &sink, ended.clone());
            // Replacing an existing feed aborts it...
            shared.attach_endpoints(&client, &source, &sink, ended.clone());
            // ...and so does releasing the endpoints.
            shared.detach_endpoints();
            let _ = done_tx.send(());
        });

        done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("attach/detach must not hold the feed lock while aborting the old feed");
    }

    // A second take is defensive: it must yield closed channels (their driver arms self-disable),
    // never panic or hand out a duplicate of the live receivers.
    #[test]
    fn video_shared_second_take_yields_closed_channels() {
        let shared = VideoShared::new();
        let _live = shared.take_receivers();
        let (in_rx, ctl_rx) = shared.take_receivers();
        assert!(in_rx.is_closed());
        assert!(ctl_rx.is_closed());
    }

    #[test]
    fn video_control_queue_preserves_state_and_coalesces_orientation() {
        let shared = VideoShared::new();
        let (_in_rx, ctl_rx) = shared.take_receivers();
        shared.send_control(VideoControl::Disable);
        for orientation in 0..100u8 {
            shared.send_control(VideoControl::SetOrientation(orientation % 4));
        }
        shared.send_control(VideoControl::Enable);

        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::Disable));
        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::Enable));
        assert_eq!(ctl_rx.try_recv(), Ok(VideoControl::SetOrientation(3)));
        assert_eq!(ctl_rx.try_recv(), Err(async_channel::TryRecvError::Empty));
    }
}
