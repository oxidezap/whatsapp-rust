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
use wacore::iq::newsletter::NEWSLETTER_XMLNS;
use wacore::request::InfoQuery;
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

#[derive(Debug, Clone, PartialEq, Eq, WireEnum)]
#[non_exhaustive]
pub enum NewsletterMessageType {
    #[wire = "text"]
    Text,
    #[wire = "media"]
    Media,
    #[wire = "reaction"]
    Reaction,
    #[wire = "revoke"]
    Revoke,
    #[wire = "poll_creation"]
    PollCreation,
    #[wire = "poll_vote"]
    PollVote,
    #[wire = "edit"]
    Edit,
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

/// A message from a newsletter's history.
#[derive(Debug, Clone)]
pub struct NewsletterMessage {
    /// Wire message id (the stanza `id`). This is what edit_message / revoke_message
    /// key on (NOT `server_id`). Empty if the server omitted it.
    pub message_id: String,
    /// Server-assigned message ID (monotonic, used for pagination cursors).
    pub server_id: u64,
    /// Message timestamp (Unix seconds).
    pub timestamp: u64,
    /// Message type (text, media, reaction, etc.).
    pub message_type: NewsletterMessageType,
    /// Whether the viewer is the sender.
    pub is_sender: bool,
    /// Decoded protobuf message (from `<plaintext>` bytes).
    pub message: Option<wa::Message>,
    /// Reaction counts on this message.
    pub reactions: Vec<NewsletterReactionCount>,
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
                ..Default::default()
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
            .query(mex_request!(fetch_newsletter {
                input: Some(fetch_newsletter::Input {
                    key: Some(jid.to_string()),
                    r#type: Some("JID".into()),
                    view_role: Some("GUEST".into()),
                }),
                fetch_viewer_metadata: Some(true),
                fetch_full_image: Some(true),
                fetch_creation_time: Some(true),
                ..Default::default()
            }))
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
    pub async fn set_follower_mute(&self, jid: &Jid, muted: bool) -> Result<(), NewsletterError> {
        self.set_user_setting_mute(jid, "MUTE_FOLLOWER_ACTIVITY", muted)
            .await
    }

    /// Mute or unmute a newsletter's admin-activity notifications
    /// (WA Web's `MUTE_ADMIN_ACTIVITY`). Only meaningful for owners/admins.
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
            .query(mex_request!(fetch_newsletter {
                input: Some(fetch_newsletter::Input {
                    key: Some(invite_code.to_string()),
                    r#type: Some("INVITE".into()),
                    view_role: Some("GUEST".into()),
                }),
                fetch_viewer_metadata: Some(true),
                fetch_full_image: Some(true),
                fetch_creation_time: Some(true),
                ..Default::default()
            }))
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

    /// Subscribe to live updates for a newsletter (reaction counts, message changes).
    ///
    /// The server will send `<notification type="newsletter">` stanzas with
    /// `<live_updates>` children, dispatched as `Event::NewsletterLiveUpdate`.
    /// Returns the subscription duration in seconds.
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
        let mut messages_node = NodeBuilder::new("messages").attr("count", count);
        if let Some(before_id) = before {
            messages_node = messages_node.attr("before", before_id);
        }

        let iq = InfoQuery::get(
            NEWSLETTER_XMLNS,
            jid.clone(),
            Some(NodeContent::Nodes(vec![messages_node.build()])),
        );

        let response = self.client.send_iq(iq).await?;
        parse_newsletter_messages_response(response.get())
    }
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
    })
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

// Node response parsing helpers

/// Parse the IQ response for newsletter message history.
///
/// Response format:
/// ```xml
/// <messages jid="NL_JID" t="TS">
///   <message id="..." server_id="123" t="TS" type="text" [is_sender="true"]>
///     <plaintext>...</plaintext>
///     <reactions><reaction code="👍" count="3"/></reactions>
///   </message>
/// </messages>
/// ```
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

        let is_sender = msg_node
            .get_attr("is_sender")
            .is_some_and(|v| v.as_str() == "true");

        // Decode <plaintext> protobuf bytes
        let message =
            msg_node
                .get_optional_child("plaintext")
                .and_then(|pt| match pt.content.as_ref() {
                    Some(NodeContentRef::Bytes(bytes)) => {
                        waproto::codec::message_decode(bytes.as_ref()).ok()
                    }
                    _ => None,
                });

        let reactions = parse_reaction_counts(msg_node);

        result.push(NewsletterMessage {
            message_id,
            server_id,
            timestamp,
            message_type,
            is_sender,
            message,
            reactions,
        });
    }

    Ok(result)
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
}
