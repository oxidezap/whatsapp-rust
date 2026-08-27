//! The buffer globals this client is entitled to write.
//!
//! A global describes the client, not an event, so almost every one of the 46
//! the catalog declares is a fact about a browser this library is not: the
//! window's memory class, the tab id, the number of CPUs the page can see. The
//! rule for this batch applies to them exactly as it does to fields (a value
//! that is not honestly derivable is absent, never a placeholder), and the
//! consequence is that the default identity writes five globals.
//!
//! The other axis is contradiction. A `browser` or `osVersion` that disagrees
//! with what the pairing `ClientPayload` announced is worse than nothing,
//! because the server has both. So the values that can be derived are derived
//! from [`ClientProfile`], the same struct the payload is built from, and the
//! ones an embedder may add are documented against that requirement.

use whatsapp_rust::ClientProfile;
use whatsapp_rust::waproto::whatsapp as wa;
use whatsapp_rust_wam_catalog::{Channel, GlobalDef, GlobalValue, enums, globals};

/// One global this client will write, with the value it will write.
///
/// Kept as a pair rather than resolved into a struct of named fields so that
/// adding a global is adding a row, and so the runtime can check every row
/// against its channel before the buffer sees it.
#[derive(Debug, Clone)]
pub struct ResolvedGlobal {
    pub def: GlobalDef,
    pub value: GlobalValueOwned,
}

/// An owned [`GlobalValue`], because the identity outlives any one buffer.
#[derive(Debug, Clone, PartialEq)]
pub enum GlobalValueOwned {
    Bool(bool),
    Int(i64),
    Str(String),
}

impl GlobalValueOwned {
    pub(crate) fn as_ref(&self) -> GlobalValue<'_> {
        match self {
            Self::Bool(b) => GlobalValue::Bool(*b),
            Self::Int(i) => GlobalValue::Int(*i),
            Self::Str(s) => GlobalValue::Str(s),
        }
    }
}

/// What this client says about itself in a WAM buffer.
///
/// # What it does not guarantee
///
/// That the values match what the server was told at pairing. They do when the
/// identity is built with [`from_profile`](Self::from_profile) from the same
/// [`ClientProfile`] the client was built with, and nothing checks it when it is
/// not: the plugin cannot read the client's configuration through any
/// capability it holds.
#[derive(Debug, Clone)]
pub struct WamIdentity {
    /// `appVersion`: the WhatsApp build this client announces.
    pub app_version: String,
    /// `platform`: which WhatsApp client family this one claims to be.
    pub platform: enums::PlatformType,
    /// `osVersion`: the OS version string the `ClientPayload` carries.
    pub os_version: Option<String>,
    /// `deviceName`: the OS name, the way the official web client reports the
    /// browser's platform. Absent by default, because [`ClientProfile::device`] holds a
    /// device *class* ("Desktop"), which is a different thing, and writing it
    /// here would be a value the server can see is not what it was told.
    pub device_name: Option<String>,
    /// `serviceImprovementOptOut`: whether the account opted out of sharing
    /// telemetry. Absent by default because the library cannot read the setting;
    /// an embedder that knows it should set it, and one that has reason to
    /// believe the user opted out should not install this plugin at all.
    pub service_improvement_opt_out: Option<bool>,
}

impl WamIdentity {
    /// The identity implied by a [`ClientProfile`] and the version the client
    /// announces.
    ///
    /// Every field is taken from the same place the pairing payload takes it,
    /// so the two cannot disagree.
    pub fn from_profile(profile: &ClientProfile, app_version: impl Into<String>) -> Self {
        Self {
            app_version: app_version.into(),
            platform: platform_for(profile.user_agent_platform),
            os_version: Some(profile.os_version.clone()),
            device_name: None,
            service_improvement_opt_out: None,
        }
    }

    /// The identity of a default web client, at the WhatsApp build this library
    /// was generated against.
    pub fn web() -> Self {
        Self::from_profile(
            &ClientProfile::web(),
            whatsapp_rust::wacore::version::WA_WEB_VERSION_STR,
        )
    }

    /// The globals to write, in the order the catalog lists them.
    ///
    /// `ocVersion` and `appIsBetaRelease` are not settable and are not defaults
    /// either: they are facts. This is not the official client, so `ocVersion` is
    /// 0, and the `ClientPayload` announces the `RELEASE` channel, so
    /// `appIsBetaRelease` is false. Nothing an embedder configures can make
    /// either untrue.
    pub fn resolved(&self) -> Vec<ResolvedGlobal> {
        let mut out = vec![
            ResolvedGlobal {
                def: globals::APP_VERSION,
                value: GlobalValueOwned::Str(self.app_version.clone()),
            },
            ResolvedGlobal {
                def: globals::PLATFORM,
                value: GlobalValueOwned::Int(self.platform.wire()),
            },
            ResolvedGlobal {
                def: globals::OC_VERSION,
                value: GlobalValueOwned::Int(0),
            },
            ResolvedGlobal {
                def: globals::APP_IS_BETA_RELEASE,
                value: GlobalValueOwned::Bool(false),
            },
        ];
        if let Some(os) = &self.os_version {
            out.push(ResolvedGlobal {
                def: globals::OS_VERSION,
                value: GlobalValueOwned::Str(os.clone()),
            });
        }
        if let Some(name) = &self.device_name {
            out.push(ResolvedGlobal {
                def: globals::DEVICE_NAME,
                value: GlobalValueOwned::Str(name.clone()),
            });
        }
        if let Some(opt_out) = self.service_improvement_opt_out {
            out.push(ResolvedGlobal {
                def: globals::SERVICE_IMPROVEMENT_OPT_OUT,
                value: GlobalValueOwned::Bool(opt_out),
            });
        }
        out
    }

    /// The globals legal on `channel`.
    ///
    /// Every global this identity writes happens to be legal on both channels
    /// the catalog defines for them, so this filters nothing today. It exists
    /// so that adding a `regular`-only global cannot silently produce a
    /// `private` buffer no client would send. The buffer would refuse it, and
    /// this is where the refusal is avoided rather than hit.
    pub fn resolved_for(&self, channel: Channel) -> Vec<ResolvedGlobal> {
        self.resolved()
            .into_iter()
            .filter(|g| g.def.channels.allows(channel))
            .collect()
    }
}

impl Default for WamIdentity {
    fn default() -> Self {
        Self::web()
    }
}

/// The WAM platform for a `ClientPayload` platform.
///
/// Only the families this library's [`ClientProfile`] constructors produce are
/// mapped. Anything else falls back to `UNKNOWN`, which is a member of the enum
/// rather than an invention: the official client has the same problem and the
/// same answer.
fn platform_for(platform: wa::client_payload::user_agent::Platform) -> enums::PlatformType {
    use wa::client_payload::user_agent::Platform;
    match platform {
        Platform::WEB => enums::PlatformType::Webclient,
        Platform::ANDROID => enums::PlatformType::Android,
        Platform::IOS => enums::PlatformType::Iphone,
        Platform::SMB_ANDROID => enums::PlatformType::Smba,
        Platform::SMB_IOS => enums::PlatformType::Smbi,
        Platform::MACOS => enums::PlatformType::Macos,
        Platform::WINDOWS => enums::PlatformType::Windows,
        _ => enums::PlatformType::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value_of(identity: &WamIdentity, id: u32) -> Option<GlobalValueOwned> {
        identity
            .resolved()
            .into_iter()
            .find(|g| g.def.id == id)
            .map(|g| g.value)
    }

    #[test]
    fn the_default_identity_writes_only_what_it_can_derive() {
        let identity = WamIdentity::web();
        let ids: Vec<u32> = identity.resolved().iter().map(|g| g.def.id).collect();
        // appVersion, platform, ocVersion, appIsBetaRelease, osVersion, and
        // nothing about a browser this library does not run in.
        assert_eq!(ids, vec![17, 11, 6251, 21, 15]);
        assert_eq!(
            value_of(&identity, 11),
            Some(GlobalValueOwned::Int(enums::PlatformType::Webclient.wire()))
        );
        // Not the official client, and the payload announces the release
        // channel: neither is configurable.
        assert_eq!(value_of(&identity, 6251), Some(GlobalValueOwned::Int(0)));
        assert_eq!(value_of(&identity, 21), Some(GlobalValueOwned::Bool(false)));
    }

    #[test]
    fn the_os_version_comes_from_the_same_place_the_payload_takes_it() {
        let mut profile = ClientProfile::android("13");
        profile.os_version = "13".to_string();
        let identity = WamIdentity::from_profile(&profile, "2.3000.1");
        assert_eq!(
            value_of(&identity, 15),
            Some(GlobalValueOwned::Str("13".to_string()))
        );
        assert_eq!(
            value_of(&identity, 11),
            Some(GlobalValueOwned::Int(enums::PlatformType::Android.wire()))
        );
    }

    #[test]
    fn an_absent_value_is_absent_rather_than_empty() {
        let identity = WamIdentity {
            os_version: None,
            ..WamIdentity::web()
        };
        assert!(value_of(&identity, 15).is_none());
        assert!(value_of(&identity, 13).is_none());
        assert!(value_of(&identity, 13293).is_none());
    }

    #[test]
    fn every_resolved_global_is_legal_on_the_regular_channel() {
        // The runtime writes these into a regular buffer, which refuses a
        // global whose channel list does not contain it. Catching that here
        // means it can never reach the buffer at all.
        for global in WamIdentity::web().resolved() {
            assert!(
                global.def.channels.allows(Channel::Regular),
                "{} is not legal on the regular channel",
                global.def.name
            );
        }
        assert_eq!(
            WamIdentity::web().resolved_for(Channel::Regular).len(),
            WamIdentity::web().resolved().len()
        );
    }
}
