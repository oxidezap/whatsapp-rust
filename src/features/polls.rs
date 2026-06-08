//! Poll creation, voting, and vote decryption.

use std::collections::HashMap;

use anyhow::{Result, anyhow};
use wacore::poll;
use wacore_binary::{Jid, JidExt};
use waproto::whatsapp as wa;

use crate::client::Client;
use crate::send::SendResult;

#[derive(Debug, Clone)]
pub struct PollOptionResult {
    pub name: String,
    pub voters: Vec<String>,
}

pub struct Polls<'a> {
    client: &'a Client,
}

impl<'a> Polls<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Caller needs the returned `message_secret` to decrypt votes.
    pub async fn create(
        &self,
        to: &Jid,
        name: &str,
        options: &[String],
        selectable_count: u32,
    ) -> Result<(SendResult, Vec<u8>)> {
        if options.len() < 2 {
            return Err(anyhow!("Poll must have at least 2 options"));
        }
        if options.len() > 12 {
            return Err(anyhow!("Polls can have a maximum of 12 options"));
        }
        if selectable_count < 1 || selectable_count > options.len() as u32 {
            return Err(anyhow!(
                "selectable_count must be between 1 and {} (got {selectable_count})",
                options.len()
            ));
        }

        // Duplicate names would produce identical SHA-256 hashes, making votes indistinguishable
        let mut seen = std::collections::HashSet::new();
        for opt in options {
            if !seen.insert(opt) {
                return Err(anyhow!("Duplicate option name: {opt}"));
            }
        }

        let poll_options: Vec<wa::message::poll_creation_message::Option> = options
            .iter()
            .map(|name| wa::message::poll_creation_message::Option {
                option_name: Some(name.clone()),
                option_hash: None,
            })
            .collect();

        let poll_msg = wa::message::PollCreationMessage {
            enc_key: None,
            name: Some(name.to_string()),
            options: poll_options,
            selectable_options_count: Some(selectable_count),
            context_info: None,
            // WA Web's GeneratePollCreationMessageProto always sets pollContentType
            // (TEXT=1 for a normal poll); omitting it drops a field the real client
            // always emits. poll_type=POLL is 0 (proto default) so None is wire-equal.
            poll_content_type: Some(wa::message::PollContentType::Text as i32),
            poll_type: None,
            correct_answer: None,
            ..Default::default()
        };

        // WA Web: v3 for single-select, v1 for multi-select (GeneratePollCreationMessageProto.js:39-41)
        let mut message = if selectable_count == 1 {
            wa::Message {
                poll_creation_message_v3: Some(Box::new(poll_msg)),
                ..Default::default()
            }
        } else {
            wa::Message {
                poll_creation_message: Some(Box::new(poll_msg)),
                ..Default::default()
            }
        };

        // WA Web generates a 32-byte random secret at poll creation time
        // (SendPollCreationMsgAction.js:158). Voters need this to derive their encryption key.
        let message_secret: Vec<u8> = {
            use rand::Rng;
            let mut secret = vec![0u8; 32];
            rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut secret);
            secret
        };

        message.message_context_info = Some(wa::MessageContextInfo {
            message_secret: Some(message_secret.clone()),
            ..Default::default()
        });

        let result = self.client.send_message(to.clone(), message).await?;
        Ok((result, message_secret))
    }

    pub async fn vote(
        &self,
        chat_jid: &Jid,
        poll_msg_id: &str,
        poll_creator_jid: &Jid,
        message_secret: &[u8],
        option_names: &[String],
    ) -> Result<SendResult> {
        let my_jid = self
            .client
            .get_pn()
            .await
            .ok_or_else(|| anyhow!("Not logged in — cannot determine own JID"))?;
        let my_base = my_jid.to_non_ad();

        let voter_jid = self
            .resolve_voter_jid(poll_creator_jid, &my_base, poll_msg_id)
            .await;
        let voter_jid_str = voter_jid.to_string();
        let creator_jid_str = poll_creator_jid.to_non_ad_string();

        let selected_hashes: Vec<Vec<u8>> = option_names
            .iter()
            .map(|name| poll::compute_option_hash(name).to_vec())
            .collect();

        let (enc_payload, iv) = poll::encrypt_poll_vote_with_secret(
            &selected_hashes,
            message_secret,
            poll_msg_id,
            &creator_jid_str,
            &voter_jid_str,
        )?;

        let from_me = my_base.is_same_user_as(poll_creator_jid);

        let poll_update = wa::message::PollUpdateMessage {
            poll_creation_message_key: Some(wa::MessageKey {
                remote_jid: Some(chat_jid.to_string()),
                from_me: Some(from_me),
                id: Some(poll_msg_id.to_string()),
                participant: if chat_jid.is_group() {
                    Some(poll_creator_jid.to_string())
                } else {
                    None
                },
            }),
            vote: Some(wa::message::PollEncValue {
                enc_payload: Some(enc_payload),
                enc_iv: Some(iv.to_vec()),
            }),
            // WA Web's GeneratePollVoteMessageProto never sets metadata; a Some(empty)
            // submessage emits a stray `1A 00` (tag 3) on the wire. Omit it.
            metadata: None,
            sender_timestamp_ms: Some(wacore::time::now_millis()),
        };

        let message = wa::Message {
            poll_update_message: Some(poll_update),
            ..Default::default()
        };

        self.client.send_message(chat_jid.clone(), message).await
    }

    /// The voter (self) JID keys the vote's HKDF/AAD, so it must use the poll
    /// creator's namespace, else the host derives a different key. Own LID for
    /// LID-addressed polls, own PN otherwise, falling back to PN when our LID
    /// isn't known yet. Matches WA Web `WAWebAddonEncryption`.
    async fn resolve_voter_jid(
        &self,
        poll_creator_jid: &Jid,
        own_pn: &Jid,
        poll_msg_id: &str,
    ) -> Jid {
        if !poll_creator_jid.is_lid() {
            return own_pn.clone();
        }
        match self.client.get_lid().await {
            Some(lid) => lid.to_non_ad(),
            None => {
                log::warn!(
                    "Poll {poll_msg_id} is LID-addressed but own LID is unknown; \
                     falling back to PN voter (host may fail to decrypt)"
                );
                own_pn.clone()
            }
        }
    }

    /// Selected option hashes (32 bytes each). Retries under the opposite
    /// namespace (LID/PN) when a counterpart is known, so votes authored across
    /// the LID migration still open. Mirrors WA Web `WAWebAddonEncryption`.
    pub async fn decrypt_vote(
        &self,
        enc_payload: &[u8],
        enc_iv: &[u8],
        message_secret: &[u8],
        poll_msg_id: &str,
        poll_creator_jid: &Jid,
        voter_jid: &Jid,
    ) -> Result<Vec<Vec<u8>>> {
        let creator = poll_creator_jid.to_non_ad();
        let voter = voter_jid.to_non_ad();
        let creator_str = creator.to_string();
        let voter_str = voter.to_string();

        let creator_alt = self.swapped_user(&creator).await;
        let voter_alt = self.swapped_user(&voter).await;
        let fallback = Self::build_fallback(&creator_alt, &voter_alt);

        poll::decrypt_poll_vote_with_fallback(
            enc_payload,
            enc_iv,
            message_secret,
            poll_msg_id,
            poll::PollVoteAddressing {
                poll_creator_jid: &creator_str,
                voter_jid: &voter_str,
            },
            fallback,
        )
    }

    /// Non-AD LID/PN counterpart of a user JID, or `None` when unmapped.
    async fn swapped_user(&self, jid: &Jid) -> Option<String> {
        self.client
            .swap_pn_lid_namespace(jid)
            .await
            .map(|j| j.to_non_ad_string())
    }

    /// Fallback pair only when both JIDs have a counterpart, keeping it
    /// homogeneous (LID or PN, never mixed) like WA Web's `decryptAddOn`.
    fn build_fallback<'b>(
        creator_alt: &'b Option<String>,
        voter_alt: &'b Option<String>,
    ) -> Option<poll::PollVoteAddressing<'b>> {
        match (creator_alt, voter_alt) {
            (Some(c), Some(v)) => Some(poll::PollVoteAddressing {
                poll_creator_jid: c,
                voter_jid: v,
            }),
            _ => None,
        }
    }

    /// Decrypts each vote and tallies per-option results.
    /// Later votes from the same voter replace earlier ones (last-vote-wins).
    /// `votes` should be ordered oldest-first.
    ///
    /// Dedupes voters by their canonical (LID-preferred) identity so a voter who
    /// re-votes under the other namespace after migrating replaces, rather than
    /// duplicates, their earlier vote.
    pub async fn aggregate_votes(
        &self,
        poll_options: &[String],
        votes: &[(&Jid, &[u8], &[u8])], // (voter_jid, enc_payload, enc_iv)
        message_secret: &[u8],
        poll_msg_id: &str,
        poll_creator_jid: &Jid,
    ) -> Result<Vec<PollOptionResult>> {
        let option_hashes: Vec<([u8; 32], &str)> = poll_options
            .iter()
            .map(|name| (poll::compute_option_hash(name), name.as_str()))
            .collect();

        // Creator addressing is constant across voters; resolve its counterpart once.
        let creator = poll_creator_jid.to_non_ad();
        let creator_str = creator.to_string();
        let creator_alt = self.swapped_user(&creator).await;

        // Keyed by canonical (LID-preferred) identity; value holds the
        // as-received JID for the reported voters list. Last-vote-wins.
        let mut latest_votes: HashMap<String, (String, Vec<Vec<u8>>)> =
            HashMap::with_capacity(votes.len());
        for (voter_jid, enc_payload, enc_iv) in votes {
            let voter = voter_jid.to_non_ad();
            let voter_str = voter.to_string();
            let voter_alt = self.swapped_user(&voter).await;
            let fallback = Self::build_fallback(&creator_alt, &voter_alt);
            let canonical_voter = if voter.is_lid() {
                voter_str.clone()
            } else {
                voter_alt.clone().unwrap_or_else(|| voter_str.clone())
            };
            match poll::decrypt_poll_vote_with_fallback(
                enc_payload,
                enc_iv,
                message_secret,
                poll_msg_id,
                poll::PollVoteAddressing {
                    poll_creator_jid: &creator_str,
                    voter_jid: &voter_str,
                },
                fallback,
            ) {
                Ok(selected_hashes) => {
                    if selected_hashes.is_empty() {
                        latest_votes.remove(&canonical_voter);
                    } else {
                        latest_votes.insert(canonical_voter, (voter_str, selected_hashes));
                    }
                }
                Err(e) => {
                    log::warn!("Failed to decrypt vote from {voter_jid}: {e}");
                }
            }
        }

        let mut results: Vec<PollOptionResult> = poll_options
            .iter()
            .map(|name| PollOptionResult {
                name: name.clone(),
                voters: Vec::new(),
            })
            .collect();

        for (display_jid, selected_hashes) in latest_votes.values() {
            for hash in selected_hashes {
                if let Ok(hash_arr) = <[u8; 32]>::try_from(hash.as_slice())
                    && let Some(idx) = option_hashes.iter().position(|(h, _)| *h == hash_arr)
                {
                    results[idx].voters.push(display_jid.clone());
                }
            }
        }

        Ok(results)
    }
}

impl Client {
    pub fn polls(&self) -> Polls<'_> {
        Polls::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lid_pn_cache::LearningSource;
    use crate::store::commands::DeviceCommand;
    use crate::test_utils::create_test_client;
    use std::sync::Arc;

    // encrypt-side voter selection (resolve_voter_jid)

    #[tokio::test]
    async fn voter_is_pn_when_poll_creator_is_pn() {
        let client: Arc<Client> = create_test_client().await;
        let own_pn = Jid::pn("5511999999999");
        let creator = Jid::pn("5511777777777");

        let voter = client
            .polls()
            .resolve_voter_jid(&creator, &own_pn, "POLLID")
            .await;
        assert_eq!(voter, own_pn);
    }

    #[tokio::test]
    async fn voter_is_own_lid_when_poll_creator_is_lid() {
        let client: Arc<Client> = create_test_client().await;
        let own_lid: Jid = "888000888000888:3@lid".parse().unwrap();
        client
            .persistence_manager
            .process_command(DeviceCommand::SetLid(Some(own_lid.clone())))
            .await;

        let own_pn = Jid::pn("5511999999999");
        let creator = Jid::lid("111000111000111");

        let voter = client
            .polls()
            .resolve_voter_jid(&creator, &own_pn, "POLLID")
            .await;
        assert!(voter.is_lid(), "voter must be LID-addressed in a LID poll");
        assert_eq!(voter.user, own_lid.user);
        assert_eq!(voter, own_lid.to_non_ad());
    }

    #[tokio::test]
    async fn voter_falls_back_to_pn_when_own_lid_unknown() {
        let client: Arc<Client> = create_test_client().await;
        // No SetLid, so get_lid() is None.
        let own_pn = Jid::pn("5511999999999");
        let creator = Jid::lid("111000111000111");

        let voter = client
            .polls()
            .resolve_voter_jid(&creator, &own_pn, "POLLID")
            .await;
        assert_eq!(voter, own_pn);
    }

    // decrypt-side LID/PN fallback

    /// A vote encrypted under the PN pair must still decrypt when the consumer
    /// only knows the LID JIDs, via the namespace-swap fallback.
    #[tokio::test]
    async fn decrypt_vote_recovers_when_fed_lid_but_encrypted_under_pn() {
        let client: Arc<Client> = create_test_client().await;
        let secret = [0x21u8; 32];
        let stanza_id = "3EB0POLLVOTE";

        let creator_pn = "5511777777777";
        let creator_lid = "111000111000111";
        let voter_pn = "5511888888888";
        let voter_lid = "222000222000222";

        client
            .add_lid_pn_mapping(creator_lid, creator_pn, LearningSource::Usync)
            .await
            .unwrap();
        client
            .add_lid_pn_mapping(voter_lid, voter_pn, LearningSource::Usync)
            .await
            .unwrap();

        let hashes = vec![poll::compute_option_hash("Yes").to_vec()];
        let (enc, iv) = poll::encrypt_poll_vote_with_secret(
            &hashes,
            &secret,
            stanza_id,
            &Jid::pn(creator_pn).to_string(),
            &Jid::pn(voter_pn).to_string(),
        )
        .unwrap();

        // Consumer feeds LID JIDs; primary (LID) fails, fallback swaps to PN.
        let out = client
            .polls()
            .decrypt_vote(
                &enc,
                &iv,
                &secret,
                stanza_id,
                &Jid::lid(creator_lid),
                &Jid::lid(voter_lid),
            )
            .await
            .expect("fallback should rescue the PN-encrypted vote");
        assert_eq!(out, hashes);
    }

    /// Without a known mapping there is no fallback pair, so a LID-fed decrypt
    /// of a PN-encrypted vote must fail rather than silently mis-decrypt.
    #[tokio::test]
    async fn decrypt_vote_fails_without_mapping() {
        let client: Arc<Client> = create_test_client().await;
        let secret = [0x21u8; 32];
        let stanza_id = "3EB0POLLVOTE";

        let (enc, iv) = poll::encrypt_poll_vote_with_secret(
            &[poll::compute_option_hash("Yes").to_vec()],
            &secret,
            stanza_id,
            &Jid::pn("5511777777777").to_string(),
            &Jid::pn("5511888888888").to_string(),
        )
        .unwrap();

        let res = client
            .polls()
            .decrypt_vote(
                &enc,
                &iv,
                &secret,
                stanza_id,
                &Jid::lid("111000111000111"),
                &Jid::lid("222000222000222"),
            )
            .await;
        assert!(res.is_err(), "no mapping → no fallback → must not decrypt");
    }

    #[tokio::test]
    async fn aggregate_votes_recovers_across_addressing() {
        let client: Arc<Client> = create_test_client().await;
        let secret = [0x31u8; 32];
        let stanza_id = "3EB0AGG";
        let options = vec!["Yes".to_string(), "No".to_string()];

        let creator_pn = "5511777777777";
        let creator_lid = "111000111000111";
        let voter_pn = "5511888888888";
        let voter_lid = "222000222000222";

        client
            .add_lid_pn_mapping(creator_lid, creator_pn, LearningSource::Usync)
            .await
            .unwrap();
        client
            .add_lid_pn_mapping(voter_lid, voter_pn, LearningSource::Usync)
            .await
            .unwrap();

        let (enc, iv) = poll::encrypt_poll_vote_with_secret(
            &[poll::compute_option_hash("Yes").to_vec()],
            &secret,
            stanza_id,
            &Jid::pn(creator_pn).to_string(),
            &Jid::pn(voter_pn).to_string(),
        )
        .unwrap();

        let voter_lid_jid = Jid::lid(voter_lid);
        let votes: Vec<(&Jid, &[u8], &[u8])> = vec![(&voter_lid_jid, &enc, &iv)];

        let results = client
            .polls()
            .aggregate_votes(&options, &votes, &secret, stanza_id, &Jid::lid(creator_lid))
            .await
            .unwrap();

        let yes = results.iter().find(|r| r.name == "Yes").unwrap();
        assert_eq!(yes.voters.len(), 1, "the LID voter's 'Yes' must be tallied");
        let no = results.iter().find(|r| r.name == "No").unwrap();
        assert!(no.voters.is_empty());
    }

    /// Same voter votes as PN then re-votes as LID after migrating. The
    /// canonical key must collapse them so last-vote-wins replaces, not
    /// duplicates, the earlier namespace's entry.
    #[tokio::test]
    async fn aggregate_dedupes_revote_across_namespace() {
        let client: Arc<Client> = create_test_client().await;
        let secret = [0x41u8; 32];
        let stanza_id = "3EB0REVOTE";
        let options = vec!["Yes".to_string(), "No".to_string()];

        let creator_pn = "5511777777777";
        let creator_lid = "111000111000111";
        let voter_pn = "5511888888888";
        let voter_lid = "222000222000222";
        client
            .add_lid_pn_mapping(creator_lid, creator_pn, LearningSource::Usync)
            .await
            .unwrap();
        client
            .add_lid_pn_mapping(voter_lid, voter_pn, LearningSource::Usync)
            .await
            .unwrap();

        // Oldest-first: PN "Yes", then LID "No".
        let (enc_pn, iv_pn) = poll::encrypt_poll_vote_with_secret(
            &[poll::compute_option_hash("Yes").to_vec()],
            &secret,
            stanza_id,
            &Jid::pn(creator_pn).to_string(),
            &Jid::pn(voter_pn).to_string(),
        )
        .unwrap();
        let (enc_lid, iv_lid) = poll::encrypt_poll_vote_with_secret(
            &[poll::compute_option_hash("No").to_vec()],
            &secret,
            stanza_id,
            &Jid::lid(creator_lid).to_string(),
            &Jid::lid(voter_lid).to_string(),
        )
        .unwrap();

        let voter_pn_jid = Jid::pn(voter_pn);
        let voter_lid_jid = Jid::lid(voter_lid);
        let votes: Vec<(&Jid, &[u8], &[u8])> = vec![
            (&voter_pn_jid, &enc_pn, &iv_pn),
            (&voter_lid_jid, &enc_lid, &iv_lid),
        ];

        let results = client
            .polls()
            .aggregate_votes(&options, &votes, &secret, stanza_id, &Jid::lid(creator_lid))
            .await
            .unwrap();

        let yes = results.iter().find(|r| r.name == "Yes").unwrap();
        let no = results.iter().find(|r| r.name == "No").unwrap();
        assert!(yes.voters.is_empty(), "the PN 'Yes' must be replaced");
        assert_eq!(no.voters.len(), 1, "only the re-vote should count, once");
    }

    /// A clear-vote received under the other namespace must remove the prior
    /// vote, not leave a stale entry keyed by the original namespace.
    #[tokio::test]
    async fn aggregate_clears_vote_across_namespace() {
        let client: Arc<Client> = create_test_client().await;
        let secret = [0x51u8; 32];
        let stanza_id = "3EB0CLEAR";
        let options = vec!["Yes".to_string(), "No".to_string()];

        let creator_pn = "5511777777777";
        let creator_lid = "111000111000111";
        let voter_pn = "5511888888888";
        let voter_lid = "222000222000222";
        client
            .add_lid_pn_mapping(creator_lid, creator_pn, LearningSource::Usync)
            .await
            .unwrap();
        client
            .add_lid_pn_mapping(voter_lid, voter_pn, LearningSource::Usync)
            .await
            .unwrap();

        let (enc_pn, iv_pn) = poll::encrypt_poll_vote_with_secret(
            &[poll::compute_option_hash("Yes").to_vec()],
            &secret,
            stanza_id,
            &Jid::pn(creator_pn).to_string(),
            &Jid::pn(voter_pn).to_string(),
        )
        .unwrap();
        // Empty selection = clear, authored under the LID pair after migrating.
        let (enc_clear, iv_clear) = poll::encrypt_poll_vote_with_secret(
            &[],
            &secret,
            stanza_id,
            &Jid::lid(creator_lid).to_string(),
            &Jid::lid(voter_lid).to_string(),
        )
        .unwrap();

        let voter_pn_jid = Jid::pn(voter_pn);
        let voter_lid_jid = Jid::lid(voter_lid);
        let votes: Vec<(&Jid, &[u8], &[u8])> = vec![
            (&voter_pn_jid, &enc_pn, &iv_pn),
            (&voter_lid_jid, &enc_clear, &iv_clear),
        ];

        let results = client
            .polls()
            .aggregate_votes(&options, &votes, &secret, stanza_id, &Jid::lid(creator_lid))
            .await
            .unwrap();

        assert!(
            results.iter().all(|r| r.voters.is_empty()),
            "the LID clear-vote must remove the earlier PN 'Yes'"
        );
    }
}
