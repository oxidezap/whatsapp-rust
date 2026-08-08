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
//! Pair code and QR code run on the same connection, and whichever completes
//! first wins — matching WA Web, which leaves its QR rotation running when the
//! user switches to phone-number linking.
//!
//! They are not, however, the same clock. A QR code is superseded every 20s and
//! the surface re-renders it; a pair code is read off a screen and typed into a
//! phone minutes later, so **a QR rotation is not a reason to request a new
//! pair code**. WA Web mints one per user action and regenerates it only on the
//! server's `refresh_code`, on `force_manual_refresh`, or on its own expiry
//! timers. [`Client::pair_with_code`] enforces that: it refuses to supersede a
//! code that is still live, and [`Client::cancel_pair_code`] is the explicit
//! way to replace one.

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
pub use wacore::pair_code::{PairCodeError, PairCodeOptions, PairCodeRejection};

/// Errors raised by the high-level pair-code flow.
///
/// Wraps `wacore::pair_code::PairCodeError` (validation, key derivation, bundle
/// building) and adds the IQ transport layer via `RequestFailed`.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PairError {
    #[error("{0}")]
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
    ///
    /// Renders what it wraps, per the [rendering
    /// convention](crate::error#rendering) — so the code and text reach a log
    /// line that only prints the error, without the reader having to reach for
    /// the `Debug` form.
    #[error("{0}")]
    RequestFailed(#[from] IqError),
}

// Imported inside each body, not at module scope: `ErrorChainExt::as_dyn_error`
// would then be ambiguous with thiserror's own `AsDynError` for every `#[from]`
// in this module.
impl PairError {
    /// How the server refused the request, as a status to branch on.
    ///
    /// `None` when nothing was refused: local validation, no connection, or a
    /// request that went unanswered. Prefer this to matching the message, which
    /// is not a stable surface.
    ///
    /// Classified from the `code` and `text` together, so a pairing WA Web
    /// would not accept yields `None` rather than the named arm — see
    /// [`PairCodeRejection::from_server`]. The refused-but-unclassifiable case
    /// is therefore indistinguishable here from "nothing was refused"; both
    /// mean the same thing to a consumer, which is that there is no typed
    /// status to act on and the message is all there is.
    pub fn rejection(&self) -> Option<PairCodeRejection> {
        use crate::error::ErrorChainExt;
        self.server_rejection()
            .and_then(|rejection| PairCodeRejection::from_server(rejection.code, rejection.text))
    }

    /// Whether this request lost the pairing flow to someone else rather than
    /// ending it — so its failure says nothing about whether a code arrives.
    ///
    /// These are the failures [`Event::PairingCodeError`] must stay silent for,
    /// because its meaning is "no code is coming" and here that is not what
    /// happened:
    ///
    /// - [`PairCodeError::CodeAlreadyOutstanding`] — refused *because* an
    ///   earlier code is still inside its validity window. That code is on
    ///   screen and may yet be entered; the consumer already has it from the
    ///   [`Event::PairingCode`] that minted it.
    /// - [`PairCodeError::Cancelled`] — the caller withdrew this request via
    ///   [`Client::cancel_pair_code`], and a replacement may already own the
    ///   slot. Reporting the *predecessor* would let a consumer read the live
    ///   replacement as failed and tear down a code that is about to arrive.
    ///
    /// Both are consequences of something the caller did, so neither is news to
    /// them, and a direct caller still receives the `Err` either way.
    pub fn lost_the_flow_to_another_request(&self) -> bool {
        matches!(
            self,
            Self::PairCode(PairCodeError::CodeAlreadyOutstanding { .. } | PairCodeError::Cancelled)
        )
    }

    /// How long the server asked the client to wait before retrying, from the
    /// `backoff` attribute.
    ///
    /// Usually `None` — the server rarely populates it on this request, and WA
    /// Web never reads it — but a value here is the server naming its own delay,
    /// which beats an interval the consumer picked.
    pub fn backoff(&self) -> Option<std::time::Duration> {
        use crate::error::ErrorChainExt;
        self.server_rejection()
            .and_then(|rejection| rejection.backoff)
            .map(|secs| std::time::Duration::from_secs(u64::from(secs)))
    }
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
    /// # One code at a time
    ///
    /// Fails with [`PairCodeError::CodeAlreadyOutstanding`] while a previously
    /// issued code is still within its validity window. A second code does not
    /// replace the first for the phone: the server routes `primary_hello` by
    /// number, so whoever enters the older code still reaches stage 2 and is
    /// answered with a key bundle their code cannot open — the phone reports a
    /// failed link and nothing surfaces here. Call
    /// [`Client::cancel_pair_code`] first when the replacement is intentional.
    ///
    /// In particular, do not drive this from QR-code rotation: the two have
    /// unrelated lifetimes, and a code being typed into a phone outlives
    /// several QR refs.
    ///
    /// # Arguments
    ///
    /// * `options` - Configuration for pair code authentication
    ///
    /// # Returns
    ///
    /// * `Ok(String)` - The 8-character pairing code to display
    /// * `Err` - If validation fails, a code is already outstanding, not
    ///   connected, or server error. A [`PairError::RequestFailed`] carrying
    ///   `bad-request` may be **rate-limiting** (throttled per phone number),
    ///   not invalid input — back off and retry.
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
        // The failure is dispatched here rather than at each `return Err`
        // below: stage 1 fails from a dozen places, and what a consumer needs
        // from all of them is the same single fact — no code is coming. Wrapping
        // the flow is also what stops a *later* early return from going
        // unreported. `BotBuilder::with_pair_code` depends on it having no gaps,
        // because it drives this from a detached task whose `Err` reaches nobody.
        //
        // Mirrors the success path, which likewise both returns the code and
        // dispatches `Event::PairingCode`; a direct caller sees the failure
        // twice, and a `with_pair_code` consumer sees it at all.
        match self.pair_with_code_inner(options).await {
            Ok(code) => Ok(code),
            Err(e) if self.failure_is_not_this_flows_to_report(&e).await => Err(e),
            Err(e) => {
                self.core.event_bus.dispatch(Event::PairingCodeError(
                    crate::types::events::PairingCodeError::builder()
                        .maybe_rejection(e.rejection())
                        .maybe_backoff(e.backoff())
                        .error(e.to_string())
                        .build(),
                ));
                Err(e)
            }
        }
    }

    /// Whether reporting this failure would speak for a flow that is not the
    /// failed request's to speak for.
    ///
    /// The event means "no code is coming", so the question is not *how* the
    /// request failed but whether a code is nonetheless on its way. Answered on
    /// the state, not on the error variant: the variants that can reach here
    /// while a flow is live are open-ended — a duplicate request, a withdrawn
    /// one, its IQ timing out, or a second caller simply passing a bad phone
    /// number while the first code is still on screen — and enumerating them
    /// has already been wrong four times.
    ///
    /// [`PairCodeError::Cancelled`] is still matched explicitly, because a
    /// cancellation with no replacement leaves the slot idle: nothing is live,
    /// yet the caller asked for exactly this and does not need telling.
    async fn failure_is_not_this_flows_to_report(self: &Arc<Self>, e: &PairError) -> bool {
        if e.lost_the_flow_to_another_request() {
            return true;
        }
        // A failing request that still owned the slot has released it by now, so
        // an outstanding flow here belongs to somebody else.
        self.pair_code_state
            .lock()
            .await
            .is_outstanding(wacore::time::now_secs())
    }

    async fn pair_with_code_inner(
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

        // A second code does not replace the first for the *phone*: the server
        // routes `primary_hello` by number, never seeing the code, so whoever
        // is still reading the older one reaches stage 2 and is handed a key
        // bundle their code cannot open — the phone reports a failed link and
        // nothing surfaces here. WA Web makes the overlap impossible by
        // guarding `startAltLinkingFlow` with `invariant(stage === Initialized)`
        // (`Alt/DeviceLinkingApi.js`); `cancel_pair_code` is our
        // `initializeAltDeviceLinking()`.
        //
        // Claimed under the same lock that reads it, because releasing it
        // across the stage-1 round trip would let two concurrent callers both
        // find the state idle. The stamp doubles as the validity clock, which
        // WA Web also starts before the request (`startAltLinkingFlow` sets
        // `codeGenerationTs` before sending), so the ~180s window covers the
        // round trip rather than starting after it.
        let code_generation_ts = wacore::time::now_secs();
        let claim = wacore::pair_code::PairCodeClaim::next();
        {
            let mut state = self.pair_code_state.lock().await;
            if state.is_outstanding(code_generation_ts) {
                return Err(PairCodeError::CodeAlreadyOutstanding {
                    remaining: state
                        .live_flow_remaining(code_generation_ts)
                        .unwrap_or_default(),
                }
                .into());
            }
            *state = PairCodeState::RequestingCode {
                code_generation_ts,
                claim,
            };
        }
        // Every path out has to hand the claim back, including a caller who
        // drops this future (a `timeout` shorter than the IQ's, say) — an
        // orphaned claim rejects every later request for the rest of the
        // validity window. Disarmed only once the flow is installed.
        let mut claim_guard = ClaimGuard {
            client: Arc::clone(self),
            claim,
            armed: true,
        };

        info!(
            target: "Client/PairCode",
            "Starting pair code authentication for phone: {}",
            phone_number
        );

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

        // The PBKDF2 above takes long enough for a `cancel_pair_code` to land.
        // Sending anyway would put a second `companion_hello` on the server for
        // this number, which then routes `primary_hello` to whichever it likes
        // — the overlap the claim exists to prevent.
        if !self.owns_code_claim(claim).await {
            // Someone else owns the slot; releasing would take theirs.
            claim_guard.armed = false;
            return Err(PairCodeError::Cancelled.into());
        }

        let response = match self.send_iq(query).await {
            Ok(response) => response,
            Err(e) => {
                // The same ownership recheck the success path does below, and
                // for the same reason. A 30s IQ timeout easily outlives a
                // `cancel_pair_code` plus its replacement, and reporting this
                // request's transport failure would then put an uncorrelated
                // error on the bus against the flow that now owns the slot.
                // Losing the slot outranks how this request happened to end.
                if !self.owns_code_claim(claim).await {
                    claim_guard.armed = false;
                    return Err(PairCodeError::Cancelled.into());
                }
                claim_guard.release_now().await;
                return Err(e.into());
            }
        };

        let Some(pairing_ref) = PairCodeUtils::parse_companion_hello_response(response.get())
        else {
            claim_guard.release_now().await;
            return Err(PairCodeError::MissingPairingRef.into());
        };

        info!(
            target: "Client/PairCode",
            "Stage 1 complete, waiting for phone confirmation. Code: {}",
            code
        );

        // Store state for when phone confirms, unless the claim was withdrawn
        // while stage 1 was in flight: installing over a cancellation would
        // revive a flow the caller asked to drop, and over a replacement would
        // strand the code that replacement returned.
        {
            let mut state = self.pair_code_state.lock().await;
            if !matches!(&*state, PairCodeState::RequestingCode { claim: c, .. } if *c == claim) {
                claim_guard.armed = false;
                return Err(PairCodeError::Cancelled.into());
            }
            *state = PairCodeState::WaitingForPhoneConfirmation {
                pairing_ref,
                phone_jid: phone_number,
                pair_code: code.clone(),
                ephemeral_keypair: Box::new(ephemeral_keypair),
                code_generation_ts,
                primary_hello_attempt_count: 0,
            };
            claim_guard.armed = false;
        }

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
        self.core.event_bus.dispatch(Event::PairingCode(
            crate::types::events::PairingCode::builder()
                .code(code.clone())
                .timeout(remaining)
                .build(),
        ));

        Ok(code)
    }

    /// Hand back a claim taken by [`Self::pair_with_code`] when stage 1 failed.
    ///
    /// Identified by its token, so a claim already superseded — by a
    /// cancellation, or by the replacement that followed one — is left alone.
    async fn owns_code_claim(self: &Arc<Self>, claim: wacore::pair_code::PairCodeClaim) -> bool {
        matches!(&*self.pair_code_state.lock().await, PairCodeState::RequestingCode { claim: c, .. } if *c == claim)
    }

    async fn release_code_claim(self: &Arc<Self>, claim: wacore::pair_code::PairCodeClaim) {
        let mut state = self.pair_code_state.lock().await;
        if matches!(&*state, PairCodeState::RequestingCode { claim: c, .. } if *c == claim) {
            *state = PairCodeState::Idle;
        }
    }

    /// Abandons the outstanding pair-code flow, if any.
    ///
    /// The explicit reset [`Client::pair_with_code`] requires before it will
    /// mint a replacement — WA Web's `initializeAltDeviceLinking()`. After this
    /// the previous code can no longer complete: a `primary_hello` for it is
    /// dropped rather than answered with a bundle its holder cannot open.
    ///
    /// A flow cancelled after it reached stage 2 also gives up the adv secret
    /// that stage derived: it is keyed to a primary that will never link. A
    /// [`PairCodeState::Completed`] flow does not, because that secret belongs
    /// to a device that paired, and re-minting it would invalidate the
    /// account's own ADV signatures.
    pub async fn cancel_pair_code(self: &Arc<Self>) {
        let mut state = self.pair_code_state.lock().await;
        if matches!(&*state, PairCodeState::Idle) {
            return;
        }
        let rotated_adv_secret = state.awaiting_pair_success();
        *state = PairCodeState::Idle;
        if rotated_adv_secret {
            // Held across the write for the reason in `retire_stage_two_flow`.
            replace_adv_secret_key(self).await;
        }
    }
}

/// Releases a stage-1 claim unless the flow it belongs to was installed.
///
/// Error paths call [`Self::release_now`]; `Drop` is the backstop for a caller
/// that drops the future instead. It cannot await, so that release is spawned —
/// the claim is identified by its token, which makes a late release harmless
/// once something else has taken the slot.
struct ClaimGuard {
    client: Arc<Client>,
    claim: wacore::pair_code::PairCodeClaim,
    armed: bool,
}

impl ClaimGuard {
    /// Hand the claim back before returning, so a caller that retries the
    /// moment it sees the error does not race the detached release and get
    /// `CodeAlreadyOutstanding` for a request that already failed. `Drop` is
    /// left to cover only the caller who never sees the error at all.
    async fn release_now(&mut self) {
        self.armed = false;
        self.client.release_code_claim(self.claim).await;
    }
}

impl Drop for ClaimGuard {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        let client = Arc::clone(&self.client);
        let claim = self.claim;
        client.clone().runtime.spawn_detached(Box::pin(async move {
            client.release_code_claim(claim).await;
        }));
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
        .and_then(|n| match n.content.as_ref() {
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
        .and_then(|n| match n.content.as_ref() {
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
        .and_then(|n| match n.content.as_ref() {
            Some(NodeContentRef::Bytes(b)) => Some(b.to_vec()),
            _ => None,
        }) {
        Some(r) => r,
        None => {
            warn!(target: "Client/PairCode", "primary_hello missing link_code_pairing_ref");
            return false;
        }
    };

    // Only the cheap guards run here. Everything they admit is handed to a
    // task, because this function's return is what releases the stanza's ack:
    // WA Web starts `handlePrimaryHello` without awaiting it and returns the
    // ack in the same expression (`Alt/DeviceLinkingHandleNotification.js`),
    // whereas running stage 2 inline puts a 131k-round PBKDF2 between the
    // server's notification and our acknowledgement of it.
    //
    // The lock still serializes stage 2 end to end — see `run_stage_two`.
    let mut state_guard = client.pair_code_state.lock().await;
    let (pairing_ref, phone_jid, pair_code, ephemeral_keypair, attempt) = match &mut *state_guard {
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
                *primary_hello_attempt_count,
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

    // Released before the task runs: `run_stage_two` re-takes it.
    drop(state_guard);

    let client = Arc::clone(client);
    // Armed on acceptance, matching WA Web: `primaryHelloReceivedAltLinking`
    // fires before `handlePrimaryHelloInternal` runs, so the screen's clock
    // starts on the notification. Waiting for a successful `companion_finish`
    // would leave a failed stage 2 with no timeout at all — the case that most
    // needs the consumer to hear about it.
    start_pair_success_timeout(Arc::clone(&client), pairing_ref.clone(), attempt);
    client.clone().runtime.spawn_detached(Box::pin(async move {
        run_stage_two(
            client,
            pairing_ref,
            phone_jid,
            pair_code,
            ephemeral_keypair,
            primary_wrapped_ephemeral,
            primary_identity_pub,
            attempt,
        )
        .await;
    }));
    true
}

/// Derive the key bundle, persist the rotated adv secret, send
/// `companion_finish`, and start the clock on the `pair-success` that should
/// answer it.
///
/// Runs under the `pair_code_state` lock from derive through send. The
/// transport dispatches `notification` stanzas on concurrent detached tasks
/// (see `client/node_io.rs`), so without it two `primary_hello` for the same
/// code could each derive a *different* random adv_secret and race
/// `SetAdvSecretKey` (last-write-wins), leaving the persisted secret out of
/// sync with the `companion_finish` the server acts on → pair-success HMAC
/// failure. The state is kept (not taken) so a genuine retry can reuse it.
///
/// The lock stops at the send, not at the answer: ordering that pair is the
/// whole reason it is held, and the answer takes no part in it. Keeping it
/// across the round trip would block `cancel_pair_code` on a server that never
/// replies — exactly the case this wait exists to detect.
#[allow(clippy::too_many_arguments)]
async fn run_stage_two(
    client: Arc<Client>,
    pairing_ref: Vec<u8>,
    phone_jid: String,
    pair_code: String,
    ephemeral_keypair: KeyPair,
    primary_wrapped_ephemeral: Vec<u8>,
    primary_identity_pub: [u8; 32],
    attempt: u32,
) {
    let state_guard = client.pair_code_state.lock().await;
    // The flow can be retired while this task waits for the lock — by
    // pair-success, a cancellation, or a replacement code. Matching the ref
    // rather than the variant is what tells a replacement apart from our own
    // flow: answering for one would persist a retired adv secret over the
    // replacement's and put a `companion_finish` on the wire for a ref nobody
    // holds.
    let still_ours = matches!(
        &*state_guard,
        PairCodeState::WaitingForPhoneConfirmation { pairing_ref: current, .. }
            if current.as_slice() == pairing_ref.as_slice()
    );
    if !still_ours {
        return;
    }

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
            return;
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
            return;
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

    // Dropping the hook without calling it releases the guard too, so a send
    // that never happened does not strand the lock either.
    let answer = client
        .send_iq_node_then(
            iq,
            Some(PairCodeUtils::companion_finish_iq_timeout()),
            Some(Box::new(move || drop(state_guard))),
        )
        .await;

    match answer {
        Ok(_) => {
            info!(
                target: "Client/PairCode",
                "Sent companion_finish, waiting for pair-success"
            );
            // State stays WaitingForPhoneConfirmation so a retry can reuse it;
            // only pair-success (see `crate::pair`) transitions to Completed.
            // The timeout that answers for this send was armed by the caller,
            // on acceptance.
        }
        Err(e) => report_stage_two_failure(&client, &pairing_ref, attempt, e).await,
    }
}

/// Write the flow off and tell the consumer why, when `companion_finish` itself
/// failed.
///
/// WA Web treats this as terminal rather than as something to retry: its RPC
/// raises `CompanionFinishError` for any answer that is not
/// `CompanionFinishResponseSuccess` (`Alt/DeviceLinkingIq.js`), the throw
/// reaches `handlePrimaryHello`, and the screen shows "Something went wrong.
/// Please try again or link with the QR code" and returns to the number entry
/// (`Link/DevicePhoneNumber.react.js`). So this reports and stops; it does not
/// resend.
///
/// Without it the refusal was invisible: the IQ went out unanswered-for, its
/// response matched no waiter, and the consumer learnt only from the one-minute
/// silence timer — with no way to tell a refused bundle from a primary that
/// went quiet.
///
/// A timeout is the one failure that does not retire anything. It carries no
/// news: silence is already what
/// [`start_pair_success_timeout`] is watching for, over a longer window, and
/// cutting the flow short here would take a link the server may still be
/// completing.
async fn report_stage_two_failure(
    client: &Arc<Client>,
    pairing_ref: &[u8],
    attempt: u32,
    error: IqError,
) {
    if error.is_timeout() {
        warn!(
            target: "Client/PairCode",
            "companion_finish went unanswered; leaving the pair-success timer to write the code off"
        );
        return;
    }

    error!(target: "Client/PairCode", "companion_finish failed: {error}");
    if !retire_stage_two_flow(client, pairing_ref, attempt).await {
        return;
    }

    let error = PairError::from(error);
    client.core.event_bus.dispatch(Event::PairingCodeError(
        crate::types::events::PairingCodeError::builder()
            .maybe_rejection(error.rejection())
            .maybe_backoff(error.backoff())
            .error(error.to_string())
            .build(),
    ));
}

/// Write the code off if `pair-success` never answers `companion_finish`.
///
/// The primary reading the code proves nothing about the outcome: if it cannot
/// open the key bundle — which is what a superseded or mistyped code looks like
/// from here — it reports a failed link to its own user and says nothing to us.
/// WA Web treats that silence as the failure signal, arming a one-minute timer
/// on `primary_hello_received` and regenerating the code when it fires
/// (`Link/DevicePhoneNumberCodeScreen.react.js`).
fn start_pair_success_timeout(client: Arc<Client>, pairing_ref: Vec<u8>, attempt: u32) {
    let timeout = PairCodeUtils::primary_hello_pair_success_timeout();
    client.clone().runtime.spawn_detached(Box::pin(async move {
        client.runtime.sleep(timeout).await;

        if !retire_stage_two_flow(&client, &pairing_ref, attempt).await {
            return;
        }

        warn!(
            target: "Client/PairCode",
            "No pair-success within {timeout:?} of companion_finish; the code will not complete"
        );
        client.core.event_bus.dispatch(Event::PairingCodeRefresh(
            crate::types::events::PairingCodeRefresh::builder()
                .force_manual(false)
                .build(),
        ));
    }));
}

/// Retire a flow whose stage 2 will not complete, and return whether this
/// caller is the one that retired it.
///
/// Keyed on the ref *and* the attempt: pair-success, a cancellation, or a
/// replacement code all leave a state this does not match, and a retry accepted
/// partway through the window owns its own attempt — so neither the timeout nor
/// a failed `companion_finish` may write off a flow that moved on.
///
/// The state is cleared before the caller's event goes out, so a consumer
/// acting on it is not turned away by the very flow it was told to replace.
async fn retire_stage_two_flow(client: &Arc<Client>, pairing_ref: &[u8], attempt: u32) -> bool {
    let mut state = client.pair_code_state.lock().await;
    let still_ours = matches!(
        &*state,
        PairCodeState::WaitingForPhoneConfirmation {
            pairing_ref: r,
            primary_hello_attempt_count,
            ..
        } if r.as_slice() == pairing_ref
            && *primary_hello_attempt_count == attempt
    );
    if !still_ours {
        return false;
    }
    *state = PairCodeState::Idle;
    // Under the same guard that retired the flow. `handle_pair_success` takes
    // this lock too, so releasing it first would open a window where a late
    // pair-success completes against the old secret and this then overwrites
    // the secret of a device that just paired.
    replace_adv_secret_key(client).await;
    true
}

/// Re-mint the device's adv secret because the flow that rotated it is dead.
///
/// Stage 2 derives the adv secret from the key bundle and persists it before
/// `companion_finish` goes out, so a flow that ends there leaves the device
/// holding a secret only the primary that failed to link could ever match. WA
/// Web sheds the same value whenever a linking flow restarts —
/// `initializeAltDeviceLinking` clears it and `initializeQRLinking` generates a
/// fresh one (`Alt/DeviceLinkingApi.js`).
///
/// Generated rather than cleared, because `Device::adv_secret_key` is a
/// `[u8; 32]` and has no absent value: an all-zero secret would be a usable key
/// that happens to be wrong, which is worse than an unrelated random one. A
/// later QR flow carries whatever is stored inside the code itself, so a fresh
/// secret is as good as the old one there.
async fn replace_adv_secret_key(client: &Arc<Client>) {
    use rand::RngExt as _;
    let mut adv_secret_key = [0u8; 32];
    rand::make_rng::<rand::rngs::StdRng>().fill(&mut adv_secret_key);
    client
        .persistence_manager
        .process_command(crate::store::commands::DeviceCommand::SetAdvSecretKey(
            adv_secret_key,
        ))
        .await;
    // The QR payload embeds this key, and a code already on screen would now
    // pair against a secret nothing matches — see `Client::refresh_pairing_qr`.
    client.refresh_pairing_qr().await;
}

/// The server asked us to refresh the code we are displaying (WA Web
/// `refreshAltLinkingCode` / `forceManualRefresh`). Surfaces a
/// [`Event::PairingCodeRefresh`] so the consumer re-requests a code, but only
/// when the notification's ref matches the flow currently in progress.
async fn handle_refresh_code(client: &Arc<Client>, reg_node: &NodeRef<'_>) -> bool {
    let notif_ref = match reg_node
        .get_optional_child_by_tag(&["link_code_pairing_ref"])
        .and_then(|n| match n.content.as_ref() {
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
    // WA Web's `getCurrentRef()` guard. A matching one retires the flow on the
    // spot: the consumer is being told to request a replacement, and leaving
    // the old flow standing would make `pair_with_code` reject it. WA Web does
    // the same, re-running `initializeAltDeviceLinking()` on the
    // `force_manual_refresh` path.
    let matches_current = {
        let mut state_guard = client.pair_code_state.lock().await;
        let matches = matches!(
            &*state_guard,
            PairCodeState::WaitingForPhoneConfirmation { pairing_ref, .. }
                if pairing_ref.as_slice() == notif_ref.as_slice()
        );
        if matches {
            *state_guard = PairCodeState::Idle;
        }
        matches
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
    client.core.event_bus.dispatch(Event::PairingCodeRefresh(
        crate::types::events::PairingCodeRefresh::builder()
            .force_manual(force_manual)
            .build(),
    ));
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pin the five arms against `WASmaxInMdIqMixinErrors.parseIqMixinErrors`,
    /// the complete set WA Web's `companion_hello` response parser accepts, so a
    /// renumber can't silently break a consumer's branching.
    #[test]
    fn rejection_codes_match_wa_web() {
        assert_eq!(PairCodeRejection::BadRequest.code(), 400);
        assert_eq!(PairCodeRejection::Forbidden.code(), 403);
        assert_eq!(PairCodeRejection::RateOverlimit.code(), 429);
        assert_eq!(PairCodeRejection::FeatureNotAvailable.code(), 452);
        assert_eq!(PairCodeRejection::InternalServerError.code(), 500);
        // A code outside WA Web's set keeps its number rather than collapsing
        // into a named arm.
        assert_eq!(
            PairCodeRejection::from(418),
            PairCodeRejection::Unknown(418)
        );
    }

    /// `CodeAlreadyOutstanding` is the one failure that must *not* dispatch: a
    /// code is still live, the consumer already has it from the `PairingCode`
    /// that minted it, and an error event would say the opposite while inviting
    /// a retry loop nothing but `cancel_pair_code` or expiry can break.
    #[tokio::test]
    async fn an_outstanding_code_is_not_reported_as_a_failure() {
        let client = create_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        // Park a live code in the slot so the next request is the duplicate.
        let now = wacore::time::now_secs();
        *client.pair_code_state.lock().await = PairCodeState::RequestingCode {
            code_generation_ts: now,
            claim: wacore::pair_code::PairCodeClaim::next(),
        };

        let err = client
            .pair_with_code(PairCodeOptions {
                phone_number: "15551234567".to_string(),
                ..Default::default()
            })
            .await
            .expect_err("a second code must be refused while one is live");
        assert!(
            err.lost_the_flow_to_another_request(),
            "expected CodeAlreadyOutstanding, got: {err:?}"
        );

        // Let any dispatch that was going to happen get through.
        tokio::task::yield_now().await;
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_))),
            "a still-live code must not be reported as 'no code is coming'"
        );
    }

    /// A request cancelled while its `companion_hello` is in flight must not
    /// report either. It resolves *after* a replacement may already own the
    /// slot, so the event would be uncorrelated with the flow actually running
    /// and could make a consumer tear down a live code.
    #[tokio::test]
    async fn a_superseded_request_is_not_reported_as_a_failure() {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        let pending = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        poll_until("the companion_hello to be on the wire", || {
            !transport.sent().is_empty()
        })
        .await;

        client.cancel_pair_code().await;
        answer_companion_hello(&client, &transport, 0, b"3@2:late").await;

        let err = pending
            .await
            .expect("the pair-code task should not panic")
            .expect_err("a cancelled request must not report a usable code");
        assert!(
            err.lost_the_flow_to_another_request(),
            "expected Cancelled, got {err:?}"
        );

        tokio::task::yield_now().await;
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_))),
            "a withdrawn request must not report against the flow that replaced it"
        );
    }

    /// The same suppression must hold when the withdrawn request ends in an *IQ
    /// failure* rather than a late success. A 30 s timeout or a server rejection
    /// easily outlives a `cancel_pair_code` plus its replacement, and reporting
    /// this request's transport failure would then land on the flow that now
    /// owns the slot.
    #[tokio::test]
    async fn a_withdrawn_request_reports_cancellation_not_its_iq_failure() {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        let pending = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        poll_until("the companion_hello to be on the wire", || {
            !transport.sent().is_empty()
        })
        .await;

        client.cancel_pair_code().await;

        // Refuse the withdrawn request's IQ, rather than answering it.
        let hello = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let id = hello
            .get()
            .attrs()
            .optional_string("id")
            .expect("companion_hello carries an id")
            .into_owned();
        let refusal = NodeBuilder::new("iq")
            .attrs([
                ("from", "s.whatsapp.net".to_string()),
                ("type", "error".to_string()),
                ("id", id.clone()),
            ])
            .children([NodeBuilder::new("error")
                .attrs([
                    ("code", "429".to_string()),
                    ("text", "rate-overlimit".to_string()),
                ])
                .build()])
            .build();
        crate::test_utils::answer_iq(&client, &id, &refusal).await;

        let err = pending
            .await
            .expect("the pair-code task should not panic")
            .expect_err("a withdrawn request must not report a usable code");
        assert!(
            err.lost_the_flow_to_another_request(),
            "losing the slot outranks how the request ended, got {err:?}"
        );

        tokio::task::yield_now().await;
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_))),
            "a withdrawn request's IQ failure must not report against its replacement"
        );
    }

    /// Validation runs before the outstanding-flow check, so a second caller
    /// with a bad number fails as `PhoneNumberTooShort` and never reaches the
    /// suppressed variants. It must still stay silent while a code is live —
    /// which is why the suppression asks the state, not the error.
    #[tokio::test]
    async fn a_validation_failure_beside_a_live_code_is_not_reported() {
        let client = create_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        *client.pair_code_state.lock().await = PairCodeState::RequestingCode {
            code_generation_ts: wacore::time::now_secs(),
            claim: wacore::pair_code::PairCodeClaim::next(),
        };

        let err = client
            .pair_with_code(PairCodeOptions {
                phone_number: "123".to_string(),
                ..Default::default()
            })
            .await
            .expect_err("a 3-digit number must be refused");
        assert!(
            matches!(err, PairError::PairCode(PairCodeError::PhoneNumberTooShort)),
            "validation must still win the race it already wins, got {err:?}"
        );

        tokio::task::yield_now().await;
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_))),
            "a live code must not be reported as failed by an unrelated bad request"
        );
    }

    /// WA Web asserts `code` and `text` as a pair and falls to its generic
    /// error path when they disagree, so a contradicting text must not keep
    /// reading as the named arm.
    #[test]
    fn a_contradicting_text_yields_no_classification() {
        let pe: PairError =
            crate::test_utils::server_error_iq(429, "something-else", None, None).into();

        assert_eq!(
            pe.rejection(),
            None,
            "a pairing WA Web would reject must not drive throttle handling"
        );
        // The code is still recoverable from the rendering, so refusing to
        // classify does not lose it.
        assert!(pe.to_string().contains("429"), "got: {pe}");
    }

    /// An absent `text` is not a contradiction. Deliberately laxer than WA Web:
    /// demoting a bare 429 would clear `is_throttled` and put the issue's silent
    /// failure back for the one refusal that most needs acting on.
    #[test]
    fn an_absent_text_still_classifies_by_code() {
        let pe: PairError = crate::test_utils::server_error_iq(429, "", None, None).into();

        assert_eq!(pe.rejection(), Some(PairCodeRejection::RateOverlimit));
        assert!(pe.rejection().is_some_and(PairCodeRejection::is_throttled));
    }

    /// The whole point of the typed status: a 429 is recoverable as
    /// `RateOverlimit` without matching the message.
    #[test]
    fn rate_overlimit_is_recoverable_as_a_typed_status() {
        let pe: PairError =
            crate::test_utils::server_error_iq(429, "rate-overlimit", None, Some(30)).into();

        assert_eq!(pe.rejection(), Some(PairCodeRejection::RateOverlimit));
        assert_eq!(pe.backoff(), Some(std::time::Duration::from_secs(30)));
        assert!(
            pe.rejection().is_some_and(PairCodeRejection::is_throttled),
            "429 must read as throttled"
        );
        // `RequestFailed` renders what it wraps, so a log line that prints only
        // the error still names the refusal.
        assert!(
            pe.to_string().contains("429") && pe.to_string().contains("rate-overlimit"),
            "Display should carry the server's code and text, got: {pe}"
        );
    }

    /// `feature-not-available` is the one refusal that retrying cannot fix — it
    /// must not read as throttled, or a consumer would back off forever instead
    /// of falling back to the QR code the way WA Web does.
    #[test]
    fn feature_not_available_is_not_throttled() {
        let pe: PairError =
            crate::test_utils::server_error_iq(452, "feature-not-available", None, None).into();

        assert_eq!(pe.rejection(), Some(PairCodeRejection::FeatureNotAvailable));
        assert!(!PairCodeRejection::FeatureNotAvailable.is_throttled());
        assert_eq!(pe.backoff(), None);
    }

    /// A failure that never reached the server has no status to report, so
    /// `rejection` stays `None` rather than inventing one.
    #[test]
    fn local_failure_reports_no_rejection() {
        let pe: PairError = PairCodeError::PhoneNumberTooShort.into();
        assert_eq!(pe.rejection(), None);
        assert_eq!(pe.backoff(), None);
    }

    /// The regression the event exists for: a failed request must be observable
    /// on the bus, not only through the `Err` that
    /// `BotBuilder::with_pair_code`'s detached task throws away.
    ///
    /// Uses a validation failure because it needs no server, and it covers the
    /// harder half of the guarantee: the dispatch wraps the whole flow, so even
    /// a path that returns before the IQ is built still reports.
    #[tokio::test]
    async fn failed_request_dispatches_pairing_code_error() {
        let client = create_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        let err = client
            .pair_with_code(PairCodeOptions {
                phone_number: "123".to_string(),
                ..Default::default()
            })
            .await
            .expect_err("a 3-digit number must be refused");
        assert!(matches!(
            err,
            PairError::PairCode(PairCodeError::PhoneNumberTooShort)
        ));

        poll_until("a PairingCodeError to reach the bus", || {
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_)))
        })
        .await;

        let events = collector.events();
        let dispatched = events
            .iter()
            .find_map(|e| match &**e {
                Event::PairingCodeError(e) => Some(e.clone()),
                _ => None,
            })
            .expect("just polled for it");
        assert_eq!(
            dispatched.rejection, None,
            "a local validation failure never reached the server"
        );
        assert_eq!(dispatched.backoff, None);
        assert!(
            dispatched.error.contains("too short"),
            "the message should say what failed, got: {}",
            dispatched.error
        );
    }

    #[test]
    fn pair_error_request_failed_preserves_iq_source() {
        let iq = crate::test_utils::server_error_iq(400, "bad-request", None, None);
        let pe: PairError = iq.into();
        let src = std::error::Error::source(&pe).expect("source preserved");
        let downcast = src.downcast_ref::<IqError>().expect("downcasts to IqError");
        assert!(matches!(downcast, IqError::ServerError { code: 400, .. }));
    }

    #[test]
    fn pair_error_paircode_walks_to_curve_error() {
        use wacore::libsignal::protocol::CurveError;
        // Wrap a wacore PairCodeError that itself carries a CurveError source.
        let pe: PairError =
            PairCodeError::EphemeralKeyAgreement(CurveError::NoKeyTypeIdentifier).into();
        assert_eq!(pe.to_string(), "ephemeral key agreement failed");
        // Hop 1 is the PairCodeError itself: the wrapper no longer erases it.
        let src = std::error::Error::source(&pe).expect("source preserved");
        let pce = src
            .downcast_ref::<PairCodeError>()
            .expect("downcasts to PairCodeError");
        assert!(matches!(pce, PairCodeError::EphemeralKeyAgreement(_)));
        let curve = std::error::Error::source(pce)
            .expect("inner source preserved")
            .downcast_ref::<CurveError>()
            .expect("downcasts to CurveError");
        assert!(matches!(curve, CurveError::NoKeyTypeIdentifier));
    }

    // ── Stage-2 notification handling (WA Web parity guards) ─────────────────
    //
    // All tests drive the top-level `handle_pair_code_notification`, so the
    // `stage` dispatch is exercised end-to-end. The guard-reject paths bail
    // before any stage-2 crypto, so "the adv secret is unchanged" is a reliable
    // proxy for "we did not process the notification"; conversely a valid
    // primary_hello rotates it (via `SetAdvSecretKey`) before the socket send.

    use crate::test_utils::{create_iq_test_client, create_test_client, poll_until};
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
        poll_until(
            "the genuine primary_hello to still reach stage 2 after stale mismatches",
            || adv(&client) != adv_before,
        )
        .await;
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

        poll_until(
            "a valid in-window retry to reach stage 2 and rotate the adv secret",
            || adv(&client) != adv_before,
        )
        .await;
    }

    /// A `refresh_code` whose ref matches the outstanding flow surfaces a
    /// `PairingCodeRefresh` event carrying `force_manual`.
    #[tokio::test]
    async fn refresh_code_matching_ref_dispatches_event() {
        let client = create_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        let pairing_ref = vec![5, 6, 7, 8];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = refresh_code_notif(&pairing_ref, Some(true));
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(handled, "a matching refresh_code should be handled");
        let events = collector.events();
        assert!(
            events
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeRefresh(r) if r.force_manual)),
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
        client.subscribe_handler(collector.clone()).detach();

        let pairing_ref = vec![5, 6, 7, 8];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = refresh_code_notif(&pairing_ref, None);
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(handled, "a matching refresh_code should be handled");
        assert!(
            collector.events().iter().any(|e| matches!(
                &**e,
                Event::PairingCodeRefresh(r) if !r.force_manual
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
        client.subscribe_handler(collector.clone()).detach();

        set_waiting(&client, vec![5, 6, 7, 8], wacore::time::now_secs(), 0).await;

        let notif = refresh_code_notif(&[1, 1, 1, 1], None);
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(!handled, "a non-matching refresh_code must be ignored");
        assert!(
            collector.events().is_empty(),
            "no event should fire for a refresh_code with an unknown ref"
        );
    }

    // ── Requesting a code over a live one ────────────────────────────────────
    //
    // WA Web guards `startAltLinkingFlow` with `invariant(stage === Initialized)`
    // (`Alt/DeviceLinkingApi.js`): a second `companion_hello` may only follow an
    // explicit `initializeAltDeviceLinking()`. Silently minting a second code
    // strands whoever is reading the first one — the server keeps routing
    // `primary_hello` by phone number, so the stale code reaches stage 2 and the
    // primary is handed a key bundle its code cannot open.

    /// Answers the `companion_hello` this flow puts on the wire, so
    /// `pair_with_code` can complete against the harness.
    async fn answer_companion_hello(
        client: &Arc<Client>,
        transport: &Arc<crate::transport::mock::CapturingMockTransport>,
        frame: usize,
        pairing_ref: &[u8],
    ) {
        let hello = crate::test_utils::decode_sent_iq(transport, frame).await;
        let id = hello
            .get()
            .attrs()
            .optional_string("id")
            .expect("companion_hello carries an id")
            .into_owned();
        let response = NodeBuilder::new("iq")
            .attrs([
                ("from", "s.whatsapp.net".to_string()),
                ("type", "result".to_string()),
                ("id", id.clone()),
            ])
            .children([NodeBuilder::new("link_code_companion_reg")
                .attr("stage", "companion_hello")
                .children([NodeBuilder::new("link_code_pairing_ref")
                    .bytes(pairing_ref.to_vec())
                    .build()])
                .build()])
            .build();
        crate::test_utils::answer_iq(client, &id, &response).await;
    }

    fn options() -> PairCodeOptions {
        PairCodeOptions {
            phone_number: "15551234567".to_string(),
            ..Default::default()
        }
    }

    /// Answers the `companion_finish` sitting in `frame` with the server's
    /// `<iq>`; `error` picks between the two refusals WA Web's own parser
    /// accepts (`WASmaxInMdCompanionFinishErrors`: bad-request and
    /// internal-server-error).
    async fn answer_companion_finish(
        client: &Arc<Client>,
        transport: &Arc<crate::transport::mock::CapturingMockTransport>,
        frame: usize,
        error: Option<(u16, &str)>,
    ) {
        let finish = crate::test_utils::decode_sent_iq(transport, frame).await;
        let id = finish
            .get()
            .attrs()
            .optional_string("id")
            .expect("companion_finish carries an id")
            .into_owned();
        let mut response = NodeBuilder::new("iq").attrs([
            ("from", "s.whatsapp.net".to_string()),
            ("id", id.clone()),
            (
                "type",
                if error.is_some() { "error" } else { "result" }.to_string(),
            ),
        ]);
        if let Some((code, text)) = error {
            response = response.children([NodeBuilder::new("error")
                .attrs([("code", code.to_string()), ("text", text.to_string())])
                .build()]);
        }
        crate::test_utils::answer_iq(client, &id, &response.build()).await;
    }

    /// Drives a flow to the point where `companion_finish` is on the wire.
    async fn reach_stage_two(
        client: &Arc<Client>,
        transport: &Arc<crate::transport::mock::CapturingMockTransport>,
        pairing_ref: &[u8],
    ) {
        set_waiting(client, pairing_ref.to_vec(), wacore::time::now_secs(), 0).await;
        let notif = primary_hello_notif(pairing_ref);
        assert!(handle_pair_code_notification(client, &notif.as_node_ref()).await);
        poll_until("companion_finish to reach the transport", || {
            !transport.sent().is_empty()
        })
        .await;
    }

    /// The happy path: the server takes the bundle, so the flow stays open for
    /// the `pair-success` that follows. Proven against the silence timer rather
    /// than by inspection — an accepted answer must reach the end of the window
    /// as silence, never as a refusal.
    #[tokio::test(start_paused = true)]
    async fn an_accepted_companion_finish_keeps_the_flow_open() {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let pairing_ref = vec![1, 2, 3, 4];
        reach_stage_two(&client, &transport, &pairing_ref).await;
        let adv_after_stage_two = adv(&client);

        answer_companion_finish(&client, &transport, 0, None).await;

        advance_past(PairCodeUtils::companion_finish_iq_timeout()).await;
        // The stage-2 task is detached, so a clock jump alone proves nothing —
        // it needs turns of the executor to act on what it received.
        for _ in 0..64 {
            tokio::task::yield_now().await;
        }
        assert!(
            is_waiting(&client).await,
            "an accepted bundle leaves pair-success still due"
        );
        assert_eq!(
            adv(&client),
            adv_after_stage_two,
            "the secret pair-success will verify against must survive"
        );
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_))),
            "nothing failed, so nothing may be reported"
        );
    }

    /// The failure this whole path exists for: the server refuses the key
    /// bundle. WA Web raises `CompanionFinishError` for any answer that is not
    /// `CompanionFinishResponseSuccess` (`Alt/DeviceLinkingIq.js`) and shows
    /// "Something went wrong" rather than waiting out the silence timer. We had
    /// been sending this IQ without reading its answer at all, so a refusal
    /// reached the consumer only as an unexplained minute of nothing.
    #[tokio::test]
    async fn a_refused_companion_finish_reports_the_rejection() {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let pairing_ref = vec![1, 2, 3, 4];
        reach_stage_two(&client, &transport, &pairing_ref).await;
        let adv_after_stage_two = adv(&client);

        answer_companion_finish(&client, &transport, 0, Some((400, "bad-request"))).await;

        poll_until("the refusal to reach the consumer", || {
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_)))
        })
        .await;
        let reported = collector
            .events()
            .iter()
            .find_map(|e| match &**e {
                Event::PairingCodeError(e) => Some(e.clone()),
                _ => None,
            })
            .expect("the refusal was just observed");
        assert_eq!(
            reported.rejection,
            Some(PairCodeRejection::BadRequest),
            "the consumer must be able to branch on the status, not the message"
        );
        assert!(
            !is_waiting(&client).await,
            "a refused flow must free the slot so a replacement can be requested"
        );
        assert_ne!(
            adv(&client),
            adv_after_stage_two,
            "the secret this dead flow rotated must not outlive it"
        );
    }

    /// A refusal that arrives for a flow already replaced belongs to nobody:
    /// reporting it would tell the consumer the live code failed, and re-minting
    /// the adv secret would break the replacement that is about to use it.
    #[tokio::test]
    async fn a_refusal_for_a_replaced_flow_is_not_reported() {
        let (client, _transport) = create_iq_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        // The replacement holds the slot by the time the answer lands.
        set_waiting(&client, vec![9, 9, 9, 9], wacore::time::now_secs(), 0).await;
        let adv_of_replacement = adv(&client);
        report_stage_two_failure(
            &client,
            &[1, 2, 3, 4],
            1,
            crate::test_utils::server_error_iq(500, "internal-server-error", None, None),
        )
        .await;

        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_))),
            "the replacement flow has not failed and must not be reported as failed"
        );
        assert_eq!(
            adv(&client),
            adv_of_replacement,
            "the replacement's adv secret must survive"
        );
        assert!(is_waiting(&client).await, "the replacement keeps the slot");

        // The guard's other half: same ref, but a retry has since opened its own
        // attempt. Covered here because a ref mismatch alone would let a
        // regression that drops the attempt comparison pass unnoticed.
        set_waiting(&client, vec![1, 2, 3, 4], wacore::time::now_secs(), 2).await;
        let adv_of_retry = adv(&client);
        report_stage_two_failure(
            &client,
            &[1, 2, 3, 4],
            1,
            crate::test_utils::server_error_iq(400, "bad-request", None, None),
        )
        .await;

        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_))),
            "an earlier attempt's refusal must not report the retry that replaced it"
        );
        assert_eq!(
            adv(&client),
            adv_of_retry,
            "the retry's adv secret must survive"
        );
        assert!(is_waiting(&client).await, "the retry keeps the slot");
    }

    /// An unanswered `companion_finish` is not a refusal — it is the silence
    /// [`start_pair_success_timeout`] already owns, over a longer window. Ending
    /// the flow on the shorter IQ timeout would cut a link the server may still
    /// be completing.
    #[tokio::test(start_paused = true)]
    async fn an_unanswered_companion_finish_leaves_the_timer_in_charge() {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        reach_stage_two(&client, &transport, &[1, 2, 3, 4]).await;

        advance_past(PairCodeUtils::companion_finish_iq_timeout()).await;
        for _ in 0..64 {
            tokio::task::yield_now().await;
        }
        assert!(
            is_waiting(&client).await,
            "the IQ giving up does not end the flow"
        );
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_))),
            "silence is not a refusal and must not be reported as one"
        );

        advance_past(PairCodeUtils::primary_hello_pair_success_timeout()).await;
        poll_until("the regeneration request", || {
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeRefresh(r) if !r.force_manual))
        })
        .await;
    }

    /// Cancelling a flow that reached stage 2 has to give up the adv secret it
    /// derived: it is keyed to a primary that will never link, and WA Web sheds
    /// the same value on `initializeAltDeviceLinking` (`Alt/DeviceLinkingApi.js`).
    #[tokio::test]
    async fn cancelling_after_stage_two_gives_up_the_secret_it_derived() {
        let (client, transport) = create_iq_test_client().await;
        reach_stage_two(&client, &transport, &[1, 2, 3, 4]).await;
        let rotated = adv(&client);

        client.cancel_pair_code().await;

        assert_ne!(
            adv(&client),
            rotated,
            "the cancelled flow's secret must not outlive it"
        );
    }

    /// The other side of that: a flow cancelled before stage 2 never rotated
    /// anything, and a paired device's secret is the account's own — re-minting
    /// either would be destructive.
    #[tokio::test]
    async fn cancelling_leaves_a_secret_stage_two_never_touched() {
        let (client, _transport) = create_iq_test_client().await;
        set_waiting(&client, vec![1, 2, 3, 4], wacore::time::now_secs(), 0).await;
        let before = adv(&client);
        client.cancel_pair_code().await;
        assert_eq!(adv(&client), before, "no stage 2 ran, so nothing rotated");

        *client.pair_code_state.lock().await = PairCodeState::Completed;
        let paired = adv(&client);
        client.cancel_pair_code().await;
        assert_eq!(
            adv(&client),
            paired,
            "a paired device's adv secret signs its own identity"
        );
    }

    #[tokio::test]
    async fn pair_with_code_refuses_to_supersede_a_live_code() {
        let (client, _transport) = create_iq_test_client().await;
        set_waiting(&client, vec![1, 2, 3, 4], wacore::time::now_secs(), 0).await;

        let err = client
            .pair_with_code(options())
            .await
            .expect_err("a second code would strand the one already displayed");

        assert!(
            matches!(
                err,
                PairError::PairCode(PairCodeError::CodeAlreadyOutstanding { .. })
            ),
            "expected CodeAlreadyOutstanding, got {err:?}"
        );
    }

    /// `cancel_pair_code` is our `initializeAltDeviceLinking()`: the explicit
    /// reset that lets a caller mint a replacement on purpose.
    #[tokio::test]
    async fn cancel_pair_code_lets_a_replacement_be_requested() {
        let (client, transport) = create_iq_test_client().await;
        set_waiting(&client, vec![1, 2, 3, 4], wacore::time::now_secs(), 0).await;

        client.cancel_pair_code().await;

        let pending = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        answer_companion_hello(&client, &transport, 0, b"3@2:fresh").await;
        let code = pending
            .await
            .expect("the pair-code task should not panic")
            .expect("a cancelled flow leaves the way clear");
        assert!(PairCodeUtils::validate_code(&code));
    }

    /// An expired code strands nobody — its holder cannot complete it either —
    /// so it must not block a fresh request.
    #[tokio::test]
    async fn an_expired_code_does_not_block_a_new_one() {
        let (client, transport) = create_iq_test_client().await;
        let stale =
            wacore::time::now_secs() - (PairCodeUtils::code_validity().as_secs() as i64 + 1);
        set_waiting(&client, vec![1, 2, 3, 4], stale, 0).await;

        let pending = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        answer_companion_hello(&client, &transport, 0, b"3@2:fresh").await;
        pending
            .await
            .expect("the pair-code task should not panic")
            .expect("an expired code must not block a new request");
    }

    /// The server's `refresh_code` asks for a replacement, so it must also
    /// clear the way for one — WA Web's `force_manual_refresh` path calls
    /// `initializeAltDeviceLinking()` before the screen re-requests.
    #[tokio::test]
    async fn refresh_code_clears_the_flow_it_asks_to_replace() {
        let client = create_test_client().await;
        let pairing_ref = vec![5, 6, 7, 8];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = refresh_code_notif(&pairing_ref, Some(true));
        assert!(handle_pair_code_notification(&client, &notif.as_node_ref()).await);

        assert!(
            !is_waiting(&client).await,
            "a consumer acting on the refresh must not be rejected by the flow it replaces"
        );
    }

    /// Regression: the guard has to survive two callers, not just two calls.
    /// Checking the state and then releasing the lock across the
    /// `companion_hello` round trip lets both pass, and the second response
    /// overwrites the first flow's key material — so the code returned first
    /// can no longer complete.
    #[tokio::test]
    async fn a_request_racing_another_is_refused_too() {
        let (client, transport) = create_iq_test_client().await;

        let first = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        poll_until("the first companion_hello to be on the wire", || {
            !transport.sent().is_empty()
        })
        .await;

        let second = client.pair_with_code(options()).await;
        assert!(
            matches!(
                second,
                Err(PairError::PairCode(
                    PairCodeError::CodeAlreadyOutstanding { .. }
                ))
            ),
            "a request in flight already owns the slot, got {second:?}"
        );

        answer_companion_hello(&client, &transport, 0, b"3@2:first").await;
        first
            .await
            .expect("the pair-code task should not panic")
            .expect("the winner still completes");
    }

    /// ...and a request that fails must hand the slot back, or the client is
    /// stuck refusing to issue any code at all.
    #[tokio::test]
    async fn a_rejected_request_frees_the_slot() {
        let (client, transport) = create_iq_test_client().await;

        let first = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        let hello = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let id = hello
            .get()
            .attrs()
            .optional_string("id")
            .expect("companion_hello carries an id")
            .into_owned();
        let error = NodeBuilder::new("iq")
            .attrs([
                ("from", "s.whatsapp.net".to_string()),
                ("type", "error".to_string()),
                ("id", id.clone()),
            ])
            .children([NodeBuilder::new("error")
                .attrs([
                    ("code", "400".to_string()),
                    ("text", "bad-request".to_string()),
                ])
                .build()])
            .build();
        crate::test_utils::answer_iq(&client, &id, &error).await;
        first
            .await
            .expect("the pair-code task should not panic")
            .expect_err("the server rejected this one");

        let retry = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        answer_companion_hello(&client, &transport, 1, b"3@2:second").await;
        retry
            .await
            .expect("the pair-code task should not panic")
            .expect("a rejected request must not leave the slot taken");
    }

    /// Move the clock past `d`, after giving spawned tasks a turn to register
    /// their timers. A jump taken before the sleep exists only pushes its
    /// deadline out of reach.
    async fn advance_past(d: std::time::Duration) {
        for _ in 0..64 {
            tokio::task::yield_now().await;
        }
        tokio::time::advance(d + std::time::Duration::from_secs(1)).await;
    }

    /// Regression: a second-granularity stamp is not an identity. Cancel a
    /// request and start its replacement inside the same wall-clock second and
    /// both claims carry the same number, so the first one's late response
    /// installs its own code over the replacement's claim — and its failure
    /// path would release the replacement's.
    #[tokio::test]
    async fn a_claim_is_identified_by_more_than_the_second_it_started_in() {
        let (client, transport) = create_iq_test_client().await;

        let first = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        poll_until("the first companion_hello", || !transport.sent().is_empty()).await;

        // Same second, by construction: no clock advances in between.
        client.cancel_pair_code().await;
        let second = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        poll_until("the replacement's companion_hello", || {
            transport.sent().len() >= 2
        })
        .await;

        answer_companion_hello(&client, &transport, 0, b"3@2:first").await;
        let stale = first
            .await
            .expect("the pair-code task should not panic")
            .expect_err("the cancelled request must not install its flow");
        assert!(
            matches!(stale, PairError::PairCode(PairCodeError::Cancelled)),
            "expected Cancelled, got {stale:?}"
        );

        answer_companion_hello(&client, &transport, 1, b"3@2:second").await;
        second
            .await
            .expect("the pair-code task should not panic")
            .expect("the replacement owns the slot and must complete");
        assert!(
            matches!(
                &*client.pair_code_state.lock().await,
                PairCodeState::WaitingForPhoneConfirmation { pairing_ref, .. }
                    if pairing_ref.as_slice() == b"3@2:second"
            ),
            "the replacement's flow must be the one left standing"
        );
    }

    /// Regression: the code's validity window and the link's are different
    /// clocks. A `primary_hello` accepted near the end of the window leaves
    /// `companion_finish` pending for up to a minute more, and a new request
    /// started in that gap races the pending pair-success for the adv secret.
    #[tokio::test]
    async fn a_pending_pair_success_still_owns_the_slot() {
        let (client, _transport) = create_iq_test_client().await;
        let expired =
            wacore::time::now_secs() - (PairCodeUtils::code_validity().as_secs() as i64 + 1);
        // Stage 2 ran: companion_finish is out, pair-success is pending.
        set_waiting(&client, vec![1, 2, 3, 4], expired, 1).await;

        let err = client
            .pair_with_code(options())
            .await
            .expect_err("a pending link still owns the flow");
        assert!(
            matches!(
                err,
                PairError::PairCode(PairCodeError::CodeAlreadyOutstanding { .. })
            ),
            "expected CodeAlreadyOutstanding, got {err:?}"
        );
    }

    /// Regression: an accepted retry deserves its own response window. Timers
    /// keyed only on the shared `pairing_ref` let the first attempt's timer
    /// retire a flow the second attempt had just renewed.
    #[tokio::test(start_paused = true)]
    async fn a_retry_gets_its_own_response_window() {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let pairing_ref = vec![1, 2, 3, 4];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = primary_hello_notif(&pairing_ref);
        assert!(handle_pair_code_notification(&client, &notif.as_node_ref()).await);
        poll_until("the first companion_finish", || {
            !transport.sent().is_empty()
        })
        .await;

        // Most of the first attempt's window goes by, then the phone retries.
        advance_past(std::time::Duration::from_secs(50)).await;
        let retry = primary_hello_notif(&pairing_ref);
        assert!(handle_pair_code_notification(&client, &retry.as_node_ref()).await);
        poll_until("the second companion_finish", || {
            transport.sent().len() >= 2
        })
        .await;

        // The first attempt's timer is due about now; the retry's is not.
        advance_past(std::time::Duration::from_secs(15)).await;
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeRefresh(_))),
            "the first attempt's timer must not cut the retry's window short"
        );

        advance_past(PairCodeUtils::primary_hello_pair_success_timeout()).await;
        poll_until("the retry's own timeout", || {
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeRefresh(_)))
        })
        .await;
    }

    /// Regression: the pairing ref and any in-flight `companion_hello` belong to
    /// the connection that carried them. Left standing across a teardown, they
    /// make the one-code guard reject the request that reconnecting is supposed
    /// to enable — for the rest of the validity window.
    #[tokio::test]
    async fn a_teardown_does_not_leave_the_slot_claimed() {
        let (client, _transport) = create_iq_test_client().await;
        set_waiting(&client, vec![1, 2, 3, 4], wacore::time::now_secs(), 0).await;

        client.cleanup_connection_state().await;

        assert!(
            matches!(&*client.pair_code_state.lock().await, PairCodeState::Idle),
            "a flow scoped to a dead connection must not outlive it"
        );
    }

    /// Regression: a caller that gives up on the request — a `timeout` shorter
    /// than the IQ's own, say — used to leave its claim behind, and an orphaned
    /// claim rejects every later request for the rest of the validity window.
    #[tokio::test]
    async fn dropping_the_request_hands_the_claim_back() {
        let (client, transport) = create_iq_test_client().await;

        {
            let client = client.clone();
            let task = tokio::spawn(async move { client.pair_with_code(options()).await });
            poll_until("the companion_hello to be on the wire", || {
                !transport.sent().is_empty()
            })
            .await;
            task.abort();
        }

        poll_until("the abandoned claim to be released", || {
            matches!(
                client.pair_code_state.try_lock().as_deref(),
                Some(PairCodeState::Idle)
            )
        })
        .await;
    }

    /// Regression: the claim has to be back *before* the error reaches the
    /// caller. Releasing it only from `Drop` schedules a detached task, and a
    /// caller that retries the moment it sees the failure gets
    /// `CodeAlreadyOutstanding` for a request that already gave up.
    // Paused clock: the failure here is the IQ timing out, and waiting 30s of
    // real time for it would be the slowest test in the suite.
    #[tokio::test(start_paused = true)]
    async fn a_failed_request_hands_the_claim_back_before_it_returns() {
        let (client, _transport) = create_iq_test_client().await;
        client.set_connected_for_test(false);

        client
            .pair_with_code(options())
            .await
            .expect_err("stage 1 cannot complete while disconnected");

        // Checked without awaiting: a detached release would not have run yet.
        assert!(
            matches!(
                client.pair_code_state.try_lock().as_deref(),
                Some(PairCodeState::Idle)
            ),
            "the slot must be free the moment the error is returned"
        );
    }

    /// The predicate stage 1 rechecks before putting `companion_hello` on the
    /// wire. Sending after a withdrawal registers a second flow for this number
    /// on the server, which then routes `primary_hello` to whichever it likes —
    /// the overlap the claim exists to prevent. (The race itself has no
    /// deterministic test: the derivation it runs against is real CPU work.)
    #[tokio::test]
    async fn a_withdrawn_claim_stops_being_owned() {
        let (client, _transport) = create_iq_test_client().await;
        let claim = wacore::pair_code::PairCodeClaim::next();
        *client.pair_code_state.lock().await = PairCodeState::RequestingCode {
            code_generation_ts: wacore::time::now_secs(),
            claim,
        };

        assert!(client.owns_code_claim(claim).await);
        client.cancel_pair_code().await;
        assert!(
            !client.owns_code_claim(claim).await,
            "a cancelled request must not reach the wire"
        );

        // Nor does a replacement's claim answer for the one it superseded.
        *client.pair_code_state.lock().await = PairCodeState::RequestingCode {
            code_generation_ts: wacore::time::now_secs(),
            claim: wacore::pair_code::PairCodeClaim::next(),
        };
        assert!(!client.owns_code_claim(claim).await);
    }

    // ── Stage-2 liveness (WA Web parity) ─────────────────────────────────────

    /// Regression: WA Web acks the `primary_hello` notification before stage 2
    /// runs — `Alt/DeviceLinkingHandleNotification.js` starts
    /// `handlePrimaryHello` without awaiting it and returns the ack in the same
    /// expression. Holding the ack behind a 131k-round PBKDF2 is a divergence
    /// the server sees. Runs on the default current-thread runtime, so the
    /// spawned stage-2 task provably has not been polled when the handler
    /// returns.
    #[tokio::test]
    async fn primary_hello_returns_before_stage_two_reaches_the_wire() {
        let (client, transport) = create_iq_test_client().await;
        let pairing_ref = vec![1, 2, 3, 4];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = primary_hello_notif(&pairing_ref);
        let handled = handle_pair_code_notification(&client, &notif.as_node_ref()).await;

        assert!(handled, "a valid primary_hello is handled");
        assert!(
            transport.sent().is_empty(),
            "the ack must not wait on stage-2 crypto; companion_finish belongs to a later poll"
        );
        poll_until("companion_finish to reach the transport", || {
            !transport.sent().is_empty()
        })
        .await;
    }

    /// Cancelling mid-request must not be undone when stage 1 lands: installing
    /// the flow anyway would revive one the caller dropped, and would overwrite
    /// whatever replaced it.
    #[tokio::test]
    async fn a_cancelled_request_does_not_install_its_flow() {
        let (client, transport) = create_iq_test_client().await;

        let pending = {
            let client = client.clone();
            tokio::spawn(async move { client.pair_with_code(options()).await })
        };
        poll_until("the companion_hello to be on the wire", || {
            !transport.sent().is_empty()
        })
        .await;

        client.cancel_pair_code().await;
        answer_companion_hello(&client, &transport, 0, b"3@2:late").await;

        let err = pending
            .await
            .expect("the pair-code task should not panic")
            .expect_err("a cancelled request must not report a usable code");
        assert!(
            matches!(err, PairError::PairCode(PairCodeError::Cancelled)),
            "expected Cancelled, got {err:?}"
        );
        assert!(
            !is_waiting(&client).await,
            "the cancelled flow must stay cancelled"
        );
    }

    /// Regression: a stage-2 task can be scheduled and then find, once it has
    /// the lock, that its flow was replaced rather than merely retired. Matching
    /// on the variant alone reads the replacement as its own flow, and it would
    /// then persist a retired adv secret and answer with a `companion_finish`
    /// keyed to the old ref — breaking the replacement that was about to work.
    #[tokio::test]
    async fn a_stage_two_task_does_not_answer_for_the_flow_that_replaced_it() {
        let (client, transport) = create_iq_test_client().await;
        // The replacement: a live flow, but not the one stage 2 was spawned for.
        set_waiting(&client, vec![9, 9, 9, 9], wacore::time::now_secs(), 0).await;
        let adv_before = adv(&client);

        run_stage_two(
            client.clone(),
            vec![1, 2, 3, 4],
            "15551234567".to_string(),
            "ABCD1234".to_string(),
            KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>()),
            vec![7u8; 80],
            [9u8; 32],
            1,
        )
        .await;

        assert_eq!(
            adv(&client),
            adv_before,
            "the replacement flow's adv secret must survive"
        );
        assert!(
            transport.sent().is_empty(),
            "no companion_finish may go out for a ref nobody is holding"
        );
    }

    /// Regression: `companion_finish` leaving the socket is not the end of the
    /// flow — the server may still never send `pair-success` (a primary that
    /// could not open the key bundle simply goes quiet). WA Web arms a
    /// one-minute timer on `primary_hello_received`
    /// (`Link/DevicePhoneNumberCodeScreen.react.js`) and regenerates the code
    /// when it fires; we had no timeout at all, leaving the consumer with a
    /// code that will never complete and no signal that anything went wrong.
    #[tokio::test(start_paused = true)]
    async fn a_primary_hello_that_never_pairs_asks_for_a_new_code() {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let pairing_ref = vec![1, 2, 3, 4];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = primary_hello_notif(&pairing_ref);
        assert!(handle_pair_code_notification(&client, &notif.as_node_ref()).await);
        poll_until("companion_finish to reach the transport", || {
            !transport.sent().is_empty()
        })
        .await;

        advance_past(PairCodeUtils::primary_hello_pair_success_timeout()).await;
        poll_until("the regeneration request", || {
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeRefresh(r) if !r.force_manual))
        })
        .await;
        assert!(
            !is_waiting(&client).await,
            "the abandoned flow must not reject the replacement it just asked for"
        );
    }

    /// A stage 2 that cannot even reach the socket ends the flow there and
    /// says so, rather than leaving the consumer to infer it from a minute of
    /// silence. WA Web does the same: `sendCompanionFinish` throwing reaches
    /// `handlePrimaryHello`, which fires `errorAltLinking` immediately
    /// (`Alt/DeviceLinkingApi.js`).
    #[tokio::test]
    async fn a_stage_two_that_cannot_send_reports_the_failure_at_once() {
        // No transport, so the companion_finish send fails.
        let client = create_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let pairing_ref = vec![1, 2, 3, 4];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = primary_hello_notif(&pairing_ref);
        assert!(handle_pair_code_notification(&client, &notif.as_node_ref()).await);

        poll_until("the failure to reach the consumer", || {
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeError(_)))
        })
        .await;
        assert!(
            !is_waiting(&client).await,
            "a flow that could not send its bundle must not keep the slot"
        );
    }

    /// The timer must not fire once pairing actually completed, or a freshly
    /// linked client would be told to hand out a new code.
    #[tokio::test(start_paused = true)]
    async fn pair_success_silences_the_regeneration_timer() {
        let (client, transport) = create_iq_test_client().await;
        let collector = Arc::new(crate::test_utils::TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        let pairing_ref = vec![1, 2, 3, 4];
        set_waiting(&client, pairing_ref.clone(), wacore::time::now_secs(), 0).await;

        let notif = primary_hello_notif(&pairing_ref);
        assert!(handle_pair_code_notification(&client, &notif.as_node_ref()).await);
        poll_until("companion_finish to reach the transport", || {
            !transport.sent().is_empty()
        })
        .await;

        // What `handle_pair_success` does once the server confirms the link.
        *client.pair_code_state.lock().await = PairCodeState::Completed;

        advance_past(PairCodeUtils::primary_hello_pair_success_timeout()).await;
        // The timer task is spawned, so it needs turns of the executor, not just
        // a clock jump — `poll_until` would return on its first check here.
        for _ in 0..64 {
            tokio::task::yield_now().await;
        }
        assert!(
            !collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingCodeRefresh(_))),
            "a completed pairing must not ask the consumer for another code"
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
