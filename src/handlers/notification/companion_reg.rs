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
use wacore_binary::NodeRef;

/// The two children WA Web's parser accepts on this notification.
const REFRESH_CHILDREN: [&str; 2] = ["companion_reg_refresh", "pair-device-rotate-qr"];

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
    // unconditionally. Past stage 2 a phone-number flow has already derived the
    // adv secret that the pair-success HMAC will be computed over, so rotating
    // it there turns a link that was about to succeed into one that cannot. We
    // deliberately cover the whole flow, not just that half: this request is
    // about the QR payload, which an outstanding pair code is in the process of
    // replacing anyway. The pending half outlives the code's own validity
    // window, so it is tracked separately from it.
    let defer_to_pair_code = {
        let state = client.pair_code_state.lock().await;
        state
            .live_flow_remaining(wacore::time::now_secs())
            .is_some()
            || state.awaiting_pair_success()
    };
    if defer_to_pair_code {
        debug!(
            target: "Client/PairRefresh",
            "Server asked to refresh companion registration; keeping the adv secret an outstanding pair-code flow depends on"
        );
        return;
    }

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
        "Server asked to refresh companion registration; rotated the adv secret"
    );
}
