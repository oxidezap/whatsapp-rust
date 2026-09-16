//! Contact-related IQ specifications.
//!
//! ## Profile Picture Wire Format
//! ```xml
//! <!-- Request (with optional tctoken for privacy gating) -->
//! <iq xmlns="w:profile:picture" type="get" to="s.whatsapp.net" target="1234567890@s.whatsapp.net" id="...">
//!   <picture type="preview" query="url">
//!     <tctoken><!-- raw token bytes (optional) --></tctoken>
//!   </picture>
//! </iq>
//!
//! <!-- Response (success) -->
//! <iq from="s.whatsapp.net" id="..." type="result">
//!   <picture id="123456789" url="https://..." direct_path="/v/..."/>
//! </iq>
//!
//! <!-- Response (not found) -->
//! <iq from="s.whatsapp.net" id="..." type="result">
//!   <picture>
//!     <error code="404" text="item-not-found"/>
//!   </picture>
//! </iq>
//! ```

use crate::iq::spec::IqSpec;
use crate::iq::tctoken::build_tc_token_node;
use crate::request::InfoQuery;
use anyhow::anyhow;
use std::time::Duration;
use wacore_binary::builder::NodeBuilder;
use wacore_binary::{Jid, Server};
use wacore_binary::{NodeContent, NodeRef};

/// Profile picture information.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfilePicture {
    pub id: String,
    pub url: String,
    pub direct_path: Option<String>,
    /// SHA-256 hash for integrity/cache validation.
    pub hash: Option<String>,
}

/// Profile picture type (preview thumbnail or full-size).
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::WireEnum)]
pub enum ProfilePictureType {
    #[wire = "preview"]
    Preview,
    #[wire = "image"]
    Full,
}

/// The outcome of a profile picture query.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ProfilePictureLookup {
    /// A profile picture was found.
    Found(ProfilePicture),
    /// The profile picture has not changed since the provided `existing_id`.
    Unchanged,
    /// The entity has no profile picture set (e.g. 404 item-not-found, 204 no-content, or empty).
    NotFound,
    /// Not authorized to view the profile picture (e.g. 401 not-authorized, 403 forbidden, privacy settings).
    NotAuthorized,
    /// Rate limit reached for profile picture queries (429 rate-overlimit).
    RateOverlimit,
}

impl ProfilePictureLookup {
    /// Returns `Some(&ProfilePicture)` if found, or `None` otherwise.
    pub fn found(&self) -> Option<&ProfilePicture> {
        match self {
            Self::Found(pic) => Some(pic),
            _ => None,
        }
    }

    /// Converts into `Option<ProfilePicture>`, dropping non-found states.
    pub fn into_found(self) -> Option<ProfilePicture> {
        match self {
            Self::Found(pic) => Some(pic),
            _ => None,
        }
    }

    /// Returns `true` if the profile picture is found.
    pub fn is_found(&self) -> bool {
        matches!(self, Self::Found(_))
    }

    /// Returns `true` if the profile picture is unchanged.
    pub fn is_unchanged(&self) -> bool {
        matches!(self, Self::Unchanged)
    }

    /// Returns `true` if the profile picture is not found.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::NotFound)
    }

    /// Returns `true` if unauthorized to view the profile picture.
    pub fn is_not_authorized(&self) -> bool {
        matches!(self, Self::NotAuthorized)
    }

    /// Returns `true` if rate limited (429 rate-overlimit).
    pub fn is_rate_overlimit(&self) -> bool {
        matches!(self, Self::RateOverlimit)
    }
}

/// Fetches the profile picture URL for a given JID.
#[derive(Debug, Clone)]
pub struct ProfilePictureSpec {
    pub jid: Jid,
    pub picture_type: ProfilePictureType,
    /// Optional tctoken to include in the IQ for privacy gating.
    pub tc_token: Option<Vec<u8>>,
    /// Current known picture ID. When set, the server can skip re-sending
    /// if the picture hasn't changed (cache optimization).
    pub existing_id: Option<String>,
    /// Common group shared with the target contact (fallback when no tctoken).
    pub common_gid: Option<Jid>,
    /// Group invite code if querying a group not yet joined.
    pub invite: Option<String>,
    /// Persona ID for Meta AI bots.
    pub persona_id: Option<String>,
    /// Whether this is a community parent profile picture (w:g2).
    pub is_community: bool,
    /// Optional request timeout override.
    pub timeout: Option<Duration>,
}

impl ProfilePictureSpec {
    pub fn preview(jid: &Jid) -> Self {
        Self {
            jid: jid.clone(),
            picture_type: ProfilePictureType::Preview,
            tc_token: None,
            existing_id: None,
            common_gid: None,
            invite: None,
            persona_id: None,
            is_community: false,
            timeout: None,
        }
    }

    pub fn full(jid: &Jid) -> Self {
        Self {
            jid: jid.clone(),
            picture_type: ProfilePictureType::Full,
            tc_token: None,
            existing_id: None,
            common_gid: None,
            invite: None,
            persona_id: None,
            is_community: false,
            timeout: None,
        }
    }

    pub fn new(jid: &Jid, picture_type: ProfilePictureType) -> Self {
        Self {
            jid: jid.clone(),
            picture_type,
            tc_token: None,
            existing_id: None,
            common_gid: None,
            invite: None,
            persona_id: None,
            is_community: false,
            timeout: None,
        }
    }

    /// Create a spec querying a community parent profile picture (`w:g2`).
    pub fn community(jid: &Jid, picture_type: ProfilePictureType) -> Self {
        Self {
            jid: jid.clone(),
            picture_type,
            tc_token: None,
            existing_id: None,
            common_gid: None,
            invite: None,
            persona_id: None,
            is_community: true,
            timeout: None,
        }
    }

    /// Supply an existing picture ID for conditional lookup (status 304 / no data if unchanged).
    pub fn with_existing_id(mut self, id: impl Into<String>) -> Self {
        self.existing_id = Some(id.into());
        self
    }

    /// Supply a common group JID shared with the contact.
    pub fn with_common_gid(mut self, gid: Jid) -> Self {
        self.common_gid = Some(gid);
        self
    }

    /// Supply an invite code when querying an unjoined group.
    pub fn with_invite(mut self, invite: impl Into<String>) -> Self {
        self.invite = Some(invite.into());
        self
    }

    /// Supply a persona ID when querying a Meta AI bot.
    pub fn with_persona_id(mut self, persona_id: impl Into<String>) -> Self {
        self.persona_id = Some(persona_id.into());
        self
    }

    /// Mark whether this query is for a community parent (`w:g2`).
    pub fn with_community(mut self, is_community: bool) -> Self {
        self.is_community = is_community;
        self
    }

    /// Include a tctoken in the profile picture IQ for privacy gating.
    pub fn with_tc_token(mut self, token: Vec<u8>) -> Self {
        self.tc_token = Some(token);
        self
    }

    /// Override the default request timeout.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }
}

impl IqSpec for ProfilePictureSpec {
    type Response = ProfilePictureLookup;

    fn build_iq(&self) -> InfoQuery<'static> {
        if self.is_community {
            let mut picture_builder = NodeBuilder::new("picture")
                .attr("parent_group_jid", &self.jid)
                .attr("type", self.picture_type.as_str())
                .attr("query", "url");

            if let Some(id) = &self.existing_id {
                picture_builder = picture_builder.attr("id", id);
            }

            let pictures_node = NodeBuilder::new("pictures")
                .children([picture_builder.build()])
                .build();

            let query = InfoQuery::get(
                "w:g2",
                self.jid.clone(),
                Some(NodeContent::Nodes(vec![pictures_node])),
            );
            match self.timeout {
                Some(timeout) => query.with_timeout(timeout),
                None => query,
            }
        } else {
            let mut picture_builder = NodeBuilder::new("picture")
                .attr("type", self.picture_type.as_str())
                .attr("query", "url");

            if let Some(id) = &self.existing_id {
                picture_builder = picture_builder.attr("id", id);
            }

            if let Some(common_gid) = &self.common_gid {
                picture_builder = picture_builder.attr("common_gid", common_gid);
            }

            if let Some(invite) = &self.invite {
                picture_builder = picture_builder.attr("invite", invite);
            }

            if let Some(persona_id) = &self.persona_id {
                picture_builder = picture_builder.attr("persona_id", persona_id);
            }

            // tctoken is a child of <picture>, matching WhatsApp Web's mixin merge pattern
            if let Some(token) = &self.tc_token {
                picture_builder = picture_builder.children([build_tc_token_node(token)]);
            }

            let query = InfoQuery::get(
                "w:profile:picture",
                Jid::new("", Server::Pn),
                Some(NodeContent::Nodes(vec![picture_builder.build()])),
            )
            .with_target_ref(&self.jid);
            match self.timeout {
                Some(timeout) => query.with_timeout(timeout),
                None => query,
            }
        }
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        // Support both direct <picture> (w:profile:picture) and wrapped <pictures><picture> (w:g2 community)
        let picture_node = if let Some(pictures_node) = response.get_optional_child("pictures") {
            pictures_node.get_optional_child("picture")
        } else {
            response.get_optional_child("picture")
        };

        let picture_node = match picture_node {
            Some(p) => p,
            // Empty <iq type="result"/> (no <picture> node = GetResponseSuccessNoData):
            // On conditional lookup with existing_id, WhatsApp Web server sends no data on unchanged.
            None => {
                return if self.existing_id.is_some() {
                    Ok(ProfilePictureLookup::Unchanged)
                } else {
                    Ok(ProfilePictureLookup::NotFound)
                };
            }
        };

        // Check for error response inside <picture>
        if let Some(error_node) = picture_node.get_optional_child("error") {
            let code = error_node.attrs().optional_string("code");
            let code_str = code.as_deref().unwrap_or("0");
            if code_str == "404" {
                return Ok(ProfilePictureLookup::NotFound);
            }
            if code_str == "401" || code_str == "403" {
                return Ok(ProfilePictureLookup::NotAuthorized);
            }
            if code_str == "429" {
                return Ok(ProfilePictureLookup::RateOverlimit);
            }
            let text = error_node.attrs().optional_string("text");
            let text_str = text.as_deref().unwrap_or("unknown error");
            return Err(anyhow!("Profile picture error {}: {}", code_str, text_str));
        }

        // Check status attribute on <picture> (e.g. status="304" unchanged, status="204" not found)
        if let Some(status) = picture_node.attrs().optional_string("status") {
            match status.as_ref() {
                "304" => return Ok(ProfilePictureLookup::Unchanged),
                "204" => return Ok(ProfilePictureLookup::NotFound),
                "500" => return Err(anyhow!("Profile picture server error: status 500")),
                "405" => return Err(anyhow!("Profile picture not allowed: status 405")),
                _ => {}
            }
        }

        let id = match picture_node.attrs().optional_string("id") {
            Some(s) => s.to_string(),
            // Empty <picture/> with no attributes = cache hit (picture unchanged)
            None => {
                return if self.existing_id.is_some() {
                    Ok(ProfilePictureLookup::Unchanged)
                } else {
                    Ok(ProfilePictureLookup::NotFound)
                };
            }
        };

        let url = match picture_node.attrs().optional_string("url") {
            Some(s) => s.to_string(),
            // <picture id="..."/> with no url = cache hit variant (picture unchanged)
            None => {
                return if self.existing_id.is_some() {
                    Ok(ProfilePictureLookup::Unchanged)
                } else {
                    Ok(ProfilePictureLookup::NotFound)
                };
            }
        };

        let direct_path = picture_node
            .attrs()
            .optional_string("direct_path")
            .map(|s| s.to_string());

        let hash = picture_node
            .attrs()
            .optional_string("hash")
            .map(|s| s.to_string());

        Ok(ProfilePictureLookup::Found(ProfilePicture {
            id,
            url,
            direct_path,
            hash,
        }))
    }
}

/// Response from setting a profile picture.
#[derive(Debug, Clone)]
pub struct SetProfilePictureResponse {
    /// The server-assigned picture ID.
    pub id: String,
}

/// Sets or removes a profile picture.
///
/// ## Wire Format (Set)
/// ```xml
/// <iq xmlns="w:profile:picture" type="set" to="s.whatsapp.net" id="...">
///   <picture type="image">{binary image data}</picture>
/// </iq>
/// ```
///
/// ## Wire Format (Remove)
/// ```xml
/// <iq xmlns="w:profile:picture" type="set" to="s.whatsapp.net" id="..."/>
/// ```
/// No `<picture>` child: `WAWebSendProfilePictureJob` emits an empty IQ.
///
/// ## Response
/// ```xml
/// <iq type="result" from="s.whatsapp.net" id="...">
///   <picture id="123456789"/>
/// </iq>
/// ```
#[derive(Debug, Clone)]
pub struct SetProfilePictureSpec {
    /// If Some, set picture for a group. If None, set for self.
    pub target: Option<Jid>,
    /// Image bytes. None means remove the picture.
    pub image_data: Option<Vec<u8>>,
}

impl SetProfilePictureSpec {
    /// Set own profile picture. Panics if `image_data` is empty (use `remove_own` instead).
    pub fn set_own(image_data: Vec<u8>) -> Self {
        assert!(
            !image_data.is_empty(),
            "image_data cannot be empty; use remove_own() to delete"
        );
        Self {
            target: None,
            image_data: Some(image_data),
        }
    }

    /// Remove own profile picture.
    pub fn remove_own() -> Self {
        Self {
            target: None,
            image_data: None,
        }
    }

    /// Set or remove the own picture from caller-supplied bytes. Empty bytes mean
    /// remove, matching WAWebSendProfilePictureJob (`a ? wap("picture",..) : null`).
    pub fn for_own(image_data: Vec<u8>) -> Self {
        if image_data.is_empty() {
            Self::remove_own()
        } else {
            Self::set_own(image_data)
        }
    }

    /// Set a group's profile picture. Panics if `image_data` is empty (use `remove_group` instead).
    pub fn set_group(group_jid: &Jid, image_data: Vec<u8>) -> Self {
        assert!(
            !image_data.is_empty(),
            "image_data cannot be empty; use remove_group() to delete"
        );
        Self {
            target: Some(group_jid.clone()),
            image_data: Some(image_data),
        }
    }

    /// Remove a group's profile picture.
    pub fn remove_group(group_jid: &Jid) -> Self {
        Self {
            target: Some(group_jid.clone()),
            image_data: None,
        }
    }

    /// Set or remove a group's picture from caller-supplied bytes. Empty bytes
    /// mean remove, mirroring [`Self::for_own`].
    pub fn for_group(group_jid: &Jid, image_data: Vec<u8>) -> Self {
        if image_data.is_empty() {
            Self::remove_group(group_jid)
        } else {
            Self::set_group(group_jid, image_data)
        }
    }
}

impl IqSpec for SetProfilePictureSpec {
    type Response = SetProfilePictureResponse;

    fn build_iq(&self) -> InfoQuery<'static> {
        // WAWebSendProfilePictureJob: emits `<picture type="image">{bytes}</picture>`
        // on set, NO `<picture>` child on remove.
        let content = self.image_data.as_ref().map(|data| {
            NodeContent::Nodes(vec![
                NodeBuilder::new("picture")
                    .attr("type", "image")
                    .bytes(data.clone())
                    .build(),
            ])
        });

        let mut iq = InfoQuery::set("w:profile:picture", Jid::new("", Server::Pn), content);

        if let Some(target) = &self.target {
            iq = iq.with_target_ref(target);
        }

        iq
    }

    fn parse_response(&self, response: &NodeRef<'_>) -> Result<Self::Response, anyhow::Error> {
        if self.image_data.is_some() {
            // Set operation: server must return <picture id="..."/>
            let picture_node = response
                .get_optional_child("picture")
                .ok_or_else(|| anyhow!("Set picture response missing 'picture' child"))?;
            let id = picture_node
                .attrs()
                .optional_string("id")
                .map(|s| s.to_string())
                .ok_or_else(|| anyhow!("Set picture response missing 'id' attribute"))?;
            Ok(SetProfilePictureResponse { id })
        } else {
            // Remove operation: server may return an empty result
            let id = response
                .get_optional_child("picture")
                .and_then(|p| p.attrs().optional_string("id").map(|s| s.to_string()))
                .unwrap_or_default();
            Ok(SetProfilePictureResponse { id })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn for_own_routes_empty_to_remove() {
        // WA Web treats empty bytes as a removal.
        assert!(
            SetProfilePictureSpec::for_own(Vec::new())
                .image_data
                .is_none()
        );
        assert_eq!(
            SetProfilePictureSpec::for_own(vec![1, 2, 3]).image_data,
            Some(vec![1, 2, 3])
        );
    }

    #[test]
    fn for_group_routes_empty_to_remove() {
        let group_jid: Jid = "123456789@g.us".parse().unwrap();
        let removed = SetProfilePictureSpec::for_group(&group_jid, Vec::new());
        assert!(removed.image_data.is_none());
        assert_eq!(removed.target.as_ref(), Some(&group_jid));

        let set = SetProfilePictureSpec::for_group(&group_jid, vec![1, 2, 3]);
        assert_eq!(set.image_data, Some(vec![1, 2, 3]));
        assert_eq!(set.target.as_ref(), Some(&group_jid));
    }

    #[test]
    fn test_profile_picture_spec_preview() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid);

        assert_eq!(spec.picture_type, ProfilePictureType::Preview);

        let iq = spec.build_iq();
        assert_eq!(iq.namespace, "w:profile:picture");
        assert_eq!(iq.target, Some(jid));

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert_eq!(nodes[0].tag, "picture");
            assert!(nodes[0].attrs.get("type").is_some_and(|s| s == "preview"));
        }
    }

    #[test]
    fn test_profile_picture_spec_full() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::full(&jid);

        assert_eq!(spec.picture_type, ProfilePictureType::Full);

        let iq = spec.build_iq();
        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert!(nodes[0].attrs.get("type").is_some_and(|s| s == "image"));
        }
    }

    #[test]
    fn test_profile_picture_spec_timeout_override() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let timeout = Duration::from_millis(1_250);
        let iq = ProfilePictureSpec::preview(&jid)
            .with_timeout(timeout)
            .build_iq();
        assert_eq!(iq.timeout, Some(timeout));
    }

    #[test]
    fn test_profile_picture_spec_parse_success() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid);

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture")
                .attr("id", "123456789")
                .attr("url", "https://example.com/pic.jpg")
                .attr("direct_path", "/v/pic.jpg")
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert!(matches!(result, ProfilePictureLookup::Found(_)));

        let pic = result.into_found().unwrap();
        assert_eq!(pic.id, "123456789");
        assert_eq!(pic.url, "https://example.com/pic.jpg");
        assert_eq!(pic.direct_path, Some("/v/pic.jpg".to_string()));
    }

    #[test]
    fn test_profile_picture_spec_parse_not_found() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid);

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture")
                .children([NodeBuilder::new("error")
                    .attr("code", "404")
                    .attr("text", "item-not-found")
                    .build()])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result, ProfilePictureLookup::NotFound);
    }

    #[test]
    fn test_profile_picture_spec_parse_not_authorized() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid);

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture")
                .children([NodeBuilder::new("error")
                    .attr("code", "401")
                    .attr("text", "not-authorized")
                    .build()])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result, ProfilePictureLookup::NotAuthorized);
    }

    #[test]
    fn test_profile_picture_spec_parse_unchanged_conditional() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid).with_existing_id("9999");

        // Case 1: empty <iq type="result"/> (GetResponseSuccessNoData in whatspec)
        let response_nodata = NodeBuilder::new("iq").attr("type", "result").build();
        let result = spec.parse_response(&response_nodata.as_node_ref()).unwrap();
        assert_eq!(result, ProfilePictureLookup::Unchanged);

        // Case 2: <picture status="304"/>
        let response_304 = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture").attr("status", "304").build()])
            .build();
        let result_304 = spec.parse_response(&response_304.as_node_ref()).unwrap();
        assert_eq!(result_304, ProfilePictureLookup::Unchanged);

        // Case 3: <picture id="9999"/> without url
        let response_no_url = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture").attr("id", "9999").build()])
            .build();
        let result_no_url = spec.parse_response(&response_no_url.as_node_ref()).unwrap();
        assert_eq!(result_no_url, ProfilePictureLookup::Unchanged);
    }

    #[test]
    fn test_profile_picture_spec_parse_status_204_not_found() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid);

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture").attr("status", "204").build()])
            .build();
        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result, ProfilePictureLookup::NotFound);
    }

    #[test]
    fn test_profile_picture_spec_parse_error_codes() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid);

        // 404 item-not-found
        let resp_404 = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture")
                .children([NodeBuilder::new("error")
                    .attr("code", "404")
                    .attr("text", "item-not-found")
                    .build()])
                .build()])
            .build();
        assert_eq!(
            spec.parse_response(&resp_404.as_node_ref()).unwrap(),
            ProfilePictureLookup::NotFound
        );

        // 401 not-authorized (privacy token missing / expired / privacy settings)
        let resp_401 = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture")
                .children([NodeBuilder::new("error")
                    .attr("code", "401")
                    .attr("text", "not-authorized")
                    .build()])
                .build()])
            .build();
        assert_eq!(
            spec.parse_response(&resp_401.as_node_ref()).unwrap(),
            ProfilePictureLookup::NotAuthorized
        );

        // 429 rate-overlimit
        let resp_429 = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture")
                .children([NodeBuilder::new("error")
                    .attr("code", "429")
                    .attr("text", "rate-overlimit")
                    .build()])
                .build()])
            .build();
        assert_eq!(
            spec.parse_response(&resp_429.as_node_ref()).unwrap(),
            ProfilePictureLookup::RateOverlimit
        );
    }

    #[test]
    fn test_profile_picture_spec_community_wire_shape() {
        let community_jid: Jid = "120363024567890123@g.us".parse().unwrap();
        let spec = ProfilePictureSpec::community(&community_jid, ProfilePictureType::Full)
            .with_existing_id("curr-photo-id");

        let iq = spec.build_iq();
        assert_eq!(iq.namespace, "w:g2");
        assert_eq!(iq.to, community_jid);
        assert_eq!(iq.target, None);

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert_eq!(nodes[0].tag, "pictures");
            let picture_node = nodes[0]
                .get_optional_child("picture")
                .expect("picture child");
            assert_eq!(
                picture_node
                    .attrs
                    .get("parent_group_jid")
                    .map(|v| v.as_str())
                    .as_deref(),
                Some("120363024567890123@g.us")
            );
            assert_eq!(
                picture_node
                    .attrs
                    .get("type")
                    .map(|v| v.as_str())
                    .as_deref(),
                Some("image")
            );
            assert_eq!(
                picture_node
                    .attrs
                    .get("query")
                    .map(|v| v.as_str())
                    .as_deref(),
                Some("url")
            );
            assert_eq!(
                picture_node.attrs.get("id").map(|v| v.as_str()).as_deref(),
                Some("curr-photo-id")
            );
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_profile_picture_spec_community_parse_success() {
        let community_jid: Jid = "120363024567890123@g.us".parse().unwrap();
        let spec = ProfilePictureSpec::community(&community_jid, ProfilePictureType::Full);

        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("pictures")
                .children([NodeBuilder::new("picture")
                    .attr("parent_group_jid", "120363024567890123@g.us")
                    .attr("id", "comm-pic-1")
                    .attr("type", "image")
                    .attr("url", "https://example.com/comm.jpg")
                    .attr("direct_path", "/v/comm.jpg")
                    .build()])
                .build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert!(matches!(result, ProfilePictureLookup::Found(_)));
        let pic = result.into_found().unwrap();
        assert_eq!(pic.id, "comm-pic-1");
        assert_eq!(pic.url, "https://example.com/comm.jpg");
        assert_eq!(pic.direct_path, Some("/v/comm.jpg".to_string()));
    }

    #[test]
    fn test_profile_picture_spec_parse_no_picture_node() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid);

        let response = NodeBuilder::new("iq").attr("type", "result").build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result, ProfilePictureLookup::NotFound);
    }

    #[test]
    fn test_profile_picture_spec_with_tc_token() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid).with_tc_token(vec![0xCA, 0xFE, 0xBA, 0xBE]);

        let iq = spec.build_iq();
        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert_eq!(nodes.len(), 1, "IQ should have one child: picture");
            let picture = &nodes[0];
            assert_eq!(picture.tag, "picture");

            // tctoken is a child of picture (matching WhatsApp Web's mixin merge)
            let tctoken_children: Vec<_> = picture.get_children_by_tag("tctoken").collect();
            assert_eq!(tctoken_children.len(), 1);
            match &tctoken_children[0].content {
                Some(NodeContent::Bytes(data)) => {
                    assert_eq!(data, &[0xCA, 0xFE, 0xBA, 0xBE]);
                }
                _ => panic!("Expected binary content in tctoken node"),
            }
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_profile_picture_spec_without_tc_token() {
        let jid: Jid = "1234567890@s.whatsapp.net".parse().unwrap();
        let spec = ProfilePictureSpec::preview(&jid);

        let iq = spec.build_iq();
        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            assert_eq!(nodes.len(), 1, "IQ should have one child: picture");
            let picture = &nodes[0];
            assert_eq!(picture.tag, "picture");
            let tctoken_children: Vec<_> = picture.get_children_by_tag("tctoken").collect();
            assert_eq!(tctoken_children.len(), 0, "No tctoken without token");
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_set_profile_picture_spec_own() {
        let spec = SetProfilePictureSpec::set_own(vec![0xFF, 0xD8, 0xFF]);
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, "w:profile:picture");
        assert_eq!(iq.query_type.as_str(), "set");
        assert!(iq.target.is_none(), "Own picture should not have target");

        if let Some(NodeContent::Nodes(nodes)) = &iq.content {
            let picture = &nodes[0];
            assert_eq!(picture.tag, "picture");
            assert!(picture.attrs.get("type").is_some_and(|v| v == "image"));
            match &picture.content {
                Some(NodeContent::Bytes(data)) => {
                    assert_eq!(data, &[0xFF, 0xD8, 0xFF]);
                }
                _ => panic!("Expected binary content in picture node"),
            }
        } else {
            panic!("Expected NodeContent::Nodes");
        }
    }

    #[test]
    fn test_set_profile_picture_spec_group() {
        let group_jid: Jid = "123456789@g.us".parse().unwrap();
        let spec = SetProfilePictureSpec::set_group(&group_jid, vec![0x89, 0x50, 0x4E]);
        let iq = spec.build_iq();

        assert_eq!(iq.namespace, "w:profile:picture");
        assert_eq!(iq.target, Some(group_jid));
    }

    #[test]
    fn test_set_profile_picture_spec_remove_own_emits_no_picture_child() {
        // WAWebSendProfilePictureJob: removal IQ has no `<picture>` child at all.
        let spec = SetProfilePictureSpec::remove_own();
        let iq = spec.build_iq();
        assert!(
            iq.content.is_none(),
            "Remove must emit an empty <iq> with no <picture> child; got {:?}",
            iq.content
        );
    }

    #[test]
    fn test_set_profile_picture_spec_remove_group_emits_no_picture_child() {
        let group_jid: Jid = "123456789@g.us".parse().unwrap();
        let spec = SetProfilePictureSpec::remove_group(&group_jid);
        let iq = spec.build_iq();
        assert!(iq.content.is_none(), "Remove group: no <picture> child");
        assert_eq!(iq.target, Some(group_jid));
    }

    #[test]
    fn test_set_profile_picture_spec_parse_response() {
        let spec = SetProfilePictureSpec::set_own(vec![0xFF, 0xD8]);
        let response = NodeBuilder::new("iq")
            .attr("type", "result")
            .children([NodeBuilder::new("picture").attr("id", "987654321").build()])
            .build();

        let result = spec.parse_response(&response.as_node_ref()).unwrap();
        assert_eq!(result.id, "987654321");
    }
}
