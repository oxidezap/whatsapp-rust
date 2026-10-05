//! Chat state (typing indicators) feature.

use crate::client::{Client, ClientError};
use log::debug;
use thiserror::Error;
pub use wacore::types::presence::ChatActivity;
use wacore_binary::Jid;
use wacore_binary::builder::NodeBuilder;

/// Error returned by chat-state (typing indicator) operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ChatStateError {
    /// Connection/transport failure sending the `<chatstate>` stanza.
    #[error("{0}")]
    Client(#[from] ClientError),
}

/// Feature handle for chat state operations.
pub struct Chatstate<'a> {
    client: &'a Client,
}

impl<'a> Chatstate<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Send a chat state update to a recipient.
    pub async fn send(&self, to: &Jid, state: ChatActivity) -> Result<(), ChatStateError> {
        debug!(target: "Chatstate", "Sending {:?} to {}", state, to);

        let node = self.build_chatstate_node(to, state);
        self.client.send_node(node).await?;
        Ok(())
    }

    pub async fn send_composing(&self, to: &Jid) -> Result<(), ChatStateError> {
        self.send(to, ChatActivity::Typing).await
    }

    pub async fn send_recording(&self, to: &Jid) -> Result<(), ChatStateError> {
        self.send(to, ChatActivity::RecordingAudio).await
    }

    pub async fn send_paused(&self, to: &Jid) -> Result<(), ChatStateError> {
        self.send(to, ChatActivity::Idle).await
    }

    fn build_chatstate_node(&self, to: &Jid, state: ChatActivity) -> wacore_binary::Node {
        NodeBuilder::new("chatstate")
            .attr("to", to)
            .children([state.into_child_node()])
            .build()
    }
}

impl Client {
    /// Access chat state operations.
    pub fn chatstate(&self) -> Chatstate<'_> {
        Chatstate::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn sends_semantic_activity_as_chatstate_payload() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let to: Jid = "12025550100@s.whatsapp.net".parse().unwrap();
        for activity in [
            ChatActivity::Typing,
            ChatActivity::RecordingAudio,
            ChatActivity::Idle,
        ] {
            client.chatstate().send(&to, activity).await.unwrap();
        }
        let frames = crate::test_utils::decrypt_wire_frames(&transport.sent(), &[0u8; 32]);
        assert_eq!(frames.len(), 3);
        for (frame, (tag, media)) in frames.iter().zip([
            ("composing", None),
            ("composing", Some("audio")),
            ("paused", None),
        ]) {
            let unpacked = wacore_binary::util::unpack(frame).unwrap();
            let owned = wacore_binary::OwnedNodeRef::new(unpacked.into_owned()).unwrap();
            let node = owned.get();
            assert_eq!(node.tag, "chatstate");
            assert_eq!(node.get_attr("to").unwrap().to_string(), to.to_string());
            let children = node.children().unwrap();
            assert_eq!(children.len(), 1);
            assert_eq!(children[0].tag, tag);
            assert_eq!(
                children[0].get_attr("media").map(|v| v.as_str()).as_deref(),
                media
            );
        }
    }
}
