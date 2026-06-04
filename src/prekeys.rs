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
    pub(crate) async fn fetch_pre_keys(
        &self,
        jids: &[Jid],
        reason: Option<PreKeyFetchReason>,
    ) -> Result<std::collections::HashMap<Jid, PreKeyBundle>, anyhow::Error> {
        let spec = match reason {
            Some(r) => PreKeyFetchSpec::with_reason(jids.to_vec(), r),
            None => PreKeyFetchSpec::new(jids.to_vec()),
        };

        let bundles = self.execute(spec).await?;

        for jid in bundles.keys() {
            log::debug!("Successfully parsed pre-key bundle for {jid}");
        }

        Ok(bundles)
    }

    /// Query the WhatsApp server for how many pre-keys it currently has for this device.
    pub(crate) async fn get_server_pre_key_count(&self) -> Result<usize, crate::request::IqError> {
        let response = self.execute(PreKeyCountSpec::new()).await?;
        Ok(response.count)
    }

    /// Upload prekeys at login if the persisted flag indicates they're needed.
    /// Matches WA Web's PassiveTasks.js:30 which checks `getServerHasPreKeys()`.
    pub(crate) async fn upload_pre_keys_at_login(&self) -> Result<(), anyhow::Error> {
        let has_prekeys = self
            .persistence_manager
            .get_device_snapshot()
            .await
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
            .await
            .server_has_prekeys
        {
            return Ok(());
        }

        log::info!("Server missing prekeys (persisted flag), uploading.");
        self.upload_pre_keys_inner().await
    }

    /// Ensure the server has enough pre-keys, uploading if below threshold.
    /// When `force` is true, skips the count guard (used by digest key repair).
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

    /// Generate and upload the configured number of pre-keys (see
    /// [`Client::set_wanted_pre_key_count`]). Shared by `upload_pre_keys` and
    /// `upload_pre_keys_at_login` to avoid redundant server count queries.
    async fn upload_pre_keys_inner(&self) -> Result<(), anyhow::Error> {
        let device_snapshot = self.persistence_manager.get_device_snapshot().await;
        let device_store = self.persistence_manager.get_device_arc().await;

        let backend = {
            let device_guard = device_store.read().await;
            device_guard.backend.clone()
        };

        // Use the persistent counter, falling back to max(store_id)+1 for migration.
        // The counter is the source of truth after the first upload.
        let max_id = backend.get_max_prekey_id().await?;
        let raw_start = if device_snapshot.next_pre_key_id > 0 {
            std::cmp::max(device_snapshot.next_pre_key_id, max_id + 1)
        } else {
            log::info!(
                "Migrating pre-key counter: MAX(key_id) in store = {}, starting from {}",
                max_id,
                max_id + 1
            );
            max_id + 1
        };

        // Wrap into valid range so lingering high-ID rows don't pin start_id
        // above the 24-bit boundary and cause repeated overwrites of low IDs.
        let start_id = ((raw_start as u64 - 1) % MAX_PREKEY_ID as u64) as u32 + 1;

        let configured = self.wanted_pre_key_count.load(Ordering::Relaxed);
        let wanted = clamp_wanted_pre_key_count(configured);
        if wanted != configured {
            log::warn!("wanted_pre_key_count {configured} out of range, clamped to {wanted}");
        }

        // Per-key X25519 generation and prost encoding are CPU-bound, and the
        // batch size is caller-configurable, so offload the whole batch to keep
        // the async executor responsive. Records are encoded into one contiguous
        // buffer with zero-copy Bytes slices instead of an alloc per record.
        let (encoded_batch, pre_key_pairs) = wacore::runtime::blocking(&*self.runtime, move || {
            use prost::Message;

            // Seed one CSPRNG and advance it per key, rather than reseeding from
            // entropy on every iteration.
            let mut rng = rand::make_rng::<rand::rngs::StdRng>();
            let mut records = Vec::with_capacity(wanted);
            let mut pre_key_pairs: Vec<(u32, PublicKey)> = Vec::with_capacity(wanted);
            for i in 0..wanted {
                let pre_key_id =
                    (((start_id as u64 - 1) + i as u64) % (MAX_PREKEY_ID as u64)) as u32 + 1;
                let key_pair = KeyPair::generate(&mut rng);
                records.push((pre_key_id, new_pre_key_record(pre_key_id, &key_pair)));
                pre_key_pairs.push((pre_key_id, key_pair.public_key));
            }

            let total_len: usize = records.iter().map(|(_, r)| r.encoded_len()).sum();
            let mut buf = Vec::with_capacity(total_len);
            let mut offsets = Vec::with_capacity(records.len());
            for (id, record) in &records {
                let start = buf.len();
                record
                    .encode(&mut buf)
                    .expect("prost encode into pre-sized Vec");
                offsets.push((*id, start..buf.len()));
            }
            let shared = bytes::Bytes::from(buf);
            let encoded_batch: Vec<(u32, bytes::Bytes)> = offsets
                .into_iter()
                .map(|(id, range)| (id, shared.slice(range)))
                .collect();

            (encoded_batch, pre_key_pairs)
        })
        .await;

        // Persist the freshly generated prekeys before uploading them so they are
        // already available for local decryption if the server starts sending
        // pkmsg traffic immediately after accepting the upload.
        // Propagate errors — uploading a key we can't store locally would cause
        // decryption failures when the server hands it out.
        backend.store_prekeys_batch(&encoded_batch, false).await?;

        let spec = PreKeyUploadSpec::new(
            device_snapshot.registration_id,
            device_snapshot.identity_key.public_key,
            device_snapshot.signed_pre_key_id,
            device_snapshot.signed_pre_key.public_key,
            device_snapshot.signed_pre_key_signature.to_vec(),
            pre_key_pairs,
        );

        self.execute(spec).await?;

        // Mark the uploaded prekeys as server-synced (reuse encoded batch)
        if let Err(e) = backend.store_prekeys_batch(&encoded_batch, true).await {
            log::warn!("Failed to mark prekeys as uploaded: {:?}", e);
        }

        // IDs wrap modulo MAX_PREKEY_ID. If the counter wraps while unconsumed
        // high-ID prekeys still exist, the upsert (.on_conflict.do_update)
        // silently overwrites them. Acceptable: the server consumes keys well
        // before a full 16M cycle completes.
        let next_id = (((start_id as u64 - 1) + wanted as u64) % (MAX_PREKEY_ID as u64)) as u32 + 1;
        self.persistence_manager
            .process_command(DeviceCommand::SetNextPreKeyId(next_id))
            .await;

        // Persist flag matching WA Web's setServerHasPreKeys(true) (PreKeysJob.js:79)
        self.persistence_manager
            .modify_device(|d| d.server_has_prekeys = true)
            .await;

        log::debug!(
            "Successfully uploaded {} new pre-keys with sequential IDs starting from {}.",
            wanted,
            start_id
        );

        Ok(())
    }

    /// Upload pre-keys with Fibonacci retry backoff matching WA Web's `PromiseRetryLoop`.
    ///
    /// Retry schedule: 1s, 2s, 3s, 5s, 8s, 13s, ... capped at 610s.
    /// Verified against WA Web JS: `{ algo: { type: "fibonacci", first: 1e3, second: 2e3 }, max: 61e4 }`
    ///
    /// When `force` is true, bypasses the count guard (used by digest repair path).
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
        let device_snapshot = self.persistence_manager.get_device_snapshot().await;
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

        let device_store = self.persistence_manager.get_device_arc().await;
        let backend = {
            let guard = device_store.read().await;
            guard.backend.clone()
        };

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
        DEFAULT_WANTED_PRE_KEY_COUNT, MAX_PRE_KEY_UPLOAD_BATCH, MIN_PRE_KEY_COUNT,
        clamp_wanted_pre_key_count, should_upload_pre_keys,
    };

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
