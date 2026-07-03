use super::*;

impl Client {
    /// Look up and include a privacy token in outgoing 1:1 message stanza nodes.
    ///
    /// Follows WA Web's fallback chain (MsgCreateFanoutStanza.js `Re = R(te) ?? D(te, s)`):
    ///   1. tctoken — stored trusted contact token, gated on
    ///      `privacy_token_sending_on_all_1_on_1_messages` (WA Web `R`).
    ///   2. cstoken — `HMAC-SHA256(nct_salt, recipient_lid)` fallback, gated
    ///      independently on `wa_nct_token_send_enabled` (WA Web `D`). The cstoken
    ///      is NOT nested behind the 1:1 prop — WA Web attaches it even when `R`
    ///      returns null.
    ///   3. No token — message sent without token (server may return 463).
    ///
    /// Returns whether a new tc token should be issued after send.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.maybe_tc_token", level = "debug", skip_all, fields(to = %to.observe())))]
    pub(super) async fn maybe_include_tc_token(
        &self,
        to: &Jid,
        extra_nodes: &mut Vec<Node>,
    ) -> bool {
        use wacore::iq::abprops::web;
        use wacore::iq::tctoken::{
            PrivacyTokenChoice, build_cs_token_node, build_tc_token_node, choose_privacy_token,
            compute_cs_token, is_tc_token_expired_with, should_send_new_tc_token_with,
        };

        // Skip for own JID — no need to send a privacy token to ourselves.
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
            return false;
        }

        // Bots and status broadcast don't participate in the privacy token system.
        if to.is_bot() || to.is_status_broadcast() {
            return false;
        }

        // Resolve the destination to an account LID once — reused for the tctoken
        // lookup key, the cstoken HMAC input, and issuance rate-limiting.
        let resolved_lid: Option<wacore_binary::CompactString> = if to.is_lid() {
            Some(to.user.clone())
        } else {
            self.lid_pn_cache.get_current_lid(&to.user).await
        };
        let token_key: &str = resolved_lid.as_deref().unwrap_or(&to.user);

        let backend = self.persistence_manager.backend();
        let tc_config = self.tc_token_config().await;

        let existing = match backend.get_tc_token(token_key).await {
            Ok(entry) => entry,
            Err(e) => {
                log::warn!(target: "Client/TcToken", "Failed to get tc_token for {}: {e}", token_key);
                None
            }
        };

        // Issuance scheduling is independent of the AB props — WA Web's sendTcToken
        // in MsgJob.js fires regardless of whether a token was attached to the stanza.
        let should_issue_after_send = should_send_new_tc_token_with(
            existing.as_ref().and_then(|entry| entry.sender_timestamp),
            &tc_config,
        );

        let has_valid_tc_token = existing.as_ref().is_some_and(|entry| {
            !entry.token.is_empty() && !is_tc_token_expired_with(entry.token_timestamp, &tc_config)
        });
        // cstoken needs both the NCT salt and a resolved account LID (WA Web `D`).
        let can_build_cs_token = snapshot.nct_salt.is_some() && resolved_lid.is_some();

        let tc_send_enabled = self
            .ab_props
            .is_enabled(web::PRIVACY_TOKEN_SENDING_ON_ALL_1_ON_1_MESSAGES)
            .await;
        let nct_send_enabled = self
            .ab_props
            .is_enabled(web::WA_NCT_TOKEN_SEND_ENABLED)
            .await;

        match choose_privacy_token(
            tc_send_enabled,
            nct_send_enabled,
            has_valid_tc_token,
            can_build_cs_token,
        ) {
            PrivacyTokenChoice::TcToken => {
                // `has_valid_tc_token` guarantees a non-empty, non-expired entry.
                if let Some(entry) = &existing {
                    extra_nodes.push(build_tc_token_node(&entry.token));
                }
            }
            PrivacyTokenChoice::CsToken => {
                if let (Some(salt), Some(lid_user)) = (&snapshot.nct_salt, &resolved_lid) {
                    // HMAC input is "user@lid" (account LID without device suffix),
                    // matching WA Web's accountLid.toString().
                    let recipient_lid =
                        wacore_binary::Jid::new(lid_user.as_str(), Server::Lid).to_string();
                    let cs_token = compute_cs_token(salt, &recipient_lid);
                    extra_nodes.push(build_cs_token_node(&cs_token));
                    log::debug!(target: "Client/CsToken", "Attached cstoken for {} (NCT fallback)", to.observe());
                }
            }
            PrivacyTokenChoice::None => {
                log::debug!(target: "Client/CsToken", "No tctoken or NCT cstoken available for {}", to.observe());
            }
        }

        should_issue_after_send
    }

    /// Returns `true` if the issuance IQ succeeded.
    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.issue_tc_token", level = "debug", skip_all, fields(to = %to.observe())))]
    pub(super) async fn issue_tc_token_after_send(&self, to: &Jid) -> bool {
        use wacore::iq::tctoken::IssuePrivacyTokensSpec;

        // Bots and status broadcast don't participate in the privacy token system.
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

        // Persist any echoed tokens (forward-compatible; the real `set privacy`
        // response carries none — WA Web ingests tokens only from the
        // privacy_token notification).
        self.store_issued_tc_tokens(&response.tokens).await;
        // Advance the issuance rate-limit on IQ success regardless of echoed
        // tokens — WA Web's sendTcToken persists tcTokenSenderTimestamp here.
        self.record_tc_token_sender_timestamp(to).await;
        true
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

    /// Record that we issued a trusted-contact token to `to` at the current time.
    ///
    /// WA Web's `sendTcToken` persists `tcTokenSenderTimestamp` on IQ success,
    /// independent of any token echoed in the response — the real `set privacy`
    /// response carries no token bytes. Deriving this update from echoed tokens
    /// would leave the sender bucket permanently unset and re-issue a privacy IQ
    /// on every 1:1 message. When no token is stored yet, a byte-less placeholder
    /// carries the timestamp until the contact's token arrives via notification.
    async fn record_tc_token_sender_timestamp(&self, to: &Jid) {
        use wacore::store::traits::TcTokenEntry;

        let key = self.resolve_tc_token_key(to).await;
        let backend = self.persistence_manager.backend();
        let now = wacore::time::now_secs();
        let entry = match backend.get_tc_token(&key).await {
            Ok(Some(existing)) => TcTokenEntry {
                sender_timestamp: Some(now),
                ..existing
            },
            Ok(None) => TcTokenEntry {
                token: Vec::new(),
                token_timestamp: now,
                sender_timestamp: Some(now),
            },
            Err(e) => {
                log::warn!(target: "Client/TcToken", "Failed to load tc_token for {key}: {e}");
                return;
            }
        };
        if let Err(e) = backend.put_tc_token(&key, &entry).await {
            log::warn!(target: "Client/TcToken", "Failed to record tc_token sender_timestamp for {key}: {e}");
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

        let token_jid = self.resolve_tc_token_key(sender).await;

        let backend = self.persistence_manager.backend();
        let entry = match backend.get_tc_token(&token_jid).await {
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

        let token_key = self.resolve_tc_token_key(jid).await;
        let tc_config = self.tc_token_config().await;
        let backend = self.persistence_manager.backend();
        match backend.get_tc_token(&token_key).await {
            Ok(Some(entry))
                if !entry.token.is_empty()
                    && !is_tc_token_expired_with(entry.token_timestamp, &tc_config) =>
            {
                Some(entry.token)
            }
            Ok(_) => None,
            Err(e) => {
                log::warn!(target: "Client/TcToken", "Failed to get tc_token for {}: {e}", token_key);
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

    /// Resolve a JID to the tc_token storage key: the account LID user when
    /// resolvable, else the bare user. The attachment lookup, issuance
    /// rate-limiting, and feature-gating helpers all key on this so they stay
    /// consistent for the same contact.
    pub(crate) async fn resolve_tc_token_key(&self, jid: &Jid) -> String {
        if jid.is_lid() {
            jid.user.to_string()
        } else if let Some(lid_user) = self.lid_pn_cache.get_current_lid(&jid.user).await {
            lid_user.to_string()
        } else {
            jid.user.to_string()
        }
    }

    /// Resolve a JID to its LID form for tc_token issuance targeting.
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

#[cfg(test)]
mod tests {
    use crate::test_utils::create_test_client;
    use wacore::store::traits::TcTokenEntry;
    use wacore_binary::{Jid, Server};

    #[tokio::test]
    async fn record_sender_timestamp_creates_byteless_placeholder() {
        let client = create_test_client().await;
        let jid = Jid::new("770000001", Server::Lid);

        client.record_tc_token_sender_timestamp(&jid).await;

        let entry = client
            .persistence_manager
            .backend()
            .get_tc_token("770000001")
            .await
            .unwrap()
            .expect("placeholder entry should be created");
        assert!(entry.token.is_empty(), "placeholder carries no token bytes");
        assert!(
            entry.sender_timestamp.is_some(),
            "placeholder records the issuance timestamp"
        );
    }

    #[tokio::test]
    async fn record_sender_timestamp_preserves_existing_token() {
        let client = create_test_client().await;
        let backend = client.persistence_manager.backend();
        backend
            .put_tc_token(
                "770000002",
                &TcTokenEntry {
                    token: vec![1, 2, 3],
                    token_timestamp: 1_700_000_000,
                    sender_timestamp: None,
                },
            )
            .await
            .unwrap();

        let jid = Jid::new("770000002", Server::Lid);
        client.record_tc_token_sender_timestamp(&jid).await;

        let entry = backend.get_tc_token("770000002").await.unwrap().unwrap();
        assert_eq!(entry.token, vec![1, 2, 3], "received token is preserved");
        assert_eq!(entry.token_timestamp, 1_700_000_000);
        assert!(
            entry.sender_timestamp.is_some(),
            "sender_timestamp is advanced on issuance"
        );
    }
}
