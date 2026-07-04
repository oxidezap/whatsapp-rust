//! Pair code authentication for phone number linking.
//!
//! This module provides an alternative to QR code pairing. Users enter an
//! 8-character code on their phone instead of scanning a QR code.
//!
//! # Usage
//!
//! ## Random Code (Default)
//!
//! ```rust,no_run
//! use whatsapp_rust::pair_code::PairCodeOptions;
//!
//! # async fn example(client: std::sync::Arc<whatsapp_rust::Client>) -> Result<(), Box<dyn std::error::Error>> {
//! let options = PairCodeOptions {
//!     phone_number: "15551234567".to_string(),
//!     ..Default::default()
//! };
//! let code = client.pair_with_code(options).await?;
//! println!("Enter this code on your phone: {}", code);
//! # Ok(())
//! # }
//! ```
//!
//! ## Custom Pairing Code
//!
//! You can specify your own 8-character code using Crockford Base32 alphabet
//! (characters: `123456789ABCDEFGHJKLMNPQRSTVWXYZ` - excludes 0, I, O, U):
//!
//! ```rust,no_run
//! use whatsapp_rust::pair_code::PairCodeOptions;
//!
//! # async fn example(client: std::sync::Arc<whatsapp_rust::Client>) -> Result<(), Box<dyn std::error::Error>> {
//! let options = PairCodeOptions {
//!     phone_number: "15551234567".to_string(),
//!     custom_code: Some("MYCODE12".to_string()), // Must be exactly 8 valid chars
//!     ..Default::default()
//! };
//! let code = client.pair_with_code(options).await?;
//! assert_eq!(code, "MYCODE12");
//! # Ok(())
//! # }
//! ```
//!
//! ## Concurrent with QR Codes
//!
//! Pair code and QR code can run simultaneously. Whichever completes first wins.

use crate::client::Client;
use crate::request::{InfoQuery, InfoQueryType, IqError};
use crate::types::events::Event;
use log::{error, info, warn};

use std::sync::Arc;
use wacore::libsignal::protocol::KeyPair;
use wacore::pair_code::{PairCodeState, PairCodeUtils, resolve_companion_platform};
use wacore_binary::Jid;
use wacore_binary::{NodeContent, NodeContentRef, NodeRef};

pub use wacore::companion_reg::{CompanionOs, CompanionWebClientType};
pub use wacore::pair_code::{PairCodeError, PairCodeOptions};

/// Errors raised by the high-level pair-code flow.
///
/// Wraps `wacore::pair_code::PairCodeError` (validation, key derivation, bundle
/// building) and adds the IQ transport layer via `RequestFailed`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PairError {
    #[error(transparent)]
    PairCode(#[from] PairCodeError),

    /// The pair-code IQ was rejected by the server.
    ///
    /// Note the server returns `bad-request` (400) **both** for genuinely invalid
    /// content and for **rate-limiting** — it throttles pair-code requests per
    /// phone number and reuses the same error. So a 400 here is not necessarily a
    /// permanent/invalid-input failure: back off and retry rather than treating
    /// every 400 as fatal. (The lib canonicalizes the `companion_platform_display`
    /// OS, so a display-shaped rejection is already ruled out — see
    /// [`wacore::companion_reg::CompanionOs`].) Any server `backoff` hint is
    /// preserved on the wrapped [`IqError`].
    #[error("pair-code IQ request failed")]
    RequestFailed(#[from] IqError),
}

impl Client {
    /// Initiates pair code authentication as an alternative to QR code pairing.
    ///
    /// This method starts the phone number linking process. The returned code should
    /// be displayed to the user, who then enters it on their phone in:
    /// **WhatsApp > Linked Devices > Link a Device > Link with phone number instead**
    ///
    /// This can run concurrently with QR code pairing - whichever completes first wins.
    ///
    /// # Arguments
    ///
    /// * `options` - Configuration for pair code authentication
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The 8-character pairing code to display
    /// * `Err` - If validation fails, not connected, or server error. A
    ///   [`PairError::RequestFailed`] carrying `bad-request` may be **rate-limiting**
    ///   (throttled per phone number), not invalid input — back off and retry.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// use whatsapp_rust::pair_code::PairCodeOptions;
    ///
    /// # async fn example(client: std::sync::Arc<whatsapp_rust::Client>) -> Result<(), Box<dyn std::error::Error>> {
    /// let options = PairCodeOptions {
    ///     phone_number: "15551234567".to_string(),
    ///     show_push_notification: true,
    ///     custom_code: None, // Generate random code
    ///     ..Default::default()
    /// };
    ///
    /// let code = client.pair_with_code(options).await?;
    /// println!("Enter this code on your phone: {}", code);
    /// # Ok(())
    /// # }
    /// ```
    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.pair.code", level = "debug", skip_all, err(Debug))
    )]
    pub async fn pair_with_code(
        self: &Arc<Self>,
        options: PairCodeOptions,
    ) -> Result<String, PairError> {
        // Strip non-digit characters from phone number (allows "+1-555-123-4567" format)
        let phone_number: String = options
            .phone_number
            .chars()
            .filter(|c| c.is_ascii_digit())
            .collect();

        // Validate phone number
        if phone_number.is_empty() {
            return Err(PairCodeError::PhoneNumberRequired.into());
        }
        if phone_number.len() < 7 {
            return Err(PairCodeError::PhoneNumberTooShort.into());
        }
        if phone_number.starts_with('0') {
            return Err(PairCodeError::PhoneNumberNotInternational.into());
        }

        // Generate or validate code
        let code = match &options.custom_code {
            Some(custom) => {
                if !PairCodeUtils::validate_code(custom) {
                    return Err(PairCodeError::InvalidCustomCode.into());
                }
                custom.to_uppercase()
            }
            None => PairCodeUtils::generate_code(),
        };

        info!(
            target: "Client/PairCode",
            "Starting pair code authentication for phone: {}",
            phone_number
        );

        // Stamp the validity clock before companion_hello, matching WA Web
        // (`startAltLinkingFlow` sets codeGenerationTs before sending), so the
        // ~180s window covers the stage-1 round-trip rather than starting after it.
        let code_generation_ts = wacore::time::now_secs();

        // Generate ephemeral keypair for this pairing session
        let ephemeral_keypair = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());

        // Get device state for noise key
        let device_snapshot = self.persistence_manager.get_device_snapshot();
        let noise_static_pub: [u8; 32] = device_snapshot
            .noise_key
            .public_key
            .public_key_bytes()
            .try_into()
            .expect("noise key is 32 bytes");

        // Derive key and encrypt ephemeral pub (expensive PBKDF2 operation)
        // Run in spawn_blocking to avoid stalling the async runtime
        let code_clone = code.clone();
        let ephemeral_pub: [u8; 32] = ephemeral_keypair
            .public_key
            .public_key_bytes()
            .try_into()
            .expect("ephemeral key is 32 bytes");

        let wrapped_ephemeral = wacore::runtime::blocking(&*self.runtime, move || {
            PairCodeUtils::encrypt_ephemeral_pub(&ephemeral_pub, &code_clone)
        })
        .await;

        let (platform_id, platform_display) =
            resolve_companion_platform(&options, &device_snapshot.device_props);
        let platform_id_str = platform_id.to_string();

        // Warn when a branding `DeviceProps::os` gets coerced to "Linux", so a
        // consumer sees why it didn't ride through (the pair-code server rejects a
        // non-OS display with bad-request; QR never sends this field). Skipped under
        // a `display_os` override. Once-per-process: retries (PairError::RequestFailed
        // is rate-limitable) reuse the same os, so repeating the warning is just noise.
        static OS_COERCE_WARNED: std::sync::Once = std::sync::Once::new();
        let os_overridden = options
            .display_os
            .as_deref()
            .is_some_and(|o| !o.trim().is_empty());
        if !os_overridden
            && let Some(os) = device_snapshot.device_props.os.as_deref()
            && !os.trim().is_empty()
            && CompanionOs::classify(os).is_none()
        {
            OS_COERCE_WARNED.call_once(|| {
                warn!(
                    target: "Client/PairCode",
                    "companion_platform_display OS {os:?} is not a recognized OS; coerced to \"Linux\" for pair-code (the server would reject a non-OS display with bad-request)"
                );
            });
        }

        let req_id = self.generate_request_id();
        let iq_content = PairCodeUtils::build_companion_hello_iq(
            &phone_number,
            &noise_static_pub,
            &wrapped_ephemeral,
            &platform_id_str,
            &platform_display,
            options.show_push_notification,
            req_id.clone(),
        );

        // Send the IQ and wait for response using the standard send_iq method
        let query = InfoQuery {
            query_type: InfoQueryType::Set,
            namespace: "md",
            to: Jid::new("", wacore_binary::Server::Pn),
            target: None,
            content: Some(NodeContent::Nodes(
                iq_content
                    .children()
                    .map(|c| c.to_vec())
                    .unwrap_or_default(),
            )),
            id: Some(req_id),
            timeout: Some(std::time::Duration::from_secs(30)),
        };

        let response = self.send_iq(query).await?;

        let pairing_ref = PairCodeUtils::parse_companion_hello_response(response.get())
            .ok_or(PairCodeError::MissingPairingRef)?;

        info!(
            target: "Client/PairCode",
            "Stage 1 complete, waiting for phone confirmation. Code: {}",
            code
        );

        // Store state for when phone confirms
        *self.pair_code_state.lock().await = PairCodeState::WaitingForPhoneConfirmation {
            pairing_ref,
            phone_jid: phone_number,
            pair_code: code.clone(),
            ephemeral_keypair: Box::new(ephemeral_keypair),
            code_generation_ts,
            primary_hello_attempt_count: 0,
        };

        // Dispatch event for the user to display the code. The validity clock
        // started at `code_generation_ts` (before stage 1), so advertise the
        // *remaining* window — otherwise a consumer's countdown would outlast the
        // server's (and our own `handle_primary_hello`) expiry by the stage-1
        // elapsed time.
        let elapsed = wacore::time::now_secs()
            .saturating_sub(code_generation_ts)
            .max(0) as u64;
        let remaining =
            PairCodeUtils::code_validity().saturating_sub(std::time::Duration::from_secs(elapsed));
        self.core.event_bus.dispatch(Event::PairingCode {
            code: code.clone(),
            timeout: remaining,
        });

        Ok(code)
    }
}

/// Handles a `link_code_companion_reg` notification. Dispatches on the child's
/// `stage` attribute, mirroring WA Web `handleAltDeviceLinkingNotification`:
/// `primary_hello` completes stage 2; `refresh_code` asks the companion to
/// regenerate the code it is displaying.
#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.pair.code_notification", level = "debug", skip_all)
)]
pub(crate) async fn handle_pair_code_notification(
    client: &Arc<Client>,
    node: &NodeRef<'_>,
) -> bool {
    let Some(reg_node) = node.get_optional_child_by_tag(&["link_code_companion_reg"]) else {
        return false;
    };

    match reg_node.get_attr("stage").map(|v| v.as_str()).as_deref() {
        Some("primary_hello") => handle_primary_hello(client, reg_node).await,
        Some("refresh_code") => handle_refresh_code(client, reg_node).await,
        other => {
            warn!(
                target: "Client/PairCode",
                "Ignoring link_code_companion_reg notification with stage {other:?}"
            );
            false
        }
    }
}

/// Stage 2: the user entered the code on their phone. The notification carries
/// the primary's encrypted ephemeral public key and identity public key.
async fn handle_primary_hello(client: &Arc<Client>, reg_node: &NodeRef<'_>) -> bool {
    // Extract primary's wrapped ephemeral public key (80 bytes: salt + iv + encrypted key)
    let primary_wrapped_ephemeral = match reg_node
        .get_optional_child_by_tag(&["link_code_pairing_wrapped_primary_ephemeral_pub"])
        .and_then(|n| match n.content.as_deref() {
            Some(NodeContentRef::Bytes(b)) if b.len() == 80 => Some(b.to_vec()),
            _ => None,
        }) {
        Some(b) => b,
        None => {
            warn!(
                target: "Client/PairCode",
                "Missing or invalid primary wrapped ephemeral pub in notification"
            );
            return false;
        }
    };

    // Extract primary's identity public key (32 bytes, unencrypted)
    let primary_identity_pub: [u8; 32] = match reg_node
        .get_optional_child_by_tag(&["primary_identity_pub"])
        .and_then(|n| match n.content.as_deref() {
            Some(NodeContentRef::Bytes(b)) if b.len() == 32 => b.as_ref().try_into().ok(),
            _ => None,
        }) {
        Some(arr) => arr,
        None => {
            warn!(
                target: "Client/PairCode",
                "Missing or invalid primary identity pub in notification"
            );
            return false;
        }
    };

    // Ref echoed by the primary. WA Web (`InvalidRefError`) rejects a
    // primary_hello whose ref doesn't match the one from our companion_hello.
    let notif_ref = match reg_node
        .get_optional_child_by_tag(&["link_code_pairing_ref"])
        .and_then(|n| match n.content.as_deref() {
            Some(NodeContentRef::Bytes(b)) => Some(b.to_vec()),
            _ => None,
        }) {
        Some(r) => r,
        None => {
            warn!(target: "Client/PairCode", "primary_hello missing link_code_pairing_ref");
            return false;
        }
    };

    // Serialize the whole of stage 2 under the pair_code_state lock. The
    // transport dispatches <notification> stanzas on concurrent detached tasks
    // (see client/node_io.rs), so holding the guard across derive → persist →
    // send makes two primary_hello for the same code sequential, matching WA
    // Web's single-threaded model. Without it, both could derive a *different*
    // random adv_secret and race SetAdvSecretKey (last-write-wins), leaving the
    // persisted secret out of sync with the companion_finish the server acts on
    // → pair-success HMAC failure. The state is kept (not taken) so a genuine
    // retry can reuse it.
    let mut state_guard = client.pair_code_state.lock().await;
    let (pairing_ref, phone_jid, pair_code, ephemeral_keypair) = match &mut *state_guard {
        PairCodeState::WaitingForPhoneConfirmation {
            pairing_ref,
            phone_jid,
            pair_code,
            ephemeral_keypair,
            code_generation_ts,
            primary_hello_attempt_count,
        } => {
            // Validate before counting: only a genuine, in-window attempt
            // (matching ref, unexpired code) may spend a retry slot, so a
            // stale/foreign or late notification — neither of which triggers
            // a companion_finish — can't exhaust the budget.
            if pairing_ref.as_slice() != notif_ref.as_slice() {
                warn!(
                    target: "Client/PairCode",
                    "primary_hello ref does not match the outstanding request; ignoring"
                );
                return false;
            }
            let age = wacore::time::now_secs() - *code_generation_ts;
            if age > PairCodeUtils::code_validity().as_secs() as i64 {
                warn!(
                    target: "Client/PairCode",
                    "primary_hello arrived for an expired code ({age}s old); ignoring"
                );
                return false;
            }
            // Check the cap before bumping so a rejected attempt never pushes the
            // counter past the limit (keeps it bounded at max).
            if *primary_hello_attempt_count >= PairCodeUtils::max_primary_hello_attempts() {
                warn!(
                    target: "Client/PairCode",
                    "Exceeded max primary_hello attempts for this code; abandoning"
                );
                return false;
            }
            *primary_hello_attempt_count += 1;
            (
                pairing_ref.clone(),
                phone_jid.clone(),
                pair_code.clone(),
                (**ephemeral_keypair).clone(),
            )
        }
        _ => {
            warn!(
                target: "Client/PairCode",
                "Received primary_hello but not in waiting state"
            );
            return false;
        }
    };

    info!(
        target: "Client/PairCode",
        "Phone confirmed code entry, processing stage 2"
    );

    // Decrypt primary's ephemeral public key (expensive PBKDF2 operation)
    // Run in spawn_blocking to avoid stalling the async runtime
    let pair_code_clone = pair_code.clone();
    let primary_ephemeral_pub = match wacore::runtime::blocking(&*client.runtime, move || {
        PairCodeUtils::decrypt_primary_ephemeral_pub(&primary_wrapped_ephemeral, &pair_code_clone)
    })
    .await
    {
        Ok(pub_key) => pub_key,
        Err(e) => {
            error!(
                target: "Client/PairCode",
                "Failed to decrypt primary ephemeral pub: {e}"
            );
            return false;
        }
    };

    // Get device keys
    let device_snapshot = client.persistence_manager.get_device_snapshot();

    // Prepare encrypted key bundle (includes rotated adv_secret_key)
    let (wrapped_bundle, new_adv_secret) = match PairCodeUtils::prepare_key_bundle(
        &ephemeral_keypair,
        &primary_ephemeral_pub,
        &primary_identity_pub,
        &device_snapshot.identity_key,
    ) {
        Ok(result) => result,
        Err(e) => {
            error!(target: "Client/PairCode", "Failed to prepare key bundle: {e}");
            return false;
        }
    };

    // Persist rotated adv_secret_key so HMAC verification works in pair-success.
    client
        .persistence_manager
        .process_command(crate::store::commands::DeviceCommand::SetAdvSecretKey(
            new_adv_secret,
        ))
        .await;

    // Build and send stage 2 IQ
    let req_id = client.generate_request_id();
    let identity_pub: [u8; 32] = device_snapshot
        .identity_key
        .public_key
        .public_key_bytes()
        .try_into()
        .expect("identity key is 32 bytes");

    let iq = PairCodeUtils::build_companion_finish_iq(
        &phone_jid,
        wrapped_bundle,
        &identity_pub,
        &pairing_ref,
        req_id,
    );

    if let Err(e) = client.send_node(iq).await {
        error!(target: "Client/PairCode", "Failed to send companion_finish: {e}");
        return false;
    }

    info!(
        target: "Client/PairCode",
        "Sent companion_finish, waiting for pair-success"
    );

    // State stays WaitingForPhoneConfirmation so a retry can reuse it; only
    // pair-success (see `crate::pair`) transitions to Completed. `state_guard`
    // (held since the top for serialization) is released here.
    drop(state_guard);
    true
}

/// The server asked us to refresh the code we are displaying (WA Web
/// `refreshAltLinkingCode` / `forceManualRefresh`). Surfaces a
/// [`Event::PairingCodeRefresh`] so the consumer re-requests a code, but only
/// when the notification's ref matches the flow currently in progress.
async fn handle_refresh_code(client: &Arc<Client>, reg_node: &NodeRef<'_>) -> bool {
    let notif_ref = match reg_node
        .get_optional_child_by_tag(&["link_code_pairing_ref"])
        .and_then(|n| match n.content.as_deref() {
            Some(NodeContentRef::Bytes(b)) => Some(b.to_vec()),
            _ => None,
        }) {
        Some(r) => r,
        None => {
            warn!(target: "Client/PairCode", "refresh_code missing link_code_pairing_ref");
            return false;
        }
    };

    let force_manual = reg_node
        .get_attr("force_manual_refresh")
        .map(|v| v.as_str().as_ref() == "true")
        .unwrap_or(false);

    // Ignore a refresh whose ref doesn't match the outstanding code — matches
    // WA Web's `getCurrentRef()` guard.
    let matches_current = {
        let state_guard = client.pair_code_state.lock().await;
        matches!(
            &*state_guard,
            PairCodeState::WaitingForPhoneConfirmation { pairing_ref, .. }
                if pairing_ref.as_slice() == notif_ref.as_slice()
        )
    };
    if !matches_current {
        warn!(
            target: "Client/PairCode",
            "refresh_code ref does not match the outstanding request; ignoring"
        );
        return false;
    }

    info!(
        target: "Client/PairCode",
        "Server requested pair-code refresh (force_manual={force_manual})"
    );
    client
        .core
        .event_bus
        .dispatch(Event::PairingCodeRefresh { force_manual });
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pair_error_request_failed_preserves_iq_source() {
        let iq = IqError::ServerError {
            code: 400,
            text: "bad-request".into(),
            error_type: None,
            backoff: None,
        };
        let pe: PairError = iq.into();
        let src = std::error::Error::source(&pe).expect("source preserved");
        let downcast = src.downcast_ref::<IqError>().expect("downcasts to IqError");
        assert!(matches!(downcast, IqError::ServerError { code: 400, .. }));
    }

    #[test]
    fn pair_error_paircode_transparent_walks_to_curve_error() {
        use wacore::libsignal::protocol::CurveError;
        // Wrap a wacore PairCodeError that itself carries a CurveError source.
        // Because PairError::PairCode is `transparent`, walking source() once
        // skips the transparent layer and lands directly on the CurveError.
        let pe: PairError =
            PairCodeError::EphemeralKeyAgreement(CurveError::NoKeyTypeIdentifier).into();
        assert_eq!(pe.to_string(), "ephemeral key agreement failed");
        let src = std::error::Error::source(&pe).expect("source preserved");
        let curve = src
            .downcast_ref::<CurveError>()
            .expect("downcasts to CurveError through transparent wrapper");
        assert!(matches!(curve, CurveError::NoKeyTypeIdentifier));
    }

    // ── Stage-2 notification handling (WA Web parity guards) ─────────────────
    //
    // All tests drive the top-level `handle_pair_code_notification`, so the
    // `stage` dispatch is exercised end-to-end. The guard-reject paths bail
    // before any stage-2 crypto, so "the adv secret is unchanged" is a reliable
    // proxy for "we did not process the notification"; conversely a valid
    // primary_hello rotates it (via `SetAdvSecretKey`) before the socket send.

    use crate::test_utils::create_test_client;
    use wacore::libsignal::protocol::KeyPair;
    use wacore_binary::Node;
    use wacore_binary::builder::NodeBuilder;

    fn primary_hello_notif(reg_ref: &[u8]) -> Node {
        NodeBuilder::new("notification")
            .attr("type", "link_code_companion_reg")
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("link_code_companion_reg")
                .attr("stage", "primary_hello")
                .children([
                    // Non-zero dummy bytes keep the stage-2 DH well-defined.
                    NodeBuilder::new("link_code_pairing_wrapped_primary_ephemeral_pub")
                        .bytes(vec![7u8; 80])
                        .build(),
                    NodeBuilder::new("primary_identity_pub")
                        .bytes(vec![9u8; 32])
                        .build(),
                    NodeBuilder::new("link_code_pairing_ref")
                        .bytes(reg_ref.to_vec())
                        .build(),
                ])
                .build()])
            .build()
    }

    fn refresh_code_notif(reg_ref: &[u8], force_manual: Option<bool>) -> Node {
        let mut reg = NodeBuilder::new("link_code_companion_reg").attr("stage", "refresh_code");
        if let Some(f) = force_manual {
            reg = reg.attr("force_manual_refresh", if f { "true" } else { "false" });
        }
        NodeBuilder::new("notification")
            .attr("type", "link_code_companion_reg")
            .attr("from", "s.whatsapp.net")
            .children([reg
                .children([NodeBuilder::new("link_code_pairing_ref")
                    .bytes(reg_ref.to_vec())
                    .build()])
                .build()])
            .build()
    }

    async fn set_waiting(client: &Arc<Client>, pairing_ref: Vec<u8>, ts: i64, count: u32) {
        *client.pair_code_state.lock().await = PairCodeState::WaitingForPhoneConfirmation {
            pairing_ref,
            phone_jid: "15551234567".to_string(),
            pair_code: "ABCD1234".to_string(),
            ephemeral_keypair: Box::new(KeyPair::generate(
                &mut rand::make_rng::<rand::rngs::StdRng>(),
            )),
            code_generation_ts: ts,
            primary_hello_attempt_count: count,
        };
    }

    fn adv(client: &Arc<Client>) -> [u8; 32] {
        client
            .persistence_manager
            .get_device_snapshot()
            .adv_secret_key
    }

    async fn is_waiting(client: &Arc<Client>) -> bool {
        matches!(
            &*client.pair_code_state.lock().await,
            PairCodeState::WaitingForPhoneConfirmation { .. }
        )
    }

    async fn attempt_count(client: &Arc<Client>) -> Option<u32> {
        match &*client.pair_code_state.lock().await {
            PairCodeState::WaitingForPhoneConfirmation {
                primary_hello_attempt_count,
                ..
            } => Some(*primary_hello_attempt_count),
            _ => None,
        }
    }

    /// Regression: a `primary_hello` whose ref doesn't match our outstanding
    /// companion_hello must be rejected (WA Web `InvalidRefError`) without
    /// running stage 2, must leave the flow intact for a later valid one, and
    /// must NOT consume a retry slot (the ref check precedes the counter bump).
    #[tokio::test]
    async fn primary_hello_rejects_mismatched_ref() {
        let client = create_test_client().await;
        set_waiting(&client, vec![1, 2, 3, 4], wacore::time::now_secs(), 0).await;
        let adv_before = adv(&client);

        let notif = primary_hello_notif(&[9, 9, 9, 9]);
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(!handled, "mismatched ref must be rejected");
        assert_eq!(
            adv(&client),
            adv_before,
            "no stage-2 crypto on ref mismatch"
        );
        assert!(
            is_waiting(&client).await,
            "state must be preserved so a later valid primary_hello can complete"
        );
        assert_eq!(
            attempt_count(&client).await,
            Some(0),
            "a ref-mismatched notification must not burn a retry slot"
        );
    }

    /// Regression: stale/foreign-ref notifications must not exhaust the attempt
    /// cap. Even after several mismatched hellos, the genuine one still reaches
    /// stage 2 (adv secret rotates).
    #[tokio::test]
    async fn stale_mismatched_hellos_do_not_block_the_valid_one() {
        let client = create_test_client().await;
        let pairing_ref = vec![1, 2, 3, 4];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;
        let adv_before = adv(&client);

        // More mismatched hellos than the cap would allow if they counted.
        for _ in 0..(PairCodeUtils::max_primary_hello_attempts() + 2) {
            let bad = primary_hello_notif(&[9, 9, 9, 9]);
            let _ = handle_pair_code_notification(&client, &bad.as_node_ref()).await;
        }
        assert_eq!(
            attempt_count(&client).await,
            Some(0),
            "mismatched hellos must leave the attempt count untouched"
        );

        let good = primary_hello_notif(&pairing_ref);
        let _ = handle_pair_code_notification(&client, &good.as_node_ref()).await;
        assert_ne!(
            adv(&client),
            adv_before,
            "the genuine primary_hello must still reach stage 2 after stale mismatches"
        );
    }

    /// Regression: a `primary_hello` for a code older than the ~180s validity
    /// window must be rejected (WA Web `OldCodeError`).
    #[tokio::test]
    async fn primary_hello_rejects_expired_code() {
        let client = create_test_client().await;
        let pairing_ref = vec![1, 2, 3, 4];
        let stale_ts =
            wacore::time::now_secs() - (PairCodeUtils::code_validity().as_secs() as i64 + 20);
        set_waiting(&client, pairing_ref.clone(), stale_ts, 0).await;
        let adv_before = adv(&client);

        let notif = primary_hello_notif(&pairing_ref);
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(
            !handled,
            "primary_hello for an expired code must be rejected"
        );
        assert_eq!(
            adv(&client),
            adv_before,
            "no stage-2 crypto on an expired code"
        );
        assert_eq!(
            attempt_count(&client).await,
            Some(0),
            "an expired-code notification must not burn a retry slot"
        );
    }

    /// Regression: at most `max_primary_hello_attempts` (WA Web `T = 3`) are
    /// processed per code; the next one is dropped (`MaxPrimaryHelloError`).
    #[tokio::test]
    async fn primary_hello_rejects_beyond_max_attempts() {
        let client = create_test_client().await;
        let pairing_ref = vec![1, 2, 3, 4];
        set_waiting(
            &client,
            pairing_ref.clone(),
            wacore::time::now_secs(),
            PairCodeUtils::max_primary_hello_attempts(),
        )
        .await;
        let adv_before = adv(&client);

        let notif = primary_hello_notif(&pairing_ref);
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(!handled, "the attempt past the cap must be rejected");
        assert_eq!(
            adv(&client),
            adv_before,
            "no stage-2 crypto once the per-code attempt cap is exhausted"
        );
        assert_eq!(
            attempt_count(&client).await,
            Some(PairCodeUtils::max_primary_hello_attempts()),
            "a rejected over-cap attempt must not push the counter past the max"
        );
    }

    /// The guards must not over-reject: a valid retry (matching ref, fresh code,
    /// still under the cap) reaches stage 2 and rotates the adv secret. The
    /// socket send then fails (no transport in tests), so the call returns
    /// false, but the rotation proves processing happened.
    #[tokio::test]
    async fn primary_hello_valid_retry_reaches_stage2() {
        let client = create_test_client().await;
        let pairing_ref = vec![1, 2, 3, 4];
        // count = 2 → this is the 3rd attempt, still within the cap of 3.
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 2).await;
        let adv_before = adv(&client);

        let notif = primary_hello_notif(&pairing_ref);
        let _ = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert_ne!(
            adv(&client),
            adv_before,
            "a valid in-window retry must reach stage 2 and rotate the adv secret"
        );
    }

    /// A `refresh_code` whose ref matches the outstanding flow surfaces a
    /// `PairingCodeRefresh` event carrying `force_manual`.
    #[tokio::test]
    async fn refresh_code_matching_ref_dispatches_event() {
        let client = create_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.register_handler(collector.clone());

        let pairing_ref = vec![5, 6, 7, 8];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = refresh_code_notif(&pairing_ref, Some(true));
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(handled, "a matching refresh_code should be handled");
        let events = collector.events();
        assert!(
            events
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeRefresh { force_manual: true })),
            "expected PairingCodeRefresh{{force_manual:true}}, got: {events:?}"
        );
    }

    /// An absent `force_manual_refresh` attribute maps to `force_manual: false`
    /// (WA Web's non-force `refreshAltLinkingCode` branch). Locks down the
    /// `== "true"` parse against a flip to `!= "false"`.
    #[tokio::test]
    async fn refresh_code_without_force_manual_defaults_false() {
        let client = create_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.register_handler(collector.clone());

        let pairing_ref = vec![5, 6, 7, 8];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = refresh_code_notif(&pairing_ref, None);
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(handled, "a matching refresh_code should be handled");
        assert!(
            collector.events().iter().any(|e| matches!(
                &**e,
                Event::PairingCodeRefresh {
                    force_manual: false
                }
            )),
            "absent force_manual_refresh must dispatch force_manual: false"
        );
    }

    /// A `refresh_code` for a different ref (or with no flow in progress) is
    /// ignored — no event, matching WA Web's `getCurrentRef()` guard.
    #[tokio::test]
    async fn refresh_code_mismatched_ref_is_ignored() {
        let client = create_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.register_handler(collector.clone());

        set_waiting(&client, vec![5, 6, 7, 8], wacore::time::now_secs(), 0).await;

        let notif = refresh_code_notif(&[1, 1, 1, 1], None);
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(!handled, "a non-matching refresh_code must be ignored");
        assert!(
            collector.events().is_empty(),
            "no event should fire for a refresh_code with an unknown ref"
        );
    }

    /// An unknown `stage` on the notification is ignored without touching the
    /// in-progress flow.
    #[tokio::test]
    async fn unknown_stage_is_ignored_and_preserves_state() {
        let client = create_test_client().await;
        set_waiting(&client, vec![1, 2, 3, 4], wacore::time::now_secs(), 0).await;

        let notif = NodeBuilder::new("notification")
            .attr("type", "link_code_companion_reg")
            .attr("from", "s.whatsapp.net")
            .children([NodeBuilder::new("link_code_companion_reg")
                .attr("stage", "some_future_stage")
                .build()])
            .build();
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(!handled, "unknown stage must not be treated as handled");
        assert!(
            is_waiting(&client).await,
            "unknown stage must leave the outstanding flow untouched"
        );
    }
}
