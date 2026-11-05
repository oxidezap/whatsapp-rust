use aes::cipher::{KeyIvInit, StreamCipher};
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use crate::libsignal::crypto::aes_256_gcm_decrypt;
use crate::libsignal::protocol::{KeyPair, PublicKey};
use crate::pair_code::PairCodeUtils;

use super::PrimaryLinkError;

type Aes256Ctr = ctr::Ctr128BE<aes::Aes256>;

/// Ephemeral exchange secret is kept in memory and cleared when dropped.
pub struct PrimaryHello {
    wrapped_ephemeral: [u8; 80],
    shared_ephemeral: Zeroizing<[u8; 32]>,
}

impl PrimaryHello {
    pub fn wrapped_ephemeral(&self) -> &[u8; 80] {
        &self.wrapped_ephemeral
    }

    /// Authenticate the companion finish and derive the ADV secret. The caller
    /// must separately bind the notification's pairing ref to its active exchange.
    pub fn complete(
        &self,
        wrapped_bundle: &[u8],
        companion_identity: &[u8; 32],
        primary_identity: &KeyPair,
    ) -> Result<Zeroizing<[u8; 32]>, PrimaryLinkError> {
        if wrapped_bundle.len() != 156 {
            return Err(PrimaryLinkError::InvalidInput("wrapped key bundle length"));
        }
        let mut key = Zeroizing::new([0; 32]);
        crate::crypto::hkdf_sha256_into(
            self.shared_ephemeral.as_ref(),
            Some(&wrapped_bundle[..32]),
            b"link_code_pairing_key_bundle_encryption_key",
            key.as_mut(),
        )
        .map_err(|_| PrimaryLinkError::KeyDerivation)?;
        let nonce: &[u8; 12] = wrapped_bundle[32..44]
            .try_into()
            .map_err(|_| PrimaryLinkError::Authentication)?;
        let mut plaintext = Zeroizing::new(Vec::with_capacity(96));
        aes_256_gcm_decrypt(&key, nonce, b"", &wrapped_bundle[44..], &mut plaintext)?;
        if plaintext.len() != 96
            || !bool::from(plaintext[..32].ct_eq(companion_identity))
            || !bool::from(plaintext[32..64].ct_eq(primary_identity.public_key.public_key_bytes()))
        {
            return Err(PrimaryLinkError::Authentication);
        }
        if primary_identity.private_key.public_key()? != primary_identity.public_key {
            return Err(PrimaryLinkError::InvalidInput("identity key-pair mismatch"));
        }
        let peer = PublicKey::from_djb_public_key_bytes(companion_identity)?;
        let shared_identity = Zeroizing::new(primary_identity.calculate_agreement(&peer)?);
        if bool::from(shared_identity.as_ref().ct_eq(&[0; 32])) {
            return Err(PrimaryLinkError::Authentication);
        }
        let mut material = Zeroizing::new([0; 96]);
        material[..32].copy_from_slice(self.shared_ephemeral.as_ref());
        material[32..64].copy_from_slice(shared_identity.as_ref());
        material[64..].copy_from_slice(&plaintext[64..]);
        let mut secret = Zeroizing::new([0; 32]);
        crate::crypto::hkdf_sha256_into(material.as_ref(), None, b"adv_secret", secret.as_mut())
            .map_err(|_| PrimaryLinkError::KeyDerivation)?;
        Ok(secret)
    }
}

/// Reverse the existing companion-side code handshake with caller-owned keys,
/// salt and IV. No generator, clock, socket or task is selected by this function.
pub fn prepare_primary_hello(
    pairing_code: &str,
    wrapped_companion_ephemeral: &[u8],
    primary_ephemeral: &KeyPair,
    salt: &[u8; 32],
    iv: &[u8; 16],
) -> Result<PrimaryHello, PrimaryLinkError> {
    if pairing_code.len() > 16 {
        return Err(PrimaryLinkError::InvalidInput("pairing code"));
    }
    let code = Zeroizing::new(
        pairing_code
            .chars()
            .filter(|c| *c != '-')
            .map(|c| c.to_ascii_uppercase())
            .collect::<String>(),
    );
    const ALPHABET: &[u8] = b"123456789ABCDEFGHJKLMNPQRSTVWXYZ";
    if code.len() != 8 || !code.bytes().all(|b| ALPHABET.contains(&b)) {
        return Err(PrimaryLinkError::InvalidInput("pairing code"));
    }
    if primary_ephemeral.private_key.public_key()? != primary_ephemeral.public_key {
        return Err(PrimaryLinkError::InvalidInput(
            "ephemeral key-pair mismatch",
        ));
    }
    let companion =
        PairCodeUtils::decrypt_primary_ephemeral_pub(wrapped_companion_ephemeral, &code)
            .map_err(|_| PrimaryLinkError::Authentication)?;
    let companion = PublicKey::from_djb_public_key_bytes(&companion)?;
    let shared_ephemeral = Zeroizing::new(primary_ephemeral.calculate_agreement(&companion)?);
    if bool::from(shared_ephemeral.as_ref().ct_eq(&[0; 32])) {
        return Err(PrimaryLinkError::Authentication);
    }
    let key = Zeroizing::new(PairCodeUtils::derive_key(&code, salt));
    let mut ciphertext = [0; 32];
    ciphertext.copy_from_slice(primary_ephemeral.public_key.public_key_bytes());
    Aes256Ctr::new((&*key).into(), iv.into()).apply_keystream(&mut ciphertext);
    let mut wrapped_ephemeral = [0; 80];
    wrapped_ephemeral[..32].copy_from_slice(salt);
    wrapped_ephemeral[32..48].copy_from_slice(iv);
    wrapped_ephemeral[48..].copy_from_slice(&ciphertext);
    Ok(PrimaryHello {
        wrapped_ephemeral,
        shared_ephemeral,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libsignal::crypto::aes_256_gcm_encrypt;
    use rand::SeedableRng;

    #[test]
    fn exchange_matches_companion_dh_and_rejects_identity_and_tag_changes() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(103);
        let companion = KeyPair::generate(&mut rng);
        let primary = KeyPair::generate(&mut rng);
        let companion_identity = KeyPair::generate(&mut rng);
        let primary_identity = KeyPair::generate(&mut rng);
        let code = "1234-5678";
        let code_key = PairCodeUtils::derive_key("12345678", &[3; 32]);
        let mut wrapped = [0; 80];
        wrapped[..32].fill(3);
        wrapped[32..48].fill(4);
        wrapped[48..].copy_from_slice(companion.public_key.public_key_bytes());
        Aes256Ctr::new((&code_key).into(), (&[4; 16]).into()).apply_keystream(&mut wrapped[48..]);
        let hello = prepare_primary_hello(code, &wrapped, &primary, &[5; 32], &[6; 16]).unwrap();
        assert_eq!(
            PairCodeUtils::decrypt_primary_ephemeral_pub(hello.wrapped_ephemeral(), "12345678")
                .unwrap()
                .as_slice(),
            primary.public_key.public_key_bytes()
        );
        let shared = companion.calculate_agreement(&primary.public_key).unwrap();
        let mut key = [0; 32];
        crate::crypto::hkdf_sha256_into(
            &shared,
            Some(&[7; 32]),
            b"link_code_pairing_key_bundle_encryption_key",
            &mut key,
        )
        .unwrap();
        let mut plain = Vec::new();
        plain.extend_from_slice(companion_identity.public_key.public_key_bytes());
        plain.extend_from_slice(primary_identity.public_key.public_key_bytes());
        plain.extend_from_slice(&[8; 32]);
        let mut bundle = vec![7; 32];
        bundle.extend_from_slice(&[9; 12]);
        aes_256_gcm_encrypt(&key, &[9; 12], b"", &plain, &mut bundle).unwrap();
        let peer: &[u8; 32] = companion_identity
            .public_key
            .public_key_bytes()
            .try_into()
            .unwrap();
        let actual = hello.complete(&bundle, peer, &primary_identity).unwrap();
        let identity_shared = companion_identity
            .calculate_agreement(&primary_identity.public_key)
            .unwrap();
        let mut material = Vec::from(shared);
        material.extend_from_slice(&identity_shared);
        material.extend_from_slice(&[8; 32]);
        let mut expected = [0; 32];
        crate::crypto::hkdf_sha256_into(&material, None, b"adv_secret", &mut expected).unwrap();
        assert_eq!(*actual, expected);
        assert!(
            hello
                .complete(&bundle, &[11; 32], &primary_identity)
                .is_err()
        );
        bundle[155] ^= 1;
        assert!(hello.complete(&bundle, peer, &primary_identity).is_err());
        assert!(
            hello
                .complete(&bundle[..80], peer, &primary_identity)
                .is_err()
        );
    }
    #[test]
    fn rejects_authenticated_low_order_keys_at_both_dh_boundaries() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(104);
        let primary = KeyPair::generate(&mut rng);
        let identity = KeyPair::generate(&mut rng);
        let mut low_order = [0; 32];
        low_order[0] = 1;
        let key = PairCodeUtils::derive_key("12345678", &[3; 32]);
        let mut wrapped = [0; 80];
        wrapped[..32].fill(3);
        wrapped[32..48].fill(4);
        wrapped[48..].copy_from_slice(&low_order);
        Aes256Ctr::new((&key).into(), (&[4; 16]).into()).apply_keystream(&mut wrapped[48..]);
        assert!(matches!(
            prepare_primary_hello("12345678", &wrapped, &primary, &[5; 32], &[6; 16]),
            Err(PrimaryLinkError::Authentication)
        ));

        let hello = PrimaryHello {
            wrapped_ephemeral: [0; 80],
            shared_ephemeral: Zeroizing::new([7; 32]),
        };
        let mut key = [0; 32];
        crate::crypto::hkdf_sha256_into(
            &[7; 32],
            Some(&[8; 32]),
            b"link_code_pairing_key_bundle_encryption_key",
            &mut key,
        )
        .unwrap();
        let mut plaintext = Vec::from(low_order);
        plaintext.extend_from_slice(identity.public_key.public_key_bytes());
        plaintext.extend_from_slice(&[9; 32]);
        let mut bundle = vec![8; 32];
        bundle.extend_from_slice(&[10; 12]);
        aes_256_gcm_encrypt(&key, &[10; 12], b"", &plaintext, &mut bundle).unwrap();
        assert!(matches!(
            hello.complete(&bundle, &low_order, &identity),
            Err(PrimaryLinkError::Authentication)
        ));
    }
}
