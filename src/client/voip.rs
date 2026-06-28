//! Call-control accessor. Signaling (reject/terminate) is always available since
//! the stanza builders live in core; media (call/accept) needs the `voip` feature.

#[cfg(feature = "voip")]
use std::sync::Arc;

use wacore::stanza::call::{TerminateParams, build_reject, build_terminate};
use wacore::types::call::IncomingCall;
use wacore_binary::Jid;

use super::{Client, ClientError};

/// Opaque call-control handle obtained via [`Client::voip`]. Borrows the client;
/// kept as a newtype so the surface can grow without breaking callers.
pub struct Voip<'a> {
    client: &'a Client,
}

impl Client {
    /// Call control: reject/terminate are always available; media (call/accept)
    /// needs the `voip` feature.
    pub fn voip(&self) -> Voip<'_> {
        Voip { client: self }
    }

    /// The per-call media registry the `voip` facade registers active calls in. `pub(crate)` so the
    /// facade and the connection-cleanup teardown share one instance.
    #[cfg(feature = "voip")]
    pub(crate) fn call_registry(&self) -> Arc<wacore::voip::CallRegistry> {
        self.call_registry.clone()
    }
}

/// Errors from call-control operations. `#[non_exhaustive]` so new variants stay
/// non-breaking after 1.0.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum CallError {
    #[error(transparent)]
    Send(#[from] ClientError),
    #[error("call_id cannot be empty")]
    EmptyCallId,
    /// `accept` was called with an `IncomingCall` that is not an `<offer>` (nothing to answer).
    #[cfg(feature = "voip")]
    #[error("not an incoming call offer")]
    NotAnOffer,
    /// `accept().start()` was called without `audio(source, sink)`.
    #[cfg(feature = "voip")]
    #[error("accept() requires audio(source, sink) before start()")]
    MissingAudio,
    /// Decrypting the offer's encrypted callKey failed.
    #[cfg(feature = "voip")]
    #[error("callKey decrypt failed: {0}")]
    Decrypt(String),
    /// Assembling the call config from the offer's relay block failed.
    #[cfg(feature = "voip")]
    #[error("call setup failed: {0}")]
    Setup(String),
    /// Connecting the relay media transport (UDP/DTLS/SCTP) failed.
    #[cfg(feature = "voip")]
    #[error("relay connect failed: {0}")]
    Connect(String),
    /// The offer was missing media material (no `<enc>`/`<relay>`, no callKey, no own LID, etc.).
    #[cfg(feature = "voip")]
    #[error("media offer error: {0}")]
    Media(&'static str),
    /// `call(peer)` resolved zero devices for the peer (nothing to address an offer to).
    #[cfg(feature = "voip")]
    #[error("peer has no resolvable devices")]
    NoDevices,
    /// An outgoing offer would emit a pkmsg `<enc>` but we hold no ADV account, so the peer could
    /// not validate the pre-key message. Refused before send to avoid advancing the sender chain
    /// (mirrors the peer-send path's `<device-identity>` requirement).
    #[cfg(feature = "voip")]
    #[error("offer pkmsg requires <device-identity> (account is None)")]
    MissingDeviceIdentity,
}

impl Voip<'_> {
    /// Reject an incoming call. Fire-and-forget — no server response is expected.
    pub async fn reject(&self, incoming: &IncomingCall) -> Result<(), CallError> {
        let call_id = incoming.action.call_id();
        if call_id.is_empty() {
            return Err(CallError::EmptyCallId);
        }
        let id = self.client.generate_request_id();
        let stanza = build_reject(call_id, &incoming.from, incoming.action.call_creator(), &id);
        // Consume the ringing flag BEFORE the async send: a caller <terminate> processed while we await
        // the send would otherwise hit take_ringing first and surface a phantom missed call for a call
        // we already declined (WA Web deletes it from _ringingCalls on reject). No-op if never ringing.
        #[cfg(feature = "voip")]
        self.client.call_registry().take_ringing(call_id);
        self.client.send_node(stanza).await?;
        Ok(())
    }

    /// Begin answering an incoming call's MEDIA plane: returns a builder; call
    /// `.audio(source, sink)` then `.start().await` to decrypt the callKey, connect the relay, and
    /// drive the call, yielding a [`CallHandle`](crate::voip::CallHandle). Signaling (preaccept /
    /// accept) is the consumer's concern; this drives only media. Requires the `voip` feature.
    #[cfg(feature = "voip")]
    pub fn accept<'b>(&'b self, incoming: &'b IncomingCall) -> crate::voip::AcceptCall<'b> {
        crate::voip::facade::AcceptCall::new(self.client, incoming)
    }

    /// Begin placing an outgoing 1:1 call to `peer`: returns a builder; call `.audio(source, sink)`
    /// then `.start().await` to generate the callKey, encrypt it per peer device, send the `<offer>`,
    /// and register the call, yielding a [`CallHandle`](crate::voip::CallHandle). The media engine
    /// only attaches once the server hands back the relay for our call-id (live), so the returned
    /// handle is dormant until then. Requires the `voip` feature.
    #[cfg(feature = "voip")]
    pub fn call<'b>(&'b self, peer: &'b Jid) -> crate::voip::OutgoingCall<'b> {
        crate::voip::facade::OutgoingCall::new(self.client, peer)
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
        #[cfg(feature = "voip")]
        crate::voip::facade::terminate_call(self.client, call_id);
        sent?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use async_trait::async_trait;
    use bytes::Bytes;
    use wacore::handshake::NoiseCipher;
    use wacore::types::call::{CallAction, IncomingCall};
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

    #[cfg(feature = "voip")]
    struct FailingTransport;

    #[cfg(feature = "voip")]
    #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
    impl crate::transport::Transport for FailingTransport {
        async fn send(&self, _data: Bytes) -> Result<(), anyhow::Error> {
            Err(anyhow::anyhow!("transport down"))
        }
        async fn disconnect(&self) {}
    }

    #[cfg(feature = "voip")]
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
    async fn terminate_sends_stanza() {
        let (client, count) = make_client_with_count().await;
        client
            .voip()
            .terminate("CALL-ID-0001", &caller(), &caller())
            .await
            .expect("terminate should send");
        assert_eq!(count.load(Ordering::SeqCst), 1);
    }

    #[cfg(feature = "voip")]
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

    #[cfg(feature = "voip")]
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
        };
        assert!(client.voip().reject(&call).await.is_err());
    }
}
