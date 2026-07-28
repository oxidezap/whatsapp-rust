//! `<notification type="companion_reg_refresh">` — the server retiring an
//! unpaired companion's registration material.
//!
//! WA Web (`Handle/CompanionReqRefreshNotification.js`) accepts the stanza with
//! either a `companion_reg_refresh` or a `pair-device-rotate-qr` child, rejects
//! it outright when neither is present, and answers by regenerating the ADV
//! secret key. That key is what the QR payload advertises, so ignoring the
//! request leaves us handing out a QR built on a secret the server has retired.

use crate::client::Client;
use log::{debug, warn};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use wacore_binary::NodeRef;

/// The two children WA Web's parser accepts on this notification.
const REFRESH_CHILDREN: [&str; 2] = ["companion_reg_refresh", "pair-device-rotate-qr"];

/// Rotate the ADV secret and re-render the QR that advertises it.
///
/// Also the landing point for a refresh that had to wait: see
/// [`apply_pending_reg_refresh`].
pub(crate) async fn rotate_companion_registration(client: &Arc<Client>) {
    use rand::Rng as _;
    let mut secret = [0u8; 32];
    rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut secret);
    client
        .persistence_manager
        .process_command(wacore::store::commands::DeviceCommand::SetAdvSecretKey(
            secret,
        ))
        .await;
    debug!(
        target: "Client/PairRefresh",
        "Rotated the adv secret the server asked to retire"
    );
    // The QR on screen embeds the old secret; re-render it rather than let it
    // stay scannable until its ref expires.
    client.refresh_pairing_qr().await;
}

/// Apply a refresh that was deferred to a pair-code flow which then ended
/// without pairing. Without this the server's request is simply lost, and the
/// QR keeps advertising material it asked to retire until the next reconnect.
pub(crate) async fn apply_pending_reg_refresh(client: &Arc<Client>) {
    if client.pending_reg_refresh.swap(false, Ordering::Relaxed) {
        debug!(target: "Client/PairRefresh", "Applying the registration refresh a pair-code flow had deferred");
        rotate_companion_registration(client).await;
    }
}

pub(super) async fn handle_companion_reg_refresh(client: &Arc<Client>, node: &NodeRef<'_>) {
    if !REFRESH_CHILDREN
        .iter()
        .any(|tag| node.get_optional_child_by_tag(&[tag]).is_some())
    {
        warn!(
            target: "Client/PairRefresh",
            "companion_reg_refresh carries neither companion_reg_refresh nor pair-device-rotate-qr; ignoring"
        );
        return;
    }

    // The one place we knowingly diverge from WA Web, which rotates
    // unconditionally. Once stage 2 has run, this is the secret the pending
    // pair-success HMAC is computed over, and rotating it turns a link that was
    // about to succeed into one that cannot. A code that is only displayed does
    // not qualify: stage 2 derives its own secret when the phone answers, so
    // deferring there would protect nothing while leaving the QR that shares
    // this connection advertising material the server just retired.
    {
        // Held across the write, not just the check: stage 2 derives its secret
        // and builds `companion_finish` under this same lock, so releasing it
        // in between would let a `primary_hello` land in the gap and have its
        // secret overwritten — the pair-success would then be verified against
        // the wrong one.
        let state = client.pair_code_state.lock().await;
        if state.awaiting_pair_success() {
            debug!(
                target: "Client/PairRefresh",
                "Server asked to refresh companion registration; keeping the adv secret a pending pair-success depends on"
            );
            // Deferred, not dropped: if that flow ends without pairing, the
            // secret the server retired is still the one the QR advertises.
            client.pending_reg_refresh.store(true, Ordering::Relaxed);
            return;
        }

        rotate_companion_registration(client).await;
        drop(state);
    }
}
