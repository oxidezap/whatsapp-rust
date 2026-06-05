//! Inbound msg-secret capture and secret-encrypted message decryption.

use super::*;

impl Client {
    /// Capture embedded `MessageContextInfo.message_secret` for add-on
    /// decrypts. Bot DMs keep the legacy LID key as a second entry.
    pub(crate) async fn maybe_capture_inbound_msg_secret(
        self: &Arc<Self>,
        msg: &wa::Message,
        info: &Arc<MessageInfo>,
    ) {
        use wacore::proto_helpers::MessageExt;

        let mci = msg.message_context_info.as_ref();
        let Some(secret_bytes) = mci.and_then(|m| m.message_secret.as_deref()) else {
            return;
        };
        if msg.is_forwarded() {
            return;
        }

        let policy = self.cache_config.msg_secret_policy;
        if !policy.persists() {
            return;
        }
        let chat_is_bot = info.source.chat.is_bot();
        // BotOnly enforcement lives in build_msg_secret_entry (the chokepoint),
        // which keys off the classified bot context including group bot prompts.
        let class = wacore::msg_secret::classify(msg, chat_is_bot);
        let message_ts = u64::try_from(info.timestamp.timestamp()).ok();

        // Build both aliases (primary, plus the bot-DM LID key) and write them
        // in one batch so a partial write can't leave only one stored.
        let mut entries = Vec::with_capacity(2);
        if let Some(entry) = self.build_msg_secret_entry(
            &info.source.chat,
            &info.source.sender,
            &info.id,
            secret_bytes,
            class,
            message_ts,
        ) {
            entries.push(entry);
        }
        if chat_is_bot
            && let Some(sender) = self.dm_sender_identity_for(&info.source.chat).await
            && sender.to_non_ad() != info.source.sender.to_non_ad()
            && let Some(entry) = self.build_msg_secret_entry(
                &info.source.chat,
                &sender,
                &info.id,
                secret_bytes,
                class,
                message_ts,
            )
        {
            entries.push(entry);
        }
        self.persist_msg_secret_entries(entries).await;
    }

    /// Build one retention entry, applying the policy gates and computing the
    /// per-row deadline. Returns `None` when the policy skips this write (not
    /// persisting, or `BotOnly` and the class isn't `Bot`) or the secret isn't
    /// 32 bytes. Pure (no I/O) so callers can batch several aliases atomically.
    fn build_msg_secret_entry(
        &self,
        chat: &Jid,
        sender: &Jid,
        msg_id: &str,
        secret_bytes: &[u8],
        class: wacore::msg_secret::RetentionClass,
        message_ts: Option<u64>,
    ) -> Option<wacore::store::traits::MsgSecretEntry> {
        const SECRET_LEN: usize = wacore::reporting_token::MESSAGE_SECRET_SIZE;
        let secret = <&[u8; SECRET_LEN]>::try_from(secret_bytes).ok()?;
        let policy = self.cache_config.msg_secret_policy;
        if !policy.persists() {
            return None;
        }
        // Single chokepoint for the BotOnly invariant: only bot-context secrets
        // (class == Bot) are persisted, no matter which write path got here.
        if policy.bot_only() && class != wacore::msg_secret::RetentionClass::Bot {
            return None;
        }
        let expires_at = wacore::msg_secret::expires_at(
            policy,
            &self.cache_config.msg_secret_retention,
            class,
            message_ts,
            wacore::time::now_secs(),
        );
        Some(wacore::store::traits::MsgSecretEntry {
            chat: chat.to_non_ad_string(),
            sender: sender.to_non_ad_string(),
            msg_id: msg_id.to_string(),
            secret: secret.to_vec(),
            expires_at,
            message_ts: message_ts.and_then(|t| i64::try_from(t).ok()).unwrap_or(0),
        })
    }

    /// Write a batch of secret aliases in one atomic upsert, so a multi-alias
    /// capture/re-persist never leaves only some aliases stored.
    async fn persist_msg_secret_entries(
        &self,
        entries: Vec<wacore::store::traits::MsgSecretEntry>,
    ) -> bool {
        if entries.is_empty() {
            return false;
        }
        match self
            .persistence_manager
            .backend()
            .put_msg_secrets(entries)
            .await
        {
            Ok(_) => true,
            Err(e) => {
                log::warn!("failed to persist messageSecrets: {e:?}");
                false
            }
        }
    }

    async fn own_jid_for_secret_encrypted(&self, info: &MessageInfo) -> Option<Jid> {
        use wacore::types::message::AddressingMode;

        if info.source.is_from_me {
            return Some(info.source.sender.to_non_ad());
        }

        match info.source.addressing_mode {
            Some(AddressingMode::Lid) => match self.get_lid().await {
                Some(jid) => Some(jid),
                None => self.get_pn().await,
            },
            Some(AddressingMode::Pn) => match self.get_pn().await {
                Some(jid) => Some(jid),
                None => self.get_lid().await,
            },
            None if info.source.sender.is_lid() || info.source.chat.is_lid() => {
                match self.get_lid().await {
                    Some(jid) => Some(jid),
                    None => self.get_pn().await,
                }
            }
            None => match self.get_pn().await {
                Some(jid) => Some(jid),
                None => self.get_lid().await,
            },
        }
    }

    pub(crate) async fn maybe_decrypt_secret_encrypted_message(
        self: &Arc<Self>,
        msg: &wa::Message,
        info: &Arc<MessageInfo>,
    ) -> Option<wa::Message> {
        use crate::features::message_edit::{self, SecretEncKind};

        let env = message_edit::extract_secret_encrypted(msg)?;
        let target_id = env.target_id()?;

        let my_jid = self.own_jid_for_secret_encrypted(info).await?;
        let original_sender = match env.original_sender_for_dispatch(
            info.source.is_from_me,
            &info.source.sender,
            &my_jid,
        ) {
            Ok(jid) => jid,
            Err(_) => return None,
        };

        let backend = self.persistence_manager.backend();
        let chat_for_lookup = info.source.chat.to_non_ad_string();
        let original_sender_str = original_sender.to_non_ad_string();
        let fallback_original_sender = self
            .alternate_msg_secret_jid(&backend, &original_sender)
            .await
            .unwrap_or_default();

        // Look up the secret AND the parent's event time (for the edit window
        // check below): primary sender, then the LID/PN alternate.
        let store_secret = match backend
            .get_msg_secret_with_ts(&chat_for_lookup, &original_sender_str, target_id)
            .await
        {
            Ok(Some(found)) => Some(found),
            Ok(None) => match fallback_original_sender.as_ref() {
                Some(alt) => {
                    let alt_str = alt.to_non_ad_string();
                    match backend
                        .get_msg_secret_with_ts(&chat_for_lookup, &alt_str, target_id)
                        .await
                    {
                        Ok(found) => found,
                        Err(e) => {
                            log::warn!(
                                "[msg:{}] secret_encrypted_message alternate secret lookup failed: {e:?}",
                                info.id
                            );
                            None
                        }
                    }
                }
                None => None,
            },
            Err(e) => {
                log::warn!(
                    "[msg:{}] backend error reading secret_encrypted_message secret: {e:?}",
                    info.id
                );
                None
            }
        };
        // On a total store miss, ask the app-supplied resolver (if any) for the
        // parent secret. This is what lets the Disabled policy still decrypt. The
        // resolver carries no parent timestamp, so parent_ts stays 0 (unknown).
        let (secret, parent_ts) = match store_secret {
            Some((secret, ts)) => (secret, ts),
            None => {
                let alternate = fallback_original_sender
                    .as_ref()
                    .map(|j| j.to_non_ad_string());
                match self
                    .resolve_msg_secret_via_app(
                        &chat_for_lookup,
                        &original_sender_str,
                        alternate.as_deref(),
                        target_id,
                    )
                    .await
                {
                    Some(secret) => (secret, 0),
                    None => return None,
                }
            }
        };

        let fallback_editor = match info.source.sender_alt.clone() {
            Some(jid) => Some(jid),
            None => self
                .alternate_msg_secret_jid(&backend, &info.source.sender)
                .await
                .unwrap_or_default(),
        };

        let inner = match message_edit::decrypt_secret_encrypted(
            env.enc_payload,
            env.enc_iv,
            &secret,
            env.kind,
            target_id,
            &original_sender,
            &info.source.sender,
        ) {
            Ok(inner) => inner,
            Err(primary_err) => {
                let mut last_err = primary_err;
                let mut decrypted = None;

                if let Some(fallback_original) = fallback_original_sender.as_ref() {
                    match message_edit::decrypt_secret_encrypted(
                        env.enc_payload,
                        env.enc_iv,
                        &secret,
                        env.kind,
                        target_id,
                        fallback_original,
                        &info.source.sender,
                    ) {
                        Ok(inner) => decrypted = Some(inner),
                        Err(e) => last_err = e,
                    }
                }

                if decrypted.is_none()
                    && let Some(fallback_editor) = fallback_editor.as_ref()
                {
                    match message_edit::decrypt_secret_encrypted(
                        env.enc_payload,
                        env.enc_iv,
                        &secret,
                        env.kind,
                        target_id,
                        &original_sender,
                        fallback_editor,
                    ) {
                        Ok(inner) => decrypted = Some(inner),
                        Err(e) => last_err = e,
                    }
                }

                if decrypted.is_none()
                    && let (Some(fallback_original), Some(fallback_editor)) =
                        (fallback_original_sender.as_ref(), fallback_editor.as_ref())
                {
                    match message_edit::decrypt_secret_encrypted(
                        env.enc_payload,
                        env.enc_iv,
                        &secret,
                        env.kind,
                        target_id,
                        fallback_original,
                        fallback_editor,
                    ) {
                        Ok(inner) => decrypted = Some(inner),
                        Err(e) => last_err = e,
                    }
                }

                match decrypted {
                    Some(inner) => inner,
                    None => {
                        log::warn!(
                            "[msg:{}] secret_encrypted_message {:?} decrypt failed: {last_err:?}",
                            info.id,
                            env.kind
                        );
                        return None;
                    }
                }
            }
        };

        // Mirror WA Web `ProcessEditProtocolMsgs`: drop a MESSAGE_EDIT authored
        // outside the parent's edit-processing window (editTs >= parentTs + 20m).
        // The check is on authored time, not "now", so a validly-authored edit
        // still applies after an offline delivery gap. Only enforceable when we
        // know the parent's event time; resolver-supplied secrets carry none
        // (parent_ts == 0), so we stay permissive there.
        if env.kind == SecretEncKind::MessageEdit && parent_ts > 0 {
            let edit_ts = info.timestamp.timestamp();
            if edit_ts >= parent_ts + wacore::msg_secret::EDIT_PROCESSING_WINDOW_SECS {
                log::debug!(
                    "[msg:{}] secret edit authored outside the {}s window (editTs={edit_ts}, parentTs={parent_ts}); dropping",
                    info.id,
                    wacore::msg_secret::EDIT_PROCESSING_WINDOW_SECS
                );
                return None;
            }
        }

        if let Some(secret_bytes) = inner
            .message_context_info
            .as_ref()
            .and_then(|m| m.message_secret.as_deref())
        {
            // The re-persisted secret keys the NEXT add-on on the same parent,
            // so its retention class follows the parent kind and the parent's own
            // event time (when known) rather than this edit's arrival time.
            let class = match env.kind {
                SecretEncKind::MessageEdit => wacore::msg_secret::RetentionClass::Text,
                _ => wacore::msg_secret::RetentionClass::PollEvent,
            };
            let message_ts = if parent_ts > 0 {
                u64::try_from(parent_ts).ok()
            } else {
                u64::try_from(info.timestamp.timestamp()).ok()
            };
            // Primary + LID/PN alternate in one batch so both survive together.
            let mut entries = Vec::with_capacity(2);
            if let Some(entry) = self.build_msg_secret_entry(
                &info.source.chat,
                &original_sender,
                target_id,
                secret_bytes,
                class,
                message_ts,
            ) {
                entries.push(entry);
            }
            if let Some(alternate_sender) = fallback_original_sender.as_ref()
                && let Some(entry) = self.build_msg_secret_entry(
                    &info.source.chat,
                    alternate_sender,
                    target_id,
                    secret_bytes,
                    class,
                    message_ts,
                )
            {
                entries.push(entry);
            }
            self.persist_msg_secret_entries(entries).await;
        }

        if env.kind != SecretEncKind::MessageEdit {
            return Some(inner);
        }

        match message_edit::rewrap_as_legacy_edit(inner) {
            Some(rewrapped) => Some(rewrapped),
            None => {
                log::warn!(
                    "[msg:{}] decrypted MESSAGE_EDIT missing protocol_message.edited_message",
                    info.id
                );
                None
            }
        }
    }

    /// Decrypt and dispatch a `<enc type="msmsg">` bot reply. Looks up the
    /// outbound `messageSecret` we persisted at send time and runs the
    /// dual-HKDF + AES-GCM open from [`wacore::bot_message`]. Failures
    /// (missing secret, GCM tag fail, malformed proto) nack with code 495.
    pub(crate) async fn handle_msmsg_payload(
        self: &Arc<Self>,
        info: &Arc<MessageInfo>,
        payload: EncPayload,
    ) {
        use prost::Message as _;
        use wa::MessageSecretMessage;
        use wacore::bot_message::{BotMessageContext, decrypt_bot_message};
        use wacore::protocol::nack::NackReason;

        let ms_msg = match MessageSecretMessage::decode(&*payload.ciphertext) {
            Ok(m) => m,
            Err(e) => {
                log::warn!(
                    "[msg:{}] failed to decode MessageSecretMessage: {e:?}",
                    info.id
                );
                self.spawn_nack(info, NackReason::ParsingError, None);
                return;
            }
        };
        let (Some(enc_iv), Some(enc_payload)) =
            (ms_msg.enc_iv.as_deref(), ms_msg.enc_payload.as_deref())
        else {
            log::warn!(
                "[msg:{}] MessageSecretMessage missing enc_iv/enc_payload",
                info.id
            );
            self.spawn_nack(info, NackReason::ParsingError, None);
            return;
        };

        // Target sender (us): meta echoes our LID/PN. Falls back to our LID
        // when sender is on the bot server, our PN otherwise (whatsmeow
        // `decryptBotMessage`).
        let target_sender = match self.resolve_msmsg_target_sender(info).await {
            Some(j) => j,
            None => {
                log::warn!("[msg:{}] msmsg: no target_sender resolvable", info.id);
                self.spawn_nack(info, NackReason::MissingMessageSecret, None);
                return;
            }
        };

        // Chat scope for the secret lookup: prefer <meta target_chat_jid>;
        // fall back to the stanza's chat (matches WA Web `decryptMsmsgBotMessage`).
        let chat_for_lookup = info
            .meta_info
            .target_chat
            .as_ref()
            .unwrap_or(&info.source.chat)
            .to_non_ad()
            .to_string();
        let target_sender_str = target_sender.to_non_ad_string();

        // The id used for the SECRET LOOKUP is `meta.target_id` (our outbound
        // id); the id used as HKDF input is the bot reply id (or
        // `bot_info.edit_target_id` when the bot is editing a prior reply).
        let target_id = match info.meta_info.target_id.as_deref() {
            Some(id) => id,
            None => {
                log::warn!(
                    "[msg:{}] msmsg: <meta> missing target_id; cannot look up secret",
                    info.id
                );
                self.spawn_nack(info, NackReason::MissingMessageSecret, None);
                return;
            }
        };

        // Mirror WA Web `C()` in `WAWebBotMessageSecret.js`: primary lookup
        // plus an alternate (PN ↔ LID swap via lid_pn_mapping) so a row
        // stored under one identity family is still found if `<meta
        // target_sender_jid>` echoes the other. Covers LID migration windows
        // and asymmetric outbound/inbound identities.
        let backend = self.persistence_manager.backend();
        // Store lookup: primary, then the LID/PN alternate. A backend error is
        // logged and treated as a miss (not a hard nack) so the resolver still
        // gets a chance — mirrors the secret-encrypted edit path.
        let store_secret = match backend
            .get_msg_secret(&chat_for_lookup, &target_sender_str, target_id)
            .await
        {
            Ok(Some(s)) => Some(s),
            Ok(None) => match self
                .alternate_msg_secret_lookup(&backend, &chat_for_lookup, &target_sender, target_id)
                .await
            {
                Ok(found) => found,
                Err(e) => {
                    log::warn!("[msg:{}] msmsg: alternate lookup failed: {e:?}", info.id);
                    None
                }
            },
            Err(e) => {
                log::warn!(
                    "[msg:{}] backend error reading message_secret: {e:?}",
                    info.id
                );
                None
            }
        };
        let secret = match store_secret {
            Some(s) => s,
            None => {
                let alternate = self
                    .alternate_msg_secret_jid(&backend, &target_sender)
                    .await
                    .ok()
                    .flatten()
                    .map(|j| j.to_non_ad_string());
                match self
                    .resolve_msg_secret_via_app(
                        &chat_for_lookup,
                        &target_sender_str,
                        alternate.as_deref(),
                        target_id,
                    )
                    .await
                {
                    Some(s) => s,
                    None => {
                        // For a group bot invocation initiated by our PRIMARY
                        // device, the messageSecret lives in the bot-addressed
                        // copy the primary sent directly to the bot — it is NOT
                        // mirrored to companions in the group skmsg. So a
                        // companion legitimately never holds the secret; this
                        // miss is expected and benign (we nack 495 and the server
                        // stops replaying). A miss in a 1:1 bot chat is unexpected
                        // and worth a warn.
                        log::log!(
                            if info.source.is_group {
                                log::Level::Debug
                            } else {
                                log::Level::Warn
                            },
                            "[msg:{}] msmsg: no message_secret stored for target_id={target_id} (primary or alternate)",
                            info.id
                        );
                        self.spawn_nack(info, NackReason::MissingMessageSecret, None);
                        return;
                    }
                }
            }
        };

        let bot_user_jid = info.source.sender.to_non_ad_string();
        // WA Web `decryptMsmsgBotMessage` dispatches on `isFbidBot()`:
        //   * fbid path pre-resolves to `edit_target_id` for INNER/LAST edits,
        //     `externalId` (info.id) otherwise. Single AES-GCM attempt.
        //   * regular path tries `externalId` first, falls back to
        //     `edit_target_id` on AES-GCM failure.
        // We don't have `isFbidBot()` detection; instead, we unify the two as
        // try-then-fallback with the fbid-style id as primary. That's a strict
        // superset: for INNER/LAST it usually succeeds on the first try (fbid
        // outcome); for any other case primary is `info.id` so we mirror the
        // regular path's first attempt. The fallback is only attempted if
        // `bot_info.edit_target_id` is present.
        let info_id = info.id.as_str();
        let primary_msg_id = info
            .bot_info
            .as_ref()
            .filter(|bi| {
                matches!(
                    bi.edit_type,
                    Some(
                        crate::types::message::BotEditType::Inner
                            | crate::types::message::BotEditType::Last
                    )
                )
            })
            .and_then(|bi| bi.edit_target_id.as_deref())
            .unwrap_or(info_id);
        let fallback_msg_id = if primary_msg_id == info_id {
            info.bot_info
                .as_ref()
                .and_then(|bi| bi.edit_target_id.as_deref())
        } else {
            Some(info_id)
        }
        .filter(|fb| *fb != primary_msg_id);

        let attempt = |msg_id: &str| {
            let ctx = BotMessageContext {
                msg_id,
                target_sender_user_jid: &target_sender_str,
                bot_user_jid: &bot_user_jid,
            };
            decrypt_bot_message(&secret, enc_iv, enc_payload, &ctx)
        };

        let plaintext = match attempt(primary_msg_id) {
            Ok(p) => p,
            Err(primary_err) => match fallback_msg_id {
                Some(fb) => match attempt(fb) {
                    Ok(p) => p,
                    Err(fallback_err) => {
                        log::warn!(
                            "[msg:{}] msmsg AES-GCM open failed both attempts (primary={primary_err:?}, fallback={fallback_err:?})",
                            info.id
                        );
                        self.spawn_nack(info, NackReason::MissingMessageSecret, None);
                        return;
                    }
                },
                None => {
                    log::warn!(
                        "[msg:{}] msmsg AES-GCM open failed and no fallback msg_id: {primary_err:?}",
                        info.id
                    );
                    self.spawn_nack(info, NackReason::MissingMessageSecret, None);
                    return;
                }
            },
        };

        let msg = match wa::Message::decode(plaintext.as_slice()) {
            Ok(m) => m,
            Err(e) => {
                log::warn!(
                    "[msg:{}] msmsg plaintext is not a Message proto: {e:?}",
                    info.id
                );
                self.spawn_nack(info, NackReason::ParsingError, None);
                return;
            }
        };

        log::info!(
            "[msg:{}] Successfully decrypted msmsg bot reply from {}",
            info.id,
            info.source.sender
        );
        self.dispatch_parsed_message(msg, info).await;
    }

    /// Resolve `target_sender` for a msmsg stanza: echo from `<meta>` when
    /// present, else fall back to our LID (sender on bot server) or PN.
    async fn resolve_msmsg_target_sender(&self, info: &Arc<MessageInfo>) -> Option<Jid> {
        if let Some(ts) = info.meta_info.target_sender.as_ref() {
            return Some(ts.clone());
        }
        if info.source.sender.server == wacore_binary::Server::Bot {
            self.get_lid().await
        } else {
            self.get_pn().await
        }
    }

    /// Second-chance lookup with the alternate identity family. Mirrors
    /// `WAWebLidMigrationUtils.getAlternateMsgKey`: swap PN ↔ LID via the
    /// `lid_pn_mapping` store and retry. Returns `Ok(None)` when no mapping
    /// is known or the alternate row is absent — the caller treats that as
    /// a terminal miss.
    async fn alternate_msg_secret_jid(
        &self,
        backend: &Arc<dyn crate::store::traits::Backend>,
        primary_sender: &Jid,
    ) -> Result<Option<Jid>, crate::store::error::StoreError> {
        let alternate = match primary_sender.server {
            wacore_binary::Server::Lid => backend
                .get_lid_mapping(&primary_sender.user)
                .await?
                .map(|m| Jid::new(m.phone_number, wacore_binary::Server::Pn)),
            wacore_binary::Server::Pn => backend
                .get_pn_mapping(&primary_sender.user)
                .await?
                .map(|m| Jid::new(m.lid, wacore_binary::Server::Lid)),
            _ => None,
        };
        Ok(alternate)
    }

    async fn alternate_msg_secret_lookup(
        &self,
        backend: &Arc<dyn crate::store::traits::Backend>,
        chat_for_lookup: &str,
        primary_sender: &Jid,
        target_id: &str,
    ) -> Result<Option<Vec<u8>>, crate::store::error::StoreError> {
        let Some(alternate) = self
            .alternate_msg_secret_jid(backend, primary_sender)
            .await?
        else {
            return Ok(None);
        };
        let alternate_str = alternate.to_non_ad_string();
        backend
            .get_msg_secret(chat_for_lookup, &alternate_str, target_id)
            .await
    }

    /// On a total store miss, consult the app-supplied resolver for the parent
    /// secret, trying the primary then the LID/PN alternate sender. Bounded by a
    /// timeout because it runs inside the per-chat receive lane, so a slow app
    /// callback degrades to a miss instead of stalling the chat.
    async fn resolve_msg_secret_via_app(
        &self,
        chat: &str,
        primary_sender: &str,
        alternate_sender: Option<&str>,
        msg_id: &str,
    ) -> Option<Vec<u8>> {
        let resolver = self.cache_config.original_message_resolver.as_ref()?;
        let lookup = async {
            if let Some(secret) = resolver
                .resolve_msg_secret(chat, primary_sender, msg_id)
                .await
            {
                return Some(secret);
            }
            if let Some(alt) = alternate_sender
                && alt != primary_sender
                && let Some(secret) = resolver.resolve_msg_secret(chat, alt, msg_id).await
            {
                return Some(secret);
            }
            None
        };
        match wacore::runtime::timeout(
            &*self.runtime,
            self.cache_config.msg_secret_resolver_timeout,
            lookup,
        )
        .await
        {
            Ok(Some(secret)) => Some(secret.to_vec()),
            Ok(None) => None,
            Err(_) => {
                log::warn!("[msg:{msg_id}] original_message_resolver timed out");
                None
            }
        }
    }
}
