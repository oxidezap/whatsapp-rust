// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Helpers shared by the integration tests.
#![allow(dead_code)]

/// A pair-success container as the primary phone builds it: the account key
/// signs our identity, and the whole blob is HMACed with the QR's ADV secret.
// Test-only protos with no waproto::codec helper; the repo allows this in tests.
#[allow(clippy::disallowed_methods)]
pub fn pair_success_container(
    device_identity: [u8; 32],
    adv_secret: &[u8; 32],
    key_index: u32,
) -> (Vec<u8>, wacore::libsignal::protocol::KeyPair) {
    use buffa::Message;
    use hmac::{KeyInit, Mac};
    use wacore::libsignal::protocol::{KeyPair, PublicKey};
    use waproto::whatsapp as wa;

    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let account = KeyPair::generate(&mut rng);
    // Only the public half is read when no device prefix is requested.
    let device = KeyPair::new(
        PublicKey::from_djb_public_key_bytes(&device_identity).unwrap(),
        KeyPair::generate(&mut rng).private_key,
    );
    let mut details = wa::ADVDeviceIdentity::default();
    details.raw_id = Some(1);
    details.timestamp = Some(0);
    details.key_index = Some(key_index);
    details.account_type = Some(wa::ADVEncryptionType::E2EE);
    details.device_type = Some(wa::ADVEncryptionType::E2EE);
    let details = details.encode_to_vec();
    let signed = wacore::adv::test_util::signed_identity(
        &account,
        &device,
        &details,
        wacore::adv::test_util::account_prefix(Some(wa::ADVEncryptionType::E2EE)),
        None,
        true,
    )
    .encode_to_vec();
    let mut mac = <hmac::Hmac<sha2::Sha256> as KeyInit>::new_from_slice(adv_secret).unwrap();
    mac.update(&signed);
    let mut container = wa::ADVSignedDeviceIdentityHMAC::default();
    container.details = Some(signed);
    container.hmac = Some(mac.finalize().into_bytes().to_vec());
    container.account_type = Some(wa::ADVEncryptionType::E2EE);
    let container = container.encode_to_vec();
    (container, account)
}

/// A paired device as its peers see it: the account key that signed it, and
/// the `<device-identity>` its prekey bundles carry.
pub struct Paired {
    pub account: [u8; 32],
    pub device_identity: Vec<u8>,
}

/// Pair `ops` the way a real primary would, so it holds an account identity.
pub async fn pair<S: wacore_signal_ops::ServiceStore>(
    ops: &wacore_signal_ops::SignalOps<S>,
) -> Paired {
    let identity = ops.public_identity().await;
    let (container, account) =
        pair_success_container(identity.identity_key, &identity.adv_secret, 1);
    let signed = ops.sign_pairing(&container).await.unwrap();
    Paired {
        account: *account.public_key.public_key_bytes(),
        device_identity: signed.signed_identity,
    }
}
