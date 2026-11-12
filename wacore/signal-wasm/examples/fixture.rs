// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Synthetic primary-device signing and stanza inspection for Node tests.
//! Input stays on stdin; output contains only a public signed container or
//! ciphertext stanza metadata. This is not an application service.
use hmac::{KeyInit, Mac};
use serde_json::{Value, json};
use std::io;
use wacore_binary::{Node, NodeContent};

fn bytes(value: &Value, name: &str) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    Ok(serde_json::from_value(value[name].clone())?)
}

// Test-only protos have no pinned codec root, as in the native pairing tests.
#[allow(clippy::disallowed_methods)]
fn pairing(value: &Value) -> Result<Value, Box<dyn std::error::Error>> {
    use buffa::Message as _;
    use wacore::libsignal::protocol::{KeyPair, PublicKey};
    use waproto::whatsapp as wa;
    let public: [u8; 32] = bytes(value, "identityKey")?
        .try_into()
        .map_err(|_| "invalid synthetic key length")?;
    let adv: [u8; 32] = bytes(value, "advSecret")?
        .try_into()
        .map_err(|_| "invalid synthetic key length")?;
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();
    let account = KeyPair::generate(&mut rng);
    let device = KeyPair::new(
        PublicKey::from_djb_public_key_bytes(&public)?,
        KeyPair::generate(&mut rng).private_key,
    );
    let mut details = wa::ADVDeviceIdentity::default();
    details.raw_id = Some(1);
    details.timestamp = Some(0);
    details.key_index = Some(1);
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
    let mut mac = <hmac::Hmac<sha2::Sha256> as KeyInit>::new_from_slice(&adv)?;
    mac.update(&signed);
    let mut container = wa::ADVSignedDeviceIdentityHMAC::default();
    container.details = Some(signed);
    container.hmac = Some(mac.finalize().into_bytes().to_vec());
    container.account_type = Some(wa::ADVEncryptionType::E2EE);
    let container = container.encode_to_vec();
    Ok(json!({"container":container}))
}

fn node(value: &Node) -> Value {
    let attrs: std::collections::BTreeMap<_, _> = value
        .attrs
        .iter()
        .map(|(key, value)| (key.to_string(), value.to_string()))
        .collect();
    let data = match &value.content {
        Some(NodeContent::Bytes(bytes)) => Some(bytes),
        _ => None,
    };
    let children: Vec<_> = value
        .children()
        .unwrap_or_default()
        .iter()
        .map(node)
        .collect();
    json!({"tag":value.tag, "attrs":attrs, "data":data, "children":children})
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let input: Value = serde_json::from_reader(io::stdin().lock())?;
    let output = match input["op"].as_str() {
        Some("pairing") => pairing(&input)?,
        Some("stanza") => node(
            &wacore_binary::marshal::unmarshal_packed_ref(&bytes(&input, "stanza")?)?.to_owned(),
        ),
        _ => return Err("unknown synthetic fixture operation".into()),
    };
    serde_json::to_writer(io::stdout().lock(), &output)?;
    Ok(())
}

fn main() {
    if run().is_err() {
        eprintln!("synthetic fixture request failed");
        std::process::exit(1);
    }
}
