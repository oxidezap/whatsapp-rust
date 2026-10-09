//! Profile management for the user's own account.
//!
//! Provides APIs for changing push name (display name) and status text (about).

use super::chat_actions::AppStateError;
use crate::client::{Client, ClientError};
use crate::request::IqError;
use crate::store::commands::DeviceCommand;
use anyhow::Result;
use log::debug;
use thiserror::Error;
use wacore::iq::contacts::SetProfilePictureSpec;
use wacore::iq::profile::SetStatusTextSpec;
use wacore_binary::builder::NodeBuilder;

pub use wacore::iq::contacts::SetProfilePictureResponse;

/// Error returned by own-profile operations (push name, status text, picture).
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum ProfileError {
    /// An IQ to the server failed (status text / profile picture).
    #[error("{0}")]
    Iq(#[from] IqError),
    /// Connection/transport failure sending a stanza (push-name presence).
    #[error("{0}")]
    Client(#[from] ClientError),
    /// Picture data was empty; use the dedicated removal operation instead.
    #[error("picture data cannot be empty; use remove_profile_picture instead")]
    EmptyPicture,
    /// A provided argument is invalid (e.g. an empty push name).
    #[error("invalid argument: {0}")]
    InvalidArgument(String),
    /// Catch-all for internal failures with no dedicated variant.
    #[error("{0}")]
    Internal(#[from] anyhow::Error),
}

/// Result after the push-name presence was sent and the local name was updated.
///
/// A successful presence send is not a server acknowledgement. Cross-device
/// synchronization can remain pending, for example before pairing supplies keys.
#[derive(Debug)]
#[must_use = "check whether push-name app-state synchronization is still pending"]
#[non_exhaustive]
pub enum PushNameOutcome {
    /// The app-state synchronization completed too.
    Synced,
    /// Presence was sent and the local name was updated, but app-state sync failed.
    SyncPending {
        /// Original failure, retained so the host can decide whether to retry.
        source: AppStateError,
    },
}

/// Feature handle for profile operations.
pub struct Profile<'a> {
    client: &'a Client,
}

impl<'a> Profile<'a> {
    pub(crate) fn new(client: &'a Client) -> Self {
        Self { client }
    }

    /// Set the user's status text (about).
    ///
    /// Uses the stable IQ-based approach matching WhatsApp Web's `WAWebSetAboutJob`:
    /// ```xml
    /// <iq type="set" xmlns="status" to="s.whatsapp.net">
    ///   <status>Hello world!</status>
    /// </iq>
    /// ```
    ///
    /// Note: This sets the profile "About" text, not ephemeral text status updates.
    pub async fn set_status_text(&self, text: &str) -> Result<(), ProfileError> {
        debug!("Setting status text (length={})", text.len());

        self.client.execute(SetStatusTextSpec::new(text)).await?;

        Ok(())
    }

    /// Set the user's push name (display name).
    ///
    /// Updates the local device store, sends a presence stanza with the new name,
    /// and propagates the change via app state sync (`setting_pushName` mutation
    /// in the `critical_block` collection) for cross-device synchronization.
    ///
    /// Follows the two operations in WhatsApp Web's `WAWebPushNameBridge`:
    /// 1. Send `<presence name="..."/>` immediately (no type attribute)
    /// 2. Sync via app state mutation to `critical_block` collection
    ///
    /// ## Wire Format
    /// ```xml
    /// <presence name="New Name"/>
    /// ```
    /// Returns [`PushNameOutcome::SyncPending`] if presence was sent but sync
    /// failed. This is usable during pairing before app-state keys arrive.
    /// Retry only that step with [`Self::sync_push_name`] once ready. A presence
    /// transport failure returns an error and leaves the local name unchanged.
    pub async fn set_push_name(&self, name: &str) -> Result<PushNameOutcome, ProfileError> {
        if name.is_empty() {
            return Err(ProfileError::InvalidArgument(
                "push name cannot be empty".into(),
            ));
        }

        debug!("Setting push name (length={})", name.len());

        // Send presence with name only (no type attribute), matching WhatsApp Web's
        // WASmaxOutPresenceAvailabilityRequest which uses OPTIONAL for type.
        let node = NodeBuilder::new("presence").attr("name", name).build();
        self.client.send_node(node).await?;

        let sync_result = self.sync_push_name(name).await;

        // Persist only after the network send succeeds
        self.client
            .persistence_manager()
            .process_command(DeviceCommand::SetPushName(name.to_string()))
            .await;

        Ok(match sync_result {
            Ok(()) => PushNameOutcome::Synced,
            Err(source) => PushNameOutcome::SyncPending { source },
        })
    }

    /// Set the user's own profile picture.
    ///
    /// Sends a JPEG image as the new profile picture. The image should already
    /// be properly sized/cropped by the caller (WhatsApp typically uses 640x640).
    ///
    /// Empty data returns [`ProfileError::EmptyPicture`] before sending anything.
    /// Use [`Profile::remove_profile_picture`] to remove the picture explicitly.
    /// Only non-emptiness is checked; JPEG contents are not validated or transformed.
    ///
    /// ## Wire Format
    /// ```xml
    /// <iq type="set" xmlns="w:profile:picture" to="s.whatsapp.net">
    ///   <picture type="image">{jpeg bytes}</picture>
    /// </iq>
    /// ```
    pub async fn set_profile_picture(
        &self,
        image_data: Vec<u8>,
    ) -> Result<SetProfilePictureResponse, ProfileError> {
        if image_data.is_empty() {
            return Err(ProfileError::EmptyPicture);
        }
        debug!("Setting profile picture (size={} bytes)", image_data.len());
        Ok(self
            .client
            .execute(SetProfilePictureSpec::set_own(image_data))
            .await?)
    }

    /// Remove the user's own profile picture.
    pub async fn remove_profile_picture(&self) -> Result<SetProfilePictureResponse, ProfileError> {
        debug!("Removing profile picture");
        Ok(self
            .client
            .execute(SetProfilePictureSpec::remove_own())
            .await?)
    }

    /// Synchronize a push name through app state without resending presence or
    /// changing the local name.
    ///
    /// Use this to retry [`PushNameOutcome::SyncPending`] after keys or the
    /// connection become available. Pass the pending name only if it is still
    /// the desired name; do not replay it after a newer name has been set.
    pub async fn sync_push_name(&self, name: &str) -> Result<(), AppStateError> {
        if name.is_empty() {
            return Err(AppStateError::InvalidRequest(
                "push name cannot be empty".into(),
            ));
        }
        use wacore::appstate::schemas;
        use waproto::whatsapp as wa;

        let value = wa::SyncActionValue {
            push_name_setting: buffa::MessageField::some(wa::sync_action_value::PushNameSetting {
                name: Some(name.to_string()),
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        // setting_pushName's index has no args (collection/version come from the schema).
        self.client
            .send_app_state_action(&schemas::SETTING_PUSH_NAME, &[], &value)
            .await?;
        Ok(())
    }
}

impl Client {
    /// Access profile operations.
    pub fn profile(&self) -> Profile<'_> {
        Profile::new(self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_utils::{create_iq_test_client, decode_sent_iq};

    #[tokio::test]
    async fn missing_pairing_keys_returns_pending_and_preserves_presence_and_local_name() {
        let (client, transport) = create_iq_test_client().await;
        let outcome = client
            .profile()
            .set_push_name("Pending Name")
            .await
            .unwrap();
        let PushNameOutcome::SyncPending { source } = outcome else {
            panic!("a client without app-state keys cannot report complete sync");
        };
        assert!(matches!(source, AppStateError::InvalidRequest(ref reason)
            if reason == "no app state sync key available"));
        assert_eq!(client.push_name(), "Pending Name");
        assert_eq!(transport.sent_count(), 1);
        let presence = decode_sent_iq(&transport, 0).await;
        let presence = presence.get();
        assert_eq!(presence.tag, "presence");
        assert_eq!(presence.get_attr("name").unwrap().as_str(), "Pending Name");
        assert!(presence.get_attr("type").is_none());

        let error = client
            .profile()
            .sync_push_name("Pending Name")
            .await
            .unwrap_err();
        assert!(matches!(error, AppStateError::InvalidRequest(_)));
        assert_eq!(
            transport.sent_count(),
            1,
            "sync retry must not send presence"
        );
        assert_eq!(client.push_name(), "Pending Name");
    }

    #[tokio::test]
    async fn pending_sync_can_complete_without_resending_presence() {
        let (client, transport) = create_iq_test_client().await;
        assert!(matches!(
            client
                .profile()
                .set_push_name("Pairing Name")
                .await
                .unwrap(),
            PushNameOutcome::SyncPending { .. }
        ));
        let backend = client.persistence_manager.backend();
        backend
            .set_sync_key(
                b"profile-key",
                crate::store::traits::AppStateSyncKey {
                    key_data: vec![5u8; 32],
                    ..Default::default()
                },
            )
            .await
            .unwrap();
        backend
            .set_version(
                "critical_block",
                wacore::appstate::hash::HashState {
                    version: 7,
                    bootstrapped: true,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        // Every retry frame must be an IQ. A resent presence fails immediately.
        let responses = async {
            let mut frame = 1;
            loop {
                let node = decode_sent_iq(&transport, frame).await;
                assert_eq!(node.get().tag, "iq");
                let id = node.get().get_attr("id").unwrap().as_str().into_owned();
                let response = NodeBuilder::new("iq")
                    .attr("type", "result")
                    .attr("id", id.as_str())
                    .attr("from", "s.whatsapp.net")
                    .children([NodeBuilder::new("sync")
                        .children([NodeBuilder::new("collection")
                            .attr("name", "critical_block")
                            .build()])
                        .build()])
                    .build();
                crate::test_utils::answer_iq(&client, &id, &response).await;
                frame += 1;
            }
        };
        let profile = client.profile();
        tokio::select! {
            result = profile.sync_push_name("Pairing Name") => result.unwrap(),
            () = responses => unreachable!(),
        }
        assert!(transport.sent_count() > 1);
        assert_eq!(client.push_name(), "Pairing Name");
    }

    #[tokio::test]
    async fn presence_transport_failure_returns_error_without_changing_local_name() {
        let (client, transport) = create_iq_test_client().await;
        let original = client.push_name();
        transport.fail_next_sends(1);
        let result = client.profile().set_push_name("Unsent Name").await;
        assert!(matches!(result, Err(ProfileError::Client(_))));
        assert_eq!(client.push_name(), original);
        assert_eq!(transport.failed_sends(), 1);
        assert_eq!(transport.sent_count(), 0);
    }

    #[tokio::test]
    async fn empty_names_are_rejected_before_any_network_or_local_changes() {
        let (client, transport) = create_iq_test_client().await;
        let original = client.push_name();
        assert!(matches!(
            client.profile().set_push_name("").await,
            Err(ProfileError::InvalidArgument(_))
        ));
        assert!(matches!(
            client.profile().sync_push_name("").await,
            Err(AppStateError::InvalidRequest(_))
        ));
        assert_eq!(client.push_name(), original);
        assert_eq!(transport.sent_count(), 0);
    }

    #[tokio::test]
    async fn sync_retry_sends_only_the_push_name_app_state_schema() {
        let mutation = super::super::chat_actions::capture_app_state_mutation(
            "critical_block",
            |client| async move { client.profile().sync_push_name("Retry Name").await },
        )
        .await;
        assert_eq!(mutation.index, vec!["setting_pushName"]);
        assert_eq!(
            mutation.operation,
            waproto::whatsapp::syncd_mutation::SyncdOperation::SET
        );
        assert_eq!(
            mutation
                .action_value
                .unwrap()
                .push_name_setting
                .as_option()
                .unwrap()
                .name
                .as_deref(),
            Some("Retry Name")
        );
    }
}
