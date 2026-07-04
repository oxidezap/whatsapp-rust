//! `companion_platform_id` + `companion_platform_display` emission.
//! Encoding only.

use waproto::whatsapp as wa;

/// Prefix `WAWebLinkDeviceQrcode` uses when iOS native-camera linking is on.
/// Concatenate with `make_qr_data` output to get a scannable deep-link URL.
pub const NATIVE_CAMERA_DEEP_LINK_PREFIX: &str = "https://wa.me/settings/linked_devices#";

/// Web codes follow `WAWebCompanionRegClientUtils.DEVICE_PLATFORM`.
/// Android letters need server-side attestation, so they're reachable
/// only through explicit opt-in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum CompanionWebClientType {
    Chrome,
    Edge,
    Firefox,
    Ie,
    Opera,
    Safari,
    Electron,
    Uwp,
    /// Default fallback. The proto's `UNKNOWN` (wire `'0'`) is absent
    /// because WA Web never emits it from a real browser and the server
    /// rejects it.
    #[default]
    OtherWebClient,
    AndroidTablet,
    AndroidPhone,
    AndroidAmbiguous,
}

impl CompanionWebClientType {
    /// Single-byte ASCII id placed in `<companion_platform_id>`.
    pub const fn wire_byte(self) -> u8 {
        match self {
            Self::Chrome => b'1',
            Self::Edge => b'2',
            Self::Firefox => b'3',
            Self::Ie => b'4',
            Self::Opera => b'5',
            Self::Safari => b'6',
            Self::Electron => b'7',
            Self::Uwp => b'8',
            Self::OtherWebClient => b'9',
            Self::AndroidTablet => b'd',
            Self::AndroidPhone => b'e',
            Self::AndroidAmbiguous => b'f',
        }
    }
}

impl std::fmt::Display for CompanionWebClientType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.wire_byte() as char)
    }
}

/// Browser label for `companion_platform_display`. Non-browser variants
/// fall back to "Chrome" because WA Web's `info().name` reports the
/// underlying Chromium renderer in those contexts. Mobile variants are
/// short-circuited by [`companion_platform_display`] before reaching here.
pub const fn companion_browser_name(ct: CompanionWebClientType) -> &'static str {
    match ct {
        CompanionWebClientType::Chrome => "Chrome",
        CompanionWebClientType::Edge => "Edge",
        CompanionWebClientType::Firefox => "Firefox",
        CompanionWebClientType::Ie => "IE",
        CompanionWebClientType::Opera => "Opera",
        CompanionWebClientType::Safari => "Safari",
        CompanionWebClientType::Electron
        | CompanionWebClientType::Uwp
        | CompanionWebClientType::OtherWebClient
        | CompanionWebClientType::AndroidTablet
        | CompanionWebClientType::AndroidPhone
        | CompanionWebClientType::AndroidAmbiguous => "Chrome",
    }
}

/// Android maps to `Chrome` because that's what real WA Web on
/// Chrome-Android emits and what the server accepts; the Android
/// letters need attestation we can't fake from this crate, so they
/// stay behind `PairCodeOptions::platform_id`. iOS/AR/VR and the
/// proto's `UNKNOWN` collapse to `OtherWebClient` — `'0'` would be
/// server-rejected.
pub const fn companion_web_client_type_for_platform(
    pt: wa::device_props::PlatformType,
) -> CompanionWebClientType {
    use CompanionWebClientType as C;
    use wa::device_props::PlatformType as P;
    match pt {
        P::CHROME => C::Chrome,
        P::FIREFOX => C::Firefox,
        P::IE => C::Ie,
        P::OPERA => C::Opera,
        P::SAFARI => C::Safari,
        P::EDGE => C::Edge,
        P::DESKTOP => C::Electron,
        P::UWP => C::Uwp,
        P::ANDROID_PHONE | P::ANDROID_TABLET | P::ANDROID_AMBIGUOUS => C::Chrome,
        P::UNKNOWN
        | P::IPAD
        | P::OHANA
        | P::ALOHA
        | P::CATALINA
        | P::TCL_TV
        | P::IOS_PHONE
        | P::IOS_CATALYST
        | P::WEAR_OS
        | P::AR_WRIST
        | P::AR_DEVICE
        | P::VR
        | P::CLOUD_API
        | P::SMARTGLASSES => C::OtherWebClient,
    }
}

pub fn companion_web_client_type_for_props(props: &wa::DeviceProps) -> CompanionWebClientType {
    props
        .platform_type
        .map(companion_web_client_type_for_platform)
        .unwrap_or(CompanionWebClientType::OtherWebClient)
}

/// Canonical OS label for the pair-code `companion_platform_display`.
///
/// WA Web only ever emits a real OS name from its UA parser
/// (`WAWebBrowserInfo().os` → `ua-parser-js` `getOS().name`). Unlike QR pairing —
/// which never sends this field and therefore tolerates an arbitrary branding
/// string in `DeviceProps::os` — the pair-code `companion_hello` server
/// **rejects a non-OS display with `bad-request`**. So the OS component must come
/// from a closed set of real OS names, never the free-form branding string.
///
/// The set is deliberately small and conservative: the server is lenient toward
/// real OS names today (it also accepts `Ubuntu`, `Fedora`, `Mac`, …), but
/// collapsing everything to a guaranteed-accepted canonical value is robust
/// against that leniency changing. Anything unrecognized coerces to
/// [`Self::Linux`] (see [`Self::from_hint`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompanionOs {
    Windows,
    MacOs,
    Linux,
    Android,
    Ios,
}

impl CompanionOs {
    /// The exact wire string, matching `ua-parser-js` `getOS().name` (what WA Web
    /// sends) — note the `"Mac OS"` and `"iOS"` spellings.
    pub const fn wire_str(self) -> &'static str {
        match self {
            Self::Windows => "Windows",
            Self::MacOs => "Mac OS",
            Self::Linux => "Linux",
            Self::Android => "Android",
            Self::Ios => "iOS",
        }
    }

    /// Classify a free-form OS hint into a known canonical OS, or `None` when it
    /// is not recognizably an OS (empty, or a branding label such as `"Veloz"`).
    /// Case-insensitive. iPad folds to [`Self::Ios`]: older UA parsers report iPad
    /// as `"iOS"`, so `"iPadOS"` is not a confirmed server-accepted value. Chrome
    /// OS and Linux distros fold to [`Self::Linux`].
    pub fn classify(os: &str) -> Option<Self> {
        let os = os.trim().to_ascii_lowercase();
        if os.is_empty() {
            None
        } else if os.contains("windows") {
            Some(Self::Windows)
        } else if os.contains("mac") || os.contains("osx") || os.contains("darwin") {
            Some(Self::MacOs)
        } else if os.contains("ipad")
            || os.contains("iphone")
            // Whole-word "ios" only, so "KaiOS" etc. don't false-match the substring.
            || os.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|tok| tok == "ios")
        {
            Some(Self::Ios)
        } else if os.contains("android") {
            Some(Self::Android)
        } else if os.contains("linux")
            || os.contains("ubuntu")
            || os.contains("debian")
            || os.contains("fedora")
            || os.contains("chrome os")
            || os.contains("chromeos")
            || os.contains("chromium")
            // Whole-word for the short ambiguous ones so branding like "March"/
            // "Search"/"across" doesn't false-match (bare "Arch"/"CrOS" still do;
            // "Arch Linux"/"archlinux" are caught by the "linux" substring above).
            || os.split(|c: char| !c.is_ascii_alphanumeric())
                .any(|tok| tok == "arch" || tok == "cros")
        {
            Some(Self::Linux)
        } else {
            None
        }
    }

    /// Coerce a free-form `DeviceProps::os` into a server-safe canonical OS,
    /// defaulting an unrecognized/branding value to [`Self::Linux`].
    pub fn from_hint(os: &str) -> Self {
        Self::classify(os).unwrap_or(Self::Linux)
    }
}

/// Formats `<Browser> (<os>)` (Android client types → `Android (<os>)`),
/// mirroring `WAWebAltDeviceLinkingIq`, with `os` used **verbatim** — no
/// canonicalization. This is the escape hatch for an advanced caller that
/// overrides the display OS (e.g. to keep a real distro name like `"Ubuntu"`
/// the server accepts); the server validates the OS, so a non-OS string here is
/// rejected with `bad-request`. Most callers want [`companion_platform_display`].
pub fn companion_platform_display_raw(ct: CompanionWebClientType, os: &str) -> String {
    use CompanionWebClientType as C;
    match ct {
        C::AndroidPhone | C::AndroidTablet | C::AndroidAmbiguous => {
            format!("Android ({os})")
        }
        _ => format!("{} ({})", companion_browser_name(ct), os),
    }
}

/// `companion_platform_display` body: `<Browser> (<OS>)` (Android client types
/// emit `Android (<OS>)`), mirroring `WAWebAltDeviceLinkingIq`. The OS is
/// canonicalized through [`CompanionOs`] because the pair-code server rejects a
/// non-OS string here with `bad-request`; an unrecognized/branding `os` (or an
/// empty one) becomes `Linux`.
pub fn companion_platform_display(ct: CompanionWebClientType, os: &str) -> String {
    companion_platform_display_raw(ct, CompanionOs::from_hint(os).wire_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_byte_matches_wa_web() {
        assert_eq!(CompanionWebClientType::Chrome.wire_byte(), b'1');
        assert_eq!(CompanionWebClientType::Edge.wire_byte(), b'2');
        assert_eq!(CompanionWebClientType::Firefox.wire_byte(), b'3');
        assert_eq!(CompanionWebClientType::Ie.wire_byte(), b'4');
        assert_eq!(CompanionWebClientType::Opera.wire_byte(), b'5');
        assert_eq!(CompanionWebClientType::Safari.wire_byte(), b'6');
        assert_eq!(CompanionWebClientType::Electron.wire_byte(), b'7');
        assert_eq!(CompanionWebClientType::Uwp.wire_byte(), b'8');
        assert_eq!(CompanionWebClientType::OtherWebClient.wire_byte(), b'9');
    }

    #[test]
    fn wire_byte_matches_apk_for_mobile() {
        assert_eq!(CompanionWebClientType::AndroidTablet.wire_byte(), b'd');
        assert_eq!(CompanionWebClientType::AndroidPhone.wire_byte(), b'e');
        assert_eq!(CompanionWebClientType::AndroidAmbiguous.wire_byte(), b'f');
    }

    #[test]
    fn display_renders_wire_byte_as_char() {
        assert_eq!(format!("{}", CompanionWebClientType::Chrome), "1");
        assert_eq!(format!("{}", CompanionWebClientType::OtherWebClient), "9");
        assert_eq!(format!("{}", CompanionWebClientType::AndroidPhone), "e");
        assert_eq!(format!("{}", CompanionWebClientType::AndroidTablet), "d");
        assert_eq!(format!("{}", CompanionWebClientType::AndroidAmbiguous), "f");
    }

    #[test]
    fn default_is_other_web_client_nine() {
        assert_eq!(
            CompanionWebClientType::default(),
            CompanionWebClientType::OtherWebClient,
        );
        assert_eq!(CompanionWebClientType::default().wire_byte(), b'9');
    }

    #[test]
    fn browser_and_desktop_platform_types_map_to_their_variants() {
        use CompanionWebClientType as C;
        use wa::device_props::PlatformType as P;
        for (pt, expected) in [
            (P::CHROME, C::Chrome),
            (P::FIREFOX, C::Firefox),
            (P::EDGE, C::Edge),
            (P::SAFARI, C::Safari),
            (P::OPERA, C::Opera),
            (P::IE, C::Ie),
            (P::DESKTOP, C::Electron),
            (P::UWP, C::Uwp),
        ] {
            assert_eq!(
                companion_web_client_type_for_platform(pt),
                expected,
                "{pt:?}"
            );
        }
    }

    #[test]
    fn android_platform_types_map_to_chrome() {
        use CompanionWebClientType as C;
        use wa::device_props::PlatformType as P;
        for pt in [P::ANDROID_PHONE, P::ANDROID_TABLET, P::ANDROID_AMBIGUOUS] {
            assert_eq!(
                companion_web_client_type_for_platform(pt),
                C::Chrome,
                "{pt:?}"
            );
        }
    }

    #[test]
    fn unconfirmed_platform_types_collapse_to_other() {
        use CompanionWebClientType as C;
        use wa::device_props::PlatformType as P;
        for pt in [
            P::IPAD,
            P::IOS_PHONE,
            P::IOS_CATALYST,
            P::WEAR_OS,
            P::AR_WRIST,
            P::AR_DEVICE,
            P::VR,
            P::OHANA,
            P::ALOHA,
            P::CATALINA,
            P::TCL_TV,
            P::CLOUD_API,
            P::SMARTGLASSES,
        ] {
            assert_eq!(
                companion_web_client_type_for_platform(pt),
                C::OtherWebClient,
                "{pt:?}",
            );
        }
    }

    #[test]
    fn proto_unknown_collapses_to_other_web_client() {
        use CompanionWebClientType as C;
        use wa::device_props::PlatformType as P;
        assert_eq!(
            companion_web_client_type_for_platform(P::UNKNOWN),
            C::OtherWebClient,
        );
    }

    #[test]
    fn android_variants_still_emit_their_wire_bytes_when_used_directly() {
        assert_eq!(CompanionWebClientType::AndroidPhone.wire_byte(), b'e');
        assert_eq!(CompanionWebClientType::AndroidTablet.wire_byte(), b'd');
        assert_eq!(CompanionWebClientType::AndroidAmbiguous.wire_byte(), b'f');
    }

    #[test]
    fn for_props_reads_platform_type() {
        let props = wa::DeviceProps {
            platform_type: Some(wa::device_props::PlatformType::CHROME),
            ..Default::default()
        };
        assert_eq!(
            companion_web_client_type_for_props(&props),
            CompanionWebClientType::Chrome,
        );
    }

    #[test]
    fn for_props_missing_platform_type_is_other_web_client() {
        let props = wa::DeviceProps::default();
        assert_eq!(
            companion_web_client_type_for_props(&props),
            CompanionWebClientType::OtherWebClient,
        );
    }

    #[test]
    fn for_props_invalid_platform_type_is_other_web_client() {
        use buffa::Message as _;

        let props = wa::DeviceProps::decode_from_slice(&[0x18, 0x8f, 0x4e]).unwrap();
        assert_eq!(
            companion_web_client_type_for_props(&props),
            CompanionWebClientType::OtherWebClient,
        );
    }

    #[test]
    fn browser_name_for_six_valid_browsers() {
        use CompanionWebClientType as C;
        for (ct, name) in [
            (C::Chrome, "Chrome"),
            (C::Edge, "Edge"),
            (C::Firefox, "Firefox"),
            (C::Ie, "IE"),
            (C::Opera, "Opera"),
            (C::Safari, "Safari"),
        ] {
            assert_eq!(companion_browser_name(ct), name, "{ct:?}");
        }
    }

    #[test]
    fn browser_name_for_non_browser_falls_back_to_chrome() {
        for ct in [
            CompanionWebClientType::Electron,
            CompanionWebClientType::Uwp,
            CompanionWebClientType::OtherWebClient,
        ] {
            assert_eq!(companion_browser_name(ct), "Chrome", "{ct:?}");
        }
    }

    #[test]
    fn platform_display_always_browser_paren_os() {
        assert_eq!(
            companion_platform_display(CompanionWebClientType::Chrome, "Linux"),
            "Chrome (Linux)"
        );
        // "Mac" canonicalizes to the ua-parser name WA Web actually sends.
        assert_eq!(
            companion_platform_display(CompanionWebClientType::Firefox, "Mac"),
            "Firefox (Mac OS)"
        );
    }

    #[test]
    fn platform_display_empty_os_defaults_to_linux() {
        assert_eq!(
            companion_platform_display(CompanionWebClientType::Chrome, ""),
            "Chrome (Linux)"
        );
        assert_eq!(
            companion_platform_display(CompanionWebClientType::Chrome, "   "),
            "Chrome (Linux)"
        );
    }

    #[test]
    fn platform_display_non_browser_uses_chrome() {
        assert_eq!(
            companion_platform_display(CompanionWebClientType::OtherWebClient, "Android"),
            "Chrome (Android)"
        );
        assert_eq!(
            companion_platform_display(CompanionWebClientType::Electron, "Mac"),
            "Chrome (Mac OS)"
        );
    }

    #[test]
    fn companion_os_wire_str_matches_ua_parser_names() {
        assert_eq!(CompanionOs::Windows.wire_str(), "Windows");
        assert_eq!(CompanionOs::MacOs.wire_str(), "Mac OS");
        assert_eq!(CompanionOs::Linux.wire_str(), "Linux");
        assert_eq!(CompanionOs::Android.wire_str(), "Android");
        assert_eq!(CompanionOs::Ios.wire_str(), "iOS");
    }

    #[test]
    fn companion_os_classify_known_aliases() {
        use CompanionOs as O;
        for (hint, want) in [
            ("Windows", O::Windows),
            ("windows 11", O::Windows),
            ("Mac", O::MacOs),
            ("macOS", O::MacOs),
            ("Mac OS", O::MacOs),
            ("Mac OS X", O::MacOs),
            ("darwin", O::MacOs),
            ("Linux", O::Linux),
            ("Ubuntu", O::Linux),
            ("Fedora", O::Linux),
            ("Arch Linux", O::Linux),
            ("Arch", O::Linux),
            ("archlinux", O::Linux),
            ("CrOS", O::Linux),
            ("Chrome OS", O::Linux),
            ("ChromeOS", O::Linux),
            ("Chromium OS", O::Linux),
            ("Android", O::Android),
            ("android 14", O::Android),
            ("iOS", O::Ios),
            ("iOS 17", O::Ios),
            ("iPhone", O::Ios),
            ("iPad", O::Ios),
            ("iPadOS", O::Ios),
        ] {
            assert_eq!(CompanionOs::classify(hint), Some(want), "{hint:?}");
        }
    }

    #[test]
    fn companion_os_branding_and_empty_are_unclassified_and_default_linux() {
        // "KaiOS" must NOT substring-match "ios", and branding containing "arch"/
        // "cros" as a fragment ("March", "Search", "across") must NOT match Linux
        // -> all unrecognized -> Linux via fallback (not via classify).
        for hint in [
            "Veloz",
            "Foobar123",
            "KaiOS",
            "March",
            "Search",
            "across",
            "",
            "   ",
        ] {
            assert_eq!(CompanionOs::classify(hint), None, "{hint:?}");
            assert_eq!(CompanionOs::from_hint(hint), CompanionOs::Linux, "{hint:?}");
        }
    }

    /// The regression this whole enum exists for: a branding `os` must never ride
    /// through to the wire (the pair-code server rejects it with `bad-request`).
    #[test]
    fn platform_display_coerces_branding_os_to_linux() {
        assert_eq!(
            companion_platform_display(CompanionWebClientType::Chrome, "Veloz"),
            "Chrome (Linux)"
        );
    }

    #[test]
    fn platform_display_canonicalizes_mac_aliases() {
        for os in ["Mac", "macOS", "Mac OS", "Mac OS X", "darwin"] {
            assert_eq!(
                companion_platform_display(CompanionWebClientType::Chrome, os),
                "Chrome (Mac OS)",
                "{os:?}"
            );
        }
    }
}
