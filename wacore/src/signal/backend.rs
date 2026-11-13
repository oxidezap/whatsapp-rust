//! Operation boundary for consumers that keep Signal execution elsewhere.
//!
//! A backend owns private keys, ratchets and durable operation completion.
//! Callers exchange public pairing data, ciphertext and explicit delivery
//! outcomes. No storage implementation, lease or transport is prescribed.
//! An error never authorizes falling back to another backend. After cancellation,
//! the backend must reconcile any uncertain write before accepting more work.
//!
//! This contract is independent of the Tokio client's storage adapters. Its
//! built-in implementation delegates to `SignalOps` without another engine.

use super::{
    OpsError, PairingSignature, PreKeyUploadBatch, PreparedGroupSend, PreparedSend, PublicIdentity,
    PublicPreKey, PublicSignedPreKey, ReceiveRequest, Received, RetryRequest, SignalResolver,
};
#[cfg(feature = "signal-operations")]
use super::{SignalOperationStore, SignalOps};
use crate::runtime::Runtime;
use crate::sync_marker::MaybeSendSync;
use wacore_binary::Jid;
use waproto::whatsapp as wa;

/// Whether the caller should process content or delivery completed elsewhere.
#[derive(Debug)]
pub enum ReceiveOutcome {
    Content(Received),
    /// The backend committed the Signal update and delivered content itself.
    DeliveredExternally,
}

/// Object-safe operations for a local, browser or remote implementation.
///
/// Native implementations are `Send + Sync`; wasm implementations may own JS
/// handles directly. Stanza transmission and receipt orchestration stay with
/// the caller. `DeliveredExternally` contains no message plaintext.
#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
pub trait SignalBackend: MaybeSendSync {
    async fn set_own_jids(&self, pn: &str, lid: Option<&str>) -> Result<(), OpsError>;
    async fn public_identity(&self) -> Result<PublicIdentity, OpsError>;
    async fn generate_prekeys(
        &self,
        runtime: &dyn Runtime,
        count: u32,
    ) -> Result<Vec<PublicPreKey>, OpsError>;
    async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> Result<(), OpsError>;
    async fn rotate_signed_prekey(&self) -> Result<PublicSignedPreKey, OpsError>;
    async fn forget_user(&self, user: &Jid) -> Result<(), OpsError>;
    async fn migrate_pn_to_lid(&self, pn: &Jid, lid: &Jid) -> Result<usize, OpsError>;
    async fn sign_pairing(
        &self,
        device_identity_container: &[u8],
    ) -> Result<PairingSignature, OpsError>;
    async fn prepare_prekey_upload(
        &self,
        runtime: &dyn Runtime,
        count: u32,
    ) -> Result<PreKeyUploadBatch, OpsError>;
    async fn acknowledge_prekey_upload(
        &self,
        batch_id: &str,
        exact_ids: &[u32],
    ) -> Result<(), OpsError>;
    async fn receive(&self, request: ReceiveRequest<'_>) -> Result<ReceiveOutcome, OpsError>;
    async fn mark_delivered(&self, receipt_key: &str) -> Result<(), OpsError>;
    async fn send_direct(
        &self,
        runtime: &dyn Runtime,
        resolver: &dyn SignalResolver,
        to: &Jid,
        message: &wa::Message,
        message_id: &str,
    ) -> Result<PreparedSend, OpsError>;
    async fn send_group(
        &self,
        runtime: &dyn Runtime,
        resolver: &dyn SignalResolver,
        group: &Jid,
        message: &wa::Message,
        message_id: &str,
    ) -> Result<PreparedGroupSend, OpsError>;
    async fn mark_sender_key_distributed(
        &self,
        group: &Jid,
        devices: &[Jid],
        distribution_epoch: u64,
    ) -> Result<bool, OpsError>;
    async fn forget_sender_key_devices(&self, group: &Jid) -> Result<(), OpsError>;
    async fn resend(
        &self,
        resolver: &dyn SignalResolver,
        request: RetryRequest<'_>,
    ) -> Result<Vec<u8>, OpsError>;
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
#[cfg(feature = "signal-operations")]
impl<S: SignalOperationStore> SignalBackend for SignalOps<S> {
    async fn set_own_jids(&self, pn: &str, lid: Option<&str>) -> Result<(), OpsError> {
        SignalOps::set_own_jids(self, pn, lid).await
    }
    async fn public_identity(&self) -> Result<PublicIdentity, OpsError> {
        Ok(SignalOps::public_identity(self).await)
    }
    async fn generate_prekeys(
        &self,
        runtime: &dyn Runtime,
        count: u32,
    ) -> Result<Vec<PublicPreKey>, OpsError> {
        SignalOps::generate_prekeys(self, runtime, count).await
    }
    async fn mark_prekeys_uploaded(&self, ids: &[u32]) -> Result<(), OpsError> {
        SignalOps::mark_prekeys_uploaded(self, ids).await
    }
    async fn rotate_signed_prekey(&self) -> Result<PublicSignedPreKey, OpsError> {
        SignalOps::rotate_signed_prekey(self).await
    }
    async fn forget_user(&self, user: &Jid) -> Result<(), OpsError> {
        SignalOps::forget_user(self, user).await
    }
    async fn migrate_pn_to_lid(&self, pn: &Jid, lid: &Jid) -> Result<usize, OpsError> {
        SignalOps::migrate_pn_to_lid(self, pn, lid).await
    }
    async fn sign_pairing(
        &self,
        device_identity_container: &[u8],
    ) -> Result<PairingSignature, OpsError> {
        SignalOps::sign_pairing(self, device_identity_container).await
    }
    async fn prepare_prekey_upload(
        &self,
        runtime: &dyn Runtime,
        count: u32,
    ) -> Result<PreKeyUploadBatch, OpsError> {
        SignalOps::prepare_prekey_upload(self, runtime, count).await
    }
    async fn acknowledge_prekey_upload(
        &self,
        batch_id: &str,
        exact_ids: &[u32],
    ) -> Result<(), OpsError> {
        SignalOps::acknowledge_prekey_upload(self, batch_id, exact_ids).await
    }
    async fn receive(&self, request: ReceiveRequest<'_>) -> Result<ReceiveOutcome, OpsError> {
        SignalOps::receive(self, request)
            .await
            .map(ReceiveOutcome::Content)
    }
    async fn mark_delivered(&self, receipt_key: &str) -> Result<(), OpsError> {
        SignalOps::mark_delivered(self, receipt_key).await
    }
    async fn send_direct(
        &self,
        runtime: &dyn Runtime,
        resolver: &dyn SignalResolver,
        to: &Jid,
        message: &wa::Message,
        message_id: &str,
    ) -> Result<PreparedSend, OpsError> {
        SignalOps::send_direct(self, runtime, resolver, to, message, message_id).await
    }
    async fn send_group(
        &self,
        runtime: &dyn Runtime,
        resolver: &dyn SignalResolver,
        group: &Jid,
        message: &wa::Message,
        message_id: &str,
    ) -> Result<PreparedGroupSend, OpsError> {
        SignalOps::send_group(self, runtime, resolver, group, message, message_id).await
    }
    async fn mark_sender_key_distributed(
        &self,
        group: &Jid,
        devices: &[Jid],
        distribution_epoch: u64,
    ) -> Result<bool, OpsError> {
        SignalOps::mark_sender_key_distributed(self, group, devices, distribution_epoch).await
    }
    async fn forget_sender_key_devices(&self, group: &Jid) -> Result<(), OpsError> {
        SignalOps::forget_sender_key_devices(self, group).await
    }
    async fn resend(
        &self,
        resolver: &dyn SignalResolver,
        request: RetryRequest<'_>,
    ) -> Result<Vec<u8>, OpsError> {
        SignalOps::resend(self, resolver, request).await
    }
}
