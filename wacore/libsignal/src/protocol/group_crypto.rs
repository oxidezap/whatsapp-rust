// SPDX-License-Identifier: MIT
//! Record-owned prepared XEdDSA contexts. Serialization never includes this cache.
use crate::core::{PreparedSigningKey, PreparedVerifyingKey, PrivateKey, PublicKey};
use crate::engine::{CryptoProvider, Error, RandomSource, Result, RustCrypto};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex, MutexGuard},
};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

const MAX_CONTEXTS: usize = 5;
fn arc_allocation_bytes<T>() -> usize {
    let header = std::alloc::Layout::new::<[std::sync::atomic::AtomicUsize; 2]>();
    header
        .extend(std::alloc::Layout::new::<T>())
        .map(|(layout, _)| layout.pad_to_align().size())
        .unwrap_or(usize::MAX)
}
#[derive(Default)]
pub(crate) struct GroupCryptoCache {
    entries: Mutex<VecDeque<Arc<PreparedContext>>>,
}
struct PreparedContext {
    public: [u8; 32],
    private: Option<Zeroizing<[u8; 32]>>,
    signing: Option<PreparedSigningKey>,
    verifying: Option<PreparedVerifyingKey>,
}
pub(crate) fn private_keys_match(left: Option<&[u8; 32]>, right: Option<&[u8; 32]>) -> bool {
    match (left, right) {
        (None, None) => true,
        (Some(left), Some(right)) => bool::from(left.ct_eq(right)),
        _ => false,
    }
}
impl PreparedContext {
    fn matches(&self, public: &[u8; 32], private: Option<&[u8; 32]>) -> bool {
        self.public == *public && private_keys_match(self.private.as_deref(), private)
    }
}
impl GroupCryptoCache {
    /// Retained heap storage, including the record's cache Arc allocation.
    /// Prepared contexts contain no further heap allocations.
    pub(crate) fn estimated_heap_bytes(&self) -> usize {
        let entries = self.entries();
        arc_allocation_bytes::<Self>()
            + entries.capacity() * size_of::<Arc<PreparedContext>>()
            + entries.len() * arc_allocation_bytes::<PreparedContext>()
    }

    fn entries(&self) -> MutexGuard<'_, VecDeque<Arc<PreparedContext>>> {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
    pub(crate) fn provider(
        &self,
        public: &[u8; 32],
        private: Option<&[u8; 32]>,
    ) -> Result<PreparedGroupProvider> {
        let mut entries = self.entries();
        if let Some(index) = entries
            .iter()
            .position(|entry| entry.matches(public, private))
        {
            // Move the shared handle, retaining the prepared context across checkout.
            if let Some(context) = entries.remove(index) {
                entries.push_back(Arc::clone(&context));
                return Ok(PreparedGroupProvider { context });
            }
        }
        let verifying = PublicKey::from_djb_public_key_bytes(public)
            .ok()
            .and_then(|key| PreparedVerifyingKey::new(&key).ok());
        let signing = if let Some(private) = private {
            if RustCrypto.public_key(private)? == *public {
                Some(PreparedSigningKey::new(
                    &PrivateKey::deserialize(private).map_err(|_| Error::Crypto)?,
                ))
            } else {
                None
            }
        } else {
            None
        };
        let context = Arc::new(PreparedContext {
            public: *public,
            private: private.map(|key| Zeroizing::new(*key)),
            signing,
            verifying,
        });
        if entries.len() == MAX_CONTEXTS {
            entries.pop_front();
        }
        entries.push_back(Arc::clone(&context));
        Ok(PreparedGroupProvider { context })
    }
    /// Remove contexts for pruned or replaced sender states. Existing in-flight
    /// provider handles retain their own contexts until their operation completes.
    pub(crate) fn retain(&self, mut keep: impl FnMut(&[u8; 32], Option<&[u8; 32]>) -> bool) {
        self.entries()
            .retain(|entry| keep(&entry.public, entry.private.as_deref()));
    }
}
#[derive(Clone)]
pub(crate) struct PreparedGroupProvider {
    context: Arc<PreparedContext>,
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
        if let (Some(expected), Some(signing)) = (&self.context.private, &self.context.signing)
            && bool::from(expected.as_ref().ct_eq(private))
        {
            return signing.calculate_signature_with_random_source(message, random);
        }
        RustCrypto.sign(private, message, random)
    }
    fn verify(&self, public: &[u8; 32], message: &[u8], signature: &[u8; 64]) -> Result<()> {
        if *public == self.context.public
            && let Some(verifying) = &self.context.verifying
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
        let provider = record.prepared_crypto(0).unwrap();
        let weak = Arc::downgrade(&provider.context);
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
        let provider = record.prepared_crypto(0).unwrap();
        let weak = Arc::downgrade(&provider.context);
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
        let provider = record.prepared_crypto(4).unwrap();
        let weak = Arc::downgrade(&provider.context);
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
        let cache = GroupCryptoCache::default();
        let provider = cache.provider(&public, Some(&private)).unwrap();
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
        let cache = Arc::new(GroupCryptoCache::default());
        let clone = Arc::clone(&cache);
        let empty_heap = cache.estimated_heap_bytes();
        assert!(empty_heap > 0);
        let private = [7; 32];
        let public = RustCrypto.public_key(&private).unwrap();
        let first = cache.provider(&public, Some(&private)).unwrap();
        let second = clone.provider(&public, Some(&private)).unwrap();
        let third = first.clone();
        assert!(Arc::ptr_eq(&first.context, &second.context));
        assert!(Arc::ptr_eq(&first.context, &third.context));
        assert_eq!(cache.entries().len(), 1);
        assert!(cache.estimated_heap_bytes() > empty_heap);
        let weak = Arc::downgrade(&first.context);
        drop(first);
        drop(second);
        drop(third);
        cache.retain(|_, _| false);
        assert!(weak.upgrade().is_none());
        for byte in 0..9 {
            let private = [byte; 32];
            let public = RustCrypto.public_key(&private).unwrap();
            cache.provider(&public, Some(&private)).unwrap();
            assert!(cache.entries().len() <= MAX_CONTEXTS);
        }
        let private = [8; 32];
        let public = RustCrypto.public_key(&private).unwrap();
        cache.retain(|key, secret| *key == public && secret == Some(&private));
        assert_eq!(cache.entries().len(), 1);
        // Exact raw private bytes are part of the key even if clamping produces
        // the same scalar. A changed private key must not select the old tuple.
        let mut changed = private;
        changed[0] ^= 1;
        let first = cache.provider(&public, Some(&private)).unwrap();
        let second = cache.provider(&public, Some(&changed)).unwrap();
        assert!(!Arc::ptr_eq(&first.context, &second.context));
    }
    #[test]
    fn mismatched_keys_and_invalid_public_use_standard_fallback() {
        let private = [7; 32];
        let public = RustCrypto.public_key(&private).unwrap();
        let other = [8; 32];
        let other_public = RustCrypto.public_key(&other).unwrap();
        let cache = GroupCryptoCache::default();
        let provider = cache.provider(&public, Some(&private)).unwrap();
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
        let mismatched = cache.provider(&public, Some(&other)).unwrap();
        assert!(mismatched.context.signing.is_none());
        assert_eq!(
            mismatched
                .sign(&other, b"synthetic", &mut Fixed(9))
                .unwrap(),
            signature
        );
        let invalid = cache.provider(&[0; 32], None).unwrap();
        assert!(invalid.context.verifying.is_none());
        assert_eq!(
            invalid.verify(&[0; 32], b"synthetic", &signature),
            RustCrypto.verify(&[0; 32], b"synthetic", &signature)
        );
    }
    #[test]
    fn historical_edwards_sign_bit_verifies_with_cached_context() {
        use curve25519_dalek::{edwards::EdwardsPoint, scalar::Scalar};
        use sha2::{Digest, Sha512};
        let cache = GroupCryptoCache::default();
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
            let provider = cache.provider(&public, None).unwrap();
            provider.verify(&public, b"synthetic", &signature).unwrap();
            RustCrypto
                .verify(&public, b"synthetic", &signature)
                .unwrap();
        }
        assert_eq!(signs, [true, true]);
    }
}
