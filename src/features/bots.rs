//! Bot directory feature: the catalogue of first-party AI bots the server
//! offers this account.
//!
//! Not to be confused with [`crate::bot`], which is the framework for building
//! a client that answers messages. Protocol-level types live in
//! `wacore::iq::bot`.

use crate::client::Client;
use crate::request::IqError;
use log::debug;
use wacore::iq::bot::GetBotListSpec;
pub use wacore::iq::bot::{
    BotDefault, BotList, BotListEntry, BotListSection, BotListVersion, BotSectionDisplayType,
    BotSectionType, BotTheme, BotThemeMode,
};

/// Feature handle for bot directory operations.
pub struct Bots<'a> {
    client: &'a Client,
}

impl<'a> Bots<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Fetch the bot directory.
    ///
    /// WhatsApp Web issues this once per session at startup and refreshes it on
    /// a `bonsai_update_interval` (24h) timer, then feeds the returned persona
    /// ids to the bot-profile MEX query. There is no server push for it, so a
    /// caller that wants fresh data calls this again.
    ///
    /// Every section is returned. Presentation metadata (`type`, `display_type`,
    /// themes) is preserved rather than collapsed, because a bot can be carried
    /// by a `category` or `featured` section and by no other.
    pub async fn list(&self) -> Result<BotList, IqError> {
        debug!(target: "Bots", "Requesting bot directory");
        let list = self.client.execute(GetBotListSpec).await?;
        debug!(
            target: "Bots",
            "Bot directory returned {} sections",
            list.sections.len()
        );
        Ok(list)
    }
}

impl Client {
    #[inline]
    pub fn bots(&self) -> Bots<'_> {
        Bots::new(self)
    }
}
