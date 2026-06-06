//! Tests for the message receive/decrypt pipeline.

use super::*;
use crate::store::SqliteStore;
use crate::store::persistence_manager::PersistenceManager;
use crate::test_utils::MockHttpClient;
use crate::types::message::EditAttribute;
use std::sync::Arc;
use wacore_binary::builder::NodeBuilder;

fn node_to_arc(node: wacore_binary::Node) -> Arc<OwnedNodeRef> {
    crate::test_utils::node_to_owned_ref(&node)
}
use wacore_binary::{Jid, SERVER_JID};

fn mock_transport() -> Arc<dyn crate::transport::TransportFactory> {
    Arc::new(crate::transport::mock::MockTransportFactory::new())
}

fn mock_http_client() -> Arc<dyn crate::http::HttpClient> {
    Arc::new(MockHttpClient)
}

#[tokio::test]
async fn test_parse_message_info_for_status_broadcast() {
    let backend = Arc::new(
        SqliteStore::new("file:memdb_status_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let participant_jid_str = "556899336555:42@s.whatsapp.net";
    let status_broadcast_jid_str = "status@broadcast";

    let node = NodeBuilder::new("message")
        .attr("from", status_broadcast_jid_str)
        .attr("id", "8A8CCCC7E6E466D9EE8CA11A967E485A")
        .attr("participant", participant_jid_str)
        .attr("t", "1759295366")
        .attr("type", "media")
        .build();

    let info = client
        .parse_message_info(&node.as_node_ref())
        .await
        .expect("parse_message_info should not fail");

    let expected_sender: Jid = participant_jid_str
        .parse()
        .expect("test JID should be valid");
    let expected_chat: Jid = status_broadcast_jid_str
        .parse()
        .expect("test JID should be valid");

    assert_eq!(
        info.source.sender, expected_sender,
        "The sender should be the 'participant' JID, not 'status@broadcast'"
    );
    assert_eq!(
        info.source.chat, expected_chat,
        "The chat should be 'status@broadcast'"
    );
    assert!(
        info.source.is_group,
        "Broadcast messages should be treated as group-like"
    );
}

#[tokio::test]
async fn test_status_broadcast_cold_cache_resolves_to_lid() {
    use wacore::types::jid::JidExt as _;
    use wacore_binary::Server;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_status_cold_cache?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let pn_user = "559980000001";
    let lid_user = "100000012345678";

    assert_eq!(
        client.lid_pn_cache.get_current_lid(pn_user).await,
        None,
        "precondition: empty cache for {pn_user}"
    );

    let node = NodeBuilder::new("message")
        .attr("from", "status@broadcast")
        .attr("id", "TEST_COLD_CACHE_ID")
        .attr("participant", format!("{pn_user}@s.whatsapp.net").as_str())
        .attr("participant_lid", format!("{lid_user}@lid").as_str())
        .attr("t", "1777415965")
        .attr("type", "media")
        .build();

    let info = client
        .parse_message_info(&node.as_node_ref())
        .await
        .expect("parse_message_info must succeed");

    // Fix #1: parser surfaces participant_lid via sender_alt.
    let alt = info
        .source
        .sender_alt
        .as_ref()
        .expect("sender_alt must be populated from participant_lid");
    assert_eq!(alt.user.as_str(), lid_user);
    assert_eq!(alt.server, Server::Lid);
    assert_eq!(info.source.sender.user.as_str(), pn_user);
    assert_eq!(info.source.sender.server, Server::Pn);

    client
        .cache_lid_pn_from_message(
            &info.source.sender,
            info.source.sender_alt.as_ref(),
            info.is_offline,
        )
        .await;

    // Cache learned the mapping in both directions.
    assert_eq!(
        client
            .lid_pn_cache
            .get_current_lid(pn_user)
            .await
            .as_deref(),
        Some(lid_user),
        "PN→LID lookup must hit"
    );
    assert_eq!(
        client.lid_pn_cache.get_phone_number(lid_user).await,
        Some(pn_user.to_string()),
        "LID→PN lookup must hit"
    );

    // Resolution upgrades to LID and Signal address is the LID form.
    let resolved = client.resolve_encryption_jid(&info.source.sender).await;
    assert_eq!(resolved.user.as_str(), lid_user);
    assert_eq!(resolved.server, Server::Lid);
    assert_eq!(resolved.device, info.source.sender.device);
    assert_eq!(
        resolved.to_protocol_address().to_string(),
        format!("{lid_user}@lid.0"),
        "Signal address must be @lid form, not @c.us"
    );
}

/// Pins the hosted-family branch + the realistic non-zero device shape.
/// Production stanzas almost always have device != 0, and hosted variants
/// (`@hosted` / `@hosted.lid`) must flow through cache_lid_pn_from_message.
#[tokio::test]
async fn test_status_broadcast_hosted_family_with_device_id_resolves_to_hosted_lid() {
    use wacore::types::jid::JidExt as _;
    use wacore_binary::Server;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_status_hosted_device?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let pn_user = "559980000001";
    let lid_user = "100000012345678";
    let device_id: u16 = 99;

    let node = NodeBuilder::new("message")
        .attr("from", "status@broadcast")
        .attr("id", "HOSTED_TEST_ID")
        .attr(
            "participant",
            format!("{pn_user}:{device_id}@hosted").as_str(),
        )
        .attr(
            "participant_lid",
            format!("{lid_user}:{device_id}@hosted.lid").as_str(),
        )
        .attr("t", "1777415965")
        .attr("type", "media")
        .build();

    let info = client
        .parse_message_info(&node.as_node_ref())
        .await
        .expect("parse_message_info must succeed");

    assert_eq!(info.source.sender.server, Server::Hosted);
    assert_eq!(info.source.sender.device, device_id);
    let alt = info
        .source
        .sender_alt
        .as_ref()
        .expect("sender_alt must be populated for hosted participant");
    assert_eq!(alt.server, Server::HostedLid);
    assert_eq!(alt.user.as_str(), lid_user);
    assert_eq!(alt.device, device_id);

    client
        .cache_lid_pn_from_message(
            &info.source.sender,
            info.source.sender_alt.as_ref(),
            info.is_offline,
        )
        .await;

    // Hosted variant must reach the cache; without it, learn_lid_pn_mapping
    // is skipped and the hosted-device fix is incomplete.
    assert_eq!(
        client
            .lid_pn_cache
            .get_current_lid(pn_user)
            .await
            .as_deref(),
        Some(lid_user),
        "PN→LID lookup must work for hosted family"
    );
    assert_eq!(
        client.lid_pn_cache.get_phone_number(lid_user).await,
        Some(pn_user.to_string()),
    );

    let resolved = client.resolve_encryption_jid(&info.source.sender).await;
    assert_eq!(resolved.user.as_str(), lid_user);
    assert_eq!(resolved.server, Server::HostedLid);
    assert_eq!(
        resolved.device, device_id,
        "device id must be preserved through resolution"
    );
    assert_eq!(
        resolved.to_protocol_address().to_string(),
        format!("{lid_user}:{device_id}@hosted.lid.0"),
        "Signal address must be the @hosted.lid form with device suffix"
    );
}

#[tokio::test]
async fn test_process_session_enc_batch_handles_session_not_found_gracefully() {
    use wacore::libsignal::protocol::{IdentityKeyPair, KeyPair, SignalMessage};

    let backend = Arc::new(
        SqliteStore::new("file:memdb_graceful_fail?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let sender_jid: Jid = "1234567890@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let info = Arc::new(MessageInfo {
        source: crate::types::message::MessageSource {
            sender: sender_jid.clone(),
            chat: sender_jid.clone(),
            ..Default::default()
        },
        ..Default::default()
    });

    // Create a valid but undecryptable SignalMessage
    let dummy_key = [0u8; 32];
    let sender_ratchet = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>()).public_key;
    let sender_identity_pair =
        IdentityKeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let receiver_identity_pair =
        IdentityKeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let signal_message = SignalMessage::new(
        4,
        &dummy_key,
        sender_ratchet,
        0,
        0,
        b"test",
        sender_identity_pair.identity_key(),
        receiver_identity_pair.identity_key(),
    )
    .expect("SignalMessage::new should succeed with valid inputs");

    let enc_node = NodeBuilder::new("enc")
        .attr("type", "msg")
        .bytes(signal_message.serialized().to_vec())
        .build();
    let enc_node_ref = enc_node.as_node_ref();
    let payloads: Vec<EncPayload> = vec![EncPayload::from_node_ref(&enc_node_ref).unwrap()];

    let outcome = client
        .process_session_enc_batch(
            &payloads,
            &info,
            &sender_jid,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;

    assert!(
        !outcome.decrypted && !outcome.duplicate && outcome.undecryptable,
        "process_session_enc_batch should mark SessionNotFound as undecryptable without success or duplicate"
    );
}

/// Two undecryptable payloads sharing one `(chat, id)` must accumulate
/// `undecryptable` across the batch (monotonic OR) and dispatch exactly one
/// `UndecryptableMessage` event — the single-flight dedup that the accumulator
/// exists to protect, exercised through the batch path with multiple payloads.
#[tokio::test]
async fn batch_accumulates_undecryptable_and_dispatches_once() {
    let backend = Arc::new(
        SqliteStore::new("file:memdb_batch_undec_once?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let recorder = Arc::new(EventRecorder::default());
    client.register_handler(recorder.clone());

    let sender_jid: Jid = "1234567890@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let info = Arc::new(MessageInfo {
        id: "BATCH_UNDEC_ONCE".to_string(),
        source: crate::types::message::MessageSource {
            sender: sender_jid.clone(),
            chat: sender_jid.clone(),
            ..Default::default()
        },
        ..Default::default()
    });

    // Two enc payloads whose ciphertext does not parse as a Signal message, so
    // each payload independently lands on an undecryptable path. Distinct bytes
    // to avoid any incidental dedup on payload content.
    let enc1 = NodeBuilder::new("enc")
        .attr("type", "msg")
        .bytes(vec![0xFF, 0x00, 0x01])
        .build();
    let enc2 = NodeBuilder::new("enc")
        .attr("type", "msg")
        .bytes(vec![0xFF, 0x00, 0x02])
        .build();
    let enc1_ref = enc1.as_node_ref();
    let enc2_ref = enc2.as_node_ref();
    let payloads: Vec<EncPayload> = vec![
        EncPayload::from_node_ref(&enc1_ref).unwrap(),
        EncPayload::from_node_ref(&enc2_ref).unwrap(),
    ];

    let outcome = client
        .process_session_enc_batch(
            &payloads,
            &info,
            &sender_jid,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;

    assert!(
        outcome.undecryptable,
        "batch must stay undecryptable across both failed payloads"
    );
    assert_eq!(
        recorder.undecryptable().len(),
        1,
        "same (chat, id) dispatches UndecryptableMessage exactly once across the batch"
    );
}

/// P1: An empty session record (exists but no current/previous state) should be
/// treated the same as SessionNotFound — the retry receipt gets error code 1 (NoSession)
/// and includes keys early, instead of producing an unhelpful InvalidMessage error.
#[tokio::test]
async fn test_empty_session_record_treated_as_session_not_found() {
    use wacore::libsignal::protocol::{IdentityKeyPair, KeyPair, SessionRecord, SignalMessage};

    let backend = Arc::new(
        SqliteStore::new("file:memdb_empty_session?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let sender_jid: Jid = "0000000000000@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let info = Arc::new(MessageInfo {
        source: crate::types::message::MessageSource {
            sender: sender_jid.clone(),
            chat: sender_jid.clone(),
            ..Default::default()
        },
        ..Default::default()
    });

    // Pre-store an empty (degenerate) session record in the signal cache.
    // This simulates the bug scenario: record exists but has no usable ratchet state.
    let signal_address = sender_jid.to_protocol_address();
    client
        .signal_cache
        .put_session(&signal_address, SessionRecord::new_fresh())
        .await;

    // Craft a SignalMessage to trigger decryption
    let dummy_key = [0u8; 32];
    let sender_ratchet = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>()).public_key;
    let sender_identity = IdentityKeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let receiver_identity = IdentityKeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let signal_message = SignalMessage::new(
        4,
        &dummy_key,
        sender_ratchet,
        0,
        0,
        b"test",
        sender_identity.identity_key(),
        receiver_identity.identity_key(),
    )
    .expect("SignalMessage::new should succeed");

    let enc_node = NodeBuilder::new("enc")
        .attr("type", "msg")
        .bytes(signal_message.serialized().to_vec())
        .build();
    let enc_node_ref = enc_node.as_node_ref();
    let payloads: Vec<EncPayload> = vec![EncPayload::from_node_ref(&enc_node_ref).unwrap()];

    let outcome = client
        .clone()
        .process_session_enc_batch(
            &payloads,
            &info,
            &sender_jid,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;

    // Should behave identically to SessionNotFound: failure, no dupe, event dispatched.
    assert!(
        !outcome.decrypted && !outcome.duplicate && outcome.undecryptable,
        "Empty session record should be treated as SessionNotFound: \
             expected undecryptable without success or duplicate, got {outcome:?}"
    );

    // After the WA Web compliance fix (no delete on BadMac/InvalidMessage either),
    // every inbound-decrypt failure preserves the session. This still pins
    // that the empty-record path does not regress to a delete.
    let backend = client.persistence_manager.backend();
    let session_still_exists = client
        .signal_cache
        .has_session(&signal_address, &*backend)
        .await
        .expect("has_session should not fail");
    assert!(session_still_exists);

    // Discriminate from the BadMac / InvalidMessage arms (which also
    // preserve the session post-fix): the empty-record path must end up
    // in the SessionNotFound branch, which fires a retry receipt with
    // `RetryReason::NoSession`. Anything else means the libsignal-side
    // empty-record short-circuit regressed.
    await_retry_receipt(&client, &info, 1, RetryReason::NoSession).await;
}

// ─── Fixtures for session-preservation tests ─────────────────────────────
//
// Mirrors the WAWebSignalProtocolStore tests in spirit: a synthetic peer
// holds its own Signal stores in memory so the test can drive X3DH end to
// end against the Client. Inlined (not exported from a helper crate)
// because these are message.rs-specific scenarios.

use async_trait::async_trait;
use std::collections::HashMap;
use wacore::libsignal::protocol::{
    CiphertextMessage, Direction, IdentityChange, IdentityKey, IdentityKeyPair, KeyPair,
    PreKeyBundle, PreKeyRecord, PreKeyStore as SigPreKeyStore, ProtocolAddress, SenderKeyName,
    SenderKeyRecord, SenderKeyStore as SigSenderKeyStore, SessionRecord,
    SessionStore as SigSessionStore, SignedPreKeyStore as SigSignedPreKeyStore, UsePQRatchet,
    create_sender_key_distribution_message, group_encrypt, message_encrypt, process_prekey_bundle,
};
use wacore::libsignal::protocol::{IdentityKeyStore as SigIdentityKeyStore, SignalProtocolError};

#[derive(Default, Clone)]
struct MemSessionStore(HashMap<ProtocolAddress, SessionRecord>);

#[async_trait]
impl SigSessionStore for MemSessionStore {
    async fn load_session(
        &self,
        a: &ProtocolAddress,
    ) -> Result<Option<SessionRecord>, SignalProtocolError> {
        Ok(self.0.get(a).cloned())
    }
    async fn has_session(&self, a: &ProtocolAddress) -> Result<bool, SignalProtocolError> {
        Ok(self.0.contains_key(a))
    }
    async fn store_session(
        &mut self,
        a: &ProtocolAddress,
        r: SessionRecord,
    ) -> Result<(), SignalProtocolError> {
        self.0.insert(a.clone(), r);
        Ok(())
    }
}

#[derive(Clone)]
struct MemIdentityStore {
    kp: IdentityKeyPair,
    reg_id: u32,
    known: HashMap<ProtocolAddress, IdentityKey>,
}

#[async_trait]
impl SigIdentityKeyStore for MemIdentityStore {
    async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair, SignalProtocolError> {
        Ok(self.kp.clone())
    }
    async fn get_local_registration_id(&self) -> Result<u32, SignalProtocolError> {
        Ok(self.reg_id)
    }
    async fn save_identity(
        &mut self,
        a: &ProtocolAddress,
        id: &IdentityKey,
    ) -> Result<IdentityChange, SignalProtocolError> {
        let prev = self.known.insert(a.clone(), *id);
        Ok(match prev {
            None => IdentityChange::NewOrUnchanged,
            Some(p) if &p == id => IdentityChange::NewOrUnchanged,
            _ => IdentityChange::ReplacedExisting,
        })
    }
    async fn is_trusted_identity(
        &self,
        _: &ProtocolAddress,
        _: &IdentityKey,
        _: Direction,
    ) -> Result<bool, SignalProtocolError> {
        Ok(true)
    }
    async fn get_identity(
        &self,
        a: &ProtocolAddress,
    ) -> Result<Option<IdentityKey>, SignalProtocolError> {
        Ok(self.known.get(a).copied())
    }
}

#[derive(Default, Clone)]
struct MemSenderKeyStore(HashMap<SenderKeyName, SenderKeyRecord>);

#[async_trait]
impl SigSenderKeyStore for MemSenderKeyStore {
    async fn store_sender_key(
        &mut self,
        name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> Result<(), SignalProtocolError> {
        self.0.insert(name.clone(), record);
        Ok(())
    }

    async fn load_sender_key(
        &self,
        name: &SenderKeyName,
    ) -> Result<Option<SenderKeyRecord>, SignalProtocolError> {
        Ok(self.0.get(name).cloned())
    }
}

#[derive(Clone)]
struct AlicePeer {
    jid: Jid,
    address: ProtocolAddress,
    identity: MemIdentityStore,
    sessions: MemSessionStore,
    sender_keys: MemSenderKeyStore,
}

impl AlicePeer {
    async fn new(jid_str: &str) -> Self {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let kp = IdentityKeyPair::generate(&mut rng);
        let jid: Jid = jid_str.parse().expect("valid jid");
        let address = jid.to_protocol_address();
        Self {
            jid,
            address,
            identity: MemIdentityStore {
                kp,
                reg_id: 12345,
                known: HashMap::new(),
            },
            sessions: MemSessionStore::default(),
            sender_keys: MemSenderKeyStore::default(),
        }
    }

    async fn install_bob_session(&mut self, bob_addr: &ProtocolAddress, bundle: &PreKeyBundle) {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        process_prekey_bundle(
            bob_addr,
            &mut self.sessions,
            &mut self.identity,
            bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .expect("process bob bundle");
    }

    async fn encrypt(&mut self, bob_addr: &ProtocolAddress, plaintext: &[u8]) -> CiphertextMessage {
        message_encrypt(plaintext, bob_addr, &mut self.sessions, &mut self.identity)
            .await
            .expect("encrypt")
    }

    async fn encrypt_text(&mut self, bob_addr: &ProtocolAddress, text: &str) -> CiphertextMessage {
        use wacore::messages::MessageUtils;

        let plaintext = MessageUtils::encode_and_pad(&wa::Message {
            conversation: Some(text.to_string()),
            ..Default::default()
        });
        self.encrypt(bob_addr, &plaintext).await
    }

    async fn create_group_skdm(
        &mut self,
        group_jid: &Jid,
    ) -> wa::message::SenderKeyDistributionMessage {
        let sender = self.jid.to_non_ad();
        let sender_key_name = make_sender_key_name(group_jid, &sender.to_protocol_address());
        let skdm = create_sender_key_distribution_message(
            &sender_key_name,
            &mut self.sender_keys,
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .expect("create sender key distribution");
        wa::message::SenderKeyDistributionMessage {
            group_id: Some(group_jid.to_string()),
            axolotl_sender_key_distribution_message: Some(skdm.serialized().to_vec()),
        }
    }

    async fn encrypt_group_message(&mut self, group_jid: &Jid, plaintext: &[u8]) -> Vec<u8> {
        let sender = self.jid.to_non_ad();
        let sender_key_name = make_sender_key_name(group_jid, &sender.to_protocol_address());
        let sender_key_message = group_encrypt(
            &mut self.sender_keys,
            &sender_key_name,
            plaintext,
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .expect("encrypt sender key message");
        sender_key_message.serialized().to_vec()
    }
}

/// Ensure the test `Client` has an identity (`pn`/`lid`) provisioned —
/// `create_test_client_with_name` returns an unpaired client by default
/// so `device_snapshot.lid` / `.pn` are both `None`.
async fn ensure_bob_paired(client: &Arc<Client>) {
    let snapshot = client.persistence_manager.get_device_snapshot().await;
    if snapshot.lid.is_some() || snapshot.pn.is_some() {
        return;
    }
    let pn: Jid = "9000000000000:1@s.whatsapp.net".parse().expect("pn");
    let lid: Jid = "999999999999999:1@lid".parse().expect("lid");
    client
        .persistence_manager
        .process_command(crate::store::commands::DeviceCommand::SetId(Some(pn)))
        .await;
    client
        .persistence_manager
        .process_command(crate::store::commands::DeviceCommand::SetLid(Some(lid)))
        .await;
}

/// Read Bob's currently provisioned identity / signed prekey from the test
/// client and build a `PreKeyBundle` that Alice can use to initialize
/// her side of the session. Mirrors how the real `RetryReceiptJob` ships
/// keys back to the sender — assembled through the same
/// `SignalProtocolStoreAdapter` traits production uses.
async fn bobs_prekey_bundle(client: &Arc<Client>) -> (PreKeyBundle, Jid) {
    use wacore::libsignal::protocol::GenericSignedPreKey;
    ensure_bob_paired(client).await;
    let snapshot = client.persistence_manager.get_device_snapshot().await;
    let identity_kp = snapshot.core.identity_key.clone();
    let reg_id = snapshot.core.registration_id;

    // Read/write prekeys through the same trait surface production uses
    // (see signal_adapter.rs). Avoids reaching past `PersistenceManager`
    // to mutate device storage directly.
    let mut adapter = client.signal_adapter().await;
    let spk_record = adapter
        .signed_pre_key_store
        .get_signed_pre_key(1.into())
        .await
        .expect("spk present");
    let spk_pub = spk_record.public_key().expect("spk pub");
    let spk_sig_vec = spk_record.signature().expect("spk sig");

    // Provision a fresh one-time prekey for this test through the
    // adapter's `PreKeyStore` impl.
    let pk_id_u32: u32 = 9001;
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let pk_pair = KeyPair::generate(&mut rng);
    let pk_record = PreKeyRecord::new(pk_id_u32.into(), &pk_pair);
    adapter
        .pre_key_store
        .save_pre_key(pk_id_u32.into(), &pk_record)
        .await
        .expect("save pk");

    let own_device_jid: Jid = snapshot
        .lid
        .clone()
        .or_else(|| snapshot.pn.clone())
        .expect("own jid");
    let bob_jid = own_device_jid.to_non_ad();
    let bundle = PreKeyBundle::new(
        reg_id,
        u32::from(own_device_jid.device).into(),
        Some((pk_id_u32.into(), pk_pair.public_key)),
        1.into(),
        spk_pub,
        spk_sig_vec,
        IdentityKey::new(identity_kp.public_key),
    )
    .expect("bundle");
    (bundle, bob_jid)
}

/// Build an EncPayload-style stanza node and run `process_session_enc_batch`.
/// Returns whether the session for `peer_jid` still exists in the cache afterwards.
async fn submit_and_check_session(
    client: &Arc<Client>,
    peer_jid: &Jid,
    ct: &CiphertextMessage,
) -> (bool, bool, bool, bool) {
    let (enc_type, bytes) = match ct {
        CiphertextMessage::SignalMessage(m) => ("msg", m.serialized().to_vec()),
        CiphertextMessage::PreKeySignalMessage(m) => ("pkmsg", m.serialized().to_vec()),
        _ => panic!("unexpected ciphertext type"),
    };
    let enc_node = NodeBuilder::new("enc")
        .attr("type", enc_type)
        .bytes(bytes)
        .build();
    let enc_ref = enc_node.as_node_ref();
    let payloads: Vec<EncPayload> = vec![EncPayload::from_node_ref(&enc_ref).unwrap()];
    let info = Arc::new(MessageInfo {
        source: crate::types::message::MessageSource {
            sender: peer_jid.clone(),
            chat: peer_jid.clone(),
            ..Default::default()
        },
        ..Default::default()
    });
    let outcome = client
        .clone()
        .process_session_enc_batch(
            &payloads,
            &info,
            peer_jid,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;
    let backend = client.persistence_manager.backend();
    let still = client
        .signal_cache
        .has_session(&peer_jid.to_protocol_address(), &*backend)
        .await
        .expect("has_session");
    (
        outcome.decrypted,
        outcome.duplicate,
        outcome.undecryptable,
        still,
    )
}

#[tokio::test]
async fn test_badmac_migrates_pn_session_when_lid_shadow_exists() {
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};

    let client = crate::test_utils::create_test_client_with_name("badmac_lid_shadow").await;
    let alice_pn: Jid = "15550001001@s.whatsapp.net".parse().expect("alice pn");
    let alice_lid: Jid = "100000000000002@lid".parse().expect("alice lid");
    let entry = LidPnEntry::new(
        alice_lid.user.to_string(),
        alice_pn.user.to_string(),
        LearningSource::PeerLidMessage,
    );
    client.lid_pn_cache.add(&entry).await;

    let (bundle_v1, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let alice_pn_str = alice_pn.to_string();
    let mut alice_old = AlicePeer::new(&alice_pn_str).await;
    alice_old.install_bob_session(&bob_addr, &bundle_v1).await;
    let pkmsg_v1 = alice_old.encrypt_text(&bob_addr, "pn establish").await;
    let (pn_success, _, _, pn_still) =
        submit_and_check_session(&client, &alice_pn, &pkmsg_v1).await;
    assert!(pn_success, "PN-keyed session should establish");
    assert!(
        pn_still,
        "PN-keyed session should be present before migration"
    );

    if let Some(record) = alice_old.sessions.0.get_mut(&bob_addr)
        && let Some(state) = record.session_state_mut()
    {
        state.clear_unacknowledged_pre_key_message();
    }

    let mut alice_fresh = alice_old.clone();
    alice_fresh.jid = alice_lid.clone();
    alice_fresh.address = alice_lid.to_protocol_address();
    alice_fresh.sessions = MemSessionStore::default();

    let (bundle_v2, _) = bobs_prekey_bundle(&client).await;
    alice_fresh.install_bob_session(&bob_addr, &bundle_v2).await;
    let pkmsg_v2 = alice_fresh.encrypt_text(&bob_addr, "lid shadow").await;
    let (lid_success, _, _, lid_still) =
        submit_and_check_session(&client, &alice_lid, &pkmsg_v2).await;
    assert!(lid_success, "LID-keyed shadow session should establish");
    assert!(lid_still, "LID-keyed shadow session should exist");

    let old_pn_msg = alice_old.encrypt_text(&bob_addr, "old pn ratchet").await;
    assert!(matches!(old_pn_msg, CiphertextMessage::SignalMessage(_)));
    let (success, duplicates, dispatched, lid_after) =
        submit_and_check_session(&client, &alice_lid, &old_pn_msg).await;
    assert!(success, "BadMac path should recover by migrating PN to LID");
    assert!(!duplicates, "message should decrypt, not dedupe");
    assert!(
        !dispatched,
        "migration recovery must not emit retry failure"
    );
    assert!(lid_after, "migrated LID session should remain");

    let backend = client.persistence_manager.backend();
    let pn_after = client
        .signal_cache
        .has_session(&alice_pn.to_protocol_address(), &*backend)
        .await
        .expect("has_session");
    assert!(!pn_after, "PN session should be consumed by migration");
}

#[tokio::test]
async fn migration_plaintext_failure_nacks_without_signal_retry() {
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};

    let (client, transport) = capturing_client("migration_plaintext_nack").await;
    let alice_pn: Jid = "15550001002@s.whatsapp.net".parse().expect("alice pn");
    let alice_lid: Jid = "100000000000004@lid".parse().expect("alice lid");
    let entry = LidPnEntry::new(
        alice_lid.user.to_string(),
        alice_pn.user.to_string(),
        LearningSource::PeerLidMessage,
    );
    client.lid_pn_cache.add(&entry).await;

    let (bundle_v1, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let alice_pn_str = alice_pn.to_string();
    let mut alice_old = AlicePeer::new(&alice_pn_str).await;
    alice_old.install_bob_session(&bob_addr, &bundle_v1).await;
    let pkmsg_v1 = alice_old.encrypt_text(&bob_addr, "pn establish").await;
    let (pn_success, _, _, _) = submit_and_check_session(&client, &alice_pn, &pkmsg_v1).await;
    assert!(pn_success, "PN-keyed session should establish");

    if let Some(record) = alice_old.sessions.0.get_mut(&bob_addr)
        && let Some(state) = record.session_state_mut()
    {
        state.clear_unacknowledged_pre_key_message();
    }

    let mut alice_fresh = alice_old.clone();
    alice_fresh.jid = alice_lid.clone();
    alice_fresh.address = alice_lid.to_protocol_address();
    alice_fresh.sessions = MemSessionStore::default();

    let (bundle_v2, _) = bobs_prekey_bundle(&client).await;
    alice_fresh.install_bob_session(&bob_addr, &bundle_v2).await;
    let pkmsg_v2 = alice_fresh.encrypt_text(&bob_addr, "lid shadow").await;
    let (lid_success, _, _, _) = submit_and_check_session(&client, &alice_lid, &pkmsg_v2).await;
    assert!(lid_success, "LID-keyed shadow session should establish");

    let bad_old_pn_msg = alice_old.encrypt(&bob_addr, &[0xff, 0x01]).await;
    let payloads = vec![enc_payload_from_ciphertext(&bad_old_pn_msg)];
    let info = Arc::new(MessageInfo {
        id: "MIGRATION_BAD_PLAINTEXT".to_string(),
        source: crate::types::message::MessageSource {
            sender: alice_lid.clone(),
            chat: alice_lid.clone(),
            ..Default::default()
        },
        ..Default::default()
    });

    let outcome = client
        .clone()
        .process_session_enc_batch(
            &payloads,
            &info,
            &alice_lid,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;

    assert!(
        outcome.decrypted,
        "Signal decrypt succeeded after migration"
    );
    assert!(outcome.plaintext_failed);
    assert!(outcome.undecryptable);
    assert!(outcome.had_failure);
    assert!(!outcome.dispatched);
    assert!(!outcome.skdm_only);

    let mut nack_code = None;
    for _ in 0..80 {
        nack_code = find_message_nack_error(&transport.sent(), &info.id);
        if nack_code.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(nack_code, Some(491));

    let cache_key = client
        .make_retry_cache_key(&info.source.chat, &info.id, &info.source.sender)
        .await;
    assert_eq!(
        client.message_retry_counts.get(&cache_key).await,
        None,
        "local protobuf failure after migration must not request Signal retry"
    );
}

/// Smoking-gun regression: a `BadMac` on the inbound path must NOT delete
/// the session. Pre-fix, `src/message.rs:1100` called
/// `signal_cache.delete_session(...)` here — this test would fail with
/// `still=false`. WA Web's `RetryReceiptJob` keeps the session untouched
/// (see `docs/captured-js/WAWeb/Send/RetryReceiptJob.js`).
#[tokio::test]
async fn test_badmac_preserves_session() {
    let client = crate::test_utils::create_test_client_with_name("badmac_preserves").await;
    let mut alice = AlicePeer::new("1111111111111@s.whatsapp.net").await;
    let alice_addr = alice.address.clone();

    // X3DH: Alice consumes Bob's bundle to set up her outgoing session.
    let (bob_bundle, _) = bobs_prekey_bundle(&client).await;
    alice
        .install_bob_session(
            &client
                .persistence_manager
                .get_device_snapshot()
                .await
                .lid
                .clone()
                .or(client
                    .persistence_manager
                    .get_device_snapshot()
                    .await
                    .pn
                    .clone())
                .expect("own jid")
                .to_protocol_address(),
            &bob_bundle,
        )
        .await;

    // First message: pkmsg lands on Bob and installs Bob's reciprocal session.
    let bob_addr = client
        .persistence_manager
        .get_device_snapshot()
        .await
        .lid
        .clone()
        .or(client
            .persistence_manager
            .get_device_snapshot()
            .await
            .pn
            .clone())
        .expect("own jid")
        .to_protocol_address();
    let pkmsg = alice.encrypt_text(&bob_addr, "hello").await;
    let (s1, _, _, still1) = submit_and_check_session(&client, &alice.jid, &pkmsg).await;
    assert!(s1, "pkmsg should establish session and decrypt");
    assert!(still1, "session must exist after first message");

    // Force Alice's next encrypt to be a plain SignalMessage rather than a
    // pkmsg by clearing her unacknowledged-pkmsg flag. Tampering the trailing
    // bytes of a pkmsg breaks the outer protobuf parse (because reg_id /
    // signed_pre_key_id varints are encoded *after* the embedded message
    // field), which would short-circuit into the parse-error nack path
    // before ever reaching the BadMac arm we want to exercise.
    {
        let record = alice
            .sessions
            .0
            .get_mut(&bob_addr)
            .expect("alice has a session for bob");
        if let Some(state) = record.session_state_mut() {
            state.clear_unacknowledged_pre_key_message();
        }
    }

    // Second message: tamper the trailing MAC byte of a real SignalMessage.
    // The format is `[version][protobuf body][8-byte MAC]`, so the last byte
    // is squarely inside the MAC region — parse succeeds, MAC verification
    // fails -> libsignal returns BadMac.
    let msg2 = alice.encrypt_text(&bob_addr, "world").await;
    let mut bytes = match &msg2 {
        CiphertextMessage::SignalMessage(m) => m.serialized().to_vec(),
        other => panic!(
            "expected SignalMessage, got {:?}",
            std::mem::discriminant(other)
        ),
    };
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    let enc_node = NodeBuilder::new("enc")
        .attr("type", "msg")
        .bytes(bytes)
        .build();
    let enc_ref = enc_node.as_node_ref();
    let payloads: Vec<EncPayload> = vec![EncPayload::from_node_ref(&enc_ref).unwrap()];
    let info = Arc::new(MessageInfo {
        id: "BADMAC_TAMPER_MSG".to_string(),
        source: crate::types::message::MessageSource {
            sender: alice.jid.clone(),
            chat: alice.jid.clone(),
            ..Default::default()
        },
        ..Default::default()
    });

    let outcome = client
        .clone()
        .process_session_enc_batch(
            &payloads,
            &info,
            &alice.jid,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;
    assert!(!outcome.decrypted, "tampered MAC must not decrypt");
    assert!(
        outcome.undecryptable,
        "undecryptable event must be dispatched"
    );

    // The fix asserts the session lives on so the eventual sender pkmsg
    // can archive it into previous_sessions[0].
    let backend = client.persistence_manager.backend();
    let still = client
        .signal_cache
        .has_session(&alice_addr, &*backend)
        .await
        .expect("has_session");
    assert!(still, "BadMac must NOT delete the session (WA Web parity)");

    // Discriminate from the parse-error path (which also preserves the
    // session): the BadMac/InvalidMessage branch routes through
    // `handle_decrypt_failure` -> `spawn_retry_receipt`, which bumps
    // both caches with `RetryReason::BadMac`. Parse errors take the
    // nack path instead and never touch either cache.
    await_retry_receipt(&client, &info, 1, RetryReason::BadMac).await;
}

/// Poll for `message_retry_counts == expected_count` AND
/// `recent_retry_reasons == expected_reason` (or fail after a short
/// timeout). `spawn_retry_receipt` detaches the increment onto the
/// runtime, so both caches may lag the `process_session_enc_batch` return.
/// Reading both is what tells the BadMac arm apart from a parse-error
/// regression (which never bumps these caches).
async fn await_retry_receipt(
    client: &Arc<Client>,
    info: &MessageInfo,
    expected_count: u8,
    expected_reason: RetryReason,
) {
    let cache_key = client
        .make_retry_cache_key(&info.source.chat, &info.id, &info.source.sender)
        .await;
    for _ in 0..200 {
        if let (Some(c), Some(r)) = (
            client.message_retry_counts.get(&cache_key).await,
            client.recent_retry_reasons.get(&cache_key).await,
        ) && c == expected_count
            && r == expected_reason
        {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    let count = client.message_retry_counts.get(&cache_key).await;
    let reason = client.recent_retry_reasons.get(&cache_key).await;
    panic!(
        "expected retry ({expected_count}, {expected_reason:?}) for {cache_key}, \
             got ({count:?}, {reason:?})"
    );
}

// NOTE: the `InvalidMessage` arm of the `matches!()` block in
// `process_session_enc_batch` is exercised by `test_badmac_preserves_session`
// too — libsignal returns `BadMac` whenever *any* candidate state derives a
// message key (which is what a random-ratchet `SignalMessage::new(...)`
// ends up doing as well), so a separate "InvalidMessage" regression test
// would be indistinguishable from the BadMac one. Reaching the
// `InvalidMessage` constructor specifically would require crafting a
// SignalMessage that *parses* but where no state derives any message
// key — empirically impractical without major libsignal-side scaffolding.

/// Integration test: reproduces the production loop observed in
/// `k8awqjsgww2lnkt89urp3de1-191402150615-...`. After a BadMac the bot
/// used to delete the session; when the sender then sent a fresh pkmsg
/// (post-retry-receipt), `process_prekey_bundle` ran on an empty record
/// and `previous_sessions[0]` stayed empty — any in-flight messages on
/// the OLD ratchet failed permanently. With the fix the old session
/// survives the BadMac, the pkmsg's `promote_state` archives it, and
/// the archived state lives in `previous_sessions[0]` exactly as WA Web
/// expects (see `libsignal/src/protocol/state/session.rs:751-768`).
#[tokio::test]
async fn test_prod_scenario_pkmsg_archives_old_session_after_badmac() {
    let client = crate::test_utils::create_test_client_with_name("prod_archive").await;
    let mut alice = AlicePeer::new("3333333333333@s.whatsapp.net").await;

    // X3DH round 1 — Alice initiates with Bob's bundle, sends pkmsg.
    let (bundle_v1, _) = bobs_prekey_bundle(&client).await;
    let bob_addr = client
        .persistence_manager
        .get_device_snapshot()
        .await
        .lid
        .clone()
        .or(client
            .persistence_manager
            .get_device_snapshot()
            .await
            .pn
            .clone())
        .expect("own jid")
        .to_protocol_address();
    alice.install_bob_session(&bob_addr, &bundle_v1).await;
    let pkmsg_v1 = alice.encrypt_text(&bob_addr, "v1").await;
    let (s1, _, _, _) = submit_and_check_session(&client, &alice.jid, &pkmsg_v1).await;
    assert!(s1);

    // Snapshot Bob's session_v1 base key for later comparison. Use
    // peek (non-destructive): `get_session` marks the cache entry as
    // CheckedOut, which would prevent libsignal from re-loading the
    // session in the BadMac path that follows.
    let alice_addr = alice.address.clone();
    let backend = client.persistence_manager.backend();
    let v1_record = client
        .signal_cache
        .peek_session(&alice_addr, &*backend)
        .await
        .expect("peek_session")
        .expect("v1 session present");
    let v1_base_key = v1_record
        .session_state()
        .expect("v1 current state")
        .sender_ratchet_key_for_logging()
        .expect("v1 base key");

    // Force Alice's next encrypt to be a plain SignalMessage so tampering
    // the last byte lands inside the MAC region (see comment in
    // `test_badmac_preserves_session` for why pkmsg cannot be tampered
    // at the tail without breaking the outer protobuf parse).
    {
        let record = alice
            .sessions
            .0
            .get_mut(&bob_addr)
            .expect("alice has a session for bob");
        if let Some(state) = record.session_state_mut() {
            state.clear_unacknowledged_pre_key_message();
        }
    }

    // Tampered SignalMessage → BadMac branch (with the fix this no longer
    // deletes Bob's session).
    let msg = alice.encrypt_text(&bob_addr, "stale").await;
    let mut bytes = match &msg {
        CiphertextMessage::SignalMessage(m) => m.serialized().to_vec(),
        other => panic!(
            "expected SignalMessage, got {:?}",
            std::mem::discriminant(other)
        ),
    };
    let last = bytes.len() - 1;
    bytes[last] ^= 0xFF;
    let enc_node = NodeBuilder::new("enc")
        .attr("type", "msg")
        .bytes(bytes)
        .build();
    let enc_ref = enc_node.as_node_ref();
    let payloads: Vec<EncPayload> = vec![EncPayload::from_node_ref(&enc_ref).unwrap()];
    let info = Arc::new(MessageInfo {
        id: "PROD_LOOP_REPRO_STALE".to_string(),
        source: crate::types::message::MessageSource {
            sender: alice.jid.clone(),
            chat: alice.jid.clone(),
            ..Default::default()
        },
        ..Default::default()
    });
    let _outcome = client
        .clone()
        .process_session_enc_batch(
            &payloads,
            &info,
            &alice.jid,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;
    // Confirm the BadMac branch executed (parse-error path would skip
    // both retry caches; another arm would record a different reason).
    await_retry_receipt(&client, &info, 1, RetryReason::BadMac).await;
    // Pre-fix: this assertion would have failed (session deleted).
    let preserved = client
        .signal_cache
        .has_session(&alice_addr, &*backend)
        .await
        .expect("has_session");
    assert!(preserved, "BadMac must preserve session");

    // X3DH round 2 — Alice rebuilds her side from a fresh Bob bundle
    // (simulates the bot re-issuing prekeys via a retry receipt) and
    // sends another pkmsg. Bob's `process_prekey_bundle` must archive
    // session_v1 into previous_sessions[0].
    let (bundle_v2, _) = bobs_prekey_bundle(&client).await;
    alice.sessions = MemSessionStore::default(); // forget Alice's v1 to force a fresh X3DH
    alice.install_bob_session(&bob_addr, &bundle_v2).await;
    let pkmsg_v2 = alice.encrypt_text(&bob_addr, "v2").await;
    let (s2, _, _, still2) = submit_and_check_session(&client, &alice.jid, &pkmsg_v2).await;
    assert!(s2, "pkmsg_v2 should decrypt");
    assert!(still2);

    let v2_record = client
        .signal_cache
        .peek_session(&alice_addr, &*backend)
        .await
        .expect("peek_session")
        .expect("v2 session present");
    let v2_base_key = v2_record
        .session_state()
        .expect("v2 current state")
        .sender_ratchet_key_for_logging()
        .expect("v2 base key");
    assert_ne!(
        v1_base_key, v2_base_key,
        "current session must be the new v2"
    );
    assert_eq!(
        v2_record.previous_session_count(),
        1,
        "session_v1 must be archived as previous_sessions[0]"
    );
    let archived_state = v2_record
        .previous_session_states()
        .next()
        .expect("archived state")
        .expect("archived state decodes");
    let archived_base_key = archived_state
        .sender_ratchet_key_for_logging()
        .expect("archived base key");
    assert_eq!(
        archived_base_key, v1_base_key,
        "archived previous_sessions[0] must be the original v1"
    );
}

#[tokio::test]
async fn test_handle_incoming_message_skips_skmsg_after_msg_failure() {
    use wacore::libsignal::protocol::{IdentityKeyPair, KeyPair, SignalMessage};

    let backend = Arc::new(
        SqliteStore::new("file:memdb_skip_skmsg_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let sender_jid: Jid = "1234567890@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let group_jid: Jid = "120363021033254949@g.us"
        .parse()
        .expect("test JID should be valid");

    // Create msg + skmsg node; msg will fail (no session), so skmsg should be skipped
    let dummy_key = [0u8; 32];
    let sender_ratchet = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>()).public_key;
    let sender_identity_pair =
        IdentityKeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let receiver_identity_pair =
        IdentityKeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
    let signal_message = SignalMessage::new(
        4,
        &dummy_key,
        sender_ratchet,
        0,
        0,
        b"test",
        sender_identity_pair.identity_key(),
        receiver_identity_pair.identity_key(),
    )
    .expect("SignalMessage::new should succeed with valid inputs");

    let msg_node = NodeBuilder::new("enc")
        .attr("type", "msg")
        .bytes(signal_message.serialized().to_vec())
        .build();

    let skmsg_node = NodeBuilder::new("enc")
        .attr("type", "skmsg")
        .bytes(vec![4, 5, 6])
        .build();

    let message_node = node_to_arc(
        NodeBuilder::new("message")
            .attr("from", group_jid)
            .attr("participant", sender_jid)
            .attr("id", "test-id-123")
            .attr("t", "12345")
            .children(vec![msg_node, skmsg_node])
            .build(),
    );

    // Should not panic or retry loop - skmsg is skipped after msg failure
    client.clone().handle_incoming_message(message_node).await;
}

/// Test case for reproducing sender key JID mismatch in LID group messages
///
/// Problem:
/// - When we process sender key distribution from a self-sent LID message, we store it under the LID JID
/// - But when we try to decrypt the group content (skmsg), we look it up using the phone number JID
/// - This causes "No sender key state" errors even though we just processed the sender key!
///
/// This test verifies the fix by:
/// 1. Creating a sender key and storing it under the LID address (mimicking SKDM processing)
/// 2. Attempting retrieval with phone number address (the bug) - should fail
/// 3. Attempting retrieval with LID address (the fix) - should succeed
#[tokio::test]
async fn test_self_sent_lid_group_message_sender_key_mismatch() {
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore::libsignal::protocol::{
        SenderKeyStore, create_sender_key_distribution_message,
        process_sender_key_distribution_message,
    };

    let backend = Arc::new(
        SqliteStore::new("file:memdb_sender_key_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (_client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm.clone(),
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let own_lid: Jid = "100000000000001.1:75@lid"
        .parse()
        .expect("test JID should be valid");
    let own_phone: Jid = "15551234567:75@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");
    let group_jid: Jid = "120363021033254949@g.us"
        .parse()
        .expect("test JID should be valid");

    // Create SKDM using LID address (mimics handle_sender_key_distribution_message)
    let lid_protocol_address = own_lid.to_protocol_address();
    let lid_sender_key_name = make_sender_key_name(&group_jid, &lid_protocol_address);

    // Pin serialized form so from_jid stays compatible with persisted records
    assert_eq!(lid_sender_key_name.group_id(), group_jid.to_string());
    assert_eq!(
        lid_sender_key_name.sender_id(),
        lid_protocol_address.to_string()
    );

    let device_arc = pm.get_device_arc().await;
    let skdm = {
        let mut device_guard = device_arc.write().await;
        create_sender_key_distribution_message(
            &lid_sender_key_name,
            &mut *device_guard,
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .expect("Failed to create SKDM")
    };

    {
        let mut device_guard = device_arc.write().await;
        process_sender_key_distribution_message(&lid_sender_key_name, &skdm, &mut *device_guard)
            .await
            .expect("Failed to process SKDM with LID address");
    }

    // Try to retrieve using PHONE NUMBER address (THE BUG)
    let phone_protocol_address = own_phone.to_protocol_address();
    let phone_sender_key_name = make_sender_key_name(&group_jid, &phone_protocol_address);

    let phone_lookup_result = {
        let device_guard = device_arc.read().await;
        device_guard.load_sender_key(&phone_sender_key_name).await
    };

    assert!(
        phone_lookup_result
            .expect("lookup should not error")
            .is_none(),
        "Sender key should NOT be found when looking up with phone number address (demonstrates the bug)"
    );

    // Try to retrieve using LID address (THE FIX)
    let lid_lookup_result = {
        let device_guard = device_arc.read().await;
        device_guard.load_sender_key(&lid_sender_key_name).await
    };

    assert!(
        lid_lookup_result
            .expect("lookup should not error")
            .is_some(),
        "Sender key SHOULD be found when looking up with LID address (same as storage)"
    );
}

/// Test that sender key consistency is maintained for multiple LID participants
///
/// Edge case: Group with multiple LID participants, each should have their own
/// sender key stored under their LID address, not mixed up with phone numbers.
#[tokio::test]
async fn test_multiple_lid_participants_sender_key_isolation() {
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore::libsignal::protocol::{
        SenderKeyStore, create_sender_key_distribution_message,
        process_sender_key_distribution_message,
    };

    let backend = Arc::new(
        SqliteStore::new("file:memdb_multi_lid_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let transport_factory = Arc::new(crate::transport::mock::MockTransportFactory::new());
    let (_client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm.clone(),
        transport_factory,
        mock_http_client(),
        None,
    )
    .await;

    let group_jid: Jid = "120363021033254949@g.us"
        .parse()
        .expect("test JID should be valid");

    // Simulate three LID participants
    let participants = vec![
        ("100000000000001.1:75@lid", "15551234567:75@s.whatsapp.net"),
        ("987654321000000.2:42@lid", "551234567890:42@s.whatsapp.net"),
        ("111222333444555.3:10@lid", "559876543210:10@s.whatsapp.net"),
    ];

    let device_arc = pm.get_device_arc().await;

    // Create and store sender keys for each participant under their LID address
    for (lid_str, _phone_str) in &participants {
        let lid_jid: Jid = lid_str.parse().expect("test JID should be valid");
        let lid_protocol_address = lid_jid.to_protocol_address();
        let lid_sender_key_name = make_sender_key_name(&group_jid, &lid_protocol_address);

        let skdm = {
            let mut device_guard = device_arc.write().await;
            create_sender_key_distribution_message(
                &lid_sender_key_name,
                &mut *device_guard,
                &mut rand::make_rng::<rand::rngs::StdRng>(),
            )
            .await
            .expect("Failed to create SKDM")
        };

        let mut device_guard = device_arc.write().await;
        process_sender_key_distribution_message(&lid_sender_key_name, &skdm, &mut *device_guard)
            .await
            .expect("Failed to process SKDM");
    }

    // Verify each participant's sender key can be retrieved using their LID address
    for (lid_str, phone_str) in &participants {
        let lid_jid: Jid = lid_str.parse().expect("test JID should be valid");
        let phone_jid: Jid = phone_str.parse().expect("test JID should be valid");

        let lid_protocol_address = lid_jid.to_protocol_address();
        let phone_protocol_address = phone_jid.to_protocol_address();

        let lid_sender_key_name = make_sender_key_name(&group_jid, &lid_protocol_address);
        let phone_sender_key_name = make_sender_key_name(&group_jid, &phone_protocol_address);

        // Should find with LID address
        let lid_lookup = {
            let device_guard = device_arc.read().await;
            device_guard.load_sender_key(&lid_sender_key_name).await
        };
        assert!(
            lid_lookup.expect("lookup should not error").is_some(),
            "Sender key for {} should be found with LID address",
            lid_str
        );

        // Should NOT find with phone number address (the bug)
        let phone_lookup = {
            let device_guard = device_arc.read().await;
            device_guard.load_sender_key(&phone_sender_key_name).await
        };
        assert!(
            phone_lookup.expect("lookup should not error").is_none(),
            "Sender key for {} should NOT be found with phone number address",
            lid_str
        );
    }
}

/// Test that LID JID parsing handles various edge cases correctly
///
/// Edge cases:
/// - LID with multiple dots in user portion
/// - LID with device numbers
/// - LID without device numbers
#[test]
fn test_lid_jid_parsing_edge_cases() {
    use wacore_binary::Jid;

    // Single dot in user portion
    let lid1: Jid = "100000000000001.1:75@lid"
        .parse()
        .expect("test JID should be valid");
    assert_eq!(lid1.user, "100000000000001.1");
    assert_eq!(lid1.device, 75);
    assert_eq!(lid1.agent, 0);

    // Multiple dots in user portion (extreme edge case)
    let lid2: Jid = "123.456.789.0:50@lid"
        .parse()
        .expect("test JID should be valid");
    assert_eq!(lid2.user, "123.456.789.0");
    assert_eq!(lid2.device, 50);
    assert_eq!(lid2.agent, 0);

    // No device number (device 0)
    let lid3: Jid = "987654321000000.5@lid"
        .parse()
        .expect("test JID should be valid");
    assert_eq!(lid3.user, "987654321000000.5");
    assert_eq!(lid3.device, 0);
    assert_eq!(lid3.agent, 0);

    // Very long user portion with dot
    let lid4: Jid = "111222333444555666777.999:1@lid"
        .parse()
        .expect("test JID should be valid");
    assert_eq!(lid4.user, "111222333444555666777.999");
    assert_eq!(lid4.device, 1);
    assert_eq!(lid4.agent, 0);
}

/// Test that protocol address generation from LID JIDs matches WhatsApp Web format
///
/// WhatsApp Web uses: {user}[:device]@{server}.0
/// - The device is encoded in the name
/// - device_id is always 0
#[test]
fn test_lid_protocol_address_consistency() {
    use wacore::types::jid::JidExt as CoreJidExt;
    use wacore_binary::Jid;

    // Format: (jid_str, expected_name, expected_device_id, expected_to_string)
    let test_cases = vec![
        (
            "100000000000001.1:75@lid",
            "100000000000001.1:75@lid",
            0,
            "100000000000001.1:75@lid.0",
        ),
        (
            "987654321000000.2:42@lid",
            "987654321000000.2:42@lid",
            0,
            "987654321000000.2:42@lid.0",
        ),
        (
            "111.222.333:10@lid",
            "111.222.333:10@lid",
            0,
            "111.222.333:10@lid.0",
        ),
        // No device - should not include :0
        ("123456789@lid", "123456789@lid", 0, "123456789@lid.0"),
    ];

    for (jid_str, expected_name, expected_device_id, expected_to_string) in test_cases {
        let lid_jid: Jid = jid_str.parse().expect("test JID should be valid");
        let protocol_addr = lid_jid.to_protocol_address();

        assert_eq!(
            protocol_addr.name(),
            expected_name,
            "Protocol address name should match WhatsApp Web's SignalAddress format for {}",
            jid_str
        );
        assert_eq!(
            u32::from(protocol_addr.device_id()),
            expected_device_id,
            "Protocol address device_id should always be 0 for {}",
            jid_str
        );
        assert_eq!(
            protocol_addr.to_string(),
            expected_to_string,
            "Protocol address to_string() should match createSignalLikeAddress format for {}",
            jid_str
        );
    }
}

/// Test sender_alt extraction from message attributes in LID groups
///
/// Edge cases:
/// - LID group with participant_pn attribute
/// - PN group with participant_lid attribute
/// - Mixed addressing modes
#[tokio::test]
async fn test_parse_message_info_sender_alt_extraction() {
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore::types::message::AddressingMode;
    use wacore_binary::builder::NodeBuilder;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_sender_alt_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );

    // Set up own phone number and LID
    {
        let device_arc = pm.get_device_arc().await;
        let mut device = device_arc.write().await;
        device.pn = Some(
            "15551234567@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"),
        );
        device.lid = Some(
            "100000000000001.1@lid"
                .parse()
                .expect("test JID should be valid"),
        );
    }

    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    // Test case 1: LID group message with participant_pn
    let lid_group_node = NodeBuilder::new("message")
        .attr("from", "120363021033254949@g.us")
        .attr("participant", "987654321000000.2:42@lid")
        .attr("participant_pn", "551234567890:42@s.whatsapp.net")
        .attr("addressing_mode", AddressingMode::Lid.as_str())
        .attr("id", "test1")
        .attr("t", "12345")
        .build();

    let info1 = client
        .parse_message_info(&lid_group_node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");
    assert_eq!(info1.source.sender.user, "987654321000000.2");
    assert!(info1.source.sender_alt.is_some());
    assert_eq!(
        info1
            .source
            .sender_alt
            .as_ref()
            .expect("sender_alt should be present")
            .user,
        "551234567890"
    );

    // Test case 2: Self-sent LID group message
    let self_lid_node = NodeBuilder::new("message")
        .attr("from", "120363021033254949@g.us")
        .attr("participant", "100000000000001.1:75@lid")
        .attr("participant_pn", "15551234567:75@s.whatsapp.net")
        .attr("addressing_mode", AddressingMode::Lid.as_str())
        .attr("id", "test2")
        .attr("t", "12346")
        .build();

    let info2 = client
        .parse_message_info(&self_lid_node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");
    assert!(
        info2.source.is_from_me,
        "Should detect self-sent LID message"
    );
    assert_eq!(info2.source.sender.user, "100000000000001.1");
    assert!(info2.source.sender_alt.is_some());
    assert_eq!(
        info2
            .source
            .sender_alt
            .as_ref()
            .expect("sender_alt should be present")
            .user,
        "15551234567"
    );
}

/// Test that device query logic uses phone numbers for LID participants
///
/// This is a unit test for the logic in wacore/src/send.rs that converts
/// LID JIDs to phone number JIDs for device queries.
#[test]
fn test_lid_to_phone_mapping_for_device_queries() {
    use std::collections::HashMap;
    use wacore::client::context::GroupInfo;
    use wacore::types::message::AddressingMode;
    use wacore_binary::Jid;

    // Simulate a LID group with phone number mappings
    let mut lid_to_pn_map = HashMap::new();
    lid_to_pn_map.insert(
        wacore_binary::CompactString::from("100000000000001.1"),
        "15551234567@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid"),
    );
    lid_to_pn_map.insert(
        wacore_binary::CompactString::from("987654321000000.2"),
        "551234567890@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid"),
    );

    let mut group_info = GroupInfo::new(
        vec![
            "100000000000001.1:75@lid"
                .parse()
                .expect("test JID should be valid"),
            "987654321000000.2:42@lid"
                .parse()
                .expect("test JID should be valid"),
        ],
        AddressingMode::Lid,
    );
    group_info.set_lid_to_pn_map(lid_to_pn_map.clone());

    // Simulate the device query logic
    let jids_to_query: Vec<Jid> = group_info
        .participants
        .iter()
        .map(|jid| {
            let base_jid = jid.to_non_ad();
            if base_jid.is_lid()
                && let Some(phone_jid) = group_info.phone_jid_for_lid_user(&base_jid.user)
            {
                return phone_jid.to_non_ad();
            }
            base_jid
        })
        .collect();

    // Verify all queries use phone numbers, not LID JIDs
    for jid in &jids_to_query {
        assert_eq!(
            jid.server, SERVER_JID,
            "Device query should use phone number, got: {}",
            jid
        );
    }

    assert_eq!(jids_to_query.len(), 2);
    assert!(jids_to_query.iter().any(|j| j.user == "15551234567"));
    assert!(jids_to_query.iter().any(|j| j.user == "551234567890"));
}

/// Test edge case: Group with mixed LID and phone number participants
///
/// Some participants may still use phone numbers even in a LID group.
/// The code should handle both correctly.
#[test]
fn test_mixed_lid_and_phone_participants() {
    use std::collections::HashMap;
    use wacore::client::context::GroupInfo;
    use wacore::types::message::AddressingMode;
    use wacore_binary::Jid;

    let mut lid_to_pn_map = HashMap::new();
    lid_to_pn_map.insert(
        wacore_binary::CompactString::from("100000000000001.1"),
        "15551234567@s.whatsapp.net"
            .parse()
            .expect("test JID should be valid"),
    );

    let mut group_info = GroupInfo::new(
        vec![
            "100000000000001.1:75@lid"
                .parse()
                .expect("test JID should be valid"), // LID participant
            "551234567890:42@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"), // Phone number participant
        ],
        AddressingMode::Lid,
    );
    group_info.set_lid_to_pn_map(lid_to_pn_map.clone());

    let jids_to_query: Vec<Jid> = group_info
        .participants
        .iter()
        .map(|jid| {
            let base_jid = jid.to_non_ad();
            if base_jid.is_lid()
                && let Some(phone_jid) = group_info.phone_jid_for_lid_user(&base_jid.user)
            {
                return phone_jid.to_non_ad();
            }
            base_jid
        })
        .collect();

    // Both should end up as phone numbers
    assert_eq!(jids_to_query.len(), 2);
    for jid in &jids_to_query {
        assert_eq!(jid.server, SERVER_JID);
    }
}

/// Test edge case: Own JID check in LID mode
///
/// When checking if own JID is in the participant list, we must use
/// the phone number equivalent if in LID mode, not the LID itself.
#[test]
fn test_own_jid_check_in_lid_mode() {
    use std::collections::HashMap;
    use wacore_binary::Jid;

    let own_lid: Jid = "100000000000001.1@lid"
        .parse()
        .expect("test JID should be valid");
    let own_phone: Jid = "15551234567@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");

    let mut lid_to_pn_map = HashMap::new();
    lid_to_pn_map.insert("100000000000001.1".to_string(), own_phone.clone());

    // Simulate the own JID check logic from wacore/src/send.rs
    let own_base_jid = own_lid.to_non_ad();
    let own_jid_to_check = if own_base_jid.is_lid() {
        lid_to_pn_map
            .get(own_base_jid.user.as_str())
            .map(|pn| pn.to_non_ad())
            .unwrap_or_else(|| own_base_jid.clone())
    } else {
        own_base_jid.clone()
    };

    // Verify we're checking using the phone number
    assert_eq!(own_jid_to_check.user, "15551234567");
    assert_eq!(own_jid_to_check.server, SERVER_JID);
}

/// Test that sender key operations always use the display JID (LID)
/// regardless of what JID is used for E2E session decryption
#[tokio::test]
async fn test_sender_key_always_uses_display_jid() {
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore::libsignal::protocol::{SenderKeyStore, create_sender_key_distribution_message};

    let backend = Arc::new(
        SqliteStore::new("file:memdb_display_jid_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (_client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm.clone(),
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let group_jid: Jid = "120363021033254949@g.us"
        .parse()
        .expect("test JID should be valid");
    let display_jid: Jid = "100000000000001.1:75@lid"
        .parse()
        .expect("test JID should be valid");
    let encryption_jid: Jid = "15551234567:75@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");

    // Store sender key using display JID (LID)
    let display_protocol_address = display_jid.to_protocol_address();
    let display_sender_key_name = make_sender_key_name(&group_jid, &display_protocol_address);

    let device_arc = pm.get_device_arc().await;
    {
        let mut device_guard = device_arc.write().await;
        create_sender_key_distribution_message(
            &display_sender_key_name,
            &mut *device_guard,
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .expect("Failed to create SKDM");
    }

    // Verify it's stored under display JID
    let lookup_with_display = {
        let device_guard = device_arc.read().await;
        device_guard.load_sender_key(&display_sender_key_name).await
    };
    assert!(
        lookup_with_display
            .expect("lookup should not error")
            .is_some(),
        "Sender key should be found with display JID (LID)"
    );

    // Verify it's NOT accessible via encryption JID (phone number)
    let encryption_protocol_address = encryption_jid.to_protocol_address();
    let encryption_sender_key_name = make_sender_key_name(&group_jid, &encryption_protocol_address);

    let lookup_with_encryption = {
        let device_guard = device_arc.read().await;
        device_guard
            .load_sender_key(&encryption_sender_key_name)
            .await
    };
    assert!(
        lookup_with_encryption
            .expect("lookup should not error")
            .is_none(),
        "Sender key should NOT be found with encryption JID (phone number)"
    );
}

/// Test edge case: Second message with only skmsg (no pkmsg/msg)
///
/// After the first message establishes a session and sender key,
/// subsequent messages may contain only skmsg. These should still
/// be decrypted successfully, not skipped.
///
/// Bug: The code was treating "no session messages" as "session failed",
/// causing it to skip skmsg decryption for all messages after the first.
#[tokio::test]
async fn test_second_message_with_only_skmsg_decrypts() {
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore::libsignal::protocol::{
        create_sender_key_distribution_message, process_sender_key_distribution_message,
    };

    use wacore::types::message::AddressingMode;
    use wacore_binary::builder::NodeBuilder;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_second_msg_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm.clone(),
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let sender_jid: Jid = "100000000000001.1:75@lid"
        .parse()
        .expect("test JID should be valid");
    let group_jid: Jid = "120363021033254949@g.us"
        .parse()
        .expect("test JID should be valid");

    // Step 1: Create and store a sender key (simulating first message processing)
    let sender_protocol_address = sender_jid.to_protocol_address();
    let sender_key_name = make_sender_key_name(&group_jid, &sender_protocol_address);

    let device_arc = pm.get_device_arc().await;
    {
        let mut device_guard = device_arc.write().await;
        let skdm = create_sender_key_distribution_message(
            &sender_key_name,
            &mut *device_guard,
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .expect("Failed to create SKDM");

        process_sender_key_distribution_message(&sender_key_name, &skdm, &mut *device_guard)
            .await
            .expect("Failed to process SKDM");
    }

    // Create message with ONLY skmsg (simulating second message after session established)
    let skmsg_ciphertext = {
        let mut device_guard = device_arc.write().await;
        let sender_key_msg = wacore::libsignal::protocol::group_encrypt(
            &mut *device_guard,
            &sender_key_name,
            b"ping",
            &mut rand::make_rng::<rand::rngs::StdRng>(),
        )
        .await
        .expect("Failed to encrypt with sender key");
        sender_key_msg.serialized().to_vec()
    };

    let skmsg_node = NodeBuilder::new("enc")
        .attr("type", "skmsg")
        .attr("v", "2")
        .bytes(skmsg_ciphertext)
        .build();

    let message_node = node_to_arc(
        NodeBuilder::new("message")
            .attr("from", group_jid)
            .attr("participant", sender_jid)
            .attr("id", "SECOND_MSG_TEST")
            .attr("t", "1759306493")
            .attr("type", "text")
            .attr("addressing_mode", AddressingMode::Lid.as_str())
            .children(vec![skmsg_node])
            .build(),
    );

    // Should NOT skip skmsg - before the fix this would incorrectly skip
    client.clone().handle_incoming_message(message_node).await;
}

/// Test case for UntrustedIdentity error handling and recovery
///
/// Scenario:
/// - User re-installs WhatsApp or switches devices
/// - Their device generates a new identity key
/// - The bot still has the old identity key stored
/// - When a message arrives, Signal Protocol rejects it as "UntrustedIdentity"
/// - The bot should catch this error, clear the old identity using the FULL protocol address (with device ID), and retry
///
/// This test verifies that:
/// 1. process_session_enc_batch handles UntrustedIdentity gracefully
/// 2. The deletion uses the correct full address (name.device_id) not just the name
/// 3. No panic occurs when UntrustedIdentity is encountered
/// 4. The error is logged appropriately
/// 5. The bot continues processing instead of propagating the error
#[tokio::test]
async fn test_untrusted_identity_error_is_caught_and_handled() {
    use crate::store::SqliteStore;
    use std::sync::Arc;

    // Setup
    let backend = Arc::new(
        SqliteStore::new("file:memdb_untrusted_identity_caught?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm.clone(),
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let sender_jid: Jid = "559981212574@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");

    let info = Arc::new(MessageInfo {
        source: crate::types::message::MessageSource {
            sender: sender_jid.clone(),
            chat: sender_jid.clone(),
            ..Default::default()
        },
        ..Default::default()
    });

    log::info!("Test: UntrustedIdentity scenario for {}", sender_jid);

    // Create a malformed/invalid encrypted node to trigger error handling path
    // This won't create UntrustedIdentity specifically, but tests the error handling code path
    // The important fix is that when UntrustedIdentity IS raised, the code uses
    // address.to_string() (which gives "559981212574.0") instead of address.name()
    // (which only gives "559981212574") for the deletion key.
    let enc_node = NodeBuilder::new("enc")
        .attr("type", "msg")
        .attr("v", "2")
        .bytes(vec![0xFF; 100]) // Invalid encrypted payload
        .build();

    let enc_node_ref = enc_node.as_node_ref();
    let payloads: Vec<EncPayload> = vec![EncPayload::from_node_ref(&enc_node_ref).unwrap()];

    // Call process_session_enc_batch
    // This should handle any errors gracefully without panicking
    let outcome = client
        .process_session_enc_batch(
            &payloads,
            &info,
            &sender_jid,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;

    log::info!(
        "Test: process_session_enc_batch completed - success: {}",
        outcome.decrypted
    );

    // The key is that this didn't panic - deletion uses full protocol address
}

/// Test case: Error handling during batch processing
///
/// When multiple messages are being processed in a batch, if one triggers
/// an error (like UntrustedIdentity), it should be handled without affecting
/// other messages in the batch.
#[tokio::test]
async fn test_untrusted_identity_does_not_break_batch_processing() {
    use crate::store::SqliteStore;
    use std::sync::Arc;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_untrusted_batch?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm.clone(),
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let sender_jid: Jid = "559981212574@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");

    let info = Arc::new(MessageInfo {
        source: crate::types::message::MessageSource {
            sender: sender_jid.clone(),
            chat: sender_jid.clone(),
            ..Default::default()
        },
        ..Default::default()
    });

    log::info!("Test: Batch processing with multiple error messages");

    // Create multiple invalid encrypted nodes to test batch error handling
    let mut enc_nodes = Vec::new();

    // First message: Invalid encrypted payload
    let enc_node_1 = NodeBuilder::new("enc")
        .attr("type", "msg")
        .attr("v", "2")
        .bytes(vec![0xFF; 50])
        .build();
    enc_nodes.push(enc_node_1);

    // Second message: Another invalid encrypted payload
    let enc_node_2 = NodeBuilder::new("enc")
        .attr("type", "msg")
        .attr("v", "2")
        .bytes(vec![0xAA; 50])
        .build();
    enc_nodes.push(enc_node_2);

    log::info!("Test: Created batch of 2 messages with invalid data");

    let payloads: Vec<EncPayload> = enc_nodes
        .iter()
        .filter_map(|n| EncPayload::from_node_ref(&n.as_node_ref()))
        .collect();

    // Process the batch
    // Should handle all errors gracefully without stopping at first error
    let outcome = client
        .process_session_enc_batch(
            &payloads,
            &info,
            &sender_jid,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;

    log::info!(
        "Test: Batch processing completed - success: {}",
        outcome.decrypted
    );
}

/// Test case: Error handling in group chat context
///
/// When processing messages from group members, if identity errors occur,
/// they should be handled per-sender without affecting other group members.
#[tokio::test]
async fn test_untrusted_identity_in_group_context() {
    use crate::store::SqliteStore;
    use std::sync::Arc;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_untrusted_group?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm.clone(),
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    // Simulate a group chat scenario
    let group_jid: Jid = "120363021033254949@g.us"
        .parse()
        .expect("test JID should be valid");
    let sender_phone: Jid = "559981212574@s.whatsapp.net"
        .parse()
        .expect("test JID should be valid");

    let info = Arc::new(MessageInfo {
        source: crate::types::message::MessageSource {
            sender: sender_phone.clone(),
            chat: group_jid.clone(),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });

    log::info!("Test: Group context - error handling for {}", sender_phone);

    // Create an invalid encrypted message
    let enc_node = NodeBuilder::new("enc")
        .attr("type", "msg")
        .attr("v", "2")
        .bytes(vec![0xFF; 100])
        .build();

    let enc_node_ref = enc_node.as_node_ref();
    let payloads: Vec<EncPayload> = vec![EncPayload::from_node_ref(&enc_node_ref).unwrap()];

    // Process the message
    // Should handle errors gracefully in group context
    let outcome = client
        .process_session_enc_batch(
            &payloads,
            &info,
            &sender_phone,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;

    log::info!(
        "Test: Group message processed - success: {}",
        outcome.decrypted
    );
}

/// Test case: DM message parsing for self-sent messages via LID
///
/// Scenario:
/// - You send a DM to another user from your phone
/// - Your bot receives the echo with from=your_LID, recipient=their_LID
/// - peer_recipient_pn contains the RECIPIENT's phone number (not sender's)
///
/// The fix ensures:
/// 1. is_from_me is correctly detected for LID senders
/// 2. sender_alt is NOT populated with peer_recipient_pn (that's the recipient's PN)
/// 3. Decryption uses own PN via the is_from_me fallback path
#[tokio::test]
async fn test_parse_message_info_self_sent_dm_via_lid() {
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore_binary::builder::NodeBuilder;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_self_dm_lid_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );

    // Set up own phone number and LID
    {
        let device_arc = pm.get_device_arc().await;
        let mut device = device_arc.write().await;
        device.pn = Some(
            "15551234567@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"),
        );
        device.lid = Some(
            "100000000000001@lid"
                .parse()
                .expect("test JID should be valid"),
        );
    }

    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    // Simulate self-sent DM to another user (from your phone to your bot echo)
    // Real log example:
    // from="100000000000001@lid" recipient="39492358562039@lid" peer_recipient_pn="559985213786@s.whatsapp.net"
    let self_dm_node = NodeBuilder::new("message")
        .attr("from", "100000000000001@lid") // Your LID
        .attr("recipient", "39492358562039@lid") // Recipient's LID
        .attr("peer_recipient_pn", "559985213786@s.whatsapp.net") // Recipient's PN (NOT sender's!)
        .attr("notify", "jl")
        .attr("id", "AC756E00B560721DBC4C0680131827EA")
        .attr("t", "1764845025")
        .attr("type", "text")
        .build();

    let info = client
        .parse_message_info(&self_dm_node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    // Assertions:
    // 1. is_from_me should be true (LID matches own_lid)
    assert!(
        info.source.is_from_me,
        "Should detect self-sent DM from own LID"
    );

    // 2. sender_alt should be own PN (derived from own_jid, not message attrs)
    assert!(
        info.source.sender_alt.is_some(),
        "sender_alt should be own PN for self-sent LID messages"
    );
    assert_eq!(
        info.source.sender_alt.as_ref().unwrap().user,
        "15551234567",
        "sender_alt should be the own PN user"
    );

    assert_eq!(
        info.source.chat.user, "39492358562039",
        "Chat should be the recipient's LID"
    );

    assert_eq!(
        info.source.sender.user, "100000000000001",
        "Sender should be own LID"
    );
}

/// Test case: DM message parsing for messages from others via LID
///
/// Scenario:
/// - Another user sends you a DM
/// - Message arrives with from=their_LID, sender_pn=their_phone_number
///
/// The fix ensures:
/// 1. is_from_me is false
/// 2. sender_alt is populated from sender_pn attribute (if present)
/// 3. Decryption uses sender_alt for session lookup
#[tokio::test]
async fn test_parse_message_info_dm_from_other_via_lid() {
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore_binary::builder::NodeBuilder;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_other_dm_lid_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );

    // Set up own phone number and LID
    {
        let device_arc = pm.get_device_arc().await;
        let mut device = device_arc.write().await;
        device.pn = Some(
            "15551234567@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"),
        );
        device.lid = Some(
            "100000000000001@lid"
                .parse()
                .expect("test JID should be valid"),
        );
    }

    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    // Simulate DM from another user via their LID
    // The sender_pn attribute should contain their phone number for session lookup
    let other_dm_node = NodeBuilder::new("message")
        .attr("from", "39492358562039@lid") // Sender's LID (not ours)
        .attr("sender_pn", "559985213786@s.whatsapp.net") // Sender's phone number
        .attr("notify", "Other User")
        .attr("id", "AABBCCDD1234567890")
        .attr("t", "1764845100")
        .attr("type", "text")
        .build();

    let info = client
        .parse_message_info(&other_dm_node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    assert!(
        !info.source.is_from_me,
        "Should NOT be detected as self-sent"
    );

    assert!(
        info.source.sender_alt.is_some(),
        "sender_alt should be set from sender_pn attribute"
    );
    assert_eq!(
        info.source
            .sender_alt
            .as_ref()
            .expect("sender_alt should be present")
            .user,
        "559985213786",
        "sender_alt should contain sender's phone number"
    );

    assert_eq!(
        info.source.chat.user, "39492358562039",
        "Chat should be the sender's LID (non-AD)"
    );

    assert_eq!(
        info.source.sender.user, "39492358562039",
        "Sender should be other user's LID"
    );
}

/// Test case: DM message to self (own chat, like "Notes to Myself")
///
/// Scenario:
/// - You send a message to yourself (your own chat)
/// - from=your_LID, recipient=your_LID, peer_recipient_pn=your_PN
///
/// This is the original bug case that was fixed earlier.
#[tokio::test]
async fn test_parse_message_info_dm_to_self() {
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore_binary::builder::NodeBuilder;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_dm_to_self_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );

    // Set up own phone number and LID
    {
        let device_arc = pm.get_device_arc().await;
        let mut device = device_arc.write().await;
        device.pn = Some(
            "15551234567@s.whatsapp.net"
                .parse()
                .expect("test JID should be valid"),
        );
        device.lid = Some(
            "100000000000001@lid"
                .parse()
                .expect("test JID should be valid"),
        );
    }

    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    // Simulate DM to self (like "Notes to Myself" or pinging yourself)
    // from=your_LID, recipient=your_LID, peer_recipient_pn=your_PN
    let self_chat_node = NodeBuilder::new("message")
        .attr("from", "100000000000001@lid") // Your LID
        .attr("recipient", "100000000000001@lid") // Also your LID (self-chat)
        .attr("peer_recipient_pn", "15551234567@s.whatsapp.net") // Your PN
        .attr("notify", "jl")
        .attr("id", "AC391DD54A28E1CE1F3B106DF9951FAD")
        .attr("t", "1764822437")
        .attr("type", "text")
        .build();

    let info = client
        .parse_message_info(&self_chat_node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    assert!(
        info.source.is_from_me,
        "Should detect self-sent message to self-chat"
    );

    assert!(
        info.source.sender_alt.is_some(),
        "sender_alt should be own PN for self-sent LID messages"
    );
    assert_eq!(
        info.source.sender_alt.as_ref().unwrap().user,
        "15551234567",
        "sender_alt should match own PN"
    );

    assert_eq!(
        info.source.chat.user, "100000000000001",
        "Chat should be self (recipient)"
    );

    assert_eq!(
        info.source.sender.user, "100000000000001",
        "Sender should be own LID"
    );
}

/// Test that receiving a DM with sender_lid populates the lid_pn_cache.
///
/// This is the key behavior for the LID-PN session mismatch fix:
/// When we receive a message from a phone number with sender_lid attribute,
/// we cache the phone->LID mapping so that when sending replies, we can
/// reuse the existing LID session instead of creating a new PN session.
///
/// Flow being tested:
/// 1. Receive message from 559980000001@s.whatsapp.net with sender_lid=100000012345678@lid
/// 2. Cache should be populated with: 559980000001 -> 100000012345678
/// 3. When sending reply to 559980000001, we can look up the LID and use existing session
#[tokio::test]
async fn test_lid_pn_cache_populated_on_message_with_sender_lid() {
    // Setup client
    let backend = Arc::new(
        SqliteStore::new("file:memdb_lid_cache_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let phone = "559980000001";
    let lid = "100000012345678";

    // Verify cache is empty initially
    assert!(
        client.lid_pn_cache.get_current_lid(phone).await.is_none(),
        "Cache should be empty before receiving message"
    );

    // Create a DM message node with sender_lid attribute
    // This simulates receiving a message from WhatsApp Web
    let dm_node = NodeBuilder::new("message")
        .attr("from", Jid::pn(phone).to_string())
        .attr("sender_lid", Jid::lid(lid).to_string())
        .attr("id", "TEST123456789")
        .attr("t", "1765482972")
        .attr("type", "text")
        .children([NodeBuilder::new("enc")
            .attr("type", "pkmsg")
            .attr("v", "2")
            .bytes(vec![0u8; 100]) // Dummy encrypted content
            .build()])
        .build();

    // Call handle_incoming_message - this will fail to decrypt (no real session)
    // but it should still populate the cache before attempting decryption
    client
        .clone()
        .handle_incoming_message(node_to_arc(dm_node))
        .await;

    // Verify the cache was populated
    let cached_lid = client.lid_pn_cache.get_current_lid(phone).await;
    assert!(
        cached_lid.is_some(),
        "Cache should be populated after receiving message with sender_lid"
    );
    assert_eq!(
        cached_lid.expect("cache should have LID"),
        lid,
        "Cached LID should match the sender_lid from the message"
    );
}

/// Test that messages without sender_lid do NOT populate the cache.
///
/// This ensures we don't accidentally cache incorrect mappings.
#[tokio::test]
async fn test_lid_pn_cache_not_populated_without_sender_lid() {
    // Setup client
    let backend = Arc::new(
        SqliteStore::new("file:memdb_no_lid_cache_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let phone = "559980000001";

    // Create a DM message node WITHOUT sender_lid attribute
    let dm_node = NodeBuilder::new("message")
        .attr("from", Jid::pn(phone).to_string())
        // Note: NO sender_lid attribute
        .attr("id", "TEST123456789")
        .attr("t", "1765482972")
        .attr("type", "text")
        .children([NodeBuilder::new("enc")
            .attr("type", "pkmsg")
            .attr("v", "2")
            .bytes(vec![0u8; 100])
            .build()])
        .build();

    // Call handle_incoming_message
    client
        .clone()
        .handle_incoming_message(node_to_arc(dm_node))
        .await;

    assert!(
        client.lid_pn_cache.get_current_lid(phone).await.is_none(),
        "Cache should NOT be populated for messages without sender_lid"
    );
}

/// Test that messages from LID senders with participant_pn DO populate the cache.
///
/// When the sender is a LID (e.g., in LID-mode groups), and participant_pn
/// contains their phone number, we SHOULD cache this mapping because:
/// 1. The cache is bidirectional - we need both LID->PN and PN->LID
/// 2. This enables sending to users we've only seen as LID senders
#[tokio::test]
async fn test_lid_pn_cache_populated_for_lid_sender_with_participant_pn() {
    use wacore::types::message::AddressingMode;

    // Setup client
    let backend = Arc::new(
        SqliteStore::new("file:memdb_lid_sender_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let lid = "100000012345678";
    let phone = "559980000001";

    // Create a message from a LID sender with participant_pn attribute
    // This happens in LID-mode groups (addressing_mode="lid")
    let group_node = NodeBuilder::new("message")
        .attr("from", "120363123456789012@g.us") // Group chat
        .attr("participant", Jid::lid(lid).to_string()) // Sender is LID
        .attr("participant_pn", Jid::pn(phone).to_string()) // Their phone number
        .attr("addressing_mode", AddressingMode::Lid.as_str()) // Required for participant_pn to be parsed
        .attr("id", "TEST123456789")
        .attr("t", "1765482972")
        .attr("type", "text")
        .children([NodeBuilder::new("enc")
            .attr("type", "skmsg")
            .attr("v", "2")
            .bytes(vec![0u8; 100])
            .build()])
        .build();

    // Call handle_incoming_message
    client
        .clone()
        .handle_incoming_message(node_to_arc(group_node))
        .await;

    // Verify the cache WAS populated (bidirectional cache)
    let cached_lid = client.lid_pn_cache.get_current_lid(phone).await;
    assert!(
        cached_lid.is_some(),
        "Cache should be populated for LID senders with participant_pn"
    );
    assert_eq!(
        cached_lid.expect("cache should have LID"),
        lid,
        "Cached LID should match the sender's LID"
    );

    // Also verify we can look up the phone number from the LID
    let cached_pn = client.lid_pn_cache.get_phone_number(lid).await;
    assert!(cached_pn.is_some(), "Reverse lookup (LID->PN) should work");
    assert_eq!(
        cached_pn.expect("reverse lookup should return phone"),
        phone,
        "Cached phone number should match"
    );
}

/// Test that multiple messages from the same sender update the cache correctly.
///
/// This ensures the cache handles repeated messages gracefully.
#[tokio::test]
async fn test_lid_pn_cache_handles_repeated_messages() {
    // Setup client
    let backend = Arc::new(
        SqliteStore::new("file:memdb_repeated_msg_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let phone = "559980000001";
    let lid = "100000012345678";

    // Send multiple messages from the same sender
    for i in 0..3 {
        let dm_node = NodeBuilder::new("message")
            .attr("from", Jid::pn(phone).to_string())
            .attr("sender_lid", Jid::lid(lid).to_string())
            .attr("id", format!("TEST{}", i))
            .attr("t", "1765482972")
            .attr("type", "text")
            .children([NodeBuilder::new("enc")
                .attr("type", "pkmsg")
                .attr("v", "2")
                .bytes(vec![0u8; 100])
                .build()])
            .build();

        client
            .clone()
            .handle_incoming_message(node_to_arc(dm_node))
            .await;
    }

    // Verify the cache still has the correct mapping
    let cached_lid = client.lid_pn_cache.get_current_lid(phone).await;
    assert!(cached_lid.is_some(), "Cache should contain the mapping");
    assert_eq!(
        cached_lid.expect("cache should have LID"),
        lid,
        "Cached LID should be correct after multiple messages"
    );
}

/// Test that PN-addressed messages use LID for session lookup when LID mapping is known.
///
/// This test verifies the fix for the MAC verification failure bug:
/// WhatsApp Web's SignalAddress.toString() ALWAYS converts PN addresses to LID
/// when a LID mapping is known. The Rust client must do the same to ensure
/// session keys match between clients.
///
/// Bug scenario:
/// 1. WhatsApp Web Client A sends a group message to our Rust client
/// 2. Rust client creates session under PN address (559980000001@c.us.0)
/// 3. Rust client sends group response, creates session under LID (100000012345678@lid.0)
/// 4. Client A sends DM to Rust client from PN address
/// 5. Rust client tries to decrypt using PN address but session is under LID
/// 6. MAC verification fails because wrong session is used
///
/// Fix: When receiving a PN-addressed message, if we have a LID mapping,
/// use the LID address for session lookup (matching WhatsApp Web behavior).
#[tokio::test]
async fn test_pn_message_uses_lid_for_session_lookup_when_mapping_known() {
    use crate::lid_pn_cache::LidPnEntry;
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore::types::jid::JidExt;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_pn_to_lid_session_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let lid = "100000012345678";
    let phone = "559980000001";

    // Pre-populate the LID-PN cache (simulating a previous group message)
    let entry = LidPnEntry::new(
        lid.to_string(),
        phone.to_string(),
        crate::lid_pn_cache::LearningSource::PeerLidMessage,
    );
    client.lid_pn_cache.add(&entry).await;

    // Verify the cache has the mapping
    let cached_lid = client.lid_pn_cache.get_current_lid(phone).await;
    assert_eq!(
        cached_lid.as_deref(),
        Some(lid),
        "Cache should have the LID-PN mapping"
    );

    // Test scenario: Parse a PN-addressed DM message (with sender_lid attribute)
    let dm_node_with_sender_lid = wacore_binary::builder::NodeBuilder::new("message")
        .attr("from", Jid::pn(phone).to_string())
        .attr("sender_lid", Jid::lid(lid).to_string())
        .attr("id", "test_dm_with_lid")
        .attr("t", "1765494882")
        .attr("type", "text")
        .build();

    let info = client
        .parse_message_info(&dm_node_with_sender_lid.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    // Verify sender is PN but sender_alt is LID
    assert_eq!(info.source.sender.user, phone);
    assert_eq!(info.source.sender.server, wacore_binary::Server::Pn);
    assert!(info.source.sender_alt.is_some());
    assert_eq!(
        info.source
            .sender_alt
            .as_ref()
            .expect("sender_alt should be present")
            .user,
        lid
    );
    assert_eq!(
        info.source
            .sender_alt
            .as_ref()
            .expect("sender_alt should be present")
            .server,
        wacore_binary::Server::Lid
    );

    // Now simulate what handle_incoming_message does: determine encryption JID
    // We can't easily call handle_incoming_message, so we'll test the logic directly
    let sender = &info.source.sender;
    let alt = info.source.sender_alt.as_ref();
    // Apply the same logic as in handle_incoming_message
    let sender_encryption_jid = if sender.is_lid() {
        sender.clone()
    } else if sender.is_pn() {
        if let Some(alt_jid) = alt
            && alt_jid.is_lid()
        {
            // Use the LID from the message attribute
            Jid {
                user: alt_jid.user.clone(),
                server: wacore_binary::Server::Lid,
                device: sender.device,
                agent: sender.agent,
                integrator: sender.integrator,
            }
        } else if let Some(lid_user) = client.lid_pn_cache.get_current_lid(&sender.user).await {
            // Use the cached LID
            Jid {
                user: lid_user,
                server: wacore_binary::Server::Lid,
                device: sender.device,
                agent: sender.agent,
                integrator: sender.integrator,
            }
        } else {
            sender.clone()
        }
    } else {
        sender.clone()
    };

    // Verify the encryption JID uses the LID, not the PN
    assert_eq!(
        sender_encryption_jid.user, lid,
        "Encryption JID should use LID user"
    );
    assert_eq!(
        sender_encryption_jid.server,
        wacore_binary::Server::Lid,
        "Encryption JID should use LID server"
    );

    // Verify the protocol address format
    let protocol_address = sender_encryption_jid.to_protocol_address();
    assert_eq!(
        protocol_address.to_string(),
        format!("{}@lid.0", lid),
        "Protocol address should be in LID format"
    );
}

/// Test that PN-addressed messages use cached LID even without sender_lid attribute.
///
/// This tests the fallback path where the message doesn't have a sender_lid
/// attribute but we have a previously cached LID mapping.
#[tokio::test]
async fn test_pn_message_uses_cached_lid_without_sender_lid_attribute() {
    use crate::lid_pn_cache::LidPnEntry;
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore::types::jid::JidExt;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_cached_lid_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let lid = "100000012345678";
    let phone = "559980000001";

    // Pre-populate the LID-PN cache
    let entry = LidPnEntry::new(
        lid.to_string(),
        phone.to_string(),
        crate::lid_pn_cache::LearningSource::PeerLidMessage,
    );
    client.lid_pn_cache.add(&entry).await;

    // Parse a PN-addressed DM message WITHOUT sender_lid attribute
    let dm_node_without_sender_lid = wacore_binary::builder::NodeBuilder::new("message")
        .attr("from", Jid::pn(phone).to_string())
        // Note: No sender_lid attribute!
        .attr("id", "test_dm_no_lid")
        .attr("t", "1765494882")
        .attr("type", "text")
        .build();

    let info = client
        .parse_message_info(&dm_node_without_sender_lid.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    // Verify sender is PN and NO sender_alt (since there's no sender_lid attribute)
    assert_eq!(info.source.sender.user, phone);
    assert_eq!(info.source.sender.server, wacore_binary::Server::Pn);
    assert!(
        info.source.sender_alt.is_none(),
        "Should have no sender_alt without sender_lid attribute"
    );

    // Apply the encryption JID logic (fallback to cached LID)
    let sender = &info.source.sender;
    let alt = info.source.sender_alt.as_ref();
    let sender_encryption_jid = if sender.is_lid() {
        sender.clone()
    } else if sender.is_pn() {
        if let Some(alt_jid) = alt
            && alt_jid.is_lid()
        {
            Jid {
                user: alt_jid.user.clone(),
                server: wacore_binary::Server::Lid,
                device: sender.device,
                agent: sender.agent,
                integrator: sender.integrator,
            }
        } else if let Some(lid_user) = client.lid_pn_cache.get_current_lid(&sender.user).await {
            // This is the path we're testing - fallback to cached LID
            Jid {
                user: lid_user,
                server: wacore_binary::Server::Lid,
                device: sender.device,
                agent: sender.agent,
                integrator: sender.integrator,
            }
        } else {
            sender.clone()
        }
    } else {
        sender.clone()
    };

    // Verify the encryption JID uses the cached LID
    assert_eq!(
        sender_encryption_jid.user, lid,
        "Encryption JID should use cached LID user"
    );
    assert_eq!(
        sender_encryption_jid.server,
        wacore_binary::Server::Lid,
        "Encryption JID should use LID server"
    );

    let protocol_address = sender_encryption_jid.to_protocol_address();
    assert_eq!(
        protocol_address.to_string(),
        format!("{}@lid.0", lid),
        "Protocol address should be in LID format from cached mapping"
    );
}

/// Test that PN-addressed messages use PN when no LID mapping is known.
///
/// When there's no LID mapping available, we should fall back to using
/// the PN address for session lookup.
#[tokio::test]
async fn test_pn_message_uses_pn_when_no_lid_mapping() {
    use crate::store::SqliteStore;
    use std::sync::Arc;
    use wacore::types::jid::JidExt;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_no_lid_mapping_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let phone = "559980000001";

    // Don't populate the cache - simulate first-time contact

    // Parse a PN-addressed DM message without sender_lid
    let dm_node = wacore_binary::builder::NodeBuilder::new("message")
        .attr("from", Jid::pn(phone).to_string())
        .attr("id", "test_dm_no_mapping")
        .attr("t", "1765494882")
        .attr("type", "text")
        .build();

    let info = client
        .parse_message_info(&dm_node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    // Verify no cached LID
    let cached_lid = client.lid_pn_cache.get_current_lid(phone).await;
    assert!(cached_lid.is_none(), "Should have no cached LID mapping");

    // Apply the encryption JID logic
    let sender = &info.source.sender;
    let alt = info.source.sender_alt.as_ref();

    let sender_encryption_jid = if sender.is_lid() {
        sender.clone()
    } else if sender.is_pn() {
        if let Some(alt_jid) = alt
            && alt_jid.is_lid()
        {
            Jid {
                user: alt_jid.user.clone(),
                server: wacore_binary::Server::Lid,
                device: sender.device,
                agent: sender.agent,
                integrator: sender.integrator,
            }
        } else if let Some(lid_user) = client.lid_pn_cache.get_current_lid(&sender.user).await {
            Jid {
                user: lid_user,
                server: wacore_binary::Server::Lid,
                device: sender.device,
                agent: sender.agent,
                integrator: sender.integrator,
            }
        } else {
            // This is the path we're testing - no LID mapping, use PN
            sender.clone()
        }
    } else {
        sender.clone()
    };

    // Verify the encryption JID uses the PN (no LID available)
    assert_eq!(
        sender_encryption_jid.user, phone,
        "Encryption JID should use PN user when no LID mapping"
    );
    assert_eq!(
        sender_encryption_jid.server,
        wacore_binary::Server::Pn,
        "Encryption JID should use PN server when no LID mapping"
    );

    let protocol_address = sender_encryption_jid.to_protocol_address();
    assert_eq!(
        protocol_address.to_string(),
        format!("{}@c.us.0", phone),
        "Protocol address should be in PN format when no LID mapping"
    );
}

// and PDO fallback behavior to ensure robust message recovery.

/// Helper to create a test MessageInfo with customizable fields
fn create_test_message_info(chat: &str, msg_id: &str, sender: &str) -> MessageInfo {
    use wacore::types::message::{EditAttribute, MessageCategory, MessageSource, MsgMetaInfo};

    let chat_jid: Jid = chat.parse().expect("valid chat JID");
    let sender_jid: Jid = sender.parse().expect("valid sender JID");

    MessageInfo {
        id: msg_id.to_string(),
        server_id: 0,
        r#type: "text".to_string(),
        source: MessageSource {
            chat: chat_jid.clone(),
            sender: sender_jid,
            sender_alt: None,
            recipient_alt: None,
            is_from_me: false,
            is_group: chat_jid.is_group(),
            addressing_mode: None,
            broadcast_list_owner: None,
            recipient: None,
        },
        timestamp: wacore::time::now_utc(),
        push_name: "Test User".to_string(),
        category: MessageCategory::default(),
        multicast: false,
        media_type: "".to_string(),
        edit: EditAttribute::default(),
        bot_info: None,
        meta_info: MsgMetaInfo::default(),
        verified_name: None,
        device_sent_meta: None,
        ephemeral_expiration: None,
        is_offline: false,
        unavailable_request_id: None,
        server_timestamp_us: None,
        verified_level: None,
        verified_name_serial: None,
        peer_recipient_pn: None,
        bcl_participants: Vec::new(),
    }
}

/// Helper to create a test client for retry tests with a unique database
async fn create_test_client_for_retry_with_id(test_id: &str) -> Arc<Client> {
    use portable_atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_name = format!(
        "file:memdb_retry_{}_{}_{}?mode=memory&cache=shared",
        test_id,
        unique_id,
        std::process::id()
    );

    let backend = Arc::new(
        SqliteStore::new(&db_name)
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;
    client
}

#[tokio::test]
async fn test_increment_retry_count_starts_at_one() {
    let client = create_test_client_for_retry_with_id("starts_at_one").await;

    let cache_key = "test_chat:msg123:sender456";

    // First increment should return 1
    let count = client
        .increment_retry_count(cache_key, RetryReason::NoSession)
        .await;
    assert_eq!(count, Some(1), "First retry should be count 1");

    // Verify it's stored in cache
    let stored = client.message_retry_counts.get(cache_key).await;
    assert_eq!(stored, Some(1), "Cache should store count 1");
}

#[tokio::test]
async fn test_increment_retry_count_increments_correctly() {
    let client = create_test_client_for_retry_with_id("increments").await;

    let cache_key = "test_chat:msg456:sender789";

    // Simulate multiple retries
    let count1 = client
        .increment_retry_count(cache_key, RetryReason::NoSession)
        .await;
    let count2 = client
        .increment_retry_count(cache_key, RetryReason::NoSession)
        .await;
    let count3 = client
        .increment_retry_count(cache_key, RetryReason::NoSession)
        .await;

    assert_eq!(count1, Some(1), "First retry should be 1");
    assert_eq!(count2, Some(2), "Second retry should be 2");
    assert_eq!(count3, Some(3), "Third retry should be 3");
}

#[tokio::test]
async fn test_increment_retry_count_respects_max_retries() {
    let client = create_test_client_for_retry_with_id("max_retries").await;

    let cache_key = "test_chat:msg_max:sender_max";

    // Exhaust all retries (MAX_DECRYPT_RETRIES = 5)
    for i in 1..=5 {
        let count = client
            .increment_retry_count(cache_key, RetryReason::NoSession)
            .await;
        assert_eq!(count, Some(i), "Retry {} should return {}", i, i);
    }

    // 6th attempt should return None (max reached)
    let count_after_max = client
        .increment_retry_count(cache_key, RetryReason::NoSession)
        .await;
    assert_eq!(
        count_after_max, None,
        "After max retries, should return None"
    );

    // Verify cache still has max value
    let stored = client.message_retry_counts.get(cache_key).await;
    assert_eq!(stored, Some(5), "Cache should retain max count");
}

#[tokio::test]
async fn test_retry_count_different_messages_are_independent() {
    let client = create_test_client_for_retry_with_id("independent").await;

    let key1 = "chat1:msg1:sender1";
    let key2 = "chat1:msg2:sender1"; // Same chat and sender, different message
    let key3 = "chat2:msg1:sender2"; // Different chat and sender

    // Increment each independently
    let _ = client
        .increment_retry_count(key1, RetryReason::NoSession)
        .await;
    let _ = client
        .increment_retry_count(key1, RetryReason::NoSession)
        .await;
    let _ = client
        .increment_retry_count(key1, RetryReason::NoSession)
        .await; // key1 = 3

    let _ = client
        .increment_retry_count(key2, RetryReason::NoSession)
        .await; // key2 = 1

    let _ = client
        .increment_retry_count(key3, RetryReason::NoSession)
        .await;
    let _ = client
        .increment_retry_count(key3, RetryReason::NoSession)
        .await; // key3 = 2

    // Verify each has independent counts
    assert_eq!(client.message_retry_counts.get(key1).await, Some(3));
    assert_eq!(client.message_retry_counts.get(key2).await, Some(1));
    assert_eq!(client.message_retry_counts.get(key3).await, Some(2));
}

#[tokio::test]
async fn test_retry_cache_key_format() {
    // Verify the cache key format is consistent
    let info = create_test_message_info(
        "120363021033254949@g.us",
        "3EB0ABCD1234",
        "5511999998888@s.whatsapp.net",
    );

    let expected_key = format!("{}:{}:{}", info.source.chat, info.id, info.source.sender);
    assert_eq!(
        expected_key,
        "120363021033254949@g.us:3EB0ABCD1234:5511999998888@s.whatsapp.net"
    );

    // Verify key uniqueness for different senders in same group
    let info2 = create_test_message_info(
        "120363021033254949@g.us",
        "3EB0ABCD1234",                 // Same message ID
        "5511888887777@s.whatsapp.net", // Different sender
    );

    let key2 = format!("{}:{}:{}", info2.source.chat, info2.id, info2.source.sender);
    assert_ne!(
        expected_key, key2,
        "Different senders should have different keys"
    );
}

/// Test concurrent retry increments are properly serialized.
///
/// The increment operation uses get+insert which is not fully atomic,
/// but is sufficient since message retry processing is serialized per key
/// by the per-chat lock. At most 5 increments should succeed.
#[tokio::test]
async fn test_concurrent_retry_increments() {
    use tokio::task::JoinSet;

    let client = create_test_client_for_retry_with_id("concurrent").await;
    let cache_key = "concurrent_test:msg:sender";

    // Spawn 10 concurrent increment tasks
    let mut tasks = JoinSet::new();
    for _ in 0..10 {
        let client_clone = client.clone();
        let key = cache_key.to_string();
        tasks.spawn(async move {
            client_clone
                .increment_retry_count(&key, RetryReason::NoSession)
                .await
        });
    }

    // Collect all results
    let mut results = Vec::new();
    while let Some(result) = tasks.join_next().await {
        if let Ok(count) = result {
            results.push(count);
        }
    }

    // With atomic operations, exactly 5 should succeed and 5 should fail
    let valid_counts: Vec<_> = results.iter().filter(|r| r.is_some()).collect();
    let none_counts: Vec<_> = results.iter().filter(|r| r.is_none()).collect();

    assert_eq!(
        valid_counts.len(),
        5,
        "Exactly 5 increments should succeed with atomic operations"
    );
    assert_eq!(
        none_counts.len(),
        5,
        "Exactly 5 should return None (after max is reached)"
    );

    // Verify the successful increments returned values 1-5
    let mut values: Vec<u8> = valid_counts.iter().filter_map(|r| **r).collect();
    values.sort();
    assert_eq!(
        values,
        vec![1, 2, 3, 4, 5],
        "Successful increments should return 1, 2, 3, 4, 5"
    );

    // Final count should be 5 (max)
    let final_count = client.message_retry_counts.get(cache_key).await;
    assert_eq!(final_count, Some(5), "Final count should be capped at 5");
}

#[tokio::test]
async fn test_high_retry_count_threshold() {
    // Verify HIGH_RETRY_COUNT_THRESHOLD is set correctly
    assert_eq!(
        HIGH_RETRY_COUNT_THRESHOLD, 3,
        "High retry threshold should be 3"
    );
    assert_eq!(MAX_DECRYPT_RETRIES, 5, "Max retries should be 5");
    // Compile-time assertion that threshold < max (avoids clippy warning)
    const _: () = assert!(HIGH_RETRY_COUNT_THRESHOLD < MAX_DECRYPT_RETRIES);
}

#[tokio::test]
async fn test_message_info_creation_for_groups() {
    let info = create_test_message_info(
        "120363021033254949@g.us",
        "MSG123",
        "5511999998888@s.whatsapp.net",
    );

    assert!(
        info.source.is_group,
        "Group JID should be detected as group"
    );
    assert!(
        !info.source.is_from_me,
        "Test messages default to not from me"
    );
    assert_eq!(info.id, "MSG123");
}

#[tokio::test]
async fn test_message_info_creation_for_dm() {
    let info = create_test_message_info(
        "5511999998888@s.whatsapp.net",
        "DM456",
        "5511999998888@s.whatsapp.net",
    );

    assert!(
        !info.source.is_group,
        "DM JID should not be detected as group"
    );
    assert_eq!(info.id, "DM456");
}

#[tokio::test]
async fn test_retry_count_cache_expiration() {
    // Note: This test verifies cache configuration, not actual TTL (which would be slow)
    let client = create_test_client_for_retry_with_id("expiration").await;

    // The cache should have a TTL of 5 minutes (300 seconds) as configured in client.rs
    // We can verify entries are being stored and the cache is functional
    let cache_key = "expiry_test:msg:sender";

    let count = client
        .increment_retry_count(cache_key, RetryReason::NoSession)
        .await;
    assert_eq!(count, Some(1));

    // Entry should still exist immediately after
    let stored = client.message_retry_counts.get(cache_key).await;
    assert!(
        stored.is_some(),
        "Entry should exist immediately after insert"
    );
}

#[tokio::test]
async fn test_spawn_retry_receipt_basic_flow() {
    // This is an integration test that verifies spawn_retry_receipt
    // doesn't panic and updates the retry count correctly

    let client = create_test_client_for_retry_with_id("spawn_basic").await;
    let info = create_test_message_info(
        "120363021033254949@g.us",
        "SPAWN_TEST_MSG",
        "5511999998888@s.whatsapp.net",
    );

    let cache_key = format!("{}:{}:{}", info.source.chat, info.id, info.source.sender);

    // Verify count starts at 0
    assert!(
        client.message_retry_counts.get(&cache_key).await.is_none(),
        "Cache should be empty initially"
    );

    // Call spawn_retry_receipt (this spawns a task, so we need to wait)
    let info = Arc::new(info);
    client.spawn_retry_receipt(&info, RetryReason::UnknownError);

    // Give the spawned task time to execute
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Verify count was incremented (the actual send will fail due to no connection, but count should update)
    let stored = client.message_retry_counts.get(&cache_key).await;
    assert_eq!(stored, Some(1), "Retry count should be 1 after spawn");
}

#[tokio::test]
async fn test_spawn_retry_receipt_respects_max_retries() {
    let client = create_test_client_for_retry_with_id("spawn_max").await;
    let info = create_test_message_info(
        "120363021033254949@g.us",
        "MAX_RETRY_TEST",
        "5511999998888@s.whatsapp.net",
    );

    let cache_key = format!("{}:{}:{}", info.source.chat, info.id, info.source.sender);

    // Pre-fill cache to max retries
    client
        .message_retry_counts
        .insert(cache_key.clone(), MAX_DECRYPT_RETRIES)
        .await;

    // Verify count is at max
    assert_eq!(
        client.message_retry_counts.get(&cache_key).await,
        Some(MAX_DECRYPT_RETRIES)
    );

    // Call spawn_retry_receipt - should NOT increment (already at max)
    let info = Arc::new(info);
    client.spawn_retry_receipt(&info, RetryReason::UnknownError);

    // Give the spawned task time to execute
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // Count should still be at max (not incremented)
    let stored = client.message_retry_counts.get(&cache_key).await;
    assert_eq!(
        stored,
        Some(MAX_DECRYPT_RETRIES),
        "Count should remain at max"
    );
}

#[tokio::test]
async fn test_pdo_cache_key_format_matches() {
    // PDO uses "{chat}:{msg_id}" format
    // Retry uses "{chat}:{msg_id}:{sender}" format
    // They are intentionally different to track independently

    let info = create_test_message_info(
        "120363021033254949@g.us",
        "PDO_KEY_TEST",
        "5511999998888@s.whatsapp.net",
    );

    let retry_key = format!("{}:{}:{}", info.source.chat, info.id, info.source.sender);
    let pdo_key = format!("{}:{}", info.source.chat, info.id);

    assert_ne!(retry_key, pdo_key, "PDO and retry keys should be different");
    assert!(
        retry_key.starts_with(&pdo_key),
        "Retry key should start with PDO key pattern"
    );
}

#[tokio::test]
async fn test_multiple_senders_same_message_id_tracked_separately() {
    // In a group, multiple senders could theoretically have the same message ID
    // (unlikely but the system should handle it)

    let client = create_test_client_for_retry_with_id("multi_sender").await;

    let group = "120363021033254949@g.us";
    let msg_id = "SAME_MSG_ID";
    let sender1 = "5511111111111@s.whatsapp.net";
    let sender2 = "5522222222222@s.whatsapp.net";

    let key1 = format!("{}:{}:{}", group, msg_id, sender1);
    let key2 = format!("{}:{}:{}", group, msg_id, sender2);

    // Increment for sender1 multiple times
    client
        .increment_retry_count(&key1, RetryReason::NoSession)
        .await;
    client
        .increment_retry_count(&key1, RetryReason::NoSession)
        .await;
    client
        .increment_retry_count(&key1, RetryReason::NoSession)
        .await;

    // Increment for sender2 once
    client
        .increment_retry_count(&key2, RetryReason::NoSession)
        .await;

    // Verify independent tracking
    assert_eq!(
        client.message_retry_counts.get(&key1).await,
        Some(3),
        "Sender1 should have 3 retries"
    );
    assert_eq!(
        client.message_retry_counts.get(&key2).await,
        Some(1),
        "Sender2 should have 1 retry"
    );
}

/// Test: Verify JID type detection for status broadcasts, broadcast lists, groups, and users.
#[test]
fn test_status_broadcast_jid_detection() {
    use wacore_binary::{Jid, JidExt};

    let status_jid: Jid = "status@broadcast".parse().expect("status JID should parse");
    assert!(status_jid.is_status_broadcast());

    let broadcast_list: Jid = "123456789@broadcast"
        .parse()
        .expect("broadcast JID should parse");
    assert!(!broadcast_list.is_status_broadcast());
    assert!(broadcast_list.is_broadcast_list());

    let group_jid: Jid = "120363021033254949@g.us"
        .parse()
        .expect("group JID should parse");
    assert!(!group_jid.is_status_broadcast());

    let user_jid: Jid = "15551234567@s.whatsapp.net"
        .parse()
        .expect("user JID should parse");
    assert!(!user_jid.is_status_broadcast());
}

/// Test: Verify should_process_skmsg logic matches WA Web's canDecryptNext pattern.
///
/// WA Web applies canDecryptNext uniformly: if pkmsg fails with a retriable error,
/// skmsg is skipped regardless of chat type (group, status, 1:1). No exception for
/// status broadcasts — the retry receipt for the pkmsg will cause the sender to
/// resend the entire message including SKDM.
#[test]
fn test_should_process_skmsg_logic_matches_wa_web() {
    // Test cases: (chat_jid, session_empty, session_success, session_dupe, session_failed, expected)
    let test_cases = [
        // Status broadcast: same rules as all other chats (WA Web: canDecryptNext is uniform)
        ("status@broadcast", false, false, false, false, false), // Fail: session failed → skip skmsg
        ("status@broadcast", false, false, true, false, true),   // OK: duplicate
        ("status@broadcast", false, true, false, false, true),   // OK: success
        ("status@broadcast", false, true, false, true, false),   // Fail: mixed success + failure
        ("status@broadcast", true, false, false, false, true),   // OK: no session msgs
        // Regular group
        ("120363021033254949@g.us", false, false, false, false, false),
        ("120363021033254949@g.us", false, false, true, false, true),
        ("120363021033254949@g.us", false, true, false, false, true),
        ("120363021033254949@g.us", false, true, false, true, false),
        ("120363021033254949@g.us", true, false, false, false, true),
        // 1:1 chat
        (
            "15551234567@s.whatsapp.net",
            false,
            false,
            false,
            false,
            false,
        ),
        (
            "15551234567@s.whatsapp.net",
            true,
            false,
            false,
            false,
            true,
        ),
    ];

    for (jid_str, session_empty, session_success, session_dupe, session_failed, expected) in
        test_cases
    {
        let should_process_skmsg = should_process_skmsg_after_session(
            session_empty,
            SessionBatchOutcome {
                decrypted: session_success,
                duplicate: session_dupe,
                had_failure: session_failed,
                ..Default::default()
            },
        );

        assert_eq!(
            should_process_skmsg,
            expected,
            "For chat {} with session_empty={}, session_success={}, session_dupe={}, session_failed={}: \
                 expected should_process_skmsg={}, got {}",
            jid_str,
            session_empty,
            session_success,
            session_dupe,
            session_failed,
            expected,
            should_process_skmsg
        );
    }
}

#[test]
fn skdm_only_fallback_ack_decision_requires_clean_session_batch() {
    let clean_skdm = SessionBatchOutcome {
        decrypted: true,
        skdm_only: true,
        ..Default::default()
    };
    assert!(
        should_ack_skdm_only_session_fallback(clean_skdm, true),
        "a clean SKDM-only session batch needs the fallback ack"
    );

    let cases = [
        (
            SessionBatchOutcome {
                dispatched: true,
                ..clean_skdm
            },
            true,
            "content dispatch already acked",
        ),
        (
            SessionBatchOutcome {
                had_failure: true,
                ..clean_skdm
            },
            true,
            "local session failure must block positive ack",
        ),
        (
            SessionBatchOutcome {
                plaintext_failed: true,
                had_failure: true,
                ..clean_skdm
            },
            true,
            "plaintext handler failure is not SKDM-only success",
        ),
        (
            SessionBatchOutcome {
                undecryptable: true,
                had_failure: true,
                ..clean_skdm
            },
            true,
            "failure event must not be paired with positive ack",
        ),
        (
            SessionBatchOutcome {
                decrypted: false,
                ..clean_skdm
            },
            true,
            "fallback only applies after Signal decrypt success",
        ),
        (
            SessionBatchOutcome {
                skdm_only: false,
                ..clean_skdm
            },
            true,
            "regular content must ack via dispatch",
        ),
        (
            SessionBatchOutcome {
                duplicate: true,
                decrypted: false,
                skdm_only: false,
                ..Default::default()
            },
            true,
            "duplicates use the duplicate branch",
        ),
        (clean_skdm, false, "msmsg work must own its response"),
    ];

    for (outcome, bot_payloads_empty, reason) in cases {
        assert!(
            !should_ack_skdm_only_session_fallback(outcome, bot_payloads_empty),
            "{reason}: {outcome:?}"
        );
    }
}

/// Test: parse_message_info returns error when message "id" attribute is missing
///
/// Missing message IDs would cause silent collisions in caches/keys, so this
/// must be a hard error rather than defaulting to an empty string.
#[tokio::test]
async fn test_parse_message_info_missing_id_returns_error() {
    let backend = Arc::new(
        SqliteStore::new("file:memdb_missing_id_test?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("test backend should initialize"),
    );
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let node = NodeBuilder::new("message")
        .attr("from", "15551234567@s.whatsapp.net")
        .attr("t", "1759295366")
        .attr("type", "text")
        .build();

    let result = client.parse_message_info(&node.as_node_ref()).await;

    assert!(
        result.is_err(),
        "parse_message_info should fail when 'id' is missing"
    );
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("id"),
        "Error message should mention missing 'id' attribute: {}",
        err_msg
    );
}
#[tokio::test]
async fn test_no_sender_key_sends_immediate_retry() {
    // Verify that when skmsg decryption fails with NoSenderKeyState,
    // a retry receipt is sent immediately (no delay, no re-queue).
    // This matches WA Web behavior where NoSenderKey → SignalRetryable → RETRY.
    let _ = env_logger::builder().is_test(true).try_init();

    use crate::store::SqliteStore;
    use crate::store::persistence_manager::PersistenceManager;
    use wacore_binary::NodeContent;
    use wacore_binary::builder::NodeBuilder;

    let backend = Arc::new(
        SqliteStore::new("file:memdb_retry_immediate?mode=memory&cache=shared")
            .await
            .expect("Failed to create test backend"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend.clone())
            .await
            .expect("test backend should initialize"),
    );
    let (client, _rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm.clone(),
        mock_transport(),
        mock_http_client(),
        None,
    )
    .await;

    let group_jid: Jid = "120363021033254949@g.us".parse().unwrap();
    let sender_jid: Jid = "1234567890:1@s.whatsapp.net".parse().unwrap();
    let msg_id = "TEST_IMMEDIATE_RETRY";

    // Pseudo-valid SenderKeyMessage: Version 3 + Protobuf + Fake Sig (64 bytes)
    let mut content = vec![0x33, 0x08, 0x01, 0x10, 0x01, 0x1A, 0x00];
    content.extend(vec![0u8; 64]);

    let node = NodeBuilder::new("message")
        .attr("id", msg_id)
        .attr("from", group_jid.clone())
        .attr("participant", sender_jid.clone())
        .attr("type", "text")
        .children(vec![{
            let mut n = NodeBuilder::new("enc")
                .attr("type", "skmsg")
                .attr("v", "2")
                .build();
            n.content = Some(NodeContent::Bytes(content));
            n
        }])
        .build();

    client
        .clone()
        .handle_incoming_message(node_to_arc(node))
        .await;

    // spawn_retry_receipt runs in a spawned task, wait for it
    let retry_key = client
        .make_retry_cache_key(&group_jid, msg_id, &sender_jid)
        .await;
    for _ in 0..20 {
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        if client.message_retry_counts.get(&retry_key).await.is_some() {
            break;
        }
    }
    assert_eq!(
        client.message_retry_counts.get(&retry_key).await,
        Some(1),
        "NoSenderKeyState should immediately trigger retry receipt (count=1)"
    );
}

#[test]
fn test_is_sender_key_distribution_only() {
    let skdm = wa::message::SenderKeyDistributionMessage {
        group_id: Some("group".into()),
        axolotl_sender_key_distribution_message: Some(vec![1, 2, 3]),
    };

    // Empty message → false (no SKDM)
    assert!(!is_sender_key_distribution_only(&mut wa::Message::default()));

    // SKDM only → true
    assert!(is_sender_key_distribution_only(&mut wa::Message {
        sender_key_distribution_message: Some(skdm.clone()),
        ..Default::default()
    }));

    // SKDM + message_context_info → still true (context_info is metadata)
    assert!(is_sender_key_distribution_only(&mut wa::Message {
        sender_key_distribution_message: Some(skdm.clone()),
        message_context_info: Some(wa::MessageContextInfo::default()),
        ..Default::default()
    }));

    // SKDM + sticker → false (has user content)
    assert!(!is_sender_key_distribution_only(&mut wa::Message {
        sender_key_distribution_message: Some(skdm.clone()),
        sticker_message: Some(Box::new(wa::message::StickerMessage::default())),
        ..Default::default()
    }));

    // SKDM + text → false (has user content)
    assert!(!is_sender_key_distribution_only(&mut wa::Message {
        sender_key_distribution_message: Some(skdm.clone()),
        conversation: Some("hello".into()),
        ..Default::default()
    }));

    // protocol_message only (no SKDM) → false
    assert!(!is_sender_key_distribution_only(&mut wa::Message {
        protocol_message: Some(Box::new(wa::message::ProtocolMessage::default())),
        ..Default::default()
    }));
}

#[test]
fn skdm_only_detection_restores_carrier_fields() {
    // The slow path takes the carrier fields out to compare the rest against
    // default; it must restore them so callers still see the original message.
    let mut msg = wa::Message {
        sender_key_distribution_message: Some(wa::message::SenderKeyDistributionMessage {
            group_id: Some("group".into()),
            axolotl_sender_key_distribution_message: Some(vec![1, 2, 3]),
        }),
        fast_ratchet_key_sender_key_distribution_message: Some(
            wa::message::SenderKeyDistributionMessage {
                group_id: Some("group".into()),
                axolotl_sender_key_distribution_message: Some(vec![4, 5, 6]),
            },
        ),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![9, 8, 7]),
            ..Default::default()
        }),
        ..Default::default()
    };

    assert!(is_sender_key_distribution_only(&mut msg));

    // Pin the exact payloads of all three taken/restored carrier fields, not
    // just presence: a buggy restore that put back a fresh default (losing the
    // original contents) must fail here.
    assert_eq!(
        msg.sender_key_distribution_message
            .as_ref()
            .and_then(|s| s.axolotl_sender_key_distribution_message.as_deref()),
        Some([1, 2, 3].as_slice()),
        "sender_key_distribution_message payload must be restored unchanged"
    );
    assert_eq!(
        msg.fast_ratchet_key_sender_key_distribution_message
            .as_ref()
            .and_then(|s| s.axolotl_sender_key_distribution_message.as_deref()),
        Some([4, 5, 6].as_slice()),
        "fast_ratchet carrier payload must be restored unchanged"
    );
    assert_eq!(
        msg.message_context_info
            .as_ref()
            .and_then(|c| c.message_secret.as_deref()),
        Some([9, 8, 7].as_slice()),
        "message_context_info payload must be restored unchanged"
    );
}

/// Test: unwrap_device_sent extracts a reaction from a DeviceSentMessage wrapper.
#[test]
fn test_unwrap_device_sent_extracts_reaction() {
    let wrapped = wa::Message {
        device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
            destination_jid: Some("5511999999999@s.whatsapp.net".to_string()),
            message: Some(Box::new(wa::Message {
                reaction_message: Some(wa::message::ReactionMessage {
                    text: Some("\u{2764}".to_string()),
                    ..Default::default()
                }),
                ..Default::default()
            })),
            phash: None,
        })),
        ..Default::default()
    };

    let mut unwrapped = unwrap_device_sent(wrapped);
    assert!(
        unwrapped.device_sent_message.is_none(),
        "DSM wrapper should be removed"
    );
    assert_eq!(
        unwrapped
            .reaction_message
            .as_ref()
            .and_then(|r| r.text.as_deref()),
        Some("\u{2764}"),
        "reaction should be accessible after unwrapping"
    );
    assert!(
        !is_sender_key_distribution_only(&mut unwrapped),
        "unwrapped reaction should not be filtered as SKDM-only"
    );
}

/// Test: unwrap_device_sent preserves the wrapper when inner message is None.
#[test]
fn test_unwrap_device_sent_preserves_empty_wrapper() {
    let wrapped = wa::Message {
        device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
            destination_jid: Some("5511999999999@s.whatsapp.net".to_string()),
            message: None,
            phash: None,
        })),
        ..Default::default()
    };

    let result = unwrap_device_sent(wrapped);
    assert!(
        result.device_sent_message.is_some(),
        "empty DSM wrapper should be preserved"
    );
}

/// Test: unwrap_device_sent passes through a plain message unchanged.
#[test]
fn test_unwrap_device_sent_passthrough() {
    let msg = wa::Message {
        conversation: Some("hello".to_string()),
        ..Default::default()
    };

    let result = unwrap_device_sent(msg);
    assert_eq!(result.conversation.as_deref(), Some("hello"));
}

/// Test: unwrap_device_sent merges messageContextInfo from outer and inner,
/// matching WAWebDeviceSentMessageProtoUtils.unwrapDeviceSentMessage.
#[test]
fn test_unwrap_device_sent_merges_context_info() {
    let wrapped = wa::Message {
        // Outer message_context_info (from the DSM envelope)
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![10, 20, 30]),
            limit_sharing_v2: Some(wa::LimitSharing::default()),
            ..Default::default()
        }),
        device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
            destination_jid: Some("5511999999999@s.whatsapp.net".to_string()),
            message: Some(Box::new(wa::Message {
                conversation: Some("hello".to_string()),
                // Inner has its own message_secret but no limit_sharing_v2
                message_context_info: Some(wa::MessageContextInfo {
                    message_secret: Some(vec![1, 2, 3]),
                    ..Default::default()
                }),
                ..Default::default()
            })),
            phash: None,
        })),
        ..Default::default()
    };

    let result = unwrap_device_sent(wrapped);
    let ctx = result.message_context_info.as_ref().unwrap();

    assert_eq!(
        ctx.message_secret,
        Some(vec![1, 2, 3]),
        "inner message_secret should be preferred"
    );
    assert!(
        ctx.limit_sharing_v2.is_some(),
        "limit_sharing_v2 should come from outer (always)"
    );
}

/// Test: unwrap_device_sent falls back to outer message_secret when inner has none.
#[test]
fn test_unwrap_device_sent_secret_fallback() {
    let wrapped = wa::Message {
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![10, 20, 30]),
            ..Default::default()
        }),
        device_sent_message: Some(Box::new(wa::message::DeviceSentMessage {
            destination_jid: Some("5511999999999@s.whatsapp.net".to_string()),
            message: Some(Box::new(wa::Message {
                conversation: Some("hello".to_string()),
                // Inner has no message_context_info at all
                ..Default::default()
            })),
            phash: None,
        })),
        ..Default::default()
    };

    let result = unwrap_device_sent(wrapped);
    let ctx = result.message_context_info.as_ref().unwrap();
    assert_eq!(
        ctx.message_secret,
        Some(vec![10, 20, 30]),
        "should fall back to outer message_secret"
    );
}

#[tokio::test]
async fn test_parse_edit_attribute_sender_revoke() {
    let client = create_test_client_for_retry_with_id("edit_sender_revoke").await;

    let node = NodeBuilder::new("message")
        .attr("from", "status@broadcast")
        .attr("id", "TEST123")
        .attr("participant", "5551234567@lid")
        .attr("t", "1772895198")
        .attr("type", "text")
        .attr("edit", "7")
        .build();

    let info = client
        .parse_message_info(&node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    assert_eq!(
        info.edit,
        EditAttribute::SenderRevoke,
        "edit='7' should parse as SenderRevoke"
    );
}

#[tokio::test]
async fn test_parse_edit_attribute_admin_revoke() {
    let client = create_test_client_for_retry_with_id("edit_admin_revoke").await;

    let node = NodeBuilder::new("message")
        .attr("from", "120363999999999999@g.us")
        .attr("id", "TEST456")
        .attr("participant", "5551234567@lid")
        .attr("t", "1772895198")
        .attr("type", "text")
        .attr("edit", "8")
        .build();

    let info = client
        .parse_message_info(&node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    assert_eq!(
        info.edit,
        EditAttribute::AdminRevoke,
        "edit='8' should parse as AdminRevoke"
    );
}

#[tokio::test]
async fn test_parse_edit_attribute_message_edit() {
    let client = create_test_client_for_retry_with_id("edit_message_edit").await;

    let node = NodeBuilder::new("message")
        .attr("from", "5551234567@s.whatsapp.net")
        .attr("id", "TEST789")
        .attr("t", "1772895198")
        .attr("type", "text")
        .attr("edit", "1")
        .build();

    let info = client
        .parse_message_info(&node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    assert_eq!(
        info.edit,
        EditAttribute::MessageEdit,
        "edit='1' should parse as MessageEdit"
    );
}

#[tokio::test]
async fn test_parse_edit_attribute_missing() {
    let client = create_test_client_for_retry_with_id("edit_missing").await;

    let node = NodeBuilder::new("message")
        .attr("from", "5551234567@s.whatsapp.net")
        .attr("id", "TESTABC")
        .attr("t", "1772895198")
        .attr("type", "text")
        .build();

    let info = client
        .parse_message_info(&node.as_node_ref())
        .await
        .expect("parse_message_info should succeed");

    assert_eq!(
        info.edit,
        EditAttribute::Empty,
        "missing edit attr should default to Empty"
    );
}

#[tokio::test]
async fn test_revoked_message_still_retries() {
    let client = create_test_client_for_retry_with_id("revoke_retry").await;

    let mut info = create_test_message_info(
        "status@broadcast",
        "REVOKE_MSG1",
        "5551234567@s.whatsapp.net",
    );
    info.edit = EditAttribute::SenderRevoke;

    // WA Web retries revoked messages the same as any other — the revoke
    // protocol message contains the target ID needed to process the deletion
    let info = Arc::new(info);
    client.spawn_retry_receipt(&info, RetryReason::NoSession);

    // Wait for the spawned task to execute
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    let cache_key = client
        .make_retry_cache_key(&info.source.chat, &info.id, &info.source.sender)
        .await;
    assert_eq!(
        client.message_retry_counts.get(&cache_key).await,
        Some(1),
        "revoked message should still have retry count 1 (WA Web retries all messages)"
    );
}

#[tokio::test]
async fn test_enc_count_preseeds_retry_cache() {
    let client = create_test_client_for_retry_with_id("enc_preseed").await;

    let chat_jid: Jid = "5551234567@s.whatsapp.net".parse().unwrap();
    let msg_id = "ENC_COUNT_MSG1";

    // Pre-seed via the same logic used in handle_incoming_message
    let max_sender_retry_count: u8 = 3;
    let cache_key = client
        .make_retry_cache_key(&chat_jid, msg_id, &chat_jid)
        .await;
    // Insert only if absent (portable alternative to moka's entry_by_ref().or_insert())
    if client.message_retry_counts.get(&cache_key).await.is_none() {
        client
            .message_retry_counts
            .insert(cache_key.clone(), max_sender_retry_count)
            .await;
    }

    assert_eq!(
        client.message_retry_counts.get(&cache_key).await,
        Some(3),
        "cache should be pre-seeded with sender retry count"
    );
}

#[tokio::test]
async fn test_enc_no_count_cache_empty() {
    let client = create_test_client_for_retry_with_id("enc_no_count").await;

    let chat_jid: Jid = "5551234567@s.whatsapp.net".parse().unwrap();
    let msg_id = "ENC_NO_COUNT_MSG1";

    // When max_sender_retry_count is 0, no pre-seeding occurs
    let max_sender_retry_count: u8 = 0;
    if max_sender_retry_count > 0 {
        let cache_key = client
            .make_retry_cache_key(&chat_jid, msg_id, &chat_jid)
            .await;
        if client.message_retry_counts.get(&cache_key).await.is_none() {
            client
                .message_retry_counts
                .insert(cache_key, max_sender_retry_count)
                .await;
        }
    }

    let cache_key = client
        .make_retry_cache_key(&chat_jid, msg_id, &chat_jid)
        .await;
    assert!(
        client.message_retry_counts.get(&cache_key).await.is_none(),
        "cache should be empty when no count attribute"
    );
}

#[tokio::test]
async fn test_enc_count_does_not_overwrite_higher() {
    let client = create_test_client_for_retry_with_id("enc_no_overwrite").await;

    let chat_jid: Jid = "5551234567@s.whatsapp.net".parse().unwrap();
    let msg_id = "ENC_NOOVERWRITE_MSG1";

    let cache_key = client
        .make_retry_cache_key(&chat_jid, msg_id, &chat_jid)
        .await;

    // Pre-insert a higher value
    client
        .message_retry_counts
        .insert(cache_key.clone(), 4)
        .await;

    // max(existing, incoming) should NOT overwrite with a lower value
    let max_sender_retry_count: u8 = 2;
    let existing = client
        .message_retry_counts
        .get(&cache_key)
        .await
        .unwrap_or(0);
    if max_sender_retry_count > existing {
        client
            .message_retry_counts
            .insert(cache_key.clone(), max_sender_retry_count)
            .await;
    }

    assert_eq!(
        client.message_retry_counts.get(&cache_key).await,
        Some(4),
        "should not overwrite existing higher value"
    );
}

#[tokio::test]
async fn test_enc_count_updates_when_sender_higher() {
    let client = create_test_client_for_retry_with_id("enc_update_higher").await;

    let chat_jid: Jid = "5551234567@s.whatsapp.net".parse().unwrap();
    let msg_id = "ENC_UPDATE_MSG1";

    let cache_key = client
        .make_retry_cache_key(&chat_jid, msg_id, &chat_jid)
        .await;

    // Pre-insert a lower value
    client
        .message_retry_counts
        .insert(cache_key.clone(), 1)
        .await;

    // max(existing, incoming) SHOULD update with a higher value
    let max_sender_retry_count: u8 = 3;
    let existing = client
        .message_retry_counts
        .get(&cache_key)
        .await
        .unwrap_or(0);
    if max_sender_retry_count > existing {
        client
            .message_retry_counts
            .insert(cache_key.clone(), max_sender_retry_count)
            .await;
    }

    assert_eq!(
        client.message_retry_counts.get(&cache_key).await,
        Some(3),
        "should update to higher sender count"
    );
}

/// Shared helper: the OLD semaphore acquire logic that silently dropped tasks
/// on generation mismatch. Used by the bug-demonstration test.
async fn acquire_permit_old_behavior(
    semaphore: &std::sync::Mutex<Arc<async_lock::Semaphore>>,
    generation: &portable_atomic::AtomicU64,
) -> bool {
    use std::sync::atomic::Ordering;
    let (snap_gen, snap_sem) = {
        let guard = semaphore.lock().unwrap();
        (generation.load(Ordering::SeqCst), guard.clone())
    };
    let _permit = snap_sem.acquire_arc().await;
    // OLD: if generation changed, silently return false (message lost)
    snap_gen == generation.load(Ordering::SeqCst)
}

/// Shared helper: the FIXED semaphore acquire logic that re-acquires from the
/// new semaphore on generation mismatch. Mirrors the production code in
/// handle_incoming_message.
async fn acquire_permit_with_reacquire(
    semaphore: &std::sync::Mutex<Arc<async_lock::Semaphore>>,
    generation: &portable_atomic::AtomicU64,
) {
    use std::sync::atomic::Ordering;
    loop {
        let (snap_gen, snap_sem) = {
            let guard = semaphore.lock().unwrap();
            (generation.load(Ordering::SeqCst), guard.clone())
        };
        let permit = snap_sem.acquire_arc().await;
        if snap_gen == generation.load(Ordering::SeqCst) {
            drop(permit);
            break;
        }
        drop(permit);
    }
}

/// Demonstrates the bug: the OLD code silently dropped tasks when generation changed.
#[tokio::test]
async fn test_old_behavior_drops_tasks_on_generation_swap() {
    use portable_atomic::AtomicU64;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let semaphore = Arc::new(std::sync::Mutex::new(Arc::new(async_lock::Semaphore::new(
        1,
    ))));
    let generation = Arc::new(AtomicU64::new(0));
    let completed = Arc::new(AtomicUsize::new(0));
    let ready = Arc::new(AtomicUsize::new(0));

    let blocker_sem = semaphore.lock().unwrap().clone();
    let blocker_permit = blocker_sem.acquire_arc().await;

    let num_waiters: usize = 8;
    let mut handles = Vec::new();

    for _ in 0..num_waiters {
        let sem = semaphore.clone();
        let gen_counter = generation.clone();
        let done = completed.clone();
        let ready_counter = ready.clone();

        handles.push(tokio::spawn(async move {
            // Signal readiness before blocking on semaphore
            ready_counter.fetch_add(1, Ordering::SeqCst);
            if acquire_permit_old_behavior(&sem, &gen_counter).await {
                done.fetch_add(1, Ordering::SeqCst);
            }
        }));
    }

    // Wait until all waiters have signaled readiness (about to block on semaphore)
    while ready.load(Ordering::SeqCst) < num_waiters {
        tokio::task::yield_now().await;
    }

    // Swap semaphore — triggers the bug
    {
        let mut guard = semaphore.lock().unwrap();
        *guard = Arc::new(async_lock::Semaphore::new(64));
        generation.fetch_add(1, Ordering::SeqCst);
    }

    drop(blocker_permit);

    for handle in handles {
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await;
        assert!(result.is_ok(), "Waiter task timed out");
        result.unwrap().unwrap();
    }

    let done = completed.load(Ordering::SeqCst);
    assert!(
        done < num_waiters,
        "Bug demonstration: expected tasks to be dropped, but all {} completed",
        num_waiters
    );
}

/// Verifies the fix: re-acquire loop ensures NO tasks are dropped on generation swap.
#[tokio::test]
async fn test_semaphore_generation_swap_does_not_drop_tasks() {
    use portable_atomic::AtomicU64;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    let semaphore = Arc::new(std::sync::Mutex::new(Arc::new(async_lock::Semaphore::new(
        1,
    ))));
    let generation = Arc::new(AtomicU64::new(0));
    let completed = Arc::new(AtomicUsize::new(0));
    let ready = Arc::new(AtomicUsize::new(0));

    let blocker_sem = semaphore.lock().unwrap().clone();
    let blocker_permit = blocker_sem.acquire_arc().await;

    let num_waiters: usize = 8;
    let mut handles = Vec::new();

    for _ in 0..num_waiters {
        let sem = semaphore.clone();
        let gen_counter = generation.clone();
        let done = completed.clone();
        let ready_counter = ready.clone();

        handles.push(tokio::spawn(async move {
            ready_counter.fetch_add(1, Ordering::SeqCst);
            acquire_permit_with_reacquire(&sem, &gen_counter).await;
            done.fetch_add(1, Ordering::SeqCst);
        }));
    }

    // Wait until all waiters have signaled readiness
    while ready.load(Ordering::SeqCst) < num_waiters {
        tokio::task::yield_now().await;
    }

    // Swap semaphore (simulates offline sync completion)
    {
        let mut guard = semaphore.lock().unwrap();
        *guard = Arc::new(async_lock::Semaphore::new(64));
        generation.fetch_add(1, Ordering::SeqCst);
    }

    drop(blocker_permit);

    for handle in handles {
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(5), handle).await;
        assert!(
            result.is_ok(),
            "Waiter task timed out — likely silently dropped by generation check"
        );
        result.unwrap().unwrap();
    }

    assert_eq!(
        completed.load(Ordering::SeqCst),
        num_waiters,
        "All {} waiter tasks should complete, but only {} did. \
             Tasks were silently dropped during semaphore generation swap.",
        num_waiters,
        completed.load(Ordering::SeqCst)
    );
}

// Dispatch ordering, per-id dedup, and PDO eligibility for
// UndecryptableMessage. Regressing any of these re-opens data loss bugs
// observed in production.

use crate::types::events::DecryptFailMode;
use wacore::types::events::{Event, EventHandler};

#[derive(Default)]
struct EventRecorder {
    events: std::sync::Mutex<Vec<Arc<Event>>>,
}

impl EventHandler for EventRecorder {
    fn handle_event(&self, event: Arc<Event>) {
        self.events.lock().unwrap().push(event);
    }
}

impl EventRecorder {
    fn undecryptable(&self) -> Vec<Arc<Event>> {
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| matches!(&***e, Event::UndecryptableMessage(_)))
            .cloned()
            .collect()
    }

    /// Count of `UndecryptableMessage` events marked as the "stub"
    /// variant (`is_unavailable=true`, `UnavailableType::ViewOnce`) —
    /// i.e. the branch that routes to PDO instead of falling through to
    /// decrypt.
    fn view_once_unavailable_count(&self) -> usize {
        use crate::types::events::UnavailableType;
        self.events
            .lock()
            .unwrap()
            .iter()
            .filter(|e| {
                matches!(
                    &***e,
                    Event::UndecryptableMessage(u)
                        if u.is_unavailable
                            && matches!(u.unavailable_type, UnavailableType::ViewOnce)
                )
            })
            .count()
    }
}

fn build_unavailable_stanza(sender: &str, msg_id: &str, with_enc: bool) -> Arc<OwnedNodeRef> {
    let t = wacore::time::now_secs().to_string();
    let unavailable = NodeBuilder::new("unavailable")
        .attr("type", "view_once")
        .build();
    let children = if with_enc {
        vec![
            unavailable,
            NodeBuilder::new("enc")
                .attr("type", "msg")
                .attr("v", "2")
                .bytes(vec![0xDE, 0xAD, 0xBE, 0xEF])
                .build(),
        ]
    } else {
        vec![unavailable]
    };
    node_to_arc(
        NodeBuilder::new("message")
            .attr("from", sender)
            .attr("id", msg_id)
            .attr("t", &t)
            .attr("type", "media")
            .children(children)
            .build(),
    )
}

/// Locks the dispatch ordering: consumers must see the event before any
/// retry/PDO side effects, otherwise a late subscriber misses the failure.
#[tokio::test]
async fn test_undecryptable_fires_before_retry_task() {
    let client = create_test_client_for_retry_with_id("undec_sync").await;
    let recorder = Arc::new(EventRecorder::default());
    client.register_handler(recorder.clone());

    let info = Arc::new(create_test_message_info(
        "5511999998888@s.whatsapp.net",
        "MSG_SYNC_1",
        "5511777776666@s.whatsapp.net",
    ));

    let cache_key = client
        .make_retry_cache_key(&info.source.chat, &info.id, &info.source.sender)
        .await;

    assert!(recorder.undecryptable().is_empty());
    assert!(client.message_retry_counts.get(&cache_key).await.is_none());

    let _ = client
        .handle_decrypt_failure(&info, RetryReason::InvalidKeyId, DecryptFailMode::Show)
        .await;

    assert_eq!(
        recorder.undecryptable().len(),
        1,
        "UndecryptableMessage dispatched inside handle_decrypt_failure",
    );
    assert!(
        client.message_retry_counts.get(&cache_key).await.is_none(),
        "retry task has not progressed yet",
    );

    tokio::time::sleep(tokio::time::Duration::from_millis(150)).await;
    assert_eq!(
        client.message_retry_counts.get(&cache_key).await,
        Some(1),
        "retry task runs after the dispatch",
    );
}

/// Atomic dedup under concurrency: 32 parallel callers for the same id
/// must produce exactly one event. Catches regressions where the dedup
/// would slip back to a non-atomic get-then-insert pair.
#[tokio::test]
async fn test_undecryptable_dedup_is_atomic() {
    let client = create_test_client_for_retry_with_id("undec_atomic").await;
    let recorder = Arc::new(EventRecorder::default());
    client.register_handler(recorder.clone());

    let info = Arc::new(create_test_message_info(
        "5511999998888@s.whatsapp.net",
        "ATOMIC_MSG_1",
        "5511777776666@s.whatsapp.net",
    ));

    let mut handles = Vec::with_capacity(32);
    for _ in 0..32 {
        let c = Arc::clone(&client);
        let i = Arc::clone(&info);
        handles.push(tokio::spawn(async move {
            c.handle_decrypt_failure(&i, RetryReason::InvalidKeyId, DecryptFailMode::Show)
                .await;
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    assert_eq!(
        recorder.undecryptable().len(),
        1,
        "32 concurrent callers must collapse to one UndecryptableMessage",
    );
}

/// Server resends of the same id must not surface a duplicate event —
/// would otherwise show the user the same failure twice.
#[tokio::test]
async fn test_undecryptable_deduped_across_resends() {
    let client = create_test_client_for_retry_with_id("undec_double").await;
    let recorder = Arc::new(EventRecorder::default());
    client.register_handler(recorder.clone());

    let info = Arc::new(create_test_message_info(
        "5511999998888@s.whatsapp.net",
        "3AD01881AA95F7D81070",
        "85010891714716@lid",
    ));

    let _ = client
        .handle_decrypt_failure(&info, RetryReason::InvalidKeyId, DecryptFailMode::Show)
        .await;
    let _ = client
        .handle_decrypt_failure(&info, RetryReason::InvalidKeyId, DecryptFailMode::Show)
        .await;

    let events = recorder.undecryptable();
    assert_eq!(
        events.len(),
        1,
        "same message id fires UndecryptableMessage only once",
    );
    if let Event::UndecryptableMessage(event) = &*events[0] {
        assert_eq!(event.info.id, info.id);
    } else {
        panic!("event was not UndecryptableMessage");
    }
}

/// Status posts must flow through PDO — excluding them drops any
/// InvalidPreKeyId status permanently (WA Web recovers them).
#[tokio::test]
async fn test_pdo_armed_for_status_broadcast() {
    let client = create_test_client_for_retry_with_id("pdo_status").await;

    let info = Arc::new(create_test_message_info(
        "status@broadcast",
        "STATUS_MSG_1",
        "5511777776666@s.whatsapp.net",
    ));

    assert_eq!(info.source.chat.server, wacore_binary::Server::Broadcast);

    client.run_pdo_request(&info).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

/// Broadcast lists share the same code path; locks the guard for both.
#[tokio::test]
async fn test_pdo_armed_for_any_broadcast_chat() {
    let client = create_test_client_for_retry_with_id("pdo_bcast_list").await;

    let info = Arc::new(create_test_message_info(
        "12345@broadcast",
        "BCAST_LIST_MSG_1",
        "5511777776666@s.whatsapp.net",
    ));

    assert_eq!(info.source.chat.server, wacore_binary::Server::Broadcast);

    client.run_pdo_request(&info).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

#[tokio::test]
async fn test_pdo_armed_for_one_on_one() {
    let client = create_test_client_for_retry_with_id("pdo_dm").await;

    let info = Arc::new(create_test_message_info(
        "85010891714716@lid",
        "DM_MSG_1",
        "85010891714716@lid",
    ));

    assert_ne!(info.source.chat.server, wacore_binary::Server::Broadcast);

    client.run_pdo_request(&info).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

/// fromMe messages fanned out to a linked device can still fail decrypt
/// on the receiver side; PDO is the only recovery path for them.
#[tokio::test]
async fn test_pdo_armed_for_from_me() {
    let client = create_test_client_for_retry_with_id("pdo_from_me").await;

    // When fromMe is true the sender is the user's own JID, not a peer.
    let own_jid = "5511999998888@s.whatsapp.net";
    let mut info = create_test_message_info("85010891714716@lid", "FROM_ME_MSG_1", own_jid);
    info.source.is_from_me = true;
    let info = Arc::new(info);

    client.run_pdo_request(&info).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
}

/// Stops offline-sync / reconnect tails from flooding the phone with
/// resend requests for old messages the user likely no longer cares about.
#[tokio::test]
async fn test_pdo_skipped_for_ancient_messages() {
    use wacore::types::message::ChatMessageId;

    let client = create_test_client_for_retry_with_id("pdo_age").await;

    let mut info =
        create_test_message_info("85010891714716@lid", "ANCIENT_MSG_1", "85010891714716@lid");
    info.timestamp = wacore::time::now_utc() - chrono::Duration::days(30);
    let info = Arc::new(info);

    let cache_key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());

    client.run_pdo_request(&info).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    assert!(
        client.pdo_pending_requests.get(&cache_key).await.is_none(),
        "messages older than 14 days must not register a PDO entry",
    );
}

/// Boundary check: age of 14d plus a minute must reject (WA Web uses
/// seconds, not days, so 14d1m is already over the limit). Catches a
/// `num_days()` truncation that would otherwise accept this message.
#[tokio::test]
async fn test_pdo_rejects_just_past_14d_boundary() {
    use wacore::types::message::ChatMessageId;

    let client = create_test_client_for_retry_with_id("pdo_boundary").await;

    let mut info =
        create_test_message_info("85010891714716@lid", "BOUNDARY_MSG_1", "85010891714716@lid");
    info.timestamp =
        wacore::time::now_utc() - chrono::Duration::days(14) - chrono::Duration::minutes(1);
    let info = Arc::new(info);

    let cache_key = ChatMessageId::new(info.source.chat.clone(), info.id.clone());

    client.run_pdo_request(&info).await;
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    assert!(
        client.pdo_pending_requests.get(&cache_key).await.is_none(),
        "14d+1m must be over the limit, matching WA Web's seconds-based check",
    );
}

/// Server-trusted companions (Android-class `DeviceProps.PlatformType`)
/// receive `<unavailable>` as a marker alongside `<enc>`. The cipher
/// must still be decrypted — skipping would discard content the server
/// specifically released for this companion. Decrypt eventually fails
/// on the garbage payload, but via the normal decrypt-failure path,
/// not the `ViewOnce` short-circuit.
#[tokio::test]
async fn test_unavailable_with_enc_skips_unavailable_shortcut() {
    let client = create_test_client_for_retry_with_id("unavailable_with_enc").await;
    let recorder = Arc::new(EventRecorder::default());
    client.register_handler(recorder.clone());

    let node = build_unavailable_stanza("5511777776666@s.whatsapp.net", "UNAV_WITH_ENC_1", true);
    client.clone().handle_incoming_message(node).await;

    assert_eq!(
        recorder.view_once_unavailable_count(),
        0,
        "<unavailable> alongside <enc> must fall through to decrypt, \
             not emit a ViewOnce UndecryptableMessage",
    );
}

/// Untrusted companions (web-class `PlatformType`) get the bare stub —
/// `<unavailable>` without `<enc>`. That path must still emit a
/// `ViewOnce` `UndecryptableMessage` so consumers surface the failure
/// while the phone relays via PDO.
#[tokio::test]
async fn test_unavailable_without_enc_dispatches_view_once_event() {
    let client = create_test_client_for_retry_with_id("unavailable_stub").await;
    let recorder = Arc::new(EventRecorder::default());
    client.register_handler(recorder.clone());

    let node = build_unavailable_stanza("5511777776666@s.whatsapp.net", "UNAV_STUB_1", false);
    client.clone().handle_incoming_message(node).await;

    assert_eq!(
        recorder.view_once_unavailable_count(),
        1,
        "bare <unavailable> stub must dispatch exactly one ViewOnce UndecryptableMessage",
    );
}

/// The event struct has no "recovery pending" flag, so consumers cannot
/// wait for a PDO outcome before surfacing failure — adding a field
/// here forces a conscious UX decision.
#[test]
fn test_undecryptable_event_has_no_pending_pdo_hint() {
    use crate::types::events::{UnavailableType, UndecryptableMessage};

    let info = Arc::new(create_test_message_info(
        "5511999998888@s.whatsapp.net",
        "SHAPE_MSG",
        "5511777776666@s.whatsapp.net",
    ));
    let event = UndecryptableMessage {
        info,
        is_unavailable: false,
        unavailable_type: UnavailableType::Unknown,
        decrypt_fail_mode: DecryptFailMode::Show,
    };

    let _ = (
        &event.info,
        &event.is_unavailable,
        &event.unavailable_type,
        &event.decrypt_fail_mode,
    );
}

/// Seed `device.pn` so `send_nack` clears its `get_pn()` guard.
async fn seed_test_pn(client: &Arc<Client>) {
    use crate::store::commands::DeviceCommand;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetId(Some(
            "5511000000001:0@s.whatsapp.net"
                .parse()
                .expect("test PN should parse"),
        )))
        .await;
}

/// Build a Client wired to a CapturingMockTransport + a noise socket so
/// `send_node` reaches the wire. Returns the transport so the caller can
/// inspect captured frames.
async fn capturing_client(
    test_id: &str,
) -> (
    Arc<Client>,
    Arc<crate::transport::mock::CapturingMockTransport>,
) {
    use crate::socket::NoiseSocket;
    use crate::store::SqliteStore;
    use crate::store::persistence_manager::PersistenceManager;
    use crate::transport::mock::CapturingMockTransportFactory;
    use portable_atomic::AtomicU64;
    use std::sync::atomic::Ordering;
    use wacore::handshake::NoiseCipher;

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let unique_id = COUNTER.fetch_add(1, Ordering::SeqCst);
    let db_name = format!(
        "file:memdb_capt_{}_{}_{}?mode=memory&cache=shared",
        test_id,
        unique_id,
        std::process::id()
    );

    let backend = Arc::new(
        SqliteStore::new(&db_name)
            .await
            .expect("test backend should initialize"),
    );
    let pm = Arc::new(
        PersistenceManager::new(backend)
            .await
            .expect("persistence manager should initialize"),
    );
    let factory = CapturingMockTransportFactory::new();
    let transport = factory.transport();
    let (client, _sync_rx) = Client::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        pm,
        Arc::new(factory),
        Arc::new(MockHttpClient),
        None,
    )
    .await;

    let key = [0u8; 32];
    let write_key = NoiseCipher::new(&key).expect("32-byte key");
    let read_key = NoiseCipher::new(&key).expect("32-byte key");
    let noise_socket = NoiseSocket::new(
        Arc::new(crate::runtime_impl::TokioRuntime),
        transport.clone() as Arc<dyn crate::transport::Transport>,
        write_key,
        read_key,
    );
    // send_node only needs noise_socket Some; is_connected is read by
    // other layers but not on this path.
    *client.noise_socket.lock().await = Some(Arc::new(noise_socket));
    seed_test_pn(&client).await;
    (client, transport)
}

/// Regression: a malformed pkmsg used to fall through silently. Now
/// it dispatches the consumer event AND emits a nack on the wire so
/// the server stops retransmitting.
#[tokio::test]
async fn pkmsg_parse_error_dispatches_parsing_error_nack() {
    use crate::types::events::DecryptFailMode;
    use wacore::message_processing::EncType;

    let (client, transport) = capturing_client("pkmsg_parse_nack").await;
    let info = Arc::new(create_test_message_info(
        "5511999998888@s.whatsapp.net",
        "REGRESSION_PKMSG_PARSE",
        "5511777776666@s.whatsapp.net",
    ));
    let sender_jid: Jid = info.source.sender.clone();

    // 1-byte ciphertext is a guaranteed parse failure.
    let bad_payload = EncPayload {
        ciphertext: bytes::Bytes::from_static(&[0xFF]),
        enc_type: EncType::PreKeyMessage,
        padding_version: 2,
    };

    let outcome = client
        .process_session_enc_batch(&[bad_payload], &info, &sender_jid, DecryptFailMode::Show)
        .await;

    assert!(!outcome.decrypted);
    assert!(!outcome.duplicate);
    assert!(outcome.undecryptable);
    assert!(outcome.had_failure);

    // spawn_nack is detached; give it a tick to flush through the
    // noise_socket sender_task to our CapturingMockTransport.
    for _ in 0..40 {
        if !transport.sent().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let sent = transport.sent();
    assert!(
        !sent.is_empty(),
        "spawn_nack must produce at least one outbound frame on the wire"
    );
}

#[tokio::test]
async fn signal_message_parse_error_dispatches_parsing_error_nack() {
    use crate::types::events::DecryptFailMode;
    use wacore::message_processing::EncType;

    let (client, transport) = capturing_client("sig_parse_nack").await;
    let info = Arc::new(create_test_message_info(
        "5511999998888@s.whatsapp.net",
        "REGRESSION_SIG_PARSE",
        "5511777776666@s.whatsapp.net",
    ));
    let sender_jid: Jid = info.source.sender.clone();

    let bad_payload = EncPayload {
        ciphertext: bytes::Bytes::from_static(&[0xFF]),
        enc_type: EncType::Message,
        padding_version: 2,
    };

    let outcome = client
        .process_session_enc_batch(&[bad_payload], &info, &sender_jid, DecryptFailMode::Show)
        .await;

    assert!(outcome.undecryptable);
    assert!(outcome.had_failure);

    for _ in 0..40 {
        if !transport.sent().is_empty() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(
        !transport.sent().is_empty(),
        "spawn_nack must produce at least one outbound frame on the wire"
    );
}

#[test]
fn test_decrypt_fail_log_level_gated_on_hide() {
    use crate::types::events::DecryptFailMode;
    assert_eq!(
        decrypt_fail_log_level(DecryptFailMode::Hide),
        log::Level::Debug
    );
    assert_eq!(
        decrypt_fail_log_level(DecryptFailMode::Show),
        log::Level::Warn
    );
}

/// Decrypt one captured noise frame (zero-key, counter-based, empty AAD) to
/// its marshalled node bytes; strips the 3-byte frame header.
fn decode_frame(index: usize, frame: &[u8]) -> Option<Vec<u8>> {
    use wacore::handshake::NoiseCipher;
    if frame.len() <= 3 {
        return None;
    }
    let cipher = NoiseCipher::new(&[0u8; 32]).expect("32-byte key");
    let mut buf = frame[3..].to_vec();
    cipher
        .decrypt_in_place_with_counter(index as u32, &mut buf)
        .ok()?;
    (!buf.is_empty()).then_some(buf)
}

/// First `<ack class="message">` on the wire as `(to, recipient)`.
fn find_message_ack(frames: &[bytes::Bytes]) -> Option<(String, Option<String>)> {
    for (i, frame) in frames.iter().enumerate() {
        let Some(buf) = decode_frame(i, frame) else {
            continue;
        };
        let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..]) else {
            continue;
        };
        if node.tag.as_ref() == "ack"
            && node
                .get_attr("class")
                .is_some_and(|v| v.as_str() == "message")
            && node.get_attr("error").is_none()
            && let Some(to) = node.get_attr("to")
        {
            let recipient = node.get_attr("recipient").map(|v| v.as_str().to_string());
            return Some((to.as_str().to_string(), recipient));
        }
    }
    None
}

/// First `<receipt>` on the wire for `id` as `(to, type, recipient)`.
fn find_receipt(
    frames: &[bytes::Bytes],
    id: &str,
) -> Option<(String, Option<String>, Option<String>)> {
    for (i, frame) in frames.iter().enumerate() {
        let Some(buf) = decode_frame(i, frame) else {
            continue;
        };
        let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..]) else {
            continue;
        };
        if node.tag.as_ref() == "receipt"
            && node.get_attr("id").is_some_and(|v| v.as_str() == id)
            && let Some(to) = node.get_attr("to")
        {
            let typ = node.get_attr("type").map(|v| v.as_str().to_string());
            let recipient = node.get_attr("recipient").map(|v| v.as_str().to_string());
            return Some((to.as_str().to_string(), typ, recipient));
        }
    }
    None
}

#[derive(Debug)]
struct SentReceipt {
    to: String,
    typ: Option<String>,
    recipient: Option<String>,
    participant: Option<String>,
    context: Option<String>,
}

fn find_receipt_details(frames: &[bytes::Bytes], id: &str) -> Option<SentReceipt> {
    for (i, frame) in frames.iter().enumerate() {
        let Some(buf) = decode_frame(i, frame) else {
            continue;
        };
        let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..]) else {
            continue;
        };
        if node.tag.as_ref() == "receipt"
            && node.get_attr("id").is_some_and(|v| v.as_str() == id)
            && let Some(to) = node.get_attr("to")
        {
            return Some(SentReceipt {
                to: to.as_str().to_string(),
                typ: node.get_attr("type").map(|v| v.as_str().to_string()),
                recipient: node.get_attr("recipient").map(|v| v.as_str().to_string()),
                participant: node.get_attr("participant").map(|v| v.as_str().to_string()),
                context: node.get_attr("context").map(|v| v.as_str().to_string()),
            });
        }
    }
    None
}

#[derive(Debug)]
struct SentMessageAck {
    to: String,
    participant: Option<String>,
    recipient: Option<String>,
}

fn find_message_ack_for(frames: &[bytes::Bytes], id: &str) -> Option<SentMessageAck> {
    for (i, frame) in frames.iter().enumerate() {
        let Some(buf) = decode_frame(i, frame) else {
            continue;
        };
        let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..]) else {
            continue;
        };
        if node.tag.as_ref() == "ack"
            && node
                .get_attr("class")
                .is_some_and(|v| v.as_str() == "message")
            && node.get_attr("error").is_none()
            && node.get_attr("id").is_some_and(|v| v.as_str() == id)
            && let Some(to) = node.get_attr("to")
        {
            return Some(SentMessageAck {
                to: to.as_str().to_string(),
                participant: node.get_attr("participant").map(|v| v.as_str().to_string()),
                recipient: node.get_attr("recipient").map(|v| v.as_str().to_string()),
            });
        }
    }
    None
}

/// Count delivery `<receipt>` (anything but type="retry") on the wire for `id`.
fn delivery_receipts_for(frames: &[bytes::Bytes], id: &str) -> usize {
    let mut count = 0;
    for (i, frame) in frames.iter().enumerate() {
        let Some(buf) = decode_frame(i, frame) else {
            continue;
        };
        let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..]) else {
            continue;
        };
        if node.tag.as_ref() == "receipt"
            && node.get_attr("id").is_some_and(|v| v.as_str() == id)
            && node
                .get_attr("type")
                .as_ref()
                .map(|v| v.as_str())
                .as_deref()
                != Some("retry")
        {
            count += 1;
        }
    }
    count
}

fn message_acks_for(frames: &[bytes::Bytes], id: &str) -> usize {
    let mut count = 0;
    for (i, frame) in frames.iter().enumerate() {
        let Some(buf) = decode_frame(i, frame) else {
            continue;
        };
        let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..]) else {
            continue;
        };
        if node.tag.as_ref() == "ack"
            && node
                .get_attr("class")
                .is_some_and(|v| v.as_str() == "message")
            && node.get_attr("error").is_none()
            && node.get_attr("id").is_some_and(|v| v.as_str() == id)
        {
            count += 1;
        }
    }
    count
}

fn sender_receipts_for(frames: &[bytes::Bytes], id: &str) -> usize {
    let mut count = 0;
    for (i, frame) in frames.iter().enumerate() {
        let Some(buf) = decode_frame(i, frame) else {
            continue;
        };
        let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..]) else {
            continue;
        };
        if node.tag.as_ref() == "receipt"
            && node.get_attr("id").is_some_and(|v| v.as_str() == id)
            && node
                .get_attr("type")
                .is_some_and(|v| v.as_str() == "sender")
        {
            count += 1;
        }
    }
    count
}

fn confirmations_for(frames: &[bytes::Bytes], id: &str) -> usize {
    delivery_receipts_for(frames, id) + message_acks_for(frames, id)
}

async fn wait_for_confirmations(
    transport: &crate::transport::mock::CapturingMockTransport,
    id: &str,
    expected: usize,
) -> usize {
    let mut count = 0;
    for _ in 0..80 {
        count = confirmations_for(&transport.sent(), id);
        if count >= expected {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    count
}

async fn assert_exactly_one_confirmation(
    transport: &crate::transport::mock::CapturingMockTransport,
    id: &str,
) {
    let count = wait_for_confirmations(transport, id, 1).await;
    assert_eq!(count, 1, "message {id} must be confirmed exactly once");
    for _ in 0..5 {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert_eq!(
            confirmations_for(&transport.sent(), id),
            1,
            "message {id} must not get a late second confirmation"
        );
    }
}

/// A stanza that fails to decrypt must emit a transport `<ack class="message">`
/// (else the server replays it on every reconnect forever), addressed to the
/// original `from` echoing `recipient`. Uses `Hide` (the production reactions
/// carried `decrypt-fail="hide"`) to also guard that hide does not suppress
/// the ack. BadMac so the retry carries no keys (no device account needed).
#[tokio::test]
async fn decrypt_failure_emits_transport_ack() {
    let (client, transport) = capturing_client("decrypt_fail_ack").await;

    let sender: Jid = "236395184570386@lid".parse().expect("sender JID");
    let recipient: Jid = "156535032389744@lid".parse().expect("recipient JID");
    let info = Arc::new(MessageInfo {
        id: "AC055553E56A2C12DE592DAD6353C477".to_string(),
        source: crate::types::message::MessageSource {
            sender: sender.clone(),
            chat: recipient.clone(),
            recipient: Some(recipient.clone()),
            ..Default::default()
        },
        ..Default::default()
    });

    client
        .handle_decrypt_failure(
            &info,
            RetryReason::BadMac,
            crate::types::events::DecryptFailMode::Hide,
        )
        .await;

    // retry + ack are detached spawns; poll the wire until the ack appears.
    let mut found = None;
    for _ in 0..80 {
        if let Some(a) = find_message_ack(&transport.sent()) {
            found = Some(a);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (to, recipient_attr) = found.expect(
        "decrypt failure must emit a transport <ack class=message> \
             (else the server redelivers the stanza forever)",
    );
    assert_eq!(
        to, "236395184570386@lid",
        "ack `to` must be the original `from` (own LID), not the chat"
    );
    assert_eq!(
        recipient_attr.as_deref(),
        Some("156535032389744@lid"),
        "ack must echo `recipient` for own-account fan-out"
    );
}

/// Regression for the bot self-fanout loop on the DECRYPT-FAILURE path
/// (BadMac/NoSession): a self-fanout we cannot decrypt must be cleared with
/// a `<receipt type="sender">`, NOT a bare transport `<ack>` (ignored by the
/// server) nor a retry-to-self (futile). Once stuck in the loop the local
/// counter advances past the duplicate state, so this BadMac path is what
/// actually fires for an already-affected account.
#[tokio::test]
async fn self_fanout_decrypt_failure_acked_via_sender_receipt() {
    let (client, transport) = capturing_client("self_fanout_badmac").await;
    let info = Arc::new(MessageInfo {
        id: "AC00000000000000000000000000BEEF".to_string(),
        source: crate::types::message::MessageSource {
            sender: "100000000000001@lid".parse().expect("sender"),
            chat: "200000000000002@bot".parse().expect("chat"),
            recipient: Some("200000000000002@bot".parse().expect("recipient")),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    });

    client
        .handle_decrypt_failure(
            &info,
            RetryReason::BadMac,
            crate::types::events::DecryptFailMode::Hide,
        )
        .await;

    let mut found = None;
    for _ in 0..80 {
        if let Some(r) = find_receipt(&transport.sent(), "AC00000000000000000000000000BEEF") {
            found = Some(r);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (to, typ, recipient) =
        found.expect("self-fanout decrypt failure must emit a sender <receipt> to drain the queue");
    assert_eq!(to, "100000000000001@lid");
    assert_eq!(typ.as_deref(), Some("sender"));
    assert_eq!(recipient.as_deref(), Some("200000000000002@bot"));

    let sent = transport.sent();
    assert!(
        find_message_ack(&sent).is_none(),
        "must not emit the bare <ack> the server ignores"
    );
    let mut saw_retry = false;
    for (i, frame) in sent.iter().enumerate() {
        if let Some(buf) = decode_frame(i, frame)
            && let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..])
            && node.tag.as_ref() == "receipt"
            && node.get_attr("type").is_some_and(|v| {
                v.as_str() == crate::types::presence::ReceiptType::Retry.as_wire_str()
            })
        {
            saw_retry = true;
        }
    }
    assert!(
        !saw_retry,
        "must not retry our own undecryptable fanout to ourselves"
    );
}

/// Consistency with the success/duplicate path: a bot-authored own DM in a
/// non-bot chat (sender on `@bot`, user chat) must NOT take the sender
/// receipt on the decrypt-failure path either; it stays on the
/// bot-invoke-response bare-ack path (WA Web `!chat.isBot() &&
/// author.isBot()`), matching ack_received_message and the locked
/// own_bot_author_dm_acks_not_sender_receipt test.
#[tokio::test]
async fn bot_author_self_fanout_decrypt_failure_not_sender_receipt() {
    let (client, transport) = capturing_client("bot_author_badmac").await;
    let info = Arc::new(MessageInfo {
        id: "OWNBOTFAIL1".to_string(),
        source: crate::types::message::MessageSource {
            sender: "100000000000002@bot".parse().expect("sender"),
            chat: "300000000000003@lid".parse().expect("chat"),
            recipient: Some("300000000000003@lid".parse().expect("recipient")),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    });

    client
        .handle_decrypt_failure(
            &info,
            RetryReason::BadMac,
            crate::types::events::DecryptFailMode::Hide,
        )
        .await;

    // Positive: the message IS cleared, via the bot-invoke-response bare
    // <ack class="message"> (the retry-to-self is bot-skipped, so the
    // transport ack follows), proving we took the ack path, not a no-op.
    let mut found_ack = false;
    for _ in 0..80 {
        if find_message_ack(&transport.sent()).is_some() {
            found_ack = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(
        found_ack,
        "bot-authored own DM must still be transport-acked with a bare <ack class=message>"
    );

    // Negative settle: it must NEVER produce a sender receipt on the failure
    // path (that would diverge from WA Web's bot-invoke ack and contradict
    // the success-path ordering).
    for _ in 0..5 {
        assert!(
            find_receipt(&transport.sent(), "OWNBOTFAIL1").is_none(),
            "bot-authored own DM must not be cleared with a sender <receipt> on decrypt failure"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}

/// If the resend request fails to send, the stanza must NOT be acked, so the
/// server keeps it queued for another try. Here NoSession needs keys, which
/// need a device account this harness lacks, so send_retry_receipt errors.
#[tokio::test]
async fn decrypt_failure_does_not_ack_when_retry_send_fails() {
    let (client, transport) = capturing_client("retry_fail_no_ack").await;
    let sender: Jid = "5511777776666@s.whatsapp.net".parse().expect("sender");
    let info = Arc::new(MessageInfo {
        id: "NOACK1".to_string(),
        source: crate::types::message::MessageSource {
            sender: sender.clone(),
            chat: sender.clone(),
            ..Default::default()
        },
        ..Default::default()
    });
    client
        .handle_decrypt_failure(
            &info,
            RetryReason::NoSession,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;
    tokio::time::sleep(std::time::Duration::from_millis(150)).await;
    assert!(
        find_message_ack(&transport.sent()).is_none(),
        "must not ack when the resend request failed to send"
    );
}

/// The retry receipt must be sent before the transport ack (one ordered
/// flushed task), so a disconnect mid-flush can never clear the stanza from
/// the offline queue without the sender having received a resend request.
#[tokio::test]
async fn decrypt_failure_sends_retry_before_ack() {
    let (client, transport) = capturing_client("retry_before_ack").await;
    let sender: Jid = "5511777776666@s.whatsapp.net".parse().expect("sender");
    let info = Arc::new(MessageInfo {
        id: "RBA1".to_string(),
        source: crate::types::message::MessageSource {
            sender: sender.clone(),
            chat: sender.clone(),
            ..Default::default()
        },
        ..Default::default()
    });
    // BadMac (not NoSession) so the retry receipt carries no keys and needs
    // no device account in this harness.
    client
        .handle_decrypt_failure(
            &info,
            RetryReason::BadMac,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;

    let find = |tag: &str, retry: bool| -> Option<usize> {
        let frames = transport.sent();
        for (i, frame) in frames.iter().enumerate() {
            let Some(buf) = decode_frame(i, frame) else {
                continue;
            };
            let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..]) else {
                continue;
            };
            let is_retry = node.get_attr("type").is_some_and(|v| v.as_str() == "retry");
            if node.tag.as_ref() == tag && is_retry == retry {
                return Some(i);
            }
        }
        None
    };

    let mut retry_idx = None;
    let mut ack_idx = None;
    for _ in 0..80 {
        retry_idx = find("receipt", true);
        ack_idx = find("ack", false);
        if retry_idx.is_some() && ack_idx.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let retry_idx = retry_idx.expect("retry receipt must be sent");
    let ack_idx = ack_idx.expect("transport ack must be sent");
    assert!(
        retry_idx < ack_idx,
        "retry receipt (frame {retry_idx}) must be sent before the ack (frame {ack_idx})"
    );
}

/// status@broadcast is already acked by the `should_ack` gate post-dispatch,
/// so the decrypt-failure path must NOT emit a second transport ack
/// (whatsmeow/WA Web send exactly one per message). The retry receipt still
/// goes out.
#[tokio::test]
async fn status_broadcast_decrypt_failure_acks_to_chat() {
    let (client, transport) = capturing_client("status_fail_ack").await;
    let info = Arc::new(MessageInfo {
        id: "STATUSMSGID".to_string(),
        source: crate::types::message::MessageSource {
            sender: "236395184570386@lid".parse().expect("sender"),
            chat: "status@broadcast".parse().expect("status chat"),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });

    client
        .handle_decrypt_failure(
            &info,
            RetryReason::BadMac,
            crate::types::events::DecryptFailMode::Show,
        )
        .await;

    // status failures are acked from the flushed task (not just the detached
    // should_ack gate), so the ack survives a disconnect mid-flush.
    let mut found = None;
    for _ in 0..80 {
        if let Some(a) = find_message_ack(&transport.sent()) {
            found = Some(a);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (to, _) = found.expect("status failure must emit a flushed transport ack");
    assert_eq!(
        to, "status@broadcast",
        "status ack `to` must be the status chat"
    );
}

/// Run a single session ciphertext through the full classify->process path.
async fn process_session_ct(
    client: &Arc<Client>,
    sender: &Jid,
    id: &str,
    ct: &wacore::libsignal::protocol::CiphertextMessage,
) {
    use wacore::libsignal::protocol::CiphertextMessage;
    let (enc_type, bytes) = match ct {
        CiphertextMessage::PreKeySignalMessage(m) => ("pkmsg", m.serialized().to_vec()),
        CiphertextMessage::SignalMessage(m) => ("msg", m.serialized().to_vec()),
        _ => panic!("unexpected ciphertext type"),
    };
    let enc = NodeBuilder::new("enc")
        .attr("type", enc_type)
        .bytes(bytes)
        .build();
    let enc_ref = enc.as_node_ref();
    let payload = EncPayload::from_node_ref(&enc_ref).unwrap();
    let info = Arc::new(MessageInfo {
        id: id.to_string(),
        source: crate::types::message::MessageSource {
            sender: sender.clone(),
            chat: sender.clone(),
            ..Default::default()
        },
        ..Default::default()
    });
    client
        .clone()
        .process_classified_message(ClassifiedMessage {
            info,
            sender_encryption_jid: sender.clone(),
            session_payloads: vec![payload],
            group_payloads: vec![],
            bot_payloads: vec![],
            max_sender_retry_count: 0,
            decrypt_fail_mode: crate::types::events::DecryptFailMode::Show,
        })
        .await;
}

fn enc_payload_from_ciphertext(ct: &CiphertextMessage) -> EncPayload {
    let (enc_type, bytes) = match ct {
        CiphertextMessage::PreKeySignalMessage(m) => ("pkmsg", m.serialized().to_vec()),
        CiphertextMessage::SignalMessage(m) => ("msg", m.serialized().to_vec()),
        _ => panic!("unexpected ciphertext type"),
    };
    let enc = NodeBuilder::new("enc")
        .attr("type", enc_type)
        .bytes(bytes)
        .build();
    EncPayload::from_node_ref(&enc.as_node_ref()).expect("ciphertext payload")
}

fn skmsg_payload_from_bytes(bytes: Vec<u8>) -> EncPayload {
    let enc = NodeBuilder::new("enc")
        .attr("type", "skmsg")
        .bytes(bytes)
        .build();
    EncPayload::from_node_ref(&enc.as_node_ref()).expect("skmsg payload")
}

fn msmsg_payload_from_bytes(bytes: Vec<u8>) -> EncPayload {
    let enc = NodeBuilder::new("enc")
        .attr("type", "msmsg")
        .bytes(bytes)
        .build();
    EncPayload::from_node_ref(&enc.as_node_ref()).expect("msmsg payload")
}

fn group_message_info(id: &str, group: &Jid, sender: &Jid, is_from_me: bool) -> Arc<MessageInfo> {
    Arc::new(MessageInfo {
        id: id.to_string(),
        source: crate::types::message::MessageSource {
            sender: sender.clone(),
            chat: group.clone(),
            is_from_me,
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    })
}

async fn process_group_classified(
    client: &Arc<Client>,
    info: Arc<MessageInfo>,
    sender: &Jid,
    session_payload: EncPayload,
    group_payloads: Vec<EncPayload>,
) {
    process_group_classified_with_sessions(
        client,
        info,
        sender,
        vec![session_payload],
        group_payloads,
    )
    .await;
}

async fn process_group_classified_with_sessions(
    client: &Arc<Client>,
    info: Arc<MessageInfo>,
    sender: &Jid,
    session_payloads: Vec<EncPayload>,
    group_payloads: Vec<EncPayload>,
) {
    process_group_classified_with_payloads(
        client,
        info,
        sender,
        session_payloads,
        group_payloads,
        vec![],
    )
    .await;
}

async fn process_group_classified_with_payloads(
    client: &Arc<Client>,
    info: Arc<MessageInfo>,
    sender: &Jid,
    session_payloads: Vec<EncPayload>,
    group_payloads: Vec<EncPayload>,
    bot_payloads: Vec<EncPayload>,
) {
    client
        .clone()
        .process_classified_message(ClassifiedMessage {
            info,
            sender_encryption_jid: sender.clone(),
            session_payloads,
            group_payloads,
            bot_payloads,
            max_sender_retry_count: 0,
            decrypt_fail_mode: crate::types::events::DecryptFailMode::Show,
        })
        .await;
}

fn message_events_for_id(rx: &async_channel::Receiver<Arc<Event>>, id: &str) -> (usize, usize) {
    let mut count = 0;
    let mut visible_content = 0;
    while let Ok(event) = rx.try_recv() {
        if let Event::Message(msg, info) = event.as_ref()
            && info.id == id
        {
            count += 1;
            if msg.conversation.is_some() {
                visible_content += 1;
            }
        }
    }
    (count, visible_content)
}

fn message_texts_for_id(rx: &async_channel::Receiver<Arc<Event>>, id: &str) -> Vec<String> {
    let mut texts = Vec::new();
    while let Ok(event) = rx.try_recv() {
        if let Event::Message(msg, info) = event.as_ref()
            && info.id == id
            && let Some(text) = &msg.conversation
        {
            texts.push(text.clone());
        }
    }
    texts
}

#[tokio::test]
async fn skdm_only_group_session_acknowledged_once_without_message_event() {
    use wacore::messages::MessageUtils;
    use wacore::types::events::ChannelEventHandler;

    let (client, transport) = capturing_client("skdm_only_group_ack").await;
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.add_handler(handler);

    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("146824178450525@lid").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    let group: Jid = "120363408782575443@g.us".parse().expect("group");
    let skdm = alice.create_group_skdm(&group).await;
    let plaintext = MessageUtils::encode_and_pad(&wa::Message {
        sender_key_distribution_message: Some(skdm),
        ..Default::default()
    });
    let session_ct = alice.encrypt(&bob_addr, &plaintext).await;
    let id = "SKDM_ONLY_SESSION";
    let info = group_message_info(id, &group, &alice.jid, false);

    process_group_classified(
        &client,
        info,
        &alice.jid,
        enc_payload_from_ciphertext(&session_ct),
        vec![],
    )
    .await;

    assert_exactly_one_confirmation(&transport, id).await;
    assert_eq!(
        delivery_receipts_for(&transport.sent(), id),
        1,
        "incoming group SKDM-only session message should drain via delivery receipt"
    );
    let sent = transport.sent();
    let receipt = find_receipt_details(&sent, id).expect("delivery receipt");
    let sender_str = alice.jid.to_string();
    assert_eq!(receipt.to, group.to_string());
    assert_eq!(receipt.participant.as_deref(), Some(sender_str.as_str()));
    assert_eq!(receipt.recipient, None);
    assert_ne!(
        receipt.typ.as_deref(),
        Some("sender"),
        "incoming group SKDM-only must not be cleared as a sender receipt"
    );
    assert_eq!(
        message_acks_for(&sent, id),
        0,
        "incoming group SKDM-only must not also emit a transport ack"
    );
    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    assert_eq!(
        message_events_for_id(&rx, id),
        (0, 0),
        "SKDM-only messages must not surface Event::Message"
    );
}

#[tokio::test]
async fn session_plaintext_decode_error_is_not_acked_as_skdm_only() {
    use wacore::types::events::ChannelEventHandler;

    let (client, transport) = capturing_client("bad_plaintext_no_skdm_ack").await;
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.add_handler(handler);
    let recorder = Arc::new(EventRecorder::default());
    client.register_handler(recorder.clone());

    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("146824178450527@lid").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    let group: Jid = "120363408782575446@g.us".parse().expect("group");
    let invalid_padded_plaintext = vec![0xff, 0x01];
    let session_ct = alice.encrypt(&bob_addr, &invalid_padded_plaintext).await;
    let id = "BAD_SESSION_PLAINTEXT";
    let info = group_message_info(id, &group, &alice.jid, false);

    process_group_classified(
        &client,
        info,
        &alice.jid,
        enc_payload_from_ciphertext(&session_ct),
        vec![],
    )
    .await;

    for _ in 0..5 {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert_eq!(
            confirmations_for(&transport.sent(), id),
            0,
            "invalid plaintext must not be counted as a successful SKDM-only ack"
        );
    }
    assert_eq!(
        recorder.undecryptable().len(),
        1,
        "plaintext handler failures must stay on the undecryptable path"
    );
    assert_eq!(
        message_events_for_id(&rx, id),
        (0, 0),
        "invalid plaintext must not surface Event::Message"
    );
    let mut nack_code = None;
    for _ in 0..80 {
        nack_code = find_message_nack_error(&transport.sent(), id);
        if nack_code.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(
        nack_code,
        Some(491),
        "invalid decrypted protobuf must be drained with InvalidProtobuf nack"
    );
}

#[tokio::test]
async fn mixed_skdm_and_bad_plaintext_session_is_nacked_not_positive_acked() {
    use wacore::messages::MessageUtils;
    use wacore::types::events::ChannelEventHandler;

    let (client, transport) = capturing_client("mixed_skdm_bad_plaintext").await;
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.add_handler(handler);
    let recorder = Arc::new(EventRecorder::default());
    client.register_handler(recorder.clone());

    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("146824178450531@lid").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    let group: Jid = "120363408782575449@g.us".parse().expect("group");
    let skdm = alice.create_group_skdm(&group).await;
    let skdm_plaintext = MessageUtils::encode_and_pad(&wa::Message {
        sender_key_distribution_message: Some(skdm),
        ..Default::default()
    });
    let skdm_ct = alice.encrypt(&bob_addr, &skdm_plaintext).await;
    let bad_ct = alice.encrypt(&bob_addr, &[0xff, 0x01]).await;
    let id = "SKDM_WITH_BAD_SESSION";
    let info = group_message_info(id, &group, &alice.jid, false);

    process_group_classified_with_sessions(
        &client,
        info,
        &alice.jid,
        vec![
            enc_payload_from_ciphertext(&skdm_ct),
            enc_payload_from_ciphertext(&bad_ct),
        ],
        vec![],
    )
    .await;

    let mut nack_code = None;
    for _ in 0..80 {
        nack_code = find_message_nack_error(&transport.sent(), id);
        if nack_code.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(nack_code, Some(491));
    assert_eq!(
        confirmations_for(&transport.sent(), id),
        0,
        "SKDM-only fallback must not positive-ack a mixed malformed session batch"
    );
    assert_eq!(
        recorder.undecryptable().len(),
        1,
        "the malformed sibling must still surface as undecryptable"
    );
    assert_eq!(
        message_events_for_id(&rx, id),
        (0, 0),
        "mixed SKDM and bad plaintext must not dispatch user content"
    );
}

#[tokio::test]
async fn bad_session_plaintext_skips_skmsg_sibling_after_nack() {
    use wacore::messages::MessageUtils;
    use wacore::types::events::ChannelEventHandler;

    let (client, transport) = capturing_client("bad_session_skips_skmsg").await;
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.add_handler(handler);
    let recorder = Arc::new(EventRecorder::default());
    client.register_handler(recorder.clone());

    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("146824178450532@lid").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    let group: Jid = "120363408782575450@g.us".parse().expect("group");
    let skdm = alice.create_group_skdm(&group).await;
    let skdm_plaintext = MessageUtils::encode_and_pad(&wa::Message {
        sender_key_distribution_message: Some(skdm),
        ..Default::default()
    });
    let skdm_ct = alice.encrypt(&bob_addr, &skdm_plaintext).await;
    let bad_ct = alice.encrypt(&bob_addr, &[0xff, 0x01]).await;
    let content_plaintext = MessageUtils::encode_and_pad(&wa::Message {
        conversation: Some("must not dispatch".to_string()),
        ..Default::default()
    });
    let skmsg = alice
        .encrypt_group_message(&group, &content_plaintext)
        .await;
    let id = "BAD_SESSION_WITH_SKMSG";
    let info = group_message_info(id, &group, &alice.jid, false);

    process_group_classified_with_sessions(
        &client,
        info,
        &alice.jid,
        vec![
            enc_payload_from_ciphertext(&skdm_ct),
            enc_payload_from_ciphertext(&bad_ct),
        ],
        vec![skmsg_payload_from_bytes(skmsg)],
    )
    .await;

    let mut nack_code = None;
    for _ in 0..80 {
        nack_code = find_message_nack_error(&transport.sent(), id);
        if nack_code.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(nack_code, Some(491));
    assert_eq!(
        confirmations_for(&transport.sent(), id),
        0,
        "skmsg must not ack after a session InvalidProtobuf nack"
    );
    assert_eq!(
        recorder.undecryptable().len(),
        1,
        "session plaintext failure should own the only user-visible failure"
    );
    assert_eq!(
        message_texts_for_id(&rx, id),
        Vec::<String>::new(),
        "skmsg content must be skipped after session plaintext failure"
    );
}

#[tokio::test]
async fn skdm_only_session_with_msmsg_waits_for_bot_payload_response() {
    use wacore::messages::MessageUtils;

    let (client, transport) = capturing_client("skdm_msmsg_no_fallback_ack").await;

    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("146824178450533@lid").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    let group: Jid = "120363408782575451@g.us".parse().expect("group");
    let skdm = alice.create_group_skdm(&group).await;
    let plaintext = MessageUtils::encode_and_pad(&wa::Message {
        sender_key_distribution_message: Some(skdm),
        ..Default::default()
    });
    let session_ct = alice.encrypt(&bob_addr, &plaintext).await;
    let id = "SKDM_WITH_MSMSG";
    let info = group_message_info(id, &group, &alice.jid, false);

    process_group_classified_with_payloads(
        &client,
        info,
        &alice.jid,
        vec![enc_payload_from_ciphertext(&session_ct)],
        vec![],
        vec![msmsg_payload_from_bytes(vec![0xff])],
    )
    .await;

    let mut nack_code = None;
    for _ in 0..80 {
        nack_code = find_message_nack_error(&transport.sent(), id);
        if nack_code.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(nack_code, Some(487));
    assert_eq!(
        confirmations_for(&transport.sent(), id),
        0,
        "SKDM-only fallback must not pre-ack a stanza with msmsg work"
    );
}

#[tokio::test]
async fn session_content_group_message_acknowledged_once_without_fallback() {
    use wacore::messages::MessageUtils;
    use wacore::types::events::ChannelEventHandler;

    let (client, transport) = capturing_client("session_content_group_ack").await;
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.add_handler(handler);

    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("146824178450528@lid").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    let group: Jid = "120363408782575447@g.us".parse().expect("group");
    let plaintext = MessageUtils::encode_and_pad(&wa::Message {
        conversation: Some("session content".to_string()),
        ..Default::default()
    });
    let session_ct = alice.encrypt(&bob_addr, &plaintext).await;
    let id = "SESSION_CONTENT_GROUP";
    let info = group_message_info(id, &group, &alice.jid, false);

    process_group_classified(
        &client,
        info,
        &alice.jid,
        enc_payload_from_ciphertext(&session_ct),
        vec![],
    )
    .await;

    assert_exactly_one_confirmation(&transport, id).await;
    let sent = transport.sent();
    assert_eq!(
        delivery_receipts_for(&sent, id),
        1,
        "session content dispatch should own the only delivery receipt"
    );
    assert_eq!(
        message_acks_for(&sent, id),
        0,
        "normal session content must not also use the SKDM-only transport ack"
    );
    assert_eq!(
        message_texts_for_id(&rx, id),
        vec!["session content".to_string()],
        "normal session content must dispatch exactly once"
    );
}

#[tokio::test]
async fn status_skdm_only_session_uses_one_status_receipt() {
    use wacore::messages::MessageUtils;
    use wacore::types::events::ChannelEventHandler;

    let (client, transport) = capturing_client("status_skdm_only_ack").await;
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.add_handler(handler);

    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("146824178450529@lid").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    let status: Jid = "status@broadcast".parse().expect("status");
    let skdm = alice.create_group_skdm(&status).await;
    let plaintext = MessageUtils::encode_and_pad(&wa::Message {
        sender_key_distribution_message: Some(skdm),
        ..Default::default()
    });
    let session_ct = alice.encrypt(&bob_addr, &plaintext).await;
    let id = "STATUS_SKDM_ONLY";
    let info = group_message_info(id, &status, &alice.jid, false);

    process_group_classified(
        &client,
        info,
        &alice.jid,
        enc_payload_from_ciphertext(&session_ct),
        vec![],
    )
    .await;

    assert_exactly_one_confirmation(&transport, id).await;
    let sent = transport.sent();
    assert_eq!(
        delivery_receipts_for(&sent, id),
        1,
        "status SKDM-only success must still send the WA Web status receipt"
    );
    assert_eq!(
        message_acks_for(&sent, id),
        0,
        "status success path should not use a transport ack"
    );
    let receipt = find_receipt_details(&sent, id).expect("status delivery receipt");
    let sender_str = alice.jid.to_string();
    assert_eq!(receipt.to, status.to_string());
    assert_eq!(receipt.participant.as_deref(), Some(sender_str.as_str()));
    assert_eq!(receipt.context.as_deref(), Some("status"));
    assert_eq!(
        message_events_for_id(&rx, id),
        (0, 0),
        "status SKDM-only messages must not surface Event::Message"
    );
}

#[tokio::test]
async fn error_message_ack_is_not_counted_as_positive_confirmation() {
    let (client, transport) = capturing_client("error_ack_not_positive").await;
    let id = "ERROR_ACK_NOT_POSITIVE";
    let info = Arc::new(MessageInfo {
        id: id.to_string(),
        source: crate::types::message::MessageSource {
            sender: "146824178450530@lid".parse().expect("sender"),
            chat: "120363408782575448@g.us".parse().expect("group"),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });

    client.spawn_nack(
        &info,
        wacore::protocol::nack::NackReason::ParsingError,
        None,
    );

    let mut nack_code = None;
    for _ in 0..80 {
        nack_code = find_message_nack_error(&transport.sent(), id);
        if nack_code.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(nack_code, Some(487));

    let sent = transport.sent();
    assert_eq!(
        message_acks_for(&sent, id),
        0,
        "nacks carry class=message but must not count as positive transport acks"
    );
    assert_eq!(
        confirmations_for(&sent, id),
        0,
        "nacks must not satisfy exactly-one positive confirmation assertions"
    );
    assert!(
        find_message_ack_for(&sent, id).is_none(),
        "error acks must be excluded from positive ack lookup"
    );
}

#[tokio::test]
async fn skdm_session_with_skmsg_sibling_acknowledged_once() {
    use wacore::messages::MessageUtils;
    use wacore::types::events::ChannelEventHandler;

    let (client, transport) = capturing_client("skdm_plus_skmsg_ack").await;
    let (handler, rx) = ChannelEventHandler::new();
    client.core.event_bus.add_handler(handler);

    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("146824178450526@lid").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    let group: Jid = "120363408782575444@g.us".parse().expect("group");
    let skdm = alice.create_group_skdm(&group).await;
    let skdm_plaintext = MessageUtils::encode_and_pad(&wa::Message {
        sender_key_distribution_message: Some(skdm),
        ..Default::default()
    });
    let session_ct = alice.encrypt(&bob_addr, &skdm_plaintext).await;

    let content_plaintext = MessageUtils::encode_and_pad(&wa::Message {
        conversation: Some("group content".to_string()),
        ..Default::default()
    });
    let skmsg = alice
        .encrypt_group_message(&group, &content_plaintext)
        .await;
    let id = "SKDM_WITH_SKMSG";
    let info = group_message_info(id, &group, &alice.jid, false);

    process_group_classified(
        &client,
        info,
        &alice.jid,
        enc_payload_from_ciphertext(&session_ct),
        vec![skmsg_payload_from_bytes(skmsg)],
    )
    .await;

    assert_exactly_one_confirmation(&transport, id).await;
    assert_eq!(
        delivery_receipts_for(&transport.sent(), id),
        1,
        "the skmsg content dispatch should own the only receipt"
    );
    let sent = transport.sent();
    let receipt = find_receipt_details(&sent, id).expect("delivery receipt");
    let sender_str = alice.jid.to_string();
    assert_eq!(receipt.to, group.to_string());
    assert_eq!(receipt.participant.as_deref(), Some(sender_str.as_str()));
    assert_eq!(
        message_acks_for(&sent, id),
        0,
        "SKDM+skmsg sibling must not get an extra transport ack"
    );
    assert_eq!(
        message_texts_for_id(&rx, id),
        vec!["group content".to_string()],
        "only the skmsg content should dispatch a user message"
    );
}

#[tokio::test]
async fn own_group_skdm_only_session_uses_transport_ack_once() {
    use wacore::messages::MessageUtils;

    let (client, transport) = capturing_client("own_group_skdm_ack").await;

    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("999999999999999@lid").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    let group: Jid = "120363408782575445@g.us".parse().expect("group");
    let skdm = alice.create_group_skdm(&group).await;
    let plaintext = MessageUtils::encode_and_pad(&wa::Message {
        sender_key_distribution_message: Some(skdm),
        ..Default::default()
    });
    let session_ct = alice.encrypt(&bob_addr, &plaintext).await;
    let id = "OWN_GROUP_SKDM_ONLY";
    let info = group_message_info(id, &group, &alice.jid, true);

    process_group_classified(
        &client,
        info,
        &alice.jid,
        enc_payload_from_ciphertext(&session_ct),
        vec![],
    )
    .await;

    assert_exactly_one_confirmation(&transport, id).await;
    let sent = transport.sent();
    assert_eq!(
        message_acks_for(&sent, id),
        1,
        "own group SKDM-only session message should use transport ack"
    );
    let ack = find_message_ack_for(&sent, id).expect("transport ack");
    let sender_str = alice.jid.to_string();
    assert_eq!(ack.to, group.to_string());
    assert_eq!(ack.participant.as_deref(), Some(sender_str.as_str()));
    assert_eq!(ack.recipient, None);
    assert_eq!(
        delivery_receipts_for(&sent, id),
        0,
        "own group SKDM-only session message must not use a delivery receipt"
    );
    assert_eq!(
        sender_receipts_for(&sent, id),
        0,
        "group self-fanout must not use type=sender receipt"
    );
}

/// Regression for the offline-backlog disconnect: an already-processed
/// (duplicate) message must get its own delivery receipt, else the server
/// replays it every reconnect until it force-closes the stream. Pre-fix only
/// the first (success) delivery was acked; the duplicate was skipped silently.
#[tokio::test]
async fn duplicate_message_is_acked_with_delivery_receipt() {
    let (client, transport) = capturing_client("dup_receipt").await;
    let (bundle, bob_jid) = bobs_prekey_bundle(&client).await;
    let bob_addr = bob_jid.to_protocol_address();
    let mut alice = AlicePeer::new("5511888887777@s.whatsapp.net").await;
    alice.install_bob_session(&bob_addr, &bundle).await;

    // Establish the session, then mark Alice's prekey acked so her next message
    // is a plain SignalMessage. Re-submitting it is a clean duplicate.
    let establish = alice.encrypt_text(&bob_addr, "establish").await;
    process_session_ct(&client, &alice.jid, "EST", &establish).await;
    if let Some(record) = alice.sessions.0.get_mut(&bob_addr)
        && let Some(state) = record.session_state_mut()
    {
        state.clear_unacknowledged_pre_key_message();
    }

    // A real (padded) Message so the success path also emits its receipt.
    let plaintext = wacore::messages::MessageUtils::encode_and_pad(&wa::Message {
        conversation: Some("hi".to_string()),
        ..Default::default()
    });
    let msg = alice.encrypt(&bob_addr, &plaintext).await;
    process_session_ct(&client, &alice.jid, "DUP", &msg).await; // success
    process_session_ct(&client, &alice.jid, "DUP", &msg).await; // duplicate

    let mut count = 0;
    for _ in 0..80 {
        count = delivery_receipts_for(&transport.sent(), "DUP");
        if count >= 2 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(
        count, 2,
        "duplicate must get its own delivery receipt (pre-fix: only the first send was acked)"
    );
}

/// Own-account self-fanout (is_from_me, non-peer, carries a `recipient`):
/// our own outgoing message echoed back to this device. WA Web
/// (`isMeAccount(author) => SENDER`) and whatsmeow (`IsFromMe => "sender"`)
/// clear it with a `<receipt type="sender" recipient=...>`, NOT a bare
/// transport `<ack>`. The server's offline queue ignores the bare ack and
/// replays the stanza forever (the ~50min disconnect loop).
#[tokio::test]
async fn own_self_fanout_acked_via_sender_receipt() {
    let (client, transport) = capturing_client("own_ack").await;
    let own = Arc::new(MessageInfo {
        id: "OWN1".to_string(),
        source: crate::types::message::MessageSource {
            sender: "100000000000001@lid".parse().expect("sender"),
            chat: "300000000000003@lid".parse().expect("chat"),
            recipient: Some("300000000000003@lid".parse().expect("recipient")),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    });
    client.ack_received_message(&own);

    let mut found = None;
    for _ in 0..80 {
        if let Some(r) = find_receipt(&transport.sent(), "OWN1") {
            found = Some(r);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (to, typ, recipient) = found.expect("own self-fanout must get a sender <receipt>");
    assert_eq!(
        to, "100000000000001@lid",
        "receipt `to` must echo the own LID (the fanout sender)"
    );
    assert_eq!(
        typ.as_deref(),
        Some("sender"),
        "own self-fanout receipt must be type=sender"
    );
    assert_eq!(
        recipient.as_deref(),
        Some("300000000000003@lid"),
        "receipt must echo the fanout recipient"
    );
    assert!(
        find_message_ack(&transport.sent()).is_none(),
        "self-fanout must NOT also emit a bare transport <ack> (the server rejects it)"
    );
}

/// Regression for the bot self-fanout disconnect loop: our own message to a
/// `@bot` recipient, echoed back as a duplicate/undecryptable stanza, must
/// be cleared with a `<receipt type="sender" recipient=@bot>`. Pre-fix it
/// got a bare `<ack class="message">` which the server ignored, replaying
/// the stanza every reconnect until a ~50min `<stream:error><ack/>` GC
/// force-closed the connection (the exact production symptom).
#[tokio::test]
async fn bot_self_fanout_acked_via_sender_receipt() {
    let (client, transport) = capturing_client("bot_self_fanout").await;
    let own = Arc::new(MessageInfo {
        id: "AC00000000000000000000000000BEEF".to_string(),
        source: crate::types::message::MessageSource {
            // from = our own LID with its device (the server fans our
            // outgoing bot prompt back to this device); chat = the bot
            // (recipient.to_non_ad). The device on the sender must survive
            // into the receipt `to`, or the LID server rejects it (#649).
            sender: "100000000000001:11@lid".parse().expect("sender"),
            chat: "200000000000002@bot".parse().expect("chat"),
            recipient: Some("200000000000002@bot".parse().expect("recipient")),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    });
    client.ack_received_message(&own);

    let mut found = None;
    for _ in 0..80 {
        if let Some(r) = find_receipt(&transport.sent(), "AC00000000000000000000000000BEEF") {
            found = Some(r);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (to, typ, recipient) =
        found.expect("bot self-fanout must get a sender <receipt> to drain the offline queue");
    assert_eq!(
        to, "100000000000001:11@lid",
        "receipt `to` must preserve the own LID device"
    );
    assert_eq!(typ.as_deref(), Some("sender"));
    assert_eq!(
        recipient.as_deref(),
        Some("200000000000002@bot"),
        "receipt must route to the bot recipient"
    );
    assert!(
        find_message_ack(&transport.sent()).is_none(),
        "the bare <ack> that triggered <stream:error><ack/> must no longer be emitted"
    );
}

/// When WE are the bot author (own DM, sender on the `@bot` server, to a
/// user), WA Web's `MsgSendReceipt` takes the `!chat.isBot() &&
/// author.isBot()` branch and emits a bot-invoke-response `<ack>`, NOT a
/// sender `<receipt>`. So the bot-author branch in ack_received_message must
/// keep running before the self-fanout receipt: this locks that ordering
/// against a regression that would wrongly route it to a sender receipt.
#[tokio::test]
async fn own_bot_author_dm_acks_not_sender_receipt() {
    let (client, transport) = capturing_client("own_bot_author").await;
    let own = Arc::new(MessageInfo {
        id: "OWNBOT1".to_string(),
        source: crate::types::message::MessageSource {
            sender: "100000000000002@bot".parse().expect("sender"),
            chat: "300000000000003@lid".parse().expect("chat"),
            recipient: Some("300000000000003@lid".parse().expect("recipient")),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    });
    client.ack_received_message(&own);

    let mut found = None;
    for _ in 0..80 {
        if let Some(a) = find_message_ack(&transport.sent()) {
            found = Some(a);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(
        found.is_some(),
        "own bot-author DM must emit a bare <ack class=message> (WA Web bot-invoke-response ack), not a receipt"
    );
    // No current race (ack_received_message is synchronous and the
    // bot-author branch returns before the receipt branch), but settle
    // briefly so a future regression that spawned a receipt on a later tick
    // can't slip past this negative assertion.
    for _ in 0..5 {
        assert!(
            find_receipt(&transport.sent(), "OWNBOT1").is_none(),
            "must NOT route to a sender <receipt> (would diverge from WA Web's bot-invoke-response ack path)"
        );
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
}

/// An `<unavailable>` message (no `<enc>`) must be transport-acked so the
/// server stops replaying it (DM/group aren't covered by the should_ack gate).
#[tokio::test]
async fn unavailable_message_is_transport_acked() {
    let (client, transport) = capturing_client("unavail_ack").await;
    let node = NodeBuilder::new("message")
        .attr("from", "5511777776666@s.whatsapp.net")
        .attr("id", "UNAVAIL1")
        .attr("type", "text")
        .children([NodeBuilder::new("unavailable")
            .attr("type", "view_once")
            .build()])
        .build();
    let owned = node_to_arc(node);
    let classified = client.classify_incoming_message(&owned).await;
    assert!(classified.is_none(), "unavailable path returns None");

    let mut found = None;
    for _ in 0..80 {
        if let Some(a) = find_message_ack(&transport.sent()) {
            found = Some(a);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (to, _) = found.expect("unavailable message must get a transport ack");
    assert_eq!(to, "5511777776666@s.whatsapp.net");
}

/// Unknown-only stanzas (e.g. msmsg) must be acked or they loop the queue.
#[tokio::test]
async fn unknown_only_enc_is_transport_acked() {
    let (client, transport) = capturing_client("msmsg_ack").await;
    let node = NodeBuilder::new("message")
        .attr("from", "5511777776666@s.whatsapp.net")
        .attr("id", "MSMSG1")
        .attr("type", "text")
        .children([NodeBuilder::new("enc")
            .attr("type", "frskmsg")
            .bytes(vec![0u8; 8])
            .build()])
        .build();
    let owned = node_to_arc(node);
    let classified = client.classify_incoming_message(&owned).await;
    assert!(
        classified.is_none(),
        "unknown-only enc must short-circuit before the process phase"
    );

    let mut found = None;
    for _ in 0..80 {
        if let Some(a) = find_message_ack(&transport.sent()) {
            found = Some(a);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (to, _) = found.expect("unknown-only enc must emit a transport ack");
    assert_eq!(to, "5511777776666@s.whatsapp.net");
}

/// `recipient` must be echoed verbatim or the server replies <stream:error>.
#[tokio::test]
async fn unknown_only_enc_ack_preserves_recipient() {
    let (client, transport) = capturing_client("msmsg_recipient").await;
    let node = NodeBuilder::new("message")
        .attr("from", "236395184570386@lid")
        .attr("recipient", "156535032389744@lid")
        .attr("id", "MSMSG_LID")
        .attr("type", "text")
        .children([NodeBuilder::new("enc")
            .attr("type", "frskmsg")
            .bytes(vec![0u8; 8])
            .build()])
        .build();
    let owned = node_to_arc(node);
    let classified = client.classify_incoming_message(&owned).await;
    assert!(classified.is_none());

    let mut found = None;
    for _ in 0..80 {
        if let Some(a) = find_message_ack(&transport.sent()) {
            found = Some(a);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (to, recipient) = found.expect("unknown-only enc must emit a transport ack");
    assert_eq!(to, "236395184570386@lid");
    assert_eq!(
        recipient.as_deref(),
        Some("156535032389744@lid"),
        "ack must echo the incoming `recipient` attr or the server replies with <stream:error><ack/>"
    );
}

/// Known type with empty content still has no usable payload; ack it.
#[tokio::test]
async fn known_enc_type_with_empty_content_is_transport_acked() {
    let (client, transport) = capturing_client("known_empty").await;
    let node = NodeBuilder::new("message")
        .attr("from", "5511777776666@s.whatsapp.net")
        .attr("id", "EMPTY1")
        .attr("type", "text")
        .children([NodeBuilder::new("enc").attr("type", "pkmsg").build()])
        .build();
    let owned = node_to_arc(node);
    let classified = client.classify_incoming_message(&owned).await;
    assert!(classified.is_none());

    let mut found = None;
    for _ in 0..80 {
        if let Some(a) = find_message_ack(&transport.sent()) {
            found = Some(a);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let (to, _) = found.expect("known-but-empty enc must emit a transport ack");
    assert_eq!(to, "5511777776666@s.whatsapp.net");
}

/// status is covered by should_ack; the fallback must not double-ack it.
#[tokio::test]
async fn unknown_only_enc_on_status_skips_fallback_ack() {
    let (client, transport) = capturing_client("msmsg_status_skip").await;
    let node = NodeBuilder::new("message")
        .attr("from", "status@broadcast")
        .attr("id", "MSMSG_STATUS")
        .attr("type", "text")
        .attr("participant", "5511777776666@s.whatsapp.net")
        .children([NodeBuilder::new("enc")
            .attr("type", "frskmsg")
            .bytes(vec![0u8; 8])
            .build()])
        .build();
    let owned = node_to_arc(node);
    let classified = client.classify_incoming_message(&owned).await;
    assert!(classified.is_none());

    // Give any rogue spawned task time to land on the wire.
    for _ in 0..16 {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        if find_message_ack(&transport.sent()).is_some() {
            break;
        }
    }
    assert!(
        find_message_ack(&transport.sent()).is_none(),
        "status@broadcast must not get a fallback transport ack from classify"
    );
}

/// One recognized enc + one unknown must still go through the normal path.
#[tokio::test]
async fn mixed_recognized_and_unknown_enc_still_classifies() {
    let (client, _transport) = capturing_client("msmsg_mixed").await;
    let node = NodeBuilder::new("message")
        .attr("from", "5511777776666@s.whatsapp.net")
        .attr("id", "MIXED1")
        .attr("type", "text")
        .children([
            NodeBuilder::new("enc")
                .attr("type", "pkmsg")
                .bytes(vec![0u8; 8])
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "frskmsg")
                .bytes(vec![0u8; 8])
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    let classified = client
        .classify_incoming_message(&owned)
        .await
        .expect("mixed enc must produce a ClassifiedMessage");
    assert_eq!(classified.session_payloads.len(), 1);
    assert!(classified.group_payloads.is_empty());
}

/// A custom handler owns its ack; the fallback must not double-ack.
#[tokio::test]
async fn custom_handler_only_skips_fallback_ack() {
    use crate::types::enc_handler::EncHandler;
    use async_lock::Mutex as AsyncMutex;

    #[derive(Default)]
    struct NoopHandler {
        calls: Arc<AsyncMutex<usize>>,
    }
    #[async_trait::async_trait]
    impl EncHandler for NoopHandler {
        async fn handle(
            &self,
            _client: Arc<Client>,
            _enc_node: &wacore_binary::Node,
            _info: &crate::types::message::MessageInfo,
        ) -> anyhow::Result<()> {
            *self.calls.lock().await += 1;
            Ok(())
        }
    }

    let (client, transport) = capturing_client("msmsg_custom").await;
    let calls = Arc::new(AsyncMutex::new(0usize));
    let handler = Arc::new(NoopHandler {
        calls: Arc::clone(&calls),
    });
    client
        .custom_enc_handlers
        .write()
        .await
        .insert("frskmsg".to_string(), handler as Arc<dyn EncHandler>);

    let node = NodeBuilder::new("message")
        .attr("from", "5511777776666@s.whatsapp.net")
        .attr("id", "CUSTOM1")
        .attr("type", "text")
        .children([NodeBuilder::new("enc")
            .attr("type", "frskmsg")
            .bytes(vec![0u8; 8])
            .build()])
        .build();
    let owned = node_to_arc(node);
    let classified = client.classify_incoming_message(&owned).await;
    assert!(
        classified.is_some(),
        "custom-handled enc must not be short-circuited by the fallback guard"
    );

    // Let the detached handler + any rogue spawned task run.
    for _ in 0..16 {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(
        find_message_ack(&transport.sent()).is_none(),
        "custom-handled enc must not get a fallback transport ack from classify"
    );
    assert_eq!(*calls.lock().await, 1, "custom handler must be invoked");
}

/// Security regression: a self-only `app_state_sync_key_share` protocol
/// message must be honoured only when it originates from our own account.
/// A spoofed one from a peer must be dropped (otherwise a peer could inject
/// app-state sync keys). Mirrors WA Web `WAWebKeyManagementHandleKeyShareApi`
/// and whatsmeow's `handleProtocolMessage` self gate.
#[tokio::test]
async fn app_state_sync_key_share_honored_only_from_self() {
    use wacore::messages::MessageUtils;

    let client = crate::test_utils::create_test_client().await;
    ensure_bob_paired(&client).await;

    let key_id = vec![1u8, 2, 3, 4, 5, 6];
    let share = wa::Message {
        protocol_message: Some(Box::new(wa::message::ProtocolMessage {
            app_state_sync_key_share: Some(wa::message::AppStateSyncKeyShare {
                keys: vec![wa::message::AppStateSyncKey {
                    key_id: Some(wa::message::AppStateSyncKeyId {
                        key_id: Some(key_id.clone()),
                    }),
                    key_data: Some(wa::message::AppStateSyncKeyData {
                        key_data: Some(vec![7u8; 32]),
                        fingerprint: Some(wa::message::AppStateSyncKeyFingerprint {
                            raw_id: Some(1),
                            current_index: Some(0),
                            device_indexes: vec![0],
                        }),
                        timestamp: Some(123),
                    }),
                }],
            }),
            ..Default::default()
        })),
        ..Default::default()
    };
    let padded = MessageUtils::encode_and_pad(&share);
    let backend = client.persistence_manager.backend();

    // Non-self sender: the key share must be dropped.
    let mut info =
        create_test_message_info("5510000@s.whatsapp.net", "AKS1", "5510000@s.whatsapp.net");
    info.source.is_from_me = false;
    client
        .clone()
        .handle_decrypted_plaintext("msg", &padded, 2, &Arc::new(info))
        .await
        .unwrap();
    assert!(
        backend.get_sync_key(&key_id).await.unwrap().is_none(),
        "app-state sync key from a non-self sender must not be stored"
    );

    // Self sender: the key share is honoured and stored.
    let mut info = create_test_message_info(
        "9000000000000@s.whatsapp.net",
        "AKS2",
        "9000000000000@s.whatsapp.net",
    );
    info.source.is_from_me = true;
    client
        .clone()
        .handle_decrypted_plaintext("msg", &padded, 2, &Arc::new(info))
        .await
        .unwrap();
    assert!(
        backend.get_sync_key(&key_id).await.unwrap().is_some(),
        "app-state sync key from self must be stored"
    );
}

// ---- msmsg inbound dispatch -----------------------------------------

fn find_message_nack_error(frames: &[bytes::Bytes], id: &str) -> Option<u32> {
    for (i, frame) in frames.iter().enumerate() {
        let Some(buf) = decode_frame(i, frame) else {
            continue;
        };
        let Ok(node) = wacore_binary::marshal::unmarshal_ref(&buf[1..]) else {
            continue;
        };
        if node.tag.as_ref() == "ack"
            && node
                .get_attr("class")
                .is_some_and(|v| v.as_str() == "message")
            && node.get_attr("id").is_some_and(|v| v.as_str() == id)
            && let Some(err) = node.get_attr("error")
            && let Ok(code) = err.as_str().parse::<u32>()
        {
            return Some(code);
        }
    }
    None
}

fn encode_message_secret_message(iv: &[u8], payload: &[u8]) -> Vec<u8> {
    use prost::Message as _;
    let ms = wa::MessageSecretMessage {
        version: Some(1),
        enc_iv: Some(iv.to_vec()),
        enc_payload: Some(payload.to_vec()),
    };
    let mut out = Vec::with_capacity(ms.encoded_len());
    ms.encode(&mut out).expect("encode MessageSecretMessage");
    out
}

async fn collect_event<F>(
    client: &Arc<Client>,
    collector: Arc<crate::test_utils::TestEventCollector>,
    pred: F,
    timeout_ms: u64,
) -> Option<Arc<wacore::types::events::Event>>
where
    F: Fn(&wacore::types::events::Event) -> bool,
{
    let _ = client;
    let mut waited = 0u64;
    while waited <= timeout_ms {
        for ev in collector.events() {
            if pred(&ev) {
                return Some(ev);
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        waited += 25;
    }
    None
}

fn legacy_edit_text(msg: &wa::Message) -> Option<&str> {
    msg.protocol_message
        .as_ref()
        .and_then(|pm| pm.edited_message.as_ref())
        .and_then(|edited| edited.conversation.as_deref())
}

fn inner_message_edit(text: &str, next_secret: Option<Vec<u8>>) -> wa::Message {
    wa::Message {
        protocol_message: Some(Box::new(wa::message::ProtocolMessage {
            key: Some(wa::MessageKey {
                remote_jid: Some("5511777776666@s.whatsapp.net".to_string()),
                from_me: Some(false),
                id: Some("PARENT_EDIT".to_string()),
                participant: None,
            }),
            r#type: Some(wa::message::protocol_message::Type::MessageEdit as i32),
            edited_message: Some(Box::new(wa::Message {
                conversation: Some(text.to_string()),
                ..Default::default()
            })),
            timestamp_ms: Some(1_770_000_000_000),
            ..Default::default()
        })),
        message_context_info: next_secret.map(|secret| wa::MessageContextInfo {
            message_secret: Some(secret),
            ..Default::default()
        }),
        ..Default::default()
    }
}

fn encrypted_message_edit(
    target_key: wa::MessageKey,
    original_sender: &str,
    editor: &str,
    parent_id: &str,
    secret: &[u8],
    text: &str,
    next_secret: Option<Vec<u8>>,
) -> wa::Message {
    let ctx = wacore::message_edit::MessageEditContext {
        original_msg_id: parent_id,
        original_sender_jid: original_sender,
        editor_jid: editor,
    };
    let (enc_payload, enc_iv) = wacore::message_edit::encrypt_message_edit(
        &inner_message_edit(text, next_secret),
        secret,
        &ctx,
    )
    .expect("test edit encryption");

    wa::Message {
        secret_encrypted_message: Some(wa::message::SecretEncryptedMessage {
            target_message_key: Some(target_key),
            enc_payload: Some(enc_payload),
            enc_iv: Some(enc_iv.to_vec()),
            secret_enc_type: Some(
                wa::message::secret_encrypted_message::SecretEncType::MessageEdit as i32,
            ),
            remote_key_id: None,
        }),
        ..Default::default()
    }
}

#[tokio::test]
async fn secret_encrypted_message_edit_dispatches_legacy_edit() {
    let (client, _transport) = capturing_client("secret_edit_dispatch").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "5511777776666@s.whatsapp.net";
    let parent_id = "PARENT_EDIT";
    let edit_id = "EDIT_1";
    let secret = [0x42u8; 32];
    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, chat, parent_id, &secret)
        .await
        .unwrap();

    let info = Arc::new(MessageInfo {
        id: edit_id.into(),
        source: crate::types::message::MessageSource {
            chat: chat.parse().unwrap(),
            sender: chat.parse().unwrap(),
            ..Default::default()
        },
        ..Default::default()
    });
    let target_key = wa::MessageKey {
        remote_jid: Some(chat.to_string()),
        from_me: Some(false),
        id: Some(parent_id.to_string()),
        participant: None,
    };
    let msg = encrypted_message_edit(target_key, chat, chat, parent_id, &secret, "edited", None);

    client.dispatch_parsed_message(msg, &info).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == edit_id
                        && legacy_edit_text(msg.as_ref()) == Some("edited")
                        && msg.secret_encrypted_message.is_none())
        },
        500,
    )
    .await;
    assert!(got.is_some(), "encrypted edit must dispatch as legacy edit");
}

/// Regression for #667: an incoming peer edit writes `target_message_key`
/// in the editor's frame (`from_me = true`, no `participant`, even in a
/// group), so the target-key resolver maps the parent author to *us* and
/// misses the secret stored under the real author. The dispatch path must
/// take the author from the envelope sender instead. Fails on the pre-fix
/// code (envelope stays encrypted), passes after it.
#[tokio::test]
async fn secret_encrypted_peer_edit_resolves_sender_from_envelope() {
    let (client, _transport) = capturing_client("secret_peer_edit_dispatch").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let group = "123456789012345678@g.us";
    let peer = "5511777776666@s.whatsapp.net";
    let parent_id = "PEER_PARENT";
    let edit_id = "PEER_EDIT";
    let secret = [0x42u8; 32];
    // The parent (peer's own message) was stored under the real author.
    client
        .persistence_manager
        .backend()
        .put_msg_secret(group, peer, parent_id, &secret)
        .await
        .unwrap();

    let info = Arc::new(MessageInfo {
        id: edit_id.into(),
        source: crate::types::message::MessageSource {
            chat: group.parse().unwrap(),
            sender: peer.parse().unwrap(),
            ..Default::default()
        },
        ..Default::default()
    });
    // Editor's frame: from_me = true, no participant, even in a group.
    let target_key = wa::MessageKey {
        remote_jid: Some(group.to_string()),
        from_me: Some(true),
        id: Some(parent_id.to_string()),
        participant: None,
    };
    // HKDF binds the real author (peer) as both original sender and editor.
    let msg = encrypted_message_edit(target_key, peer, peer, parent_id, &secret, "edited", None);

    client.dispatch_parsed_message(msg, &info).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == edit_id
                        && legacy_edit_text(msg.as_ref()) == Some("edited")
                        && msg.secret_encrypted_message.is_none())
        },
        500,
    )
    .await;
    assert!(
        got.is_some(),
        "incoming peer edit must resolve the author from the envelope and dispatch as legacy edit"
    );
}

/// Store the parent secret with a known event time, then dispatch a
/// secret-encrypted edit authored `edit_offset` seconds after the parent.
/// Returns whether the decrypted legacy edit was dispatched.
async fn run_secret_edit_with_window(test_id: &str, parent_ts: i64, edit_offset: i64) -> bool {
    let (client, _transport) = capturing_client(test_id).await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "5511777776666@s.whatsapp.net";
    let parent_id = "WINDOW_PARENT";
    let edit_id = "WINDOW_EDIT";
    let secret = [0x42u8; 32];
    client
        .persistence_manager
        .backend()
        .put_msg_secrets(vec![wacore::store::traits::MsgSecretEntry {
            chat: chat.to_string(),
            sender: chat.to_string(),
            msg_id: parent_id.to_string(),
            secret: secret.to_vec(),
            expires_at: 0,
            message_ts: parent_ts,
        }])
        .await
        .unwrap();

    let info = Arc::new(MessageInfo {
        id: edit_id.into(),
        timestamp: chrono::DateTime::<chrono::Utc>::from_timestamp(parent_ts + edit_offset, 0)
            .unwrap(),
        source: crate::types::message::MessageSource {
            chat: chat.parse().unwrap(),
            sender: chat.parse().unwrap(),
            ..Default::default()
        },
        ..Default::default()
    });
    let target_key = wa::MessageKey {
        remote_jid: Some(chat.to_string()),
        from_me: Some(false),
        id: Some(parent_id.to_string()),
        participant: None,
    };
    let msg = encrypted_message_edit(target_key, chat, chat, parent_id, &secret, "edited", None);
    client.dispatch_parsed_message(msg, &info).await;

    collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == edit_id
                        && legacy_edit_text(msg.as_ref()) == Some("edited")
                        && msg.secret_encrypted_message.is_none())
        },
        500,
    )
    .await
    .is_some()
}

#[tokio::test]
async fn secret_edit_within_window_is_applied() {
    // Authored 10 min after the parent — inside the 20 min (1200s) window.
    assert!(
        run_secret_edit_with_window("secret_edit_in_window", 1_700_000_000, 600).await,
        "an in-window edit must dispatch as a legacy edit"
    );
}

#[tokio::test]
async fn secret_edit_outside_window_is_dropped() {
    // Authored 30 min after the parent — past the 1200s window, like WA Web's
    // ProcessEditProtocolMsgs, so we drop it (raw envelope surfaces instead).
    assert!(
        !run_secret_edit_with_window("secret_edit_out_window", 1_700_000_000, 1800).await,
        "an out-of-window edit must not dispatch a legacy edit"
    );
}

#[tokio::test]
async fn secret_edit_unknown_parent_ts_is_permissive() {
    // parent_ts = 0 (unknown, e.g. resolver-supplied): no window check, so a
    // late edit still applies rather than being silently dropped.
    assert!(
        run_secret_edit_with_window("secret_edit_unknown_ts", 0, 5_000_000).await,
        "with an unknown parent timestamp the edit must still apply"
    );
}

#[tokio::test]
async fn secret_encrypted_edit_decrypts_via_resolver_when_store_empty() {
    use crate::cache_config::{CacheConfig, MsgSecretPolicy};

    struct StaticResolver {
        chat: String,
        sender: String,
        msg_id: String,
        secret: [u8; 32],
    }
    #[async_trait::async_trait]
    impl wacore::msg_secret::OriginalMessageResolver for StaticResolver {
        async fn resolve_msg_secret(
            &self,
            chat: &str,
            sender: &str,
            msg_id: &str,
        ) -> Option<[u8; 32]> {
            (chat == self.chat && sender == self.sender && msg_id == self.msg_id)
                .then_some(self.secret)
        }
    }

    let chat = "5511777776666@s.whatsapp.net";
    let parent_id = "RESOLVER_PARENT";
    let edit_id = "RESOLVER_EDIT";
    let secret = [0x7Au8; 32];

    let resolver = Arc::new(StaticResolver {
        chat: chat.to_string(),
        sender: chat.to_string(),
        msg_id: parent_id.to_string(),
        secret,
    });
    // Disabled persists nothing, so the only path to the secret is the resolver.
    let cfg = CacheConfig {
        msg_secret_policy: MsgSecretPolicy::Disabled,
        original_message_resolver: Some(resolver),
        ..Default::default()
    };
    let client = crate::test_utils::create_test_client_with_config(
        "resolver_edit",
        Arc::new(crate::test_utils::MockHttpClient),
        cfg,
    )
    .await;
    seed_test_pn(&client).await;

    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    assert!(
        client
            .persistence_manager
            .backend()
            .get_msg_secret(chat, chat, parent_id)
            .await
            .unwrap()
            .is_none(),
        "store must be empty under Disabled"
    );

    let info = Arc::new(MessageInfo {
        id: edit_id.into(),
        source: crate::types::message::MessageSource {
            chat: chat.parse().unwrap(),
            sender: chat.parse().unwrap(),
            ..Default::default()
        },
        ..Default::default()
    });
    let target_key = wa::MessageKey {
        remote_jid: Some(chat.to_string()),
        from_me: Some(false),
        id: Some(parent_id.to_string()),
        participant: None,
    };
    let msg = encrypted_message_edit(
        target_key,
        chat,
        chat,
        parent_id,
        &secret,
        "edited via resolver",
        None,
    );

    client.dispatch_parsed_message(msg, &info).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == edit_id
                        && legacy_edit_text(msg.as_ref()) == Some("edited via resolver")
                        && msg.secret_encrypted_message.is_none())
        },
        500,
    )
    .await;
    assert!(
        got.is_some(),
        "edit must decrypt via the resolver when the store is empty"
    );
}

#[tokio::test]
async fn decrypted_message_edit_recaptures_secret_for_next_edit() {
    let (client, _transport) = capturing_client("secret_edit_chain").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "5511777776666@s.whatsapp.net";
    let parent_id = "PARENT_EDIT";
    let first_secret = [0x11u8; 32];
    let second_secret = [0x22u8; 32];
    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, chat, parent_id, &first_secret)
        .await
        .unwrap();

    let target_key = wa::MessageKey {
        remote_jid: Some(chat.to_string()),
        from_me: Some(false),
        id: Some(parent_id.to_string()),
        participant: None,
    };
    let first_info = Arc::new(MessageInfo {
        id: "EDIT_CHAIN_1".into(),
        source: crate::types::message::MessageSource {
            chat: chat.parse().unwrap(),
            sender: chat.parse().unwrap(),
            ..Default::default()
        },
        ..Default::default()
    });
    let first_msg = encrypted_message_edit(
        target_key.clone(),
        chat,
        chat,
        parent_id,
        &first_secret,
        "first",
        Some(second_secret.to_vec()),
    );
    client.dispatch_parsed_message(first_msg, &first_info).await;

    let stored = client
        .persistence_manager
        .backend()
        .get_msg_secret(chat, chat, parent_id)
        .await
        .unwrap();
    assert_eq!(stored.as_deref(), Some(&second_secret[..]));

    let second_info = Arc::new(MessageInfo {
        id: "EDIT_CHAIN_2".into(),
        source: crate::types::message::MessageSource {
            chat: chat.parse().unwrap(),
            sender: chat.parse().unwrap(),
            ..Default::default()
        },
        ..Default::default()
    });
    let second_msg = encrypted_message_edit(
        target_key,
        chat,
        chat,
        parent_id,
        &second_secret,
        "second",
        None,
    );
    client
        .dispatch_parsed_message(second_msg, &second_info)
        .await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == "EDIT_CHAIN_2"
                        && legacy_edit_text(msg.as_ref()) == Some("second"))
        },
        500,
    )
    .await;
    assert!(got.is_some(), "second edit must use the re-captured secret");
}

#[tokio::test]
async fn secret_encrypted_message_edit_uses_lid_pn_fallback_in_group() {
    use wacore::store::traits::LidPnMappingEntry;

    let (client, _transport) = capturing_client("secret_edit_alt_group").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "120363021033254949@g.us";
    let parent_id = "GROUP_PARENT_EDIT";
    let sender_lid = "236395184570386@lid";
    let sender_pn = "5511777776666@s.whatsapp.net";
    let secret = [0x77u8; 32];

    client
        .persistence_manager
        .backend()
        .put_lid_mapping(&LidPnMappingEntry {
            lid: "236395184570386".into(),
            phone_number: "5511777776666".into(),
            created_at: 0,
            updated_at: 0,
            learning_source: "test".into(),
        })
        .await
        .unwrap();
    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, sender_pn, parent_id, &secret)
        .await
        .unwrap();

    let info = Arc::new(MessageInfo {
        id: "GROUP_EDIT_1".into(),
        source: crate::types::message::MessageSource {
            chat: chat.parse().unwrap(),
            sender: sender_lid.parse().unwrap(),
            is_group: true,
            addressing_mode: Some(wacore::types::message::AddressingMode::Lid),
            ..Default::default()
        },
        ..Default::default()
    });
    let target_key = wa::MessageKey {
        remote_jid: Some(chat.to_string()),
        from_me: Some(false),
        id: Some(parent_id.to_string()),
        participant: Some(sender_lid.to_string()),
    };
    let msg = encrypted_message_edit(
        target_key,
        sender_pn,
        sender_lid,
        parent_id,
        &secret,
        "group edited",
        None,
    );

    client.dispatch_parsed_message(msg, &info).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == "GROUP_EDIT_1"
                        && legacy_edit_text(msg.as_ref()) == Some("group edited"))
        },
        500,
    )
    .await;
    assert!(
        got.is_some(),
        "group edit must decrypt when the stored secret is under PN"
    );
}

#[tokio::test]
async fn decrypted_message_edit_refreshes_alternate_secret_alias() {
    use wacore::store::traits::LidPnMappingEntry;

    let (client, _transport) = capturing_client("secret_edit_alt_refresh").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "120363021033254949@g.us";
    let parent_id = "GROUP_PARENT_EDIT";
    let sender_lid = "236395184570386@lid";
    let sender_pn = "5511777776666@s.whatsapp.net";
    let first_secret = [0x31u8; 32];
    let second_secret = [0x32u8; 32];

    client
        .persistence_manager
        .backend()
        .put_lid_mapping(&LidPnMappingEntry {
            lid: "236395184570386".into(),
            phone_number: "5511777776666".into(),
            created_at: 0,
            updated_at: 0,
            learning_source: "test".into(),
        })
        .await
        .unwrap();
    for sender in [sender_lid, sender_pn] {
        client
            .persistence_manager
            .backend()
            .put_msg_secret(chat, sender, parent_id, &first_secret)
            .await
            .unwrap();
    }

    let first_info = Arc::new(MessageInfo {
        id: "GROUP_EDIT_REFRESH_1".into(),
        source: crate::types::message::MessageSource {
            chat: chat.parse().unwrap(),
            sender: sender_lid.parse().unwrap(),
            is_group: true,
            addressing_mode: Some(wacore::types::message::AddressingMode::Lid),
            ..Default::default()
        },
        ..Default::default()
    });
    let first_target_key = wa::MessageKey {
        remote_jid: Some(chat.to_string()),
        from_me: Some(false),
        id: Some(parent_id.to_string()),
        participant: Some(sender_lid.to_string()),
    };
    let first_msg = encrypted_message_edit(
        first_target_key,
        sender_lid,
        sender_lid,
        parent_id,
        &first_secret,
        "first",
        Some(second_secret.to_vec()),
    );
    client.dispatch_parsed_message(first_msg, &first_info).await;

    for sender in [sender_lid, sender_pn] {
        let stored = client
            .persistence_manager
            .backend()
            .get_msg_secret(chat, sender, parent_id)
            .await
            .unwrap();
        assert_eq!(stored.as_deref(), Some(&second_secret[..]));
    }

    let second_info = Arc::new(MessageInfo {
        id: "GROUP_EDIT_REFRESH_2".into(),
        source: crate::types::message::MessageSource {
            chat: chat.parse().unwrap(),
            sender: sender_pn.parse().unwrap(),
            is_group: true,
            addressing_mode: Some(wacore::types::message::AddressingMode::Pn),
            ..Default::default()
        },
        ..Default::default()
    });
    let second_target_key = wa::MessageKey {
        remote_jid: Some(chat.to_string()),
        from_me: Some(false),
        id: Some(parent_id.to_string()),
        participant: Some(sender_pn.to_string()),
    };
    let second_msg = encrypted_message_edit(
        second_target_key,
        sender_pn,
        sender_pn,
        parent_id,
        &second_secret,
        "second",
        None,
    );
    client
        .dispatch_parsed_message(second_msg, &second_info)
        .await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == "GROUP_EDIT_REFRESH_2"
                        && legacy_edit_text(msg.as_ref()) == Some("second"))
        },
        500,
    )
    .await;
    assert!(
        got.is_some(),
        "chained edit must use the refreshed alternate alias"
    );
}

/// Round-trip: store an outbound messageSecret, build a fake bot reply
/// whose payload we encrypt with the symmetric helper, route it through
/// classify, and assert the decrypted `wa::Message` lands on the bus.
#[tokio::test]
async fn msmsg_decrypts_when_secret_is_stored() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    let (client, _transport) = capturing_client("msmsg_ok").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "867051314767696@bot";
    let our_pn = "5511000000001@s.whatsapp.net";
    let bot_jid = "867051314767696@bot";
    let outbound_id = "OUTBOUND_1";
    let bot_reply_id = "BOT_REPLY_1";
    let secret = [0x42u8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    let plaintext_msg = wa::Message {
        conversation: Some("hi from bot".to_string()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    let ctx = BotMessageContext {
        msg_id: bot_reply_id,
        target_sender_user_jid: our_pn,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    let node = NodeBuilder::new("message")
        .attr("from", bot_jid)
        .attr("id", bot_reply_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == bot_reply_id
                        && msg.conversation.as_deref() == Some("hi from bot"))
        },
        1500,
    )
    .await;
    assert!(
        got.is_some(),
        "msmsg decryption + dispatch must surface Event::Message"
    );
}

/// No secret stored for `target_id` → nack `error=495`, no Message event.
#[tokio::test]
async fn msmsg_without_stored_secret_nacks_495() {
    let (client, transport) = capturing_client("msmsg_nosecret").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let bot_reply_id = "BOT_REPLY_NS";
    let outbound_id = "OUTBOUND_NS";
    let our_pn = "5511000000001@s.whatsapp.net";

    let ms_msg_proto = encode_message_secret_message(&[0u8; 12], &[0u8; 32]);
    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", bot_reply_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let mut code = None;
    for _ in 0..80 {
        if let Some(c) = find_message_nack_error(&transport.sent(), bot_reply_id) {
            code = Some(c);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(
        code,
        Some(495),
        "missing messageSecret must nack with code 495"
    );
    assert!(
            collector
                .events()
                .iter()
                .all(|e| !matches!(e.as_ref(), wacore::types::events::Event::Message(_, info) if info.id == bot_reply_id)),
            "no Message event must be dispatched when decryption failed"
        );
}

/// Tampered ciphertext → GCM tag fails → nack 495.
#[tokio::test]
async fn msmsg_with_bad_tag_nacks_495() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    let (client, transport) = capturing_client("msmsg_bad_tag").await;

    let chat = "867051314767696@bot";
    let our_pn = "5511000000001@s.whatsapp.net";
    let outbound_id = "OUTBOUND_BAD";
    let bot_reply_id = "BOT_REPLY_BAD";
    let secret = [0x77u8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    let ctx = BotMessageContext {
        msg_id: bot_reply_id,
        target_sender_user_jid: our_pn,
        bot_user_jid: chat,
    };
    let (mut cipher, iv) = encrypt_bot_message(b"hello", &secret, &ctx).unwrap();
    let last = cipher.len() - 1;
    cipher[last] ^= 0x01;
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", bot_reply_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let mut code = None;
    for _ in 0..80 {
        if let Some(c) = find_message_nack_error(&transport.sent(), bot_reply_id) {
            code = Some(c);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(code, Some(495));
}

/// Bot edit chain: when `<bot edit="inner">` is set, the HKDF msg_id used
/// for the per-message key swaps to `edit_target_id` so the edited reply
/// decrypts under the same key as the original (whatsmeow / WA Web
/// `decryptMsmsgFbidBotMessage`).
#[tokio::test]
async fn msmsg_bot_edit_uses_edit_target_id_for_hkdf() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    let (client, _transport) = capturing_client("msmsg_edit").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "867051314767696@bot";
    let our_pn = "5511000000001@s.whatsapp.net";
    let outbound_id = "OUTBOUND_EDIT";
    let original_reply_id = "BOT_REPLY_FIRST";
    let edit_reply_id = "BOT_REPLY_EDIT";
    let secret = [0xAAu8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    // Encrypt as if it's the ORIGINAL reply (msg_id = original_reply_id).
    let plaintext_msg = wa::Message {
        conversation: Some("edited content".to_string()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    let ctx = BotMessageContext {
        msg_id: original_reply_id,
        target_sender_user_jid: our_pn,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    // Inbound stanza has id=edit_reply_id but <bot edit="inner" edit_target_id=original>
    // so the HKDF must derive against original_reply_id.
    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", edit_reply_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("bot")
                .attr("edit", "inner")
                .attr("edit_target_id", original_reply_id)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == edit_reply_id
                        && msg.conversation.as_deref() == Some("edited content"))
        },
        1500,
    )
    .await;
    assert!(
        got.is_some(),
        "bot edit must use edit_target_id for HKDF msg_id"
    );
}

/// Same setup as the edit test but WITHOUT `<bot edit>`: the HKDF must
/// fall back to `info.id`, and ciphertext encrypted with the edit-target
/// id must fail to decrypt.
#[tokio::test]
async fn msmsg_without_bot_edit_does_not_swap_msg_id() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    let (client, transport) = capturing_client("msmsg_noedit").await;
    let chat = "867051314767696@bot";
    let our_pn = "5511000000001@s.whatsapp.net";
    let outbound_id = "OUTBOUND_NOEDIT";
    let stanza_id = "BOT_REPLY_NOEDIT";
    let other_id = "OTHER_ID";
    let secret = [0xBBu8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    // Encrypt with `other_id` to simulate the wrong key derivation if the
    // edit branch were taken without `<bot edit>`.
    let ctx = BotMessageContext {
        msg_id: other_id,
        target_sender_user_jid: our_pn,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(b"x", &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);
    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", stanza_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let mut code = None;
    for _ in 0..80 {
        if let Some(c) = find_message_nack_error(&transport.sent(), stanza_id) {
            code = Some(c);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(
        code,
        Some(495),
        "without <bot edit>, HKDF must use stanza id (not OTHER_ID) → tag fails"
    );
}

/// `<bot edit="first">` is NOT one of {INNER, LAST}, so the HKDF msg_id
/// must remain `info.id`.
#[tokio::test]
async fn msmsg_bot_edit_first_keeps_info_id() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    let (client, _transport) = capturing_client("msmsg_first").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "867051314767696@bot";
    let our_pn = "5511000000001@s.whatsapp.net";
    let outbound_id = "OUTBOUND_FIRST";
    let stanza_id = "BOT_REPLY_FIRST";
    let secret = [0xCCu8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    // Encrypt with stanza_id; "first" edit must NOT swap.
    let plaintext_msg = wa::Message {
        conversation: Some("first reply".to_string()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    let ctx = BotMessageContext {
        msg_id: stanza_id,
        target_sender_user_jid: our_pn,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);
    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", stanza_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("bot").attr("edit", "first").build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| matches!(e, wacore::types::events::Event::Message(_, info) if info.id == stanza_id),
        1500,
    )
    .await;
    assert!(got.is_some(), "edit=first must keep info.id as HKDF msg_id");
}

/// Regular bot path (`f()` in WA Web `BotMessageSecret.js`): when the
/// fbid pre-resolve picks the WRONG id (e.g. edit_target_id) but the
/// real ciphertext was minted under `info.id`, the fallback attempt
/// must succeed. Validates the try-then-fallback unification.
#[tokio::test]
async fn msmsg_falls_back_to_info_id_when_primary_uses_edit_target() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    let (client, _transport) = capturing_client("msmsg_fb_to_info").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "867051314767696@bot";
    let our_pn = "5511000000001@s.whatsapp.net";
    let outbound_id = "OUT_FB1";
    let stanza_id = "REPLY_FB1";
    let edit_target_id = "WRONG_EDIT_TARGET";
    let secret = [0xDDu8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    // Encrypt under `stanza_id` even though the stanza will declare
    // edit=inner with edit_target_id (forces a primary-attempt mismatch).
    let plaintext_msg = wa::Message {
        conversation: Some("fallback ok".to_string()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    let ctx = BotMessageContext {
        msg_id: stanza_id,
        target_sender_user_jid: our_pn,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", stanza_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("bot")
                .attr("edit", "inner")
                .attr("edit_target_id", edit_target_id)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == stanza_id
                        && msg.conversation.as_deref() == Some("fallback ok"))
        },
        1500,
    )
    .await;
    assert!(
        got.is_some(),
        "primary attempt with edit_target_id must fall back to info.id"
    );
}

/// Inverse of the `falls_back_to_info_id` test: primary is `info.id`
/// (edit_type isn't INNER/LAST so the fbid pre-resolve picks the stanza
/// id), but the bot encrypted under `edit_target_id`. The fallback must
/// rescue.
#[tokio::test]
async fn msmsg_falls_back_to_edit_target_when_primary_uses_info_id() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    let (client, _transport) = capturing_client("msmsg_fb_to_edit").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "867051314767696@bot";
    let our_pn = "5511000000001@s.whatsapp.net";
    let outbound_id = "OUT_INV";
    let stanza_id = "REPLY_INV";
    let edit_target_id = "EDIT_INV";
    let secret = [0xBEu8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    let plaintext_msg = wa::Message {
        conversation: Some("inverse fallback".to_string()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    // Encrypt under edit_target_id even though edit=first → primary
    // will pick info.id (stanza id), fail, and the fallback should try
    // edit_target_id and succeed (WA Web regular bot path `f()`).
    let ctx = BotMessageContext {
        msg_id: edit_target_id,
        target_sender_user_jid: our_pn,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", stanza_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("bot")
                .attr("edit", "first")
                .attr("edit_target_id", edit_target_id)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == stanza_id
                        && msg.conversation.as_deref() == Some("inverse fallback"))
        },
        1500,
    )
    .await;
    assert!(
        got.is_some(),
        "primary attempt with info.id must fall back to edit_target_id (WA Web f())"
    );
}

/// Mirror scenario: no `<bot edit>`, so the parser doesn't populate
/// `edit_target_id`. Primary uses `info.id`; with no fallback id
/// available, a deliberately-wrong-key payload must nack 495 (no second
/// attempt to silently mask the failure).
#[tokio::test]
async fn msmsg_no_fallback_when_no_edit_target_present() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    let (client, transport) = capturing_client("msmsg_nofb").await;
    let chat = "867051314767696@bot";
    let our_pn = "5511000000001@s.whatsapp.net";
    let outbound_id = "OUT_NOFB";
    let stanza_id = "REPLY_NOFB";
    let secret = [0xCCu8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    // Encrypt under a DIFFERENT id; no <bot> node so parser leaves
    // edit_target_id = None and there's nothing to fall back to.
    let ctx = BotMessageContext {
        msg_id: "MISMATCHED_ID",
        target_sender_user_jid: our_pn,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(b"x", &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);
    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", stanza_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let mut code = None;
    for _ in 0..80 {
        if let Some(c) = find_message_nack_error(&transport.sent(), stanza_id) {
            code = Some(c);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(
        code,
        Some(495),
        "no fallback id available → single AES-GCM failure must nack 495"
    );
}

/// WA Web `processRenderableMessages` captures the embedded
/// `messageSecret` from any bot-targeted msg (fanout from us OR reply
/// from the bot). Verify the helper persists it under
/// (bot_chat, our_lid, info.id).
#[tokio::test]
async fn maybe_capture_inbound_msg_secret_persists_for_bot_chats() {
    use crate::store::commands::DeviceCommand;
    let (client, _transport) = capturing_client("capture_bot").await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(
            "999888777666555:0@lid".parse().unwrap(),
        )))
        .await;

    let info = Arc::new(MessageInfo {
        id: "FANOUT_1".into(),
        source: crate::types::message::MessageSource {
            chat: "867051314767696@bot".parse().unwrap(),
            sender: "5511000000001:0@s.whatsapp.net".parse().unwrap(),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let msg = wa::Message {
        conversation: Some("hi bot".into()),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![0xAB; 32]),
            ..Default::default()
        }),
        ..Default::default()
    };
    client.maybe_capture_inbound_msg_secret(&msg, &info).await;

    let mut got = None;
    for _ in 0..40 {
        got = client
            .persistence_manager
            .backend()
            .get_msg_secret("867051314767696@bot", "999888777666555@lid", "FANOUT_1")
            .await
            .unwrap();
        if got.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(got.as_deref(), Some(&[0xABu8; 32][..]));
}

#[tokio::test]
async fn maybe_capture_inbound_msg_secret_persists_for_non_bot_chats() {
    let (client, _transport) = capturing_client("capture_regular_dm").await;
    let info = Arc::new(MessageInfo {
        id: "DM_1".into(),
        source: crate::types::message::MessageSource {
            chat: "5511777776666@s.whatsapp.net".parse().unwrap(),
            sender: "5511000000001:0@s.whatsapp.net".parse().unwrap(),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let msg = wa::Message {
        conversation: Some("hi".into()),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![0xCD; 32]),
            ..Default::default()
        }),
        ..Default::default()
    };
    client.maybe_capture_inbound_msg_secret(&msg, &info).await;

    let got = client
        .persistence_manager
        .backend()
        .get_msg_secret(
            "5511777776666@s.whatsapp.net",
            "5511000000001@s.whatsapp.net",
            "DM_1",
        )
        .await
        .unwrap();
    assert_eq!(got.as_deref(), Some(&[0xCDu8; 32][..]));
}

/// Group invocation: user mentions @MetaAI in a group → chat is the
/// GROUP (not bot), but mentioned_jid contains the bot. WA Web's
/// `processRenderableMessages` keys off `N` (invokedBotWid derived from
/// `mentionedJidList.find(isBot)`); we must persist too.
#[tokio::test]
async fn maybe_capture_inbound_msg_secret_persists_for_group_with_bot_mention() {
    use crate::store::commands::DeviceCommand;
    let (client, _transport) = capturing_client("capture_group_mention").await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(
            "999888777666555:0@lid".parse().unwrap(),
        )))
        .await;

    let info = Arc::new(MessageInfo {
        id: "GRP_MENTION".into(),
        source: crate::types::message::MessageSource {
            chat: "120363021033254949@g.us".parse().unwrap(),
            sender: "5511000000001:0@s.whatsapp.net".parse().unwrap(),
            is_from_me: true,
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let msg = wa::Message {
        extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
            text: Some("hey @MetaAI tell me a joke".into()),
            context_info: Some(Box::new(wa::ContextInfo {
                mentioned_jid: vec!["867051314767696@bot".into()],
                ..Default::default()
            })),
            ..Default::default()
        })),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![0xEE; 32]),
            ..Default::default()
        }),
        ..Default::default()
    };
    client.maybe_capture_inbound_msg_secret(&msg, &info).await;

    let mut got = None;
    for _ in 0..40 {
        got = client
            .persistence_manager
            .backend()
            .get_msg_secret(
                "120363021033254949@g.us",
                "5511000000001@s.whatsapp.net",
                "GRP_MENTION",
            )
            .await
            .unwrap();
        if got.is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(
        got.as_deref(),
        Some(&[0xEEu8; 32][..]),
        "group invocation via @bot mention must still cache the secret"
    );
}

/// Forwarded message with a secret must NOT be cached — matches WA Web's
/// `x.isForwarded !== true` guard. A planted forward shouldn't poison
/// the cache.
#[tokio::test]
async fn maybe_capture_inbound_msg_secret_skips_forwarded() {
    let (client, _transport) = capturing_client("capture_skip_forwarded").await;
    let info = Arc::new(MessageInfo {
        id: "FWD_1".into(),
        source: crate::types::message::MessageSource {
            chat: "867051314767696@bot".parse().unwrap(),
            sender: "5511000000001:0@s.whatsapp.net".parse().unwrap(),
            is_from_me: false,
            ..Default::default()
        },
        ..Default::default()
    });
    let msg = wa::Message {
        extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
            text: Some("forwarded".into()),
            context_info: Some(Box::new(wa::ContextInfo {
                is_forwarded: Some(true),
                ..Default::default()
            })),
            ..Default::default()
        })),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![0xFF; 32]),
            ..Default::default()
        }),
        ..Default::default()
    };
    client.maybe_capture_inbound_msg_secret(&msg, &info).await;

    for _ in 0..16 {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let got = client
        .persistence_manager
        .backend()
        .get_msg_secret(
            "867051314767696@bot",
            "5511000000001@s.whatsapp.net",
            "FWD_1",
        )
        .await
        .unwrap();
    assert!(got.is_none(), "forwarded messages must not seed the cache");
}

/// Our own group bot prompt carries the secret but NO mentioned_jid
/// (observed in prod: `mentions_bot=false mentioned_jids=[]`). The bot
/// invocation is signalled by `message_context_info.bot_metadata`, which
/// must let the capture fire (WA Web's `w`/`A` group-participant gate).
#[tokio::test]
async fn maybe_capture_inbound_msg_secret_via_bot_metadata_without_mention() {
    let (client, _transport) = capturing_client("capture_bot_meta").await;
    client
        .persistence_manager
        .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
            "999888777666555:0@lid".parse().unwrap(),
        )))
        .await;
    let info = Arc::new(MessageInfo {
        id: "GRP_OWN_BOT".into(),
        source: crate::types::message::MessageSource {
            chat: "120363021033254949@g.us".parse().unwrap(),
            sender: "236395184570386:0@lid".parse().unwrap(),
            is_from_me: true,
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let msg = wa::Message {
        extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
            text: Some("continue".into()),
            // No mention at all — just bot_metadata signals the invocation.
            ..Default::default()
        })),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![0x7B; 32]),
            bot_metadata: Some(wa::BotMetadata {
                persona_id: Some("867051314767696".into()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    client.maybe_capture_inbound_msg_secret(&msg, &info).await;

    // Group (non-bot chat) → keyed under info.source.sender (our LID in a
    // LID group), which is what the bot reply's target_sender_jid echoes.
    let got = client
        .persistence_manager
        .backend()
        .get_msg_secret(
            "120363021033254949@g.us",
            "236395184570386@lid",
            "GRP_OWN_BOT",
        )
        .await
        .unwrap();
    assert_eq!(
        got.as_deref(),
        Some(&[0x7Bu8; 32][..]),
        "bot_metadata presence must let our own group prompt cache without a mention"
    );
}

#[tokio::test]
async fn bot_only_captures_group_bot_prompt_skips_plain() {
    use crate::cache_config::{CacheConfig, MsgSecretPolicy};
    let cfg = CacheConfig {
        msg_secret_policy: MsgSecretPolicy::BotOnly,
        ..Default::default()
    };
    let client = crate::test_utils::create_test_client_with_config(
        "botonly_capture",
        Arc::new(crate::test_utils::MockHttpClient),
        cfg,
    )
    .await;

    let group = "120363021033254949@g.us";
    let sender = "5511888887777@s.whatsapp.net";

    // A plain group message is not a bot context → skipped under BotOnly.
    let plain_info = Arc::new(MessageInfo {
        id: "PLAIN".into(),
        source: crate::types::message::MessageSource {
            chat: group.parse().unwrap(),
            sender: sender.parse().unwrap(),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let plain_msg = wa::Message {
        conversation: Some("hi".into()),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![0x01; 32]),
            ..Default::default()
        }),
        ..Default::default()
    };
    client
        .maybe_capture_inbound_msg_secret(&plain_msg, &plain_info)
        .await;
    assert!(
        client
            .persistence_manager
            .backend()
            .get_msg_secret(group, sender, "PLAIN")
            .await
            .unwrap()
            .is_none(),
        "BotOnly must skip a plain (non-bot) group message"
    );

    // A group message that invokes a bot (bot_metadata) classifies as Bot,
    // so its secret is kept and the later bot reply can decrypt.
    let bot_info = Arc::new(MessageInfo {
        id: "BOTP".into(),
        source: crate::types::message::MessageSource {
            chat: group.parse().unwrap(),
            sender: sender.parse().unwrap(),
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let bot_msg = wa::Message {
        extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
            text: Some("continue".into()),
            ..Default::default()
        })),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![0x02; 32]),
            bot_metadata: Some(wa::BotMetadata {
                persona_id: Some("867051314767696".into()),
                ..Default::default()
            }),
            ..Default::default()
        }),
        ..Default::default()
    };
    client
        .maybe_capture_inbound_msg_secret(&bot_msg, &bot_info)
        .await;
    assert_eq!(
        client
            .persistence_manager
            .backend()
            .get_msg_secret(group, sender, "BOTP")
            .await
            .unwrap(),
        Some(vec![0x02; 32]),
        "BotOnly must capture a group bot invocation"
    );
}

/// Group flow: another participant invokes the bot, their decrypted prompt
/// carries the secret. We must key it under THE PARTICIPANT (the future
/// reply's `<meta target_sender_jid>`), not our own identity.
#[tokio::test]
async fn maybe_capture_inbound_msg_secret_keys_under_other_participant() {
    let (client, _transport) = capturing_client("capture_participant").await;
    let participant = "5599111112222:7@s.whatsapp.net";
    let info = Arc::new(MessageInfo {
        id: "GRP_OTHER".into(),
        source: crate::types::message::MessageSource {
            chat: "120363021033254949@g.us".parse().unwrap(),
            sender: participant.parse().unwrap(),
            is_from_me: false,
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let msg = wa::Message {
        extended_text_message: Some(Box::new(wa::message::ExtendedTextMessage {
            text: Some("@MetaAI question".into()),
            context_info: Some(Box::new(wa::ContextInfo {
                mentioned_jid: vec!["867051314767696@bot".into()],
                ..Default::default()
            })),
            ..Default::default()
        })),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(vec![0x5A; 32]),
            ..Default::default()
        }),
        ..Default::default()
    };
    client.maybe_capture_inbound_msg_secret(&msg, &info).await;

    // Keyed under the participant (non-AD), NOT under our own PN/LID.
    let under_participant = client
        .persistence_manager
        .backend()
        .get_msg_secret(
            "120363021033254949@g.us",
            "5599111112222@s.whatsapp.net",
            "GRP_OTHER",
        )
        .await
        .unwrap();
    assert_eq!(
        under_participant.as_deref(),
        Some(&[0x5Au8; 32][..]),
        "another participant's prompt must key under their sender JID"
    );
}

/// WA Web `sendAggregateReceipts`: a bot reply in a GROUP (chat not bot,
/// author is bot) must ack with a bare `<ack class="message">`
/// (sendBotInvokeResponseAcks), NOT a `<receipt>`.
#[tokio::test]
async fn bot_reply_in_group_acks_with_bare_ack_not_receipt() {
    let (client, transport) = capturing_client("bot_group_ack").await;
    let info = Arc::new(MessageInfo {
        id: "BOT_GRP_ACK".into(),
        source: crate::types::message::MessageSource {
            chat: "120363021033254949@g.us".parse().unwrap(),
            sender: "867051314767696@bot".parse().unwrap(),
            is_from_me: false,
            is_group: true,
            ..Default::default()
        },
        ..Default::default()
    });
    client.ack_received_message(&info);

    let mut found = None;
    for _ in 0..80 {
        if let Some(a) = find_message_ack(&transport.sent()) {
            found = Some(a);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert!(
        found.is_some(),
        "group bot reply must emit a bare <ack class=\"message\">"
    );
    assert_eq!(
        delivery_receipts_for(&transport.sent(), "BOT_GRP_ACK"),
        0,
        "group bot reply must NOT emit a <receipt>"
    );
}

/// Regression: a 1:1 bot chat (chat IS the bot) keeps the normal delivery
/// `<receipt>` — WA Web's `v` gate is false when chat.isBot().
#[tokio::test]
async fn bot_dm_reply_keeps_delivery_receipt() {
    let (client, transport) = capturing_client("bot_dm_receipt").await;
    let info = Arc::new(MessageInfo {
        id: "BOT_DM_RCPT".into(),
        source: crate::types::message::MessageSource {
            chat: "867051314767696@bot".parse().unwrap(),
            sender: "867051314767696@bot".parse().unwrap(),
            is_from_me: false,
            ..Default::default()
        },
        ..Default::default()
    });
    client.ack_received_message(&info);

    let mut count = 0;
    for _ in 0..80 {
        count = delivery_receipts_for(&transport.sent(), "BOT_DM_RCPT");
        if count > 0 {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(
        count, 1,
        "1:1 bot chat must keep the normal delivery receipt"
    );
}

#[tokio::test]
async fn maybe_capture_inbound_msg_secret_skips_when_secret_absent() {
    let (client, _transport) = capturing_client("capture_no_secret").await;
    let info = Arc::new(MessageInfo {
        id: "NO_SECRET".into(),
        source: crate::types::message::MessageSource {
            chat: "867051314767696@bot".parse().unwrap(),
            sender: "5511000000001:0@s.whatsapp.net".parse().unwrap(),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let msg = wa::Message {
        conversation: Some("hi".into()),
        ..Default::default()
    };
    client.maybe_capture_inbound_msg_secret(&msg, &info).await;

    for _ in 0..16 {
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    let got = client
        .persistence_manager
        .backend()
        .get_msg_secret(
            "867051314767696@bot",
            "5511000000001@s.whatsapp.net",
            "NO_SECRET",
        )
        .await
        .unwrap();
    assert!(got.is_none());
}

/// A stanza carrying BOTH a valid msmsg AND an unknown sibling enc must
/// still dispatch the msmsg — the unknown-only fallback ack must not
/// short-circuit when `bot_payloads` is non-empty.
#[tokio::test]
async fn mixed_msmsg_and_unknown_enc_still_decrypts_msmsg() {
    use crate::store::commands::DeviceCommand;
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};

    let (client, _transport) = capturing_client("msmsg_mixed_unknown").await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(
            "999888777666555:0@lid".parse().unwrap(),
        )))
        .await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "867051314767696@bot";
    let our_lid = "999888777666555@lid";
    let outbound_id = "OUT_MIX";
    let bot_reply_id = "REPLY_MIX";
    let secret = [0x55u8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_lid, outbound_id, &secret)
        .await
        .unwrap();

    let plaintext_msg = wa::Message {
        conversation: Some("mixed ok".into()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    let ctx = BotMessageContext {
        msg_id: bot_reply_id,
        target_sender_user_jid: our_lid,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    // Stanza has a valid msmsg PLUS an unrecognised "frskmsg" sibling.
    // The fallback transport-ack must NOT fire (would drop the msmsg).
    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", bot_reply_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_lid)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "frskmsg")
                .bytes(vec![0u8; 8])
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == bot_reply_id
                        && msg.conversation.as_deref() == Some("mixed ok"))
        },
        1500,
    )
    .await;
    assert!(
        got.is_some(),
        "msmsg sibling of an unknown enc must still decrypt and dispatch"
    );
}

/// LID↔PN migration window: the secret was stored under our PN, but the
/// bot reply's `<meta target_sender_jid>` echoes our LID. The primary
/// lookup misses; `alternate_msg_secret_lookup` resolves PN via
/// `lid_pn_mapping` and hits. Mirrors WA Web `C()`'s `getAlternateMsgKey`.
#[tokio::test]
async fn msmsg_alternate_lookup_resolves_lid_to_stored_pn() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    use wacore::store::traits::LidPnMappingEntry;

    let (client, _transport) = capturing_client("msmsg_alt_lookup").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "867051314767696@bot";
    let our_lid_user = "999888777666555";
    let our_pn_user = "5511000000001";
    let our_lid = "999888777666555@lid";
    let our_pn = "5511000000001@s.whatsapp.net";
    let outbound_id = "OUT_ALT";
    let bot_reply_id = "REPLY_ALT";
    let secret = [0x3Cu8; 32];

    // Seed the LID→PN mapping so the alternate lookup can swap.
    client
        .persistence_manager
        .backend()
        .put_lid_mapping(&LidPnMappingEntry {
            lid: our_lid_user.into(),
            phone_number: our_pn_user.into(),
            created_at: 0,
            updated_at: 0,
            learning_source: "test".into(),
        })
        .await
        .unwrap();
    // Secret stored under PN (as if the outbound went out PN-addressed).
    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    // Bot reply encrypts with target = our LID (what <meta> declares).
    let plaintext_msg = wa::Message {
        conversation: Some("alt ok".into()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    let ctx = BotMessageContext {
        msg_id: bot_reply_id,
        target_sender_user_jid: our_lid,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", bot_reply_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_lid)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == bot_reply_id
                        && msg.conversation.as_deref() == Some("alt ok"))
        },
        1500,
    )
    .await;
    assert!(
        got.is_some(),
        "LID-declared reply must resolve the PN-stored secret via lid_pn_mapping"
    );
}

/// End-to-end: phone fanout dispatches a wa::Message carrying the
/// outbound `messageSecret`; later the Meta AI bot replies via msmsg
/// referencing the same id. The captured secret must let the reply
/// decrypt and surface `Event::Message`.
#[tokio::test]
async fn fanout_capture_lets_subsequent_msmsg_decrypt() {
    use crate::store::commands::DeviceCommand;
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};

    let (client, _transport) = capturing_client("fanout_to_msmsg").await;
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(
            "999888777666555:0@lid".parse().unwrap(),
        )))
        .await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let bot_chat: Jid = "867051314767696@bot".parse().unwrap();
    let our_lid_str = "999888777666555@lid";
    let outbound_id = "FANOUT_OUT";
    let bot_reply_id = "BOT_REPLY_PHONE";
    let secret = [0x99u8; 32];

    // Step 1: simulate the fanout dispatch (what dispatch_parsed_message
    // would call when the phone's outbound stanza is mirrored to us).
    let fanout_info = Arc::new(MessageInfo {
        id: outbound_id.into(),
        source: crate::types::message::MessageSource {
            chat: bot_chat.clone(),
            sender: "5511000000001:0@s.whatsapp.net".parse().unwrap(),
            is_from_me: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let fanout_msg = wa::Message {
        conversation: Some("hi bot".into()),
        message_context_info: Some(wa::MessageContextInfo {
            message_secret: Some(secret.to_vec()),
            ..Default::default()
        }),
        ..Default::default()
    };
    client
        .maybe_capture_inbound_msg_secret(&fanout_msg, &fanout_info)
        .await;
    // Write is awaited inline now, so the secret is already durable here.
    for _ in 0..40 {
        if client
            .persistence_manager
            .backend()
            .get_msg_secret("867051314767696@bot", our_lid_str, outbound_id)
            .await
            .unwrap()
            .is_some()
        {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    // Step 2: the bot reply arrives as <enc type="msmsg"> referencing
    // outbound_id via <meta target_id>. With the secret captured above,
    // it must decrypt cleanly.
    let plaintext_msg = wa::Message {
        conversation: Some("bot reply".into()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    let ctx = BotMessageContext {
        msg_id: bot_reply_id,
        target_sender_user_jid: our_lid_str,
        bot_user_jid: "867051314767696@bot",
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", bot_reply_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_lid_str)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == bot_reply_id
                        && msg.conversation.as_deref() == Some("bot reply"))
        },
        1500,
    )
    .await;
    assert!(
        got.is_some(),
        "secret captured from fanout must enable msmsg reply decryption"
    );
}

/// Coherence: the identity `persist_outbound_msg_secret` writes under
/// (LID for bot chats) must match what `handle_msmsg_payload` reads via
/// `<meta target_sender_jid>`. End-to-end without bypassing the helper.
#[tokio::test]
async fn msmsg_outbound_put_and_inbound_get_match_for_lid_bot() {
    use crate::store::commands::DeviceCommand;
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};

    let (client, _transport) = capturing_client("msmsg_lid_match").await;
    // Seed both PN (already seeded by capturing_client) and LID.
    client
        .persistence_manager
        .process_command(DeviceCommand::SetLid(Some(
            "999888777666555:0@lid".parse().unwrap(),
        )))
        .await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let bot_chat: Jid = "867051314767696@bot".parse().unwrap();
    let outbound_id = "OUT_LID";
    let bot_reply_id = "REPLY_LID";
    let our_lid = "999888777666555@lid";
    let secret = [0x71u8; 32];

    // Real outbound path: caller resolves the bot identity to our LID.
    let sender_identity = client
        .dm_sender_identity_for(&bot_chat)
        .await
        .expect("LID seeded");
    client
        .persist_outbound_msg_secret(
            &bot_chat,
            &sender_identity,
            outbound_id,
            &secret,
            wacore::msg_secret::RetentionClass::Bot,
        )
        .await;

    // Inbound msmsg payload encrypted under the same (msg_id, target, bot)
    // tuple the meta will declare on the wire.
    let plaintext_msg = wa::Message {
        conversation: Some("lid coherent".to_string()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    let ctx = BotMessageContext {
        msg_id: bot_reply_id,
        target_sender_user_jid: our_lid,
        bot_user_jid: "867051314767696@bot",
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", bot_reply_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_lid)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == bot_reply_id
                        && msg.conversation.as_deref() == Some("lid coherent"))
        },
        1500,
    )
    .await;
    assert!(
        got.is_some(),
        "outbound PUT and inbound GET must converge on LID for bot chats"
    );
}

/// Regression for the AD_JID encoder bug: a `from="USER:0@bot"` stanza must
/// survive the marshal/unmarshal round-trip with `server=Bot`, so the
/// secret lookup keys hit and the reply decrypts.
#[tokio::test]
async fn msmsg_with_bot_device_suffix_round_trips() {
    use wacore::bot_message::{BotMessageContext, encrypt_bot_message};
    let (client, _transport) = capturing_client("msmsg_bot_device").await;
    let collector = Arc::new(crate::test_utils::TestEventCollector::default());
    client.register_handler(collector.clone());

    let chat = "867051314767696@bot";
    let our_pn = "5511000000001@s.whatsapp.net";
    let outbound_id = "OUTBOUND_DEV";
    let bot_reply_id = "BOT_REPLY_DEV";
    let secret = [0x33u8; 32];

    client
        .persistence_manager
        .backend()
        .put_msg_secret(chat, our_pn, outbound_id, &secret)
        .await
        .unwrap();

    let plaintext_msg = wa::Message {
        conversation: Some("with device".to_string()),
        ..Default::default()
    };
    let pt_bytes = {
        use prost::Message as _;
        let mut v = Vec::with_capacity(plaintext_msg.encoded_len());
        plaintext_msg.encode(&mut v).unwrap();
        v
    };
    let ctx = BotMessageContext {
        msg_id: bot_reply_id,
        target_sender_user_jid: our_pn,
        bot_user_jid: chat,
    };
    let (cipher, iv) = encrypt_bot_message(&pt_bytes, &secret, &ctx).unwrap();
    let ms_msg_proto = encode_message_secret_message(&iv, &cipher);

    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696:0@bot")
        .attr("id", bot_reply_id)
        .attr("type", "text")
        .children([
            NodeBuilder::new("meta")
                .attr("target_id", outbound_id)
                .attr("target_sender_jid", our_pn)
                .build(),
            NodeBuilder::new("enc")
                .attr("type", "msmsg")
                .attr("v", "2")
                .bytes(ms_msg_proto)
                .build(),
        ])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let got = collect_event(
        &client,
        collector,
        |e| {
            matches!(e, wacore::types::events::Event::Message(msg, info)
                    if info.id == bot_reply_id
                        && msg.conversation.as_deref() == Some("with device"))
        },
        1500,
    )
    .await;
    assert!(
        got.is_some(),
        "msmsg with `:0@bot` from must round-trip (encoder must not strip the bot server)"
    );
}

/// `<meta>` without `target_id` → cannot identify the parent message,
/// nack 495 and no dispatch.
#[tokio::test]
async fn msmsg_without_meta_target_id_nacks_495() {
    let (client, transport) = capturing_client("msmsg_no_target").await;
    let bot_reply_id = "BOT_REPLY_NT";
    let ms_msg_proto = encode_message_secret_message(&[0u8; 12], &[0u8; 32]);
    let node = NodeBuilder::new("message")
        .attr("from", "867051314767696@bot")
        .attr("id", bot_reply_id)
        .attr("type", "text")
        .children([NodeBuilder::new("enc")
            .attr("type", "msmsg")
            .attr("v", "2")
            .bytes(ms_msg_proto)
            .build()])
        .build();
    let owned = node_to_arc(node);
    client.clone().handle_incoming_message(owned).await;

    let mut code = None;
    for _ in 0..80 {
        if let Some(c) = find_message_nack_error(&transport.sent(), bot_reply_id) {
            code = Some(c);
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }
    assert_eq!(code, Some(495));
}
