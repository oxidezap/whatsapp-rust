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

#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.dm_prepare", level = "debug", skip_all, fields(to = %to_jid.observe()), err(Debug)))]
#[allow(clippy::too_many_arguments)]
pub async fn prepare_dm_stanza(
    runtime: &dyn Runtime,
    stores: &mut SignalStores<'_>,
    resolver: &dyn SendContextResolver,
    own_jid: &Jid,
    own_lid: Option<&Jid>,
    account: Option<&wa::ADVSignedDeviceIdentity>,
    to_jid: Jid,
    message: &wa::Message,
    request_id: String,
    edit: Option<crate::types::message::EditAttribute>,
    extra_stanza_nodes: &[Node],
    all_devices: Vec<Jid>,
    // Avoids a second full encode when the caller already serialized the message;
    // ignored on the mci-hoist path (see `shared_content`).
    pre_encoded: Option<std::sync::Arc<Vec<u8>>>,
) -> Result<PreparedDmStanza> {
    // Encode the message at most once (reusing the caller's `pre_encoded` bytes when
    // provided) and thread those bytes through both the reporting token
    // (whitelisted-field extraction) and the wire plaintext below. The rare mci-hoist
    // path (message carries a top-level message_context_info) can't share: its
    // plaintext folds the reporting secret into the existing mci, diverging from the
    // bytes the token is computed over, so it re-encodes.
    let shared_content = message.message_context_info.is_unset().then(|| {
        pre_encoded.unwrap_or_else(|| std::sync::Arc::new(waproto::codec::message_to_vec(message)))
    });

    // sender is the author's own jid, remote is the chat jid (WAWebReportingTokenUtils:
    // getSender vs e.to). Both previously used to_jid, conflating sender with remote.
    // Reuse the message's own secret when the caller set one (e.g. polls), instead of
    // minting a fresh one that would overwrite it: WA Web derives the reporting token
    // from the message's existing messageSecret (`p ?? e.messageSecret`), and a poll's
    // creator must keep the secret that ends up on the wire to decrypt later votes.
    let existing_secret = crate::reporting_token::extract_message_secret(message);
    let reporting_result = match &shared_content {
        Some(content) => generate_reporting_token_from_encoded(
            message,
            content,
            &request_id,
            own_jid,
            &to_jid,
            existing_secret,
        ),
        None => generate_reporting_token(message, &request_id, own_jid, &to_jid, existing_secret),
    };

    // The reporting token's MessageContextInfo (message_secret + version) is spliced
    // straight onto the encoded plaintexts instead of deep-cloning the whole message
    // via prepare_message_with_context just to attach two fields.
    let extra_context = reporting_result.as_ref().map(reporting_context_info);

    // Partition first so phash reflects the actual sent set (sender excluded) and so
    // the own-device plaintext can be skipped when there's nothing to send it to.
    let total_devices = all_devices.len();
    let (recipient_devices, own_other_devices) =
        partition_dm_devices(all_devices, own_jid, own_lid);

    let phash = MessageUtils::participant_list_hash(
        recipient_devices.iter().chain(own_other_devices.iter()),
    )
    .ok();

    // Splice the shared content into the recipient plaintext and, when present, the
    // own-device DeviceSentMessage plaintext. With no own companion devices (e.g. an
    // account with nothing else linked), the DSM plaintext — and the destination-jid
    // stringify it needs — would be built only to go unused, so encode just the recipient.
    // The mci-hoist path re-encodes via `encode_dm_plaintexts` (see `shared_content`).
    let crate::messages::DmPlaintexts {
        recipient: recipient_plaintext,
        own_devices: own_devices_plaintext,
    } = match &shared_content {
        Some(content) if own_other_devices.is_empty() => crate::messages::DmPlaintexts {
            recipient: MessageUtils::pad_with_context_from_encoded(content, extra_context.as_ref()),
            own_devices: Vec::new(),
        },
        Some(content) => MessageUtils::dm_plaintexts_from_encoded(
            content,
            extra_context.as_ref(),
            &to_jid.to_string(),
        ),
        None => {
            MessageUtils::encode_dm_plaintexts(message, extra_context.as_ref(), &to_jid.to_string())
        }
    };

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

    // DM stays lenient when pkmsg lacks an account (no pre-flight here): map the
    // helper's error back to omission so the wire shape is unchanged.
    if let Some(device_identity_bytes) = needs_device_identity(includes_prekey_message, account)
        .ok()
        .flatten()
    {
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
pub async fn pkmsg_would_be_emitted<S>(
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
#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.dm_retry", level = "debug", skip_all, fields(to = %to_jid.observe()), err(Debug)))]
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
    account: Option<&wa::ADVSignedDeviceIdentity>,
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
    // Defense in depth: pre-flight should have caught a no-account pkmsg, but a
    // corrupt session that triggers a fresh pkmsg mid-call would slip past.
    if let Some(device_identity_bytes) = needs_device_identity(is_prekey, account)? {
        children.push(
            NodeBuilder::new("device-identity")
                .bytes(device_identity_bytes)
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
