//! Handler for incoming `<chatstate>` stanzas (typing indicators).

use super::traits::StanzaHandler;
use crate::client::Client;
use async_trait::async_trait;
use log::debug;
use std::sync::Arc;
use wacore::iq::chatstate::{
    ChatstateParseError, ChatstateSource, ChatstateStanza, ReceivedChatState,
};
use wacore::stanza::wire_tags::StanzaTag;
use wacore_binary::Jid;

/// Event for incoming chatstate (`<chatstate/>`) stanzas.
///
/// Contains the chat JID, optional participant (for groups), and the parsed state.
/// State values align with WhatsApp Web's `WAChatState` constants.
#[derive(Debug, Clone)]
pub struct ChatStateEvent {
    /// The chat where the event occurred (user JID for 1:1, group JID for groups)
    pub chat: Jid,
    /// For group chats, the participant who triggered the event
    pub participant: Option<Jid>,
    /// The chat state (typing, recording_audio, or idle)
    pub state: ReceivedChatState,
}

impl ChatStateEvent {
    /// Convert the bus payload back to the compatibility chatstate view.
    pub fn from_presence(presence: &wacore::types::events::ChatPresenceUpdate) -> Self {
        use wacore::types::presence::{ChatPresence, ChatPresenceMedia};
        let state = match (presence.state, presence.media) {
            (ChatPresence::Composing, ChatPresenceMedia::Audio) => {
                ReceivedChatState::RecordingAudio
            }
            (ChatPresence::Composing, _) => ReceivedChatState::Typing,
            _ => ReceivedChatState::Idle,
        };
        Self {
            chat: presence.source.chat.clone(),
            participant: presence
                .source
                .is_group
                .then(|| presence.source.sender.clone()),
            state,
        }
    }

    /// Create a `ChatStateEvent` from a parsed `ChatstateStanza`.
    pub fn from_stanza(stanza: ChatstateStanza) -> Self {
        let (chat, participant) = match stanza.source {
            ChatstateSource::User { from } => (from, None),
            ChatstateSource::Group { from, participant } => (from, Some(participant)),
        };
        Self {
            chat,
            participant,
            state: stanza.state,
        }
    }
}

pub(crate) struct ChatstateRegistration {
    pub(crate) callback: Arc<crate::bot::CallbackEventHandler>,
    pub(crate) count: Arc<std::sync::atomic::AtomicUsize>,
}

impl wacore::types::events::EventHandler for ChatstateRegistration {
    fn handle_event(&self, event: Arc<wacore::types::events::Event>) {
        self.callback.handle_event(event);
    }

    fn interest(&self) -> wacore::types::events::EventInterest {
        self.callback.interest()
    }
}

impl Drop for ChatstateRegistration {
    fn drop(&mut self) {
        self.count
            .fetch_sub(1, std::sync::atomic::Ordering::Relaxed);
    }
}

/// Handler for `<chatstate>` stanzas.
///
/// Parses incoming chatstate stanzas using the `ProtocolNode` pattern
/// and dispatches events to registered handlers.
#[derive(Default)]
pub struct ChatstateHandler;

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl StanzaHandler for ChatstateHandler {
    fn tag(&self) -> &'static str {
        StanzaTag::ChatState.as_str()
    }

    #[cfg_attr(
        feature = "tracing",
        tracing::instrument(name = "wa.recv.chatstate", level = "debug", skip_all)
    )]
    async fn handle(
        &self,
        client: Arc<Client>,
        node: Arc<wacore_binary::OwnedNodeRef>,
        _cancelled: &mut bool,
    ) -> bool {
        match ChatstateStanza::parse(node.get()) {
            Ok(stanza) => {
                debug!(
                    target: "ChatstateHandler",
                    "Received chatstate: {:?} from {:?}",
                    stanza.state,
                    stanza.source
                );
                client.dispatch_chatstate_event(stanza).await;
            }
            Err(ChatstateParseError::SelfEcho) => {
                debug!(
                    target: "ChatstateHandler",
                    "Ignoring self-echo chatstate"
                );
            }
            Err(e) => {
                log::warn!(
                    target: "ChatstateHandler",
                    "Failed to parse chatstate stanza: {e}"
                );
            }
        }
        true
    }
}
