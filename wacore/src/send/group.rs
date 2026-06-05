//! Group stanza preparation, phash/stale-device helpers and sender-key distribution.

use super::*;

/// Pairwise-encrypted retry stanza for a single group participant.
/// WA Web sends retries to the failing device only (RetryMsgJob.js:71),
/// NOT as a sender-key broadcast to all participants.
#[allow(clippy::too_many_arguments)]
pub async fn prepare_group_retry_stanza<S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    group_jid: Jid,
    participant_jid: Jid,
    encryption_jid: Jid,
    message: &wa::Message,
    message_id: String,
    retry_count: u8,
    account: Option<&wa::AdvSignedDeviceIdentity>,
    addressing_mode: crate::types::message::AddressingMode,
    edit: Option<crate::types::message::EditAttribute>,
) -> Result<Node>
where
    S: crate::libsignal::protocol::SessionStore,
    I: crate::libsignal::protocol::IdentityKeyStore,
{
    let plaintext = MessageUtils::encode_and_pad(message);
    let signal_address = encryption_jid.to_protocol_address();

    if account.is_none() && pkmsg_would_be_emitted(session_store, &signal_address).await? {
        bail!(
            "group retry pkmsg requires <device-identity> (account is None); \
             refusing before message_encrypt to avoid advancing the sender chain"
        );
    }

    let encrypted =
        message_encrypt(&plaintext, &signal_address, session_store, identity_store).await?;

    let (enc_type, is_prekey, serialized) = extract_ciphertext(encrypted)
        .ok_or_else(|| anyhow!("Unexpected encryption message type for group retry"))?;

    // count="N" distinguishes retries from normal sends (MsgCreateDeviceStanza.js:150-153)
    let mut enc_builder = NodeBuilder::new("enc")
        .attr("v", stanza::ENC_VERSION)
        .attr("type", enc_type)
        .attr("count", retry_count);
    if let Some(mt) = media_type_from_message(message) {
        enc_builder = enc_builder.attr("mediatype", mt);
    }
    let enc_node = enc_builder.bytes(serialized).build();

    let mut children = vec![enc_node];

    if is_prekey {
        // Defense in depth: pre-flight should have caught this, but a corrupt
        // session that triggers a fresh pkmsg mid-call would slip past.
        let acc = account.ok_or_else(|| {
            anyhow!("group retry pkmsg without <device-identity> (unreachable via pre-flight)")
        })?;
        children.push(
            NodeBuilder::new("device-identity")
                .bytes(acc.encode_to_vec())
                .build(),
        );
    }

    let stanza_type = stanza_type_from_message(message);
    let mut stanza_builder = NodeBuilder::new("message")
        .attr("to", group_jid)
        .attr("participant", participant_jid)
        .attr("id", message_id)
        .attr("type", stanza_type);

    // WA Web always sets addressing_mode for groups (MsgCreateDeviceStanza.js:131-135)
    stanza_builder = stanza_builder.attr("addressing_mode", addressing_mode.as_str());

    // Without `edit`, the resend looks like a normal message and the client never
    // applies the revoke/edit.
    if let Some(e) = edit
        && e != crate::types::message::EditAttribute::Empty
    {
        stanza_builder = stanza_builder.attr("edit", e.to_string_val());
    }

    Ok(stanza_builder.children(children).build())
}

/// Result of `prepare_group_stanza` — carries the stanza node and the exact
/// device list used for SKDM distribution, so callers can persist sender key
/// tracking without re-resolving devices.
pub struct PreparedGroupStanza {
    pub node: Node,
    /// Full SKDM distribution target set, marked `has_key=true` after the
    /// server ACK. Mirrors WA Web `markHasSenderKey(x, M)` which marks the
    /// whole target set `M`, not only the devices that encrypted successfully:
    /// devices that failed (406 / no bundle) are marked too so they are not
    /// re-targeted on every send (the retry-receipt path repairs any that are
    /// actually alive and keyless via `mark_forget_sender_key`).
    pub skdm_devices: Vec<Jid>,
    /// Users whose device registry should be invalidated because their
    /// devices returned 406 (unregistered) during SKDM prekey fetch.
    /// Empty when no 406 occurred.
    pub stale_device_users: Vec<String>,
    /// Generated `MessageContextInfo.message_secret`; populated when the
    /// reporting token was produced for this send.
    pub message_secret: Option<[u8; crate::reporting_token::MESSAGE_SECRET_SIZE]>,
    /// The identity we addressed this group send under (LID for LID-mode
    /// groups, PN for PN-mode). Used to key the persisted `messageSecret`
    /// so msmsg bot replies referencing this msg_id hit the same row that
    /// `<meta target_sender_jid>` echoes back at lookup time.
    pub sender_identity: Jid,
}

#[allow(clippy::too_many_arguments)]
pub async fn prepare_group_stanza<
    'a,
    S: crate::libsignal::protocol::SessionStore + Clone + Send + Sync + 'static,
    I: crate::libsignal::protocol::IdentityKeyStore + Clone + Send + Sync + 'static,
    P: crate::libsignal::protocol::PreKeyStore + Send + Sync,
    SP: crate::libsignal::protocol::SignedPreKeyStore + Send + Sync,
>(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'a, S, I, P, SP>,
    resolver: &dyn SendContextResolver,
    // Caller guarantees `own_base_jid` is already present in `participants`, so
    // this reads the shared (Arc-backed) metadata without cloning it.
    group_info: &GroupInfo,
    own_jid: &Jid,
    own_lid: &Jid,
    account: Option<&wa::AdvSignedDeviceIdentity>,
    to_jid: Jid,
    message: &wa::Message,
    request_id: String,
    force_skdm_distribution: bool,
    skdm_target_devices: Option<Vec<Jid>>,
    // Full resolved device set for the phash (groups only). `Some` on warm/partial
    // sends so the phash covers every device + self even when no SKDM is sent;
    // `None` on the cold `force_skdm` path (the set is resolved here) and for
    // status broadcasts (which keep the prior phash behavior).
    all_devices_for_phash: Option<Vec<Jid>>,
    edit: Option<crate::types::message::EditAttribute>,
    extra_stanza_nodes: &[Node],
) -> Result<PreparedGroupStanza> {
    let (own_sending_jid, _) = match group_info.addressing_mode {
        crate::types::message::AddressingMode::Lid => (own_lid.clone(), "lid"),
        crate::types::message::AddressingMode::Pn => (own_jid.clone(), "pn"),
    };

    // Generate reporting token if the message type supports it
    // For groups, both sender_jid and remote_jid are the group JID (to_jid) per Baileys implementation
    let reporting_result = generate_reporting_token(message, &request_id, &to_jid, &to_jid, None);

    // Prepare message with MessageContextInfo containing the message secret
    let message_for_encryption = if let Some(ref result) = reporting_result {
        prepare_message_with_context(message, &result.message_secret)
    } else {
        message.clone()
    };

    let own_base_jid = own_sending_jid.to_non_ad();

    let mut message_children: Vec<Node> = Vec::new();
    let mut includes_prekey_message = false;
    let mut phash_for_stanza: Option<String> = None;
    let mut skdm_encrypted_devices: Vec<Jid> = Vec::new();

    // Build the chain name once and hold its lock across SKDM creation + the
    // skmsg encrypt, so concurrent same-(group, sender) sends can't split the
    // key between the SKDM and the skmsg (nor reuse a chain iteration).
    let sender_key_name = make_sender_key_name(&to_jid, &own_sending_jid.to_protocol_address());
    let chain_lock = stores
        .sender_key_store
        .sender_key_lock(&sender_key_name)
        .await;
    let _chain_guard = chain_lock.lock().await;

    // Determine if we need to distribute SKDM and to which devices
    let distribution_list: Option<Vec<Jid>> = if let Some(target_devices) = skdm_target_devices {
        // Use the specific list of devices that need SKDM
        if target_devices.is_empty() {
            None
        } else {
            log::debug!(
                "SKDM distribution to {} specific devices for group {}",
                target_devices.len(),
                to_jid
            );
            Some(target_devices)
        }
    } else if force_skdm_distribution {
        // Resolve all devices for all participants (legacy behavior)
        // For LID groups, use phone numbers for device queries (LID usync may not work for own JID)
        // For PN groups, use JIDs directly
        let mut jids_to_resolve: Vec<Jid> = group_info
            .participants
            .iter()
            .map(|jid| {
                let base_jid = jid.to_non_ad();
                // If this is a LID JID and we have a phone number mapping, use it for device query
                if base_jid.is_lid()
                    && let Some(phone_jid) = group_info.phone_jid_for_lid_user(&base_jid.user)
                {
                    log::debug!(
                        "Using phone number {} for LID {} device query",
                        phone_jid,
                        base_jid
                    );
                    return phone_jid.to_non_ad();
                }
                base_jid
            })
            .collect();

        // Determine what user to check for — use the PN user when own is LID
        // and we have a mapping. Keeping this as a borrow avoids allocating a
        // throwaway Jid when own is already in the list.
        let own_pn_mapping = if own_base_jid.is_lid() {
            group_info.phone_jid_for_lid_user(&own_base_jid.user)
        } else {
            None
        };
        let own_check_user = own_pn_mapping
            .map(|pn| pn.user.as_str())
            .unwrap_or(own_base_jid.user.as_str());

        if !jids_to_resolve.iter().any(|p| p.user == own_check_user) {
            jids_to_resolve.push(match own_pn_mapping {
                Some(pn) => pn.to_non_ad(),
                None => own_base_jid.clone(),
            });
        }

        crate::types::jid::sort_dedup_by_user(&mut jids_to_resolve);

        log::debug!(
            "Resolving devices for {} participants",
            jids_to_resolve.len()
        );

        let mut resolved_list = resolver.resolve_devices(&jids_to_resolve).await?;

        // For LID groups, convert phone-based device JIDs back to LID format
        // This is necessary because WhatsApp Web expects LID addressing in SKDM <to> nodes
        if group_info.addressing_mode == crate::types::message::AddressingMode::Lid {
            resolved_list = resolved_list
                .into_iter()
                .map(|device_jid| group_info.phone_device_jid_into_lid(device_jid))
                .collect();
            log::debug!(
                "Converted {} devices to LID addressing for group {}",
                resolved_list.len(),
                to_jid
            );
        }

        // Dedup AFTER LID conversion to avoid duplicates when both phone and LID
        // queries return the same user (e.g., 559980000003:33 and 100000037037034:33
        // both convert to 100000037037034:33@lid).
        // Key on (user, server, agent, device) — excludes `integrator` which is not
        // part of the wire JID identity used in <to jid> and phash.
        crate::types::jid::sort_dedup_by_device(&mut resolved_list);

        // Filter devices for SKDM distribution:
        // - Exclude the exact sending device (own_sending_jid) - we already have our own sender key
        // - Keep ALL other devices including our own other devices (phone, other companions)
        //   because they need the SKDM to decrypt messages we send from this device
        // - Exclude hosted/Cloud API devices (device ID 99 or @hosted server) - they don't
        //   participate in group E2EE, only in 1:1 chats
        let own_user = &own_sending_jid.user;
        let own_device = own_sending_jid.device;
        let before_filter = resolved_list.len();
        resolved_list.retain(|device_jid| {
            let is_exact_sender = device_jid.user == *own_user && device_jid.device == own_device;
            let is_hosted = device_jid.is_hosted();
            // Exclude the exact sending device and hosted devices
            !is_exact_sender && !is_hosted
        });
        log::debug!(
            "Filtered SKDM devices from {} to {} (excluded sender {}:{} and hosted devices)",
            before_filter,
            resolved_list.len(),
            own_user,
            own_device
        );

        log::debug!(
            "SKDM distribution list for {} resolved to {} devices",
            to_jid,
            resolved_list.len(),
        );

        Some(resolved_list)
    } else {
        None
    };

    // Phash (groups): cover the FULL participant device set + the sending device
    // on EVERY send, matching WA Web `phashV2([].concat(A, [B]))`. Verified
    // against a real WA Web capture: the recipient set plus the sending device
    // reproduced the on-wire phash exactly, the recipient set alone did not. The
    // server validates it silently (it is not echoed on a normal ack). Status
    // broadcasts keep the prior behavior (phash over the distribution list only,
    // when distributing); WA Web's status path does not augment with self.
    if to_jid.is_group() {
        // Warm/partial sends pass the complete set in `all_devices_for_phash`;
        // the cold `force_skdm` path leaves it None and `distribution_list`
        // already holds the full resolved set.
        if let Some(src) = all_devices_for_phash
            .as_deref()
            .or(distribution_list.as_deref())
        {
            let phash_set = build_group_phash_set(src, &own_sending_jid);
            match MessageUtils::participant_list_hash(&phash_set) {
                Ok(phash) => phash_for_stanza = Some(phash),
                Err(e) => log::warn!("Failed to compute group phash for {}: {:?}", to_jid, e),
            }
        }
    } else if let Some(ref distribution_list) = distribution_list {
        match MessageUtils::participant_list_hash(distribution_list) {
            Ok(phash) => phash_for_stanza = Some(phash),
            Err(e) => log::warn!("Failed to compute phash for {}: {:?}", to_jid, e),
        }
    }

    let mut had_unregistered_devices = false;

    if let Some(ref distribution_list) = distribution_list {
        let axolotl_skdm_bytes = create_sender_key_distribution_message_for_group(
            stores.sender_key_store,
            &sender_key_name,
        )
        .await?;

        let skdm_wrapper_msg = wa::Message {
            sender_key_distribution_message: Some(wa::message::SenderKeyDistributionMessage {
                group_id: Some(to_jid.to_string()),
                axolotl_sender_key_distribution_message: Some(axolotl_skdm_bytes),
            }),
            ..Default::default()
        };
        let skdm_plaintext_to_encrypt = MessageUtils::encode_and_pad(&skdm_wrapper_msg);

        // WA Web's GroupSkmsgJob wraps ensureE2ESessions in try/catch — logs error
        // but does NOT rethrow. SKDM distribution failure must not prevent the group
        // message from being sent. Only successfully encrypted devices are tracked.
        // Must match the rule applied to the main skmsg payload below: if SKDM carries
        // `decrypt-fail="hide"` but the payload does not (e.g. AdminRevoke), recipients
        // without a sender key never decrypt the skmsg and the revoke is silently dropped.
        let skdm_hide_decrypt_fail = should_hide_decrypt_fail_for_send(edit.as_ref(), message);
        match encrypt_for_devices(
            runtime,
            stores,
            resolver,
            distribution_list,
            &skdm_plaintext_to_encrypt,
            skdm_hide_decrypt_fail,
            None,
        )
        .await
        {
            Ok(result) => {
                includes_prekey_message = includes_prekey_message || result.includes_prekey_message;
                if result.had_unregistered_device {
                    had_unregistered_devices = true;
                }
                skdm_encrypted_devices = result.encrypted_devices;

                if !result.participant_nodes.is_empty() {
                    message_children.push(
                        NodeBuilder::new("participants")
                            .children(result.participant_nodes)
                            .build(),
                    );
                    if includes_prekey_message && let Some(acc) = account {
                        message_children.push(
                            NodeBuilder::new("device-identity")
                                .bytes(acc.encode_to_vec())
                                .build(),
                        );
                    }
                }
            }
            Err(e) => {
                log::warn!(
                    "SKDM distribution failed for group {}, continuing without it: {e}",
                    to_jid
                );
                if is_device_unregistered_error(&e) {
                    had_unregistered_devices = true;
                }
            }
        }
    }

    let plaintext = MessageUtils::encode_and_pad(&message_for_encryption);
    let skmsg = encrypt_group_message(
        stores.sender_key_store,
        &sender_key_name,
        &plaintext,
        &mut rand::make_rng::<rand::rngs::StdRng>(),
    )
    .await?;

    let skmsg_ciphertext = skmsg.into_serialized();

    let mediatype = media_type_from_message(message);
    let hide_decrypt_fail = should_hide_decrypt_fail_for_send(edit.as_ref(), message);

    let mut enc_builder = NodeBuilder::new("enc")
        .attr("v", stanza::ENC_VERSION)
        .attr("type", stanza::ENC_TYPE_SKMSG);
    if let Some(mt) = mediatype {
        enc_builder = enc_builder.attr("mediatype", mt);
    }
    enc_builder = enc_builder.bytes(skmsg_ciphertext);
    if hide_decrypt_fail {
        enc_builder = enc_builder.attr("decrypt-fail", "hide");
    }
    let content_node = enc_builder.build();

    let stanza_type = stanza_type_from_message(message);
    let mut stanza_builder = NodeBuilder::new("message")
        .attr("to", to_jid)
        .attr("id", request_id)
        .attr("type", stanza_type);

    // WA Web always sets addressing_mode for groups (MsgCreateDeviceStanza.js:131-135)
    stanza_builder = stanza_builder.attr("addressing_mode", group_info.addressing_mode.as_str());

    if let Some(edit_attr) = &edit
        && *edit_attr != crate::types::message::EditAttribute::Empty
    {
        stanza_builder = stanza_builder.attr("edit", edit_attr.to_string_val());
    }
    // NOTE: WhatsApp Web does NOT include participant attribute on initial admin revoke send
    // The participant attribute only appears on retry/fanout messages

    message_children.push(content_node);

    // Add reporting token node if we generated one
    if let Some(ref result) = reporting_result {
        message_children.push(build_reporting_node(result));
    }

    // Add phash if we distributed keys in this message
    if let Some(phash) = phash_for_stanza {
        stanza_builder = stanza_builder.attr("phash", phash);
    }

    // Add any extra stanza nodes provided by the caller
    message_children.extend(extra_stanza_nodes.iter().cloned());

    let stanza = stanza_builder.children(message_children).build();

    let stale_users = if had_unregistered_devices {
        collect_stale_device_users(
            distribution_list.as_deref(),
            &skdm_encrypted_devices,
            group_info,
        )
    } else {
        Vec::new()
    };

    Ok(PreparedGroupStanza {
        node: stanza,
        // Mark the full target set (matches WA Web `markHasSenderKey(x, M)`), not
        // just `skdm_encrypted_devices`. `stale_users` above already used the
        // encrypted subset to find which devices to re-resolve.
        skdm_devices: distribution_list.unwrap_or_default(),
        stale_device_users: stale_users,
        message_secret: reporting_result.map(|r| r.message_secret),
        sender_identity: own_sending_jid,
    })
}

/// Build the device set hashed into a group `phash`, matching WA Web
/// `phashV2([].concat(A, [B]))`: every participant device (`A`) plus the
/// sending device `B`. `devices` is the resolved set (recipients); the sending
/// device is excluded from it (we never SKDM ourselves) so it is appended here.
/// Hosted devices don't take part in group E2EE and are dropped, mirroring the
/// SKDM distribution filter. `participant_list_hash` sorts before hashing, so
/// order here is irrelevant.
pub(crate) fn build_group_phash_set(devices: &[Jid], own_sending_jid: &Jid) -> Vec<Jid> {
    let mut set: Vec<Jid> = devices.iter().filter(|d| !d.is_hosted()).cloned().collect();
    if !set
        .iter()
        .any(|d| d.user == own_sending_jid.user && d.device == own_sending_jid.device)
    {
        set.push(own_sending_jid.clone());
    }
    crate::types::jid::sort_dedup_by_device(&mut set);
    set
}

/// Collect users whose devices failed SKDM so the caller can invalidate their
/// registry entries. In LID-mode groups, both the LID and PN aliases are
/// emitted when the group knows the mapping — `invalidate_device_cache` needs
/// both to clean up zombie records that were stored under whichever alias
/// `update_device_list` canonicalised to at the time of the write.
pub(crate) fn collect_stale_device_users(
    distribution_list: Option<&[Jid]>,
    skdm_encrypted_devices: &[Jid],
    group_info: &GroupInfo,
) -> Vec<String> {
    let Some(dist) = distribution_list else {
        return Vec::new();
    };
    let is_lid_mode = group_info.addressing_mode == crate::types::message::AddressingMode::Lid;
    let encrypted_set: HashSet<&Jid> = skdm_encrypted_devices.iter().collect();
    let mut user_set: HashSet<String> = HashSet::new();
    for d in dist {
        if encrypted_set.contains(d) {
            continue;
        }
        user_set.insert(d.user.to_string());
        if is_lid_mode
            && d.is_lid()
            && let Some(pn_jid) = group_info.phone_jid_for_lid_user(&d.user)
            && pn_jid.is_pn()
        {
            user_set.insert(pn_jid.user.to_string());
        }
    }
    user_set.into_iter().collect()
}

/// Caller must hold `SenderKeyStore::sender_key_lock` for `sender_key_name`
/// across this creation + the matching skmsg encrypt (see `encrypt_group_message`).
pub async fn create_sender_key_distribution_message_for_group(
    store: &mut (dyn SenderKeyStore + Send + Sync),
    sender_key_name: &SenderKeyName,
) -> Result<Vec<u8>> {
    let mut rng = rand::make_rng::<rand::rngs::StdRng>();

    let skdm = crate::libsignal::protocol::create_sender_key_distribution_message(
        sender_key_name,
        store,
        &mut rng,
    )
    .await?;

    Ok(skdm.into_serialized().into_vec())
}

/// Build a `Message.ProtocolMessage` for `GROUP_MEMBER_LABEL_CHANGE`.
///
/// Sent via the standard E2EE fanout, not an IQ. Empty `label` clears.
/// `ts_secs` is unix seconds, matching WA Web's `unixTime()`.
pub fn build_member_label_message(label: String, ts_secs: i64) -> wa::Message {
    wa::Message {
        protocol_message: Some(Box::new(wa::message::ProtocolMessage {
            r#type: Some(wa::message::protocol_message::Type::GroupMemberLabelChange as i32),
            member_label: Some(wa::MemberLabel {
                label: Some(label),
                label_timestamp: Some(ts_secs),
            }),
            ..Default::default()
        })),
        ..Default::default()
    }
}
