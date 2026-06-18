use super::*;

impl Client {
    /// Look up and include a privacy token in outgoing 1:1 message stanza nodes.
    ///
    /// Follows WA Web's fallback chain (MsgCreateFanoutStanza.js):
    ///   1. tctoken — from stored trusted contact token (if valid, non-expired)
    ///   2. cstoken — HMAC-SHA256(nct_salt, recipient_lid) fallback for first-contact
    ///   3. No token — message sent without token (server may return 463)
    ///
    /// Returns whether we should issue a new tc token after send, and the cache key
    /// of the attached valid tc token when that token should be marked as used.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.maybe_tc_token", level = "debug", skip_all, fields(to = %to.observe())))]
    pub(super) async fn maybe_include_tc_token(
        &self,
        to: &Jid,
        extra_nodes: &mut Vec<Node>,
    ) -> (bool, Option<String>) {
        use wacore::iq::abprops::web;
        use wacore::iq::tctoken::{
            build_cs_token_node, build_tc_token_node, compute_cs_token, is_tc_token_expired_with,
            should_send_new_tc_token_with,
        };

        // Skip for own JID — no need to send privacy token to ourselves
        let snapshot = self.persistence_manager.get_device_snapshot();
        let is_self = snapshot
            .pn
            .as_ref()
            .is_some_and(|pn| pn.is_same_user_as(to))
            || snapshot
                .lid
                .as_ref()
                .is_some_and(|lid| lid.is_same_user_as(to));
        if is_self {
            return (false, None);
        }

        // Bots and status broadcast don't participate in the privacy token system
        if to.is_bot() || to.is_status_broadcast() {
            return (false, None);
        }

        // Resolve the destination to a LID user string once — reused for
        // tctoken lookup, issuance, and cstoken HMAC input.
        let cached_lid = if to.is_lid() {
            None
        } else {
            self.lid_pn_cache.get_current_lid(&to.user).await
        };
        let resolved_lid_user: Option<&str> = if to.is_lid() {
            Some(&to.user)
        } else {
            cached_lid.as_deref()
        };
        let token_jid: &str = resolved_lid_user.unwrap_or(&to.user);

        let backend = self.persistence_manager.backend();
        let tc_config = self.tc_token_config().await;

        // Look up existing tctoken
        let existing = match backend.get_tc_token(token_jid).await {
            Ok(entry) => entry,
            Err(e) => {
                log::warn!(target: "Client/TcToken", "Failed to get tc_token for {}: {e}", token_jid);
                None
            }
        };

        // Issuance scheduling is independent of the AB prop — WA Web's sendTcToken
        // in MsgJob.js fires regardless of whether a token was attached to the stanza
        let should_issue_after_send = should_send_new_tc_token_with(
            existing.as_ref().and_then(|entry| entry.sender_timestamp),
            &tc_config,
        );

        // AB prop gates stanza inclusion only (not issuance scheduling)
        let token_send_enabled = self
            .ab_props
            .is_enabled(web::PRIVACY_TOKEN_SENDING_ON_ALL_1_ON_1_MESSAGES)
            .await;

        if token_send_enabled {
            match existing {
                Some(ref entry)
                    if !is_tc_token_expired_with(entry.token_timestamp, &tc_config)
                        && !entry.token.is_empty() =>
                {
                    extra_nodes.push(build_tc_token_node(&entry.token));
                    return (should_issue_after_send, Some(token_jid.to_string()));
                }
                _ => {
                    // cstoken fallback — gated by wa_nct_token_send_enabled
                    let nct_send_enabled = self
                        .ab_props
                        .is_enabled(web::WA_NCT_TOKEN_SEND_ENABLED)
                        .await;

                    if nct_send_enabled
                        && let Some(salt) = &snapshot.nct_salt
                        && let Some(lid_user) = &resolved_lid_user
                    {
                        // HMAC input is "user@lid" (account LID without device suffix),
                        // matching WA Web's accountLid.toString()
                        let recipient_lid =
                            wacore_binary::Jid::new(*lid_user, Server::Lid).to_string();
                        let cs_token = compute_cs_token(salt, &recipient_lid);
                        extra_nodes.push(build_cs_token_node(&cs_token));
                        log::debug!(target: "Client/CsToken", "Attached cstoken for {} (NCT fallback)", to.observe());
                    } else {
                        log::debug!(target: "Client/CsToken", "No tctoken or NCT salt/LID available for {}", to.observe());
                    }
                }
            }
        }

        (should_issue_after_send, None)
    }

    /// Returns `true` if the issuance IQ succeeded.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.issue_tc_token", level = "debug", skip_all, fields(to = %to.observe())))]
    pub(super) async fn issue_tc_token_after_send(&self, to: &Jid) -> bool {
        use wacore::iq::tctoken::IssuePrivacyTokensSpec;

        // Bots and status broadcast don't participate in the privacy token system
        if to.is_bot() || to.is_status_broadcast() {
            return false;
        }

        let issuance_jid = self.resolve_issuance_jid(to).await;
        let Ok(response) = self
            .execute(IssuePrivacyTokensSpec::new(std::slice::from_ref(
                &issuance_jid,
            )))
            .await
        else {
            log::debug!(target: "Client/TcToken", "Failed to issue tc_token for {}", issuance_jid.observe());
            return false;
        };

        self.store_issued_tc_tokens(&response.tokens).await
    }

    /// Returns true if at least one token was persisted.
    pub(crate) async fn store_issued_tc_tokens(
        &self,
        tokens: &[wacore::iq::tctoken::ReceivedTcToken],
    ) -> bool {
        use wacore::store::traits::TcTokenEntry;

        if tokens.is_empty() {
            return false;
        }

        let backend = self.persistence_manager.backend();
        let now = wacore::time::now_secs();
        let mut any_stored = false;
        for received in tokens {
            if received.token.is_empty() {
                log::warn!(target: "Client/TcToken", "Server returned empty tc_token for {}, skipping", received.jid.observe());
                continue;
            }

            let entry = TcTokenEntry {
                token: received.token.clone(),
                token_timestamp: received.timestamp,
                sender_timestamp: Some(now),
            };

            if let Err(e) = backend.put_tc_token(&received.jid.user, &entry).await {
                log::warn!(target: "Client/TcToken", "Failed to store issued tc_token: {e}");
            } else {
                any_stored = true;
            }
        }
        any_stored
    }

    /// Variant of [`store_issued_tc_tokens`] that preserves the original
    /// sender_timestamp for identity-change re-issuance (bucket continuity).
    async fn store_issued_tc_tokens_with_sender_ts(
        &self,
        tokens: &[wacore::iq::tctoken::ReceivedTcToken],
        sender_ts: i64,
    ) {
        use wacore::store::traits::TcTokenEntry;

        let backend = self.persistence_manager.backend();
        for received in tokens {
            if received.token.is_empty() {
                continue;
            }
            let entry = TcTokenEntry {
                token: received.token.clone(),
                token_timestamp: received.timestamp,
                sender_timestamp: Some(sender_ts),
            };
            if let Err(e) = backend.put_tc_token(&received.jid.user, &entry).await {
                log::warn!(target: "Client/TcToken", "Failed to store re-issued tc_token: {e}");
            }
        }
    }

    pub(super) async fn mark_tc_token_used_after_send(&self, token_key: &str) {
        use wacore::store::traits::TcTokenEntry;

        let backend = self.persistence_manager.backend();
        let existing = match backend.get_tc_token(token_key).await {
            Ok(entry) => entry,
            Err(e) => {
                log::warn!(target: "Client/TcToken", "Failed to reload tc_token for {}: {e}", token_key);
                return;
            }
        };

        let Some(entry) = existing else {
            return;
        };
        if entry.token.is_empty() {
            return;
        }

        let updated_entry = TcTokenEntry {
            sender_timestamp: Some(wacore::time::now_secs()),
            ..entry
        };
        if let Err(e) = backend.put_tc_token(token_key, &updated_entry).await {
            log::warn!(target: "Client/TcToken", "Failed to update sender_timestamp for {}: {e}", token_key);
        }
    }

    /// Re-issue tctoken after a contact's device identity changes.
    /// Only re-issues if we previously sent a token (sender_timestamp valid).
    /// Uses session_locks to deduplicate concurrent spawns for the same sender.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.reissue_tc_token", level = "debug", skip_all, fields(sender = %sender.observe())))]
    pub(crate) async fn reissue_tc_token_after_identity_change(&self, sender: &Jid) {
        use wacore::iq::tctoken::{IssuePrivacyTokensSpec, is_sender_tc_token_expired};

        // Dedup via session_locks — bare JID won't collide with protocol addresses ("user:device")
        let bare = sender.to_non_ad_string();
        let mutex = self.session_lock_for(&bare).await;
        let Some(_guard) = mutex.try_lock() else {
            return;
        };

        let resolved_lid = if sender.is_lid() {
            None
        } else {
            self.lid_pn_cache.get_current_lid(&sender.user).await
        };
        let token_jid: &str = resolved_lid.as_deref().unwrap_or(&sender.user);

        let backend = self.persistence_manager.backend();
        let entry = match backend.get_tc_token(token_jid).await {
            Ok(Some(e)) => e,
            _ => return,
        };

        let Some(sender_ts) = entry.sender_timestamp else {
            return;
        };

        // Sender-side expiration (may use different bucket config than receiver)
        let tc_config = self.tc_token_config().await;
        if is_sender_tc_token_expired(sender_ts, &tc_config) {
            return;
        }

        // Use stored sender_ts so the bucket window isn't advanced
        let issuance_jid = self.resolve_issuance_jid(sender).await;
        match self
            .execute(IssuePrivacyTokensSpec::with_timestamp(
                std::slice::from_ref(&issuance_jid),
                sender_ts,
            ))
            .await
        {
            Ok(response) => {
                // Keep original sender_ts so the bucket window isn't advanced
                self.store_issued_tc_tokens_with_sender_ts(&response.tokens, sender_ts)
                    .await;
                log::debug!(
                    target: "Client/TcToken",
                    "Re-issued tctoken after identity change for {}",
                    sender.observe()
                );
            }
            Err(e) => {
                log::debug!(
                    target: "Client/TcToken",
                    "Failed to re-issue tctoken after identity change for {}: {e}",
                    sender.observe()
                );
            }
        }
    }

    /// Look up a valid (non-expired) tctoken for a JID. Returns the raw token bytes if found.
    ///
    /// Used by profile picture, presence subscribe, and other features that need tctoken gating.
    pub(crate) async fn lookup_tc_token_for_jid(&self, jid: &Jid) -> Option<Vec<u8>> {
        use wacore::iq::tctoken::is_tc_token_expired_with;

        let resolved_lid = if jid.is_lid() {
            None
        } else {
            self.lid_pn_cache.get_current_lid(&jid.user).await
        };
        let token_jid: &str = resolved_lid.as_deref().unwrap_or(&jid.user);

        let tc_config = self.tc_token_config().await;
        let backend = self.persistence_manager.backend();
        match backend.get_tc_token(token_jid).await {
            Ok(Some(entry))
                if !entry.token.is_empty()
                    && !is_tc_token_expired_with(entry.token_timestamp, &tc_config) =>
            {
                Some(entry.token)
            }
            Ok(_) => None,
            Err(e) => {
                log::warn!(target: "Client/TcToken", "Failed to get tc_token for {}: {e}", token_jid);
                None
            }
        }
    }

    /// Build tctoken timing config from AB props, falling back to defaults.
    pub(crate) async fn tc_token_config(&self) -> wacore::iq::tctoken::TcTokenConfig {
        use wacore::iq::abprops::web;
        use wacore::iq::tctoken::TcTokenConfig;

        TcTokenConfig {
            bucket_duration: self.ab_props.get_int(web::TCTOKEN_DURATION).await,
            num_buckets: self.ab_props.get_int(web::TCTOKEN_NUM_BUCKETS).await,
            sender_bucket_duration: self.ab_props.get_int(web::TCTOKEN_DURATION_SENDER).await,
            sender_num_buckets: self.ab_props.get_int(web::TCTOKEN_NUM_BUCKETS_SENDER).await,
        }
        .clamped()
    }

    /// Resolve a JID to its LID form for tc_token storage.
    async fn resolve_to_lid_jid(&self, jid: &Jid) -> Jid {
        if jid.is_lid() {
            return jid.to_non_ad();
        }

        if let Some(lid_user) = self.lid_pn_cache.get_current_lid(&jid.user).await {
            Jid::new(lid_user, Server::Lid)
        } else {
            jid.to_non_ad()
        }
    }

    /// Resolve the target JID for privacy token issuance.
    /// Gated by `lid_trusted_token_issue_to_lid` — LID when true, PN when false.
    async fn resolve_issuance_jid(&self, jid: &Jid) -> Jid {
        use wacore::iq::abprops::web;

        // Matches the upstream default (false); the server overrides per-account.
        let issue_to_lid = self
            .ab_props
            .is_enabled(web::LID_TRUSTED_TOKEN_ISSUE_TO_LID)
            .await;

        let resolved = if issue_to_lid {
            self.resolve_to_lid_jid(jid).await
        } else if jid.is_lid() {
            if let Some(pn) = self.lid_pn_cache.get_phone_number(&jid.user).await {
                Jid::new(&pn, Server::Pn)
            } else {
                jid.to_non_ad()
            }
        } else {
            jid.to_non_ad()
        };
        // Issuance targets bare account JIDs, not device-scoped ones
        resolved.into_non_ad()
    }
}
