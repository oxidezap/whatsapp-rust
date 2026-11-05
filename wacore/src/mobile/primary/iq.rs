use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Node, NodeContent, NodeRef};
use waproto::whatsapp as wa;

use crate::iq::spec::IqSpec;
use crate::request::InfoQuery;

use super::{
    PrimaryLinkError, SignedCompanionIdentity, bounded_text, validate_accounts, validate_companion,
};

/// An upload result only assigns a device address; callers must still obtain
/// and authenticate companion proof before durable link publication.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PairDeviceAcknowledgement {
    pub device_jid: Jid,
}

/// Explicit optional PEM profile from the pinned mobile references. No TTL,
/// key identifier, algorithm or version is selected implicitly.
pub struct PairDevicePem {
    key: zeroize::Zeroizing<[u8; 32]>,
    ttl_seconds: u64,
    key_id: Vec<u8>,
    algorithm: String,
    version: String,
}
impl PairDevicePem {
    pub fn new(
        key: [u8; 32],
        ttl_seconds: u64,
        key_id: &[u8],
        algorithm: &str,
        version: &str,
    ) -> Result<Self, PrimaryLinkError> {
        if key == [0; 32]
            || ttl_seconds == 0
            || key_id.is_empty()
            || key_id.len() > 128
            || !bounded_text(algorithm, 32)
            || !bounded_text(version, 16)
        {
            return Err(PrimaryLinkError::InvalidInput("PEM profile"));
        }
        Ok(Self {
            key: zeroize::Zeroizing::new(key),
            ttl_seconds,
            key_id: key_id.to_vec(),
            algorithm: algorithm.to_owned(),
            version: version.to_owned(),
        })
    }
}
/// Reference-derived optional PEM key derivation; callers select whether their
/// qualified mobile profile requires this extension.
pub fn derive_companion_pem_key(
    adv_secret: &[u8; 32],
    noise_public: &[u8; 32],
) -> Result<zeroize::Zeroizing<[u8; 32]>, PrimaryLinkError> {
    let mut key = zeroize::Zeroizing::new([0; 32]);
    crate::crypto::hkdf_sha256_into(
        adv_secret,
        Some(noise_public),
        b"Canonical Ent Companion Nonce Encrypt",
        key.as_mut(),
    )
    .map_err(|_| PrimaryLinkError::KeyDerivation)?;
    Ok(key)
}
pub struct PairDeviceSpec {
    server: Jid,
    own_accounts: Vec<Jid>,
    reference: Vec<u8>,
    companion_noise: [u8; 32],
    identity: SignedCompanionIdentity,
    timestamp_seconds: u64,
    client_props: Option<wa::ClientPairingProps>,
    pem: Option<PairDevicePem>,
}

impl PairDeviceSpec {
    pub fn new(
        server: &Jid,
        own_accounts: &[Jid],
        reference: &[u8],
        companion_noise: &[u8; 32],
        identity: SignedCompanionIdentity,
        timestamp_seconds: u64,
        client_props: Option<wa::ClientPairingProps>,
    ) -> Result<Self, PrimaryLinkError> {
        validate_accounts(own_accounts)?;
        validate_reference(reference)?;
        validate_signed_list(&identity.key_index_list, timestamp_seconds)?;
        if identity.device_identity.is_empty() || identity.device_identity.len() > 16384 {
            return Err(PrimaryLinkError::InvalidInput("signed companion identity"));
        }
        waproto::codec::adv_signed_device_identity_hmac_decode(&identity.device_identity)?;
        Ok(Self {
            server: server.clone(),
            own_accounts: own_accounts.to_vec(),
            reference: reference.to_vec(),
            companion_noise: *companion_noise,
            identity,
            timestamp_seconds,
            client_props,
            pem: None,
        })
    }
}

impl PairDeviceSpec {
    pub fn with_pem(mut self, pem: PairDevicePem) -> Self {
        self.pem = Some(pem);
        self
    }
}
impl IqSpec for PairDeviceSpec {
    type Response = PairDeviceAcknowledgement;
    fn build_iq(&self) -> InfoQuery<'static> {
        let mut children = vec![
            NodeBuilder::new("ref")
                .bytes(self.reference.clone())
                .build(),
            NodeBuilder::new("pub-key")
                .bytes(self.companion_noise)
                .build(),
            NodeBuilder::new("device-identity")
                .bytes(self.identity.device_identity.clone())
                .build(),
            key_index_node(&self.identity.key_index_list, self.timestamp_seconds),
        ];
        if let Some(props) = &self.client_props {
            children.push(
                NodeBuilder::new("client-props")
                    .bytes(waproto::codec::client_pairing_props_to_vec(props))
                    .build(),
            );
        }
        if let Some(pem) = &self.pem {
            children.push(
                NodeBuilder::new("pem")
                    .attr("version", pem.version.clone())
                    .attr("algorithm", pem.algorithm.clone())
                    .children([
                        NodeBuilder::new("pem").bytes(pem.key.to_vec()).build(),
                        NodeBuilder::new("ttl")
                            .attr("ts_s", pem.ttl_seconds.to_string())
                            .build(),
                        NodeBuilder::new("key_id").bytes(pem.key_id.clone()).build(),
                    ])
                    .build(),
            );
        }
        query(
            &self.server,
            NodeBuilder::new("pair-device").children(children).build(),
        )
    }
    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        result(response)?;
        let mut candidates = Vec::new();
        for child in response.children().unwrap_or_default() {
            if child.tag == "device" {
                candidates.push(child);
            } else {
                candidates.extend(child.get_children_by_tag("device"));
            }
            if candidates.len() > 1 {
                return Err(PrimaryLinkError::InvalidResponse("ambiguous device result").into());
            }
        }
        let device = candidates
            .first()
            .ok_or(PrimaryLinkError::InvalidResponse("device result missing"))?;
        let jid = device
            .attrs()
            .optional_jid("jid")
            .ok_or(PrimaryLinkError::InvalidResponse("device JID missing"))?;
        validate_companion(&jid, &self.own_accounts)?;
        Ok(PairDeviceAcknowledgement { device_jid: jid })
    }
}

pub struct PublishKeyIndexListSpec {
    server: Jid,
    signed: Vec<u8>,
    timestamp_seconds: u64,
}
impl PublishKeyIndexListSpec {
    pub fn new(
        server: &Jid,
        signed: Vec<u8>,
        timestamp_seconds: u64,
    ) -> Result<Self, PrimaryLinkError> {
        validate_signed_list(&signed, timestamp_seconds)?;
        Ok(Self {
            server: server.clone(),
            signed,
            timestamp_seconds,
        })
    }
}
impl IqSpec for PublishKeyIndexListSpec {
    type Response = ();
    fn build_iq(&self) -> InfoQuery<'static> {
        query(
            &self.server,
            key_index_node(&self.signed, self.timestamp_seconds),
        )
    }
    fn parse_response(&self, response: &NodeRef<'_>) -> Result<(), anyhow::Error> {
        result(response).map_err(Into::into)
    }
}

pub struct RemovePrimaryCompanionSpec {
    server: Jid,
    device: Jid,
    reason: String,
}
impl RemovePrimaryCompanionSpec {
    pub fn new(
        server: &Jid,
        own_accounts: &[Jid],
        device: &Jid,
        reason: &str,
    ) -> Result<Self, PrimaryLinkError> {
        validate_companion(device, own_accounts)?;
        if !bounded_text(reason, 128) {
            return Err(PrimaryLinkError::InvalidInput("revocation reason"));
        }
        Ok(Self {
            server: server.clone(),
            device: device.clone(),
            reason: reason.to_owned(),
        })
    }
}
impl IqSpec for RemovePrimaryCompanionSpec {
    type Response = ();
    fn build_iq(&self) -> InfoQuery<'static> {
        query(
            &self.server,
            NodeBuilder::new("remove-companion-device")
                .attr("jid", &self.device)
                .attr("reason", self.reason.clone())
                .build(),
        )
    }
    fn parse_response(&self, response: &NodeRef<'_>) -> Result<(), anyhow::Error> {
        result(response).map_err(Into::into)
    }
}

pub struct RemoveAllCompanionsSpec {
    server: Jid,
    reason: String,
    exclude_hosted: bool,
}
impl RemoveAllCompanionsSpec {
    pub fn new(server: &Jid, reason: &str, exclude_hosted: bool) -> Result<Self, PrimaryLinkError> {
        if !bounded_text(reason, 128) {
            return Err(PrimaryLinkError::InvalidInput("revocation reason"));
        }
        Ok(Self {
            server: server.clone(),
            reason: reason.to_owned(),
            exclude_hosted,
        })
    }
}
impl IqSpec for RemoveAllCompanionsSpec {
    type Response = ();
    fn build_iq(&self) -> InfoQuery<'static> {
        let mut node = NodeBuilder::new("remove-companion-device")
            .attr("all", "true")
            .attr("reason", self.reason.clone());
        if self.exclude_hosted {
            node = node.attr("exclude_hosted_companion", "true");
        }
        query(&self.server, node.build())
    }
    fn parse_response(&self, response: &NodeRef<'_>) -> Result<(), anyhow::Error> {
        result(response).map_err(Into::into)
    }
}

pub struct PrimaryHelloSpec {
    server: Jid,
    reference: Vec<u8>,
    wrapped: [u8; 80],
    identity: [u8; 32],
}
impl PrimaryHelloSpec {
    pub fn new(
        server: &Jid,
        reference: &[u8],
        wrapped: &[u8; 80],
        identity: &[u8; 32],
    ) -> Result<Self, PrimaryLinkError> {
        validate_reference(reference)?;
        Ok(Self {
            server: server.clone(),
            reference: reference.to_vec(),
            wrapped: *wrapped,
            identity: *identity,
        })
    }
}
impl IqSpec for PrimaryHelloSpec {
    type Response = ();
    fn build_iq(&self) -> InfoQuery<'static> {
        query(
            &self.server,
            NodeBuilder::new("link_code_companion_reg")
                .attr("stage", "primary_hello")
                .children([
                    NodeBuilder::new("link_code_pairing_wrapped_primary_ephemeral_pub")
                        .bytes(self.wrapped)
                        .build(),
                    NodeBuilder::new("primary_identity_pub")
                        .bytes(self.identity)
                        .build(),
                    NodeBuilder::new("link_code_pairing_ref")
                        .bytes(self.reference.clone())
                        .build(),
                ])
                .build(),
        )
    }
    fn parse_response(&self, response: &NodeRef<'_>) -> Result<(), anyhow::Error> {
        result(response).map_err(Into::into)
    }
}

fn query(server: &Jid, node: Node) -> InfoQuery<'static> {
    InfoQuery::set_ref("md", server, Some(NodeContent::Nodes(vec![node])))
}
fn key_index_node(signed: &[u8], timestamp: u64) -> Node {
    NodeBuilder::new("key-index-list")
        .attr("ts", timestamp.to_string())
        .bytes(signed.to_vec())
        .build()
}
fn validate_reference(reference: &[u8]) -> Result<(), PrimaryLinkError> {
    if reference.is_empty() || reference.len() > 4096 {
        Err(PrimaryLinkError::InvalidInput("pairing reference"))
    } else {
        Ok(())
    }
}
fn validate_signed_list(bytes: &[u8], timestamp: u64) -> Result<(), PrimaryLinkError> {
    if bytes.is_empty() || bytes.len() > 16384 {
        return Err(PrimaryLinkError::InvalidInput("signed key-index list"));
    }
    let signed = waproto::codec::adv_signed_key_index_list_decode(bytes)?;
    let details = waproto::codec::adv_key_index_list_decode(
        signed
            .details
            .as_deref()
            .ok_or(PrimaryLinkError::InvalidInput("key-index details"))?,
    )?;
    if signed
        .account_signature
        .as_ref()
        .is_none_or(|s| s.len() != 64)
        || details.timestamp != Some(timestamp)
    {
        return Err(PrimaryLinkError::InvalidInput(
            "key-index signature or timestamp",
        ));
    }
    Ok(())
}
fn result(response: &NodeRef<'_>) -> Result<(), PrimaryLinkError> {
    if response.tag != "iq" || response.attrs().optional_string("type").as_deref() != Some("result")
    {
        return Err(PrimaryLinkError::InvalidResponse("expected IQ result"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::libsignal::protocol::KeyPair;
    use crate::mobile::primary::{
        AdvDefaultEncoding, IdentityParameters, build_signed_companion_identity,
    };
    use rand::SeedableRng;
    use std::str::FromStr;

    #[test]
    fn upload_preserves_bytes_and_rejects_ambiguous_or_foreign_acknowledgements() {
        let mut rng = rand::rngs::StdRng::seed_from_u64(109);
        let primary = KeyPair::generate(&mut rng);
        let peer = KeyPair::generate(&mut rng);
        let params = IdentityParameters {
            raw_id: 3,
            timestamp_seconds: 456,
            key_index: 4,
            valid_indexes: &[0, 4],
            default_encoding: AdvDefaultEncoding::Omit,
        };
        let identity = build_signed_companion_identity(
            &primary,
            peer.public_key.public_key_bytes().try_into().unwrap(),
            &[7; 32],
            &params,
            &mut rng,
        )
        .unwrap();
        let server = Jid::from_str("s.whatsapp.net").unwrap();
        let account = Jid::from_str("123@s.whatsapp.net").unwrap();
        let spec = PairDeviceSpec::new(&server, &[account], b"ref", &[8; 32], identity, 456, None)
            .unwrap();
        let query = spec.build_iq();
        assert_eq!(query.namespace, "md");
        let Some(NodeContent::Nodes(nodes)) = query.content else {
            panic!("missing nodes")
        };
        assert_eq!(nodes[0].tag, "pair-device");
        assert!(
            nodes[0]
                .as_node_ref()
                .get_optional_child("client-props")
                .is_none()
        );
        let device = || {
            NodeBuilder::new("device")
                .attr("jid", "123:2@s.whatsapp.net")
                .build()
        };
        let good = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([device()])
            .build();
        assert_eq!(
            spec.parse_response(&good.as_node_ref())
                .unwrap()
                .device_jid
                .device,
            2
        );
        let ambiguous = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([device(), device()])
            .build();
        assert!(spec.parse_response(&ambiguous.as_node_ref()).is_err());
        let foreign = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("device")
                .attr("jid", "999:2@s.whatsapp.net")
                .build()])
            .build();
        assert!(spec.parse_response(&foreign.as_node_ref()).is_err());
        let error = NodeBuilder::new("iq")
            .attr("type", "error")
            .children([device()])
            .build();
        assert!(spec.parse_response(&error.as_node_ref()).is_err());
    }
    #[test]
    fn optional_pem_hkdf_matches_independent_node_crypto_vector() {
        // Node crypto.hkdfSync SHA256, synthetic 0x07 IKM / 0x08 salt.
        let key = derive_companion_pem_key(&[7; 32], &[8; 32]).unwrap();
        assert_eq!(
            *key,
            [
                45, 230, 238, 48, 67, 125, 249, 56, 238, 139, 246, 39, 49, 8, 124, 49, 45, 218, 98,
                214, 128, 33, 91, 17, 7, 161, 187, 146, 115, 214, 184, 103
            ]
        );
        assert!(PairDevicePem::new([0; 32], 9, b"key", "rsa2048", "1").is_err());
        assert!(PairDevicePem::new([7; 32], 0, b"key", "rsa2048", "1").is_err());
    }
}
