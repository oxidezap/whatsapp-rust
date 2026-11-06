use base64::Engine as _;
use subtle::ConstantTimeEq;
use zeroize::Zeroizing;

use super::{FORM_LIMIT, RegistrationWireError};

/// A host-generated fresh ephemeral X25519 secret consumed by one encryption.
/// No Clone/Debug/serde or ambient entropy source is provided.
pub struct EphemeralRegistrationKey(Zeroizing<[u8; 32]>);

impl EphemeralRegistrationKey {
    /// The host must use a secure entropy source and never reuse these bytes.
    pub fn from_bytes(secret: [u8; 32]) -> Self {
        Self(Zeroizing::new(secret))
    }

    pub fn seal(
        self,
        server_public_key: &[u8; 32],
        plaintext: &[u8],
    ) -> Result<RegistrationEnvelope, RegistrationWireError> {
        if plaintext.len() > FORM_LIMIT {
            return Err(RegistrationWireError::SizeLimit);
        }
        use wacore_libsignal::protocol::{PrivateKey, PublicKey};
        let private =
            PrivateKey::deserialize(self.0.as_ref()).map_err(|_| RegistrationWireError::Crypto)?;
        let public = private
            .public_key()
            .map_err(|_| RegistrationWireError::Crypto)?;
        let server = PublicKey::from_djb_public_key_bytes(server_public_key)
            .map_err(|_| RegistrationWireError::InvalidServerKey)?;
        // The engine also rejects all-zero DH; keep the envelope's typed error
        // and its explicit defensive check before publishing ciphertext.
        let shared = Zeroizing::new(
            private
                .calculate_agreement(&server)
                .map_err(|_| RegistrationWireError::InvalidServerKey)?,
        );
        if bool::from(shared.ct_eq(&[0; 32])) || public.public_key_bytes() == &[0; 32] {
            return Err(RegistrationWireError::InvalidServerKey);
        }
        let mut payload = Zeroizing::new(Vec::with_capacity(32 + plaintext.len() + 16));
        payload.extend_from_slice(plaintext);
        // Zero nonce is the traced envelope construction. Fresh per-request DH
        // material is mandatory: this type is consumed to prevent accidental reuse.
        wacore_libsignal::crypto::transport_aead(&shared)
            .map_err(|_| RegistrationWireError::Crypto)?
            .encrypt_in_place(&[0; 12], b"", &mut *payload)
            .map_err(|_| RegistrationWireError::Crypto)?;
        let mut wire = Zeroizing::new(Vec::with_capacity(32 + payload.len()));
        wire.extend_from_slice(public.public_key_bytes());
        wire.extend_from_slice(&payload);
        Ok(RegistrationEnvelope {
            encoded: base64::engine::general_purpose::URL_SAFE.encode(&*wire),
        })
    }
}

/// Public ephemeral key and authenticated ciphertext, URL-base64 with padding.
pub struct RegistrationEnvelope {
    encoded: String,
}

impl RegistrationEnvelope {
    /// Exact UTF-8 bytes to supply to a legitimate external body-attestation provider.
    pub fn encoded(&self) -> &str {
        &self.encoded
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_independent_node_crypto_rfc7748_vector() {
        // RFC 7748 Alice secret/Bob public, encrypted independently with Node's
        // native X25519 and AES-256-GCM, nonce=0, empty AAD; synthetic phone.
        fn bytes(hex: &str) -> [u8; 32] {
            std::array::from_fn(|i| u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap())
        }
        let secret = bytes("77076d0a7318a57d3c16c17251b26645df4c2f87ebc0992ab177fba51db92c2a");
        let recipient = bytes("de9edb7d7b7dc1b4d35b61c2ece435373f8343c85b78674dadfc7e146f882b4f");
        let envelope = EphemeralRegistrationKey::from_bytes(secret)
            .seal(&recipient, b"cc=999&in=10000000")
            .unwrap();
        assert_eq!(
            envelope.encoded(),
            "hSDwCYkwp1R0i33ctD73Wg2_Og0mOBr066SpjqqbTmqzyqIiPBXyIaX7wcy1xwrck5Lj4_ZPR5frJIN2D73fK7Or"
        );
    }
    #[test]
    fn rejects_low_order_recipient_without_publishing_ciphertext() {
        assert!(matches!(
            EphemeralRegistrationKey::from_bytes([3; 32]).seal(&[0; 32], b"cc=999"),
            Err(RegistrationWireError::InvalidServerKey)
        ));
    }
}
