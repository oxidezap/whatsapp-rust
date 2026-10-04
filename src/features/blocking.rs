//! Blocking feature for managing blocked contacts.
//!
//! This module provides high-level APIs for blocking and unblocking contacts.
//! Protocol-level types are defined in `wacore::iq::blocklist`.

use crate::client::Client;
use crate::request::IqError;
use log::debug;
use thiserror::Error;
pub use wacore::iq::blocklist::BlocklistEntry;
use wacore::iq::blocklist::{GetBlocklistSpec, UpdateBlocklistSpec};
use wacore_binary::Jid;

/// Error returned by blocklist operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum BlockingError {
    /// The IQ to the server failed (transport, timeout, server rejection).
    #[error("{0}")]
    Iq(#[from] IqError),
    /// The target JID is not a user JID, or has no resolvable LID↔PN mapping
    /// (modern WA requires both sides for a block).
    #[error("invalid blocklist target: {0}")]
    InvalidJid(String),
    /// Catch-all for internal failures (e.g. LID/PN store lookup).
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

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
    async fn resolve_lid_pn(&self, bare: Jid) -> Result<(Jid, Jid), BlockingError> {
        if !(bare.is_lid() || bare.is_pn()) {
            return Err(BlockingError::InvalidJid(
                "jid is neither PN nor LID".into(),
            ));
        }
        let entry = self.client.get_lid_pn_entry(&bare).await?.ok_or_else(|| {
            BlockingError::InvalidJid("no LID↔PN mapping for provided jid".into())
        })?;
        Ok(if bare.is_lid() {
            (bare, Jid::pn(&*entry.phone_number))
        } else {
            (Jid::lid(&*entry.lid), bare)
        })
    }

    /// Block a contact. Accepts either LID or PN; the wire stanza always
    /// carries both (`jid=LID, pn_jid=PN`) — modern WA rejects PN-only blocks.
    pub async fn block(&self, jid: &Jid) -> Result<(), BlockingError> {
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
    pub async fn unblock(&self, jid: &Jid) -> Result<(), BlockingError> {
        debug!(target: "Blocking", "Unblocking contact");
        // The unblock stanza only needs the LID, so a LID input must not require a
        // PN↔LID mapping (resolve_lid_pn hard-fails when none exists).
        let bare = jid.to_non_ad();
        let lid_jid = if bare.is_lid() {
            bare
        } else {
            self.resolve_lid_pn(bare).await?.0
        };
        self.client
            .execute(UpdateBlocklistSpec::unblock(&lid_jid))
            .await?;
        debug!(target: "Blocking", "Successfully unblocked contact");
        Ok(())
    }

    /// Get the full blocklist.
    pub async fn get_blocklist(&self) -> Result<Vec<BlocklistEntry>, BlockingError> {
        debug!(target: "Blocking", "Fetching blocklist...");
        let entries = self.client.execute(GetBlocklistSpec).await?;
        debug!(target: "Blocking", "Fetched {} blocked contacts", entries.len());
        Ok(entries)
    }

    /// Check if a contact is blocked.
    ///
    /// Ignores device IDs while preserving the PN/LID namespace. The other
    /// namespace is checked only when an explicit LID↔PN mapping is available.
    /// Non-PN/LID targets are rejected before sending an IQ; mapping lookup
    /// failures are returned rather than treated as an absent mapping.
    pub async fn is_blocked(&self, jid: &Jid) -> Result<bool, BlockingError> {
        let bare = jid.to_non_ad();
        if !(bare.is_lid() || bare.is_pn()) {
            return Err(BlockingError::InvalidJid(
                "jid is neither PN nor LID".into(),
            ));
        }
        let mapping = self.client.get_lid_pn_entry(&bare).await?;
        let alternative = mapping.map(|entry| {
            if bare.is_lid() {
                Jid::pn(&*entry.phone_number)
            } else {
                Jid::lid(&*entry.lid)
            }
        });
        let blocklist = self.get_blocklist().await?;
        Ok(blocklist.into_iter().any(|entry| {
            let entry = entry.jid.into_non_ad();
            entry == bare || alternative.as_ref() == Some(&entry)
        }))
    }
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
    use crate::lid_pn_cache::{LearningSource, LidPnEntry};
    use crate::test_utils::{answer_iq, create_iq_test_client, decode_sent_iq};
    use wacore_binary::builder::NodeBuilder;

    #[tokio::test]
    async fn invalid_query_rejected_before_iq() {
        let (client, transport) = create_iq_test_client().await;
        for target in ["15555550100@g.us", "status@broadcast", "123@newsletter"] {
            let jid = target.parse().unwrap();
            let result = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                client.blocking().is_blocked(&jid),
            )
            .await
            .expect("invalid targets must fail without waiting for an IQ response");
            assert!(matches!(result, Err(BlockingError::InvalidJid(_))));
        }
        assert!(transport.sent().is_empty());
    }

    #[tokio::test]
    async fn mapping_backend_failure_is_returned_before_iq() {
        let uri = format!(
            "file:blocking_mapping_failure_{}?mode=memory&cache=shared",
            uuid::Uuid::new_v4()
        );
        let backend = std::sync::Arc::new(crate::store::SqliteStore::open(&uri).await.unwrap());
        let (client, transport) =
            crate::test_utils::create_iq_test_client_with_backend(backend).await;
        tokio::task::spawn_blocking(move || {
            use diesel::{Connection, connection::SimpleConnection};
            let mut conn = diesel::SqliteConnection::establish(&uri).unwrap();
            conn.batch_execute("DROP TABLE lid_pn_mapping").unwrap();
        })
        .await
        .unwrap();

        for jid in [Jid::pn("15555550100"), Jid::lid("100000000000001")] {
            let error = tokio::time::timeout(
                std::time::Duration::from_secs(1),
                client.blocking().is_blocked(&jid),
            )
            .await
            .expect("lookup failure must return without waiting for an IQ")
            .unwrap_err();
            let BlockingError::Internal(cause) = error else {
                panic!("expected the mapping backend error, got {error:?}");
            };
            assert!(
                cause
                    .downcast_ref::<wacore::store::error::StoreError>()
                    .is_some()
            );
        }
        assert!(transport.sent().is_empty());
    }

    #[tokio::test]
    async fn blocklist_query_preserves_namespace_and_resolves_explicit_mapping() {
        let (client, transport) = create_iq_test_client().await;
        let pn = Jid::pn("15555550100");
        let lid = Jid::lid("100000000000001");
        let cases = [
            (pn.clone(), Jid::lid(&*pn.user), false),
            (lid.clone(), Jid::pn(&*lid.user), false),
            (
                pn.clone(),
                format!("{}@g.us", pn.user).parse().unwrap(),
                false,
            ),
            (pn.with_device(7), pn.with_device(2), true),
            (lid.with_device(3), lid.with_device(8), true),
            (pn.clone(), lid.clone(), false),
            (pn.with_device(4), lid.with_device(2), true),
            (lid.with_device(5), pn.with_device(3), true),
        ];
        for (index, (query, blocked, expected)) in cases.into_iter().enumerate() {
            if index == 6 {
                client
                    .lid_pn_cache
                    .add(&LidPnEntry::new(
                        lid.user.to_string(),
                        pn.user.to_string(),
                        LearningSource::Usync,
                    ))
                    .await;
            }
            let task = {
                let client = client.clone();
                tokio::spawn(async move { client.blocking().is_blocked(&query).await })
            };
            let request = decode_sent_iq(&transport, index).await;
            let id = request
                .get()
                .attrs()
                .optional_string("id")
                .unwrap()
                .into_owned();
            let response = NodeBuilder::new("iq")
                .attr("id", id.as_str())
                .attr("type", "result")
                .children([NodeBuilder::new("list")
                    .children([NodeBuilder::new("item").attr("jid", blocked).build()])
                    .build()])
                .build();
            answer_iq(&client, &id, &response).await;
            assert_eq!(task.await.unwrap().unwrap(), expected, "case {index}");
        }
        assert_eq!(transport.sent().len(), 8);
    }
}
