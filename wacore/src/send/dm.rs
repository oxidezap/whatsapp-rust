//! 1:1 (DM) stanza preparation and DM retry stanzas.

use super::*;

fn is_exact_dm_sender_device(device_jid: &Jid, own_jid: &Jid, own_lid: Option<&Jid>) -> bool {
    (device_jid.is_same_user_as(own_jid) && device_jid.device == own_jid.device)
        || own_lid
            .is_some_and(|lid| device_jid.is_same_user_as(lid) && device_jid.device == lid.device)
}

pub(crate) fn partition_dm_devices(
    all_devices: Vec<Jid>,
    own_jid: &Jid,
    own_lid: Option<&Jid>,
) -> (Vec<Jid>, Vec<Jid>) {
    let mut recipient_devices = Vec::with_capacity(all_devices.len());
    let mut own_other_devices = Vec::with_capacity(4);

    for device_jid in all_devices {
        if is_exact_dm_sender_device(&device_jid, own_jid, own_lid) {
            continue;
        }

        if device_jid.matches_user_or_lid(own_jid, own_lid) {
            own_other_devices.push(device_jid);
        } else {
            recipient_devices.push(device_jid);
        }
    }

    (recipient_devices, own_other_devices)
}

/// Result of `prepare_dm_stanza` — carries the stanza node and the
/// locally computed phash for server ACK validation.
pub struct PreparedDmStanza {
    pub node: Node,
    /// Locally computed phash from the sent device set. Not sent on the
    /// wire (WA Web only sends phash for groups). Used by the caller to
    /// compare against the server's ACK phash for device-list drift detection.
    pub phash: Option<String>,
    /// `MessageContextInfo.message_secret` generated for this stanza so the
    /// caller can persist it for later addon (msmsg/poll/edit) decryption.
    /// `None` when the message had no reporting token (no secret was used).
    pub message_secret: Option<[u8; crate::reporting_token::MESSAGE_SECRET_SIZE]>,
}

#[allow(clippy::too_many_arguments)]
pub async fn prepare_dm_stanza<
    'a,
    S: crate::libsignal::protocol::SessionStore + Clone + Send + Sync + 'static,
    I: crate::libsignal::protocol::IdentityKeyStore + Clone + Send + Sync + 'static,
    P: crate::libsignal::protocol::PreKeyStore + Send + Sync,
    SP: crate::libsignal::protocol::SignedPreKeyStore + Send + Sync,
>(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'a, S, I, P, SP>,
    resolver: &dyn SendContextResolver,
    own_jid: &Jid,
    own_lid: Option<&Jid>,
    account: Option<&wa::AdvSignedDeviceIdentity>,
    to_jid: Jid,
    message: &wa::Message,
    request_id: String,
    edit: Option<crate::types::message::EditAttribute>,
    extra_stanza_nodes: &[Node],
    all_devices: Vec<Jid>,
) -> Result<PreparedDmStanza> {
    // sender is the author's own jid, remote is the chat jid (WAWebReportingTokenUtils:
    // getSender vs e.to). Both previously used to_jid, conflating sender with remote.
    let reporting_result = generate_reporting_token(message, &request_id, own_jid, &to_jid, None);

    let message_for_encryption = if let Some(ref result) = reporting_result {
        prepare_message_with_context(message, &result.message_secret)
    } else {
        message.clone()
    };

    let recipient_plaintext = MessageUtils::encode_and_pad(&message_for_encryption);

    // Partition first so phash reflects the actual sent set (sender excluded)
    let total_devices = all_devices.len();
    let (recipient_devices, own_other_devices) =
        partition_dm_devices(all_devices, own_jid, own_lid);

    let phash = MessageUtils::participant_list_hash(
        recipient_devices.iter().chain(own_other_devices.iter()),
    )
    .ok();

    let dsm = crate::messages::wrap_device_sent(message_for_encryption, to_jid.to_string());

    let own_devices_plaintext = MessageUtils::encode_and_pad(&dsm);

    let mut participant_nodes = Vec::with_capacity(total_devices);
    let mut includes_prekey_message = false;

    let hide_decrypt_fail = should_hide_decrypt_fail_for_send(edit.as_ref(), message);

    let mediatype = media_type_from_message(message);

    // NOTE: WA Web has a bare-<enc> fast path for single primary device
    // (WAWebSendMsgCreateFanoutStanza). Not implemented here because
    // encrypt_for_devices always wraps in <to jid=...> nodes;
    // a bare-enc mode would require refactoring the encryption layer.
    // The <participants> form is accepted by the server regardless.

    if !recipient_devices.is_empty() {
        let result = encrypt_for_devices(
            runtime,
            stores,
            resolver,
            &recipient_devices,
            &recipient_plaintext,
            hide_decrypt_fail,
            mediatype,
        )
        .await?;
        participant_nodes.extend(result.participant_nodes);
        includes_prekey_message = includes_prekey_message || result.includes_prekey_message;
    }

    if !own_other_devices.is_empty() {
        let result = encrypt_for_devices(
            runtime,
            stores,
            resolver,
            &own_other_devices,
            &own_devices_plaintext,
            hide_decrypt_fail,
            mediatype,
        )
        .await?;
        participant_nodes.extend(result.participant_nodes);
        includes_prekey_message = includes_prekey_message || result.includes_prekey_message;
    }

    // All per-device encrypts failed: an empty <participants> would silently
    // drop the message. WA Web's encryptAndSendUserMsg rejects here too.
    let attempted_devices = recipient_devices.len() + own_other_devices.len();
    if participant_nodes.is_empty() && attempted_devices > 0 {
        return Err(anyhow!(
            "encryption failed for all {attempted_devices} recipient device(s)"
        ));
    }

    let mut message_content_nodes = vec![
        NodeBuilder::new("participants")
            .children(participant_nodes)
            .build(),
    ];

    if includes_prekey_message && let Some(acc) = account {
        let device_identity_bytes = acc.encode_to_vec();
        message_content_nodes.push(
            NodeBuilder::new("device-identity")
                .bytes(device_identity_bytes)
                .build(),
        );
    }

    // Add reporting token node if we generated one
    if let Some(ref result) = reporting_result {
        message_content_nodes.push(build_reporting_node(result));
    }

    // Add any extra stanza nodes provided by the caller
    message_content_nodes.extend(extra_stanza_nodes.iter().cloned());

    let stanza_type = stanza_type_from_message(message);

    let mut stanza_builder = NodeBuilder::new("message")
        .attr("to", to_jid)
        .attr("id", request_id)
        .attr("type", stanza_type);

    if let Some(edit_attr) = edit
        && edit_attr != crate::types::message::EditAttribute::Empty
    {
        stanza_builder = stanza_builder.attr("edit", edit_attr.to_string_val());
    }

    let stanza = stanza_builder.children(message_content_nodes).build();

    Ok(PreparedDmStanza {
        node: stanza,
        phash,
        message_secret: reporting_result.map(|r| r.message_secret),
    })
}

/// Returns true if `message_encrypt` on `signal_address` would produce
/// a pkmsg (no session yet, or session with un-acked pre-key still
/// pending). Used before `message_encrypt` to fail-fast when `account`
/// is None — pkmsg without `<device-identity>` reproduces the linked
/// device deadlock.
///
/// `SessionStore::load_session` is take-semantics in production
/// (`SessionAdapter` → `SignalStoreCache::get_session` marks the slot
/// `CheckedOut`); the loaded record is put back via `store_session`
/// so the subsequent `message_encrypt` finds the slot Present.
pub(crate) async fn pkmsg_would_be_emitted<S>(
    session_store: &mut S,
    signal_address: &ProtocolAddress,
) -> Result<bool>
where
    S: crate::libsignal::protocol::SessionStore,
{
    let loaded = session_store.load_session(signal_address).await?;
    // Conservative read: treat any failure to interrogate the session as
    // "would be pkmsg" so the caller bails. Silently treating Err as false
    // would let message_encrypt run with a corrupt session and potentially
    // burn the sender chain.
    let needs_pkmsg = match &loaded {
        None => true,
        Some(record) => match record.session_state() {
            None => true,
            Some(state) => match state.unacknowledged_pre_key_message_items() {
                Ok(Some(_)) => true,
                Ok(None) => false,
                Err(_) => true,
            },
        },
    };
    if let Some(record) = loaded {
        session_store
            .store_session(signal_address, record)
            .await
            .map_err(|e| anyhow!("restoring checked-out session after pre-flight: {e}"))?;
    }
    Ok(needs_pkmsg)
}

/// Mirrors `WAWebSendMsgCreateDeviceStanza.createUserDeviceMsgStanza`.
/// `<enc>` goes directly under `<message>`; the fanout wrapper
/// (`<participants><to>`) is server-rejected with 479 on retries.
/// `recipient_jid` is propagated verbatim from the retry receipt
/// (`f && (k.recipient = f)` in `WAWebHandleRetryRequest`); pass `None`
/// when the incoming receipt didn't carry it.
#[allow(clippy::too_many_arguments)]
pub async fn prepare_dm_retry_stanza<S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    to_jid: Jid,
    recipient_jid: Option<Jid>,
    encryption_jid: Jid,
    message: &wa::Message,
    message_id: String,
    retry_count: u8,
    account: Option<&wa::AdvSignedDeviceIdentity>,
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
            "DM retry pkmsg requires <device-identity> (account is None); \
             refusing before message_encrypt to avoid advancing the sender chain"
        );
    }

    let encrypted =
        message_encrypt(&plaintext, &signal_address, session_store, identity_store).await?;

    let (enc_type, is_prekey, serialized) = extract_ciphertext(encrypted)
        .ok_or_else(|| anyhow!("Unexpected encryption message type for DM retry"))?;

    let hide_decrypt_fail = should_hide_decrypt_fail_for_send(edit.as_ref(), message);
    let mut enc_builder = NodeBuilder::new("enc")
        .attr("v", stanza::ENC_VERSION)
        .attr("type", enc_type)
        .attr("count", retry_count);
    if let Some(mt) = media_type_from_message(message) {
        enc_builder = enc_builder.attr("mediatype", mt);
    }
    if hide_decrypt_fail {
        enc_builder = enc_builder.attr("decrypt-fail", "hide");
    }
    let enc_node = enc_builder.bytes(serialized).build();

    let mut children = vec![enc_node];
    if is_prekey {
        // Defense in depth: pre-flight should have caught this, but a corrupt
        // session that triggers a fresh pkmsg mid-call would slip past.
        let acc = account.ok_or_else(|| {
            anyhow!("DM retry pkmsg without <device-identity> (unreachable via pre-flight)")
        })?;
        children.push(
            NodeBuilder::new("device-identity")
                .bytes(acc.encode_to_vec())
                .build(),
        );
    }

    let mut stanza_builder = NodeBuilder::new("message")
        .attr("to", to_jid)
        .attr("id", message_id)
        .attr("type", stanza_type_from_message(message));
    if let Some(r) = recipient_jid {
        stanza_builder = stanza_builder.attr("recipient", r);
    }

    // Without `edit`, the resend looks like a normal message and the client never
    // applies the revoke/edit.
    if let Some(e) = edit
        && e != crate::types::message::EditAttribute::Empty
    {
        stanza_builder = stanza_builder.attr("edit", e.to_string_val());
    }

    Ok(stanza_builder.children(children).build())
}
