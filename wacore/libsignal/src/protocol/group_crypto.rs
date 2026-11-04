// SPDX-License-Identifier: MIT
//! Record-owned, bounded prepared XEdDSA contexts; never serialized.
use crate::core::{PreparedSigningKey, PreparedVerifyingKey, PublicKey};
use crate::engine::{CryptoProvider, Error, RandomSource, Result, RustCrypto};
use std::sync::Arc;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

const MAX_CONTEXTS: usize = crate::engine::MAX_SENDER_STATES;
fn arc_allocation_bytes<T>() -> usize {
    let header = std::alloc::Layout::new::<[std::sync::atomic::AtomicUsize; 2]>();
    header
        .extend(std::alloc::Layout::new::<T>())
        .map(|(layout, _)| layout.pad_to_align().size())
        .unwrap_or(usize::MAX)
}
#[derive(Clone, Default)]
pub(crate) struct GroupCryptoCache {
    signing: [Option<Arc<SigningContext>>; MAX_CONTEXTS],
    verifying: [Option<Arc<VerifyingContext>>; MAX_CONTEXTS],
    next_signing: u8,
    next_verifying: u8,
}
struct SigningContext {
    public: [u8; 32],
    private: Zeroizing<[u8; 32]>,
    signing: Option<PreparedSigningKey>,
}
struct VerifyingContext {
    public: [u8; 32],
    verifying: Option<PreparedVerifyingKey>,
}
// Share the constant-time comparison across the unrolled cache scans.
#[inline(never)]
pub(crate) fn private_keys_match(left: Option<&[u8; 32]>, right: Option<&[u8; 32]>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => bool::from(left.ct_eq(right)),
        _ => false,
    }
}
impl GroupCryptoCache {
    /// Heap referenced by a record, including this cache's Arc allocation.
    pub(crate) fn estimated_heap_bytes(&self) -> usize {
        arc_allocation_bytes::<Self>()
            + self.signing.iter().flatten().count() * arc_allocation_bytes::<SigningContext>()
            + self.verifying.iter().flatten().count() * arc_allocation_bytes::<VerifyingContext>()
    }
    pub(crate) fn prepare(
        cache: &mut Option<Arc<Self>>,
        public: &[u8; 32],
        private: Option<&[u8; 32]>,
    ) -> PreparedGroupProvider {
        if let Some(provider) = cache
            .as_ref()
            .and_then(|cache| cache.cached(public, private))
        {
            return provider;
        }
        let cache = cache.get_or_insert_with(|| Arc::new(Self::default()));
        Arc::make_mut(cache).provider(public, private)
    }
    fn cached(
        &self,
        public: &[u8; 32],
        private: Option<&[u8; 32]>,
    ) -> Option<PreparedGroupProvider> {
        let context = if let Some(private) = private {
            PreparedContext::Signing(Arc::clone(self.signing.iter().flatten().find(
                |context| {
                    context.public == *public
                        && private_keys_match(Some(&context.private), Some(private))
                },
            )?))
        } else {
            PreparedContext::Verifying(Arc::clone(
                self.verifying
                    .iter()
                    .flatten()
                    .find(|context| context.public == *public)?,
            ))
        };
        Some(PreparedGroupProvider { context })
    }
    pub(crate) fn prune(
        cache: &mut Option<Arc<Self>>,
        mut keep: impl FnMut(&[u8; 32], Option<&[u8; 32]>) -> bool,
    ) {
        let Some(cache) = cache else { return };
        let stale = cache
            .signing
            .iter()
            .flatten()
            .any(|context| !keep(&context.public, Some(&context.private)))
            || cache
                .verifying
                .iter()
                .flatten()
                .any(|context| !keep(&context.public, None));
        if stale {
            Arc::make_mut(cache).retain(keep);
        }
    }
    pub(crate) fn provider(
        &mut self,
        public: &[u8; 32],
        private: Option<&[u8; 32]>,
    ) -> PreparedGroupProvider {
        let context = if let Some(private) = private {
            if let Some(context) = self.signing.iter().flatten().find(|context| {
                context.public == *public
                    && private_keys_match(Some(&context.private), Some(private))
            }) {
                return PreparedGroupProvider {
                    context: PreparedContext::Signing(Arc::clone(context)),
                };
            }
            let context = Arc::new(SigningContext {
                public: *public,
                private: Zeroizing::new(*private),
                signing: PreparedSigningKey::new_matching(private, public),
            });
            let index = self
                .signing
                .iter()
                .position(Option::is_none)
                .unwrap_or(usize::from(self.next_signing));
            self.signing[index] = Some(Arc::clone(&context));
            self.next_signing =
                u8::try_from((index + 1) % MAX_CONTEXTS).expect("bounded cache index");
            PreparedContext::Signing(context)
        } else {
            if let Some(context) = self
                .verifying
                .iter()
                .flatten()
                .find(|context| context.public == *public)
            {
                return PreparedGroupProvider {
                    context: PreparedContext::Verifying(Arc::clone(context)),
                };
            }
            let verifying = PublicKey::from_djb_public_key_bytes(public)
                .ok()
                .and_then(|key| PreparedVerifyingKey::new(&key).ok());
            let context = Arc::new(VerifyingContext {
                public: *public,
                verifying,
            });
            let index = self
                .verifying
                .iter()
                .position(Option::is_none)
                .unwrap_or(usize::from(self.next_verifying));
            self.verifying[index] = Some(Arc::clone(&context));
            self.next_verifying =
                u8::try_from((index + 1) % MAX_CONTEXTS).expect("bounded cache index");
            PreparedContext::Verifying(context)
        };
        PreparedGroupProvider { context }
    }
    pub(crate) fn retain(&mut self, mut keep: impl FnMut(&[u8; 32], Option<&[u8; 32]>) -> bool) {
        for slot in &mut self.signing {
            if slot
                .as_ref()
                .is_some_and(|context| !keep(&context.public, Some(&context.private)))
            {
                *slot = None;
            }
        }
        for slot in &mut self.verifying {
            if slot
                .as_ref()
                .is_some_and(|context| !keep(&context.public, None))
            {
                *slot = None;
            }
        }
    }
}
#[derive(Clone)]
enum PreparedContext {
    Signing(Arc<SigningContext>),
    Verifying(Arc<VerifyingContext>),
}
#[derive(Clone)]
pub(crate) struct PreparedGroupProvider {
    context: PreparedContext,
}
impl CryptoProvider for PreparedGroupProvider {
    fn public_key(&self, private: &[u8; 32]) -> Result<[u8; 32]> {
        RustCrypto.public_key(private)
    }
    fn agreement(&self, private: &[u8; 32], public: &[u8; 32]) -> Result<[u8; 32]> {
        RustCrypto.agreement(private, public)
    }
    fn sign(
        &self,
        private: &[u8; 32],
        message: &[u8],
        random: &mut dyn RandomSource,
    ) -> Result<[u8; 64]> {
        if let PreparedContext::Signing(context) = &self.context
            && bool::from(context.private.as_ref().ct_eq(private))
            && let Some(signing) = &context.signing
        {
            return signing.calculate_signature_with_random_source(message, random);
        }
        RustCrypto.sign(private, message, random)
    }
    fn verify(&self, public: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> Result<()> {
        if let PreparedContext::Verifying(context) = &self.context
            && *public == context.public
            && let Some(verifying) = &context.verifying
        {
            return if verifying.verify_signature(message, signature) {
                Ok(())
            } else {
                Err(Error::Authentication)
            };
        }
        RustCrypto.verify(public, message, signature)
    }
    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        RustCrypto.sha256(data)
    }
    fn hmac_sha256(&self, key: &[u8], parts: &[&[u8]]) -> Result<[u8; 32]> {
        Ok(RustCrypto.hmac_sha256_parts_array(key, parts))
    }
    fn chain_step(&self, key: &[u8; 32]) -> Result<([u8; 32], [u8; 32])> {
        RustCrypto.chain_step(key)
    }
    fn hkdf_sha256(&self, salt: &[u8], input: &[u8], info: &[u8], output: &mut [u8]) -> Result<()> {
        RustCrypto.hkdf_sha256(salt, input, info, output)
    }
    fn encrypt_cbc(&self, key: &[u8; 32], iv: &[u8; 16], plaintext: &[u8]) -> Result<Vec<u8>> {
        RustCrypto.encrypt_cbc(key, iv, plaintext)
    }
    fn decrypt_cbc(&self, key: &[u8; 32], iv: &[u8; 16], ciphertext: &[u8]) -> Result<Vec<u8>> {
        RustCrypto.decrypt_cbc(key, iv, ciphertext)
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        SenderKeyDistributionMessage, SenderKeyName, SenderKeyRecord, SenderKeyStore,
        create_sender_key_distribution_message, process_sender_key_distribution_message,
    };
    use super::*;
    use crate::core::PrivateKey;
    impl PreparedGroupProvider {
        fn signer(&self) -> &Arc<SigningContext> {
            let PreparedContext::Signing(context) = &self.context else {
                panic!("expected signer")
            };
            context
        }
        fn verifier(&self) -> &Arc<VerifyingContext> {
            let PreparedContext::Verifying(context) = &self.context else {
                panic!("expected verifier")
            };
            context
        }
    }
    fn add_state(record: &mut SenderKeyRecord, id: u32, byte: u8, private: bool) {
        let key = PrivateKey::deserialize(&[byte; 32]).unwrap();
        let public = key.public_key().unwrap();
        record
            .add_sender_key_state(3, id, 0, &[byte; 32], public, private.then_some(key))
            .unwrap();
    }
    struct Store(Option<SenderKeyRecord>);
    #[async_trait::async_trait]
    impl SenderKeyStore for Store {
        async fn load_sender_key(
            &self,
            _: &SenderKeyName,
        ) -> super::super::error::Result<Option<SenderKeyRecord>> {
            Ok(self.0.clone())
        }
        async fn store_sender_key(
            &mut self,
            _: &SenderKeyName,
            record: SenderKeyRecord,
        ) -> super::super::error::Result<()> {
            self.0 = Some(record);
            Ok(())
        }
    }
    #[test]
    fn adding_states_drops_evicted_private_context_immediately() {
        let mut record = SenderKeyRecord::new_empty();
        add_state(&mut record, 0, 7, true);
        let provider = record.prepared_signer(0).unwrap();
        let weak = Arc::downgrade(provider.signer());
        drop(provider);
        assert!(weak.upgrade().is_some());
        for id in 1..=5 {
            add_state(&mut record, id, id as u8 + 7, false);
        }
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn processing_replacement_distribution_drops_old_private_context_immediately() {
        let mut record = SenderKeyRecord::new_empty();
        add_state(&mut record, 7, 7, true);
        let provider = record.prepared_signer(0).unwrap();
        let weak = Arc::downgrade(provider.signer());
        drop(provider);
        let public = PrivateKey::deserialize(&[8; 32])
            .unwrap()
            .public_key()
            .unwrap();
        let message = SenderKeyDistributionMessage::new(3, 7, 0, [8; 32], public).unwrap();
        let name = SenderKeyName::from_parts("group.invalid@g.us", "sender.invalid:1");
        let mut store = Store(Some(record));
        futures::executor::block_on(process_sender_key_distribution_message(
            &name, &message, &mut store,
        ))
        .unwrap();
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn creating_distribution_drops_evicted_private_context_immediately() {
        use rand::SeedableRng;
        let mut record = SenderKeyRecord::new_empty();
        add_state(&mut record, 0, 7, true);
        for id in 1..5 {
            add_state(&mut record, id, id as u8 + 7, false);
        }
        let provider = record.prepared_signer(4).unwrap();
        let weak = Arc::downgrade(provider.signer());
        drop(provider);
        let name = SenderKeyName::from_parts("group.invalid@g.us", "sender.invalid:1");
        let mut store = Store(Some(record));
        let mut rng = rand::rngs::StdRng::from_seed([9; 32]);
        futures::executor::block_on(create_sender_key_distribution_message(
            &name, &mut store, &mut rng,
        ))
        .unwrap();
        assert!(weak.upgrade().is_none());
    }

    struct Seeded(rand::rngs::StdRng);
    impl Seeded {
        fn new() -> Self {
            use rand::SeedableRng;
            Self(rand::rngs::StdRng::from_seed([9; 32]))
        }
    }
    impl RandomSource for Seeded {
        fn fill(&mut self, out: &mut [u8]) -> Result<()> {
            rand::Rng::fill_bytes(&mut self.0, out);
            Ok(())
        }
    }
    struct Fixed(u8);
    impl RandomSource for Fixed {
        fn fill(&mut self, out: &mut [u8]) -> Result<()> {
            out.fill(self.0);
            Ok(())
        }
    }
    #[test]
    fn prepared_signatures_match_standard_and_random_failures_propagate() {
        let private = [7; 32];
        let public = RustCrypto.public_key(&private).unwrap();
        let mut cache = GroupCryptoCache::default();
        let provider = cache.provider(&public, Some(&private));
        let signature = provider
            .sign(&private, b"synthetic", &mut Seeded::new())
            .unwrap();
        assert_eq!(
            signature,
            RustCrypto
                .sign(&private, b"synthetic", &mut Seeded::new())
                .unwrap()
        );
        provider.verify(&public, b"synthetic", &signature).unwrap();
        RustCrypto
            .verify(&public, b"synthetic", &signature)
            .unwrap();
        assert!(provider.verify(&public, b"modified", &signature).is_err());
        struct Failed;
        impl RandomSource for Failed {
            fn fill(&mut self, _: &mut [u8]) -> Result<()> {
                Err(Error::Crypto)
            }
        }
        assert_eq!(
            provider.sign(&private, b"synthetic", &mut Failed),
            Err(Error::Crypto)
        );
    }
    #[test]
    fn providers_share_contexts_cache_is_bounded_and_retention_drops_pruned_keys() {
        let mut cache = GroupCryptoCache::default();
        let empty_heap = cache.estimated_heap_bytes();
        assert!(empty_heap > 0);
        let private = [7; 32];
        let public = RustCrypto.public_key(&private).unwrap();
        let first = cache.provider(&public, Some(&private));
        assert!(cache.verifying.iter().all(Option::is_none));
        let mut clone = cache.clone();
        let second = clone.provider(&public, Some(&private));
        let third = first.clone();
        assert!(Arc::ptr_eq(first.signer(), second.signer()));
        assert!(Arc::ptr_eq(first.signer(), third.signer()));
        assert_eq!(cache.signing.iter().flatten().count(), 1);
        assert!(cache.estimated_heap_bytes() > empty_heap);
        let weak = Arc::downgrade(first.signer());
        drop(first);
        drop(second);
        drop(third);
        cache.retain(|_, _| false);
        assert!(weak.upgrade().is_some()); // The cloned record still owns its key.
        clone.retain(|_, _| false);
        assert!(weak.upgrade().is_none());
        for byte in 0..9 {
            let private = [byte; 32];
            let public = RustCrypto.public_key(&private).unwrap();
            cache.provider(&public, Some(&private));
            assert!(cache.signing.iter().flatten().count() <= MAX_CONTEXTS);
        }
        let private = [8; 32];
        let public = RustCrypto.public_key(&private).unwrap();
        cache.retain(|key, secret| *key == public && secret == Some(&private));
        assert_eq!(cache.signing.iter().flatten().count(), 1);
        // Exact raw private bytes are part of the key even if clamping produces
        // the same scalar. A changed private key must not select the old tuple.
        let mut changed = private;
        changed[0] ^= 1;
        let first = cache.provider(&public, Some(&private));
        let second = cache.provider(&public, Some(&changed));
        assert!(!Arc::ptr_eq(first.signer(), second.signer()));
    }
    #[test]
    fn mismatched_keys_and_invalid_public_use_standard_fallback() {
        let private = [7; 32];
        let public = RustCrypto.public_key(&private).unwrap();
        let other = [8; 32];
        let other_public = RustCrypto.public_key(&other).unwrap();
        let mut cache = GroupCryptoCache::default();
        let provider = cache.provider(&public, Some(&private));
        let signature = provider.sign(&other, b"synthetic", &mut Fixed(9)).unwrap();
        assert_eq!(
            signature,
            RustCrypto
                .sign(&other, b"synthetic", &mut Fixed(9))
                .unwrap()
        );
        provider
            .verify(&other_public, b"synthetic", &signature)
            .unwrap();
        let mismatched = cache.provider(&public, Some(&other));
        assert!(mismatched.signer().signing.is_none());
        assert_eq!(
            mismatched
                .sign(&other, b"synthetic", &mut Fixed(9))
                .unwrap(),
            signature
        );
        let invalid = cache.provider(&[0; 32], None);
        assert!(invalid.verifier().verifying.is_none());
        assert_eq!(
            invalid.verify(&[0; 32], b"synthetic", &signature),
            RustCrypto.verify(&[0; 32], b"synthetic", &signature)
        );
    }
    #[test]
    fn cache_hits_share_sidecars_and_pruning_detaches_only_the_changed_snapshot() {
        let private = [7; 32];
        let public = RustCrypto.public_key(&private).unwrap();
        let mut owner = None;
        let first = GroupCryptoCache::prepare(&mut owner, &public, Some(&private));
        let mut sibling = owner.clone();
        let second = GroupCryptoCache::prepare(&mut sibling, &public, Some(&private));
        assert!(Arc::ptr_eq(
            owner.as_ref().unwrap(),
            sibling.as_ref().unwrap()
        ));
        assert!(Arc::ptr_eq(first.signer(), second.signer()));
        let weak = Arc::downgrade(first.signer());
        GroupCryptoCache::prune(&mut sibling, |_, _| false);
        assert!(!Arc::ptr_eq(
            owner.as_ref().unwrap(),
            sibling.as_ref().unwrap()
        ));
        assert!(
            sibling
                .as_ref()
                .unwrap()
                .cached(&public, Some(&private))
                .is_none()
        );
        assert!(
            owner
                .as_ref()
                .unwrap()
                .cached(&public, Some(&private))
                .is_some()
        );
        drop(first);
        drop(second);
        assert!(weak.upgrade().is_some());
        drop(owner);
        assert!(weak.upgrade().is_none());
    }
    #[test]
    fn ingestion_prepares_only_the_needed_role_and_preserves_storage_bytes() {
        let mut own = SenderKeyRecord::new_empty();
        add_state(&mut own, 1, 7, true);
        let bytes = own.serialize().unwrap();
        let signing_heap = own.estimated_heap_bytes();
        own.prepared_signer(0).unwrap();
        assert_eq!(own.estimated_heap_bytes(), signing_heap);
        own.prepared_verifier(0).unwrap();
        assert!(own.estimated_heap_bytes() > signing_heap);
        assert_eq!(own.serialize().unwrap(), bytes);
        let mut receiver = SenderKeyRecord::new_empty();
        add_state(&mut receiver, 1, 7, false);
        let bytes = receiver.serialize().unwrap();
        let verifying_heap = receiver.estimated_heap_bytes();
        receiver.prepared_verifier(0).unwrap();
        assert_eq!(receiver.estimated_heap_bytes(), verifying_heap);
        assert!(receiver.prepared_signer(0).is_err());
        assert_eq!(receiver.estimated_heap_bytes(), verifying_heap);
        assert_eq!(receiver.serialize().unwrap(), bytes);
    }

    #[test]
    fn historical_edwards_sign_bit_verifies_with_cached_context() {
        use curve25519_dalek::{edwards::EdwardsPoint, scalar::Scalar};
        use sha2::{Digest, Sha512};
        let mut cache = GroupCryptoCache::default();
        let mut signs = [false; 2];
        for byte in 0..32 {
            let private = [byte; 32];
            let public = RustCrypto.public_key(&private).unwrap();
            let mut clamped = private;
            clamped[0] &= 248;
            clamped[31] = (clamped[31] & 127) | 64;
            let scalar = Scalar::from_bytes_mod_order(clamped);
            let encoded_public = EdwardsPoint::mul_base(&scalar).compress();
            signs[(encoded_public.as_bytes()[31] >> 7) as usize] = true;
            let nonce = Scalar::from(13u64);
            let r = EdwardsPoint::mul_base(&nonce).compress();
            let mut hash = Sha512::new();
            hash.update(r.as_bytes());
            hash.update(encoded_public.as_bytes());
            hash.update(b"synthetic");
            let challenge = Scalar::from_bytes_mod_order_wide(&hash.finalize().into());
            let s = nonce + challenge * scalar;
            let mut signature = [0; 64];
            signature[..32].copy_from_slice(r.as_bytes());
            signature[32..].copy_from_slice(s.as_bytes());
            signature[63] |= encoded_public.as_bytes()[31] & 128;
            let provider = cache.provider(&public, None);
            provider.verify(&public, b"synthetic", &signature).unwrap();
            RustCrypto
                .verify(&public, b"synthetic", &signature)
                .unwrap();
        }
        assert_eq!(signs, [true, true]);
    }
}
