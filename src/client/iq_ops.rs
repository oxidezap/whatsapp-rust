//! IQ-based operations: props, privacy settings, profiles and assorted requests.

use super::*;

impl Client {
    pub async fn set_passive(&self, passive: bool) -> Result<(), crate::request::IqError> {
        use wacore::iq::passive::PassiveModeSpec;
        self.execute(PassiveModeSpec::new(passive)).await
    }

    pub async fn fetch_props(&self) -> Result<(), crate::request::IqError> {
        use wacore::iq::props::PropsSpec;
        use wacore::store::commands::DeviceCommand;

        let stored_hash = self
            .persistence_manager
            .get_device_snapshot()
            .props_hash
            .clone();

        // Deltas only contain changed props, so they're invalid against an empty cache.
        let spec = match &stored_hash {
            Some(hash) if self.ab_props.is_seeded() => {
                debug!("Fetching props with hash for delta update...");
                PropsSpec::with_hash(hash)
            }
            _ => {
                debug!("Fetching props (full)...");
                PropsSpec::new()
            }
        };

        let response = self.execute(spec).await?;

        if response.delta_update {
            debug!(
                "Props delta update received ({} changed props)",
                response.experiment_props.len()
            );
        } else {
            debug!(
                "Props full update received ({} props, hash={:?})",
                response.experiment_props.len(),
                response.hash
            );
        }

        self.ab_props
            .apply_props(response.delta_update, response.experiment_props.into_iter())
            .await;

        if let Some(new_hash) = response.hash {
            self.persistence_manager
                .process_command(DeviceCommand::SetPropsHash(Some(new_hash)))
                .await;
        }

        Ok(())
    }

    pub(crate) fn ab_props(&self) -> &wacore::store::ab_props::AbPropsCache {
        &self.ab_props
    }

    pub async fn fetch_privacy_settings(
        &self,
    ) -> Result<wacore::iq::privacy::PrivacySettingsResponse, crate::request::IqError> {
        use wacore::iq::privacy::PrivacySettingsSpec;

        debug!("Fetching privacy settings...");

        self.execute(PrivacySettingsSpec::new()).await
    }

    /// Set a privacy setting.
    ///
    /// Use [`PrivacyCategory::is_valid_value`] to check valid combinations.
    ///
    /// # Example
    /// ```ignore
    /// use wacore::iq::privacy::{PrivacyCategory, PrivacyValue};
    /// client.set_privacy_setting(PrivacyCategory::Last, PrivacyValue::Contacts).await?;
    /// ```
    pub async fn set_privacy_setting(
        &self,
        category: wacore::iq::privacy::PrivacyCategory,
        value: wacore::iq::privacy::PrivacyValue,
    ) -> Result<wacore::iq::privacy::SetPrivacySettingResponse, crate::request::IqError> {
        use wacore::iq::privacy::SetPrivacySettingSpec;
        self.execute(SetPrivacySettingSpec::new(category, value))
            .await
    }

    /// Set a privacy setting to `contact_blacklist` with a disallowed list update.
    ///
    /// Only `Last`, `Profile`, `Status`, `GroupAdd` support disallowed lists.
    /// Returns the server's updated dhash for use in subsequent updates.
    pub async fn set_privacy_disallowed_list(
        &self,
        category: wacore::iq::privacy::PrivacyCategory,
        update: wacore::iq::privacy::DisallowedListUpdate,
    ) -> Result<wacore::iq::privacy::SetPrivacySettingResponse, crate::request::IqError> {
        use wacore::iq::privacy::SetPrivacySettingSpec;
        self.execute(SetPrivacySettingSpec::with_disallowed_list(
            category, update,
        ))
        .await
    }

    /// Set the default disappearing messages duration (seconds). Pass 0 to disable.
    pub async fn set_default_disappearing_mode(
        &self,
        duration: u32,
    ) -> Result<(), crate::request::IqError> {
        use wacore::iq::privacy::SetDefaultDisappearingModeSpec;
        self.execute(SetDefaultDisappearingModeSpec::new(duration))
            .await
    }

    /// Turn disappearing messages on or off for a 1:1 chat (`duration` in
    /// seconds; `0` disables).
    ///
    /// Sends an `EPHEMERAL_SETTING` protocol message, mirroring WA Web's
    /// `WAWebUpdateEphemeralSettingChatAction`. For groups use
    /// [`Groups::set_ephemeral`](crate::Groups::set_ephemeral); for the account
    /// default use [`Client::set_default_disappearing_mode`].
    pub async fn set_chat_disappearing_timer(
        &self,
        chat: wacore_binary::Jid,
        duration: u32,
    ) -> Result<crate::send::SendResult, crate::send::SendError> {
        // 1:1 only: groups use Groups::set_ephemeral (a separate IQ). Sending the
        // EPHEMERAL_SETTING body to a group/status/newsletter would produce a
        // message that does not change the chat's timer, so fail fast instead.
        if !(chat.is_pn() || chat.is_lid()) {
            return Err(crate::send::SendError::InvalidRequest(
                "set_chat_disappearing_timer is 1:1-only; use Groups::set_ephemeral for groups"
                    .into(),
            ));
        }
        let msg = build_ephemeral_setting_message(duration, wacore::time::now_secs_u64() as i64);
        self.send_message(chat, msg).await
    }

    /// Get business profile for a WhatsApp Business account.
    pub async fn get_business_profile(
        &self,
        jid: &wacore_binary::Jid,
    ) -> Result<Option<wacore::iq::business::BusinessProfile>, crate::request::IqError> {
        use wacore::iq::business::BusinessProfileSpec;
        self.execute(BusinessProfileSpec::new(jid)).await
    }

    pub async fn send_digest_key_bundle(&self) -> Result<(), crate::request::IqError> {
        use wacore::iq::prekeys::DigestKeyBundleSpec;

        debug!("Sending digest key bundle...");

        self.execute(DigestKeyBundleSpec::new()).await.map(|_| ())
    }

    /// Override `DeviceProps` fields before the initial pairing. Only fields
    /// with `Some` are changed. In-memory only — WA Web regenerates
    /// `device_props` at each registration, and it has no wire effect after
    /// pairing. Call before `connect()` on every process start that still
    /// needs to pair.
    pub async fn set_device_props(&self, override_: wacore::store::DevicePropsOverride) {
        use wacore::store::commands::DeviceCommand;
        if override_.is_empty() {
            return;
        }
        if self.persistence_manager.get_device_snapshot().pn.is_some() {
            warn!(
                target: "Client/DeviceProps",
                "set_device_props called after pairing — stored but not sent on the wire"
            );
        }
        self.persistence_manager
            .process_command(DeviceCommand::SetDeviceProps(override_))
            .await;
    }

    /// Set the noise-handshake `ClientPayload` profile. In-memory only;
    /// call before each `connect()` on a fresh process.
    pub async fn set_client_profile(&self, profile: wacore::client_profile::ClientProfile) {
        use wacore::store::commands::DeviceCommand;
        self.persistence_manager
            .process_command(DeviceCommand::SetClientProfile(profile))
            .await;
    }
}

/// Builds the `EPHEMERAL_SETTING` protocol message that turns a 1:1 chat's
/// disappearing timer on/off. The timer data lives directly on `ProtocolMessage`
/// (`ephemeral_expiration`, `ephemeral_setting_timestamp`, `disappearing_mode`);
/// there is no `ephemeral_setting` field. Timestamp is unix seconds. Mirrors
/// WA Web's `MsgChatActionUtils` (`disappearingModeInitiator: ChangedInChat`,
/// `disappearingModeTrigger: Unknown`).
fn build_ephemeral_setting_message(duration: u32, now_secs: i64) -> waproto::whatsapp::Message {
    use waproto::whatsapp as wa;
    wa::Message {
        protocol_message: Some(Box::new(wa::message::ProtocolMessage {
            r#type: Some(wa::message::protocol_message::Type::EphemeralSetting as i32),
            ephemeral_expiration: Some(duration),
            ephemeral_setting_timestamp: Some(now_secs),
            disappearing_mode: Some(wa::DisappearingMode {
                initiator: Some(wa::disappearing_mode::Initiator::ChangedInChat as i32),
                trigger: Some(wa::disappearing_mode::Trigger::Unknown as i32),
                ..Default::default()
            }),
            ..Default::default()
        })),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::build_ephemeral_setting_message;
    use waproto::whatsapp as wa;

    #[test]
    fn ephemeral_setting_message_shape() {
        let msg = build_ephemeral_setting_message(86400, 1_700_000_000);
        let pm = msg.protocol_message.expect("protocol_message set");
        assert_eq!(
            pm.r#type,
            Some(wa::message::protocol_message::Type::EphemeralSetting as i32)
        );
        assert_eq!(pm.ephemeral_expiration, Some(86400));
        assert_eq!(pm.ephemeral_setting_timestamp, Some(1_700_000_000));
        let dm = pm.disappearing_mode.expect("disappearing_mode set");
        assert_eq!(
            dm.initiator,
            Some(wa::disappearing_mode::Initiator::ChangedInChat as i32)
        );
        assert_eq!(
            dm.trigger,
            Some(wa::disappearing_mode::Trigger::Unknown as i32)
        );
    }

    #[test]
    fn ephemeral_setting_disable_uses_zero_duration() {
        let msg = build_ephemeral_setting_message(0, 1);
        assert_eq!(msg.protocol_message.unwrap().ephemeral_expiration, Some(0));
    }
}
