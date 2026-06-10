//! LID-PN (Linked ID to Phone Number) mapping methods for Client.
//!
//! This module contains methods for managing the bidirectional mapping
//! between LIDs (Linked IDs) and phone numbers.
//!
//! Key features:
//! - Cache warm-up from persistent storage
//! - Adding new LID-PN mappings with automatic migration
//! - Resolving JIDs to their LID equivalents
//! - Bidirectional lookup (LID to PN and PN to LID)

use std::sync::Arc;

use anyhow::Result;
use log::debug;
use wacore::store::traits::LidPnMappingEntry;
use wacore_binary::Jid;

use super::Client;
use crate::lid_pn_cache::{LearningSource, LidPnEntry};

/// Exclusive upper bound for the device-id range we iterate when migrating
/// PN→LID. WhatsApp's protocol caps companion devices well below this, but
/// the conservative bound covers paired devices learned via offline syncs
/// without unbounded looping.
const MIGRATION_DEVICE_RANGE: u16 = 100;

/// Backend `LidPnMappingEntry` → in-memory `LidPnEntry`.
fn mapping_to_entry(m: LidPnMappingEntry) -> LidPnEntry {
    LidPnEntry::with_timestamp(
        m.lid,
        m.phone_number,
        m.created_at,
        LearningSource::parse(&m.learning_source),
    )
}

impl Client {
    /// Warm up the LID-PN cache from persistent storage.
    /// This is called during client initialization to populate the in-memory cache
    /// with previously learned LID-PN mappings.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.warm_up_lid_pn_cache",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn warm_up_lid_pn_cache(&self) -> Result<(), anyhow::Error> {
        let backend = self.persistence_manager.backend();
        let entries = backend.get_all_lid_mappings().await?;

        if entries.is_empty() {
            debug!("LID-PN cache warm-up: no entries found in storage");
            return Ok(());
        }

        self.lid_pn_cache
            .warm_up(entries.into_iter().map(mapping_to_entry))
            .await;
        Ok(())
    }

    /// Awaits the persist + any device/session migrations. Hot paths should
    /// prefer [`learn_lid_pn_mapping_fast`].
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.add_lid_pn_mapping",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn add_lid_pn_mapping(
        &self,
        lid: &str,
        phone_number: &str,
        source: LearningSource,
    ) -> Result<()> {
        let (entry, is_new_mapping) = self
            .record_lid_pn_in_memory(lid, phone_number, source)
            .await;
        self.persist_and_migrate_lid_pn(entry, is_new_mapping).await
    }

    /// Hot-path variant: cache is updated synchronously (so a subsequent
    /// `resolve_encryption_jid` sees the mapping), DB write + migrations run
    /// in a detached task. Matches WA Web's `warmUpLidPnMapping` + the
    /// deferred `lidPnCacheDirtySet` flush in `WAWebDBCreateLidPnMappings`.
    ///
    /// `is_offline` mirrors WA Web's `flushImmediately = msgInfo.offline == null`:
    /// offline replays only warm the in-memory cache, so a burst of queued
    /// messages on reconnect doesn't fan out one persist task per message.
    /// Offline mappings are re-learned from the next live message or usync.
    ///
    /// Durability: if the spawned persist task fails (DB error, shutdown
    /// mid-write), the mapping is only in-memory and will be lost on restart.
    /// Use [`add_lid_pn_mapping`] when the caller needs a durable guarantee.
    ///
    /// Concurrent calls for the same phone number may both observe
    /// `is_new_mapping = true` and each spawn a persist task. The downstream
    /// work tolerates this:
    /// - `put_lid_mapping` is an upsert
    /// - `migrate_device_registry_on_lid_discovery` no-ops after the PN-keyed
    ///   record is gone
    /// - `migrate_signal_sessions_on_lid_discovery` no-ops after the sessions
    ///   are migrated
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.learn_lid_pn_fast", level = "trace", skip_all, fields(is_offline = is_offline)))]
    pub(crate) async fn learn_lid_pn_mapping_fast(
        self: &Arc<Self>,
        lid: &str,
        phone_number: &str,
        source: LearningSource,
        is_offline: bool,
    ) {
        // Skip the per-message re-record/re-persist only when this exact pair is
        // durably persisted and resolvable both ways in cache: an offline-only
        // learn, a remap (even one whose write failed), or an evicted entry all
        // fall through and re-warm/persist. `source` drift is ignored.
        if self.lid_pn_cache.can_skip_relearn(phone_number, lid).await {
            return;
        }
        let (entry, is_new_mapping) = self
            .record_lid_pn_in_memory(lid, phone_number, source)
            .await;
        if is_offline {
            return;
        }
        let client = Arc::clone(self);
        self.runtime
            .spawn(Box::pin(async move {
                if let Err(err) = client
                    .persist_and_migrate_lid_pn(entry, is_new_mapping)
                    .await
                {
                    log::warn!("Background LID-PN persist failed: {err}");
                }
            }))
            .detach();
    }

    /// Batched variant of [`learn_lid_pn_mapping_fast`]. Updates the in-memory
    /// cache synchronously for every entry, then fires one detached task that
    /// persists the whole batch in a single backend transaction and runs the
    /// device/session migrations for newly discovered PN↔LID pairs.
    ///
    /// Mirrors WA Web's `createLidPnMappings({ mappings, flushImmediately, learningSource })`
    /// call shape: one backend write for N participants instead of N detached
    /// tasks racing each other. The savings are linear in batch size and
    /// matter most on first `query_info` of large groups.
    ///
    /// `is_offline` mirrors the single-entry path: skip the persist task for
    /// offline replays; mappings are re-learned from the next live event.
    ///
    /// Takes owned `(lid, phone_number)` pairs; each `String` moves directly
    /// into the `LidPnEntry` stored in the cache, then (via `into_iter`) into
    /// the `LidPnMappingEntry` that's persisted — no clones on either step.
    /// The `Vec` itself is consumed, so no copy of the outer container either.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.learn_lid_pn_batch", level = "debug", skip_all, fields(count = mappings.len(), is_offline = is_offline)))]
    pub(crate) async fn learn_lid_pn_mappings_batch(
        self: &Arc<Self>,
        mappings: Vec<(String, String)>,
        source: LearningSource,
        is_offline: bool,
    ) {
        if mappings.is_empty() {
            return;
        }
        // Dedup by phone_number, last lid wins. Otherwise the same phone
        // appearing twice in one batch yields is_new=true for the first
        // (lid_A) and is_new=false for the second (lid_B), so signal
        // migration runs for lid_A while the persisted mapping ends up
        // pointing at lid_B — migration done against the wrong LID.
        let cap = mappings.len();
        let mut deduped: std::collections::HashMap<String, String> =
            std::collections::HashMap::with_capacity(cap);
        for (lid, phone_number) in mappings {
            deduped.insert(phone_number, lid);
        }

        let mut entries: Vec<LidPnEntry> = Vec::with_capacity(deduped.len());
        let mut is_new_flags: Vec<bool> = Vec::with_capacity(deduped.len());
        for (phone_number, lid) in deduped {
            // Same fast path as the single-entry learn: an already durable and
            // both-ways cached pair needs no re-add, re-persist or migration.
            if self
                .lid_pn_cache
                .can_skip_relearn(&phone_number, &lid)
                .await
            {
                continue;
            }
            let is_new = self
                .lid_pn_cache
                .get_current_lid(&phone_number)
                .await
                .is_none();
            let entry = LidPnEntry::new(lid, phone_number, source);
            self.lid_pn_cache.add(&entry).await;
            entries.push(entry);
            is_new_flags.push(is_new);
        }

        // Every pair was already durable; nothing to persist or migrate.
        if is_offline || entries.is_empty() {
            return;
        }

        let client = Arc::clone(self);
        self.runtime
            .spawn(Box::pin(async move {
                if let Err(err) = client
                    .persist_and_migrate_lid_pn_batch(entries, is_new_flags)
                    .await
                {
                    log::warn!("Background LID-PN batch persist failed: {err}");
                }
            }))
            .detach();
    }

    async fn record_lid_pn_in_memory(
        &self,
        lid: &str,
        phone_number: &str,
        source: LearningSource,
    ) -> (LidPnEntry, bool) {
        let is_new_mapping = self
            .lid_pn_cache
            .get_current_lid(phone_number)
            .await
            .is_none();
        let entry = LidPnEntry::new(lid.to_string(), phone_number.to_string(), source);
        self.lid_pn_cache.add(&entry).await;
        (entry, is_new_mapping)
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.persist_migrate_lid_pn", level = "debug", skip_all, fields(is_new = is_new_mapping), err(Debug)))]
    async fn persist_and_migrate_lid_pn(
        &self,
        entry: LidPnEntry,
        is_new_mapping: bool,
    ) -> Result<()> {
        use anyhow::anyhow;

        let storage_entry = LidPnMappingEntry {
            lid: entry.lid.to_string(),
            phone_number: entry.phone_number.to_string(),
            created_at: entry.created_at,
            updated_at: entry.created_at,
            learning_source: entry.learning_source.as_str().to_string(),
        };

        self.persistence_manager
            .backend()
            .put_lid_mapping(&storage_entry)
            .await
            .map_err(|e| anyhow!("persisting LID-PN mapping: {e}"))?;

        // After the write, not before: a failed persist stays un-marked so the
        // next live message retries instead of skipping.
        self.lid_pn_cache
            .mark_persisted(&storage_entry.phone_number, &storage_entry.lid)
            .await;

        if is_new_mapping {
            self.migrate_device_registry_on_lid_discovery(
                &storage_entry.phone_number,
                &storage_entry.lid,
            )
            .await;
            self.migrate_signal_sessions_on_lid_discovery(
                &storage_entry.phone_number,
                &storage_entry.lid,
            )
            .await;
        }

        Ok(())
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.persist_migrate_lid_pn_batch", level = "debug", skip_all, fields(count = entries.len()), err(Debug)))]
    async fn persist_and_migrate_lid_pn_batch(
        &self,
        entries: Vec<LidPnEntry>,
        is_new_flags: Vec<bool>,
    ) -> Result<()> {
        use anyhow::anyhow;

        // Consume entries so `lid`/`phone_number` move into storage rather
        // than being cloned. Only `learning_source` is allocated, and only
        // because `LidPnMappingEntry.learning_source` is a `String` field.
        let storage: Vec<LidPnMappingEntry> = entries
            .into_iter()
            .map(|entry| LidPnMappingEntry {
                lid: entry.lid.to_string(),
                phone_number: entry.phone_number.to_string(),
                created_at: entry.created_at,
                updated_at: entry.created_at,
                learning_source: entry.learning_source.as_str().to_string(),
            })
            .collect();

        self.persistence_manager
            .backend()
            .put_lid_mappings(&storage)
            .await
            .map_err(|e| anyhow!("persisting LID-PN mapping batch: {e}"))?;

        for (entry, is_new) in storage.iter().zip(is_new_flags.iter()) {
            self.lid_pn_cache
                .mark_persisted(&entry.phone_number, &entry.lid)
                .await;
            if *is_new {
                self.migrate_device_registry_on_lid_discovery(&entry.phone_number, &entry.lid)
                    .await;
                self.migrate_signal_sessions_on_lid_discovery(&entry.phone_number, &entry.lid)
                    .await;
            }
        }

        Ok(())
    }

    /// Ensure phone-to-LID mappings are resolved for the given JIDs.
    /// Matches WhatsApp Web's WAWebManagePhoneNumberMappingJob.ensurePhoneNumberToLidMapping().
    /// Should be called before establishing new E2E sessions to avoid duplicate sessions.
    ///
    /// This checks the local cache for existing mappings. For JIDs without cached mappings,
    /// the caller should consider fetching them via usync query if establishing sessions.
    pub(crate) async fn resolve_lid_mappings(&self, jids: &[Jid]) -> Vec<Jid> {
        let mut resolved = Vec::with_capacity(jids.len());

        for jid in jids {
            // Only resolve for user JIDs (not groups, status, etc.)
            if !jid.is_pn() && !jid.is_lid() {
                resolved.push(jid.clone());
                continue;
            }

            // If it's already a LID, use as-is
            if jid.is_lid() {
                resolved.push(jid.clone());
                continue;
            }

            // Try to resolve PN to LID from cache
            if let Some(lid_user) = self.lid_pn_cache.get_current_lid(&jid.user).await {
                resolved.push(Jid::lid_device(lid_user, jid.device));
            } else {
                // No cached mapping — use original JID. Mapping will be learned
                // organically from incoming messages or usync responses.
                resolved.push(jid.clone());
            }
        }

        resolved
    }

    /// Mirrors WA Web `SignalAddress.toString()` (`WAWeb/Signal/Address.js`):
    /// upgrade Pn → Lid and Hosted → HostedLid when a mapping is known, else
    /// preserve the input.
    pub(crate) async fn resolve_encryption_jid(&self, target: &Jid) -> Jid {
        use wacore_binary::Server;
        let lid_server = match target.server {
            Server::Pn => Server::Lid,
            Server::Hosted => Server::HostedLid,
            _ => return target.clone(),
        };
        match self.lid_pn_cache.get_current_lid(&target.user).await {
            Some(lid_user) => Jid {
                user: lid_user,
                server: lid_server,
                device: target.device,
                agent: target.agent,
                integrator: target.integrator,
            },
            None => target.clone(),
        }
    }

    /// Swap a JID's namespace between PN and LID, preserving device/agent/integrator.
    /// Returns `None` if no mapping exists or the JID is neither PN nor LID.
    pub(crate) async fn swap_pn_lid_namespace(&self, jid: &Jid) -> Option<Jid> {
        if jid.is_lid() {
            let pn_user = self.lid_pn_cache.get_phone_number(&jid.user).await?;
            Some(Jid {
                user: pn_user.into(),
                server: wacore_binary::Server::Pn,
                device: jid.device,
                agent: jid.agent,
                integrator: jid.integrator,
            })
        } else if jid.is_pn() {
            let lid_user = self.lid_pn_cache.get_current_lid(&jid.user).await?;
            Some(Jid {
                user: lid_user,
                server: wacore_binary::Server::Lid,
                device: jid.device,
                agent: jid.agent,
                integrator: jid.integrator,
            })
        } else {
            None
        }
    }

    /// Migrate Signal sessions and identity keys from PN to LID address.
    ///
    /// All reads/writes go through `signal_cache` to avoid reading stale data
    /// from the backend when the cache has unflushed mutations (e.g., after
    /// SKDM encryption ratcheted the session).
    /// Read-modify-write of PN and LID Signal session/identity slots must
    /// hold the same per-address locks that encrypt/decrypt take, otherwise
    /// concurrent message_encrypt on LID can clobber the migrated session.
    ///
    /// Callers must NOT hold `session_lock_for(<lid_addr>)` for any device
    /// in [0, 100) — `async_lock::Mutex` is not reentrant. The decrypt path
    /// drops its address lock around the call (`try_pn_to_lid_migration_decrypt`).
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.migrate_signal_sessions",
            level = "debug",
            skip_all
        )
    )]
    pub(crate) async fn migrate_signal_sessions_on_lid_discovery(&self, pn: &str, lid: &str) {
        use log::{info, warn};
        use wacore::types::jid::JidExt;

        let backend = self.persistence_manager.backend();

        // Nothing to migrate unless the PN side has Signal state. For a freshly
        // resolved peer (e.g. every member of a large group on first send) this
        // skips MIGRATION_DEVICE_RANGE lock+lookup iterations that would all
        // find nothing. On a lookup error, fall through to the full scan.
        if let Ok(false) = self
            .signal_cache
            .has_state_for_user(pn, backend.as_ref())
            .await
        {
            return;
        }

        for device_id in 0..MIGRATION_DEVICE_RANGE {
            // `&str` → `CompactString` is inline for ≤24-byte user parts
            // (all PN/LID identifiers fit), so no String intermediate.
            let pn_jid = Jid::pn_device(pn, device_id);
            let lid_jid = Jid::lid_device(lid, device_id);

            let pn_proto = pn_jid.to_protocol_address();
            let lid_proto = lid_jid.to_protocol_address();

            // Acquire both per-address locks in stable lexicographic order to
            // avoid deadlock against concurrent paths that legitimately hold
            // only one side. (Callers never hold either lock.)
            let pn_lock = self.session_lock_for(pn_proto.as_str()).await;
            let lid_lock = self.session_lock_for(lid_proto.as_str()).await;
            let (_first_guard, _second_guard) = if pn_proto.as_str() <= lid_proto.as_str() {
                let pn_g = pn_lock.lock_arc().await;
                let lid_g = lid_lock.lock_arc().await;
                (pn_g, lid_g)
            } else {
                let lid_g = lid_lock.lock_arc().await;
                let pn_g = pn_lock.lock_arc().await;
                (lid_g, pn_g)
            };

            // PN wins on conflict — mirrors whatsmeow's `MigratePNToLID`
            // (`ON CONFLICT DO UPDATE SET session=excluded.session`).
            if let Ok(Some(session)) = self
                .signal_cache
                .get_session(&pn_proto, backend.as_ref())
                .await
            {
                self.signal_cache.put_session(&lid_proto, session).await;
                self.signal_cache.delete_session(&pn_proto).await;
                info!(
                    "Migrated session {} -> {} (PN wins on conflict)",
                    pn_proto, lid_proto
                );
            }

            // Identity uses LID-wins (the inverse of session). For the same
            // physical device the identity_key is stable across PN/LID, so
            // either policy yields the same bytes in the steady state. The
            // asymmetry only matters if the peer re-paired between our PN
            // and LID identity captures — in that case the fresher LID
            // identity is on the namespace we're migrating *to*, and PN's
            // stale value should not clobber it.
            //
            // Match the LID lookup result explicitly so a transient read
            // failure isn't collapsed with `Ok(None)` and used as license
            // to overwrite a potentially-valid LID identity.
            if let Ok(Some(identity_data)) = self
                .signal_cache
                .get_identity(&pn_proto, backend.as_ref())
                .await
            {
                match self
                    .signal_cache
                    .get_identity(&lid_proto, backend.as_ref())
                    .await
                {
                    Ok(None) => {
                        self.signal_cache
                            .put_identity(&lid_proto, &identity_data)
                            .await;
                        self.signal_cache.delete_identity(&pn_proto).await;
                        info!("Migrated identity {} -> {}", pn_proto, lid_proto);
                    }
                    Ok(Some(_)) => {
                        // LID-wins: existing LID identity preserved; drop the PN copy.
                        self.signal_cache.delete_identity(&pn_proto).await;
                    }
                    Err(e) => {
                        warn!(
                            "Skipping identity migration {} -> {}: \
                             failed to read LID identity: {e:?}",
                            pn_proto, lid_proto
                        );
                    }
                }
            }
        }

        // Flush migrated state to backend so it survives restarts
        if let Err(e) = self.signal_cache.flush(backend.as_ref()).await {
            warn!("Failed to flush signal cache after migration: {e:?}");
        }
    }

    /// Look up the LID↔phone mapping for a JID. Cache-aside: falls back to
    /// the backend on cache miss so mappings survive cache eviction and any
    /// backend implementation gets the fallback without warm-up.
    ///
    /// Backend errors are propagated — callers can distinguish "no mapping"
    /// (`Ok(None)`) from "lookup failed" (`Err(_)`).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.get_lid_pn_entry", level = "trace", skip_all, fields(peer = %jid.observe()), err(Debug)))]
    pub async fn get_lid_pn_entry(&self, jid: &Jid) -> Result<Option<LidPnEntry>> {
        let (hit, is_lid) = if jid.is_lid() {
            (self.lid_pn_cache.get_entry_by_lid(&jid.user).await, true)
        } else if jid.is_pn() {
            (self.lid_pn_cache.get_entry_by_phone(&jid.user).await, false)
        } else {
            return Ok(None);
        };

        if let Some(entry) = hit {
            return Ok(Some(entry));
        }

        let backend = self.persistence_manager.backend();
        let mapping = if is_lid {
            backend.get_lid_mapping(&jid.user).await?
        } else {
            backend.get_pn_mapping(&jid.user).await?
        };

        let Some(mapping) = mapping else {
            return Ok(None);
        };

        let entry = mapping_to_entry(mapping);
        self.lid_pn_cache.add(&entry).await;
        Ok(Some(entry))
    }

    /// Resolve any user JID to its bare LID form, or `None` when no LID is
    /// available. Mirrors WA Web's `WAWebLidMigrationUtils.toUserLid`: LID
    /// passes through, PN goes through the cache-aside mapping, anything
    /// else and any lookup failure returns `None`.
    ///
    /// Used by `send_status_message` to replicate WA Web's
    /// `compactMap(list, toUserLid)` skip-on-unresolvable semantics.
    pub(crate) async fn resolve_recipient_to_lid(&self, jid: &Jid) -> Option<Jid> {
        if jid.is_lid() {
            return Some(jid.to_non_ad());
        }
        if !jid.is_pn() {
            return None;
        }
        match self.get_lid_pn_entry(jid).await {
            Ok(Some(entry)) => Some(Jid::new(&*entry.lid, wacore_binary::Server::Lid)),
            Ok(None) => None,
            Err(e) => {
                log::warn!(
                    "resolve_recipient_to_lid: LID lookup for {} failed: {:?}",
                    jid.observe(),
                    e
                );
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lid_pn_cache::LearningSource;
    use crate::test_utils::create_test_client;
    use std::sync::Arc;
    use wacore_binary::Server;

    #[tokio::test]
    async fn test_resolve_encryption_jid_pn_to_lid() {
        let client: Arc<Client> = create_test_client().await;
        let pn = "55999999999";
        let lid = "100000012345678";

        // Add mapping to cache
        client
            .add_lid_pn_mapping(lid, pn, LearningSource::PeerPnMessage)
            .await
            .unwrap();

        let pn_jid = Jid::pn(pn);
        let resolved = client.resolve_encryption_jid(&pn_jid).await;

        assert_eq!(resolved.user, lid);
        assert_eq!(resolved.server, Server::Lid);
    }

    #[tokio::test]
    async fn test_resolve_encryption_jid_preserves_lid() {
        let client: Arc<Client> = create_test_client().await;
        let lid = "100000012345678";
        let lid_jid = Jid::lid(lid);

        let resolved = client.resolve_encryption_jid(&lid_jid).await;

        assert_eq!(resolved, lid_jid);
    }

    #[tokio::test]
    async fn test_resolve_encryption_jid_no_mapping_returns_pn() {
        let client: Arc<Client> = create_test_client().await;
        let pn = "55999999999";
        let pn_jid = Jid::pn(pn);

        let resolved = client.resolve_encryption_jid(&pn_jid).await;

        assert_eq!(resolved, pn_jid);
    }

    #[tokio::test]
    async fn test_resolve_encryption_jid_hosted_with_lid_upgrades_to_hosted_lid() {
        let client: Arc<Client> = create_test_client().await;
        let user = "55999999999";
        let lid = "100000012345678";

        client
            .add_lid_pn_mapping(lid, user, LearningSource::PeerPnMessage)
            .await
            .unwrap();

        for device in [99u16, 7] {
            let mut hosted = Jid::new(user, Server::Hosted);
            hosted.device = device;
            hosted.agent = 0xAB;
            hosted.integrator = 0xBEEF;
            let resolved = client.resolve_encryption_jid(&hosted).await;

            assert_eq!(resolved.user, lid);
            assert_eq!(resolved.server, Server::HostedLid);
            assert_eq!(
                resolved.device, device,
                "device must round-trip, not be coerced to 99"
            );
            assert_eq!(resolved.agent, hosted.agent);
            assert_eq!(resolved.integrator, hosted.integrator);
        }
    }

    #[tokio::test]
    async fn test_resolve_encryption_jid_hosted_no_mapping_keeps_hosted() {
        let client: Arc<Client> = create_test_client().await;
        let mut hosted = Jid::new("55999999999", Server::Hosted);
        hosted.device = 99;

        let resolved = client.resolve_encryption_jid(&hosted).await;

        assert_eq!(resolved, hosted);
    }

    #[tokio::test]
    async fn test_resolve_encryption_jid_preserves_hosted_lid() {
        let client: Arc<Client> = create_test_client().await;
        let mut hosted_lid = Jid::new("100000012345678", Server::HostedLid);
        hosted_lid.device = 99;

        let resolved = client.resolve_encryption_jid(&hosted_lid).await;

        assert_eq!(resolved, hosted_lid);
    }

    #[tokio::test]
    async fn test_get_lid_pn_entry_from_pn() {
        let client: Arc<Client> = create_test_client().await;
        let pn = "55999999999";
        let lid = "100000012345678";

        assert!(
            client
                .get_lid_pn_entry(&Jid::pn(pn))
                .await
                .unwrap()
                .is_none()
        );

        client
            .add_lid_pn_mapping(lid, pn, LearningSource::Usync)
            .await
            .unwrap();

        let entry = client
            .get_lid_pn_entry(&Jid::pn(pn))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&*entry.lid, lid);
        assert_eq!(&*entry.phone_number, pn);
    }

    #[tokio::test]
    async fn test_get_lid_pn_entry_from_lid() {
        let client: Arc<Client> = create_test_client().await;
        let pn = "55999999999";
        let lid = "100000012345678";

        assert!(
            client
                .get_lid_pn_entry(&Jid::lid(lid))
                .await
                .unwrap()
                .is_none()
        );

        client
            .add_lid_pn_mapping(lid, pn, LearningSource::Usync)
            .await
            .unwrap();

        let entry = client
            .get_lid_pn_entry(&Jid::lid(lid))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&*entry.lid, lid);
        assert_eq!(&*entry.phone_number, pn);
    }

    /// Cache-aside fallback: if the in-memory cache is missing an entry the
    /// backend has, the lookup should still succeed and re-populate the cache.
    #[tokio::test]
    async fn test_get_lid_pn_entry_falls_back_to_backend() {
        use wacore::store::traits::LidPnMappingEntry;

        let client: Arc<Client> = create_test_client().await;
        let pn = "15555550123";
        let lid = "100000000000123";

        let backend = client.persistence_manager.backend();
        backend
            .put_lid_mapping(&LidPnMappingEntry {
                lid: lid.into(),
                phone_number: pn.into(),
                created_at: 1,
                updated_at: 1,
                learning_source: "usync".into(),
            })
            .await
            .unwrap();

        // Cache was never warmed from this backend write → cache miss path.
        let entry = client
            .get_lid_pn_entry(&Jid::lid(lid))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&*entry.lid, lid);
        assert_eq!(&*entry.phone_number, pn);

        // Subsequent lookup served from cache.
        let entry = client
            .get_lid_pn_entry(&Jid::pn(pn))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(&*entry.lid, lid);
    }

    /// `learn_lid_pn_mapping_fast` must leave the in-memory cache populated
    /// by the time it returns — `resolve_encryption_jid` runs immediately
    /// after on the decrypt hot path and needs to find the LID.
    #[tokio::test]
    async fn test_learn_lid_pn_mapping_fast_populates_cache_synchronously() {
        let client: Arc<Client> = create_test_client().await;
        let pn = "5511999998877";
        let lid = "200000000007788";

        client
            .learn_lid_pn_mapping_fast(lid, pn, LearningSource::PeerPnMessage, false)
            .await;

        let resolved = client.resolve_encryption_jid(&Jid::pn(pn)).await;
        assert_eq!(resolved.user, lid, "cache must have the mapping on return");
        assert_eq!(resolved.server, Server::Lid);
    }

    /// A mapping first warmed memory-only by an offline replay must still be
    /// persisted on its first live message; the fast-path skip must not swallow
    /// it just because the cache already holds it.
    #[tokio::test]
    async fn learn_fast_offline_then_live_persists() {
        let client: Arc<Client> = create_test_client().await;
        let lid = "200000000012345";
        let pn = "5511988887777";
        let backend = client.persistence_manager.backend();

        client
            .learn_lid_pn_mapping_fast(lid, pn, LearningSource::PeerPnMessage, true)
            .await;
        assert_eq!(client.resolve_encryption_jid(&Jid::pn(pn)).await.user, lid);
        assert!(
            backend.get_lid_mapping(lid).await.unwrap().is_none(),
            "offline learn must not persist"
        );

        client
            .learn_lid_pn_mapping_fast(lid, pn, LearningSource::PeerPnMessage, false)
            .await;
        // Poll until persisted; tolerate the transient SQLite read/write lock
        // while the detached persist task is mid-write.
        let start = wacore::time::Instant::now();
        while !matches!(backend.get_lid_mapping(lid).await, Ok(Some(_))) {
            assert!(
                start.elapsed() < std::time::Duration::from_secs(5),
                "live learn after an offline-only learn must persist"
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }

    /// Batched variant must populate the in-memory cache synchronously for
    /// every entry before returning; WA Web parity for `createLidPnMappings`.
    #[tokio::test]
    async fn test_learn_lid_pn_mappings_batch_populates_cache_synchronously() {
        let client: Arc<Client> = create_test_client().await;
        let pairs = [
            ("200000000000001", "5511911111111"),
            ("200000000000002", "5511922222222"),
            ("200000000000003", "5511933333333"),
        ];

        let batch: Vec<(String, String)> = pairs
            .iter()
            .map(|(lid, pn)| ((*lid).to_string(), (*pn).to_string()))
            .collect();
        client
            .learn_lid_pn_mappings_batch(batch, LearningSource::Other, false)
            .await;

        for (lid, pn) in &pairs {
            let resolved = client.resolve_encryption_jid(&Jid::pn(*pn)).await;
            assert_eq!(resolved.user, *lid, "batch entry {pn} missing from cache");
            assert_eq!(resolved.server, Server::Lid);
        }
    }

    /// Empty batch is a no-op (no detached task, no panic).
    #[tokio::test]
    async fn test_learn_lid_pn_mappings_batch_empty_is_noop() {
        let client: Arc<Client> = create_test_client().await;
        client
            .learn_lid_pn_mappings_batch(Vec::new(), LearningSource::Other, false)
            .await;
        assert_eq!(client.lid_pn_cache.lid_count().await, 0);
    }

    /// Online (`is_offline = false`) batch must persist the mapping to the
    /// backend AND run `migrate_device_registry_on_lid_discovery` for each
    /// newly learned PN. Polls until the detached task completes.
    #[tokio::test]
    async fn test_learn_lid_pn_mappings_batch_online_persists_and_migrates() {
        use wacore::store::traits::{DeviceInfo, DeviceListRecord};
        use wacore_binary::Jid;

        let client: Arc<Client> = create_test_client().await;
        let lid = "200000000077777";
        let pn = "5511955550000";
        let backend = client.persistence_manager.backend();

        // Seed a PN-keyed device registry row so the migration has something
        // to move when the mapping is learned. Without this, the migration
        // helper is a no-op and the test can't distinguish "migration ran"
        // from "migration never called".
        backend
            .update_device_list(DeviceListRecord {
                user: pn.to_string(),
                devices: vec![DeviceInfo {
                    device_id: 3,
                    key_index: None,
                }],
                timestamp: wacore::time::now_secs(),
                phash: None,
                raw_id: None,
            })
            .await
            .unwrap();

        client
            .learn_lid_pn_mappings_batch(
                vec![(lid.to_string(), pn.to_string())],
                LearningSource::Other,
                false,
            )
            .await;

        // Poll for the end-of-chain migration effect (device row moved to
        // LID key). That strictly happens after both `put_lid_mappings` and
        // `migrate_device_registry_on_lid_discovery`, so observing it
        // guarantees both steps ran.
        let start = wacore::time::Instant::now();
        let deadline = std::time::Duration::from_secs(5);
        loop {
            if backend.get_devices(lid).await.unwrap().is_some() {
                break;
            }
            assert!(
                start.elapsed() < deadline,
                "timed out waiting for batch persist + migration"
            );
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }

        assert!(
            backend.get_lid_mapping(lid).await.unwrap().is_some(),
            "mapping must be persisted"
        );
        assert!(
            backend.get_devices(pn).await.unwrap().is_none(),
            "migration must delete the old PN-keyed device row"
        );
        let lid_row = backend.get_devices(lid).await.unwrap().unwrap();
        assert_eq!(lid_row.devices[0].device_id, 3);
        // And the mapping resolves from both directions.
        assert_eq!(
            client
                .get_lid_pn_entry(&Jid::pn(pn))
                .await
                .unwrap()
                .unwrap()
                .lid,
            lid.into()
        );
    }

    /// Offline batch only warms the in-memory cache; the persist task never
    /// fires. Mirrors WA Web's `flushImmediately = false` semantics.
    #[tokio::test]
    async fn test_learn_lid_pn_mappings_batch_offline_skips_persist() {
        use wacore_binary::Jid;

        let client: Arc<Client> = create_test_client().await;
        let lid = "200000000009999";
        let pn = "5511900009999";

        client
            .learn_lid_pn_mappings_batch(
                vec![(lid.to_string(), pn.to_string())],
                LearningSource::Other,
                true,
            )
            .await;

        let resolved = client.resolve_encryption_jid(&Jid::pn(pn)).await;
        assert_eq!(resolved.user, lid);

        assert!(
            client
                .persistence_manager
                .backend()
                .get_lid_mapping(lid)
                .await
                .unwrap()
                .is_none(),
            "offline batch must not persist to DB"
        );
    }

    /// Duplicate phone_numbers in a single batch must collapse to one
    /// (lid, phone) → migration entry, and that entry must use the FINAL
    /// lid for the phone. Otherwise migration runs against the stale lid
    /// while the persisted mapping resolves to the fresh one.
    #[tokio::test]
    async fn test_learn_lid_pn_mappings_batch_dedups_duplicate_phones() {
        use wacore_binary::Jid;

        let client: Arc<Client> = create_test_client().await;
        let pn = "5511900000007";
        let lid_stale = "200000000007777";
        let lid_fresh = "200000000007999";

        client
            .learn_lid_pn_mappings_batch(
                vec![
                    (lid_stale.to_string(), pn.to_string()),
                    (lid_fresh.to_string(), pn.to_string()),
                ],
                LearningSource::Other,
                true, // offline → no spawned persist, no migration races
            )
            .await;

        // Final cache state must reflect the LAST mapping for this phone.
        let resolved = client.resolve_encryption_jid(&Jid::pn(pn)).await;
        assert_eq!(
            resolved.user, lid_fresh,
            "dedup must keep the last lid for a repeated phone_number"
        );
    }

    /// Produce a SessionRecord blob with a distinctive remote_registration_id
    /// so we can tell which side of a migration won by parsing the surviving
    /// session, not by raw-byte comparison.
    fn tagged_session_blob(remote_regid: u32) -> Vec<u8> {
        use wacore::libsignal::protocol::{SessionRecord, SessionState};
        use waproto::whatsapp::SessionStructure;

        let state = SessionState::from_session_structure(SessionStructure {
            session_version: Some(3),
            local_identity_public: None,
            remote_identity_public: None,
            root_key: None,
            previous_counter: Some(0),
            sender_chain: None,
            receiver_chains: vec![],
            pending_pre_key: None,
            remote_registration_id: Some(remote_regid),
            local_registration_id: Some(0),
            alice_base_key: Some(vec![]),
            needs_refresh: None,
            pending_key_exchange: None,
        });
        SessionRecord::new(state)
            .serialize()
            .expect("serialize session record")
    }

    /// Both PN and LID slots hold a session for the same peer; the
    /// PN one is the working Double Ratchet state, the LID one was
    /// built freshly by `process_prekey_bundle` and has no link to
    /// the peer's outbound chain. Migration must keep the PN blob —
    /// silently dropping it leaves the linked device pinned to the
    /// fresh stub forever. Reg-id tags identify which side won.
    #[tokio::test]
    async fn migration_preserves_working_session_when_both_namespaces_present() {
        use wacore::libsignal::protocol::SessionRecord;
        use wacore::types::jid::JidExt as _;

        let client: Arc<Client> = create_test_client().await;
        let pn = "5500000000000";
        let lid = "111111111111111";

        client
            .add_lid_pn_mapping(lid, pn, LearningSource::PeerPnMessage)
            .await
            .unwrap();

        let pn_addr = Jid::pn_device(pn.to_string(), 0).to_protocol_address();
        let lid_addr = Jid::lid_device(lid.to_string(), 0).to_protocol_address();

        // The working session — what Bob's outbound chain is actually
        // ratcheted against — lives in the PN slot. Tag it with a
        // distinctive registration id so post-migration we can prove
        // the surviving session is the SAME blob.
        const WORKING_REGID: u32 = 0xDEAD_BEEF;
        const FRESH_REGID: u32 = 0x0BAD_F00D;

        let backend = client.persistence_manager.backend();

        // Seed both slots through signal_cache so the cache holds Present
        // entries when migrate runs. Raw backend writes alone leave the
        // cache cold and migrate's `get_session` then races with whatever
        // populated Absent markers for unknown peers during test bring-up.
        client
            .signal_cache
            .put_session(
                &pn_addr,
                SessionRecord::deserialize(&tagged_session_blob(WORKING_REGID))
                    .expect("seed PN blob deserializes"),
            )
            .await;
        client
            .signal_cache
            .put_session(
                &lid_addr,
                SessionRecord::deserialize(&tagged_session_blob(FRESH_REGID))
                    .expect("seed LID blob deserializes"),
            )
            .await;
        client.signal_cache.flush(backend.as_ref()).await.unwrap();

        client
            .migrate_signal_sessions_on_lid_discovery(pn, lid)
            .await;

        // PN must be drained — future loads route to LID once the
        // mapping is known.
        assert!(
            backend
                .get_session(pn_addr.as_str())
                .await
                .unwrap()
                .is_none(),
            "PN address must be cleared post-migration"
        );

        let surviving_bytes = backend
            .get_session(lid_addr.as_str())
            .await
            .unwrap()
            .expect("LID slot must have a session after migration");
        let record = SessionRecord::deserialize(&surviving_bytes)
            .expect("surviving session blob must parse");
        let surviving_regid = record
            .remote_registration_id()
            .expect("surviving session must expose its remote reg id");

        assert_eq!(
            surviving_regid, WORKING_REGID,
            "LID slot held the FRESH (regid={:#x}) blob — that's the prod \
             deadlock: the working PN session ({:#x}) got discarded by the \
             'both exist' branch, leaving us pinned to a session that has no \
             link to the peer's outbound chain.",
            surviving_regid, WORKING_REGID
        );
    }

    /// A freshly-resolved peer (no prior PN Signal state) must short-circuit the
    /// per-device migration scan: nothing to move, so no LID session appears and
    /// the MIGRATION_DEVICE_RANGE lock/lookup loop is skipped.
    #[tokio::test]
    async fn migrate_skips_when_no_pn_signal_state() {
        use wacore::types::jid::JidExt as _;

        let client: Arc<Client> = create_test_client().await;
        let pn = "5500000000777";
        let lid = "222222222222222";
        client
            .add_lid_pn_mapping(lid, pn, LearningSource::PeerPnMessage)
            .await
            .unwrap();
        let backend = client.persistence_manager.backend();

        // Fresh peer: no PN session or identity anywhere, so the guard skips.
        assert!(
            !client
                .signal_cache
                .has_state_for_user(pn, backend.as_ref())
                .await
                .unwrap(),
            "fresh peer should have no PN Signal state"
        );

        client
            .migrate_signal_sessions_on_lid_discovery(pn, lid)
            .await;

        // No LID session was materialized (nothing was migrated).
        let lid_addr = Jid::lid_device(lid.to_string(), 0).to_protocol_address();
        assert!(
            client
                .signal_cache
                .get_session(&lid_addr, backend.as_ref())
                .await
                .unwrap()
                .is_none(),
            "migration of a stateless peer must not create a LID session"
        );
    }

    /// Migration must hold the same per-address session locks that
    /// encrypt/decrypt take. Otherwise a concurrent `message_encrypt`
    /// on the LID slot can clobber the just-migrated session (or read
    /// mid-update state). Externally hold the LID lock, kick off
    /// migration, and assert it blocks until the lock is released.
    #[tokio::test]
    async fn migration_blocks_on_per_address_session_lock() {
        use std::time::Duration;
        use wacore::types::jid::JidExt as _;

        let client: Arc<Client> = create_test_client().await;
        let pn = "5500000000000";
        let lid = "111111111111111";
        client
            .add_lid_pn_mapping(lid, pn, LearningSource::PeerPnMessage)
            .await
            .unwrap();

        // Seed a PN session so the migration actually enters its per-device
        // loop. The existence guard skips when there is nothing to migrate, and
        // this test is about the lock the loop takes when migrating real state.
        let pn_addr = Jid::pn_device(pn.to_string(), 0).to_protocol_address();
        client
            .signal_cache
            .put_session(
                &pn_addr,
                wacore::libsignal::protocol::SessionRecord::deserialize(&tagged_session_blob(
                    0xDEAD_BEEF,
                ))
                .expect("seed PN blob deserializes"),
            )
            .await;

        let lid_addr = Jid::lid_device(lid.to_string(), 0).to_protocol_address();
        let lid_lock = client.session_lock_for(lid_addr.as_str()).await;
        let held = lid_lock.lock().await;

        let migrate_client = client.clone();
        let pn_s = pn.to_string();
        let lid_s = lid.to_string();
        let mut handle = tokio::spawn(async move {
            migrate_client
                .migrate_signal_sessions_on_lid_discovery(&pn_s, &lid_s)
                .await;
        });

        let blocked = tokio::time::timeout(Duration::from_millis(200), &mut handle).await;
        assert!(
            blocked.is_err(),
            "migration must block while another holder owns the LID address \
             session lock — otherwise concurrent encrypt/decrypt races"
        );

        // Release the lock; migration should now complete so the spawned task
        // doesn't outlive the test (and contaminate parallel test state).
        drop(held);
        tokio::time::timeout(Duration::from_secs(5), handle)
            .await
            .expect("migration must complete once the lock is released")
            .expect("migration task must not panic");
    }

    /// Regression guard for the decrypt-path deadlock: `decrypt_message`
    /// holds `session_lock_for(<lid_addr>)` while invoking
    /// `try_pn_to_lid_migration_decrypt`, whose migration loop re-enters
    /// that same mutex. The fix is to drop the guard around the call.
    /// This test exercises the exact drop → migrate → reacquire dance the
    /// production code does, asserting it never deadlocks.
    #[tokio::test]
    async fn migration_lock_dance_completes_when_caller_drops_guard() {
        use std::time::Duration;
        use wacore::types::jid::JidExt as _;

        let client: Arc<Client> = create_test_client().await;
        let pn = "5500000000000";
        let lid = "111111111111111";
        client
            .add_lid_pn_mapping(lid, pn, LearningSource::PeerPnMessage)
            .await
            .unwrap();

        let lid_addr = Jid::lid_device(lid.to_string(), 0).to_protocol_address();
        let session_mutex = client.session_lock_for(lid_addr.as_str()).await;
        let mut session_guard: Option<async_lock::MutexGuardArc<()>> =
            Some(session_mutex.lock_arc().await);

        // Exactly mirrors try_pn_to_lid_migration_decrypt: drop, migrate,
        // reacquire. If the migration's per-device lock loop ever re-enters
        // a held guard, this hangs and the timeout fires.
        let dance = async {
            session_guard = None;
            client
                .migrate_signal_sessions_on_lid_discovery(pn, lid)
                .await;
            session_guard = Some(session_mutex.lock_arc().await);
        };
        tokio::time::timeout(Duration::from_secs(5), dance)
            .await
            .expect("drop → migrate → reacquire must not deadlock");

        assert!(
            session_guard.is_some(),
            "guard must be re-held after the dance so the next batch payload \
             stays serialized on the address lock"
        );
    }
}
