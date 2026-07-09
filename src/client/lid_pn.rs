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
use wacore::iq::usync::LidQuerySpec;
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

/// Per-mapping write policy, mirroring WhatsApp Web's `createLidPnMappings`
/// `switch (learningSource)` (`WAWebDBCreateLidPnMappings`). The learning
/// source is not mere provenance: it decides whether an incoming pair may
/// overwrite what the cache already holds.
///
/// Inputs (WA Web `c`/`y`/`C`):
/// - `lid_unseen`: the LID has no phone number cached yet (`c`).
/// - `exact`: the phone already resolves to this exact LID (`y`).
/// - derived `lid_known_mismatch = !lid_unseen && !exact` (`C`): the LID is
///   already known but the phone currently resolves elsewhere.
///
/// Returns `(write, needs_usync)`:
/// - `write` (WA Web `v`): update the cache/DB with this pair.
/// - `needs_usync` (WA Web `b`): a conservative source hit a conflicting LID;
///   re-resolve the phone authoritatively via a live LID query instead of
///   trusting the observational pair. Only ever set when `write` is false.
fn lid_pn_write_policy(source: LearningSource, lid_unseen: bool, exact: bool) -> (bool, bool) {
    let lid_known_mismatch = !lid_unseen && !exact;
    match source {
        // Device-list usync: authoritative for a new LID and for correcting a
        // known LID whose phone drifted.
        LearningSource::Usync => (lid_unseen || lid_known_mismatch, false),
        // Directed sources: overwrite on any difference from what's cached.
        LearningSource::PeerPnMessage
        | LearningSource::PeerLidMessage
        | LearningSource::RecipientLatestLid
        | LearningSource::MigrationSyncLatest
        | LearningSource::MigrationSyncOld
        | LearningSource::BlocklistActive
        | LearningSource::BlocklistInactive => (!exact, false),
        // Observational bulk sources (WA Web `default`, i.e. `learningSource:
        // "other"`): only seed genuinely new LIDs; on a conflict with an
        // already-known LID, don't clobber — request a live re-resolve. WA Web
        // tags history sync, group/participant seeds, device- and
        // contact-notifications, status/voip, etc. all as "other".
        LearningSource::Other | LearningSource::Pairing | LearningSource::DeviceNotification => {
            (lid_unseen, lid_known_mismatch)
        }
    }
}

/// WA Web `S`: sources carrying known-stale data get `created_at = 0` so any
/// later live mapping for the same phone outranks them in the cache's
/// most-recent-wins (PN→LID) resolution. Only the forward direction is
/// timestamp-ordered; the LID→PN reverse map always takes the latest write (as
/// in WA Web), so this does not guard the reverse lookup.
fn is_stale_source(source: LearningSource) -> bool {
    matches!(
        source,
        LearningSource::MigrationSyncOld | LearningSource::BlocklistInactive
    )
}

/// Outcome of recording one (lid, phone) pair against current cache state.
enum RecordOutcome {
    /// Already durable in both directions; nothing to do.
    Skipped,
    /// Written to (or re-affirmed in) the cache; the caller should persist it.
    /// `is_new` drives the PN→LID device/session migration.
    Written { entry: LidPnEntry, is_new: bool },
    /// An observational source conflicted with a known LID; the phone should be
    /// re-resolved via a live LID query rather than trusting this pair.
    NeedsUsync,
}

/// Outcome of recording a batch: entries to persist (with their `is_new`
/// flags) plus phones that need a live LID re-query.
struct BatchRecordOutcome {
    entries: Vec<LidPnEntry>,
    is_new_flags: Vec<bool>,
    usync_phones: Vec<String>,
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
    ///
    /// Public so embedders can feed in pairs the library never observes
    /// itself — e.g. app-state `ContactAction` mutations, which carry
    /// `lidJid`/`pnJid` for the user's address-book contacts — instead of
    /// writing the backend mapping table behind the cache's back.
    ///
    /// `lid` and `phone_number` are bare user parts (no `@lid` /
    /// `@s.whatsapp.net` server, no device suffix). Pick the
    /// [`LearningSource`] that matches where the pair came from;
    /// [`LearningSource::Other`] covers sources without a dedicated variant.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.add_lid_pn_mapping",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub async fn add_lid_pn_mapping(
        &self,
        lid: &str,
        phone_number: &str,
        source: LearningSource,
    ) -> Result<()> {
        match self
            .record_lid_pn_in_memory(lid, phone_number, source)
            .await
        {
            RecordOutcome::Skipped => Ok(()),
            RecordOutcome::NeedsUsync => {
                self.spawn_lid_usync_reconcile(vec![phone_number.to_string()]);
                Ok(())
            }
            RecordOutcome::Written { entry, is_new } => {
                self.persist_and_migrate_lid_pn(entry, is_new).await
            }
        }
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
        let (entry, is_new_mapping) = match self
            .record_lid_pn_in_memory(lid, phone_number, source)
            .await
        {
            RecordOutcome::Skipped => return,
            RecordOutcome::NeedsUsync => {
                self.spawn_lid_usync_reconcile(vec![phone_number.to_string()]);
                return;
            }
            RecordOutcome::Written { entry, is_new } => (entry, is_new),
        };
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
        let BatchRecordOutcome {
            entries,
            is_new_flags,
            usync_phones,
        } = self.record_lid_pn_batch_in_memory(mappings, source).await;

        // Conflicting observational pairs re-resolve live, independent of the
        // flush gate (WA Web fires syncContactListJob regardless of
        // flushImmediately).
        self.spawn_lid_usync_reconcile(usync_phones);

        // Nothing written, or an offline replay: skip the persist/migrate task.
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

    /// Fire-and-forget the WA Web `syncContactListJob({mode:"query"})` analog:
    /// one background LID usync for phones an observational source found in
    /// conflict with a known LID, learning the authoritative result under
    /// `LearningSource::Usync` (which cannot itself trigger another reconcile,
    /// so there is no query→learn→query loop). Best-effort: a failed query
    /// leaves the existing mapping untouched.
    fn spawn_lid_usync_reconcile(&self, phones: Vec<String>) {
        if phones.is_empty() {
            return;
        }
        let Some(client) = self.self_weak.get().and_then(|w| w.upgrade()) else {
            return;
        };
        let runtime = client.runtime.clone();
        runtime
            .spawn(Box::pin(async move {
                client.reconcile_lid_mappings_via_usync(phones).await;
            }))
            .detach();
    }

    async fn reconcile_lid_mappings_via_usync(&self, phones: Vec<String>) {
        let jids: Vec<Jid> = phones.iter().map(|p| Jid::pn(p.as_str())).collect();
        let sid = self.generate_request_id();
        match self.execute(LidQuerySpec::new(jids, sid)).await {
            Ok(resp) => {
                for mapping in &resp.lid_mappings {
                    if let Err(err) = self
                        .add_lid_pn_mapping(
                            &mapping.lid,
                            &mapping.phone_number,
                            LearningSource::Usync,
                        )
                        .await
                    {
                        log::warn!(
                            "LID reconcile persist failed for {} -> {}: {err}",
                            mapping.phone_number,
                            mapping.lid
                        );
                    }
                }
            }
            Err(err) => debug!("LID reconcile usync query failed: {err}"),
        }
    }

    /// Batch cache warm-up shared by the fire-and-forget learn path and the
    /// migration-sync handler (which awaits persistence instead). Each pair
    /// runs through [`Self::record_lid_pn_in_memory`] under the source's write
    /// policy. Dedups by phone_number (last lid wins) — otherwise the same
    /// phone appearing twice in one batch yields is_new=true for the first
    /// (lid_A) and is_new=false for the second (lid_B), so signal migration
    /// runs for lid_A while the persisted mapping ends up pointing at lid_B.
    /// (WA Web instead records the superseded entry with created_at=0; dropping
    /// it is equivalent for the resolved PN→LID mapping.)
    async fn record_lid_pn_batch_in_memory(
        &self,
        mappings: Vec<(String, String)>,
        source: LearningSource,
    ) -> BatchRecordOutcome {
        let cap = mappings.len();
        let mut deduped: std::collections::HashMap<String, String> =
            std::collections::HashMap::with_capacity(cap);
        for (lid, phone_number) in mappings {
            deduped.insert(phone_number, lid);
        }

        let mut entries: Vec<LidPnEntry> = Vec::with_capacity(deduped.len());
        let mut is_new_flags: Vec<bool> = Vec::with_capacity(deduped.len());
        let mut usync_phones: Vec<String> = Vec::new();
        for (phone_number, lid) in deduped {
            match self
                .record_lid_pn_in_memory(&lid, &phone_number, source)
                .await
            {
                RecordOutcome::Skipped => {}
                RecordOutcome::Written { entry, is_new } => {
                    entries.push(entry);
                    is_new_flags.push(is_new);
                }
                RecordOutcome::NeedsUsync => usync_phones.push(phone_number),
            }
        }
        BatchRecordOutcome {
            entries,
            is_new_flags,
            usync_phones,
        }
    }

    /// Record one pair in the in-memory cache under [`lid_pn_write_policy`].
    /// Does not persist — the caller drives persistence/migration from the
    /// returned [`RecordOutcome`].
    async fn record_lid_pn_in_memory(
        &self,
        lid: &str,
        phone_number: &str,
        source: LearningSource,
    ) -> RecordOutcome {
        // Fully durable and resolvable both ways: nothing to re-add or persist.
        if self.lid_pn_cache.can_skip_relearn(phone_number, lid).await {
            return RecordOutcome::Skipped;
        }

        let current_lid = self.lid_pn_cache.get_current_lid(phone_number).await;
        let reverse_pn = self.lid_pn_cache.get_phone_number(lid).await;
        let exact = current_lid.as_deref() == Some(lid);

        // Re-warm/re-affirm durability for a pair that is already the cached
        // mapping (exact, or reverse-only after a bounded-cache PN eviction).
        // Precedes the write/conflict branches so a self-consistent pair is
        // neither re-migrated as a fresh directed write nor re-queried as a
        // conflict; no migration runs, the PN↔LID association is unchanged.
        let same_pair_forward_evicted =
            current_lid.is_none() && reverse_pn.as_deref() == Some(phone_number);
        if exact || same_pair_forward_evicted {
            let existing = match self.lid_pn_cache.get_entry_by_phone(phone_number).await {
                Some(entry) => Some(entry),
                None => self.lid_pn_cache.get_entry_by_lid(lid).await,
            };
            return match existing {
                Some(entry) => {
                    self.lid_pn_cache.add(&entry).await;
                    RecordOutcome::Written {
                        entry,
                        is_new: false,
                    }
                }
                None => RecordOutcome::Skipped,
            };
        }

        // Not a self-consistent pair: apply the source's write policy.
        let lid_unseen = reverse_pn.is_none();
        let (write, needs_usync) = lid_pn_write_policy(source, lid_unseen, exact);

        if write {
            let created_at = if is_stale_source(source) {
                0
            } else {
                wacore::time::now_secs()
            };
            let entry = LidPnEntry::with_timestamp(lid, phone_number, created_at, source);
            self.lid_pn_cache.add(&entry).await;
            return RecordOutcome::Written {
                entry,
                is_new: current_lid.is_none(),
            };
        }

        // A genuine observational conflict with a different known LID: leave the
        // live mapping in place and request an authoritative live re-resolve.
        if needs_usync {
            return RecordOutcome::NeedsUsync;
        }
        RecordOutcome::Skipped
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
        let storage = self.persist_lid_pn_batch(entries).await?;
        self.migrate_lid_pn_batch(storage, is_new_flags).await;
        Ok(())
    }

    /// Durable half of the batch learn: one backend transaction plus the
    /// cache persisted-markers, no migrations.
    async fn persist_lid_pn_batch(
        &self,
        entries: Vec<LidPnEntry>,
    ) -> Result<Vec<LidPnMappingEntry>> {
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

        for entry in &storage {
            self.lid_pn_cache
                .mark_persisted(&entry.phone_number, &entry.lid)
                .await;
        }
        Ok(storage)
    }

    /// Registry + Signal-session migrations for a persisted batch. Split from
    /// the persist so callers on the message pipeline can await durability but
    /// defer this part — each new mapping walks up to MIGRATION_DEVICE_RANGE
    /// per-address locks, which must not stall the global processing permit.
    async fn migrate_lid_pn_batch(&self, storage: Vec<LidPnMappingEntry>, is_new_flags: Vec<bool>) {
        for (entry, is_new) in storage.iter().zip(is_new_flags.iter()) {
            if *is_new {
                self.migrate_device_registry_on_lid_discovery(&entry.phone_number, &entry.lid)
                    .await;
                self.migrate_signal_sessions_on_lid_discovery(&entry.phone_number, &entry.lid)
                    .await;
            }
        }
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
    ///
    /// Session-layer addressing only: outbound DM wire jids must go through
    /// [`Self::resolve_dm_wire_jid`] instead, which gates the namespace on
    /// the account's 1:1-LID-migration state (an unmigrated account's LID
    /// stanza is 400-nacked by the server).
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

    /// Mirrors WA Web `Lid1X1MigrationUtils.isLidMigrated()`: the pairing- or
    /// migration-persisted account flag, with the `lid_one_on_one_migration_enabled`
    /// ab prop covering sessions paired before the flag existed (the prop is
    /// what lets WA Web start the 1:1 migration on an already-linked client).
    pub async fn is_lid_migrated(&self) -> bool {
        if self.persistence_manager.get_device_snapshot().lid_migrated {
            return true;
        }
        self.ab_props()
            .is_enabled(wacore::iq::abprops::web::LID_ONE_ON_ONE_MIGRATION_ENABLED)
            .await
    }

    /// One-way latch run after every props fetch: the props cache is not
    /// persisted, so without this a prop-only-migrated account re-enters PN
    /// wire addressing on every process start until the fetch lands, flapping
    /// the DM namespace. Persisting the observation makes the state durable,
    /// like WA Web's pref outliving the prop.
    pub(crate) async fn latch_lid_migrated_from_props(&self) {
        if !self.persistence_manager.get_device_snapshot().lid_migrated
            && self
                .ab_props()
                .is_enabled(wacore::iq::abprops::web::LID_ONE_ON_ONE_MIGRATION_ENABLED)
                .await
        {
            log::info!("Account is 1:1-LID-migrated (ab prop observation)");
            self.persistence_manager
                .process_command(crate::store::commands::DeviceCommand::SetLidMigrated(true))
                .await;
        }
    }

    /// Wire namespace for a 1:1 recipient. WAWebSendMsgCreateFanoutStanza
    /// addresses the whole DM stanza from the chat wid, which is LID only once
    /// the account is 1:1-LID-migrated (WAWebMessageDestinationChat); an
    /// unmigrated account keeps 1:1 chats on PN even with a known mapping.
    /// Signal session addressing is NOT gated by this — WAWebSignalAddress
    /// upgrades PN to LID unconditionally.
    ///
    /// Known limit: a LID input with no cached PN mapping stays LID even on
    /// an unmigrated account (and may 400). There is no reverse LID-to-PN
    /// resolution to fall back on — WA Web has none either; its unmigrated
    /// accounts simply never hold LID 1:1 chats.
    pub(crate) async fn resolve_dm_wire_jid(&self, to: &Jid) -> Jid {
        if self.is_lid_migrated().await {
            return self.resolve_encryption_jid(to).await.into_non_ad();
        }
        let bare = to.to_non_ad();
        if bare.is_lid() {
            self.swap_pn_lid_namespace(&bare).await.unwrap_or(bare)
        } else {
            bare
        }
    }

    /// Handle the primary's 1:1 LID-migration mapping push (WA Web
    /// HandleMsgProcess -> `setLidMigrationMappings`). Learns the PN-LID pairs
    /// and, once the migration ab prop allows it (WA Web's state machine only
    /// migrates past WAITING_PROP with the prop on), persists the account as
    /// migrated so DMs switch to LID wire addressing.
    pub(crate) async fn handle_lid_migration_mapping_sync(
        self: &Arc<Self>,
        sync: &waproto::whatsapp::LIDMigrationMappingSyncMessage,
    ) {
        use buffa::Message as _;

        let Some(payload_bytes) = sync.encoded_mapping_payload.as_deref() else {
            log::warn!("lid_migration_mapping_sync without payload");
            return;
        };
        let payload = match waproto::whatsapp::LIDMigrationMappingSyncPayload::decode_from_slice(
            payload_bytes,
        ) {
            Ok(p) => p,
            Err(e) => {
                log::warn!("Failed to decode LID migration mapping payload: {e}");
                return;
            }
        };

        let mappings: Vec<(String, String)> = payload
            .pn_to_lid_mappings
            .iter()
            .filter_map(|mapping| {
                // Absent (or explicit-zero) scalar fields decode as 0; a "0"
                // user would poison the cache, and a zero latest_lid falls
                // back to the required assigned_lid instead of dropping the
                // whole mapping.
                let lid = mapping
                    .latest_lid
                    .filter(|&l| l != 0)
                    .unwrap_or(mapping.assigned_lid);
                if mapping.pn == 0 || lid == 0 {
                    log::warn!("Skipping migration mapping with zero pn/lid");
                    return None;
                }
                Some((lid.to_string(), mapping.pn.to_string()))
            })
            .collect();
        // The persist is awaited (unlike the fire-and-forget learn path) so
        // the mappings are durable before the migrated flag below is; a crash
        // in between must not leave a migrated account without its mapping
        // rows. The per-mapping registry/session migrations are deferred to a
        // detached task instead: this handler runs under the message
        // pipeline's processing permit, and a large first push walking
        // MIGRATION_DEVICE_RANGE locks per mapping would stall it.
        let BatchRecordOutcome {
            entries,
            is_new_flags,
            usync_phones,
        } = self
            .record_lid_pn_batch_in_memory(
                mappings,
                crate::lid_pn_cache::LearningSource::MigrationSyncLatest,
            )
            .await;
        self.spawn_lid_usync_reconcile(usync_phones);
        if !entries.is_empty() {
            match self.persist_lid_pn_batch(entries).await {
                Ok(storage) => {
                    // A shutdown can drop this task with the migrations unrun;
                    // that is accepted, not retried: both halves self-heal
                    // lazily (decrypt-side session migration via
                    // try_pn_to_lid_migration_decrypt, and a LID registry miss
                    // just re-warms over the network on the next send).
                    let client = Arc::clone(self);
                    self.runtime
                        .spawn(Box::pin(async move {
                            client.migrate_lid_pn_batch(storage, is_new_flags).await;
                        }))
                        .detach();
                }
                Err(e) => {
                    // Do not advance migration state on a failed save (WA
                    // Web's setLidMigrationMappings rethrows); the primary's
                    // push is gone, but the ab prop keeps addressing correct
                    // until re-pair.
                    log::warn!("Failed to persist migration mappings: {e:?}");
                    return;
                }
            }
        }

        if !self.persistence_manager.get_device_snapshot().lid_migrated
            && self
                .ab_props()
                .is_enabled(wacore::iq::abprops::web::LID_ONE_ON_ONE_MIGRATION_ENABLED)
                .await
        {
            log::info!("Account is 1:1-LID-migrated (primary mapping sync)");
            self.persistence_manager
                .process_command(crate::store::commands::DeviceCommand::SetLidMigrated(true))
                .await;
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
    ///
    /// Returns whether anything moved into a LID slot. When `false`, decrypt
    /// state is unchanged, so a failed decrypt retried after this call is
    /// guaranteed to fail identically and callers can skip the retry.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.migrate_signal_sessions",
            level = "debug",
            skip_all
        )
    )]
    pub(crate) async fn migrate_signal_sessions_on_lid_discovery(
        &self,
        pn: &str,
        lid: &str,
    ) -> bool {
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
            return false;
        }

        let mut migrated = false;

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
                migrated = true;
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
                        migrated = true;
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
        migrated
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

    /// Fixture: test client with one cached peer LID-PN mapping.
    async fn client_with_peer_mapping() -> (Arc<Client>, &'static str, &'static str) {
        let client = create_test_client().await;
        let pn = "5511987650001";
        let lid = "111000011112222";
        client
            .add_lid_pn_mapping(lid, pn, LearningSource::PeerPnMessage)
            .await
            .unwrap();
        (client, pn, lid)
    }

    // ── WA Web `createLidPnMappings` switch(learningSource) parity ─────────

    /// Every non-default `LearningSource`, for the pure decision table below.
    const ALL_SOURCES: [LearningSource; 11] = [
        LearningSource::Usync,
        LearningSource::PeerPnMessage,
        LearningSource::PeerLidMessage,
        LearningSource::RecipientLatestLid,
        LearningSource::MigrationSyncLatest,
        LearningSource::MigrationSyncOld,
        LearningSource::BlocklistActive,
        LearningSource::BlocklistInactive,
        LearningSource::Pairing,
        LearningSource::DeviceNotification,
        LearningSource::Other,
    ];

    /// Pure decision table for [`lid_pn_write_policy`], mirroring WA Web's
    /// `createLidPnMappings` `switch (learningSource)`. Columns are
    /// `(lid_unseen, exact)`; `exact` implies the LID is already seen, so
    /// `(true, true)` is unreachable.
    #[test]
    fn test_lid_pn_write_policy_switch_matrix() {
        // Brand-new LID: every source writes it, none needs a re-query.
        for src in ALL_SOURCES {
            assert_eq!(
                lid_pn_write_policy(src, true, false),
                (true, false),
                "a brand-new LID must always be written, never re-queried ({src:?})"
            );
        }

        // Exact match already cached: no source rewrites, none re-queries.
        for src in ALL_SOURCES {
            assert_eq!(
                lid_pn_write_policy(src, false, true),
                (false, false),
                "an exact match is a no-op ({src:?})"
            );
        }

        // Known-LID conflict (WA Web `C`): directed sources overwrite;
        // observational sources refuse and request a live re-resolve.
        let directed = [
            LearningSource::Usync,
            LearningSource::PeerPnMessage,
            LearningSource::PeerLidMessage,
            LearningSource::RecipientLatestLid,
            LearningSource::MigrationSyncLatest,
            LearningSource::MigrationSyncOld,
            LearningSource::BlocklistActive,
            LearningSource::BlocklistInactive,
        ];
        for src in directed {
            assert_eq!(
                lid_pn_write_policy(src, false, false),
                (true, false),
                "a directed source overwrites a conflicting known LID ({src:?})"
            );
        }
        for src in [
            LearningSource::Other,
            LearningSource::Pairing,
            LearningSource::DeviceNotification,
        ] {
            assert_eq!(
                lid_pn_write_policy(src, false, false),
                (false, true),
                "an observational source must not clobber; it re-queries ({src:?})"
            );
        }
    }

    #[test]
    fn test_is_stale_source() {
        assert!(is_stale_source(LearningSource::MigrationSyncOld));
        assert!(is_stale_source(LearningSource::BlocklistInactive));
        // Every other source must be non-stale. Iterates ALL_SOURCES, which
        // `all_sources_is_exhaustive` keeps in step with the enum.
        for src in ALL_SOURCES {
            if matches!(
                src,
                LearningSource::MigrationSyncOld | LearningSource::BlocklistInactive
            ) {
                continue;
            }
            assert!(!is_stale_source(src), "{src:?} is not stale");
        }
    }

    /// Keeps `ALL_SOURCES` (a hand-written array the policy tests iterate) in
    /// sync with the enum: the wildcard-free match below fails to compile when
    /// a `LearningSource` variant is added, and its arm count is asserted equal
    /// to `ALL_SOURCES.len()`, so a new variant must be appended to both.
    #[test]
    fn all_sources_is_exhaustive() {
        fn arm_count(s: LearningSource) -> usize {
            // Wildcard-free on purpose — a new variant breaks compilation here.
            match s {
                LearningSource::Usync
                | LearningSource::PeerPnMessage
                | LearningSource::PeerLidMessage
                | LearningSource::RecipientLatestLid
                | LearningSource::MigrationSyncLatest
                | LearningSource::MigrationSyncOld
                | LearningSource::BlocklistActive
                | LearningSource::BlocklistInactive
                | LearningSource::Pairing
                | LearningSource::DeviceNotification
                | LearningSource::Other => 11,
            }
        }
        assert_eq!(
            ALL_SOURCES.len(),
            arm_count(LearningSource::Other),
            "add the new LearningSource variant to ALL_SOURCES (and the match above)"
        );
        // Reject duplicates: a repeated entry could pad the length back to the
        // arm count while a variant is silently missing.
        for (idx, src) in ALL_SOURCES.iter().enumerate() {
            assert!(
                !ALL_SOURCES[..idx].contains(src),
                "ALL_SOURCES contains duplicate {src:?}"
            );
        }
    }

    /// An observational source (`Other`, e.g. the history-sync seed) must not
    /// overwrite a live-learned LID for the same phone; it returns
    /// `NeedsUsync` and leaves the cache untouched.
    #[tokio::test]
    async fn test_record_observational_preserves_conflicting_known_lid() {
        let client = create_test_client().await;
        let phone = "5511900000001";
        let lid_live = "200000000000001";
        let lid_other = "200000000000002";
        client
            .add_lid_pn_mapping(lid_live, phone, LearningSource::Usync)
            .await
            .unwrap();
        // Make lid_other a *known* LID (mapped to some other phone).
        client
            .add_lid_pn_mapping(lid_other, "5511900000099", LearningSource::Usync)
            .await
            .unwrap();

        let outcome = client
            .record_lid_pn_in_memory(lid_other, phone, LearningSource::Other)
            .await;

        assert!(
            matches!(outcome, RecordOutcome::NeedsUsync),
            "observational conflict must request a usync, not clobber"
        );
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(lid_live),
            "the live mapping must survive an observational conflict"
        );
    }

    /// A directed source (`PeerPnMessage`) does overwrite a conflicting known
    /// LID — the WA Web `!y` branch.
    #[tokio::test]
    async fn test_record_directed_overwrites_conflicting_known_lid() {
        let client = create_test_client().await;
        let phone = "5511900000010";
        let lid_old = "200000000000010";
        let lid_new = "200000000000020";
        client
            .add_lid_pn_mapping(lid_old, phone, LearningSource::Usync)
            .await
            .unwrap();
        client
            .add_lid_pn_mapping(lid_new, "5511900000098", LearningSource::Usync)
            .await
            .unwrap();

        let outcome = client
            .record_lid_pn_in_memory(lid_new, phone, LearningSource::PeerPnMessage)
            .await;

        assert!(matches!(
            outcome,
            RecordOutcome::Written { is_new: false, .. }
        ));
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(lid_new),
            "a directed source must overwrite a conflicting known LID"
        );
    }

    /// Even an observational source seeds a *brand-new* LID for an existing
    /// phone (WA Web `c` is true), overwriting the prior mapping.
    #[tokio::test]
    async fn test_record_observational_seeds_new_lid_over_existing() {
        let client = create_test_client().await;
        let phone = "5511900000030";
        let lid_old = "200000000000030";
        let lid_brand_new = "200000000000031";
        client
            .add_lid_pn_mapping(lid_old, phone, LearningSource::Usync)
            .await
            .unwrap();

        let outcome = client
            .record_lid_pn_in_memory(lid_brand_new, phone, LearningSource::Other)
            .await;

        assert!(matches!(outcome, RecordOutcome::Written { .. }));
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(lid_brand_new),
            "a brand-new LID is seeded even by an observational source"
        );
    }

    /// An exact re-learn of a not-yet-durable pair re-affirms durability
    /// (WA Web dirty-set flush analog) without a migration.
    #[tokio::test]
    async fn test_record_exact_match_heals_without_migration() {
        let client = create_test_client().await;
        let phone = "5511900000040";
        let lid = "200000000000040";
        // Cache-only seed (never persisted), so can_skip_relearn stays false.
        let _ = client
            .record_lid_pn_in_memory(lid, phone, LearningSource::Other)
            .await;

        let outcome = client
            .record_lid_pn_in_memory(lid, phone, LearningSource::Other)
            .await;

        assert!(
            matches!(outcome, RecordOutcome::Written { is_new: false, .. }),
            "an exact re-learn re-affirms durability, no migration"
        );
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(lid)
        );
    }

    /// A stale source (`MigrationSyncOld`) writes, but with `created_at = 0`,
    /// so the cache's most-recent-wins keeps a fresher mapping for the phone.
    #[tokio::test]
    async fn test_stale_source_does_not_outrank_fresh_mapping() {
        let client = create_test_client().await;
        let phone = "5511900000050";
        let lid_fresh = "200000000000050";
        let lid_stale = "200000000000051";
        client
            .add_lid_pn_mapping(lid_fresh, phone, LearningSource::Usync)
            .await
            .unwrap();

        let outcome = client
            .record_lid_pn_in_memory(lid_stale, phone, LearningSource::MigrationSyncOld)
            .await;

        assert!(matches!(outcome, RecordOutcome::Written { .. }));
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(lid_fresh),
            "a created_at=0 stale mapping must not outrank a fresh one"
        );
        // The reverse LID→phone direction is still recorded.
        assert_eq!(
            client
                .lid_pn_cache
                .get_phone_number(lid_stale)
                .await
                .as_deref(),
            Some(phone)
        );
    }

    /// A stale source still seeds into an empty cache (no fresher mapping to
    /// lose to).
    #[tokio::test]
    async fn test_stale_source_seeds_empty_cache() {
        let client = create_test_client().await;
        let phone = "5511900000060";
        let lid = "200000000000060";

        let outcome = client
            .record_lid_pn_in_memory(lid, phone, LearningSource::MigrationSyncOld)
            .await;

        assert!(matches!(
            outcome,
            RecordOutcome::Written { is_new: true, .. }
        ));
        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(lid)
        );
    }

    /// The batch recorder routes a conflicting observational pair to
    /// `usync_phones` while still writing the non-conflicting new pair.
    #[tokio::test]
    async fn test_record_batch_splits_written_and_usync() {
        let client = create_test_client().await;
        let phone_conflict = "5511900000070";
        let lid_live = "200000000000070";
        let lid_known = "200000000000071";
        let phone_fresh = "5511900000072";
        let lid_fresh = "200000000000073";
        client
            .add_lid_pn_mapping(lid_live, phone_conflict, LearningSource::Usync)
            .await
            .unwrap();
        client
            .add_lid_pn_mapping(lid_known, "5511900000079", LearningSource::Usync)
            .await
            .unwrap();

        let outcome = client
            .record_lid_pn_batch_in_memory(
                vec![
                    (lid_known.to_string(), phone_conflict.to_string()),
                    (lid_fresh.to_string(), phone_fresh.to_string()),
                ],
                LearningSource::Other,
            )
            .await;

        assert_eq!(outcome.usync_phones, vec![phone_conflict.to_string()]);
        assert_eq!(outcome.entries.len(), 1);
        assert_eq!(&*outcome.entries[0].phone_number, phone_fresh);
        assert_eq!(
            client
                .lid_pn_cache
                .get_current_lid(phone_conflict)
                .await
                .as_deref(),
            Some(lid_live),
            "the conflicting phone must keep its live LID"
        );
    }

    /// End-to-end through the public batch learn: an `Other` (history-sync)
    /// seed must not overwrite a live-learned LID for the same phone.
    #[tokio::test]
    async fn test_learn_batch_other_preserves_live_mapping() {
        let client = create_test_client().await;
        let phone = "5511900000080";
        let lid_live = "200000000000080";
        let lid_hist = "200000000000081";
        client
            .add_lid_pn_mapping(lid_live, phone, LearningSource::Usync)
            .await
            .unwrap();
        client
            .add_lid_pn_mapping(lid_hist, "5511900000089", LearningSource::Usync)
            .await
            .unwrap();

        client
            .learn_lid_pn_mappings_batch(
                vec![(lid_hist.to_string(), phone.to_string())],
                LearningSource::Other,
                false,
            )
            .await;

        assert_eq!(
            client.lid_pn_cache.get_current_lid(phone).await.as_deref(),
            Some(lid_live),
            "a history-sync seed must not clobber the live mapping"
        );
    }

    #[tokio::test]
    async fn test_latch_lid_migrated_from_props() {
        let client: Arc<Client> = create_test_client().await;

        // Prop absent: nothing latched.
        client.latch_lid_migrated_from_props().await;
        assert!(
            !client
                .persistence_manager
                .get_device_snapshot()
                .lid_migrated
        );

        // Prop observed on: persisted, and it outlives the prop disappearing
        // from a later fetch.
        client
            .ab_props()
            .apply_props(
                false,
                std::iter::once((
                    wacore::iq::abprops::web::LID_ONE_ON_ONE_MIGRATION_ENABLED.code,
                    "1".into(),
                )),
            )
            .await;
        client.latch_lid_migrated_from_props().await;
        client
            .ab_props()
            .apply_props(false, std::iter::empty())
            .await;
        assert!(
            client
                .persistence_manager
                .get_device_snapshot()
                .lid_migrated
        );
        assert!(client.is_lid_migrated().await);
    }

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
    async fn test_resolve_dm_wire_jid_unmigrated_keeps_pn() {
        let (client, pn, lid) = client_with_peer_mapping().await;

        // Unmigrated account: the wire jid stays PN even with a cached mapping.
        assert_eq!(client.resolve_dm_wire_jid(&Jid::pn(pn)).await, Jid::pn(pn));
        // A LID chat id maps back to the PN chat (WA Web keeps 1:1 chats on
        // PN until the account migrates).
        assert_eq!(
            client.resolve_dm_wire_jid(&Jid::lid(lid)).await,
            Jid::pn(pn)
        );
        // Signal session addressing is deliberately not gated.
        assert_eq!(client.resolve_encryption_jid(&Jid::pn(pn)).await.user, lid);
    }

    #[tokio::test]
    async fn test_resolve_dm_wire_jid_unmigrated_unmapped_lid_stays_lid() {
        let client: Arc<Client> = create_test_client().await;
        let lid_jid = Jid::lid("111000011112222");
        assert_eq!(client.resolve_dm_wire_jid(&lid_jid).await, lid_jid);
    }

    #[tokio::test]
    async fn test_resolve_dm_wire_jid_migrated_flag_upgrades_to_lid() {
        let (client, pn, lid) = client_with_peer_mapping().await;

        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetLidMigrated(true))
            .await;

        assert!(client.is_lid_migrated().await);
        assert_eq!(
            client.resolve_dm_wire_jid(&Jid::pn(pn)).await,
            Jid::lid(lid)
        );
    }

    #[tokio::test]
    async fn test_resolve_dm_wire_jid_migration_prop_upgrades_to_lid() {
        let (client, pn, lid) = client_with_peer_mapping().await;

        assert!(!client.is_lid_migrated().await);
        client
            .ab_props()
            .apply_props(
                false,
                std::iter::once((
                    wacore::iq::abprops::web::LID_ONE_ON_ONE_MIGRATION_ENABLED.code,
                    "1".into(),
                )),
            )
            .await;

        assert!(client.is_lid_migrated().await);
        assert_eq!(
            client.resolve_dm_wire_jid(&Jid::pn(pn)).await,
            Jid::lid(lid)
        );
    }

    #[tokio::test]
    async fn test_lid_migration_mapping_sync_learns_and_migrates_with_prop() {
        use buffa::Message as _;
        use waproto::whatsapp as wa;

        let client: Arc<Client> = create_test_client().await;
        let payload = wa::LIDMigrationMappingSyncPayload {
            pn_to_lid_mappings: vec![wa::LIDMigrationMapping {
                pn: 5511987650001,
                assigned_lid: 111000011112222,
                latest_lid: None,
            }],
            chat_db_migration_timestamp: None,
        };
        let sync = wa::LIDMigrationMappingSyncMessage {
            encoded_mapping_payload: Some(payload.encode_to_vec()),
        };

        // Prop off: mappings are learned but the account stays unmigrated,
        // mirroring WA Web's state machine parking at WAITING_PROP.
        client.handle_lid_migration_mapping_sync(&sync).await;
        assert_eq!(
            client
                .resolve_encryption_jid(&Jid::pn("5511987650001"))
                .await
                .user,
            "111000011112222"
        );
        assert!(!client.is_lid_migrated().await);

        // Prop on: the same push persists the migrated flag...
        client
            .ab_props()
            .apply_props(
                false,
                std::iter::once((
                    wacore::iq::abprops::web::LID_ONE_ON_ONE_MIGRATION_ENABLED.code,
                    "1".into(),
                )),
            )
            .await;
        client.handle_lid_migration_mapping_sync(&sync).await;

        // ...which outlives the prop, like the WA Web pref.
        client
            .ab_props()
            .apply_props(false, std::iter::empty())
            .await;
        assert!(client.is_lid_migrated().await);
    }

    #[tokio::test]
    async fn test_is_lid_migrated_prop_zero_or_absent_is_false() {
        let client: Arc<Client> = create_test_client().await;
        assert!(!client.is_lid_migrated().await);

        client
            .ab_props()
            .apply_props(
                false,
                std::iter::once((
                    wacore::iq::abprops::web::LID_ONE_ON_ONE_MIGRATION_ENABLED.code,
                    "0".into(),
                )),
            )
            .await;
        assert!(!client.is_lid_migrated().await);
    }

    #[tokio::test]
    async fn test_lid_migration_mapping_sync_missing_or_malformed_payload_is_ignored() {
        use waproto::whatsapp as wa;

        let client: Arc<Client> = create_test_client().await;
        client
            .ab_props()
            .apply_props(
                false,
                std::iter::once((
                    wacore::iq::abprops::web::LID_ONE_ON_ONE_MIGRATION_ENABLED.code,
                    "1".into(),
                )),
            )
            .await;

        // Missing payload: WA Web treats this as malformed; nothing is
        // learned and the account must not flip to migrated.
        let missing = wa::LIDMigrationMappingSyncMessage {
            encoded_mapping_payload: None,
        };
        client.handle_lid_migration_mapping_sync(&missing).await;
        assert!(
            !client
                .persistence_manager
                .get_device_snapshot()
                .lid_migrated
        );

        let malformed = wa::LIDMigrationMappingSyncMessage {
            encoded_mapping_payload: Some(vec![0xFF, 0xFF, 0xFF]),
        };
        client.handle_lid_migration_mapping_sync(&malformed).await;
        assert!(
            !client
                .persistence_manager
                .get_device_snapshot()
                .lid_migrated
        );
    }

    #[tokio::test]
    async fn test_lid_migration_mapping_sync_prefers_latest_lid() {
        use buffa::Message as _;
        use waproto::whatsapp as wa;

        let client: Arc<Client> = create_test_client().await;
        let payload = wa::LIDMigrationMappingSyncPayload {
            pn_to_lid_mappings: vec![wa::LIDMigrationMapping {
                pn: 5511987650001,
                assigned_lid: 111000011112222,
                latest_lid: Some(999000099990000),
            }],
            chat_db_migration_timestamp: None,
        };
        let sync = wa::LIDMigrationMappingSyncMessage {
            encoded_mapping_payload: Some(payload.encode_to_vec()),
        };

        client.handle_lid_migration_mapping_sync(&sync).await;
        assert_eq!(
            client
                .resolve_encryption_jid(&Jid::pn("5511987650001"))
                .await
                .user,
            "999000099990000"
        );
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
            sender_chain: buffa::MessageField::none(),
            receiver_chains: vec![],
            pending_pre_key: buffa::MessageField::none(),
            remote_registration_id: Some(remote_regid),
            local_registration_id: Some(0),
            alice_base_key: Some(vec![]),
            needs_refresh: None,
            pending_key_exchange: buffa::MessageField::none(),
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

    /// `try_pn_to_lid_migration_decrypt` skips its retry decrypt when the
    /// migration reports nothing moved: with decrypt state unchanged, the
    /// retry would fail identically and log a second decrypt error for
    /// every redelivered copy of an undecryptable message.
    #[tokio::test]
    async fn migration_reports_whether_anything_moved() {
        use wacore::libsignal::protocol::SessionRecord;
        use wacore::types::jid::JidExt as _;

        let client: Arc<Client> = create_test_client().await;
        let pn = "5500000001111";
        let lid = "122222222222222";

        client
            .add_lid_pn_mapping(lid, pn, LearningSource::PeerPnMessage)
            .await
            .unwrap();

        assert!(
            !client
                .migrate_signal_sessions_on_lid_discovery(pn, lid)
                .await,
            "no PN signal state, so nothing can move"
        );

        let pn_addr = Jid::pn_device(pn.to_string(), 0).to_protocol_address();
        client
            .signal_cache
            .put_session(
                &pn_addr,
                SessionRecord::deserialize(&tagged_session_blob(7)).expect("blob deserializes"),
            )
            .await;
        let backend = client.persistence_manager.backend();
        client.signal_cache.flush(backend.as_ref()).await.unwrap();

        assert!(
            client
                .migrate_signal_sessions_on_lid_discovery(pn, lid)
                .await,
            "a PN session moved into the LID slot"
        );
        assert!(
            !client
                .migrate_signal_sessions_on_lid_discovery(pn, lid)
                .await,
            "second call finds the PN side already drained"
        );
    }
}
