use crate::libsignal::protocol::{IdentityKey, PreKeyBundle, PreKeyId, PublicKey, SignedPreKeyId};
use std::collections::HashMap;
use wacore_binary::CompactString;
use wacore_binary::Jid;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Node, NodeRef};

pub struct PreKeyUtils;

/// Compute SHA-1 digest of a key bundle for validation against server.
///
/// Matches WA Web's `validateLocalKeyBundle` hash computation:
/// SHA-1(identity_pub_key || signed_prekey_pub || signed_prekey_signature || prekey_pub_1 || ...)
pub fn compute_key_bundle_digest(
    identity_pub_key: &[u8],
    signed_prekey_pub: &[u8],
    signed_prekey_signature: &[u8],
    prekey_pubkeys: &[&[u8]],
) -> Vec<u8> {
    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(identity_pub_key);
    hasher.update(signed_prekey_pub);
    hasher.update(signed_prekey_signature);
    for pk in prekey_pubkeys {
        hasher.update(pk);
    }
    hasher.finalize().to_vec()
}

/// Extract the `publicKey` field (tag 2) from a protobuf-encoded PreKeyRecordStructure
/// without full prost decode. Uses last-one-wins semantics per protobuf spec.
/// Skips unknown fields gracefully.
pub fn extract_prekey_public_key(record: &[u8]) -> Option<&[u8]> {
    let mut pos = 0;
    let mut result: Option<&[u8]> = None;
    while pos < record.len() {
        let (tag_byte, consumed) = decode_varint(&record[pos..])?;
        pos += consumed;
        let field_number = (tag_byte >> 3) as u32;
        let wire_type = (tag_byte & 0x7) as u32;
        match wire_type {
            // varint
            0 => {
                let (_, c) = decode_varint(&record[pos..])?;
                pos += c;
            }
            // length-delimited
            2 => {
                let (len, c) = decode_varint(&record[pos..])?;
                pos += c;
                let len = len as usize;
                if pos + len > record.len() {
                    return result;
                }
                if field_number == 2 {
                    result = Some(&record[pos..pos + len]);
                }
                pos += len;
            }
            // fixed64
            1 => {
                if pos + 8 > record.len() {
                    return result;
                }
                pos += 8;
            }
            // fixed32
            5 => {
                if pos + 4 > record.len() {
                    return result;
                }
                pos += 4;
            }
            // Unknown wire type -- skip gracefully
            _ => return result,
        }
    }
    result
}

fn decode_varint(buf: &[u8]) -> Option<(u64, usize)> {
    let mut result: u64 = 0;
    for (i, &byte) in buf.iter().enumerate().take(10) {
        // The 10th byte (i==9) carries the highest bits; only the low bit
        // is valid payload (64 - 9*7 = 1). Reject if more bits are set.
        if i == 9 && (byte & 0x7F) > 1 {
            return None;
        }
        result |= ((byte & 0x7F) as u64) << (i * 7);
        if byte & 0x80 == 0 {
            return Some((result, i + 1));
        }
    }
    None
}

impl PreKeyUtils {
    pub fn build_fetch_prekeys_request(jids: &[Jid], reason: Option<&str>) -> Node {
        let user_nodes = jids.iter().map(|jid| {
            let mut user_builder = NodeBuilder::new("user").attr("jid", jid);
            if let Some(r) = reason {
                user_builder = user_builder.attr("reason", r);
            }
            user_builder.build()
        });

        NodeBuilder::new("key").children(user_nodes).build()
    }

    pub fn build_upload_prekeys_request<'a>(
        registration_id: u32,
        identity_key_bytes: &[u8],
        signed_pre_key_id: u32,
        signed_pre_key_public_bytes: &[u8],
        signed_pre_key_signature: &[u8],
        pre_keys: impl IntoIterator<Item = (u32, &'a [u8])>,
    ) -> Vec<Node> {
        let pre_keys = pre_keys.into_iter();
        let (lower, upper) = pre_keys.size_hint();
        let mut pre_key_nodes = Vec::with_capacity(upper.unwrap_or(lower));
        for (pre_key_id, public_bytes) in pre_keys {
            let node = NodeBuilder::new("key")
                .children([
                    NodeBuilder::new("id")
                        .bytes(pre_key_id.to_be_bytes()[1..].to_vec())
                        .build(),
                    NodeBuilder::new("value").bytes(public_bytes).build(),
                ])
                .build();
            pre_key_nodes.push(node);
        }

        let signed_pre_key_node = NodeBuilder::new("skey")
            .children([
                NodeBuilder::new("id")
                    .bytes(signed_pre_key_id.to_be_bytes()[1..].to_vec())
                    .build(),
                NodeBuilder::new("value")
                    .bytes(signed_pre_key_public_bytes)
                    .build(),
                NodeBuilder::new("signature")
                    .bytes(signed_pre_key_signature)
                    .build(),
            ])
            .build();

        vec![
            NodeBuilder::new("registration")
                .bytes(registration_id.to_be_bytes().to_vec())
                .build(),
            NodeBuilder::new("type").bytes(vec![5u8]).build(),
            NodeBuilder::new("identity")
                .bytes(identity_key_bytes)
                .build(),
            NodeBuilder::new("list").children(pre_key_nodes).build(),
            signed_pre_key_node,
        ]
    }

    /// Parse a prekey-fetch response into bundles.
    ///
    /// `account_identities` maps a (normalized) companion JID to its account's
    /// primary (device 0) identity key, used as the ADV `account_signature_key`
    /// fallback when the server omits that field from `<device-identity>` (the
    /// normal case for a contact's companion device). The caller pre-loads these
    /// from the identity store; pass an empty map when no fallback is available.
    pub fn parse_prekeys_response(
        resp_node: &NodeRef<'_>,
        account_identities: &HashMap<Jid, [u8; 32]>,
    ) -> Result<HashMap<Jid, PreKeyBundle>, anyhow::Error> {
        let list_node = resp_node
            .get_optional_child("list")
            .ok_or_else(|| anyhow::anyhow!("<list> not found in pre-key response"))?;

        let children = list_node.children().unwrap_or_default();
        let mut bundles = HashMap::with_capacity(children.len());
        for user_node_ref in children {
            if user_node_ref.tag != "user" {
                continue;
            }
            let mut jid = user_node_ref
                .attrs()
                .jid("jid")
                .normalize_for_prekey_bundle();
            if jid.device == 0
                && matches!(
                    jid.server,
                    wacore_binary::Server::Pn | wacore_binary::Server::Lid
                )
                && let Some((user_base, device_str)) = jid.user.split_once(':')
                && let Ok(device) = device_str.parse::<u16>()
            {
                jid.user = CompactString::from(user_base);
                jid.device = device;
            }
            let account_identity = account_identities.get(&jid);
            let bundle =
                match Self::node_to_pre_key_bundle_ref(&jid, user_node_ref, account_identity) {
                    Ok(b) => b,
                    Err(e) => {
                        log::warn!("Failed to parse prekey bundle for {}: {}", jid, e);
                        continue;
                    }
                };
            bundles.insert(jid, bundle);
        }

        Ok(bundles)
    }

    fn node_to_pre_key_bundle_ref(
        jid: &Jid,
        node: &NodeRef<'_>,
        account_identity: Option<&[u8; 32]>,
    ) -> Result<PreKeyBundle, anyhow::Error> {
        use crate::xml::DisplayableNodeRef;
        use wacore_binary::NodeContentRef;

        fn extract_bytes_ref(node: Option<&NodeRef<'_>>) -> Result<Vec<u8>, anyhow::Error> {
            match node.and_then(|n| n.content.as_deref()) {
                Some(NodeContentRef::Bytes(b)) => Ok(b.to_vec()),
                _ => Err(anyhow::anyhow!("Expected bytes in node content")),
            }
        }

        if let Some(error_node) = node.get_optional_child("error") {
            return Err(anyhow::anyhow!(
                "Error getting prekeys: {}",
                DisplayableNodeRef(error_node)
            ));
        }

        let reg_id_bytes = extract_bytes_ref(node.get_optional_child("registration"))?;
        if reg_id_bytes.len() != 4 {
            return Err(anyhow::anyhow!("Invalid registration ID length"));
        }
        let registration_id = u32::from_be_bytes([
            reg_id_bytes[0],
            reg_id_bytes[1],
            reg_id_bytes[2],
            reg_id_bytes[3],
        ]);

        let keys_node = node.get_optional_child("keys").unwrap_or(node);

        let identity_key_bytes = extract_bytes_ref(keys_node.get_optional_child("identity"))?;
        let identity_key_array: [u8; 32] =
            identity_key_bytes.try_into().map_err(|v: Vec<u8>| {
                anyhow::anyhow!("Invalid identity key length: got {}, expected 32", v.len())
            })?;
        let identity_key =
            IdentityKey::new(PublicKey::from_djb_public_key_bytes(&identity_key_array)?);

        let mut pre_key_tuple = None;
        if let Some(pre_key_node) = keys_node.get_optional_child("key")
            && let Some((id, key_bytes)) = Self::node_to_pre_key_ref(pre_key_node)?
        {
            let pre_key_id: PreKeyId = id.into();
            let pre_key_public = PublicKey::from_djb_public_key_bytes(&key_bytes)?;
            pre_key_tuple = Some((pre_key_id, pre_key_public));
        }

        let signed_pre_key_node = keys_node
            .get_optional_child("skey")
            .ok_or(anyhow::anyhow!("Missing signed prekey"))?;
        let (signed_pre_key_id_u32, signed_pre_key_public_bytes, signed_pre_key_signature) =
            Self::node_to_signed_pre_key_ref(signed_pre_key_node)?;

        let signed_pre_key_id: SignedPreKeyId = signed_pre_key_id_u32.into();
        let signed_pre_key_public =
            PublicKey::from_djb_public_key_bytes(&signed_pre_key_public_bytes)?;

        let bundle = PreKeyBundle::new(
            registration_id,
            (jid.device as u32).into(),
            pre_key_tuple,
            signed_pre_key_id,
            signed_pre_key_public,
            signed_pre_key_signature.to_vec(),
            identity_key,
        )?;

        // Companion devices (device != 0) carry a <device-identity> that ADV-binds
        // the fetched identity key to the account, so a relay can't substitute a
        // fabricated identity. Matches WA Web SessionApi.createSignalSession. The
        // account key is the in-blob `account_signature_key` or, when the server
        // omits it (the normal case for a contact's companion), the account's
        // primary identity from the store (`account_identity`). Invalid → reject
        // the bundle; a missing device-identity or an unverifiable-for-lack-of-key
        // chain is logged but not fatal (WA Web throws here, but our store-set is
        // best-effort and a relay could already strip <device-identity> entirely).
        if jid.device != 0 {
            let device_identity = node
                .get_optional_child("device-identity")
                .or_else(|| keys_node.get_optional_child("device-identity"))
                .and_then(|n| match n.content.as_deref() {
                    Some(NodeContentRef::Bytes(b)) => Some(b),
                    _ => None,
                });
            match device_identity {
                Some(di) => match crate::adv::validate_adv_with_identity_key(
                    di,
                    &identity_key_array,
                    account_identity,
                ) {
                    crate::adv::AdvValidation::Valid => {}
                    crate::adv::AdvValidation::Invalid => {
                        return Err(anyhow::anyhow!(
                            "device-identity ADV validation failed for companion {jid}"
                        ));
                    }
                    crate::adv::AdvValidation::NoAccountKey => log::warn!(
                        "prekey bundle for companion {jid} omits account_signature_key and no stored account identity; proceeding without ADV validation"
                    ),
                },
                None => log::warn!(
                    "prekey bundle for companion {jid} omits <device-identity>; proceeding without ADV validation"
                ),
            }
        }

        Ok(bundle)
    }

    fn node_to_pre_key_ref(node: &NodeRef<'_>) -> Result<Option<(u32, [u8; 32])>, anyhow::Error> {
        use wacore_binary::NodeContentRef;

        let id_content = node
            .get_optional_child("id")
            .and_then(|n| n.content.as_deref());

        let id = match id_content {
            Some(NodeContentRef::Bytes(b)) if !b.is_empty() => {
                if b.len() == 3 {
                    Ok(u32::from_be_bytes([0, b[0], b[1], b[2]]))
                } else if let Ok(s) = std::str::from_utf8(b.as_ref()) {
                    let trimmed_s = s.trim();
                    if trimmed_s.is_empty() {
                        Err(anyhow::anyhow!("ID content is only whitespace"))
                    } else {
                        u32::from_str_radix(trimmed_s, 16).map_err(|e| e.into())
                    }
                } else {
                    Err(anyhow::anyhow!("ID is not valid UTF-8 hex or 3-byte int"))
                }
            }
            _ => Err(anyhow::anyhow!("Missing or empty pre-key ID content")),
        };

        let id = match id {
            Ok(val) => val,
            Err(_e) => return Ok(None),
        };

        let value_bytes = node
            .get_optional_child("value")
            .and_then(|n| n.content.as_deref())
            .and_then(|c| {
                if let NodeContentRef::Bytes(b) = c {
                    Some(b.to_vec())
                } else {
                    None
                }
            })
            .ok_or(anyhow::anyhow!("Missing pre-key value"))?;
        if value_bytes.len() != 32 {
            return Err(anyhow::anyhow!("Invalid pre-key value length"));
        }

        let mut value_arr = [0u8; 32];
        value_arr.copy_from_slice(&value_bytes);
        Ok(Some((id, value_arr)))
    }

    fn node_to_signed_pre_key_ref(
        node: &NodeRef<'_>,
    ) -> Result<(u32, [u8; 32], [u8; 64]), anyhow::Error> {
        use wacore_binary::NodeContentRef;

        let (id, public_key_bytes) = match Self::node_to_pre_key_ref(node)? {
            Some((id, key)) => (id, key),
            None => return Err(anyhow::anyhow!("Signed pre-key is missing ID or value")),
        };
        let signature_bytes = node
            .get_optional_child("signature")
            .and_then(|n| n.content.as_deref())
            .and_then(|c| {
                if let NodeContentRef::Bytes(b) = c {
                    Some(b.to_vec())
                } else {
                    None
                }
            })
            .ok_or(anyhow::anyhow!("Missing signed pre-key signature"))?;
        if signature_bytes.len() != 64 {
            return Err(anyhow::anyhow!("Invalid signature length"));
        }

        let mut sig_arr = [0u8; 64];
        sig_arr.copy_from_slice(&signature_bytes);
        Ok((id, public_key_bytes, sig_arr))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::iq::prekeys::PreKeyBundleUserNode;
    use crate::libsignal::protocol::{IdentityKeyPair, KeyPair};
    use crate::protocol::ProtocolNode;

    use wacore_binary::NodeValue;

    fn create_mock_bundle(device_id: u32) -> PreKeyBundle {
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let identity_pair = IdentityKeyPair::generate(&mut rng);
        let signed_prekey_pair = KeyPair::generate(&mut rng);
        let prekey_pair = KeyPair::generate(&mut rng);

        PreKeyBundle::new(
            1,
            device_id.into(),
            Some((1u32.into(), prekey_pair.public_key)),
            2u32.into(),
            signed_prekey_pair.public_key,
            vec![0u8; 64],
            *identity_pair.identity_key(),
        )
        .expect("Failed to create PreKeyBundle")
    }

    #[test]
    fn test_parse_prekeys_response_normalizes_lid_device_jid() {
        let base_jid = Jid::lid_device("100000012345678", 33);
        let bundle = create_mock_bundle(33);
        let mut user_node = PreKeyBundleUserNode::from_bundle(base_jid.clone(), &bundle, None)
            .expect("build bundle node")
            .into_node();

        let raw_jid = Jid {
            user: "100000012345678:33".into(),
            server: wacore_binary::Server::Lid,
            agent: 1,
            device: 0,
            integrator: 0,
        };
        user_node
            .attrs
            .insert("jid".to_string(), NodeValue::Jid(raw_jid.clone()));

        let response = NodeBuilder::new("iq")
            .children([NodeBuilder::new("list").children([user_node]).build()])
            .build();

        let bundles = PreKeyUtils::parse_prekeys_response(&response.as_node_ref(), &HashMap::new())
            .expect("parse bundles");
        assert!(bundles.contains_key(&base_jid));
        assert!(!bundles.contains_key(&raw_jid));

        let parsed_jid = bundles.keys().next().expect("parsed jid");
        assert_eq!(parsed_jid.user, base_jid.user);
        assert_eq!(parsed_jid.device, base_jid.device);
        assert_eq!(parsed_jid.agent, 0);
    }

    fn parse_one(jid: Jid, device_identity: Option<Vec<u8>>) -> HashMap<Jid, PreKeyBundle> {
        let bundle = create_mock_bundle(jid.device as u32);
        let user_node = PreKeyBundleUserNode::from_bundle(jid, &bundle, device_identity)
            .expect("build bundle node")
            .into_node();
        let response = NodeBuilder::new("iq")
            .children([NodeBuilder::new("list").children([user_node]).build()])
            .build();
        PreKeyUtils::parse_prekeys_response(&response.as_node_ref(), &HashMap::new())
            .expect("parse bundles")
    }

    #[test]
    fn parse_skips_companion_bundle_with_invalid_device_identity() {
        let companion = Jid::lid_device("100000012345678", 33);
        let bundles = parse_one(companion.clone(), Some(vec![0xDE, 0xAD, 0xBE, 0xEF]));
        assert!(
            !bundles.contains_key(&companion),
            "companion bundle with an unverifiable device-identity must be dropped"
        );
    }

    // A trimmed device-identity (no account_signature_key) with no stored
    // account-identity fallback is unverifiable, not forged: the bundle must be
    // KEPT (proceed without ADV), otherwise the companion stops receiving. This
    // is the regression #772 introduced (it dropped the bundle outright).
    #[test]
    fn parse_keeps_companion_bundle_when_account_key_omitted_and_no_fallback() {
        use crate::libsignal::protocol::KeyPair;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let account = KeyPair::generate(&mut rng);
        let device = KeyPair::generate(&mut rng);
        let companion = Jid::lid_device("100000012345678", 33);
        // device-identity without account_signature_key, signatures over the
        // bundle's identity key (create_mock_bundle's identity is unrelated, but
        // the NoAccountKey path short-circuits before signature checks anyway).
        let di = trimmed_device_identity(&account, &device, b"details");
        let bundles = parse_one(companion.clone(), Some(di));
        assert!(
            bundles.contains_key(&companion),
            "trimmed device-identity with no fallback must be kept, not dropped"
        );
    }

    #[test]
    fn parse_keeps_primary_bundle_ignoring_device_identity() {
        // device 0 is the account's primary: ADV validation does not apply, so a
        // junk device-identity must not get it dropped.
        let primary = Jid::lid_device("100000012345678", 0);
        let bundles = parse_one(primary.clone(), Some(vec![0xDE, 0xAD, 0xBE, 0xEF]));
        assert!(bundles.contains_key(&primary));
    }

    /// Encode an ADVSignedDeviceIdentity without the account_signature_key field.
    fn trimmed_device_identity(
        account: &crate::libsignal::protocol::KeyPair,
        device: &crate::libsignal::protocol::KeyPair,
        details: &[u8],
    ) -> Vec<u8> {
        use prost::Message;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let identity = device.public_key.public_key_bytes();
        let account_key = account.public_key.public_key_bytes();
        let account_sig = account
            .private_key
            .calculate_signature(&[&[6u8, 0][..], details, identity].concat(), &mut rng)
            .unwrap()
            .to_vec();
        let device_sig = device
            .private_key
            .calculate_signature(
                &[&[6u8, 1][..], details, identity, account_key].concat(),
                &mut rng,
            )
            .unwrap()
            .to_vec();
        waproto::whatsapp::AdvSignedDeviceIdentity {
            details: Some(details.to_vec()),
            account_signature_key: None,
            account_signature: Some(account_sig),
            device_signature: Some(device_sig),
        }
        .encode_to_vec()
    }
}
