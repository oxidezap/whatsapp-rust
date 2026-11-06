use subtle::ConstantTimeEq;
use wacore_binary::Jid;

use super::{
    PrimaryLinkError, SignedCompanionIdentity, validate_accounts, validate_companion,
    verify_companion_proof,
};

/// Account- and reference-bound pending approval. Hosts retain this alongside
/// their request journal; it contains public signed material, never the QR secret.
/// Dropping it cancels local approval rather than marking a device linked.
pub struct PendingCompanion {
    reference: Vec<u8>,
    own_accounts: Vec<Jid>,
    peer_identity: [u8; 32],
    issued: SignedCompanionIdentity,
}

impl PendingCompanion {
    pub fn new(
        reference: &[u8],
        own_accounts: &[Jid],
        peer_identity: &[u8; 32],
        issued: SignedCompanionIdentity,
    ) -> Result<Self, PrimaryLinkError> {
        validate_accounts(own_accounts)?;
        if issued.device_identity.is_empty()
            || issued.device_identity.len() > 16384
            || issued.key_index_list.is_empty()
            || issued.key_index_list.len() > 16384
        {
            return Err(PrimaryLinkError::InvalidInput("signed companion identity"));
        }
        if reference.is_empty() || reference.len() > 4096 {
            return Err(PrimaryLinkError::InvalidInput("pairing reference"));
        }
        Ok(Self {
            reference: reference.to_vec(),
            own_accounts: own_accounts.to_vec(),
            peer_identity: *peer_identity,
            issued,
        })
    }

    /// Associate the device address returned by this operation's IQ response.
    /// Correlation of the transport request ID remains the IQ executor's job.
    pub fn acknowledge(self, device_jid: &Jid) -> Result<AcknowledgedCompanion, PrimaryLinkError> {
        validate_companion(device_jid, &self.own_accounts)?;
        Ok(AcknowledgedCompanion {
            pending: self,
            device_jid: device_jid.clone(),
        })
    }
}

/// Server acknowledgement alone cannot produce a proof-verified device.
pub struct AcknowledgedCompanion {
    pending: PendingCompanion,
    device_jid: Jid,
}
impl AcknowledgedCompanion {
    /// Bind the independently received notification to this exchange before
    /// checking signatures. On failure the host may retry a valid notification.
    pub fn verify(
        &self,
        reference: &[u8],
        device_jid: &Jid,
        proof: &[u8],
    ) -> Result<VerifiedCompanion, PrimaryLinkError> {
        if !bool::from(self.pending.reference.as_slice().ct_eq(reference))
            || device_jid != &self.device_jid
        {
            return Err(PrimaryLinkError::Authentication);
        }
        verify_companion_proof(
            &self.pending.issued,
            proof,
            &self.pending.peer_identity,
            &self.pending.own_accounts,
            device_jid,
        )?;
        Ok(VerifiedCompanion {
            device_jid: device_jid.clone(),
            peer_identity: self.pending.peer_identity,
            proof: proof.to_vec(),
        })
    }
}

/// Authenticated material awaiting the host's durable DeviceCommand publication.
/// This type deliberately makes no claim about storage or bootstrap completion.
pub struct VerifiedCompanion {
    device_jid: Jid,
    peer_identity: [u8; 32],
    proof: Vec<u8>,
}
impl VerifiedCompanion {
    pub fn device_jid(&self) -> &Jid {
        &self.device_jid
    }
    pub fn peer_identity(&self) -> &[u8; 32] {
        &self.peer_identity
    }
    pub fn signed_identity(&self) -> &[u8] {
        &self.proof
    }
}
