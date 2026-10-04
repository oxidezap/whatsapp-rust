//! Offline downstream compile/link probe. Never connects or sends an IQ.
use std::{future::Future, pin::Pin, time::Duration};
use whatsapp_rust::features::{GroupError, GroupProfilePicture, PictureType};
use whatsapp_rust::{
    Client, ContactError, Pictures, ProfilePictureLookup, ProfilePictureRequest,
    ProfilePictureTarget, ProfilePictureType, async_trait, wacore_binary::Jid,
};

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
trait PictureHost {
    async fn picture(&self, jid: &Jid) -> Result<ProfilePictureLookup, ContactError>;
}

#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
impl PictureHost for Client {
    async fn picture(&self, jid: &Jid) -> Result<ProfilePictureLookup, ContactError> {
        self.pictures()
            .lookup(ProfilePictureRequest::new(
                ProfilePictureTarget::Contact(jid),
                ProfilePictureType::Full,
            ))
            .await
    }
}

#[cfg(not(target_arch = "wasm32"))]
type LookupFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ProfilePictureLookup, ContactError>> + Send + 'a>>;
#[cfg(target_arch = "wasm32")]
type LookupFuture<'a> =
    Pin<Box<dyn Future<Output = Result<ProfilePictureLookup, ContactError>> + 'a>>;

fn facade(client: &Client) -> Pictures<'_> {
    client.pictures()
}

fn lookup<'a>(client: &'a Client, request: ProfilePictureRequest<'a>) -> LookupFuture<'a> {
    Box::pin(async move { client.pictures().lookup(request).await })
}

async fn batch(client: &Client, jid: &Jid) -> Result<Vec<GroupProfilePicture>, GroupError> {
    client
        .groups()
        .get_profile_pictures(vec![jid.clone()], PictureType::Preview)
        .await
}

fn main() {
    let jid = Jid::group("15550000001-7");
    for target in [
        ProfilePictureTarget::Contact(&jid),
        ProfilePictureTarget::Group(&jid),
        ProfilePictureTarget::Community(&jid),
    ] {
        for size in [ProfilePictureType::Preview, ProfilePictureType::Full] {
            let request = ProfilePictureRequest::new(target, size)
                .existing_id(None) // Missing bytes: ask for fresh data, not cache confirmation.
                .common_gid(Some(&jid))
                .invite(Some("synthetic-invite"))
                .persona_id(Some("synthetic-persona"))
                .timeout(Some(Duration::from_secs(3)));
            std::hint::black_box(request);
        }
    }
    std::hint::black_box(facade);
    std::hint::black_box(lookup);
    std::hint::black_box(batch);
    std::hint::black_box(<Client as PictureHost>::picture);
    for state in [
        ProfilePictureLookup::Unchanged,
        ProfilePictureLookup::NotFound,
        ProfilePictureLookup::NotAuthorized,
    ] {
        assert!(state.into_found().is_none());
    }
}
