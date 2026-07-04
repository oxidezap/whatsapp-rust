//! Contact information feature.
//!
//! Profile picture types are defined in `wacore::iq::contacts`.
//! Usync types are defined in `wacore::iq::usync`.

use crate::client::Client;
use crate::request::IqError;
use log::debug;
use std::collections::HashMap;
use thiserror::Error;
use wacore::iq::contacts::{ProfilePictureSpec, ProfilePictureType};
use wacore::iq::usync::{IsOnWhatsAppQueryType, IsOnWhatsAppSpec, IsOnWhatsAppUser, UserInfoSpec};
use wacore_binary::{Jid, JidExt};

// Re-export types from wacore
pub use wacore::iq::contacts::ProfilePicture;
pub use wacore::iq::usync::{IsOnWhatsAppResult, UserInfo, UsyncSubprotocolError};
pub use wacore::stanza::business::VerifiedName;

/// Error returned by contact-information operations (existence checks,
/// profile pictures, user info).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ContactError {
    /// The usync/profile IQ to the server failed.
    #[error(transparent)]
    Iq(#[from] IqError),
    /// An input JID is not supported for this query (only PN and LID are).
    #[error("unsupported contact JID: {0}")]
    InvalidJid(String),
}

fn ensure_is_on_whatsapp_jids_supported(jids: &[Jid]) -> Result<(), ContactError> {
    if let Some(jid) = jids.iter().find(|jid| !jid.is_pn() && !jid.is_lid()) {
        return Err(ContactError::InvalidJid(format!(
            "is_on_whatsapp only supports PN and LID JIDs, got {jid}"
        )));
    }
    Ok(())
}

/// Mapping extractors as fn items, NOT closures. A closure returning
/// references tied to its argument is inferred at a concrete lifetime, and
/// because its type is embedded in the public methods' future types, callers
/// that box those futures (`#[async_trait]`, `Box<dyn Future + Send>`) hit
/// "implementation of `FnOnce` is not general enough" (issue #825). Fn items
/// implement `Fn` for every lifetime by construction.
/// PN-primary result -> (PN, LID mapping).
fn forward_lid_pair(r: &IsOnWhatsAppResult) -> (&Jid, Option<&Jid>) {
    (&r.jid, r.lid.as_ref())
}

/// LID-primary result inverted to (PN, LID); None when not LID-primary.
fn reverse_lid_pair(r: &IsOnWhatsAppResult) -> Option<(&Jid, Option<&Jid>)> {
    if r.jid.is_lid() {
        r.pn_jid.as_ref().map(|pn| (pn, Some(&r.jid)))
    } else {
        None
    }
}

/// UserInfo entry -> (queried JID, LID mapping).
fn user_info_lid_pair(entry: &UserInfo) -> (&Jid, Option<&Jid>) {
    (&entry.jid, entry.lid.as_ref())
}

pub struct Contacts<'a> {
    client: &'a Client,
}

impl<'a> Contacts<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Callers must pass `fn` items (e.g. [`forward_lid_pair`]), NOT
    /// closures: a closure returning borrowed pairs embeds a non-HRTB type in
    /// the public caller's future and breaks `#[async_trait]` consumers
    /// (issue #825, guarded by tests/async_trait_boxed_future_compat.rs).
    async fn persist_lid_mappings<'b, I>(&self, entries: I)
    where
        I: IntoIterator<Item = (&'b Jid, Option<&'b Jid>)>,
    {
        for (jid, lid) in entries {
            let Some(lid) = lid else {
                continue;
            };
            if !jid.is_pn() || !lid.is_lid() {
                continue;
            }
            if let Err(err) = self
                .client
                .add_lid_pn_mapping(
                    &lid.user,
                    &jid.user,
                    crate::lid_pn_cache::LearningSource::Usync,
                )
                .await
            {
                log::warn!(
                    "Failed to persist usync LID mapping {} -> {}: {err}",
                    jid,
                    lid
                );
            }
        }
    }

    /// Check if JIDs are registered on WhatsApp.
    ///
    /// Accepts both PN JIDs (`Jid::pn("1234567890")`) and LID JIDs (`Jid::lid("100000001")`).
    /// PN and LID queries use different protocols (matching WA Web ExistsJob), so mixed
    /// inputs are split into separate requests.
    pub async fn is_on_whatsapp(
        &self,
        jids: &[Jid],
    ) -> Result<Vec<IsOnWhatsAppResult>, ContactError> {
        if jids.is_empty() {
            return Ok(Vec::new());
        }
        ensure_is_on_whatsapp_jids_supported(jids)?;

        debug!("is_on_whatsapp: checking {} JIDs", jids.len());

        let mut pn_users = Vec::new();
        let mut lid_users = Vec::new();
        for jid in jids {
            if jid.is_pn() {
                let known_lid = self.client.lid_pn_cache.get_current_lid(&jid.user).await;
                pn_users.push(IsOnWhatsAppUser {
                    jid: jid.to_non_ad(),
                    known_lid,
                });
            } else if jid.is_lid() {
                lid_users.push(IsOnWhatsAppUser {
                    jid: jid.to_non_ad(),
                    known_lid: None,
                });
            } else {
                #[cfg(debug_assertions)]
                panic!("is_on_whatsapp: unexpected JID type {jid} after validation");

                #[cfg(not(debug_assertions))]
                continue;
            }
        }

        // PN and LID existence use different protocols (two independent IQs), so
        // when a mixed input produces both, run them concurrently.
        let pn_fut = async {
            if pn_users.is_empty() {
                Ok(Vec::new())
            } else {
                let sid = self.client.generate_request_id();
                self.client
                    .execute(IsOnWhatsAppSpec::new(
                        pn_users,
                        sid,
                        IsOnWhatsAppQueryType::Pn,
                    ))
                    .await
            }
        };
        let lid_fut = async {
            if lid_users.is_empty() {
                Ok(Vec::new())
            } else {
                let sid = self.client.generate_request_id();
                self.client
                    .execute(IsOnWhatsAppSpec::new(
                        lid_users,
                        sid,
                        IsOnWhatsAppQueryType::Lid,
                    ))
                    .await
            }
        };
        // try_join! fails fast: it returns the instant either query errors and
        // drops the sibling in-flight future. That's now safe — `send_and_wait_iq`
        // registers a `ResponseWaiterGuard` that removes the waiter on drop, so a
        // cancelled sibling can't leak its `response_waiters` entry (which would
        // otherwise suppress keepalives). The old sequential code also failed on
        // the first error, so fail-fast matches the original latency profile.
        let (mut results, lid_results) = futures::try_join!(pn_fut, lid_fut)?;
        results.extend(lid_results);

        self.persist_lid_mappings(results.iter().map(forward_lid_pair))
            .await;
        self.persist_lid_mappings(results.iter().filter_map(reverse_lid_pair))
            .await;

        Ok(results)
    }

    pub async fn get_profile_picture(
        &self,
        jid: &Jid,
        preview: bool,
    ) -> Result<Option<ProfilePicture>, ContactError> {
        debug!(
            "get_profile_picture: fetching {} picture for {}",
            if preview { "preview" } else { "full" },
            jid
        );

        let picture_type = if preview {
            ProfilePictureType::Preview
        } else {
            ProfilePictureType::Full
        };
        let mut spec = ProfilePictureSpec::new(jid, picture_type);

        // Skip own JID: server never responds when tctoken is sent for self
        let is_own_jid = {
            let snap = self.client.persistence_manager.get_device_snapshot();
            snap.pn.as_ref().is_some_and(|pn| pn.is_same_user_as(jid))
                || snap
                    .lid
                    .as_ref()
                    .is_some_and(|lid| lid.is_same_user_as(jid))
        };
        if !jid.is_group()
            && !jid.is_newsletter()
            && !jid.is_bot()
            && !jid.is_broadcast_list()
            && !jid.is_status_broadcast()
            && !is_own_jid
            && self
                .client
                .ab_props
                .is_enabled(wacore::iq::props::stale::PROFILE_PIC_PRIVACY_TOKEN)
                .await
            && let Some(token) = self.client.lookup_tc_token_for_jid(jid).await
        {
            spec = spec.with_tc_token(token);
        }

        match self.client.execute(spec).await {
            Ok(pic) => Ok(pic),
            // 404/401 = no profile picture (or not authorized to see it).
            // WhatsApp server returns type="error" IQ for these cases.
            Err(IqError::ServerError { code, .. }) if code == 404 || code == 401 => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    pub async fn get_user_info(
        &self,
        jids: &[Jid],
    ) -> Result<HashMap<Jid, UserInfo>, ContactError> {
        if jids.is_empty() {
            return Ok(HashMap::new());
        }

        debug!("get_user_info: fetching info for {} JIDs", jids.len());

        let request_id = self.client.generate_request_id();
        let mut spec = UserInfoSpec::new(jids.to_vec(), request_id);

        // Attach per-user tctokens so the status/about of privacy-restricted
        // contacts is returned, matching WA Web's USyncStatusProtocol.getUserElement.
        if self
            .client
            .ab_props()
            .is_enabled(wacore::iq::abprops::web::PROFILE_SCRAPING_PRIVACY_TOKEN_IN_ABOUT_USYNC)
            .await
        {
            let lookups = futures::future::join_all(jids.iter().map(|jid| async move {
                (
                    jid.to_non_ad().to_string(),
                    self.client.lookup_tc_token_for_jid(jid).await,
                )
            }))
            .await;
            let tc_tokens: HashMap<String, Vec<u8>> = lookups
                .into_iter()
                .filter_map(|(key, token)| token.map(|t| (key, t)))
                .collect();
            if !tc_tokens.is_empty() {
                spec = spec.with_tc_tokens(tc_tokens);
            }
        }

        let info = self.client.execute(spec).await?;
        self.persist_lid_mappings(info.values().map(user_info_lid_pair))
            .await;
        Ok(info)
    }
}

impl Client {
    pub fn contacts(&self) -> Contacts<'_> {
        Contacts::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_picture_struct() {
        let pic = ProfilePicture {
            id: "123456789".to_string(),
            url: "https://example.com/pic.jpg".to_string(),
            direct_path: Some("/v/pic.jpg".to_string()),
            hash: None,
        };

        assert_eq!(pic.id, "123456789");
        assert_eq!(pic.url, "https://example.com/pic.jpg");
        assert!(pic.direct_path.is_some());
    }

    #[test]
    fn is_on_whatsapp_accepts_pn_and_lid_jids() {
        ensure_is_on_whatsapp_jids_supported(&[Jid::pn("15550000001"), Jid::lid("100000001")])
            .unwrap();
    }

    #[test]
    fn is_on_whatsapp_rejects_unsupported_jid_type() {
        let err = ensure_is_on_whatsapp_jids_supported(&[Jid::group("15550000001-1234567890")])
            .unwrap_err();

        assert!(err.to_string().contains("only supports PN and LID JIDs"));
    }
}
