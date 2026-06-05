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
            .await
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

    /// Get business profile for a WhatsApp Business account.
    pub async fn get_business_profile(
        &self,
        jid: &wacore_binary::Jid,
    ) -> Result<Option<wacore::iq::business::BusinessProfile>, crate::request::IqError> {
        use wacore::iq::business::BusinessProfileSpec;
        self.execute(BusinessProfileSpec::new(jid)).await
    }

    /// Reject an incoming call. Fire-and-forget — no server response is expected.
    pub async fn reject_call(
        &self,
        call_id: &str,
        call_from: &wacore_binary::Jid,
    ) -> Result<(), anyhow::Error> {
        anyhow::ensure!(!call_id.is_empty(), "call_id cannot be empty");
        let id = self.generate_request_id();

        let stanza = wacore_binary::builder::NodeBuilder::new("call")
            .attr("to", call_from)
            .attr("id", id)
            .children([wacore_binary::builder::NodeBuilder::new("reject")
                .attr("call-id", call_id)
                .attr("call-creator", call_from)
                .attr("count", "0")
                .build()])
            .build();

        self.send_node(stanza).await?;
        Ok(())
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
        if self
            .persistence_manager
            .get_device_snapshot()
            .await
            .pn
            .is_some()
        {
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
