use crate::client::Client;
use crate::lid_pn_cache::LearningSource;
use crate::types::events::{Event, PairError, PairSuccess};
use log::{debug, error, info, warn};

use std::sync::Arc;
use std::sync::atomic::Ordering;
use wacore::companion_reg::companion_web_client_type_for_props;
use wacore::libsignal::protocol::KeyPair;
use wacore_binary::NodeRef;
use wacore_binary::{Jid, SERVER_JID};

pub use wacore::companion_reg::{CompanionWebClientType, NATIVE_CAMERA_DEEP_LINK_PREFIX};
pub use wacore::pair::{DeviceState, PairCryptoError, PairUtils};

/// Auto-derives client type from `device_props`; see
/// [`make_qr_data_with_client_type`] to override.
pub fn make_qr_data(store: &crate::store::Device, ref_str: &str) -> String {
    let client_type = companion_web_client_type_for_props(&store.device_props);
    make_qr_data_with_client_type(store, ref_str, client_type)
}

pub fn make_qr_data_with_client_type(
    store: &crate::store::Device,
    ref_str: &str,
    client_type: CompanionWebClientType,
) -> String {
    let device_state = DeviceState {
        identity_key: store.identity_key.clone(),
        noise_key: store.noise_key.clone(),
        adv_secret_key: store.adv_secret_key,
    };
    PairUtils::make_qr_data(&device_state, ref_str, client_type)
}

impl Client {
    /// Ask the QR rotation task to re-render the ref it is currently showing.
    ///
    /// The QR payload embeds the adv secret key, so anything that re-mints that
    /// key has to reach the code already on screen — waiting out the current
    /// ref would leave a scannable code whose pairing data `handle_pair_success`
    /// then verifies against a secret that no longer matches. WA Web drives its
    /// QR re-render from an adv-secret change through the same path as a ref
    /// change (`Link/DeviceQrcode.react.js`).
    ///
    /// A no-op when no rotation is in progress.
    pub(crate) async fn refresh_pairing_qr(self: &Arc<Self>) {
        if let Some(tx) = self.pairing_qr_refresh_tx.lock().await.as_ref() {
            // Full means a refresh is already queued, which is just as good.
            let _ = tx.try_send(());
        }
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.pair.handle_iq", level = "debug", skip_all)
)]
pub async fn handle_iq(client: &Arc<Client>, node: &NodeRef<'_>) -> bool {
    // Server JID is "s.whatsapp.net" (no @ prefix for server-only JIDs)
    if node.get_attr("from").is_none_or(|v| v != SERVER_JID) {
        return false;
    }

    if let Some(children) = node.children() {
        for child in children {
            let handled = match child.tag.as_ref() {
                "pair-device" => {
                    if let Some(ack_node) = PairUtils::build_ack_node_ref(node)
                        && let Err(e) = client.send_node(ack_node).await
                    {
                        warn!("Failed to send acknowledgement: {e:?}");
                    }

                    // Refs, not payloads: the QR string embeds the ADV secret,
                    // and anything that re-mints it mid-rotation (see the
                    // `companion_reg_refresh` handler) would leave every queued
                    // payload advertising a retired one. WA Web builds each
                    // payload as it publishes the ref, from the state of that
                    // moment.
                    let refs: Vec<String> = child
                        .get_children_by_tag("ref")
                        .filter_map(|grandchild| {
                            let bytes = grandchild.content_bytes()?;
                            std::str::from_utf8(bytes).ok().map(str::to_owned)
                        })
                        .collect();

                    let (stop_tx, stop_rx) = async_channel::bounded::<()>(1);
                    let (refresh_tx, refresh_rx) = async_channel::bounded::<()>(1);
                    // Published before the task can run: `spawn` may poll it
                    // immediately or on another thread, and a refresh arriving
                    // in that window would find no sender and be dropped —
                    // leaving the code already on screen keyed to a secret the
                    // server has retired.
                    *client.pairing_cancellation_tx.lock().await = Some(stop_tx);
                    *client.pairing_qr_refresh_tx.lock().await = Some(refresh_tx);
                    let client_clone = client.clone();

                    client
                        .runtime
                        .spawn(Box::pin(async move {
                            let mut is_first = true;

                            'refs: for pairing_ref in refs {
                                // Guard: pairing may complete before this task gets polled
                                // (single-threaded runtimes, fast auto-pair, mock servers)
                                if client_clone.is_logged_in() {
                                    info!("Already logged in, stopping QR rotation.");
                                    return;
                                }

                                let ttl = if is_first {
                                    is_first = false;
                                    std::time::Duration::from_secs(60)
                                } else {
                                    std::time::Duration::from_secs(20)
                                };
                                let started = wacore::time::Instant::now();

                                // Re-rendering the ref already on screen is its
                                // own loop, because the payload embeds the adv
                                // secret: a rotation mid-ref would otherwise
                                // leave a scannable code keyed to a secret the
                                // server has retired, for as long as this ref
                                // has left. WA Web re-renders through one path
                                // for a ref change and for an adv-secret change
                                // (`Link/DeviceQrcode.react.js`).
                                // One sleep for the whole ref, polled across
                                // re-renders rather than recreated by them: it
                                // is the runtime's own clock that decides when
                                // this ref is spent, and restarting it would
                                // let a refresh extend a ref past the deadline
                                // the server set. `started` only feeds the
                                // advisory countdown on the event.
                                let sleep = client_clone.runtime.sleep(ttl);
                                futures::pin_mut!(sleep);

                                loop {
                                    let snapshot =
                                        client_clone.persistence_manager.get_device_snapshot();
                                    let code = PairUtils::make_qr_data(
                                        &DeviceState {
                                            identity_key: snapshot.identity_key.clone(),
                                            noise_key: snapshot.noise_key.clone(),
                                            adv_secret_key: snapshot.adv_secret_key,
                                        },
                                        &pairing_ref,
                                        companion_web_client_type_for_props(&snapshot.device_props),
                                    );
                                    client_clone.core.event_bus.dispatch(Event::PairingQrCode(
                                        crate::types::events::PairingQrCode::builder()
                                            .code(code)
                                            .timeout(ttl.saturating_sub(started.elapsed()))
                                            .build(),
                                    ));

                                    let stop = stop_rx.recv();
                                    let refresh = refresh_rx.recv();
                                    futures::pin_mut!(stop);
                                    futures::pin_mut!(refresh);
                                    let outcome = futures::future::select(
                                        sleep.as_mut(),
                                        futures::future::select(stop, refresh),
                                    )
                                    .await;
                                    match outcome {
                                        futures::future::Either::Left(_) => {
                                            if client_clone.is_logged_in() {
                                                info!(
                                                    "Logged in during QR timeout, stopping rotation."
                                                );
                                                return;
                                            }
                                            continue 'refs;
                                        }
                                        futures::future::Either::Right((
                                            futures::future::Either::Left(_),
                                            _,
                                        )) => {
                                            info!("Pairing complete. Stopping QR code rotation.");
                                            return;
                                        }
                                        // Same ref and deadline, rebuilt payload.
                                        futures::future::Either::Right((
                                            futures::future::Either::Right(_),
                                            _,
                                        )) => continue,
                                }
                                }
                            }

                            if client_clone.is_logged_in() {
                                return;
                            }

                            // WA Web stops here without closing the socket: its
                            // rotation timer (`Handle/PairDevice.js`) cancels
                            // itself and reports `UNPAIRED_IDLE`, leaving the
                            // "click to reload" decision to the surface above.
                            // That matters because the same connection can be
                            // carrying a phone-number flow, whose code stays
                            // valid past the rotation budget the six refs buy
                            // (60s + 5×20s) — disconnecting would revoke a code
                            // we advertised as still good, and the server would
                            // route the eventual `primary_hello` to a session
                            // that no longer exists. QR-only callers keep the
                            // self-disconnect they rely on to get fresh refs.
                            let pair_code_outstanding = client_clone
                                .pair_code_state
                                .lock()
                                .await
                                .is_outstanding(wacore::time::now_secs());

                            info!(
                                "All QR codes for this session have expired\
                                 (pair-code flow outstanding: {pair_code_outstanding})."
                            );
                            client_clone
                                .core
                                .event_bus
                                .dispatch(Event::PairingQrCodesExhausted(
                                    crate::types::events::PairingQrCodesExhausted::builder()
                                        .disconnected(!pair_code_outstanding)
                                        .build(),
                                ));
                            if !pair_code_outstanding {
                                client_clone.disconnect().await;
                            }
                        }))
                        .detach();

                    true
                }
                "pair-success" => {
                    handle_pair_success(client, node, child).await;
                    true
                }
                _ => false,
            };
            if handled {
                return true;
            }
        }
    }

    false
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.pair.success", level = "debug", skip_all)
)]
async fn handle_pair_success<'a>(
    client: &Arc<Client>,
    request_node: &NodeRef<'a>,
    success_node: &NodeRef<'a>,
) {
    client.update_server_time_offset(request_node);

    let req_id = match request_node.get_attr("id").map(|v| v.as_str()) {
        Some(id) => id.into_owned(),
        None => {
            error!("Received pair-success without request ID");
            return;
        }
    };

    let device_identity_bytes = match PairUtils::extract_device_identity_bytes(success_node) {
        Some(b) => b,
        None => {
            let error_node = PairUtils::build_pair_error_node(&req_id, 500, "internal-error");
            if let Err(e) = client.send_node(error_node).await {
                error!("Failed to send pair error node: {e}");
            }
            error!("pair-success is missing device-identity");
            return;
        }
    };

    let business_name = success_node
        .get_optional_child_by_tag(&["biz"])
        .map(|n| {
            n.get_attr("name")
                .map(|v| v.as_str().into_owned())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    let platform = success_node
        .get_optional_child_by_tag(&["platform"])
        .map(|n| {
            n.get_attr("name")
                .map(|v| v.as_str().into_owned())
                .unwrap_or_default()
        })
        .unwrap_or_default();

    // For jid and lid, parse them together to handle errors correctly
    let (jid, lid) = if let Some(device_node) = success_node.get_optional_child_by_tag(&["device"])
    {
        let mut parser = device_node.attrs();
        let parsed_jid = parser.optional_jid("jid").unwrap_or_default();
        let parsed_lid = parser.optional_jid("lid").unwrap_or_default();
        (parsed_jid, parsed_lid)
    } else {
        (Jid::default(), Jid::default())
    };

    // Taken before the secret is read, not just before the state is written: a
    // pair-code flow being retired re-mints the adv secret under this same lock,
    // and verifying against the old value and then completing against the new
    // one would persist a paired device whose ADV signatures cannot validate.
    let mut pair_code_state = client.pair_code_state.lock().await;

    let device_snapshot = client.persistence_manager.get_device_snapshot();
    let device_state = DeviceState {
        identity_key: device_snapshot.identity_key.clone(),
        noise_key: device_snapshot.noise_key.clone(),
        adv_secret_key: device_snapshot.adv_secret_key,
    };

    let result = PairUtils::do_pair_crypto(&device_state, device_identity_bytes);

    match result {
        Ok((self_signed_identity_bytes, key_index)) => {
            // Both retired only once the identity and its HMAC check out. A
            // pair-success that arrives after its own flow was written off can
            // otherwise cancel the replacement that succeeded it, and then fail
            // verification against the secret that replacement derived —
            // leaving neither able to complete. Stopping the QR rotation early
            // is the same mistake in the other direction: a rejected response
            // would take the displayed code down with it, and nothing puts one
            // back.
            *pair_code_state = wacore::pair_code::PairCodeState::Completed;
            drop(pair_code_state);
            if let Some(tx) = client.pairing_cancellation_tx.lock().await.take() {
                let _ = tx.try_send(());
                debug!("Sent QR rotation stop signal");
            } else {
                debug!(
                    "QR rotation channel not yet stored — is_logged_in guard will stop the task"
                );
            }

            let signed_identity_for_event = match waproto::codec::adv_signed_device_identity_decode(
                self_signed_identity_bytes.as_slice(),
            ) {
                Ok(identity) => identity,
                Err(e) => {
                    error!(
                        "FATAL: Failed to re-decode self-signed identity for event, pairing cannot complete: {e}"
                    );
                    client.core.event_bus.dispatch(Event::PairError(
                        PairError::builder()
                            .id(jid.clone())
                            .lid(lid.clone())
                            .business_name(business_name.clone())
                            .platform(platform.clone())
                            .error(format!(
                                "internal error: failed to decode identity for event: {e}"
                            ))
                            .build(),
                    ));
                    return;
                }
            };

            client
                .persistence_manager
                .process_command(crate::store::commands::DeviceCommand::SetId(Some(
                    jid.clone(),
                )))
                .await;
            client
                .persistence_manager
                .process_command(crate::store::commands::DeviceCommand::SetAccount(Some(
                    signed_identity_for_event.clone(),
                )))
                .await;
            client
                .persistence_manager
                .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                    lid.clone(),
                )))
                .await;

            // The primary reports whether the account is 1:1-LID-migrated via
            // <client-props> (WA Web HandlePairSuccess -> setIsLidMigrated).
            // This gates outbound DM wire addressing between LID and PN.
            // Pairing a different account must not inherit the previous
            // account's state, but a same-account relink whose pair-success
            // omitted client-props must not lose it either.
            let props_migrated = PairUtils::extract_pairing_props(success_node)
                .is_some_and(|props| props.is_chat_db_lid_migrated.unwrap_or(false));
            let account_changed = device_snapshot
                .pn
                .as_ref()
                .is_none_or(|prev| prev.user != jid.user);
            if let Some(lid_migrated) =
                PairUtils::lid_migrated_update(props_migrated, account_changed)
            {
                info!("Account 1:1-LID-migrated (pair-success client-props): {lid_migrated}");
                client
                    .persistence_manager
                    .process_command(crate::store::commands::DeviceCommand::SetLidMigrated(
                        lid_migrated,
                    ))
                    .await;
            }

            // A prior pairing's `server_has_prekeys=true` would make
            // `upload_pre_keys_at_login` skip and leave the server bundle stale.
            // Reset it so the next connect re-uploads, matching WA Web where a
            // freshly registered device always uploads its prekeys.
            client
                .persistence_manager
                .modify_device(|d| d.server_has_prekeys = false)
                .await;

            // Add the own LID-PN mapping to the cache so that when sending DMs to self,
            // we can find the existing LID-based session instead of creating a new PN-based one.
            // This is critical for self-messaging to work correctly.
            if !jid.user.is_empty() && !lid.user.is_empty() {
                if let Err(err) = client
                    .add_lid_pn_mapping(&lid.user, &jid.user, LearningSource::Pairing)
                    .await
                {
                    warn!(
                        "Failed to persist own LID-PN mapping {} <-> {}: {err}",
                        lid.user, jid.user
                    );
                } else {
                    info!(
                        "Added own LID-PN mapping to cache: {} <-> {}",
                        lid.user, jid.user
                    );
                }
            }

            if !business_name.is_empty() {
                info!("✅ Setting push_name during pairing: '{}'", business_name);
                client
                    .persistence_manager
                    .process_command(crate::store::commands::DeviceCommand::SetPushName(
                        business_name.clone(),
                    ))
                    .await;
            } else {
                info!(
                    "⚠️ business_name not found in pair-success, push_name remains unset for now."
                );
            }

            let response_node = PairUtils::build_pair_success_response(
                &req_id,
                self_signed_identity_bytes,
                key_index,
            );

            if let Err(e) = client.send_node(response_node).await {
                error!("Failed to send pair-device-sign: {e}");
                return;
            }

            let client_for_unified = client.clone();
            client
                .runtime
                .spawn(Box::pin(async move {
                    client_for_unified.send_unified_session().await;
                }))
                .detach();

            // --- START: FIX ---
            // Set the flag to trigger a full sync on the next successful connection.
            client.needs_initial_full_sync.arm_for_pairing();
            // --- END: FIX ---

            client.expected_disconnect.store(true, Ordering::Relaxed);

            info!("Successfully paired {}", jid.observe());

            let success_event = PairSuccess::builder()
                .id(jid)
                .lid(lid)
                .business_name(business_name)
                .platform(platform)
                .build();
            client
                .core
                .event_bus
                .dispatch(Event::PairSuccess(success_event));
        }
        Err(e) => {
            // Nothing left to keep atomic once verification failed: no pairing
            // will be completed against this secret, and the refusal below is a
            // socket write a retirement should not have to queue behind.
            drop(pair_code_state);
            error!("Pairing crypto failed: {e}");
            let error_node = PairUtils::build_pair_error_node(&req_id, e.code, e.text);
            if let Err(send_err) = client.send_node(error_node).await {
                error!("Failed to send pair error node: {send_err}");
            }

            let pair_error_event = PairError::builder()
                .id(jid)
                .lid(lid)
                .business_name(business_name)
                .platform(platform)
                .error(e.to_string())
                .build();
            client
                .core
                .event_bus
                .dispatch(Event::PairError(pair_error_event));
        }
    }
}

#[cfg_attr(
    feature = "tracing",
    tracing::instrument(name = "wa.pair.qr", level = "debug", skip_all, err(Debug))
)]
pub async fn pair_with_qr_code(client: &Arc<Client>, qr_code: &str) -> Result<(), anyhow::Error> {
    info!(target: "Client/PairTest", "Master client attempting to pair with QR code.");

    let (pairing_ref, dut_noise_pub, dut_identity_pub) = PairUtils::parse_qr_code(qr_code)?;

    let master_ephemeral = KeyPair::generate(&mut rand::make_rng::<rand::rngs::StdRng>());

    let device_snapshot = client.persistence_manager.get_device_snapshot();
    let device_state = DeviceState {
        identity_key: device_snapshot.identity_key.clone(),
        noise_key: device_snapshot.noise_key.clone(),
        adv_secret_key: device_snapshot.adv_secret_key,
    };

    let encrypted = PairUtils::prepare_master_pairing_message(
        &device_state,
        &pairing_ref,
        &dut_noise_pub,
        &dut_identity_pub,
        master_ephemeral,
    )?;

    let master_jid = device_snapshot
        .pn
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Cannot pair: device has no phone number JID configured"))?;
    let req_id = client.generate_request_id();

    let iq = PairUtils::build_master_pair_iq(&master_jid, encrypted, req_id);

    client.send_node(iq).await?;

    info!(target: "Client/PairTest", "Master client sent pairing confirmation.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{TestEventCollector, create_iq_test_client, poll_until};
    use wacore::pair_code::PairCodeState;
    use wacore_binary::Node;
    use wacore_binary::builder::NodeBuilder;

    /// The six refs the server hands out in one `<pair-device>`, and the
    /// rotation budget they buy: WA Web `Handle/PairDevice.js` waits 60s on the
    /// first (`u = 6e4`) and 20s on each of the rest (`c = 20 * 1e3`).
    const QR_REFS: usize = 6;

    fn qr_codes_seen(collector: &Arc<TestEventCollector>) -> usize {
        collector
            .events()
            .iter()
            .filter(|e| matches!(&***e, Event::PairingQrCode(_)))
            .count()
    }

    /// Burn the whole rotation budget without a real-time wait.
    ///
    /// Each step waits for the code to actually be published before moving the
    /// clock: the rotation task must have registered its sleep first, or the
    /// jump lands before the timer exists and the deadline is simply pushed out
    /// of reach.
    async fn exhaust_qr_rotation(collector: &Arc<TestEventCollector>) {
        for nth in 1..=QR_REFS {
            poll_until("the next QR code to be published", || {
                qr_codes_seen(collector) >= nth
            })
            .await;
            // Longer than the first ref's 60s, which covers the 20s ones too.
            tokio::time::advance(std::time::Duration::from_secs(61)).await;
        }
    }

    fn pair_device_iq() -> Node {
        NodeBuilder::new("iq")
            .attrs([
                ("from", SERVER_JID.to_string()),
                ("type", "set".to_string()),
                ("id", "pair-1".to_string()),
                ("xmlns", "md".to_string()),
            ])
            .children([NodeBuilder::new("pair-device")
                .children((0..QR_REFS).map(|i| {
                    NodeBuilder::new("ref")
                        .bytes(format!("2@ref{i}").into_bytes())
                        .build()
                }))
                .build()])
            .build()
    }

    async fn set_pair_code_waiting(client: &Arc<Client>) {
        *client.pair_code_state.lock().await = PairCodeState::WaitingForPhoneConfirmation {
            pairing_ref: b"3@2:ref".to_vec(),
            phone_jid: "15551234567".to_string(),
            pair_code: "ABCD1234".to_string(),
            ephemeral_keypair: Box::new(KeyPair::generate(
                &mut rand::make_rng::<rand::rngs::StdRng>(),
            )),
            code_generation_ts: wacore::time::now_secs(),
            primary_hello_attempt_count: 0,
        };
    }

    /// Regression: the six payloads used to be built once, from a single
    /// snapshot, and handed to the rotation task. Anything that re-mints the ADV
    /// secret mid-rotation (see the `companion_reg_refresh` handler) then left
    /// every queued payload advertising a secret the server had retired, so a
    /// scan produced pairing data `handle_pair_success` verifies against the new
    /// one. WA Web builds each payload when it publishes the ref, not up front.
    #[tokio::test(start_paused = true)]
    async fn each_qr_payload_carries_the_adv_secret_of_its_own_moment() {
        use base64::Engine as _;

        let (client, _transport) = create_iq_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        let iq = pair_device_iq();
        assert!(handle_iq(&client, &iq.as_node_ref()).await);
        poll_until("the first QR code", || qr_codes_seen(&collector) >= 1).await;

        let rotated = [7u8; 32];
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetAdvSecretKey(
                rotated,
            ))
            .await;
        tokio::time::advance(std::time::Duration::from_secs(61)).await;
        poll_until("the second QR code", || qr_codes_seen(&collector) >= 2).await;

        let expected = base64::engine::general_purpose::STANDARD.encode(rotated);
        let codes: Vec<String> = collector
            .events()
            .iter()
            .filter_map(|e| match &**e {
                Event::PairingQrCode(qr) => Some(qr.code.clone()),
                _ => None,
            })
            .collect();
        assert!(
            !codes[0].contains(&expected),
            "the first code predates the rotation"
        );
        assert!(
            codes[1].contains(&expected),
            "a code published after the rotation must advertise the new secret"
        );
    }

    /// Regression: rotating the ADV secret has to reach the code already on
    /// screen, not just the next one. WA Web listens on `advSecretEventEmitter`
    /// and re-renders through the same path as a ref change
    /// (`Link/DeviceQrcode.react.js`); waiting out the current 20s or 60s ref
    /// leaves a QR whose scan produces pairing data verified against a secret
    /// that no longer matches.
    #[tokio::test(start_paused = true)]
    async fn rotating_the_adv_secret_re_emits_the_qr_on_screen() {
        use base64::Engine as _;

        let (client, _transport) = create_iq_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        let iq = pair_device_iq();
        assert!(handle_iq(&client, &iq.as_node_ref()).await);
        poll_until("the first QR code", || qr_codes_seen(&collector) >= 1).await;

        let rotated = [7u8; 32];
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetAdvSecretKey(
                rotated,
            ))
            .await;
        client.refresh_pairing_qr().await;

        // No clock movement: the current ref is nowhere near expiry.
        poll_until("the QR to be re-emitted", || qr_codes_seen(&collector) >= 2).await;
        let expected = base64::engine::general_purpose::STANDARD.encode(rotated);
        let codes: Vec<String> = collector
            .events()
            .iter()
            .filter_map(|e| match &**e {
                Event::PairingQrCode(qr) => Some(qr.code.clone()),
                _ => None,
            })
            .collect();
        assert!(
            codes[1].contains(&expected),
            "the re-emitted code must carry the new secret"
        );
        assert_eq!(
            codes[0].split(',').next(),
            codes[1].split(',').next(),
            "it is the same ref, re-rendered — not the next one"
        );
    }

    /// Regression: the pair-code lifetime the exhaustion guard has to respect is
    /// the link's, not the code's. A `primary_hello` accepted near the end of
    /// the validity window leaves `companion_finish` pending for up to a minute
    /// more, and tearing the socket down there kills a confirmation still on its
    /// way.
    #[tokio::test(start_paused = true)]
    async fn qr_exhaustion_keeps_the_socket_up_for_a_pending_pair_success() {
        let (client, _transport) = create_iq_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        let expired = wacore::time::now_secs()
            - (wacore::pair_code::PairCodeUtils::code_validity().as_secs() as i64 + 1);
        *client.pair_code_state.lock().await = PairCodeState::WaitingForPhoneConfirmation {
            pairing_ref: b"3@2:ref".to_vec(),
            phone_jid: "15551234567".to_string(),
            pair_code: "ABCD1234".to_string(),
            ephemeral_keypair: Box::new(KeyPair::generate(
                &mut rand::make_rng::<rand::rngs::StdRng>(),
            )),
            code_generation_ts: expired,
            // Stage 2 ran: pair-success is still due.
            primary_hello_attempt_count: 1,
        };

        let iq = pair_device_iq();
        assert!(handle_iq(&client, &iq.as_node_ref()).await);
        exhaust_qr_rotation(&collector).await;
        poll_until("the QR rotation task to run out of refs", || {
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingQrCodesExhausted(_)))
        })
        .await;

        assert!(
            client.is_running.load(Ordering::Relaxed),
            "a pending pair-success still needs this socket"
        );
    }

    /// Regression: a pair-code flow outlives the QR refs. WA Web's rotation
    /// timer (`Handle/PairDevice.js`) cancels itself and reports
    /// `UNPAIRED_IDLE` when the refs run out — it never closes the socket — so
    /// tearing the connection down here would kill a phone-number link that the
    /// server still considers open (and whose code we advertised for longer
    /// than the rotation budget).
    #[tokio::test(start_paused = true)]
    async fn qr_exhaustion_keeps_the_socket_up_while_a_pair_code_is_outstanding() {
        let (client, _transport) = create_iq_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();
        set_pair_code_waiting(&client).await;

        let iq = pair_device_iq();
        assert!(handle_iq(&client, &iq.as_node_ref()).await);

        exhaust_qr_rotation(&collector).await;
        poll_until("the QR rotation task to run out of refs", || {
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingQrCodesExhausted(x) if !x.disconnected))
        })
        .await;

        assert!(
            client.is_running.load(Ordering::Relaxed),
            "exhausted QR refs must not tear down a connection a pair-code flow is still using"
        );
    }

    /// The companion still needs to hear that the QR codes are gone: WA Web
    /// switches the screen to `UNPAIRED_IDLE` ("click to reload"), which is the
    /// consumer's cue to reconnect. Without a pair-code flow in progress the
    /// legacy self-disconnect stays, so QR-only consumers keep the reconnect
    /// they already rely on.
    #[tokio::test(start_paused = true)]
    async fn qr_exhaustion_reports_itself_and_still_disconnects_a_qr_only_flow() {
        let (client, _transport) = create_iq_test_client().await;
        let collector = Arc::new(TestEventCollector::default());
        client.subscribe_handler(collector.clone()).detach();

        let iq = pair_device_iq();
        assert!(handle_iq(&client, &iq.as_node_ref()).await);

        exhaust_qr_rotation(&collector).await;
        poll_until("the QR rotation task to give up", || {
            collector
                .events()
                .iter()
                .any(|e| matches!(&**e, Event::PairingQrCodesExhausted(x) if x.disconnected))
        })
        .await;

        poll_until("the QR-only client to disconnect", || {
            !client.is_running.load(Ordering::Relaxed)
        })
        .await;
    }
}
