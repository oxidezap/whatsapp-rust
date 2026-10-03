//! Standalone consumer: no workspace Cargo configuration or direct engine dependency.
use std::sync::Arc;
use whatsapp_rust::futures::future::BoxFuture;
use whatsapp_rust::voip::{CallEvent, CallEvents, CallHandle};
use whatsapp_rust::voip_control::{
    CallDirection, GroupCallUpdate, MediaCloseReason, MediaCommand, MediaEvent, MediaOpenContext,
    MediaSessionKey, MediaSessionSpec, MediaSetupError, MediaStats, VoipMediaBackend,
    VoipMediaSession,
};
use whatsapp_rust::{async_channel, async_trait};

pub fn acquire(call: &CallHandle) -> Option<CallEvents> {
    call.take_events()
}

pub async fn receive(events: &mut CallEvents) -> Result<CallEvent, async_channel::RecvError> {
    events.recv().await
}

pub async fn stream(events: &mut CallEvents) -> Option<CallEvent> {
    use whatsapp_rust::futures::StreamExt;
    events.next().await
}

pub fn consumer_task(events: CallEvents) -> BoxFuture<'static, ()> {
    use whatsapp_rust::futures::FutureExt;
    async move {
        let mut events = events;
        while let Ok(_event) = events.recv().await {}
    }
    .boxed()
}

pub struct ExternalBackend;
struct ExternalSession {
    sender: async_channel::Sender<MediaEvent>,
    receiver: async_channel::Receiver<MediaEvent>,
}

impl VoipMediaSession for ExternalSession {
    fn submit(&self, _: MediaCommand) -> bool {
        true
    }
    fn group_update_fits(&self, _: &GroupCallUpdate, _: bool) -> bool {
        true
    }
    fn publish(&self, event: MediaEvent) -> bool {
        self.sender.try_send(event).is_ok()
    }
    fn stats(&self) -> MediaStats {
        MediaStats::default()
    }
    fn subscribe(&self) -> async_channel::Receiver<MediaEvent> {
        self.receiver.clone()
    }
    fn close(&self, _: MediaCloseReason) {
        self.sender.close();
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl VoipMediaBackend for ExternalBackend {
    fn reserve(&self, _: &MediaSessionKey, _: CallDirection) -> Arc<dyn VoipMediaSession> {
        let (sender, receiver) = async_channel::bounded(8);
        Arc::new(ExternalSession { sender, receiver })
    }
    async fn open(&self, _: MediaSessionSpec, _: MediaOpenContext) -> Result<(), MediaSetupError> {
        Ok(())
    }
}

// Portable transport injection still uses the same public trait, not a native dialer.
pub fn install_transport(
    client: &whatsapp_rust::Client,
    provider: Arc<dyn whatsapp_rust::voip::RelayTransportProvider>,
) {
    client.set_relay_transport_provider(provider);
}
