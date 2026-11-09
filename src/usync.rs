//! User device list synchronization.
//!
//! Device list IQ specification is defined in `wacore::iq::usync`.

use crate::client::{Client, SignalMaintenance};
use crate::request::IqError;
use log::{debug, warn};
use wacore::iq::usync::{DeviceListResponse, DeviceListSpec};
use wacore_binary::{Jid, JidExt as _};

#[cfg(test)]
#[path = "usync/hosted_recovery_tests.rs"]
mod hosted_recovery_tests;

#[cfg(test)]
mod ordinary_recovery_tests;

/// An authoritative refresh retries when a newer registry mutation wins while
/// its IQ is in flight. Bound retries so a continuously changing account never
/// turns one send into an unbounded request loop.
const DEVICE_REFRESH_MAX_ATTEMPTS: usize = 3;

/// Bound sequential key recovery IQs across all topology retries of one group.
const GROUP_ADV_RECOVERY_LIMIT: usize = 8;

struct GroupAdvRecoveryBudget {
    remaining: usize,
    attempted: std::collections::HashSet<Jid>,
}

impl GroupAdvRecoveryBudget {
    fn take(&mut self, user: &Jid) -> bool {
        if self.remaining == 0 || !self.attempted.insert(user.to_non_ad()) {
            return false;
        }
        self.remaining -= 1;
        true
    }
}

/// A failed list never authorizes replacing the account's trusted identity.
#[derive(thiserror::Error)]
#[error("device list ADV signature verification failed for {user} (hosted={hosted})")]
struct DeviceListAdvError {
    user: Jid,
    hosted: bool,
    response: DeviceListResponse,
    topology_generation: u64,
}

impl std::fmt::Debug for DeviceListAdvError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Report the failed identity, without dumping the rest of the batch.
        std::fmt::Display::fmt(self, f)
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum DeviceListApplyMode {
    CachePreferred,
    StrictRefresh,
    GroupRefresh,
}

#[derive(Clone, Copy)]
enum PreparedAccountIdentity {
    Trusted([u8; 32]),
    /// Keep a group contact rejected even if its conflict disappears later.
    Rejected,
}

type PreparedDeviceListIdentities = Vec<(Jid, PreparedAccountIdentity)>;

#[derive(Clone)]
struct VerifiedDeviceListEnvelope {
    decoded: wacore::adv::DecodedKeyIndex,
    key: [u8; 32],
}

/// Proofs stay positionally bound to these owned, immutable accepted rows.
struct StagedDeviceLists {
    response: DeviceListResponse,
    proofs: Vec<Option<VerifiedDeviceListEnvelope>>,
}

#[derive(Default)]
struct StagedAccounts {
    index: std::collections::HashMap<Jid, usize>,
    rows: Vec<StagedAccount>,
    rejected: usize,
}

struct StagedAccount {
    user: Jid,
    identity: Option<[u8; 32]>,
    prepared: Option<[u8; 32]>,
    desired: Option<[u8; 32]>,
    rejected: bool,
    empty: bool,
    requires_maintenance: bool,
}

impl StagedAccounts {
    #[inline(never)]
    fn track(&mut self, user: &Jid) -> usize {
        if let Some(index) = self.index.get(user) {
            return *index;
        }
        let index = self.rows.len();
        self.index.insert(user.clone(), index);
        self.rows.push(StagedAccount {
            user: user.clone(),
            identity: None,
            prepared: None,
            desired: None,
            rejected: false,
            empty: false,
            requires_maintenance: false,
        });
        index
    }

    #[inline(never)]
    fn reject(&mut self, user: &Jid) {
        let index = self.track(user);
        let row = &mut self.rows[index];
        if !row.rejected {
            row.rejected = true;
            self.rejected += 1;
        }
    }

    #[inline(never)]
    fn row(&self, user: &Jid) -> Option<&StagedAccount> {
        self.index.get(user).map(|index| &self.rows[*index])
    }
}

// Share collection probes without changing the full JID hash/equality or
// the caller's alias priority, identity selection and rejection rules.
#[inline(never)]
fn staged_mapping<'a>(
    mappings: &'a std::collections::HashMap<Jid, Jid>,
    user: &Jid,
) -> Option<&'a Jid> {
    mappings.get(user)
}

#[inline(never)]
fn staged_identity(keys: &StagedAccounts, user: &Jid) -> Option<[u8; 32]> {
    keys.row(user).and_then(|row| row.identity)
}

#[inline(never)]
fn staged_rejected(rejected: &StagedAccounts, user: &Jid) -> bool {
    rejected.row(user).is_some_and(|row| row.rejected)
}

fn project_adv_devices(
    list: &wacore::usync::UserDeviceList,
    decoded: Option<&wacore::adv::DecodedKeyIndex>,
    previous: Option<&wacore::store::traits::DeviceListRecord>,
) -> Vec<wacore::store::traits::DeviceInfo> {
    list.devices
        .iter()
        .filter_map(|device| {
            let index = device.key_index.or_else(|| {
                previous.and_then(|record| {
                    record
                        .devices
                        .iter()
                        .find(|old| old.device_id() == device.device)
                        .and_then(|old| old.key_index())
                })
            });
            let valid = device.device == 0
                || decoded.is_none_or(|decoded| {
                    index.is_some_and(|index| {
                        decoded.valid_indexes.contains(&index)
                            || index > decoded.current_index
                                && previous.is_some_and(|record| {
                                    record.devices.iter().any(|old| {
                                        old.device_id() == device.device
                                            && old.key_index() == Some(index)
                                    })
                                })
                    })
                });
            valid.then(|| {
                wacore::store::traits::DeviceInfo::new(device.device, index)
                    .with_hosting(device.is_hosted)
            })
        })
        .collect()
}

fn device_requires_cleanup(
    previous: &wacore::store::traits::DeviceInfo,
    current: &[wacore::store::traits::DeviceInfo],
    decoded: Option<&wacore::adv::DecodedKeyIndex>,
    retained: &std::collections::HashSet<u16>,
) -> bool {
    !retained.contains(&previous.device_id())
        || current.iter().any(|device| {
            device.device_id() == previous.device_id() && device.is_hosted() != previous.is_hosted()
        })
        || (previous.device_id() != 0
            && decoded.is_some_and(|decoded| {
                !wacore::adv::is_key_index_valid(previous.key_index(), decoded)
            }))
}

fn projection_requires_maintenance(
    previous: Option<&wacore::store::traits::DeviceListRecord>,
    current: &[wacore::store::traits::DeviceInfo],
    decoded: Option<&wacore::adv::DecodedKeyIndex>,
    mode: DeviceListApplyMode,
    retained: &mut std::collections::HashSet<u16>,
) -> bool {
    let Some(previous) = previous else {
        return false;
    };
    let raw_changed = previous
        .raw_id
        .zip(decoded.map(|decoded| decoded.raw_id))
        .is_some_and(|(old, new)| old != new);
    if current.is_empty() {
        return raw_changed && mode == DeviceListApplyMode::CachePreferred;
    }
    if raw_changed {
        return true;
    }
    retained.clear();
    retained.extend(current.iter().map(|device| device.device_id()));
    previous
        .devices
        .iter()
        .any(|device| device_requires_cleanup(device, current, decoded, retained))
}

/// Group partial success applies only to trust conflicts, never storage faults.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DeviceListIdentityConflict {
    #[error("conflicting account alias during ADV verification")]
    Alias,
    #[error("conflicting primary identities during ADV verification")]
    Primary,
    #[error("primary identity changed during ADV bootstrap")]
    Bootstrap,
}

#[derive(Default)]
struct DeviceListIdentityContext<'a> {
    prepared: &'a [(Jid, PreparedAccountIdentity)],
    recovered: Option<&'a RecoveredAccountIdentity>,
}

struct RecoveredAccountIdentity {
    user: Jid,
    identity: [u8; 32],
    snapshot: AdvRecoverySnapshot,
}

struct AdvRecoverySnapshot {
    observed_alias: Option<Jid>,
    publication_alias: Option<Jid>,
    identities: Vec<(Jid, Option<[u8; 32]>)>,
}

impl AdvRecoverySnapshot {
    fn matches_observed_alias(&self, root: &Jid, user: &Jid, alias: Option<&Jid>) -> bool {
        if user == root {
            alias == self.observed_alias.as_ref()
        } else if self.publication_alias.as_ref() == Some(user) {
            alias == self.observed_alias.as_ref().map(|_| root)
        } else {
            true
        }
    }
}

/// A replay may publish only the relation whose endpoints were captured before
/// primary authentication, never another edge incident to either endpoint.
fn validate_recovery_mappings(
    response: &DeviceListResponse,
    user: &Jid,
    alias: Option<&Jid>,
) -> anyhow::Result<()> {
    for mapping in &response.lid_mappings {
        let pn = Jid::pn(mapping.phone_number.as_str());
        let lid = Jid::lid(mapping.lid.as_str());
        if pn == *user || lid == *user || alias.is_some_and(|alias| pn == *alias || lid == *alias) {
            anyhow::ensure!(
                alias.is_some_and(
                    |alias| (pn == *user && lid == *alias) || (lid == *user && pn == *alias)
                ),
                "account alias changed during ADV recovery"
            );
        }
    }
    Ok(())
}

/// Retry outside topology guards when a response needs Signal mutation.
#[derive(Debug, thiserror::Error)]
#[error("device-list response requires Signal maintenance")]
struct DeviceListMaintenanceRequired;

/// A verified account trust change staged until the complete batch is accepted.
struct AdvIdentityUpdate<'a> {
    user: &'a Jid,
    identity: [u8; 32],
    previous_identity: Option<[u8; 32]>,
    identities: Vec<(
        wacore::libsignal::protocol::ProtocolAddress,
        Option<[u8; 32]>,
    )>,
    previous_record: Option<wacore::store::traits::DeviceListRecord>,
    reset_primary_session: bool,
}

impl From<crate::cache::Freshness> for DeviceListApplyMode {
    fn from(freshness: crate::cache::Freshness) -> Self {
        match freshness {
            crate::cache::Freshness::CachePreferred => Self::CachePreferred,
            crate::cache::Freshness::Refresh => Self::StrictRefresh,
        }
    }
}

#[inline]
/// Every user a device-list response speaks for, in both namespaces, as the
/// member set a topology change is checked against.
fn device_response_users(
    response: &DeviceListResponse,
) -> crate::client::member_index::MemberIndex {
    crate::client::member_index::MemberIndex::from_users(
        response
            .device_lists
            .iter()
            .map(|device_list| device_list.user.user.as_str())
            .chain(
                response
                    .lid_mappings
                    .iter()
                    .flat_map(|mapping| [mapping.phone_number.as_str(), mapping.lid.as_str()]),
            ),
    )
}

pub use wacore::iq::usync::{
    UsyncAddressingMode, UsyncBotCommand, UsyncBotProfessionalType, UsyncBotProfileResult,
    UsyncBotPrompt, UsyncBusinessResult, UsyncContactResult, UsyncContext, UsyncDeviceListResult,
    UsyncDeviceResult, UsyncDeviceSyncHint, UsyncDevicesResult, UsyncDisappearingModeResult,
    UsyncFeature, UsyncFeatureResult, UsyncKeyIndexResult, UsyncMode, UsyncOutcome, UsyncProtocol,
    UsyncProtocolKind, UsyncProtocolResult, UsyncProtocolState, UsyncQuery, UsyncResponse,
    UsyncStatusResult, UsyncSubprotocolError, UsyncTextStatusResult, UsyncUser, UsyncUserResult,
    UsyncValidationError,
};

impl Client {
    /// Executes a typed USync query.
    ///
    /// The client generates the protocol `sid` independently from the IQ ID,
    /// matching WhatsApp Web. This neutral operation only returns decoded wire
    /// data; cache and persistence effects remain in specialized client APIs.
    pub async fn query_usync(&self, query: UsyncQuery) -> Result<UsyncResponse, IqError> {
        let sid = self.generate_request_id();
        let spec = wacore::iq::usync::UsyncQuerySpec::new(query, sid)
            .map_err(|error| IqError::EncodeError(error.into()))?;
        self.execute(spec).await
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.usync.get_user_devices", level = "debug", skip_all, fields(users = jids.len()), err(Debug)))]
    pub(crate) async fn get_user_devices(&self, jids: &[Jid]) -> Result<Vec<Jid>, anyhow::Error> {
        let mut owned = Vec::with_capacity(jids.len());
        owned.extend(jids.iter().map(Jid::to_non_ad));
        self.get_user_devices_owned(owned).await
    }

    pub(crate) async fn get_user_devices_owned(
        &self,
        jids: Vec<Jid>,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        // One batched registry pass: the cache answers per user, and every
        // user it cannot answer goes to the backend in ONE read. The
        // previous per-user fan-out (16 concurrent `get_devices`) bought
        // nothing, because the backend read is on the single write permit
        // and the permit re-serialized them. Order is irrelevant (phash
        // sorts, encrypt fan-out is order-agnostic). An unresolved user is
        // one with no usable record anywhere (WA Web always keeps device 0,
        // so an empty record counts as none) and falls through to the
        // network below.
        let jids: Vec<Jid> = jids.into_iter().map(Jid::into_non_ad).collect();
        let (mut all_devices, mut jids_to_fetch) = self.get_devices_from_registry_batch(jids).await;

        if !jids_to_fetch.is_empty() {
            wacore::types::jid::sort_dedup_by_user(&mut jids_to_fetch);
            debug!(
                "get_user_devices: Cache miss, fetching from network for {} unique users",
                jids_to_fetch.len()
            );
            let fetched = self.fetch_user_devices(jids_to_fetch).await?;
            self.remove_superseded_cached_devices(&mut all_devices, &fetched)
                .await;
            all_devices.extend(fetched);
        }

        Ok(all_devices)
    }

    pub(crate) async fn refresh_user_devices(
        &self,
        mut jids: Vec<Jid>,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        for jid in &mut jids {
            jid.agent = 0;
            jid.device = 0;
        }
        wacore::types::jid::sort_dedup_by_user(&mut jids);
        self.fetch_user_devices_with_freshness(jids, crate::cache::Freshness::Refresh)
            .await
    }

    /// Publish successful group sync results, then read the complete local fanout.
    /// An omitted or invalid user keeps its previous registry record.
    pub(crate) async fn refresh_group_user_devices(
        &self,
        mut jids: Vec<Jid>,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        for jid in &mut jids {
            jid.agent = 0;
            jid.device = 0;
        }
        wacore::types::jid::sort_dedup_by_user(&mut jids);
        if jids.is_empty() {
            return Ok(Vec::new());
        }
        let mut recovery_budget = GroupAdvRecoveryBudget {
            remaining: GROUP_ADV_RECOVERY_LIMIT,
            attempted: std::collections::HashSet::new(),
        };
        for _ in 0..DEVICE_REFRESH_MAX_ATTEMPTS {
            let generation = self.device_topology.current();
            let response = self
                .execute_streaming(DeviceListSpec::new(
                    jids.clone(),
                    self.generate_request_id(),
                ))
                .await?;
            self.recover_group_adv_identities(&response, &mut recovery_budget)
                .await;
            if let Some(devices) = self
                .try_process_group_device_list_response(&response, &jids, generation)
                .await?
            {
                return Ok(devices);
            }
        }
        anyhow::bail!("device registry kept changing while a group refresh was in flight")
    }

    /// Recover rotated contacts individually before the group topology CAS.
    /// A successful recovery invalidates the old batch generation; failures
    /// leave that contact's old record available while valid peers still apply.
    async fn recover_group_adv_identities(
        &self,
        response: &DeviceListResponse,
        budget: &mut GroupAdvRecoveryBudget,
    ) {
        for list in &response.device_lists {
            if list.devices.iter().any(|device| device.is_hosted) {
                continue;
            }
            let Some(bytes) = list.key_index_bytes.as_deref() else {
                continue;
            };
            let identity = {
                let Ok((_sessions, _mapping)) = self
                    .device_list_identity_guards(std::slice::from_ref(&list.user))
                    .await
                else {
                    continue;
                };
                let Ok(Some(identity)) = self.device_list_account_identity(&list.user).await else {
                    continue;
                };
                identity
            };
            if wacore::adv::verify_key_index_list(bytes, &identity).is_some()
                || wacore::adv::decode_key_index_list(bytes).is_none()
            {
                continue;
            }
            let snapshot = self.persistence_manager.get_device_snapshot();
            if snapshot
                .pn
                .iter()
                .chain(snapshot.lid.iter())
                .any(|own| own.is_same_chat_as(&list.user))
                || !budget.take(&list.user)
            {
                continue;
            }
            if let Err(error) = Box::pin(self.fetch_user_devices_with_freshness(
                vec![list.user.clone()],
                crate::cache::Freshness::Refresh,
            ))
            .await
            {
                let category = if error.downcast_ref::<DeviceListAdvError>().is_some() {
                    "signature verification"
                } else {
                    "request or persistence"
                };
                warn!(
                    "Group ADV recovery failed for {} ({category}); retaining previous devices",
                    list.user.observe()
                );
            }
        }
    }

    /// Keep partial group results and omitted members under one topology CAS.
    async fn try_process_group_device_list_response(
        &self,
        response: &DeviceListResponse,
        users: &[Jid],
        topology_generation: u64,
    ) -> Result<Option<Vec<Jid>>, anyhow::Error> {
        self.apply_device_list_response(
            response,
            DeviceListApplyMode::GroupRefresh,
            Some(topology_generation),
            Some(users),
        )
        .await
    }

    async fn fetch_user_devices(&self, jids: Vec<Jid>) -> Result<Vec<Jid>, anyhow::Error> {
        self.fetch_user_devices_with_freshness(jids, crate::cache::Freshness::CachePreferred)
            .await
    }

    /// Retry one failed ADV identity authoritatively, then rebuild its batch.
    #[inline(never)]
    fn fetch_user_devices_with_freshness(
        &self,
        jids: Vec<Jid>,
        freshness: crate::cache::Freshness,
    ) -> wacore::runtime::BoxFuture<'_, Result<Vec<Jid>, anyhow::Error>> {
        // Cache misses and explicit refreshes reuse one retry/recovery body.
        Box::pin(async move {
            match self.fetch_user_devices_once(jids, freshness).await {
                Ok(devices) => Ok(devices),
                Err(error) => {
                    let failure = error.downcast::<DeviceListAdvError>()?;
                    let user = failure.user.into_non_ad();
                    // Strict refresh bypasses the cached list. Keep the previous
                    // record until verification so normal application can compare
                    // devices/raw_id and clean up removed devices and identity resets.
                    let refreshed = self
                        .fetch_user_devices_once(
                            vec![user.clone()],
                            crate::cache::Freshness::Refresh,
                        )
                        .await;
                    let refreshed =
                        match refreshed {
                            Ok(devices) => devices,
                            Err(error) => self.recover_ordinary_adv_identity(error).await.map_err(
                                |error| {
                                    let context =
                                        format!("ADV device-list refresh for {user}: {error:#}");
                                    error.context(context)
                                },
                            )?,
                        };
                    if failure.response.device_lists.len() == 1 {
                        return Ok(refreshed);
                    }
                    let response_users = failure
                        .response
                        .device_lists
                        .into_iter()
                        .map(|list| list.user)
                        .collect();
                    let mut devices = self
                        .fetch_user_devices_once(response_users, freshness)
                        .await?;
                    if !self.device_list_contains_user(&devices, &user).await {
                        // Partial replies may omit an unchanged user. Read its
                        // current record, since notifications may have superseded
                        // the targeted refresh while this batch was in flight.
                        let (current, missing) = self
                            .get_devices_from_registry_batch(vec![user.clone()])
                            .await;
                        anyhow::ensure!(
                            missing.is_empty(),
                            "device registry no longer contains {user} after ADV recovery"
                        );
                        devices.extend(current);
                    }
                    Ok(devices)
                }
            }
        })
    }

    /// A second ordinary failure can mean a missed primary identity rotation.
    /// Authenticate both the fresh primary prekey and the complete refreshed list
    /// before publishing trust. Never use the list's embedded key as an anchor.
    #[cold]
    #[inline(never)]
    fn recover_ordinary_adv_identity(
        &self,
        error: anyhow::Error,
    ) -> wacore::runtime::BoxFuture<'_, anyhow::Result<Vec<Jid>>> {
        // Group and individual refreshes share the full authenticated recovery
        // state machine, including its network request and owned publication.
        Box::pin(async move {
            let Some(failure) = error.downcast_ref::<DeviceListAdvError>() else {
                return Err(error);
            };
            if failure.hosted {
                return Err(error);
            }
            self.ensure_signal_maintenance_context()?;
            let user = &failure.user;
            let snapshot = self.persistence_manager.get_device_snapshot();
            if snapshot
                .pn
                .iter()
                .chain(snapshot.lid.iter())
                .any(|own| own.is_same_chat_as(user))
            {
                return Err(error);
            }
            let Some(bytes) = failure
                .response
                .device_lists
                .iter()
                .find(|list| list.user == *user)
                .and_then(|list| list.key_index_bytes.as_deref())
            else {
                return Err(error);
            };
            // Malformed input is not evidence of identity rotation.
            if wacore::adv::decode_key_index_list(bytes).is_none() {
                return Err(error);
            }
            let snapshot = self
                .capture_adv_recovery_snapshot(user, &failure.response)
                .await?;
            let identity = self.fetch_adv_account_identity(user).await?;
            if wacore::adv::verify_key_index_list(bytes, &identity).is_none() {
                return Err(error);
            }
            let client = self
                .self_weak
                .get()
                .and_then(|weak| weak.upgrade())
                .ok_or_else(|| {
                    anyhow::anyhow!("client ownership unavailable during ADV recovery")
                })?;
            let response = failure.response.clone();
            let generation = failure.topology_generation;
            let recovered = RecoveredAccountIdentity {
                user: user.clone(),
                identity,
                snapshot,
            };
            client
                .apply_owned_device_list_response(
                    response,
                    DeviceListApplyMode::StrictRefresh,
                    Some(generation),
                    None,
                    Vec::new(),
                    Some(recovered),
                )
                .await?
                .ok_or_else(|| {
                    anyhow::anyhow!("device topology changed during ADV identity recovery")
                })
        })
    }

    /// Capture a first response-supplied alias without learning it or trusting
    /// its envelope. All endpoint locks are released before the primary IQ.
    async fn capture_adv_recovery_snapshot(
        &self,
        user: &Jid,
        response: &DeviceListResponse,
    ) -> anyhow::Result<AdvRecoverySnapshot> {
        let mut proposed = None;
        for mapping in &response.lid_mappings {
            let pn = Jid::pn(mapping.phone_number.as_str());
            let lid = Jid::lid(mapping.lid.as_str());
            let candidate = if pn == *user {
                Some(lid)
            } else if lid == *user {
                Some(pn)
            } else {
                None
            };
            if let Some(candidate) = candidate {
                anyhow::ensure!(
                    proposed
                        .as_ref()
                        .is_none_or(|previous| *previous == candidate),
                    "account alias changed during ADV recovery"
                );
                proposed = Some(candidate);
            }
        }
        // Only observe relationships here. Release mapping before acquiring any
        // sessions, and reject an occupied candidate without locking its owner.
        let basis = {
            let _mapping = self.lid_pn_cache.lock_mutation().await;
            let observed_alias = self.device_list_identity_alias(user).await;
            if let Some(alias) = &observed_alias {
                anyhow::ensure!(
                    self.device_list_identity_alias(alias).await.as_ref() == Some(user),
                    "account alias changed during ADV recovery"
                );
                anyhow::ensure!(
                    proposed.as_ref().is_none_or(|proposed| proposed == alias),
                    "account alias changed during ADV recovery"
                );
            }
            let publication_alias = proposed.or_else(|| observed_alias.clone());
            self.validate_adv_recovery_ownership(user, publication_alias.as_ref())?;
            self.validate_adv_recovery_durable_alias(user, publication_alias.as_ref(), &_mapping)
                .await?;
            validate_recovery_mappings(response, user, publication_alias.as_ref())?;
            if observed_alias.is_none()
                && let Some(alias) = &publication_alias
            {
                anyhow::ensure!(
                    self.device_list_identity_alias(alias).await.is_none(),
                    "account alias changed during ADV recovery"
                );
            }
            AdvRecoverySnapshot {
                observed_alias,
                publication_alias,
                identities: Vec::new(),
            }
        };
        let users: Vec<_> = std::iter::once(user.clone())
            .chain(basis.publication_alias.iter().cloned())
            .collect();
        let (_sessions, _mapping) = self
            .device_list_identity_guards_with_recovery(&users, Some((user, &basis)))
            .await?;
        self.validate_adv_recovery_ownership(user, basis.publication_alias.as_ref())?;
        self.validate_adv_recovery_durable_alias(user, basis.publication_alias.as_ref(), &_mapping)
            .await?;
        let primary = self.cached_adv_account_identity(user).await?;
        let mut identities = vec![(user.clone(), primary)];
        if let Some(alias) = &basis.publication_alias {
            let previous = self.cached_adv_account_identity(alias).await?;
            anyhow::ensure!(
                if basis.observed_alias.is_none() {
                    previous.is_none() || previous == primary
                } else {
                    primary.is_none() || previous.is_none() || primary == previous
                },
                "conflicting primary identities during ADV recovery"
            );
            identities.push((alias.clone(), previous));
        }
        Ok(AdvRecoverySnapshot {
            identities,
            ..basis
        })
    }

    /// Cache eviction is not evidence that a proposed alias has no owner.
    /// Learning owns this same lock from recording through the backend ACK,
    /// so an older detached write cannot finish behind these reads.
    async fn validate_adv_recovery_durable_alias(
        &self,
        user: &Jid,
        alias: Option<&Jid>,
        _mapping: &crate::lid_pn_cache::LidPnMutationGuard<'_>,
    ) -> anyhow::Result<()> {
        use wacore_binary::jid::Server;
        let Some(alias) = alias else {
            return Ok(());
        };
        let (phone, lid) = match (user.server, alias.server) {
            (Server::Pn, Server::Lid) => (user, alias),
            (Server::Lid, Server::Pn) => (alias, user),
            _ => anyhow::bail!("invalid recovery alias namespaces"),
        };
        let backend = self.persistence_manager.backend();
        let candidate = backend.get_lid_mapping(&lid.user).await?;
        if let Some(entry) = &candidate {
            anyhow::ensure!(
                entry.phone_number == phone.user.as_str() && entry.lid == lid.user.as_str(),
                "durable account alias changed during ADV recovery"
            );
        }
        if let Some(entry) = backend.get_pn_mapping(&phone.user).await? {
            anyhow::ensure!(
                entry.phone_number == phone.user.as_str(),
                "durable account alias changed during ADV recovery"
            );
            if entry.lid != lid.user.as_str() {
                // Historical LIDs are retained. A seconds-resolution tie in
                // get_pn_mapping cannot overrule the live, durably ACKed pair.
                // This exception proves ownership from the candidate's own
                // durable row; cold/cache-only aliases and newer rows still fail.
                anyhow::ensure!(
                    candidate
                        .as_ref()
                        .is_some_and(|row| row.updated_at == entry.updated_at)
                        && self
                            .lid_pn_cache
                            .get_current_lid(&phone.user)
                            .await
                            .as_deref()
                            == Some(lid.user.as_str())
                        && self
                            .lid_pn_cache
                            .get_phone_number(&lid.user)
                            .await
                            .as_deref()
                            == Some(phone.user.as_str())
                        && self.lid_pn_cache.is_persisted(&phone.user, &lid.user).await,
                    "durable account alias changed during ADV recovery"
                );
            }
        }
        Ok(())
    }

    fn validate_adv_recovery_ownership(
        &self,
        user: &Jid,
        alias: Option<&Jid>,
    ) -> anyhow::Result<()> {
        let own = self.persistence_manager.get_device_snapshot();
        anyhow::ensure!(
            !std::iter::once(user).chain(alias).any(|alias| own
                .pn
                .iter()
                .chain(own.lid.iter())
                .any(|jid| jid.is_same_chat_as(alias))),
            "ADV recovery cannot replace our paired primary through an alias"
        );
        Ok(())
    }

    /// Fetch and verify a device list without starting ADV recovery recursively.
    #[inline(never)]
    fn fetch_user_devices_once(
        &self,
        mut jids: Vec<Jid>,
        freshness: crate::cache::Freshness,
    ) -> wacore::runtime::BoxFuture<'_, Result<Vec<Jid>, anyhow::Error>> {
        // Initial lookup, targeted refresh and batch rebuild all poll the same
        // query/application implementation instead of embedding three copies.
        Box::pin(async move {
            if jids.is_empty() {
                return Ok(Vec::new());
            }
            if freshness == crate::cache::Freshness::CachePreferred {
                let sid = self.generate_request_id();
                let response = self
                    .execute_streaming(DeviceListSpec::new(jids, sid))
                    .await?;
                return self
                    .process_device_list_response(&response, freshness)
                    .await;
            }

            for attempt in 0..DEVICE_REFRESH_MAX_ATTEMPTS {
                let topology_generation = self.device_topology.current();
                let sid = self.generate_request_id();
                let response = self
                    .execute_streaming(DeviceListSpec::new(jids, sid).require_complete_response())
                    .await?;

                if let Some(devices) = self
                    .try_process_refreshed_device_list_response(
                        &response,
                        freshness,
                        topology_generation,
                    )
                    .await?
                {
                    return Ok(devices);
                }

                if attempt + 1 == DEVICE_REFRESH_MAX_ATTEMPTS {
                    anyhow::bail!(
                        "device registry kept changing while an authoritative refresh was in flight"
                    );
                }

                // The complete response contains every requested identity. Move its
                // canonical JIDs into the next attempt only on this rare race path;
                // the ordinary refresh performs no duplicate query-vector clone.
                jids = response
                    .device_lists
                    .into_iter()
                    .map(|user| user.user)
                    .collect();
            }

            unreachable!("bounded device refresh loop always returns")
        })
    }

    async fn learn_device_list_mappings_guarded(
        &self,
        response: &DeviceListResponse,
        guard: &crate::lid_pn_cache::LidPnMutationGuard<'_>,
    ) -> Result<(), anyhow::Error> {
        // Learn LID↔PN mappings via the same batched, guarded learner routing_info
        // uses (one detached transaction, skipping already-durable pairs), so
        // per-mapping DB writes stay off the send's critical path. Client
        // construction always installs `self_weak`; failing the impossible
        // upgrade keeps mapping and registry publication atomic.
        //
        // Ordering: the old per-mapping path AWAITED migrate_signal_sessions_on_lid_discovery.
        // Detaching it can let a standalone usync (sync_own_device_list /
        // flush_pending_device_sync) that is the FIRST learner of a LID for a
        // contact with prior PN Signal state encrypt before the PN-wins migration
        // runs — but the per-address session_lock_for both take is the real
        // barrier (they can't interleave). The group-send path is unchanged:
        // routing_info already learns these same pairs detached upstream.
        if response.lid_mappings.is_empty() {
            return Ok(());
        }
        let client = self
            .self_weak
            .get()
            .and_then(|weak| weak.upgrade())
            .ok_or_else(|| anyhow::anyhow!("client ownership unavailable during device sync"))?;
        let mappings: Vec<(String, String)> = response
            .lid_mappings
            .iter()
            .map(|mapping| (mapping.lid.to_string(), mapping.phone_number.to_string()))
            .collect();
        client
            .learn_lid_pn_mappings_batch_guarded(
                mappings,
                crate::lid_pn_cache::LearningSource::Usync,
                false,
                guard,
            )
            .await?;
        Ok(())
    }

    /// Apply a usync device-list response to the registry: persist LID mappings,
    /// rebuild each returned user's `DeviceListRecord` (preserving key indices and
    /// handling raw_id identity changes), and batch-write them. Returns the
    /// resolved device JIDs for the users present in the response.
    ///
    /// Users the server OMITS — unchanged ones, when we sent a `device_hash` — are
    /// simply absent here, so their cached records are left untouched (the
    /// merge-safe behavior the `device_hash` optimization depends on).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.usync.process_device_list", level = "debug", skip_all, fields(users = response.device_lists.len())))]
    async fn process_device_list_response(
        &self,
        response: &DeviceListResponse,
        freshness: crate::cache::Freshness,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        self.apply_device_list_response(response, freshness.into(), None, None)
            .await
            .and_then(|devices| {
                devices.ok_or_else(|| anyhow::anyhow!("unchecked device response was stale"))
            })
    }

    /// Apply an authoritative result only if its queried topology is still current.
    async fn try_process_refreshed_device_list_response(
        &self,
        response: &DeviceListResponse,
        freshness: crate::cache::Freshness,
        topology_generation: u64,
    ) -> Result<Option<Vec<Jid>>, anyhow::Error> {
        self.apply_device_list_response(response, freshness.into(), Some(topology_generation), None)
            .await
    }

    /// Finish hosted trust changes even if the requesting send is cancelled.
    /// The worker owns its guards, response and identity reactions, while
    /// ordinary responses without mappings retain their in-task path.
    #[inline(never)]
    fn apply_device_list_response<'a>(
        &'a self,
        response: &'a DeviceListResponse,
        mode: DeviceListApplyMode,
        topology_generation: Option<u64>,
        group_users: Option<&'a [Jid]>,
    ) -> wacore::runtime::BoxFuture<'a, anyhow::Result<Option<Vec<Jid>>>> {
        // Share admission and hosted-worker dispatch across strict, cached and
        // group response callers, just as the guarded publication body is shared.
        Box::pin(async move {
            let prepared = self.prepare_device_list_identities(response, mode).await?;
            let client = self
                .self_weak
                .get()
                .and_then(|weak| weak.upgrade())
                .ok_or_else(|| {
                    anyhow::anyhow!("client ownership unavailable during device sync")
                })?;
            if !response
                .device_lists
                .iter()
                .any(|list| list.devices.iter().any(|device| device.is_hosted))
            {
                // Poll-scoped durability-hook restrictions do not follow a
                // spawned task. Carry admission explicitly across that boundary.
                let admission = self.ensure_signal_maintenance_context();
                if !response.lid_mappings.is_empty() {
                    return client
                        .apply_owned_ordinary_device_list_response(
                            response.clone(),
                            mode,
                            topology_generation,
                            group_users.map(<[Jid]>::to_vec),
                            prepared,
                            admission,
                        )
                        .await;
                }
                return client
                    .apply_ordinary_device_list_response(
                        response,
                        mode,
                        topology_generation,
                        group_users,
                        &prepared,
                        admission,
                    )
                    .await;
            }
            self.ensure_signal_maintenance_context()?;
            client
                .apply_owned_device_list_response(
                    response.clone(),
                    mode,
                    topology_generation,
                    group_users.map(<[Jid]>::to_vec),
                    prepared,
                    None,
                )
                .await
        })
    }

    /// Preserve metadata-only admission in hooks that already own the drain
    /// permit. Destructive planning still requires the caller's admission proof.
    #[inline(never)]
    fn apply_ordinary_device_list_response<'a>(
        self: &'a std::sync::Arc<Self>,
        response: &'a DeviceListResponse,
        mode: DeviceListApplyMode,
        topology_generation: Option<u64>,
        group_users: Option<&'a [Jid]>,
        prepared: &'a [(Jid, PreparedAccountIdentity)],
        admission: anyhow::Result<()>,
    ) -> wacore::runtime::BoxFuture<'a, anyhow::Result<Option<Vec<Jid>>>> {
        Box::pin(async move {
            let mapping_has_history = self
                .device_mapping_has_registry_history(&response.lid_mappings)
                .await;
            if !mapping_has_history {
                let result = self
                    .apply_device_list_response_inner(
                        response,
                        mode,
                        topology_generation,
                        group_users,
                        DeviceListIdentityContext {
                            prepared,
                            recovered: None,
                        },
                        None,
                    )
                    .await;
                if !matches!(&result, Err(error) if error.is::<DeviceListMaintenanceRequired>()) {
                    return result;
                }
            }
            admission?;
            let maintenance = self.begin_signal_maintenance().await?;
            self.apply_device_list_response_inner(
                response,
                mode,
                topology_generation,
                group_users,
                DeviceListIdentityContext {
                    prepared,
                    recovered: None,
                },
                Some(&maintenance),
            )
            .await
        })
    }

    /// Mapped ordinary responses own their in-flight store write even when the
    /// initiating hook/send is cancelled. They do not inherit hosted admission.
    #[inline(never)]
    fn apply_owned_ordinary_device_list_response(
        self: std::sync::Arc<Self>,
        response: DeviceListResponse,
        mode: DeviceListApplyMode,
        topology_generation: Option<u64>,
        group_users: Option<Vec<Jid>>,
        prepared: PreparedDeviceListIdentities,
        admission: anyhow::Result<()>,
    ) -> wacore::runtime::BoxFuture<'static, anyhow::Result<Option<Vec<Jid>>>> {
        Box::pin(async move {
            let (send, receive) = futures::channel::oneshot::channel();
            let runtime = self.runtime.clone();
            runtime.spawn_detached(Box::pin(async move {
                let result = self
                    .apply_ordinary_device_list_response(
                        &response,
                        mode,
                        topology_generation,
                        group_users.as_deref(),
                        &prepared,
                        admission,
                    )
                    .await;
                if let Err(Err(error)) = send.send(result) {
                    warn!("Owned ordinary device sync failed after caller cancellation: {error:#}");
                }
            }));
            receive
                .await
                .map_err(|_| anyhow::anyhow!("owned ordinary device sync worker stopped"))?
        })
    }

    /// Share cancellation-safe publication for hosted lists and ordinary
    /// recovery. The worker owns admission, account fences and all input data.
    #[inline(never)]
    fn apply_owned_device_list_response(
        self: std::sync::Arc<Self>,
        response: DeviceListResponse,
        mode: DeviceListApplyMode,
        topology_generation: Option<u64>,
        group_users: Option<Vec<Jid>>,
        prepared: PreparedDeviceListIdentities,
        recovered: Option<RecoveredAccountIdentity>,
    ) -> wacore::runtime::BoxFuture<'static, anyhow::Result<Option<Vec<Jid>>>> {
        Box::pin(async move {
            self.ensure_signal_maintenance_context()?;
            let (send, receive) = futures::channel::oneshot::channel();
            let runtime = self.runtime.clone();
            runtime.spawn_detached(Box::pin(async move {
                let result = async {
                    let maintenance = self.begin_signal_maintenance().await?;
                    self.apply_device_list_response_inner(
                        &response,
                        mode,
                        topology_generation,
                        group_users.as_deref(),
                        DeviceListIdentityContext {
                            prepared: &prepared,
                            recovered: recovered.as_ref(),
                        },
                        Some(&maintenance),
                    )
                    .await
                }
                .await;
                if let Err(Err(error)) = send.send(result) {
                    warn!("Owned ADV device sync failed after caller cancellation: {error:#}");
                }
            }));
            receive
                .await
                .map_err(|_| anyhow::anyhow!("owned ADV device sync worker stopped"))?
        })
    }

    /// Hold mapping and topology guards through CAS, cleanup and publication.
    /// Share one boxed state machine between ordinary and owned hosted callers.
    #[inline(never)]
    fn apply_device_list_response_inner<'a>(
        &'a self,
        response: &'a DeviceListResponse,
        mode: DeviceListApplyMode,
        topology_generation: Option<u64>,
        group_users: Option<&'a [Jid]>,
        identities: DeviceListIdentityContext<'a>,
        maintenance: Option<&'a SignalMaintenance<'a>>,
    ) -> wacore::runtime::BoxFuture<'a, anyhow::Result<Option<Vec<Jid>>>> {
        Box::pin(async move {
            let recovered_identity = identities.recovered;
            if let Some(maintenance) = maintenance {
                maintenance.ensure_current()?;
            }
            // Serialize both mutation classes across the CAS and publication. The
            // response's own mappings are deliberately learned only after the CAS.
            let mut users: Vec<_> = response
                .device_lists
                .iter()
                .map(|list| list.user.with_device(0))
                .collect();
            for mapping in &response.lid_mappings {
                // Recovery must reject a response-restored alias outside its
                // captured identity set before waiting on that alias's mutex.
                if recovered_identity.is_some_and(|recovered| {
                    recovered.user == Jid::pn(mapping.phone_number.as_str())
                        || recovered.user == Jid::lid(mapping.lid.as_str())
                }) {
                    continue;
                }
                users.push(Jid::pn(mapping.phone_number.as_str()));
                users.push(Jid::lid(mapping.lid.as_str()));
            }
            if let Some(recovered) = recovered_identity {
                users.extend(
                    recovered
                        .snapshot
                        .identities
                        .iter()
                        .map(|(jid, _)| jid.with_device(0)),
                );
            }
            let (_session_guards, mapping_guard) = self
                .device_list_identity_guards_with_recovery(
                    &users,
                    recovered_identity.map(|recovered| (&recovered.user, &recovered.snapshot)),
                )
                .await?;
            let registry_guard = self.device_topology.lock_registry().await;
            if let Some(generation) = topology_generation
                && !self
                    .device_topology
                    .unchanged_for(generation, &device_response_users(response))
            {
                return Ok(None);
            }
            let staged = if response.lid_mappings.is_empty()
                && !response
                    .device_lists
                    .iter()
                    .any(|list| list.devices.iter().any(|device| device.is_hosted))
            {
                None
            } else {
                Some(
                    self.stage_device_list_identities(
                        response,
                        mode,
                        &identities,
                        &mapping_guard,
                        maintenance.is_none(),
                    )
                    .await?,
                )
            };
            let response = staged.as_ref().map_or(response, |staged| &staged.response);
            self.learn_device_list_mappings_guarded(response, &mapping_guard)
                .await?;
            let mut rejected = Vec::new();
            for (user, identity) in identities.prepared {
                if let PreparedAccountIdentity::Trusted(identity) = identity {
                    match self.device_list_account_identity(user).await {
                        Ok(current) if current == Some(*identity) => continue,
                        Err(error)
                            if mode != DeviceListApplyMode::GroupRefresh
                                || !error.is::<DeviceListIdentityConflict>() =>
                        {
                            return Err(error);
                        }
                        _ => {}
                    }
                }
                anyhow::ensure!(
                    mode == DeviceListApplyMode::GroupRefresh,
                    "primary identity changed during ADV preparation"
                );
                rejected.push(user);
            }
            // Rejected group contacts retain their old fanout. Never let a
            // later read turn a rejected preparation into newly accepted trust.
            let accepted = if rejected.is_empty() {
                std::borrow::Cow::Borrowed(response)
            } else {
                let mut accepted = response.clone();
                accepted
                    .device_lists
                    .retain(|list| !rejected.contains(&&list.user));
                std::borrow::Cow::Owned(accepted)
            };
            let proofs = staged.as_ref().map(|staged| {
                staged
                    .response
                    .device_lists
                    .iter()
                    .zip(&staged.proofs)
                    .filter(|(list, _)| !rejected.contains(&&list.user))
                    .map(|(_, proof)| proof.clone())
                    .collect::<Vec<_>>()
            });
            let devices = self
                .process_device_list_response_guarded(
                    &accepted,
                    mode,
                    &registry_guard,
                    recovered_identity,
                    maintenance,
                    proofs.as_deref(),
                )
                .await?;
            if let Some(users) = group_users {
                // Include omitted group users under the same publication guards.
                let (devices, _) = self.get_devices_from_registry_batch(users.to_vec()).await;
                Ok(Some(devices))
            } else {
                Ok(Some(devices))
            }
        })
    }

    /// Validate the prospective account graph before publishing mappings or
    /// scheduling migration. Rejection is monotonic across a batch: removing a
    /// bad list must also remove the binding that could redirect its old fanout.
    async fn stage_device_list_identities(
        &self,
        response: &DeviceListResponse,
        mode: DeviceListApplyMode,
        identities: &DeviceListIdentityContext<'_>,
        mapping_guard: &crate::lid_pn_cache::LidPnMutationGuard<'_>,
        metadata_only: bool,
    ) -> anyhow::Result<StagedDeviceLists> {
        use std::collections::HashMap;
        if let Some(recovered) = identities.recovered {
            self.validate_adv_recovery_ownership(
                &recovered.user,
                recovered.snapshot.publication_alias.as_ref(),
            )?;
            self.validate_adv_recovery_durable_alias(
                &recovered.user,
                recovered.snapshot.publication_alias.as_ref(),
                mapping_guard,
            )
            .await?;
            for (jid, previous) in &recovered.snapshot.identities {
                anyhow::ensure!(
                    self.cached_adv_account_identity(jid).await? == *previous,
                    "primary identity changed during ADV recovery"
                );
            }
            let expected = recovered.snapshot.observed_alias.as_ref();
            anyhow::ensure!(
                self.device_list_identity_alias(&recovered.user)
                    .await
                    .as_ref()
                    == expected,
                "account alias changed during ADV recovery"
            );
            if expected.is_none()
                && let Some(alias) = &recovered.snapshot.publication_alias
            {
                anyhow::ensure!(
                    self.device_list_identity_alias(alias).await.is_none(),
                    "account alias changed during ADV recovery"
                );
            }
            validate_recovery_mappings(
                response,
                &recovered.user,
                recovered.snapshot.publication_alias.as_ref(),
            )?;
        }
        let mut accounts = StagedAccounts::default();
        for list in &response.device_lists {
            accounts.track(&list.user.with_device(0));
        }
        let mut proposed = HashMap::new();
        let mut edges = Vec::new();
        for mapping in &response.lid_mappings {
            let pn = Jid::pn(mapping.phone_number.as_str());
            let lid = Jid::lid(mapping.lid.as_str());
            accounts.track(&pn);
            accounts.track(&lid);
            edges.push((pn.clone(), lid.clone()));
            for (user, alias) in [(pn.clone(), lid.clone()), (lid, pn)] {
                if let Some(previous) = proposed.insert(user.clone(), alias.clone())
                    && previous != alias
                {
                    for rejected in [user, alias, previous] {
                        accounts.reject(&rejected);
                    }
                }
            }
        }
        let mut current = HashMap::new();
        let initial_users: Vec<_> = accounts.rows.iter().map(|row| row.user.clone()).collect();
        for user in initial_users {
            if let Some(alias) = self.device_list_identity_alias(&user).await {
                if self.device_list_identity_alias(&alias).await.as_ref() == Some(&user) {
                    edges.push((user.clone(), alias.clone()));
                    current.insert(alias.clone(), user.clone());
                }
                accounts.track(&alias);
                current.insert(user, alias);
            }
        }
        for row in &mut accounts.rows {
            row.identity = self.cached_adv_account_identity(&row.user).await?;
        }
        let alias_for =
            |user: &Jid| staged_mapping(&proposed, user).or_else(|| staged_mapping(&current, user));
        for mapping in &response.lid_mappings {
            let pn = Jid::pn(mapping.phone_number.as_str());
            let lid = Jid::lid(mapping.lid.as_str());
            if staged_mapping(&current, &pn) == Some(&lid)
                && staged_mapping(&current, &lid) == Some(&pn)
            {
                // A hosted signature may repair/rotate this already known pair.
                continue;
            }
            let mut anchor = None;
            for address in [&pn, &lid]
                .into_iter()
                .chain(staged_mapping(&current, &pn))
                .chain(staged_mapping(&current, &lid))
            {
                if let Some(key) = staged_identity(&accounts, address) {
                    if anchor.is_some_and(|anchor| anchor != key) {
                        accounts.reject(&pn);
                        accounts.reject(&lid);
                    }
                    anchor = Some(key);
                }
            }
        }
        for (user, prepared) in identities.prepared {
            let user = user.with_device(0);
            let PreparedAccountIdentity::Trusted(expected) = prepared else {
                accounts.reject(&user);
                continue;
            };
            let primary = staged_identity(&accounts, &user);
            let mut live = primary;
            if let Some(alias) = alias_for(&user) {
                if alias_for(alias) != Some(&user) {
                    accounts.reject(&user);
                }
                let other = staged_identity(&accounts, alias);
                if primary.is_some() && other.is_some() && primary != other {
                    accounts.reject(&user);
                }
                live = primary.or(other);
            }
            if live != Some(*expected) {
                accounts.reject(&user);
            }
        }
        for (jid, key) in identities.prepared {
            if let PreparedAccountIdentity::Trusted(key) = key {
                let index = accounts.track(jid);
                accounts.rows[index].prepared.get_or_insert(*key);
            }
        }
        let projection_users: Vec<_> = response
            .device_lists
            .iter()
            .map(|list| &list.user)
            .collect();
        let mut projection_records = self
            .load_device_records_batch_projected_checked(&projection_users, &proposed)
            .await?
            .into_iter();
        let mut proofs = vec![None; response.device_lists.len()];
        let mut retained = std::collections::HashSet::new();
        for (position, list) in response.device_lists.iter().enumerate() {
            let previous_record = projection_records.next().flatten();
            let Some(bytes) = list.key_index_bytes.as_deref() else {
                if metadata_only && previous_record.is_some() {
                    let devices = project_adv_devices(list, None, previous_record.as_ref());
                    let index = accounts.track(&list.user.with_device(0));
                    accounts.rows[index].requires_maintenance |= projection_requires_maintenance(
                        previous_record.as_ref(),
                        &devices,
                        None,
                        mode,
                        &mut retained,
                    );
                }
                if list.devices.iter().any(|device| device.device != 0) {
                    anyhow::ensure!(
                        mode == DeviceListApplyMode::GroupRefresh,
                        "companion device list omits ADV signature"
                    );
                    accounts.reject(&list.user.with_device(0));
                }
                if list.devices.is_empty() {
                    anyhow::ensure!(
                        mode != DeviceListApplyMode::StrictRefresh,
                        "device-list refresh left no valid devices for {}",
                        list.user
                    );
                    if mode == DeviceListApplyMode::CachePreferred {
                        let index = accounts.track(&list.user.with_device(0));
                        accounts.rows[index].empty = true;
                    } else {
                        accounts.reject(&list.user.with_device(0));
                    }
                }
                continue;
            };
            let user = list.user.with_device(0);
            if let Some(alias) = alias_for(&user)
                && alias_for(alias) != Some(&user)
            {
                accounts.reject(&user);
            }
            let hosted = list.devices.iter().any(|device| device.is_hosted);
            let verified = if hosted {
                wacore::adv::verify_hosted_key_index_list(bytes)
            } else {
                let primary = staged_identity(&accounts, &user);
                let other = alias_for(&user).and_then(|alias| staged_identity(&accounts, alias));
                if primary.is_some() && other.is_some() && primary != other {
                    accounts.reject(&user);
                }
                let key = identities
                    .recovered
                    .filter(|recovered| recovered.user == list.user)
                    .map(|recovered| recovered.identity)
                    .or_else(|| accounts.row(&list.user).and_then(|row| row.prepared))
                    .or(primary.or(other));
                key.and_then(|key| {
                    wacore::adv::verify_key_index_list(bytes, &key).map(|decoded| (decoded, key))
                })
            };
            let Some((decoded, key)) = verified else {
                if mode == DeviceListApplyMode::GroupRefresh {
                    accounts.reject(&user);
                    continue;
                }
                return Err(DeviceListAdvError {
                    user: list.user.clone(),
                    hosted: list.devices.iter().any(|device| device.is_hosted),
                    response: response.clone(),
                    topology_generation: self.device_topology.current(),
                }
                .into());
            };
            let replacement = hosted
                || identities
                    .recovered
                    .is_some_and(|recovered| recovered.user == list.user);
            let changes_key = replacement
                && std::iter::once(&user)
                    .chain(alias_for(&user))
                    .any(|jid| staged_identity(&accounts, jid).is_some_and(|old| old != key));
            let discard_previous = changes_key
                || previous_record
                    .as_ref()
                    .and_then(|record| record.raw_id)
                    .is_some_and(|raw| raw != decoded.raw_id);
            let devices = project_adv_devices(
                list,
                Some(&decoded),
                if discard_previous {
                    None
                } else {
                    previous_record.as_ref()
                },
            );
            if metadata_only {
                let index = accounts.track(&user);
                accounts.rows[index].requires_maintenance |= projection_requires_maintenance(
                    previous_record.as_ref(),
                    &devices,
                    Some(&decoded),
                    mode,
                    &mut retained,
                );
            }
            if devices.is_empty() {
                anyhow::ensure!(
                    mode != DeviceListApplyMode::StrictRefresh,
                    "device-list refresh left no valid devices for {}",
                    list.user
                );
                if mode == DeviceListApplyMode::CachePreferred {
                    let index = accounts.track(&user);
                    accounts.rows[index].empty = true;
                    // Keep the authenticated row for raw-id cleanup. It cannot
                    // authorize a new mapping or become a published fanout.
                    proofs[position] = Some(VerifiedDeviceListEnvelope { decoded, key });
                } else {
                    accounts.reject(&user);
                }
                continue;
            }
            proofs[position] = Some(VerifiedDeviceListEnvelope { decoded, key });
            for address in std::iter::once(&user).chain(alias_for(&user)) {
                let index = accounts.track(address);
                if let Some(previous) = accounts.rows[index].desired.replace(key)
                    && previous != key
                {
                    accounts.reject(&user);
                    accounts.reject(address);
                }
            }
        }
        loop {
            let before = accounts.rejected;
            for (user, alias) in &edges {
                if staged_rejected(&accounts, user) || staged_rejected(&accounts, alias) {
                    accounts.reject(user);
                    accounts.reject(alias);
                }
            }
            if accounts.rejected == before {
                break;
            }
        }
        anyhow::ensure!(
            accounts.rejected == 0 || mode == DeviceListApplyMode::GroupRefresh,
            "conflicting account identities in ADV response"
        );
        let mut accepted = response.clone();
        accepted
            .device_lists
            .retain(|list| !staged_rejected(&accounts, &list.user.with_device(0)));
        accepted.lid_mappings.retain(|mapping| {
            [
                Jid::pn(mapping.phone_number.as_str()),
                Jid::lid(mapping.lid.as_str()),
            ]
            .iter()
            .all(|user| {
                !staged_rejected(&accounts, user) && accounts.row(user).is_none_or(|row| !row.empty)
            })
        });
        if metadata_only {
            // Cold mapped contacts can share a response with a different peer
            // that acquired history while the IQ was in flight. Admission must
            // cover that whole batch before any mapping reaches its write ACK.
            if identities.recovered.is_some()
                || accepted
                    .device_lists
                    .iter()
                    .any(|list| list.devices.iter().any(|device| device.is_hosted))
            {
                return Err(DeviceListMaintenanceRequired.into());
            }
            if accepted.lid_mappings.len() != response.lid_mappings.len() {
                // Empty cached projections suppress their own mappings. Replan
                // against the aliases that will actually be learned, not those
                // discarded during trust/empty-row filtering.
                proposed.retain(|user, alias| {
                    [user, alias].into_iter().all(|user| {
                        !staged_rejected(&accounts, user)
                            && accounts.row(user).is_none_or(|row| !row.empty)
                    })
                });
                for row in &mut accounts.rows {
                    row.requires_maintenance = false;
                }
                let records = self
                    .load_device_records_batch_projected_checked(&projection_users, &proposed)
                    .await?;
                for ((list, proof), previous) in
                    response.device_lists.iter().zip(&proofs).zip(records)
                {
                    let user = list.user.with_device(0);
                    if staged_rejected(&accounts, &user) {
                        continue;
                    }
                    let decoded = proof.as_ref().map(|proof| &proof.decoded);
                    let raw_changed = previous
                        .as_ref()
                        .and_then(|previous| previous.raw_id)
                        .zip(decoded.map(|decoded| decoded.raw_id))
                        .is_some_and(|(old, new)| old != new);
                    let devices = project_adv_devices(
                        list,
                        decoded,
                        if raw_changed { None } else { previous.as_ref() },
                    );
                    let index = accounts.track(&user);
                    accounts.rows[index].requires_maintenance |= projection_requires_maintenance(
                        previous.as_ref(),
                        &devices,
                        decoded,
                        mode,
                        &mut retained,
                    );
                }
            }
            if accounts
                .rows
                .iter()
                .any(|row| !row.rejected && row.requires_maintenance)
            {
                return Err(DeviceListMaintenanceRequired.into());
            }
        }
        let proofs = response
            .device_lists
            .iter()
            .zip(proofs)
            .filter(|(list, _)| !staged_rejected(&accounts, &list.user.with_device(0)))
            .map(|(_, proof)| proof)
            .collect();
        Ok(StagedDeviceLists {
            response: accepted,
            proofs,
        })
    }

    /// Lock every known primary before mappings or registry, matching migration.
    /// Recheck aliases after acquiring the locks: a mapping can change while we wait.
    async fn device_list_identity_guards(
        &self,
        users: &[Jid],
    ) -> anyhow::Result<(
        Vec<async_lock::MutexGuardArc<()>>,
        crate::lid_pn_cache::LidPnMutationGuard<'_>,
    )> {
        self.device_list_identity_guards_with_recovery(users, None)
            .await
    }

    /// Recovery cannot adopt a newly discovered alias while waiting for locks.
    /// Check its observed relations before collecting addresses and once locked.
    async fn device_list_identity_guards_with_recovery(
        &self,
        users: &[Jid],
        recovery: Option<(&Jid, &AdvRecoverySnapshot)>,
    ) -> anyhow::Result<(
        Vec<async_lock::MutexGuardArc<()>>,
        crate::lid_pn_cache::LidPnMutationGuard<'_>,
    )> {
        for _ in 0..DEVICE_REFRESH_MAX_ATTEMPTS {
            let mut aliases = Vec::with_capacity(users.len());
            let mut addresses: Vec<_> = users.iter().map(|user| user.with_device(0)).collect();
            for user in users {
                let alias = self.device_list_identity_alias(user).await;
                anyhow::ensure!(
                    recovery.is_none_or(|(root, snapshot)| snapshot.matches_observed_alias(
                        root,
                        user,
                        alias.as_ref()
                    )),
                    "account alias changed during ADV recovery"
                );
                addresses.extend(alias.iter().map(|alias| alias.with_device(0)));
                aliases.push(alias);
            }
            addresses.sort_unstable_by(wacore::types::jid::cmp_for_lock_order);
            addresses.dedup_by(|a, b| wacore::types::jid::cmp_for_lock_order(a, b).is_eq());
            let sessions = self.session_guards_for(&addresses).await;
            let mapping = self.lid_pn_cache.lock_mutation().await;
            let mut unchanged = true;
            for (user, alias) in users.iter().zip(&aliases) {
                let current = self.device_list_identity_alias(user).await;
                anyhow::ensure!(
                    recovery.is_none_or(|(root, snapshot)| snapshot.matches_observed_alias(
                        root,
                        user,
                        current.as_ref()
                    )),
                    "account alias changed during ADV recovery"
                );
                unchanged &= current == *alias;
            }
            if unchanged {
                return Ok((sessions, mapping));
            }
        }
        anyhow::bail!("account aliases changed during ADV lock acquisition")
    }

    /// Only a known reciprocal alias can carry the same trusted primary across
    /// migration. Never resolve a conflict by choosing one stored key over another.
    /// Callers hold the mapping mutation guard through the live reads and
    /// publication or cold CAS/ACK. That same guard excludes companion bootstrap
    /// without reacquiring session locks its caller may already own.
    async fn device_list_account_identity(&self, user: &Jid) -> anyhow::Result<Option<[u8; 32]>> {
        let primary = self.cached_adv_account_identity(user).await?;
        let Some(alias) = self.device_list_identity_alias(user).await else {
            return Ok(primary);
        };
        if self.device_list_identity_alias(&alias).await.as_ref() != Some(&user.with_device(0)) {
            return Err(DeviceListIdentityConflict::Alias.into());
        }
        let other = self.cached_adv_account_identity(&alias).await?;
        if primary.is_some() && other.is_some() && primary != other {
            return Err(DeviceListIdentityConflict::Primary.into());
        }
        Ok(primary.or(other))
    }

    /// First-contact writes share publication's fence, but never reacquire
    /// session locks: companion fetchers may already hold them. Network fetch
    /// and authentication finish before entering this write scope.
    #[cfg(test)]
    pub(crate) async fn persist_adv_bootstrap_identity(
        &self,
        primary: &Jid,
        key: &[u8; 32],
    ) -> anyhow::Result<bool> {
        self.persist_adv_bootstrap_identity_in_epoch(
            primary,
            key,
            self.signal_cache.identity_continuity.snapshot(),
        )
        .await
    }

    pub(crate) async fn persist_adv_bootstrap_identity_in_epoch(
        &self,
        primary: &Jid,
        key: &[u8; 32],
        epoch: Option<u64>,
    ) -> anyhow::Result<bool> {
        let client = self
            .self_weak
            .get()
            .and_then(|weak| weak.upgrade())
            .ok_or_else(|| anyhow::anyhow!("client ownership unavailable during bootstrap"))?;
        let primary = primary.clone();
        let key = *key;
        let (send, receive) = futures::channel::oneshot::channel();
        self.runtime.spawn_detached(Box::pin(async move {
            let result = client
                .persist_adv_bootstrap_identity_inner(&primary, &key, epoch)
                .await;
            if let Err(Err(error)) = send.send(result) {
                warn!("Owned ADV bootstrap failed after caller cancellation: {error:#}");
            }
        }));
        receive
            .await
            .map_err(|_| anyhow::anyhow!("bootstrap identity worker stopped"))?
    }

    async fn persist_adv_bootstrap_identity_inner(
        &self,
        primary: &Jid,
        key: &[u8; 32],
        epoch: Option<u64>,
    ) -> anyhow::Result<bool> {
        use wacore::types::jid::JidExt;
        let _mapping = self.lid_pn_cache.lock_mutation().await;
        // Retain the paired-account fallback check. The cache transaction below
        // repeats both live Signal anchors after waiting for durable ordering.
        if self
            .device_list_account_identity(primary)
            .await?
            .is_some_and(|current| current != *key)
        {
            return Ok(false);
        }
        let alias = self.device_list_identity_alias(primary).await;
        if let Some(alias) = &alias
            && self.device_list_identity_alias(alias).await.as_ref()
                != Some(&primary.with_device(0))
        {
            return Err(DeviceListIdentityConflict::Alias.into());
        }
        let alias = alias.map(|alias| alias.to_protocol_address());
        let backend = self.persistence_manager.backend();
        self.signal_cache
            .persist_identity_and_alias_in_epoch(
                &primary.to_protocol_address(),
                alias.as_ref(),
                key,
                backend.as_ref(),
                epoch,
            )
            .await
    }

    /// Load ordinary account anchors before taking mapping/registry guards.
    /// A device notification on the read loop may need those guards before the
    /// IQ reply is read. Hosted lists carry their separately verified anchor.
    async fn prepare_device_list_identities(
        &self,
        response: &DeviceListResponse,
        mode: DeviceListApplyMode,
    ) -> anyhow::Result<PreparedDeviceListIdentities> {
        let mut prepared = Vec::new();
        for user in &response.device_lists {
            if user.key_index_bytes.is_some() && !user.devices.iter().any(|device| device.is_hosted)
            {
                let cached = {
                    let (_sessions, _mapping) = self
                        .device_list_identity_guards(std::slice::from_ref(&user.user))
                        .await?;
                    self.device_list_account_identity(&user.user).await
                };
                let identity = match cached {
                    Ok(Some(identity)) => Ok(identity),
                    Ok(None) => self.ensure_adv_account_identity(&user.user).await,
                    Err(error) => Err(error),
                };
                let identity = match identity {
                    Ok(identity) => PreparedAccountIdentity::Trusted(identity),
                    Err(error)
                        if mode == DeviceListApplyMode::GroupRefresh
                            && error.is::<DeviceListIdentityConflict>() =>
                    {
                        PreparedAccountIdentity::Rejected
                    }
                    Err(error) => return Err(error),
                };
                prepared.push((user.user.clone(), identity));
            }
        }
        Ok(prepared)
    }

    /// Inspect both alias directions in one registry batch before learning
    /// mappings, so a cold group does not add per-contact storage round trips.
    async fn device_mapping_has_registry_history(
        &self,
        mappings: &[wacore::usync::UsyncLidMapping],
    ) -> bool {
        let users: Vec<_> = mappings
            .iter()
            .flat_map(|mapping| {
                [
                    Jid::pn(mapping.phone_number.as_str()),
                    Jid::lid(mapping.lid.as_str()),
                ]
            })
            .collect();
        self.load_device_records_batch(&users.iter().collect::<Vec<_>>())
            .await
            .into_iter()
            .any(|record| record.is_some())
    }

    /// Retire old account state and publish trust before applying the new topology.
    /// Keep this rare reaction out of the common device-list polling loop.
    #[cold]
    #[inline(never)]
    async fn apply_adv_identity_update(
        &self,
        update: AdvIdentityUpdate<'_>,
        guard: &crate::client::device_topology::DeviceRegistryMutationGuard<'_>,
        maintenance: &SignalMaintenance<'_>,
    ) -> anyhow::Result<()> {
        maintenance.ensure_current()?;
        let ownership = self.persistence_manager.lock_device_ownership().await;
        let alias = self.device_list_identity_alias(update.user).await;
        self.validate_adv_recovery_ownership(update.user, alias.as_ref())?;
        let changed = update
            .identities
            .iter()
            .any(|(_, previous)| previous.is_some_and(|previous| previous != update.identity));
        if changed {
            // Re-adding primary membership requires a fresh SKDM. Ordinary
            // prekey recovery must also retire the session tied to the old key.
            self.delete_sender_key_rows_for_device(&update.user.user, 0)
                .await?;
            if update.reset_primary_session {
                for (address, _) in &update.identities {
                    self.signal_cache.delete_session(address).await;
                }
                maintenance.flush().await?;
            }
        }
        if let Some(previous) = update.previous_record {
            for device in &previous.devices {
                if device.device_id() != 0 {
                    self.delete_sender_key_rows_for_device(&update.user.user, device.device_id())
                        .await?;
                }
            }
            // This scope propagates persistence failure, so one flush is the
            // durable retirement barrier before publishing replacement trust.
            maintenance
                .clear_device_record(&update.user.user, &previous)
                .await?;
        }
        if changed {
            self.try_invalidate_device_cache_guarded(&update.user.user, guard)
                .await?;
        }
        let backend = self.persistence_manager.backend();
        maintenance.ensure_current()?;
        anyhow::ensure!(
            self.signal_cache
                .persist_identities_if_equal(
                    &update.identities,
                    &update.identity,
                    backend,
                    self.runtime.as_ref(),
                )
                .await?,
            "primary identity changed during hosted ADV verification for {}",
            update.user
        );
        // Event dispatch and refresh can invoke host/runtime code; neither
        // belongs inside the manager's ownership read scope.
        drop(ownership);
        if let Some(previous) = update.previous_identity
            && previous != update.identity
        {
            use crate::types::events::{Event, EventKind};
            self.core
                .event_bus
                .dispatch_with(EventKind::IdentityChange, || {
                    Event::IdentityChange(
                        crate::types::events::IdentityChange::builder()
                            .user(update.user.to_non_ad())
                            .implicit(true)
                            .previous_identity_key(previous)
                            .current_identity_key(update.identity)
                            .build(),
                    )
                });
            // The token IQ stays outside registry locks and cannot reset the
            // newly published topology a second time.
            if !update.user.is_bot()
                && !update.user.is_status_broadcast()
                && let Some(client) = self.self_weak.get().and_then(|weak| weak.upgrade())
            {
                let user = update.user.clone();
                self.runtime
                    .spawn(Box::pin(async move {
                        client.reissue_tc_token_after_identity_change(&user).await;
                    }))
                    .detach();
            }
        }
        Ok(())
    }

    /// Share the batch state machine instead of embedding it in each caller.
    #[inline(never)]
    fn process_device_list_response_guarded<'a>(
        &'a self,
        response: &'a DeviceListResponse,
        mode: DeviceListApplyMode,
        guard: &'a crate::client::device_topology::DeviceRegistryMutationGuard<'_>,
        recovered_identity: Option<&'a RecoveredAccountIdentity>,
        maintenance: Option<&'a SignalMaintenance<'a>>,
        proofs: Option<&'a [Option<VerifiedDeviceListEnvelope>]>,
    ) -> wacore::runtime::BoxFuture<'a, Result<Vec<Jid>, anyhow::Error>> {
        Box::pin(self.process_device_list_response_guarded_inner(
            response,
            mode,
            guard,
            recovered_identity,
            maintenance,
            proofs,
        ))
    }

    /// Validate the accepted batch, retire old state and publish verified lists.
    async fn process_device_list_response_guarded_inner(
        &self,
        response: &DeviceListResponse,
        mode: DeviceListApplyMode,
        guard: &crate::client::device_topology::DeviceRegistryMutationGuard<'_>,
        recovered_identity: Option<&RecoveredAccountIdentity>,
        maintenance: Option<&SignalMaintenance<'_>>,
        proofs: Option<&[Option<VerifiedDeviceListEnvelope>]>,
    ) -> Result<Vec<Jid>, anyhow::Error> {
        let mut fetched_devices = Vec::new();
        if mode != DeviceListApplyMode::GroupRefresh {
            fetched_devices.reserve(response.device_lists.len());
        }
        let mut device_records: Vec<wacore::store::traits::DeviceListRecord> =
            Vec::with_capacity(response.device_lists.len());
        let track_subjects =
            mode != DeviceListApplyMode::GroupRefresh && response.device_lists.len() > 1;
        let mut accepted_subjects = Vec::new();
        if track_subjects {
            accepted_subjects.reserve(response.device_lists.len());
        }
        struct PendingIdentityReset<'a> {
            user: &'a Jid,
            previous: wacore::store::traits::DeviceListRecord,
            invalidate_registry: bool,
        }
        // Identity changes are rare, so this stays allocation-free for the
        // ordinary response. More importantly, deferring the destructive work
        // lets an authoritative refresh validate every user before any prior
        // Signal sessions or registry snapshot are discarded.
        let mut pending_identity_resets = Vec::new();
        let mut identity_updates = Vec::new();
        let mut pending_removals = Vec::new();
        let mut retained_ids = std::collections::HashSet::new();

        // The existing records for the whole response in one alias-aware
        // (LID <-> PN) batch, so a response covering a large group does not
        // pay a serialized backend read per member before it can be applied.
        let users: Vec<&Jid> = response
            .device_lists
            .iter()
            .map(|user_list| &user_list.user)
            .collect();
        let mut existing_records = self
            .load_device_records_batch_checked(&users)
            .await?
            .into_iter();

        for (position, user_list) in response.device_lists.iter().enumerate() {
            // Update device registry (single source of truth for device lists).
            // Preserve key_index values from existing records (set via account_sync)
            let mut existing_record = existing_records.next().flatten();

            let hosted = user_list.devices.iter().any(|device| device.is_hosted);
            let proof = proofs
                .and_then(|proofs| proofs.get(position))
                .and_then(Option::as_ref);
            let mut verified_identity = None;
            let decoded_key_index = if let Some(bytes) = user_list.key_index_bytes.as_deref() {
                let decoded = if hosted {
                    proof
                        .map(|proof| (proof.decoded.clone(), proof.key))
                        .or_else(|| wacore::adv::verify_hosted_key_index_list(bytes))
                        .map(|(decoded, identity)| {
                            verified_identity = Some(identity);
                            decoded
                        })
                } else {
                    let identity = if let Some(recovered) = recovered_identity
                        && recovered.user == user_list.user
                    {
                        self.validate_adv_recovery_ownership(
                            &recovered.user,
                            recovered.snapshot.publication_alias.as_ref(),
                        )?;
                        for (jid, previous) in &recovered.snapshot.identities {
                            anyhow::ensure!(
                                self.cached_adv_account_identity(jid).await? == *previous,
                                "primary identity changed during ADV recovery"
                            );
                        }
                        anyhow::ensure!(
                            self.device_list_identity_alias(&user_list.user)
                                .await
                                .as_ref()
                                == recovered.snapshot.publication_alias.as_ref(),
                            "account alias changed during ADV recovery"
                        );
                        if let Some(alias) = &recovered.snapshot.publication_alias {
                            anyhow::ensure!(
                                self.device_list_identity_alias(alias).await.as_ref()
                                    == Some(&recovered.user),
                                "account alias changed during ADV recovery"
                            );
                        }
                        verified_identity = Some(recovered.identity);
                        recovered.identity
                    } else {
                        self.device_list_account_identity(&user_list.user)
                            .await?
                            .ok_or_else(|| {
                                anyhow::anyhow!("device list account identity unavailable")
                            })?
                    };
                    if let Some(proof) = proof {
                        // Envelope authentication is reusable; live primary
                        // continuity is still checked under publication guards.
                        (proof.key == identity).then(|| proof.decoded.clone())
                    } else {
                        wacore::adv::verify_key_index_list(bytes, &identity)
                    }
                };
                if decoded.is_none() {
                    if mode == DeviceListApplyMode::GroupRefresh {
                        continue;
                    }
                    return Err(DeviceListAdvError {
                        user: user_list.user.clone(),
                        hosted,
                        response: response.clone(),
                        topology_generation: self.device_topology.current(),
                    }
                    .into());
                }
                decoded
            } else {
                if user_list.devices.iter().any(|device| device.device != 0) {
                    if mode == DeviceListApplyMode::GroupRefresh {
                        continue;
                    }
                    anyhow::bail!("companion device list omits ADV signature");
                }
                None
            };

            // A hosted signature or authenticated primary recovery can authorize
            // a different key with unchanged raw_id. Retire the old trust epoch.
            let identity_update = if let Some(identity) = verified_identity {
                let previous_identity = self.cached_adv_account_identity(&user_list.user).await?;
                let snapshot = self.persistence_manager.get_device_snapshot();
                let is_self = snapshot
                    .pn
                    .iter()
                    .chain(snapshot.lid.iter())
                    .any(|own| own.is_same_chat_as(&user_list.user));
                if is_self
                    && previous_identity != Some(identity)
                    && mode == DeviceListApplyMode::GroupRefresh
                {
                    continue;
                }
                anyhow::ensure!(
                    !is_self || previous_identity == Some(identity),
                    "hosted list cannot replace our paired primary identity"
                );
                let mut identities = Vec::with_capacity(2);
                use wacore::types::jid::JidExt as _;
                identities.push((
                    user_list.user.with_device(0).to_protocol_address(),
                    previous_identity,
                ));
                if !is_self {
                    let alias = if let Some(recovered) = recovered_identity
                        && recovered.user == user_list.user
                    {
                        recovered.snapshot.publication_alias.clone()
                    } else {
                        self.device_list_identity_alias(&user_list.user).await
                    };
                    if let Some(alias) = alias {
                        let previous = self.cached_adv_account_identity(&alias).await?;
                        identities.push((alias.to_protocol_address(), previous));
                    }
                }
                let changed = identities
                    .iter()
                    .any(|(_, previous)| previous.is_some_and(|previous| previous != identity));
                if is_self
                    || identities
                        .iter()
                        .all(|(_, previous)| *previous == Some(identity))
                {
                    None
                } else {
                    Some(AdvIdentityUpdate {
                        user: &user_list.user,
                        identity,
                        previous_identity: identities
                            .iter()
                            .filter_map(|(_, previous)| *previous)
                            .find(|previous| *previous != identity)
                            .or(previous_identity),
                        identities,
                        reset_primary_session: !hosted,
                        previous_record: if changed {
                            Some(match existing_record.take() {
                                Some(mut record) if !hosted => {
                                    // A selected registry row is not a complete
                                    // history of sessions under both aliases.
                                    let orphans =
                                        self.recover_device_cleanup_record(&user_list.user).await?;
                                    let mut devices = record.devices.into_vec();
                                    for device in orphans.devices {
                                        if !devices
                                            .iter()
                                            .any(|old| old.device_id() == device.device_id())
                                        {
                                            devices.push(device);
                                        }
                                    }
                                    record.devices = devices.into_boxed_slice();
                                    record
                                }
                                Some(record) => record,
                                None => {
                                    match self.recover_device_cleanup_record(&user_list.user).await
                                    {
                                        Ok(record) => record,
                                        Err(error) if mode == DeviceListApplyMode::GroupRefresh => {
                                            warn!(
                                                "Skipping hosted user after orphan cleanup recovery failed: {error:#}"
                                            );
                                            continue;
                                        }
                                        Err(error) => return Err(error),
                                    }
                                }
                            })
                        } else {
                            None
                        },
                    })
                }
            } else {
                None
            };

            // Check raw_id mismatch for identity change detection
            // TODO: also check advAccountType mismatch (see patch_device_add TODO)
            let mut raw_id = decoded_key_index.as_ref().map(|d| d.raw_id);
            let pending_identity_reset = if let Some(ref decoded) = decoded_key_index
                && let Some(ref existing) = existing_record
                && let Some(stored_raw_id) = existing.raw_id
                && stored_raw_id != decoded.raw_id
            {
                log::info!(
                    "raw_id mismatch for user {} in usync: stored={stored_raw_id}, received={}. Scheduling record reset.",
                    user_list.user.user,
                    decoded.raw_id
                );
                existing_record.take()
            } else {
                None
            };

            // Preserve raw_id from existing when usync didn't provide one
            // (no key-index-list). An identity change takes the old record out
            // above, so its raw_id and key indices cannot leak into the new one.
            if raw_id.is_none() {
                raw_id = existing_record
                    .as_ref()
                    .filter(|record| !record.devices.is_empty())
                    .and_then(|record| record.raw_id);
            }

            let devices = project_adv_devices(
                user_list,
                decoded_key_index.as_ref(),
                existing_record.as_ref(),
            );

            // An empty device list is never valid — WA Web always keeps the primary
            // (device 0), so a usync returning no devices for a user is transient or
            // corrupt. Persisting it would clobber a good cached record, or store an
            // empty one that get_user_devices then re-fetches on every send.
            if devices.is_empty() {
                if mode == DeviceListApplyMode::StrictRefresh {
                    anyhow::bail!(
                        "device-list refresh left no valid devices for {}",
                        user_list.user
                    );
                }
                if mode == DeviceListApplyMode::GroupRefresh {
                    continue;
                }
                if let Some(previous) = pending_identity_reset {
                    pending_identity_resets.push(PendingIdentityReset {
                        user: &user_list.user,
                        previous,
                        // No replacement record will be written. Keeping the old
                        // snapshot would pair a new identity with stale devices
                        // and suppress the next authoritative network fetch.
                        invalidate_registry: true,
                    });
                }
                continue;
            }

            if let Some(previous) = existing_record.as_ref() {
                retained_ids.clear();
                retained_ids.extend(devices.iter().map(|device| device.device_id()));
                for device in &previous.devices {
                    // The same response can reuse an ID whose old key index
                    // was revoked. Its replacement must not inherit a warm mark.
                    if device_requires_cleanup(
                        device,
                        &devices,
                        decoded_key_index.as_ref(),
                        &retained_ids,
                    ) {
                        pending_removals.push((&user_list.user, device.device_id()));
                    }
                }
            }

            if let Some(previous) = pending_identity_reset {
                pending_identity_resets.push(PendingIdentityReset {
                    user: &user_list.user,
                    previous,
                    invalidate_registry: false,
                });
            }

            if let Some(update) = identity_update {
                if let Some(earlier) =
                    identity_updates
                        .iter()
                        .find(|earlier: &&AdvIdentityUpdate<'_>| {
                            earlier.identities.iter().any(|(address, _)| {
                                update.identities.iter().any(|(other, _)| address == other)
                            })
                        })
                {
                    anyhow::ensure!(
                        earlier.identity == update.identity,
                        "conflicting hosted identities in one device-list response"
                    );
                } else {
                    identity_updates.push(update);
                }
            }

            // Convert filtered DeviceInfo list back to JIDs for return
            if mode != DeviceListApplyMode::GroupRefresh {
                let user_jid = &user_list.user;
                for d in &devices {
                    fetched_devices
                        .push(user_jid.with_device_hosting(d.device_id(), d.is_hosted()));
                }
            }

            device_records.push(wacore::store::traits::DeviceListRecord {
                user: std::sync::Arc::from(user_list.user.user.as_str()),
                devices: devices.into_boxed_slice(),
                timestamp: self.runtime.wall_time_secs(),
                phash: user_list.phash.clone().map(Box::<str>::from),
                raw_id,
            });
            if track_subjects {
                accepted_subjects.push(Some(&user_list.user));
            }
        }

        if track_subjects && device_records.len() > 1 {
            // Storage and cache keep the last row for each canonical account.
            // Verify every row first, then return exactly that same snapshot.
            let mut last_by_account =
                std::collections::HashMap::with_capacity(device_records.len());
            let mut duplicate = false;
            for (position, record) in device_records.iter().enumerate() {
                let key = self.canonical_device_record_key(&record.user).await;
                if let Some(previous) = last_by_account.insert(key, position) {
                    accepted_subjects[previous] = None;
                    duplicate = true;
                }
            }
            if duplicate {
                fetched_devices.clear();
                for (subject, record) in accepted_subjects.into_iter().zip(&device_records) {
                    if let Some(subject) = subject {
                        fetched_devices.extend(record.devices.iter().map(|device| {
                            subject.with_device_hosting(device.device_id(), device.is_hosted())
                        }));
                    }
                }
            }
        }

        if maintenance.is_none()
            && (!identity_updates.is_empty()
                || !pending_identity_resets.is_empty()
                || !pending_removals.is_empty())
        {
            return Err(DeviceListMaintenanceRequired.into());
        }

        // Keep the durable old topology until every companion removal finishes.
        // It is the retry record if any cleanup operation fails. Only then may
        // invalidation and the verified identity replacement consume that history.
        for update in identity_updates {
            self.apply_adv_identity_update(
                update,
                guard,
                maintenance.ok_or(DeviceListMaintenanceRequired)?,
            )
            .await?;
        }

        // Lists without trust changes publish before destructive cleanup so a
        // failed write preserves their previous sessions. Key replacements above
        // already cleared and invalidated the old trust epoch. Everything here
        // runs under the mapping and registry guards.
        //
        // One batched backend write for the whole usync response — for
        // large groups this collapses N spawn_blocking SQLite hops into
        // a single transaction, which dominated the per-send wall-clock.
        if let Some(maintenance) = maintenance {
            maintenance.ensure_current()?;
        }
        self.update_device_lists_guarded(device_records, guard)
            .await?;
        for (user, device_id) in pending_removals {
            if device_id != 0 {
                let maintenance = maintenance.ok_or(DeviceListMaintenanceRequired)?;
                self.delete_sessions_for_devices_unflushed(&user.user, &[device_id])
                    .await;
                maintenance.flush().await?;
            }
            if let Err(e) = self
                .delete_sender_key_rows_for_device(&user.user, device_id)
                .await
            {
                // The replacement records are already durable; bias the live
                // cache toward redistribution so a removed-then-readded device
                // cannot ride a stale warm mark past the SKDM it needs.
                self.sender_key_device_cache
                    .invalidate_entries_for_device(&user.user, device_id)
                    .await;
                return Err(e.into());
            }
        }
        for reset in pending_identity_resets {
            for device in &reset.previous.devices {
                if device.device_id() != 0
                    && let Err(e) = self
                        .delete_sender_key_rows_for_device(&reset.user.user, device.device_id())
                        .await
                {
                    self.sender_key_device_cache
                        .invalidate_entries_for_device(&reset.user.user, device.device_id())
                        .await;
                    return Err(e.into());
                }
            }
            maintenance
                .ok_or(DeviceListMaintenanceRequired)?
                .clear_device_record(&reset.user.user, &reset.previous)
                .await?;
            if reset.invalidate_registry {
                self.invalidate_device_cache_guarded(&reset.user.user, guard)
                    .await;
            }
        }

        Ok(fetched_devices)
    }

    /// Re-sync own device list from the server. Mirrors WA Web `syncMyDeviceList`:
    /// sends the cached per-user `device_hash` so the server answers "unchanged"
    /// (by omitting the user) instead of returning the full list on every reconnect.
    /// On a changed list the server returns it and we update; omitted users keep
    /// their cache.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.usync.sync_own_device_list",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn sync_own_device_list(&self) -> Result<(), anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();

        let mut jids = Vec::with_capacity(2);
        let mut hashes: std::collections::HashMap<Jid, (String, i64)> =
            std::collections::HashMap::new();
        for own in device_snapshot.pn.iter().chain(device_snapshot.lid.iter()) {
            let bare = own.to_non_ad();
            // Carry the cached device_hash so an unchanged list is skipped server-side.
            if let Some(record) = self.load_device_record_for_jid(&bare).await
                && let Some(phash) = record.phash
            {
                hashes.insert(bare.clone(), (String::from(phash), record.timestamp));
            }
            jids.push(bare);
        }

        if jids.is_empty() {
            return Ok(());
        }

        let sid = self.generate_request_id();
        let spec = DeviceListSpec::with_hashes(jids, sid, hashes);
        let response = self.execute_streaming(spec).await?;
        // `process_device_list_response` only touches users the server actually
        // returned, so unchanged (omitted) own devices keep their cache.
        let devices = self
            .process_device_list_response(&response, crate::cache::Freshness::CachePreferred)
            .await?;
        log::info!(
            "Re-synced own device list: {} device(s) updated",
            devices.len()
        );
        Ok(())
    }

    /// WA Web: `doPendingDeviceSync()` — flush batched unknown-device users.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.usync.flush_pending_device_sync",
            level = "debug",
            skip_all
        )
    )]
    pub(crate) async fn flush_pending_device_sync(&self) {
        let pending = self.pending_device_sync.take_all();
        if pending.is_empty() {
            return;
        }

        debug!("Flushing pending device sync for {} users", pending.len());

        // Invalidate stale records so get_user_devices hits the network
        for jid in &pending {
            self.invalidate_device_cache(&jid.user).await;
        }

        match self.get_user_devices(&pending).await {
            Ok(devices) => {
                debug!(
                    "Pending device sync completed: {} devices across {} users",
                    devices.len(),
                    pending.len()
                );
            }
            Err(e) => {
                warn!(
                    "Pending device sync failed, re-enqueueing {} users: {e:?}",
                    pending.len()
                );
                for jid in &pending {
                    self.pending_device_sync.add(jid);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cache::Freshness;
    use crate::test_utils::create_test_client;
    use wacore::libsignal::protocol::{ProtocolAddress, SessionRecord};
    use wacore::store::traits::{DeviceInfo, DeviceListRecord};
    use wacore::types::jid::JidExt;

    /// Sign fictitious key-index details with the shared test account.
    pub(super) fn signed_key_index_bytes(valid_indexes: Vec<u32>, current_index: u32) -> Vec<u8> {
        let key_index = {
            let mut proto_ = waproto::whatsapp::ADVKeyIndexList::default();
            proto_.raw_id = Some(1);
            proto_.timestamp = Some(1_700_000_000);
            proto_.current_index = Some(current_index);
            proto_.valid_indexes = valid_indexes;
            proto_
        };
        crate::test_utils::sign_adv_key_index(waproto::codec::adv_key_index_list_to_vec(&key_index))
    }

    /// Authenticate fixture lists, including the required hosted envelope key.
    async fn authenticated_response_fixture(
        client: &Client,
        mut response: DeviceListResponse,
    ) -> DeviceListResponse {
        for user in &mut response.device_lists {
            crate::test_utils::seed_adv_primary(client, &user.user).await;
            if let Some(bytes) = &user.key_index_bytes {
                if let Ok(signed) = waproto::codec::adv_signed_key_index_list_decode(bytes)
                    && let Some(details) = signed.details
                    && waproto::codec::adv_key_index_list_decode(&details).is_ok()
                {
                    user.key_index_bytes = Some(crate::test_utils::sign_adv_key_index(details));
                }
            } else if user.devices.iter().any(|device| device.device != 0) {
                let previous = client.load_device_record_for_jid(&user.user).await;
                for device in &mut user.devices {
                    if device.device != 0 && device.key_index.is_none() {
                        device.key_index = previous
                            .as_ref()
                            .and_then(|record| {
                                record
                                    .devices
                                    .iter()
                                    .find(|old| old.device_id() == device.device)
                                    .and_then(|old| old.key_index())
                            })
                            .or(Some(u32::from(device.device)));
                    }
                }
                let indexes: Vec<_> = user
                    .devices
                    .iter()
                    .filter_map(|device| device.key_index)
                    .collect();
                let mut list = waproto::whatsapp::ADVKeyIndexList::default();
                list.raw_id = previous
                    .as_ref()
                    .and_then(|record| record.raw_id)
                    .or(Some(1));
                list.timestamp = Some(1_700_000_000);
                list.current_index = indexes.iter().copied().max();
                list.valid_indexes = indexes;
                user.key_index_bytes = Some(crate::test_utils::sign_adv_key_index(
                    waproto::codec::adv_key_index_list_to_vec(&list),
                ));
            }
            if user.devices.iter().any(|device| device.is_hosted)
                && let Some(bytes) = user.key_index_bytes.as_mut()
                && let Ok(mut signed) = waproto::codec::adv_signed_key_index_list_decode(bytes)
            {
                signed.account_signature_key = Some(
                    crate::test_utils::adv_test_account()
                        .public_key
                        .public_key_bytes()
                        .to_vec(),
                );
                *bytes = waproto::codec::adv_signed_key_index_list_to_vec(&signed);
            }
        }
        response
    }

    /// Seed a distinguishable session for preservation and cleanup assertions.
    pub(super) async fn seed_fresh_session(client: &Client, jid: &Jid) -> ProtocolAddress {
        let address = jid.to_protocol_address();
        client
            .signal_cache
            .put_session(&address, SessionRecord::new_fresh())
            .await;
        address
    }

    /// Observe both pending cache state and the persisted session fallback.
    pub(super) async fn has_session(client: &Client, address: &ProtocolAddress) -> bool {
        let snapshot = client.persistence_manager.get_device_snapshot();
        client
            .signal_cache
            .has_session(address, &*snapshot.backend)
            .await
            .unwrap()
    }

    /// Build the standard primary-plus-companion ADV response fixture.
    pub(super) fn adv_wire_response(user: &Jid, signature: Vec<u8>) -> wacore_binary::Node {
        adv_wire_response_with_device(user, signature, 2)
    }

    /// Build a signed-list fixture with one chosen companion device.
    fn adv_wire_response_with_device(
        user: &Jid,
        signature: Vec<u8>,
        companion: u16,
    ) -> wacore_binary::Node {
        adv_wire_response_with_hosting(user, signature, companion, false)
    }

    /// Build a wire fixture that preserves a companion's hosted addressing.
    fn adv_wire_response_with_hosting(
        user: &Jid,
        signature: Vec<u8>,
        companion: u16,
        hosted: bool,
    ) -> wacore_binary::Node {
        use wacore_binary::builder::NodeBuilder;
        let mut device = NodeBuilder::new("device")
            .attr("id", companion.to_string())
            .attr("key-index", "1");
        if hosted {
            device = device.attr("is_hosted", "true");
        }
        NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("user")
                        .attr("jid", user.to_string())
                        .children([NodeBuilder::new("devices")
                            .children([
                                NodeBuilder::new("device-list")
                                    .children([
                                        NodeBuilder::new("device").attr("id", "0").build(),
                                        device.build(),
                                    ])
                                    .build(),
                                NodeBuilder::new("key-index-list")
                                    .attr("ts", "1700000000")
                                    .bytes(signature)
                                    .build(),
                            ])
                            .build()])
                        .build()])
                    .build()])
                .build()])
            .build()
    }

    /// Bound recovery while preserving trusted state until verification succeeds.
    #[tokio::test]
    async fn adv_failure_refreshes_only_the_affected_user_once() {
        use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
        // Exercise both ordinary resolution and authoritative refresh. A failed
        // refresh must never restart recovery recursively or replace Signal trust.
        for freshness in [Freshness::CachePreferred, Freshness::Refresh] {
            for recover in [true, false] {
                let (client, transport) = create_iq_test_client().await;
                let user = Jid::pn("12025550130");
                let other = Jid::pn("12025550131");
                crate::test_utils::seed_adv_primary(&client, &user).await;
                let trusted = client.cached_adv_account_identity(&user).await.unwrap();
                for jid in [&user, &other] {
                    client
                        .update_device_list(DeviceListRecord {
                            user: jid.user.as_str().into(),
                            devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
                            timestamp: 1,
                            phash: None,
                            raw_id: Some(1),
                        })
                        .await
                        .unwrap();
                }
                let session = seed_fresh_session(&client, &user.with_device(7)).await;
                let group = "120363000000000130@g.us";
                let rows = vec![
                    (user.to_string(), true),
                    (user.with_device(7).to_string(), true),
                ];
                client
                    .persistence_manager
                    .set_sender_key_status(
                        group,
                        &[(rows[0].0.as_str(), true), (rows[1].0.as_str(), true)],
                    )
                    .await
                    .unwrap();
                assert_eq!(
                    client
                        .skdm_device_map(group)
                        .await
                        .device_has_key(&user.user, 7),
                    Some(true)
                );
                let operation = {
                    let client = client.clone();
                    let user = user.clone();
                    tokio::spawn(async move {
                        client
                            .fetch_user_devices_with_freshness(vec![user], freshness)
                            .await
                    })
                };
                let first = decode_sent_iq(&transport, 0).await;
                let id = first
                    .get()
                    .attrs()
                    .optional_string("id")
                    .unwrap()
                    .into_owned();
                answer_iq(&client, &id, &adv_wire_response(&user, vec![0xff])).await;
                let retry = decode_sent_iq(&transport, 1).await;
                let node = retry.get();
                let requested = node
                    .get_optional_child("usync")
                    .unwrap()
                    .get_optional_child("list")
                    .unwrap();
                let requested = requested.children().unwrap();
                assert_eq!(requested.len(), 1);
                assert_eq!(requested[0].attrs().jid("jid"), user);
                // Bad input must not erase the history needed for cleanup after
                // a verified replacement, or discard Signal trust and sessions.
                let previous = client.load_device_record_for_jid(&user).await.unwrap();
                assert_eq!(previous.raw_id, Some(1));
                assert_eq!(previous.devices[1].device_id(), 7);
                assert!(client.load_device_record_for_jid(&other).await.is_some());
                assert!(has_session(&client, &session).await);
                let id = node.attrs().optional_string("id").unwrap().into_owned();
                let signature = if recover {
                    signed_key_index_bytes(vec![1], 1)
                } else {
                    vec![0xff]
                };
                answer_iq(&client, &id, &adv_wire_response(&user, signature)).await;
                let result = operation.await.unwrap();
                if recover {
                    assert_eq!(result.unwrap(), vec![user.clone(), user.with_device(2)]);
                    assert!(
                        !has_session(&client, &session).await,
                        "a verified replacement must remove the old device's session"
                    );
                    let stored = client
                        .persistence_manager
                        .get_sender_key_devices(group)
                        .await
                        .unwrap();
                    assert_eq!(stored, vec![(user.to_string(), true)]);
                    assert_eq!(
                        client
                            .skdm_device_map(group)
                            .await
                            .device_has_key(&user.user, 7),
                        None
                    );
                } else {
                    let error = result.unwrap_err();
                    assert!(error.downcast_ref::<DeviceListAdvError>().is_some());
                    assert!(format!("{error:#}").contains(&user.to_string()));
                    assert!(error.to_string().contains("signature verification failed"));
                    assert!(client.load_device_record_for_jid(&user).await.is_some());
                    assert!(has_session(&client, &session).await);
                    assert_eq!(
                        client
                            .persistence_manager
                            .get_sender_key_devices(group)
                            .await
                            .unwrap(),
                        rows
                    );
                    assert_eq!(
                        client
                            .skdm_device_map(group)
                            .await
                            .device_has_key(&user.user, 7),
                        Some(true)
                    );
                }
                assert_eq!(transport.sent().len(), 2, "recovery must be bounded");
                assert_eq!(
                    client.cached_adv_account_identity(&user).await.unwrap(),
                    trusted
                );
                assert!(client.load_device_record_for_jid(&other).await.is_some());
                client.signal_shutdown_sync();
            }
        }
    }

    /// Hosted USync signs with the embedded account key, even after a key change.
    /// Web 1052559453: WAWebHandleAdvKeyIndexResultApi selects
    /// verifySKeyIndexWithAccSigKey only when the returned list has isHosted.
    #[tokio::test]
    async fn hosted_usync_verifies_embedded_key_and_replaces_stale_identity() {
        use wacore::libsignal::protocol::KeyPair;
        use wacore::usync::{UserDeviceList, UsyncDevice};
        for freshness in [Freshness::CachePreferred, Freshness::Refresh] {
            for hosted in [true, false] {
                for valid in [true, false] {
                    let client = create_test_client().await;
                    let collector =
                        std::sync::Arc::new(crate::test_utils::TestEventCollector::default());
                    client.subscribe_handler(collector.clone()).detach();
                    let user = Jid::pn("12025550160");
                    crate::test_utils::seed_adv_primary(&client, &user).await;
                    let trusted = client.cached_adv_account_identity(&user).await.unwrap();
                    client
                        .update_device_list(DeviceListRecord {
                            user: user.user.as_str().into(),
                            devices: [DeviceInfo::new(0, None), DeviceInfo::new(99, Some(1))]
                                .into(),
                            timestamp: 1,
                            phash: None,
                            raw_id: Some(1),
                        })
                        .await
                        .unwrap();
                    let session = seed_fresh_session(&client, &user.with_device(99)).await;
                    let group = "120363000000000160@g.us";
                    let device_jid = user.with_device(99).to_string();
                    client
                        .persistence_manager
                        .set_sender_key_status(
                            group,
                            &[
                                (user.to_string().as_str(), true),
                                (device_jid.as_str(), true),
                            ],
                        )
                        .await
                        .unwrap();
                    assert_eq!(
                        client
                            .skdm_device_map(group)
                            .await
                            .device_has_key(&user.user, 99),
                        Some(true)
                    );
                    let account = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
                    let mut signed = waproto::codec::adv_signed_key_index_list_decode(
                        &signed_key_index_bytes(vec![1], 1),
                    )
                    .unwrap();
                    let details = signed.details.as_ref().unwrap();
                    signed.account_signature = Some(
                        account
                            .private_key
                            .calculate_signature(
                                &[&[6, 2], details.as_slice()].concat(),
                                &mut rand::make_rng::<rand::rngs::StdRng>(),
                            )
                            .unwrap()
                            .to_vec(),
                    );
                    signed.account_signature_key =
                        Some(account.public_key.public_key_bytes().to_vec());
                    if !valid {
                        signed.account_signature.as_mut().unwrap()[0] ^= 1;
                    }
                    let response = DeviceListResponse {
                        device_lists: vec![UserDeviceList {
                            user: user.clone(),
                            // Reuse ID and raw_id: a key change must still clear old state.
                            devices: vec![
                                UsyncDevice::new(0, None),
                                UsyncDevice::new(99, Some(1)).with_hosting(hosted),
                            ],
                            phash: None,
                            key_index_bytes: Some(
                                waproto::codec::adv_signed_key_index_list_to_vec(&signed),
                            ),
                        }],
                        lid_mappings: vec![],
                    };
                    let result = client
                        .process_device_list_response(&response, freshness)
                        .await;
                    let events = collector.events();
                    let changes: Vec<_> = events
                        .iter()
                        .filter_map(|event| match &**event {
                            crate::types::events::Event::IdentityChange(change) => Some(change),
                            _ => None,
                        })
                        .collect();
                    if hosted && valid {
                        assert_eq!(changes.len(), 1);
                        assert_eq!(changes[0].previous_identity_key, trusted);
                        assert_eq!(
                            changes[0].current_identity_key,
                            Some(*account.public_key.public_key_bytes())
                        );
                        assert!(changes[0].implicit);
                        assert_eq!(
                            result.unwrap(),
                            vec![user.clone(), user.with_device_hosting(99, true)]
                        );
                        assert_eq!(
                            client.cached_adv_account_identity(&user).await.unwrap(),
                            Some(*account.public_key.public_key_bytes())
                        );
                        assert!(!has_session(&client, &session).await);
                        assert_eq!(
                            client
                                .skdm_device_map(group)
                                .await
                                .device_has_key(&user.user, 0),
                            None
                        );
                        assert!(
                            client
                                .persistence_manager
                                .get_sender_key_devices(group)
                                .await
                                .unwrap()
                                .is_empty()
                        );
                        assert_eq!(
                            client
                                .skdm_device_map(group)
                                .await
                                .device_has_key(&user.user, 99),
                            None
                        );
                        client.flush_signal_cache_batch_safe().await.unwrap();
                        assert_eq!(
                            client
                                .persistence_manager
                                .backend()
                                .load_identity(user.to_protocol_address().as_str())
                                .await
                                .unwrap(),
                            Some(*account.public_key.public_key_bytes())
                        );
                    } else {
                        assert!(changes.is_empty());
                        assert!(result.is_err());
                        assert_eq!(
                            client.cached_adv_account_identity(&user).await.unwrap(),
                            trusted
                        );
                        assert!(has_session(&client, &session).await);
                        assert_eq!(
                            client
                                .skdm_device_map(group)
                                .await
                                .device_has_key(&user.user, 0),
                            Some(true)
                        );
                        assert_eq!(
                            client
                                .skdm_device_map(group)
                                .await
                                .device_has_key(&user.user, 99),
                            Some(true)
                        );
                        assert!(
                            !client
                                .load_device_record_for_jid(&user)
                                .await
                                .unwrap()
                                .devices[1]
                                .is_hosted()
                        );
                    }
                    client.signal_shutdown_sync();
                }
            }
        }
    }

    /// Cold hosted resolution needs no primary prekey query or recovery IQ.
    #[tokio::test]
    async fn cold_hosted_usync_persists_identity_without_a_primary_fetch() {
        use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
        use wacore::usync::{UserDeviceList, UsyncDevice};
        for user in [Jid::pn("12025550163"), Jid::lid("100000000000163")] {
            let (client, transport) = create_iq_test_client().await;
            let response = authenticated_response_fixture(
                &client,
                DeviceListResponse {
                    device_lists: vec![UserDeviceList {
                        user: user.clone(),
                        devices: vec![
                            UsyncDevice::new(0, None),
                            UsyncDevice::new(99, Some(1)).with_hosting(true),
                        ],
                        phash: None,
                        key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
                    }],
                    lid_mappings: vec![],
                },
            )
            .await;
            // The fixture seeds identity for ordinary tests. Remove it here to
            // prove that a cold hosted list is sufficient on its own.
            client
                .signal_cache
                .delete_identity(&user.to_protocol_address())
                .await;
            let operation = {
                let client = client.clone();
                let user = user.clone();
                tokio::spawn(async move { client.get_user_devices_owned(vec![user]).await })
            };
            let query = decode_sent_iq(&transport, 0).await;
            let id = query
                .get()
                .attrs()
                .optional_string("id")
                .unwrap()
                .into_owned();
            let wire = adv_wire_response_with_hosting(
                &user,
                response.device_lists[0].key_index_bytes.clone().unwrap(),
                99,
                true,
            );
            answer_iq(&client, &id, &wire).await;
            let devices = tokio::time::timeout(std::time::Duration::from_secs(5), operation)
                .await
                .unwrap()
                .unwrap()
                .unwrap();
            assert_eq!(
                devices,
                vec![user.clone(), user.with_device_hosting(99, true)]
            );
            assert_eq!(transport.sent_count(), 1);
            assert_eq!(
                client
                    .get_user_devices_owned(vec![user.clone()])
                    .await
                    .unwrap(),
                devices
            );
            assert_eq!(transport.sent_count(), 1);
            let stored = client
                .persistence_manager
                .backend()
                .load_identity(user.to_protocol_address().as_str())
                .await
                .unwrap();
            assert_eq!(
                stored,
                Some(
                    *crate::test_utils::adv_test_account()
                        .public_key
                        .public_key_bytes()
                )
            );
            client.signal_shutdown_sync();
        }
    }

    /// Moving an existing ID into hosted addressing invalidates its warm mark.
    #[tokio::test]
    async fn hosted_namespace_change_clears_old_session_without_key_rotation() {
        use wacore::usync::{UserDeviceList, UsyncDevice};
        let client = create_test_client().await;
        let user = Jid::pn("12025550166");
        let response = authenticated_response_fixture(
            &client,
            DeviceListResponse {
                device_lists: vec![UserDeviceList {
                    user: user.clone(),
                    devices: vec![
                        UsyncDevice::new(0, None),
                        UsyncDevice::new(99, Some(1)).with_hosting(true),
                    ],
                    phash: None,
                    key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
                }],
                lid_mappings: vec![],
            },
        )
        .await;
        let trusted = client.cached_adv_account_identity(&user).await.unwrap();
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(99, Some(1))].into(),
                timestamp: 1,
                phash: None,
                raw_id: Some(1),
            })
            .await
            .unwrap();
        let session = seed_fresh_session(&client, &user.with_device(99)).await;
        let group = "120363000000000166@g.us";
        let old = user.with_device(99).to_string();
        client
            .persistence_manager
            .set_sender_key_status(group, &[(old.as_str(), true)])
            .await
            .unwrap();
        assert_eq!(
            client
                .skdm_device_map(group)
                .await
                .device_has_key(&user.user, 99),
            Some(true)
        );
        client
            .process_device_list_response(&response, Freshness::Refresh)
            .await
            .unwrap();
        assert_eq!(
            client.cached_adv_account_identity(&user).await.unwrap(),
            trusted
        );
        assert!(!has_session(&client, &session).await);
        assert_eq!(
            client
                .skdm_device_map(group)
                .await
                .device_has_key(&user.user, 99),
            None
        );
        client.signal_shutdown_sync();
    }

    /// A peer-style hosted response cannot rotate the paired account's own key.
    #[tokio::test]
    async fn hosted_usync_cannot_replace_our_paired_primary_identity() {
        use wacore::store::commands::DeviceCommand;
        use wacore::usync::{UserDeviceList, UsyncDevice};
        let client = create_test_client().await;
        let user = Jid::pn("12025550167");
        let response = authenticated_response_fixture(
            &client,
            DeviceListResponse {
                device_lists: vec![UserDeviceList {
                    user: user.clone(),
                    devices: vec![
                        UsyncDevice::new(0, None),
                        UsyncDevice::new(99, Some(1)).with_hosting(true),
                    ],
                    phash: None,
                    key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
                }],
                lid_mappings: vec![],
            },
        )
        .await;
        let paired = wacore::libsignal::protocol::KeyPair::generate(&mut rand::make_rng::<
            rand::rngs::StdRng,
        >());
        let mut account = waproto::whatsapp::ADVSignedDeviceIdentity::default();
        account.account_signature_key = Some(paired.public_key.public_key_bytes().to_vec());
        client
            .signal_cache
            .delete_identity(&user.to_protocol_address())
            .await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetId(Some(user.with_device(1))))
            .await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetAccount(Some(account)))
            .await;
        let error = client
            .process_device_list_response(&response, Freshness::Refresh)
            .await
            .unwrap_err();
        assert!(error.to_string().contains("paired primary identity"));
        assert_eq!(
            client.cached_adv_account_identity(&user).await.unwrap(),
            Some(*paired.public_key.public_key_bytes())
        );
        assert!(client.load_device_record_for_jid(&user).await.is_none());
        client.signal_shutdown_sync();
    }

    /// A later invalid user must not commit an earlier hosted identity update.
    #[tokio::test]
    async fn rejected_usync_batch_preserves_hosted_trust_and_sessions() {
        use wacore::usync::{UserDeviceList, UsyncDevice};
        let client = create_test_client().await;
        let user = Jid::pn("12025550164");
        let other = Jid::pn("12025550165");
        let old = wacore::libsignal::protocol::KeyPair::generate(&mut rand::make_rng::<
            rand::rngs::StdRng,
        >());
        client
            .signal_cache
            .put_identity(
                &user.to_protocol_address(),
                old.public_key.public_key_bytes(),
            )
            .await;
        crate::test_utils::seed_adv_primary(&client, &other).await;
        let session = seed_fresh_session(&client, &user.with_device(7)).await;
        let mut signed =
            waproto::codec::adv_signed_key_index_list_decode(&signed_key_index_bytes(vec![1], 1))
                .unwrap();
        signed.account_signature_key = Some(
            crate::test_utils::adv_test_account()
                .public_key
                .public_key_bytes()
                .to_vec(),
        );
        let response = DeviceListResponse {
            device_lists: vec![
                UserDeviceList {
                    user: user.clone(),
                    devices: vec![
                        UsyncDevice::new(0, None),
                        UsyncDevice::new(99, Some(1)).with_hosting(true),
                    ],
                    phash: None,
                    key_index_bytes: Some(waproto::codec::adv_signed_key_index_list_to_vec(
                        &signed,
                    )),
                },
                UserDeviceList {
                    user: other,
                    devices: vec![UsyncDevice::new(0, None)],
                    phash: None,
                    key_index_bytes: Some(vec![255]),
                },
            ],
            lid_mappings: vec![],
        };
        assert!(
            client
                .process_device_list_response(&response, Freshness::Refresh)
                .await
                .is_err()
        );
        assert_eq!(
            client.cached_adv_account_identity(&user).await.unwrap(),
            Some(*old.public_key.public_key_bytes())
        );
        assert!(has_session(&client, &session).await);
        assert!(client.load_device_record_for_jid(&user).await.is_none());
        client.signal_shutdown_sync();
    }

    /// Rebuild a verified batch without invalidating unrelated users.
    #[tokio::test]
    async fn adv_recovery_rebuilds_the_full_batch_without_invalidating_other_users() {
        use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
        use wacore_binary::builder::NodeBuilder;
        let (client, transport) = create_iq_test_client().await;
        let users = [Jid::pn("12025550132"), Jid::pn("12025550133")];
        for user in &users {
            crate::test_utils::seed_adv_primary(&client, user).await;
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
                    timestamp: 1,
                    phash: None,
                    raw_id: Some(1),
                })
                .await
                .unwrap();
        }
        let batch_response = |bad| {
            let responses = [
                adv_wire_response(
                    &users[0],
                    if bad {
                        vec![0xff]
                    } else {
                        signed_key_index_bytes(vec![1], 1)
                    },
                ),
                adv_wire_response(&users[1], signed_key_index_bytes(vec![1], 1)),
            ];
            NodeBuilder::new("iq")
                .attr("type", "result")
                .children([NodeBuilder::new("usync")
                    .children([NodeBuilder::new("list")
                        .children(responses.iter().map(|response| {
                            response
                                .get_optional_child("usync")
                                .unwrap()
                                .get_optional_child("list")
                                .unwrap()
                                .get_optional_child("user")
                                .unwrap()
                                .clone()
                        }))
                        .build()])
                    .build()])
                .build()
        };
        let operation = {
            let client = client.clone();
            let users = users.to_vec();
            tokio::spawn(async move { client.refresh_user_devices(users).await })
        };
        for frame in 0..3 {
            let query = decode_sent_iq(&transport, frame).await;
            let node = query.get();
            let list = node
                .get_optional_child("usync")
                .unwrap()
                .get_optional_child("list")
                .unwrap();
            assert_eq!(
                list.children().unwrap().len(),
                if frame == 1 { 1 } else { 2 }
            );
            let response = if frame == 1 {
                let previous = client.load_device_record_for_jid(&users[0]).await.unwrap();
                assert_eq!(previous.devices[1].device_id(), 7);
                let other = client.load_device_record_for_jid(&users[1]).await.unwrap();
                assert_eq!(other.devices[1].device_id(), 7);
                adv_wire_response(&users[0], signed_key_index_bytes(vec![1], 1))
            } else {
                batch_response(frame == 0)
            };
            let id = node.attrs().optional_string("id").unwrap().into_owned();
            answer_iq(&client, &id, &response).await;
        }
        let devices = operation.await.unwrap().unwrap();
        for user in users {
            assert!(devices.contains(&user));
            assert!(devices.contains(&user.with_device(2)));
        }
        assert_eq!(devices.len(), 4);
        assert_eq!(transport.sent().len(), 3);
        client.signal_shutdown_sync();
    }

    /// A partial final batch must retain the recovered user's latest fanout.
    #[tokio::test]
    async fn adv_recovery_preserves_devices_omitted_from_the_final_batch() {
        use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
        use wacore_binary::builder::NodeBuilder;
        // The final reply either omits the user, follows a concurrent registry
        // update, or supplies a newer verified result that must take precedence.
        for (expected_companion, lid_to_pn) in [
            (0, None),
            (0, Some(false)),
            (0, Some(true)),
            (2, None),
            (4, None),
            (3, None),
            (3, Some(false)),
            (3, Some(true)),
        ] {
            let (client, transport) = create_iq_test_client().await;
            let phone = Jid::pn("12025550135");
            let lid = Jid::lid("100000000000135");
            let (user, alias) = if lid_to_pn == Some(true) {
                (lid.clone(), phone.clone())
            } else {
                (phone.clone(), lid.clone())
            };
            let users = [user, Jid::pn("12025550136")];
            if lid_to_pn.is_some() {
                client
                    .add_lid_pn_mapping(
                        &lid.user,
                        &phone.user,
                        crate::lid_pn_cache::LearningSource::Usync,
                    )
                    .await
                    .unwrap();
                // Mapping publication migrates Signal state; seed both wire
                // namespaces afterwards so the fixture never needs a key IQ.
                crate::test_utils::seed_adv_primary(&client, &alias).await;
            }
            for user in &users {
                crate::test_utils::seed_adv_primary(&client, user).await;
            }
            let operation = {
                let client = client.clone();
                let users = users.to_vec();
                tokio::spawn(async move { client.fetch_user_devices(users).await })
            };
            let mut frame = 0;
            let mut targeted_attempts = 0;
            loop {
                let query = decode_sent_iq(&transport, frame).await;
                let requested = query
                    .get()
                    .get_optional_child("usync")
                    .unwrap()
                    .get_optional_child("list")
                    .unwrap()
                    .children()
                    .unwrap()
                    .iter()
                    .map(|user| user.attrs().jid("jid"))
                    .collect::<Vec<_>>();
                let targeted = requested.len() == 1;
                let final_batch = frame > 0 && !targeted;
                if targeted {
                    assert_eq!(requested, vec![users[0].clone()]);
                    targeted_attempts += 1;
                    assert!(targeted_attempts <= DEVICE_REFRESH_MAX_ATTEMPTS);
                    if targeted_attempts == 1 {
                        // A topology change while the response is in flight
                        // must cause a strict retry, not consume the final batch.
                        client.device_topology.record([users[0].user.as_str()]);
                    }
                } else {
                    assert_eq!(requested, users);
                }
                let id = query
                    .get()
                    .attrs()
                    .optional_string("id")
                    .unwrap()
                    .into_owned();
                let mut replies = Vec::new();
                if !final_batch || expected_companion == 3 {
                    replies.push(adv_wire_response_with_device(
                        if final_batch && lid_to_pn.is_some() {
                            &alias
                        } else {
                            &users[0]
                        },
                        if frame == 0 {
                            vec![0xff]
                        } else {
                            signed_key_index_bytes(vec![1], 1)
                        },
                        if final_batch { 3 } else { 2 },
                    ));
                }
                if !targeted {
                    replies.push(adv_wire_response(
                        &users[1],
                        signed_key_index_bytes(vec![1], 1),
                    ));
                }
                if final_batch && expected_companion == 4 {
                    client
                        .update_device_list(DeviceListRecord {
                            user: users[0].user.as_str().into(),
                            devices: [DeviceInfo::new(0, None), DeviceInfo::new(4, Some(1))].into(),
                            timestamp: 2,
                            phash: None,
                            raw_id: Some(1),
                        })
                        .await
                        .unwrap();
                }
                if final_batch && expected_companion == 0 {
                    assert!(client.load_device_record_for_jid(&users[0]).await.is_some());
                    client.invalidate_device_cache(&users[0].user).await;
                    assert!(client.load_device_record_for_jid(&users[0]).await.is_none());
                    if lid_to_pn.is_some() {
                        assert!(client.load_device_record_for_jid(&alias).await.is_none());
                    }
                }
                let response = NodeBuilder::new("iq")
                    .attr("type", "result")
                    .children([NodeBuilder::new("usync")
                        .children([NodeBuilder::new("list")
                            .children(replies.iter().map(|reply| {
                                reply
                                    .get_optional_child("usync")
                                    .unwrap()
                                    .get_optional_child("list")
                                    .unwrap()
                                    .get_optional_child("user")
                                    .unwrap()
                                    .clone()
                            }))
                            .build()])
                        .build()])
                    .build();
                answer_iq(&client, &id, &response).await;
                if final_batch {
                    break;
                }
                frame += 1;
                assert!(frame <= DEVICE_REFRESH_MAX_ATTEMPTS + 1);
            }
            let result = operation.await.unwrap();
            if expected_companion == 0 {
                let error = result.unwrap_err();
                assert!(format!("{error:#}").contains(&users[0].to_string()));
                assert!(client.load_device_record_for_jid(&users[0]).await.is_none());
                assert!(client.load_device_record_for_jid(&users[1]).await.is_some());
                assert!(targeted_attempts >= 2);
                assert_eq!(transport.sent().len(), targeted_attempts + 2);
                client.signal_shutdown_sync();
                continue;
            }
            let mut actual = result.unwrap();
            let expected_user = if lid_to_pn.is_some() {
                &alias
            } else {
                &users[0]
            };
            let mut expected = vec![
                expected_user.clone(),
                expected_user.with_device(expected_companion),
                users[1].clone(),
                users[1].with_device(2),
            ];
            actual.sort_by_key(ToString::to_string);
            expected.sort_by_key(ToString::to_string);
            assert_eq!(actual, expected);
            assert!(
                targeted_attempts >= 2,
                "the topology change must force a retry"
            );
            assert_eq!(transport.sent().len(), targeted_attempts + 2);
            client.signal_shutdown_sync();
        }
    }

    /// Learning an alias during recovery must replace its earlier cached fanout.
    #[tokio::test]
    async fn adv_recovery_reconciles_aliases_learned_after_the_cache_read() {
        use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
        use wacore_binary::builder::NodeBuilder;
        for cached_is_lid in [false, true] {
            let (client, transport) = create_iq_test_client().await;
            let phone = Jid::pn("12025550138");
            let lid = Jid::lid("100000000000138");
            let other = Jid::pn("12025550140");
            let cached_other = Jid::pn("12025550139");
            let (cached, queried) = if cached_is_lid {
                (lid.clone(), phone.clone())
            } else {
                (phone.clone(), lid.clone())
            };
            for (user, companion) in [(&cached, 7), (&cached_other, 9)] {
                client
                    .update_device_list(DeviceListRecord {
                        user: user.user.as_str().into(),
                        devices: [
                            DeviceInfo::new(0, None),
                            DeviceInfo::new(companion, Some(1)),
                        ]
                        .into(),
                        timestamp: 1,
                        phash: None,
                        raw_id: Some(1),
                    })
                    .await
                    .unwrap();
            }
            for user in [&phone, &lid, &other] {
                crate::test_utils::seed_adv_primary(&client, user).await;
            }
            assert!(
                client
                    .lid_pn_cache
                    .get_current_lid(&phone.user)
                    .await
                    .is_none()
            );
            let operation = {
                let client = client.clone();
                let users = vec![
                    cached.clone(),
                    queried.clone(),
                    other.clone(),
                    cached_other.clone(),
                ];
                tokio::spawn(async move { client.get_user_devices_owned(users).await })
            };
            let mut frame = 0;
            let mut targeted_attempts = 0;
            loop {
                let query = decode_sent_iq(&transport, frame).await;
                let requested: Vec<_> = query
                    .get()
                    .get_optional_child("usync")
                    .unwrap()
                    .get_optional_child("list")
                    .unwrap()
                    .children()
                    .unwrap()
                    .iter()
                    .map(|user| user.attrs().jid("jid"))
                    .collect();
                let targeted = requested.len() == 1;
                let final_batch = frame > 0 && !targeted;
                if targeted {
                    assert!(requested[0] == queried || requested[0] == lid);
                    targeted_attempts += 1;
                    assert!(targeted_attempts <= DEVICE_REFRESH_MAX_ATTEMPTS);
                } else {
                    assert_eq!(requested, vec![queried.clone(), other.clone()]);
                }
                if frame == 0 {
                    assert!(
                        client
                            .lid_pn_cache
                            .get_current_lid(&phone.user)
                            .await
                            .is_none()
                    );
                    let old = client.load_device_record_for_jid(&cached).await.unwrap();
                    assert_eq!(old.devices[1].device_id(), 7);
                }
                if final_batch {
                    assert_eq!(
                        client
                            .lid_pn_cache
                            .get_current_lid(&phone.user)
                            .await
                            .as_deref(),
                        Some(lid.user.as_str())
                    );
                    let current = client.load_device_record_for_jid(&queried).await.unwrap();
                    assert_eq!(current.devices[1].device_id(), 2);
                }
                let mut replies = Vec::new();
                if frame == 0 {
                    replies.push(
                        adv_wire_response(&queried, vec![0xff])
                            .get_optional_child("usync")
                            .unwrap()
                            .get_optional_child("list")
                            .unwrap()
                            .get_optional_child("user")
                            .unwrap()
                            .clone(),
                    );
                } else if targeted {
                    let valid = adv_wire_response(&lid, signed_key_index_bytes(vec![1], 1));
                    let devices = valid
                        .get_optional_child("usync")
                        .unwrap()
                        .get_optional_child("list")
                        .unwrap()
                        .get_optional_child("user")
                        .unwrap()
                        .get_optional_child("devices")
                        .unwrap()
                        .clone();
                    replies.push(
                        NodeBuilder::new("user")
                            .attr("jid", lid.to_string())
                            .attr("pn_jid", phone.to_string())
                            .children([devices])
                            .build(),
                    );
                }
                if !targeted {
                    replies.push(
                        adv_wire_response(&other, signed_key_index_bytes(vec![1], 1))
                            .get_optional_child("usync")
                            .unwrap()
                            .get_optional_child("list")
                            .unwrap()
                            .get_optional_child("user")
                            .unwrap()
                            .clone(),
                    );
                }
                let response = NodeBuilder::new("iq")
                    .attr("type", "result")
                    .children([NodeBuilder::new("usync")
                        .children([NodeBuilder::new("list").children(replies).build()])
                        .build()])
                    .build();
                let id = query
                    .get()
                    .attrs()
                    .optional_string("id")
                    .unwrap()
                    .into_owned();
                answer_iq(&client, &id, &response).await;
                if final_batch {
                    break;
                }
                frame += 1;
                assert!(frame <= DEVICE_REFRESH_MAX_ATTEMPTS + 1);
            }
            let mut actual = operation.await.unwrap().unwrap();
            let mut expected = vec![
                queried.clone(),
                queried.with_device(2),
                other.clone(),
                other.with_device(2),
                cached_other.clone(),
                cached_other.with_device(9),
            ];
            actual.sort_by_key(ToString::to_string);
            expected.sort_by_key(ToString::to_string);
            assert_eq!(actual, expected);
            assert!(targeted_attempts >= 1);
            assert_eq!(transport.sent().len(), targeted_attempts + 2);
            client.signal_shutdown_sync();
        }
    }

    /// The targeted recovery must require a complete authoritative response.
    #[tokio::test]
    async fn adv_recovery_rejects_an_omitted_target_user() {
        use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
        use wacore_binary::builder::NodeBuilder;
        for freshness in [Freshness::CachePreferred, Freshness::Refresh] {
            let (client, transport) = create_iq_test_client().await;
            let user = Jid::pn("12025550137");
            crate::test_utils::seed_adv_primary(&client, &user).await;
            let trusted = client.cached_adv_account_identity(&user).await.unwrap();
            let operation = {
                let client = client.clone();
                let user = user.clone();
                tokio::spawn(async move {
                    client
                        .fetch_user_devices_with_freshness(vec![user], freshness)
                        .await
                })
            };
            for frame in 0..2 {
                let query = decode_sent_iq(&transport, frame).await;
                let id = query
                    .get()
                    .attrs()
                    .optional_string("id")
                    .unwrap()
                    .into_owned();
                let response = if frame == 0 {
                    adv_wire_response(&user, vec![0xff])
                } else {
                    NodeBuilder::new("iq")
                        .attr("type", "result")
                        .children([NodeBuilder::new("usync")
                            .children([NodeBuilder::new("list").build()])
                            .build()])
                        .build()
                };
                answer_iq(&client, &id, &response).await;
            }
            let error = operation.await.unwrap().unwrap_err();
            let message = format!("{error:#}");
            assert!(
                message.contains("device-list response omitted user"),
                "{message}"
            );
            assert!(message.contains(&user.to_string()), "{message}");
            assert_eq!(transport.sent().len(), 2);
            assert_eq!(
                client.cached_adv_account_identity(&user).await.unwrap(),
                trusted
            );
            client.signal_shutdown_sync();
        }
    }

    /// Leave state and request count unchanged for non-ADV IQ failures.
    #[tokio::test]
    async fn ordinary_iq_failure_does_not_invalidate_devices_or_retry() {
        use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
        use wacore_binary::builder::NodeBuilder;
        let (client, transport) = create_iq_test_client().await;
        let user = Jid::pn("12025550134");
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None)].into(),
                timestamp: 1,
                phash: None,
                raw_id: Some(1),
            })
            .await
            .unwrap();
        let operation = {
            let client = client.clone();
            let user = user.clone();
            tokio::spawn(async move { client.refresh_user_devices(vec![user]).await })
        };
        let query = decode_sent_iq(&transport, 0).await;
        let id = query
            .get()
            .attrs()
            .optional_string("id")
            .unwrap()
            .into_owned();
        let error = NodeBuilder::new("iq")
            .attr("type", "error")
            .children([NodeBuilder::new("error").attr("code", "500").build()])
            .build();
        answer_iq(&client, &id, &error).await;
        assert!(operation.await.unwrap().is_err());
        assert_eq!(transport.sent().len(), 1);
        assert!(client.load_device_record_for_jid(&user).await.is_some());
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn waiting_for_primary_identity_does_not_hold_registry_or_mapping_locks() {
        use wacore::usync::{UserDeviceList, UsyncDevice};
        let client = create_test_client().await;
        let user = Jid::pn("12025550127");
        let lock = client
            .session_lock_for(user.to_protocol_address().as_str())
            .await;
        let primary_guard = lock.lock().await;
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(2, Some(1))],
                phash: None,
                key_index_bytes: Some(signed_key_index_bytes(vec![1], 1)),
            }],
            lid_mappings: Vec::new(),
        };
        let operation = client.process_device_list_response(&response, Freshness::Refresh);
        futures::pin_mut!(operation);
        assert!(futures::poll!(operation.as_mut()).is_pending());
        let mapping = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            client.lid_pn_cache.lock_mutation(),
        )
        .await
        .unwrap();
        let registry = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            client.device_topology.lock_registry(),
        )
        .await
        .unwrap();
        drop(registry);
        drop(mapping);
        crate::test_utils::seed_adv_primary(&client, &user).await;
        drop(primary_guard);
        assert_eq!(
            operation.await.unwrap(),
            vec![user.clone(), user.with_device(2)]
        );
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn unverified_device_lists_preserve_registry_and_cannot_add_future_indexes() {
        use wacore::usync::{UserDeviceList, UsyncDevice};
        let client = create_test_client().await;
        let user = Jid::pn("12025550126");
        crate::test_utils::seed_adv_primary(&client, &user).await;
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(1))].into(),
                timestamp: 1,
                phash: None,
                raw_id: Some(1),
            })
            .await
            .unwrap();
        let valid = signed_key_index_bytes(vec![1], 1);
        let mut tampered = valid.clone();
        tampered[0] ^= 1;
        for signature in [None, Some(vec![0xff]), Some(tampered)] {
            let response = DeviceListResponse {
                device_lists: vec![UserDeviceList {
                    user: user.clone(),
                    devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(8, Some(100))],
                    phash: None,
                    key_index_bytes: signature,
                }],
                lid_mappings: Vec::new(),
            };
            assert!(
                client
                    .process_device_list_response(&response, Freshness::Refresh)
                    .await
                    .is_err()
            );
            let record = client.load_device_record_for_jid(&user).await.unwrap();
            assert_eq!(
                record
                    .devices
                    .iter()
                    .map(|device| device.device_id())
                    .collect::<Vec<_>>(),
                vec![0, 7]
            );
        }
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![
                    UsyncDevice::new(0, None),
                    UsyncDevice::new(7, Some(1)),
                    UsyncDevice::new(8, Some(100)),
                ],
                phash: None,
                key_index_bytes: Some(valid),
            }],
            lid_mappings: Vec::new(),
        };
        let devices = client
            .process_device_list_response(&response, Freshness::Refresh)
            .await
            .unwrap();
        assert_eq!(devices, vec![user.clone(), user.with_device(7)]);
        client.signal_shutdown_sync();
    }

    #[tokio::test]
    async fn group_refresh_partial_success_rebuilds_cached_unresolved_users() {
        use wacore::iq::spec::IqSpec;
        use wacore_binary::builder::NodeBuilder;

        let client = create_test_client().await;
        let users = vec![
            Jid::pn("12025550121"),
            Jid::pn("12025550122"),
            Jid::pn("12025550127"),
        ];
        for user in &users {
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                    timestamp: 1,
                    phash: None,
                    raw_id: Some(2),
                })
                .await
                .unwrap();
        }
        let wire = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("usync")
                .children([NodeBuilder::new("list")
                    .children([
                        NodeBuilder::new("user")
                            .attr("jid", users[0].to_string())
                            .children([NodeBuilder::new("devices")
                                .children([NodeBuilder::new("device-list")
                                    .children([NodeBuilder::new("device").attr("id", "0").build()])
                                    .build()])
                                .build()])
                            .build(),
                        NodeBuilder::new("user")
                            .attr("jid", users[1].to_string())
                            .children([NodeBuilder::new("devices")
                                .children([NodeBuilder::new("error").attr("code", "500").build()])
                                .build()])
                            .build(),
                    ])
                    .build()])
                .build()])
            .build();
        let response = DeviceListSpec::new(users.clone(), "group-partial")
            .parse_response(&wire.as_node_ref())
            .unwrap();
        assert!(
            DeviceListSpec::new(users.clone(), "dm-strict")
                .require_complete_response()
                .parse_response(&wire.as_node_ref())
                .is_err()
        );
        let devices = client
            .try_process_group_device_list_response(
                &response,
                &users,
                client.device_topology.current(),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(devices.len(), 5);
        assert!(!devices.contains(&users[0].with_device(7)));
        assert!(devices.contains(&users[1].with_device(7)));
        assert!(devices.contains(&users[2].with_device(7)));
    }

    #[tokio::test]
    async fn group_refresh_preserves_malformed_and_filtered_identity_records() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        for (bytes, devices) in [
            (Some(vec![0xff]), vec![UsyncDevice::new(7, Some(3))]),
            (
                Some(signed_key_index_bytes(Vec::new(), 10)),
                vec![UsyncDevice::new(7, Some(3))],
            ),
            (None, Vec::new()),
        ] {
            let client = create_test_client().await;
            let user = Jid::pn("12025550123");
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                    timestamp: 1,
                    phash: Some("old".into()),
                    raw_id: Some(2),
                })
                .await
                .unwrap();
            let session = seed_fresh_session(&client, &user.with_device(7)).await;
            let response = DeviceListResponse {
                device_lists: vec![UserDeviceList {
                    user: user.clone(),
                    devices,
                    phash: None,
                    key_index_bytes: bytes,
                }],
                lid_mappings: Vec::new(),
            };
            let response = authenticated_response_fixture(&client, response).await;
            let devices = client
                .try_process_group_device_list_response(
                    &response,
                    std::slice::from_ref(&user),
                    client.device_topology.current(),
                )
                .await
                .unwrap()
                .unwrap();
            assert_eq!(devices.len(), 2);
            let record = client.load_device_record_for_jid(&user).await.unwrap();
            assert_eq!(record.raw_id, Some(2));
            assert_eq!(record.phash.as_deref(), Some("old"));
            assert!(has_session(&client, &session).await);
        }
    }

    #[tokio::test]
    async fn group_refresh_rejects_response_after_concurrent_notification() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("12025550124");
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None)].into(),
                timestamp: 1,
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();
        let generation = client.device_topology.current();
        crate::test_utils::signed_device_add(
            &client,
            &user.user,
            &wacore::stanza::devices::DeviceElement {
                jid: user.with_device(7),
                key_index: None,
                lid: None,
            },
        )
        .await;
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        let response = authenticated_response_fixture(&client, response).await;
        assert!(
            client
                .try_process_group_device_list_response(
                    &response,
                    std::slice::from_ref(&user),
                    generation,
                )
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            client
                .get_devices_from_registry(&user)
                .await
                .unwrap()
                .contains(&user.with_device(7))
        );
        let omitted = DeviceListResponse {
            device_lists: Vec::new(),
            lid_mappings: Vec::new(),
        };
        let devices = client
            .try_process_group_device_list_response(
                &omitted,
                std::slice::from_ref(&user),
                generation,
            )
            .await
            .unwrap()
            .unwrap();
        assert!(devices.contains(&user.with_device(7)));
    }

    #[tokio::test]
    async fn group_refresh_removed_then_reused_device_id_is_cold() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("12025550125");
        let group = "120363000000000125@g.us";
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                timestamp: 1,
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();
        let rows = vec![
            (user.to_string(), true),
            (user.with_device(7).to_string(), true),
        ];
        client
            .persistence_manager
            .set_sender_key_status(
                group,
                &[(rows[0].0.as_str(), true), (rows[1].0.as_str(), true)],
            )
            .await
            .unwrap();
        client
            .sender_key_device_cache
            .get_or_init(group, async {
                std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(&rows))
            })
            .await;
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        let response = authenticated_response_fixture(&client, response).await;
        client
            .try_process_group_device_list_response(
                &response,
                std::slice::from_ref(&user),
                client.device_topology.current(),
            )
            .await
            .unwrap()
            .unwrap();
        crate::test_utils::signed_device_add(
            &client,
            &user.user,
            &wacore::stanza::devices::DeviceElement {
                jid: user.with_device(7),
                key_index: Some(4),
                lid: None,
            },
        )
        .await;
        let rows = client
            .persistence_manager
            .get_sender_key_devices(group)
            .await
            .unwrap();
        assert_eq!(rows, vec![(user.to_string(), true)]);
        let map = client
            .sender_key_device_cache
            .get_or_init(group, async {
                std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(&rows))
            })
            .await;
        assert_eq!(map.device_has_key(&user.user, 7), None);
        assert_eq!(map.device_has_key(&user.user, 0), Some(true));
        assert_eq!(
            client
                .load_device_record_for_jid(&user)
                .await
                .unwrap()
                .devices
                .iter()
                .find(|device| device.device_id() == 7)
                .unwrap()
                .key_index(),
            Some(4)
        );
    }

    #[tokio::test]
    async fn group_refresh_same_response_reused_id_checks_previous_key_index() {
        use crate::sender_key_device_cache::SenderKeyDeviceMap;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        for previous_index_valid in [false, true] {
            let client = create_test_client().await;
            let user = Jid::pn("12025550128");
            let group = "120363000000000128@g.us";
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                    timestamp: 1,
                    phash: None,
                    raw_id: Some(1),
                })
                .await
                .unwrap();
            let rows = vec![
                (user.to_string(), true),
                (user.with_device(7).to_string(), true),
            ];
            client
                .persistence_manager
                .set_sender_key_status(
                    group,
                    &[(rows[0].0.as_str(), true), (rows[1].0.as_str(), true)],
                )
                .await
                .unwrap();
            let warm = client
                .sender_key_device_cache
                .get_or_init(group, async {
                    std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(&rows))
                })
                .await;
            assert!(warm.device_and_primary_warm(&user.user, 7));

            let valid_indexes = if previous_index_valid {
                vec![3, 4]
            } else {
                vec![4]
            };
            let response = DeviceListResponse {
                device_lists: vec![UserDeviceList {
                    user: user.clone(),
                    devices: vec![UsyncDevice::new(0, None), UsyncDevice::new(7, Some(4))],
                    phash: None,
                    key_index_bytes: Some(signed_key_index_bytes(valid_indexes, 4)),
                }],
                lid_mappings: Vec::new(),
            };
            let response = authenticated_response_fixture(&client, response).await;
            let devices = client
                .try_process_group_device_list_response(
                    &response,
                    std::slice::from_ref(&user),
                    client.device_topology.current(),
                )
                .await
                .unwrap()
                .unwrap();
            assert!(devices.contains(&user.with_device(7)));
            let record = client.load_device_record_for_jid(&user).await.unwrap();
            assert_eq!(record.raw_id, Some(1));
            assert_eq!(
                record
                    .devices
                    .iter()
                    .find(|device| device.device_id() == 7)
                    .unwrap()
                    .key_index(),
                Some(4)
            );

            let rows = client
                .persistence_manager
                .get_sender_key_devices(group)
                .await
                .unwrap();
            assert_eq!(
                rows.iter()
                    .any(|(jid, has_key)| jid == &user.with_device(7).to_string() && *has_key),
                previous_index_valid
            );
            let map = client
                .sender_key_device_cache
                .get_or_init(group, async {
                    std::sync::Arc::new(SenderKeyDeviceMap::from_db_rows(&rows))
                })
                .await;
            assert_eq!(
                map.device_has_key(&user.user, 7),
                previous_index_valid.then_some(true)
            );
            assert_eq!(map.device_has_key(&user.user, 0), Some(true));
        }
    }

    /// A failed registry write must not leave sender-key rows and sessions
    /// deleted for a device list that never landed: the cleanup runs after
    /// the write, so a write that never happened leaves everything intact.
    #[tokio::test]
    async fn group_refresh_write_failure_keeps_tracking_and_sessions() {
        use std::sync::atomic::Ordering;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("12025550129");
        let group = "120363000000000129@g.us";
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                timestamp: 1,
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();
        let rows = [
            (user.to_string(), true),
            (user.with_device(7).to_string(), true),
        ];
        client
            .persistence_manager
            .set_sender_key_status(
                group,
                &[(rows[0].0.as_str(), true), (rows[1].0.as_str(), true)],
            )
            .await
            .unwrap();
        let session = seed_fresh_session(&client, &user.with_device(7)).await;
        // The replacement drops device 7, so the old order would delete its
        // sender-key rows and session before attempting the write.
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        let response = authenticated_response_fixture(&client, response).await;
        client
            .fail_next_device_list_write
            .store(true, Ordering::SeqCst);
        client
            .try_process_group_device_list_response(
                &response,
                std::slice::from_ref(&user),
                client.device_topology.current(),
            )
            .await
            .expect_err("the injected write failure must surface");
        let rows = client
            .persistence_manager
            .get_sender_key_devices(group)
            .await
            .unwrap();
        assert!(
            rows.iter()
                .any(|(jid, has_key)| jid == &user.with_device(7).to_string() && *has_key),
            "a write that never happened must not delete sender-key tracking: {rows:?}"
        );
        assert!(
            has_session(&client, &session).await,
            "a write that never happened must not delete sessions"
        );
        assert!(
            client
                .load_device_record_for_jid(&user)
                .await
                .unwrap()
                .devices
                .iter()
                .any(|device| device.device_id() == 7),
            "the previous device list must survive a failed write"
        );
    }

    #[tokio::test]
    async fn test_device_registry_hit_resolves_devices() {
        let client = create_test_client().await;

        let user_jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();

        // Insert a device record into the registry (simulates prior usync/notification)
        let record = DeviceListRecord {
            user: "1234567890".into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(3, Some(10))].into(),
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client.update_device_list(record).await.unwrap();

        // get_user_devices should resolve from registry without network
        let devices = client.get_user_devices(&[user_jid]).await.unwrap();
        assert_eq!(devices.len(), 2);
        assert!(devices.iter().any(|d| d.device == 0));
        assert!(devices.iter().any(|d| d.device == 3));
        assert!(devices.iter().all(|d| d.is_pn()));
    }

    #[tokio::test]
    async fn refresh_bypasses_a_warm_registry_without_clearing_it_first() {
        let client = create_test_client().await;
        let user: Jid = "12025550102@s.whatsapp.net".parse().unwrap();
        client
            .update_device_list(DeviceListRecord {
                user: "12025550102".into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(8, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();

        let cached = client
            .get_user_devices(std::slice::from_ref(&user))
            .await
            .unwrap();
        assert_eq!(
            cached.len(),
            2,
            "cache-preferred must use the warm snapshot"
        );

        let refresh = client.refresh_user_devices(vec![user.clone()]).await;
        assert!(
            refresh.is_err(),
            "the offline fixture proves refresh consulted the source"
        );

        let preserved = client
            .get_devices_from_registry(&user)
            .await
            .expect("a failed refresh must leave the previous snapshot readable");
        assert_eq!(preserved.len(), 2);
        assert!(preserved.iter().any(|device| device.device == 8));
    }

    #[tokio::test]
    async fn test_device_registry_hit_for_lid_jid() {
        let client = create_test_client().await;

        let lid_jid: Jid = "100000012345678@lid".parse().unwrap();

        let record = DeviceListRecord {
            user: "100000012345678".into(),
            devices: [DeviceInfo::new(0, None), DeviceInfo::new(39, Some(25))].into(),
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client.update_device_list(record).await.unwrap();

        let devices = client.get_user_devices(&[lid_jid]).await.unwrap();
        assert_eq!(devices.len(), 2);
        assert!(devices.iter().any(|d| d.device == 0));
        assert!(devices.iter().any(|d| d.device == 39));
        assert!(devices.iter().all(|d| d.is_lid()));
    }

    #[tokio::test]
    async fn test_device_registry_db_fallback() {
        let client = create_test_client().await;

        let user_jid: Jid = "9876543210@s.whatsapp.net".parse().unwrap();

        // Insert into backend DB via update_device_list
        let record = DeviceListRecord {
            user: "9876543210".into(),
            devices: [DeviceInfo::new(5, None)].into(),
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client.update_device_list(record).await.unwrap();

        // Evict from registry cache to force DB path
        client
            .device_registry_cache
            .raw_invalidate_for_tests("9876543210")
            .await;
        client.device_registry_cache.run_pending_tasks().await;

        // Should still resolve from DB
        let devices = client.get_user_devices(&[user_jid]).await.unwrap();
        assert_eq!(devices.len(), 1);
        assert_eq!(devices[0].device, 5);
    }

    // A present-but-empty device record is local corruption and must not be
    // treated as authoritative: get_user_devices falls through to the network so
    // the list can self-heal. Offline, that surfaces as an error, which proves the
    // fetch was attempted (the old behavior returned Ok([]) with no fetch).
    #[tokio::test]
    async fn test_empty_device_record_falls_through_to_network() {
        let client = create_test_client().await;

        let user_jid: Jid = "5551230000@s.whatsapp.net".parse().unwrap();

        let record = DeviceListRecord {
            user: "5551230000".into(),
            devices: Box::default(),
            timestamp: wacore::time::now_secs(),
            phash: None,
            raw_id: None,
        };
        client.update_device_list(record).await.unwrap();

        let result = client.get_user_devices(&[user_jid]).await;
        assert!(
            result.is_err(),
            "empty record must fall through to the network, got {result:?}"
        );
    }

    #[tokio::test]
    async fn test_cache_size_eviction() {
        use crate::cache::Cache;

        let cache: Cache<i32, String> = Cache::builder().max_capacity(2).build();

        cache.insert(1, "one".to_string()).await;
        cache.insert(2, "two".to_string()).await;
        cache.insert(3, "three".to_string()).await;

        cache.run_pending_tasks().await;

        let count = cache.entry_count();
        assert!(
            count <= 2,
            "Cache should have at most 2 items, has {}",
            count
        );
    }

    /// #3 merge-safety: when the server omits an unchanged user (the `device_hash`
    /// skip), `process_device_list_response` must update only the returned users
    /// and leave the omitted user's cached devices untouched.
    #[tokio::test]
    async fn process_response_preserves_omitted_users() {
        use wacore::iq::usync::DeviceListResponse;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;

        // Seed user B in the registry (will be the "unchanged/omitted" one).
        client
            .update_device_list(DeviceListRecord {
                user: "2222222222".into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("2:oldB".into()),
                raw_id: None,
            })
            .await
            .unwrap();

        // Response only contains user A — B is omitted (unchanged).
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: "1111111111@s.whatsapp.net".parse().unwrap(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: Some("2:a".into()),
                key_index_bytes: None,
            }],
            lid_mappings: vec![],
        };
        let response = authenticated_response_fixture(&client, response).await;

        let fetched = client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();
        assert!(
            fetched.iter().any(|j| j.user == "1111111111"),
            "returned user A must be resolved"
        );

        // B's cache is preserved (still 2 devices) — not wiped by the omission.
        let b_jid: Jid = "2222222222@s.whatsapp.net".parse().unwrap();
        let b_devices = client
            .get_devices_from_registry(&b_jid)
            .await
            .expect("omitted user B must keep its cached record");
        assert_eq!(
            b_devices.len(),
            2,
            "omitted user's devices must be preserved"
        );
    }

    #[tokio::test]
    async fn process_response_preserves_hosted_device_addressing() {
        use wacore::iq::usync::DeviceListResponse;
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("1111111111");
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![
                    UsyncDevice::new(0, None),
                    UsyncDevice::new(7, Some(3)).with_hosting(true),
                ],
                phash: Some("2:hosted".into()),
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        let response = authenticated_response_fixture(&client, response).await;

        let fetched = client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();
        assert!(
            fetched
                .iter()
                .any(|jid| jid.device == 7 && jid.server == wacore_binary::Server::Hosted)
        );

        let cached = client
            .get_devices_from_registry(&user)
            .await
            .expect("processed devices should be cached");
        assert!(
            cached
                .iter()
                .any(|jid| jid.device == 7 && jid.server == wacore_binary::Server::Hosted)
        );
    }

    #[tokio::test]
    async fn stale_refresh_does_not_overwrite_a_newer_device_notification() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("12025550102");
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("1:before".into()),
                raw_id: None,
            })
            .await
            .unwrap();

        let refresh_started_at = client.device_topology.current();
        let stale_response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(0, None)],
                phash: Some("1:stale".into()),
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };

        crate::test_utils::signed_device_add(
            &client,
            &user.user,
            &wacore::stanza::devices::DeviceElement {
                jid: user.with_device(7),
                key_index: None,
                lid: None,
            },
        )
        .await;

        let published = client
            .try_process_refreshed_device_list_response(
                &stale_response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();
        assert!(published.is_none(), "the stale response must be retried");

        let retained = client
            .get_devices_from_registry(&user)
            .await
            .expect("notification snapshot must remain available");
        assert!(retained.iter().any(|device| device.device == 7));
    }

    #[tokio::test]
    async fn stale_refresh_does_not_overwrite_a_newer_lid_mapping() {
        use wacore::usync::UsyncLidMapping;

        let client = create_test_client().await;
        let phone = "12025550110";
        let current_lid = "100000000000110";
        let stale_lid = "100000000000111";
        let refresh_started_at = client.device_topology.current();

        client
            .add_lid_pn_mapping(
                current_lid,
                phone,
                crate::lid_pn_cache::LearningSource::PeerPnMessage,
            )
            .await
            .unwrap();

        let stale_response = DeviceListResponse {
            device_lists: Vec::new(),
            lid_mappings: vec![UsyncLidMapping {
                phone_number: phone.into(),
                lid: stale_lid.into(),
            }],
        };
        let published = client
            .try_process_refreshed_device_list_response(
                &stale_response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();

        assert!(published.is_none(), "the stale response must be retried");
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(current_lid),
            "the mapping learned after the refresh started must survive"
        );
    }

    #[tokio::test]
    async fn refresh_commits_its_own_lid_mapping_after_the_cas() {
        use wacore::usync::UsyncLidMapping;

        let client = create_test_client().await;
        let phone = "12025550112";
        let lid = "100000000000112";
        let refresh_started_at = client.device_topology.current();
        let response = DeviceListResponse {
            device_lists: Vec::new(),
            lid_mappings: vec![UsyncLidMapping {
                phone_number: phone.into(),
                lid: lid.into(),
            }],
        };
        let response = authenticated_response_fixture(&client, response).await;

        let published = client
            .try_process_refreshed_device_list_response(
                &response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();

        assert!(
            published.is_some(),
            "the response must not conflict with itself"
        );
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(lid)
        );
    }

    #[tokio::test]
    async fn unrelated_registry_change_does_not_restart_a_refresh() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let refreshed = Jid::pn("12025550104");
        let refresh_started_at = client.device_topology.current();
        client
            .update_device_list(DeviceListRecord {
                user: "12025550105".into(),
                devices: [DeviceInfo::new(0, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();

        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: refreshed,
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        let response = authenticated_response_fixture(&client, response).await;
        let published = client
            .try_process_refreshed_device_list_response(
                &response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();
        assert!(published.is_some());
    }

    #[tokio::test]
    async fn unrelated_mapping_change_does_not_restart_a_refresh() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let refreshed = Jid::pn("12025550113");
        let refresh_started_at = client.device_topology.current();
        client
            .add_lid_pn_mapping(
                "100000000000114",
                "12025550114",
                crate::lid_pn_cache::LearningSource::PeerPnMessage,
            )
            .await
            .unwrap();

        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: refreshed,
                devices: vec![UsyncDevice::new(0, None)],
                phash: None,
                key_index_bytes: None,
            }],
            lid_mappings: Vec::new(),
        };
        let response = authenticated_response_fixture(&client, response).await;
        let published = client
            .try_process_refreshed_device_list_response(
                &response,
                Freshness::Refresh,
                refresh_started_at,
            )
            .await
            .unwrap();

        assert!(published.is_some());
    }

    /// A usync that returns an empty device list for a user is transient or
    /// corrupt (WA Web always keeps device 0). `process_device_list_response`
    /// must not persist it: a good cached record stays intact instead of being
    /// clobbered with an empty list that `get_user_devices` then re-fetches on
    /// every send.
    #[tokio::test]
    async fn process_response_skips_empty_device_list() {
        use wacore::usync::UserDeviceList;

        let client = create_test_client().await;

        client
            .update_device_list(DeviceListRecord {
                user: "3333333333".into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(4, None)].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("3:old".into()),
                raw_id: None,
            })
            .await
            .unwrap();

        // The same user comes back from usync with no devices.
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: "3333333333@s.whatsapp.net".parse().unwrap(),
                devices: vec![],
                phash: Some("3:empty".into()),
                key_index_bytes: None,
            }],
            lid_mappings: vec![],
        };
        let response = authenticated_response_fixture(&client, response).await;

        let fetched = client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();
        assert!(
            !fetched.iter().any(|j| j.user == "3333333333"),
            "an empty returned list contributes no devices"
        );

        // The good cached record survives — not clobbered with an empty list.
        let jid: Jid = "3333333333@s.whatsapp.net".parse().unwrap();
        let devices = client
            .get_devices_from_registry(&jid)
            .await
            .expect("the good record must survive an empty usync response");
        assert_eq!(
            devices.len(),
            2,
            "empty response must not clobber the record"
        );
    }

    #[tokio::test]
    async fn refresh_rejects_a_device_list_emptied_by_key_index_filtering() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("4444444444");
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("2:previous".into()),
                raw_id: Some(1),
            })
            .await
            .unwrap();

        // The wire response is non-empty, but its only companion is outside the
        // signed key-index set. This exercises the post-projection completeness
        // check rather than the raw USync response check.
        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(7, Some(3))],
                phash: Some("2:incomplete".into()),
                key_index_bytes: Some(signed_key_index_bytes(Vec::new(), 10)),
            }],
            lid_mappings: Vec::new(),
        };
        let response = authenticated_response_fixture(&client, response).await;

        let error = client
            .process_device_list_response(&response, Freshness::Refresh)
            .await
            .expect_err("an authoritative refresh must not return an empty projection");
        assert!(error.to_string().contains("no valid devices"));

        let preserved = client
            .get_devices_from_registry(&user)
            .await
            .expect("a rejected refresh must preserve the previous snapshot");
        assert_eq!(preserved.len(), 2);
        assert!(preserved.iter().any(|device| device.device == 7));
    }

    #[tokio::test]
    async fn rejected_refresh_defers_identity_cleanup_for_every_user() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let identity_changed = Jid::pn("4444444451");
        let invalid = Jid::pn("4444444452");

        for (user, raw_id) in [(&identity_changed, 2), (&invalid, 1)] {
            client
                .update_device_list(DeviceListRecord {
                    user: user.user.as_str().into(),
                    devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                    timestamp: wacore::time::now_secs(),
                    phash: Some("2:previous".into()),
                    raw_id: Some(raw_id),
                })
                .await
                .unwrap();
        }

        let previous_session = seed_fresh_session(&client, &identity_changed.with_device(7)).await;
        let response = DeviceListResponse {
            device_lists: vec![
                UserDeviceList {
                    user: identity_changed.clone(),
                    // A primary device survives key-index filtering, so this
                    // first user schedules a valid identity replacement.
                    devices: vec![UsyncDevice::new(0, None)],
                    phash: Some("1:changed".into()),
                    key_index_bytes: Some(signed_key_index_bytes(Vec::new(), 10)),
                },
                UserDeviceList {
                    user: invalid,
                    // The second user makes the authoritative response invalid
                    // only after key-index projection.
                    devices: vec![UsyncDevice::new(7, Some(3))],
                    phash: Some("1:invalid".into()),
                    key_index_bytes: Some(signed_key_index_bytes(Vec::new(), 10)),
                },
            ],
            lid_mappings: Vec::new(),
        };
        let response = authenticated_response_fixture(&client, response).await;

        client
            .process_device_list_response(&response, Freshness::Refresh)
            .await
            .expect_err("the second user must reject the whole authoritative refresh");

        assert!(
            has_session(&client, &previous_session).await,
            "validation failure must not partially clear an earlier user's sessions"
        );
        let preserved = client
            .get_devices_from_registry(&identity_changed)
            .await
            .expect("validation failure must preserve the earlier registry snapshot");
        assert!(preserved.iter().any(|device| device.device == 7));
    }

    #[tokio::test]
    async fn filtered_identity_change_invalidates_the_stale_registry() {
        use wacore::usync::{UserDeviceList, UsyncDevice};

        let client = create_test_client().await;
        let user = Jid::pn("4444444453");
        client
            .update_device_list(DeviceListRecord {
                user: user.user.as_str().into(),
                devices: [DeviceInfo::new(0, None), DeviceInfo::new(7, Some(3))].into(),
                timestamp: wacore::time::now_secs(),
                phash: Some("2:previous".into()),
                raw_id: Some(2),
            })
            .await
            .unwrap();
        let previous_session = seed_fresh_session(&client, &user.with_device(7)).await;

        let response = DeviceListResponse {
            device_lists: vec![UserDeviceList {
                user: user.clone(),
                devices: vec![UsyncDevice::new(7, Some(3))],
                phash: Some("1:changed".into()),
                key_index_bytes: Some(signed_key_index_bytes(Vec::new(), 10)),
            }],
            lid_mappings: Vec::new(),
        };
        let response = authenticated_response_fixture(&client, response).await;

        let fetched = client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();
        assert!(fetched.is_empty());
        assert!(
            !has_session(&client, &previous_session).await,
            "an accepted identity change must clear sessions from the old identity"
        );
        assert!(
            client.get_devices_from_registry(&user).await.is_none(),
            "without a replacement snapshot, the stale registry must be invalidated"
        );
    }

    /// The batched LID-PN learn path warms the in-memory cache SYNCHRONOUSLY
    /// (the persist runs detached), so a mapping from the usync response is
    /// resolvable the moment `process_device_list_response` returns. Locks the
    /// contract that the new path doesn't defer the cache update.
    #[tokio::test]
    async fn process_response_warms_lid_pn_cache_synchronously() {
        use wacore::usync::UsyncLidMapping;

        let client = create_test_client().await;

        // Pin that we exercise the BATCHED branch, not the per-mapping fallback:
        // the branch is `if let Some(client) = self.self_weak...upgrade()`, so a
        // live self_weak upgrade means learn_lid_pn_mappings_batch is the path
        // taken. (Both paths warm the cache, so without this the test could pass
        // via the fallback.)
        assert!(
            client.self_weak.get().and_then(|w| w.upgrade()).is_some(),
            "fixture must populate self_weak so the batched learner is exercised"
        );

        let response = DeviceListResponse {
            device_lists: vec![],
            lid_mappings: vec![UsyncLidMapping {
                phone_number: "559980000123".into(),
                lid: "100000000000123".into(),
            }],
        };
        let response = authenticated_response_fixture(&client, response).await;

        assert!(
            client
                .lid_pn_cache
                .get_current_lid("559980000123")
                .await
                .is_none()
        );

        client
            .process_device_list_response(&response, Freshness::CachePreferred)
            .await
            .unwrap();

        assert_eq!(
            client
                .lid_pn_cache
                .get_current_lid("559980000123")
                .await
                .as_deref(),
            Some("100000000000123"),
            "usync LID mapping must be in the cache synchronously after the call"
        );
    }
}
