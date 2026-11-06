//! Peer (own-device) stanza preparation.

use super::*;

#[allow(clippy::too_many_arguments)]
pub async fn prepare_peer_stanza<S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    transport_jid: Jid,
    signal_address: &ProtocolAddress,
    message: &wa::Message,
    request_id: &str,
    account: Option<&wa::ADVSignedDeviceIdentity>,
) -> Result<Node>
where
    S: crate::libsignal::protocol::SessionStore,
    I: crate::libsignal::protocol::IdentityKeyStore,
{
    let options = peer_message_options_from_message(message);
    prepare_peer_stanza_with_options(
        session_store,
        identity_store,
        transport_jid,
        signal_address,
        message,
        request_id,
        account,
        options,
    )
    .await
}

/// Role-aware peer path. The caller must validate its persisted primary binding.
#[allow(clippy::too_many_arguments)]
pub async fn prepare_peer_stanza_for_role<S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    transport_jid: Jid,
    signal_address: &ProtocolAddress,
    message: &wa::Message,
    request_id: &str,
    account: Option<&wa::ADVSignedDeviceIdentity>,
    role: SenderIdentityRole,
) -> Result<Node>
where
    S: crate::libsignal::protocol::SessionStore,
    I: crate::libsignal::protocol::IdentityKeyStore,
{
    let options = peer_message_options_from_message(message);
    prepare_peer_stanza_with_options_for_role(
        session_store,
        identity_store,
        transport_jid,
        signal_address,
        message,
        request_id,
        account,
        options,
        role,
    )
    .await
}

#[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.peer_prepare", level = "debug", skip_all, fields(to = %transport_jid.observe()), err(Debug)))]
#[allow(clippy::too_many_arguments)]
pub async fn prepare_peer_stanza_with_options<S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    transport_jid: Jid,
    signal_address: &ProtocolAddress,
    message: &wa::Message,
    request_id: &str,
    account: Option<&wa::ADVSignedDeviceIdentity>,
    options: PeerMessageOptions,
) -> Result<Node>
where
    S: crate::libsignal::protocol::SessionStore,
    I: crate::libsignal::protocol::IdentityKeyStore,
{
    prepare_peer_stanza_with_options_for_role(
        session_store,
        identity_store,
        transport_jid,
        signal_address,
        message,
        request_id,
        account,
        options,
        SenderIdentityRole::Companion,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn prepare_peer_stanza_with_options_for_role<S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    transport_jid: Jid,
    signal_address: &ProtocolAddress,
    message: &wa::Message,
    request_id: &str,
    account: Option<&wa::ADVSignedDeviceIdentity>,
    options: PeerMessageOptions,
    role: SenderIdentityRole,
) -> Result<Node>
where
    S: crate::libsignal::protocol::SessionStore,
    I: crate::libsignal::protocol::IdentityKeyStore,
{
    role.validate()?;
    let plaintext = MessageUtils::encode_and_pad(message);

    if role.requires_account()
        && account.is_none()
        && pkmsg_would_be_emitted(session_store, signal_address).await?
    {
        bail!(
            "peer pkmsg requires <device-identity> (account is None); \
             refusing before message_encrypt to avoid advancing the sender chain"
        );
    }

    let encrypted_message =
        message_encrypt(&plaintext, signal_address, session_store, identity_store).await?;

    let (enc_type, is_prekey, serialized_bytes) = extract_ciphertext(encrypted_message)
        .ok_or_else(|| anyhow!("Unexpected peer encryption message type"))?;

    let enc_node = NodeBuilder::new("enc")
        .attrs([("v", "2"), ("type", enc_type)])
        .bytes(serialized_bytes)
        .build();

    let meta_node = NodeBuilder::new("meta").attr("appdata", "default").build();

    let mut children = vec![meta_node, enc_node];
    // Defense in depth: pre-flight should have caught a no-account pkmsg, but a
    // corrupt session that triggers a fresh pkmsg mid-call would slip past.
    if let Some(device_identity_bytes) = needs_device_identity_for_role(is_prekey, account, role)? {
        children.push(
            NodeBuilder::new("device-identity")
                .bytes(device_identity_bytes)
                .build(),
        );
    }

    let mut stanza_builder = NodeBuilder::new("message")
        .attr("to", transport_jid)
        .attr("id", request_id)
        .attr("type", stanza::MSG_TYPE_TEXT)
        .attr("category", "peer")
        .attr("push_priority", options.push_priority().as_str());
    if let Some(privacy_sensitive) = options.privacy_sensitive() {
        stanza_builder = stanza_builder.attr("privacy_sensitive", privacy_sensitive.as_str());
    }

    Ok(stanza_builder.children(children).build())
}
