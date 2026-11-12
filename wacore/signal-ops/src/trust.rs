// SPDX-License-Identifier: MIT
// Copyright (C) 2026 Polymorfa (Softium LLC)

//! Peer bundle verification.
//!
//! The client supplies every prekey bundle, so a bundle is only used once it
//! is bound to the peer account's pinned identity:
//! - a primary device's identity *is* the account key. The first one seen
//!   is pinned; a different key later is refused until the application
//!   accepts it with [`SignalOps::trust_account_identity`];
//! - a companion device must arrive with its ADV `<device-identity>`, whose
//!   account and device signatures must verify against the pinned key
//!   (WA Web `WAWebSignalSessionApi.createSignalSession` and
//!   `WAWebAdvSignatureApi.validateADVwithIdentityKey`; wacore's own
//!   prekey parser applies the same rule).
//!
//! Unlike WA Web, a primary identity change is never accepted silently and an
//! in-blob account key never replaces the pinned one: the client that would
//! report either is the party this check distrusts.

use crate::error::OpsError;
use crate::ops::{ServiceStore, SignalOps};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use wacore::adv::{AdvValidation, validate_adv_with_identity_key};
use wacore::client::context::{GroupRoutingInfo, SendContextResolver, SessionLockGuard};
use wacore::libsignal::protocol::PreKeyBundle;
use wacore::prekeys::{PreKeyFetchOutcome, RejectedDevice};
use wacore_binary::{Jid, JidExt as _};
use wacore_recordstore::Namespace;

/// A fetched bundle and the `<device-identity>` the server sent with it.
#[derive(Debug, Clone)]
pub struct FetchedBundle {
    pub bundle: PreKeyBundle,
    /// The `ADVSignedDeviceIdentity` bytes. Required for companion devices;
    /// primary devices carry none.
    pub device_identity: Option<Vec<u8>>,
}

/// What one verifiable prekey fetch produced.
#[derive(Debug, Clone, Default)]
pub struct VerifiablePrekeys {
    pub bundles: HashMap<Jid, FetchedBundle>,
    /// Devices the server rejected by name, as in [`PreKeyFetchOutcome`].
    pub rejected: Vec<RejectedDevice>,
}

/// The client side of a send: wacore's [`SendContextResolver`], plus prekey
/// fetches that keep each companion's `<device-identity>`. The service uses
/// only these fetches to set up sessions and verifies every bundle itself;
/// the resolver's own `fetch_prekeys` is not called.
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
pub trait SignalResolver: SendContextResolver {
    async fn fetch_verifiable_prekeys(&self, jids: &[Jid]) -> anyhow::Result<VerifiablePrekeys>;
}

/// Why a bundle was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BundleRejection {
    #[error("the account's primary identity changed and was not accepted")]
    IdentityChanged,
    #[error("a companion bundle carries no device identity")]
    MissingDeviceIdentity,
    #[error("no pinned account identity to verify the companion against")]
    NoAccountIdentity,
    #[error("the companion's device identity does not verify against the account")]
    InvalidDeviceIdentity,
}

fn account_key(device: &Jid) -> String {
    device.to_non_ad().to_string()
}

fn is_primary(device: &Jid) -> bool {
    device.device == 0 && !device.is_hosted()
}

fn identity_bytes(bundle: &PreKeyBundle) -> Result<[u8; 32], OpsError> {
    Ok(*bundle.identity_key()?.public_key().public_key_bytes())
}

fn decode_pin(bytes: Option<Vec<u8>>) -> Option<[u8; 32]> {
    bytes.and_then(|bytes| <[u8; 32]>::try_from(bytes.as_slice()).ok())
}

impl<S: ServiceStore> SignalOps<S> {
    /// The pinned identity of `account` (any device of it), if one is known.
    pub async fn account_identity(&self, account: &Jid) -> Result<Option<[u8; 32]>, OpsError> {
        Self::pinned(self.store(), account).await
    }

    /// Pin `identity` as `account`'s identity key, replacing any earlier pin.
    /// Call this only once the application itself has accepted the key, for
    /// example after the user confirmed a changed security code: the client
    /// is not trusted to say so.
    pub async fn trust_account_identity(
        &self,
        account: &Jid,
        identity: [u8; 32],
    ) -> Result<(), OpsError> {
        Ok(self
            .store()
            .put_aux(Namespace::AccountIdentity, &account_key(account), &identity)
            .await?)
    }

    async fn pinned(store: &S, device: &Jid) -> Result<Option<[u8; 32]>, OpsError> {
        Ok(decode_pin(
            store
                .load_aux(Namespace::AccountIdentity, &account_key(device))
                .await?,
        ))
    }

    /// Verify `bundle` for `device` and pin a first-seen primary identity in
    /// `store`, which is the batch the session it allows will commit in.
    /// `primary_hint` is the identity of the account's primary device fetched
    /// together with this bundle, used when nothing is pinned yet.
    pub(crate) async fn verify_bundle(
        &self,
        store: &S,
        device: &Jid,
        bundle: &PreKeyBundle,
        device_identity: Option<&[u8]>,
        primary_hint: Option<[u8; 32]>,
    ) -> Result<Result<(), BundleRejection>, OpsError> {
        let identity = identity_bytes(bundle)?;
        let pinned = Self::pinned(store, device).await?;
        if is_primary(device) {
            return Ok(match pinned {
                Some(pinned) if pinned != identity => Err(BundleRejection::IdentityChanged),
                Some(_) => Ok(()),
                None => {
                    store
                        .put_aux(Namespace::AccountIdentity, &account_key(device), &identity)
                        .await?;
                    Ok(())
                }
            });
        }
        let Some(blob) = device_identity else {
            return Ok(Err(BundleRejection::MissingDeviceIdentity));
        };
        let Some(account) = pinned.or(primary_hint) else {
            return Ok(Err(BundleRejection::NoAccountIdentity));
        };
        if validate_adv_with_identity_key(blob, &identity, Some(&account), device)
            != AdvValidation::Valid
        {
            return Ok(Err(BundleRejection::InvalidDeviceIdentity));
        }
        if pinned.is_none() {
            store
                .put_aux(Namespace::AccountIdentity, &account_key(device), &account)
                .await?;
        }
        Ok(Ok(()))
    }

    /// Verify a fetch, returning only the bundles that pass. Primary bundles
    /// are checked first so a fresh account's primary can anchor its
    /// companions in the same fetch.
    pub(crate) async fn verify_fetched(
        &self,
        store: &S,
        fetched: VerifiablePrekeys,
    ) -> Result<(HashMap<Jid, PreKeyBundle>, Vec<RejectedDevice>), OpsError> {
        let mut accepted = HashMap::with_capacity(fetched.bundles.len());
        let mut primaries: HashMap<String, [u8; 32]> = HashMap::new();
        let mut entries: Vec<(Jid, FetchedBundle)> = fetched.bundles.into_iter().collect();
        entries.sort_by_key(|(jid, _)| !is_primary(jid));
        for (jid, fetched) in entries {
            let hint = primaries.get(&account_key(&jid)).copied();
            match self
                .verify_bundle(
                    store,
                    &jid,
                    &fetched.bundle,
                    fetched.device_identity.as_deref(),
                    hint,
                )
                .await?
            {
                Ok(()) => {
                    if is_primary(&jid) {
                        primaries.insert(account_key(&jid), identity_bytes(&fetched.bundle)?);
                    }
                    accepted.insert(jid, fetched.bundle);
                }
                Err(reason) => log::warn!("refusing a prekey bundle for {jid}: {reason}"),
            }
        }
        Ok((accepted, fetched.rejected))
    }
}

/// Wraps the client's resolver for wacore's send path, so every bundle a send
/// installs has passed [`SignalOps::verify_bundle`].
pub(crate) struct Verifying<'a, S> {
    pub ops: &'a SignalOps<S>,
    pub inner: &'a dyn SignalResolver,
    pub store: Arc<S>,
}

fn op_error(error: OpsError) -> anyhow::Error {
    anyhow::anyhow!("{error}")
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<S: ServiceStore> SendContextResolver for Verifying<'_, S> {
    fn sender_identity_role(&self) -> wacore::send::SenderIdentityRole {
        self.inner.sender_identity_role()
    }

    async fn resolve_devices(&self, jids: &[Jid]) -> anyhow::Result<Vec<Jid>> {
        self.inner.resolve_devices(jids).await
    }

    async fn fetch_prekeys(&self, jids: &[Jid]) -> anyhow::Result<HashMap<Jid, PreKeyBundle>> {
        Ok(self.fetch_prekeys_for_identity_check(jids).await?.bundles)
    }

    async fn fetch_prekeys_for_identity_check(
        &self,
        jids: &[Jid],
    ) -> anyhow::Result<PreKeyFetchOutcome> {
        let fetched = self.inner.fetch_verifiable_prekeys(jids).await?;
        let (bundles, rejected) = self
            .ops
            .verify_fetched(&self.store, fetched)
            .await
            .map_err(op_error)?;
        Ok(PreKeyFetchOutcome { bundles, rejected })
    }

    async fn resolve_group_routing_info(&self, jid: &Jid) -> anyhow::Result<Arc<GroupRoutingInfo>> {
        self.inner.resolve_group_routing_info(jid).await
    }

    async fn get_lid_for_phone(&self, phone_user: &str) -> Option<wacore_binary::CompactString> {
        self.inner.get_lid_for_phone(phone_user).await
    }

    fn on_local_identity_change(&self, jid: &Jid) {
        self.inner.on_local_identity_change(jid);
    }

    fn on_local_identity_change_with_keys(
        &self,
        jid: &Jid,
        change: wacore::libsignal::protocol::IdentityChange,
    ) {
        self.inner.on_local_identity_change_with_keys(jid, change);
    }

    fn on_unkeyable_devices(&self, reason: wacore::stats::UnkeyableDevice, count: u64) {
        self.inner.on_unkeyable_devices(reason, count);
    }

    async fn lock_device_sessions(&self, device_jids: &[Jid]) -> SessionLockGuard {
        self.inner.lock_device_sessions(device_jids).await
    }
}
