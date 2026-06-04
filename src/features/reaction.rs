//! Sending reactions to DM/group/status messages.
//!
//! Newsletter reactions go through a different (plaintext) wire path; use
//! [`Client::newsletter`]'s `send_reaction` for channels.

use wacore_binary::Jid;
use waproto::whatsapp as wa;

use crate::client::Client;
use crate::send::SendResult;

impl Client {
    /// React to a DM, group, or status@broadcast message.
    ///
    /// `target_key` references the message being reacted to. For groups and
    /// status it must carry `participant` (the original sender) so the receipt
    /// can be attributed; [`crate::bot::MessageContext::react`] fills this in
    /// from the incoming message. An empty `emoji` removes a previous reaction
    /// (WA Web's empty-text reaction == sender-revoke).
    ///
    /// status@broadcast reactions fan out to the status author's devices; the
    /// author is read from `target_key.participant` by the send path.
    pub async fn send_reaction(
        &self,
        chat: &Jid,
        target_key: wa::MessageKey,
        emoji: &str,
    ) -> Result<SendResult, anyhow::Error> {
        let reaction = wacore::proto_helpers::build_reaction_message(
            target_key,
            emoji,
            wacore::time::now_millis(),
        );
        self.send_message(chat.clone(), reaction).await
    }
}
