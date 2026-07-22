use wacore::WireEnum;
use wacore_binary::Jid;
use waproto::whatsapp as wa;

use crate::cache::Freshness;
use crate::client::Client;
use crate::send::{SendError, SendResult};
use crate::upload::UploadResponse;
use wacore_binary::Node;

/// Privacy setting sent in the `<meta>` node of the status stanza.
/// Matches WhatsApp Web's `status_setting` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum StatusPrivacySetting {
    /// Send to all contacts in address book.
    #[wire_default]
    #[wire = "contacts"]
    Contacts,
    /// Send only to contacts in an allow list.
    #[wire = "allowlist"]
    AllowList,
    /// Send to all contacts except those in a deny list.
    #[wire = "denylist"]
    DenyList,
}

/// Options for sending a status update.
#[derive(Debug, Clone, Default)]
pub struct StatusSendOptions {
    /// Privacy setting for this status. Sent in the `<meta>` stanza node.
    pub privacy: StatusPrivacySetting,
    /// Override the generated message ID.
    pub message_id: Option<String>,
    /// Extra child nodes appended to the status stanza.
    pub extra_stanza_nodes: Vec<Node>,
    /// Freshness policy for the recipient device lists used by this send.
    pub device_freshness: Freshness,
}

/// High-level API for WhatsApp status/story updates.
pub struct Status<'a> {
    client: &'a Client,
}

impl<'a> Status<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Send a text status update to the given recipients.
    ///
    /// `background_argb` is the background color as 0xAARRGGBB (e.g., `0xFF1E6E4F`).
    /// `font` selects the status font; values outside the protocol enum can't be
    /// passed (the prior `i32` form silently dropped them at encode time).
    pub async fn send_text(
        &self,
        text: &str,
        background_argb: u32,
        font: wa::message::extended_text_message::FontType,
        recipients: &[Jid],
        options: StatusSendOptions,
    ) -> Result<SendResult, SendError> {
        let message = wa::Message {
            extended_text_message: buffa::MessageField::some(wa::message::ExtendedTextMessage {
                text: Some(text.to_string()),
                background_argb: Some(background_argb),
                font: Some(font),
                ..Default::default()
            }),
            ..Default::default()
        };

        self.client
            .send_status_message(message, recipients, options)
            .await
    }

    /// Send an image status update.
    ///
    /// The caller must upload the media first via `client.upload()` and provide
    /// the `UploadResponse`, JPEG thumbnail bytes, and optional caption.
    pub async fn send_image(
        &self,
        upload: UploadResponse,
        thumbnail: Vec<u8>,
        caption: Option<&str>,
        recipients: &[Jid],
        options: StatusSendOptions,
    ) -> Result<SendResult, SendError> {
        let message = crate::media::image_message(
            upload,
            crate::media::ImageOptions {
                caption: caption.map(|c| c.to_string()),
                jpeg_thumbnail: Some(thumbnail),
                ..Default::default()
            },
        );

        self.client
            .send_status_message(message, recipients, options)
            .await
    }

    /// Send a video status update.
    ///
    /// The caller must upload the media first via `client.upload()` and provide
    /// the `UploadResponse`, JPEG thumbnail bytes, duration in seconds, and optional caption.
    pub async fn send_video(
        &self,
        upload: UploadResponse,
        thumbnail: Vec<u8>,
        duration_seconds: u32,
        caption: Option<&str>,
        recipients: &[Jid],
        options: StatusSendOptions,
    ) -> Result<SendResult, SendError> {
        let message = crate::media::video_message(
            upload,
            crate::media::VideoOptions {
                caption: caption.map(|c| c.to_string()),
                jpeg_thumbnail: Some(thumbnail),
                duration_seconds: Some(duration_seconds),
                ..Default::default()
            },
        );

        self.client
            .send_status_message(message, recipients, options)
            .await
    }

    /// Send a raw `wa::Message` as a status update.
    ///
    /// Use this for message types not covered by the convenience methods above.
    pub async fn send_raw(
        &self,
        message: wa::Message,
        recipients: &[Jid],
        options: StatusSendOptions,
    ) -> Result<SendResult, SendError> {
        self.client
            .send_status_message(message, recipients, options)
            .await
    }

    /// Delete (revoke) a previously sent status update.
    ///
    /// `recipients` should be the same list used when posting the status,
    /// since the revoke must be encrypted to the same set of devices.
    pub async fn revoke(
        &self,
        message_id: impl Into<String>,
        recipients: &[Jid],
        options: StatusSendOptions,
    ) -> Result<SendResult, SendError> {
        let message_id = message_id.into();
        let to = Jid::status_broadcast();

        let revoke_message = wa::Message {
            protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
                key: buffa::MessageField::some(wa::MessageKey {
                    remote_jid: Some(to.to_string()),
                    from_me: Some(true),
                    id: Some(message_id),
                    ..Default::default()
                }),
                r#type: Some(wa::message::protocol_message::Type::REVOKE),
                ..Default::default()
            }),
            ..Default::default()
        };

        self.client
            .send_status_message(revoke_message, recipients, options)
            .await
    }
}

impl Client {
    /// Access the status/story API for posting, revoking, and managing status updates.
    ///
    /// # Example
    /// ```no_run
    /// # async fn example(client: &whatsapp_rust::Client) -> anyhow::Result<()> {
    /// use waproto::whatsapp::message::extended_text_message::FontType;
    /// let recipients = [whatsapp_rust::Jid::pn("15551234567")];
    /// let id = client
    ///     .status()
    ///     .send_text("Hello!", 0xFF1E6E4F, FontType::SYSTEM, &recipients, Default::default())
    ///     .await?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn status(&self) -> Status<'_> {
        Status::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_privacy_setting_values() {
        // Verify the string values match WhatsApp Web's status_setting attribute
        assert_eq!(StatusPrivacySetting::Contacts.as_str(), "contacts");
        assert_eq!(StatusPrivacySetting::AllowList.as_str(), "allowlist");
        assert_eq!(StatusPrivacySetting::DenyList.as_str(), "denylist");
    }

    #[test]
    fn test_status_privacy_default_is_contacts() {
        let default = StatusPrivacySetting::default();
        assert_eq!(default.as_str(), "contacts");
    }

    #[test]
    fn test_status_send_options_default() {
        let opts = StatusSendOptions::default();
        assert_eq!(opts.privacy.as_str(), "contacts");
    }

    #[test]
    fn test_status_text_message_structure() {
        // Verify the message structure matches WhatsApp Web's extendedTextMessage format
        use waproto::whatsapp::message::extended_text_message::FontType;
        let text = "Hello from Rust!";
        let bg = 0xFF1E6E4F_u32;
        let font = FontType::FB_SCRIPT;

        let message = waproto::whatsapp::Message {
            extended_text_message: buffa::MessageField::some(
                waproto::whatsapp::message::ExtendedTextMessage {
                    text: Some(text.to_string()),
                    background_argb: Some(bg),
                    font: Some(font),
                    ..Default::default()
                },
            ),
            ..Default::default()
        };

        let ext = message.extended_text_message.as_option().unwrap();
        assert_eq!(ext.text.as_deref(), Some(text));
        assert_eq!(ext.background_argb, Some(bg));
        assert_eq!(ext.font, Some(font));
    }

    #[test]
    fn test_status_revoke_message_structure() {
        use waproto::whatsapp as wa;

        let original_id = "3EB06D00CAB92340790621";
        let to = Jid::status_broadcast();

        let revoke_message = wa::Message {
            protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
                key: wa::MessageKey {
                    remote_jid: Some(to.to_string()),
                    from_me: Some(true),
                    id: Some(original_id.to_string()),
                    ..Default::default()
                }
                .into(),
                r#type: Some(wa::message::protocol_message::Type::REVOKE),
                ..Default::default()
            }),
            ..Default::default()
        };

        let pm = revoke_message.protocol_message.as_option().unwrap();
        assert_eq!(pm.r#type, Some(wa::message::protocol_message::Type::REVOKE));
        let key = pm.key.as_option().unwrap();
        assert_eq!(key.remote_jid.as_deref(), Some("status@broadcast"));
        assert_eq!(key.from_me, Some(true));
        assert_eq!(key.id.as_deref(), Some(original_id));
    }

    #[test]
    fn test_revoke_is_detected_as_revoke() {
        use waproto::whatsapp as wa;

        // Non-revoke message
        let text_msg = wa::Message {
            extended_text_message: buffa::MessageField::some(wa::message::ExtendedTextMessage {
                text: Some("hello".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let is_revoke = text_msg
            .protocol_message
            .as_option()
            .is_some_and(|pm| pm.r#type == Some(wa::message::protocol_message::Type::REVOKE));
        assert!(!is_revoke, "text message should not be detected as revoke");

        // Revoke message
        let revoke_msg = wa::Message {
            protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
                r#type: Some(wa::message::protocol_message::Type::REVOKE),
                ..Default::default()
            }),
            ..Default::default()
        };
        let is_revoke = revoke_msg
            .protocol_message
            .as_option()
            .is_some_and(|pm| pm.r#type == Some(wa::message::protocol_message::Type::REVOKE));
        assert!(is_revoke, "revoke message should be detected as revoke");
    }
}
