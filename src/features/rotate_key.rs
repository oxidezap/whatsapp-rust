//! Signed pre-key rotation, mirroring WhatsApp Web's `RotateKeyJob`.
//!
//! The signed pre-key minted at pairing is otherwise permanent. WA Web
//! periodically generates a fresh one, uploads it via an `encrypt` IQ, and
//! retains the old ones so prekey messages already in flight against a
//! previous signed pre-key still decrypt.

use crate::client::Client;
use crate::request::IqError;
use buffa::Message;
use wacore::iq::prekeys::RotateSignedPreKeySpec;
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
    pub(crate) async fn maybe_rotate_signed_pre_key(&self) -> Result<(), anyhow::Error> {
        let last = self
            .persistence_manager
            .get_device_snapshot()
            .last_signed_pre_key_rotation_ms;
        let now = wacore::time::now_millis();

        if last == 0 {
            self.persistence_manager
                .process_command(DeviceCommand::SetSignedPreKeyRotationBaseline(now))
                .await;
            self.persistence_manager
                .flush()
                .await
                .map_err(|e| anyhow::anyhow!("failed to flush rotation baseline: {e:?}"))?;
            return Ok(());
        }

        if should_rotate_signed_pre_key(last, now) {
            self.rotate_signed_pre_key().await?;
        }
        Ok(())
    }

    /// Generate a fresh signed pre-key, upload it, and only then switch to it
    /// locally: retain the outgoing key so in-flight prekey messages still
    /// decrypt, promote the new key, stamp the cadence, and prune to
    /// [`SIGNED_PRE_KEY_RETENTION`].
    ///
    /// Upload-first is deliberate. The server keeps advertising the previous
    /// signed pre-key until it accepts the new one, so switching or advancing
    /// the cadence before the server accepts would let a failed upload both skip
    /// retries for a full interval AND, across repeated failures, prune the key
    /// the server is still handing out — breaking new prekey sessions.
    pub(crate) async fn rotate_signed_pre_key(&self) -> Result<(), anyhow::Error> {
        let snapshot = self.persistence_manager.get_device_snapshot();
        let now = wacore::time::now_millis();

        let mut rng = rand::make_rng::<rand::rngs::StdRng>();
        let new_kp = wacore::libsignal::protocol::KeyPair::generate(&mut rng);
        // Sign the new public with the identity private key over the serialized
        // (not raw) public bytes, matching Device::new().
        let signature: [u8; 64] = snapshot
            .identity_key
            .private_key
            .calculate_signature(&new_kp.public_key.serialize(), &mut rng)?
            .as_ref()
            .try_into()
            .map_err(|_| anyhow::anyhow!("Ed25519 signature must be 64 bytes"))?;

        let old_id = snapshot.signed_pre_key_id;
        let new_id = next_signed_pre_key_id(old_id);

        // Upload before any local mutation. WA Web reads 406 = bad key, 409 =
        // server validation fail, >=500 = transient; none warrant hard-failing
        // login, and none should advance our state — a later connect retries.
        match self
            .execute(RotateSignedPreKeySpec::new(
                new_id,
                new_kp.public_key,
                signature.to_vec(),
            ))
            .await
        {
            Ok(()) => {}
            Err(IqError::ServerError { code, text, .. }) => {
                log::warn!(
                    "signed pre-key rotation upload rejected (code={code}, text='{text}'); \
                     keeping the current key, will retry on a later connect"
                );
                return Ok(());
            }
            Err(e) => {
                log::warn!(
                    "signed pre-key rotation upload failed: {e:?}; \
                     keeping the current key, will retry on a later connect"
                );
                return Ok(());
            }
        }

        // Server accepted new_id. Retain the outgoing key so prekey messages
        // naming the old id still decrypt, then promote the new key + stamp.
        let backend = self.persistence_manager.backend();
        let old_record = new_signed_pre_key_record(
            old_id,
            &snapshot.signed_pre_key,
            snapshot.signed_pre_key_signature,
            wacore::time::now_utc(),
        );
        // Best-effort: the server already accepted new_id, so promotion must
        // proceed even if retaining the old key fails — otherwise local state
        // stays on a key the server no longer advertises and new prekey sessions
        // break. A failed retain only shortens the old key's decrypt window.
        if let Err(e) = backend
            .store_signed_prekey(old_id, &old_record.encode_to_vec())
            .await
        {
            log::warn!(
                "failed to retain old signed pre-key {old_id}: {e}; \
                 continuing with server-accepted signed pre-key {new_id}"
            );
        }

        self.persistence_manager
            .process_command(DeviceCommand::SetSignedPreKey {
                key_pair: new_kp,
                id: new_id,
                signature,
                rotation_ms: now,
            })
            .await;
        self.persistence_manager
            .flush()
            .await
            .map_err(|e| anyhow::anyhow!("failed to flush rotated signed pre-key: {e:?}"))?;

        // Prune to RETENTION total addressable keys. The current key lives in the
        // device field (never the backend table), so the backend keeps only the
        // RETENTION-1 most recent rotated-out keys. Numeric ordering is safe: ids
        // advance one per rotation, so the wrap at MAX is ~300k years out.
        let mut retained = backend
            .load_all_signed_prekeys()
            .await
            .map_err(|e| anyhow::anyhow!("failed to load retained signed pre-keys: {e}"))?;
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
}
