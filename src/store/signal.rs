use crate::store::Device;
use async_lock::Mutex;
use async_trait::async_trait;
use rand::RngExt;
use std::sync::{Arc, OnceLock};
use wacore::libsignal::protocol::error::Result as SignalResult;
use wacore::libsignal::protocol::{
    Direction, IdentityChange, IdentityKey, IdentityKeyPair, IdentityKeyStore, PrivateKey,
    ProtocolAddress, PublicKey, SenderKeyRecord, SenderKeyStore, SessionRecord,
    SignalProtocolError,
};
use wacore::libsignal::store::sender_key_name::SenderKeyName;
use wacore::libsignal::store::*;
use waproto::whatsapp::{PreKeyRecordStructure, SignedPreKeyRecordStructure};

type StoreError = Box<dyn std::error::Error + Send + Sync>;

type DirectStoreIncarnation = [u8; 16];

// Synchronous direct writes make process restart the only unsafe reload boundary.
fn direct_store_incarnation() -> &'static DirectStoreIncarnation {
    static INCARNATION: OnceLock<DirectStoreIncarnation> = OnceLock::new();
    INCARNATION.get_or_init(|| {
        let mut incarnation = [0; 16];
        rand::make_rng::<rand::rngs::StdRng>().fill(&mut incarnation);
        incarnation
    })
}

macro_rules! impl_store_wrapper {
    ($wrapper_ty:ty, $read_lock:ident, $write_lock:ident) => {
        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl IdentityKeyStore for $wrapper_ty {
            async fn get_identity_key_pair(&self) -> SignalResult<IdentityKeyPair> {
                self.0.$read_lock().await.get_identity_key_pair().await
            }

            async fn get_local_registration_id(&self) -> SignalResult<u32> {
                self.0.$read_lock().await.get_local_registration_id().await
            }

            async fn save_identity(
                &mut self,
                address: &ProtocolAddress,
                identity_key: &IdentityKey,
            ) -> SignalResult<IdentityChange> {
                self.0
                    .$write_lock()
                    .await
                    .save_identity(address, identity_key)
                    .await
            }

            async fn is_trusted_identity(
                &self,
                address: &ProtocolAddress,
                identity_key: &IdentityKey,
                direction: Direction,
            ) -> SignalResult<bool> {
                self.0
                    .$read_lock()
                    .await
                    .is_trusted_identity(address, identity_key, direction)
                    .await
            }

            async fn get_identity(
                &self,
                address: &ProtocolAddress,
            ) -> SignalResult<Option<IdentityKey>> {
                self.0.$read_lock().await.get_identity(address).await
            }
        }

        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl PreKeyStore for $wrapper_ty {
            async fn load_prekey(
                &self,
                prekey_id: u32,
            ) -> Result<Option<PreKeyRecordStructure>, StoreError> {
                self.0.$read_lock().await.load_prekey(prekey_id).await
            }

            async fn store_prekey(
                &self,
                prekey_id: u32,
                record: PreKeyRecordStructure,
                uploaded: bool,
            ) -> Result<(), StoreError> {
                self.0
                    .$write_lock()
                    .await
                    .store_prekey(prekey_id, record, uploaded)
                    .await
            }

            async fn contains_prekey(&self, prekey_id: u32) -> Result<bool, StoreError> {
                self.0.$read_lock().await.contains_prekey(prekey_id).await
            }

            async fn remove_prekey(&self, prekey_id: u32) -> Result<(), StoreError> {
                self.0.$write_lock().await.remove_prekey(prekey_id).await
            }
        }

        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl SignedPreKeyStore for $wrapper_ty {
            async fn load_signed_prekey(
                &self,
                signed_prekey_id: u32,
            ) -> Result<Option<SignedPreKeyRecordStructure>, StoreError> {
                self.0
                    .$read_lock()
                    .await
                    .load_signed_prekey(signed_prekey_id)
                    .await
            }

            async fn load_signed_prekeys(
                &self,
            ) -> Result<Vec<SignedPreKeyRecordStructure>, StoreError> {
                self.0.$read_lock().await.load_signed_prekeys().await
            }

            async fn store_signed_prekey(
                &self,
                signed_prekey_id: u32,
                record: SignedPreKeyRecordStructure,
            ) -> Result<(), StoreError> {
                self.0
                    .$write_lock()
                    .await
                    .store_signed_prekey(signed_prekey_id, record)
                    .await
            }

            async fn contains_signed_prekey(
                &self,
                signed_prekey_id: u32,
            ) -> Result<bool, StoreError> {
                self.0
                    .$read_lock()
                    .await
                    .contains_signed_prekey(signed_prekey_id)
                    .await
            }

            async fn remove_signed_prekey(&self, signed_prekey_id: u32) -> Result<(), StoreError> {
                self.0
                    .$write_lock()
                    .await
                    .remove_signed_prekey(signed_prekey_id)
                    .await
            }
        }

        #[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
        #[cfg_attr(not(target_arch = "wasm32"), async_trait)]
        impl SessionStore for $wrapper_ty {
            async fn load_session(
                &self,
                address: &ProtocolAddress,
            ) -> Result<SessionRecord, StoreError> {
                self.0.$read_lock().await.load_session(address).await
            }

            async fn get_sub_device_sessions(&self, name: &str) -> Result<Vec<u32>, StoreError> {
                self.0
                    .$read_lock()
                    .await
                    .get_sub_device_sessions(name)
                    .await
            }

            async fn store_session(
                &self,
                address: &ProtocolAddress,
                record: &SessionRecord,
            ) -> Result<(), StoreError> {
                self.0
                    .$write_lock()
                    .await
                    .store_session(address, record)
                    .await
            }

            async fn contains_session(
                &self,
                address: &ProtocolAddress,
            ) -> Result<bool, StoreError> {
                self.0.$read_lock().await.contains_session(address).await
            }

            async fn delete_session(&self, address: &ProtocolAddress) -> Result<(), StoreError> {
                self.0.$write_lock().await.delete_session(address).await
            }

            async fn delete_all_sessions(&self, name: &str) -> Result<(), StoreError> {
                self.0.$write_lock().await.delete_all_sessions(name).await
            }
        }
    };
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl IdentityKeyStore for Device {
    async fn get_identity_key_pair(&self) -> SignalResult<IdentityKeyPair> {
        Ok(self.identity_key.clone().into())
    }

    async fn get_local_registration_id(&self) -> SignalResult<u32> {
        Ok(self.registration_id)
    }

    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity_key: &IdentityKey,
    ) -> SignalResult<IdentityChange> {
        let address_str = address.as_str();
        let key_bytes = identity_key.public_key().public_key_bytes();
        let existing_identity_opt = self.get_identity(address).await?;

        self.backend
            .put_identity(
                address_str,
                key_bytes.try_into().map_err(|_| {
                    SignalProtocolError::InvalidArgument("Invalid key length".into())
                })?,
            )
            .await
            .map_err(|e| SignalProtocolError::BackendError("backend put_identity", Box::new(e)))?;

        match existing_identity_opt {
            None => Ok(IdentityChange::NewOrUnchanged),
            Some(existing) if &existing == identity_key => Ok(IdentityChange::NewOrUnchanged),
            Some(_) => Ok(IdentityChange::ReplacedExisting),
        }
    }

    async fn is_trusted_identity(
        &self,
        _address: &ProtocolAddress,
        _identity_key: &IdentityKey,
        _direction: Direction,
    ) -> SignalResult<bool> {
        // WA Web: ProtocolStoreUnifiedApi.js — isTrustedIdentity always returns true.
        // Identity changes are handled in save_identity (safety number change
        // notification), not by rejecting messages.
        Ok(true)
    }

    async fn get_identity(&self, address: &ProtocolAddress) -> SignalResult<Option<IdentityKey>> {
        let identity_bytes = self
            .backend
            .load_identity(address.as_str())
            .await
            .map_err(|e| SignalProtocolError::BackendError("backend get_identity", Box::new(e)))?;

        match identity_bytes {
            Some(bytes) if !bytes.is_empty() => {
                let public_key = PublicKey::from_djb_public_key_bytes(&bytes)?;
                Ok(Some(IdentityKey::new(public_key)))
            }
            _ => Ok(None),
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PreKeyStore for Device {
    async fn load_prekey(
        &self,
        prekey_id: u32,
    ) -> Result<Option<PreKeyRecordStructure>, StoreError> {
        use wacore::libsignal::protocol::KeyPair;
        use wacore::libsignal::store::record_helpers::new_pre_key_record;

        match self.backend.load_prekey(prekey_id).await {
            Ok(Some(bytes)) => {
                // Try new format first (protobuf-encoded PreKeyRecordStructure)
                if let Ok(record) = waproto::codec::pre_key_record_decode(&bytes) {
                    return Ok(Some(record));
                }

                // Fallback: old format stored just the private key bytes (32 bytes)
                // Reconstruct the full record by deriving the public key
                if let Ok(private_key) = PrivateKey::deserialize(&bytes)
                    && let Ok(public_key) = private_key.public_key()
                {
                    let key_pair = KeyPair::new(public_key, private_key);
                    let record = new_pre_key_record(prekey_id, &key_pair);
                    return Ok(Some(record));
                }

                // Could not decode in either format
                Ok(None)
            }
            Ok(None) => Ok(None),
            Err(e) => Err(Box::new(e) as StoreError),
        }
    }

    async fn store_prekey(
        &self,
        prekey_id: u32,
        record: PreKeyRecordStructure,
        uploaded: bool,
    ) -> Result<(), StoreError> {
        let bytes = waproto::codec::pre_key_record_to_vec(&record);
        self.backend
            .store_prekey(prekey_id, &bytes, uploaded)
            .await
            .map_err(|e| Box::new(e) as StoreError)
    }

    async fn contains_prekey(&self, prekey_id: u32) -> Result<bool, StoreError> {
        match self.backend.load_prekey(prekey_id).await {
            Ok(opt) => Ok(opt.is_some()),
            Err(e) => Err(Box::new(e) as StoreError),
        }
    }

    async fn remove_prekey(&self, prekey_id: u32) -> Result<(), StoreError> {
        self.backend
            .remove_prekey(prekey_id)
            .await
            .map_err(|e| Box::new(e) as StoreError)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SignedPreKeyStore for Device {
    async fn load_signed_prekey(
        &self,
        signed_prekey_id: u32,
    ) -> Result<Option<SignedPreKeyRecordStructure>, StoreError> {
        if signed_prekey_id == self.signed_pre_key_id {
            let record = wacore::libsignal::store::record_helpers::new_signed_pre_key_record(
                self.signed_pre_key_id,
                &self.signed_pre_key,
                self.signed_pre_key_signature,
                wacore::time::now_utc(),
            );
            return Ok(Some(record));
        }
        // Rotated-out key: a prekey message minted against a previous signed
        // pre-key still names its old id, so fall back to the retained records.
        match self
            .backend
            .load_signed_prekey(signed_prekey_id)
            .await
            .map_err(|e| Box::new(e) as StoreError)?
        {
            Some(bytes) => {
                let record = waproto::codec::signed_pre_key_record_decode(&bytes)
                    .map_err(|e| Box::new(e) as StoreError)?;
                Ok(Some(record))
            }
            None => Ok(None),
        }
    }

    async fn load_signed_prekeys(&self) -> Result<Vec<SignedPreKeyRecordStructure>, StoreError> {
        log::warn!(
            "Device: load_signed_prekeys() - returning empty list. Only the device's own signed pre-key should be accessed via load_signed_prekey()."
        );
        Ok(Vec::new())
    }

    async fn store_signed_prekey(
        &self,
        signed_prekey_id: u32,
        _record: SignedPreKeyRecordStructure,
    ) -> Result<(), StoreError> {
        log::warn!(
            "Device: store_signed_prekey({}) - no-op. Signed pre-keys should only be set once during device creation/pairing and managed via PersistenceManager.",
            signed_prekey_id
        );
        Ok(())
    }

    async fn contains_signed_prekey(&self, signed_prekey_id: u32) -> Result<bool, StoreError> {
        if signed_prekey_id == self.signed_pre_key_id {
            return Ok(true);
        }
        // Stay consistent with load_signed_prekey: a rotated-out key retained in
        // the backend table must not read as absent, or a gated load would drop
        // a still-decryptable prekey message.
        Ok(self
            .backend
            .load_signed_prekey(signed_prekey_id)
            .await
            .map_err(|e| Box::new(e) as StoreError)?
            .is_some())
    }

    async fn remove_signed_prekey(&self, signed_prekey_id: u32) -> Result<(), StoreError> {
        log::warn!(
            "Device: remove_signed_prekey({}) - no-op. Signed pre-keys are managed via PersistenceManager and should not be removed individually.",
            signed_prekey_id
        );
        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SessionStore for Device {
    async fn load_session(&self, address: &ProtocolAddress) -> Result<SessionRecord, StoreError> {
        let address_str = address.as_str();
        match self.backend.get_session(address_str).await {
            Ok(Some(session_data)) => {
                SessionRecord::deserialize_for_store(&session_data, direct_store_incarnation())
                    .map_err(|e| Box::new(e) as StoreError)
            }
            Ok(None) => Ok(SessionRecord::new_fresh()),
            Err(e) => Err(Box::new(e) as StoreError),
        }
    }

    async fn get_sub_device_sessions(&self, name: &str) -> Result<Vec<u32>, StoreError> {
        let _ = name;
        Ok(Vec::new())
    }

    async fn store_session(
        &self,
        address: &ProtocolAddress,
        record: &SessionRecord,
    ) -> Result<(), StoreError> {
        let address_str = address.as_str();
        let mut session_data = Vec::new();
        record.serialize_into_for_store(&mut session_data, direct_store_incarnation());

        self.backend
            .put_session(address_str, &session_data)
            .await
            .map_err(|e| Box::new(e) as StoreError)
    }

    async fn contains_session(&self, address: &ProtocolAddress) -> Result<bool, StoreError> {
        let address_str = address.as_str();
        self.backend
            .has_session(address_str)
            .await
            .map_err(|e| Box::new(e) as StoreError)
    }

    async fn delete_session(&self, address: &ProtocolAddress) -> Result<(), StoreError> {
        let address_str = address.as_str();
        self.backend
            .delete_session(address_str)
            .await
            .map_err(|e| Box::new(e) as StoreError)
    }

    async fn delete_all_sessions(&self, name: &str) -> Result<(), StoreError> {
        let _ = name;
        Ok(())
    }
}

use async_lock::RwLock;

pub struct DeviceRwLockWrapper(pub Arc<RwLock<Device>>);

impl DeviceRwLockWrapper {
    pub fn new(device: Arc<RwLock<Device>>) -> Self {
        Self(device)
    }
}

impl Clone for DeviceRwLockWrapper {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl_store_wrapper!(DeviceRwLockWrapper, read, write);

pub struct DeviceStore(pub Arc<Mutex<Device>>);

impl DeviceStore {
    pub fn new(device: Arc<Mutex<Device>>) -> Self {
        Self(device)
    }
}

impl Clone for DeviceStore {
    fn clone(&self) -> Self {
        Self(self.0.clone())
    }
}

impl_store_wrapper!(DeviceStore, lock, lock);

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SenderKeyStore for Device {
    async fn store_sender_key(
        &mut self,
        sender_key_name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> SignalResult<()> {
        let serialized_record = record.serialize_for_store(direct_store_incarnation())?;
        self.backend
            .put_sender_key(sender_key_name.cache_key(), &serialized_record)
            .await
            .map_err(|e| SignalProtocolError::BackendError("store_sender_key", Box::new(e)))
    }

    async fn load_sender_key(
        &self,
        sender_key_name: &SenderKeyName,
    ) -> SignalResult<Option<SenderKeyRecord>> {
        match self
            .backend
            .get_sender_key(sender_key_name.cache_key())
            .await
            .map_err(|e| SignalProtocolError::BackendError("load_sender_key", Box::new(e)))?
        {
            Some(data) => {
                let record =
                    SenderKeyRecord::deserialize_for_store(&data, direct_store_incarnation())?;
                if record.is_empty() {
                    Ok(None)
                } else {
                    Ok(Some(record))
                }
            }
            None => Ok(None),
        }
    }
}

#[cfg(test)]
#[allow(clippy::disallowed_methods)]
mod tests {
    use super::*;

    fn leased_session() -> SessionRecord {
        use wacore::libsignal::protocol::{ChainKey, KeyPair, RootKey, SessionState};

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let local = IdentityKey::new(KeyPair::generate(&mut rng).public_key);
        let remote = IdentityKey::new(KeyPair::generate(&mut rng).public_key);
        let base_key = KeyPair::generate(&mut rng).public_key;
        let mut state = SessionState::new(3, &local, &remote, &RootKey::new([0; 32]), &base_key);
        state.set_sender_chain(&KeyPair::generate(&mut rng), &ChainKey::new([1; 32], 0));
        let mut record = SessionRecord::new(state);
        record.reserve_sender_chain_counters(0);
        record
    }

    fn session_chain_index(record: &SessionRecord) -> u32 {
        record
            .session_state()
            .expect("session")
            .get_sender_chain_key()
            .expect("sender chain")
            .index()
    }

    #[tokio::test]
    async fn direct_session_store_preserves_clean_reload_and_recovery_ceiling() {
        let backend = crate::test_utils::create_test_backend().await;
        let device = Device::new(backend.clone());
        let address = ProtocolAddress::new("15550001001".to_string(), 1.into());

        SessionStore::store_session(&device, &address, &leased_session())
            .await
            .expect("store session");
        let clean = SessionStore::load_session(&device, &address)
            .await
            .expect("clean reload");
        assert_eq!(session_chain_index(&clean), 0);

        let replacement = Device::new(backend.clone());
        let same_process = SessionStore::load_session(&replacement, &address)
            .await
            .expect("same-process reload");
        assert_eq!(session_chain_index(&same_process), 0);

        let durable = backend
            .get_session(address.as_str())
            .await
            .expect("read durable session")
            .expect("durable session");
        let recovered = SessionRecord::deserialize(&durable).expect("recovery reload");
        assert_eq!(
            session_chain_index(&recovered),
            wacore::libsignal::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH
        );
    }

    #[tokio::test]
    async fn direct_sender_key_store_preserves_clean_reloads_and_recovery_ceiling() {
        use wacore::libsignal::protocol::{
            create_sender_key_distribution_message, group_decrypt, group_encrypt,
            process_sender_key_distribution_message,
        };

        let sender_backend = crate::test_utils::create_test_backend().await;
        let mut sender = Device::new(sender_backend.clone());
        let mut receiver = Device::new(crate::test_utils::create_test_backend().await);
        let name = SenderKeyName::from_parts("1234567890@g.us", "15550001000@s.whatsapp.net:0");
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let distribution = create_sender_key_distribution_message(&name, &mut sender, &mut rng)
            .await
            .expect("sender setup");
        process_sender_key_distribution_message(&name, &distribution, &mut receiver)
            .await
            .expect("receiver setup");

        let mut last = None;
        for expected_iteration in 0..=32 {
            let message = group_encrypt(&mut sender, &name, b"payload", &mut rng)
                .await
                .expect("group encrypt");
            assert_eq!(message.iteration(), expected_iteration);
            last = Some(message);
        }

        let plaintext = group_decrypt(
            last.expect("last message").serialized(),
            &mut receiver,
            &name,
        )
        .await
        .expect("receiver decrypts after missed messages");
        assert_eq!(plaintext, b"payload");

        let mut replacement = Device::new(sender_backend.clone());
        let same_process = group_encrypt(&mut replacement, &name, b"same-process", &mut rng)
            .await
            .expect("encrypt after same-process replacement");
        assert_eq!(same_process.iteration(), 33);

        let durable = sender_backend
            .get_sender_key(name.cache_key())
            .await
            .expect("read durable sender key")
            .expect("durable sender key");
        let recovered = SenderKeyRecord::deserialize(&durable).expect("recovery reload");
        assert_eq!(
            recovered
                .sender_key_state()
                .expect("sender-key state")
                .sender_chain_key()
                .expect("sender chain")
                .iteration(),
            wacore::libsignal::protocol::consts::SENDER_CHAIN_RESERVATION_BATCH
        );
    }

    // A rotated-out signed pre-key (id != current field) must load from the
    // backend table so delayed prekey messages naming the old id still decrypt.
    #[tokio::test]
    async fn load_signed_prekey_falls_back_to_backend_for_rotated_out_id() {
        use buffa::Message;

        let backend = crate::test_utils::create_test_backend().await;
        let device = Device::new(backend.clone());

        let current = device.signed_pre_key_id;
        let old_id = current + 7; // any non-current id

        let kp = wacore::libsignal::protocol::KeyPair::generate(&mut rand::make_rng::<
            rand::rngs::StdRng,
        >());
        let record = wacore::libsignal::store::record_helpers::new_signed_pre_key_record(
            old_id,
            &kp,
            [9u8; 64],
            wacore::time::now_utc(),
        );
        backend
            .store_signed_prekey(old_id, &record.encode_to_vec())
            .await
            .expect("store retained signed pre-key");

        let loaded = SignedPreKeyStore::load_signed_prekey(&device, old_id)
            .await
            .expect("load must not error")
            .expect("rotated-out key must load from backend");
        assert_eq!(loaded.id, Some(old_id));
        assert_eq!(
            loaded.public_key.as_deref(),
            Some(kp.public_key.public_key_bytes())
        );

        // A truly-unknown id still returns None.
        let missing = SignedPreKeyStore::load_signed_prekey(&device, current + 999)
            .await
            .expect("load must not error");
        assert!(missing.is_none());

        // contains_signed_prekey must agree with load: current, rotated-out, and
        // unknown ids report present/present/absent respectively.
        assert!(
            SignedPreKeyStore::contains_signed_prekey(&device, current)
                .await
                .expect("contains current")
        );
        assert!(
            SignedPreKeyStore::contains_signed_prekey(&device, old_id)
                .await
                .expect("contains rotated-out")
        );
        assert!(
            !SignedPreKeyStore::contains_signed_prekey(&device, current + 999)
                .await
                .expect("contains unknown")
        );
    }
}
