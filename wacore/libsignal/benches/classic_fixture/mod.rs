// SPDX-License-Identifier: MIT
//! Synthetic workloads through public protocol ports. No timing or old engine dependency.
use async_trait::async_trait;
#[cfg(feature = "bench-internals")]
use buffa::Message as _;
use futures::executor::block_on;
use rand::{SeedableRng, rngs::StdRng};
use std::{
    collections::HashMap,
    fmt,
    hash::{BuildHasherDefault, DefaultHasher},
    hint::black_box,
};
use wacore_libsignal::protocol::*;
#[cfg(feature = "bench-internals")]
mod access;

type Map<K, V> = HashMap<K, V, BuildHasherDefault<DefaultHasher>>;
type Result<T> = std::result::Result<T, SignalProtocolError>;

#[derive(Clone, Copy, Debug)]
pub enum Case {
    Establish,
    #[cfg(feature = "bench-internals")]
    SameIdentity,
    #[cfg(feature = "bench-internals")]
    DifferentIdentity,
    EncryptFirst,
    DecryptFirst,
    EncryptSubsequent,
    DecryptSubsequent,
    DecryptOwnedSubsequent,
    GroupCreate,
    GroupEncrypt,
    GroupDecrypt,
    GroupIngest,
    Conversation,
    SignTen,
    SignAndVerifyTen,
    VerifyTenAfterSigning,
    GenerateTen,
    RejectPreKey256,
    RootUpdate,
    SessionSerialize200,
    SenderSerialize200,
    SenderSerializeEmpty,
    SenderExportShared200,
    ArchiveTen,
    ReverseTwenty,
    PromoteTwenty,
    RetryCurrentTwenty,
    GroupWorst1999,
    GroupNext1999,
    #[cfg(feature = "bench-internals")]
    PreparedRebuildEncrypt,
    #[cfg(feature = "bench-internals")]
    PreparedRebuildDecrypt,
    #[cfg(feature = "bench-internals")]
    ColdLookup2000,
    #[cfg(feature = "bench-internals")]
    ColdLookup4000,
    #[cfg(feature = "bench-internals")]
    ColdLookup8000,
    #[cfg(feature = "bench-internals")]
    TailLookup2000,
    #[cfg(feature = "bench-internals")]
    TailLookup4000,
    #[cfg(feature = "bench-internals")]
    TailLookup8000,
    #[cfg(feature = "bench-internals")]
    ColdLoadLookup2000,
    #[cfg(feature = "bench-internals")]
    ColdLoadLookup4000,
    #[cfg(feature = "bench-internals")]
    ColdLoadLookup8000,
    #[cfg(feature = "bench-internals")]
    EvictTwoHundred,
    #[cfg(feature = "legacy-session-interop")]
    ImportDefault0,
    #[cfg(feature = "legacy-session-interop")]
    ImportDefault2000,
    #[cfg(feature = "legacy-session-interop")]
    ImportBudget2000,
    #[cfg(feature = "legacy-session-interop")]
    ImportBudget4000,
    #[cfg(feature = "legacy-session-interop")]
    ImportBudget8000,
}
pub const CASES: &[Case] = &[
    Case::Establish,
    #[cfg(feature = "bench-internals")]
    Case::SameIdentity,
    #[cfg(feature = "bench-internals")]
    Case::DifferentIdentity,
    Case::EncryptFirst,
    Case::DecryptFirst,
    Case::EncryptSubsequent,
    Case::DecryptSubsequent,
    Case::DecryptOwnedSubsequent,
    Case::GroupCreate,
    Case::GroupEncrypt,
    Case::GroupDecrypt,
    Case::GroupIngest,
    Case::Conversation,
    Case::SignTen,
    Case::SignAndVerifyTen,
    Case::VerifyTenAfterSigning,
    Case::GenerateTen,
    Case::RejectPreKey256,
    Case::RootUpdate,
    Case::SessionSerialize200,
    Case::SenderSerialize200,
    Case::SenderSerializeEmpty,
    Case::SenderExportShared200,
    Case::ArchiveTen,
    Case::ReverseTwenty,
    Case::PromoteTwenty,
    Case::RetryCurrentTwenty,
    Case::GroupWorst1999,
    Case::GroupNext1999,
    #[cfg(feature = "bench-internals")]
    Case::PreparedRebuildEncrypt,
    #[cfg(feature = "bench-internals")]
    Case::PreparedRebuildDecrypt,
    #[cfg(feature = "bench-internals")]
    Case::ColdLookup2000,
    #[cfg(feature = "bench-internals")]
    Case::ColdLookup4000,
    #[cfg(feature = "bench-internals")]
    Case::ColdLookup8000,
    #[cfg(feature = "bench-internals")]
    Case::TailLookup2000,
    #[cfg(feature = "bench-internals")]
    Case::TailLookup4000,
    #[cfg(feature = "bench-internals")]
    Case::TailLookup8000,
    #[cfg(feature = "bench-internals")]
    Case::ColdLoadLookup2000,
    #[cfg(feature = "bench-internals")]
    Case::ColdLoadLookup4000,
    #[cfg(feature = "bench-internals")]
    Case::ColdLoadLookup8000,
    #[cfg(feature = "bench-internals")]
    Case::EvictTwoHundred,
    #[cfg(feature = "legacy-session-interop")]
    Case::ImportDefault0,
    #[cfg(feature = "legacy-session-interop")]
    Case::ImportDefault2000,
    #[cfg(feature = "legacy-session-interop")]
    Case::ImportBudget2000,
    #[cfg(feature = "legacy-session-interop")]
    Case::ImportBudget4000,
    #[cfg(feature = "legacy-session-interop")]
    Case::ImportBudget8000,
];
impl fmt::Display for Case {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{self:?}")
    }
}
fn random() -> StdRng {
    StdRng::seed_from_u64(0x1828_c1a5)
}
fn payload(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| (index.wrapping_mul(13) % 251) as u8)
        .collect()
}
#[derive(Default)]
struct Sessions(Map<ProtocolAddress, SessionRecord>);
#[async_trait]
impl SessionStore for Sessions {
    async fn has_session(&self, address: &ProtocolAddress) -> Result<bool> {
        Ok(self
            .0
            .get(address)
            .is_some_and(|record| record.session_state().is_some()))
    }
    async fn load_session(&self, address: &ProtocolAddress) -> Result<Option<SessionRecord>> {
        Ok(self.0.get(address).cloned())
    }
    async fn store_session(
        &mut self,
        address: &ProtocolAddress,
        record: SessionRecord,
    ) -> Result<()> {
        self.0.insert(address.clone(), record);
        Ok(())
    }
}
struct Identities {
    pair: IdentityKeyPair,
    known: Map<ProtocolAddress, IdentityKey>,
}
#[async_trait]
impl IdentityKeyStore for Identities {
    async fn get_identity_key_pair(&self) -> Result<IdentityKeyPair> {
        Ok(self.pair.clone())
    }
    async fn get_local_registration_id(&self) -> Result<u32> {
        Ok(19)
    }
    async fn save_identity(
        &mut self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
    ) -> Result<IdentityChange> {
        let old = self.known.insert(address.clone(), *identity);
        Ok(if old.is_some_and(|key| key != *identity) {
            IdentityChange::ReplacedExisting
        } else {
            IdentityChange::NewOrUnchanged
        })
    }
    async fn is_trusted_identity(
        &self,
        address: &ProtocolAddress,
        identity: &IdentityKey,
        _: Direction,
    ) -> Result<bool> {
        Ok(self
            .known
            .get(address)
            .is_none_or(|known| known == identity))
    }
    async fn get_identity(&self, address: &ProtocolAddress) -> Result<Option<IdentityKey>> {
        Ok(self.known.get(address).copied())
    }
}
#[derive(Default)]
struct PreKeys(Map<PreKeyId, PreKeyRecord>);
#[async_trait]
impl PreKeyStore for PreKeys {
    async fn get_pre_key(&self, id: PreKeyId) -> Result<PreKeyRecord> {
        self.0
            .get(&id)
            .cloned()
            .ok_or(SignalProtocolError::InvalidPreKeyId)
    }
    async fn save_pre_key(&mut self, id: PreKeyId, record: &PreKeyRecord) -> Result<()> {
        self.0.insert(id, record.clone());
        Ok(())
    }
    async fn remove_pre_key(&mut self, id: PreKeyId) -> Result<()> {
        self.0.remove(&id);
        Ok(())
    }
}
struct SignedKeys(SignedPreKeyRecord);
#[async_trait]
impl SignedPreKeyStore for SignedKeys {
    async fn get_signed_pre_key(&self, id: SignedPreKeyId) -> Result<SignedPreKeyRecord> {
        if id == self.0.id()? {
            Ok(self.0.clone())
        } else {
            Err(SignalProtocolError::InvalidSignedPreKeyId)
        }
    }
    async fn save_signed_pre_key(
        &mut self,
        _: SignedPreKeyId,
        record: &SignedPreKeyRecord,
    ) -> Result<()> {
        self.0 = record.clone();
        Ok(())
    }
}
#[derive(Default)]
struct Groups(Map<SenderKeyName, SenderKeyRecord>);
#[async_trait]
impl SenderKeyStore for Groups {
    async fn load_sender_key(&self, name: &SenderKeyName) -> Result<Option<SenderKeyRecord>> {
        Ok(self.0.get(name).cloned())
    }
    async fn store_sender_key(
        &mut self,
        name: &SenderKeyName,
        record: SenderKeyRecord,
    ) -> Result<()> {
        self.0.insert(name.clone(), record);
        Ok(())
    }
}
#[cfg(feature = "bench-internals")]
struct PreparedGroups {
    components: SenderKeyRecordComponents,
    context: access::CryptoContext,
}
#[cfg(feature = "bench-internals")]
impl PreparedGroups {
    fn new(mut record: SenderKeyRecord) -> Self {
        let context = access::capture_crypto(&mut record);
        Self {
            components: record.into_components().unwrap(),
            context,
        }
    }
}
#[cfg(feature = "bench-internals")]
#[async_trait]
impl SenderKeyStore for PreparedGroups {
    async fn load_sender_key(&self, _: &SenderKeyName) -> Result<Option<SenderKeyRecord>> {
        let mut record = SenderKeyRecord::from_components(self.components.clone())?;
        access::restore_crypto(&mut record, &self.context);
        Ok(Some(record))
    }
    async fn store_sender_key(&mut self, _: &SenderKeyName, record: SenderKeyRecord) -> Result<()> {
        self.components = record.into_components()?;
        Ok(())
    }
}
struct Peer {
    address: ProtocolAddress,
    identities: Identities,
    sessions: Sessions,
    prekeys: PreKeys,
    signed: SignedKeys,
    prekey_id: PreKeyId,
    generation: u64,
}
impl Peer {
    fn new(number: u64) -> Self {
        let mut rng = StdRng::seed_from_u64(0x1828_0000 + number);
        let identity = IdentityKeyPair::generate(&mut rng);
        let signed = KeyPair::generate(&mut rng);
        let prekey = KeyPair::generate(&mut rng);
        let signature = identity
            .private_key()
            .calculate_signature(&signed.public_key.serialize(), &mut rng)
            .unwrap();
        // Use the common stored representation rather than either revision's
        // typed timestamp/constructor signature. This happens outside timing.
        let mut stored = waproto::whatsapp::SignedPreKeyRecordStructure::default();
        stored.id = Some(3);
        stored.public_key = Some(signed.public_key.serialize().to_vec());
        stored.private_key = Some(signed.private_key.serialize().to_vec());
        stored.signature = Some(signature.to_vec());
        stored.timestamp = Some(1);
        let encoded = waproto::codec::signed_pre_key_record_to_vec(&stored);
        let signed = SignedPreKeyRecord::deserialize(&encoded).unwrap();
        let mut prekeys = PreKeys::default();
        prekeys
            .0
            .insert(4.into(), PreKeyRecord::new(4.into(), &prekey));
        Self {
            address: ProtocolAddress::new(&format!("peer{number}.invalid"), 1.into()),
            identities: Identities {
                pair: identity,
                known: Map::default(),
            },
            sessions: Sessions::default(),
            prekeys,
            signed: SignedKeys(signed),
            prekey_id: 4.into(),
            generation: 0,
        }
    }
    fn bundle(&self) -> PreKeyBundle {
        PreKeyBundle::new(
            19,
            1.into(),
            Some((
                self.prekey_id,
                self.prekeys.0[&self.prekey_id].public_key().unwrap(),
            )),
            3.into(),
            self.signed.0.public_key().unwrap(),
            self.signed.0.signature().unwrap(),
            *self.identities.pair.identity_key(),
        )
        .unwrap()
    }
    fn establish(&mut self, other: &Peer) {
        self.generation += 1;
        let mut rng = StdRng::seed_from_u64(0x1828_c1a5 + self.generation);
        block_on(process_prekey_bundle(
            &other.address,
            &mut self.sessions,
            &mut self.identities,
            &other.bundle(),
            &mut rng,
            UsePQRatchet::No,
        ))
        .unwrap();
    }
    fn rotate_prekey(&mut self, generation: u32) {
        let key = KeyPair::generate(&mut StdRng::seed_from_u64(
            0x1828_9000 + u64::from(generation),
        ));
        self.prekey_id = (4 + generation).into();
        self.prekeys
            .0
            .insert(self.prekey_id, PreKeyRecord::new(self.prekey_id, &key));
    }
    fn retry_prekey(&mut self, address: &ProtocolAddress, encoded: &[u8]) -> bool {
        let message =
            CiphertextMessage::PreKeySignalMessage(PreKeySignalMessage::try_from(encoded).unwrap());
        black_box(block_on(message_decrypt(
            &message,
            address,
            &mut self.sessions,
            &mut self.identities,
            &mut self.prekeys,
            &self.signed,
            &mut random(),
            UsePQRatchet::No,
        )))
        .is_ok()
    }
    fn encrypt(&mut self, address: &ProtocolAddress, data: &[u8]) -> Vec<u8> {
        match block_on(message_encrypt(
            data,
            address,
            &mut self.sessions,
            &mut self.identities,
        ))
        .unwrap()
        {
            CiphertextMessage::PreKeySignalMessage(message) => message.serialized().to_vec(),
            CiphertextMessage::SignalMessage(message) => message.serialized().to_vec(),
            _ => panic!("pairwise fixture produced a non-pairwise envelope"),
        }
    }
    fn receive(&mut self, address: &ProtocolAddress, encoded: &[u8], prekey: bool, owned: bool) {
        let message = if prekey {
            CiphertextMessage::PreKeySignalMessage(PreKeySignalMessage::try_from(encoded).unwrap())
        } else {
            CiphertextMessage::SignalMessage(SignalMessage::try_from(encoded).unwrap())
        };
        if owned {
            let mut input = OwnedCiphertextMessage::from(message);
            black_box(
                block_on(message_decrypt_owned(
                    &mut input,
                    address,
                    &mut self.sessions,
                    &mut self.identities,
                    &mut self.prekeys,
                    &self.signed,
                    &mut random(),
                    UsePQRatchet::No,
                ))
                .unwrap(),
            );
        } else {
            black_box(
                block_on(message_decrypt(
                    &message,
                    address,
                    &mut self.sessions,
                    &mut self.identities,
                    &mut self.prekeys,
                    &self.signed,
                    &mut random(),
                    UsePQRatchet::No,
                ))
                .unwrap(),
            );
        }
    }
}
pub struct Fixture {
    alice: Peer,
    bob: Peer,
    sender: Groups,
    receiver: Groups,
    name: SenderKeyName,
    packet: Vec<u8>,
    packets: Vec<Vec<u8>>,
    plaintext: Vec<u8>,
    distribution: Option<SenderKeyDistributionMessage>,
    session: Option<SessionRecord>,
    state: Option<SessionState>,
    sender_record: Option<SenderKeyRecord>,
    shared_record: Option<SenderKeyRecord>,
    signature_key: KeyPair,
    #[cfg(feature = "bench-internals")]
    stored: Vec<u8>,
    #[cfg(feature = "bench-internals")]
    lookup_key: Option<PublicKey>,
    #[cfg(feature = "bench-internals")]
    lookup_index: u32,
    #[cfg(feature = "bench-internals")]
    prepared: Option<PreparedGroups>,
    #[cfg(feature = "legacy-session-interop")]
    legacy: Option<LegacySessionRecordV1>,
}
impl Fixture {
    #[cfg(test)]
    pub fn encoded_input_digest(&self) -> Option<(usize, String)> {
        use sha2::{Digest as _, Sha256};
        let mut hash = Sha256::new();
        let mut length = self.packet.len();
        hash.update(&self.packet);
        for packet in &self.packets {
            length += packet.len();
            hash.update((packet.len() as u64).to_le_bytes());
            hash.update(packet);
        }
        #[cfg(feature = "bench-internals")]
        {
            length += self.stored.len();
            hash.update(&self.stored);
        }
        (length != 0).then(|| (length, hex::encode(hash.finalize())))
    }
    #[cfg(test)]
    pub fn encoded_group_body_digest(&self, case: Case) -> Option<String> {
        use sha2::{Digest as _, Sha256};
        let group = matches!(
            case,
            Case::GroupDecrypt
                | Case::SenderSerialize200
                | Case::GroupWorst1999
                | Case::GroupNext1999
        );
        #[cfg(feature = "bench-internals")]
        let group = group || matches!(case, Case::PreparedRebuildDecrypt);
        group.then(|| hex::encode(Sha256::digest(&self.packet[..self.packet.len() - 64])))
    }
    pub fn new(case: Case) -> Self {
        let mut this = Self {
            alice: Peer::new(1),
            bob: Peer::new(2),
            sender: Groups::default(),
            receiver: Groups::default(),
            name: SenderKeyName::new(
                "classic.invalid@g.us".to_owned(),
                "sender.invalid:1".to_owned(),
            ),
            packet: Vec::new(),
            packets: Vec::new(),
            plaintext: Vec::new(),
            distribution: None,
            session: None,
            state: None,
            sender_record: None,
            shared_record: None,
            signature_key: KeyPair::generate(&mut random()),
            #[cfg(feature = "bench-internals")]
            stored: Vec::new(),
            #[cfg(feature = "bench-internals")]
            lookup_key: None,
            #[cfg(feature = "bench-internals")]
            lookup_index: 0,
            #[cfg(feature = "bench-internals")]
            prepared: None,
            #[cfg(feature = "legacy-session-interop")]
            legacy: None,
        };
        match case {
            Case::Establish | Case::Conversation | Case::GenerateTen | Case::GroupCreate => {}
            #[cfg(feature = "bench-internals")]
            Case::SameIdentity | Case::DifferentIdentity => {
                let local = this.alice.identities.pair.identity_key();
                let remote = if matches!(case, Case::SameIdentity) {
                    local
                } else {
                    this.bob.identities.pair.identity_key()
                };
                this.state = Some(SessionState::new(
                    3,
                    local,
                    remote,
                    &RootKey::new([41; 32]),
                    &this.signature_key.public_key,
                ));
            }
            Case::EncryptFirst => {
                this.alice.establish(&this.bob);
                this.plaintext = payload(37);
            }
            Case::DecryptFirst | Case::RejectPreKey256 => {
                this.alice.establish(&this.bob);
                this.packet = this.alice.encrypt(&this.bob.address, &payload(25));
            }
            Case::EncryptSubsequent => {
                this.established();
                this.plaintext = payload(57);
            }
            Case::DecryptSubsequent | Case::DecryptOwnedSubsequent | Case::SessionSerialize200 => {
                this.steady();
                this.packet = this.alice.encrypt(&this.bob.address, &payload(27));
                if matches!(case, Case::SessionSerialize200) {
                    for _ in 0..200 {
                        this.packet = this.alice.encrypt(&this.bob.address, &payload(7));
                    }
                    this.bob
                        .receive(&this.alice.address, &this.packet, false, false);
                    this.session = this.bob.sessions.0.remove(&this.alice.address);
                }
            }
            Case::GroupEncrypt
            | Case::GroupDecrypt
            | Case::GroupIngest
            | Case::SenderSerialize200 => {
                this.distribution = Some(this.initialize_group());
                if !matches!(case, Case::GroupIngest) {
                    block_on(process_sender_key_distribution_message(
                        &this.name,
                        this.distribution.as_ref().unwrap(),
                        &mut this.receiver,
                    ))
                    .unwrap();
                }
                this.plaintext = payload(48);
                if matches!(case, Case::GroupDecrypt) {
                    this.packet = block_on(group_encrypt(
                        &mut this.sender,
                        &this.name,
                        &payload(21),
                        &mut random(),
                    ))
                    .unwrap()
                    .serialized()
                    .to_vec();
                }
                if matches!(case, Case::SenderSerialize200) {
                    for _ in 0..=200 {
                        this.packet = block_on(group_encrypt(
                            &mut this.sender,
                            &this.name,
                            &payload(7),
                            &mut random(),
                        ))
                        .unwrap()
                        .serialized()
                        .to_vec();
                    }
                    block_on(group_decrypt(&this.packet, &mut this.receiver, &this.name)).unwrap();
                    this.sender_record = this.receiver.0.remove(&this.name);
                }
            }
            Case::SenderSerializeEmpty | Case::SenderExportShared200 => {
                let count = if matches!(case, Case::SenderSerializeEmpty) {
                    0
                } else {
                    200
                };
                this.sender_record = Some(
                    SenderKeyRecord::from_components(SenderKeyRecordComponents {
                        states: vec![SenderKeyStateComponents {
                            key_id: 31,
                            chain_key: SenderChainKeyComponents {
                                iteration: count,
                                seed: payload(32),
                            },
                            signing_key: SenderSigningKeyComponents {
                                public: this.signature_key.public_key.serialize().to_vec(),
                                private: Some(this.signature_key.private_key.serialize().to_vec()),
                            },
                            message_keys: (0..count)
                                .map(|iteration| SenderMessageKeyComponents {
                                    iteration,
                                    seed: vec![(iteration % 251) as u8; 32],
                                })
                                .collect(),
                        }],
                    })
                    .unwrap(),
                );
                if count != 0 {
                    this.shared_record = this.sender_record.clone();
                }
            }
            Case::RootUpdate => {
                this.alice.establish(&this.bob);
                let record = this.alice.sessions.0.remove(&this.bob.address).unwrap();
                this.state = Some(record.session_state().unwrap().clone());
            }
            Case::SignTen | Case::SignAndVerifyTen | Case::VerifyTenAfterSigning => {
                this.plaintext = payload(64);
            }
            Case::ArchiveTen => {
                this.generation(1);
                this.packet = this.alice.encrypt(&this.bob.address, &payload(29));
                for generation in 2..=10 {
                    this.generation(generation);
                }
                assert_eq!(
                    this.bob.sessions.0[&this.alice.address].previous_session_count(),
                    9
                );
            }
            Case::ReverseTwenty => {
                this.alice.establish(&this.bob);
                let first = this.alice.encrypt(&this.bob.address, &payload(15));
                this.bob.receive(&this.alice.address, &first, true, false);
                let ack = this.bob.encrypt(&this.alice.address, &payload(14));
                this.alice.receive(&this.bob.address, &ack, false, false);
                for index in 0u32..20 {
                    this.packets.push(
                        this.alice
                            .encrypt(&this.bob.address, &payload(12 + index.to_string().len())),
                    );
                }
            }
            Case::PromoteTwenty | Case::RetryCurrentTwenty => {
                this.alice.establish(&this.bob);
                let first = this.alice.encrypt(&this.bob.address, &payload(13));
                this.bob.receive(&this.alice.address, &first, true, false);
                let original = this.alice.sessions.0[&this.bob.address].clone();
                for generation in 2..=20 {
                    this.bob.rotate_prekey(generation);
                    this.alice.establish(&this.bob);
                    let first = this.alice.encrypt(&this.bob.address, &payload(13));
                    this.bob.receive(&this.alice.address, &first, true, false);
                }
                if matches!(case, Case::PromoteTwenty) {
                    this.alice
                        .sessions
                        .0
                        .insert(this.bob.address.clone(), original);
                }
                this.packet = this.alice.encrypt(&this.bob.address, &payload(34));
                let receiving = &this.bob.sessions.0[&this.alice.address];
                assert_eq!(receiving.previous_session_count(), 19);
                let current_base = receiving
                    .clone()
                    .into_components()
                    .unwrap()
                    .current_session
                    .unwrap()
                    .alice_base_key
                    .unwrap();
                let packet_base = PreKeySignalMessage::try_from(this.packet.as_slice())
                    .unwrap()
                    .base_key()
                    .serialize()
                    .to_vec();
                assert_eq!(
                    current_base == packet_base,
                    matches!(case, Case::RetryCurrentTwenty),
                    "current-session retry and archived promotion must select distinct paths"
                );
            }
            Case::GroupWorst1999 | Case::GroupNext1999 => {
                let distribution = this.initialize_group();
                block_on(process_sender_key_distribution_message(
                    &this.name,
                    &distribution,
                    &mut this.receiver,
                ))
                .unwrap();
                for index in 0u32..=2000 {
                    let encoded = block_on(group_encrypt(
                        &mut this.sender,
                        &this.name,
                        &payload(10 + index.to_string().len()),
                        &mut random(),
                    ))
                    .unwrap();
                    this.packets.push(encoded.serialized().to_vec());
                }
                block_on(group_decrypt(
                    &this.packets[1999],
                    &mut this.receiver,
                    &this.name,
                ))
                .unwrap();
                let target = if matches!(case, Case::GroupWorst1999) {
                    // Use the same oldest delayed message on both implementations.
                    // Selecting each container's last slot would change the workload.
                    0
                } else {
                    2000
                };
                this.packet = std::mem::take(&mut this.packets[target]);
                this.packets.clear();
            }
            #[cfg(feature = "bench-internals")]
            Case::PreparedRebuildEncrypt | Case::PreparedRebuildDecrypt => {
                let distribution = this.initialize_group();
                block_on(process_sender_key_distribution_message(
                    &this.name,
                    &distribution,
                    &mut this.receiver,
                ))
                .unwrap();
                let warmup = block_on(group_encrypt(
                    &mut this.sender,
                    &this.name,
                    b"warmup",
                    &mut random(),
                ))
                .unwrap();
                block_on(group_decrypt(
                    warmup.serialized(),
                    &mut this.receiver,
                    &this.name,
                ))
                .unwrap();
                this.plaintext = payload(7);
                let record = if matches!(case, Case::PreparedRebuildEncrypt) {
                    this.sender.0.remove(&this.name).unwrap()
                } else {
                    this.packet = block_on(group_encrypt(
                        &mut this.sender,
                        &this.name,
                        &this.plaintext,
                        &mut random(),
                    ))
                    .unwrap()
                    .serialized()
                    .to_vec();
                    this.receiver.0.remove(&this.name).unwrap()
                };
                this.prepared = Some(PreparedGroups::new(record));
            }
            #[cfg(feature = "bench-internals")]
            Case::ColdLookup2000
            | Case::ColdLookup4000
            | Case::ColdLookup8000
            | Case::TailLookup2000
            | Case::TailLookup4000
            | Case::TailLookup8000
            | Case::ColdLoadLookup2000
            | Case::ColdLoadLookup4000
            | Case::ColdLoadLookup8000
            | Case::EvictTwoHundred => {
                let count = match case {
                    Case::ColdLookup4000 | Case::TailLookup4000 | Case::ColdLoadLookup4000 => 4000,
                    Case::ColdLookup8000 | Case::TailLookup8000 | Case::ColdLoadLookup8000 => 8000,
                    Case::EvictTwoHundred => 1999,
                    _ => 2000,
                };
                let (stored, ratchet) =
                    this.stored_backlog(count, matches!(case, Case::EvictTwoHundred));
                this.stored = stored;
                this.lookup_key = Some(ratchet);
                this.lookup_index = if matches!(
                    case,
                    Case::TailLookup2000 | Case::TailLookup4000 | Case::TailLookup8000
                ) {
                    count - 1
                } else {
                    count / 2
                };
                if !matches!(
                    case,
                    Case::ColdLoadLookup2000 | Case::ColdLoadLookup4000 | Case::ColdLoadLookup8000
                ) {
                    this.session = Some(SessionRecord::deserialize(&this.stored).unwrap());
                }
            }
            #[cfg(feature = "legacy-session-interop")]
            Case::ImportDefault0
            | Case::ImportDefault2000
            | Case::ImportBudget2000
            | Case::ImportBudget4000
            | Case::ImportBudget8000 => {
                let count = match case {
                    Case::ImportDefault0 => 0,
                    Case::ImportBudget4000 => 4000,
                    Case::ImportBudget8000 => 8000,
                    _ => 2000,
                };
                this.legacy = Some(this.legacy_input(count));
            }
        }
        this
    }
    fn established(&mut self) {
        self.alice.establish(&self.bob);
        let first = self.alice.encrypt(&self.bob.address, &payload(10));
        self.bob.receive(&self.alice.address, &first, true, false);
    }
    fn generation(&mut self, generation: u32) {
        if generation != 1 {
            self.bob.rotate_prekey(generation);
        }
        self.alice.establish(&self.bob);
        let first = self.alice.encrypt(&self.bob.address, &payload(12));
        self.bob.receive(&self.alice.address, &first, true, false);
        let ack = self.bob.encrypt(&self.alice.address, &payload(11));
        self.alice.receive(&self.bob.address, &ack, false, false);
    }
    #[cfg(feature = "bench-internals")]
    fn stored_backlog(&self, count: u32, evict: bool) -> (Vec<u8>, PublicKey) {
        use waproto::whatsapp::{
            self as wa,
            session_structure::{
                Chain,
                chain::{ChainKey as StoredChainKey, MessageKey as StoredMessageKey},
            },
        };
        let mut rng = random();
        let sender = KeyPair::generate(&mut rng);
        let receiver = KeyPair::generate(&mut rng);
        let base = KeyPair::generate(&mut rng);
        let mut sender_key = StoredChainKey::default();
        sender_key.index = Some(0);
        sender_key.key = Some(vec![91; 32].into());
        let mut sending = Chain::default();
        sending.sender_ratchet_key = Some(sender.public_key.serialize().to_vec());
        sending.sender_ratchet_key_private = Some(sender.private_key.serialize().to_vec());
        sending.chain_key = sender_key.into();
        let mut receiver_key = StoredChainKey::default();
        receiver_key.index = Some(if evict { 2300 } else { count });
        receiver_key.key = Some(vec![97; 32].into());
        let mut receiving = Chain::default();
        receiving.sender_ratchet_key = Some(receiver.public_key.serialize().to_vec());
        receiving.chain_key = receiver_key.into();
        receiving.message_key_limit = Some(count.max(2000));
        for index in 0..count {
            let seed = [(index % 251) as u8; 32];
            let mut material = [0; 80];
            hkdf::Hkdf::<sha2::Sha256>::new(None, &seed)
                .expand(b"WhisperMessageKeys", &mut material)
                .unwrap();
            let mut key = StoredMessageKey::default();
            key.index = Some(index);
            key.cipher_key = Some(material[..32].to_vec().into());
            key.mac_key = Some(material[32..64].to_vec().into());
            key.iv = Some(material[64..].to_vec().into());
            key.seed = Some(seed.to_vec().into());
            receiving.message_keys.push(key);
        }
        let mut session = wa::SessionStructure::default();
        session.session_version = Some(3);
        session.local_identity_public = Some(
            self.alice
                .identities
                .pair
                .identity_key()
                .serialize()
                .to_vec(),
        );
        session.remote_identity_public =
            Some(self.bob.identities.pair.identity_key().serialize().to_vec());
        session.root_key = Some(payload(32));
        session.previous_counter = Some(0);
        session.sender_chain = sending.into();
        session.receiver_chains.push(receiving);
        session.local_registration_id = Some(19);
        session.remote_registration_id = Some(19);
        session.alice_base_key = Some(base.public_key.serialize().to_vec());
        let mut record = wa::RecordStructure::default();
        record.current_session = session.into();
        (
            {
                #[allow(clippy::disallowed_methods)]
                let encoded = record.encode_to_vec();
                encoded
            },
            receiver.public_key,
        )
    }
    #[cfg(feature = "legacy-session-interop")]
    fn legacy_input(&self, count: u32) -> LegacySessionRecordV1 {
        use bytes::Bytes;
        let mut rng = random();
        let base = KeyPair::generate(&mut rng);
        let sending = KeyPair::generate(&mut rng);
        let receiving = KeyPair::generate(&mut rng);
        let base: Bytes = base.public_key.serialize().to_vec().into();
        let sender_public: Bytes = sending.public_key.serialize().to_vec().into();
        let receiver_public: Bytes = receiving.public_key.serialize().to_vec().into();
        LegacySessionRecordV1::from_indexed_sessions(vec![LegacyIndexedSessionV1 {
            index_key: base.clone(),
            session: LegacySessionV1 {
                registration_id: 19,
                ratchet: LegacySessionRatchetV1 {
                    key_pair: LegacySessionKeyPairV1 {
                        public: sender_public.clone(),
                        private: sending.private_key.serialize().to_vec().into(),
                    },
                    last_remote_ephemeral_key: receiver_public.clone(),
                    previous_counter: 0,
                    root_key: payload(32).into(),
                },
                index: LegacySessionIndexV1 {
                    base_key: base,
                    base_key_role: LegacySessionBaseKeyRoleV1::Remote,
                    disposition: LegacySessionDispositionV1::Current,
                    used_at_ms: 1,
                    created_at_ms: 1,
                    remote_identity_key: self
                        .bob
                        .identities
                        .pair
                        .identity_key()
                        .serialize()
                        .to_vec()
                        .into(),
                },
                chains: vec![
                    LegacySessionChainV1 {
                        ratchet_key: sender_public,
                        role: LegacySessionChainRoleV1::Sending,
                        chain_key: LegacySessionChainKeyV1 {
                            counter: LegacySessionChainCounterV1::new(-1).unwrap(),
                            key: Some(vec![79; 32].into()),
                        },
                        message_keys: Vec::new(),
                    },
                    LegacySessionChainV1 {
                        ratchet_key: receiver_public,
                        role: LegacySessionChainRoleV1::Receiving,
                        chain_key: LegacySessionChainKeyV1 {
                            counter: LegacySessionChainCounterV1::new(i64::from(count) - 1)
                                .unwrap(),
                            key: Some(vec![83; 32].into()),
                        },
                        message_keys: (0..count)
                            .map(|index| LegacySessionMessageKeyV1 {
                                index,
                                seed: vec![(index % 251) as u8; 32].into(),
                            })
                            .collect(),
                    },
                ],
                pending_pre_key: None,
            },
        }])
        .unwrap()
    }
    fn initialize_group(&mut self) -> SenderKeyDistributionMessage {
        // Fix logical keys and counters across libraries whose RNG draw order differs.
        let record = SenderKeyRecord::from_components(SenderKeyRecordComponents {
            states: vec![SenderKeyStateComponents {
                key_id: 31,
                chain_key: SenderChainKeyComponents {
                    iteration: 0,
                    seed: payload(32),
                },
                signing_key: SenderSigningKeyComponents {
                    public: self.signature_key.public_key.serialize().to_vec(),
                    private: Some(self.signature_key.private_key.serialize().to_vec()),
                },
                message_keys: Vec::new(),
            }],
        })
        .unwrap();
        self.sender.0.insert(self.name.clone(), record);
        block_on(create_sender_key_distribution_message(
            &self.name,
            &mut self.sender,
            &mut random(),
        ))
        .unwrap()
    }
    fn steady(&mut self) {
        self.established();
        let ack = self.bob.encrypt(&self.alice.address, &payload(3));
        self.alice.receive(&self.bob.address, &ack, false, false);
        let next = self.alice.encrypt(&self.bob.address, &payload(16));
        self.bob.receive(&self.alice.address, &next, false, false);
    }
    pub fn exercise(&mut self, case: Case) {
        match case {
            Case::Establish => self.alice.establish(&self.bob),
            #[cfg(feature = "bench-internals")]
            Case::SameIdentity | Case::DifferentIdentity => {
                assert_eq!(
                    access::same_identity(self.state.as_ref().unwrap()),
                    matches!(case, Case::SameIdentity)
                );
            }

            Case::EncryptFirst | Case::EncryptSubsequent => {
                black_box(
                    block_on(message_encrypt(
                        &self.plaintext,
                        &self.bob.address,
                        &mut self.alice.sessions,
                        &mut self.alice.identities,
                    ))
                    .unwrap(),
                );
            }
            Case::DecryptFirst => self
                .bob
                .receive(&self.alice.address, &self.packet, true, false),
            Case::DecryptSubsequent | Case::DecryptOwnedSubsequent => {
                self.bob.receive(
                    &self.alice.address,
                    &self.packet,
                    false,
                    matches!(case, Case::DecryptOwnedSubsequent),
                );
            }
            Case::GroupCreate => {
                black_box(
                    block_on(create_sender_key_distribution_message(
                        &self.name,
                        &mut self.sender,
                        &mut random(),
                    ))
                    .unwrap(),
                );
            }
            Case::GroupEncrypt => {
                black_box(
                    block_on(group_encrypt(
                        &mut self.sender,
                        &self.name,
                        &self.plaintext,
                        &mut random(),
                    ))
                    .unwrap(),
                );
            }
            Case::GroupDecrypt => {
                black_box(
                    block_on(group_decrypt(&self.packet, &mut self.receiver, &self.name)).unwrap(),
                );
            }
            Case::GroupIngest => {
                block_on(process_sender_key_distribution_message(
                    &self.name,
                    self.distribution.as_ref().unwrap(),
                    &mut self.receiver,
                ))
                .unwrap();
            }
            Case::SessionSerialize200 => {
                black_box(self.session.as_ref().unwrap().serialize().unwrap());
            }
            Case::SenderSerialize200 | Case::SenderSerializeEmpty => {
                black_box(self.sender_record.as_ref().unwrap().serialize().unwrap());
            }
            Case::SenderExportShared200 => {
                black_box(&self.shared_record);
                black_box(
                    self.sender_record
                        .take()
                        .unwrap()
                        .into_components()
                        .unwrap(),
                );
            }
            Case::RootUpdate => {
                self.state
                    .as_mut()
                    .unwrap()
                    .set_root_key(&RootKey::new([73; 32]));
                black_box(self.state.as_ref().unwrap());
            }
            Case::RejectPreKey256 => {
                for _ in 0..256 {
                    assert!(SignalMessage::try_from(self.packet.as_slice()).is_err());
                }
            }
            Case::GenerateTen => {
                let mut rng = random();
                for _ in 0..10 {
                    black_box(KeyPair::generate(&mut rng));
                }
            }
            Case::SignTen | Case::SignAndVerifyTen => {
                let mut rng = random();
                for _ in 0..10 {
                    let signature = self
                        .signature_key
                        .private_key
                        .calculate_signature(&self.plaintext, &mut rng)
                        .unwrap();
                    if matches!(case, Case::SignAndVerifyTen) {
                        assert!(
                            self.signature_key
                                .public_key
                                .verify_signature(&self.plaintext, &signature)
                        );
                    }
                    black_box(signature);
                }
            }
            Case::VerifyTenAfterSigning => {
                let signature = self
                    .signature_key
                    .private_key
                    .calculate_signature(&self.plaintext, &mut random())
                    .unwrap();
                for _ in 0..10 {
                    assert!(black_box(
                        self.signature_key
                            .public_key
                            .verify_signature(&self.plaintext, &signature)
                    ));
                }
            }
            Case::Conversation => {
                self.alice.establish(&self.bob);
                let first = self.alice.encrypt(&self.bob.address, &[71; 10]);
                self.bob.receive(&self.alice.address, &first, true, false);
                let reply = self.bob.encrypt(&self.alice.address, &[72; 9]);
                self.alice.receive(&self.bob.address, &reply, false, false);
                let last = self.alice.encrypt(&self.bob.address, &[73; 12]);
                self.bob.receive(&self.alice.address, &last, false, false);
            }
            Case::ArchiveTen => self
                .bob
                .receive(&self.alice.address, &self.packet, false, false),
            Case::ReverseTwenty => {
                for packet in self.packets.iter().rev() {
                    self.bob.receive(&self.alice.address, packet, false, false);
                }
            }
            Case::PromoteTwenty | Case::RetryCurrentTwenty => {
                let successes = (0..5)
                    .filter(|_| self.bob.retry_prekey(&self.alice.address, &self.packet))
                    .count();
                assert_eq!(successes, 1);
            }
            Case::GroupWorst1999 | Case::GroupNext1999 => {
                black_box(
                    block_on(group_decrypt(&self.packet, &mut self.receiver, &self.name)).unwrap(),
                );
            }
            #[cfg(feature = "bench-internals")]
            Case::PreparedRebuildEncrypt => {
                black_box(
                    block_on(group_encrypt(
                        self.prepared.as_mut().unwrap(),
                        &self.name,
                        &self.plaintext,
                        &mut random(),
                    ))
                    .unwrap(),
                );
            }
            #[cfg(feature = "bench-internals")]
            Case::PreparedRebuildDecrypt => {
                black_box(
                    block_on(group_decrypt(
                        &self.packet,
                        self.prepared.as_mut().unwrap(),
                        &self.name,
                    ))
                    .unwrap(),
                );
            }
            #[cfg(feature = "bench-internals")]
            Case::ColdLookup2000
            | Case::ColdLookup4000
            | Case::ColdLookup8000
            | Case::TailLookup2000
            | Case::TailLookup4000
            | Case::TailLookup8000 => {
                assert!(access::take(
                    self.session.as_mut().unwrap().session_state_mut().unwrap(),
                    self.lookup_key.as_ref().unwrap(),
                    self.lookup_index
                ));
            }
            #[cfg(feature = "bench-internals")]
            Case::ColdLoadLookup2000 | Case::ColdLoadLookup4000 | Case::ColdLoadLookup8000 => {
                let mut record = SessionRecord::deserialize(&self.stored).unwrap();
                assert!(access::take(
                    record.session_state_mut().unwrap(),
                    self.lookup_key.as_ref().unwrap(),
                    self.lookup_index
                ));
                self.session = Some(record);
            }
            #[cfg(feature = "bench-internals")]
            Case::EvictTwoHundred => {
                let state = self.session.as_mut().unwrap().session_state_mut().unwrap();
                for counter in 1999..2199 {
                    access::insert(
                        state,
                        self.lookup_key.as_ref().unwrap(),
                        counter,
                        [(counter % 251) as u8; 32],
                    );
                }
            }
            #[cfg(feature = "legacy-session-interop")]
            Case::ImportDefault0
            | Case::ImportDefault2000
            | Case::ImportBudget2000
            | Case::ImportBudget4000
            | Case::ImportBudget8000 => {
                let input = self.legacy.take().unwrap();
                let context = LegacySessionLocalContext {
                    identity_key: *self.alice.identities.pair.identity_key(),
                    registration_id: 19,
                };
                if matches!(case, Case::ImportDefault0 | Case::ImportDefault2000) {
                    black_box(input.into_session_record(context).unwrap());
                } else {
                    let count = match case {
                        Case::ImportBudget4000 => 4000,
                        Case::ImportBudget8000 => 8000,
                        _ => 2000,
                    };
                    black_box(
                        input
                            .into_session_record_with_limits(
                                context,
                                LegacySessionImportLimits {
                                    max_message_keys_per_chain: count,
                                    max_message_keys_per_record: count,
                                },
                            )
                            .unwrap(),
                    );
                }
            }
        }
    }
    #[cfg(all(test, feature = "bench-internals"))]
    pub fn check_cached_key_postconditions(&mut self, case: Case) {
        if matches!(
            case,
            Case::ColdLookup2000
                | Case::ColdLookup4000
                | Case::ColdLookup8000
                | Case::TailLookup2000
                | Case::TailLookup4000
                | Case::TailLookup8000
                | Case::ColdLoadLookup2000
                | Case::ColdLoadLookup4000
                | Case::ColdLoadLookup8000
        ) {
            assert!(!access::take(
                self.session.as_mut().unwrap().session_state_mut().unwrap(),
                self.lookup_key.as_ref().unwrap(),
                self.lookup_index
            ));
        }
        if matches!(case, Case::EvictTwoHundred) {
            let components = self
                .session
                .as_ref()
                .unwrap()
                .clone()
                .into_components()
                .unwrap();
            let keys = &components.current_session.unwrap().receiver_chains[0].message_keys;
            assert!(keys.len() <= 2051);
            assert!(keys.iter().any(|key| key.index == 2198));
        }
    }
}
