//! Pre-key management for Signal Protocol.
//!
//! Pre-key IDs use a persistent monotonic counter (Device::next_pre_key_id)
//! matching WhatsApp Web's NEXT_PK_ID pattern. IDs only increase to prevent
//! collisions when prekeys are consumed non-sequentially from the store.

use crate::client::Client;
use anyhow;
use log;

use std::sync::atomic::Ordering;
use wacore::iq::prekeys::{
    DigestKeyBundleSpec, PreKeyCountSpec, PreKeyFetchReason, PreKeyFetchSpec, PreKeyUploadSpec,
};
use wacore::libsignal::protocol::{KeyPair, PreKeyBundle, PublicKey};
use wacore::libsignal::store::record_helpers::new_pre_key_record;
use wacore::store::commands::DeviceCommand;
use wacore_binary::Jid;

pub use wacore::prekeys::PreKeyUtils;

/// Default number of one-time pre-keys generated and uploaded per batch.
/// Mirrors WA Web's UPLOAD_KEYS_COUNT (`WAWebUploadPreKeysJob`).
pub(crate) const DEFAULT_WANTED_PRE_KEY_COUNT: usize = 812;

const MIN_PRE_KEY_COUNT: usize = 5;

/// Whether `upload_pre_keys` should upload, given the `force` flag and the server's
/// reported pre-key count. The prekey-low path forces, matching WA Web's
/// `handlePreKeyLow` which uploads unconditionally, so `force` bypasses the count guard.
fn should_upload_pre_keys(force: bool, server_count: usize) -> bool {
    force || server_count < MIN_PRE_KEY_COUNT
}

/// WA Web uses 24-bit PreKey IDs (max 2^24 - 1); IDs wrap modulo this.
const MAX_PREKEY_ID: u32 = 16_777_215;

/// Next one-time prekey id to mint from the persistent monotonic counter, falling back to
/// `max_store_id + 1` on migration (when the counter is unset) and wrapping into the 24-bit
/// range. Shared by the batch upload path and the retry-receipt single-key allocation so both
/// draw from the same `NEXT_PK_ID` namespace and never collide (matching WA Web).
fn start_prekey_id(next_pre_key_id: u32, max_store_id: u32) -> u32 {
    let raw = if next_pre_key_id > 0 {
        std::cmp::max(next_pre_key_id as u64, max_store_id as u64 + 1)
    } else {
        max_store_id as u64 + 1
    };
    ((raw - 1) % MAX_PREKEY_ID as u64) as u32 + 1
}

/// One upload pass's accounting, mirroring WA Web `getOrGenPreKeys` semantics
/// (`WAWebSignalStoreApi`): the upload window starts at the FIRST_UNUPLOAD
/// watermark and re-offers leftover generated-but-unuploaded keys, generating
/// only enough new ones to reach the target.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PreKeyUploadPlan {
    /// First id of the upload window (the FIRST_UNUPLOAD watermark after
    /// migration init / self-heal).
    window_start: u32,
    /// Leftover generated-but-unuploaded keys already in the store.
    available: u32,
    /// New keys to generate (`wanted - available`, floored at 0).
    gen_count: u32,
    /// First id of the newly generated range (`window_start + available`).
    gen_start: u32,
    /// NEXT_PK_ID after generation (`gen_start + gen_count`).
    new_next: u32,
}

/// Compute the upload window and generation range from the two watermarks.
///
/// `first_unupload == 0` is the unset/legacy state: the window starts fresh at
/// the legacy-safe `start_prekey_id` (which skips stored-but-unconfirmed rows
/// from the pre-watermark model) with no leftovers. The same reset handles a
/// corrupt `first > next` pair and a window that would cross the 24-bit id
/// boundary; the wrap collapse keeps the window contiguous and is the same
/// accepted tradeoff as the old per-id modulo (the server consumes keys well
/// before a 16M cycle).
fn plan_prekey_upload(
    first_unupload: u32,
    next: u32,
    max_store_id: u32,
    wanted: usize,
) -> PreKeyUploadPlan {
    let wanted = wanted as u64;
    let first = first_unupload as u64;
    let next_eff = next as u64;

    let fresh_window = |start: u64| {
        let start = if start + wanted - 1 > MAX_PREKEY_ID as u64 {
            1
        } else {
            start
        };
        PreKeyUploadPlan {
            window_start: start as u32,
            available: 0,
            gen_count: wanted as u32,
            gen_start: start as u32,
            new_next: (start + wanted) as u32,
        }
    };

    if first == 0 || first > next_eff {
        return fresh_window(start_prekey_id(next, max_store_id) as u64);
    }
    // Cap leftovers at the target: surplus stays in the window for next time
    // (WA Web p <= 0 path uploads only getPreKeysByRange(s, wanted)). New
    // generation always starts at NEXT (savePreKeys semantics), so a capped
    // window never regresses the counter.
    let available = (next_eff - first).min(wanted);
    let gen_count = wanted - available;
    if first + wanted - 1 > MAX_PREKEY_ID as u64
        || (gen_count > 0 && next_eff + gen_count - 1 > MAX_PREKEY_ID as u64)
    {
        // Window or generation would cross the id boundary: collapse to a
        // fresh window at 1 (old high-id rows are overwritten progressively,
        // same acceptance as the previous modulo wrap).
        return fresh_window(1);
    }
    PreKeyUploadPlan {
        window_start: first as u32,
        available: available as u32,
        gen_count: gen_count as u32,
        gen_start: next_eff as u32,
        new_next: (next_eff + gen_count) as u32,
    }
}

/// The upload IQ encodes the pre-key `<list>` length as a u16
/// (`Encoder::write_list_start`), so a larger batch fails to encode after the
/// keys were already generated and stored. Well below MAX_PREKEY_ID, so a single
/// batch never reuses an ID either.
const MAX_PRE_KEY_UPLOAD_BATCH: usize = u16::MAX as usize;

/// Below MIN_PRE_KEY_COUNT the pool ends up flagged-but-empty or loops on
/// re-upload (the count guard never clears); above MAX_PRE_KEY_UPLOAD_BATCH the
/// upload IQ fails to encode. Only an explicitly misconfigured count hits either.
fn clamp_wanted_pre_key_count(n: usize) -> usize {
    n.clamp(MIN_PRE_KEY_COUNT, MAX_PRE_KEY_UPLOAD_BATCH)
}

impl Client {
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.fetch_pre_keys", level = "debug", skip_all, fields(count = jids.len()), err(Debug)))]
    pub(crate) async fn fetch_pre_keys(
        &self,
        jids: &[Jid],
        reason: Option<PreKeyFetchReason>,
    ) -> Result<std::collections::HashMap<Jid, PreKeyBundle>, anyhow::Error> {
        let spec = match reason {
            Some(r) => PreKeyFetchSpec::with_reason(jids.to_vec(), r),
            None => PreKeyFetchSpec::new(jids.to_vec()),
        };

        // Pre-load each companion's account (device 0) identity as the ADV
        // `account_signature_key` fallback: the server omits that field from a
        // contact's companion `<device-identity>` because it's the contact's
        // primary identity the client already stores. Without it we'd reject the
        // bundle and the device would stop receiving (WA Web uses the same stored
        // identity as the fallback in validateADVwithIdentityKey).
        let spec = spec.with_account_identities(self.collect_account_identities(jids).await);

        let bundles = self.execute(spec).await?;

        for jid in bundles.keys() {
            log::debug!("Successfully parsed pre-key bundle for {}", jid.observe());
        }

        Ok(bundles)
    }

    /// Load, for each companion JID, its account (device 0) identity key from the
    /// store, keyed by the normalized companion JID so the prekey parser can use
    /// it as the ADV `account_signature_key` fallback. Missing entries are simply
    /// absent (no fallback for that JID).
    async fn collect_account_identities(
        &self,
        jids: &[Jid],
    ) -> std::collections::HashMap<Jid, [u8; 32]> {
        use futures::StreamExt;
        // Fan out the per-companion identity loads (independent cache/DB reads) —
        // the keyless-companion set can be large on a cold group send. Owned Vec so
        // the stream doesn't borrow `jids` through buffer_unordered (Send bound).
        const COMPANION_IDENTITY_LOAD_CONCURRENCY: usize = 16;
        let companions: Vec<Jid> = jids.iter().filter(|j| j.device != 0).cloned().collect();
        futures::stream::iter(companions)
            .map(|jid| async move {
                self.load_account_identity(&jid)
                    .await
                    .map(|id| (jid.normalize_for_prekey_bundle(), id))
            })
            .buffer_unordered(COMPANION_IDENTITY_LOAD_CONCURRENCY)
            .filter_map(|entry| async move { entry })
            .collect()
            .await
    }

    /// Load a companion's account (device 0) identity from the store, for use as
    /// the ADV `account_signature_key` fallback (WA Web `validateADVwithIdentityKey`
    /// loads the same stored identity). Reads through the signal cache so an
    /// identity established earlier this session (not yet flushed) is still found.
    /// `None` when not stored.
    pub(crate) async fn load_account_identity(&self, companion_jid: &Jid) -> Option<[u8; 32]> {
        use wacore::types::jid::JidExt;

        let account_jid = companion_jid.with_device(0);
        let addr = account_jid.to_protocol_address();
        let backend = self
            .persistence_manager
            .get_device_snapshot()
            .backend
            .clone();
        match self.signal_cache.get_identity(&addr, &*backend).await {
            Ok(Some(id)) if id.len() == 32 => {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(&id);
                Some(arr)
            }
            Ok(_) => None,
            Err(e) => {
                log::debug!(
                    "ADV fallback: failed to load account identity for {}: {}",
                    account_jid.observe(),
                    e
                );
                None
            }
        }
    }

    /// Query the WhatsApp server for how many pre-keys it currently has for this device.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.server_pre_key_count",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn get_server_pre_key_count(&self) -> Result<usize, crate::request::IqError> {
        let response = self.execute(PreKeyCountSpec::new()).await?;
        Ok(response.count)
    }

    /// Upload prekeys at login if the persisted flag indicates they're needed.
    /// Matches WA Web's PassiveTasks.js:30 which checks `getServerHasPreKeys()`.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.upload_pre_keys_at_login",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn upload_pre_keys_at_login(&self) -> Result<(), anyhow::Error> {
        let has_prekeys = self
            .persistence_manager
            .get_device_snapshot()
            .server_has_prekeys;

        if has_prekeys {
            log::debug!("Server has prekeys (persisted flag), skipping login upload.");
            return Ok(());
        }

        // Serialize with prekey-low/digest paths to avoid duplicate uploads
        let _guard = self.prekey_upload_lock.lock().await;

        // Re-check after acquiring lock (another task may have uploaded)
        if self
            .persistence_manager
            .get_device_snapshot()
            .server_has_prekeys
        {
            return Ok(());
        }

        log::info!("Server missing prekeys (persisted flag), uploading.");
        // Operation-level outcome (the login path skips the retry wrapper).
        let r = self.upload_pre_keys_inner().await;
        wacore::telemetry::prekey_upload(if r.is_ok() { "ok" } else { "fail" });
        r
    }

    /// Ensure the server has enough pre-keys, uploading if below threshold.
    /// When `force` is true, skips the count guard (used by digest key repair).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.upload_pre_keys", level = "debug", skip_all, fields(force = force), err(Debug)))]
    pub(crate) async fn upload_pre_keys(&self, force: bool) -> Result<(), anyhow::Error> {
        // Decision is should_upload_pre_keys(force, count), but a forced upload short-circuits
        // and skips the server-count IQ entirely: WA Web's handlePreKeyLow uploads
        // unconditionally, so a stale or transiently-failing count must never block or delay
        // the replenish. Only a non-forced caller queries the count and applies the guard.
        if !force {
            let server_count = self
                .get_server_pre_key_count()
                .await
                .map_err(|e| anyhow::anyhow!(e))?;

            if !should_upload_pre_keys(force, server_count) {
                log::debug!("Server has {server_count} pre-keys, no upload needed.");
                return Ok(());
            }

            log::debug!("Server has {server_count} pre-keys, uploading.");
        }

        self.upload_pre_keys_inner().await
    }

    /// Get-or-generate ONE one-time prekey, mirroring WA Web's
    /// `getOrGenSinglePreKey` = `getOrGenPreKeys(1)`: reuse the first
    /// generated-but-unuploaded window key when one exists (it stays in the
    /// window and is uploaded by the next batch, like WA Web), else generate a
    /// fresh key at `NEXT_PK_ID` and advance the counter at generation time.
    /// The caller must hold `prekey_upload_lock` to serialize the watermark
    /// math with the upload path.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.get_or_gen_prekey",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn get_or_gen_single_pre_key(
        &self,
    ) -> Result<(u32, PublicKey), anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let backend = device_snapshot.backend.clone();
        let max_id = backend.get_max_prekey_id().await?;
        let plan = plan_prekey_upload(
            device_snapshot.first_unupload_pre_key_id,
            device_snapshot.next_pre_key_id,
            max_id,
            1,
        );

        if plan.gen_count == 0 {
            // Load the whole remaining window: a consumed head (a previously
            // reused retry key the peer already spent) must not abandon the
            // still-live keys behind it, so the heal advances FIRST to the
            // next stored id instead of past the window. WA Web throws on a
            // missing head; healing is strictly better and stays in the same
            // id namespace.
            let window_ids: Vec<u32> =
                (plan.window_start..device_snapshot.next_pre_key_id).collect();
            let mut rows = backend.load_prekeys_batch(&window_ids).await?;
            rows.sort_unstable_by_key(|(id, _)| *id);
            if let Some((id, record)) = rows.into_iter().next() {
                if id != plan.window_start {
                    log::warn!(
                        "prekey window head {} missing from store; advancing to {id}",
                        plan.window_start
                    );
                    self.persistence_manager
                        .process_command(DeviceCommand::SetPreKeyWatermarks {
                            next_pre_key_id: device_snapshot.next_pre_key_id,
                            first_unupload_pre_key_id: id,
                        })
                        .await;
                }
                use buffa::Message;
                let structure =
                    waproto::whatsapp::PreKeyRecordStructure::decode_from_slice(&record)?;
                let record = wacore::libsignal::store::record_helpers::prekey_structure_to_record(
                    structure,
                )?;
                return Ok((id, record.key_pair()?.public_key));
            }
            log::warn!(
                "prekey window [{}, {}) fully consumed; generating fresh",
                plan.window_start,
                device_snapshot.next_pre_key_id
            );
        }

        let id = if plan.gen_count > 0 {
            plan.gen_start
        } else {
            // Empty window: generate at NEXT and collapse FIRST onto it. This
            // path bypasses the planner's boundary handling, so wrap to 1 at
            // the 24-bit edge like the planner's collapse does.
            let raw = device_snapshot
                .next_pre_key_id
                .max(plan.window_start.saturating_add(1));
            if raw > MAX_PREKEY_ID { 1 } else { raw }
        };
        let key_pair = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());
        let record = new_pre_key_record(id, &key_pair);
        use buffa::Message;
        backend
            .store_prekey(id, &record.encode_to_vec(), false)
            .await?;
        self.persistence_manager
            .process_command(DeviceCommand::SetPreKeyWatermarks {
                next_pre_key_id: id.saturating_add(1),
                first_unupload_pre_key_id: if plan.gen_count > 0 {
                    plan.window_start
                } else {
                    id
                },
            })
            .await;
        // Same durability pairing as the batch path: the stored key is
        // durable, so its watermarks must not ride the lazy saver. A failed
        // flush fails the allocation; the retry dance recovers later.
        self.persistence_manager
            .flush()
            .await
            .map_err(|e| anyhow::anyhow!("failed to flush prekey watermarks: {e:?}"))?;
        Ok((id, key_pair.public_key))
    }

    /// WA Web `markKeyAsUploaded`: exclude a retry-distributed one-time prekey
    /// from the next batch upload's server offer, so the same id is never both
    /// direct-distributed AND pooled. The `_guard` proof makes holding
    /// `prekey_upload_lock` a compile-time requirement: the snapshot read and
    /// the watermark write must be atomic against the batch upload path, or a
    /// stale `next_pre_key_id` could roll the upper watermark back. Idempotent.
    pub(crate) async fn mark_single_prekey_uploaded(
        &self,
        _guard: &async_lock::MutexGuard<'_, ()>,
        id: u32,
    ) -> Result<(), anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        if device_snapshot.first_unupload_pre_key_id != id {
            return Ok(());
        }
        let next_first = if id >= MAX_PREKEY_ID { 1 } else { id + 1 };
        // Only when marking the terminal id itself (id == MAX) does a preceding
        // allocation leave NEXT at MAX+1 (out of range); collapse it onto the
        // wrapped low watermark so the next window is empty ([next_first,
        // next_first)). Keying on NEXT alone would wrongly discard a non-terminal
        // high-end window whose head sits just below MAX.
        let next_pre_key_id =
            if id >= MAX_PREKEY_ID && device_snapshot.next_pre_key_id > MAX_PREKEY_ID {
                next_first
            } else {
                device_snapshot.next_pre_key_id
            };
        self.persistence_manager
            .process_command(DeviceCommand::SetPreKeyWatermarks {
                next_pre_key_id,
                first_unupload_pre_key_id: next_first,
            })
            .await;
        self.persistence_manager
            .flush()
            .await
            .map_err(|e| anyhow::anyhow!("failed to flush prekey watermark after mark: {e:?}"))?;
        Ok(())
    }

    /// Generate and upload the configured number of pre-keys (see
    /// [`Client::set_wanted_pre_key_count`]). Shared by `upload_pre_keys` and
    /// `upload_pre_keys_at_login` to avoid redundant server count queries.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.upload_pre_keys_inner",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    async fn upload_pre_keys_inner(&self) -> Result<(), anyhow::Error> {
        self.upload_pre_keys_pass(true).await
    }

    /// One upload pass. `allow_collapse_retry` permits a single inline rerun
    /// after collapsing a fully consumed window, so a one-shot caller (the
    /// login path logs and moves on) still ends the pass with fresh keys; the
    /// rerun cannot hit the empty branch again because the collapsed plan
    /// generates a full batch.
    async fn upload_pre_keys_pass(&self, allow_collapse_retry: bool) -> Result<(), anyhow::Error> {
        // INVARIANT: every caller holds `prekey_upload_lock` (login, prekey-low
        // notification, refresh, digest repair), serializing the watermark math
        // with the retry-receipt single-key path.
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let backend = device_snapshot.backend.clone();

        let configured = self.wanted_pre_key_count.load(Ordering::Relaxed);
        let wanted = clamp_wanted_pre_key_count(configured);
        if wanted != configured {
            log::warn!("wanted_pre_key_count {configured} out of range, clamped to {wanted}");
        }

        // WA Web getOrGenPreKeys: re-offer the leftover generated-but-unuploaded
        // window first and only generate enough new keys to reach the target.
        let max_id = backend.get_max_prekey_id().await?;
        if device_snapshot.first_unupload_pre_key_id == 0 {
            log::info!(
                "Initialising prekey upload window (legacy counter = {}, MAX(key_id) = {})",
                device_snapshot.next_pre_key_id,
                max_id
            );
        }
        let plan = plan_prekey_upload(
            device_snapshot.first_unupload_pre_key_id,
            device_snapshot.next_pre_key_id,
            max_id,
            wanted,
        );

        // Public keys of the freshly generated batch, kept from generation so the
        // upload never reads them back out of the store and never decodes protobuf
        // to recover a key it just held in hand. Empty when the plan only re-offers
        // leftover window keys (gen_count == 0).
        let mut fresh_pre_keys: Vec<(u32, PublicKey)> = Vec::new();
        if plan.gen_count > 0 {
            let gen_start = plan.gen_start;
            let gen_count = plan.gen_count as usize;
            // Per-key X25519 generation and prost encoding are CPU-bound, and the
            // batch size is caller-configurable, so offload the whole batch to keep
            // the async executor responsive. Records are encoded into one contiguous
            // buffer with zero-copy Bytes slices instead of an alloc per record.
            let (encoded_batch, generated) = wacore::runtime::blocking(&*self.runtime, move || {
                use buffa::Message;

                // Seed one CSPRNG and advance it per key, rather than reseeding from
                // entropy on every iteration.
                let mut rng = rand::make_rng::<rand::rngs::StdRng>();
                // Encode each record into the shared buffer and drop it immediately so the
                // whole batch of PreKeyRecordStructures (each owns two heap Vecs of key bytes)
                // is never resident at once — that batch was the dominant controllable peak on
                // the connect path. MAX_RECORD_LEN keeps the buffer to a single allocation;
                // being just a capacity hint, it uses the type-level upper bound (1-byte id
                // tag + <=5-byte u32 varint + two 34 B key fields = 74) rather than depending
                // on the tighter 24-bit id cap.
                const MAX_RECORD_LEN: usize = 74;
                let mut pubkeys = Vec::with_capacity(gen_count);
                let mut offsets = Vec::with_capacity(gen_count);
                let mut buf = Vec::with_capacity(gen_count * MAX_RECORD_LEN);
                for i in 0..gen_count {
                    let pre_key_id = gen_start + i as u32;
                    let key_pair = KeyPair::generate(&mut rng);
                    let start = buf.len();
                    new_pre_key_record(pre_key_id, &key_pair).encode(&mut buf);
                    offsets.push((pre_key_id, start..buf.len()));
                    pubkeys.push((pre_key_id, key_pair.public_key));
                }
                let shared = bytes::Bytes::from(buf);
                let encoded_batch: Vec<(u32, bytes::Bytes)> = offsets
                    .into_iter()
                    .map(|(id, range)| (id, shared.slice(range)))
                    .collect();
                (encoded_batch, pubkeys)
            })
            .await;

            // Persist the freshly generated prekeys before uploading them so they are
            // already available for local decryption if the server starts sending
            // pkmsg traffic immediately after accepting the upload.
            // Propagate errors — uploading a key we can't store locally would cause
            // decryption failures when the server hands it out.
            backend.store_prekeys_batch(&encoded_batch, false).await?;
            fresh_pre_keys = generated;
        }

        // Advance NEXT at GENERATION time (WA Web savePreKeys) and initialise
        // FIRST for a legacy device, in one command. From here the window
        // covers every stored-but-unuploaded key, so a failure below never
        // leads to regenerating over live ids.
        self.persistence_manager
            .process_command(DeviceCommand::SetPreKeyWatermarks {
                next_pre_key_id: plan.new_next,
                first_unupload_pre_key_id: plan.window_start,
            })
            .await;
        // The generated rows are already durable; the watermarks ride the lazy
        // device saver. Flush them now so a crash before the IQ cannot reload
        // pre-generation watermarks and orphan the stored window. A failed
        // flush aborts the pass: proceeding would upload keys whose
        // accounting is not durable, the exact state this barrier prevents.
        self.persistence_manager
            .flush()
            .await
            .map_err(|e| anyhow::anyhow!("failed to flush prekey watermarks: {e:?}"))?;

        // Only the leftover (already-stored) window keys are read back and decoded;
        // the fresh ones are already in `fresh_pre_keys`. On the common connect path
        // the window is all-fresh, so this reads and decodes nothing. Leftover gaps
        // are tolerated (a window key consumed via a retry receipt leaves a hole).
        let leftover_ids: Vec<u32> = (0..plan.available).map(|i| plan.window_start + i).collect();
        let mut leftover_rows = if leftover_ids.is_empty() {
            Vec::new()
        } else {
            backend.load_prekeys_batch(&leftover_ids).await?
        };
        leftover_rows.sort_unstable_by_key(|(id, _)| *id);

        if plan.gen_count == 0 && leftover_rows.is_empty() {
            // A fully consumed/missing leftover window with no generation would bail
            // forever (available > 0 keeps gen_count at 0). Collapse the window and
            // rerun the pass so a one-shot caller still uploads.
            self.persistence_manager
                .process_command(DeviceCommand::SetPreKeyWatermarks {
                    next_pre_key_id: plan.new_next,
                    first_unupload_pre_key_id: plan.new_next,
                })
                .await;
            if allow_collapse_retry {
                log::warn!(
                    "prekey window [{}, {}) fully missing; collapsed, regenerating",
                    plan.window_start,
                    plan.new_next
                );
                return Box::pin(self.upload_pre_keys_pass(false)).await;
            }
            anyhow::bail!("no prekey available to upload");
        }

        let pre_key_pairs = {
            let mut pairs: Vec<(u32, PublicKey)> =
                Vec::with_capacity(leftover_rows.len() + fresh_pre_keys.len());
            // Leftover keys live only in the store, so decode them in full — the same
            // `PreKeyRecordStructure::decode` the consume path runs, so a record
            // accepted here is one this device can later decrypt with. Fresh keys skip
            // decode entirely; their public keys never left memory.
            use buffa::Message;
            for (id, record) in &leftover_rows {
                let public_key =
                    waproto::whatsapp::PreKeyRecordStructure::decode_from_slice(&record[..])
                        .map_err(anyhow::Error::from)
                        .and_then(|s| {
                            let raw = s
                                .public_key
                                .ok_or_else(|| anyhow::anyhow!("record missing public key"))?;
                            PublicKey::from_djb_public_key_bytes(&raw).map_err(anyhow::Error::from)
                        });
                match public_key {
                    Ok(public_key) => pairs.push((*id, public_key)),
                    Err(e) => log::warn!("skipping undecodable prekey record {id}: {e:?}"),
                }
            }
            // Fresh ids exceed every leftover id and were generated in ascending
            // order, so appending keeps `pairs` sorted (last_id reads the tail).
            pairs.extend(fresh_pre_keys);
            if pairs.is_empty() {
                anyhow::bail!("no decodable prekey available to upload");
            }
            pairs
        };
        let last_id = pre_key_pairs
            .last()
            .map(|(id, _)| *id)
            .expect("non-empty checked above");
        let uploaded_count = pre_key_pairs.len();
        let pre_key_ids: Vec<u32> = pre_key_pairs.iter().map(|(id, _)| *id).collect();

        let spec = PreKeyUploadSpec::new(
            device_snapshot.registration_id,
            device_snapshot.identity_key.public_key,
            device_snapshot.signed_pre_key_id,
            device_snapshot.signed_pre_key.public_key,
            device_snapshot.signed_pre_key_signature.to_vec(),
            pre_key_pairs,
        );

        // Mark the window uploaded BEFORE the send, like WA Web's
        // markKeyAsUploaded (PreKeysJob.js runs it ahead of the IQ). On a
        // mid-flight failure the server state is unknown, so the keys are
        // abandoned rather than re-offered: re-uploading an id a peer may
        // already have consumed would corrupt the server pool. The keys stay
        // stored locally and remain decryptable if the upload did land.
        self.persistence_manager
            .process_command(DeviceCommand::SetPreKeyWatermarks {
                next_pre_key_id: plan.new_next,
                first_unupload_pre_key_id: plan.window_start.max(last_id.saturating_add(1)),
            })
            .await;
        // The abandon watermark must be durable BEFORE the fallible send: a
        // crash after a failed IQ would otherwise reload FIRST=window_start
        // and re-offer ids that may already be in the server pool. A failed
        // flush aborts instead of sending with non-durable abandonment.
        self.persistence_manager
            .flush()
            .await
            .map_err(|e| anyhow::anyhow!("failed to flush abandon watermark: {e:?}"))?;

        self.execute(spec).await?;

        // Mark the uploaded prekeys as server-synced. UPDATE semantics: a
        // window key consumed by an inbound pkmsg while the IQ was in flight
        // (retry-receipt keys are reachable that way, and consumption is not
        // serialized by prekey_upload_lock) must stay deleted, not be
        // resurrected by an upsert of the stale record.
        let uploaded_ids: Vec<u32> = pre_key_ids;
        if let Err(e) = backend.mark_prekeys_uploaded(&uploaded_ids).await {
            log::warn!("Failed to mark prekeys as uploaded: {:?}", e);
        }

        // Persist flag matching WA Web's setServerHasPreKeys(true) (PreKeysJob.js:79)
        self.persistence_manager
            .modify_device(|d| d.server_has_prekeys = true)
            .await;

        log::debug!(
            "Successfully uploaded {} pre-keys ({} reused from the window) starting from {}.",
            uploaded_count,
            plan.available,
            plan.window_start
        );

        Ok(())
    }

    /// Upload pre-keys with Fibonacci retry backoff matching WA Web's `PromiseRetryLoop`.
    ///
    /// Retry schedule: 1s, 2s, 3s, 5s, 8s, 13s, ... capped at 610s.
    /// Verified against WA Web JS: `{ algo: { type: "fibonacci", first: 1e3, second: 2e3 }, max: 61e4 }`
    ///
    /// When `force` is true, bypasses the count guard (used by digest repair path).
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.session.upload_pre_keys_retry", level = "debug", skip_all, fields(force = force), err(Debug)))]
    pub(crate) async fn upload_pre_keys_with_retry(
        &self,
        force: bool,
    ) -> Result<(), anyhow::Error> {
        let mut delay_a: u64 = 1;
        let mut delay_b: u64 = 2;
        const MAX_DELAY_SECS: u64 = 610;

        loop {
            match self.upload_pre_keys(force).await {
                Ok(()) => {
                    log::info!("Pre-key upload succeeded");
                    // Operation-level outcome: one emit per logical upload, not per attempt.
                    wacore::telemetry::prekey_upload("ok");
                    return Ok(());
                }
                Err(e) => {
                    let delay = delay_a.min(MAX_DELAY_SECS);
                    log::warn!("Pre-key upload failed, retrying in {}s: {:?}", delay, e);

                    self.runtime
                        .sleep(std::time::Duration::from_secs(delay))
                        .await;

                    // Bail if disconnected during retry wait
                    if !self.is_logged_in.load(Ordering::Relaxed) {
                        wacore::telemetry::prekey_upload("fail");
                        return Err(anyhow::anyhow!(
                            "Connection lost during pre-key upload retry"
                        ));
                    }

                    let next = delay_a + delay_b;
                    delay_a = delay_b;
                    delay_b = next;
                }
            }
        }
    }

    /// Force-refresh the server's one-time pre-key pool with a fresh batch.
    ///
    /// Intended for callers that just restored a device from an external source
    /// (e.g., migrating a Baileys session into an `InMemoryBackend`). The server
    /// may still hold pre-key IDs whose private key material the caller cannot
    /// reconstruct; any `pkmsg` referencing those IDs will fail forever with
    /// `InvalidPreKeyId`. Uploading a fresh batch gives the server new IDs the
    /// caller *does* have locally, and old unmatched IDs drain as peers consume
    /// them.
    ///
    /// Acquires `prekey_upload_lock` for the duration so this force-upload
    /// cannot race on `start_id` with the count-based and digest-repair paths.
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.refresh_pre_keys",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub async fn refresh_pre_keys(&self) -> Result<(), anyhow::Error> {
        let _guard = self.prekey_upload_lock.lock().await;
        self.upload_pre_keys_with_retry(true).await
    }

    /// Validate server key bundle digest, re-uploading only when the server has no record.
    ///
    /// Matches WA Web's `WAWebDigestKeyJob.digestKey()`:
    /// 1. Queries server for key bundle digest (identity + signed prekey + prekey IDs + SHA-1 hash)
    /// 2. If server returns 404 (no record): triggers `upload_pre_keys_with_retry()`
    /// 3. If server returns 406/503/other error: logs and does nothing
    /// 4. On success: loads local keys and computes SHA-1 over the same material
    /// 5. If validation fails (regId mismatch, missing prekey, hash mismatch): logs warning,
    ///    does NOT re-upload — WA Web catches all `validateLocalKeyBundle` exceptions without
    ///    re-uploading; the normal `RotateKeyJob` will eventually refresh keys
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(
            name = "wa.session.validate_digest_key",
            level = "debug",
            skip_all,
            err(Debug)
        )
    )]
    pub(crate) async fn validate_digest_key(&self) -> Result<(), anyhow::Error> {
        // Hold the lock across the whole pass so the 404 re-upload can't race with
        // `upload_pre_keys_at_login`, `handle_prekey_low`, or `refresh_pre_keys` on
        // `next_pre_key_id` allocation.
        let _guard = self.prekey_upload_lock.lock().await;

        let response = match self.execute(DigestKeyBundleSpec::new()).await {
            Ok(resp) => resp,
            Err(crate::request::IqError::ServerError { code: 404, .. }) => {
                log::warn!("digestKey: no record found for current user, re-uploading");
                return self.upload_pre_keys_with_retry(true).await;
            }
            Err(crate::request::IqError::ServerError { code: 406, .. }) => {
                log::warn!("digestKey: malformed request");
                return Ok(());
            }
            Err(crate::request::IqError::ServerError { code: 503, .. }) => {
                log::warn!("digestKey: service unavailable");
                return Ok(());
            }
            Err(crate::request::IqError::ParseError(e)) => {
                // WA Web catches parse failures without re-uploading
                log::debug!("digestKey: unparseable digest response ({e}), skipping");
                return Ok(());
            }
            Err(e) => {
                if !self.is_shutting_down() {
                    log::warn!("digestKey: server error: {:?}", e);
                }
                return Ok(());
            }
        };

        // WA Web's validateLocalKeyBundle validates but catches ALL exceptions without
        // re-uploading. The catch block in digestKey() sets a=false for any throw from y(),
        // meaning only 404 triggers re-upload. We match that: log warnings, return Ok(()).
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        if response.reg_id != device_snapshot.registration_id {
            log::warn!(
                "digestKey: registration ID mismatch (server={}, local={}), skipping",
                response.reg_id,
                device_snapshot.registration_id
            );
            return Ok(());
        }

        // Compute local SHA-1 digest over the same material as WA Web's validateLocalKeyBundle:
        // identity_pub_key + signed_prekey_pub + signed_prekey_signature + (for each prekey ID: load 32-byte pubkey)
        let identity_bytes = device_snapshot.identity_key.public_key.public_key_bytes();
        let skey_pub_bytes = device_snapshot.signed_pre_key.public_key.public_key_bytes();
        let skey_sig_bytes = &device_snapshot.signed_pre_key_signature;

        let backend = self
            .persistence_manager
            .get_device_snapshot()
            .backend
            .clone();

        // Batch-load all prekeys referenced by the server digest
        let loaded = match backend.load_prekeys_batch(&response.prekey_ids).await {
            Ok(v) => v,
            Err(e) => {
                log::warn!("digestKey: failed to batch-load prekeys: {:?}, skipping", e);
                return Ok(());
            }
        };

        // Build a lookup so we preserve the server-requested order.
        // Dedupe the expected count since the server may send duplicate IDs.
        let loaded_map: std::collections::HashMap<u32, bytes::Bytes> = loaded.into_iter().collect();
        let unique_requested: std::collections::HashSet<&u32> =
            response.prekey_ids.iter().collect();

        if loaded_map.len() < unique_requested.len() {
            log::warn!(
                "digestKey: missing {} local prekeys, skipping",
                unique_requested.len() - loaded_map.len()
            );
            return Ok(());
        }

        // Extract public keys directly from stored protobuf bytes without full decode
        let mut prekey_pubkeys = Vec::with_capacity(response.prekey_ids.len());
        for prekey_id in &response.prekey_ids {
            let Some(record_bytes) = loaded_map.get(prekey_id) else {
                log::warn!("digestKey: missing local prekey {}, skipping", prekey_id);
                return Ok(());
            };
            match wacore::prekeys::extract_prekey_public_key(record_bytes) {
                Some(pk) => prekey_pubkeys.push(pk),
                None => {
                    log::warn!(
                        "digestKey: prekey {} has no public key, skipping",
                        prekey_id
                    );
                    return Ok(());
                }
            }
        }

        let local_hash = wacore::prekeys::compute_key_bundle_digest(
            identity_bytes,
            skey_pub_bytes,
            skey_sig_bytes,
            &prekey_pubkeys,
        );

        if local_hash.as_slice() != response.hash.as_slice() {
            log::warn!(
                "digestKey: hash mismatch (server={}, local={}), skipping",
                hex::encode(&response.hash),
                hex::encode(local_hash)
            );
            return Ok(());
        }

        log::debug!("digestKey: key bundle validation successful");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_WANTED_PRE_KEY_COUNT, MAX_PRE_KEY_UPLOAD_BATCH, MAX_PREKEY_ID, MIN_PRE_KEY_COUNT,
        clamp_wanted_pre_key_count, plan_prekey_upload, should_upload_pre_keys, start_prekey_id,
    };

    #[test]
    fn plan_initialises_window_for_legacy_device() {
        // first unset: fresh window at the legacy-safe start (max(counter, store+1)),
        // full generation.
        let p = plan_prekey_upload(0, 7, 20, 812);
        assert_eq!(p.window_start, 21, "legacy start skips stored rows");
        assert_eq!(p.available, 0);
        assert_eq!(p.gen_count, 812);
        assert_eq!(p.gen_start, 21);
        assert_eq!(p.new_next, 833);
    }

    #[test]
    fn plan_generates_full_batch_on_empty_window() {
        let p = plan_prekey_upload(100, 100, 99, 812);
        assert_eq!(p.window_start, 100);
        assert_eq!(p.available, 0);
        assert_eq!(p.gen_count, 812);
        assert_eq!(p.gen_start, 100);
        assert_eq!(p.new_next, 912);
    }

    #[test]
    fn plan_reuses_leftovers_and_tops_up() {
        // 50 leftover unuploaded keys: only 762 new ones, window re-offers all 812.
        let p = plan_prekey_upload(100, 150, 149, 812);
        assert_eq!(p.window_start, 100);
        assert_eq!(p.available, 50);
        assert_eq!(p.gen_count, 762);
        assert_eq!(p.gen_start, 150, "generation starts at NEXT (savePreKeys)");
        assert_eq!(p.new_next, 912);
    }

    #[test]
    fn plan_full_window_generates_nothing_and_keeps_next() {
        // More leftovers than the target (WA Web p <= 0): upload the first
        // `wanted`, generate nothing, and never regress NEXT.
        let p = plan_prekey_upload(100, 1500, 1499, 812);
        assert_eq!(p.window_start, 100);
        assert_eq!(p.available, 812);
        assert_eq!(p.gen_count, 0);
        assert_eq!(p.new_next, 1500, "a capped window must not regress NEXT");
    }

    #[test]
    fn plan_heals_corrupt_first_past_next() {
        let p = plan_prekey_upload(500, 100, 600, 812);
        assert_eq!(p.window_start, 601, "heals via the legacy-safe start");
        assert_eq!(p.available, 0);
        assert_eq!(p.gen_count, 812);
    }

    #[test]
    fn plan_collapses_window_at_id_boundary() {
        // Window would cross the 24-bit boundary: collapse to a fresh window at 1.
        let p = plan_prekey_upload(
            MAX_PREKEY_ID - 10,
            MAX_PREKEY_ID - 5,
            MAX_PREKEY_ID - 6,
            812,
        );
        assert_eq!(p.window_start, 1);
        assert_eq!(p.available, 0);
        assert_eq!(p.gen_count, 812);
        assert_eq!(p.new_next, 813);
    }

    #[test]
    fn plan_single_key_reuses_window_head() {
        // getOrGenSinglePreKey = getOrGenPreKeys(1): a non-empty window means
        // no generation; the head key is the answer.
        let p = plan_prekey_upload(10, 12, 11, 1);
        assert_eq!(p.window_start, 10);
        assert_eq!(p.available, 1);
        assert_eq!(p.gen_count, 0);
        assert_eq!(p.new_next, 12);
    }

    #[test]
    fn default_matches_wa_web_upload_keys_count() {
        // WAWebUploadPreKeysJob's UPLOAD_KEYS_COUNT; drift here diverges from WA Web.
        assert_eq!(DEFAULT_WANTED_PRE_KEY_COUNT, 812);
    }

    #[test]
    fn clamp_wanted_pre_key_count_bounds() {
        assert_eq!(clamp_wanted_pre_key_count(0), MIN_PRE_KEY_COUNT);
        assert_eq!(clamp_wanted_pre_key_count(2), MIN_PRE_KEY_COUNT);
        assert_eq!(clamp_wanted_pre_key_count(4), MIN_PRE_KEY_COUNT);
        assert_eq!(
            clamp_wanted_pre_key_count(MIN_PRE_KEY_COUNT),
            MIN_PRE_KEY_COUNT
        );
        assert_eq!(clamp_wanted_pre_key_count(812), 812);
        assert_eq!(
            clamp_wanted_pre_key_count(MAX_PRE_KEY_UPLOAD_BATCH),
            MAX_PRE_KEY_UPLOAD_BATCH
        );
        assert_eq!(
            clamp_wanted_pre_key_count(MAX_PRE_KEY_UPLOAD_BATCH + 1),
            MAX_PRE_KEY_UPLOAD_BATCH
        );
        assert_eq!(
            clamp_wanted_pre_key_count(usize::MAX),
            MAX_PRE_KEY_UPLOAD_BATCH
        );
    }

    #[test]
    fn start_prekey_id_uses_counter_and_wraps() {
        // Counter ahead of the store: use the counter (monotonic, never reuses an id).
        assert_eq!(start_prekey_id(10, 5), 10);
        // Store ahead of (or equal to) the counter: use max_store + 1.
        assert_eq!(start_prekey_id(3, 100), 101);
        // Migration (counter unset = 0): max_store + 1.
        assert_eq!(start_prekey_id(0, 5), 6);
        // Wraps into the 24-bit range instead of pinning above MAX_PREKEY_ID.
        assert_eq!(start_prekey_id(MAX_PREKEY_ID + 1, 0), 1);
        assert_eq!(start_prekey_id(MAX_PREKEY_ID, 0), MAX_PREKEY_ID);
    }

    #[test]
    fn force_upload_bypasses_count_guard() {
        // WA Web's handlePreKeyLow uploads unconditionally, so the prekey-low path forces
        // and the count guard must not apply.
        assert!(should_upload_pre_keys(true, 1000), "force always uploads");
        assert!(
            !should_upload_pre_keys(false, MIN_PRE_KEY_COUNT),
            "count guard skips when not forced and at/above threshold"
        );
        assert!(
            should_upload_pre_keys(false, MIN_PRE_KEY_COUNT - 1),
            "below threshold uploads even without force"
        );
    }
}

#[cfg(test)]
mod window_tests {
    use wacore::libsignal::protocol::PublicKey;

    fn snapshot(client: &crate::client::Client) -> (u32, u32) {
        let d = client.persistence_manager.get_device_snapshot();
        (d.next_pre_key_id, d.first_unupload_pre_key_id)
    }

    fn backend(
        client: &crate::client::Client,
    ) -> std::sync::Arc<dyn crate::store::traits::Backend> {
        client.persistence_manager.backend()
    }

    /// A failed upload IQ must leave the watermarks past the generated window
    /// (WA Web abandons on unknown server state) and the next attempt must
    /// mint FRESH ids, never regenerating over the stored ones: that
    /// regeneration was the prekey-collision class on partial success.
    #[tokio::test]
    async fn failed_upload_abandons_window_and_never_remints_ids() {
        let client = crate::test_utils::create_test_client_with_name("prekey_window_fail").await;
        client.set_wanted_pre_key_count(5);

        let err = client.upload_pre_keys_inner().await;
        assert!(err.is_err(), "IQ must fail on a disconnected client");

        let (next, first) = snapshot(&client);
        assert_eq!(next, 6, "NEXT advances at generation time");
        assert_eq!(first, 6, "FIRST is marked past the window before the send");

        let rows = backend(&client)
            .load_prekeys_batch(&[1, 2, 3, 4, 5])
            .await
            .expect("load");
        assert_eq!(rows.len(), 5, "the generated window stays stored");
        let before: Vec<_> = rows.into_iter().collect();

        let _ = client.upload_pre_keys_inner().await;
        let (next, first) = snapshot(&client);
        assert_eq!(next, 11, "second attempt mints fresh ids 6..=10");
        assert_eq!(first, 11);

        let rows2 = backend(&client)
            .load_prekeys_batch(&[6, 7, 8, 9, 10])
            .await
            .expect("load");
        assert_eq!(rows2.len(), 5);

        let after = backend(&client)
            .load_prekeys_batch(&[1, 2, 3, 4, 5])
            .await
            .expect("load");
        assert_eq!(
            before, after,
            "abandoned rows must never be regenerated (collision class)"
        );
    }

    /// getOrGenSinglePreKey parity: the window head is reused until an upload
    /// (or consumption) moves past it, and a consumed head heals by skipping
    /// the dead slot instead of failing like WA Web does.
    #[tokio::test]
    async fn single_prekey_is_reused_until_consumed() {
        let client = crate::test_utils::create_test_client_with_name("prekey_single_reuse").await;

        let (id1, pk1) = client.get_or_gen_single_pre_key().await.expect("gen");
        let (id2, pk2) = client.get_or_gen_single_pre_key().await.expect("reuse");
        assert_eq!(id1, id2, "window head must be reused");
        assert_eq!(
            pk1.serialize(),
            pk2.serialize(),
            "same stored key, not a regenerated one"
        );
        let (next, first) = snapshot(&client);
        assert_eq!(first, id1);
        assert_eq!(next, id1 + 1);

        // The peer consumed it via pkmsg: the row is gone.
        backend(&client).remove_prekey(id1).await.expect("remove");
        let (id3, pk3) = client.get_or_gen_single_pre_key().await.expect("heal");
        assert_eq!(id3, id1 + 1, "dead slot skipped, fresh id minted");
        assert_ne!(pk3.serialize(), pk1.serialize());
        let (next, first) = snapshot(&client);
        assert_eq!(first, id3);
        assert_eq!(next, id3 + 1);
    }

    /// WA Web `markKeyAsUploaded`: a retry-distributed prekey must leave the
    /// unuploaded window so the next batch upload does not re-offer it (which
    /// would let a third party consume the same one-time id).
    #[tokio::test]
    async fn marking_retry_prekey_uploaded_excludes_it_from_reuse() {
        let client = crate::test_utils::create_test_client_with_name("prekey_mark_uploaded").await;

        let (id1, _) = client.get_or_gen_single_pre_key().await.expect("gen");
        let (next, first) = snapshot(&client);
        assert_eq!(first, id1);
        assert_eq!(next, id1 + 1);

        let guard = client.prekey_upload_lock.lock().await;
        client
            .mark_single_prekey_uploaded(&guard, id1)
            .await
            .expect("mark");
        drop(guard);
        let (next2, first2) = snapshot(&client);
        assert_eq!(
            first2,
            id1 + 1,
            "low watermark advances past the marked key"
        );
        assert_eq!(next2, id1 + 1, "window is now empty");

        // The next retry key is a fresh id, not a reuse of the marked one.
        let (id2, _) = client.get_or_gen_single_pre_key().await.expect("fresh");
        assert_ne!(id2, id1, "marked key must not be reused");

        // Marking with a now-stale id is a no-op (idempotent, head already moved).
        let guard = client.prekey_upload_lock.lock().await;
        client
            .mark_single_prekey_uploaded(&guard, id1)
            .await
            .expect("noop");
        drop(guard);
        let (_, first3) = snapshot(&client);
        assert_eq!(first3, id2, "stale mark leaves the current head untouched");
    }

    /// Marking a non-terminal head near the 24-bit edge must NOT collapse NEXT:
    /// only advancing past MAX itself wraps. Here the head sits at MAX-1 while
    /// the window still holds MAX; keying the collapse on NEXT alone would drop
    /// the surviving terminal key.
    #[tokio::test]
    async fn marking_near_boundary_head_preserves_terminal_window_key() {
        use wacore::store::commands::DeviceCommand;

        let client =
            crate::test_utils::create_test_client_with_name("prekey_mark_near_boundary").await;
        // Window [MAX-1, MAX+1): holds ids MAX-1 and MAX. NEXT is legitimately
        // out of range (exclusive upper bound past the terminal id).
        client
            .persistence_manager
            .process_command(DeviceCommand::SetPreKeyWatermarks {
                next_pre_key_id: super::MAX_PREKEY_ID + 1,
                first_unupload_pre_key_id: super::MAX_PREKEY_ID - 1,
            })
            .await;

        let guard = client.prekey_upload_lock.lock().await;
        client
            .mark_single_prekey_uploaded(&guard, super::MAX_PREKEY_ID - 1)
            .await
            .expect("mark");
        drop(guard);

        let (next, first) = snapshot(&client);
        assert_eq!(
            first,
            super::MAX_PREKEY_ID,
            "head advances onto the surviving key"
        );
        assert_eq!(
            next,
            super::MAX_PREKEY_ID + 1,
            "NEXT untouched: terminal key MAX is still in the window"
        );
    }

    /// Marking the terminal id itself DOES collapse: with NEXT already pinned at
    /// MAX+1, the window must wrap to an empty low range, not a ~16M span.
    #[tokio::test]
    async fn marking_terminal_head_collapses_wrapped_window() {
        use wacore::store::commands::DeviceCommand;

        let client = crate::test_utils::create_test_client_with_name("prekey_mark_terminal").await;
        client
            .persistence_manager
            .process_command(DeviceCommand::SetPreKeyWatermarks {
                next_pre_key_id: super::MAX_PREKEY_ID + 1,
                first_unupload_pre_key_id: super::MAX_PREKEY_ID,
            })
            .await;

        let guard = client.prekey_upload_lock.lock().await;
        client
            .mark_single_prekey_uploaded(&guard, super::MAX_PREKEY_ID)
            .await
            .expect("mark");
        drop(guard);

        let (next, first) = snapshot(&client);
        assert_eq!(first, 1, "head wraps to the low watermark");
        assert_eq!(
            next, 1,
            "NEXT collapses onto the wrapped head: window empty"
        );
    }

    /// A consumed window head must not abandon the live keys behind it: the
    /// heal advances FIRST to the next stored id and reuses it.
    #[tokio::test]
    async fn consumed_head_advances_to_next_live_window_key() {
        use buffa::Message;
        use wacore::libsignal::protocol::KeyPair;
        use wacore::libsignal::store::record_helpers::new_pre_key_record;
        use wacore::store::commands::DeviceCommand;

        let client = crate::test_utils::create_test_client_with_name("prekey_window_heal").await;
        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let mut publics = std::collections::HashMap::new();
        for id in 10u32..13 {
            let kp = KeyPair::generate(&mut rng);
            publics.insert(id, kp.public_key);
            backend(&client)
                .store_prekey(id, &new_pre_key_record(id, &kp).encode_to_vec(), false)
                .await
                .expect("store");
        }
        client
            .persistence_manager
            .process_command(DeviceCommand::SetPreKeyWatermarks {
                next_pre_key_id: 13,
                first_unupload_pre_key_id: 10,
            })
            .await;

        backend(&client).remove_prekey(10).await.expect("consume");
        let (id, pk) = client.get_or_gen_single_pre_key().await.expect("heal");
        assert_eq!(id, 11, "heal must advance to the next LIVE window key");
        assert_eq!(pk.serialize(), publics[&11].serialize());
        let (next, first) = snapshot(&client);
        assert_eq!(first, 11, "FIRST lands on the surviving key");
        assert_eq!(next, 13, "NEXT untouched: 12 is still in the window");

        // And the key behind it is still reachable afterwards.
        backend(&client).remove_prekey(11).await.expect("consume");
        let (id, _) = client.get_or_gen_single_pre_key().await.expect("heal 2");
        assert_eq!(id, 12);
    }

    /// A fully missing window must collapse AND regenerate within the same
    /// pass: the login path is one-shot, so bailing without minting would
    /// leave the device without prekeys until an unrelated trigger.
    #[tokio::test]
    async fn fully_missing_window_collapses_and_regenerates_in_one_pass() {
        use wacore::store::commands::DeviceCommand;

        let client =
            crate::test_utils::create_test_client_with_name("prekey_window_collapse").await;
        client.set_wanted_pre_key_count(5);
        // Watermarks claim a 5-key window, but nothing is stored (all consumed).
        client
            .persistence_manager
            .process_command(DeviceCommand::SetPreKeyWatermarks {
                next_pre_key_id: 15,
                first_unupload_pre_key_id: 10,
            })
            .await;

        // The IQ still fails (disconnected), but the SAME pass must have
        // collapsed and generated a fresh batch.
        let _ = client.upload_pre_keys_inner().await;
        let (next, first) = snapshot(&client);
        assert_eq!(next, 20, "fresh batch minted at the collapsed NEXT");
        assert_eq!(first, 20, "marked past the window before the send");
        let rows = backend(&client)
            .load_prekeys_batch(&[15, 16, 17, 18, 19])
            .await
            .expect("load");
        assert_eq!(rows.len(), 5, "regeneration happened within the pass");
    }

    /// A retry-receipt single key lives in the unuploaded window, so the next
    /// batch upload re-offers the SAME stored key and only tops up the rest
    /// (WA Web getOrGenPreKeys target-total semantics).
    #[tokio::test]
    async fn upload_window_includes_retry_single_key() {
        let client = crate::test_utils::create_test_client_with_name("prekey_window_topup").await;
        client.set_wanted_pre_key_count(5);

        let (retry_id, retry_pk) = client.get_or_gen_single_pre_key().await.expect("gen");
        let before = backend(&client)
            .load_prekeys_batch(&[retry_id])
            .await
            .expect("load");
        assert_eq!(before.len(), 1);

        let _ = client.upload_pre_keys_inner().await;
        let (next, _) = snapshot(&client);
        assert_eq!(
            next,
            retry_id + 5,
            "only wanted - available new keys are generated"
        );

        let after = backend(&client)
            .load_prekeys_batch(&[retry_id])
            .await
            .expect("load");
        assert_eq!(
            before, after,
            "the retry key is re-offered, not regenerated"
        );
        let window = backend(&client)
            .load_prekeys_batch(&[
                retry_id,
                retry_id + 1,
                retry_id + 2,
                retry_id + 3,
                retry_id + 4,
            ])
            .await
            .expect("load");
        assert_eq!(window.len(), 5, "window = retry key + top-up");

        use buffa::Message;
        let structure =
            waproto::whatsapp::PreKeyRecordStructure::decode_from_slice(&after[0].1[..])
                .expect("decode structure");
        let reloaded = PublicKey::from_djb_public_key_bytes(
            structure.public_key.as_deref().expect("public key"),
        )
        .expect("pub");
        assert_eq!(
            reloaded.serialize(),
            retry_pk.serialize(),
            "stored record matches the key shipped in the receipt"
        );
    }
}
