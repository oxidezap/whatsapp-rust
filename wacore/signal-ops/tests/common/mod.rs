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
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let account = wacore::libsignal::protocol::KeyPair::generate(&mut rng);
    let container = pair_success_container_for(&account, device_identity, adv_secret, key_index);
    (container, account)
}

/// As [`pair_success_container`], signed by an existing `account` key, the way
/// a primary re-pairs a reset or replacement companion under the same account.
#[allow(clippy::disallowed_methods)]
pub fn pair_success_container_for(
    account: &wacore::libsignal::protocol::KeyPair,
    device_identity: [u8; 32],
    adv_secret: &[u8; 32],
    key_index: u32,
) -> Vec<u8> {
    use buffa::Message;
    use hmac::{KeyInit, Mac};
    use wacore::libsignal::protocol::{KeyPair, PublicKey};
    use waproto::whatsapp as wa;

    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
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
        account,
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
    container.encode_to_vec()
}

/// A paired device as its peers see it: the account key that signed it, and
/// the `<device-identity>` its prekey bundles carry.
pub struct Paired {
    pub account: [u8; 32],
    /// The account key pair, to re-pair a replacement device under it.
    pub account_keys: wacore::libsignal::protocol::KeyPair,
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
        account_keys: account,
        device_identity: signed.signed_identity,
    }
}

/// Pair `ops` under an existing account key, as a primary does for a device
/// that was reset or replaced. Returns its new `<device-identity>`.
pub async fn pair_under<S: wacore_signal_ops::ServiceStore>(
    ops: &wacore_signal_ops::SignalOps<S>,
    account: &wacore::libsignal::protocol::KeyPair,
) -> Vec<u8> {
    let identity = ops.public_identity().await;
    let container =
        pair_success_container_for(account, identity.identity_key, &identity.adv_secret, 1);
    ops.sign_pairing(&container).await.unwrap().signed_identity
}

/// Pair `peer` under a fresh account and have `sender` pin that account for
/// `peer_jid`, as it would after meeting the peer's primary device. Returns
/// the `<device-identity>` the peer's bundle must carry.
pub async fn vouch<S: wacore_signal_ops::ServiceStore, P: wacore_signal_ops::ServiceStore>(
    sender: &wacore_signal_ops::SignalOps<S>,
    peer: &wacore_signal_ops::SignalOps<P>,
    peer_jid: &str,
) -> Vec<u8> {
    let paired = pair(peer).await;
    let jid: wacore_binary::Jid = peer_jid.parse().unwrap();
    sender
        .trust_account_identity(&jid, paired.account)
        .await
        .unwrap();
    paired.device_identity
}
