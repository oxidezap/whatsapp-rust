//! Client-side SHORTCAKE_PASSKEY linking flow: the runtime glue that drives the
//! deterministic primitives in [`wacore::shortcake`] over real IQ exchanges.
//!
//! The handshake sits on top of the normal companion-linking connection: the
//! server requests a WebAuthn assertion, the companion answers with an ephemeral
//! identity prologue, both sides exchange nonces to derive a shared key, and the
//! companion finally sends its rotated ADV secret encrypted under that key. Linking
//! then completes through the ordinary `pair-success` path.
//!
//! The handshake state machine ([`ShortcakeSession`]) drives against a
//! [`ShortcakeIo`] seam rather than the concrete [`Client`], so the full IQ
//! sequence is unit-testable with a scripted stand-in.

use crate::client::Client;
use crate::passkey::{Assertion, PasskeyAuthenticator, PasskeyError, parse_request_options};
use crate::request::InfoQuery;
use crate::store::commands::DeviceCommand;
use crate::types::events::{Event, PairPasskeyConfirmation, PairPasskeyError, PairPasskeyRequest};
use async_trait::async_trait;
use log::warn;
use rand::RngExt;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wacore::libsignal::protocol::KeyPair;
use wacore::shortcake::ShortcakeUtils;
use wacore::sync_marker::MaybeSendSync;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Node, NodeContent, NodeRef, OwnedNodeRef, SERVER_JID, Server};
use waproto::whatsapp as wa;

/// `<notification type=...>` routing keys, consumed by the notification dispatcher.
pub(crate) const NOTIF_PASSKEY_REQUEST: &str = "passkey_prologue_request";
pub(crate) const NOTIF_PASSKEY_CONTINUATION: &str = "crsc_continuation";

const MD_NAMESPACE: &str = "md";
const TAG_REF: &str = "ref";
const TAG_PASSKEY_REQUEST_OPTIONS: &str = "passkey_request_options";
const TAG_PASSKEY_PROLOGUE: &str = "passkey_prologue";
const TAG_CREDENTIAL_ID: &str = "credential_id";
const TAG_WEBAUTHN_ASSERTION: &str = "webauthn_assertion";
const TAG_PROLOGUE_PAYLOAD: &str = "prologue_payload";
const TAG_PAIRING_HANDOFF_PROOF: &str = "pairing_handoff_proof";
const TAG_PRIMARY_EPHEMERAL_IDENTITY: &str = "primary_ephemeral_identity";
const TAG_COMPANION_NONCE: &str = "companion_nonce";
const TAG_ENCRYPTED_PAIRING_REQUEST: &str = "encrypted_pairing_request";

/// Length of each half of the "XXXX-XXXX" verification code grouping.
const CODE_GROUP_LEN: usize = 4;

/// The device material the handshake reads: the companion's static public keys
/// and the reported platform.
#[derive(Clone)]
struct DeviceMaterial {
    noise_public: [u8; 32],
    identity_public: [u8; 32],
    device_type: wa::device_props::PlatformType,
}

/// The effects the handshake needs from its environment. Abstracted so the full
/// IQ sequence can be driven by a scripted stand-in in tests.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
trait ShortcakeIo: MaybeSendSync {
    async fn query(&self, query: InfoQuery<'static>) -> Result<Arc<OwnedNodeRef>, PasskeyError>;
    fn device_material(&self) -> Result<DeviceMaterial, PasskeyError>;
    async fn commit_adv_secret(&self, secret: [u8; 32]);
}

#[derive(PartialEq, Eq)]
enum Stage {
    AwaitingPrimaryIdentity,
    AwaitingConfirmation,
    Done,
}

/// The in-flight handshake, created by [`ShortcakeSession::open`] and advanced by
/// the continuation + confirmation steps. Its rotated ADV secret is held here (not
/// in the device store) until [`confirm`](Self::confirm) commits it, so an
/// abandoned attempt never rotates to a secret the primary never received.
struct ShortcakeSession {
    keypair: KeyPair,
    companion_nonce: [u8; 32],
    pairing_ref: String,
    device_type: wa::device_props::PlatformType,
    new_adv_secret: [u8; 32],
    skip_handoff_ux: bool,
    stage: Stage,
    encryption_key: Option<[u8; 32]>,
}

impl ShortcakeSession {
    /// Fetch a fresh ref, build the ephemeral identity + commitment, attach the
    /// handoff proof on a re-link, and send the `passkey_prologue` IQ.
    async fn open(
        io: &dyn ShortcakeIo,
        assertion: Assertion,
        handoff_key: Option<[u8; 32]>,
    ) -> Result<Self, PasskeyError> {
        let device_type = io.device_material()?.device_type;
        let pairing_ref = fetch_ref(io).await?;

        let keypair = ShortcakeUtils::generate_companion_ephemeral_keypair();
        let companion_nonce = ShortcakeUtils::generate_companion_nonce();
        let mut new_adv_secret = [0u8; 32];
        rand::make_rng::<rand::rngs::StdRng>().fill(&mut new_adv_secret);

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

        let handoff_proof = handoff_key
            .map(|key| ShortcakeUtils::compute_pairing_handoff_proof(&key, &prologue_payload));
        let skip_handoff_ux = handoff_proof.is_some();

        let prologue = build_prologue_node(
            assertion.credential_id,
            assertion.assertion_json,
            prologue_payload,
            handoff_proof,
        );
        io.query(InfoQuery::set(
            MD_NAMESPACE,
            server_jid(),
            Some(NodeContent::Nodes(vec![prologue])),
        ))
        .await
        .map_err(|e| PasskeyError::Flow(format!("passkey_prologue iq failed: {e}")))?;

        Ok(Self {
            keypair,
            companion_nonce,
            pairing_ref,
            device_type,
            new_adv_secret,
            skip_handoff_ux,
            stage: Stage::AwaitingPrimaryIdentity,
            encryption_key: None,
        })
    }

    /// Agree on the shared secret, reveal the companion nonce, and derive the code
    /// and encryption key. Returns the confirmation payload for the caller to
    /// publish, so the caller can restore the session before a synchronous listener
    /// that confirms observes it.
    async fn on_primary_identity(
        &mut self,
        io: &dyn ShortcakeIo,
        primary_bytes: &[u8],
    ) -> Result<PairPasskeyConfirmation, PasskeyError> {
        if self.stage != Stage::AwaitingPrimaryIdentity {
            return Err(PasskeyError::Flow(
                "unexpected continuation for this stage".into(),
            ));
        }
        let primary = ShortcakeUtils::parse_primary_ephemeral_identity(primary_bytes)
            .map_err(|e| PasskeyError::Flow(format!("primary ephemeral identity: {e}")))?;

        let nonce_node = NodeBuilder::new(TAG_COMPANION_NONCE)
            .bytes(self.companion_nonce.to_vec())
            .build();
        io.query(InfoQuery::set(
            MD_NAMESPACE,
            server_jid(),
            Some(NodeContent::Nodes(vec![nonce_node])),
        ))
        .await
        .map_err(|e| PasskeyError::Flow(format!("companion_nonce iq failed: {e}")))?;

        let encryption_key = ShortcakeUtils::derive_encryption_key(
            &self.keypair,
            &primary.public_key,
            self.device_type,
            &self.pairing_ref,
        )
        .map_err(|e| PasskeyError::Flow(format!("encryption key: {e}")))?;
        let bare = ShortcakeUtils::derive_verification_code(
            &self.companion_nonce,
            &primary.public_key,
            &primary.nonce,
        );
        // Grouped "XXXX-XXXX" for display (the code is ASCII).
        let code = format!("{}-{}", &bare[..CODE_GROUP_LEN], &bare[CODE_GROUP_LEN..]);

        self.encryption_key = Some(encryption_key);
        self.stage = Stage::AwaitingConfirmation;
        Ok(PairPasskeyConfirmation::builder()
            .code(code)
            .skip_handoff_ux(self.skip_handoff_ux)
            .build())
    }

    /// Encrypt the `PairingRequest` (companion static keys + rotated ADV secret)
    /// and send `<encrypted_pairing_request>`, then commit the rotation.
    async fn confirm(&mut self, io: &dyn ShortcakeIo) -> Result<(), PasskeyError> {
        if self.stage != Stage::AwaitingConfirmation {
            return Err(PasskeyError::Flow(
                "confirmation before the verification stage".into(),
            ));
        }
        let encryption_key = self.encryption_key.ok_or_else(|| {
            PasskeyError::Flow("confirmation before encryption key derived".into())
        })?;
        let material = io.device_material()?;

        let plaintext = ShortcakeUtils::build_pairing_request(
            &material.noise_public,
            &material.identity_public,
            &self.new_adv_secret,
        );
        let encrypted = ShortcakeUtils::encrypt_pairing_request(&plaintext, &encryption_key)
            .map_err(|e| PasskeyError::Flow(format!("encrypt pairing request: {e}")))?;
        let wrapped = ShortcakeUtils::build_encrypted_pairing_request(&encrypted);

        let node = NodeBuilder::new(TAG_ENCRYPTED_PAIRING_REQUEST)
            .bytes(wrapped)
            .build();
        io.query(InfoQuery::set(
            MD_NAMESPACE,
            server_jid(),
            Some(NodeContent::Nodes(vec![node])),
        ))
        .await
        .map_err(|e| PasskeyError::Flow(format!("encrypted_pairing_request iq failed: {e}")))?;

        // Primary has the secret now: commit the rotation (before pair-success
        // validates against it).
        io.commit_adv_secret(self.new_adv_secret).await;
        self.stage = Stage::Done;
        Ok(())
    }
}

/// SHORTCAKE_PASSKEY flow state held on the [`Client`].
#[derive(Default)]
pub(crate) struct PasskeyFlowState {
    /// HMAC key from the pre-rotation ADV secret; presence marks the re-link path
    /// that lets the server skip the verification-code UX. Consumed once.
    handoff_key: Option<[u8; 32]>,
    session: Option<ShortcakeSession>,
    authenticator: Option<Arc<dyn PasskeyAuthenticator>>,
}

/// Holds the wait-free open reservation and releases it on drop. Because it clears
/// a plain [`AtomicBool`] (not a flag behind the async lock), the release is a sync,
/// always-succeeding store, so a `send_passkey_response` cancelled at any await
/// can't leave the reservation stuck.
struct OpeningGuard<'a> {
    flag: &'a AtomicBool,
}

impl Drop for OpeningGuard<'_> {
    fn drop(&mut self) {
        self.flag.store(false, Ordering::Release);
    }
}

fn server_jid() -> Jid {
    Jid::new("", Server::Pn)
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

/// Pure so the wire shape (child tags + conditional proof) is unit-testable.
fn build_prologue_node(
    credential_id: Vec<u8>,
    webauthn_assertion: Vec<u8>,
    prologue_payload: Vec<u8>,
    handoff_proof: Option<[u8; 32]>,
) -> Node {
    let mut children = vec![
        NodeBuilder::new(TAG_CREDENTIAL_ID)
            .bytes(credential_id)
            .build(),
        NodeBuilder::new(TAG_WEBAUTHN_ASSERTION)
            .bytes(webauthn_assertion)
            .build(),
        NodeBuilder::new(TAG_PROLOGUE_PAYLOAD)
            .bytes(prologue_payload)
            .build(),
    ];
    if let Some(proof) = handoff_proof {
        children.push(
            NodeBuilder::new(TAG_PAIRING_HANDOFF_PROOF)
                .bytes(proof.to_vec())
                .build(),
        );
    }
    NodeBuilder::new(TAG_PASSKEY_PROLOGUE)
        .children(children)
        .build()
}

async fn fetch_ref(io: &dyn ShortcakeIo) -> Result<String, PasskeyError> {
    let resp = io
        .query(InfoQuery::get(
            MD_NAMESPACE,
            server_jid(),
            Some(NodeContent::Nodes(vec![NodeBuilder::new(TAG_REF).build()])),
        ))
        .await
        .map_err(|e| PasskeyError::Flow(format!("ref iq failed: {e}")))?;
    child_payload(resp.get(), TAG_REF)
        .and_then(|b| String::from_utf8(b).ok())
        .ok_or_else(|| PasskeyError::Flow("missing ref in server response".into()))
}

async fn fetch_request_options(io: &dyn ShortcakeIo) -> Result<String, PasskeyError> {
    let resp = io
        .query(InfoQuery::get(
            MD_NAMESPACE,
            server_jid(),
            Some(NodeContent::Nodes(vec![
                NodeBuilder::new(TAG_PASSKEY_REQUEST_OPTIONS).build(),
            ])),
        ))
        .await
        .map_err(|e| PasskeyError::Flow(format!("passkey_request_options iq failed: {e}")))?;
    child_payload(resp.get(), TAG_PASSKEY_REQUEST_OPTIONS)
        .and_then(|b| String::from_utf8(b).ok())
        .ok_or_else(|| PasskeyError::Flow("missing passkey_request_options in response".into()))
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl ShortcakeIo for Client {
    async fn query(&self, query: InfoQuery<'static>) -> Result<Arc<OwnedNodeRef>, PasskeyError> {
        self.send_iq(query)
            .await
            .map_err(|e| PasskeyError::Flow(e.to_string()))
    }

    fn device_material(&self) -> Result<DeviceMaterial, PasskeyError> {
        let snapshot = self.persistence_manager.get_device_snapshot();
        let noise_public: [u8; 32] = snapshot
            .noise_key
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| PasskeyError::Flow("noise public key is not 32 bytes".into()))?;
        let identity_public: [u8; 32] = snapshot
            .identity_key
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| PasskeyError::Flow("identity public key is not 32 bytes".into()))?;
        Ok(DeviceMaterial {
            noise_public,
            identity_public,
            device_type: snapshot
                .device_props
                .platform_type
                .unwrap_or(wa::device_props::PlatformType::UNKNOWN),
        })
    }

    async fn commit_adv_secret(&self, secret: [u8; 32]) {
        self.persistence_manager
            .process_command(DeviceCommand::SetAdvSecretKey(secret))
            .await;
    }
}

impl Client {
    /// Register a passkey authenticator. When set, the client auto-drives the
    /// assertion step and auto-confirms a re-link (where the handoff proof skips the
    /// verification-code UX). Leave it unset to drive the steps manually via the
    /// `Event::PairPasskey*` events.
    pub async fn set_passkey_authenticator(&self, authenticator: Arc<dyn PasskeyAuthenticator>) {
        self.passkey_state.lock().await.authenticator = Some(authenticator);
    }

    async fn passkey_authenticator(&self) -> Option<Arc<dyn PasskeyAuthenticator>> {
        self.passkey_state.lock().await.authenticator.clone()
    }

    /// Send the WebAuthn assertion as `<passkey_prologue>` and open the handshake.
    /// Call after an [`Event::PairPasskeyRequest`].
    pub async fn send_passkey_response(&self, assertion: Assertion) -> Result<(), PasskeyError> {
        // Reserve the single open slot BEFORE the awaits, so a concurrent response
        // can't open a second overlapping handshake and clobber this one's nonce/ref
        // (which would then commit the wrong ADV rotation). The guard releases the
        // reservation on every exit, including cancellation mid-open.
        if self
            .passkey_opening
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(PasskeyError::Flow(
                "a passkey open is already in progress".into(),
            ));
        }
        let _guard = OpeningGuard {
            flag: &self.passkey_opening,
        };

        let handoff_key = {
            let mut state = self.passkey_state.lock().await;
            if state.session.is_some() {
                return Err(PasskeyError::Flow(
                    "a passkey link is already in progress".into(),
                ));
            }
            state.handoff_key.take()
        };

        let session = ShortcakeSession::open(self, assertion, handoff_key).await?;
        self.passkey_state.lock().await.session = Some(session);
        Ok(())
    }

    /// Finish the link. For a fresh link, call this only after the user confirms the
    /// [`Event::PairPasskeyConfirmation`] code.
    pub async fn send_passkey_confirmation(&self) -> Result<(), PasskeyError> {
        // Only consume the session once it's actually at the confirmation stage — a
        // premature call must NOT drop the in-flight attempt.
        let mut session = {
            let mut state = self.passkey_state.lock().await;
            match state.session.take() {
                Some(s) if s.stage == Stage::AwaitingConfirmation => s,
                Some(s) => {
                    state.session = Some(s);
                    return Err(PasskeyError::Flow(
                        "confirmation before the verification stage".into(),
                    ));
                }
                None => {
                    return Err(PasskeyError::Flow(
                        "confirmation without an active session".into(),
                    ));
                }
            }
        };
        session.confirm(self).await
    }

    async fn drive_continuation(&self, primary_bytes: Vec<u8>) -> Result<(), PasskeyError> {
        let mut session = self
            .passkey_state
            .lock()
            .await
            .session
            .take()
            .ok_or_else(|| PasskeyError::Flow("continuation without an active session".into()))?;
        let confirmation = session.on_primary_identity(self, &primary_bytes).await?;
        let skip = confirmation.skip_handoff_ux;

        // Restore the session BEFORE publishing the event, so a synchronous listener
        // that confirms from it sees an active session.
        self.passkey_state.lock().await.session = Some(session);
        self.core
            .event_bus
            .dispatch(Event::PairPasskeyConfirmation(confirmation));

        // Re-link: continuity is already proven, so finish without a user code when
        // an authenticator is driving.
        if skip && self.passkey_authenticator().await.is_some() {
            self.send_passkey_confirmation().await?;
        }
        Ok(())
    }
}

/// Handle a `passkey_prologue_request` notification: emit the request (and
/// auto-drive it if an authenticator is registered).
pub(crate) async fn handle_passkey_notification(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    // The staged rotation is security-sensitive: only honor a server request.
    if node
        .get()
        .get_attr("from")
        .is_none_or(|v| v.as_str() != SERVER_JID)
    {
        warn!("ignoring passkey notification from a non-server JID");
        return;
    }

    match child_payload(node.get(), TAG_PASSKEY_REQUEST_OPTIONS)
        .and_then(|b| String::from_utf8(b).ok())
    {
        Some(json) => drive_passkey_request(client, json).await,
        // Options omitted: fetch them via IQ. Spawned because it awaits a round-trip.
        None => {
            let client = client.clone();
            client
                .clone()
                .runtime
                .spawn(Box::pin(async move {
                    match fetch_request_options(client.as_ref()).await {
                        Ok(json) => drive_passkey_request(&client, json).await,
                        Err(e) => {
                            warn!("failed to fetch passkey request options: {e}");
                            client.core.event_bus.dispatch(Event::PairPasskeyError(
                                PairPasskeyError::builder()
                                    .error(e.to_string())
                                    .continuation(false)
                                    .build(),
                            ));
                        }
                    }
                }))
                .detach();
        }
    }
}

async fn drive_passkey_request(client: &Arc<Client>, options_json: String) {
    // The handoff key suppresses the verification-code UX, so it must be a RE-LINK
    // signal only: adv_secret_key alone is present on a fresh device too, which would
    // disable the code check on a first link. Gate on a prior linked identity.
    let snapshot = client.persistence_manager.get_device_snapshot();
    let previously_linked =
        snapshot.account.is_some() || snapshot.pn.is_some() || snapshot.lid.is_some();
    let handoff_key = if previously_linked {
        ShortcakeUtils::derive_pairing_handoff_hmac_key(&snapshot.adv_secret_key)
            .inspect_err(|e| warn!("failed to derive pairing-handoff key: {e}"))
            .ok()
    } else {
        None
    };
    client.passkey_state.lock().await.handoff_key = handoff_key;

    client.core.event_bus.dispatch(Event::PairPasskeyRequest(
        PairPasskeyRequest::builder()
            .request_options_json(options_json.clone())
            .build(),
    ));

    if let Some(authenticator) = client.passkey_authenticator().await {
        let client = client.clone();
        client
            .clone()
            .runtime
            .spawn(Box::pin(async move {
                if let Err(e) = auto_drive_response(&client, authenticator, &options_json).await {
                    warn!("passkey auto-drive failed: {e}");
                    client.core.event_bus.dispatch(Event::PairPasskeyError(
                        PairPasskeyError::builder()
                            .error(e.to_string())
                            .continuation(false)
                            .build(),
                    ));
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

/// Handle a `crsc_continuation` notification. Spawned: it awaits an IQ round-trip
/// and must not block the receive loop.
pub(crate) async fn handle_passkey_continuation(client: &Arc<Client>, node: Arc<OwnedNodeRef>) {
    if node
        .get()
        .get_attr("from")
        .is_none_or(|v| v.as_str() != SERVER_JID)
    {
        warn!("ignoring passkey continuation from a non-server JID");
        return;
    }

    let primary_bytes = match child_payload(node.get(), TAG_PRIMARY_EPHEMERAL_IDENTITY) {
        Some(bytes) => bytes,
        None => {
            warn!("passkey continuation missing primary_ephemeral_identity");
            client.core.event_bus.dispatch(Event::PairPasskeyError(
                PairPasskeyError::builder()
                    .error("missing primary_ephemeral_identity".into())
                    .continuation(true)
                    .build(),
            ));
            return;
        }
    };

    let client = client.clone();
    client
        .clone()
        .runtime
        .spawn(Box::pin(async move {
            if let Err(e) = client.drive_continuation(primary_bytes).await {
                warn!("passkey continuation failed: {e}");
                client.core.event_bus.dispatch(Event::PairPasskeyError(
                    PairPasskeyError::builder()
                        .error(e.to_string())
                        .continuation(true)
                        .build(),
                ));
            }
        }))
        .detach();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{TestEventCollector, create_test_client, node_to_owned_ref};
    use crate::types::events::EventHandler;
    use buffa::Message as _;
    use std::sync::Mutex;
    use std::time::Duration;
    use wacore::libsignal::protocol::PublicKey;
    use waproto::whatsapp as wa;

    fn server_notification(notif_type: &'static str, child: Option<Node>) -> Arc<OwnedNodeRef> {
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

    // Scripted IQ stand-in: answers each `md` IQ by its child tag and records the
    // child that was sent, plus the committed secret.
    struct MockIo {
        device: DeviceMaterial,
        pairing_ref: String,
        options_json: String,
        sent: Mutex<Vec<Node>>,
        committed: Mutex<Option<[u8; 32]>>,
    }

    impl MockIo {
        fn sent_tags(&self) -> Vec<String> {
            self.sent
                .lock()
                .unwrap()
                .iter()
                .map(|n| n.tag.to_string())
                .collect()
        }

        fn sent_node(&self, tag: &str) -> Node {
            self.sent
                .lock()
                .unwrap()
                .iter()
                .find(|n| n.tag == tag)
                .cloned()
                .unwrap_or_else(|| panic!("expected a {tag} IQ to have been sent"))
        }
    }

    #[async_trait]
    impl ShortcakeIo for MockIo {
        async fn query(
            &self,
            query: InfoQuery<'static>,
        ) -> Result<Arc<OwnedNodeRef>, PasskeyError> {
            let child = match &query.content {
                Some(NodeContent::Nodes(nodes)) => nodes.first().cloned(),
                _ => None,
            };
            let child = child.expect("md IQ must carry a child node");
            let tag = child.tag.to_string();
            self.sent.lock().unwrap().push(child);

            let response = if tag == TAG_REF {
                NodeBuilder::new("iq")
                    .children([NodeBuilder::new(TAG_REF)
                        .bytes(self.pairing_ref.as_bytes().to_vec())
                        .build()])
                    .build()
            } else if tag == TAG_PASSKEY_REQUEST_OPTIONS {
                NodeBuilder::new("iq")
                    .children([NodeBuilder::new(TAG_PASSKEY_REQUEST_OPTIONS)
                        .bytes(self.options_json.as_bytes().to_vec())
                        .build()])
                    .build()
            } else {
                NodeBuilder::new("iq").build()
            };
            Ok(node_to_owned_ref(&response))
        }

        fn device_material(&self) -> Result<DeviceMaterial, PasskeyError> {
            Ok(self.device.clone())
        }

        async fn commit_adv_secret(&self, secret: [u8; 32]) {
            *self.committed.lock().unwrap() = Some(secret);
        }
    }

    fn child_bytes(node: &Node, tag: &str) -> Vec<u8> {
        child_payload(&node.as_node_ref(), tag).unwrap_or_else(|| panic!("missing {tag} child"))
    }

    #[tokio::test]
    async fn full_handshake_drives_the_iq_sequence_and_delivers_the_committed_secret() {
        // Companion static keys (arbitrary but distinct) + a primary playing the peer.
        let device = DeviceMaterial {
            noise_public: [0x11; 32],
            identity_public: [0x12; 32],
            device_type: wa::device_props::PlatformType::CHROME,
        };
        let io = MockIo {
            device: device.clone(),
            pairing_ref: "REF-XYZ".to_string(),
            options_json: "{}".to_string(),
            sent: Mutex::new(Vec::new()),
            committed: Mutex::new(None),
        };

        // A re-link: pass a handoff key so the prologue carries the proof.
        let handoff_key = [0x55u8; 32];
        let assertion = Assertion {
            assertion_json: br#"{"type":"public-key"}"#.to_vec(),
            credential_id: b"cred-id".to_vec(),
        };

        let mut session = ShortcakeSession::open(&io, assertion, Some(handoff_key))
            .await
            .unwrap();

        // step 1-2: ref fetched, then a prologue with the handoff proof.
        assert_eq!(io.sent_tags(), vec![TAG_REF, TAG_PASSKEY_PROLOGUE]);
        let prologue = io.sent_node(TAG_PASSKEY_PROLOGUE);
        assert_eq!(child_bytes(&prologue, TAG_CREDENTIAL_ID), b"cred-id");
        assert!(
            prologue
                .as_node_ref()
                .get_optional_child(TAG_PAIRING_HANDOFF_PROOF)
                .is_some()
        );

        // primary's ephemeral identity
        let primary_kp = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let primary_pub: [u8; 32] = primary_kp.public_key.public_key_bytes().try_into().unwrap();
        let primary_nonce = [0x77u8; 32];
        let primary_bytes = wa::PrimaryEphemeralIdentity {
            public_key: Some(primary_pub.to_vec()),
            nonce: Some(primary_nonce.to_vec()),
        }
        .encode_to_vec();

        // step 3: continuation sends the companion nonce and yields the code.
        let confirmation = session
            .on_primary_identity(&io, &primary_bytes)
            .await
            .unwrap();
        assert!(
            confirmation.skip_handoff_ux,
            "re-link with a handoff proof skips the code UX"
        );
        assert_eq!(confirmation.code.len(), 9, "code is grouped XXXX-XXXX");
        assert_eq!(
            io.sent_tags().last().map(String::as_str),
            Some(TAG_COMPANION_NONCE)
        );

        // step 4: confirm seals + sends the pairing request and commits the secret.
        session.confirm(&io).await.unwrap();
        assert_eq!(
            io.sent_tags().last().map(String::as_str),
            Some(TAG_ENCRYPTED_PAIRING_REQUEST)
        );
        let committed = io
            .committed
            .lock()
            .unwrap()
            .expect("secret must be committed");

        // The primary decrypts the pairing request and reads the SAME secret that
        // was committed — proving the deferred rotation delivers what it persists.
        let prologue_payload = child_bytes(&prologue, TAG_PROLOGUE_PAYLOAD);
        let companion_eph_pub = wa::CompanionEphemeralIdentity::decode_from_slice(
            wa::ProloguePayload::decode_from_slice(prologue_payload.as_slice())
                .unwrap()
                .companion_ephemeral_identity
                .unwrap()
                .as_slice(),
        )
        .unwrap()
        .public_key
        .unwrap();
        let shared = primary_kp
            .private_key
            .calculate_agreement(&PublicKey::from_djb_public_key_bytes(&companion_eph_pub).unwrap())
            .unwrap();
        let key = ShortcakeUtils::derive_encryption_key_from_shared_secret(
            &shared,
            wa::device_props::PlatformType::CHROME,
            "REF-XYZ",
        )
        .unwrap();
        let wrapped = match io.sent_node(TAG_ENCRYPTED_PAIRING_REQUEST).content {
            Some(NodeContent::Bytes(bytes)) => bytes,
            _ => panic!("encrypted_pairing_request must carry bytes"),
        };
        let epr = wa::EncryptedPairingRequest::decode_from_slice(wrapped.as_slice()).unwrap();
        let iv: [u8; 12] = epr.iv.unwrap().as_slice().try_into().unwrap();
        let mut plaintext = Vec::new();
        wacore::libsignal::crypto::aes_256_gcm_decrypt(
            &key,
            &iv,
            b"",
            &epr.encrypted_payload.unwrap(),
            &mut plaintext,
        )
        .unwrap();
        let pr = wa::PairingRequest::decode_from_slice(plaintext.as_slice()).unwrap();
        assert_eq!(pr.adv_secret.as_deref(), Some(&committed[..]));
        assert_eq!(
            pr.companion_public_key.as_deref(),
            Some(&device.noise_public[..])
        );
    }

    #[tokio::test]
    async fn premature_confirmation_keeps_the_session() {
        let client = create_test_client().await;
        // A session that hasn't reached the confirmation stage yet.
        client.passkey_state.lock().await.session = Some(ShortcakeSession {
            keypair: KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>()),
            companion_nonce: [0; 32],
            pairing_ref: "r".into(),
            device_type: wa::device_props::PlatformType::CHROME,
            new_adv_secret: [1; 32],
            skip_handoff_ux: false,
            stage: Stage::AwaitingPrimaryIdentity,
            encryption_key: None,
        });

        assert!(
            client.send_passkey_confirmation().await.is_err(),
            "confirming before the verification stage errors"
        );
        assert!(
            client.passkey_state.lock().await.session.is_some(),
            "a premature confirmation must not drop the in-flight attempt"
        );
    }

    #[tokio::test]
    async fn cancelled_open_releases_the_reservation() {
        let client = create_test_client().await;
        client.passkey_opening.store(true, Ordering::Release);
        // A dropped guard stands in for a send_passkey_response cancelled mid-open;
        // the release is a sync store, so it holds even under lock contention.
        drop(OpeningGuard {
            flag: &client.passkey_opening,
        });
        assert!(
            !client.passkey_opening.load(Ordering::Acquire),
            "a cancelled open must release the reservation"
        );
    }

    #[tokio::test]
    async fn passkey_prologue_request_emits_event_without_committing_rotation() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone() as Arc<dyn EventHandler>);

        let before = client
            .persistence_manager
            .get_device_snapshot()
            .adv_secret_key;

        let options = r#"{"challenge":"YWJjZGVm","rpId":"web.whatsapp.com"}"#;
        let child = NodeBuilder::new(TAG_PASSKEY_REQUEST_OPTIONS)
            .bytes(options.as_bytes().to_vec())
            .build();
        client
            .process_node(server_notification(NOTIF_PASSKEY_REQUEST, Some(child)))
            .await;

        // The rotation is deferred to confirmation, so the stored secret is unchanged.
        let after = client
            .persistence_manager
            .get_device_snapshot()
            .adv_secret_key;
        assert_eq!(before, after, "ADV secret must not commit at request time");

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

        let child = NodeBuilder::new(TAG_PASSKEY_REQUEST_OPTIONS)
            .bytes(b"{}".to_vec())
            .build();
        let node = NodeBuilder::new("notification")
            .attr("type", NOTIF_PASSKEY_REQUEST)
            .attr("from", "12345@s.whatsapp.net")
            .children([child])
            .build();
        client.process_node(node_to_owned_ref(&node)).await;

        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(e.as_ref(), Event::PairPasskeyRequest(_))),
            "no event for a non-server request"
        );
    }

    #[tokio::test]
    async fn passkey_prologue_request_without_inline_options_falls_back_to_fetch() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone() as Arc<dyn EventHandler>);

        // No inline options: the handler falls back to an IQ fetch. The test client
        // isn't connected, so the fetch fails and surfaces a non-continuation error.
        client
            .process_node(server_notification(NOTIF_PASSKEY_REQUEST, None))
            .await;

        wait_for(&collector, |e| {
            matches!(
                e,
                Event::PairPasskeyError(err)
                    if !err.continuation && err.error.contains("passkey_request_options iq failed")
            )
        })
        .await;
    }

    #[tokio::test]
    async fn passkey_continuation_without_session_emits_error() {
        let client = create_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.register_handler(collector.clone() as Arc<dyn EventHandler>);

        let primary = wa::PrimaryEphemeralIdentity {
            public_key: Some(vec![0xAB; 32]),
            nonce: Some(vec![0xCD; 32]),
        };
        let child = NodeBuilder::new(TAG_PRIMARY_EPHEMERAL_IDENTITY)
            .bytes(buffa::Message::encode_to_vec(&primary))
            .build();
        client
            .process_node(server_notification(NOTIF_PASSKEY_CONTINUATION, Some(child)))
            .await;

        wait_for(
            &collector,
            |e| matches!(e, Event::PairPasskeyError(err) if err.continuation),
        )
        .await;
    }

    #[test]
    fn prologue_node_wire_shape() {
        let node = build_prologue_node(
            b"cred-id".to_vec(),
            b"{\"type\":\"public-key\"}".to_vec(),
            b"prologue-proto".to_vec(),
            Some([0x42; 32]),
        );
        let nr = node.as_node_ref();
        assert_eq!(nr.tag.as_ref(), TAG_PASSKEY_PROLOGUE);
        assert_eq!(
            nr.get_optional_child(TAG_CREDENTIAL_ID)
                .and_then(|n| n.content_bytes()),
            Some(&b"cred-id"[..])
        );
        assert_eq!(
            nr.get_optional_child(TAG_PAIRING_HANDOFF_PROOF)
                .and_then(|n| n.content_bytes()),
            Some(&[0x42u8; 32][..])
        );

        let fresh = build_prologue_node(b"c".to_vec(), b"a".to_vec(), b"p".to_vec(), None);
        assert!(
            fresh
                .as_node_ref()
                .get_optional_child(TAG_PAIRING_HANDOFF_PROOF)
                .is_none()
        );
    }

    /// A fresh device (never linked) must NOT derive a handoff key, so
    /// skip_handoff_ux stays false and the verification-code check runs.
    #[tokio::test]
    async fn fresh_link_does_not_derive_a_handoff_key() {
        let client = create_test_client().await;
        drive_passkey_request(&client, "{}".to_string()).await;
        assert!(
            client.passkey_state.lock().await.handoff_key.is_none(),
            "a fresh link must leave handoff_key None (verification-code UX stays on)"
        );
    }

    /// A re-link (a prior identity is present) derives the handoff key, which is
    /// what legitimately lets the server skip the verification-code UX.
    #[tokio::test]
    async fn relink_derives_a_handoff_key() {
        let client = create_test_client().await;
        let pn: Jid = "15551230000:1@s.whatsapp.net".parse().unwrap();
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetId(Some(pn)))
            .await;
        drive_passkey_request(&client, "{}".to_string()).await;
        assert!(
            client.passkey_state.lock().await.handoff_key.is_some(),
            "a re-link with a prior identity must derive the handoff key"
        );
    }
}
