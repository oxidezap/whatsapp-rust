//! The resident media backend: builds the `wacore` engine from the neutral seam spec.
//!
//! This is the other half of the seam. `wacore::voip_control` declares
//! [`VoipMediaBackend`]/[`VoipMediaSession`] with no engine type in the API; this module implements
//! them on top of [`wacore::voip::CallEngine`], translating the flat [`MediaSessionSpec`] into the
//! engine's [`CallConfig`]. Events need no translation: [`MediaEvent`] is the engine's
//! `CallEvent` under its seam name, so the drive loop publishes it directly.
//!
//! The executor and the relay transport are constructor state here, never fields of the spec:
//! they are trait objects that cannot cross a process boundary, which is why the neutral contract
//! cannot carry them. The command mailboxes into a running drive task live on the resident session
//! ([`ResidentMediaSession`]), which the call registry stores.

use std::sync::Arc;

use bytes::Bytes;

use wacore::voip::engine::{CallConfig, CallEngine, CodecDecisionSource};
use wacore::voip::media_session::ResidentMediaSession;
use wacore::voip::transport::RelayEndpointParams;
use wacore::voip_control::{
    CallDirection, MediaCommand, MediaEvent, MediaGroupSpec, MediaSessionSpec, MediaSetupError,
    VoipMediaBackend, VoipMediaSession,
};

/// Three 60 ms frames absorb scheduling jitter without building a long capture delay.
const MIC_CHANNEL_CAPACITY: usize = 3;
/// Mono 16 kHz 60 ms frame length the engine expects; a muted frame is zeroed only at this length.
const WA_FRAME_SAMPLES: usize = 960;
/// Outbound video AU backlog before the source feed back-pressures.
const VIDEO_IN_CHANNEL_CAP: usize = 4;
/// Inbound video AU backlog between the drive loop and the sink forwarder.
const VIDEO_OUT_CHANNEL_CAP: usize = 8;

/// The relay endpoint a platform transport dials, read off the engine config the relay walk
/// already resolved.
pub fn relay_endpoint(config: &CallConfig) -> Result<RelayEndpointParams, MediaSetupError> {
    let addr = format!("{}:{}", config.relay_ip, config.relay_port)
        .parse()
        .map_err(|_| MediaSetupError::BadEndpoint)?;
    Ok(RelayEndpointParams {
        addr,
        ice_ufrag: wacore::voip_control::relay_parse::token_to_ice_ufrag(&config.auth_token),
        ice_pwd: String::from_utf8_lossy(&config.integrity_key).into_owned(),
    })
}

/// Build the engine from an already-parsed [`CallConfig`], through the neutral spec.
///
/// This is the resident backend's entry point on the live path: the facade parses the `<relay>` and
/// decrypts the callKey exactly as before, then hands the resulting config across the seam instead
/// of reaching for `CallEngine::new` itself. `generation` is the registration token the call
/// registry assigned, carried into the neutral [`MediaSessionSpec::key`] so a foreign backend can
/// reject a message from a superseded generation.
pub fn build_engine_from_config(
    config: CallConfig,
    generation: u64,
    group: Option<MediaGroupSpec>,
    tx_ids: Box<dyn wacore::voip::engine::TxIdSource>,
) -> Result<CallEngine, MediaSetupError> {
    let mut spec = MediaSessionSpec::try_from((config, generation))?;
    spec.group = group;
    build_engine(spec, tx_ids)
}

/// Give the engine the platform's standard-Opus codec, when this build has one.
///
/// A no-op without `voip-libopus`, and that is the honest outcome: the call still runs, and if the
/// peer turns out to speak Opus the engine reports `AudioSilence` with `NoDecoderForNegotiatedCodec`.
pub fn with_platform_audio_codec(engine: CallEngine) -> CallEngine {
    #[cfg(feature = "voip-libopus")]
    {
        let engine = engine
            .with_foreign_audio_codec_factory(Box::new(crate::voip::audio::LibopusCodecFactory));
        match crate::voip::audio::LibopusAudioCodec::new() {
            Ok(codec) => engine.with_foreign_audio_codec(Box::new(codec)),
            Err(e) => {
                log::warn!("voip: libopus unavailable for this call, Opus will not decode: {e}");
                engine
            }
        }
    }
    #[cfg(not(feature = "voip-libopus"))]
    engine
}

/// Build the engine from the neutral spec, applying the group media the spec carries.
pub fn build_engine(
    spec: MediaSessionSpec,
    tx_ids: Box<dyn wacore::voip::engine::TxIdSource>,
) -> Result<CallEngine, MediaSetupError> {
    // The non-lossy split: the group and the key come back beside the config, because `CallConfig`
    // cannot hold them.
    let parts = wacore::voip_control::engine_bridge::into_engine_parts(spec)?;
    let engine = CallEngine::new(parts.config, tx_ids)
        .map(with_platform_audio_codec)
        .map_err(|error| MediaSetupError::Backend(error.to_string()))?;
    let Some(group) = parts.group else {
        return Ok(engine);
    };
    let mut engine = engine;
    engine
        .configure_group(wacore::voip::engine::GroupEngineConfig {
            call_creator: group.call_creator,
            self_jid: group.self_jid,
            initial_update: group.initial_update,
            direct_peer: group
                .direct_peer
                .map(|peer| wacore::voip::engine::DirectPeer {
                    user_jid: peer.user_jid,
                    device_jid: peer.device_jid,
                    call_key: peer.call_key,
                }),
        })
        .map_err(|error| MediaSetupError::Backend(error.to_string()))?;
    Ok(engine)
}

/// The resident backend.
///
/// It owns the executor and, through a weak client, the relay-transport path, and reserves one
/// [`ResidentMediaSession`] per call. Its `open` builds the engine from the neutral spec and the
/// platform ports, wires the session's own mailboxes, and starts the drive loop on this backend's
/// runtime, owning that task internally. The control plane never names an engine type and never
/// holds the drive task.
pub struct WacoreVoipMediaBackend {
    runtime: Arc<dyn wacore::runtime::Runtime>,
    /// Weak so the backend does not keep the client alive; upgraded for `is_connected` and the
    /// relay-transport factory on the live path.
    client: std::sync::Weak<crate::client::Client>,
    /// Weak so a closed or dropped call's session frees once its last handle does: the registry
    /// entry owns the session, never this map. Dead entries are pruned on lookup.
    sessions: std::sync::Mutex<
        std::collections::HashMap<
            wacore::voip_control::MediaSessionKey,
            std::sync::Weak<ResidentMediaSession>,
        >,
    >,
}

impl WacoreVoipMediaBackend {
    #[must_use]
    pub fn new(
        runtime: Arc<dyn wacore::runtime::Runtime>,
        client: std::sync::Weak<crate::client::Client>,
    ) -> Self {
        Self {
            runtime,
            client,
            sessions: std::sync::Mutex::new(std::collections::HashMap::new()),
        }
    }

    /// The concrete resident session reserved under `key`, for this backend's own wiring.
    #[must_use]
    pub fn resident_session(
        &self,
        key: &wacore::voip_control::MediaSessionKey,
    ) -> Option<Arc<ResidentMediaSession>> {
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        // Prune entries whose session dropped: a long-lived client must not accumulate a dead key
        // per call it ever placed.
        sessions.retain(|_, weak| weak.strong_count() > 0);
        sessions.get(key).and_then(|weak| weak.upgrade())
    }

    #[must_use]
    pub fn runtime(&self) -> &Arc<dyn wacore::runtime::Runtime> {
        &self.runtime
    }

    /// Live map size, so tests can pin the anti-accumulation invariant. Production never
    /// needs it: pruning happens on reserve and lookup.
    #[cfg(test)]
    fn session_count(&self) -> usize {
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .len()
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl VoipMediaBackend for WacoreVoipMediaBackend {
    fn reserve(
        &self,
        key: &wacore::voip_control::MediaSessionKey,
        _direction: CallDirection,
    ) -> Arc<dyn VoipMediaSession> {
        let session = ResidentMediaSession::new();
        let mut sessions = self.sessions.lock().unwrap_or_else(|e| e.into_inner());
        // Same prune as the lookup path: calls reserved and ended before `open` must not
        // accumulate a dead key per reservation on a long-lived client.
        sessions.retain(|_, weak| weak.strong_count() > 0);
        sessions.insert(key.clone(), Arc::downgrade(&session));
        session
    }

    /// Bring the reserved session operational.
    ///
    /// This is the sole media startup path: it builds the engine from the neutral spec, dials the
    /// relay through the client's transport factory, wires the session's own command mailboxes and
    /// the platform ports into a [`CallChannels`](wacore::voip::CallChannels), and runs the drive
    /// loop on this backend's runtime. The drive task's abort handle is owned by the session, so
    /// `close` ends it; the control plane never sees it. On return the terminal reason is recorded
    /// and the registry entry reaped.
    async fn open(
        &self,
        _session: &Arc<dyn VoipMediaSession>,
        spec: MediaSessionSpec,
        mut ctx: wacore::voip_control::MediaOpenContext,
    ) -> Result<(), MediaSetupError> {
        let client = self.client.upgrade().ok_or(MediaSetupError::NoBackend)?;
        // A call cannot start media over a dropped session. The facade checked this before the
        // dial; `open` is the sole startup path now, so the check lives here.
        if !client.is_connected() {
            return Err(MediaSetupError::Backend(
                "connection dropped during call setup".into(),
            ));
        }
        let resident = self
            .resident_session(&spec.key)
            .ok_or(MediaSetupError::NoBackend)?;
        let key = spec.key.clone();
        let registry = client.call_registry();

        let endpoint = RelayEndpointParams::from_spec(&spec).ok_or(MediaSetupError::BadEndpoint)?;
        let mut engine = build_engine(spec, Box::new(crate::voip::driver::RandTxIds))?;
        // An epoch the caller already authenticated and fanned out is installed before media
        // starts, so the first packet the engine emits is keyed under it.
        if let Some((transaction_id, epoch)) = ctx.group_epoch.as_ref() {
            engine
                .apply_group_raw_epoch(*transaction_id, epoch.as_bytes())
                .map_err(|e| MediaSetupError::Backend(e.to_string()))?;
        }
        // A codec the caller selected from the peer's capability, adopted before the first packet.
        // The source keeps its own grammar; only the wire grammar moves.
        if let Some(codec) = ctx.initial_codec {
            engine
                .switch_audio_codec(codec, CodecDecisionSource::Negotiated)
                .map_err(|e| MediaSetupError::Backend(e.to_string()))?;
        }
        let warp_mi_tag_len = engine.media_warp_mi_tag_len();

        let factory = client
            .relay_transport_factory(&endpoint)
            .await
            .map_err(|e| MediaSetupError::Backend(e.to_string()))?;
        let (transport, relay_events) = factory
            .connect()
            .await
            .map_err(|e| MediaSetupError::Backend(e.to_string()))?;

        let stats = resident.install_fresh_stats_cell();
        // The session has owned the public event stream since reservation, and the `CallHandle`
        // already reads it through `subscribe`: signaling events published before media attaches
        // and the drive loop's media events share that one ordered stream with no install here.
        // If the caller pre-created the video plumbing, adopt its control sender so
        // `submit(MediaCommand::EnableVideo …)` reaches the loop's receiver while the handle steers
        // the same channel.
        if let Some(channels) = ctx.video_channels.as_ref() {
            resident.install_video_sender(channels.control_sender.clone());
        }
        // Adopt the caller-held video state: replay the peer rotations the registry retained
        // before media attached, and install the teardown hook on the entry. Both must happen
        // before the drive loop starts, so the first frames are stamped and the call's drop
        // releases the local source exactly once.
        adopt_video_state(
            &registry,
            &key,
            &resident,
            ctx.video_teardown.take(),
            std::mem::take(&mut ctx.peer_video_orientations),
        );
        // Replay the committed roster and reject a changed WARP tag width, exactly as the registry's
        // attach-time group wiring did; a refusal is the typed setup failure the facade reported.
        let (committed, established) = client
            .call_registry()
            .group_attach_replay(&key.call_id, key.generation)
            .unwrap_or((None, None));
        let group_ctl = resident.install_group_channel(warp_mi_tag_len, committed, established);
        let group_ctl = Some(group_ctl.ok_or_else(|| {
            MediaSetupError::Backend(
                "group relay WARP tag length changed during media attachment".into(),
            )
        })?);
        // The outbound recv-rekey receiver: the caller may hand one in (foreign backends), else the
        // session's own (resident).
        let rekey = ctx.rekey.take().or_else(|| resident.take_rekey_receiver());
        // The drive loop publishes into the session's own stream, which the handle subscribed at
        // registration: installing the context sender here would swap the stream out from under it.
        let events = resident.event_sender();
        let channels = build_channels(
            &client,
            ctx,
            stats,
            group_ctl,
            key.generation,
            rekey,
            events,
        )?;

        let runtime = Arc::clone(&self.runtime);
        let registry_for_task = Arc::clone(&registry);
        let cid = key.call_id.clone();
        let generation = key.generation;
        let task = self.runtime.spawn(Box::pin(async move {
            let reason =
                wacore::voip::run_call(runtime, transport, relay_events, channels, engine).await;
            // Record why the drive ended before the entry drops, so the session's `close` receives
            // the real reason rather than the `Local` default.
            registry_for_task.set_close_reason(&cid, generation, reason);
            registry_for_task.remove_if_current(&cid, generation);
        }));
        // The session owns the task; `close` aborts it (F6/F14).
        resident.install_drive_task(task);
        Ok(())
    }
}

/// Adopt the caller-held video state at open time, before the drive loop starts.
///
/// The retained peer rotations replay through the session's video sender, so the peer's first
/// frames are stamped with the announced rotation; without pre-created plumbing there is no
/// control sender and the replay is dropped with the video-less setup. The teardown hook is
/// installed on the registry entry (generation-guarded) so the call's drop runs it exactly
/// once; if the entry is already gone the call is dead and the hook is dropped with it.
fn adopt_video_state(
    registry: &wacore::voip_control::registry::CallRegistry,
    key: &wacore::voip_control::MediaSessionKey,
    session: &ResidentMediaSession,
    teardown: Option<Box<dyn Fn() + Send + Sync>>,
    orientations: Vec<(Option<wacore_binary::Jid>, u8)>,
) {
    for (participant, orientation) in orientations {
        session.submit(MediaCommand::SetVideoOrientation {
            participant,
            orientation,
        });
    }
    if let Some(teardown) = teardown {
        registry.set_video_teardown(&key.call_id, key.generation, teardown);
    }
}

/// Wire the platform ports and the session's command mailboxes into the driver's channels.
///
/// Only the selected audio I/O pair stays open; the inactive pair is a bounded(1) channel with the
/// far half dropped, so its select arm retires immediately without per-frame branching.
fn build_channels(
    client: &crate::client::Client,
    ctx: wacore::voip_control::MediaOpenContext,
    media_stats: Arc<wacore::voip_control::media_stats::MediaStatsCell>,
    group_ctl: Option<async_channel::Receiver<wacore::voip::GroupControl>>,
    generation: u64,
    rekey: Option<async_channel::Receiver<wacore::voip_control::control::PeerAnswer>>,
    events: async_channel::Sender<MediaEvent>,
) -> Result<wacore::voip::CallChannels, MediaSetupError> {
    use wacore::voip::CallChannels;

    // Audio: the selected pair stays open; the other is a closed bounded(1) stub. The mute feed and
    // the video feeds are detached: each ends when the drive loop drops the receiver it writes to,
    // which happens when the drive task ends.
    let (mic, speaker, encoded_audio_in, encoded_audio_out) = match ctx.audio {
        wacore::voip_control::MediaAudioPorts::Pcm { source, sink } => {
            let (mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(MIC_CHANNEL_CAPACITY);
            client.runtime.spawn_detached(Box::pin(
                MuteFeed {
                    src: source.frames(),
                    out: mic_tx,
                    muted: ctx.muted.clone(),
                }
                .run(),
            ));
            let (_enc_tx, enc_in) = async_channel::bounded::<Bytes>(1);
            let (enc_out, _enc_rx) = async_channel::bounded::<wacore::voip::EncodedAudioFrame>(1);
            (mic_rx, sink.playout(), enc_in, enc_out)
        }
        wacore::voip_control::MediaAudioPorts::Encoded { source, sink } => {
            let (_mic_tx, mic_rx) = async_channel::bounded::<Vec<i16>>(1);
            let (speaker, _speaker_rx) = async_channel::bounded::<Vec<i16>>(1);
            (mic_rx, speaker, source.frames(), sink.frames())
        }
        // The ports enum is non-exhaustive: a future I/O mode refuses with a typed setup error
        // instead of silently running with dead audio.
        _ => {
            return Err(MediaSetupError::Backend("unsupported audio ports".into()));
        }
    };

    // Video: the drive loop needs the loop halves. When the caller pre-created the plumbing (so a
    // dormant handle can steer video), those halves arrive in `ctx.video_channels` and the caller's
    // own feed/sink machinery owns the other ends; the backend must not create its own, or the
    // handle's sender would reach a different channel.
    let (video_in, timed_video_in, video_ctl, video_out) = match ctx.video_channels {
        Some(channels) => (
            channels.video_in,
            channels.timed_video_in,
            channels.control,
            channels.video_out,
        ),
        None => {
            // No pre-created plumbing: wire the source and sink directly, or retire the arms.
            let (video_in_tx, video_in) = async_channel::bounded::<Vec<u8>>(VIDEO_IN_CHANNEL_CAP);
            let (timed_video_in_tx, timed_video_in) =
                async_channel::bounded::<wacore::voip::VideoInput>(VIDEO_IN_CHANNEL_CAP);
            let (_ctl_tx, video_ctl) = wacore::voip::video_control_channel();
            let (video_out, video_out_rx) =
                async_channel::bounded::<wacore::voip::VideoFrame>(VIDEO_OUT_CHANNEL_CAP);
            if let Some(video) = ctx.video {
                let sink = video.sink.playout();
                client.runtime.spawn_detached(Box::pin(async move {
                    while let Ok(mut frame) = video_out_rx.recv().await {
                        frame.generation = generation;
                        // Loss tolerant: a stalled sink sheds frames rather than back-pressuring.
                        let _ = sink.try_send(frame);
                    }
                }));
                client.runtime.spawn_detached(Box::pin(
                    SourceFeed {
                        source: video.source,
                        out_legacy: video_in_tx,
                        out_timed: timed_video_in_tx,
                    }
                    .run(),
                ));
            } else {
                drop(video_out_rx);
                drop(video_in_tx);
                drop(timed_video_in_tx);
            }
            (video_in, Some(timed_video_in), video_ctl, video_out)
        }
    };

    Ok(CallChannels {
        mic,
        speaker,
        encoded_audio_in,
        encoded_audio_out,
        events,
        rekey,
        video_in,
        timed_video_in,
        video_out,
        video_ctl,
        group_ctl,
        media_stats,
    })
}

/// Configures and runs the mic mute feed.
struct MuteFeed {
    src: async_channel::Receiver<Vec<i16>>,
    out: async_channel::Sender<Vec<i16>>,
    muted: Arc<std::sync::atomic::AtomicBool>,
}

impl MuteFeed {
    async fn run(self) {
        use std::sync::atomic::Ordering;
        while let Ok(mut frame) = self.src.recv().await {
            if self.muted.load(Ordering::Relaxed) && frame.len() == WA_FRAME_SAMPLES {
                frame.fill(0);
            }
            if self.out.send(frame).await.is_err() {
                break;
            }
        }
    }
}

/// Pumps a video source into the drive loop's legacy and (if present) timestamped inputs.
struct SourceFeed {
    source: Arc<dyn wacore::voip_control::VideoSource>,
    out_legacy: async_channel::Sender<Vec<u8>>,
    out_timed: async_channel::Sender<wacore::voip::VideoInput>,
}

impl SourceFeed {
    async fn run(self) {
        if let Some(timed) = self.source.timed_frames() {
            while let Ok(frame) = timed.recv().await {
                if self
                    .out_timed
                    .send(
                        wacore::voip::VideoInput::builder()
                            .data(frame.data)
                            .timestamp(frame.timestamp)
                            .generation(0)
                            .build(),
                    )
                    .await
                    .is_err()
                {
                    break;
                }
            }
            return;
        }
        let legacy = self.source.frames();
        while let Ok(au) = legacy.recv().await {
            if self.out_legacy.send(au).await.is_err() {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore::voip_control::{MediaAudioFormat, MediaAudioIo, MediaAudioSpec, MediaSessionKey};

    fn spec() -> MediaSessionSpec {
        MediaSessionSpec::builder()
            .key(
                MediaSessionKey::builder()
                    .call_id("SEAM-1".into())
                    .generation(1)
                    .build(),
            )
            .direction(CallDirection::Incoming)
            .self_lid("1:0@lid".into())
            .peer_lid("2:0@lid".into())
            .call_key(vec![0u8; 32])
            .ssrc(7)
            .audio(
                MediaAudioSpec::builder()
                    .format(MediaAudioFormat::MLOW_16KHZ_60MS)
                    .io(MediaAudioIo::Pcm)
                    .build(),
            )
            .relay_token(vec![1, 2, 3])
            .auth_token(vec![])
            .relay_ip("127.0.0.1".into())
            .relay_port(3478)
            .integrity_key(b"key".into())
            .warp_mi_tag_len(4)
            .enable_media(false)
            .enable_video(false)
            .enable_sframe(false)
            .build()
    }

    fn tx_ids() -> Box<dyn wacore::voip::engine::TxIdSource> {
        Box::new(wacore::voip::engine::SequentialTxIds::new())
    }

    /// A runtime that is never asked to run anything: these tests reserve and open sessions, they do
    /// not drive a call.
    struct IdleRuntime;

    use std::future::Future;

    impl wacore::runtime::Runtime for IdleRuntime {
        fn spawn(
            &self,
            _future: std::pin::Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
        ) -> wacore::runtime::AbortHandle {
            wacore::runtime::AbortHandle::noop()
        }

        fn sleep(
            &self,
            _duration: std::time::Duration,
        ) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(std::future::pending())
        }

        fn spawn_blocking(
            &self,
            _f: Box<dyn FnOnce() + Send + 'static>,
        ) -> std::pin::Pin<Box<dyn Future<Output = ()> + Send>> {
            Box::pin(std::future::pending())
        }

        fn yield_now(
            &self,
        ) -> Option<std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send>>> {
            None
        }
    }

    fn backend() -> WacoreVoipMediaBackend {
        WacoreVoipMediaBackend::new(
            Arc::new(IdleRuntime) as Arc<dyn wacore::runtime::Runtime>,
            std::sync::Weak::new(),
        )
    }

    #[test]
    fn reserve_takes_the_generational_key() {
        // Item 1: the session's first step must carry the anti-ABA identity, not a bare call-id, so
        // a pre-attach command from a superseded generation can be told apart at reservation time.
        let backend = backend();
        let key = MediaSessionKey::builder()
            .call_id("SEAM-1".into())
            .generation(2)
            .build();
        let session = backend.reserve(&key, CallDirection::Outgoing);
        assert_eq!(session.stats(), wacore::voip_control::MediaStats::default());
    }

    #[test]
    fn open_adopts_the_caller_held_video_state() {
        // Item 4: the retained peer rotations replay through the session's video sender, and
        // the teardown hook lands on the entry so the call's drop runs it exactly once.
        use wacore::voip_control::control::VideoControl;
        use wacore::voip_control::registry::CallRegistry;
        use wacore::voip_control::resident_session::NoMediaBackend;

        let registry = CallRegistry::with_backend(Arc::new(NoMediaBackend));
        let generation = registry.insert(wacore::voip_control::CallSession::new_outgoing(
            "SEAM-VIDEO",
            wacore_binary::Jid::new("2", wacore_binary::Server::Lid),
            wacore_binary::Jid::new("1", wacore_binary::Server::Lid),
        ));
        let key = MediaSessionKey::builder()
            .call_id("SEAM-VIDEO".into())
            .generation(generation)
            .build();
        let session = ResidentMediaSession::new();
        let video_rx = session.install_video_channel();
        let torn_down = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let flag = Arc::clone(&torn_down);
        adopt_video_state(
            &registry,
            &key,
            &session,
            Some(Box::new(move || {
                flag.store(true, std::sync::atomic::Ordering::Release)
            })),
            vec![
                (None, 1),
                (
                    Some(wacore_binary::Jid::new("2", wacore_binary::Server::Lid)),
                    2,
                ),
            ],
        );
        // Self and participant rotations travel separate sub-channels, so their relative
        // order is not significant; both must arrive.
        let mut saw_self = false;
        let mut saw_peer = false;
        for _ in 0..2 {
            match video_rx.try_recv() {
                Ok(VideoControl::SetOrientation(1)) => saw_self = true,
                Ok(VideoControl::SetParticipantOrientation { orientation: 2, .. }) => {
                    saw_peer = true
                }
                other => panic!("unexpected video control: {other:?}"),
            }
        }
        assert!(saw_self && saw_peer);
        assert!(registry.run_video_teardown("SEAM-VIDEO", generation));
        assert!(torn_down.load(std::sync::atomic::Ordering::Acquire));
    }

    #[test]
    fn a_dropped_session_leaves_no_entry_behind() {
        // Item 3: the map holds only a weak handle, so a closed call's session frees once its
        // last handle does, and the dead key is pruned on lookup.
        let backend = backend();
        let key = MediaSessionKey::builder()
            .call_id("SEAM-WEAK".into())
            .generation(1)
            .build();
        let session = backend.reserve(&key, CallDirection::Outgoing);
        assert!(backend.resident_session(&key).is_some());
        drop(session);
        assert!(backend.resident_session(&key).is_none());
    }

    #[test]
    fn reserve_prunes_sessions_dropped_before_open() {
        // Item 3: calls reserved and ended before `open` must not accumulate a dead key per
        // reservation; the next reserve prunes them.
        let backend = backend();
        let first = MediaSessionKey::builder()
            .call_id("SEAM-DEAD".into())
            .generation(1)
            .build();
        let session = backend.reserve(&first, CallDirection::Outgoing);
        assert_eq!(backend.session_count(), 1);
        drop(session);
        let second = MediaSessionKey::builder()
            .call_id("SEAM-LIVE".into())
            .generation(1)
            .build();
        let _live = backend.reserve(&second, CallDirection::Outgoing);
        assert_eq!(backend.session_count(), 1);
        assert!(backend.resident_session(&first).is_none());
        assert!(backend.resident_session(&second).is_some());
    }

    #[test]
    fn the_engine_builds_from_the_neutral_spec() {
        let engine = build_engine(spec(), tx_ids()).expect("the neutral spec builds an engine");
        assert_eq!(engine.call_id(), "SEAM-1");
    }

    #[test]
    fn the_spec_carries_the_generational_identity() {
        // The registry distinguishes `call ABC gen 12` from `gen 13`; the neutral spec must too, or
        // a foreign backend keyed on the call-id alone could route a stale generation into the new
        // session. The key survives the config projection.
        let spec = spec();
        let key = spec.key.clone();
        assert_eq!(key.call_id, "SEAM-1");
        assert_eq!(key.generation, 1);
        let parts = wacore::voip_control::engine_bridge::into_engine_parts(spec)
            .expect("the spec splits into engine parts");
        assert_eq!(parts.key, key);
        assert_eq!(parts.config.call_id, "SEAM-1");
    }

    #[test]
    fn a_group_spec_rides_the_session_spec_and_configures_the_engine() {
        // Item 3: a foreign backend cannot open a group call without the group inputs, so they ride
        // the spec rather than a separate `open` parameter. The roster carries a relay and the local
        // participant is connected, so `configure_group` accepts it and the engine reports a group.
        let group = MediaGroupSpec::builder()
            .call_creator(wacore_binary::Jid::new("1", wacore_binary::Server::Lid))
            .self_jid(wacore_binary::Jid::new("1", wacore_binary::Server::Lid))
            .initial_update(group_update())
            .build();
        let mut spec = spec();
        spec.enable_media = true;
        spec.group = Some(group);
        let engine = build_engine(spec, tx_ids()).expect("a group spec builds a group engine");
        assert!(engine.is_group());
    }

    fn group_update() -> wacore::types::group_call::GroupCallUpdate {
        use wacore::types::group_call::{
            GroupCallDevice, GroupCallParticipant, GroupCallRelay, GroupCallRelayEndpoint,
            GroupCallUpdate,
        };
        let jid = wacore_binary::Jid::new("1", wacore_binary::Server::Lid);
        let mut local =
            GroupCallParticipant::new(jid.clone(), vec![GroupCallDevice::new(jid.clone())]);
        local.state = Some("connected".to_string());
        let relay = GroupCallRelay::builder()
            .transaction_id(1)
            .self_pid(1)
            .uuid("relay".to_string())
            .participant_uuid("participant".to_string())
            .attribute_padding(false)
            .warp_mi_tag_len(4)
            .key(b"relay-key".to_vec())
            .tokens(vec![vec![0x47]])
            .auth_tokens(vec![vec![0x57]])
            .endpoints(vec![
                GroupCallRelayEndpoint::builder()
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
        GroupCallUpdate::builder()
            .call_id("SEAM-1".to_string())
            .call_creator(jid)
            .transaction_id(1)
            .media("audio".to_string())
            .connected_limit(32)
            .joinable(true)
            .av_upgradable(true)
            .rekey_requested(false)
            .participants(vec![local])
            .relay(relay)
            .build()
    }

    #[test]
    fn a_short_call_key_is_refused_without_building() {
        let mut bad = spec();
        bad.call_key = vec![0u8; 8];
        assert_eq!(
            build_engine(bad, tx_ids()).err(),
            Some(MediaSetupError::BadCallKey)
        );
    }

    #[test]
    fn a_zero_timing_format_is_refused() {
        let mut bad = spec();
        bad.audio.format.sample_rate = 0;
        assert_eq!(
            build_engine(bad, tx_ids()).err(),
            Some(MediaSetupError::BadAudioFormat)
        );
    }
}
