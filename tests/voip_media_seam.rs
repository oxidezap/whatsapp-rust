//! The seam's API guard: a foreign media backend implemented importing only
//! `whatsapp_rust::voip_control`, with no `wacore` in any path.
//!
//! If the contract ever leaks an engine type into a trait signature, a spec field, or a method
//! another crate must name, this file stops compiling, which is the point. The stub backend below
//! is deliberately minimal: it stores the reserved session, accepts every command, and opens
//! without media. What it proves is that `reserve` → `open` → `submit`/`stats`/`subscribe` →
//! `close` is drivable from the re-exported surface alone.
#![cfg(all(feature = "voip-control", not(target_arch = "wasm32")))]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use whatsapp_rust::voip_control::{
    CallDirection, GroupCallUpdate, MediaAudioFormat, MediaAudioIo, MediaAudioSpec,
    MediaCloseReason, MediaCommand, MediaEvent, MediaOpenContext, MediaSessionKey,
    MediaSessionSpec, MediaSetupError, MediaStats, VoipMediaBackend, VoipMediaSession,
};

struct StubSession {
    sender: async_channel::Sender<MediaEvent>,
    receiver: Mutex<Option<async_channel::Receiver<MediaEvent>>>,
    closed: Mutex<Vec<String>>,
}

impl StubSession {
    fn new() -> Self {
        let (sender, receiver) = async_channel::bounded::<MediaEvent>(8);
        Self {
            sender,
            receiver: Mutex::new(Some(receiver)),
            closed: Mutex::new(Vec::new()),
        }
    }
}

impl VoipMediaSession for StubSession {
    fn submit(&self, _command: MediaCommand) -> bool {
        true
    }

    fn group_update_fits(&self, _update: &GroupCallUpdate, _is_call_link: bool) -> bool {
        true
    }

    fn publish(&self, event: MediaEvent) -> bool {
        self.sender.try_send(event).is_ok()
    }

    fn stats(&self) -> MediaStats {
        MediaStats::default()
    }

    fn subscribe(&self) -> async_channel::Receiver<MediaEvent> {
        self.receiver
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .take()
            .unwrap_or_else(|| {
                let (sender, receiver) = async_channel::bounded::<MediaEvent>(1);
                receiver.close();
                drop(sender);
                receiver
            })
    }

    fn close(&self, reason: MediaCloseReason) {
        self.closed
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .push(format!("{reason:?}"));
    }
}

struct StubBackend {
    sessions: Mutex<HashMap<MediaSessionKey, Arc<StubSession>>>,
}

impl StubBackend {
    fn new() -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
        }
    }
}

#[async_trait::async_trait]
impl VoipMediaBackend for StubBackend {
    fn reserve(
        &self,
        key: &MediaSessionKey,
        _direction: CallDirection,
    ) -> Arc<dyn VoipMediaSession> {
        let session = Arc::new(StubSession::new());
        self.sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(key.clone(), session.clone());
        session
    }

    async fn open(
        &self,
        spec: MediaSessionSpec,
        _ctx: MediaOpenContext,
    ) -> Result<(), MediaSetupError> {
        if self
            .sessions
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains_key(&spec.key)
        {
            Ok(())
        } else {
            Err(MediaSetupError::NoBackend)
        }
    }
}

fn spec(key: MediaSessionKey) -> MediaSessionSpec {
    MediaSessionSpec::builder()
        .key(key)
        .direction(CallDirection::Outgoing)
        .self_lid("1@lid".to_string())
        .peer_lid("2@lid".to_string())
        .call_key(vec![0x5A; 32])
        .ssrc(1)
        .audio(
            MediaAudioSpec::builder()
                .format(MediaAudioFormat::OPUS_16KHZ_60MS)
                .io(MediaAudioIo::Pcm)
                .build(),
        )
        .relay_token(vec![0xA5; 16])
        .auth_token(vec![0xC3; 16])
        .relay_ip("127.0.0.1".to_string())
        .relay_port(3478)
        .integrity_key(vec![0x3C; 16])
        .warp_mi_tag_len(10)
        .enable_media(true)
        .enable_video(false)
        .enable_sframe(false)
        .build()
}

#[tokio::test]
async fn foreign_backend_drives_the_full_lifecycle() {
    let backend = StubBackend::new();
    let key = MediaSessionKey::builder()
        .call_id("seam-guard".to_string())
        .generation(1)
        .build();

    let session = backend.reserve(&key, CallDirection::Outgoing);
    assert!(session.submit(MediaCommand::RequireVideoKeyframe));
    assert_eq!(session.stats(), MediaStats::default());
    let stream = session.subscribe();
    assert!(session.publish(MediaEvent::RelayAllocated));
    assert!(matches!(stream.try_recv(), Ok(MediaEvent::RelayAllocated)));

    backend
        .open(spec(key), MediaOpenContext::for_test())
        .await
        .expect("reserved session opens");

    session.close(MediaCloseReason::Local);
}
