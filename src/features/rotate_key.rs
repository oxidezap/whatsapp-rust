//! Signed pre-key rotation, mirroring WhatsApp Web's `RotateKeyJob`.
//!
//! The signed pre-key minted at pairing is otherwise permanent. WA Web
//! periodically generates a fresh one, uploads it via an `encrypt` IQ, and
//! retains the old ones so prekey messages already in flight against a
//! previous signed pre-key still decrypt.

use crate::client::{Client, SignalMaintenanceError};
use crate::request::IqError;
use wacore::iq::prekeys::RotateSignedPreKeySpec;
use wacore::libsignal::protocol::{KeyPair, PrivateKey, PublicKey};
use wacore::libsignal::store::record_helpers::new_signed_pre_key_record;
use wacore::store::commands::DeviceCommand;

/// Rotation cadence. This is the one value NOT grounded in the WA Web bundle
/// (there it is a persisted background job with a server-tuned schedule), so
/// treat it as a policy default that is safe to tune.
pub(crate) const SIGNED_PRE_KEY_ROTATION_INTERVAL_MS: i64 = 7 * 24 * 60 * 60 * 1000; // weekly

/// Total signed pre-keys kept addressable: the current key (device field) plus
/// the RETENTION-1 most recent rotated-out keys in the backend table. Bounds
/// the decrypt window for delayed prekey messages built against a rotated key.
pub(crate) const SIGNED_PRE_KEY_RETENTION: usize = 3;

/// 24-bit ceiling, matching the one-time prekey id border. Ids advance by one
/// per rotation and wrap back to 1 here.
const MAX_SIGNED_PRE_KEY_ID: u32 = 16_777_215;

/// Whether the cadence has elapsed. `last == 0` means the field predates this
/// feature; the baseline path handles that, so we never rotate on `0`.
pub(crate) fn should_rotate_signed_pre_key(last_rotation_ms: i64, now_ms: i64) -> bool {
    last_rotation_ms != 0
        && now_ms.saturating_sub(last_rotation_ms) >= SIGNED_PRE_KEY_ROTATION_INTERVAL_MS
}

/// Next id = current + 1, wrapping at the 24-bit border back to 1.
pub(crate) fn next_signed_pre_key_id(current: u32) -> u32 {
    if current >= MAX_SIGNED_PRE_KEY_ID {
        1
    } else {
        current + 1
    }
}

impl Client {
    /// Rotate the signed pre-key if the cadence has elapsed. Seeds the cadence
    /// baseline (without rotating) for devices upgraded in with the field at 0.
    pub(crate) async fn maybe_rotate_signed_pre_key(&self) -> Result<(), SignalMaintenanceError> {
        // Single-flight: a concurrent rotation (e.g. an older post-login task
        // racing a newer one across reconnect churn) already covers this cadence,
        // so skip rather than run the rotate/upload/prune flow twice.
        let Some(_guard) = self.signed_pre_key_rotation_lock.try_lock() else {
            return Ok(());
        };

        let last = self
            .persistence_manager
            .get_device_snapshot()
            .last_signed_pre_key_rotation_ms;
        let now = wacore::time::now_millis();

        if last == 0 {
            self.persistence_manager
                .process_command(DeviceCommand::SetSignedPreKeyRotationBaseline(now))
                .await;
            self.persistence_manager.flush().await.map_err(|e| {
                SignalMaintenanceError::Storage(
                    anyhow::Error::new(e).context("failed to flush rotation baseline"),
                )
            })?;
            return Ok(());
        }

        if should_rotate_signed_pre_key(last, now) {
            self.rotate_signed_pre_key_inner().await?;
        }
        Ok(())
    }

    /// Stage a fresh signed pre-key durably, upload it, and only on server
    /// acceptance promote it locally: retain the outgoing key, advance the
    /// current key + cadence, and prune to [`SIGNED_PRE_KEY_RETENTION`].
    ///
    /// Both the new candidate and the outgoing key are written to the backend
    /// table *before* upload (the candidate reused verbatim on retry), so every
    /// partial failure is safe: whatever the server ends up advertising, we hold
    /// its private key, and the old id's decrypt window survives regardless. An
    /// ambiguous transport error (the server may have accepted `new_id`) leaves
    /// the staged key decryptable via the load fallback; a definitive rejection
    /// just leaves the current key in place to retry — never advancing the
    /// cadence or pruning the key the server still hands out. Calls are
    /// serialized with the automatic rotation path.
    pub async fn rotate_signed_pre_key(&self) -> Result<(), SignalMaintenanceError> {
        let _guard = self.signed_pre_key_rotation_lock.lock().await;
        self.rotate_signed_pre_key_inner().await
    }

    async fn rotate_signed_pre_key_inner(&self) -> Result<(), SignalMaintenanceError> {
        let snapshot = self.persistence_manager.get_device_snapshot();
        let now = wacore::time::now_millis();
        let backend = self.persistence_manager.backend();

        let old_id = snapshot.signed_pre_key_id;
        let new_id = next_signed_pre_key_id(old_id);

        // Stage the candidate before upload, reusing an already-staged one for
        // this id verbatim. A retry after an ambiguous failure then re-uploads
        // THIS exact key instead of minting a fresh one under the same id, so the
        // key the server may already have accepted is never overwritten/lost.
        let (new_kp, signature) = match backend.load_signed_prekey(new_id).await.map_err(|e| {
            SignalMaintenanceError::Storage(
                anyhow::Error::new(e).context("failed to load staged signed pre-key"),
            )
        })? {
            Some(bytes) => {
                let s = waproto::codec::signed_pre_key_record_decode(&bytes).map_err(|e| {
                    SignalMaintenanceError::CorruptKey(format!("staged record decode: {e}"))
                })?;
                let public = PublicKey::from_djb_public_key_bytes(s.public_key.as_deref().ok_or(
                    SignalMaintenanceError::CorruptKey("staged record missing public".to_string()),
                )?)
                .map_err(|e| {
                    SignalMaintenanceError::CorruptKey(format!("staged record public key: {e}"))
                })?;
                let private = PrivateKey::deserialize(s.private_key.as_deref().ok_or(
                    SignalMaintenanceError::CorruptKey("staged record missing private".to_string()),
                )?)
                .map_err(|e| {
                    SignalMaintenanceError::CorruptKey(format!("staged record private key: {e}"))
                })?;
                let signature: [u8; 64] = s
                    .signature
                    .ok_or(SignalMaintenanceError::CorruptKey(
                        "staged record missing signature".to_string(),
                    ))?
                    .try_into()
                    .map_err(|_| {
                        SignalMaintenanceError::CorruptKey(
                            "staged signature must be 64 bytes".to_string(),
                        )
                    })?;
                (KeyPair::new(public, private), signature)
            }
            None => {
                let mut rng = rand::make_rng::<rand::rngs::StdRng>();
                let kp = KeyPair::generate(&mut rng);
                // Sign the new public with the identity private key over the
                // serialized (not raw) public bytes, matching Device::new().
                let signature: [u8; 64] = snapshot
                    .identity_key
                    .private_key
                    .calculate_signature(&kp.public_key.serialize(), &mut rng)
                    .map_err(|e| SignalMaintenanceError::Signal(e.into()))?
                    .as_ref()
                    .try_into()
                    .map_err(|_| {
                        SignalMaintenanceError::CorruptKey(
                            "Ed25519 signature must be 64 bytes".to_string(),
                        )
                    })?;
                let record =
                    new_signed_pre_key_record(new_id, &kp, signature, wacore::time::now_utc());
                backend
                    .store_signed_prekey(
                        new_id,
                        &waproto::codec::signed_pre_key_record_to_vec(&record),
                    )
                    .await
                    .map_err(|e| {
                        SignalMaintenanceError::Storage(
                            anyhow::Error::new(e).context("failed to stage new signed pre-key"),
                        )
                    })?;
                (kp, signature)
            }
        };

        // Retain the outgoing key BEFORE upload, so once the server accepts the
        // new key the old id's decrypt window is already durable — no
        // post-acceptance write can strand it. Required: on failure we abort
        // before sending anything, leaving the current key fully intact to retry.
        let old_record = new_signed_pre_key_record(
            old_id,
            &snapshot.signed_pre_key,
            snapshot.signed_pre_key_signature,
            wacore::time::now_utc(),
        );
        backend
            .store_signed_prekey(
                old_id,
                &waproto::codec::signed_pre_key_record_to_vec(&old_record),
            )
            .await
            .map_err(|e| {
                SignalMaintenanceError::Storage(
                    anyhow::Error::new(e).context("failed to retain old signed pre-key"),
                )
            })?;

        // WA Web reads 406 = bad key, 409 = server validation fail, >=500 =
        // transient; none advance local state or fail the automatic login path.
        // Deterministic rejections discard the candidate; retryable and
        // ambiguous failures retain it verbatim for the next attempt.
        let upload_result = self
            .execute(RotateSignedPreKeySpec::new(
                new_id,
                new_kp.public_key,
                signature.to_vec(),
            ))
            .await;
        match upload_result {
            Ok(()) => {}
            Err(error) => {
                if let IqError::ServerError { code, text, .. } = &error {
                    // WA Web treats 406 (bad key) and 409 (validation fail) as
                    // deterministic rejections of THIS key; reusing the staged
                    // candidate on retry would then wedge rotation forever (old_id
                    // never advances, so new_id is recomputed the same). Drop it to
                    // force a fresh mint — and REQUIRE the cleanup: if the remove
                    // fails, propagate so we never silently leave the rejected key
                    // staged. Every other code (rate limits, transient 5xx, …) is
                    // retryable, so keep the staged key for a plain retry.
                    if *code == 406 || *code == 409 {
                        backend.remove_signed_prekey(new_id).await.map_err(|e| {
                            SignalMaintenanceError::Storage(anyhow::Error::new(e).context(format!(
                                "failed to drop rejected staged signed pre-key {new_id}"
                            )))
                        })?;
                        log::warn!(
                            "signed pre-key rotation rejected (code={code}, text='{text}'); \
                             discarded the rejected key, will remint on a later connect"
                        );
                    } else {
                        log::warn!(
                            "signed pre-key rotation upload rejected (code={code}, text='{text}'); \
                             keeping the staged key, will retry on a later connect"
                        );
                    }
                } else {
                    // Ambiguous transport failure: the server may have accepted the
                    // key, so keep the staged candidate and reuse it on retry.
                    log::warn!(
                        "signed pre-key rotation upload failed: {error:?}; \
                         keeping the staged key, will retry on a later connect"
                    );
                }
                return Err(error.into());
            }
        }

        // Server accepted new_id, and both the old (retained) and new (staged)
        // keys are already durable, so promotion cannot strand either.
        self.persistence_manager
            .process_command(DeviceCommand::SetSignedPreKey {
                key_pair: new_kp,
                id: new_id,
                signature,
                rotation_ms: now,
            })
            .await;
        self.persistence_manager.flush().await.map_err(|e| {
            SignalMaintenanceError::Storage(
                anyhow::Error::new(e).context("failed to flush rotated signed pre-key"),
            )
        })?;

        // new_id now lives in the device field, so drop its redundant staged copy
        // before pruning to RETENTION total addressable keys (field + RETENTION-1
        // rotated-out). Numeric ordering is safe: ids advance one per rotation, so
        // the wrap at MAX is ~300k years out.
        if let Err(e) = backend.remove_signed_prekey(new_id).await {
            log::warn!("failed to drop staged signed pre-key {new_id}: {e}");
        }
        let mut retained = backend.load_all_signed_prekeys().await.map_err(|e| {
            SignalMaintenanceError::Storage(
                anyhow::Error::new(e).context("failed to load retained signed pre-keys"),
            )
        })?;
        retained.sort_unstable_by_key(|(id, _)| std::cmp::Reverse(*id));
        for (id, _) in retained
            .into_iter()
            .skip(SIGNED_PRE_KEY_RETENTION.saturating_sub(1))
        {
            if let Err(e) = backend.remove_signed_prekey(id).await {
                log::warn!("failed to prune retained signed pre-key {id}: {e}");
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn should_rotate_truth_table() {
        // last == 0 never rotates (baseline path owns it).
        assert!(!should_rotate_signed_pre_key(0, i64::MAX));

        let last = 1_000_000_000_000;
        // Just before the interval: no rotation.
        assert!(!should_rotate_signed_pre_key(
            last,
            last + SIGNED_PRE_KEY_ROTATION_INTERVAL_MS - 1
        ));
        // Exactly at the boundary: rotate.
        assert!(should_rotate_signed_pre_key(
            last,
            last + SIGNED_PRE_KEY_ROTATION_INTERVAL_MS
        ));
        // Well past: rotate.
        assert!(should_rotate_signed_pre_key(
            last,
            last + SIGNED_PRE_KEY_ROTATION_INTERVAL_MS * 3
        ));
        // Clock skew backwards: saturating_sub yields 0, no rotation.
        assert!(!should_rotate_signed_pre_key(last, last - 1));
    }

    #[test]
    fn next_id_increments_and_wraps() {
        assert_eq!(next_signed_pre_key_id(1), 2);
        assert_eq!(next_signed_pre_key_id(41), 42);
        assert_eq!(
            next_signed_pre_key_id(MAX_SIGNED_PRE_KEY_ID - 1),
            MAX_SIGNED_PRE_KEY_ID
        );
        // At and beyond the 24-bit border, wrap back to 1.
        assert_eq!(next_signed_pre_key_id(MAX_SIGNED_PRE_KEY_ID), 1);
    }

    #[tokio::test]
    async fn public_rotation_uses_the_single_flight_lock() {
        let client = crate::test_utils::create_test_client().await;
        let guard = client.signed_pre_key_rotation_lock.lock().await;

        assert!(
            tokio::time::timeout(
                std::time::Duration::from_millis(100),
                client.rotate_signed_pre_key(),
            )
            .await
            .is_err(),
            "manual rotation must serialize with an active rotation"
        );
        drop(guard);
    }

    #[tokio::test]
    async fn due_automatic_rotation_does_not_relock_its_single_flight_guard() {
        let client = crate::test_utils::create_test_client().await;
        let snapshot = client.persistence_manager.get_device_snapshot();
        let staged_id = next_signed_pre_key_id(snapshot.signed_pre_key_id);
        let due_baseline =
            wacore::time::now_millis().saturating_sub(SIGNED_PRE_KEY_ROTATION_INTERVAL_MS);
        client
            .persistence_manager
            .process_command(DeviceCommand::SetSignedPreKeyRotationBaseline(due_baseline))
            .await;
        client
            .persistence_manager
            .flush()
            .await
            .expect("persist due rotation baseline");

        let error = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            client.maybe_rotate_signed_pre_key(),
        )
        .await
        .expect("automatic rotation must not recursively acquire its held lock")
        .expect_err("the disconnected test client must fail at upload");
        assert!(matches!(
            error,
            SignalMaintenanceError::Iq(IqError::NotConnected)
        ));
        assert!(
            client
                .persistence_manager
                .backend()
                .load_signed_prekey(staged_id)
                .await
                .expect("load staged signed pre-key")
                .is_some(),
            "the due path must reach the inner rotation flow before upload"
        );
    }

    #[tokio::test]
    async fn public_rotation_reports_upload_failure() {
        let client = crate::test_utils::create_test_client().await;

        let error = client
            .rotate_signed_pre_key()
            .await
            .expect_err("manual rotation must report a failed upload");
        assert!(matches!(
            error,
            SignalMaintenanceError::Iq(IqError::NotConnected)
        ));
    }

    #[tokio::test]
    async fn rotation_reports_an_unusable_staged_key_as_corrupt() {
        let client = crate::test_utils::create_test_client().await;
        let snapshot = client.persistence_manager.get_device_snapshot();
        let staged_id = next_signed_pre_key_id(snapshot.signed_pre_key_id);
        // A record that decodes but carries no key material: rereading it would
        // yield the same bytes, so it is a corruption, not a storage failure.
        client
            .persistence_manager
            .backend()
            .store_signed_prekey(staged_id, &[])
            .await
            .expect("stage an empty signed pre-key record");

        let error = client
            .rotate_signed_pre_key()
            .await
            .expect_err("an unusable staged key must abort the rotation");
        assert!(matches!(error, SignalMaintenanceError::CorruptKey(_)));
    }
}
