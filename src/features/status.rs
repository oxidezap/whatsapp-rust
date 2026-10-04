use wacore::WireEnum;
use wacore_binary::Jid;
use waproto::whatsapp as wa;

use crate::cache::Freshness;
use crate::client::Client;
use crate::send::{SendError, SendResult};
use crate::upload::UploadResponse;
use crate::{MessageId, StanzaId};
use wacore_binary::Node;

/// Privacy setting sent in the `<meta>` node of the status stanza.
/// Matches WhatsApp Web's `status_setting` attribute.
#[derive(Debug, Clone, Copy, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum StatusPrivacySetting {
    /// Set `status_setting` to `contacts`.
    #[wire_default]
    #[wire = "contacts"]
    Contacts,
    /// Set `status_setting` to `allowlist`.
    #[wire = "allowlist"]
    AllowList,
    /// Set `status_setting` to `denylist`.
    #[wire = "denylist"]
    DenyList,
}

/// Options for sending a status update.
///
/// The privacy mode only sets the stanza's `status_setting`. It does not
/// filter `recipients`. Callers must check [`Status::audience`] and supply a
/// compatible recipient list themselves. If the audience is unknown, do not
/// infer that all contacts are allowed.
/// Start from the neutral [`Default`] and chain the `with_*` setters, as with
/// [`crate::SendOptions`].
/// Recipients are required separately by every send method; these options do
/// not infer an audience. Content and operation ID overrides are mutually
/// exclusive and must match the message being sent.
///
/// ```
/// use whatsapp_rust::{MessageId, StatusPrivacySetting, StatusSendOptions};
/// let options = StatusSendOptions::default()
///     .with_privacy(StatusPrivacySetting::AllowList)
///     .with_message_id(MessageId::new("STATUS-CONTENT")?);
/// # Ok::<(), whatsapp_rust::MessageRefError>(())
/// ```
#[derive(Debug, Clone, Default)]
#[non_exhaustive]
pub struct StatusSendOptions {
    /// Privacy setting for this status. Sent in the `<meta>` stanza node.
    pub privacy: StatusPrivacySetting,
    /// Override the generated content ID for a status post. Not valid for a
    /// revoke or reaction; use [`Self::stanza_id`] for those operations.
    pub message_id: Option<MessageId>,
    /// Override the outer operation ID for a revoke or raw reaction. Not valid
    /// for a content post. For revokes this must differ from the target ID.
    pub stanza_id: Option<StanzaId>,
    /// Extra child nodes appended to the status stanza.
    pub extra_stanza_nodes: Vec<Node>,
    /// Freshness policy for the recipient device lists used by this send.
    pub device_freshness: Freshness,
}

impl StatusSendOptions {
    /// See [`Self::privacy`].
    #[must_use]
    pub fn with_privacy(mut self, privacy: StatusPrivacySetting) -> Self {
        self.privacy = privacy;
        self
    }

    /// See [`Self::message_id`].
    #[must_use]
    pub fn with_message_id(mut self, message_id: MessageId) -> Self {
        self.message_id = Some(message_id);
        self
    }

    /// See [`Self::stanza_id`].
    #[must_use]
    pub fn with_stanza_id(mut self, stanza_id: StanzaId) -> Self {
        self.stanza_id = Some(stanza_id);
        self
    }

    /// See [`Self::extra_stanza_nodes`].
    #[must_use]
    pub fn with_extra_stanza_nodes(mut self, nodes: Vec<Node>) -> Self {
        self.extra_stanza_nodes = nodes;
        self
    }

    /// See [`Self::device_freshness`].
    #[must_use]
    pub fn with_device_freshness(mut self, freshness: Freshness) -> Self {
        self.device_freshness = freshness;
        self
    }
}

/// High-level API for WhatsApp status/story updates.
pub struct Status<'a> {
    client: &'a Client,
}

impl<'a> Status<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Last status audience synced from the phone. `None` means no audience
    /// has been observed yet, not that all contacts are allowed. The complete
    /// action includes custom lists and cross-posting settings. Unknown numeric
    /// modes remain present instead of decoding as absent. This is a snapshot,
    /// not a recipient calculation: sends still require an explicit recipient
    /// list and do not automatically apply these settings.
    pub fn audience(&self) -> Option<std::sync::Arc<wa::sync_action_value::StatusPrivacyAction>> {
        self.client
            .persistence_manager
            .get_device_snapshot()
            .status_privacy
            .clone()
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
    /// The target is a content [`MessageId`]; an optional outer operation ID
    /// belongs in [`StatusSendOptions::stanza_id`]. The returned
    /// [`SendResult::stanza_id`] identifies the revoke, not its target.
    pub async fn revoke(
        &self,
        message_id: MessageId,
        recipients: &[Jid],
        options: StatusSendOptions,
    ) -> Result<SendResult, SendError> {
        let to = Jid::status_broadcast();

        let revoke_message = wa::Message {
            protocol_message: buffa::MessageField::some(wa::message::ProtocolMessage {
                key: buffa::MessageField::some(wa::MessageKey {
                    remote_jid: Some(to.to_string()),
                    from_me: Some(true),
                    id: Some(message_id.into_string()),
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
    fn options_setters_preserve_all_fields() {
        let id = MessageId::new("STATUS-CONTENT").unwrap();
        let node = wacore_binary::builder::NodeBuilder::new("custom").build();
        let built = StatusSendOptions {
            privacy: StatusPrivacySetting::DenyList,
            message_id: Some(id.clone()),
            stanza_id: None,
            extra_stanza_nodes: vec![node.clone()],
            device_freshness: Freshness::Refresh,
        };
        let chained = StatusSendOptions::default()
            .with_privacy(StatusPrivacySetting::DenyList)
            .with_message_id(id)
            .with_extra_stanza_nodes(vec![node])
            .with_device_freshness(Freshness::Refresh);
        assert_eq!(built.privacy, chained.privacy);
        assert_eq!(built.message_id, chained.message_id);
        assert_eq!(built.extra_stanza_nodes, chained.extra_stanza_nodes);
        assert_eq!(built.device_freshness, chained.device_freshness);
        assert!(built.stanza_id.is_none());
        let neutral = StatusSendOptions::default();
        assert_eq!(neutral.privacy, StatusPrivacySetting::Contacts);
        assert!(neutral.message_id.is_none());
        assert!(neutral.stanza_id.is_none());
        assert!(neutral.extra_stanza_nodes.is_empty());
        assert_eq!(
            neutral.device_freshness,
            StatusSendOptions::default().device_freshness
        );
    }

    #[tokio::test]
    async fn status_requires_explicit_audience_for_posts_and_revokes() {
        let client = crate::test_utils::create_test_client().await;
        let status = client.status();
        assert!(
            status.audience().is_none(),
            "unobserved is not an all-contacts audience"
        );
        let target = MessageId::new("STATUS-TARGET").unwrap();
        for privacy in [
            StatusPrivacySetting::Contacts,
            StatusPrivacySetting::AllowList,
            StatusPrivacySetting::DenyList,
        ] {
            let options = StatusSendOptions::default().with_privacy(privacy);
            let posted = status
                .send_raw(wa::Message::default(), &[], options.clone())
                .await;
            let revoked = status.revoke(target.clone(), &[], options.clone()).await;
            for result in [posted, revoked] {
                assert!(
                    matches!(result, Err(SendError::InvalidRequest(ref message)) if message.contains("no recipients"))
                );
            }
            // Privacy never synthesizes or filters this explicit list: both
            // paths proceed to the same identity check, not an audience error.
            let recipients = [Jid::pn("15550000001")];
            assert!(matches!(
                status
                    .send_raw(wa::Message::default(), &recipients, options.clone())
                    .await,
                Err(SendError::NotLoggedIn)
            ));
            assert!(matches!(
                status.revoke(target.clone(), &recipients, options).await,
                Err(SendError::NotLoggedIn)
            ));
        }
        let collision =
            StatusSendOptions::default().with_stanza_id(StanzaId::from_message_id(&target));
        assert!(matches!(
            status.revoke(target, &[Jid::pn("15550000001")], collision).await,
            Err(SendError::InvalidRequest(ref message)) if message.contains("must differ")
        ));
        assert!(status.audience().is_none());
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
