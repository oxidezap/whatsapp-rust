use super::*;

impl Client {
    /// Whether `jid` is our own account (PN or LID). The privacy-token paths
    /// never attach to or issue for ourselves; a single source of truth keeps
    /// the message and call paths from drifting apart.
    fn is_own_jid(&self, jid: &Jid) -> bool {
        let snapshot = self.persistence_manager.get_device_snapshot();
        snapshot
            .pn
            .as_ref()
            .is_some_and(|pn| pn.is_same_user_as(jid))
            || snapshot
                .lid
                .as_ref()
                .is_some_and(|lid| lid.is_same_user_as(jid))
    }

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
        if self.is_own_jid(to) {
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
                log::warn!(target: "Client/TcToken", "Failed to get tc_token for {}: {e}", to.observe());
                None
            }
        };

        // Issuance scheduling is independent of the AB props — WA Web's sendTcToken
        // in MsgJob.js fires regardless of whether a token was attached to the stanza.
        let should_issue_after_send = should_send_new_tc_token_with(
            existing.as_ref().and_then(|entry| entry.sender_timestamp),
            &tc_config,
        );

        // Bind the token payloads up front so the match arms encode the choice
        // without re-checking invariants that choose_privacy_token already proved.
        let valid_tc_token: Option<&[u8]> = existing.as_ref().and_then(|entry| {
            (!entry.token.is_empty()
                && !is_tc_token_expired_with(entry.token_timestamp, &tc_config))
            .then_some(entry.token.as_slice())
        });
        // cstoken needs both the NCT salt and a resolved account LID (WA Web `D`).
        let snapshot = self.persistence_manager.get_device_snapshot();
        let cs_token_inputs: Option<(&[u8], &wacore_binary::CompactString)> =
            match (&snapshot.nct_salt, &resolved_lid) {
                (Some(salt), Some(lid)) => Some((salt.as_slice(), lid)),
                _ => None,
            };

        let tc_send_enabled = self
            .ab_props
            .is_enabled(web::PRIVACY_TOKEN_SENDING_ON_ALL_1_ON_1_MESSAGES)
            .await;
        let nct_send_enabled = self
            .ab_props
            .is_enabled(web::WA_NCT_TOKEN_SEND_ENABLED)
            .await;

        let choice = choose_privacy_token(
            tc_send_enabled,
            nct_send_enabled,
            valid_tc_token.is_some(),
            cs_token_inputs.is_some(),
        );
        match choice {
            PrivacyTokenChoice::TcToken => {
                extra_nodes.extend(valid_tc_token.map(build_tc_token_node))
            }
            PrivacyTokenChoice::CsToken => {
                extra_nodes.extend(cs_token_inputs.map(|(salt, lid_user)| {
                    // HMAC input is "user@lid" (account LID without device suffix),
                    // matching WA Web's accountLid.toString().
                    let recipient_lid = Jid::new(lid_user.as_str(), Server::Lid).to_string();
                    build_cs_token_node(&compute_cs_token(salt, &recipient_lid))
                }));
            }
            PrivacyTokenChoice::None => {}
        }
        log::debug!(target: "Client/TcToken", "privacy token for {}: {choice:?}", to.observe());

        should_issue_after_send
    }

    #[cfg_attr(feature = "tracing", tracing::instrument(name = "wa.send.issue_tc_token", level = "debug", skip_all, fields(to = %to.observe())))]
    pub(crate) async fn issue_tc_token_after_send(&self, to: &Jid) {
        use wacore::iq::tctoken::IssuePrivacyTokensSpec;

        // Bots and status broadcast don't participate in the privacy token system.
        if to.is_bot() || to.is_status_broadcast() {
            return;
        }

        let issuance_jid = self.resolve_issuance_jid(to).await;
        // WA Web's sendTcToken ignores the response body — the echoed token, if
        // any, is not the one we attach (that comes from the privacy_token
        // notification). Only the sender-side timestamp is recorded on success.
        if let Err(e) = self
            .execute(IssuePrivacyTokensSpec::new(std::slice::from_ref(
                &issuance_jid,
            )))
            .await
        {
            log::debug!(target: "Client/TcToken", "Failed to issue tc_token for {}: {e}", issuance_jid.observe());
            return;
        }
        self.record_tc_token_sender_timestamp(to).await;
    }

    /// Whether a fresh tctoken should be issued to `to`, rate-limited by the
    /// sender bucket. Independent of the 1:1 message AB props — WA Web schedules
    /// `sendTcToken` on its own cadence (`MsgJob`, `StartCall`) regardless of
    /// whether a token was attached to the outgoing stanza.
    #[cfg(feature = "voip-runtime")]
    pub(crate) async fn should_issue_tc_token(&self, to: &Jid) -> bool {
        use wacore::iq::tctoken::should_send_new_tc_token_with;

        // A self-call must never issue or record a token for our own account.
        if self.is_own_jid(to) {
            return false;
        }

        if to.is_bot() || to.is_status_broadcast() {
            return false;
        }

        let key = self.resolve_tc_token_key(to).await;
        let sender_ts = match self.persistence_manager.backend().get_tc_token(&key).await {
            Ok(entry) => entry.and_then(|e| e.sender_timestamp),
            Err(e) => {
                log::warn!(target: "Client/TcToken", "Failed to read tc_token for {}: {e}", to.observe());
                None
            }
        };

        should_send_new_tc_token_with(sender_ts, &self.tc_token_config().await)
    }

    /// Persist tokens returned by the explicit `tc_token().issue_tokens()` API.
    pub(crate) async fn store_issued_tc_tokens(
        &self,
        tokens: &[wacore::iq::tctoken::ReceivedTcToken],
    ) {
        use wacore::store::traits::TcTokenEntry;

        let backend = self.persistence_manager.backend();
        let now = wacore::time::now_secs();
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
            }
        }
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

    /// Advance the issuance rate-limit on IQ success, independent of any tokens
    /// echoed in the response.
    ///
    /// WA Web's `sendTcToken` persists `tcTokenSenderTimestamp` unconditionally
    /// on success; the real `set privacy` response echoes no bytes, so deriving
    /// the update from it would leave the sender bucket unset and re-issue on
    /// every 1:1 message — hence the byte-less placeholder when no entry exists.
    async fn record_tc_token_sender_timestamp(&self, to: &Jid) {
        let key = self.resolve_tc_token_key(to).await;
        let now = wacore::time::now_secs();
        // Atomic merge so a concurrent privacy_token notification writing the real
        // token isn't clobbered by this placeholder's read-modify-write.
        if let Err(e) = self
            .persistence_manager
            .backend()
            .touch_tc_token_sender_timestamp(&key, now)
            .await
        {
            log::warn!(target: "Client/TcToken", "Failed to record tc_token sender_timestamp for {}: {e}", to.observe());
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
                log::warn!(target: "Client/TcToken", "Failed to get tc_token for {}: {e}", jid.observe());
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

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn should_issue_tc_token_true_for_unknown_contact() {
        let client = create_test_client().await;
        let jid = Jid::new("770000003", Server::Lid);
        assert!(
            client.should_issue_tc_token(&jid).await,
            "a contact with no recorded issuance should get a token"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn should_issue_tc_token_false_within_sender_bucket() {
        let client = create_test_client().await;
        client
            .persistence_manager
            .backend()
            .put_tc_token(
                "770000004",
                &TcTokenEntry {
                    token: Vec::new(),
                    token_timestamp: 0,
                    sender_timestamp: Some(wacore::time::now_secs()),
                },
            )
            .await
            .unwrap();

        let jid = Jid::new("770000004", Server::Lid);
        assert!(
            !client.should_issue_tc_token(&jid).await,
            "a fresh issuance in the current bucket must not re-issue"
        );
    }

    #[cfg(feature = "voip-runtime")]
    #[tokio::test]
    async fn should_issue_tc_token_false_for_self() {
        let client = create_test_client().await;
        let own = Jid::new("999000111", Server::Lid);
        client
            .persistence_manager
            .process_command(crate::store::commands::DeviceCommand::SetLid(Some(
                own.clone(),
            )))
            .await;

        assert!(
            !client.should_issue_tc_token(&own).await,
            "a self-call must never issue a tc token for our own account"
        );
    }
}
