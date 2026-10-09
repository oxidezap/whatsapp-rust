//! Synthetic ADV fixtures shared by unit and downstream integration tests.

use crate::libsignal::protocol::KeyPair;
use wacore_binary::{Jid, Server};
use waproto::whatsapp::{self as wa, ADVEncryptionType};

pub const TEST_PN: &str = "12025550111";

pub const ENCRYPTION_TYPES: [Option<ADVEncryptionType>; 4] = [
    None,
    Some(ADVEncryptionType::E2EE),
    Some(ADVEncryptionType::HOSTED),
    Some(ADVEncryptionType::NON_E2EE),
];

pub fn device_cases() -> [(Jid, bool); 8] {
    [
        (Jid::pn_device(TEST_PN, 1), false),
        (Jid::lid_device("100000000000001", 1), false),
        (Jid::pn_device(TEST_PN, 99), true),
        (Jid::lid_device("100000000000001", 99), true),
        (Jid::new(TEST_PN, Server::Hosted).with_device(99), true),
        (
            Jid::new("100000000000001", Server::HostedLid).with_device(99),
            true,
        ),
        // The JID API also permits hosted servers with non-99 device IDs.
        (Jid::new(TEST_PN, Server::Hosted).with_device(7), true),
        (
            Jid::new("100000000000001", Server::HostedLid).with_device(7),
            true,
        ),
    ]
}

// Keep the fixture oracle independent of production prefix and message builders.
pub fn account_prefix(device_type: Option<ADVEncryptionType>) -> &'static [u8; 2] {
    match device_type {
        Some(ADVEncryptionType::HOSTED) => &[6, 5],
        None | Some(ADVEncryptionType::E2EE | ADVEncryptionType::NON_E2EE) => &[6, 0],
    }
}

pub fn signed_identity(
    account: &KeyPair,
    device: &KeyPair,
    details: &[u8],
    account_prefix: &[u8; 2],
    device_prefix: Option<&[u8; 2]>,
    include_account_key: bool,
) -> wa::ADVSignedDeviceIdentity {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let identity = device.public_key.public_key_bytes();
    let account_key = account.public_key.public_key_bytes();
    let account_signature = account
        .private_key
        .calculate_signature(
            &[account_prefix.as_slice(), details, identity].concat(),
            &mut rng,
        )
        .expect("synthetic account signature");
    let device_signature = device_prefix.map(|prefix| {
        device
            .private_key
            .calculate_signature(
                &[prefix.as_slice(), details, identity, account_key].concat(),
                &mut rng,
            )
            .expect("synthetic device signature")
            .to_vec()
    });
    wa::ADVSignedDeviceIdentity {
        details: Some(details.to_vec()),
        account_signature_key: include_account_key.then(|| account_key.to_vec()),
        account_signature: Some(account_signature.to_vec()),
        device_signature,
    }
}
