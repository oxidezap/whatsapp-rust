//! The incoming-call MEDIA facade: `client.voip().accept(&incoming).audio(src, sink).start()` ->
//! [`CallHandle`]. It internalizes the offer-decrypt -> relay-connect -> engine-spawn orchestration
//! the example drove by hand, so a consumer never touches the relay socket, the Signal session, or
//! the sans-IO engine directly. Behind the `voip` feature; signaling (reject/terminate) stays
//! feature-free in `super`.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use log::warn;
use wacore::message_processing::EncType;
use wacore::messages::MessageUtils;
use wacore::stanza::call::{
    CAPABILITY_OFFER, CAPABILITY_VIDEO_OFFER, OfferDeviceKey, OfferParams, VideoStateParams,
    build_offer, build_video_state,
};
use wacore::types::call::{CallAction, IncomingCall, VideoState};
use wacore::voip::relay_parse::RelayData;
use wacore::voip::transport::RelayTransportFactory;
use wacore::voip::{
    CallChannels, CallConfig, CallEngine, CallEvent, VideoControl, VideoControlReceiver,
    VideoControlSender, VideoFrame, video_control_channel,
};
use wacore_binary::{Jid, JidExt as _, Server};
use waproto::whatsapp as wa;

use crate::client::{CallError, Client};
use crate::voip::audio::{AudioSink, AudioSource, WA_FRAME_SAMPLES};
use crate::voip::driver::{RandTxIds, run_call_tokio};
use crate::voip::transport::RelayMediaChannelFactory;
use crate::voip::video::{VideoSink, VideoSource};

/// Builder returned by [`Voip::accept`](super::super::client::voip::Voip::accept). Holds the offer
/// and, once [`audio`](Self::audio) is called, the source/sink, then [`start`](Self::start) drives
/// the call. Borrows the client so it can't outlive it.
pub struct AcceptCall<'a> {
    pub(crate) client: &'a Client,
    pub(crate) incoming: &'a IncomingCall,
    source: Option<Arc<dyn AudioSource>>,
    sink: Option<Arc<dyn AudioSink>>,
    video: Option<VideoEndpoints>,
}

impl<'a> AcceptCall<'a> {
    pub(crate) fn new(client: &'a Client, incoming: &'a IncomingCall) -> Self {
        Self {
            client,
            incoming,
            source: None,
            sink: None,
            video: None,
        }
    }

    /// Provide the microphone source and speaker sink for the call. A bare
    /// `async_channel::Receiver<Vec<i16>>` / `Sender<Vec<i16>>` works directly (blanket impls).
    pub fn audio<S, K>(mut self, source: S, sink: K) -> Self
    where
        S: AudioSource,
        K: AudioSink,
    {
        self.source = Some(Arc::new(source));
        self.sink = Some(Arc::new(sink));
        self
    }

    /// Answer with VIDEO media as well: `source` supplies our encoded H.264 AUs, `sink` receives
    /// the peer's. Use for a video-from-the-start offer (`CallAction::Offer { is_video: true }`);
    /// an audio-only call can upgrade later via [`CallHandle::start_video`].
    pub fn video<S, K>(mut self, source: S, sink: K) -> Self
    where
        S: VideoSource,
        K: VideoSink,
    {
        self.video = Some(VideoEndpoints::new(source, sink));
        self
    }

    /// Decrypt the callKey, connect the relay, spawn the call driver, and register it. The returned
    /// [`CallHandle`] controls the live call. Live-only past the relay connect (DTLS/SCTP need a real
    /// relay); everything up to the connect is offline testable.
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
        let source = self.source.take().ok_or(CallError::MissingAudio)?;
        let sink = self.sink.take().ok_or(CallError::MissingAudio)?;
        let video = self.video.take();
        if video.as_ref().is_some_and(|v| !v.has_valid_timing()) {
            return Err(CallError::Media(
                "video RTP timestamp stride must be non-zero",
            ));
        }
        // Answering consumes the ringing flag now, BEFORE the media-setup awaits (callKey decrypt /
        // relay connect): a peer <terminate> racing this window must not record a missed call for a
        // call we are answering. A failed start() leaves it cleared -- an attempted answer reads as
        // ended, not an ignored missed ring.
        self.client
            .call_registry()
            .take_ringing(self.incoming.action.call_id());
        let (engine, call_id, addr) = self.build_engine(video.is_some()).await?;
        // The decrypt above may await on the network (prekey fetch). If the connection dropped
        // meanwhile, cleanup_connection_state already ran with no registry entry to abort, so bail
        // rather than register + connect a relay that would outlive the connection.
        if !self.client.is_connected() {
            return Err(CallError::Connect(ERR_DISCONNECTED_DURING_SETUP.into()));
        }
        let factory = RelayMediaChannelFactory::new(addr, self.client.runtime.clone());
        let mut session = wacore::voip::CallSession::new_incoming(
            &call_id,
            self.incoming.from.clone(),
            self.incoming.action.call_creator().clone(),
        );
        session.is_video = video.is_some();
        spawn_call(
            self.client,
            call_id,
            session,
            engine,
            &factory,
            source,
            sink,
            video,
        )
        .await
    }

    /// Build the [`CallEngine`] from the offer: decrypt the callKey over the Signal session, then
    /// assemble the incoming-call config from the parsed relay. No network I/O beyond the Signal
    /// session the decrypt needs.
    async fn build_engine(
        &self,
        enable_video: bool,
    ) -> Result<(CallEngine, String, SocketAddr), CallError> {
        let media = self.incoming.media.as_ref().ok_or(CallError::NotAnOffer)?;
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
        let own_lid = self
            .client
            .get_lid()
            .ok_or(CallError::Media("no own LID"))?;
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
        config.enable_video = enable_video;
        // Read the dial addr off the config before CallEngine::new consumes it (no second relay walk).
        let addr = socket_addr_from_config(&config)?;
        let engine = CallEngine::new(config, Box::new(RandTxIds))
            .map_err(|e| CallError::Setup(e.to_string()))?;
        Ok((engine, call_id.clone(), addr))
    }
}

/// Builder returned by [`Voip::call`](super::super::client::voip::Voip::call). Mirrors [`AcceptCall`]:
/// holds the peer and, once [`audio`](Self::audio) is called, the source/sink, then [`start`](Self::start)
/// generates the callKey, encrypts it per peer device, sends the `<offer>`, and registers the call.
/// Borrows the client so it can't outlive it.
pub struct OutgoingCall<'a> {
    pub(crate) client: &'a Client,
    pub(crate) peer: &'a Jid,
    source: Option<Arc<dyn AudioSource>>,
    sink: Option<Arc<dyn AudioSink>>,
    video: Option<VideoEndpoints>,
}

impl<'a> OutgoingCall<'a> {
    pub(crate) fn new(client: &'a Client, peer: &'a Jid) -> Self {
        Self {
            client,
            peer,
            source: None,
            sink: None,
            video: None,
        }
    }

    /// Provide the microphone source and speaker sink for the call. A bare
    /// `async_channel::Receiver<Vec<i16>>` / `Sender<Vec<i16>>` works directly (blanket impls).
    pub fn audio<S, K>(mut self, source: S, sink: K) -> Self
    where
        S: AudioSource,
        K: AudioSink,
    {
        self.source = Some(Arc::new(source));
        self.sink = Some(Arc::new(sink));
        self
    }

    /// Place a VIDEO call: the offer advertises `<video>` and the media plane carries H.264 both
    /// ways once the call connects. An audio-only call can upgrade later via
    /// [`CallHandle::start_video`].
    pub fn video<S, K>(mut self, source: S, sink: K) -> Self
    where
        S: VideoSource,
        K: VideoSink,
    {
        self.video = Some(VideoEndpoints::new(source, sink));
        self
    }

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
    /// [`attach_outgoing_relay`]. Everything here (device resolution, encrypt, offer build + send) is
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
        let source = self.source.take().ok_or(CallError::MissingAudio)?;
        let sink = self.sink.take().ok_or(CallError::MissingAudio)?;
        let video = self.video.take();
        if video.as_ref().is_some_and(|v| !v.has_valid_timing()) {
            return Err(CallError::Media(
                "video RTP timestamp stride must be non-zero",
            ));
        }

        // WA Web `_e()`: call-id is "00" + 15 random bytes as lowercase hex (32 hex chars total).
        let call_id = gen_call_id();

        // Our own LID is the send-side SRTP participant id; required for E2E key derivation.
        let own_lid = self
            .client
            .get_lid()
            .ok_or(CallError::Media("no own LID"))?;

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
            let session_mutexes = self.client.session_mutexes_for(&lock_jids).await;
            let mut session_guards = Vec::with_capacity(session_mutexes.len());
            for mutex in &session_mutexes {
                session_guards.push(mutex.lock().await);
            }

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
            source,
            sink,
            video,
        )
        .await
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

fn offer_capability(video: bool) -> &'static [u8] {
    if video {
        &CAPABILITY_VIDEO_OFFER
    } else {
        &CAPABILITY_OFFER
    }
}

/// Drain every dormant outgoing call (relay never arrived) and notify each one's `ended` so any
/// parked `wait_ended()` resolves. The relay socket and signaling are connection-scoped, so a dormant
/// outgoing call can't survive a disconnect; the registry's `abort_all` already covers attached calls,
/// but those have no media-task drop-guard yet, so this is their only end-notify path. Called from the
/// client's connection teardown.
pub(crate) fn drain_pending_outgoing_on_disconnect(client: &Client) {
    let drained: Vec<PendingOutgoing> = {
        let mut map = client
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
    source: Arc<dyn AudioSource>,
    sink: Arc<dyn AudioSink>,
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
        let session_mutexes = client.session_mutexes_for(&lock_jids).await;
        let mut _session_guards = Vec::with_capacity(session_mutexes.len());
        for mutex in &session_mutexes {
            _session_guards.push(mutex.lock().await);
        }

        // Sessions were asserted upstream (`OutgoingCall::start`), so skip the network session-ensure and
        // encrypt against the existing sessions directly: a device whose session is somehow still
        // missing fails its encrypt and is skipped, exactly as the old per-device loop did.
        let plan = wacore::send::SessionPlan::assume_ready(devices.len());
        let mut adapter = client.signal_adapter().await;
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
    let offer = build_offer(&OfferParams {
        call_id: &call_id,
        to: peer,
        call_creator,
        device_keys: &device_keys,
        privacy_token: privacy_token.as_deref(),
        capability: Some(offer_capability(video.is_some())),
        device_identity: device_identity.as_deref(),
        id: Some(&offer_stanza_id),
        // Keep the addressed `<destination>` shape for a multi-device callee even if encryption
        // failures left a single surviving key, so that key stays tied to its device.
        multi_device,
        video: video.is_some(),
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
    let generation = registry.insert(session);

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
                source,
                sink,
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
        let removed = take_pending_if_current(&client.pending_outgoing_calls, &call_id, generation);
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
        pending_outgoing_calls: client.pending_outgoing_calls.clone(),
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
const OFFER_ACK_RELAY_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

/// Pre-encode mic-frame backlog before the channel back-pressures the source (8 × 20 ms = 160 ms).
const MIC_CHANNEL_CAPACITY: usize = 8;

/// Bound on the consumer-facing `CallEvent` queue. The driver posts with `try_send`, so once a slow
/// or absent consumer lets it fill, further events drop instead of growing without bound: an
/// authenticated peer streaming `ForeignAudio` frames can't drive an OOM. Lifecycle events
/// (RelayAllocated/Failed/TimedOut) are emitted before any media flows, so they are never dropped,
/// and call teardown is driven by the `ended` flag, not this channel.
const CALL_EVENT_CHANNEL_CAPACITY: usize = 64;

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
    let pending = take_pending_if_current(&client.pending_outgoing_calls, call_id, generation);
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
    source: Arc<dyn AudioSource>,
    sink: Arc<dyn AudioSink>,
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
        engine,
        &factory,
        pending.source,
        pending.sink,
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

/// Spawn the call driver over `factory` and register it. Generic over the relay factory so a test
/// can inject an in-memory transport instead of the real DTLS/SCTP dialer. The session is supplied so
/// both incoming (`new_incoming`) and outgoing (`new_outgoing`) callers register the right direction.
#[allow(clippy::too_many_arguments)]
async fn spawn_call(
    client: &Client,
    call_id: String,
    session: wacore::voip::CallSession,
    engine: CallEngine,
    factory: &dyn RelayTransportFactory,
    source: Arc<dyn AudioSource>,
    sink: Arc<dyn AudioSink>,
    video: Option<VideoEndpoints>,
) -> Result<CallHandle, CallError> {
    // Register BEFORE connecting so the entry exists before the driver task can self-clean.
    let registry = client.call_registry();
    let peer_jid = session.peer_jid.clone();
    let call_creator = session.call_creator.clone();
    let generation = registry.insert(session);

    let muted = Arc::new(AtomicBool::new(false));
    let ended = Arc::new(EndedFlag::default());
    // Wake wait_ended() whenever this registry entry is removed -- including a terminal stanza or a
    // disconnect that lands while attach_engine is still dialing (no media task yet).
    registry.set_ended_notify(&call_id, generation, {
        let ended = ended.clone();
        move || ended.notify()
    });
    let (ev_tx, ev_rx) = async_channel::bounded::<CallEvent>(CALL_EVENT_CHANNEL_CAPACITY);
    let video_shared = Arc::new(VideoShared::new());
    registry.set_video_channels(
        &call_id,
        generation,
        ev_tx.clone(),
        video_shared.ctl_tx.clone(),
        video_teardown_hook(&video_shared),
    );
    attach_engine(
        client,
        &call_id,
        generation,
        engine,
        factory,
        source,
        sink,
        video,
        video_shared.clone(),
        muted.clone(),
        ended.clone(),
        ev_tx,
        // Incoming (callee): no recv-rekey — the callee already keys recv on its own self LID.
        None,
    )
    .await?;
    Ok(CallHandle {
        call_id,
        generation,
        peer_jid,
        call_creator,
        client_registry: client.call_registry(),
        pending_outgoing_calls: client.pending_outgoing_calls.clone(),
        client: client_weak(client),
        muted,
        video: video_shared,
        events: ev_rx,
        ended,
    })
}

/// Connect the relay and spawn the driver task against pre-built shared handle state (mute flag,
/// ended flag, event sender). Shared so the outgoing relay-arrival path can drive the same
/// already-handed-out [`CallHandle`]. The registry entry under `generation` must already exist.
#[allow(clippy::too_many_arguments)]
async fn attach_engine(
    client: &Client,
    call_id: &str,
    generation: u64,
    engine: CallEngine,
    factory: &dyn RelayTransportFactory,
    source: Arc<dyn AudioSource>,
    sink: Arc<dyn AudioSink>,
    video: Option<VideoEndpoints>,
    video_shared: Arc<VideoShared>,
    muted: Arc<AtomicBool>,
    ended: Arc<EndedFlag>,
    ev_tx: async_channel::Sender<CallEvent>,
    // Caller-only recv-rekey receiver; `None` for an incoming call (the callee keys recv on its own
    // self LID and never rekeys).
    rekey_rx: Option<async_channel::Receiver<String>>,
) -> Result<(), CallError> {
    // The registry entry already exists. Re-check is_connected NOW (after insert, before connect) so a
    // disconnect that clears is_connected before abort_all can't slip through the gap between an
    // earlier guard's load and the insert: either we inserted before abort_all (it catches us) or this
    // sees !is_connected and self-cleans. Reap our generation and wake wait_ended() before bailing, so
    // the just-registered entry can't leak and a parked wait_ended() resolves.
    if !client.is_connected() {
        client
            .call_registry()
            .remove_if_current(call_id, generation);
        ended.notify();
        return Err(CallError::Connect(ERR_DISCONNECTED_DURING_SETUP.into()));
    }

    // Connect failure leaves the call already visible (registry entry inserted before connect; for an
    // outgoing call the PendingOutgoing was already removed by attach_outgoing_relay). Reap our own
    // generation and wake any wait_ended() waiter before propagating, else an incoming call leaks in
    // the registry and an outgoing handle's wait_ended() hangs with no dormant entry left to drain.
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
                client
                    .call_registry()
                    .remove_if_current(call_id, generation);
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

    // The shared mute flag the mic feed checks: muted frames become exact-zero (the engine sends a
    // cheap DTX comfort-noise frame for an all-zero frame, so the relay stream never gaps).
    let (mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(MIC_CHANNEL_CAPACITY);
    let mute_feed = MuteFeed {
        src: source.frames(),
        out: mic_tx,
        muted,
    };
    // Keep the feed's AbortHandle (don't detach): it moves into the driver task below so the feed
    // dies with the call instead of parking on `src.recv()` forever, holding the mic channel open.
    let mic_feed = client.runtime.spawn(Box::pin(mute_feed.run()));

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
        speaker: sink.playout(),
        events: ev_tx,
        rekey: rekey_rx,
        video_in: video_in_rx,
        video_out: video_out_tx,
        video_ctl: video_ctl_rx,
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
        let _mic_feed = mic_feed;
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
        if let Some(old) = self
            .feed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .replace(handle)
        {
            old.abort();
        }
    }

    /// Release the endpoints (downgrade / refused upgrade): the source feed is aborted, the sink is
    /// dropped, and the drive loop's video plane is disabled so it stops emitting/decoding video.
    fn detach_endpoints(&self) {
        *self.sink_slot.lock().unwrap_or_else(|e| e.into_inner()) = None;
        if let Some(feed) = self.feed.lock().unwrap_or_else(|e| e.into_inner()).take() {
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

impl CallHandle {
    /// The call-id this handle controls.
    pub fn call_id(&self) -> &str {
        &self.call_id
    }

    /// The peer this call is with, as the `<terminate>` target. For an outgoing call this is the
    /// callee device that answered (learned from the inbound `<accept>`) once one has, since call
    /// signaling is addressed per device; before any accept, or for an incoming call, it is the bare
    /// peer the offer rang. Returns owned so it can merge the answering device tracked on the registry.
    pub fn peer_jid(&self) -> Jid {
        self.client_registry
            .answering_device_if_current(&self.call_id, self.generation)
            .unwrap_or_else(|| self.peer_jid.clone())
    }

    /// The call's creator JID, as carried in the signaling (needed by `voip().terminate(..)`).
    pub fn call_creator(&self) -> &Jid {
        &self.call_creator
    }

    /// Mute or unmute the local microphone. While muted the engine sends DTX comfort-noise (the
    /// stream stays fed); it does not gap, so the peer doesn't re-negotiate the transport.
    pub fn set_muted(&self, muted: bool) {
        self.muted.store(muted, Ordering::Relaxed);
    }

    /// Whether the microphone is currently muted.
    pub fn is_muted(&self) -> bool {
        self.muted.load(Ordering::Relaxed)
    }

    /// UPGRADE the call to video (we initiate): attaches the endpoints, enables the media plane,
    /// and sends `<video state=11 dec="H264" voip_settings="video">`. The peer answers with
    /// `UpgradeAccept`/`Enabled` (surfaced as [`CallEvent::VideoStateChanged`]); media flows as
    /// soon as both planes are up. Also the way to start media on a `.video()` call whose builder
    /// endpoints you skipped.
    pub async fn start_video<S, K>(&self, source: S, sink: K) -> Result<(), CallError>
    where
        S: VideoSource,
        K: VideoSink,
    {
        self.begin_video(
            Arc::new(source),
            Arc::new(sink),
            VideoState::UpgradeRequestV2,
        )
        .await
    }

    /// ACCEPT the peer's video upgrade request (a `VideoStateChanged { state: UpgradeRequestV2 }`
    /// event): attaches the endpoints, enables the media plane, and answers
    /// `<video state=4 dec="H264,AV1">` followed by `<video state=1>` (Enabled).
    pub async fn accept_video<S, K>(&self, source: S, sink: K) -> Result<(), CallError>
    where
        S: VideoSource,
        K: VideoSink,
    {
        self.begin_video(Arc::new(source), Arc::new(sink), VideoState::UpgradeAccept)
            .await
    }

    /// Send the standalone `<video state=1>` used after a mid-call video upgrade. Captured
    /// video-from-start callees do not need this extra stanza.
    pub async fn announce_video_enabled(&self) -> Result<(), CallError> {
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        let stanza = build_video_state(&VideoStateParams {
            call_id: &self.call_id,
            to: &self.peer_jid(),
            id: &client.generate_request_id(),
            call_creator: &self.call_creator,
            state: VideoState::Enabled,
            dec: None,
            device_orientation: None,
        });
        client.send_node(stanza).await?;
        Ok(())
    }

    /// DOWNGRADE to audio: sends `<video state=6>` (Stopped, no marker), tears the local video
    /// plane down, and releases the source/sink. The audio plane is untouched. Idempotent.
    pub async fn stop_video(&self) -> Result<(), CallError> {
        self.ensure_current()?;
        let client = self.upgrade_client()?;
        // Tear local media down FIRST, matching `Voip::terminate`: the app asked to stop video, so
        // a failed signaling send must NOT leave the camera streaming. If the peer misses the
        // Stopped it keeps sending video, but our plane is disabled so those PT-97 packets drop.
        self.teardown_local_video();
        let stanza = build_video_state(&VideoStateParams {
            call_id: &self.call_id,
            to: &self.peer_jid(),
            id: &client.generate_request_id(),
            call_creator: &self.call_creator,
            state: VideoState::Stopped,
            dec: None,
            device_orientation: Some(0),
        });
        client.send_node(stanza).await?;
        Ok(())
    }

    /// Release the local video plane: detach the endpoints (stops the camera feed, drops the sink,
    /// disables the drive-loop plane) and clear the session flag. Shared by `stop_video` and the
    /// `begin_video` rollback.
    fn teardown_local_video(&self) {
        let pending_video = {
            let mut pending = self
                .pending_outgoing_calls
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            pending
                .get_mut(&self.call_id)
                .filter(|entry| entry.generation == self.generation)
                .and_then(|entry| entry.video.take())
        };
        drop(pending_video);
        if !self
            .client_registry
            .run_video_teardown(&self.call_id, self.generation)
        {
            self.video.detach_endpoints();
        }
        self.client_registry
            .set_is_video(&self.call_id, self.generation, false);
    }

    /// Shared upgrade/accept body: attach endpoints, enable the plane, send the handshake stanzas.
    async fn begin_video(
        &self,
        source: Arc<dyn VideoSource>,
        sink: Arc<dyn VideoSink>,
        role: VideoState,
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
        self.video
            .attach_endpoints(&client, &source, &sink, self.ended.clone());
        if !self.client_registry.set_video_teardown(
            &self.call_id,
            self.generation,
            video_teardown_hook(&self.video),
        ) {
            self.video.detach_endpoints();
            return Err(CallError::Media("call no longer active"));
        }
        // The upgrade INITIATOR holds outbound video until the peer accepts (the handler ungates on
        // the peer's UpgradeAccept/Enabled). The acceptor's peer already requested, so it may send
        // immediately.
        let ctl = if role == VideoState::UpgradeRequestV2 {
            VideoControl::EnableAwaitingAccept
        } else {
            VideoControl::Enable
        };
        self.video.send_control(ctl);
        self.client_registry
            .set_is_video(&self.call_id, self.generation, true);

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
                device_orientation: None,
            });
            let client = client.clone();
            async move { client.send_node(stanza).await }
        };
        match role {
            VideoState::UpgradeRequestV2 => {
                if let Err(e) =
                    send_state(VideoState::UpgradeRequestV2, Some(VIDEO_DEC_REQUEST)).await
                {
                    self.teardown_local_video();
                    return Err(e.into());
                }
            }
            _ => {
                if let Err(e) = send_state(VideoState::UpgradeAccept, Some(VIDEO_DEC_ACCEPT)).await
                {
                    self.teardown_local_video();
                    return Err(e.into());
                }
                if let Err(e) = send_state(VideoState::Enabled, None).await {
                    // The peer times out an incomplete handshake; keep local media aligned with it.
                    warn!(
                        "voip: video upgrade handshake failed call_id={} phase=enabled_after_accept error={e}",
                        self.call_id
                    );
                    self.teardown_local_video();
                    return Err(e.into());
                }
            }
        }
        Ok(())
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
    /// [`Voip::terminate`](super::super::client::voip::Voip::terminate) if the peer must be told.
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

    /// Subscribe to the call's engine events (relay allocate, foreign-audio, allocate failures).
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

    fn mk_session() -> wacore::voip::CallSession {
        wacore::voip::CallSession::new_incoming("CID-FACADE", caller(), caller())
    }

    // peer_jid() is the <terminate> target: the bare peer until an <accept> records the answering
    // device on the registry, then that device (call signaling is addressed per device).
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
            pending_outgoing_calls: client.pending_outgoing_calls.clone(),
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
            "CID-FACADE".into(),
            mk_session(),
            engine(),
            &factory,
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
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
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.wait_ended())
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
            "CID-FACADE".into(),
            session,
            engine(),
            &factory,
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.wait_ended())
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
            "CID-FACADE".into(),
            mk_session(),
            engine(),
            &factory,
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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
            "CID-FACADE".into(),
            mk_session(),
            engine(),
            &f1,
            mic1,
            spk1,
            None,
        )
        .await
        .expect("first spawn_call");
        // A same-call-id re-offer replaces the first (new generation, aborts its task).
        let (f2, mic2, spk2) = spawn(&client);
        let live = spawn_call(
            &client,
            "CID-FACADE".into(),
            mk_session(),
            engine(),
            &f2,
            mic2,
            spk2,
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
            "CID-FACADE".into(),
            mk_session(),
            engine(),
            &f1,
            mic1,
            spk1,
            None,
        )
        .await
        .expect("first spawn_call");
        let (f2, mic2, spk2) = spawn(&client);
        let _live = spawn_call(
            &client,
            "CID-FACADE".into(),
            mk_session(),
            engine(),
            &f2,
            mic2,
            spk2,
            None,
        )
        .await
        .expect("replacement spawn_call");

        // The replacement aborted the stale task; its wait_ended must still resolve.
        tokio::time::timeout(std::time::Duration::from_secs(2), stale.wait_ended())
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
                "CID-FACADE".into(),
                mk_session(),
                engine(),
                &factory,
                Arc::new(mic_rx),
                Arc::new(spk_tx),
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
        tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        handle.hangup().await;
        tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
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
    async fn make_sending_client() -> (Arc<Client>, Arc<std::sync::atomic::AtomicUsize>) {
        make_sending_client_with_failure_after(None).await
    }

    async fn make_sending_client_with_failure_after(
        failure_after: Option<usize>,
    ) -> (Arc<Client>, Arc<std::sync::atomic::AtomicUsize>) {
        let backend = create_test_backend().await;
        make_sending_client_with_backend(backend, failure_after).await
    }

    async fn make_sending_client_with_backend(
        backend: Arc<dyn Backend>,
        failure_after: Option<usize>,
    ) -> (Arc<Client>, Arc<std::sync::atomic::AtomicUsize>) {
        use wacore::handshake::NoiseCipher;
        let pm = PersistenceManager::new(backend).await.expect("pm");
        // Set our own LID so get_lid() resolves (the send-side participant id).
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
        let key = [0u8; 32];
        let noise_socket = crate::socket::NoiseSocket::new(
            Arc::new(crate::runtime_impl::TokioRuntime),
            socket_transport,
            NoiseCipher::new(&key).expect("key"),
            NoiseCipher::new(&key).expect("key"),
        );
        *client.noise_socket.lock().await = Some(Arc::new(noise_socket));
        client.set_connected_for_test(true);
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

        let own_lid = client.get_lid().expect("own lid");
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
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .contains_key("00abcdef0123456789abcdef01234567"),
            "the relay-attach material must be parked pending the relay"
        );

        let node = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
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
        let own_lid = client.get_lid().expect("own lid");
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
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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

        let own_lid = client.get_lid().expect("own lid");
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
            Arc::new(mic_rx),
            Arc::new(spk_tx),
            None,
        )
        .await
        .expect("place_call");

        let node = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
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

        let own_lid = client.get_lid().expect("own lid");
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
            Arc::new(mic_rx),
            Arc::new(spk_tx),
            None,
        )
        .await
        .expect("place_call must succeed with surviving devices");

        assert_eq!(handle.call_id(), "00abcdef0123456789abcdef0123c0de");
        assert!(
            sent_count.load(Ordering::SeqCst) >= 1,
            "the offer for the surviving devices must be sent"
        );

        let node = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
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
        let own_lid = client.get_lid().expect("own lid");
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
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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
            client.pending_outgoing_calls.lock().unwrap().is_empty(),
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
        let own_lid = client.get_lid().expect("own lid");
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
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .contains_key(&call_id),
            "the dormant call is parked pending the relay"
        );

        handle.hangup().await;

        assert!(
            client.pending_outgoing_calls.lock().unwrap().is_empty(),
            "hangup must drop the dormant pending entry"
        );
        assert_eq!(
            client.call_registry().active_count(),
            0,
            "hangup must deregister the dormant call"
        );
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.wait_ended())
            .await
            .expect("dormant hangup must resolve wait_ended (no engine task to notify it)");
    }

    // A disconnect must tear down dormant outgoing calls: drain pending_outgoing_calls and notify each
    // `ended`, so a parked wait_ended() resolves instead of hanging across the reconnect.
    #[tokio::test]
    async fn disconnect_drains_dormant_outgoing_and_resolves_wait_ended() {
        let (client, _count) = make_sending_client().await;
        let (handle, _call_id) = place_dormant_outgoing(&client).await;

        crate::voip::facade::drain_pending_outgoing_on_disconnect(&client);

        assert!(
            client.pending_outgoing_calls.lock().unwrap().is_empty(),
            "disconnect must drain dormant outgoing calls"
        );
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.wait_ended())
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
                    "CID-FACADE".into(),
                    mk_session(),
                    engine(),
                    &factory,
                    Arc::new(mic_rx),
                    Arc::new(spk_tx),
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
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
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
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.wait_ended())
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
            "CID-FACADE".into(),
            mk_session(),
            engine(),
            &factory,
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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
            pending_outgoing_calls: client.pending_outgoing_calls.clone(),
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
                    engine(),
                    &*factory,
                    Arc::new(mic_rx),
                    Arc::new(spk_tx),
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
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        handle.hangup().await;

        tokio::time::timeout(std::time::Duration::from_secs(2), handle.wait_ended())
            .await
            .expect(
                "hangup in the connect window must wake wait_ended without the dial completing",
            );

        let res = tokio::time::timeout(std::time::Duration::from_secs(2), attach)
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
                    engine(),
                    &*factory,
                    Arc::new(mic_rx),
                    Arc::new(spk_tx),
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
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        // A disconnect clears the task-less registry entry, whose on_terminal hook wakes `ended`.
        client.call_registry().abort_all();

        tokio::time::timeout(std::time::Duration::from_secs(2), ended.wait())
            .await
            .expect(
                "a disconnect in the connect window must wake `ended` without the dial completing",
            );

        let res = tokio::time::timeout(std::time::Duration::from_secs(2), attach)
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
                    engine(),
                    &*factory,
                    Arc::new(mic_rx),
                    Arc::new(spk_tx),
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
        tokio::time::sleep(std::time::Duration::from_millis(30)).await;

        // The peer terminal-stanza path (no pending entry; entry has no media task yet).
        terminate_call(&client, "CID-FACADE");

        tokio::time::timeout(std::time::Duration::from_secs(2), ended.wait())
            .await
            .expect("a peer terminate in the connect window must wake `ended`");

        let res = tokio::time::timeout(std::time::Duration::from_secs(2), attach)
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
            "CID-FACADE".into(),
            mk_session(),
            engine(),
            &FailingFactory,
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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
        let own_lid = client.get_lid().expect("own lid");
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
            Arc::new(mic_rx),
            Arc::new(spk_tx),
            None,
        )
        .await;
        assert!(
            matches!(res, Err(CallError::Send(_))),
            "a send failure must surface as a Send error"
        );
        assert!(
            client.pending_outgoing_calls.lock().unwrap().is_empty(),
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
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.wait_ended())
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
        tokio::time::timeout(std::time::Duration::from_secs(2), handle.wait_ended())
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
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
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
        let own_lid = client.get_lid().expect("own lid");
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
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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
            client.pending_outgoing_calls.lock().unwrap().is_empty(),
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
    fn outgoing_video_offer_uses_video_capability() {
        assert_eq!(offer_capability(false), CAPABILITY_OFFER);
        assert_eq!(offer_capability(true), CAPABILITY_VIDEO_OFFER);
        assert_ne!(offer_capability(false), offer_capability(true));
    }

    /// A live handle over a mock relay + a sending client, for the video handshake tests. The
    /// returned relay sender is a keepalive: dropping it disconnects the relay and ends the call.
    async fn sending_handle() -> (
        Arc<Client>,
        Arc<std::sync::atomic::AtomicUsize>,
        CallHandle,
        async_channel::Sender<wacore::voip::transport::RelayTransportEvent>,
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
            "CID-FACADE".into(),
            mk_session(),
            engine(),
            &factory,
            Arc::new(mic_rx),
            Arc::new(spk_tx),
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

    /// The action child of a sent `<call>` node.
    fn call_action_of(node: &wacore_binary::Node) -> wacore_binary::Node {
        node.as_node_ref().children().unwrap()[0].to_owned()
    }

    #[tokio::test]
    async fn start_video_sends_upgrade_request_with_marker() {
        let (client, _sent, handle, _relay_keepalive) = sending_handle().await;
        let (vsrc, vsink) = video_endpoints();
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        handle.start_video(vsrc, vsink).await.expect("start_video");
        let node = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
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
            pending_outgoing_calls: client.pending_outgoing_calls.clone(),
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
        let (vsrc, vsink) = video_endpoints();
        let before = sent_count.load(Ordering::SeqCst);
        let waiter = client.wait_for_sent_node(crate::client::NodeFilter::tag("call"));
        handle
            .accept_video(vsrc, vsink)
            .await
            .expect("accept_video");
        assert_eq!(
            sent_count.load(Ordering::SeqCst) - before,
            2,
            "accept_video sends UpgradeAccept then Enabled"
        );
        let node = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
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
            pending_outgoing_calls: client.pending_outgoing_calls.clone(),
            client: Arc::downgrade(&client),
            muted: Arc::new(AtomicBool::new(false)),
            video: video.clone(),
            events,
            ended: Arc::new(EndedFlag::default()),
        };

        let (source, sink) = video_endpoints();
        assert!(handle.accept_video(source, sink).await.is_err());
        assert_eq!(sent.load(Ordering::SeqCst), 2);
        assert!(video.sink_slot.lock().unwrap().is_none());
        assert!(!registry.snapshot("CID-FACADE").unwrap().is_video);
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
        let node = tokio::time::timeout(std::time::Duration::from_secs(2), waiter)
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
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .get(&call_id)
                .is_some_and(|pending| pending.video.is_some())
        );

        handle.stop_video().await.expect("dormant downgrade");
        assert!(
            client
                .pending_outgoing_calls
                .lock()
                .unwrap()
                .get(&call_id)
                .is_some_and(|pending| pending.video.is_none()),
            "relay attach must not resurrect endpoints removed before its ack"
        );
        assert!(!client.call_registry().snapshot(&call_id).unwrap().is_video);
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
        let got = tokio::time::timeout(std::time::Duration::from_secs(2), in_rx.recv())
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
        let after = tokio::time::timeout(std::time::Duration::from_millis(300), in_rx.recv()).await;
        assert!(
            after.is_err(),
            "an AU sent after ended must not be forwarded (feed stopped)"
        );
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
