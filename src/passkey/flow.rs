//! Client-side SHORTCAKE_PASSKEY linking flow — the runtime glue that drives the
//! deterministic primitives in [`wacore::shortcake`] over real IQ exchanges.
//!
//! Mirrors whatsmeow's `pair-passkey.go`. The protocol is a four-step handshake
//! on top of the normal companion-linking connection:
//!  1. server pushes `<notification type="passkey_prologue_request">` carrying the
//!     WebAuthn request options → [`handle_passkey_notification`] derives the
//!     handoff key from the current ADV secret, ROTATES the ADV secret, and emits
//!     [`Event::PairPasskeyRequest`];
//!  2. host (or the registered authenticator) produces a WebAuthn assertion →
//!     [`Client::send_passkey_response`] sends `<passkey_prologue>` (ephemeral
//!     identity + commitment, plus a handoff proof on a re-link);
//!  3. server pushes `<notification type="crsc_continuation">` with its
//!     `<primary_ephemeral_identity>` → [`handle_passkey_continuation`] does the
//!     X25519 agreement, sends `<companion_nonce>`, derives the verification code
//!     and encryption key, and emits [`Event::PairPasskeyConfirmation`];
//!  4. host (or auto, when the handoff proof skips the code UX) →
//!     [`Client::send_passkey_confirmation`] sends the AES-256-GCM-encrypted
//!     `<encrypted_pairing_request>` carrying the newly-rotated ADV secret.
//!
//! Linking then completes through the ordinary `pair-success` path.

use crate::client::Client;
use crate::passkey::{Assertion, PasskeyAuthenticator, PasskeyError, parse_request_options};
use crate::request::InfoQuery;
use crate::store::commands::DeviceCommand;
use crate::types::events::{Event, PairPasskeyConfirmation, PairPasskeyError, PairPasskeyRequest};
use log::warn;
use rand::RngExt;
use std::sync::Arc;
use wacore::libsignal::protocol::KeyPair;
use wacore::shortcake::ShortcakeUtils;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Node, NodeContent, NodeRef, OwnedNodeRef, SERVER_JID, Server};

/// Per-attempt linking state, set by [`Client::send_passkey_response`] and consumed
/// by the continuation + confirmation steps.
struct LinkingCache {
    keypair: KeyPair,
    companion_nonce: [u8; 32],
    pairing_ref: String,
    device_type: i32,
    skip_handoff_ux: bool,
    encryption_key: Option<[u8; 32]>,
}

/// SHORTCAKE_PASSKEY flow state held on the [`Client`].
#[derive(Default)]
pub(crate) struct PasskeyFlowState {
    /// Handoff HMAC key derived from the ADV secret BEFORE rotation; consumed once
    /// by the next [`Client::send_passkey_response`]. Presence ⇒ re-link path, which
    /// lets the server skip the verification-code UX. We consume-once rather than
    /// honor whatsmeow's 5-minute TTL: a new request always overwrites it.
    handoff_key: Option<[u8; 32]>,
    linking: Option<LinkingCache>,
    authenticator: Option<Arc<dyn PasskeyAuthenticator>>,
}

fn server_jid() -> Jid {
    Jid::new("", Server::Pn)
}

/// Build the `<passkey_prologue>` node: credential id + WebAuthn assertion JSON +
/// the prologue payload, plus the pairing-handoff proof on a re-link. Pure so the
/// exact wire shape (child tags + conditional proof) is unit-testable without a
/// live connection.
fn build_prologue_node(
    credential_id: Vec<u8>,
    webauthn_assertion: Vec<u8>,
    prologue_payload: Vec<u8>,
    handoff_proof: Option<[u8; 32]>,
) -> Node {
    let mut children = vec![
        NodeBuilder::new("credential_id")
            .bytes(credential_id)
            .build(),
        NodeBuilder::new("webauthn_assertion")
            .bytes(webauthn_assertion)
            .build(),
        NodeBuilder::new("prologue_payload")
            .bytes(prologue_payload)
            .build(),
    ];
    if let Some(proof) = handoff_proof {
        children.push(
            NodeBuilder::new("pairing_handoff_proof")
                .bytes(proof.to_vec())
                .build(),
        );
    }
    NodeBuilder::new("passkey_prologue")
        .children(children)
        .build()
}

/// Pull a child node's payload as bytes, accepting either binary or string content
/// (the server sends the options JSON as a text node).
fn child_payload(nr: &NodeRef<'_>, tag: &str) -> Option<Vec<u8>> {
    let child = nr.get_optional_child(tag)?;
    if let Some(b) = child.content_bytes() {
        Some(b.to_vec())
    } else {
        child.content_str().map(|s| s.as_bytes().to_vec())
    }
}

impl Client {
    /// Register a passkey authenticator. When set, the client AUTO-DRIVES the
    /// WebAuthn assertion step, and auto-confirms a re-link whose handoff proof
    /// makes the verification-code UX unnecessary. Hosts that want to show the code
    /// or drive the steps manually can leave this unset and react to the
    /// `Event::PairPasskey*` events with [`Client::send_passkey_response`] /
    /// [`Client::send_passkey_confirmation`].
    pub async fn set_passkey_authenticator(&self, authenticator: Arc<dyn PasskeyAuthenticator>) {
        self.passkey_state.lock().await.authenticator = Some(authenticator);
    }

    async fn passkey_authenticator(&self) -> Option<Arc<dyn PasskeyAuthenticator>> {
        self.passkey_state.lock().await.authenticator.clone()
    }

    /// Send the WebAuthn assertion to the server as `<passkey_prologue>`. Fetches a
    /// fresh pairing ref, builds the companion ephemeral identity + commitment, and
    /// attaches the pairing-handoff proof when this is a re-link. Call this after an
    /// [`Event::PairPasskeyRequest`].
    pub async fn send_passkey_response(&self, assertion: Assertion) -> Result<(), PasskeyError> {
        let server = server_jid();

        let ref_query = InfoQuery::get(
            "md",
            server.clone(),
            Some(NodeContent::Nodes(vec![NodeBuilder::new("ref").build()])),
        );
        let resp = self
            .send_iq(ref_query)
            .await
            .map_err(|e| PasskeyError::Flow(format!("ref iq failed: {e}")))?;
        let pairing_ref = child_payload(resp.get(), "ref")
            .and_then(|b| String::from_utf8(b).ok())
            .ok_or_else(|| PasskeyError::Flow("missing <ref> in server response".into()))?;

        let keypair = ShortcakeUtils::generate_companion_ephemeral_keypair();
        let companion_nonce = ShortcakeUtils::generate_companion_nonce();
        let snapshot = self.persistence_manager.get_device_snapshot();
        let device_type = snapshot.device_props.platform_type.unwrap_or(0);
        let companion_pub: [u8; 32] = keypair
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| PasskeyError::Flow("ephemeral public key is not 32 bytes".into()))?;

        let identity = ShortcakeUtils::build_companion_ephemeral_identity(
            &companion_pub,
            device_type,
            &pairing_ref,
        );
        let commitment = ShortcakeUtils::commitment_hash(&identity, &companion_nonce);
        let prologue_payload = ShortcakeUtils::build_prologue_payload(&identity, &commitment);

        let handoff_proof = {
            let mut state = self.passkey_state.lock().await;
            let proof = state
                .handoff_key
                .take()
                .map(|key| ShortcakeUtils::compute_pairing_handoff_proof(&key, &prologue_payload));
            state.linking = Some(LinkingCache {
                keypair,
                companion_nonce,
                pairing_ref,
                device_type,
                skip_handoff_ux: proof.is_some(),
                encryption_key: None,
            });
            proof
        };

        let prologue = build_prologue_node(
            assertion.credential_id,
            assertion.assertion_json,
            prologue_payload,
            handoff_proof,
        );
        self.send_iq(InfoQuery::set(
            "md",
            server,
            Some(NodeContent::Nodes(vec![prologue])),
        ))
        .await
        .map_err(|e| PasskeyError::Flow(format!("passkey_prologue iq failed: {e}")))?;
        Ok(())
    }

    /// Process the continuation notification: agree on the shared secret, send the
    /// companion nonce, derive the verification code + encryption key, and emit
    /// [`Event::PairPasskeyConfirmation`]. Spawned off the receive loop because it
    /// awaits a `<companion_nonce>` IQ round-trip.
    async fn process_passkey_continuation(
        &self,
        primary_bytes: Vec<u8>,
    ) -> Result<(), PasskeyError> {
        let primary = ShortcakeUtils::parse_primary_ephemeral_identity(&primary_bytes)
            .map_err(|e| PasskeyError::Flow(format!("primary ephemeral identity: {e}")))?;

        let (keypair, companion_nonce, pairing_ref, device_type, skip_handoff_ux) = {
            let state = self.passkey_state.lock().await;
            let cache = state
                .linking
                .as_ref()
                .ok_or_else(|| PasskeyError::Flow("continuation without a linking cache".into()))?;
            (
                cache.keypair.clone(),
                cache.companion_nonce,
                cache.pairing_ref.clone(),
                cache.device_type,
                cache.skip_handoff_ux,
            )
        };

        let nonce_node = NodeBuilder::new("companion_nonce")
            .bytes(companion_nonce.to_vec())
            .build();
        self.send_iq(InfoQuery::set(
            "md",
            server_jid(),
            Some(NodeContent::Nodes(vec![nonce_node])),
        ))
        .await
        .map_err(|e| PasskeyError::Flow(format!("companion_nonce iq failed: {e}")))?;

        let encryption_key = ShortcakeUtils::derive_encryption_key(
            &keypair,
            &primary.public_key,
            device_type,
            &pairing_ref,
        )
        .map_err(|e| PasskeyError::Flow(format!("encryption key: {e}")))?;
        let bare = ShortcakeUtils::derive_verification_code(
            &companion_nonce,
            &primary.public_key,
            &primary.nonce,
        );
        // Match WA Web's displayed "XXXX-XXXX" grouping (the code is 8 ASCII chars).
        let code = format!("{}-{}", &bare[..4], &bare[4..]);

        {
            let mut state = self.passkey_state.lock().await;
            if let Some(cache) = state.linking.as_mut() {
                cache.encryption_key = Some(encryption_key);
            }
        }

        self.core
            .event_bus
            .dispatch(Event::PairPasskeyConfirmation(PairPasskeyConfirmation {
                code,
                skip_handoff_ux,
            }));

        // Re-link: the handoff proof already established continuity, so no user code
        // confirmation is needed — auto-finish when an authenticator is driving.
        if skip_handoff_ux && self.passkey_authenticator().await.is_some() {
            self.send_passkey_confirmation().await?;
        }
        Ok(())
    }

    /// Finish the link: encrypt the `PairingRequest` (companion static keys + the
    /// newly-rotated ADV secret) under the derived key and send
    /// `<encrypted_pairing_request>`. For a fresh link, call this only after the
    /// user has confirmed the [`Event::PairPasskeyConfirmation`] code.
    pub async fn send_passkey_confirmation(&self) -> Result<(), PasskeyError> {
        let encryption_key = {
            let state = self.passkey_state.lock().await;
            let cache = state
                .linking
                .as_ref()
                .ok_or_else(|| PasskeyError::Flow("confirmation without a linking cache".into()))?;
            cache.encryption_key.ok_or_else(|| {
                PasskeyError::Flow("confirmation before encryption key derived".into())
            })?
        };

        let snapshot = self.persistence_manager.get_device_snapshot();
        let companion_public: [u8; 32] = snapshot
            .noise_key
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| PasskeyError::Flow("noise public key is not 32 bytes".into()))?;
        let companion_identity: [u8; 32] = snapshot
            .identity_key
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| PasskeyError::Flow("identity public key is not 32 bytes".into()))?;
        // The rotated ADV secret (set when the prologue request arrived).
        let adv_secret = snapshot.adv_secret_key;

        let plaintext = ShortcakeUtils::build_pairing_request(
            &companion_public,
            &companion_identity,
            &adv_secret,
        );
        let encrypted = ShortcakeUtils::encrypt_pairing_request(&plaintext, &encryption_key)
            .map_err(|e| PasskeyError::Flow(format!("encrypt pairing request: {e}")))?;
        let wrapped = ShortcakeUtils::build_encrypted_pairing_request(&encrypted);

        let node = NodeBuilder::new("encrypted_pairing_request")
            .bytes(wrapped)
            .build();
        self.send_iq(InfoQuery::set(
            "md",
            server_jid(),
            Some(NodeContent::Nodes(vec![node])),
        ))
        .await
        .map_err(|e| PasskeyError::Flow(format!("encrypted_pairing_request iq failed: {e}")))?;

        self.passkey_state.lock().await.linking = None;
        Ok(())
    }
}

/// `<notification type="passkey_prologue_request">`: derive the handoff key, rotate
/// the ADV secret, emit [`Event::PairPasskeyRequest`], and (if an authenticator is
/// registered) auto-drive the assertion + response.
pub(crate) async fn handle_passkey_notification(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    // The ADV-secret rotation below is security-sensitive: only honor a request
    // that actually came from the server.
    if node
        .get()
        .get_attr("from")
        .is_none_or(|v| v.as_str() != SERVER_JID)
    {
        warn!("ignoring passkey notification from a non-server JID");
        return;
    }

    let options_json = match child_payload(node.get(), "passkey_request_options")
        .and_then(|b| String::from_utf8(b).ok())
    {
        Some(json) => json,
        None => {
            warn!("passkey notification missing a valid <passkey_request_options>");
            client
                .core
                .event_bus
                .dispatch(Event::PairPasskeyError(PairPasskeyError {
                    error: "missing passkey_request_options".into(),
                    continuation: false,
                }));
            return;
        }
    };

    // Derive the handoff key from the CURRENT ADV secret, then rotate it: the proof
    // proves continuity from the prior secret, while the PairingRequest carries the
    // new one.
    let snapshot = client.persistence_manager.get_device_snapshot();
    match ShortcakeUtils::derive_pairing_handoff_hmac_key(&snapshot.adv_secret_key) {
        Ok(key) => client.passkey_state.lock().await.handoff_key = Some(key),
        Err(e) => warn!("failed to derive pairing-handoff key: {e}"),
    }
    let mut new_adv = [0u8; 32];
    rand::make_rng::<rand::rngs::StdRng>().fill(&mut new_adv);
    client
        .persistence_manager
        .process_command(DeviceCommand::SetAdvSecretKey(new_adv))
        .await;

    client
        .core
        .event_bus
        .dispatch(Event::PairPasskeyRequest(PairPasskeyRequest {
            request_options_json: options_json.clone(),
        }));

    if let Some(authenticator) = client.passkey_authenticator().await {
        let client = client.clone();
        client
            .clone()
            .runtime
            .spawn(Box::pin(async move {
                if let Err(e) = auto_drive_response(&client, authenticator, &options_json).await {
                    warn!("passkey auto-drive failed: {e}");
                    client
                        .core
                        .event_bus
                        .dispatch(Event::PairPasskeyError(PairPasskeyError {
                            error: e.to_string(),
                            continuation: false,
                        }));
                }
            }))
            .detach();
    }
}

async fn auto_drive_response(
    client: &Arc<Client>,
    authenticator: Arc<dyn PasskeyAuthenticator>,
    options_json: &str,
) -> Result<(), PasskeyError> {
    let request = parse_request_options(options_json)?;
    let assertion = authenticator.get_assertion(&request).await?;
    client.send_passkey_response(assertion).await
}

/// `<notification type="crsc_continuation">`: spawn the continuation processing
/// (it awaits a `<companion_nonce>` IQ, so it must not block the receive loop).
pub(crate) async fn handle_passkey_continuation(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    if node
        .get()
        .get_attr("from")
        .is_none_or(|v| v.as_str() != SERVER_JID)
    {
        warn!("ignoring passkey continuation from a non-server JID");
        return;
    }

    let primary_bytes = match child_payload(node.get(), "primary_ephemeral_identity") {
        Some(bytes) => bytes,
        None => {
            warn!("passkey continuation missing <primary_ephemeral_identity>");
            client
                .core
                .event_bus
                .dispatch(Event::PairPasskeyError(PairPasskeyError {
                    error: "missing primary_ephemeral_identity".into(),
                    continuation: true,
                }));
            return;
        }
    };

    let client = client.clone();
    client
        .clone()
        .runtime
        .spawn(Box::pin(async move {
            if let Err(e) = client.process_passkey_continuation(primary_bytes).await {
                warn!("passkey continuation failed: {e}");
                client
                    .core
                    .event_bus
                    .dispatch(Event::PairPasskeyError(PairPasskeyError {
                        error: e.to_string(),
                        continuation: true,
                    }));
            }
        }))
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{TestEventCollector, create_test_client, node_to_owned_ref};
    use crate::types::events::EventHandler;
    use std::time::Duration;

    fn server_notification(
        notif_type: &'static str,
        child: Option<wacore_binary::Node>,
    ) -> Arc<OwnedNodeRef> {
        let mut builder = NodeBuilder::new("notification")
            .attr("type", notif_type)
            .attr("from", SERVER_JID);
        if let Some(child) = child {
            builder = builder.children([child]);
        }
        node_to_owned_ref(&builder.build())
    }

    async fn wait_for(collector: &Arc<TestEventCollector>, pred: impl Fn(&Event) -> bool) {
        for _ in 0..200 {
            if collector.events().iter().any(|e| pred(e.as_ref())) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("expected event was not observed within the timeout");
    }

    #[test]
    fn prologue_node_wire_shape() {
        // re-link: the handoff proof child is present
        let node = build_prologue_node(
            b"cred-id".to_vec(),
            b"{\"type\":\"public-key\"}".to_vec(),
            b"prologue-proto".to_vec(),
            Some([0x42; 32]),
        );
        let nr = node.as_node_ref();
        assert_eq!(nr.tag.as_ref(), "passkey_prologue");
        assert_eq!(
            nr.get_optional_child("credential_id")
                .and_then(|n| n.content_bytes()),
            Some(&b"cred-id"[..])
        );
        assert_eq!(
            nr.get_optional_child("webauthn_assertion")
                .and_then(|n| n.content_bytes()),
            Some(&b"{\"type\":\"public-key\"}"[..])
        );
        assert_eq!(
            nr.get_optional_child("prologue_payload")
                .and_then(|n| n.content_bytes()),
            Some(&b"prologue-proto"[..])
        );
        assert_eq!(
            nr.get_optional_child("pairing_handoff_proof")
                .and_then(|n| n.content_bytes()),
            Some(&[0x42u8; 32][..])
        );

        // fresh link: no handoff proof child
        let fresh = build_prologue_node(b"c".to_vec(), b"a".to_vec(), b"p".to_vec(), None);
        assert!(
            fresh
                .as_node_ref()
                .get_optional_child("pairing_handoff_proof")
                .is_none()
        );
    }

    #[tokio::test]
    async fn passkey_prologue_request_rotates_adv_and_emits_event() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone() as Arc<dyn EventHandler>);

        let before = client
            .persistence_manager
            .get_device_snapshot()
            .adv_secret_key;

        // verbatim options JSON; the handler forwards it untouched in the event
        let options = r#"{"challenge":"YWJjZGVm","rpId":"web.whatsapp.com"}"#;
        let child = NodeBuilder::new("passkey_request_options")
            .bytes(options.as_bytes().to_vec())
            .build();
        client
            .process_node(server_notification("passkey_prologue_request", Some(child)))
            .await;

        let after = client
            .persistence_manager
            .get_device_snapshot()
            .adv_secret_key;
        assert_ne!(before, after, "ADV secret must rotate on a passkey request");

        let request = collector
            .events()
            .into_iter()
            .find_map(|e| match e.as_ref() {
                Event::PairPasskeyRequest(r) => Some(r.clone()),
                _ => None,
            })
            .expect("a PairPasskeyRequest event must be dispatched");
        assert_eq!(request.request_options_json, options);
    }

    #[tokio::test]
    async fn passkey_prologue_request_from_non_server_is_ignored() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone() as Arc<dyn EventHandler>);

        let before = client
            .persistence_manager
            .get_device_snapshot()
            .adv_secret_key;
        let child = NodeBuilder::new("passkey_request_options")
            .bytes(b"{}".to_vec())
            .build();
        // forge a notification from a non-server JID
        let node = NodeBuilder::new("notification")
            .attr("type", "passkey_prologue_request")
            .attr("from", "12345@s.whatsapp.net")
            .children([child])
            .build();
        client.process_node(node_to_owned_ref(&node)).await;

        let after = client
            .persistence_manager
            .get_device_snapshot()
            .adv_secret_key;
        assert_eq!(
            before, after,
            "a forged request must NOT rotate the ADV secret"
        );
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(e.as_ref(), Event::PairPasskeyRequest(_))),
            "no event for a non-server request"
        );
    }

    #[tokio::test]
    async fn passkey_prologue_request_without_options_emits_error() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone() as Arc<dyn EventHandler>);

        client
            .process_node(server_notification("passkey_prologue_request", None))
            .await;

        assert!(
            collector.events().iter().any(|e| matches!(
                e.as_ref(),
                Event::PairPasskeyError(err) if !err.continuation
            )),
            "missing passkey_request_options must emit a non-continuation error"
        );
    }

    #[tokio::test]
    async fn passkey_continuation_without_linking_cache_emits_error() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone() as Arc<dyn EventHandler>);

        // a well-formed primary identity, but no prior send_passkey_response ran, so
        // there is no linking cache — the continuation must surface a continuation error.
        let primary = waproto::whatsapp::PrimaryEphemeralIdentity {
            public_key: Some(vec![0xAB; 32]),
            nonce: Some(vec![0xCD; 32]),
        };
        let child = NodeBuilder::new("primary_ephemeral_identity")
            .bytes(prost::Message::encode_to_vec(&primary))
            .build();
        client
            .process_node(server_notification("crsc_continuation", Some(child)))
            .await;

        // the continuation runs in a spawned task, so poll for the event
        wait_for(
            &collector,
            |e| matches!(e, Event::PairPasskeyError(err) if err.continuation),
        )
        .await;
    }
}
