use crate::client::Client;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use wacore::client::context::{GroupRoutingInfo, SendContextResolver};
use wacore::iq::prekeys::PreKeyFetchReason;
use wacore::libsignal::protocol::PreKeyBundle;
use wacore_binary::Jid;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl SendContextResolver for Client {
    /// Recheck the retained certificate anchor under the fanout's account lock.
    async fn validate_prekey_account_identity(
        &self,
        jid: &Jid,
        bundle: &PreKeyBundle,
    ) -> Result<(), anyhow::Error> {
        self.check_prekey_account_identity(jid, bundle)
            .await
            .map_err(Into::into)
    }

    fn sender_identity_role(&self) -> wacore::send::SenderIdentityRole {
        self.sending_identity_role()
    }

    async fn resolve_devices(&self, jids: &[Jid]) -> Result<Vec<Jid>, anyhow::Error> {
        // Only group sends resolve through here.
        self.get_group_user_devices_owned(jids.iter().map(Jid::to_non_ad).collect())
            .await
    }

    async fn fetch_prekeys(
        &self,
        jids: &[Jid],
    ) -> Result<HashMap<Jid, PreKeyBundle>, anyhow::Error> {
        // The fan-out has its own batch-level handling; it wants the bundles only.
        self.fetch_pre_keys(jids, None).await.map(|o| o.bundles)
    }

    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> Result<wacore::prekeys::PreKeyFetchOutcome, anyhow::Error> {
        self.fetch_pre_keys(jids, Some(PreKeyFetchReason::Identity))
            .await
            .map_err(|e| {
                // Re-wrap server errors as wacore::ServerErrorCode so
                // encrypt_for_devices can downcast across crate boundaries
                if let Some(crate::request::IqError::ServerError {
                    code,
                    text,
                    error_type,
                    backoff,
                    ..
                }) = e.downcast_ref::<crate::request::IqError>()
                {
                    return anyhow::Error::new(wacore::request::ServerErrorCode {
                        code: *code,
                        text: text.clone(),
                        error_type: error_type.clone(),
                        backoff: *backoff,
                    });
                }
                e
            })
    }

    async fn resolve_group_routing_info(
        &self,
        jid: &Jid,
    ) -> Result<Arc<GroupRoutingInfo>, anyhow::Error> {
        Ok(self.groups().routing_info(jid).await?)
    }

    async fn get_lid_for_phone(&self, phone_user: &str) -> Option<wacore_binary::CompactString> {
        self.lid_pn_cache.get_current_lid(phone_user).await
    }

    fn on_local_identity_change(&self, jid: &Jid) {
        self.react_to_local_identity_change(
            jid,
            wacore::libsignal::protocol::IdentityChange::ReplacedExisting,
        );
    }

    fn on_local_identity_change_with_keys(
        &self,
        jid: &Jid,
        change: wacore::libsignal::protocol::IdentityChange,
    ) {
        self.react_to_local_identity_change(jid, change);
    }

    fn on_unkeyable_devices(&self, reason: wacore::stats::UnkeyableDevice, count: u64) {
        self.stats.record_unkeyable_devices(reason, count);
    }

    async fn lock_device_sessions(
        &self,
        device_jids: &[Jid],
    ) -> wacore::client::context::SessionLockGuard {
        use wacore::client::context::SessionLockGuard;
        if device_jids.is_empty() {
            return SessionLockGuard::none();
        }
        // Reuse the DM path's helpers so both lock the identical per-device mutexes.
        let keys = self.build_session_lock_keys(device_jids).await;
        SessionLockGuard::hold(Box::new(self.session_guards_for(&keys).await))
    }
}

impl Client {
    /// Derive omission permission from the current validated persisted binding,
    /// never from a device-number heuristic or a missing account certificate.
    pub(crate) fn sending_identity_role(&self) -> wacore::send::SenderIdentityRole {
        use wacore::send::SenderIdentityRole;
        let snapshot = self.persistence_manager.get_device_snapshot();
        #[cfg(feature = "mobile-core")]
        if let Some(session) = &self.mobile_session {
            return if session.validate_device(&snapshot).is_ok() {
                session
                    .primary_identity_policy()
                    .map(SenderIdentityRole::MobilePrimary)
                    .unwrap_or(SenderIdentityRole::Unknown)
            } else {
                SenderIdentityRole::Unknown
            };
        }
        if snapshot.mobile_session.is_some() {
            SenderIdentityRole::Unknown
        } else {
            SenderIdentityRole::Companion
        }
    }
}
