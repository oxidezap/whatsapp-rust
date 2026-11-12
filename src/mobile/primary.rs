//! Primary companion hosting through the existing IQ, Signal and persistence paths.
use crate::Client;
use rand::{CryptoRng, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::io::Write as _;
use subtle::ConstantTimeEq;
use wacore::mobile::primary::*;
use wacore::store::MobileSessionBinding;
use wacore::types::jid::JidExt as _;
use wacore_binary::{Jid, JidExt as _};
use waproto::protobuf::{Enumeration as _, Message as _};
use waproto::whatsapp as wa;
use zeroize::Zeroizing;

const MAX_JOURNAL_BYTES: usize = 64 * 1024;
const MAX_TRACKED_COMPANIONS: usize = 128;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PrimaryManagementError {
    #[error("invalid primary operation: {0}")]
    Invalid(&'static str),
    #[error("{0}")]
    Crypto(#[from] PrimaryLinkError),
    #[error("{0}")]
    Store(#[from] crate::store::error::StoreError),
    #[error("primary protocol operation failed")]
    Protocol(#[source] anyhow::Error),
    #[error("invalid primary journal")]
    Journal(#[from] serde_json::Error),
}

impl PrimaryManagementError {
    fn protocol(error: impl Into<anyhow::Error>) -> Self {
        Self::Protocol(error.into())
    }
}

/// Explicit host entropy and clock input for first app-state key initialization.
/// This secret is deliberately absent from Debug and the primary link journal.
#[derive(Clone)]
pub struct PrimaryAppStateKeyInput {
    key_id: [u8; 6],
    key_data: Zeroizing<[u8; 32]>,
    raw_id: u32,
    timestamp_millis: i64,
}
impl PrimaryAppStateKeyInput {
    pub fn new(
        key_id: [u8; 6],
        key_data: [u8; 32],
        raw_id: u32,
        timestamp_millis: i64,
    ) -> Result<Self, PrimaryManagementError> {
        let input = Self {
            key_id,
            key_data: Zeroizing::new(key_data),
            raw_id,
            timestamp_millis,
        };
        input.validate()?;
        Ok(input)
    }
    pub fn validate(&self) -> Result<(), PrimaryManagementError> {
        if self.key_id[..2] != [0, 0]
            || self.key_id[2..] == [0; 4]
            || *self.key_data == [0; 32]
            || self.timestamp_millis < 0
        {
            return Err(PrimaryManagementError::Invalid("initial app-state key"));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum PrimaryOperationStage {
    Prepared,
    UploadAttempted,
    Acknowledged,
    Verified,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PendingOperation {
    reference: Vec<u8>,
    peer_identity: [u8; 32],
    noise_public: [u8; 32],
    issued_identity: Vec<u8>,
    signed_indexes: Vec<u8>,
    key_index: u32,
    timestamp_seconds: u64,
    device_jid: Option<Jid>,
    stage: PrimaryOperationStage,
}
/// An abandoned approval remains recoverable; a delayed server result can still land.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AbandonedApproval {
    pending: PendingOperation,
}
impl AbandonedApproval {
    pub fn key_index(&self) -> u32 {
        self.pending.key_index
    }
    pub fn peer_identity(&self) -> &[u8; 32] {
        &self.pending.peer_identity
    }
    pub fn reference(&self) -> &[u8] {
        &self.pending.reference
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ApprovalObservation {
    NotAdvertised,
    IndexAdvertisedWithoutDevice,
    DeviceAdvertised,
}
/// Observation describes two completed snapshots, never a proof of permanent non-delivery.
pub struct ApprovalSettlement {
    key_index: u32,
    observation: ApprovalObservation,
}
impl ApprovalSettlement {
    pub fn key_index(&self) -> u32 {
        self.key_index
    }
    pub fn observation(&self) -> ApprovalObservation {
        self.observation
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostedCompanion {
    device_jid: Jid,
    peer_identity: [u8; 32],
    signed_identity: Vec<u8>,
    key_index: u32,
    history_sent: bool,
    app_state_sent: bool,
    history_cursors: Vec<HistoryCursor>,
}
impl HostedCompanion {
    pub fn device_jid(&self) -> &Jid {
        &self.device_jid
    }
    pub fn key_index(&self) -> u32 {
        self.key_index
    }
    /// Both bootstrap messages were dispatched and checkpointed locally.
    /// This does not claim peer application or full account-history completion.
    pub fn bootstrap_dispatch_complete(&self) -> bool {
        self.history_sent && self.app_state_sent
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct HistoryCursor {
    sync_type: i32,
    chunk_order: u32,
    progress: u32,
    file_sha256: [u8; 32],
}
/// Chunk metadata supplied by the host's actual history export cursor. Progress
/// is never replaced with an invented full-history completion percentage.
pub struct PrimaryHistoryChunk {
    pub sync_type: wa::message::HistorySyncType,
    pub chunk_order: u32,
    pub progress: u32,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Maintenance {
    Publish {
        signed: Vec<u8>,
        timestamp: u64,
    },
    Revoke {
        device: Jid,
        key_index: u32,
        reason: String,
        signed: Vec<u8>,
        timestamp: u64,
        removed: bool,
    },
    RevokeAll {
        reason: String,
        signed: Vec<u8>,
        timestamp: u64,
        removed: bool,
    },
    Bootstrap {
        device: Jid,
        peer_identity: [u8; 32],
        key_index: u32,
        payload_hash: [u8; 32],
        media_key_fingerprint: Option<[u8; 32]>,
        history_message_id: String,
        app_state_message_id: String,
        history_message_hash: Option<[u8; 32]>,
        app_state_message_hash: Option<[u8; 32]>,
        app_state_key_id: Option<Vec<u8>>,
    },
    History {
        device: Jid,
        peer_identity: [u8; 32],
        key_index: u32,
        cursor: HistoryCursor,
        message_id: String,
        message_hash: [u8; 32],
    },
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PrimaryJournal {
    schema: u32,
    raw_id: u32,
    high_water: u32,
    key_index_timestamp: u64,
    live_indexes: Vec<u32>,
    explicit_defaults: bool,
    pending: Option<PendingOperation>,
    maintenance: Option<Maintenance>,
    companions: Vec<HostedCompanion>,
    #[serde(default)]
    abandoned: Vec<AbandonedApproval>,
    #[serde(default)]
    abandoned_dispatches: Vec<Maintenance>,
}
impl PrimaryJournal {
    fn validate(&self) -> Result<(), PrimaryManagementError> {
        if self.schema != 1
            || self.companions.len() > MAX_TRACKED_COMPANIONS
            || self.abandoned.len() > MAX_TRACKED_COMPANIONS
            || self.abandoned_dispatches.len() > MAX_TRACKED_COMPANIONS
            || self.live_indexes.is_empty()
            || self.live_indexes.len() > 4096
            || !self.live_indexes.contains(&0)
            || self.live_indexes.iter().any(|v| *v > self.high_water)
        {
            return Err(PrimaryManagementError::Invalid(
                "journal schema or capacity",
            ));
        }
        match &self.maintenance {
            Some(Maintenance::Bootstrap {
                history_message_id,
                app_state_message_id,
                app_state_key_id,
                ..
            }) if !valid_message_id(history_message_id)
                || !valid_message_id(app_state_message_id)
                || history_message_id == app_state_message_id
                || app_state_key_id.as_ref().is_some_and(|id| id.len() != 6) =>
            {
                return Err(PrimaryManagementError::Invalid("bootstrap message journal"));
            }
            Some(Maintenance::History { message_id, .. }) if !valid_message_id(message_id) => {
                return Err(PrimaryManagementError::Invalid("history message journal"));
            }
            _ => {}
        }
        let mut indexes = std::collections::BTreeSet::new();
        let mut devices = std::collections::BTreeSet::new();
        for abandoned in &self.abandoned {
            let pending = &abandoned.pending;
            if pending.key_index == 0
                || pending.key_index > self.high_water
                || pending.reference.is_empty()
                || pending.reference.len() > 4096
                || pending.issued_identity.is_empty()
                || pending.issued_identity.len() > 16384
                || pending.signed_indexes.is_empty()
                || pending.signed_indexes.len() > 16384
            {
                return Err(PrimaryManagementError::Invalid("abandoned approval record"));
            }
        }
        for companion in &self.companions {
            if companion.key_index == 0
                || companion.key_index > self.high_water
                || !indexes.insert(companion.key_index)
                || !devices.insert(companion.device_jid.to_string())
                || companion.signed_identity.is_empty()
                || companion.signed_identity.len() > 16384
            {
                return Err(PrimaryManagementError::Invalid("companion journal record"));
            }
        }
        if let Some(pending) = &self.pending
            && (pending.reference.is_empty()
                || pending.reference.len() > 4096
                || pending.key_index == 0
                || pending.key_index > self.high_water
                || pending.issued_identity.is_empty()
                || pending.issued_identity.len() > 16384
                || pending.signed_indexes.is_empty()
                || pending.signed_indexes.len() > 16384
                || (matches!(
                    pending.stage,
                    PrimaryOperationStage::Acknowledged | PrimaryOperationStage::Verified
                ) && pending.device_jid.is_none()))
        {
            return Err(PrimaryManagementError::Invalid("pending journal record"));
        }
        Ok(())
    }
    // A proof remains bound to its original PN/LID address and key index. A
    // fresh authenticated alias of that same device slot may retain it, but a
    // removed slot or a replacement key must never revive its old index.
    fn retain_observed_companions(&mut self, observed: &PrimaryDeviceSnapshot, accounts: &[Jid]) {
        self.companions.retain(|record| {
            observed.devices.iter().any(|(jid, index)| {
                same_owned_device_slot(jid, &record.device_jid, accounts)
                    && *index == record.key_index
            })
        });
    }
    fn apply_observed(&mut self, observed: &PrimaryDeviceSnapshot) {
        self.live_indexes = observed.live_indexes.clone();
        self.live_indexes.extend(
            self.abandoned
                .iter()
                .filter(|record| {
                    observed
                        .advertised_indexes
                        .contains(&record.pending.key_index)
                })
                .map(|record| record.pending.key_index),
        );
        self.live_indexes.sort_unstable();
        self.live_indexes.dedup();
        self.high_water = self.high_water.max(observed.high_water);
        self.key_index_timestamp = self.key_index_timestamp.max(observed.key_index_timestamp);
    }
    fn encode(&self) -> Result<Vec<u8>, PrimaryManagementError> {
        self.validate()?;
        let encoded = serde_json::to_vec(self)?;
        if encoded.len() > MAX_JOURNAL_BYTES {
            return Err(PrimaryManagementError::Invalid("journal capacity"));
        }
        Ok(encoded)
    }
}

/// One in-memory, account-bound code exchange. It cannot be serialized or logged.
pub struct PrimaryCodeExchange {
    hello: PrimaryHello,
    reference: String,
    companion_noise: [u8; 32],
    platform: String,
    identity_hash: [u8; 32],
}
impl PrimaryCodeExchange {
    pub fn reference(&self) -> &str {
        &self.reference
    }
}
struct PrimaryDeviceSnapshot {
    advertised_indexes: Vec<u32>,
    devices: Vec<(Jid, u32)>,
    live_indexes: Vec<u32>,
    high_water: u32,
    key_index_timestamp: u64,
}

/// An explicitly prepared external-history descriptor. It contains a media key;
/// hosts retain it in their protected storage before dispatch and restore the
/// exact descriptor after cancellation. No descriptor is silently re-uploaded.
pub struct PreparedBootstrapHistory {
    upload: crate::upload::UploadResponse,
    payload_hash: [u8; 32],
}
impl PreparedBootstrapHistory {
    pub fn from_uploaded(
        history: &wa::HistorySync,
        upload: crate::upload::UploadResponse,
    ) -> Result<Self, PrimaryManagementError> {
        validate_initial_history(history)?;
        let bytes = bounded_history_bytes(history, 16 * 1024 * 1024)?;
        let payload_hash = Sha256::digest(&bytes).into();
        let compressed = compress_history(&bytes)?;
        if upload.media_key == [0; 32]
            || upload.direct_path.is_empty()
            || upload.direct_path.len() > 4096
            || upload.file_length != compressed.len() as u64
            || upload.file_sha256 != <[u8; 32]>::from(Sha256::digest(&compressed))
        {
            return Err(PrimaryManagementError::Invalid(
                "external history descriptor does not match payload",
            ));
        }
        Ok(Self {
            upload,
            payload_hash,
        })
    }
    pub fn upload_descriptor(&self) -> &crate::upload::UploadResponse {
        &self.upload
    }
}

pub struct MobilePrimaryApi<'a> {
    client: &'a Client,
}
impl Client {
    pub fn mobile_primary(&self) -> Result<MobilePrimaryApi<'_>, PrimaryManagementError> {
        let session = self
            .mobile_session
            .as_ref()
            .ok_or(PrimaryManagementError::Invalid(
                "mobile-primary session required",
            ))?;
        session
            .validate_device(&self.persistence_manager.get_device_snapshot())
            .map_err(|_| PrimaryManagementError::Invalid("registered mobile identity"))?;
        Ok(MobilePrimaryApi { client: self })
    }
    /// Preserve a usable persisted key; require explicit host material only on an empty store.
    pub async fn ensure_primary_app_state_key(
        &self,
        input: Option<&PrimaryAppStateKeyInput>,
    ) -> Result<Vec<u8>, PrimaryManagementError> {
        self.mobile_primary()?;
        let snapshot = self.persistence_manager.get_device_snapshot();
        let backend = snapshot.backend.clone();
        if let Some(id) = backend.get_latest_sync_key_id().await? {
            let key = backend
                .get_sync_key(&id)
                .await?
                .ok_or(PrimaryManagementError::Invalid(
                    "missing latest app-state key",
                ))?;
            validate_app_state_key(&id, &key)?;
            return Ok(id);
        }
        let input = input.ok_or(PrimaryManagementError::Invalid(
            "initial app-state key material required",
        ))?;
        input.validate()?;
        let mut fingerprint = wa::message::AppStateSyncKeyFingerprint::default();
        fingerprint.raw_id = Some(input.raw_id);
        fingerprint.current_index = Some(0);
        fingerprint.device_indexes = vec![0];
        let selected = self
            .persistence_manager
            .initialize_mobile_app_state_key(
                input.key_id.to_vec(),
                wacore::store::traits::AppStateSyncKey {
                    key_data: input.key_data.to_vec(),
                    fingerprint: waproto::codec::app_state_sync_key_fingerprint_to_vec(
                        &fingerprint,
                    ),
                    timestamp: input.timestamp_millis,
                },
            )
            .await?;
        let winner =
            backend
                .get_sync_key(&selected)
                .await?
                .ok_or(PrimaryManagementError::Invalid(
                    "missing initialized app-state key",
                ))?;
        validate_app_state_key(&selected, &winner)?;
        Ok(selected)
    }
}
impl MobilePrimaryApi<'_> {
    fn state(
        &self,
    ) -> Result<(MobileSessionBinding, PrimaryJournal, Vec<Jid>), PrimaryManagementError> {
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let binding = snapshot
            .mobile_session
            .as_deref()
            .cloned()
            .ok_or(PrimaryManagementError::Invalid("mobile binding"))?;
        let bytes = binding
            .primary_state
            .as_deref()
            .ok_or(PrimaryManagementError::Invalid(
                "primary host not initialized",
            ))?;
        if bytes.len() > MAX_JOURNAL_BYTES {
            return Err(PrimaryManagementError::Invalid("journal capacity"));
        }
        let journal: PrimaryJournal = serde_json::from_slice(bytes)?;
        journal.validate()?;
        let mut accounts = vec![
            snapshot
                .pn
                .clone()
                .ok_or(PrimaryManagementError::Invalid("primary account"))?,
        ];
        if let Some(lid) = &snapshot.lid {
            accounts.push(lid.clone());
        }
        for companion in &journal.companions {
            validate_owned(&companion.device_jid, &accounts)?;
        }
        if let Some(device) = journal.pending.as_ref().and_then(|v| v.device_jid.as_ref()) {
            validate_owned(device, &accounts)?;
        }
        Ok((binding, journal, accounts))
    }
    async fn save(
        &self,
        expected: MobileSessionBinding,
        journal: &PrimaryJournal,
    ) -> Result<(), PrimaryManagementError> {
        self.client
            .persistence_manager
            .update_mobile_primary_state(expected, journal.encode()?)
            .await?;
        Ok(())
    }
    pub async fn initialize(
        &self,
        raw_id: u32,
        encoding: AdvDefaultEncoding,
    ) -> Result<(), PrimaryManagementError> {
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let binding = snapshot
            .mobile_session
            .as_deref()
            .cloned()
            .ok_or(PrimaryManagementError::Invalid("mobile binding"))?;
        if binding.primary_state.is_some() {
            return Err(PrimaryManagementError::Invalid(
                "primary host already initialized",
            ));
        }
        let journal = PrimaryJournal {
            schema: 1,
            raw_id,
            high_water: 0,
            key_index_timestamp: 0,
            live_indexes: vec![0],
            explicit_defaults: encoding == AdvDefaultEncoding::Explicit,
            pending: None,
            maintenance: None,
            companions: Vec::new(),
            abandoned: Vec::new(),
            abandoned_dispatches: Vec::new(),
        };
        self.save(binding, &journal).await
    }
    pub fn companions(&self) -> Result<Vec<HostedCompanion>, PrimaryManagementError> {
        Ok(self.state()?.1.companions)
    }
    pub fn pending_stage(&self) -> Result<Option<PrimaryOperationStage>, PrimaryManagementError> {
        Ok(self.state()?.1.pending.map(|v| v.stage))
    }
    /// Burn a unique key index durably before network upload. Concurrent callers lose CAS.
    pub async fn prepare_qr<R: Rng + CryptoRng>(
        &self,
        qr: &CompanionQr,
        timestamp_seconds: u64,
        live_indexes: &[u32],
        rng: &mut R,
    ) -> Result<u32, PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let observed = self.query_current_devices().await?;
        let (binding, mut journal, accounts) = self.state()?;
        if journal.pending.is_some() || journal.maintenance.is_some() {
            return Err(PrimaryManagementError::Invalid(
                "pending operation or capacity",
            ));
        }
        if timestamp_seconds < observed.key_index_timestamp {
            return Err(PrimaryManagementError::Invalid(
                "key-index timestamp regressed",
            ));
        }
        journal.retain_observed_companions(&observed, &accounts);
        if journal.companions.len() >= MAX_TRACKED_COMPANIONS {
            return Err(PrimaryManagementError::Invalid(
                "pending operation or capacity",
            ));
        }
        journal.apply_observed(&observed);
        journal.key_index_timestamp = timestamp_seconds;
        let mut indexes: Vec<u32> = journal
            .live_indexes
            .iter()
            .copied()
            .chain(live_indexes.iter().copied())
            .chain([0])
            .collect();
        indexes.sort_unstable();
        indexes.dedup();
        let index = next_companion_key_index(journal.high_water, &indexes)?;
        indexes.push(index);
        let params = IdentityParameters {
            raw_id: journal.raw_id,
            timestamp_seconds,
            key_index: index,
            valid_indexes: &indexes,
            default_encoding: if journal.explicit_defaults {
                AdvDefaultEncoding::Explicit
            } else {
                AdvDefaultEncoding::Omit
            },
        };
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let issued = build_signed_companion_identity(
            &snapshot.identity_key,
            qr.identity_public_key(),
            qr.adv_secret_key(),
            &params,
            rng,
        )?;
        journal.high_water = index;
        journal.pending = Some(PendingOperation {
            reference: qr.reference().as_bytes().to_vec(),
            peer_identity: *qr.identity_public_key(),
            noise_public: *qr.noise_public_key(),
            issued_identity: issued.device_identity,
            signed_indexes: issued.key_index_list,
            key_index: index,
            timestamp_seconds,
            device_jid: None,
            stage: PrimaryOperationStage::Prepared,
        });
        self.save(binding, &journal).await?;
        Ok(index)
    }
    /// Bind the primary code hello to one companion invitation and account.
    pub async fn send_code_hello(
        &self,
        server: &Jid,
        reference: &str,
        companion_noise: &[u8; 32],
        platform: &str,
        hello: PrimaryHello,
    ) -> Result<PrimaryCodeExchange, PrimaryManagementError> {
        if reference.is_empty()
            || reference.len() > 4096
            || platform.is_empty()
            || platform.len() > 64
            || reference.chars().any(char::is_control)
            || platform.chars().any(char::is_control)
            || *companion_noise == [0; 32]
        {
            return Err(PrimaryManagementError::Invalid("code invitation"));
        }
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let identity = snapshot.identity_key.public_key.public_key_bytes();
        let identity_hash = snapshot
            .mobile_session
            .as_ref()
            .ok_or(PrimaryManagementError::Invalid("mobile binding"))?
            .identity_hash;
        self.client
            .execute(PrimaryHelloSpec::new(
                server,
                reference.as_bytes(),
                hello.wrapped_ephemeral(),
                identity,
            )?)
            .await
            .map_err(PrimaryManagementError::protocol)?;
        Ok(PrimaryCodeExchange {
            hello,
            reference: reference.to_owned(),
            companion_noise: *companion_noise,
            platform: platform.to_owned(),
            identity_hash,
        })
    }
    /// Authenticate the finish reference before deriving its ADV secret and reserving
    /// the durable approval. A finish from another invitation cannot advance it.
    #[allow(clippy::too_many_arguments)]
    pub async fn prepare_code_finish<R: Rng + CryptoRng>(
        &self,
        exchange: &PrimaryCodeExchange,
        notification_reference: &[u8],
        companion_identity: &[u8; 32],
        wrapped_bundle: &[u8],
        timestamp_seconds: u64,
        live_indexes: &[u32],
        rng: &mut R,
    ) -> Result<u32, PrimaryManagementError> {
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        if !bool::from(exchange.reference.as_bytes().ct_eq(notification_reference))
            || snapshot
                .mobile_session
                .as_ref()
                .is_none_or(|v| v.identity_hash != exchange.identity_hash)
        {
            return Err(PrimaryManagementError::Invalid(
                "code exchange reference or account",
            ));
        }
        let secret =
            exchange
                .hello
                .complete(wrapped_bundle, companion_identity, &snapshot.identity_key)?;
        let invitation = CompanionQr::from_parts(
            &exchange.reference,
            exchange.companion_noise,
            *companion_identity,
            *secret,
            &exchange.platform,
        )?;
        self.prepare_qr(&invitation, timestamp_seconds, live_indexes, rng)
            .await
    }
    /// An ambiguous upload remains journaled. This method never automatically resends it.
    pub async fn upload_prepared(
        &self,
        server: &Jid,
        client_props: Option<wa::ClientPairingProps>,
    ) -> Result<Jid, PrimaryManagementError> {
        self.upload_prepared_with_pem(server, client_props, None)
            .await
    }
    pub async fn upload_prepared_with_pem(
        &self,
        server: &Jid,
        client_props: Option<wa::ClientPairingProps>,
        pem: Option<PairDevicePem>,
    ) -> Result<Jid, PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        self.client.persistence_manager.flush().await?;
        let (binding, mut journal, accounts) = self.state()?;
        let pending = journal
            .pending
            .as_mut()
            .ok_or(PrimaryManagementError::Invalid("no pending operation"))?;
        if pending.stage != PrimaryOperationStage::Prepared {
            return Err(PrimaryManagementError::Invalid(
                "upload already attempted; reconcile first",
            ));
        }
        let spec = PairDeviceSpec::new(
            server,
            &accounts,
            &pending.reference,
            &pending.noise_public,
            SignedCompanionIdentity {
                device_identity: pending.issued_identity.clone(),
                key_index_list: pending.signed_indexes.clone(),
            },
            pending.timestamp_seconds,
            client_props,
        )?;
        let spec = if let Some(pem) = pem {
            spec.with_pem(pem)
        } else {
            spec
        };
        pending.stage = PrimaryOperationStage::UploadAttempted;
        self.save(binding, &journal).await?;
        let acknowledged = self
            .client
            .execute(spec)
            .await
            .map_err(PrimaryManagementError::protocol)?;
        let (binding, mut journal, _) = self.state()?;
        let pending = journal
            .pending
            .as_mut()
            .ok_or(PrimaryManagementError::Invalid("pending operation changed"))?;
        if pending.stage != PrimaryOperationStage::UploadAttempted {
            return Err(PrimaryManagementError::Invalid("pending operation changed"));
        }
        pending.device_jid = Some(acknowledged.device_jid.clone());
        pending.stage = PrimaryOperationStage::Acknowledged;
        self.save(binding, &journal).await?;
        Ok(acknowledged.device_jid)
    }
    /// Recover an acknowledgement from an independently queried current device list.
    /// The caller must supply the issued reference and cryptographic companion proof.
    pub async fn confirm(
        &self,
        reference: &[u8],
        device: &Jid,
        proof: &[u8],
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let observed = self.query_current_devices().await?;
        let (binding, mut journal, accounts) = self.state()?;
        let pending = journal
            .pending
            .as_mut()
            .ok_or(PrimaryManagementError::Invalid("no pending operation"))?;
        if !matches!(
            pending.stage,
            PrimaryOperationStage::Acknowledged | PrimaryOperationStage::UploadAttempted
        ) {
            return Err(PrimaryManagementError::Invalid("upload not attempted"));
        }
        if pending.device_jid.as_ref().is_some_and(|jid| jid != device) {
            return Err(PrimaryManagementError::Invalid(
                "acknowledged device mismatch",
            ));
        }
        if !observed
            .devices
            .iter()
            .any(|(jid, index)| jid == device && *index == pending.key_index)
        {
            return Err(PrimaryManagementError::Invalid(
                "companion absent from authenticated device snapshot",
            ));
        }
        let verified = PendingCompanion::new(
            &pending.reference,
            &accounts,
            &pending.peer_identity,
            SignedCompanionIdentity {
                device_identity: pending.issued_identity.clone(),
                key_index_list: pending.signed_indexes.clone(),
            },
        )?
        .acknowledge(device)?
        .verify(reference, device, proof)?;
        journal.companions.push(HostedCompanion {
            device_jid: verified.device_jid().clone(),
            peer_identity: *verified.peer_identity(),
            signed_identity: verified.signed_identity().to_vec(),
            key_index: pending.key_index,
            history_sent: false,
            app_state_sent: false,
            history_cursors: Vec::new(),
        });
        journal.apply_observed(&observed);
        journal.high_water = journal.high_water.max(observed.high_water);
        journal.key_index_timestamp = journal
            .key_index_timestamp
            .max(observed.key_index_timestamp);
        journal.pending = None;
        self.save(binding, &journal).await
    }
    /// Query and authenticate a complete current account device list. Signed
    /// indexes for untracked companions are retained during later publications.
    async fn query_current_devices(&self) -> Result<PrimaryDeviceSnapshot, PrimaryManagementError> {
        // A cancelled/failed prior save may already be visible in the snapshot.
        // Settle its retained durability future before any recovery wire action.
        self.client.persistence_manager.flush().await?;
        let sid = self.client.generate_request_id();
        let (_, journal, accounts) = self.state()?;
        let response = self
            .client
            .execute(
                wacore::iq::usync::DeviceListSpec::new(accounts.clone(), sid)
                    .require_complete_response(),
            )
            .await
            .map_err(PrimaryManagementError::protocol)?;
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let primary: &[u8; 32] = snapshot.identity_key.public_key.public_key_bytes();
        let mut devices = Vec::new();
        let mut live_indexes = vec![0];
        let mut advertised_indexes = vec![0];
        let mut high_water = journal.high_water;
        let mut key_index_timestamp = journal.key_index_timestamp;
        for list in response.device_lists {
            if !accounts.iter().any(|account| account == &list.user) {
                return Err(PrimaryManagementError::Invalid(
                    "foreign device-list account",
                ));
            }
            if let Some(bytes) = list.key_index_bytes {
                let verified = wacore::adv::verify_key_index_list(&bytes, primary)
                    .ok_or(PrimaryManagementError::Invalid("untrusted device indexes"))?;
                if verified.raw_id != journal.raw_id {
                    return Err(PrimaryManagementError::Invalid("account epoch changed"));
                }
                advertised_indexes.extend(verified.valid_indexes.iter().copied());
                high_water = high_water.max(verified.current_index);
                key_index_timestamp = key_index_timestamp.max(verified.timestamp);
                for device in list.devices {
                    if device.device == 0 {
                        continue;
                    }
                    let index = device.key_index.ok_or(PrimaryManagementError::Invalid(
                        "missing companion key index",
                    ))?;
                    if !verified.valid_indexes.contains(&index) {
                        return Err(PrimaryManagementError::Invalid(
                            "unsigned companion key index",
                        ));
                    }
                    live_indexes.push(index);
                    devices.push((
                        list.user
                            .with_device_hosting(device.device, device.is_hosted),
                        index,
                    ));
                }
            } else if list.devices.iter().any(|v| v.device != 0) {
                return Err(PrimaryManagementError::Invalid("unsigned device snapshot"));
            }
        }
        live_indexes.sort_unstable();
        live_indexes.dedup();
        if let Some(index) = live_indexes.last() {
            high_water = high_water.max(*index);
        }
        advertised_indexes.sort_unstable();
        advertised_indexes.dedup();
        devices.sort_unstable_by_key(|(jid, index)| (jid.to_string(), *index));
        devices.dedup();
        Ok(PrimaryDeviceSnapshot {
            advertised_indexes,
            devices,
            live_indexes,
            high_water,
            key_index_timestamp,
        })
    }
    pub async fn reconcile(&self) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let observed = self.query_current_devices().await?;
        let (binding, mut journal, accounts) = self.state()?;
        if journal.pending.is_some() || journal.maintenance.is_some() {
            return Err(PrimaryManagementError::Invalid(
                "active operation must be recovered first",
            ));
        }
        journal.retain_observed_companions(&observed, &accounts);
        journal.apply_observed(&observed);
        journal.key_index_timestamp = observed.key_index_timestamp;
        self.save(binding, &journal).await
    }
    /// Only an unsent prepared approval can be cancelled locally. Its index stays spent.
    pub async fn cancel_prepared(&self) -> Result<(), PrimaryManagementError> {
        let (binding, mut journal, _) = self.state()?;
        if journal
            .pending
            .as_ref()
            .is_none_or(|v| v.stage != PrimaryOperationStage::Prepared)
        {
            return Err(PrimaryManagementError::Invalid(
                "approval may have reached server; reconcile proof first",
            ));
        }
        journal.pending = None;
        self.save(binding, &journal).await
    }
    /// Release an abandoned approval's active slot while retaining its public
    /// recovery tombstone and spent index. Matching snapshots do not prove that
    /// a delayed result can never land; the returned observation stays explicit.
    pub async fn settle_unsuccessful_approval(
        &self,
    ) -> Result<ApprovalSettlement, PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        self.client.persistence_manager.flush().await?;
        let (_, journal, _) = self.state()?;
        let pending = journal
            .pending
            .as_ref()
            .ok_or(PrimaryManagementError::Invalid("no pending approval"))?;
        let spent_index = pending.key_index;
        let first = self.query_current_devices().await?;
        let second = self.query_current_devices().await?;
        if first.devices != second.devices
            || first.live_indexes != second.live_indexes
            || first.advertised_indexes != second.advertised_indexes
            || first.high_water != second.high_water
        {
            return Err(PrimaryManagementError::Invalid(
                "account topology is changing",
            ));
        }
        let observation = if second
            .devices
            .iter()
            .any(|(_, index)| *index == spent_index)
        {
            ApprovalObservation::DeviceAdvertised
        } else if second.advertised_indexes.contains(&spent_index) {
            ApprovalObservation::IndexAdvertisedWithoutDevice
        } else {
            ApprovalObservation::NotAdvertised
        };
        let (binding, mut journal, _) = self.state()?;
        if journal
            .pending
            .as_ref()
            .is_none_or(|value| value.key_index != spent_index)
            || journal.abandoned.len() >= MAX_TRACKED_COMPANIONS
        {
            return Err(PrimaryManagementError::Invalid(
                "approval changed or recovery capacity reached",
            ));
        }
        let pending = journal
            .pending
            .take()
            .ok_or(PrimaryManagementError::Invalid("approval changed"))?;
        journal.abandoned.push(AbandonedApproval { pending });
        journal.apply_observed(&second);
        self.save(binding, &journal).await?;
        Ok(ApprovalSettlement {
            key_index: spent_index,
            observation,
        })
    }
    pub fn abandoned_approvals(&self) -> Result<Vec<AbandonedApproval>, PrimaryManagementError> {
        Ok(self.state()?.1.abandoned)
    }
    /// Recover a late approval only with its original reference, signatures,
    /// peer identity and a fresh authenticated device/index observation.
    pub async fn recover_abandoned_approval(
        &self,
        key_index: u32,
        reference: &[u8],
        device: &Jid,
        proof: &[u8],
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let observed = self.query_current_devices().await?;
        let (binding, mut journal, accounts) = self.state()?;
        if journal.pending.is_some() || journal.maintenance.is_some() {
            return Err(PrimaryManagementError::Invalid("active operation"));
        }
        let position = journal
            .abandoned
            .iter()
            .position(|v| v.pending.key_index == key_index)
            .ok_or(PrimaryManagementError::Invalid(
                "unknown abandoned approval",
            ))?;
        let pending = &journal.abandoned[position].pending;
        if pending
            .device_jid
            .as_ref()
            .is_some_and(|acknowledged| acknowledged != device)
        {
            return Err(PrimaryManagementError::Invalid(
                "acknowledged abandoned companion mismatch",
            ));
        }
        if !observed
            .devices
            .iter()
            .any(|(jid, index)| jid == device && *index == key_index)
        {
            return Err(PrimaryManagementError::Invalid(
                "late companion not advertised",
            ));
        }
        let verified = PendingCompanion::new(
            &pending.reference,
            &accounts,
            &pending.peer_identity,
            SignedCompanionIdentity {
                device_identity: pending.issued_identity.clone(),
                key_index_list: pending.signed_indexes.clone(),
            },
        )?
        .acknowledge(device)?
        .verify(reference, device, proof)?;
        journal.companions.push(HostedCompanion {
            device_jid: verified.device_jid().clone(),
            peer_identity: *verified.peer_identity(),
            signed_identity: verified.signed_identity().to_vec(),
            key_index,
            history_sent: false,
            app_state_sent: false,
            history_cursors: vec![],
        });
        journal.abandoned.remove(position);
        journal.apply_observed(&observed);
        self.save(binding, &journal).await
    }
    pub async fn revoke(
        &self,
        server: &Jid,
        device: &Jid,
        reason: &str,
        timestamp_seconds: u64,
        rng: &mut impl CryptoRng,
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let observed = self.query_current_devices().await?;
        let (binding, mut journal, accounts) = self.state()?;
        if timestamp_seconds < observed.key_index_timestamp {
            return Err(PrimaryManagementError::Invalid(
                "key-index timestamp regressed",
            ));
        }
        let observed_key_index = observed_owned_device_index(&observed, device, &accounts)?;
        if observed_key_index.is_some_and(|index| {
            journal.companions.iter().any(|record| {
                same_owned_device_slot(&record.device_jid, device, &accounts)
                    && record.key_index != index
            })
        }) {
            return Err(PrimaryManagementError::Invalid(
                "tracked companion address changed key index; reconcile first",
            ));
        }
        journal.apply_observed(&observed);
        journal.key_index_timestamp = timestamp_seconds;
        if journal.pending.is_some()
            || journal.maintenance.is_some()
            || (observed_key_index.is_none()
                && !journal
                    .companions
                    .iter()
                    .any(|v| same_owned_device_slot(&v.device_jid, device, &accounts)))
        {
            return Err(PrimaryManagementError::Invalid(
                "untracked device or active operation",
            ));
        }
        RemovePrimaryCompanionSpec::new(server, &accounts, device, reason)?;
        let key_index = observed_key_index
            .or_else(|| {
                journal
                    .companions
                    .iter()
                    .find(|record| same_owned_device_slot(&record.device_jid, device, &accounts))
                    .map(|record| record.key_index)
            })
            .ok_or(PrimaryManagementError::Invalid("untracked companion"))?;
        let signed = self.sign_indexes(&journal, Some(key_index), timestamp_seconds, rng)?;
        journal.maintenance = Some(Maintenance::Revoke {
            device: device.clone(),
            key_index,
            reason: reason.to_owned(),
            signed,
            timestamp: timestamp_seconds,
            removed: false,
        });
        self.save(binding, &journal).await?;
        self.resume_maintenance_inner(server).await
    }
    /// Revoke every companion, including hosted devices, then publish only the primary index.
    pub async fn revoke_all(
        &self,
        server: &Jid,
        reason: &str,
        timestamp_seconds: u64,
        rng: &mut impl CryptoRng,
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let observed = self.query_current_devices().await?;
        let (binding, mut journal, _) = self.state()?;
        if timestamp_seconds < observed.key_index_timestamp {
            return Err(PrimaryManagementError::Invalid(
                "key-index timestamp regressed",
            ));
        }
        journal.apply_observed(&observed);
        journal.key_index_timestamp = timestamp_seconds;
        if journal.pending.is_some() || journal.maintenance.is_some() {
            return Err(PrimaryManagementError::Invalid("active operation"));
        }
        RemoveAllCompanionsSpec::new(server, reason, false)?;
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let signed = build_signed_key_index_list(
            &snapshot.identity_key,
            &IdentityParameters {
                raw_id: journal.raw_id,
                timestamp_seconds,
                key_index: journal.high_water,
                valid_indexes: &[0],
                default_encoding: if journal.explicit_defaults {
                    AdvDefaultEncoding::Explicit
                } else {
                    AdvDefaultEncoding::Omit
                },
            },
            rng,
        )?;
        journal.maintenance = Some(Maintenance::RevokeAll {
            reason: reason.to_owned(),
            signed,
            timestamp: timestamp_seconds,
            removed: false,
        });
        self.save(binding, &journal).await?;
        self.resume_maintenance_inner(server).await
    }
    fn sign_indexes(
        &self,
        journal: &PrimaryJournal,
        excluding: Option<u32>,
        timestamp_seconds: u64,
        rng: &mut impl CryptoRng,
    ) -> Result<Vec<u8>, PrimaryManagementError> {
        let indexes: Vec<_> = std::iter::once(0)
            .chain(journal.live_indexes.iter().copied())
            .filter(|index| Some(*index) != excluding)
            .collect();
        let mut indexes = indexes;
        indexes.sort_unstable();
        indexes.dedup();
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        Ok(build_signed_key_index_list(
            &snapshot.identity_key,
            &IdentityParameters {
                raw_id: journal.raw_id,
                timestamp_seconds,
                key_index: journal.high_water,
                valid_indexes: &indexes,
                default_encoding: if journal.explicit_defaults {
                    AdvDefaultEncoding::Explicit
                } else {
                    AdvDefaultEncoding::Omit
                },
            },
            rng,
        )?)
    }
    pub async fn publish_indexes(
        &self,
        server: &Jid,
        timestamp_seconds: u64,
        rng: &mut impl CryptoRng,
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let observed = self.query_current_devices().await?;
        let (binding, mut journal, accounts) = self.state()?;
        if timestamp_seconds < observed.key_index_timestamp {
            return Err(PrimaryManagementError::Invalid(
                "key-index timestamp regressed",
            ));
        }
        journal.retain_observed_companions(&observed, &accounts);
        journal.apply_observed(&observed);
        journal.key_index_timestamp = timestamp_seconds;
        if journal.pending.is_some() || journal.maintenance.is_some() {
            return Err(PrimaryManagementError::Invalid("active operation"));
        }
        let signed = self.sign_indexes(&journal, None, timestamp_seconds, rng)?;
        journal.maintenance = Some(Maintenance::Publish {
            signed,
            timestamp: timestamp_seconds,
        });
        self.save(binding, &journal).await?;
        self.resume_maintenance_inner(server).await
    }
    /// Explicit recovery of a journaled maintenance operation after transport or storage failure.
    /// A failed revoke may have reached the server; the caller decides when to retry it.
    pub async fn resume_maintenance(&self, server: &Jid) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        self.client.persistence_manager.flush().await?;
        self.resume_maintenance_inner(server).await
    }
    /// Re-sign a failed maintenance publication against a new authenticated snapshot.
    pub async fn refresh_maintenance_indexes(
        &self,
        timestamp_seconds: u64,
        rng: &mut impl CryptoRng,
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let fresh = self.query_current_devices().await?;
        let (binding, mut journal, accounts) = self.state()?;
        if timestamp_seconds < fresh.key_index_timestamp {
            return Err(PrimaryManagementError::Invalid(
                "key-index timestamp regressed",
            ));
        }
        journal.retain_observed_companions(&fresh, &accounts);
        journal.apply_observed(&fresh);
        journal.key_index_timestamp = timestamp_seconds;
        let operation = journal
            .maintenance
            .clone()
            .ok_or(PrimaryManagementError::Invalid("no maintenance operation"))?;
        let excluding = match &operation {
            Maintenance::Revoke {
                device, key_index, ..
            } => {
                if observed_owned_device_index(&fresh, device, &accounts)?
                    .is_some_and(|index| index != *key_index)
                {
                    return Err(PrimaryManagementError::Invalid("companion address reused"));
                }
                Some(*key_index)
            }
            Maintenance::Publish { .. } | Maintenance::RevokeAll { removed: true, .. } => None,
            _ => {
                return Err(PrimaryManagementError::Invalid(
                    "maintenance is not ready for index refresh",
                ));
            }
        };
        if let Maintenance::Revoke { key_index, .. } = &operation {
            journal.live_indexes.retain(|index| index != key_index);
        }
        let signed = self.sign_indexes(&journal, excluding, timestamp_seconds, rng)?;
        match journal.maintenance.as_mut() {
            Some(Maintenance::Publish {
                signed: target,
                timestamp,
            })
            | Some(Maintenance::Revoke {
                signed: target,
                timestamp,
                ..
            })
            | Some(Maintenance::RevokeAll {
                signed: target,
                timestamp,
                ..
            }) => {
                *target = signed;
                *timestamp = timestamp_seconds;
            }
            _ => return Err(PrimaryManagementError::Invalid("maintenance changed")),
        }
        self.save(binding, &journal).await
    }
    async fn resume_maintenance_inner(&self, server: &Jid) -> Result<(), PrimaryManagementError> {
        let (_, journal, accounts) = self.state()?;
        let operation = journal
            .maintenance
            .ok_or(PrimaryManagementError::Invalid("no maintenance operation"))?;
        let (signed, timestamp) = match operation {
            Maintenance::Bootstrap { .. } | Maintenance::History { .. } => {
                return Err(PrimaryManagementError::Invalid(
                    "resume with bootstrap payload",
                ));
            }
            Maintenance::Publish { signed, timestamp } => (signed, timestamp),
            Maintenance::RevokeAll {
                reason,
                signed,
                timestamp,
                removed,
            } => {
                if !removed {
                    self.client
                        .execute(RemoveAllCompanionsSpec::new(server, &reason, false)?)
                        .await
                        .map_err(PrimaryManagementError::protocol)?;
                    let (binding, mut latest, _) = self.state()?;
                    if let Some(Maintenance::RevokeAll { removed, .. }) =
                        latest.maintenance.as_mut()
                    {
                        *removed = true;
                    } else {
                        return Err(PrimaryManagementError::Invalid("maintenance changed"));
                    }
                    latest.companions.clear();
                    latest.abandoned.clear();
                    latest.abandoned_dispatches.clear();
                    latest.live_indexes = vec![0];
                    self.save(binding, &latest).await?;
                }
                (signed, timestamp)
            }
            Maintenance::Revoke {
                device,
                key_index,
                reason,
                signed,
                timestamp,
                removed,
            } => {
                if !removed {
                    let fresh = self.query_current_devices().await?;
                    if let Some(current_index) =
                        observed_owned_device_index(&fresh, &device, &accounts)?
                    {
                        if current_index != key_index {
                            return Err(PrimaryManagementError::Invalid(
                                "companion address reused with another key index",
                            ));
                        }
                        self.client
                            .execute(RemovePrimaryCompanionSpec::new(
                                server, &accounts, &device, &reason,
                            )?)
                            .await
                            .map_err(PrimaryManagementError::protocol)?;
                    }
                    let (binding, mut latest, _) = self.state()?;
                    if let Some(Maintenance::Revoke { removed, .. }) = latest.maintenance.as_mut() {
                        *removed = true;
                    } else {
                        return Err(PrimaryManagementError::Invalid("maintenance changed"));
                    }
                    latest
                        .companions
                        .retain(|record| record.key_index != key_index);
                    latest
                        .abandoned
                        .retain(|record| record.pending.key_index != key_index);
                    latest
                        .abandoned_dispatches
                        .retain(|operation| !dispatch_for_key_index(operation, key_index));
                    latest.live_indexes.retain(|index| *index != key_index);
                    self.save(binding, &latest).await?;
                }
                (signed, timestamp)
            }
        };
        let fresh = self.query_current_devices().await?;
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let primary = snapshot.identity_key.public_key.public_key_bytes();
        let expected = wacore::adv::verify_key_index_list(&signed, primary).ok_or(
            PrimaryManagementError::Invalid("maintenance index signature"),
        )?;
        let mut expected_live = expected.valid_indexes;
        expected_live.sort_unstable();
        expected_live.dedup();
        let (_, mut observed_state, _) = self.state()?;
        observed_state.apply_observed(&fresh);
        if fresh.high_water > expected.current_index
            || fresh.key_index_timestamp > timestamp
            || observed_state.live_indexes != expected_live
        {
            return Err(PrimaryManagementError::Invalid(
                "account devices changed; refresh maintenance indexes before retry",
            ));
        }
        self.client
            .execute(PublishKeyIndexListSpec::new(server, signed, timestamp)?)
            .await
            .map_err(PrimaryManagementError::protocol)?;
        let (binding, mut journal, _) = self.state()?;
        journal.maintenance = None;
        self.save(binding, &journal).await
    }
    /// Send the real history and app-state bootstrap through existing peer Signal encryption.
    /// Each successful send is checkpointed; retry resumes the missing part.
    pub async fn bootstrap(
        &self,
        device: &Jid,
        history: &wa::HistorySync,
    ) -> Result<(), PrimaryManagementError> {
        self.bootstrap_inner(device, history, None).await
    }
    pub async fn prepare_external_bootstrap_history(
        &self,
        history: &wa::HistorySync,
        media_key: [u8; 32],
    ) -> Result<PreparedBootstrapHistory, PrimaryManagementError> {
        validate_initial_history(history)?;
        if media_key == [0; 32] {
            return Err(PrimaryManagementError::Invalid("history media key"));
        }
        let bytes = bounded_history_bytes(history, 16 * 1024 * 1024)?;
        let payload_hash = Sha256::digest(&bytes).into();
        let compressed =
            wacore::runtime::blocking(&*self.client.runtime, move || compress_history(&bytes))
                .await?;
        let upload = self
            .client
            .upload(
                compressed,
                wacore::download::MediaType::History,
                crate::UploadOptions::new().with_media_key(media_key),
            )
            .await
            .map_err(PrimaryManagementError::protocol)?;
        Ok(PreparedBootstrapHistory {
            upload,
            payload_hash,
        })
    }
    /// Retry with the original retained upload, even if a later CDN refresh would
    /// return another path. The exact notification hash and message ID stay bound.
    pub async fn bootstrap_with_uploaded_history(
        &self,
        device: &Jid,
        history: &wa::HistorySync,
        prepared: &PreparedBootstrapHistory,
    ) -> Result<(), PrimaryManagementError> {
        self.bootstrap_inner(device, history, Some(prepared)).await
    }
    async fn bootstrap_inner(
        &self,
        device: &Jid,
        history: &wa::HistorySync,
        prepared: Option<&PreparedBootstrapHistory>,
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        self.client.persistence_manager.flush().await?;
        validate_initial_history(history)?;
        let history_size = history
            .try_encoded_len()
            .map_err(PrimaryManagementError::protocol)? as usize;
        if history_size > 16 * 1024 * 1024 || (history_size > 64 * 1024 && prepared.is_none()) {
            return Err(PrimaryManagementError::Invalid(
                "bootstrap requires a bounded payload and retained external descriptor",
            ));
        }
        let history_hash: [u8; 32] =
            Sha256::digest(bounded_history_bytes(history, 16 * 1024 * 1024)?).into();
        if prepared.is_some_and(|value| value.payload_hash != history_hash) {
            return Err(PrimaryManagementError::Invalid(
                "prepared history payload changed",
            ));
        }
        let media_key_fingerprint =
            prepared.map(|value| Sha256::digest(value.upload.media_key).into());
        let (_, journal, accounts) = self.state()?;
        validate_owned(device, &accounts)?;
        let record = journal
            .companions
            .iter()
            .find(|v| &v.device_jid == device)
            .ok_or(PrimaryManagementError::Invalid("unconfirmed device"))?;
        if journal
            .abandoned_dispatches
            .iter()
            .any(|op| dispatch_for_companion(op, record, &accounts))
        {
            return Err(PrimaryManagementError::Invalid(
                "dispatch was abandoned; resume it explicitly or revoke device",
            ));
        }
        if journal.pending.is_some() || journal.maintenance.as_ref().is_some_and(|op| !matches!(op, Maintenance::Bootstrap { device: reserved, peer_identity, key_index, payload_hash, media_key_fingerprint: expected_key, .. } if reserved == device && peer_identity == &record.peer_identity && *key_index == record.key_index && payload_hash == &history_hash && expected_key == &media_key_fingerprint)) {
            return Err(PrimaryManagementError::Invalid("another operation is active"));
        }
        let expected_identity = record.peer_identity;
        let (binding, mut reserved, _) = self.state()?;
        if reserved.maintenance.is_none() {
            reserved.maintenance = Some(Maintenance::Bootstrap {
                device: device.clone(),
                peer_identity: record.peer_identity,
                key_index: record.key_index,
                payload_hash: history_hash,
                media_key_fingerprint,
                history_message_id: self.client.generate_message_id().to_string(),
                app_state_message_id: self.client.generate_message_id().to_string(),
                history_message_hash: None,
                app_state_message_hash: None,
                app_state_key_id: None,
            });
            self.save(binding, &reserved).await?;
        }
        self.ensure_companion_identity(device, &expected_identity)
            .await?;
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        if !record.history_sent {
            let message = self.build_history_message(history, prepared)?;
            let message_id = self.bind_bootstrap_message(device, &message, true).await?;
            self.send_peer(device, &message, &message_id, expected_identity)
                .await?;
            self.mark_bootstrap(device, true).await?;
        }
        if !record.app_state_sent {
            let session = self
                .client
                .mobile_session
                .as_ref()
                .ok_or(PrimaryManagementError::Invalid("mobile session"))?;
            let (_, current, _) = self.state()?;
            let selected = match current.maintenance.as_ref() {
                Some(Maintenance::Bootstrap {
                    app_state_key_id, ..
                }) => app_state_key_id.clone(),
                _ => {
                    return Err(PrimaryManagementError::Invalid(
                        "bootstrap operation changed",
                    ));
                }
            };
            let id = if let Some(id) = selected {
                id
            } else {
                let id = self
                    .client
                    .ensure_primary_app_state_key(session.app_state_key())
                    .await?;
                let (binding, mut current, _) = self.state()?;
                if let Some(Maintenance::Bootstrap {
                    app_state_key_id, ..
                }) = current.maintenance.as_mut()
                {
                    *app_state_key_id = Some(id.clone());
                } else {
                    return Err(PrimaryManagementError::Invalid(
                        "bootstrap operation changed",
                    ));
                }
                self.save(binding, &current).await?;
                id
            };
            let stored = snapshot
                .backend
                .get_sync_key(&id)
                .await?
                .ok_or(PrimaryManagementError::Invalid("app-state key missing"))?;
            validate_app_state_key(&id, &stored)?;
            let fingerprint =
                waproto::codec::app_state_sync_key_fingerprint_decode(&stored.fingerprint)
                    .map_err(|_| PrimaryManagementError::Invalid("app-state fingerprint"))?;
            let mut key_id = wa::message::AppStateSyncKeyId::default();
            key_id.key_id = Some(id);
            let mut key_data = wa::message::AppStateSyncKeyData::default();
            key_data.key_data = Some(stored.key_data);
            key_data.fingerprint = waproto::protobuf::MessageField::some(fingerprint);
            key_data.timestamp = Some(stored.timestamp);
            let mut key = wa::message::AppStateSyncKey::default();
            key.key_id = waproto::protobuf::MessageField::some(key_id);
            key.key_data = waproto::protobuf::MessageField::some(key_data);
            let mut share = wa::message::AppStateSyncKeyShare::default();
            share.keys.push(key);
            let mut protocol = wa::message::ProtocolMessage::default();
            protocol.r#type = Some(wa::message::protocol_message::Type::AppStateSyncKeyShare);
            protocol.app_state_sync_key_share = waproto::protobuf::MessageField::some(share);
            let mut message = wa::Message::default();
            message.protocol_message = waproto::protobuf::MessageField::some(protocol);
            let message_id = self.bind_bootstrap_message(device, &message, false).await?;
            self.send_peer(device, &message, &message_id, expected_identity)
                .await?;
            self.mark_bootstrap(device, false).await?;
        }
        let (binding, mut latest, _) = self.state()?;
        latest.maintenance = None;
        self.save(binding, &latest).await?;
        Ok(())
    }
    /// Stop local dispatch without erasing sent-leg progress or message identities.
    /// This releases management operations; late peer application remains possible.
    pub async fn abandon_dispatch(&self, device: &Jid) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        self.client.persistence_manager.flush().await?;
        let (binding, mut journal, _) = self.state()?;
        if journal.abandoned_dispatches.len() >= MAX_TRACKED_COMPANIONS
            || journal
                .maintenance
                .as_ref()
                .is_none_or(|op| !dispatch_for_device(op, device))
        {
            return Err(PrimaryManagementError::Invalid(
                "no dispatch or recovery capacity reached",
            ));
        }
        let operation = journal
            .maintenance
            .take()
            .ok_or(PrimaryManagementError::Invalid("dispatch changed"))?;
        journal.abandoned_dispatches.push(operation);
        self.save(binding, &journal).await
    }
    /// Restore exactly the archived IDs and payload hashes before retrying the
    /// original bootstrap/history API. No fresh logical send identity is minted.
    pub async fn resume_abandoned_dispatch(
        &self,
        device: &Jid,
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        self.client.persistence_manager.flush().await?;
        let (binding, mut journal, accounts) = self.state()?;
        if journal.pending.is_some() || journal.maintenance.is_some() {
            return Err(PrimaryManagementError::Invalid("active operation"));
        }
        let record = journal
            .companions
            .iter()
            .find(|record| &record.device_jid == device)
            .ok_or(PrimaryManagementError::Invalid(
                "archived companion identity changed",
            ))?;
        let position = journal
            .abandoned_dispatches
            .iter()
            // Recovery must keep the original dispatch target as well as its
            // IDs and hashes. An authenticated alias cannot rewrite that target.
            .rposition(|op| {
                dispatch_for_device(op, device) && dispatch_for_companion(op, record, &accounts)
            })
            .ok_or(PrimaryManagementError::Invalid(
                "no abandoned dispatch for current companion",
            ))?;
        journal.maintenance = Some(journal.abandoned_dispatches.remove(position));
        self.save(binding, &journal).await
    }
    /// Replace a local bootstrap only when no peer payload has ever been bound
    /// for sending. Once a message hash exists, recovery must retain its identity.
    pub async fn cancel_unsent_bootstrap(
        &self,
        device: &Jid,
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        self.client.persistence_manager.flush().await?;
        let (binding, mut journal, _) = self.state()?;
        let safe = matches!(&journal.maintenance, Some(Maintenance::Bootstrap { device: reserved, history_message_hash: None, app_state_message_hash: None, .. }) if reserved == device)
            && journal.companions.iter().any(|record| {
                &record.device_jid == device && !record.history_sent && !record.app_state_sent
            });
        if !safe {
            return Err(PrimaryManagementError::Invalid(
                "bootstrap may have reached companion",
            ));
        }
        journal.maintenance = None;
        self.save(binding, &journal).await
    }
    async fn bind_bootstrap_message(
        &self,
        device: &Jid,
        message: &wa::Message,
        history: bool,
    ) -> Result<String, PrimaryManagementError> {
        let hash: [u8; 32] = Sha256::digest(waproto::codec::message_to_vec(message)).into();
        let (binding, mut journal, _) = self.state()?;
        let Some(Maintenance::Bootstrap {
            device: reserved,
            history_message_id,
            app_state_message_id,
            history_message_hash,
            app_state_message_hash,
            ..
        }) = journal.maintenance.as_mut()
        else {
            return Err(PrimaryManagementError::Invalid(
                "bootstrap operation changed",
            ));
        };
        if reserved != device {
            return Err(PrimaryManagementError::Invalid(
                "bootstrap companion changed",
            ));
        }
        let (id, expected) = if history {
            (history_message_id.clone(), history_message_hash)
        } else {
            (app_state_message_id.clone(), app_state_message_hash)
        };
        if expected.as_ref().is_some_and(|value| value != &hash) {
            return Err(PrimaryManagementError::Invalid(
                "ambiguous bootstrap payload changed",
            ));
        }
        if expected.is_none() {
            *expected = Some(hash);
            self.save(binding, &journal).await?;
        }
        Ok(id)
    }
    fn build_history_message(
        &self,
        history: &wa::HistorySync,
        prepared: Option<&PreparedBootstrapHistory>,
    ) -> Result<wa::Message, PrimaryManagementError> {
        validate_initial_history(history)?;
        if let Some(prepared) = prepared {
            return Ok(external_history_message(
                &PrimaryHistoryChunk {
                    sync_type: wa::message::HistorySyncType::INITIAL_BOOTSTRAP,
                    chunk_order: 0,
                    progress: 100,
                },
                &prepared.upload,
            ));
        }
        build_history_bootstrap(history)
    }
    /// Export one externally uploaded history chunk and checkpoint its exact hash
    /// and cursor after the encrypted peer send. Hosts can use upload_stream with
    /// an encrypted UploadSource for bounded-memory upload and retry/resume.
    pub async fn send_uploaded_history_chunk(
        &self,
        device: &Jid,
        metadata: PrimaryHistoryChunk,
        upload: &crate::upload::UploadResponse,
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        self.client.persistence_manager.flush().await?;
        if metadata.progress > 100
            || upload.media_key == [0; 32]
            || upload.direct_path.is_empty()
            || upload.direct_path.len() > 4096
        {
            return Err(PrimaryManagementError::Invalid("history chunk metadata"));
        }
        let (binding, mut journal, accounts) = self.state()?;
        validate_owned(device, &accounts)?;
        let record = journal
            .companions
            .iter()
            .find(|v| &v.device_jid == device)
            .ok_or(PrimaryManagementError::Invalid("unconfirmed companion"))?;
        if journal
            .abandoned_dispatches
            .iter()
            .any(|op| dispatch_for_companion(op, record, &accounts))
        {
            return Err(PrimaryManagementError::Invalid(
                "dispatch was abandoned; resume it explicitly or revoke device",
            ));
        }
        let cursor = HistoryCursor {
            sync_type: metadata.sync_type.to_i32(),
            chunk_order: metadata.chunk_order,
            progress: metadata.progress,
            file_sha256: upload.file_sha256,
        };
        if let Some(previous) = record
            .history_cursors
            .iter()
            .find(|v| v.sync_type == cursor.sync_type)
        {
            if previous.chunk_order == cursor.chunk_order
                && previous.file_sha256 == cursor.file_sha256
                && previous.progress == cursor.progress
            {
                return Ok(());
            }
            if previous.chunk_order.checked_add(1) != Some(cursor.chunk_order)
                || cursor.progress < previous.progress
            {
                return Err(PrimaryManagementError::Invalid(
                    "history cursor discontinuity",
                ));
            }
        } else if cursor.chunk_order != 0 {
            return Err(PrimaryManagementError::Invalid("initial history cursor"));
        }
        let message = external_history_message(&metadata, upload);
        let message_hash: [u8; 32] =
            Sha256::digest(waproto::codec::message_to_vec(&message)).into();
        if journal.pending.is_some() || journal.maintenance.as_ref().is_some_and(|v| !matches!(v, Maintenance::History { device: reserved, peer_identity, key_index, cursor: expected, message_hash: expected_message, .. } if reserved == device && peer_identity == &record.peer_identity && *key_index == record.key_index && expected.sync_type == cursor.sync_type && expected.chunk_order == cursor.chunk_order && expected.progress == cursor.progress && expected.file_sha256 == cursor.file_sha256 && expected_message == &message_hash)) { return Err(PrimaryManagementError::Invalid("another operation is active")); }
        let message_id = if let Some(Maintenance::History { message_id, .. }) = &journal.maintenance
        {
            message_id.clone()
        } else {
            self.client.generate_message_id().to_string()
        };
        journal.maintenance = Some(Maintenance::History {
            device: device.clone(),
            peer_identity: record.peer_identity,
            key_index: record.key_index,
            cursor: cursor.clone(),
            message_id: message_id.clone(),
            message_hash,
        });
        self.save(binding, &journal).await?;
        self.ensure_companion_identity(device, &record.peer_identity)
            .await?;
        let expected_identity = record.peer_identity;
        self.send_peer(device, &message, &message_id, expected_identity)
            .await?;
        let (binding, mut journal, _) = self.state()?;
        let record = journal
            .companions
            .iter_mut()
            .find(|v| &v.device_jid == device)
            .ok_or(PrimaryManagementError::Invalid("companion removed"))?;
        record
            .history_cursors
            .retain(|v| v.sync_type != cursor.sync_type);
        record.history_cursors.push(cursor);
        if metadata.sync_type == wa::message::HistorySyncType::INITIAL_BOOTSTRAP
            && metadata.progress == 100
        {
            record.history_sent = true;
        }
        journal.maintenance = None;
        self.save(binding, &journal).await
    }
    async fn ensure_companion_identity(
        &self,
        device: &Jid,
        expected: &[u8; 32],
    ) -> Result<(), PrimaryManagementError> {
        self.client
            .ensure_e2e_sessions(std::slice::from_ref(device))
            .await
            .map_err(PrimaryManagementError::protocol)?;
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let encryption_jid = self.client.resolve_encryption_jid(device).await;
        let address = encryption_jid.to_protocol_address();
        let identity = self
            .client
            .signal_cache
            .get_identity(&address, snapshot.backend.as_ref())
            .await
            .map_err(PrimaryManagementError::protocol)?
            .ok_or(PrimaryManagementError::Invalid(
                "missing companion Signal identity",
            ))?;
        if !bool::from(identity.as_ref().ct_eq(expected)) {
            return Err(PrimaryManagementError::Invalid(
                "companion Signal identity mismatch",
            ));
        }
        Ok(())
    }
    async fn send_peer(
        &self,
        device: &Jid,
        message: &wa::Message,
        message_id: &str,
        expected_identity: [u8; 32],
    ) -> Result<(), PrimaryManagementError> {
        self.client
            .send_message_impl(
                device.clone(),
                message,
                crate::send::SendPipelineOptions {
                    peer: true,
                    request_id: Some(message_id),
                    expected_peer_identity: Some(expected_identity),
                    ..Default::default()
                },
            )
            .await
            .map_err(PrimaryManagementError::protocol)?;
        Ok(())
    }
    async fn mark_bootstrap(
        &self,
        device: &Jid,
        history: bool,
    ) -> Result<(), PrimaryManagementError> {
        let (binding, mut journal, _) = self.state()?;
        let record = journal
            .companions
            .iter_mut()
            .find(|v| &v.device_jid == device)
            .ok_or(PrimaryManagementError::Invalid(
                "companion removed during bootstrap",
            ))?;
        if history {
            record.history_sent = true;
        } else {
            record.app_state_sent = true;
        }
        self.save(binding, &journal).await
    }
}
fn validate_owned(device: &Jid, accounts: &[Jid]) -> Result<(), PrimaryManagementError> {
    if device.device == 0
        || device.agent != 0
        || device.integrator != 0
        || device.is_hosted()
        || !accounts
            .iter()
            .any(|account| account.server == device.server && account.user == device.user)
    {
        return Err(PrimaryManagementError::Invalid(
            "foreign or unsupported companion address",
        ));
    }
    Ok(())
}
fn same_owned_device_slot(left: &Jid, right: &Jid, accounts: &[Jid]) -> bool {
    left.device == right.device
        && validate_owned(left, accounts).is_ok()
        && validate_owned(right, accounts).is_ok()
}
fn observed_owned_device_index(
    observed: &PrimaryDeviceSnapshot,
    device: &Jid,
    accounts: &[Jid],
) -> Result<Option<u32>, PrimaryManagementError> {
    validate_owned(device, accounts)?;
    let mut selected = None;
    for (_, index) in observed
        .devices
        .iter()
        .filter(|(jid, _)| same_owned_device_slot(jid, device, accounts))
    {
        if selected.is_some_and(|previous| previous != *index) {
            return Err(PrimaryManagementError::Invalid(
                "owned device aliases disagree on key index",
            ));
        }
        selected = Some(*index);
    }
    Ok(selected)
}
fn dispatch_for_key_index(operation: &Maintenance, target: u32) -> bool {
    matches!(operation, Maintenance::Bootstrap { key_index, .. } | Maintenance::History { key_index, .. } if *key_index == target)
}
fn dispatch_for_device(operation: &Maintenance, target: &Jid) -> bool {
    matches!(operation, Maintenance::Bootstrap { device, .. } | Maintenance::History { device, .. } if device == target)
}
fn dispatch_for_companion(
    operation: &Maintenance,
    companion: &HostedCompanion,
    accounts: &[Jid],
) -> bool {
    // Device slots can be reused. Retain old recovery evidence without making
    // it a barrier for a separately confirmed key/identity at the same address.
    matches!(operation,
        Maintenance::Bootstrap { device, peer_identity, key_index, .. }
        | Maintenance::History { device, peer_identity, key_index, .. }
        if same_owned_device_slot(device, &companion.device_jid, accounts)
            && *key_index == companion.key_index
            && peer_identity == &companion.peer_identity)
}
fn validate_initial_history(history: &wa::HistorySync) -> Result<(), PrimaryManagementError> {
    if history.sync_type != wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP
        || history.chunk_order != Some(0)
        || history.progress != Some(100)
    {
        return Err(PrimaryManagementError::Invalid(
            "initial bootstrap metadata",
        ));
    }
    Ok(())
}
fn valid_message_id(value: &str) -> bool {
    !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control)
}
fn bounded_history_bytes(
    history: &wa::HistorySync,
    limit: usize,
) -> Result<Vec<u8>, PrimaryManagementError> {
    let size = history
        .try_encoded_len()
        .map_err(PrimaryManagementError::protocol)? as usize;
    if size > limit {
        return Err(PrimaryManagementError::Invalid(
            "history chunk capacity; use streaming upload",
        ));
    }
    Ok(waproto::codec::history_sync_to_vec(history))
}
fn build_history_bootstrap(
    history: &wa::HistorySync,
) -> Result<wa::Message, PrimaryManagementError> {
    if history.sync_type != wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP {
        return Err(PrimaryManagementError::Invalid(
            "initial history bootstrap type",
        ));
    }
    let bytes = bounded_history_bytes(history, 64 * 1024)?;
    if bytes.len() > 64 * 1024 {
        return Err(PrimaryManagementError::Invalid(
            "inline bootstrap capacity; external history upload required",
        ));
    }
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder
        .write_all(&bytes)
        .map_err(PrimaryManagementError::protocol)?;
    let compressed = encoder.finish().map_err(PrimaryManagementError::protocol)?;
    let mut notification = wa::message::HistorySyncNotification::default();
    notification.sync_type = Some(wa::message::HistorySyncType::INITIAL_BOOTSTRAP);
    notification.chunk_order = Some(0);
    notification.progress = Some(100);
    notification.initial_hist_bootstrap_inline_payload = Some(compressed);
    let mut protocol = wa::message::ProtocolMessage::default();
    protocol.r#type = Some(wa::message::protocol_message::Type::HistorySyncNotification);
    protocol.history_sync_notification = waproto::protobuf::MessageField::some(notification);
    let mut message = wa::Message::default();
    message.protocol_message = waproto::protobuf::MessageField::some(protocol);
    Ok(message)
}

fn validate_app_state_key(
    id: &[u8],
    key: &wacore::store::traits::AppStateSyncKey,
) -> Result<(), PrimaryManagementError> {
    let fingerprint = waproto::codec::app_state_sync_key_fingerprint_decode(&key.fingerprint)
        .map_err(|_| PrimaryManagementError::Invalid("app-state fingerprint"))?;
    if id.len() != 6
        || id[..2] != [0, 0]
        || id[2..] == [0; 4]
        || key.key_data.len() != 32
        || key.key_data.as_slice() == [0; 32]
        || key.timestamp < 0
        || fingerprint.raw_id.is_none()
        || fingerprint.current_index.is_none()
        || !fingerprint.device_indexes.contains(&0)
    {
        return Err(PrimaryManagementError::Invalid(
            "persisted app-state key profile",
        ));
    }
    Ok(())
}

fn compress_history(bytes: &[u8]) -> Result<Vec<u8>, PrimaryManagementError> {
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
    encoder
        .write_all(bytes)
        .map_err(PrimaryManagementError::protocol)?;
    encoder.finish().map_err(PrimaryManagementError::protocol)
}
fn external_history_message(
    metadata: &PrimaryHistoryChunk,
    upload: &crate::upload::UploadResponse,
) -> wa::Message {
    let mut notification = wa::message::HistorySyncNotification::default();
    notification.sync_type = Some(metadata.sync_type);
    notification.chunk_order = Some(metadata.chunk_order);
    notification.progress = Some(metadata.progress);
    notification.file_sha256 = Some(upload.file_sha256.to_vec());
    notification.file_enc_sha256 = Some(upload.file_enc_sha256.to_vec());
    notification.media_key = Some(upload.media_key.to_vec());
    notification.file_length = Some(upload.file_length);
    notification.direct_path = Some(upload.direct_path.clone());
    let mut protocol = wa::message::ProtocolMessage::default();
    protocol.r#type = Some(wa::message::protocol_message::Type::HistorySyncNotification);
    protocol.history_sync_notification = waproto::protobuf::MessageField::some(notification);
    let mut message = wa::Message::default();
    message.protocol_message = waproto::protobuf::MessageField::some(protocol);
    message
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn initial_key_profile_and_persisted_key_reject_semantic_corruption() {
        assert!(PrimaryAppStateKeyInput::new([0; 6], [1; 32], 7, 9).is_err());
        assert!(PrimaryAppStateKeyInput::new([0, 0, 0, 0, 0, 1], [0; 32], 7, 9).is_err());
        assert!(PrimaryAppStateKeyInput::new([0, 0, 0, 0, 0, 1], [1; 32], 7, -1).is_err());
        let mut fingerprint = wa::message::AppStateSyncKeyFingerprint::default();
        fingerprint.raw_id = Some(7);
        fingerprint.current_index = Some(0);
        fingerprint.device_indexes = vec![0];
        let mut key = wacore::store::traits::AppStateSyncKey {
            key_data: vec![2; 32],
            fingerprint: waproto::codec::app_state_sync_key_fingerprint_to_vec(&fingerprint),
            timestamp: 9,
        };
        validate_app_state_key(&[0, 0, 0, 0, 0, 1], &key).unwrap();
        key.key_data.fill(0);
        assert!(validate_app_state_key(&[0, 0, 0, 0, 0, 1], &key).is_err());
        key.key_data.fill(2);
        key.fingerprint.clear();
        assert!(validate_app_state_key(&[0, 0, 0, 0, 0, 1], &key).is_err());
    }
    #[test]
    fn archived_dispatch_binding_requires_key_identity_and_owned_device_slot() {
        let accounts = [
            "15550000001@s.whatsapp.net".parse().unwrap(),
            "10000000001@lid".parse().unwrap(),
        ];
        let mut companion = HostedCompanion {
            device_jid: "15550000001:2@s.whatsapp.net".parse().unwrap(),
            peer_identity: [7; 32],
            signed_identity: vec![1],
            key_index: 1,
            history_sent: false,
            app_state_sent: false,
            history_cursors: vec![],
        };
        let alias = "10000000001:2@lid".parse().unwrap();
        for operation in [
            Maintenance::Bootstrap {
                device: alias,
                peer_identity: [7; 32],
                key_index: 1,
                payload_hash: [1; 32],
                media_key_fingerprint: None,
                history_message_id: "synthetic-history".into(),
                app_state_message_id: "synthetic-state".into(),
                history_message_hash: None,
                app_state_message_hash: None,
                app_state_key_id: None,
            },
            Maintenance::History {
                device: "10000000001:2@lid".parse().unwrap(),
                peer_identity: [7; 32],
                key_index: 1,
                cursor: HistoryCursor {
                    sync_type: wa::message::HistorySyncType::FULL.to_i32(),
                    chunk_order: 0,
                    progress: 37,
                    file_sha256: [1; 32],
                },
                message_id: "synthetic-chunk".into(),
                message_hash: [2; 32],
            },
        ] {
            assert!(dispatch_for_companion(&operation, &companion, &accounts));
            companion.key_index = 2;
            assert!(!dispatch_for_companion(&operation, &companion, &accounts));
            companion.key_index = 1;
            companion.peer_identity = [8; 32];
            assert!(!dispatch_for_companion(&operation, &companion, &accounts));
            companion.peer_identity = [7; 32];
            companion.device_jid.device = 3;
            assert!(!dispatch_for_companion(&operation, &companion, &accounts));
            companion.device_jid.device = 2;
            assert!(!dispatch_for_companion(
                &operation,
                &companion,
                &accounts[..1]
            ));
        }
    }
    #[test]
    fn journal_strict_schema_and_capacities_survive_roundtrip() {
        let journal = PrimaryJournal {
            schema: 1,
            raw_id: 2,
            high_water: 7,
            key_index_timestamp: 0,
            live_indexes: vec![0],
            explicit_defaults: false,
            pending: None,
            maintenance: None,
            companions: vec![],
            abandoned: vec![],
            abandoned_dispatches: vec![],
        };
        let encoded = journal.encode().unwrap();
        let restored: PrimaryJournal = serde_json::from_slice(&encoded).unwrap();
        assert_eq!(restored.high_water, 7);
        let mut value: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        value["unexpected"] = true.into();
        assert!(serde_json::from_value::<PrimaryJournal>(value).is_err());
        let mut bad = restored;
        bad.schema = 2;
        assert!(bad.encode().is_err());
    }
    #[test]
    fn inline_bootstrap_roundtrips_real_history_proto() {
        use std::io::Read as _;
        let mut history = wa::HistorySync::default();
        history.sync_type = wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP;
        history.chunk_order = Some(0);
        history.progress = Some(100);
        let message = build_history_bootstrap(&history).unwrap();
        let protocol = message.protocol_message.as_option().unwrap();
        let notification = protocol.history_sync_notification.as_option().unwrap();
        let mut plain = vec![];
        flate2::read::ZlibDecoder::new(
            notification
                .initial_hist_bootstrap_inline_payload
                .as_deref()
                .unwrap(),
        )
        .read_to_end(&mut plain)
        .unwrap();
        assert_eq!(plain, waproto::codec::history_sync_to_vec(&history));
    }
    #[test]
    fn external_history_uses_upload_crypto_and_preserves_actual_progress() {
        let upload = crate::upload::UploadResponse {
            url: "https://synthetic.invalid/history".into(),
            direct_path: "/history/synthetic".into(),
            media_key: [1; 32],
            file_enc_sha256: [2; 32],
            file_sha256: [3; 32],
            file_length: 123,
            media_key_timestamp: 9,
            streaming_sidecar: None,
        };
        let message = external_history_message(
            &PrimaryHistoryChunk {
                sync_type: wa::message::HistorySyncType::FULL,
                chunk_order: 4,
                progress: 37,
            },
            &upload,
        );
        let notification = message
            .protocol_message
            .as_option()
            .unwrap()
            .history_sync_notification
            .as_option()
            .unwrap();
        assert_eq!(notification.progress, Some(37));
        assert_eq!(notification.chunk_order, Some(4));
        assert_eq!(
            notification.file_sha256.as_deref(),
            Some(upload.file_sha256.as_slice())
        );
        assert_eq!(
            notification.file_enc_sha256.as_deref(),
            Some(upload.file_enc_sha256.as_slice())
        );
        assert_eq!(
            notification.media_key.as_deref(),
            Some(upload.media_key.as_slice())
        );
        assert_eq!(notification.file_length, Some(123));
        assert!(notification.initial_hist_bootstrap_inline_payload.is_none());
    }
    struct Wire {
        client: std::sync::Arc<Client>,
        transport: std::sync::Arc<crate::transport::mock::CapturingMockTransport>,
        events: async_channel::Sender<crate::transport::TransportEvent>,
        reader: tokio::task::JoinHandle<()>,
        received: u32,
    }
    impl Wire {
        async fn new(backend: std::sync::Arc<dyn crate::store::Backend>) -> Self {
            let (client, transport) =
                crate::test_utils::create_primary_iq_test_client_with_backend(backend).await;
            let (events, receiver) = async_channel::bounded(8);
            *client.transport_events.lock().await = Some(receiver);
            let reader_client = client.clone();
            let reader = tokio::spawn(async move {
                reader_client
                    .connection_for_test()
                    .read_until_disconnected()
                    .await;
            });
            Self {
                client,
                transport,
                events,
                reader,
                received: 0,
            }
        }
        async fn request(&self, index: usize) -> std::sync::Arc<wacore_binary::OwnedNodeRef> {
            crate::test_utils::decode_sent_iq(&self.transport, index).await
        }
        async fn reply(
            &mut self,
            request: &wacore_binary::NodeRef<'_>,
            children: Vec<wacore_binary::Node>,
            error: bool,
        ) {
            use wacore_binary::builder::NodeBuilder;
            let response = NodeBuilder::new("iq")
                .attr("from", "s.whatsapp.net")
                .attr(
                    "id",
                    request.attrs().optional_string("id").unwrap().into_owned(),
                )
                .attr("type", if error { "error" } else { "result" })
                .children(children)
                .build();
            let mut packed = wacore_binary::marshal::marshal(&response).unwrap();
            let cipher = wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap();
            cipher
                .encrypt_in_place_with_counter(self.received, &mut packed)
                .unwrap();
            self.received += 1;
            self.events
                .send(crate::transport::TransportEvent::DataReceived(
                    bytes::Bytes::from(wacore::framing::encode_frame(&packed, None).unwrap()),
                ))
                .await
                .unwrap();
        }
        async fn devices(&mut self, request: &wacore_binary::NodeRef<'_>, signed: Option<&[u8]>) {
            self.devices_snapshot(request, signed, signed.is_some())
                .await;
        }
        async fn devices_snapshot(
            &mut self,
            request: &wacore_binary::NodeRef<'_>,
            signed: Option<&[u8]>,
            present: bool,
        ) {
            use wacore_binary::builder::NodeBuilder;
            let mut entries = vec![NodeBuilder::new("device").attr("id", "0").build()];
            let mut children = vec![];
            if let Some(signed) = signed {
                if present {
                    entries.push(
                        NodeBuilder::new("device")
                            .attr("id", "2")
                            .attr("key-index", "1")
                            .build(),
                    );
                }
                children.push(
                    NodeBuilder::new("key-index-list")
                        .attr("ts", "100")
                        .bytes(signed.to_vec())
                        .build(),
                );
            }
            children.push(NodeBuilder::new("device-list").children(entries).build());
            let devices = NodeBuilder::new("devices").children(children).build();
            self.reply(
                request,
                vec![
                    NodeBuilder::new("usync")
                        .children([NodeBuilder::new("list")
                            .children([NodeBuilder::new("user")
                                .attr("jid", "15550000001@s.whatsapp.net")
                                .children([devices])
                                .build()])
                            .build()])
                        .build(),
                ],
                false,
            )
            .await;
        }
        async fn alias_devices(
            &mut self,
            request: &wacore_binary::NodeRef<'_>,
            signed: &[u8],
            pn: &[(u16, u32)],
            lid: &[(u16, u32)],
            timestamp: u64,
        ) {
            use wacore_binary::builder::NodeBuilder;
            let mut users = Vec::new();
            for (user, devices) in [("15550000001@s.whatsapp.net", pn), ("10000000001@lid", lid)] {
                let entries = std::iter::once(NodeBuilder::new("device").attr("id", "0").build())
                    .chain(devices.iter().map(|(device, index)| {
                        NodeBuilder::new("device")
                            .attr("id", device.to_string())
                            .attr("key-index", index.to_string())
                            .build()
                    }));
                users.push(
                    NodeBuilder::new("user")
                        .attr("jid", user)
                        .children([NodeBuilder::new("devices")
                            .children([
                                NodeBuilder::new("device-list").children(entries).build(),
                                NodeBuilder::new("key-index-list")
                                    .attr("ts", timestamp.to_string())
                                    .bytes(signed.to_vec())
                                    .build(),
                            ])
                            .build()])
                        .build(),
                );
            }
            self.reply(
                request,
                vec![
                    NodeBuilder::new("usync")
                        .children([NodeBuilder::new("list").children(users).build()])
                        .build(),
                ],
                false,
            )
            .await;
        }
        async fn close(self) {
            self.events
                .send(crate::transport::TransportEvent::Disconnected(
                    wacore::net::DisconnectReason::StreamEnded,
                ))
                .await
                .unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(5), self.reader)
                .await
                .unwrap()
                .unwrap();
        }
    }
    #[tokio::test]
    async fn ambiguous_upload_requires_proof_and_ambiguous_revoke_recovers_from_fresh_wire_snapshot()
     {
        use rand::SeedableRng;
        use wacore::libsignal::protocol::KeyPair;
        use wacore::pair::{DeviceState, PairUtils};
        use wacore_binary::builder::NodeBuilder;
        let backend = crate::test_utils::primary_test_backend();
        let mut wire = Wire::new(backend.clone()).await;
        wire.client
            .mobile_primary()
            .unwrap()
            .initialize(7, AdvDefaultEncoding::Omit)
            .await
            .unwrap();
        let mut rng = rand::rngs::StdRng::seed_from_u64(173);
        let companion = DeviceState {
            identity_key: KeyPair::generate(&mut rng),
            noise_key: KeyPair::generate(&mut rng),
            adv_secret_key: [7; 32],
        };
        let qr = CompanionQr::parse(&PairUtils::make_qr_data(
            &companion,
            "synthetic-reference",
            wacore::companion_reg::CompanionWebClientType::Chrome,
        ))
        .unwrap();
        let client = wire.client.clone();
        let prepare = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .prepare_qr(&qr, 100, &[], &mut rand::rngs::StdRng::seed_from_u64(174))
                .await
        });
        let request = wire.request(0).await;
        assert_eq!(
            request.get().get_optional_child("usync").unwrap().tag,
            "usync"
        );
        wire.devices(request.get(), None).await;
        assert_eq!(prepare.await.unwrap().unwrap(), 1);
        let (_, journal, _) = wire.client.mobile_primary().unwrap().state().unwrap();
        let pending = journal.pending.unwrap();
        let (proof, _) = PairUtils::do_pair_crypto(&companion, &pending.issued_identity).unwrap();
        let signed_indexes = pending.signed_indexes;
        let client = wire.client.clone();
        let upload = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .upload_prepared(&"s.whatsapp.net".parse().unwrap(), None)
                .await
        });
        let request = wire.request(1).await;
        assert!(request.get().get_optional_child("pair-device").is_some());
        wire.reply(
            request.get(),
            vec![NodeBuilder::new("error").attr("code", "500").build()],
            true,
        )
        .await;
        assert!(upload.await.unwrap().is_err());
        let api = wire.client.mobile_primary().unwrap();
        assert_eq!(
            api.pending_stage().unwrap(),
            Some(PrimaryOperationStage::UploadAttempted)
        );
        assert!(api.cancel_prepared().await.is_err());
        assert!(
            api.upload_prepared(&"s.whatsapp.net".parse().unwrap(), None)
                .await
                .is_err()
        );
        assert_eq!(wire.transport.sent().len(), 2);
        let client = wire.client.clone();
        let confirm = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .confirm(
                    b"synthetic-reference",
                    &"15550000001:2@s.whatsapp.net".parse().unwrap(),
                    &proof,
                )
                .await
        });
        let request = wire.request(2).await;
        wire.devices(request.get(), Some(&signed_indexes)).await;
        confirm.await.unwrap().unwrap();
        assert_eq!(
            wire.client
                .mobile_primary()
                .unwrap()
                .companions()
                .unwrap()
                .len(),
            1
        );
        let client = wire.client.clone();
        let revoke = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .revoke(
                    &"s.whatsapp.net".parse().unwrap(),
                    &"15550000001:2@s.whatsapp.net".parse().unwrap(),
                    "user_initiated",
                    101,
                    &mut rand::rngs::StdRng::seed_from_u64(175),
                )
                .await
        });
        let request = wire.request(3).await;
        wire.devices(request.get(), Some(&signed_indexes)).await;
        // Recovery queries before removal, rather than trusting its earlier snapshot.
        let request = wire.request(4).await;
        wire.devices(request.get(), Some(&signed_indexes)).await;
        let request = wire.request(5).await;
        assert!(
            request
                .get()
                .get_optional_child("remove-companion-device")
                .is_some()
        );
        wire.reply(
            request.get(),
            vec![NodeBuilder::new("error").attr("code", "500").build()],
            true,
        )
        .await;
        assert!(revoke.await.unwrap().is_err());
        let client = wire.client.clone();
        let resume = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .resume_maintenance(&"s.whatsapp.net".parse().unwrap())
                .await
        });
        let request = wire.request(6).await;
        wire.devices(request.get(), None).await;
        let request = wire.request(7).await;
        wire.devices(request.get(), None).await;
        let request = wire.request(8).await;
        assert!(request.get().get_optional_child("key-index-list").is_some());
        wire.reply(request.get(), vec![], false).await;
        resume.await.unwrap().unwrap();
        assert!(
            wire.client
                .mobile_primary()
                .unwrap()
                .companions()
                .unwrap()
                .is_empty()
        );
        let (_, journal, _) = wire.client.mobile_primary().unwrap().state().unwrap();
        assert_eq!(journal.high_water, 1);
        assert!(journal.maintenance.is_none());
        wire.close().await;
        let restarted = crate::test_utils::primary_startup_client().await;
        // A distinct namespace starts empty; the original backend still contains spent index 1.
        assert!(restarted.mobile_primary().unwrap().pending_stage().is_err());
        let (reloaded, _) =
            crate::test_utils::create_primary_iq_test_client_with_backend(backend).await;
        assert_eq!(
            reloaded
                .mobile_primary()
                .unwrap()
                .state()
                .unwrap()
                .1
                .high_water,
            1
        );
    }
    async fn seed_confirmed_cache_only(
        client: &std::sync::Arc<Client>,
        initialize_app_state: bool,
    ) -> (Jid, [u8; 32]) {
        seed_confirmed_companion(client, initialize_app_state, 1).await
    }
    async fn seed_confirmed_companion(
        client: &std::sync::Arc<Client>,
        initialize_app_state: bool,
        key_index: u32,
    ) -> (Jid, [u8; 32]) {
        use rand::SeedableRng;
        use wacore::libsignal::protocol::{
            IdentityKeyPair, KeyPair, PreKeyBundle, UsePQRatchet, process_prekey_bundle,
        };
        use wacore::pair::{DeviceState, PairUtils};
        let mut rng = rand::rngs::StdRng::seed_from_u64(184 + u64::from(key_index));
        let peer = DeviceState {
            identity_key: KeyPair::generate(&mut rng),
            noise_key: KeyPair::generate(&mut rng),
            adv_secret_key: [7; 32],
        };
        let peer_identity: [u8; 32] = *peer.identity_key.public_key.public_key_bytes();
        let device: Jid = "15550000001:2@s.whatsapp.net".parse().unwrap();
        let api = client.mobile_primary().unwrap();
        let first_companion = api.state().is_err();
        if first_companion {
            api.initialize(7, AdvDefaultEncoding::Omit).await.unwrap();
        }
        let snapshot = client.persistence_manager.get_device_snapshot();
        let issued = build_signed_companion_identity(
            &snapshot.identity_key,
            &peer_identity,
            &peer.adv_secret_key,
            &IdentityParameters {
                raw_id: 7,
                timestamp_seconds: 100,
                key_index,
                valid_indexes: &[0, key_index],
                default_encoding: AdvDefaultEncoding::Omit,
            },
            &mut rng,
        )
        .unwrap();
        let (proof, _) = PairUtils::do_pair_crypto(&peer, &issued.device_identity).unwrap();
        verify_companion_proof(
            &issued,
            &proof,
            &peer_identity,
            std::slice::from_ref(snapshot.pn.as_ref().unwrap()),
            &device,
        )
        .unwrap();
        let (binding, mut journal, _) = api.state().unwrap();
        journal.high_water = journal.high_water.max(key_index);
        journal.live_indexes = vec![0, key_index];
        journal.key_index_timestamp = 100;
        assert!(
            journal
                .companions
                .iter()
                .all(|record| record.device_jid != device)
        );
        journal.companions.push(HostedCompanion {
            device_jid: device.clone(),
            peer_identity,
            signed_identity: proof,
            key_index,
            history_sent: false,
            app_state_sent: false,
            history_cursors: vec![],
        });
        api.save(binding, &journal).await.unwrap();
        if initialize_app_state {
            client
                .ensure_primary_app_state_key(Some(
                    &PrimaryAppStateKeyInput::new([0, 0, 0, 0, 0, 1], [7; 32], 7, 1000).unwrap(),
                ))
                .await
                .unwrap();
        }
        let receiver = IdentityKeyPair::from(peer.identity_key);
        let spk = KeyPair::generate(&mut rng);
        let opk = KeyPair::generate(&mut rng);
        let signature = receiver
            .private_key()
            .calculate_signature(&spk.public_key.serialize(), &mut rng)
            .unwrap();
        let bundle = PreKeyBundle::new(
            1,
            1u32.into(),
            Some((1u32.into(), opk.public_key)),
            1u32.into(),
            spk.public_key,
            signature,
            *receiver.identity_key(),
        )
        .unwrap();
        let mut adapter = client.signal_adapter();
        if !first_companion {
            // Confirmation of a replacement proof permits a fresh synthetic
            // Signal session for that identity at the reused device address.
            client
                .signal_cache
                .put_identity(&device.to_protocol_address(), &peer_identity)
                .await;
        }
        process_prekey_bundle(
            &device.to_protocol_address(),
            &mut adapter.session_store,
            &mut adapter.identity_store,
            &bundle,
            &mut rng,
            UsePQRatchet::No,
        )
        .await
        .unwrap();
        if first_companion {
            assert!(
                snapshot
                    .backend
                    .load_identity(device.to_protocol_address().as_str())
                    .await
                    .unwrap()
                    .is_none(),
                "fixture identity must exist only in write-behind cache"
            );
        }
        (device, peer_identity)
    }
    fn initial_history() -> wa::HistorySync {
        let mut history = wa::HistorySync::default();
        history.sync_type = wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP;
        history.chunk_order = Some(0);
        history.progress = Some(100);
        history
    }
    fn prepared_history(path: &str) -> PreparedBootstrapHistory {
        let history = initial_history();
        let bytes = bounded_history_bytes(&history, 16 * 1024 * 1024).unwrap();
        let compressed = compress_history(&bytes).unwrap();
        PreparedBootstrapHistory::from_uploaded(
            &history,
            crate::upload::UploadResponse {
                url: "https://synthetic.invalid/history".into(),
                direct_path: path.into(),
                media_key: [1; 32],
                file_sha256: Sha256::digest(&compressed).into(),
                file_enc_sha256: [2; 32],
                file_length: compressed.len() as u64,
                media_key_timestamp: 100,
                streaming_sidecar: None,
            },
        )
        .unwrap()
    }
    struct FailCheckpointAfterWrite {
        inner: std::sync::Arc<crate::transport::mock::CapturingMockTransport>,
        fail: std::sync::Arc<std::sync::atomic::AtomicBool>,
    }
    #[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
    #[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
    impl crate::transport::Transport for FailCheckpointAfterWrite {
        async fn send(&self, data: bytes::Bytes) -> anyhow::Result<()> {
            crate::transport::Transport::send(self.inner.as_ref(), data).await?;
            // Arm storage failure synchronously after the real Noise frame was
            // accepted, before the sender can checkpoint its completed dispatch.
            self.fail.store(true, std::sync::atomic::Ordering::SeqCst);
            Ok(())
        }
        async fn disconnect(&self) {
            crate::transport::Transport::disconnect(self.inner.as_ref()).await;
        }
    }
    #[tokio::test]
    async fn bootstrap_uses_cache_only_identity_and_reuses_durable_id_after_failed_checkpoint_restart()
     {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let (mut probe, _) = crate::store::release::tests::ProbeBackend::new();
        probe.backend = crate::test_utils::primary_test_backend();
        let fail = Arc::new(AtomicBool::new(false));
        probe.fail_before_method = Some(("save", fail.clone()));
        let backend: Arc<dyn crate::store::Backend> = Arc::new(probe);
        let wire = Wire::new(backend.clone()).await;
        let (device, peer) = seed_confirmed_cache_only(&wire.client, true).await;
        let api = wire.client.mobile_primary().unwrap();
        wire.client
            .signal_cache
            .put_identity(&device.to_protocol_address(), &[9; 32])
            .await;
        assert!(api.ensure_companion_identity(&device, &peer).await.is_err());
        assert_eq!(wire.transport.sent_count(), 0);
        wire.client
            .signal_cache
            .put_identity(&device.to_protocol_address(), &peer)
            .await;
        api.ensure_companion_identity(&device, &peer).await.unwrap();
        let mut invalid = initial_history();
        invalid.progress = Some(37);
        assert!(api.bootstrap(&device, &invalid).await.is_err());
        assert!(
            api.state().unwrap().1.maintenance.is_none(),
            "invalid input cannot reserve the journal"
        );
        let fault_transport = Arc::new(FailCheckpointAfterWrite {
            inner: wire.transport.clone(),
            fail: fail.clone(),
        });
        let socket = crate::socket::NoiseSocket::with_observers(
            wire.client.runtime.clone(),
            fault_transport,
            wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap(),
            wacore::handshake::NoiseCipher::new(&[0; 32]).unwrap(),
            crate::socket::noise_socket::SendObservers::with_stats(wire.client.stats.clone())
                .with_sent_frames(wire.client.sent_frame_tap.clone()),
        );
        *wire.client.noise_socket.lock().unwrap() = Some(Arc::new(socket));
        let prepared = prepared_history("/history/original");
        assert!(
            api.bootstrap_with_uploaded_history(&device, &initial_history(), &prepared)
                .await
                .is_err()
        );
        assert!(
            fail.load(Ordering::SeqCst),
            "checkpoint fault must be armed by the real peer frame"
        );
        let first = wire.request(0).await;
        assert_eq!(first.get().tag, "message");
        assert!(
            first.get().get_optional_child("enc").is_some(),
            "bootstrap must use real Signal encryption"
        );
        let first_id = first
            .get()
            .attrs()
            .optional_string("id")
            .unwrap()
            .into_owned();
        let durable = backend.load().await.unwrap().unwrap();
        let durable_journal: PrimaryJournal = serde_json::from_slice(
            durable
                .mobile_session
                .as_ref()
                .unwrap()
                .primary_state
                .as_deref()
                .unwrap(),
        )
        .unwrap();
        assert!(!durable_journal.companions[0].history_sent);
        assert!(
            matches!(durable_journal.maintenance, Some(Maintenance::Bootstrap { history_message_id, history_message_hash: Some(_), .. }) if history_message_id == first_id)
        );
        let release = wire.client.store_release();
        wire.close().await;
        tokio::time::timeout(std::time::Duration::from_secs(5), release.wait())
            .await
            .unwrap();
        fail.store(false, Ordering::SeqCst);
        let resumed = Wire::new(backend).await;
        let changed = prepared_history("/history/refreshed-path");
        assert!(
            resumed
                .client
                .mobile_primary()
                .unwrap()
                .bootstrap_with_uploaded_history(&device, &initial_history(), &changed)
                .await
                .is_err()
        );
        assert_eq!(
            resumed.transport.sent_count(),
            0,
            "changed descriptor cannot rebind an ambiguous message"
        );
        resumed
            .client
            .mobile_primary()
            .unwrap()
            .bootstrap_with_uploaded_history(&device, &initial_history(), &prepared)
            .await
            .unwrap();
        let repeated = resumed.request(0).await;
        assert_eq!(
            repeated.get().attrs().optional_string("id").as_deref(),
            Some(first_id.as_str())
        );
        assert!(
            resumed
                .client
                .mobile_primary()
                .unwrap()
                .companions()
                .unwrap()[0]
                .bootstrap_dispatch_complete()
        );
        resumed.close().await;
    }
    #[tokio::test]
    async fn undurable_bootstrap_intent_cannot_send_until_retained_flush_succeeds() {
        use std::sync::{
            Arc,
            atomic::{AtomicBool, Ordering},
        };
        let (mut probe, _) = crate::store::release::tests::ProbeBackend::new();
        probe.backend = crate::test_utils::primary_test_backend();
        let fail = Arc::new(AtomicBool::new(false));
        probe.fail_before_method = Some(("save", fail.clone()));
        let wire = Wire::new(Arc::new(probe)).await;
        let (device, _) = seed_confirmed_cache_only(&wire.client, true).await;
        let api = wire.client.mobile_primary().unwrap();
        fail.store(true, Ordering::SeqCst);
        assert!(api.bootstrap(&device, &initial_history()).await.is_err());
        let id = match api.state().unwrap().1.maintenance.unwrap() {
            Maintenance::Bootstrap {
                history_message_id, ..
            } => history_message_id,
            _ => panic!("bootstrap intent"),
        };
        assert!(api.bootstrap(&device, &initial_history()).await.is_err());
        assert_eq!(
            wire.transport.sent_count(),
            0,
            "resume must settle the failed intent flush before wire"
        );
        fail.store(false, Ordering::SeqCst);
        api.bootstrap(&device, &initial_history()).await.unwrap();
        let sent = wire.request(0).await;
        assert_eq!(
            sent.get().attrs().optional_string("id").as_deref(),
            Some(id.as_str())
        );
        wire.close().await;
    }
    #[tokio::test]
    async fn abandoned_upload_releases_slot_preserves_spent_index_and_recovers_or_revokes_late_device()
     {
        use rand::SeedableRng;
        use wacore::libsignal::protocol::KeyPair;
        use wacore::pair::{DeviceState, PairUtils};
        use wacore_binary::builder::NodeBuilder;
        for advertised_only in [false, true] {
            let mut wire = Wire::new(crate::test_utils::primary_test_backend()).await;
            wire.client
                .mobile_primary()
                .unwrap()
                .initialize(7, AdvDefaultEncoding::Omit)
                .await
                .unwrap();
            let mut rng = rand::rngs::StdRng::seed_from_u64(190);
            let peer = DeviceState {
                identity_key: KeyPair::generate(&mut rng),
                noise_key: KeyPair::generate(&mut rng),
                adv_secret_key: [7; 32],
            };
            let qr_text = PairUtils::make_qr_data(
                &peer,
                "abandoned-reference",
                wacore::companion_reg::CompanionWebClientType::Chrome,
            );
            let client = wire.client.clone();
            let qr = CompanionQr::parse(&qr_text).unwrap();
            let prepare = tokio::spawn(async move {
                client
                    .mobile_primary()
                    .unwrap()
                    .prepare_qr(&qr, 100, &[], &mut rand::rngs::StdRng::seed_from_u64(191))
                    .await
            });
            let request = wire.request(0).await;
            wire.devices(request.get(), None).await;
            assert_eq!(prepare.await.unwrap().unwrap(), 1);
            let (_, journal, _) = wire.client.mobile_primary().unwrap().state().unwrap();
            let pending = journal.pending.unwrap();
            let signed = pending.signed_indexes;
            let (proof, _) = PairUtils::do_pair_crypto(&peer, &pending.issued_identity).unwrap();
            let client = wire.client.clone();
            let upload = tokio::spawn(async move {
                client
                    .mobile_primary()
                    .unwrap()
                    .upload_prepared(&"s.whatsapp.net".parse().unwrap(), None)
                    .await
            });
            let request = wire.request(1).await;
            wire.reply(
                request.get(),
                vec![NodeBuilder::new("error").attr("code", "500").build()],
                true,
            )
            .await;
            assert!(upload.await.unwrap().is_err());
            let client = wire.client.clone();
            let settle = tokio::spawn(async move {
                client
                    .mobile_primary()
                    .unwrap()
                    .settle_unsuccessful_approval()
                    .await
            });
            for index in [2, 3] {
                let request = wire.request(index).await;
                wire.devices_snapshot(
                    request.get(),
                    if advertised_only { Some(&signed) } else { None },
                    false,
                )
                .await;
            }
            let settlement = settle.await.unwrap().unwrap();
            assert_eq!(settlement.key_index(), 1);
            assert_eq!(
                settlement.observation(),
                if advertised_only {
                    ApprovalObservation::IndexAdvertisedWithoutDevice
                } else {
                    ApprovalObservation::NotAdvertised
                }
            );
            let api = wire.client.mobile_primary().unwrap();
            assert_eq!(api.abandoned_approvals().unwrap().len(), 1);
            assert!(api.pending_stage().unwrap().is_none());
            let client = wire.client.clone();
            let qr = CompanionQr::parse(&qr_text).unwrap();
            let prepare = tokio::spawn(async move {
                client
                    .mobile_primary()
                    .unwrap()
                    .prepare_qr(&qr, 100, &[], &mut rand::rngs::StdRng::seed_from_u64(192))
                    .await
            });
            let request = wire.request(4).await;
            wire.devices_snapshot(
                request.get(),
                if advertised_only { Some(&signed) } else { None },
                false,
            )
            .await;
            assert_eq!(
                prepare.await.unwrap().unwrap(),
                2,
                "an abandoned index must never be reused"
            );
            wire.client
                .mobile_primary()
                .unwrap()
                .cancel_prepared()
                .await
                .unwrap();
            let client = wire.client.clone();
            if !advertised_only {
                let recover = tokio::spawn(async move {
                    client
                        .mobile_primary()
                        .unwrap()
                        .recover_abandoned_approval(
                            1,
                            b"abandoned-reference",
                            &"15550000001:2@s.whatsapp.net".parse().unwrap(),
                            &proof,
                        )
                        .await
                });
                let request = wire.request(5).await;
                wire.devices(request.get(), Some(&signed)).await;
                recover.await.unwrap().unwrap();
                assert_eq!(
                    wire.client
                        .mobile_primary()
                        .unwrap()
                        .companions()
                        .unwrap()
                        .len(),
                    1
                );
            } else {
                let revoke = tokio::spawn(async move {
                    client
                        .mobile_primary()
                        .unwrap()
                        .revoke(
                            &"s.whatsapp.net".parse().unwrap(),
                            &"15550000001:2@s.whatsapp.net".parse().unwrap(),
                            "user_initiated",
                            101,
                            &mut rand::rngs::StdRng::seed_from_u64(193),
                        )
                        .await
                });
                for index in [5, 6] {
                    let request = wire.request(index).await;
                    wire.devices(request.get(), Some(&signed)).await;
                }
                let request = wire.request(7).await;
                assert!(
                    request
                        .get()
                        .get_optional_child("remove-companion-device")
                        .is_some()
                );
                wire.reply(request.get(), vec![], false).await;
                let request = wire.request(8).await;
                wire.devices(request.get(), None).await;
                let request = wire.request(9).await;
                assert!(request.get().get_optional_child("key-index-list").is_some());
                wire.reply(request.get(), vec![], false).await;
                revoke.await.unwrap().unwrap();
            }
            assert!(
                wire.client
                    .mobile_primary()
                    .unwrap()
                    .abandoned_approvals()
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                wire.client
                    .mobile_primary()
                    .unwrap()
                    .state()
                    .unwrap()
                    .1
                    .high_water,
                2
            );
            wire.close().await;
        }
    }
    #[tokio::test]
    async fn cancelled_intent_save_is_settled_before_any_bootstrap_frame() {
        use std::sync::Arc;
        let (mut probe, _) = crate::store::release::tests::ProbeBackend::new();
        probe.backend = crate::test_utils::primary_test_backend();
        let (release_tx, release_rx) = async_channel::unbounded();
        let (entered_tx, entered_rx) = async_channel::unbounded();
        for _ in 0..32 {
            release_tx.try_send(()).unwrap();
        }
        probe.gate = Some(release_rx.clone());
        probe.entered = Some(entered_tx);
        probe.gate_method = "save";
        let wire = Wire::new(Arc::new(probe)).await;
        let (device, _) = seed_confirmed_cache_only(&wire.client, true).await;
        while release_rx.try_recv().is_ok() {}
        while entered_rx.try_recv().is_ok() {}
        let client = wire.client.clone();
        let target = device.clone();
        let first = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .bootstrap(&target, &initial_history())
                .await
        });
        tokio::time::timeout(std::time::Duration::from_secs(5), entered_rx.recv())
            .await
            .unwrap()
            .unwrap();
        first.abort();
        assert!(first.await.unwrap_err().is_cancelled());
        let operation = wire
            .client
            .mobile_primary()
            .unwrap()
            .state()
            .unwrap()
            .1
            .maintenance
            .unwrap();
        let id = match &operation {
            Maintenance::Bootstrap {
                history_message_id, ..
            } => history_message_id.clone(),
            _ => panic!("bootstrap intent"),
        };
        let mut legacy = serde_json::to_value(&operation).unwrap();
        legacy.as_object_mut().unwrap().remove("history_message_id");
        assert!(
            serde_json::from_value::<Maintenance>(legacy).is_err(),
            "old ambiguous journals cannot silently mint replacement IDs"
        );
        let client = wire.client.clone();
        let target = device.clone();
        let retry = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .bootstrap(&target, &initial_history())
                .await
        });
        for _ in 0..32 {
            tokio::task::yield_now().await;
        }
        assert!(!retry.is_finished());
        assert_eq!(wire.transport.sent_count(), 0);
        for _ in 0..32 {
            release_tx.try_send(()).unwrap();
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), retry)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        let request = wire.request(0).await;
        assert_eq!(
            request.get().attrs().optional_string("id").as_deref(),
            Some(id.as_str())
        );
        wire.close().await;
    }
    #[tokio::test]
    async fn abandoned_alias_dispatch_is_not_activated_under_a_different_target() {
        let wire = Wire::new(crate::test_utils::primary_test_backend()).await;
        let (device, _) = seed_confirmed_cache_only(&wire.client, false).await;
        let api = wire.client.mobile_primary().unwrap();
        assert!(api.bootstrap(&device, &initial_history()).await.is_err());
        api.abandon_dispatch(&device).await.unwrap();
        wire.client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                "10000000001@lid".parse().unwrap(),
            )))
            .await;
        let (binding, mut journal, _) = api.state().unwrap();
        let Maintenance::Bootstrap { device: target, .. } = &mut journal.abandoned_dispatches[0]
        else {
            panic!("bootstrap archive")
        };
        *target = "10000000001:2@lid".parse().unwrap();
        let archive_bytes = serde_json::to_vec(&journal.abandoned_dispatches).unwrap();
        api.save(binding, &journal).await.unwrap();
        let sent_before = wire.transport.sent_count();
        assert!(api.resume_abandoned_dispatch(&device).await.is_err());
        assert!(api.bootstrap(&device, &initial_history()).await.is_err());
        assert_eq!(wire.transport.sent_count(), sent_before);
        let (_, latest, _) = api.state().unwrap();
        assert!(latest.maintenance.is_none());
        assert_eq!(
            serde_json::to_vec(&latest.abandoned_dispatches).unwrap(),
            archive_bytes,
            "rejected alias recovery cannot retarget or consume the original archive"
        );
        wire.close().await;
    }
    #[tokio::test]
    async fn abandoned_dispatch_does_not_block_replacement_at_the_same_device_address() {
        let mut wire = Wire::new(crate::test_utils::primary_test_backend()).await;
        let (device, old_identity) = seed_confirmed_cache_only(&wire.client, false).await;
        let api_client = wire.client.clone();
        let api = api_client.mobile_primary().unwrap();
        assert!(api.bootstrap(&device, &initial_history()).await.is_err());
        api.abandon_dispatch(&device).await.unwrap();
        let old_archive = api.state().unwrap().1.abandoned_dispatches[0].clone();
        let old_archive_bytes = serde_json::to_vec(&old_archive).unwrap();

        let client = wire.client.clone();
        let reconcile =
            tokio::spawn(async move { client.mobile_primary().unwrap().reconcile().await });
        let request = wire.request(1).await;
        wire.devices(request.get(), None).await;
        reconcile.await.unwrap().unwrap();
        assert!(api.companions().unwrap().is_empty());
        let (replacement, new_identity) = seed_confirmed_companion(&wire.client, true, 2).await;
        assert_eq!(replacement, device);
        assert_ne!(new_identity, old_identity);
        assert!(api.resume_abandoned_dispatch(&device).await.is_err());
        let before = wire.transport.sent_count();
        api.bootstrap(&device, &initial_history()).await.unwrap();
        assert_eq!(wire.transport.sent_count(), before + 2);
        let upload = prepared_history("/history/replacement").upload;
        api.send_uploaded_history_chunk(
            &device,
            PrimaryHistoryChunk {
                sync_type: wa::message::HistorySyncType::FULL,
                chunk_order: 0,
                progress: 37,
            },
            &upload,
        )
        .await
        .unwrap();
        assert_eq!(wire.transport.sent_count(), before + 3);
        let (_, journal, _) = api.state().unwrap();
        assert!(journal.maintenance.is_none());
        assert_eq!(journal.high_water, 2);
        assert!(journal.companions[0].bootstrap_dispatch_complete());
        assert_eq!(journal.companions[0].key_index, 2);
        assert_eq!(journal.companions[0].history_cursors.len(), 1);
        assert_eq!(journal.abandoned_dispatches.len(), 1);
        assert_eq!(
            serde_json::to_vec(&journal.abandoned_dispatches[0]).unwrap(),
            old_archive_bytes,
            "replacement dispatch must retain the old recovery evidence unchanged"
        );
        wire.close().await;
    }
    #[tokio::test]
    async fn partially_dispatched_bootstrap_can_be_abandoned_and_device_revoked() {
        use rand::SeedableRng;
        let mut wire = Wire::new(crate::test_utils::primary_test_backend()).await;
        let (device, _) = seed_confirmed_cache_only(&wire.client, false).await;
        let api = wire.client.mobile_primary().unwrap();
        assert!(
            api.bootstrap(&device, &initial_history()).await.is_err(),
            "missing app-state key interrupts after history dispatch"
        );
        let (_, journal, _) = api.state().unwrap();
        assert!(journal.companions[0].history_sent);
        assert!(!journal.companions[0].app_state_sent);
        let original_ids = match journal.maintenance.as_ref().unwrap() {
            Maintenance::Bootstrap {
                history_message_id,
                app_state_message_id,
                ..
            } => (history_message_id.clone(), app_state_message_id.clone()),
            _ => panic!("bootstrap intent"),
        };
        let signed = api
            .sign_indexes(
                &journal,
                None,
                100,
                &mut rand::rngs::StdRng::seed_from_u64(195),
            )
            .unwrap();
        api.abandon_dispatch(&device).await.unwrap();
        let (_, archived, _) = api.state().unwrap();
        assert!(archived.maintenance.is_none());
        assert!(archived.companions[0].history_sent);
        assert!(!archived.companions[0].app_state_sent);
        assert!(
            matches!(&archived.abandoned_dispatches[0], Maintenance::Bootstrap { history_message_id, app_state_message_id, .. } if (history_message_id, app_state_message_id) == (&original_ids.0, &original_ids.1))
        );
        let sent_before = wire.transport.sent_count();
        assert!(api.bootstrap(&device, &initial_history()).await.is_err());
        assert!(
            api.send_uploaded_history_chunk(
                &device,
                PrimaryHistoryChunk {
                    sync_type: wa::message::HistorySyncType::FULL,
                    chunk_order: 0,
                    progress: 37,
                },
                &prepared_history("/history/blocked").upload,
            )
            .await
            .is_err()
        );
        assert_eq!(
            wire.transport.sent_count(),
            sent_before,
            "abandoned dispatch cannot auto-resend with new IDs"
        );
        let client = wire.client.clone();
        let revoke = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .revoke(
                    &"s.whatsapp.net".parse().unwrap(),
                    &device,
                    "user_initiated",
                    101,
                    &mut rand::rngs::StdRng::seed_from_u64(196),
                )
                .await
        });
        for index in [1, 2] {
            let request = wire.request(index).await;
            wire.devices(request.get(), Some(&signed)).await;
        }
        let request = wire.request(3).await;
        wire.reply(request.get(), vec![], false).await;
        let request = wire.request(4).await;
        wire.devices(request.get(), None).await;
        let request = wire.request(5).await;
        wire.reply(request.get(), vec![], false).await;
        revoke.await.unwrap().unwrap();
        let (_, final_state, _) = wire.client.mobile_primary().unwrap().state().unwrap();
        assert!(final_state.companions.is_empty());
        assert!(final_state.abandoned_dispatches.is_empty());
        wire.close().await;
    }
    #[tokio::test]
    async fn preparing_after_external_removal_prunes_tracked_index_and_preserves_high_water() {
        use rand::SeedableRng;
        use wacore::libsignal::protocol::KeyPair;
        use wacore::pair::{DeviceState, PairUtils};
        let mut wire = Wire::new(crate::test_utils::primary_test_backend()).await;
        seed_confirmed_cache_only(&wire.client, false).await;
        let api = wire.client.mobile_primary().unwrap();
        let (binding, mut journal, accounts) = api.state().unwrap();
        journal.high_water = 9;
        let mut capacity_fixture = journal.clone();
        capacity_fixture
            .companions
            .resize(MAX_TRACKED_COMPANIONS, journal.companions[0].clone());
        // Exercise retention at the capacity bound independently of proof encoding:
        // the real persisted fixture below retains its verified identity proof.
        capacity_fixture.retain_observed_companions(
            &PrimaryDeviceSnapshot {
                devices: vec![],
                live_indexes: vec![0],
                advertised_indexes: vec![0],
                high_water: 9,
                key_index_timestamp: 100,
            },
            &accounts,
        );
        assert!(capacity_fixture.companions.len() < MAX_TRACKED_COMPANIONS);
        assert_eq!(capacity_fixture.high_water, 9);
        api.save(binding, &journal).await.unwrap();
        let snapshot = wire.client.persistence_manager.get_device_snapshot();
        let mut rng = rand::rngs::StdRng::seed_from_u64(214);
        let signed = build_signed_key_index_list(
            &snapshot.identity_key,
            &IdentityParameters {
                raw_id: 7,
                timestamp_seconds: 100,
                key_index: 9,
                valid_indexes: &[0],
                default_encoding: AdvDefaultEncoding::Omit,
            },
            &mut rng,
        )
        .unwrap();
        let companion = DeviceState {
            identity_key: KeyPair::generate(&mut rng),
            noise_key: KeyPair::generate(&mut rng),
            adv_secret_key: [6; 32],
        };
        let qr = CompanionQr::parse(&PairUtils::make_qr_data(
            &companion,
            "replacement-reference",
            wacore::companion_reg::CompanionWebClientType::Chrome,
        ))
        .unwrap();
        let client = wire.client.clone();
        let prepare = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .prepare_qr(&qr, 101, &[7], &mut rand::rngs::StdRng::seed_from_u64(215))
                .await
        });
        let request = wire.request(0).await;
        wire.devices_snapshot(request.get(), Some(&signed), false)
            .await;
        assert_eq!(prepare.await.unwrap().unwrap(), 10);
        let (_, prepared, _) = wire.client.mobile_primary().unwrap().state().unwrap();
        assert!(prepared.companions.is_empty());
        assert_eq!(prepared.high_water, 10);
        let pending = prepared.pending.unwrap();
        let envelope =
            waproto::codec::adv_signed_key_index_list_decode(&pending.signed_indexes).unwrap();
        let indexes =
            waproto::codec::adv_key_index_list_decode(envelope.details.as_deref().unwrap())
                .unwrap();
        assert_eq!(indexes.current_index, Some(10));
        assert_eq!(
            indexes.valid_indexes,
            vec![0, 7, 10],
            "caller-supplied indexes remain explicit"
        );
        assert_eq!(
            wire.transport.sent_count(),
            1,
            "preparation sends only the fresh device query"
        );
        wire.close().await;
    }
    #[tokio::test]
    async fn publishing_filters_removed_companions_before_signing_and_durable_refresh() {
        use rand::SeedableRng;
        fn assert_published_indexes(
            node: &wacore_binary::NodeRef<'_>,
            high_water: u32,
            indexes: &[u32],
        ) {
            let signed = node.get_optional_child("key-index-list").unwrap();
            let envelope =
                waproto::codec::adv_signed_key_index_list_decode(signed.content_bytes().unwrap())
                    .unwrap();
            let details =
                waproto::codec::adv_key_index_list_decode(envelope.details.as_deref().unwrap())
                    .unwrap();
            assert_eq!(details.current_index, Some(high_water));
            assert_eq!(details.valid_indexes, indexes);
        }
        for removed_before_publish in [true, false] {
            let mut wire = Wire::new(crate::test_utils::primary_test_backend()).await;
            let (tracked, _) = seed_confirmed_cache_only(&wire.client, false).await;
            wire.client
                .persistence_manager
                .process_command(wacore::store::DeviceCommand::SetLid(Some(
                    "10000000001@lid".parse().unwrap(),
                )))
                .await;
            wire.client.persistence_manager.flush().await.unwrap();
            let api = wire.client.mobile_primary().unwrap();
            let (binding, mut journal, _) = api.state().unwrap();
            journal.high_water = 9;
            api.save(binding, &journal).await.unwrap();
            let snapshot = wire.client.persistence_manager.get_device_snapshot();
            let mut rng = rand::rngs::StdRng::seed_from_u64(211);
            let mut signed_snapshot = |indexes: &[u32]| {
                build_signed_key_index_list(
                    &snapshot.identity_key,
                    &IdentityParameters {
                        raw_id: 7,
                        timestamp_seconds: 100,
                        key_index: 9,
                        valid_indexes: indexes,
                        default_encoding: AdvDefaultEncoding::Omit,
                    },
                    &mut rng,
                )
                .unwrap()
            };
            let present = signed_snapshot(&[0, 1]);
            let absent = signed_snapshot(&[0]);
            let client = wire.client.clone();
            let publish = tokio::spawn(async move {
                client
                    .mobile_primary()
                    .unwrap()
                    .publish_indexes(
                        &"s.whatsapp.net".parse().unwrap(),
                        101,
                        &mut rand::rngs::StdRng::seed_from_u64(212),
                    )
                    .await
            });
            let request = wire.request(0).await;
            if removed_before_publish {
                wire.alias_devices(request.get(), &absent, &[], &[], 100)
                    .await;
            } else {
                // A LID-only alias retains the original PN-bound proof until removal.
                wire.alias_devices(request.get(), &present, &[], &[(2, 1)], 100)
                    .await;
            }
            let request = wire.request(1).await;
            wire.alias_devices(request.get(), &absent, &[], &[], 100)
                .await;
            let publication_frame = if removed_before_publish {
                2
            } else {
                assert!(
                    publish.await.unwrap().is_err(),
                    "drift must pause publication"
                );
                let (_, interrupted, _) = wire.client.mobile_primary().unwrap().state().unwrap();
                assert_eq!(interrupted.companions[0].device_jid(), &tracked);
                assert!(matches!(
                    interrupted.maintenance,
                    Some(Maintenance::Publish { .. })
                ));
                let client = wire.client.clone();
                let refresh = tokio::spawn(async move {
                    client
                        .mobile_primary()
                        .unwrap()
                        .refresh_maintenance_indexes(
                            102,
                            &mut rand::rngs::StdRng::seed_from_u64(213),
                        )
                        .await
                });
                let request = wire.request(2).await;
                wire.alias_devices(request.get(), &absent, &[], &[], 100)
                    .await;
                refresh.await.unwrap().unwrap();
                let (_, refreshed, _) = wire.client.mobile_primary().unwrap().state().unwrap();
                assert!(refreshed.companions.is_empty());
                assert_eq!(refreshed.high_water, 9, "removed indexes remain spent");
                let client = wire.client.clone();
                let resume = tokio::spawn(async move {
                    client
                        .mobile_primary()
                        .unwrap()
                        .resume_maintenance(&"s.whatsapp.net".parse().unwrap())
                        .await
                });
                let request = wire.request(3).await;
                wire.alias_devices(request.get(), &absent, &[], &[], 100)
                    .await;
                let request = wire.request(4).await;
                assert_published_indexes(request.get(), 9, &[0]);
                wire.reply(request.get(), vec![], false).await;
                resume.await.unwrap().unwrap();
                let (_, complete, _) = wire.client.mobile_primary().unwrap().state().unwrap();
                assert!(complete.companions.is_empty());
                assert!(complete.maintenance.is_none());
                assert_eq!(complete.high_water, 9);
                wire.close().await;
                continue;
            };
            let request = wire.request(publication_frame).await;
            assert_published_indexes(request.get(), 9, &[0]);
            wire.reply(request.get(), vec![], false).await;
            publish.await.unwrap().unwrap();
            let (_, complete, _) = wire.client.mobile_primary().unwrap().state().unwrap();
            assert!(complete.companions.is_empty());
            assert!(complete.maintenance.is_none());
            assert_eq!(complete.high_water, 9);
            wire.close().await;
        }
    }
    #[tokio::test]
    async fn lid_alias_revocation_excludes_tracked_pn_index_through_ambiguous_resume_and_refresh() {
        use rand::SeedableRng;
        use wacore::libsignal::protocol::KeyPair;
        use wacore::pair::{DeviceState, PairUtils};
        use wacore_binary::builder::NodeBuilder;
        let mut wire = Wire::new(crate::test_utils::primary_test_backend()).await;
        let (tracked, first_identity) = seed_confirmed_cache_only(&wire.client, true).await;
        wire.client
            .persistence_manager
            .process_command(wacore::store::DeviceCommand::SetLid(Some(
                "10000000001@lid".parse().unwrap(),
            )))
            .await;
        wire.client.persistence_manager.flush().await.unwrap();
        let alias: Jid = "10000000001:2@lid".parse().unwrap();
        let other: Jid = "15550000001:3@s.whatsapp.net".parse().unwrap();
        let snapshot = wire.client.persistence_manager.get_device_snapshot();
        let mut rng = rand::rngs::StdRng::seed_from_u64(201);
        let second = DeviceState {
            identity_key: KeyPair::generate(&mut rng),
            noise_key: KeyPair::generate(&mut rng),
            adv_secret_key: [4; 32],
        };
        let second_identity: [u8; 32] = *second.identity_key.public_key.public_key_bytes();
        let issued = build_signed_companion_identity(
            &snapshot.identity_key,
            &second_identity,
            &second.adv_secret_key,
            &IdentityParameters {
                raw_id: 7,
                timestamp_seconds: 100,
                key_index: 2,
                valid_indexes: &[0, 1, 2],
                default_encoding: AdvDefaultEncoding::Omit,
            },
            &mut rng,
        )
        .unwrap();
        let (proof, _) = PairUtils::do_pair_crypto(&second, &issued.device_identity).unwrap();
        let api = wire.client.mobile_primary().unwrap();
        let (binding, mut journal, accounts) = api.state().unwrap();
        verify_companion_proof(&issued, &proof, &second_identity, &accounts, &other).unwrap();
        journal.high_water = 9;
        journal.live_indexes = vec![0, 1, 2];
        journal.companions.push(HostedCompanion {
            device_jid: other.clone(),
            peer_identity: second_identity,
            signed_identity: proof,
            key_index: 2,
            history_sent: true,
            app_state_sent: false,
            history_cursors: vec![],
        });
        for (device, peer_identity, key_index) in [
            (tracked.clone(), first_identity, 1),
            (other.clone(), second_identity, 2),
        ] {
            journal.abandoned_dispatches.push(Maintenance::Bootstrap {
                device,
                peer_identity,
                key_index,
                payload_hash: [1; 32],
                media_key_fingerprint: None,
                history_message_id: format!("synthetic-history-{key_index}"),
                app_state_message_id: format!("synthetic-state-{key_index}"),
                history_message_hash: Some([2; 32]),
                app_state_message_hash: None,
                app_state_key_id: None,
            });
        }
        api.save(binding, &journal).await.unwrap();
        let signed = build_signed_key_index_list(
            &snapshot.identity_key,
            &IdentityParameters {
                raw_id: 7,
                timestamp_seconds: 100,
                key_index: 9,
                valid_indexes: &[0, 1, 2],
                default_encoding: AdvDefaultEncoding::Omit,
            },
            &mut rng,
        )
        .unwrap();
        let changed = build_signed_key_index_list(
            &snapshot.identity_key,
            &IdentityParameters {
                raw_id: 7,
                timestamp_seconds: 101,
                key_index: 10,
                valid_indexes: &[0, 2, 10],
                default_encoding: AdvDefaultEncoding::Omit,
            },
            &mut rng,
        )
        .unwrap();
        let decode_indexes = |signed: &[u8]| {
            let envelope = waproto::codec::adv_signed_key_index_list_decode(signed).unwrap();
            waproto::codec::adv_key_index_list_decode(envelope.details.as_deref().unwrap()).unwrap()
        };
        // A fresh LID-only observation of companions must retain their stored PN proof addresses.
        let client = wire.client.clone();
        let reconcile =
            tokio::spawn(async move { client.mobile_primary().unwrap().reconcile().await });
        let request = wire.request(0).await;
        wire.alias_devices(request.get(), &signed, &[], &[(2, 1), (3, 2)], 100)
            .await;
        reconcile.await.unwrap().unwrap();
        assert_eq!(
            wire.client
                .mobile_primary()
                .unwrap()
                .companions()
                .unwrap()
                .len(),
            2
        );
        let client = wire.client.clone();
        let target = alias.clone();
        let revoke = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .revoke(
                    &"s.whatsapp.net".parse().unwrap(),
                    &target,
                    "user_initiated",
                    101,
                    &mut rand::rngs::StdRng::seed_from_u64(202),
                )
                .await
        });
        for frame in [1, 2] {
            let request = wire.request(frame).await;
            wire.alias_devices(request.get(), &signed, &[], &[(2, 1), (3, 2)], 100)
                .await;
        }
        let request = wire.request(3).await;
        assert_eq!(
            request
                .get()
                .get_optional_child("remove-companion-device")
                .unwrap()
                .attrs()
                .optional_jid("jid"),
            Some(alias)
        );
        wire.reply(
            request.get(),
            vec![NodeBuilder::new("error").attr("code", "500").build()],
            true,
        )
        .await;
        assert!(revoke.await.unwrap().is_err());
        let (_, interrupted, _) = wire.client.mobile_primary().unwrap().state().unwrap();
        let Some(Maintenance::Revoke {
            signed: prepared,
            removed: false,
            ..
        }) = interrupted.maintenance
        else {
            panic!("ambiguous removal retained")
        };
        let prepared = decode_indexes(&prepared);
        assert_eq!(prepared.current_index, Some(9));
        assert_eq!(prepared.valid_indexes, vec![0, 2]);
        let client = wire.client.clone();
        let resume = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .resume_maintenance(&"s.whatsapp.net".parse().unwrap())
                .await
        });
        let request = wire.request(4).await;
        wire.alias_devices(request.get(), &signed, &[], &[(3, 2)], 100)
            .await;
        // Another companion arrives after removal: publication must pause for an explicit re-sign.
        let request = wire.request(5).await;
        wire.alias_devices(request.get(), &changed, &[], &[(3, 2), (4, 10)], 101)
            .await;
        assert!(resume.await.unwrap().is_err());
        let (_, removed, _) = wire.client.mobile_primary().unwrap().state().unwrap();
        assert_eq!(removed.high_water, 9);
        assert_eq!(removed.companions.len(), 1);
        assert_eq!(removed.companions[0].device_jid(), &other);
        assert_eq!(removed.abandoned_dispatches.len(), 1);
        assert!(dispatch_for_key_index(&removed.abandoned_dispatches[0], 2));
        assert!(matches!(
            removed.maintenance,
            Some(Maintenance::Revoke {
                removed: true,
                key_index: 1,
                ..
            })
        ));
        let client = wire.client.clone();
        let refresh = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .refresh_maintenance_indexes(102, &mut rand::rngs::StdRng::seed_from_u64(203))
                .await
        });
        let request = wire.request(6).await;
        wire.alias_devices(request.get(), &changed, &[], &[(3, 2), (4, 10)], 101)
            .await;
        refresh.await.unwrap().unwrap();
        let client = wire.client.clone();
        let resume = tokio::spawn(async move {
            client
                .mobile_primary()
                .unwrap()
                .resume_maintenance(&"s.whatsapp.net".parse().unwrap())
                .await
        });
        let request = wire.request(7).await;
        wire.alias_devices(request.get(), &changed, &[], &[(3, 2), (4, 10)], 101)
            .await;
        let request = wire.request(8).await;
        let published = request
            .get()
            .get_optional_child("key-index-list")
            .unwrap()
            .content_bytes()
            .unwrap()
            .to_vec();
        let details = decode_indexes(&published);
        assert_eq!(details.current_index, Some(10));
        assert_eq!(details.valid_indexes, vec![0, 2, 10]);
        wire.reply(request.get(), vec![], false).await;
        resume.await.unwrap().unwrap();
        let client = wire.client.clone();
        let reconcile =
            tokio::spawn(async move { client.mobile_primary().unwrap().reconcile().await });
        let request = wire.request(9).await;
        wire.alias_devices(request.get(), &published, &[], &[(3, 2), (4, 10)], 102)
            .await;
        reconcile.await.unwrap().unwrap();
        let (_, complete, _) = wire.client.mobile_primary().unwrap().state().unwrap();
        assert_eq!(complete.high_water, 10);
        assert_eq!(complete.live_indexes, vec![0, 2, 10]);
        assert_eq!(complete.companions.len(), 1);
        assert_eq!(complete.companions[0].device_jid(), &other);
        assert!(complete.maintenance.is_none());
        assert_eq!(
            wire.transport.sent_count(),
            10,
            "resume must not repeat the acknowledged or already-absent remove"
        );
        wire.close().await;
    }
}
