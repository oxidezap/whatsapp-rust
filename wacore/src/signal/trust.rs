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

use crate::adv::{AdvValidation, validate_adv_with_identity_key};
use crate::client::context::{GroupRoutingInfo, SendContextResolver, SessionLockGuard};
use crate::libsignal::protocol::{PreKeyBundle, ProtocolAddress};
use crate::prekeys::{PreKeyFetchOutcome, RejectedDevice};
use crate::signal::error::OpsError;
use crate::signal::ops::{SignalOperationStore, SignalOps};
use crate::store::transaction::SignalRecord as Namespace;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use wacore_binary::{Jid, JidExt as _};

pub use super::error::BundleRejection;
pub use super::resolver::{FetchedBundle, SignalResolver, VerifiablePrekeys};

/// The device a Signal address names. WhatsApp addresses carry the device
/// in the name (Signal's device id is always 0) and spell the PN server as
/// legacy `c.us`; a plain libsignal address keeps it in the device id.
pub(crate) fn device_of(address: &ProtocolAddress) -> Result<Jid, OpsError> {
    let name = address.name();
    let name = name
        .strip_suffix("@c.us")
        .map_or_else(|| name.to_owned(), |user| format!("{user}@s.whatsapp.net"));
    let device = name
        .parse::<Jid>()
        .map_err(|_| OpsError::InvalidInput(format!("{address} is not a device address")))?;
    let device_id = u16::try_from(u32::from(address.device_id()))
        .map_err(|_| OpsError::InvalidInput("device id out of range".into()))?;
    Ok(if device.device == 0 && device_id != 0 {
        device.with_device(device_id)
    } else {
        device
    })
}

/// The record key of `device`'s account pin. A hosted device belongs to
/// its ordinary account (`hosted` to the phone number, `hosted.lid` to the
/// LID), as in WA Web's `asUserWidOrThrow`, so it shares that account's pin.
fn account_key(device: &Jid) -> String {
    crate::types::jid::adv_account_jid(device).to_string()
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

impl<S: SignalOperationStore> SignalOps<S> {
    /// The pinned identity of `account` (any device of it), if one is known.
    pub async fn account_identity(&self, account: &Jid) -> Result<Option<[u8; 32]>, OpsError> {
        Self::pinned(self.store(), account).await
    }

    /// Pin `identity` as `account`'s identity key, replacing any earlier pin.
    /// Call this only once the application itself has accepted the key, for
    /// example after the user confirmed a changed security code: the client
    /// is not trusted to say so. It waits for sends, receives and migrations
    /// that hold the account, so none of them can write over it afterwards.
    pub async fn trust_account_identity(
        &self,
        account: &Jid,
        identity: [u8; 32],
    ) -> Result<(), OpsError> {
        let held = self
            .lock_keys(vec![crate::signal::ops::account_lock(&account.user)])
            .await?;
        held.run(async {
            Ok(self
                .store()
                .put_aux(Namespace::AccountIdentity, &account_key(account), &identity)
                .await?)
        })
        .await
    }

    async fn pinned(store: &S, device: &Jid) -> Result<Option<[u8; 32]>, OpsError> {
        Ok(decode_pin(
            store
                .load_aux(Namespace::AccountIdentity, &account_key(device))
                .await?,
        ))
    }

    /// Whether `device` is one of this account's devices, by its stored PN
    /// and LID. Never taken from the client.
    pub(crate) async fn is_own_account(&self, device: &Jid) -> bool {
        let keys = self.keys().await;
        let account = crate::types::jid::adv_account_jid(device);
        [keys.own_pn.as_deref(), keys.own_lid.as_deref()]
            .into_iter()
            .flatten()
            .filter_map(|jid| jid.parse::<Jid>().ok())
            .any(|jid| jid.to_non_ad() == account)
    }

    /// This device's own account key, from its pairing, when `device` belongs
    /// to this account. Own devices are anchored to it rather than to a pin.
    async fn own_account(&self, device: &Jid) -> Option<[u8; 32]> {
        if !self.is_own_account(device).await {
            return None;
        }
        let keys = self.keys().await;
        let signed =
            waproto::codec::adv_signed_device_identity_decode(keys.account.as_deref()?).ok()?;
        <[u8; 32]>::try_from(signed.account_signature_key?.as_slice()).ok()
    }

    /// Verify `bundle` for `device`; see [`Self::verify_identity`].
    pub(crate) async fn verify_bundle(
        &self,
        store: &S,
        device: &Jid,
        bundle: &PreKeyBundle,
        device_identity: Option<&[u8]>,
        primary_hint: Option<[u8; 32]>,
    ) -> Result<Result<(), BundleRejection>, OpsError> {
        self.verify_identity(
            store,
            device,
            identity_bytes(bundle)?,
            device_identity,
            primary_hint,
        )
        .await
    }

    /// Verify that `identity` may start a session with `device`, pinning a
    /// first-seen account identity in `store`, the batch that session commits
    /// in. `primary_hint` is the account's primary identity fetched in the
    /// same batch, used when nothing is pinned yet. A first-use pin is an
    /// exclusive insert, so a key the application accepted meanwhile is never
    /// overwritten; that commit fails instead.
    pub(crate) async fn verify_identity(
        &self,
        store: &S,
        device: &Jid,
        identity: [u8; 32],
        device_identity: Option<&[u8]>,
        primary_hint: Option<[u8; 32]>,
    ) -> Result<Result<(), BundleRejection>, OpsError> {
        let own = self.own_account(device).await;
        let pinned = match own {
            Some(own) => Some(own),
            None => Self::pinned(store, device).await?,
        };
        if is_primary(device) {
            return Ok(match pinned {
                Some(pinned) if pinned != identity => Err(BundleRejection::IdentityChanged),
                Some(_) => Ok(()),
                None => {
                    store
                        .insert_aux(Namespace::AccountIdentity, &account_key(device), &identity)
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
                .insert_aux(Namespace::AccountIdentity, &account_key(device), &account)
                .await?;
        }
        Ok(Ok(()))
    }

    /// Copy `from`'s account pin to `to`'s account in `store` when `to` has
    /// none. Both name the same account in different address forms. False
    /// when `to` is already pinned to another key: a session at `to` must not
    /// be built from keys that only `from`'s pin vouches for.
    pub(crate) async fn mirror_pin(
        &self,
        store: &S,
        from: &Jid,
        to: &Jid,
    ) -> Result<bool, OpsError> {
        let Some(pin) = Self::pinned(store, from).await? else {
            return Ok(true);
        };
        match Self::pinned(store, to).await? {
            Some(existing) => Ok(existing == pin),
            None => {
                store
                    .insert_aux(Namespace::AccountIdentity, &account_key(to), &pin)
                    .await?;
                Ok(true)
            }
        }
    }

    /// Whether `device` is one of this account's own devices, anchored by
    /// this device's pairing. Before pairing nothing is: a sender that only
    /// names the stored own phone number or LID proves nothing.
    pub(crate) async fn is_anchored_own_device(&self, device: &Jid) -> bool {
        self.own_account(device).await.is_some()
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
    /// The LID account each phone-number user is encrypted to in this
    /// operation, as fixed before its locks were taken.
    pub lids: HashMap<String, Jid>,
}

impl<'a, S> Verifying<'a, S> {
    pub fn new(ops: &'a SignalOps<S>, inner: &'a dyn SignalResolver, store: Arc<S>) -> Self {
        Self {
            ops,
            inner,
            store,
            lids: HashMap::new(),
        }
    }

    /// Record the addresses an operation encrypts to, `(device, address)`.
    pub fn with_addresses<'j>(
        mut self,
        pairs: impl IntoIterator<Item = (&'j Jid, &'j Jid)>,
    ) -> Self {
        for (device, address) in pairs {
            if device.is_pn() && address.is_lid() {
                self.lids
                    .insert(device.user.to_string(), address.to_non_ad());
            }
        }
        self
    }
}

/// Carry `error` through wacore's `anyhow` boundary intact, so the send
/// can return it typed (lost write authority must remain typed).
pub(crate) fn op_error(error: OpsError) -> anyhow::Error {
    anyhow::Error::new(error)
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl<S: SignalOperationStore> SendContextResolver for Verifying<'_, S> {
    fn sender_identity_role(&self) -> crate::send::SenderIdentityRole {
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
        // A session for a PN device is set up at its LID when the mapping is
        // known, and later messages arrive from that LID; pin the account
        // there too. An existing LID pin is kept, and when it names another
        // key the user's bundles are dropped: they verified only against the
        // PN pin, and the session would be built at the LID.
        let mut bundles = bundles;
        let mut mapped: Vec<Jid> = bundles
            .keys()
            .filter(|jid| jid.is_pn() && self.lids.contains_key(jid.user.as_str()))
            .cloned()
            .collect();
        mapped.sort_by_key(|jid| jid.to_string());
        let mut checked = std::collections::HashSet::new();
        let mut conflicting = std::collections::HashSet::new();
        for jid in mapped {
            let lid = &self.lids[jid.user.as_str()];
            // Per PN user: two users mapped to one LID each need their own
            // pin to agree with it.
            if checked.insert((jid.user.clone(), lid.clone()))
                && !self
                    .ops
                    .mirror_pin(&self.store, &jid, lid)
                    .await
                    .map_err(op_error)?
            {
                conflicting.insert(jid.user.clone());
            }
        }
        bundles.retain(|jid, _| !(jid.is_pn() && conflicting.contains(&jid.user)));
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
        change: crate::libsignal::protocol::IdentityChange,
    ) {
        self.inner.on_local_identity_change_with_keys(jid, change);
    }

    fn on_unkeyable_devices(&self, reason: crate::stats::UnkeyableDevice, count: u64) {
        self.inner.on_unkeyable_devices(reason, count);
    }

    async fn lock_device_sessions(&self, device_jids: &[Jid]) -> SessionLockGuard {
        self.inner.lock_device_sessions(device_jids).await
    }
}
