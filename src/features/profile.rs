//! Profile management for the user's own account.
//!
//! Provides APIs for changing push name (display name) and status text (about).

use crate::client::Client;
use crate::store::commands::DeviceCommand;
use anyhow::Result;
use log::{debug, warn};
use wacore::iq::contacts::SetProfilePictureSpec;
use wacore::iq::profile::SetStatusTextSpec;
use wacore_binary::builder::NodeBuilder;

pub use wacore::iq::contacts::SetProfilePictureResponse;

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
    pub async fn set_status_text(&self, text: &str) -> Result<()> {
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
    /// Matches WhatsApp Web's `WAWebPushNameBridge` behavior:
    /// 1. Send `<presence name="..."/>` immediately (no type attribute)
    /// 2. Sync via app state mutation to `critical_block` collection
    ///
    /// ## Wire Format
    /// ```xml
    /// <presence name="New Name"/>
    /// ```
    pub async fn set_push_name(&self, name: &str) -> Result<()> {
        if name.is_empty() {
            return Err(anyhow::anyhow!("Push name cannot be empty"));
        }

        debug!("Setting push name (length={})", name.len());

        // Send presence with name only (no type attribute), matching WhatsApp Web's
        // WASmaxOutPresenceAvailabilityRequest which uses OPTIONAL for type.
        let node = NodeBuilder::new("presence").attr("name", name).build();
        self.client.send_node(node).await?;

        // Send app state sync mutation for cross-device propagation.
        // This writes a `setting_pushName` mutation to the `critical_block` collection,
        // matching WhatsApp Web's WAWebPushNameBridge behavior.
        if let Err(e) = self.send_push_name_mutation(name).await {
            // Non-fatal: the presence was already sent so the name change takes
            // effect immediately. App state sync may fail if keys aren't available
            // yet (e.g. right after pairing, before initial sync completes).
            warn!("Failed to send push name app state mutation: {e}");
        }

        // Persist only after the network send succeeds
        self.client
            .persistence_manager()
            .process_command(DeviceCommand::SetPushName(name.to_string()))
            .await;

        Ok(())
    }

    /// Set the user's own profile picture.
    ///
    /// Sends a JPEG image as the new profile picture. The image should already
    /// be properly sized/cropped by the caller (WhatsApp typically uses 640x640).
    ///
    /// Passing empty `image_data` **removes** the picture (matching WhatsApp Web);
    /// call [`Profile::remove_profile_picture`] when removal is the intent.
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
    ) -> Result<SetProfilePictureResponse> {
        // for_own routes empty bytes to the remove path, matching WA Web; no panic.
        debug!("Setting profile picture (size={} bytes)", image_data.len());
        Ok(self
            .client
            .execute(SetProfilePictureSpec::for_own(image_data))
            .await?)
    }

    /// Remove the user's own profile picture.
    pub async fn remove_profile_picture(&self) -> Result<SetProfilePictureResponse> {
        debug!("Removing profile picture");
        Ok(self
            .client
            .execute(SetProfilePictureSpec::remove_own())
            .await?)
    }

    /// Build and send the `setting_pushName` app state mutation.
    async fn send_push_name_mutation(&self, name: &str) -> Result<()> {
        use wacore::appstate::schemas;
        use waproto::whatsapp as wa;

        let value = wa::SyncActionValue {
            push_name_setting: Some(wa::sync_action_value::PushNameSetting {
                name: Some(name.to_string()),
            }),
            timestamp: Some(wacore::time::now_millis()),
            ..Default::default()
        };
        // setting_pushName's index has no args (collection/version come from the schema).
        self.client
            .send_app_state_action(&schemas::SETTING_PUSH_NAME, &[], &value)
            .await
    }
}

impl Client {
    /// Access profile operations.
    pub fn profile(&self) -> Profile<'_> {
        Profile::new(self)
    }
}
