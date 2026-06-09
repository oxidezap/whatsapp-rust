//! Blocking feature for managing blocked contacts.
//!
//! This module provides high-level APIs for blocking and unblocking contacts.
//! Protocol-level types are defined in `wacore::iq::blocklist`.

use crate::client::Client;
use crate::request::IqError;
use log::debug;
pub use wacore::iq::blocklist::BlocklistEntry;
use wacore::iq::blocklist::{GetBlocklistSpec, UpdateBlocklistSpec};
use wacore_binary::Jid;

/// Feature handle for blocklist operations.
pub struct Blocking<'a> {
    client: &'a Client,
}

impl<'a> Blocking<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Resolve `bare` (LID or PN) into the `(lid, pn)` pair the server expects
    /// on blocklist stanzas.
    async fn resolve_lid_pn(&self, bare: Jid) -> Result<(Jid, Jid), IqError> {
        if !(bare.is_lid() || bare.is_pn()) {
            return Err(IqError::EncodeError(anyhow::anyhow!(
                "blocklist: jid is neither PN nor LID"
            )));
        }
        let entry = self
            .client
            .get_lid_pn_entry(&bare)
            .await
            .map_err(IqError::EncodeError)?
            .ok_or_else(|| {
                IqError::EncodeError(anyhow::anyhow!(
                    "blocklist: no LID↔PN mapping for provided jid"
                ))
            })?;
        Ok(if bare.is_lid() {
            (bare, Jid::pn(&*entry.phone_number))
        } else {
            (Jid::lid(&*entry.lid), bare)
        })
    }

    /// Block a contact. Accepts either LID or PN; the wire stanza always
    /// carries both (`jid=LID, pn_jid=PN`) — modern WA rejects PN-only blocks.
    pub async fn block(&self, jid: &Jid) -> Result<(), IqError> {
        debug!(target: "Blocking", "Blocking contact");
        let (lid_jid, pn_jid) = self.resolve_lid_pn(jid.to_non_ad()).await?;
        self.client
            .execute(UpdateBlocklistSpec::block_with_pn(&lid_jid, &pn_jid))
            .await?;
        debug!(target: "Blocking", "Successfully blocked contact");
        Ok(())
    }

    /// Unblock a contact. Stanza only needs the LID, but PN input is accepted
    /// and resolved through the mapping.
    pub async fn unblock(&self, jid: &Jid) -> Result<(), IqError> {
        debug!(target: "Blocking", "Unblocking contact");
        let (lid_jid, _) = self.resolve_lid_pn(jid.to_non_ad()).await?;
        self.client
            .execute(UpdateBlocklistSpec::unblock(&lid_jid))
            .await?;
        debug!(target: "Blocking", "Successfully unblocked contact");
        Ok(())
    }

    /// Get the full blocklist.
    pub async fn get_blocklist(&self) -> anyhow::Result<Vec<BlocklistEntry>> {
        debug!(target: "Blocking", "Fetching blocklist...");
        let entries = self.client.execute(GetBlocklistSpec).await?;
        debug!(target: "Blocking", "Fetched {} blocked contacts", entries.len());
        Ok(entries)
    }

    /// Check if a contact is blocked.
    ///
    /// Compares only the user part of the JID, ignoring device ID, since blocking
    /// applies to the entire user account, not individual devices.
    pub async fn is_blocked(&self, jid: &Jid) -> anyhow::Result<bool> {
        let blocklist = self.get_blocklist().await?;
        let bare = jid.to_non_ad();

        // Blocks are stored keyed by LID (block() always resolves the input to a LID), so a
        // PN-input query must resolve to its LID before comparing or it never matches a
        // LID-keyed entry. Match against the raw user plus the resolved LID and PN. Propagate a
        // backend failure (swallowing it would fall back to the raw user and re-introduce the
        // false negative); a genuine absence (Ok(None), incl. a non-LID/PN input) falls back.
        let mapping = self.client.get_lid_pn_entry(&bare).await?;
        let mut users: Vec<&str> = vec![bare.user.as_str()];
        if let Some(entry) = mapping.as_ref() {
            users.push(&*entry.lid);
            users.push(&*entry.phone_number);
        }

        Ok(blocklist_contains(&blocklist, &users))
    }
}

/// Whether any blocklist entry's user part matches one of `candidate_users`.
///
/// Blocks are stored keyed by LID, so the caller resolves the queried JID to its
/// LID/PN pair and passes all of them (raw, LID, PN) to catch a LID-keyed entry from
/// a PN-input query (and the reverse).
fn blocklist_contains(blocklist: &[BlocklistEntry], candidate_users: &[&str]) -> bool {
    blocklist
        .iter()
        .any(|e| candidate_users.contains(&e.jid.user.as_str()))
}

impl Client {
    /// Access blocking operations.
    pub fn blocking(&self) -> Blocking<'_> {
        Blocking::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lid_entry(user: &str) -> BlocklistEntry {
        BlocklistEntry {
            jid: Jid::lid(user.to_string()),
            timestamp: None,
        }
    }

    #[test]
    fn pn_query_matches_lid_keyed_block_only_when_resolved() {
        // A block stored under the LID (modern WA) must be found once the PN query is
        // resolved to that LID. Without resolution the LID-keyed block is missed (the bug).
        let blocklist = vec![lid_entry("100000012345678")];

        assert!(
            blocklist_contains(&blocklist, &["559980000001", "100000012345678"]),
            "resolved PN->LID candidate matches the LID-keyed block"
        );
        assert!(
            !blocklist_contains(&blocklist, &["559980000001"]),
            "raw PN alone misses the LID-keyed block (the false negative)"
        );
        assert!(
            blocklist_contains(&blocklist, &["100000012345678"]),
            "a LID query matches directly"
        );
        assert!(
            !blocklist_contains(&blocklist, &["559981111111", "100000099999999"]),
            "an unrelated contact is not blocked"
        );
    }
}
