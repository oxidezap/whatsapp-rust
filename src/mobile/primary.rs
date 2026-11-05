//! Primary companion hosting through the existing IQ, Signal and persistence paths.
use crate::Client;
use buffa::Enumeration as _;
use rand::{CryptoRng, Rng};
use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};
use std::io::Write as _;
use subtle::ConstantTimeEq;
use wacore::mobile::primary::*;
use wacore::store::MobileSessionBinding;
use wacore::types::jid::JidExt as _;
use wacore_binary::{Jid, JidExt as _};
use waproto::whatsapp as wa;
use zeroize::Zeroizing;

const MAX_JOURNAL_BYTES: usize = 64 * 1024;
const MAX_TRACKED_COMPANIONS: usize = 128;

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PrimaryManagementError {
    #[error("invalid primary operation: {0}")]
    Invalid(&'static str),
    #[error(transparent)]
    Crypto(#[from] PrimaryLinkError),
    #[error(transparent)]
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
    pub fn bootstrap_complete(&self) -> bool {
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
        payload_hash: [u8; 32],
    },
    History {
        device: Jid,
        cursor: HistoryCursor,
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
}
impl PrimaryJournal {
    fn validate(&self) -> Result<(), PrimaryManagementError> {
        if self.schema != 1
            || self.companions.len() > MAX_TRACKED_COMPANIONS
            || self.live_indexes.is_empty()
            || self.live_indexes.len() > 4096
            || !self.live_indexes.contains(&0)
            || self.live_indexes.iter().any(|v| *v > self.high_water)
        {
            return Err(PrimaryManagementError::Invalid(
                "journal schema or capacity",
            ));
        }
        let mut indexes = std::collections::BTreeSet::new();
        let mut devices = std::collections::BTreeSet::new();
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
    devices: Vec<(Jid, u32)>,
    live_indexes: Vec<u32>,
    high_water: u32,
    key_index_timestamp: u64,
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
        let (binding, mut journal, _) = self.state()?;
        if journal.pending.is_some()
            || journal.maintenance.is_some()
            || journal.companions.len() >= MAX_TRACKED_COMPANIONS
        {
            return Err(PrimaryManagementError::Invalid(
                "pending operation or capacity",
            ));
        }
        if timestamp_seconds < observed.key_index_timestamp {
            return Err(PrimaryManagementError::Invalid(
                "key-index timestamp regressed",
            ));
        }
        journal.live_indexes = observed.live_indexes;
        journal.high_water = observed.high_water;
        journal.key_index_timestamp = timestamp_seconds;
        let mut indexes: Vec<u32> = journal
            .companions
            .iter()
            .map(|v| v.key_index)
            .chain(journal.live_indexes.iter().copied())
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
        let identity = snapshot
            .identity_key
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| PrimaryManagementError::Invalid("primary identity"))?;
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
        journal.live_indexes = observed.live_indexes;
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
        let primary: &[u8; 32] = snapshot
            .identity_key
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| PrimaryManagementError::Invalid("primary identity"))?;
        let mut devices = Vec::new();
        let mut live_indexes = vec![0];
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
        Ok(PrimaryDeviceSnapshot {
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
        let (binding, mut journal, _) = self.state()?;
        if journal.pending.is_some() || journal.maintenance.is_some() {
            return Err(PrimaryManagementError::Invalid(
                "active operation must be recovered first",
            ));
        }
        journal.companions.retain(|v| {
            observed
                .devices
                .iter()
                .any(|(jid, index)| jid == &v.device_jid && *index == v.key_index)
        });
        journal.live_indexes = observed.live_indexes;
        journal.high_water = observed.high_water;
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
    pub async fn revoke(
        &self,
        server: &Jid,
        device: &Jid,
        reason: &str,
        timestamp_seconds: u64,
        rng: &mut (impl Rng + CryptoRng),
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
        journal.live_indexes = observed.live_indexes;
        journal.high_water = observed.high_water;
        journal.key_index_timestamp = timestamp_seconds;
        if journal.pending.is_some()
            || journal.maintenance.is_some()
            || !journal.companions.iter().any(|v| &v.device_jid == device)
        {
            return Err(PrimaryManagementError::Invalid(
                "untracked device or active operation",
            ));
        }
        RemovePrimaryCompanionSpec::new(server, &accounts, device, reason)?;
        let signed = self.sign_indexes(&journal, Some(device), timestamp_seconds, rng)?;
        let key_index = journal
            .companions
            .iter()
            .find(|v| &v.device_jid == device)
            .ok_or(PrimaryManagementError::Invalid("untracked companion"))?
            .key_index;
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
        rng: &mut (impl Rng + CryptoRng),
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
        journal.live_indexes = observed.live_indexes;
        journal.high_water = observed.high_water;
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
        excluding: Option<&Jid>,
        timestamp_seconds: u64,
        rng: &mut (impl Rng + CryptoRng),
    ) -> Result<Vec<u8>, PrimaryManagementError> {
        let indexes: Vec<_> = std::iter::once(0)
            .chain(journal.live_indexes.iter().copied().filter(|index| {
                excluding.is_none_or(|jid| {
                    !journal
                        .companions
                        .iter()
                        .any(|v| &v.device_jid == jid && v.key_index == *index)
                })
            }))
            .chain(
                journal
                    .companions
                    .iter()
                    .filter(|v| Some(&v.device_jid) != excluding)
                    .map(|v| v.key_index),
            )
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
        rng: &mut (impl Rng + CryptoRng),
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
        journal.live_indexes = observed.live_indexes;
        journal.high_water = observed.high_water;
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
        self.resume_maintenance_inner(server).await
    }
    /// Re-sign a failed maintenance publication against a new authenticated snapshot.
    pub async fn refresh_maintenance_indexes(
        &self,
        timestamp_seconds: u64,
        rng: &mut (impl Rng + CryptoRng),
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let fresh = self.query_current_devices().await?;
        let (binding, mut journal, _) = self.state()?;
        if timestamp_seconds < fresh.key_index_timestamp {
            return Err(PrimaryManagementError::Invalid(
                "key-index timestamp regressed",
            ));
        }
        journal.live_indexes = fresh.live_indexes;
        journal.high_water = fresh.high_water;
        journal.key_index_timestamp = timestamp_seconds;
        let operation = journal
            .maintenance
            .clone()
            .ok_or(PrimaryManagementError::Invalid("no maintenance operation"))?;
        let excluding = match &operation {
            Maintenance::Revoke {
                device, key_index, ..
            } => {
                if fresh
                    .devices
                    .iter()
                    .any(|(jid, index)| jid == device && index != key_index)
                {
                    return Err(PrimaryManagementError::Invalid("companion address reused"));
                }
                Some(device)
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
                    if let Some((_, current_index)) =
                        fresh.devices.iter().find(|(jid, _)| jid == &device)
                    {
                        if *current_index != key_index {
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
                    let removed_index = latest
                        .companions
                        .iter()
                        .find(|v| v.device_jid == device)
                        .map(|v| v.key_index);
                    latest.companions.retain(|v| v.device_jid != device);
                    if let Some(index) = removed_index {
                        latest.live_indexes.retain(|v| *v != index);
                    }
                    self.save(binding, &latest).await?;
                }
                (signed, timestamp)
            }
        };
        let fresh = self.query_current_devices().await?;
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let primary = snapshot
            .identity_key
            .public_key
            .public_key_bytes()
            .try_into()
            .map_err(|_| PrimaryManagementError::Invalid("primary identity"))?;
        let expected = wacore::adv::verify_key_index_list(&signed, primary).ok_or(
            PrimaryManagementError::Invalid("maintenance index signature"),
        )?;
        let mut expected_live = expected.valid_indexes;
        expected_live.sort_unstable();
        expected_live.dedup();
        if fresh.high_water > expected.current_index
            || fresh.key_index_timestamp > timestamp
            || fresh.live_indexes != expected_live
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
        self.bootstrap_with_media_key(device, history, None).await
    }
    /// Large bootstrap uploads require an explicit host-owned media key.
    pub async fn bootstrap_with_media_key(
        &self,
        device: &Jid,
        history: &wa::HistorySync,
        media_key: Option<[u8; 32]>,
    ) -> Result<(), PrimaryManagementError> {
        let _operation_guard = self
            .client
            .persistence_manager
            .mobile_primary_operation_guard()
            .await;
        let history_hash: [u8; 32] =
            Sha256::digest(waproto::codec::history_sync_to_vec(history)).into();
        let (_, journal, accounts) = self.state()?;
        validate_owned(device, &accounts)?;
        let record = journal
            .companions
            .iter()
            .find(|v| &v.device_jid == device)
            .ok_or(PrimaryManagementError::Invalid("unconfirmed device"))?;
        if journal.pending.is_some() || journal.maintenance.as_ref().is_some_and(|op| !matches!(op, Maintenance::Bootstrap { device: reserved, payload_hash } if reserved == device && payload_hash == &history_hash)) {
            return Err(PrimaryManagementError::Invalid("another operation is active"));
        }
        let expected_identity = record.peer_identity;
        let (binding, mut reserved, _) = self.state()?;
        if reserved.maintenance.is_none() {
            reserved.maintenance = Some(Maintenance::Bootstrap {
                device: device.clone(),
                payload_hash: history_hash,
            });
            self.save(binding, &reserved).await?;
        }
        self.client
            .ensure_e2e_sessions(std::slice::from_ref(device))
            .await
            .map_err(PrimaryManagementError::protocol)?;
        let snapshot = self.client.persistence_manager.get_device_snapshot();
        let identity = snapshot
            .backend
            .load_identity(device.to_protocol_address().as_str())
            .await?
            .ok_or(PrimaryManagementError::Invalid(
                "missing companion Signal identity",
            ))?;
        if !bool::from(identity.as_slice().ct_eq(&expected_identity)) {
            return Err(PrimaryManagementError::Invalid(
                "companion Signal identity mismatch",
            ));
        }
        if !record.history_sent {
            let message = self.build_history_message(history, media_key).await?;
            self.send_peer(device, &message).await?;
            self.mark_bootstrap(device, true).await?;
        }
        if !record.app_state_sent {
            let session = self
                .client
                .mobile_session
                .as_ref()
                .ok_or(PrimaryManagementError::Invalid("mobile session"))?;
            let id = self
                .client
                .ensure_primary_app_state_key(session.app_state_key())
                .await?;
            let stored = snapshot
                .backend
                .get_sync_key(&id)
                .await?
                .ok_or(PrimaryManagementError::Invalid("app-state key missing"))?;
            let fingerprint =
                waproto::codec::app_state_sync_key_fingerprint_decode(&stored.fingerprint)
                    .map_err(|_| PrimaryManagementError::Invalid("app-state fingerprint"))?;
            let mut key_id = wa::message::AppStateSyncKeyId::default();
            key_id.key_id = Some(id);
            let mut key_data = wa::message::AppStateSyncKeyData::default();
            key_data.key_data = Some(stored.key_data);
            key_data.fingerprint = buffa::MessageField::some(fingerprint);
            key_data.timestamp = Some(stored.timestamp);
            let mut key = wa::message::AppStateSyncKey::default();
            key.key_id = buffa::MessageField::some(key_id);
            key.key_data = buffa::MessageField::some(key_data);
            let mut share = wa::message::AppStateSyncKeyShare::default();
            share.keys.push(key);
            let mut protocol = wa::message::ProtocolMessage::default();
            protocol.r#type = Some(wa::message::protocol_message::Type::AppStateSyncKeyShare);
            protocol.app_state_sync_key_share = buffa::MessageField::some(share);
            let mut message = wa::Message::default();
            message.protocol_message = buffa::MessageField::some(protocol);
            self.send_peer(device, &message).await?;
            self.mark_bootstrap(device, false).await?;
        }
        let (binding, mut latest, _) = self.state()?;
        latest.maintenance = None;
        self.save(binding, &latest).await?;
        Ok(())
    }
    async fn build_history_message(
        &self,
        history: &wa::HistorySync,
        media_key: Option<[u8; 32]>,
    ) -> Result<wa::Message, PrimaryManagementError> {
        if history.sync_type != wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP
            || history.progress != Some(100)
            || history.chunk_order != Some(0)
        {
            return Err(PrimaryManagementError::Invalid(
                "initial bootstrap metadata",
            ));
        }
        let bytes = waproto::codec::history_sync_to_vec(history);
        if bytes.len() <= 64 * 1024 {
            return build_history_bootstrap(history);
        }
        if bytes.len() > 16 * 1024 * 1024 {
            return Err(PrimaryManagementError::Invalid(
                "history chunk capacity; use streaming upload",
            ));
        }
        let media_key = media_key.ok_or(PrimaryManagementError::Invalid(
            "external history media key required",
        ))?;
        if media_key == [0; 32] {
            return Err(PrimaryManagementError::Invalid("history media key"));
        }
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
        Ok(external_history_message(
            &PrimaryHistoryChunk {
                sync_type: wa::message::HistorySyncType::INITIAL_BOOTSTRAP,
                chunk_order: 0,
                progress: 100,
            },
            &upload,
        ))
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
        if journal.pending.is_some() || journal.maintenance.as_ref().is_some_and(|v| !matches!(v, Maintenance::History { device: reserved, cursor: expected } if reserved == device && expected.sync_type == cursor.sync_type && expected.chunk_order == cursor.chunk_order && expected.progress == cursor.progress && expected.file_sha256 == cursor.file_sha256)) { return Err(PrimaryManagementError::Invalid("another operation is active")); }
        journal.maintenance = Some(Maintenance::History {
            device: device.clone(),
            cursor: cursor.clone(),
        });
        self.save(binding, &journal).await?;
        self.ensure_companion_identity(device, &record.peer_identity)
            .await?;
        self.send_peer(device, &external_history_message(&metadata, upload))
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
        let identity = snapshot
            .backend
            .load_identity(device.to_protocol_address().as_str())
            .await?
            .ok_or(PrimaryManagementError::Invalid(
                "missing companion Signal identity",
            ))?;
        if !bool::from(identity.ct_eq(expected)) {
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
    ) -> Result<(), PrimaryManagementError> {
        self.client
            .send_message_impl(
                device.clone(),
                message,
                crate::send::SendPipelineOptions {
                    peer: true,
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
fn build_history_bootstrap(
    history: &wa::HistorySync,
) -> Result<wa::Message, PrimaryManagementError> {
    if history.sync_type != wa::history_sync::HistorySyncType::INITIAL_BOOTSTRAP {
        return Err(PrimaryManagementError::Invalid(
            "initial history bootstrap type",
        ));
    }
    let bytes = waproto::codec::history_sync_to_vec(history);
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
    protocol.history_sync_notification = buffa::MessageField::some(notification);
    let mut message = wa::Message::default();
    message.protocol_message = buffa::MessageField::some(protocol);
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
    protocol.history_sync_notification = buffa::MessageField::some(notification);
    let mut message = wa::Message::default();
    message.protocol_message = buffa::MessageField::some(protocol);
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
            use wacore_binary::builder::NodeBuilder;
            let mut entries = vec![NodeBuilder::new("device").attr("id", "0").build()];
            let mut children = vec![];
            if let Some(signed) = signed {
                entries.push(
                    NodeBuilder::new("device")
                        .attr("id", "2")
                        .attr("key-index", "1")
                        .build(),
                );
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
        let backend = std::sync::Arc::new(wacore::store::InMemoryBackend::new());
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
}
