use wacore::libsignal::protocol::{DeviceId, ProtocolAddress};
use wacore::runtime::Runtime;
use wacore::signal::{
    OpsError, PairingSignature, PreKeyUploadBatch, PreparedGroupSend, PreparedSend, PublicIdentity,
    PublicPreKey, PublicSignedPreKey, ReceiveKind, ReceiveOutcome, ReceiveRequest, RetryRequest,
    SignalBackend, SignalResolver,
};
use wacore_binary::Jid;
use waproto::whatsapp as wa;

#[cfg(target_arch = "wasm32")]
type Counter = std::rc::Rc<std::cell::Cell<usize>>;
#[cfg(not(target_arch = "wasm32"))]
type Counter = std::sync::Arc<std::sync::atomic::AtomicUsize>;

#[derive(Default)]
struct ExternalBackend {
    calls: Counter,
}

impl ExternalBackend {
    fn bump(&self) {
        #[cfg(target_arch = "wasm32")]
        self.calls.set(self.calls.get() + 1);
        #[cfg(not(target_arch = "wasm32"))]
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }

    fn count(&self) -> usize {
        #[cfg(target_arch = "wasm32")]
        {
            self.calls.get()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            self.calls.load(std::sync::atomic::Ordering::SeqCst)
        }
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait::async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait::async_trait)]
impl SignalBackend for ExternalBackend {
    async fn set_own_jids(&self, _pn: &str, _lid: Option<&str>) -> Result<(), OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn public_identity(&self) -> Result<PublicIdentity, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn generate_prekeys(
        &self,
        _runtime: &dyn Runtime,
        _count: u32,
    ) -> Result<Vec<PublicPreKey>, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn mark_prekeys_uploaded(&self, _ids: &[u32]) -> Result<(), OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn rotate_signed_prekey(&self) -> Result<PublicSignedPreKey, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn forget_user(&self, _user: &Jid) -> Result<(), OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn migrate_pn_to_lid(&self, _pn: &Jid, _lid: &Jid) -> Result<usize, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn sign_pairing(
        &self,
        _device_identity_container: &[u8],
    ) -> Result<PairingSignature, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn prepare_prekey_upload(
        &self,
        _runtime: &dyn Runtime,
        _count: u32,
    ) -> Result<PreKeyUploadBatch, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn acknowledge_prekey_upload(
        &self,
        _batch_id: &str,
        _exact_ids: &[u32],
    ) -> Result<(), OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn receive(&self, _request: ReceiveRequest<'_>) -> Result<ReceiveOutcome, OpsError> {
        self.bump();
        Ok(ReceiveOutcome::DeliveredExternally)
    }
    async fn mark_delivered(&self, _receipt_key: &str) -> Result<(), OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn send_direct(
        &self,
        _runtime: &dyn Runtime,
        _resolver: &dyn SignalResolver,
        _to: &Jid,
        _message: &wa::Message,
        _message_id: &str,
    ) -> Result<PreparedSend, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn send_group(
        &self,
        _runtime: &dyn Runtime,
        _resolver: &dyn SignalResolver,
        _group: &Jid,
        _message: &wa::Message,
        _message_id: &str,
    ) -> Result<PreparedGroupSend, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn mark_sender_key_distributed(
        &self,
        _group: &Jid,
        _devices: &[Jid],
        _distribution_epoch: u64,
    ) -> Result<bool, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn forget_sender_key_devices(&self, _group: &Jid) -> Result<(), OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
    async fn resend(
        &self,
        _resolver: &dyn SignalResolver,
        _request: RetryRequest<'_>,
    ) -> Result<Vec<u8>, OpsError> {
        Err(OpsError::InvalidInput("unused fixture operation".into()))
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test::wasm_bindgen_test)]
#[cfg_attr(not(target_arch = "wasm32"), test)]
fn external_delivery_needs_no_local_keys_or_store() {
    let external = ExternalBackend::default();
    let backend: &dyn SignalBackend = &external;
    let sender = ProtocolAddress::new("15555550100@s.whatsapp.net", DeviceId::from(1));
    let result = futures::executor::block_on(backend.receive(ReceiveRequest {
        chat: "15555550100@s.whatsapp.net",
        sender: &sender,
        kind: ReceiveKind::Message,
        ciphertext: b"external-payload",
        padding_version: 2,
        is_from_me: false,
        device_identity: None,
    }))
    .unwrap();
    assert!(matches!(result, ReceiveOutcome::DeliveredExternally));
    assert_eq!(external.count(), 1);
}
