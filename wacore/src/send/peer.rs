//! Peer (own-device) stanza preparation.

use super::*;

#[allow(clippy::too_many_arguments)]
pub async fn prepare_peer_stanza<S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    transport_jid: Jid,
    signal_address: &ProtocolAddress,
    message: &wa::Message,
    request_id: String,
    account: Option<&wa::AdvSignedDeviceIdentity>,
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

#[allow(clippy::too_many_arguments)]
pub async fn prepare_peer_stanza_with_options<S, I>(
    session_store: &mut S,
    identity_store: &mut I,
    transport_jid: Jid,
    signal_address: &ProtocolAddress,
    message: &wa::Message,
    request_id: String,
    account: Option<&wa::AdvSignedDeviceIdentity>,
    options: PeerMessageOptions,
) -> Result<Node>
where
    S: crate::libsignal::protocol::SessionStore,
    I: crate::libsignal::protocol::IdentityKeyStore,
{
    let plaintext = MessageUtils::encode_and_pad(message);

    if account.is_none() && pkmsg_would_be_emitted(session_store, signal_address).await? {
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
    if is_prekey {
        // Defense in depth: pre-flight should have caught this, but a corrupt
        // session that triggers a fresh pkmsg mid-call would slip past.
        let account = account.ok_or_else(|| {
            anyhow!("peer pkmsg without <device-identity> (unreachable via pre-flight)")
        })?;
        children.push(
            NodeBuilder::new("device-identity")
                .bytes(account.encode_to_vec())
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
