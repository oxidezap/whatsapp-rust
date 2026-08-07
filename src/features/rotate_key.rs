//! Signed pre-key rotation, mirroring WhatsApp Web's `RotateKeyJob`.
//!
//! The signed pre-key minted at pairing is otherwise permanent. WA Web
//! periodically generates a fresh one, uploads it via an `encrypt` IQ, and
//! retains the old ones so prekey messages already in flight against a
//! previous signed pre-key still decrypt.

use crate::client::{Client, SignalMaintenanceError};
use crate::request::IqError;
use wacore::iq::prekeys::RotateSignedPreKeySpec;
use wacore::libsignal::protocol::{KeyPair, PrivateKey, PublicKey, SignalProtocolError};
use wacore::libsignal::store::record_helpers::new_signed_pre_key_record;
use wacore::store::commands::DeviceCommand;

/// Rotation cadence, matching the interval WA Web's `ROTATE_KEY` task returns to
/// its scheduler. Not configurable: there is no A/B property behind it, so a
/// per-client override would only widen the API for a value the official client
/// hardcodes. `rotate_signed_pre_key()` forces one out of band.
pub(crate) const SIGNED_PRE_KEY_ROTATION_INTERVAL_MS: i64 = 27 * 24 * 60 * 60 * 1000;

/// Retry delay after a transient (>= 500) upload rejection, capped at the
/// cadence by [`rotation_timestamp_after_failed_upload`].
pub(crate) const SIGNED_PRE_KEY_SERVER_ERROR_BACKOFF_MS: i64 = 24 * 60 * 60 * 1000;

/// Total signed pre-keys kept addressable: the current key (device field) plus
/// the RETENTION-1 most recent rotated-out keys in the backend table. With the
/// cadence above a key stays decryptable for RETENTION * interval, and every
/// private key past that is destroyed: WA Web never prunes, which buys interop
/// by keeping every signed pre-key private key on disk forever, and that is the
/// trade this bound refuses.
pub(crate) const SIGNED_PRE_KEY_RETENTION: usize = 3;

/// 24-bit ceiling, matching the one-time prekey id border. Ids advance by one
/// per rotation and wrap back to 1 here.
const MAX_SIGNED_PRE_KEY_ID: u32 = 16_777_215;

/// Wraps a backend failure as [`SignalMaintenanceError::Storage`], keeping the
/// typed cause in the `source()` chain under `context`.
fn storage_err<E>(
    context: impl std::fmt::Display + Send + Sync + 'static,
) -> impl FnOnce(E) -> SignalMaintenanceError
where
    E: std::error::Error + Send + Sync + 'static,
{
    move |e| SignalMaintenanceError::Storage(anyhow::Error::new(e).context(context))
}

/// Whether the cadence has elapsed. `last == 0` means the field predates this
/// feature; the baseline path handles that, so we never rotate on `0`.
pub(crate) fn should_rotate_signed_pre_key(last_rotation_ms: i64, now_ms: i64) -> bool {
    last_rotation_ms != 0
        && now_ms.saturating_sub(last_rotation_ms) >= SIGNED_PRE_KEY_ROTATION_INTERVAL_MS
}

/// Cadence timestamp to persist after a failed upload, or `None` to leave it
/// untouched so the next connect retries immediately.
///
/// A rejection the server actually issued costs a round trip it will charge
/// again on every reconnect, so it consumes the cadence; only a transient fault
/// earns the shorter retry. An error that never reached the server (transport,
/// encode, parse) may still have been accepted, so it stays on the per-connect
/// retry that converges the staged key.
pub(crate) fn rotation_timestamp_after_failed_upload(now_ms: i64, error: &IqError) -> Option<i64> {
    let IqError::ServerError { code, .. } = error else {
        return None;
    };
    let scheduled = if *code < 500 {
        now_ms
    } else {
        // Expressed as a backdated cadence so the retry delay stays a property of
        // the one timestamp `should_rotate_signed_pre_key` reads.
        let retry_in =
            SIGNED_PRE_KEY_SERVER_ERROR_BACKOFF_MS.min(SIGNED_PRE_KEY_ROTATION_INTERVAL_MS);
        now_ms
            .saturating_sub(SIGNED_PRE_KEY_ROTATION_INTERVAL_MS)
            .saturating_add(retry_in)
    };
    // Dodge the exact "cadence never seeded" sentinel, which routes to the
    // baseline path and never rotates. Only 0: clamping the whole negative range
    // would turn a backdated retry into a full interval on a pre-epoch clock.
    Some(if scheduled == 0 { 1 } else { scheduled })
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
            self.persistence_manager
                .flush()
                .await
                .map_err(storage_err("failed to flush rotation baseline"))?;
            return Ok(());
        }

        if should_rotate_signed_pre_key(last, now) {
            self.rotate_signed_pre_key_inner(true).await?;
        }
        Ok(())
    }

    /// Stage a fresh signed pre-key durably, upload it, and only on server
    /// acceptance promote it locally: retain the outgoing key, advance the
    /// current key + cadence, and prune to `SIGNED_PRE_KEY_RETENTION`.
    ///
    /// Both the new candidate and the outgoing key are written to the backend
    /// table *before* upload (the candidate reused verbatim on retry), so every
    /// partial failure is safe: whatever the server ends up advertising, we hold
    /// its private key, and the old id's decrypt window survives regardless. An
    /// ambiguous transport error (the server may have accepted `new_id`) leaves
    /// the staged key decryptable via the load fallback; a definitive rejection
    /// leaves the current key in place, never pruning the key the server still
    /// hands out. A failure here leaves the automatic cadence exactly as it was:
    /// this call did not consult that schedule, so it must not reprogram it.
    /// Calls are serialized with the automatic rotation path.
    pub async fn rotate_signed_pre_key(&self) -> Result<(), SignalMaintenanceError> {
        let _guard = self.signed_pre_key_rotation_lock.lock().await;
        self.rotate_signed_pre_key_inner(false).await
    }

    /// `reschedule_on_failure` belongs to the cadence-driven caller alone; see
    /// [`rotation_timestamp_after_failed_upload`] for what it then writes.
    async fn rotate_signed_pre_key_inner(
        &self,
        reschedule_on_failure: bool,
    ) -> Result<(), SignalMaintenanceError> {
        let snapshot = self.persistence_manager.get_device_snapshot();
        let now = wacore::time::now_millis();
        let backend = self.persistence_manager.backend();

        let old_id = snapshot.signed_pre_key_id;
        let new_id = next_signed_pre_key_id(old_id);

        // Stage the candidate before upload, reusing an already-staged one for
        // this id verbatim. A retry after an ambiguous failure then re-uploads
        // THIS exact key instead of minting a fresh one under the same id, so the
        // key the server may already have accepted is never overwritten/lost.
        let (new_kp, signature) = match backend
            .load_signed_prekey(new_id)
            .await
            .map_err(storage_err("failed to load staged signed pre-key"))?
        {
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
                    // Not CorruptKey: nothing was read back from storage here, so
                    // a wrong width means the signing backend broke its contract.
                    .map_err(|_| {
                        SignalMaintenanceError::Signal(SignalProtocolError::InvalidState(
                            "rotate_signed_pre_key",
                            "Ed25519 signature must be 64 bytes".to_string(),
                        ))
                    })?;
                let record =
                    new_signed_pre_key_record(new_id, &kp, signature, wacore::time::now_utc());
                backend
                    .store_signed_prekey(
                        new_id,
                        &waproto::codec::signed_pre_key_record_to_vec(&record),
                    )
                    .await
                    .map_err(storage_err("failed to stage new signed pre-key"))?;
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
            .map_err(storage_err("failed to retain old signed pre-key"))?;

        // WA Web reads 406 = bad key, 409 = server validation fail, >=500 =
        // transient; none promote the key or fail the automatic login path.
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
                        backend
                            .remove_signed_prekey(new_id)
                            .await
                            .map_err(storage_err(format!(
                                "failed to drop rejected staged signed pre-key {new_id}"
                            )))?;
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
                if let Some(next_rotation_ms) = rotation_timestamp_after_failed_upload(now, &error)
                    .filter(|_| reschedule_on_failure)
                {
                    self.persistence_manager
                        .process_command(DeviceCommand::SetSignedPreKeyRotationBaseline(
                            next_rotation_ms,
                        ))
                        .await;
                    // The command already updated the cached snapshot and left the
                    // store dirty for the background saver, so a failed flush still
                    // holds the delay; only losing the process first reverts to an
                    // immediate retry, which is what not writing it would have done.
                    if let Err(e) = self.persistence_manager.flush().await {
                        log::warn!("failed to persist the signed pre-key rotation schedule: {e}");
                    }
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
        self.persistence_manager
            .flush()
            .await
            .map_err(storage_err("failed to flush rotated signed pre-key"))?;

        // new_id now lives in the device field, so drop its redundant staged copy
        // before pruning to RETENTION total addressable keys (field + RETENTION-1
        // rotated-out). Numeric ordering is safe: ids advance one per rotation, so
        // the wrap at MAX is ~300k years out.
        if let Err(e) = backend.remove_signed_prekey(new_id).await {
            log::warn!("failed to drop staged signed pre-key {new_id}: {e}");
        }
        let mut retained = backend
            .load_all_signed_prekeys()
            .await
            .map_err(storage_err("failed to load retained signed pre-keys"))?;
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

    fn server_error(code: u16) -> IqError {
        IqError::ServerError {
            code,
            text: "test".to_string(),
            error_type: None,
            backoff: None,
        }
    }

    const NOW: i64 = 1_700_000_000_000;

    #[test]
    fn failed_upload_schedule_truth_table() {
        // A verdict the server issued: it will issue the same one on every
        // reconnect, so it consumes the cadence.
        for code in [400, 401, 406, 409, 429, 499] {
            assert_eq!(
                rotation_timestamp_after_failed_upload(NOW, &server_error(code)),
                Some(NOW),
                "code {code} must consume the cadence"
            );
        }

        // Transient server fault: shorter retry, expressed as a backdated
        // cadence so `should_rotate_signed_pre_key` needs no second field.
        let backoff_ts =
            NOW - SIGNED_PRE_KEY_ROTATION_INTERVAL_MS + SIGNED_PRE_KEY_SERVER_ERROR_BACKOFF_MS;
        for code in [500, 503, 599] {
            assert_eq!(
                rotation_timestamp_after_failed_upload(NOW, &server_error(code)),
                Some(backoff_ts),
                "code {code} must schedule the server-error backoff"
            );
        }

        // Never reached the server, or we cannot tell: retry on the next connect.
        for error in [
            IqError::Timeout,
            IqError::NotConnected,
            IqError::InternalChannelClosed,
            IqError::UnexpectedResponseType { got: None },
        ] {
            assert_eq!(
                rotation_timestamp_after_failed_upload(NOW, &error),
                None,
                "{error:?} must leave the cadence untouched"
            );
        }
    }

    /// The delay is a property of the backdated timestamp alone, so it must hold
    /// wherever the clock sits. A clock below one interval makes the schedule
    /// negative, which is exactly why the sentinel guard rejects only 0 rather
    /// than clamping the range.
    #[test]
    fn the_server_error_backoff_defers_the_next_attempt_by_exactly_its_delay() {
        for now in [0, 1, SIGNED_PRE_KEY_ROTATION_INTERVAL_MS, NOW] {
            let scheduled = rotation_timestamp_after_failed_upload(now, &server_error(503))
                .expect("a 5xx schedules a retry");
            assert!(!should_rotate_signed_pre_key(scheduled, now));
            assert!(
                !should_rotate_signed_pre_key(
                    scheduled,
                    now + SIGNED_PRE_KEY_SERVER_ERROR_BACKOFF_MS - 1
                ),
                "must still wait at now={now}"
            );
            assert!(
                should_rotate_signed_pre_key(
                    scheduled,
                    now + SIGNED_PRE_KEY_SERVER_ERROR_BACKOFF_MS
                ),
                "must be due one backoff later at now={now}"
            );
        }
    }

    /// 0 means "cadence never seeded" and routes to the baseline path, which
    /// does not rotate. A schedule that lands on it would disable rotation.
    #[test]
    fn a_scheduled_retry_is_never_the_unseeded_sentinel() {
        for now in [0, 1, SIGNED_PRE_KEY_ROTATION_INTERVAL_MS, NOW] {
            for code in [406, 503] {
                assert_ne!(
                    rotation_timestamp_after_failed_upload(now, &server_error(code)),
                    Some(0)
                );
            }
        }
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

    // ---- Round-trip fixtures for the upload paths ----

    use std::sync::Arc;
    use wacore_binary::builder::NodeBuilder;
    use wacore_binary::node::Node;

    fn iq_result(id: &str) -> Node {
        NodeBuilder::new("iq")
            .attr("id", id)
            .attr("type", "result")
            .build()
    }

    fn iq_error_response(id: &str, code: u16) -> Node {
        NodeBuilder::new("iq")
            .attr("id", id)
            .attr("type", "error")
            .children([NodeBuilder::new("error")
                .attr("code", code.to_string())
                .attr("text", "test-rejection")
                .build()])
            .build()
    }

    /// Backdate the cadence so the next `maybe_rotate_signed_pre_key()` is due.
    async fn make_rotation_due(client: &Arc<Client>) {
        let due = wacore::time::now_millis().saturating_sub(SIGNED_PRE_KEY_ROTATION_INTERVAL_MS);
        client
            .persistence_manager
            .process_command(DeviceCommand::SetSignedPreKeyRotationBaseline(due))
            .await;
        client
            .persistence_manager
            .flush()
            .await
            .expect("persist the due cadence");
    }

    /// Read the `<rotate>` IQ the client wrote as frame `frame` and answer it,
    /// returning the signed pre-key id it carried.
    async fn answer_rotation(
        client: &Arc<Client>,
        transport: &Arc<crate::transport::mock::CapturingMockTransport>,
        frame: usize,
        response: impl FnOnce(&str) -> Node,
    ) -> u32 {
        let iq = crate::test_utils::decode_sent_iq(transport, frame).await;
        let iq = iq.get();
        let skey = iq
            .get_optional_child("rotate")
            .expect("the upload is a <rotate>")
            .get_optional_child("skey")
            .expect("<rotate> carries an <skey>");
        let id_bytes = skey
            .get_optional_child("id")
            .expect("<skey> carries an <id>")
            .content_bytes()
            .expect("the id is raw bytes")
            .to_vec();
        assert_eq!(id_bytes.len(), 3, "the wire id stays 3-byte big-endian");
        let uploaded_id = u32::from_be_bytes([0, id_bytes[0], id_bytes[1], id_bytes[2]]);

        let request_id = iq
            .attrs()
            .optional_string("id")
            .expect("the IQ carries an id")
            .into_owned();
        crate::test_utils::answer_iq(client, &request_id, &response(&request_id)).await;
        uploaded_id
    }

    /// Drive one full successful rotation, returning the promoted id.
    async fn rotate_once(
        client: &Arc<Client>,
        transport: &Arc<crate::transport::mock::CapturingMockTransport>,
        frame: usize,
    ) -> u32 {
        let task = {
            let client = Arc::clone(client);
            tokio::spawn(async move { client.rotate_signed_pre_key().await })
        };
        let uploaded = answer_rotation(client, transport, frame, iq_result).await;
        task.await
            .expect("the rotation task must not panic")
            .expect("the rotation must succeed");
        uploaded
    }

    /// The bug this batch exists for: 202 and 249 logins over 13 days were
    /// observed in production, so an upload failure that leaves the cadence
    /// untouched re-runs the whole stage/retain/upload flow on every one of
    /// them. A transient server fault must buy a real delay instead.
    #[tokio::test]
    async fn a_server_error_upload_does_not_retry_on_the_next_connect() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        make_rotation_due(&client).await;

        let attempt = {
            let client = Arc::clone(&client);
            tokio::spawn(async move { client.maybe_rotate_signed_pre_key().await })
        };
        answer_rotation(&client, &transport, 0, |id| iq_error_response(id, 503)).await;
        let error = attempt
            .await
            .expect("the rotation task must not panic")
            .expect_err("a 503 must surface to the caller");
        assert!(matches!(
            error,
            SignalMaintenanceError::Iq(IqError::ServerError { code: 503, .. })
        ));

        client
            .maybe_rotate_signed_pre_key()
            .await
            .expect("the next connect must find the cadence not due");
        assert_eq!(
            transport.sent_count(),
            1,
            "a reconnect inside the backoff must not upload again"
        );
    }

    /// `rotate_signed_pre_key()` forces a rotation without consulting the
    /// cadence, so a failure there must not reprogram it in either direction: a
    /// 5xx would otherwise drag a schedule weeks out forward to 24h, and a
    /// definitive rejection would push an already-due rotation a full interval
    /// away. Only the cadence-driven caller writes the cadence.
    #[tokio::test]
    async fn a_failed_manual_rotation_leaves_the_automatic_cadence_alone() {
        for code in [503, 406] {
            let (client, transport) = crate::test_utils::create_iq_test_client().await;
            // Not due: one interval away, which a reschedule would visibly move.
            let baseline = wacore::time::now_millis();
            client
                .persistence_manager
                .process_command(DeviceCommand::SetSignedPreKeyRotationBaseline(baseline))
                .await;
            client
                .persistence_manager
                .flush()
                .await
                .expect("persist the baseline");

            let attempt = {
                let client = Arc::clone(&client);
                tokio::spawn(async move { client.rotate_signed_pre_key().await })
            };
            answer_rotation(&client, &transport, 0, |id| iq_error_response(id, code)).await;
            attempt
                .await
                .expect("the rotation task must not panic")
                .expect_err("the rejection must surface to the caller");

            assert_eq!(
                client
                    .persistence_manager
                    .get_device_snapshot()
                    .last_signed_pre_key_rotation_ms,
                baseline,
                "a failed forced rotation (code {code}) must not move the cadence"
            );
        }
    }

    /// A definitive rejection is deterministic: the server will answer the same
    /// way on the next connect, so it consumes the cadence too. The staged key
    /// is still dropped so the retry mints a fresh one.
    #[tokio::test]
    async fn a_rejected_key_is_discarded_and_still_consumes_the_cadence() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let before = client.persistence_manager.get_device_snapshot();
        let staged_id = next_signed_pre_key_id(before.signed_pre_key_id);
        make_rotation_due(&client).await;

        let attempt = {
            let client = Arc::clone(&client);
            tokio::spawn(async move { client.maybe_rotate_signed_pre_key().await })
        };
        answer_rotation(&client, &transport, 0, |id| iq_error_response(id, 406)).await;
        attempt
            .await
            .expect("the rotation task must not panic")
            .expect_err("a 406 must surface to the caller");

        assert!(
            client
                .persistence_manager
                .backend()
                .load_signed_prekey(staged_id)
                .await
                .expect("load the staged id")
                .is_none(),
            "a rejected candidate must not stay staged"
        );
        client
            .maybe_rotate_signed_pre_key()
            .await
            .expect("the next connect must find the cadence not due");
        assert_eq!(
            transport.sent_count(),
            1,
            "a reconnect must not re-upload after a definitive rejection"
        );
    }

    /// A failure that never reached the server is ambiguous: it may have been
    /// accepted, so the staged key must be re-uploaded verbatim on the next
    /// connect rather than sitting out the cadence.
    #[tokio::test]
    async fn an_ambiguous_transport_failure_retries_on_the_next_connect() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let staged_id = next_signed_pre_key_id(
            client
                .persistence_manager
                .get_device_snapshot()
                .signed_pre_key_id,
        );
        make_rotation_due(&client).await;

        let attempt = {
            let client = Arc::clone(&client);
            tokio::spawn(async move { client.maybe_rotate_signed_pre_key().await })
        };
        // Drop the waiter without answering: the request went out and the
        // response channel closes, which is what a dropped connection looks like.
        let iq = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let request_id = iq
            .get()
            .attrs()
            .optional_string("id")
            .expect("the IQ carries an id")
            .into_owned();
        crate::test_utils::poll_until("the IQ waiter to register", || {
            client.response_waiters_guard().contains_key(&request_id)
        })
        .await;
        drop(client.response_waiters_guard().remove(&request_id));
        attempt
            .await
            .expect("the rotation task must not panic")
            .expect_err("a dropped response must surface to the caller");

        assert!(
            client
                .persistence_manager
                .backend()
                .load_signed_prekey(staged_id)
                .await
                .expect("load the staged id")
                .is_some(),
            "the candidate the server may already hold must stay staged"
        );
        let retry = {
            let client = Arc::clone(&client);
            tokio::spawn(async move { client.maybe_rotate_signed_pre_key().await })
        };
        let retried_id = answer_rotation(&client, &transport, 1, iq_result).await;
        retry
            .await
            .expect("the retry task must not panic")
            .expect("the retry must succeed");
        assert_eq!(
            retried_id, staged_id,
            "the retry must re-upload the staged key, not a fresh one"
        );
    }

    /// A successful rotation advances the cadence and leaves the key state
    /// coherent: the id the server now advertises has a local private key, and
    /// the one it replaced is still addressable.
    #[tokio::test]
    async fn a_successful_rotation_advances_the_cadence_and_keeps_both_keys_addressable() {
        use wacore::libsignal::protocol::{GenericSignedPreKey, SignedPreKeyStore};

        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let before = client.persistence_manager.get_device_snapshot();
        let old_id = before.signed_pre_key_id;
        let started = wacore::time::now_millis();

        let promoted = rotate_once(&client, &transport, 0).await;
        assert_eq!(promoted, next_signed_pre_key_id(old_id));

        let after = client.persistence_manager.get_device_snapshot();
        assert_eq!(after.signed_pre_key_id, promoted);
        assert!(
            after.last_signed_pre_key_rotation_ms >= started,
            "a success must advance the cadence"
        );

        let store = client.signal_adapter().signed_pre_key_store;
        let current = store
            .get_signed_pre_key(promoted.into())
            .await
            .expect("the promoted id resolves");
        assert_eq!(
            current.public_key().expect("public"),
            after.signed_pre_key.public_key,
            "the advertised key must be the one we hold the private half of"
        );
        assert!(
            current.private_key().is_ok(),
            "the promoted record must carry its private key"
        );
        assert!(
            store.get_signed_pre_key(old_id.into()).await.is_ok(),
            "the replaced id must stay addressable"
        );
    }

    /// Anchors `SIGNED_PRE_KEY_RETENTION` in behavior rather than opinion: after
    /// enough rotations to fill the window, the oldest key still inside it
    /// resolves and the one that just fell out does not. This is the symptom
    /// from production, reproduced: a peer whose bundle names a key older than
    /// the window gets `InvalidSignedPreKeyId`, and the id reaches a log.
    #[tokio::test]
    async fn the_retention_boundary_decides_which_ids_still_decrypt() {
        use wacore::libsignal::protocol::SignedPreKeyStore;

        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let first_id = client
            .persistence_manager
            .get_device_snapshot()
            .signed_pre_key_id;

        // One rotation past the window: `first_id` is the key that just fell out.
        let mut ids = vec![first_id];
        for frame in 0..SIGNED_PRE_KEY_RETENTION {
            ids.push(rotate_once(&client, &transport, frame).await);
        }

        let store = client.signal_adapter().signed_pre_key_store;
        assert!(
            matches!(
                store.get_signed_pre_key(first_id.into()).await,
                Err(SignalProtocolError::InvalidSignedPreKeyId)
            ),
            "the key one rotation past the window must not resolve"
        );
        for id in &ids[1..] {
            assert!(
                store.get_signed_pre_key((*id).into()).await.is_ok(),
                "id {id} is inside the window and must still decrypt"
            );
        }
        assert_eq!(
            ids.len() - 1,
            SIGNED_PRE_KEY_RETENTION,
            "the window holds exactly RETENTION addressable keys"
        );
    }
}
