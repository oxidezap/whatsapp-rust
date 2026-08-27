use hkdf::Hkdf;
use hmac::{Hmac, KeyInit as _, Mac as _};
use sha2::Sha256;
use std::sync::LazyLock;

/// ExpandedAppStateKeys corresponds 1:1 with whatsmeow's ExpandedAppStateKeys.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExpandedAppStateKeys {
    pub index: [u8; 32],
    pub value_encryption: [u8; 32],
    pub value_mac: [u8; 32],
    pub snapshot_mac: [u8; 32],
    pub patch_mac: [u8; 32],
}

/// App-state key expansion runs HKDF with no salt, so its extract step is always
/// HMAC keyed by a constant zero block. Caching that keyed state lets each
/// expansion clone past the ipad/opad key schedule instead of recomputing it.
/// Same trick as `MESSAGE_KEY_EXTRACT_HMAC` on the Signal message-key path.
static EXTRACT_HMAC: LazyLock<Hmac<Sha256>> =
    LazyLock::new(|| Hmac::<Sha256>::new_from_slice(&[0u8; 32]).expect("32-byte HMAC key"));

/// Expand the 32 byte master app state sync key material into 160 bytes of sub-keys.
/// Go reference: expandAppStateKeys in vendor/whatsmeow/appstate/keys.go
pub fn expand_app_state_keys(key_data: &[u8]) -> ExpandedAppStateKeys {
    // HKDF-SHA256 with info "WhatsApp Mutation Keys" length 160
    const INFO: &[u8] = b"WhatsApp Mutation Keys";
    let mut extract = EXTRACT_HMAC.clone();
    extract.update(key_data);
    let prk = extract.finalize().into_bytes();
    let hk = Hkdf::<Sha256>::from_prk(&prk).expect("PRK is hash-sized");
    let mut okm = [0u8; 160];
    hk.expand(INFO, &mut okm).expect("hkdf expand");
    let take32 = |start: usize| {
        let mut arr = [0u8; 32];
        arr.copy_from_slice(&okm[start..start + 32]);
        arr
    };
    ExpandedAppStateKeys {
        index: take32(0),
        value_encryption: take32(32),
        value_mac: take32(64),
        snapshot_mac: take32(96),
        patch_mac: take32(128),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The cached zero-salt extract state must produce exactly what a plain
    /// `Hkdf::new(None, ..)` does, or every app-state MAC silently changes.
    #[test]
    fn cached_extract_matches_plain_hkdf() {
        for key in [[0u8; 32], [7u8; 32], [0xffu8; 32]] {
            let mut expected = [0u8; 160];
            Hkdf::<Sha256>::new(None, &key)
                .expand(b"WhatsApp Mutation Keys", &mut expected)
                .expect("hkdf expand");
            let got = expand_app_state_keys(&key);
            assert_eq!(got.index, expected[0..32]);
            assert_eq!(got.value_encryption, expected[32..64]);
            assert_eq!(got.value_mac, expected[64..96]);
            assert_eq!(got.snapshot_mac, expected[96..128]);
            assert_eq!(got.patch_mac, expected[128..160]);
        }
    }

    #[test]
    fn expansion_deterministic() {
        let key = [7u8; 32];
        let a = expand_app_state_keys(&key);
        let b = expand_app_state_keys(&key);
        assert_eq!(a, b);
    }
}
