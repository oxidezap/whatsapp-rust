//! Multi-domain picture lookup; mutations remain in their domain facades.

use super::contacts::ContactError;
use crate::{client::Client, request::IqError};
use std::time::Duration;
use wacore::iq::contacts::ProfilePictureSpec;

pub use wacore::iq::contacts::{ProfilePicture, ProfilePictureLookup, ProfilePictureType};
use wacore_binary::{Jid, JidExt};

/// Read profile pictures for contacts, groups, and community parents.
///
/// Setters and removals remain on [`crate::Profile`], [`crate::Groups`], and
/// [`crate::Community`]. Group batch lookup remains on [`crate::Groups`].
///
/// This replaces `Contacts::lookup_picture`; the old entry point is removed:
/// ```compile_fail,E0599
/// use whatsapp_rust::{Client, ContactError, ProfilePictureLookup,
///     ProfilePictureRequest, ProfilePictureTarget, ProfilePictureType};
/// use whatsapp_rust::wacore_binary::Jid;
/// async fn old_lookup(client: &Client, jid: &Jid) -> Result<ProfilePictureLookup, ContactError> {
///     client.contacts().lookup_picture(ProfilePictureRequest::new(
///         ProfilePictureTarget::Contact(jid), ProfilePictureType::Full,
///     )).await
/// }
/// ```
pub struct Pictures<'a> {
    client: &'a Client,
}

impl Pictures<'_> {
    /// Canonical lookup with explicit size/route and preserved rejection metadata.
    ///
    /// Found/Unchanged/NotFound/NotAuthorized remain distinct. A 429 is a
    /// [`ContactError::Iq`] wrapping [`IqError::ServerError`] with its original
    /// stanza and optional backoff. The existing error contract is preserved.
    /// There is no automatic community fallback. `into_found()` explicitly
    /// discards all non-found states; Unchanged says nothing about cached bytes.
    ///
    /// ```no_run
    /// # use whatsapp_rust::{Client, ContactError, ProfilePictureRequest, ProfilePictureTarget, ProfilePictureType};
    /// # use whatsapp_rust::wacore_binary::Jid;
    /// # async fn example(client: &Client, jid: &Jid) -> Result<(), ContactError> {
    /// let outcome = client.pictures().lookup(ProfilePictureRequest::new(
    ///     ProfilePictureTarget::Group(jid), ProfilePictureType::Full,
    /// )).await?;
    /// // Only when the consumer intentionally needs newly found URLs alone:
    /// let picture = outcome.into_found();
    /// # let _ = picture;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn lookup(
        &self,
        request: ProfilePictureRequest<'_>,
    ) -> Result<ProfilePictureLookup, ContactError> {
        Ok(lookup(self.client, request).await?)
    }
}

impl Client {
    /// Access picture lookup across contacts, groups, and communities.
    pub fn pictures(&self) -> Pictures<'_> {
        Pictures { client: self }
    }
}

/// Explicit picture route. No route triggers a fallback or metadata query.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum ProfilePictureTarget<'a> {
    /// `w:profile:picture`, with privacy-token discovery when applicable.
    Contact(&'a Jid),
    /// `w:profile:picture`, not the community-parent route.
    Group(&'a Jid),
    /// Community parent via `w:g2` and `<pictures>`.
    Community(&'a Jid),
}

impl<'a> ProfilePictureTarget<'a> {
    pub fn jid(self) -> &'a Jid {
        match self {
            Self::Contact(jid) | Self::Group(jid) | Self::Community(jid) => jid,
        }
    }
}

/// Canonical borrowed request with explicit size and route.
///
/// An `existing_id` permits `Unchanged`, which supplies no fresh URL and proves
/// nothing about locally cached bytes. Omit it when bytes are missing. A second
/// community lookup is a consumer policy, never an implicit network call here.
#[derive(Debug, Clone)]
#[non_exhaustive]
pub struct ProfilePictureRequest<'a> {
    target: ProfilePictureTarget<'a>,
    picture_type: ProfilePictureType,
    existing_id: Option<&'a str>,
    common_gid: Option<&'a Jid>,
    invite: Option<&'a str>,
    persona_id: Option<&'a str>,
    timeout: Option<Duration>,
}

impl<'a> ProfilePictureRequest<'a> {
    pub fn new(target: ProfilePictureTarget<'a>, picture_type: ProfilePictureType) -> Self {
        Self {
            target,
            picture_type,
            existing_id: None,
            common_gid: None,
            invite: None,
            persona_id: None,
            timeout: None,
        }
    }

    pub fn target(&self) -> ProfilePictureTarget<'a> {
        self.target
    }

    pub fn existing_id(mut self, id: Option<&'a str>) -> Self {
        self.existing_id = id;
        self
    }

    /// Shared-group privacy fallback when no contact token is available.
    pub fn common_gid(mut self, jid: Option<&'a Jid>) -> Self {
        self.common_gid = jid;
        self
    }

    /// Invite code on the profile-picture route; not sent on the community route.
    pub fn invite(mut self, invite: Option<&'a str>) -> Self {
        self.invite = invite;
        self
    }

    /// Persona on the profile-picture route; not sent on the community route.
    pub fn persona_id(mut self, id: Option<&'a str>) -> Self {
        self.persona_id = id;
        self
    }

    pub fn timeout(mut self, timeout: Option<Duration>) -> Self {
        self.timeout = timeout;
        self
    }

    fn spec(&self, token: Option<Vec<u8>>) -> ProfilePictureSpec {
        let mut spec = match self.target {
            ProfilePictureTarget::Community(jid) => {
                ProfilePictureSpec::community(jid, self.picture_type)
            }
            ProfilePictureTarget::Contact(jid) | ProfilePictureTarget::Group(jid) => {
                ProfilePictureSpec::new(jid, self.picture_type)
            }
        };
        if let Some(id) = self.existing_id {
            spec = spec.with_existing_id(id);
        }
        if let Some(timeout) = self.timeout {
            spec = spec.with_timeout(timeout);
        }
        if !matches!(self.target, ProfilePictureTarget::Community(_)) {
            if let Some(invite) = self.invite {
                spec = spec.with_invite(invite);
            }
            if let Some(id) = self.persona_id {
                spec = spec.with_persona_id(id);
            }
            // Preserve the existing token-first policy: common_gid is its fallback.
            if let Some(token) = token {
                spec = spec.with_tc_token(token);
            } else if let Some(jid) = self.common_gid {
                spec = spec.with_common_gid(jid.clone());
            }
        }
        spec
    }
}

fn token_eligible(target: ProfilePictureTarget<'_>, is_own: bool) -> bool {
    let jid = target.jid();
    matches!(target, ProfilePictureTarget::Contact(_))
        && !jid.is_group()
        && !jid.is_newsletter()
        && !jid.is_bot()
        && !jid.is_broadcast_list()
        && !jid.is_status_broadcast()
        && !is_own
}

pub(crate) async fn build_spec(
    client: &Client,
    request: &ProfilePictureRequest<'_>,
) -> ProfilePictureSpec {
    // Self never answers an IQ carrying its own tctoken.
    let token = if token_eligible(request.target, client.is_own_jid(request.target.jid()))
        && client
            .ab_props
            .is_enabled(wacore::iq::props::stale::PROFILE_PIC_PRIVACY_TOKEN)
            .await
    {
        client.lookup_tc_token_for_jid(request.target.jid()).await
    } else {
        None
    };
    request.spec(token)
}

pub(crate) fn classify(
    result: Result<ProfilePictureLookup, IqError>,
) -> Result<ProfilePictureLookup, IqError> {
    match result {
        Err(IqError::ServerError { code: 404, .. }) => Ok(ProfilePictureLookup::NotFound),
        Err(IqError::ServerError {
            code: 401 | 403, ..
        }) => Ok(ProfilePictureLookup::NotAuthorized),
        other => other,
    }
}

pub(crate) async fn lookup(
    client: &Client,
    request: ProfilePictureRequest<'_>,
) -> Result<ProfilePictureLookup, IqError> {
    // The system JID never answers this IQ, even with a timeout override.
    if request.target.jid().is_psa() {
        return Ok(ProfilePictureLookup::NotFound);
    }
    let spec = build_spec(client, &request).await;
    classify(client.execute(spec).await)
}

#[cfg(test)]
#[path = "pictures_tests.rs"]
mod tests;
