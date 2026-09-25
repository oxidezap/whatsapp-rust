//! Newsletter (Channel) feature.
//!
//! Provides methods for listing, fetching, and managing newsletter channels.
//! Uses MEX (GraphQL) for metadata/management and standard IQ for message operations.
//! Newsletter messages are plaintext (no Signal E2E encryption).

use wacore::WireEnum;

use crate::client::{Client, ClientError};
use crate::features::mex::{MexError, mex_request};
use crate::request::IqError;
use thiserror::Error;
use wacore::iq::mex_operations::{
    change_newsletter_owner, create_newsletter, delete_newsletter, demote_newsletter_admin,
    fetch_all_newsletters_metadata, fetch_newsletter, fetch_newsletter_admin_info,
    fetch_newsletter_followers, join_newsletter, leave_newsletter, update_newsletter,
    update_newsletter_user_setting,
};
use wacore::iq::newsletter::{MyAddOnsSpec, NEWSLETTER_XMLNS};
pub use wacore::iq::newsletter::{NewsletterMyAddOns, NewsletterMyPollVote, NewsletterMyReaction};
use wacore::request::InfoQuery;
use wacore::types::message::{EditAttribute, PollType};
use wacore_binary::Jid;
use wacore_binary::JidExt as _;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{NodeContent, NodeContentRef, NodeRef};
use waproto::whatsapp as wa;

/// Error returned by newsletter (channel) operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum NewsletterError {
    /// A MEX (GraphQL) query/mutation failed or returned malformed data.
    #[error("{0}")]
    Mex(#[from] MexError),
    /// An IQ (message history, live updates) failed.
    #[error("{0}")]
    Iq(#[from] IqError),
    /// Connection/transport failure sending a plaintext stanza (edit/revoke).
    #[error("{0}")]
    Client(#[from] ClientError),
    /// The request was malformed (e.g. a non-newsletter JID, an empty target
    /// message id, or a missing element in the server response).
    #[error("invalid newsletter request: {0}")]
    InvalidRequest(String),
    /// Catch-all for internal failures with no dedicated variant.
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

impl NewsletterError {
    /// Recover the concrete typed error from an `anyhow` bubbled up by a helper
    /// that still threads `anyhow` (e.g. `send_server_reaction`), so transport
    /// failures stay matchable as `Client`/`Iq` instead of collapsing into the
    /// `Internal` catch-all via the blanket `#[from] anyhow::Error`.
    pub(crate) fn from_anyhow(err: anyhow::Error) -> Self {
        match err.downcast::<ClientError>() {
            Ok(ClientError::Iq(iq)) => NewsletterError::Iq(iq),
            Ok(client) => NewsletterError::Client(client),
            Err(other) => match other.downcast::<IqError>() {
                Ok(iq) => NewsletterError::Iq(iq),
                Err(other) => NewsletterError::Internal(other),
            },
        }
    }
}

// Types

/// The `type` attribute of a `<message>` in a newsletter's history.
///
/// Only [`Text`](Self::Text), [`Media`](Self::Media) and [`Poll`](Self::Poll)
/// are the history wire values confirmed by both the pinned and latest
/// whatspec IR. What the other variants try to name lives in sibling fields
/// instead, and matching on them will never fire:
///
/// - an edit is `edit="3"` and a revocation `edit="8"`, both read into
///   [`NewsletterMessage::edit`];
/// - a poll's stage is `<meta polltype>`, read into
///   [`NewsletterMessage::poll_type`], which is where the `creation` /
///   `quiz_creation` / `result_snapshot` distinction actually lives;
/// - a reaction is never a message type here: reactions arrive as counts on
///   the message they apply to ([`NewsletterMessage::reactions`]).
///
/// They are kept because removing a variant breaks callers that match on it;
/// prefer the fields above.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum NewsletterMessageType {
    #[wire = "text"]
    Text,
    #[wire = "media"]
    Media,
    /// A poll, quiz, or poll result snapshot. Which of the three is in
    /// [`NewsletterMessage::poll_type`].
    #[wire = "poll"]
    Poll,
    /// Never produced by the wire; see the type-level docs.
    #[wire = "reaction"]
    Reaction,
    /// Never produced by the wire; a revocation is `edit="8"`.
    #[wire = "revoke"]
    Revoke,
    /// Never produced by the wire; use [`Poll`](Self::Poll) plus
    /// [`NewsletterMessage::poll_type`].
    #[wire = "poll_creation"]
    PollCreation,
    /// Never produced by the wire; a channel poll's votes are counts on the
    /// poll message ([`NewsletterMessage::votes`]).
    #[wire = "poll_vote"]
    PollVote,
    /// Never produced by the wire; an edit is `edit="3"`.
    #[wire = "edit"]
    Edit,
    #[wire_fallback]
    Other(String),
}

/// The `mediatype` attribute of newsletter `<plaintext>`.
///
/// The known values come from the history IQ shape. `Other` keeps this API
/// forward-compatible if WhatsApp adds another media kind.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum NewsletterMediaType {
    #[wire = "1p_sticker"]
    OnePSticker,
    #[wire = "audio"]
    Audio,
    #[wire = "avatar_sticker"]
    AvatarSticker,
    #[wire = "cataloglink"]
    CatalogLink,
    #[wire = "collection"]
    Collection,
    #[wire = "document"]
    Document,
    #[wire = "genai_sticker"]
    GenAiSticker,
    #[wire = "gif"]
    Gif,
    #[wire = "image"]
    Image,
    #[wire = "motion_photo"]
    MotionPhoto,
    #[wire = "motion_video"]
    MotionVideo,
    #[wire = "productlink"]
    ProductLink,
    #[wire = "ptt"]
    Ptt,
    #[wire = "ptv"]
    Ptv,
    #[wire = "sticker"]
    Sticker,
    #[wire = "sticker_pack"]
    StickerPack,
    #[wire = "url"]
    Url,
    #[wire = "user_created_sticker"]
    UserCreatedSticker,
    #[wire = "vcard"]
    Vcard,
    #[wire = "video"]
    Video,
    #[wire_fallback]
    Other(String),
}

/// The `message_association_type` attribute in newsletter `<meta>`.
///
/// Unknown values are retained in `Other` instead of rejected, because this
/// attribute is a closed set in the current IR but can grow with new media
/// relationships.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum NewsletterMessageAssociationType {
    #[wire = "hd_image_dual_upload"]
    HdImageDualUpload,
    #[wire = "hd_video_dual_upload"]
    HdVideoDualUpload,
    #[wire = "hevc_video_dual_upload"]
    HevcVideoDualUpload,
    #[wire = "media_poll"]
    MediaPoll,
    #[wire = "motion_photo"]
    MotionPhoto,
    #[wire = "poll_add_option"]
    PollAddOption,
    #[wire = "sticker_annotation"]
    StickerAnnotation,
    #[wire_fallback]
    Other(String),
}

/// The `questiontype` attribute in newsletter `<meta>`.
///
/// This small closed set is kept forward-compatible for future question
/// variants.
#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum NewsletterQuestionType {
    #[wire = "question"]
    Question,
    #[wire = "reply"]
    Reply,
    #[wire_fallback]
    Other(String),
}

/// Newsletter verification status.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NewsletterVerification {
    Verified,
    Unverified,
}

/// Newsletter state.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NewsletterState {
    Active,
    Suspended,
    Geosuspended,
}

/// The viewer's role in a newsletter.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NewsletterRole {
    Owner,
    Admin,
    Subscriber,
    Guest,
}

/// Metadata for a newsletter (channel).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsletterMetadata {
    pub jid: Jid,
    pub name: String,
    pub description: Option<String>,
    pub subscriber_count: u64,
    pub verification: NewsletterVerification,
    pub state: NewsletterState,
    pub picture_url: Option<String>,
    pub preview_url: Option<String>,
    pub invite_code: Option<String>,
    pub role: Option<NewsletterRole>,
    pub creation_time: Option<u64>,
    /// Whether the viewer has muted the channel's updates (WA Web's
    /// `MUTE_ADMIN_ACTIVITY`, the "Mute" a follower toggles). `None` when the
    /// server sent no setting for it, which is what a non-follower sees.
    pub muted: Option<bool>,
    /// Whether the viewer has muted follower-activity notifications (WA Web's
    /// `MUTE_FOLLOWER_ACTIVITY`). Only admins and owners receive these.
    pub follower_activity_muted: Option<bool>,
}

/// An admin's public profile within a newsletter.
///
/// `id` is the profile's own identifier, not a JID: the server hands it back as
/// an opaque string and WA Web never parses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsletterAdminProfile {
    pub id: Option<String>,
    pub name: String,
    pub picture_id: Option<String>,
    pub picture_direct_path: Option<String>,
}

/// Admin-side information about a newsletter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsletterAdminInfo {
    /// How many admins the newsletter has. The server only answers this for
    /// admins and owners, so it is absent for everyone else.
    pub admin_count: Option<u32>,
    /// The viewer's own admin profile, present once they have set one up.
    pub admin_profile: Option<NewsletterAdminProfile>,
    /// Whether admin profiles are enabled for this newsletter.
    pub admin_profiles_enabled: Option<bool>,
}

/// A follower (subscriber) of a newsletter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewsletterFollower {
    /// The follower's identity JID — a LID on accounts that have migrated.
    pub jid: Jid,
    /// The follower's phone-number JID, withheld by the server when the
    /// follower's privacy settings hide it.
    pub phone_jid: Option<Jid>,
    pub display_name: Option<String>,
    pub username: Option<String>,
    /// The follower's role in the newsletter.
    pub role: Option<NewsletterRole>,
    /// When the follower subscribed (Unix seconds).
    pub follow_time: Option<u64>,
    /// Set when the follower is an admin who published a profile.
    pub admin_profile: Option<NewsletterAdminProfile>,
}

/// A reaction count on a newsletter message.
#[derive(Debug, Clone)]
pub struct NewsletterReactionCount {
    pub code: String,
    pub count: u64,
}

/// A poll option's vote tally on a newsletter message.
///
/// Channel polls are counted server-side, so unlike a DM or group vote (which
/// arrives encrypted, see [`wacore::poll`]) there is nothing to decrypt here:
/// the server hands over the totals directly, keyed by option hash.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct NewsletterPollVote {
    /// SHA-256 of the option name, the same digest
    /// [`wacore::poll::compute_option_hash`] produces. Match it against the
    /// options of the `pollCreationMessage` in
    /// [`NewsletterMessage::message`] to recover which option was voted for.
    pub option_hash: [u8; 32],
    /// How many followers picked this option.
    pub count: u64,
}

/// A message from a newsletter's history.
///
/// `#[non_exhaustive]` because the server keeps adding children to
/// `<message>`: construct it from a parsed response rather than by struct
/// literal, so the next counter WhatsApp ships is an added field and not a
/// break.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct NewsletterMessage {
    /// Wire message id (the stanza `id`). This is what edit_message / revoke_message
    /// key on (NOT `server_id`). Empty if the server omitted it.
    pub message_id: String,
    /// Server-assigned message ID (monotonic, used for pagination cursors).
    pub server_id: u64,
    /// Message timestamp (Unix seconds).
    pub timestamp: u64,
    /// Message type: `text`, `media` or `poll`. An edit or a revocation is not
    /// a type of its own, see [`edit`](Self::edit).
    pub message_type: NewsletterMessageType,
    /// The `edit` attribute: [`EditAttribute::AdminEdit`] (`3`) for an edited
    /// message, [`EditAttribute::AdminRevoke`] (`8`) for a revoked one.
    /// [`EditAttribute::Empty`] when the attribute is absent, which is the
    /// common case.
    ///
    /// A revoked message keeps its envelope and loses its body: the server
    /// sends an empty `<plaintext/>`, drops the forward counter, and
    /// [`message`](Self::message) is `None`.
    pub edit: EditAttribute,
    /// Whether the viewer is the sender.
    pub is_sender: bool,
    /// Decoded protobuf message (from `<plaintext>` bytes). `None` when the
    /// server sent no body, as it does for a revocation.
    pub message: Option<wa::Message>,
    /// The `mediatype` attribute of `<plaintext>`, a hint about the payload
    /// that is readable without decoding it. Unknown values are retained as
    /// [`NewsletterMediaType::Other`].
    pub media_type: Option<NewsletterMediaType>,
    /// Reaction counts on this message.
    pub reactions: Vec<NewsletterReactionCount>,
    /// Per-option vote tallies, for a `poll` message.
    pub votes: Vec<NewsletterPollVote>,
    /// How many times the message was forwarded.
    ///
    /// `None` means the server sent no counter at all — it omits the node
    /// rather than sending a zero, so `Some(0)` is not what an unforwarded
    /// message looks like.
    pub forwards_count: Option<u64>,
    /// How many times the message was viewed.
    ///
    /// Modelled from the IQ contract and from whatsmeow, which reads it; a
    /// capture of 267 history messages across 9 channels, taken as a plain
    /// follower, never carried one. Expect `None` unless the viewer's role in
    /// the channel entitles them to view counts.
    pub views_count: Option<u64>,
    /// How many responses a channel question collected. Same caveat as
    /// [`views_count`](Self::views_count): contract-derived, not observed.
    pub responses_count: Option<u64>,
    /// `<meta original_msg_t>` (Unix **seconds**): when the message was first
    /// posted, as opposed to [`timestamp`](Self::timestamp), which moves with
    /// an edit.
    pub original_timestamp: Option<u64>,
    /// `<meta msg_edit_t>` (Unix **milliseconds**, unlike every other
    /// timestamp on this struct): when the message was last edited.
    pub last_edit_timestamp_ms: Option<u64>,
    /// `<meta polltype>`: which stage of a poll's lifecycle this message is.
    ///
    /// Read only when [`message_type`](Self::message_type) is
    /// [`NewsletterMessageType::Poll`], the same scoping WA Web applies, so a
    /// `polltype` on anything else is ignored rather than recorded.
    pub poll_type: Option<PollType>,
    /// `<meta contenttype>`.
    pub content_type: Option<String>,
    /// `<meta questiontype>`: `question` for a channel question, `reply` for an
    /// admin's reply to one. Unknown values are retained as `Other`.
    pub question_type: Option<NewsletterQuestionType>,
    /// `<meta message_association_type>`: how this media relates to another
    /// message (`media_poll`, `motion_photo`, `poll_add_option`, …). Unknown
    /// values are retained as `Other`.
    pub message_association_type: Option<NewsletterMessageAssociationType>,
    /// `<meta is_wamo_sub="true">`.
    pub is_wamo_sub: bool,
    /// `<meta><admin_profile>`: the publishing admin's public profile, on
    /// channels that enabled admin profiles.
    pub admin_profile: Option<NewsletterAdminProfile>,
    /// `<rcat>` content bytes, an opaque receiver-side token. WA Web
    /// associates it with `type="media"` and `mediatype="url"` link
    /// previews; this parser intentionally accepts it on any message for
    /// forward compatibility and carries it verbatim without interpreting it.
    pub rcat: Option<Vec<u8>>,
}

/// Feature handle for newsletter (channel) operations.
pub struct Newsletter<'a> {
    client: &'a Client,
}

impl<'a> Newsletter<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// List all newsletters the user is subscribed to.
    pub async fn list_subscribed(&self) -> Result<Vec<NewsletterMetadata>, NewsletterError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(fetch_all_newsletters_metadata {
                // Both gate response blocks this client does not parse. WA Web
                // reads the same two feature flags and sends whatever they say,
                // so `false` is a value the server sees from it every day.
                fetch_status_metadata: Some(false),
                fetch_wamo_sub: Some(false),
            }))
            .await?;

        let data = response
            .data
            .ok_or_else(|| NewsletterError::InvalidRequest("missing data".into()))?;
        let newsletters = data["xwa2_newsletter_subscribed"]
            .as_array()
            .ok_or_else(|| {
                NewsletterError::InvalidRequest("missing xwa2_newsletter_subscribed array".into())
            })?;

        newsletters.iter().map(parse_newsletter_metadata).collect()
    }

    /// Fetch metadata for a newsletter by its JID.
    pub async fn get_metadata(&self, jid: &Jid) -> Result<NewsletterMetadata, NewsletterError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(
                fetch_newsletter,
                newsletter_variables(&jid.to_string(), "JID")
            ))
            .await?;

        let data = response
            .data
            .ok_or_else(|| NewsletterError::InvalidRequest("missing data".into()))?;
        let newsletter = &data["xwa2_newsletter"];
        if newsletter.is_null() {
            return Err(NewsletterError::InvalidRequest(format!(
                "newsletter not found: {}",
                jid
            )));
        }
        parse_newsletter_metadata(newsletter)
    }

    /// Create a new newsletter.
    ///
    /// Returns the metadata of the newly created newsletter.
    pub async fn create(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<NewsletterMetadata, NewsletterError> {
        let response = self
            .client
            .mex()
            .mutate(mex_request!(create_newsletter {
                input: Some(create_newsletter::Input {
                    name: Some(name.to_string()),
                    description: description.map(str::to_string),
                    picture: None,
                }),
            }))
            .await?;

        let data = response
            .data
            .ok_or_else(|| NewsletterError::InvalidRequest("missing data".into()))?;
        let newsletter = &data["xwa2_newsletter_create"];
        if newsletter.is_null() {
            return Err(NewsletterError::InvalidRequest(
                "newsletter creation failed".into(),
            ));
        }
        parse_newsletter_metadata(newsletter)
    }

    /// Join (subscribe to) a newsletter.
    ///
    /// Returns the newsletter metadata with the viewer's role set to `Subscriber`.
    pub async fn join(&self, jid: &Jid) -> Result<NewsletterMetadata, NewsletterError> {
        let response = self
            .client
            .mex()
            .mutate(mex_request!(join_newsletter {
                newsletter_id: Some(jid.to_string()),
            }))
            .await?;

        let data = response
            .data
            .ok_or_else(|| NewsletterError::InvalidRequest("missing data".into()))?;
        let newsletter = &data["xwa2_newsletter_join_v2"];
        if newsletter.is_null() {
            return Err(NewsletterError::InvalidRequest(format!(
                "failed to join newsletter: {}",
                jid
            )));
        }
        parse_newsletter_metadata(newsletter)
    }

    /// Leave (unsubscribe from) a newsletter.
    pub async fn leave(&self, jid: &Jid) -> Result<(), NewsletterError> {
        let response = self
            .client
            .mex()
            .mutate(mex_request!(leave_newsletter {
                newsletter_id: Some(jid.to_string()),
            }))
            .await?;

        let data = response
            .data
            .ok_or_else(|| NewsletterError::InvalidRequest("missing data".into()))?;
        if data["xwa2_newsletter_leave_v2"].is_null() {
            return Err(NewsletterError::InvalidRequest(format!(
                "failed to leave newsletter: {}",
                jid
            )));
        }
        Ok(())
    }

    /// Update a newsletter's name and/or description.
    pub async fn update(
        &self,
        jid: &Jid,
        name: Option<&str>,
        description: Option<&str>,
    ) -> Result<NewsletterMetadata, NewsletterError> {
        let response = self
            .client
            .mex()
            .mutate(mex_request!(update_newsletter {
                newsletter_id: Some(jid.to_string()),
                updates: Some(update_newsletter::Updates {
                    name: name.map(str::to_string),
                    description: description.map(str::to_string),
                    picture: None,
                    settings: None,
                }),
            }))
            .await?;

        let newsletter = take_data_field(response.data, "xwa2_newsletter_update")?;
        parse_newsletter_metadata(&newsletter)
    }

    /// Replace a newsletter's picture with `jpeg`.
    ///
    /// Carried by the same mutation as [`Newsletter::update`] — WA Web edits the
    /// picture through `updates.picture`, base64-encoded — so the response is the
    /// newsletter's refreshed metadata.
    pub async fn set_picture(
        &self,
        jid: &Jid,
        jpeg: &[u8],
    ) -> Result<NewsletterMetadata, NewsletterError> {
        self.update_picture(jid, Some(jpeg)).await
    }

    /// Remove a newsletter's picture.
    pub async fn remove_picture(&self, jid: &Jid) -> Result<NewsletterMetadata, NewsletterError> {
        self.update_picture(jid, None).await
    }

    async fn update_picture(
        &self,
        jid: &Jid,
        jpeg: Option<&[u8]>,
    ) -> Result<NewsletterMetadata, NewsletterError> {
        let response = self
            .client
            .mex()
            .mutate(mex_request!(
                update_newsletter,
                picture_update_variables(jid, jpeg)
            ))
            .await?;

        let newsletter = take_data_field(response.data, "xwa2_newsletter_update")?;
        parse_newsletter_metadata(&newsletter)
    }

    /// Delete a newsletter. Owner-only.
    pub async fn delete(&self, jid: &Jid) -> Result<(), NewsletterError> {
        let response = self
            .client
            .mex()
            .mutate(mex_request!(delete_newsletter, delete_variables(jid)))
            .await?;

        take_data_field(response.data, "xwa2_newsletter_delete_v2")?;
        Ok(())
    }

    /// Transfer a newsletter's ownership to `user`. Owner-only.
    ///
    /// `user` may be a LID or a phone-number JID; the latter is resolved to its
    /// LID first, since that is the only form the server addresses admins by.
    pub async fn change_owner(&self, jid: &Jid, user: &Jid) -> Result<(), NewsletterError> {
        let user = self.resolve_admin_target(user).await?;
        let response = self
            .client
            .mex()
            .mutate(mex_request!(
                change_newsletter_owner,
                change_owner_variables(jid, &user)
            ))
            .await?;

        take_data_field(response.data, "xwa2_newsletter_change_owner")?;
        Ok(())
    }

    /// Demote an admin of a newsletter back to subscriber. Owner-only.
    ///
    /// Same LID resolution as [`Newsletter::change_owner`].
    pub async fn demote_admin(&self, jid: &Jid, user: &Jid) -> Result<(), NewsletterError> {
        let user = self.resolve_admin_target(user).await?;
        let response = self
            .client
            .mex()
            .mutate(mex_request!(
                demote_newsletter_admin,
                demote_admin_variables(jid, &user)
            ))
            .await?;

        take_data_field(response.data, "xwa2_newsletter_admin_demote")?;
        Ok(())
    }

    /// Mirror of WA Web's `toUserLidOrThrow`: the admin mutations refuse a target
    /// that has no known LID rather than sending a PN the server cannot address.
    async fn resolve_admin_target(&self, user: &Jid) -> Result<Jid, NewsletterError> {
        self.client
            .resolve_recipient_to_lid(user)
            .await
            .ok_or_else(|| {
                NewsletterError::InvalidRequest(format!("no known LID for newsletter admin {user}"))
            })
    }

    /// Fetch a newsletter's admin-side information, including its admin count.
    ///
    /// The admin count has no query of its own: it rides along with the admin
    /// profile and the admin-profiles setting in this one response.
    pub async fn get_admin_info(&self, jid: &Jid) -> Result<NewsletterAdminInfo, NewsletterError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(
                fetch_newsletter_admin_info,
                admin_info_variables(jid)
            ))
            .await?;

        let admin = take_data_field(response.data, "xwa2_newsletter_admin")?;
        Ok(parse_newsletter_admin_info(&admin))
    }

    /// List up to `count` of a newsletter's followers.
    ///
    /// The operation takes no cursor — WA Web asks for one page clamped to its
    /// subscriber-list limit — so `count` is the whole request.
    pub async fn get_followers(
        &self,
        jid: &Jid,
        count: u32,
    ) -> Result<Vec<NewsletterFollower>, NewsletterError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(
                fetch_newsletter_followers,
                followers_variables(jid, count)
            ))
            .await?;

        let followers = take_data_field(response.data, "xwa2_newsletter_followers")?;
        parse_newsletter_followers(&followers)
    }

    /// Mute or unmute a newsletter's follower-activity notifications
    /// (WA Web's `MUTE_FOLLOWER_ACTIVITY`). `muted = true` silences them.
    /// Only meaningful for owners/admins: WA Web offers it behind its
    /// admin-notifications gate.
    pub async fn set_follower_mute(&self, jid: &Jid, muted: bool) -> Result<(), NewsletterError> {
        self.set_user_setting_mute(jid, "MUTE_FOLLOWER_ACTIVITY", muted)
            .await
    }

    /// Mute or unmute a newsletter's admin-activity notifications (WA Web's
    /// `MUTE_ADMIN_ACTIVITY`): the channel's own updates, so this is the
    /// "Mute" a follower toggles. WA Web stores it as the chat's mute state.
    pub async fn set_admin_mute(&self, jid: &Jid, muted: bool) -> Result<(), NewsletterError> {
        self.set_user_setting_mute(jid, "MUTE_ADMIN_ACTIVITY", muted)
            .await
    }

    async fn set_user_setting_mute(
        &self,
        jid: &Jid,
        mute_type: &str,
        muted: bool,
    ) -> Result<(), NewsletterError> {
        let response = self
            .client
            .mex()
            .mutate(mex_request!(
                update_newsletter_user_setting,
                mute_user_setting_variables(jid, mute_type, muted)
            ))
            .await?;

        let data = response
            .data
            .ok_or_else(|| NewsletterError::InvalidRequest("missing data".into()))?;
        if data["xwa2_newsletter_update_user_setting"].is_null() {
            return Err(NewsletterError::InvalidRequest(format!(
                "failed to update newsletter user setting: {jid}"
            )));
        }
        Ok(())
    }

    /// Fetch metadata for a newsletter by its invite code.
    pub async fn get_metadata_by_invite(
        &self,
        invite_code: &str,
    ) -> Result<NewsletterMetadata, NewsletterError> {
        let response = self
            .client
            .mex()
            .query(mex_request!(
                fetch_newsletter,
                newsletter_variables(invite_code, "INVITE")
            ))
            .await?;

        let data = response
            .data
            .ok_or_else(|| NewsletterError::InvalidRequest("missing data".into()))?;
        let newsletter = &data["xwa2_newsletter"];
        if newsletter.is_null() {
            return Err(NewsletterError::InvalidRequest(format!(
                "newsletter not found for invite: {}",
                invite_code
            )));
        }
        parse_newsletter_metadata(newsletter)
    }

    // ─── Live updates ───────────────────────────────────────────────────

    /// Subscribe to live counter updates for a newsletter: reactions, forwards
    /// and poll tallies per message, delivered as
    /// [`wacore::types::events::Event::NewsletterLiveUpdate`].
    ///
    /// Returns the subscription duration in seconds, after which it has to be
    /// renewed. The server sets this; 90 seconds is what it answered in a real
    /// session, and the 300 below is only the fallback for a response that
    /// omits the attribute entirely.
    pub async fn subscribe_live_updates(
        &self,
        jid: impl Into<Jid>,
    ) -> Result<u64, NewsletterError> {
        let jid = &jid.into();
        let iq = InfoQuery::set(
            NEWSLETTER_XMLNS,
            jid.clone(),
            Some(NodeContent::Nodes(vec![
                NodeBuilder::new("live_updates").build(),
            ])),
        );

        let response = self.client.send_iq(iq).await?;
        let nr = response.get();
        let duration = nr
            .get_optional_child("live_updates")
            .and_then(|n| n.get_attr("duration"))
            .map(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(300);

        Ok(duration)
    }

    /// Send a reaction to a newsletter message.
    ///
    /// `server_id` is the server-assigned ID of the message to react to.
    /// `reaction` is the emoji code (e.g., "👍", "❤️"), or empty to remove.
    pub async fn send_reaction(
        &self,
        jid: &Jid,
        server_id: u64,
        reaction: &str,
    ) -> Result<(), NewsletterError> {
        self.client
            .send_server_reaction(jid, server_id, reaction)
            .await
            .map_err(NewsletterError::from_anyhow)?;
        Ok(())
    }

    /// Vote in a newsletter poll.
    ///
    /// `server_id` is the poll's. `option_hashes` is the whole selection, one
    /// [`wacore::poll::compute_option_hash`] per chosen option, the same key
    /// [`NewsletterPollVote::option_hash`] tallies by. Hash the option name
    /// exactly as the poll payload carries it: names hold variation selectors
    /// and ZWJ sequences, and a retyped or normalized name hashes to an option
    /// the server does not know.
    ///
    /// Every send replaces the previous selection on the server, so changing a
    /// vote is a send with the new list and an empty slice removes it.
    ///
    /// Returns the stanza id. Match it against the `id` of the server's ack,
    /// which arrives as [`wacore::types::events::Event::ServerAck`]; the ack
    /// also names the poll's `server_id` on the wire, but that event does not
    /// carry it.
    pub async fn send_poll_vote(
        &self,
        jid: &Jid,
        server_id: u64,
        option_hashes: &[[u8; 32]],
    ) -> Result<String, NewsletterError> {
        if !jid.is_newsletter() {
            return Err(NewsletterError::InvalidRequest(
                "send_poll_vote is only valid for newsletter (channel) JIDs".into(),
            ));
        }
        validate_poll_vote(option_hashes)?;
        let id = self.client.generate_message_id();
        self.client
            .send_node(build_poll_vote_node(jid, &id, server_id, option_hashes))
            .await?;
        Ok(id)
    }

    /// Edit a message in a newsletter (channel). Channels are plaintext (not E2E).
    ///
    /// `message_id` is the target message's id (the `message_id` from
    /// [`NewsletterMessage`] / the id returned when it was sent), NOT its
    /// `server_id` (edit/revoke key on the message id, unlike reactions which use
    /// `server_id`). `new_content` is the replacement body (e.g.
    /// `wa::Message { conversation: Some(..), .. }`).
    pub async fn edit_message(
        &self,
        jid: &Jid,
        message_id: impl Into<String>,
        new_content: wa::Message,
    ) -> Result<(), NewsletterError> {
        if !jid.is_newsletter() {
            return Err(NewsletterError::InvalidRequest(
                "edit_message is only valid for newsletter (channel) JIDs; use Client::edit_message for DM/group".into(),
            ));
        }
        let id = message_id.into();
        if id.is_empty() {
            return Err(NewsletterError::InvalidRequest(
                "newsletter edit needs a target message_id (NewsletterMessage.message_id is empty when the server omits the id)".into(),
            ));
        }
        let node = crate::send::build_newsletter_edit_node(
            jid,
            &id,
            crate::send::NewsletterEdit::Edit(&new_content),
        );
        self.client.send_node(node).await?;
        Ok(())
    }

    /// Revoke (delete) a message in a newsletter (channel).
    ///
    /// `message_id` is the target message's id (the `message_id` from
    /// [`NewsletterMessage`]), NOT its `server_id`.
    pub async fn revoke_message(
        &self,
        jid: &Jid,
        message_id: impl Into<String>,
    ) -> Result<(), NewsletterError> {
        if !jid.is_newsletter() {
            return Err(NewsletterError::InvalidRequest(
                "revoke_message is only valid for newsletter (channel) JIDs; use Client::revoke_message for DM/group".into(),
            ));
        }
        let id = message_id.into();
        if id.is_empty() {
            return Err(NewsletterError::InvalidRequest(
                "newsletter revoke needs a target message_id (NewsletterMessage.message_id is empty when the server omits the id)".into(),
            ));
        }
        let node =
            crate::send::build_newsletter_edit_node(jid, &id, crate::send::NewsletterEdit::Revoke);
        self.client.send_node(node).await?;
        Ok(())
    }

    /// Fetch message history from a newsletter.
    ///
    /// Returns up to `count` messages. Use `before` with a `server_id` from a previous
    /// response to paginate backwards through history.
    pub async fn get_messages(
        &self,
        jid: impl Into<Jid>,
        count: u32,
        before: Option<u64>,
    ) -> Result<Vec<NewsletterMessage>, NewsletterError> {
        let jid = &jid.into();
        let response = self
            .client
            .send_iq(build_newsletter_messages_iq(jid, count, before))
            .await?;
        parse_newsletter_messages_response(response.get())
    }

    /// Read this account's own reactions and poll votes on a newsletter's
    /// recent messages.
    ///
    /// Only messages the account has an add-on on are returned, up to `limit`
    /// of them.
    pub async fn get_my_addons(
        &self,
        jid: &Jid,
        limit: u32,
    ) -> Result<Vec<NewsletterMyAddOns>, NewsletterError> {
        if !jid.is_newsletter() {
            return Err(NewsletterError::InvalidRequest(
                "get_my_addons is only valid for newsletter (channel) JIDs".into(),
            ));
        }
        Ok(self.client.execute(MyAddOnsSpec::new(jid, limit)).await?)
    }
}

/// The most `<vote>` children one vote may carry, from the
/// `REPEATED_CHILD(<vote>, 0, 1000)` in WA Web's
/// `WASmaxOutMessagePublishNewsletterPollVoteMixin`.
const MAX_POLL_VOTE_OPTIONS: usize = 1000;

/// Refuse a selection WA Web could never send. Its UI produces a set of at
/// most [`MAX_POLL_VOTE_OPTIONS`] distinct options, and how the server answers
/// anything else has not been observed, so neither is left for it to decide.
fn validate_poll_vote(option_hashes: &[[u8; 32]]) -> Result<(), NewsletterError> {
    if option_hashes.len() > MAX_POLL_VOTE_OPTIONS {
        return Err(NewsletterError::InvalidRequest(format!(
            "a poll vote carries at most {MAX_POLL_VOTE_OPTIONS} options, got {}",
            option_hashes.len()
        )));
    }
    let mut seen = std::collections::HashSet::with_capacity(option_hashes.len());
    if option_hashes.iter().any(|hash| !seen.insert(hash)) {
        return Err(NewsletterError::InvalidRequest(
            "a poll vote names each option at most once".into(),
        ));
    }
    Ok(())
}

/// Build a newsletter poll vote.
///
/// The reaction envelope of `send_server_reaction`, typed `poll`, with
/// `server_id` naming the poll rather than a message of its own. `<meta>`
/// comes before `<votes>` because that is the order WA Web sends; whether the
/// server requires it is unknown. `<meta>` never carries `contenttype` on a
/// vote: `WAWebNewsletterSendMessageQueryJob` does not pass one on that path.
/// Nothing is encrypted, since a channel has no key.
fn build_poll_vote_node(
    jid: &Jid,
    id: &str,
    server_id: u64,
    option_hashes: &[[u8; 32]],
) -> wacore_binary::Node {
    let votes = option_hashes
        .iter()
        .map(|hash| NodeBuilder::new("vote").bytes(hash.as_slice()).build());
    NodeBuilder::new("message")
        .attr("to", jid)
        .attr("id", id)
        .attr("server_id", server_id)
        .attr("type", "poll")
        .children([
            NodeBuilder::new("meta").attr("polltype", "vote").build(),
            NodeBuilder::new("votes").children(votes).build(),
        ])
        .build()
}

/// Build the history IQ.
///
/// History is the one `newsletter` request addressed to the server rather than
/// to the channel: `to` picks the server-side handler, and the server-scoped
/// handler has to be told which thread, hence `type="jid" jid="…"` on the node.
/// The thread-scoped requests (`live_updates`, `message_updates`) keep
/// `to = jid` and name nothing inside. Both halves are load-bearing — sending
/// `<messages>` to the channel, or to the server without naming the channel,
/// draws no stanza back at all, not even an error, and the pending IQ then
/// suppresses the keepalive ping until the dead-socket watchdog reconnects.
/// `makeGetNewsletterMessagesRequest` in the IR pins the server target as a
/// constant, unlike the sibling requests that take theirs as an argument.
fn build_newsletter_messages_iq(jid: &Jid, count: u32, before: Option<u64>) -> InfoQuery<'static> {
    let mut messages_node = NodeBuilder::new("messages")
        .attr("type", "jid")
        .attr("jid", jid.clone())
        .attr("count", count);
    if let Some(before_id) = before {
        messages_node = messages_node.attr("before", before_id);
    }

    InfoQuery::get(
        NEWSLETTER_XMLNS,
        crate::jid_utils::server_jid().clone(),
        Some(NodeContent::Nodes(vec![messages_node.build()])),
    )
}

impl Client {
    /// Access newsletter (channel) operations.
    #[inline]
    pub fn newsletter(&self) -> Newsletter<'_> {
        Newsletter::new(self)
    }
}

// JSON parsing helper

/// Take a top-level field out of a MEX response body, rejecting a `null` one.
/// WA Web reads the same absence as a server failure rather than an empty result
/// (`WAWebMexDeleteNewsletterJob` raises a 500 on a null payload).
fn take_data_field(
    data: Option<serde_json::Value>,
    field: &str,
) -> Result<serde_json::Value, NewsletterError> {
    let mut data = data.ok_or_else(|| NewsletterError::InvalidRequest("missing data".into()))?;
    match data.get_mut(field).map(serde_json::Value::take) {
        Some(value) if !value.is_null() => Ok(value),
        _ => Err(NewsletterError::InvalidRequest(format!(
            "newsletter response missing {field}"
        ))),
    }
}

/// `viewer_metadata` spells the role lowercase while the follower list spells it
/// uppercase, so the comparison has to ignore case.
fn parse_newsletter_role(raw: &str) -> Option<NewsletterRole> {
    if raw.eq_ignore_ascii_case("owner") {
        Some(NewsletterRole::Owner)
    } else if raw.eq_ignore_ascii_case("admin") {
        Some(NewsletterRole::Admin)
    } else if raw.eq_ignore_ascii_case("subscriber") {
        Some(NewsletterRole::Subscriber)
    } else if raw.eq_ignore_ascii_case("guest") {
        Some(NewsletterRole::Guest)
    } else {
        None
    }
}

/// A profile exists only once it carries a name; WA Web drops a nameless one.
fn parse_admin_profile(value: &serde_json::Value) -> Option<NewsletterAdminProfile> {
    let name = value["name"].as_str()?;
    Some(NewsletterAdminProfile {
        id: value["id"].as_str().map(str::to_string),
        name: name.to_string(),
        picture_id: value["picture"]["id"].as_str().map(str::to_string),
        picture_direct_path: value["picture"]["direct_path"].as_str().map(str::to_string),
    })
}

fn parse_newsletter_admin_info(value: &serde_json::Value) -> NewsletterAdminInfo {
    NewsletterAdminInfo {
        admin_count: parse_json_u64(&value["admin_count"]).and_then(|c| u32::try_from(c).ok()),
        admin_profile: parse_admin_profile(&value["admin_profile"]),
        admin_profiles_enabled: value["admin_settings"]["admin_profiles_enabled"].as_bool(),
    }
}

fn parse_newsletter_followers(
    value: &serde_json::Value,
) -> Result<Vec<NewsletterFollower>, NewsletterError> {
    // An absent `edges` is an empty page, not a failure — the follower list is
    // only withheld wholesale, and that already failed in `take_data_field`.
    let Some(edges) = value["followers"]["edges"].as_array() else {
        return Ok(Vec::new());
    };

    let mut followers = Vec::with_capacity(edges.len());
    for edge in edges {
        let node = &edge["node"];
        // WA Web drops an edge with no id: there is nobody to address.
        let Some(jid) = node["id"].as_str() else {
            continue;
        };
        let jid: Jid = jid
            .parse()
            .map_err(|e| NewsletterError::InvalidRequest(format!("invalid follower id: {e}")))?;
        let phone_jid = match node["pn"].as_str() {
            Some(pn) => Some(pn.parse().map_err(|e| {
                NewsletterError::InvalidRequest(format!("invalid follower pn: {e}"))
            })?),
            None => None,
        };

        followers.push(NewsletterFollower {
            jid,
            phone_jid,
            display_name: node["display_name"].as_str().map(str::to_string),
            username: node["username_info"]["username"]
                .as_str()
                .map(str::to_string),
            role: edge["role"].as_str().and_then(parse_newsletter_role),
            follow_time: parse_json_u64(&edge["follow_time"]),
            admin_profile: parse_admin_profile(&edge["admin_profile"]),
        });
    }
    Ok(followers)
}

/// MEX numbers reach us as either JSON numbers or decimal strings depending on
/// the field's server-side width, and the IR types are too loose to tell which.
fn parse_json_u64(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_str().and_then(|s| s.parse().ok()))
}

fn parse_newsletter_metadata(
    value: &serde_json::Value,
) -> Result<NewsletterMetadata, NewsletterError> {
    let jid_str = value["id"]
        .as_str()
        .ok_or_else(|| NewsletterError::InvalidRequest("missing newsletter id".into()))?;
    let jid: Jid = jid_str
        .parse()
        .map_err(|e| NewsletterError::InvalidRequest(format!("invalid newsletter id: {e}")))?;

    let thread = &value["thread_metadata"];

    let name = thread["name"]["text"].as_str().unwrap_or("").to_string();
    let description = thread["description"]["text"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string());

    let subscriber_count = thread["subscribers_count"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);

    let verification = match thread["verification"].as_str() {
        Some("VERIFIED") => NewsletterVerification::Verified,
        _ => NewsletterVerification::Unverified,
    };

    let state = match value["state"]["type"].as_str() {
        Some("suspended") => NewsletterState::Suspended,
        Some("geosuspended") => NewsletterState::Geosuspended,
        _ => NewsletterState::Active,
    };

    let picture_url = thread["picture"]["direct_path"]
        .as_str()
        .map(|s| s.to_string());
    let preview_url = thread["preview"]["direct_path"]
        .as_str()
        .map(|s| s.to_string());
    let invite_code = thread["invite"].as_str().map(|s| s.to_string());

    let creation_time = thread["creation_time"]
        .as_str()
        .and_then(|s| s.parse::<u64>().ok());

    let role = value["viewer_metadata"]["role"]
        .as_str()
        .and_then(parse_newsletter_role);

    let settings = &value["viewer_metadata"]["settings"];
    let muted = parse_viewer_mute_setting(settings, "MUTE_ADMIN_ACTIVITY");
    let follower_activity_muted = parse_viewer_mute_setting(settings, "MUTE_FOLLOWER_ACTIVITY");

    Ok(NewsletterMetadata {
        jid,
        name,
        description,
        subscriber_count,
        verification,
        state,
        picture_url,
        preview_url,
        invite_code,
        role,
        creation_time,
        muted,
        follower_activity_muted,
    })
}

/// Read one mute setting from `viewer_metadata.settings`, a list of
/// `{ type, value }` pairs. Like WA Web (`WAWebNewsletterModelUtils`), only
/// `ON`/`OFF` count; anything else leaves the state unknown.
fn parse_viewer_mute_setting(settings: &serde_json::Value, mute_type: &str) -> Option<bool> {
    let setting = settings
        .as_array()?
        .iter()
        .find(|s| s["type"].as_str() == Some(mute_type))?;
    match setting["value"].as_str()? {
        "ON" => Some(true),
        "OFF" => Some(false),
        _ => None,
    }
}

// ─── Shared parsing helpers ────────────────────────────────────────────

/// Parse reaction counts from a `<reactions>` node.
/// Used by both message history parsing and notification handling.
pub(crate) fn parse_reaction_counts(node: &NodeRef<'_>) -> Vec<NewsletterReactionCount> {
    let mut reactions = Vec::new();
    if let Some(reactions_node) = node.get_optional_child("reactions")
        && let Some(children) = reactions_node.children()
    {
        for r in children.iter().filter(|n| n.tag.as_ref() == "reaction") {
            let Some(code) = r
                .get_attr("code")
                .map(|v| v.as_str())
                .filter(|s| !s.is_empty())
                .map(|s| s.into_owned())
            else {
                continue;
            };
            let count = r
                .get_attr("count")
                .map(|v| v.as_str())
                .and_then(|s| s.parse::<u64>().ok())
                .unwrap_or(0);
            reactions.push(NewsletterReactionCount { code, count });
        }
    }
    reactions
}

/// Parse per-option tallies from a `<votes>` node.
///
/// Not gated on the message's `type`: history responses can carry `<votes>`
/// on a poll envelope whose type is missing or is not yet known, so gating
/// here would make the parser brittle as the wire grows. Live updates carry
/// `<votes>` in the same shape and are read by this helper too.
///
/// A `<vote>` whose content is not a 32-byte digest is skipped rather than
/// truncated or padded — the hash is the only handle on which option was
/// voted for, and a wrong one silently attributes votes to the wrong option.
pub(crate) fn parse_poll_votes(node: &NodeRef<'_>) -> Vec<NewsletterPollVote> {
    let mut votes = Vec::new();
    if let Some(votes_node) = node.get_optional_child("votes")
        && let Some(children) = votes_node.children()
    {
        for v in children.iter().filter(|n| n.tag.as_ref() == "vote") {
            let Some(option_hash) = v
                .content_bytes()
                .and_then(|bytes| <[u8; 32]>::try_from(bytes).ok())
            else {
                continue;
            };
            let Some(count) = v.attrs().optional_u64("count") else {
                // `count` is required by the history contract; an absent or
                // malformed value is not the same fact as count zero.
                continue;
            };
            votes.push(NewsletterPollVote { option_hash, count });
        }
    }
    votes
}

/// Read a `<tag count="N"/>` child, the shape `<forwards_count>`,
/// `<views_count>` and `<responses_count>` share.
///
/// `<views_count>` has a second form upstream, `<views_count type="views"
/// count="N"/>`, which puts the count in the same place; both are read here.
///
/// The server omits the node instead of sending a zero, so `None` means "no
/// counter" and does not collapse with a count of zero.
pub(crate) fn parse_count_child(node: &NodeRef<'_>, tag: &str) -> Option<u64> {
    node.get_optional_child(tag)?.attrs().optional_u64("count")
}

// Node response parsing helpers

/// Parse the IQ response for newsletter message history.
///
/// Response format:
/// ```xml
/// <messages jid="NL_JID">
///   <message id="..." server_id="123" t="TS" type="media" [edit="3"|"8"] [is_sender="true"]>
///     <forwards_count count="12"/>
///     <rcat>…opaque bytes…</rcat>
///     <meta original_msg_t="TS" msg_edit_t="TS_MILLIS" [polltype="creation"]
///           [contenttype="…"] [questiontype="…"] [is_wamo_sub="true"]
///           [message_association_type="…"]>
///       <admin_profile id="…"><name>…</name><picture id="…" direct_path="…"/></admin_profile>
///     </meta>
///     <votes><vote count="7">…32-byte option hash…</vote></votes>
///     <reactions><reaction code="👍" count="3"/></reactions>
///     <plaintext mediatype="image">…protobuf…</plaintext>
///   </message>
/// </messages>
/// ```
///
/// The pinned and latest IQ shapes confirm the fields modelled below. The IR
/// also exposes paid-partnership and newsletter-AI content mixins, but it does
/// not provide a source path or wire shape for either. Raw bundle JavaScript
/// and a sanitized capture were not inspected for those mixins, so this
/// parser deliberately does not claim complete parity with the official
/// parser. An earlier version read only `<plaintext>` and `<reactions>`;
/// keeping this comment limited to the confirmed fields prevents that gap
/// from becoming the API's specification again.
///
/// A `<meta>` node appears at most once, in one of two mutually exclusive
/// shapes: attributes, or a single `<admin_profile>` child. Reading it by
/// attribute alone silently drops the profile.
///
/// Deliberately lenient where WA Web discriminates: the bundle gates message
/// subtypes on `<plaintext>` / `<reaction>` presence, but this parser flattens
/// every message into one struct without subtype dispatch, so a gate can never
/// misroute here — absence is `None`, never a rejection. Tightening this
/// (e.g. requiring `<plaintext>`) would drop messages WA delivers.
fn parse_newsletter_messages_response(
    response: &NodeRef<'_>,
) -> Result<Vec<NewsletterMessage>, NewsletterError> {
    // Response is the IQ result node; find <messages> child
    let messages_node = response.get_optional_child("messages").ok_or_else(|| {
        NewsletterError::InvalidRequest("missing <messages> in newsletter response".into())
    })?;

    let children = match messages_node.children() {
        Some(c) => c,
        None => return Ok(vec![]),
    };

    let mut result = Vec::with_capacity(children.len());
    for msg_node in children.iter().filter(|n| n.tag.as_ref() == "message") {
        // Skip nodes without a valid server_id (required for pagination/correlation)
        let Some(server_id) = msg_node
            .get_attr("server_id")
            .map(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
        else {
            continue;
        };

        // The wire `id` (string) is what edit/revoke key on; keep it alongside
        // server_id (which is used for pagination/reactions).
        let message_id = msg_node
            .get_attr("id")
            .map(|v| v.as_str().into_owned())
            .unwrap_or_default();

        let timestamp = msg_node
            .get_attr("t")
            .map(|v| v.as_str())
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0);

        let message_type = msg_node
            .get_attr("type")
            .map(|v| v.as_str())
            .map(|s| NewsletterMessageType::from(s.as_ref()))
            .unwrap_or(NewsletterMessageType::Text);

        let edit = msg_node
            .get_attr("edit")
            .map(|v| EditAttribute::from(v.as_str().as_ref()))
            .unwrap_or(EditAttribute::Empty);

        let is_sender = msg_node
            .get_attr("is_sender")
            .is_some_and(|v| v.as_str() == "true");

        let plaintext = msg_node.get_optional_child("plaintext");

        // Decode <plaintext> protobuf bytes. A revocation can carry an
        // explicit empty `<plaintext/>`; decoding that as protobuf would
        // produce `Message::default()`, so preserve the absent body.
        let message = plaintext.and_then(|pt| match pt.content.as_ref() {
            Some(NodeContentRef::Bytes(bytes)) if !bytes.as_ref().is_empty() => {
                waproto::codec::message_decode(bytes.as_ref()).ok()
            }
            _ => None,
        });

        let media_type = plaintext
            .and_then(|pt| pt.get_attr("mediatype"))
            .map(|v| NewsletterMediaType::from(v.as_str().as_ref()));

        let reactions = parse_reaction_counts(msg_node);
        let votes = parse_poll_votes(msg_node);

        let meta = parse_message_meta(msg_node, &message_type);

        result.push(NewsletterMessage {
            message_id,
            server_id,
            timestamp,
            message_type,
            edit,
            is_sender,
            message,
            media_type,
            reactions,
            votes,
            forwards_count: parse_count_child(msg_node, "forwards_count"),
            views_count: parse_count_child(msg_node, "views_count"),
            responses_count: parse_count_child(msg_node, "responses_count"),
            original_timestamp: meta.original_timestamp,
            last_edit_timestamp_ms: meta.last_edit_timestamp_ms,
            poll_type: meta.poll_type,
            content_type: meta.content_type,
            question_type: meta.question_type,
            message_association_type: meta.message_association_type,
            is_wamo_sub: meta.is_wamo_sub,
            admin_profile: meta.admin_profile,
            rcat: msg_node
                .get_optional_child("rcat")
                .and_then(|n| n.content_bytes())
                .map(<[u8]>::to_vec),
        });
    }

    Ok(result)
}

/// The `<meta>` child of a newsletter `<message>`, flattened.
///
/// Mirrors how `MessageInfo` treats `<meta>` for ordinary messages: the
/// attributes belong to the message, not to a nested object the caller has to
/// unwrap twice.
#[derive(Default)]
struct MessageMeta {
    original_timestamp: Option<u64>,
    last_edit_timestamp_ms: Option<u64>,
    poll_type: Option<PollType>,
    content_type: Option<String>,
    question_type: Option<NewsletterQuestionType>,
    message_association_type: Option<NewsletterMessageAssociationType>,
    is_wamo_sub: bool,
    admin_profile: Option<NewsletterAdminProfile>,
}

fn parse_message_meta(msg_node: &NodeRef<'_>, message_type: &NewsletterMessageType) -> MessageMeta {
    let Some(meta_node) = msg_node.get_optional_child("meta") else {
        return MessageMeta::default();
    };
    let mut attrs = meta_node.attrs();

    MessageMeta {
        // Seconds, while `msg_edit_t` right below is milliseconds. The two
        // units sit in the same node; do not unify them.
        original_timestamp: attrs.optional_u64("original_msg_t"),
        last_edit_timestamp_ms: attrs.optional_u64("msg_edit_t"),
        // WA Web scopes `polltype` to poll envelopes, so a value on any other
        // type is not the poll stage and is not recorded as one.
        poll_type: if *message_type == NewsletterMessageType::Poll {
            attrs
                .optional_string("polltype")
                .and_then(|s| PollType::try_from(s.as_ref()).ok())
        } else {
            None
        },
        content_type: attrs.optional_string("contenttype").map(|s| s.into_owned()),
        question_type: attrs
            .optional_string("questiontype")
            .map(|s| NewsletterQuestionType::from(s.as_ref())),
        message_association_type: attrs
            .optional_string("message_association_type")
            .map(|s| NewsletterMessageAssociationType::from(s.as_ref())),
        is_wamo_sub: attrs
            .optional_string("is_wamo_sub")
            .is_some_and(|s| s == "true"),
        admin_profile: meta_node
            .get_optional_child("admin_profile")
            .map(parse_admin_profile_node),
    }
}

/// `<admin_profile id="…"><name>…</name><picture id="…" direct_path="…"/></admin_profile>`,
/// the node form of the profile the MEX responses deliver as JSON.
fn parse_admin_profile_node(node: &NodeRef<'_>) -> NewsletterAdminProfile {
    let picture = node.get_optional_child("picture");
    NewsletterAdminProfile {
        id: node.get_attr("id").map(|v| v.as_str().into_owned()),
        name: node
            .get_optional_child("name")
            .and_then(|n| n.content_as_string())
            .map(|s| s.to_string())
            .unwrap_or_default(),
        picture_id: picture
            .and_then(|p| p.get_attr("id"))
            .map(|v| v.as_str().into_owned()),
        picture_direct_path: picture
            .and_then(|p| p.get_attr("direct_path"))
            .map(|v| v.as_str().into_owned()),
    }
}

/// Build the MEX variables for `update_newsletter_user_setting`. WA Web
/// (WAWebNewsletterUpdateUserSettingJob) sends `{ input: { newsletter_id, type, value } }`
/// with value ON/OFF; the mute-expiration is local DB state, never on the wire. The
/// generated op's input type is opaque (a bare string), so the structured object is
/// passed directly as variables.
/// Build the MEX variables for a picture-only `update_newsletter`. WA Web edits
/// each field through a "changed?" flag whose null value collapses to an empty
/// string, which is what clears the picture server-side.
fn picture_update_variables(jid: &Jid, jpeg: Option<&[u8]>) -> update_newsletter::Variables {
    use base64::Engine as _;

    let picture = jpeg
        .map(|bytes| base64::engine::general_purpose::STANDARD.encode(bytes))
        .unwrap_or_default();
    update_newsletter::Variables {
        newsletter_id: Some(jid.to_string()),
        updates: Some(update_newsletter::Updates {
            name: None,
            description: None,
            picture: Some(picture),
            settings: None,
        }),
    }
}

/// Variables for `WAWebMexFetchNewsletterJobQuery`, addressing a channel by
/// `key`/`key_type` (`"JID"` or `"INVITE"`).
///
/// The three `false` flags ask for response blocks this client does not parse;
/// WA Web puts each behind a feature flag and sends the flag's value either
/// way. `view_role` is `GUEST` because WA Web maps an absent role to it.
fn newsletter_variables(key: &str, key_type: &str) -> fetch_newsletter::Variables {
    fetch_newsletter::Variables {
        input: Some(fetch_newsletter::Input {
            key: Some(key.to_string()),
            r#type: Some(key_type.to_string()),
            view_role: Some("GUEST".into()),
        }),
        fetch_viewer_metadata: Some(true),
        fetch_full_image: Some(true),
        fetch_creation_time: Some(true),
        fetch_pinned_messages: Some(false),
        fetch_status_metadata: Some(false),
        fetch_wamo_sub: Some(false),
    }
}

fn delete_variables(jid: &Jid) -> delete_newsletter::Variables {
    delete_newsletter::Variables {
        newsletter_id: Some(jid.to_string()),
    }
}

fn change_owner_variables(jid: &Jid, user: &Jid) -> change_newsletter_owner::Variables {
    change_newsletter_owner::Variables {
        newsletter_id: Some(jid.to_string()),
        user_id: Some(user.to_string()),
    }
}

fn demote_admin_variables(jid: &Jid, user: &Jid) -> demote_newsletter_admin::Variables {
    demote_newsletter_admin::Variables {
        newsletter_id: Some(jid.to_string()),
        user_id: Some(user.to_string()),
    }
}

fn admin_info_variables(jid: &Jid) -> fetch_newsletter_admin_info::Variables {
    fetch_newsletter_admin_info::Variables {
        newsletter_id: Some(jid.to_string()),
    }
}

fn followers_variables(jid: &Jid, count: u32) -> fetch_newsletter_followers::Variables {
    fetch_newsletter_followers::Variables {
        input: Some(fetch_newsletter_followers::Input {
            newsletter_id: Some(jid.to_string()),
            count: Some(count.into()),
        }),
    }
}

fn mute_user_setting_variables(jid: &Jid, mute_type: &str, muted: bool) -> serde_json::Value {
    serde_json::json!({
        "input": {
            "newsletter_id": jid.to_string(),
            "type": mute_type,
            "value": if muted { "ON" } else { "OFF" },
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wacore_binary::builder::NodeBuilder;

    /// Regression for the 400 the server answers a persisted query that omits a
    /// declared variable with. WhatsApp Web sends both flags unconditionally,
    /// as whatever their feature gates say.
    #[test]
    fn subscribed_list_request_declares_every_variable() {
        let request = mex_request!(fetch_all_newsletters_metadata {
            fetch_status_metadata: Some(false),
            fetch_wamo_sub: Some(false),
        });

        assert_eq!(
            request.missing_variables().expect("serialize"),
            Vec::<&str>::new()
        );
        assert_eq!(
            serde_json::to_value(&request.variables).expect("serialize"),
            json!({ "fetch_status_metadata": false, "fetch_wamo_sub": false })
        );
    }

    /// Same regression for the newsletter fetch, whose seven variables WA Web
    /// also writes out in full at every call site.
    #[test]
    fn newsletter_fetch_request_declares_every_variable() {
        for (key, key_type) in [
            ("120363000000000001@newsletter", "JID"),
            ("0029Vabcdefghij0123456789", "INVITE"),
        ] {
            let request = mex_request!(fetch_newsletter, newsletter_variables(key, key_type));

            assert_eq!(
                request.missing_variables().expect("serialize"),
                Vec::<&str>::new(),
                "{key_type}"
            );
            assert_eq!(
                serde_json::to_value(&request.variables).expect("serialize"),
                json!({
                    "input": { "key": key, "type": key_type, "view_role": "GUEST" },
                    "fetch_viewer_metadata": true,
                    "fetch_full_image": true,
                    "fetch_creation_time": true,
                    "fetch_pinned_messages": false,
                    "fetch_status_metadata": false,
                    "fetch_wamo_sub": false,
                })
            );
        }
    }

    /// Pins the omission that makes `updates` a tri-state. Dropping
    /// `skip_serializing_if` from the nested types would send `name` and
    /// `description` as nulls, which clear them: see `picture_update_variables`.
    #[test]
    fn a_picture_update_does_not_touch_the_name_or_description() {
        for jpeg in [Some(&b"\xff\xd8\xff"[..]), None] {
            let updates = serde_json::to_value(picture_update_variables(&newsletter_jid(), jpeg))
                .expect("serialize");
            let updates = &updates["updates"];

            assert!(updates.get("name").is_none(), "{updates}");
            assert!(updates.get("description").is_none(), "{updates}");
            assert!(updates.get("settings").is_none(), "{updates}");
            assert!(updates["picture"].is_string(), "{updates}");
        }
        // Clearing needs an explicit empty string, because an omitted key means
        // "leave alone" and the old picture would survive.
        let cleared = serde_json::to_value(picture_update_variables(&newsletter_jid(), None))
            .expect("serialize");
        assert_eq!(cleared["updates"]["picture"], json!(""));
    }

    #[test]
    fn mute_variables_match_wa_web_shape() {
        let jid: Jid = "111222333@newsletter".parse().unwrap();
        let on = mute_user_setting_variables(&jid, "MUTE_FOLLOWER_ACTIVITY", true);
        assert_eq!(on["input"]["newsletter_id"], "111222333@newsletter");
        assert_eq!(on["input"]["type"], "MUTE_FOLLOWER_ACTIVITY");
        assert_eq!(on["input"]["value"], "ON");

        let off = mute_user_setting_variables(&jid, "MUTE_ADMIN_ACTIVITY", false);
        assert_eq!(off["input"]["type"], "MUTE_ADMIN_ACTIVITY");
        assert_eq!(off["input"]["value"], "OFF");
    }

    fn newsletter_jid() -> Jid {
        "120363000000000001@newsletter".parse().expect("jid")
    }

    mod my_addons {
        use super::*;

        fn response_for(jid: &Jid, id: &str) -> wacore_binary::Node {
            NodeBuilder::new("iq")
                .attr("type", "result")
                .attr("id", id)
                .children([NodeBuilder::new("my_addons")
                    .children([NodeBuilder::new("messages")
                        .attr("jid", jid.clone())
                        .children([NodeBuilder::new("message")
                            .attr("server_id", 777u64)
                            .children([NodeBuilder::new("votes").attr("t", 5u64).build()])
                            .build()])
                        .build()])
                    .build()])
                .build()
        }

        #[tokio::test]
        async fn get_my_addons_sends_the_query_and_reads_the_answer() {
            let (client, transport) = crate::test_utils::create_iq_test_client().await;
            let jid = newsletter_jid();

            let request = {
                let client = client.clone();
                let jid = jid.clone();
                tokio::spawn(async move { client.newsletter().get_my_addons(&jid, 20).await })
            };

            let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
            let sent = sent.get();
            let my_addons = sent
                .get_optional_child("my_addons")
                .expect("my_addons query");
            assert_eq!(my_addons.attrs().optional_jid("jid"), Some(jid.clone()));
            let id = sent
                .attrs()
                .optional_string("id")
                .expect("iq id")
                .into_owned();

            let response = response_for(&jid, &id);
            crate::test_utils::answer_iq(&client, &id, &response).await;

            let addons = request.await.expect("task").expect("answered");
            assert_eq!(addons.len(), 1);
            assert_eq!(addons[0].server_id, 777);
        }

        #[tokio::test]
        async fn get_my_addons_refuses_a_non_newsletter_jid() {
            let (client, transport) = crate::test_utils::create_iq_test_client().await;
            let group: Jid = "120363000000000002@g.us".parse().expect("jid");

            assert!(matches!(
                client.newsletter().get_my_addons(&group, 20).await,
                Err(NewsletterError::InvalidRequest(_))
            ));
            assert_eq!(transport.sent_count(), 0);
        }
    }

    /// The two halves of the history request are load-bearing in opposite
    /// ways: addressed to the channel, or sent to the server without naming
    /// the channel, the server answers nothing at all — no `<iq type="error">`,
    /// no `<stream:error>`. The silent IQ then holds the keepalive ping back
    /// until the dead-socket watchdog reconnects the account, so a wrong shape
    /// here costs far more than a failed call.
    #[test]
    fn history_request_goes_to_the_server_and_names_the_channel() {
        let query = build_newsletter_messages_iq(&newsletter_jid(), 20, Some(777));

        assert_eq!(&query.to, crate::jid_utils::server_jid());
        let Some(NodeContent::Nodes(children)) = &query.content else {
            panic!("the query carries a <messages> child");
        };
        let node = children[0].as_node_ref();
        assert_eq!(node.tag.as_ref(), "messages");
        assert!(node.get_attr("type").is_some_and(|v| v == "jid"));
        assert!(
            node.get_attr("jid")
                .is_some_and(|v| v == newsletter_jid().to_string().as_str())
        );
        assert!(node.get_attr("count").is_some_and(|v| v == "20"));
        assert!(node.get_attr("before").is_some_and(|v| v == "777"));
    }

    /// `before` is a cursor into a previous page, so the first page omits it
    /// rather than sending a sentinel.
    #[test]
    fn history_request_omits_an_absent_cursor() {
        let query = build_newsletter_messages_iq(&newsletter_jid(), 5, None);

        let Some(NodeContent::Nodes(children)) = &query.content else {
            panic!("the query carries a <messages> child");
        };
        assert!(children[0].as_node_ref().get_attr("before").is_none());
    }

    /// `Some(0)` is a cursor like any other, not an absent one: a server_id of
    /// zero would be dropped by a builder that tested the number instead of the
    /// `Option`, and the caller would silently get the newest page back.
    #[test]
    fn history_request_keeps_a_zero_cursor() {
        let query = build_newsletter_messages_iq(&newsletter_jid(), 5, Some(0));

        let Some(NodeContent::Nodes(children)) = &query.content else {
            panic!("the query carries a <messages> child");
        };
        assert!(
            children[0]
                .as_node_ref()
                .get_attr("before")
                .is_some_and(|v| v == "0")
        );
    }

    /// Counts and cursors go on the wire as decimal text, so the extremes of
    /// both integer types have to survive the trip without a cast narrowing
    /// them.
    #[test]
    fn history_request_renders_its_bounds_as_decimal_text() {
        for (count, before, expected_count, expected_before) in [
            (0u32, u64::MAX, "0", "18446744073709551615"),
            (u32::MAX, 1u64, "4294967295", "1"),
        ] {
            let query = build_newsletter_messages_iq(&newsletter_jid(), count, Some(before));

            let Some(NodeContent::Nodes(children)) = &query.content else {
                panic!("the query carries a <messages> child");
            };
            let node = children[0].as_node_ref();
            assert!(node.get_attr("count").is_some_and(|v| v == expected_count));
            assert!(
                node.get_attr("before")
                    .is_some_and(|v| v == expected_before)
            );
        }
    }

    /// The channel is named by the node's `jid` attribute, never by the IQ's
    /// own `target`: those are two different addressing mechanisms, and the
    /// server-scoped handler reads the first one.
    #[test]
    fn history_request_is_a_newsletter_get_with_no_iq_target() {
        let query = build_newsletter_messages_iq(&newsletter_jid(), 10, None);

        assert_eq!(query.namespace, NEWSLETTER_XMLNS);
        assert_eq!(query.query_type, wacore::request::InfoQueryType::Get);
        assert!(query.target.is_none());
    }

    /// A result that answers with some other child is a protocol error, not an
    /// empty page — the same way a body-less result is. `<live_updates>` is the
    /// shape the thread-scoped handler answers, so it is exactly what a request
    /// misrouted back to the channel would be most likely to bring.
    #[test]
    fn a_response_carrying_another_child_is_an_error() {
        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("live_updates")
                .attr("duration", "90")
                .build()])
            .build();

        assert!(matches!(
            parse_newsletter_messages_response(&response.as_node_ref()),
            Err(NewsletterError::InvalidRequest(_))
        ));
    }

    fn user_lid() -> Jid {
        "111111111111111@lid".parse().expect("jid")
    }

    #[test]
    fn delete_variables_match_wa_web_shape() {
        let vars = serde_json::to_value(delete_variables(&newsletter_jid())).expect("serialize");
        assert_eq!(
            vars,
            json!({ "newsletter_id": "120363000000000001@newsletter" })
        );
    }

    #[test]
    fn delete_response_is_accepted_and_a_null_payload_is_an_error() {
        let ok = take_data_field(
            Some(json!({
                "xwa2_newsletter_delete_v2": {
                    "id": "120363000000000001@newsletter",
                    "state": { "type": "DELETED" }
                }
            })),
            "xwa2_newsletter_delete_v2",
        )
        .expect("delete payload");
        assert_eq!(ok["state"]["type"], "DELETED");

        assert!(matches!(
            take_data_field(
                Some(json!({ "xwa2_newsletter_delete_v2": null })),
                "xwa2_newsletter_delete_v2",
            ),
            Err(NewsletterError::InvalidRequest(_))
        ));
        assert!(matches!(
            take_data_field(Some(json!({})), "xwa2_newsletter_delete_v2"),
            Err(NewsletterError::InvalidRequest(_))
        ));
        assert!(matches!(
            take_data_field(None, "xwa2_newsletter_delete_v2"),
            Err(NewsletterError::InvalidRequest(_))
        ));
    }

    #[test]
    fn change_owner_variables_match_wa_web_shape() {
        let vars = serde_json::to_value(change_owner_variables(&newsletter_jid(), &user_lid()))
            .expect("serialize");
        assert_eq!(
            vars,
            json!({
                "newsletter_id": "120363000000000001@newsletter",
                "user_id": "111111111111111@lid",
            })
        );
    }

    #[test]
    fn change_owner_response_is_accepted_and_a_null_payload_is_an_error() {
        let ok = take_data_field(
            Some(json!({
                "xwa2_newsletter_change_owner": {
                    "__typename": "XWA2Newsletter",
                    "id": "120363000000000001@newsletter"
                }
            })),
            "xwa2_newsletter_change_owner",
        )
        .expect("change owner payload");
        assert_eq!(ok["id"], "120363000000000001@newsletter");

        assert!(matches!(
            take_data_field(
                Some(json!({ "xwa2_newsletter_change_owner": null })),
                "xwa2_newsletter_change_owner",
            ),
            Err(NewsletterError::InvalidRequest(_))
        ));
    }

    #[tokio::test]
    async fn admin_mutations_refuse_a_target_with_no_known_lid() {
        let client = crate::test_utils::create_test_client().await;
        let unmapped = Jid::pn("12025550111");

        // A disconnected client fails with `Iq(NotConnected)` once it reaches the
        // wire, so `InvalidRequest` here proves the target was rejected before it.
        let err = client
            .newsletter()
            .change_owner(&newsletter_jid(), &unmapped)
            .await
            .unwrap_err();
        assert!(matches!(err, NewsletterError::InvalidRequest(_)));

        let err = client
            .newsletter()
            .demote_admin(&newsletter_jid(), &unmapped)
            .await
            .unwrap_err();
        assert!(matches!(err, NewsletterError::InvalidRequest(_)));
    }

    #[test]
    fn demote_admin_variables_match_wa_web_shape() {
        let vars = serde_json::to_value(demote_admin_variables(&newsletter_jid(), &user_lid()))
            .expect("serialize");
        assert_eq!(
            vars,
            json!({
                "newsletter_id": "120363000000000001@newsletter",
                "user_id": "111111111111111@lid",
            })
        );
    }

    #[test]
    fn demote_admin_response_is_accepted_and_a_null_payload_is_an_error() {
        let ok = take_data_field(
            Some(json!({
                "xwa2_newsletter_admin_demote": {
                    "__typename": "XWA2Newsletter",
                    "id": "120363000000000001@newsletter"
                }
            })),
            "xwa2_newsletter_admin_demote",
        )
        .expect("demote payload");
        assert_eq!(ok["id"], "120363000000000001@newsletter");

        assert!(matches!(
            take_data_field(
                Some(json!({ "xwa2_newsletter_admin_demote": null })),
                "xwa2_newsletter_admin_demote",
            ),
            Err(NewsletterError::InvalidRequest(_))
        ));
    }

    #[test]
    fn admin_info_variables_match_wa_web_shape() {
        let vars =
            serde_json::to_value(admin_info_variables(&newsletter_jid())).expect("serialize");
        assert_eq!(
            vars,
            json!({ "newsletter_id": "120363000000000001@newsletter" })
        );
    }

    #[test]
    fn admin_info_carries_the_admin_count_and_profile() {
        let admin = take_data_field(
            Some(json!({
                "xwa2_newsletter_admin": {
                    "id": "120363000000000001@newsletter",
                    "admin_count": 3,
                    "admin_profile": {
                        "id": "9990001",
                        "name": "Test Admin",
                        "picture": { "id": "1700000000", "direct_path": "/v/t61.0-24/pic.enc" }
                    },
                    "admin_settings": { "admin_profiles_enabled": true }
                }
            })),
            "xwa2_newsletter_admin",
        )
        .expect("admin payload");

        let info = parse_newsletter_admin_info(&admin);
        assert_eq!(info.admin_count, Some(3));
        assert_eq!(info.admin_profiles_enabled, Some(true));
        let profile = info.admin_profile.expect("admin profile");
        assert_eq!(profile.name, "Test Admin");
        assert_eq!(profile.id.as_deref(), Some("9990001"));
        assert_eq!(profile.picture_id.as_deref(), Some("1700000000"));
    }

    #[test]
    fn admin_info_leaves_absent_fields_unset_and_a_null_payload_is_an_error() {
        // Non-admins get the envelope without a count; nothing here may fall
        // back to a zero that reads as "this newsletter has no admins".
        let info = parse_newsletter_admin_info(&json!({
            "id": "120363000000000001@newsletter",
            "admin_profile": { "id": "9990001" }
        }));
        assert_eq!(info.admin_count, None);
        assert_eq!(info.admin_profiles_enabled, None);
        assert!(info.admin_profile.is_none());

        assert!(matches!(
            take_data_field(
                Some(json!({ "xwa2_newsletter_admin": null })),
                "xwa2_newsletter_admin",
            ),
            Err(NewsletterError::InvalidRequest(_))
        ));
    }

    #[test]
    fn followers_variables_match_wa_web_shape() {
        let vars =
            serde_json::to_value(followers_variables(&newsletter_jid(), 100)).expect("serialize");
        assert_eq!(
            vars,
            json!({
                "input": {
                    "newsletter_id": "120363000000000001@newsletter",
                    "count": 100,
                }
            })
        );
    }

    #[test]
    fn followers_response_is_parsed_into_domain_types() {
        let followers = take_data_field(
            Some(json!({
                "xwa2_newsletter_followers": {
                    "followers": {
                        "edges": [
                            {
                                "node": {
                                    "id": "111111111111111@lid",
                                    "display_name": "Test Owner",
                                    "pn": "12025550111@s.whatsapp.net",
                                    "username_info": {
                                        "__typename": "XWA2Username",
                                        "username": "test.owner"
                                    }
                                },
                                "follow_time": "1700000000",
                                "role": "OWNER",
                                "admin_profile": {
                                    "id": "9990001",
                                    "name": "Test Owner",
                                    "picture": { "id": "1", "direct_path": "/v/t61.0-24/pic.enc" }
                                }
                            },
                            {
                                "node": {
                                    "id": "222222222222222@lid",
                                    "display_name": null,
                                    "pn": null,
                                    "username_info": null
                                },
                                "follow_time": 1700000001i64,
                                "role": "SUBSCRIBER",
                                "admin_profile": null
                            },
                            { "node": { "id": null }, "role": "SUBSCRIBER" }
                        ]
                    }
                }
            })),
            "xwa2_newsletter_followers",
        )
        .expect("followers payload");

        let followers = parse_newsletter_followers(&followers).expect("parsed");
        // The id-less edge is dropped: there is nobody to address.
        assert_eq!(followers.len(), 2);

        assert_eq!(followers[0].jid, user_lid());
        assert_eq!(followers[0].role, Some(NewsletterRole::Owner));
        assert_eq!(followers[0].follow_time, Some(1700000000));
        assert_eq!(followers[0].username.as_deref(), Some("test.owner"));
        assert_eq!(
            followers[0].phone_jid,
            Some("12025550111@s.whatsapp.net".parse().expect("jid"))
        );
        assert_eq!(
            followers[0].admin_profile.as_ref().map(|p| p.name.as_str()),
            Some("Test Owner")
        );

        assert_eq!(followers[1].role, Some(NewsletterRole::Subscriber));
        assert_eq!(followers[1].follow_time, Some(1700000001));
        assert!(followers[1].phone_jid.is_none());
        assert!(followers[1].display_name.is_none());
        assert!(followers[1].admin_profile.is_none());
    }

    #[test]
    fn followers_null_payload_is_an_error_and_an_absent_edge_list_is_empty() {
        assert!(matches!(
            take_data_field(
                Some(json!({ "xwa2_newsletter_followers": null })),
                "xwa2_newsletter_followers",
            ),
            Err(NewsletterError::InvalidRequest(_))
        ));

        let empty = parse_newsletter_followers(&json!({ "followers": null })).expect("parsed");
        assert!(empty.is_empty());
    }

    #[test]
    fn follower_roles_are_matched_case_insensitively() {
        // The follower list spells roles uppercase; viewer_metadata lowercase.
        assert_eq!(parse_newsletter_role("ADMIN"), Some(NewsletterRole::Admin));
        assert_eq!(parse_newsletter_role("admin"), Some(NewsletterRole::Admin));
        assert_eq!(parse_newsletter_role("GUEST"), Some(NewsletterRole::Guest));
        assert_eq!(parse_newsletter_role("unknown"), None);
    }

    #[test]
    fn viewer_mute_settings_are_parsed_from_viewer_metadata() {
        let metadata = parse_newsletter_metadata(&json!({
            "id": "120363000000000001@newsletter",
            "thread_metadata": { "name": { "text": "Test Channel" } },
            "viewer_metadata": {
                "role": "admin",
                "settings": [
                    { "type": "MUTE_ADMIN_ACTIVITY", "value": "ON" },
                    { "type": "MUTE_FOLLOWER_ACTIVITY", "value": "OFF" }
                ]
            }
        }))
        .expect("metadata");
        assert_eq!(metadata.role, Some(NewsletterRole::Admin));
        assert_eq!(metadata.muted, Some(true));
        assert_eq!(metadata.follower_activity_muted, Some(false));

        // A missing list, a missing entry and an unknown value all leave the
        // state unknown rather than guessing "unmuted".
        let unknown = parse_newsletter_metadata(&json!({
            "id": "120363000000000001@newsletter",
            "viewer_metadata": {
                "settings": [{ "type": "MUTE_ADMIN_ACTIVITY", "value": "SOMETIMES" }]
            }
        }))
        .expect("metadata");
        assert_eq!(unknown.muted, None);
        assert_eq!(unknown.follower_activity_muted, None);

        let absent = parse_newsletter_metadata(&json!({
            "id": "120363000000000001@newsletter"
        }))
        .expect("metadata");
        assert_eq!(absent.muted, None);
    }

    #[test]
    fn set_picture_sends_base64_and_remove_sends_an_empty_string() {
        let jid = newsletter_jid();
        let set = serde_json::to_value(picture_update_variables(&jid, Some(b"\xff\xd8jpeg-bytes")))
            .expect("serialize");
        assert_eq!(set["updates"]["picture"], "/9hqcGVnLWJ5dGVz");
        assert!(set["updates"].get("name").is_none());
        assert!(set["updates"].get("description").is_none());

        // Removal is an explicitly-present empty string. Dropping the field
        // instead would leave the current picture in place.
        let removed =
            serde_json::to_value(picture_update_variables(&jid, None)).expect("serialize");
        assert_eq!(removed["updates"]["picture"], "");
        assert!(removed["updates"].get("picture").is_some());
    }

    #[test]
    fn picture_update_response_is_metadata_and_a_null_payload_is_an_error() {
        let updated = take_data_field(
            Some(json!({
                "xwa2_newsletter_update": {
                    "id": "120363000000000001@newsletter",
                    "state": { "type": "ACTIVE" },
                    "thread_metadata": {
                        "name": { "text": "Test Channel" },
                        "description": { "text": "" },
                        "picture": { "direct_path": "/v/t61.0-24/pic.enc", "id": "1" },
                        "verification": "UNVERIFIED"
                    }
                }
            })),
            "xwa2_newsletter_update",
        )
        .expect("update payload");

        let metadata = parse_newsletter_metadata(&updated).expect("metadata");
        assert_eq!(metadata.jid, newsletter_jid());
        assert_eq!(metadata.name, "Test Channel");
        assert_eq!(metadata.picture_url.as_deref(), Some("/v/t61.0-24/pic.enc"));

        assert!(matches!(
            take_data_field(
                Some(json!({ "xwa2_newsletter_update": null })),
                "xwa2_newsletter_update",
            ),
            Err(NewsletterError::InvalidRequest(_))
        ));
    }

    #[test]
    fn test_missing_type_attribute_defaults_to_text() {
        let response = NodeBuilder::new("iq")
            .children([NodeBuilder::new("messages")
                .children([NodeBuilder::new("message")
                    .attr("server_id", "42")
                    .attr("t", "1700000000")
                    .build()])
                .build()])
            .build();

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].message_type, NewsletterMessageType::Text);
    }

    #[test]
    fn test_message_without_plaintext_is_kept_with_no_payload() {
        let response = NodeBuilder::new("iq")
            .children([NodeBuilder::new("messages")
                .children([NodeBuilder::new("message")
                    .attr("server_id", "42")
                    .attr("t", "1700000000")
                    .attr("type", "text")
                    .build()])
                .build()])
            .build();

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
        assert_eq!(msgs.len(), 1);
        assert!(msgs[0].message.is_none());
    }

    #[test]
    fn test_explicit_type_attribute_parsed() {
        let response = NodeBuilder::new("iq")
            .children([NodeBuilder::new("messages")
                .children([NodeBuilder::new("message")
                    .attr("server_id", "1")
                    .attr("t", "1700000000")
                    .attr("type", "media")
                    .build()])
                .build()])
            .build();

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
        assert_eq!(msgs[0].message_type, NewsletterMessageType::Media);
    }

    /// Wrap message nodes in the `<iq><messages>` envelope the server answers
    /// a history request with.
    fn history_response(messages: Vec<wacore_binary::Node>) -> wacore_binary::Node {
        NodeBuilder::new("iq")
            .children([NodeBuilder::new("messages")
                .attr("jid", "111222333444555666@newsletter")
                .children(messages)
                .build()])
            .build()
    }

    /// Every sibling of `<plaintext>` the server sends, on one message. The
    /// parser used to read two of them and drop the rest.
    #[test]
    fn full_message_reads_every_child() {
        let hash_a = wacore::poll::compute_option_hash("Option A");
        let hash_b = wacore::poll::compute_option_hash("Option B");

        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "907")
                .attr("id", "FICTIONALMSGID01")
                .attr("t", "1700000000")
                .attr("type", "poll")
                .children([
                    NodeBuilder::new("forwards_count")
                        .attr("count", "12")
                        .build(),
                    NodeBuilder::new("views_count").attr("count", "23").build(),
                    NodeBuilder::new("responses_count")
                        .attr("count", "31")
                        .build(),
                    NodeBuilder::new("rcat").bytes(vec![0xAAu8; 155]).build(),
                    NodeBuilder::new("meta")
                        .attr("polltype", "creation")
                        .attr("contenttype", "add_on")
                        .attr("questiontype", "question")
                        .attr("message_association_type", "media_poll")
                        .attr("is_wamo_sub", "true")
                        .build(),
                    NodeBuilder::new("votes")
                        .children([
                            NodeBuilder::new("vote")
                                .attr("count", "7")
                                .bytes(hash_a.to_vec())
                                .build(),
                            NodeBuilder::new("vote")
                                .attr("count", "3")
                                .bytes(hash_b.to_vec())
                                .build(),
                        ])
                        .build(),
                    NodeBuilder::new("reactions")
                        .children([NodeBuilder::new("reaction")
                            .attr("code", "👍")
                            .attr("count", "5")
                            .build()])
                        .build(),
                    NodeBuilder::new("plaintext")
                        .attr("mediatype", "image")
                        .bytes(Vec::new())
                        .build(),
                ])
                .build(),
        ]);

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
        let msg = &msgs[0];

        assert_eq!(msg.message_type, NewsletterMessageType::Poll);
        assert_eq!(msg.forwards_count, Some(12));
        assert_eq!(msg.views_count, Some(23));
        assert_eq!(msg.responses_count, Some(31));
        assert_eq!(msg.poll_type, Some(PollType::Creation));
        assert_eq!(msg.content_type.as_deref(), Some("add_on"));
        assert_eq!(msg.question_type, Some(NewsletterQuestionType::Question));
        assert_eq!(
            msg.message_association_type,
            Some(NewsletterMessageAssociationType::MediaPoll)
        );
        assert!(msg.is_wamo_sub);
        assert_eq!(msg.media_type, Some(NewsletterMediaType::Image));
        assert_eq!(msg.rcat.as_ref().map(Vec::len), Some(155));
        assert_eq!(
            msg.votes,
            vec![
                NewsletterPollVote {
                    option_hash: hash_a,
                    count: 7,
                },
                NewsletterPollVote {
                    option_hash: hash_b,
                    count: 3,
                },
            ]
        );
        assert_eq!(msg.reactions.len(), 1);
        // Distinct values protect the three sibling mappings from swaps.
        assert_eq!(msg.forwards_count, Some(12));
        assert_eq!(msg.views_count, Some(23));
        assert_eq!(msg.responses_count, Some(31));
    }

    /// A message with none of the optional children still parses, and every
    /// counter reads as absent rather than as zero.
    #[test]
    fn bare_message_leaves_every_counter_absent() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "1")
                .attr("id", "FICTIONALMSGID02")
                .attr("t", "1700000000")
                .attr("type", "text")
                .children([NodeBuilder::new("plaintext").bytes(Vec::new()).build()])
                .build(),
        ]);

        let msg = &parse_newsletter_messages_response(&response.as_node_ref()).unwrap()[0];

        assert_eq!(msg.forwards_count, None);
        assert_eq!(msg.views_count, None);
        assert_eq!(msg.responses_count, None);
        assert_eq!(msg.edit, EditAttribute::Empty);
        assert!(msg.votes.is_empty());
        assert!(msg.reactions.is_empty());
        assert_eq!(msg.poll_type, None);
        assert!(msg.admin_profile.is_none());
        assert!(msg.rcat.is_none());
        assert!(msg.media_type.is_none());
    }

    /// The two `<meta>` shapes are mutually exclusive: attributes, or a single
    /// `<admin_profile>` child. Reading the node by attribute alone drops the
    /// profile silently.
    #[test]
    fn meta_carries_either_attributes_or_an_admin_profile() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "2")
                .attr("t", "1700000000")
                .attr("type", "media")
                .children([
                    NodeBuilder::new("meta")
                        .children([NodeBuilder::new("admin_profile")
                            .attr("id", "fictional-profile-id")
                            .children([
                                NodeBuilder::new("name")
                                    .bytes(b"Fictional Admin".to_vec())
                                    .build(),
                                NodeBuilder::new("picture")
                                    .attr("id", "1")
                                    .attr("direct_path", "/v/t61.0-24/fictional.enc")
                                    .build(),
                            ])
                            .build()])
                        .build(),
                    NodeBuilder::new("plaintext").bytes(Vec::new()).build(),
                ])
                .build(),
            NodeBuilder::new("message")
                .attr("server_id", "3")
                .attr("t", "1700000100")
                .attr("type", "text")
                .children([
                    NodeBuilder::new("meta")
                        .attr("original_msg_t", "1700000000")
                        .attr("msg_edit_t", "1700000100000")
                        .build(),
                    NodeBuilder::new("plaintext").bytes(Vec::new()).build(),
                ])
                .build(),
        ]);

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();

        let profile = msgs[0].admin_profile.as_ref().expect("admin profile");
        assert_eq!(profile.name, "Fictional Admin");
        assert_eq!(profile.id.as_deref(), Some("fictional-profile-id"));
        assert_eq!(
            profile.picture_direct_path.as_deref(),
            Some("/v/t61.0-24/fictional.enc")
        );
        assert_eq!(msgs[0].original_timestamp, None);

        // Seconds and milliseconds, side by side in one node.
        assert_eq!(msgs[1].original_timestamp, Some(1_700_000_000));
        assert_eq!(msgs[1].last_edit_timestamp_ms, Some(1_700_000_100_000));
        assert!(msgs[1].admin_profile.is_none());
    }

    /// A revocation keeps the envelope and loses the body: `edit="8"`, no
    /// forward counter, an empty `<plaintext/>`.
    #[test]
    fn revoked_message_parses_with_no_payload() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "4")
                .attr("id", "FICTIONALMSGID03")
                .attr("t", "1700000200")
                .attr("type", "text")
                .attr("edit", "8")
                .children([
                    NodeBuilder::new("meta")
                        .attr("original_msg_t", "1700000000")
                        .build(),
                    // Explicit zero-length bytes are distinct from an
                    // omitted body and must not decode to Message::default().
                    NodeBuilder::new("plaintext").bytes(Vec::new()).build(),
                ])
                .build(),
        ]);

        let msg = &parse_newsletter_messages_response(&response.as_node_ref()).unwrap()[0];

        assert_eq!(msg.edit, EditAttribute::AdminRevoke);
        assert!(msg.message.is_none());
        assert_eq!(msg.forwards_count, None);
        assert_eq!(msg.original_timestamp, Some(1_700_000_000));
    }

    /// An edit is an attribute, not a message type.
    #[test]
    fn edited_message_keeps_its_content_type() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "5")
                .attr("t", "1700000300")
                .attr("type", "media")
                .attr("edit", "3")
                .children([NodeBuilder::new("plaintext")
                    .attr("mediatype", "video")
                    .bytes(Vec::new())
                    .build()])
                .build(),
        ]);

        let msg = &parse_newsletter_messages_response(&response.as_node_ref()).unwrap()[0];

        assert_eq!(msg.edit, EditAttribute::AdminEdit);
        assert_eq!(msg.message_type, NewsletterMessageType::Media);
        assert_eq!(msg.media_type, Some(NewsletterMediaType::Video));
    }

    /// `polltype` is scoped to poll envelopes, the same way WA Web scopes it
    /// for ordinary messages.
    #[test]
    fn polltype_is_ignored_off_a_poll_message() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "6")
                .attr("t", "1700000400")
                .attr("type", "text")
                .children([NodeBuilder::new("meta")
                    .attr("polltype", "creation")
                    .build()])
                .build(),
        ]);

        let msg = &parse_newsletter_messages_response(&response.as_node_ref()).unwrap()[0];
        assert_eq!(msg.poll_type, None);
    }

    /// A `<vote>` whose content is not a 32-byte digest, or whose required
    /// count is missing/malformed, is dropped rather than attributed to the
    /// wrong option or silently changed to zero.
    #[test]
    fn malformed_poll_votes_are_dropped() {
        let good = wacore::poll::compute_option_hash("Yes");
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "7")
                .attr("t", "1700000500")
                .attr("type", "poll")
                .children([NodeBuilder::new("votes")
                    .children([
                        NodeBuilder::new("vote")
                            .attr("count", "2")
                            .bytes(vec![0x01, 0x02, 0x03])
                            .build(),
                        NodeBuilder::new("vote").bytes(good.to_vec()).build(),
                        NodeBuilder::new("vote")
                            .attr("count", "many")
                            .bytes(good.to_vec())
                            .build(),
                        NodeBuilder::new("vote")
                            .attr("count", "9")
                            .bytes(good.to_vec())
                            .build(),
                    ])
                    .build()])
                .build(),
        ]);

        let msg = &parse_newsletter_messages_response(&response.as_node_ref()).unwrap()[0];
        assert_eq!(
            msg.votes,
            vec![NewsletterPollVote {
                option_hash: good,
                count: 9,
            }]
        );
    }

    /// An empty channel answers `<messages jid="…"/>` with no children at all.
    #[test]
    fn empty_messages_node_yields_no_messages() {
        let response = NodeBuilder::new("iq")
            .children([NodeBuilder::new("messages")
                .attr("jid", "111222333444555666@newsletter")
                .build()])
            .build();

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
        assert!(msgs.is_empty());
    }

    // ── Happy path ────────────────────────────────────────────────────────

    /// A page of history is parsed message by message, and nothing leaks
    /// sideways: each message keeps its own counters, its own poll stage and
    /// its own profile. A parser that hoisted a child lookup out of the loop
    /// would still pass every single-message test above.
    #[test]
    fn a_page_of_history_keeps_each_message_to_its_own_fields() {
        let hash = wacore::poll::compute_option_hash("Yes");
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "10")
                .attr("id", "FICTIONALMSGID10")
                .attr("t", "1700000000")
                .attr("type", "text")
                .children([
                    NodeBuilder::new("forwards_count")
                        .attr("count", "4")
                        .build(),
                    NodeBuilder::new("plaintext").bytes(Vec::new()).build(),
                ])
                .build(),
            NodeBuilder::new("message")
                .attr("server_id", "11")
                .attr("id", "FICTIONALMSGID11")
                .attr("t", "1700000100")
                .attr("type", "poll")
                .children([
                    NodeBuilder::new("meta")
                        .attr("polltype", "creation")
                        .build(),
                    NodeBuilder::new("votes")
                        .children([NodeBuilder::new("vote")
                            .attr("count", "2")
                            .bytes(hash.to_vec())
                            .build()])
                        .build(),
                    NodeBuilder::new("plaintext").bytes(Vec::new()).build(),
                ])
                .build(),
            NodeBuilder::new("message")
                .attr("server_id", "12")
                .attr("id", "FICTIONALMSGID12")
                .attr("t", "1700000200")
                .attr("type", "media")
                .children([NodeBuilder::new("plaintext")
                    .attr("mediatype", "gif")
                    .bytes(Vec::new())
                    .build()])
                .build(),
        ]);

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();

        assert_eq!(msgs.len(), 3);
        assert_eq!(
            msgs.iter().map(|m| m.server_id).collect::<Vec<_>>(),
            vec![10, 11, 12],
            "history keeps the server's order; pagination cursors depend on it"
        );

        assert_eq!(msgs[0].forwards_count, Some(4));
        assert!(msgs[0].votes.is_empty());
        assert_eq!(msgs[0].poll_type, None);

        assert_eq!(msgs[1].forwards_count, None);
        assert_eq!(msgs[1].poll_type, Some(PollType::Creation));
        assert_eq!(msgs[1].votes.len(), 1);

        assert_eq!(msgs[2].media_type, Some(NewsletterMediaType::Gif));
        assert!(msgs[2].votes.is_empty());
        assert_eq!(msgs[2].forwards_count, None);
    }

    /// `<views_count>` has two shapes upstream, the plain one whatsmeow reads
    /// and a newer one tagged `type="views"`. Both put the number in the same
    /// attribute, and both have to read.
    #[test]
    fn views_count_reads_in_both_wire_shapes() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "20")
                .attr("t", "1700000000")
                .attr("type", "text")
                .children([NodeBuilder::new("views_count").attr("count", "321").build()])
                .build(),
            NodeBuilder::new("message")
                .attr("server_id", "21")
                .attr("t", "1700000100")
                .attr("type", "text")
                .children([NodeBuilder::new("views_count")
                    .attr("type", "views")
                    .attr("count", "654")
                    .build()])
                .build(),
        ]);

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
        assert_eq!(msgs[0].views_count, Some(321));
        assert_eq!(msgs[1].views_count, Some(654));
    }

    // ── Unhappy path ──────────────────────────────────────────────────────

    /// `server_id` is what pagination and reactions key on, so a message
    /// without a usable one is skipped rather than given a made-up id. The
    /// well-formed messages around it still come through.
    #[test]
    fn a_message_without_a_usable_server_id_is_skipped() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("t", "1700000000")
                .attr("type", "text")
                .build(),
            NodeBuilder::new("message")
                .attr("server_id", "not-a-number")
                .attr("t", "1700000100")
                .attr("type", "text")
                .build(),
            NodeBuilder::new("message")
                .attr("server_id", "30")
                .attr("t", "1700000200")
                .attr("type", "text")
                .build(),
        ]);

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0].server_id, 30);
    }

    /// An IQ result with no `<messages>` child is a protocol error, not an
    /// empty page: reporting it as zero messages would read as "this channel
    /// has no history" and stop a pagination loop early.
    #[test]
    fn a_response_without_a_messages_node_is_an_error() {
        let response = NodeBuilder::new("iq").attr("type", "result").build();

        assert!(matches!(
            parse_newsletter_messages_response(&response.as_node_ref()),
            Err(NewsletterError::InvalidRequest(_))
        ));
    }

    /// A counter whose `count` is not a number reads as absent. The node is
    /// there but says nothing usable, and inventing a zero would claim the
    /// message was never forwarded.
    #[test]
    fn a_counter_with_an_unparsable_count_reads_as_absent() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "40")
                .attr("t", "1700000000")
                .attr("type", "text")
                .children([
                    NodeBuilder::new("forwards_count")
                        .attr("count", "many")
                        .build(),
                    NodeBuilder::new("views_count").build(),
                ])
                .build(),
        ]);

        let msg = &parse_newsletter_messages_response(&response.as_node_ref()).unwrap()[0];
        assert_eq!(msg.forwards_count, None);
        assert_eq!(msg.views_count, None);
    }

    // ── Regression ────────────────────────────────────────────────────────

    /// The parser must not depend on the order the children arrive in.
    ///
    /// The server sends them in a stable order (`forwards_count`, `rcat`,
    /// `meta`, `votes`, `reactions`, `plaintext`), which is exactly the kind
    /// of incidental regularity a reader starts relying on by accident. This
    /// builds the same message backwards.
    #[test]
    fn regression_child_order_does_not_change_what_is_read() {
        let hash = wacore::poll::compute_option_hash("Yes");
        let children_forward = [
            NodeBuilder::new("forwards_count")
                .attr("count", "9")
                .build(),
            NodeBuilder::new("meta")
                .attr("polltype", "creation")
                .build(),
            NodeBuilder::new("votes")
                .children([NodeBuilder::new("vote")
                    .attr("count", "3")
                    .bytes(hash.to_vec())
                    .build()])
                .build(),
            NodeBuilder::new("reactions")
                .children([NodeBuilder::new("reaction")
                    .attr("code", "👍")
                    .attr("count", "1")
                    .build()])
                .build(),
            NodeBuilder::new("plaintext")
                .attr("mediatype", "image")
                .bytes(Vec::new())
                .build(),
        ];
        let mut children_reversed = children_forward.clone();
        children_reversed.reverse();

        let parse = |children: [wacore_binary::Node; 5]| {
            let response = history_response(vec![
                NodeBuilder::new("message")
                    .attr("server_id", "50")
                    .attr("t", "1700000000")
                    .attr("type", "poll")
                    .children(children)
                    .build(),
            ]);
            let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
            let msg = &msgs[0];
            (
                msg.forwards_count,
                msg.poll_type,
                msg.votes.clone(),
                msg.reactions.len(),
                msg.media_type.clone(),
            )
        };

        assert_eq!(parse(children_forward), parse(children_reversed));
    }

    /// An unrecognized child must not cost the children we do know.
    ///
    /// WhatsApp adds counters to this stanza over time — `forwards_count` is
    /// itself recent enough that whatsmeow does not read it — so the next one
    /// will arrive as a sibling nobody here has heard of. It has to be
    /// ignorable, and the message has to survive it.
    #[test]
    fn regression_an_unknown_child_does_not_cost_the_known_ones() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "60")
                .attr("t", "1700000000")
                .attr("type", "text")
                .children([
                    NodeBuilder::new("shares_count").attr("count", "5").build(),
                    NodeBuilder::new("forwards_count")
                        .attr("count", "7")
                        .build(),
                    NodeBuilder::new("reactions")
                        .children([NodeBuilder::new("reaction")
                            .attr("code", "❤")
                            .attr("count", "2")
                            .build()])
                        .build(),
                    NodeBuilder::new("plaintext").bytes(Vec::new()).build(),
                ])
                .build(),
        ]);

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
        assert_eq!(msgs.len(), 1, "an unknown child must not drop the message");
        assert_eq!(msgs[0].forwards_count, Some(7));
        assert_eq!(msgs[0].reactions.len(), 1);
    }

    // ── Edge cases ────────────────────────────────────────────────────────

    /// A count of zero is a fact, not an absence.
    ///
    /// The server omits the node instead of sending `count="0"` (no zero
    /// appears in a capture of 267 messages), so this shape is unobserved —
    /// which is precisely why the distinction has to be pinned: if it ever
    /// does arrive, `Some(0)` must not collapse into `None`.
    #[test]
    fn a_zero_count_is_kept_apart_from_an_absent_one() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "70")
                .attr("t", "1700000000")
                .attr("type", "text")
                .children([NodeBuilder::new("forwards_count")
                    .attr("count", "0")
                    .build()])
                .build(),
            NodeBuilder::new("message")
                .attr("server_id", "71")
                .attr("t", "1700000100")
                .attr("type", "text")
                .build(),
        ]);

        let msgs = parse_newsletter_messages_response(&response.as_node_ref()).unwrap();
        assert_eq!(msgs[0].forwards_count, Some(0));
        assert_eq!(msgs[1].forwards_count, None);
    }

    /// A `type` this build does not model is kept verbatim instead of being
    /// forced into `Text`, and it does not enable the poll-only reads.
    #[test]
    fn an_unmodelled_message_type_is_kept_verbatim() {
        let response = history_response(vec![
            NodeBuilder::new("message")
                .attr("server_id", "80")
                .attr("t", "1700000000")
                .attr("type", "hologram")
                .children([NodeBuilder::new("meta")
                    .attr("polltype", "creation")
                    .build()])
                .build(),
        ]);

        let msg = &parse_newsletter_messages_response(&response.as_node_ref()).unwrap()[0];
        assert_eq!(
            msg.message_type,
            NewsletterMessageType::Other("hologram".to_string())
        );
        assert_eq!(
            msg.poll_type, None,
            "the poll stage is read for poll envelopes only"
        );
    }

    /// Option hashes of a real channel poll, with the name each was taken
    /// over. The names carry a variation selector and a ZWJ sequence, the
    /// bytes a retyped name loses.
    const JUST_THIS: &str = "🫠 Just this.";
    const JUST_THIS_HASH: &str = "107f7671b96fcff7c2524b29a031dfb5c0c6ea19c9cba6e47de723e5c6983c83";
    const GOOD_MORNING: &str = "☀️ GOOD MORNING EVERYONE LET'S GO!!!";
    const GOOD_MORNING_HASH: &str =
        "2e090fda1d75dab720e00f72f5b06d88020d56c637d25a5d64529b51faeb6acf";
    const MONDAYS: &str = "😶‍🌫️ Mondays should be illegal.";
    const MONDAYS_HASH: &str = "ea53ff01672231aa49905b2a74164330b2a006d6f731d1227171fd92d3779322";

    fn option_hash(hex_digest: &str) -> [u8; 32] {
        hex::decode(hex_digest)
            .expect("hex")
            .try_into()
            .expect("32 bytes")
    }

    fn vote_bytes(node: &wacore_binary::Node) -> Vec<Vec<u8>> {
        let votes = node.get_optional_child("votes").expect("votes child");
        votes
            .children()
            .unwrap_or_default()
            .iter()
            .map(|vote| {
                assert_eq!(vote.tag, "vote");
                assert!(vote.attrs.is_empty(), "a sent <vote> carries no attrs");
                match vote.content.as_ref() {
                    Some(NodeContent::Bytes(bytes)) => bytes.clone(),
                    other => panic!("a <vote> holds the raw digest, got {other:?}"),
                }
            })
            .collect()
    }

    /// The hash a vote sends is `compute_option_hash` over the option name,
    /// byte for byte: these are the digests the server tallied and acked.
    #[test]
    fn poll_vote_hashes_are_the_option_name_digests() {
        for (name, digest) in [
            (JUST_THIS, JUST_THIS_HASH),
            (GOOD_MORNING, GOOD_MORNING_HASH),
            (MONDAYS, MONDAYS_HASH),
        ] {
            assert_eq!(
                wacore::poll::compute_option_hash(name),
                option_hash(digest),
                "{name}"
            );
        }
    }

    /// The shape WA Web sent for one option: a plaintext `<message
    /// type="poll">` whose `server_id` is the poll's, `<meta polltype="vote">`
    /// with no `contenttype`, then `<votes>` with one bare `<vote>` digest.
    #[test]
    fn poll_vote_node_matches_the_wa_web_stanza() {
        let jid = newsletter_jid();
        let node = build_poll_vote_node(
            &jid,
            "3EB0000000000000000001",
            777,
            &[option_hash(JUST_THIS_HASH)],
        );

        assert_eq!(node.tag, "message");
        let mut attrs = node.attrs();
        assert_eq!(attrs.jid("to"), jid);
        assert_eq!(
            attrs.optional_string("id").unwrap(),
            "3EB0000000000000000001"
        );
        assert_eq!(attrs.optional_u64("server_id"), Some(777));
        assert_eq!(attrs.optional_string("type").unwrap(), "poll");
        assert_eq!(
            node.attrs.len(),
            4,
            "no attribute beyond the four WA Web sends"
        );

        let tags: Vec<_> = node
            .children()
            .expect("children")
            .iter()
            .map(|child| child.tag.as_ref())
            .collect();
        assert_eq!(
            tags,
            ["meta", "votes"],
            "meta precedes votes, as WA Web sends it"
        );

        let meta = node.get_optional_child("meta").expect("meta child");
        assert_eq!(meta.attrs().optional_string("polltype").unwrap(), "vote");
        assert_eq!(meta.attrs.len(), 1, "a vote's meta carries no contenttype");
        assert!(meta.content.is_none());

        assert_eq!(vote_bytes(&node), [option_hash(JUST_THIS_HASH).to_vec()]);
    }

    /// A multi-select vote is one stanza with a `<vote>` per option, in the
    /// order the caller gave them.
    #[test]
    fn poll_vote_node_sends_every_option_in_the_given_order() {
        for order in [
            [GOOD_MORNING_HASH, MONDAYS_HASH],
            [MONDAYS_HASH, GOOD_MORNING_HASH],
        ] {
            let hashes = order.map(option_hash);
            let node = build_poll_vote_node(&newsletter_jid(), "3EB0", 777, &hashes);
            assert_eq!(
                vote_bytes(&node),
                hashes.iter().map(|hash| hash.to_vec()).collect::<Vec<_>>()
            );
        }
    }

    /// Removing a vote is the same stanza with an empty `<votes>`: there is no
    /// separate revoke type or attribute.
    #[test]
    fn an_empty_poll_vote_removes_the_vote() {
        let node = build_poll_vote_node(&newsletter_jid(), "3EB0", 777, &[]);

        assert_eq!(node.attrs().optional_string("type").unwrap(), "poll");
        assert!(
            node.get_optional_child("meta").is_some(),
            "meta stays on a removal"
        );
        assert!(vote_bytes(&node).is_empty());
    }

    #[test]
    fn a_poll_vote_is_refused_past_the_option_cap_or_with_a_repeat() {
        let distinct: Vec<[u8; 32]> = (0..=MAX_POLL_VOTE_OPTIONS)
            .map(|i| wacore::poll::compute_option_hash(&i.to_string()))
            .collect();

        assert!(validate_poll_vote(&[]).is_ok());
        assert!(validate_poll_vote(&distinct[..MAX_POLL_VOTE_OPTIONS]).is_ok());
        assert!(matches!(
            validate_poll_vote(&distinct),
            Err(NewsletterError::InvalidRequest(_))
        ));

        let repeated = [
            option_hash(GOOD_MORNING_HASH),
            option_hash(MONDAYS_HASH),
            option_hash(GOOD_MORNING_HASH),
        ];
        assert!(matches!(
            validate_poll_vote(&repeated),
            Err(NewsletterError::InvalidRequest(_))
        ));
    }

    /// The id handed back is the one on the wire, since it is the caller's
    /// only way to match the server's ack to this vote.
    #[tokio::test]
    async fn send_poll_vote_returns_the_id_it_sent() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let jid = newsletter_jid();

        let id = client
            .newsletter()
            .send_poll_vote(&jid, 777, &[option_hash(JUST_THIS_HASH)])
            .await
            .expect("vote is sent");

        let sent = crate::test_utils::decode_sent_iq(&transport, 0).await;
        let sent = sent.get();
        assert_eq!(sent.tag.as_ref(), "message");
        let mut attrs = sent.attrs();
        assert_eq!(attrs.optional_string("id").unwrap(), id.as_str());
        assert_eq!(attrs.optional_u64("server_id"), Some(777));
        assert_eq!(attrs.jid("to"), jid);
    }

    /// A refused vote puts nothing on the wire.
    #[tokio::test]
    async fn send_poll_vote_refuses_a_non_newsletter_jid_or_a_bad_selection() {
        let (client, transport) = crate::test_utils::create_iq_test_client().await;
        let group: Jid = "120363000000000002@g.us".parse().expect("jid");
        let hash = option_hash(JUST_THIS_HASH);

        let not_a_channel = client
            .newsletter()
            .send_poll_vote(&group, 777, &[hash])
            .await;
        assert!(matches!(
            not_a_channel,
            Err(NewsletterError::InvalidRequest(_))
        ));

        let repeated = client
            .newsletter()
            .send_poll_vote(&newsletter_jid(), 777, &[hash, hash])
            .await;
        assert!(matches!(repeated, Err(NewsletterError::InvalidRequest(_))));

        assert_eq!(transport.sent_count(), 0);
    }
}
