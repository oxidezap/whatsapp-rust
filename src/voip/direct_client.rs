//! The native client's implementation of direct-call host capabilities.

use std::sync::Arc;

use async_trait::async_trait;
use wacore::message_processing::EncType;
use wacore::runtime::Runtime;
use wacore::stanza::call::OfferDeviceKey;
use wacore::stats::UnkeyableDevice;
use wacore_binary::{Jid, Node, Server};

use super::direct::{CallAckFuture, DirectCallHost, DirectCallKeys, DirectCallState};
use super::facade::{drop_hosted_devices, keep_non_pkmsg_devices};
use crate::client::{CallError, Client};

impl Client {
    pub(crate) fn direct_host(&self) -> Result<Arc<dyn DirectCallHost>, CallError> {
        self.self_weak
            .get()
            .and_then(std::sync::Weak::upgrade)
            .map(|client| client as Arc<dyn DirectCallHost>)
            .ok_or(CallError::CallEndedDuringSetup)
    }
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl DirectCallHost for Client {
    fn call_state(&self) -> &DirectCallState {
        &self.voip_state().direct
    }

    fn runtime(&self) -> Arc<dyn Runtime> {
        self.runtime.clone()
    }

    fn own_lid(&self) -> Option<Jid> {
        self.lid()
    }

    fn own_pn(&self) -> Option<Jid> {
        self.pn()
    }

    async fn namespace_alias(&self, jid: &Jid) -> Option<Jid> {
        self.swap_pn_lid_namespace(jid).await
    }

    async fn learn_identity(
        &self,
        lid: &Jid,
        pn: &Jid,
        offline: bool,
        has_username: bool,
    ) -> Result<(), CallError> {
        use crate::lid_pn_cache::LearningSource;
        use wacore::iq::abprops::web;

        if has_username {
            let privacy = self.ab_props.snapshot().await;
            if (self.ab_props_fetch_enabled() && !privacy.applied_in_generation())
                || (privacy.is_enabled(web::USERNAME_CONTACT_DISPLAY)
                    && privacy.is_enabled(web::ENABLE_CALLING_PHONE_NUMBER_PRIVACY))
            {
                return Ok(());
            }
        }
        if offline {
            let client = self
                .self_weak
                .get()
                .and_then(std::sync::Weak::upgrade)
                .ok_or(CallError::CallEndedDuringSetup)?;
            client
                .learn_lid_pn_mapping_fast(&lid.user, &pn.user, LearningSource::Other, true)
                .await;
            Ok(())
        } else {
            self.add_lid_pn_mapping(&lid.user, &pn.user, LearningSource::Other)
                .await
                .map_err(|error| CallError::Setup(error.to_string()))
        }
    }

    fn is_connected(&self) -> bool {
        self.is_socket_connected()
    }

    fn generate_request_id(&self) -> String {
        Client::generate_request_id(self)
    }

    async fn resolve_peer(&self, peer: &Jid) -> Result<Jid, CallError> {
        if peer.server == Server::Lid {
            return Ok(peer.clone());
        }
        if let Some(lid) = self.lid_pn_cache.get_current_lid(&peer.user).await {
            return Ok(Jid::new(lid.as_str(), Server::Lid));
        }
        self.signal()
            .get_user_devices(std::slice::from_ref(peer))
            .await
            .map_err(|error| CallError::Setup(error.to_string()))?;
        self.lid_pn_cache
            .get_current_lid(&peer.user)
            .await
            .map(|lid| Jid::new(lid.as_str(), Server::Lid))
            .ok_or(CallError::Media(
                "no known LID for the PN callee; cannot derive media keys",
            ))
    }

    async fn prepare_keys(
        &self,
        peer: &Jid,
        plaintext: &[u8],
    ) -> Result<DirectCallKeys, CallError> {
        let epoch = self.call_state().epoch();
        let mut devices = drop_hosted_devices(
            self.signal()
                .get_user_devices(std::slice::from_ref(peer))
                .await
                .map_err(|error| CallError::Setup(error.to_string()))?,
        );
        self.call_state().ensure_epoch(epoch)?;
        if devices.is_empty() {
            return Err(CallError::NoDevices);
        }
        let ring_devices = devices.clone();
        if self
            .persistence_manager()
            .get_device_snapshot()
            .account
            .is_none()
        {
            let lock_jids = self.build_session_lock_keys(&devices).await;
            let guards = self.session_guards_for(&lock_jids).await;
            self.call_state().ensure_epoch(epoch)?;
            let mut would_pkmsg = Vec::with_capacity(devices.len());
            for device in &devices {
                self.call_state().ensure_epoch(epoch)?;
                would_pkmsg.push(
                    self.would_emit_pkmsg(device)
                        .await
                        .map_err(|error| CallError::Setup(error.to_string()))?,
                );
            }
            drop(guards);
            devices = keep_non_pkmsg_devices(devices, &would_pkmsg)?;
        }
        self.signal()
            .assert_sessions(&devices)
            .await
            .map_err(|error| CallError::Setup(error.to_string()))?;
        self.call_state().ensure_epoch(epoch)?;
        encrypt_known_call_keys(self, &devices, &ring_devices, plaintext).await
    }

    async fn decrypt(
        &self,
        peer: &Jid,
        enc_type: EncType,
        ciphertext: &[u8],
    ) -> Result<Vec<u8>, CallError> {
        self.signal()
            .decrypt_message(peer, enc_type, ciphertext)
            .await
            .map_err(|error| CallError::Decrypt(error.to_string()))
    }

    async fn read_token(&self, peer: &Jid) -> Result<Option<Vec<u8>>, CallError> {
        Ok(self.lookup_tc_token_for_jid(peer).await)
    }

    async fn issue_token(&self, peer: &Jid) {
        if self.should_issue_tc_token(peer).await {
            self.issue_tc_token_after_send(peer).await;
        }
    }

    async fn send_node(&self, node: Node) -> Result<(), CallError> {
        Client::send_node(self, node).await.map_err(Into::into)
    }

    fn watch_ack(&self, id: &str) -> Result<CallAckFuture, CallError> {
        let (receiver, generation) = self
            .try_register_ack_waiter(id)
            .ok_or_else(|| CallError::Response("duplicate call request id".into()))?;
        let guard = crate::request::ResponseWaiterGuard::new(
            self.response_waiters.clone(),
            id.to_owned(),
            generation,
        );
        Ok(Box::pin(async move {
            let _guard = guard;
            receiver
                .await
                .map(|node| node.to_owned_node())
                .map_err(|error| CallError::Response(error.to_string()))
        }))
    }
}

/// Kept separate from discovery for native callers that already resolved and locked their device set.
pub(crate) async fn encrypt_known_call_keys(
    client: &Client,
    devices: &[Jid],
    ring_devices: &[Jid],
    plaintext: &[u8],
) -> Result<DirectCallKeys, CallError> {
    let epoch = client.voip_state().direct.epoch();
    let raw = {
        let lock_jids = client.build_session_lock_keys(devices).await;
        let guards = client.session_guards_for(&lock_jids).await;
        client.voip_state().direct.ensure_epoch(epoch)?;
        let plan = wacore::send::SessionPlan::assume_ready(devices.len());
        let mut adapter = client.signal_adapter();
        let mut stores = adapter.as_signal_stores();
        let raw = wacore::send::encrypt_for_devices_with_sessions_raw(
            &*client.runtime,
            &mut stores,
            devices,
            plaintext,
            plan,
        )
        .await
        .map_err(|error| CallError::Setup(error.to_string()))?;
        client
            .stats
            .record_unkeyable_devices(UnkeyableDevice::Encrypt, raw.unkeyed_at_encrypt);
        drop(guards);
        client
            .persist_signal_state_pre_wire()
            .await
            .map_err(|error| CallError::Setup(error.to_string()))?;
        raw
    };
    let device_keys: Vec<OfferDeviceKey> = devices
        .iter()
        .filter_map(|device| raw.devices.iter().find(|entry| &entry.device_jid == device))
        .map(|entry| OfferDeviceKey {
            device_jid: entry.device_jid.clone(),
            ciphertext: entry.ciphertext.clone(),
            enc_type: entry.enc_type.to_string(),
        })
        .collect();
    if device_keys.is_empty() {
        return Err(CallError::NoDevices);
    }
    let account = client
        .persistence_manager()
        .get_device_snapshot()
        .account
        .clone();
    let device_identity =
        wacore::send::needs_device_identity(raw.includes_prekey_message, account.as_deref())
            .map_err(|_| CallError::MissingDeviceIdentity)?;
    Ok(DirectCallKeys {
        device_keys,
        ring_devices: ring_devices.to_vec(),
        device_identity,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use wacore_binary::{OwnedNodeRef, builder::NodeBuilder};

    #[tokio::test]
    async fn ack_registration_is_immediate_and_cancelled_future_releases_it() {
        let client = crate::test_utils::create_test_client().await;
        let first = client.watch_ack("DIRECT-WAITER").expect("reserve waiter");
        assert!(client.try_register_ack_waiter("DIRECT-WAITER").is_none());
        drop(first);
        let second = client
            .watch_ack("DIRECT-WAITER")
            .expect("release on cancellation");
        drop(second);
        assert!(client.try_register_ack_waiter("DIRECT-WAITER").is_some());
    }

    #[tokio::test]
    async fn ack_error_metadata_survives_host_correlation_without_query_coercion() {
        let client = crate::test_utils::create_test_client().await;
        let ack = NodeBuilder::new("ack")
            .attr("id", "DIRECT-ERROR")
            .attr("class", "call")
            .attr("type", "offer")
            .attr("error", "synthetic-error")
            .attr("code", "503")
            .build();
        let waiting = client
            .watch_ack("DIRECT-ERROR")
            .expect("reserve before receipt");
        let bytes = wacore_binary::marshal::marshal_ref(&ack.as_node_ref()).expect("marshal ack");
        let parsed = OwnedNodeRef::new(
            wacore_binary::util::unpack(&bytes)
                .expect("unpack")
                .into_owned(),
        )
        .expect("parse ack");
        assert!(client.handle_ack_response_owned(parsed));
        let received = waiting
            .await
            .expect("raw ack remains a successful host result");
        assert_eq!(received, ack);
        assert!(client.try_register_ack_waiter("DIRECT-ERROR").is_some());
    }

    #[tokio::test]
    async fn native_and_host_paths_share_the_same_registry_and_transition_lane() {
        let client = crate::test_utils::create_test_client().await;
        let host = client.direct_host().expect("host from native lifetime");
        assert!(Arc::ptr_eq(
            host.call_state().registry(),
            &client.call_registry()
        ));
        assert!(Arc::ptr_eq(
            &host.call_state().transition("DIRECT-LANE"),
            &client.answer_transition_lock("DIRECT-LANE"),
        ));
    }
}
