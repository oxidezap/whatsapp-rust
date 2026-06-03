//! Identity presented in the noise-handshake `ClientPayload.UserAgent`.
//! Independent of `DeviceProps`. Default is [`ClientProfile::web`].

use waproto::whatsapp as wa;

#[derive(Debug, Clone)]
pub struct ClientProfile {
    pub user_agent_platform: wa::client_payload::user_agent::Platform,
    pub device: String,
    pub os_version: String,
    pub manufacturer: String,
    pub include_web_info: bool,
    /// `ClientPayload.passive` value on login. WA Web defaults to `false` so
    /// the server delivers queued offline messages on (re)connect. Set to
    /// `true` to keep the connection passive (server holds queued messages
    /// until pulled), matching the whatsmeow convention.
    pub passive_login: bool,
    /// `UserAgent.phone_id`. WA Web sends a random UUID per connect; the
    /// server uses it for anti-abuse / device-tracking heuristics. `None`
    /// auto-generates a fresh UUID-v4 on every payload build.
    pub phone_id: Option<String>,
    /// `UserAgent.locale_language_iso6391` (ISO-639-1, e.g. "pt").
    pub locale_language: String,
    /// `UserAgent.locale_country_iso31661_alpha2` (ISO-3166-1 alpha-2,
    /// e.g. "BR"). The previous hardcoded `"en"` is not a valid country code.
    pub locale_country: String,
}

impl Default for ClientProfile {
    fn default() -> Self {
        Self::web()
    }
}

impl ClientProfile {
    pub fn web() -> Self {
        Self {
            user_agent_platform: wa::client_payload::user_agent::Platform::WEB,
            device: "Desktop".to_string(),
            os_version: "0.1.0".to_string(),
            manufacturer: String::new(),
            include_web_info: true,
            passive_login: false,
            phone_id: None,
            locale_language: "en".to_string(),
            locale_country: "US".to_string(),
        }
    }

    pub fn android(os_version: impl Into<String>) -> Self {
        Self {
            user_agent_platform: wa::client_payload::user_agent::Platform::ANDROID,
            device: "Smartphone".to_string(),
            os_version: os_version.into(),
            manufacturer: String::new(),
            include_web_info: false,
            passive_login: false,
            phone_id: None,
            locale_language: "en".to_string(),
            locale_country: "US".to_string(),
        }
    }

    pub fn smb_android(os_version: impl Into<String>) -> Self {
        Self {
            user_agent_platform: wa::client_payload::user_agent::Platform::SMB_ANDROID,
            device: "Smartphone".to_string(),
            os_version: os_version.into(),
            manufacturer: String::new(),
            include_web_info: false,
            passive_login: false,
            phone_id: None,
            locale_language: "en".to_string(),
            locale_country: "US".to_string(),
        }
    }

    pub fn ios(os_version: impl Into<String>) -> Self {
        Self {
            user_agent_platform: wa::client_payload::user_agent::Platform::IOS,
            device: "iPhone".to_string(),
            os_version: os_version.into(),
            manufacturer: "Apple".to_string(),
            include_web_info: false,
            passive_login: false,
            phone_id: None,
            locale_language: "en".to_string(),
            locale_country: "US".to_string(),
        }
    }

    pub fn macos(os_version: impl Into<String>) -> Self {
        Self {
            user_agent_platform: wa::client_payload::user_agent::Platform::MACOS,
            device: "Desktop".to_string(),
            os_version: os_version.into(),
            manufacturer: "Apple".to_string(),
            include_web_info: false,
            passive_login: false,
            phone_id: None,
            locale_language: "en".to_string(),
            locale_country: "US".to_string(),
        }
    }

    pub fn windows(os_version: impl Into<String>) -> Self {
        Self {
            user_agent_platform: wa::client_payload::user_agent::Platform::WINDOWS,
            device: "Desktop".to_string(),
            os_version: os_version.into(),
            manufacturer: String::new(),
            include_web_info: false,
            passive_login: false,
            phone_id: None,
            locale_language: "en".to_string(),
            locale_country: "US".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_profile_matches_legacy_defaults() {
        let p = ClientProfile::web();
        assert_eq!(
            p.user_agent_platform,
            wa::client_payload::user_agent::Platform::WEB
        );
        assert_eq!(p.device, "Desktop");
        assert_eq!(p.os_version, "0.1.0");
        assert_eq!(p.manufacturer, "");
        assert!(p.include_web_info);
    }

    #[test]
    fn android_profile_omits_web_info_and_carries_os_version() {
        let p = ClientProfile::android("13");
        assert_eq!(
            p.user_agent_platform,
            wa::client_payload::user_agent::Platform::ANDROID
        );
        assert_eq!(p.os_version, "13");
        assert!(!p.include_web_info);
    }

    #[test]
    fn smb_android_uses_smb_platform() {
        let p = ClientProfile::smb_android("14");
        assert_eq!(
            p.user_agent_platform,
            wa::client_payload::user_agent::Platform::SMB_ANDROID
        );
        assert!(!p.include_web_info);
    }

    #[test]
    fn ios_profile_marks_apple_manufacturer() {
        let p = ClientProfile::ios("17.4");
        assert_eq!(
            p.user_agent_platform,
            wa::client_payload::user_agent::Platform::IOS
        );
        assert_eq!(p.manufacturer, "Apple");
        assert!(!p.include_web_info);
    }

    #[test]
    fn native_profiles_all_drop_web_info() {
        for p in [
            ClientProfile::android(""),
            ClientProfile::smb_android(""),
            ClientProfile::ios(""),
            ClientProfile::macos(""),
            ClientProfile::windows(""),
        ] {
            assert!(!p.include_web_info, "{:?} must omit web_info", p);
        }
    }
}
