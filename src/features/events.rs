//! Event creation and response (RSVP).

use anyhow::{Result, anyhow};
use wacore::event;
use wacore_binary::{Jid, JidExt};
use waproto::whatsapp as wa;

pub use waproto::whatsapp::message::event_response_message::EventResponseType;

use crate::client::Client;
use crate::send::SendResult;

/// Parameters for creating an event message. Only `name` is required.
#[derive(Debug, Clone, Default)]
pub struct EventCreationParams {
    pub name: String,
    pub description: Option<String>,
    pub start_time: Option<i64>,
    pub end_time: Option<i64>,
    pub join_link: Option<String>,
    pub location: Option<wa::message::LocationMessage>,
    pub is_scheduled_call: Option<bool>,
    pub extra_guests_allowed: Option<bool>,
}

pub struct Events<'a> {
    client: &'a Client,
}

impl<'a> Events<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Create an event. Returns the `message_secret` the creator needs to decrypt
    /// later responses (RSVPs) via [`wacore::event::decrypt_event_response_with_secret`].
    pub async fn create(
        &self,
        to: impl Into<Jid>,
        params: EventCreationParams,
    ) -> Result<(SendResult, Vec<u8>)> {
        let to = &to.into();
        if params.name.trim().is_empty() {
            return Err(anyhow!("Event name must not be empty"));
        }

        let mut message = wa::Message {
            event_message: Some(Box::new(build_event_message(params))),
            ..Default::default()
        };

        // Events carry a per-message secret (like polls); responders derive their
        // RSVP encryption key from it. WA Web rejects an event without one
        // (Events/ValidationError MISSING_MESSAGE_SECRET).
        let message_secret: Vec<u8> = {
            use rand::Rng;
            let mut secret = vec![0u8; 32];
            rand::make_rng::<rand::rngs::StdRng>().fill_bytes(&mut secret);
            secret
        };
        message.message_context_info = Some(Box::new(wa::MessageContextInfo {
            message_secret: Some(message_secret.clone()),
            ..Default::default()
        }));

        let result = self.client.send_message(to, message).await?;
        Ok((result, message_secret))
    }

    /// RSVP to an event. `message_secret` is the event's secret (from its creation
    /// message); `event_creator_jid` is who created the event.
    pub async fn respond(
        &self,
        chat_jid: impl Into<Jid>,
        event_msg_id: &str,
        event_creator_jid: &Jid,
        message_secret: &[u8],
        response: EventResponseType,
        extra_guest_count: Option<i32>,
    ) -> Result<SendResult> {
        let chat_jid = &chat_jid.into();
        let my_jid = self
            .client
            .get_pn()
            .ok_or_else(|| anyhow!("Not logged in — cannot determine own JID"))?;
        let my_base = my_jid.to_non_ad();

        let responder = self
            .resolve_responder_jid(event_creator_jid, &my_base)
            .await;
        let responder_str = responder.to_string();
        let creator_str = event_creator_jid.to_non_ad_string();

        let response_msg = wa::message::EventResponseMessage {
            response: Some(response as i32),
            timestamp_ms: Some(wacore::time::now_millis()),
            extra_guest_count,
        };

        let (enc_payload, iv) = event::encrypt_event_response_with_secret(
            &response_msg,
            message_secret,
            event_msg_id,
            &creator_str,
            &responder_str,
        )?;

        let from_me = my_base.is_same_user_as(event_creator_jid);
        let enc = wa::message::EncEventResponseMessage {
            event_creation_message_key: Some(wa::MessageKey {
                remote_jid: Some(chat_jid.to_string()),
                from_me: Some(from_me),
                id: Some(event_msg_id.to_string()),
                participant: if chat_jid.is_group() {
                    Some(event_creator_jid.to_string())
                } else {
                    None
                },
            }),
            enc_payload: Some(enc_payload),
            enc_iv: Some(iv.to_vec()),
        };

        let message = wa::Message {
            enc_event_response_message: Some(Box::new(enc)),
            ..Default::default()
        };

        self.client.send_message(chat_jid, message).await
    }

    /// The responder (self) JID keys the RSVP's HKDF/AAD, so it must use the event
    /// creator's namespace: own LID for a LID-addressed event, own PN otherwise,
    /// falling back to PN when our LID isn't known. Mirrors the poll-vote path.
    async fn resolve_responder_jid(&self, event_creator_jid: &Jid, own_pn: &Jid) -> Jid {
        if !event_creator_jid.is_lid() {
            return own_pn.clone();
        }
        match self.client.get_lid() {
            Some(lid) => lid.to_non_ad(),
            None => own_pn.clone(),
        }
    }
}

impl Client {
    pub fn events(&self) -> Events<'_> {
        Events::new(self)
    }
}

/// Build an `EventMessage` from the public params. Mirrors WA Web's
/// `GenerateEventCreationMessageProto` field set.
fn build_event_message(params: EventCreationParams) -> wa::message::EventMessage {
    wa::message::EventMessage {
        name: Some(params.name),
        description: params.description,
        start_time: params.start_time,
        end_time: params.end_time,
        join_link: params.join_link,
        location: params.location.map(Box::new),
        is_schedule_call: params.is_scheduled_call,
        extra_guests_allowed: params.extra_guests_allowed,
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::create_test_client;
    use std::sync::Arc;

    #[test]
    fn build_event_message_maps_fields() {
        let params = EventCreationParams {
            name: "Launch".into(),
            description: Some("desc".into()),
            start_time: Some(1_700_000_000),
            end_time: Some(1_700_003_600),
            join_link: Some("https://call".into()),
            is_scheduled_call: Some(true),
            extra_guests_allowed: Some(true),
            ..Default::default()
        };
        let msg = build_event_message(params);
        assert_eq!(msg.name.as_deref(), Some("Launch"));
        assert_eq!(msg.description.as_deref(), Some("desc"));
        assert_eq!(msg.start_time, Some(1_700_000_000));
        assert_eq!(msg.end_time, Some(1_700_003_600));
        assert_eq!(msg.join_link.as_deref(), Some("https://call"));
        assert_eq!(msg.is_schedule_call, Some(true));
        assert_eq!(msg.extra_guests_allowed, Some(true));
        assert!(msg.is_canceled.is_none());
    }

    #[tokio::test]
    async fn responder_is_pn_when_creator_is_pn() {
        let client: Arc<Client> = create_test_client().await;
        let own_pn = Jid::pn("5511999999999");
        let creator = Jid::pn("5511777777777");
        let responder = client
            .events()
            .resolve_responder_jid(&creator, &own_pn)
            .await;
        assert_eq!(responder, own_pn);
    }
}
