use crate::http::{HTTP_STATUS_REDIRECTION_START, HttpClient, HttpRequest};
use crate::store::commands::DeviceCommand;
use crate::store::persistence_manager::PersistenceManager;
use anyhow::{Context as _, Result, anyhow};
use log::debug;
use std::sync::Arc;
use wacore::types::events::{AppVersionFallback, AppVersionFallbackReason};

pub use wacore::version::{WA_WEB_VERSION, WA_WEB_VERSION_STR, parse_meta_sdk_js, parse_sw_js};

/// Where the client revision is read from on one target.
struct VersionSource {
    url: &'static str,
    headers: &'static [(&'static str, &'static str)],
    parse: fn(&str) -> Option<(u32, u32, u32)>,
    /// The field the parser looks for, so a parse failure can name it.
    field: &'static str,
    /// Whether an unreachable source is survivable or fatal to the connect.
    fallback_on_failure: bool,
}

/// WhatsApp Web's own service worker: the most direct source, and the one used
/// wherever the request is not made by a browser.
///
/// It answers 200 only to a request carrying `Sec-Fetch-Site: none`; any other
/// value, or none at all, is a 400. That header, `Connection` and `User-Agent`
/// are all forbidden header names under the Fetch spec, and no response from
/// `web.whatsapp.com` carries `Access-Control-Allow-Origin`, so this source is
/// doubly unreachable from a page. That is the whole reason for [`SDK_SOURCE`].
// Both are referenced: one by this target's build, the other by the other
// target's and by the source-selection tests.
#[allow(dead_code)]
const SW_SOURCE: VersionSource = VersionSource {
    url: "https://web.whatsapp.com/sw.js",
    // `Connection: close` because this fetch runs at most once a day per
    // device: a pooled idle TLS connection would be retained for the rest of
    // the session and buys nothing back before it is purged.
    headers: &[
        ("sec-fetch-site", "none"),
        ("connection", "close"),
        (
            "user-agent",
            "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36",
        ),
    ],
    parse: parse_sw_js,
    field: "client_revision",
    // Fatal: this host serves WhatsApp Web itself, so a client that cannot
    // reach it has hit a real break, and connecting anyway would bury it.
    fallback_on_failure: false,
};

/// The Facebook JS SDK bundle, used on wasm because it is the one place Meta
/// publishes this revision for cross-origin loading (`Access-Control-Allow-Origin: *`,
/// no fetch-metadata gate). The number is the revision of Meta's shared `www`
/// build, so it is the same one `sw.js` reports.
///
/// It is not a published contract: the field can be renamed or dropped, and a
/// failure here reaches the caller as an ordinary network failure. No headers
/// are sent because a browser sets its own and discards anything set here.
#[allow(dead_code)]
const SDK_SOURCE: VersionSource = VersionSource {
    url: "https://connect.facebook.net/en_US/sdk.js",
    headers: &[],
    parse: parse_meta_sdk_js,
    field: "JSSDKRuntimeConfig.revision",
    // Survivable, unlike the source above: this host is on the common
    // tracker blocklists, so content blockers and corporate DNS refuse it by
    // default. That is the expected condition for a large share of browser
    // users, not evidence anything broke, and failing closed on it would turn
    // an ad blocker into a client that never connects.
    fallback_on_failure: true,
};

/// One source per target, not a fallback chain: falling back would hide a real
/// break of the primary source, which is the thing worth hearing about.
const fn version_source() -> VersionSource {
    #[cfg(target_family = "wasm")]
    {
        SDK_SOURCE
    }
    #[cfg(not(target_family = "wasm"))]
    {
        SW_SOURCE
    }
}

pub async fn fetch_latest_app_version(
    http_client: &Arc<dyn HttpClient>,
) -> Result<(u32, u32, u32)> {
    let source = version_source();
    let mut request = HttpRequest::get(source.url);
    for (key, value) in source.headers {
        request = request.with_header(*key, *value);
    }
    let response = http_client
        .execute(request)
        .await
        .map_err(|e| anyhow!("HTTP request to {} failed: {}", source.url, e))?;

    // `HttpClient` returns a non-2xx as a response, so name the status here
    // instead of letting an error page fall through to a parse failure.
    if response.status_code >= HTTP_STATUS_REDIRECTION_START {
        let status = response.status_code;
        return Err(crate::http::HttpStatusError { status }.into_error(format!(
            "HTTP request to {} returned status {status}",
            source.url
        )));
    }

    // A body that will not decode still came from a source that answered, so
    // it is the source's shape being wrong, not the source being out of reach.
    let body_str = response.body_string().map_err(|e| {
        anyhow::Error::new(VersionShapeError::Body {
            url: source.url,
            detail: e.to_string(),
        })
    })?;

    (source.parse)(&body_str).ok_or_else(|| {
        anyhow::Error::new(VersionShapeError::Field {
            field: source.field,
            url: source.url,
        })
    })
}

/// A source that answered but did not carry the version where it should be.
/// Typed so the resolution can tell it from a source it never reached: one is
/// routine, the other is the source having changed shape.
#[derive(Debug, thiserror::Error)]
enum VersionShapeError {
    #[error("could not decode the response body from {url}: {detail}")]
    Body { url: &'static str, detail: String },
    #[error("could not find '{field}' in the response from {url}")]
    Field {
        field: &'static str,
        url: &'static str,
    },
}

/// What a session settles for when a survivable source fails: the version the
/// device already holds, and whether that is the compiled-in one.
fn fallback_to_device_version(
    device: &wacore::store::Device,
    reason: AppVersionFallbackReason,
) -> AppVersionFallback {
    let version = (
        device.app_version_primary,
        device.app_version_secondary,
        device.app_version_tertiary,
    );
    AppVersionFallback::builder()
        .version(version)
        // Compared, not inferred from the fetch stamp: `with_version` also
        // stamps one, so a device that only ever carried an override would
        // otherwise be reported as having resolved a version.
        .compiled_default(version == WA_WEB_VERSION)
        .reason(reason)
        .build()
}

/// The fallback a resolution that never finished settles for, or `None` when
/// this target's source is one whose failure is fatal. A request left hanging
/// is unreachability with a longer wait, so it has to reach the same answer as
/// a refused one rather than becoming a connect timeout.
pub(crate) fn fallback_for_unreachable_source(
    device: &wacore::store::Device,
) -> Option<AppVersionFallback> {
    version_source()
        .fallback_on_failure
        .then(|| fallback_to_device_version(device, AppVersionFallbackReason::SourceUnreachable))
}

/// Resolves the app version and persists it, returning the fallback the
/// session settled for, if any. `Ok(None)` is the normal outcome: freshly
/// fetched, served from the 24 h cache, or supplied by the caller.
pub async fn resolve_and_update_version(
    persistence_manager: &Arc<PersistenceManager>,
    http_client: &Arc<dyn HttpClient>,
    override_version: Option<(u32, u32, u32)>,
) -> Result<Option<AppVersionFallback>> {
    if let Some((p, s, t)) = override_version {
        debug!("Using user-provided override version: {}.{}.{}", p, s, t);
        persistence_manager
            .process_command(DeviceCommand::SetAppVersion((p, s, t)))
            .await;
        return Ok(None);
    }

    let device = persistence_manager.get_device_snapshot();
    let last_fetched_ms = device.app_version_last_fetched_ms;

    let needs_fetch = if last_fetched_ms == 0 {
        true
    } else {
        match wacore::time::from_millis(last_fetched_ms) {
            Some(last_fetched_dt) => {
                wacore::time::now_utc().signed_duration_since(last_fetched_dt)
                    > chrono::Duration::hours(24)
            }
            None => true,
        }
    };

    if needs_fetch {
        debug!("WhatsApp version is stale or missing, fetching latest...");
        // `.context`, not `anyhow!("… {e}")`: reformatting builds a new error
        // and drops the chain, so the `HttpStatusError` the fetch attached
        // would never reach a caller holding a `ConnectError::Version`. The
        // message is the same either way; only the recoverability differs.
        let fetched = fetch_latest_app_version(http_client)
            .await
            .context("Failed to fetch latest WhatsApp version");

        let (p, s, t) = match fetched {
            Ok(version) => version,
            Err(e) if version_source().fallback_on_failure => {
                // A source that answered with the wrong shape is not the same
                // as one that never answered, and burying that would undo the
                // parser's fail-closed behaviour, so the two are reported
                // apart. Both stay survivable: a DNS sinkhole that serves a
                // page rather than refusing the request lands here too.
                let reason = if e.chain().any(|c| c.is::<VersionShapeError>()) {
                    AppVersionFallbackReason::SourceUnparsable
                } else {
                    AppVersionFallbackReason::SourceUnreachable
                };
                // The stamp is deliberately left alone, so the next connect
                // tries the source again instead of waiting out a 24 h cache
                // that no successful fetch ever filled.
                let fallback = fallback_to_device_version(&device, reason);
                debug!(
                    "Version source unreachable, connecting on {:?}: {e:#}",
                    fallback.version
                );
                return Ok(Some(fallback));
            }
            Err(e) => return Err(e),
        };

        debug!("Fetched latest version: {}.{}.{}", p, s, t);
        persistence_manager
            .process_command(DeviceCommand::SetAppVersion((p, s, t)))
            .await;
    } else {
        debug!(
            "Using cached version: {}.{}.{}",
            device.app_version_primary, device.app_version_secondary, device.app_version_tertiary
        );
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::ErrorChainExt;
    use crate::http::HttpResponse;

    struct StatusOnlyHttpClient(u16);

    #[derive(Default)]
    struct HeaderCapturingHttpClient {
        seen: std::sync::Mutex<Option<std::collections::HashMap<String, String>>>,
        url: std::sync::Mutex<Option<String>>,
    }

    #[async_trait::async_trait]
    impl HttpClient for HeaderCapturingHttpClient {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse> {
            *self.url.lock().unwrap() = Some(request.url.clone());
            *self.seen.lock().unwrap() = Some(request.headers);
            Ok(HttpResponse {
                status_code: 200,
                body: b"client_revision:12345;".to_vec(),
            })
        }
    }

    #[async_trait::async_trait]
    impl HttpClient for StatusOnlyHttpClient {
        async fn execute(&self, _request: HttpRequest) -> Result<HttpResponse> {
            Ok(HttpResponse {
                status_code: self.0,
                body: b"<html>error</html>".to_vec(),
            })
        }
    }

    /// The status has to survive the layer a real caller actually goes through.
    /// `Client::connect()` reaches this fetch via `resolve_and_update_version`,
    /// which used to reformat the error into a new one — the message kept the
    /// status while the typed cause was dropped, so `http_status()` answered
    /// `None` on the only path that matters.
    #[tokio::test]
    async fn the_version_status_survives_resolve_and_update() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("in-memory persistence"),
        );
        let http_client: Arc<dyn HttpClient> = Arc::new(StatusOnlyHttpClient(403));

        let err = resolve_and_update_version(&persistence_manager, &http_client, None)
            .await
            .expect_err("a 403 must not resolve a version");

        let cause: &(dyn std::error::Error + 'static) = err.as_ref();
        assert_eq!(
            cause.http_status(),
            Some(403),
            "the wrap must add context, not rebuild the error, got: {err:?}"
        );
    }

    /// A non-2xx sw.js fetch arrives as a response, so the status has to be
    /// named here — not swallowed into a "no client_revision" parse failure.
    #[tokio::test]
    async fn fetch_version_reports_the_http_status_on_a_non_2xx_response() {
        let http_client: Arc<dyn HttpClient> = Arc::new(StatusOnlyHttpClient(403));
        let err = fetch_latest_app_version(&http_client)
            .await
            .expect_err("a 403 must not be parsed as a version document");
        // Recoverable by type, not only readable — the same contract the media
        // paths answer, so a caller does not have to know which fetch it was.
        let cause: &(dyn std::error::Error + 'static) = err.as_ref();
        assert_eq!(cause.http_status(), Some(403), "got: {err:?}");
        assert!(
            err.to_string().contains("403"),
            "the error must name the status, got: {err}"
        );
    }

    /// The header is the whole saving, so pin both halves: that the fetch
    /// sends it, and that a version still resolves with it set. Gated off wasm,
    /// where the source is the SDK bundle and neither the header nor this body
    /// applies.
    #[cfg(not(target_family = "wasm"))]
    #[tokio::test]
    async fn the_version_fetch_declines_to_leave_a_pooled_connection() {
        let capturing = Arc::new(HeaderCapturingHttpClient::default());
        let http_client: Arc<dyn HttpClient> = capturing.clone();

        let version = fetch_latest_app_version(&http_client)
            .await
            .expect("a 200 sw.js resolves a version");

        assert_eq!(version, (2, 3000, 12345));
        let headers = capturing
            .seen
            .lock()
            .unwrap()
            .clone()
            .expect("the fetch issued a request");
        assert_eq!(
            headers.get("connection").map(String::as_str),
            Some("close"),
            "got: {headers:?}"
        );
        // The chosen source has to be the one actually requested, not just the
        // one the constant names.
        assert_eq!(
            capturing.url.lock().unwrap().as_deref(),
            Some(version_source().url)
        );
    }

    /// The asymmetry is the design: an unreachable `sw.js` is a real break and
    /// must stay fatal, while the browser source is blocked by default by
    /// common content blockers, so failing closed there would turn an ad
    /// blocker into a client that never connects.
    #[test]
    fn only_the_browser_source_falls_back() {
        let by_source = [
            (SW_SOURCE.url, SW_SOURCE.fallback_on_failure),
            (SDK_SOURCE.url, SDK_SOURCE.fallback_on_failure),
        ];
        assert_eq!(
            by_source.map(|(_, falls_back)| falls_back),
            [false, true],
            "got: {by_source:?}"
        );
    }

    /// The failure path of the fatal source: an unreachable source is an error,
    /// and no fallback is reported in its place.
    #[cfg(not(target_family = "wasm"))]
    #[tokio::test]
    async fn a_fatal_source_failure_resolves_no_version() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("in-memory persistence"),
        );
        let http_client: Arc<dyn HttpClient> = Arc::new(StatusOnlyHttpClient(500));

        resolve_and_update_version(&persistence_manager, &http_client, None)
            .await
            .expect_err("a source that cannot answer must not resolve a version");
    }

    /// The happy path reports no fallback, so `Some` stays the whole signal.
    #[tokio::test]
    async fn a_resolved_version_reports_no_fallback() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("in-memory persistence"),
        );
        let http_client: Arc<dyn HttpClient> = Arc::new(HeaderCapturingHttpClient::default());

        let fallback = resolve_and_update_version(&persistence_manager, &http_client, None)
            .await
            .expect("a 200 resolves a version");
        assert!(fallback.is_none(), "got: {fallback:?}");
    }

    /// An override is the caller's own answer, not a fallback.
    #[tokio::test]
    async fn an_override_reports_no_fallback() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("in-memory persistence"),
        );
        let http_client: Arc<dyn HttpClient> = Arc::new(StatusOnlyHttpClient(500));

        let fallback =
            resolve_and_update_version(&persistence_manager, &http_client, Some((2, 3000, 7)))
                .await
                .expect("an override resolves without fetching");
        assert!(fallback.is_none(), "got: {fallback:?}");
    }

    /// The whole point of the browser fallback: a blocked source still yields a
    /// connectable session, and says so in a way a consumer can act on rather
    /// than a log line. Driven through the source description so it runs on
    /// every target, since the wasm target this describes has no test runner.
    #[tokio::test]
    async fn a_survivable_source_failure_reports_the_version_it_settled_for() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("in-memory persistence"),
        );
        let device = persistence_manager.get_device_snapshot();
        let compiled = (
            device.app_version_primary,
            device.app_version_secondary,
            device.app_version_tertiary,
        );

        let fallback =
            fallback_to_device_version(&device, AppVersionFallbackReason::SourceUnreachable);
        assert_eq!(fallback.version, compiled);
        assert_eq!(fallback.version, WA_WEB_VERSION);
        assert!(
            fallback.compiled_default,
            "a device still on the compiled version says so"
        );
        assert_eq!(fallback.reason, AppVersionFallbackReason::SourceUnreachable);
    }

    /// `compiled_default` is a comparison, not an inference from the fetch
    /// stamp: `with_version` stamps one too, so a device carrying only an
    /// override must not be reported as running the compiled version.
    #[tokio::test]
    async fn an_overridden_version_is_not_reported_as_the_compiled_default() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("in-memory persistence"),
        );
        let override_version = (WA_WEB_VERSION.0, WA_WEB_VERSION.1, WA_WEB_VERSION.2 + 1);
        persistence_manager
            .process_command(DeviceCommand::SetAppVersion(override_version))
            .await;

        let device = persistence_manager.get_device_snapshot();
        assert_ne!(
            device.app_version_last_fetched_ms, 0,
            "the command stamps a fetch time even for an override, which is why \
             the flag cannot be read off it"
        );

        let fallback =
            fallback_to_device_version(&device, AppVersionFallbackReason::SourceUnreachable);
        assert_eq!(fallback.version, override_version);
        assert!(!fallback.compiled_default);
    }

    /// A source that answers with the wrong shape is news, and a source that
    /// never answers is routine. Both survive, and they must not look alike.
    #[test]
    fn a_shape_failure_is_reported_apart_from_an_unreachable_source() {
        let missing_field = anyhow::Error::new(VersionShapeError::Field {
            field: "JSSDKRuntimeConfig.revision",
            url: "https://example.invalid/sdk.js",
        })
        .context("Failed to fetch latest WhatsApp version");
        assert!(missing_field.chain().any(|c| c.is::<VersionShapeError>()));

        // A body that will not decode is the same class: the source answered.
        let bad_body = anyhow::Error::new(VersionShapeError::Body {
            url: "https://example.invalid/sdk.js",
            detail: "invalid utf-8".to_owned(),
        })
        .context("Failed to fetch latest WhatsApp version");
        assert!(bad_body.chain().any(|c| c.is::<VersionShapeError>()));

        let unreachable = anyhow!("HTTP request to https://example.invalid/sdk.js failed: refused")
            .context("Failed to fetch latest WhatsApp version");
        assert!(!unreachable.chain().any(|c| c.is::<VersionShapeError>()));
    }

    /// A hung source is unreachability with a longer wait, so the fatal source
    /// still refuses and the survivable one still settles for a version.
    #[test]
    fn a_timed_out_source_falls_back_exactly_where_a_refused_one_does() {
        let device = wacore::store::Device::default();
        let timed_out = fallback_for_unreachable_source(&device);
        assert_eq!(
            timed_out.is_some(),
            version_source().fallback_on_failure,
            "the timeout path must not disagree with the source policy"
        );
        if let Some(fallback) = timed_out {
            assert_eq!(fallback.reason, AppVersionFallbackReason::SourceUnreachable);
        }
    }

    /// Every header name the Fetch spec forbids a page from setting. A source
    /// that depends on one of these cannot work in a browser, whatever the
    /// server answers.
    const FORBIDDEN_HEADER_PREFIXES: &[&str] = &[
        "sec-fetch-",
        "connection",
        "user-agent",
        "origin",
        "host",
        "referer",
    ];

    /// The wasm source exists precisely because the default one cannot be
    /// requested from a page, so it must not repeat the mistake.
    #[test]
    fn the_browser_source_sends_no_header_a_page_is_forbidden_to_set() {
        for (key, _) in SDK_SOURCE.headers {
            let key = key.to_ascii_lowercase();
            assert!(
                !FORBIDDEN_HEADER_PREFIXES
                    .iter()
                    .any(|forbidden| key.starts_with(forbidden)),
                "the wasm version source cannot depend on '{key}'"
            );
        }
    }

    /// The saving header is what turns a 400 into a 200, so pin that the
    /// non-browser source still sends it.
    #[test]
    fn the_default_source_is_the_service_worker_with_its_fetch_metadata_header() {
        assert_eq!(SW_SOURCE.url, "https://web.whatsapp.com/sw.js");
        assert!(
            SW_SOURCE.headers.contains(&("sec-fetch-site", "none")),
            "got: {:?}",
            SW_SOURCE.headers
        );
    }

    /// The choice of source is a compile-time fact, so assert it as one.
    #[test]
    fn the_source_is_chosen_by_target() {
        let source = version_source();
        if cfg!(target_family = "wasm") {
            assert_eq!(source.url, SDK_SOURCE.url);
        } else {
            assert_eq!(source.url, SW_SOURCE.url);
        }
    }

    /// A source that fails must not spend the 24 h cache: the next connect has
    /// to be free to try again, and a still-valid stamp has to survive.
    #[tokio::test]
    async fn a_failed_fetch_leaves_a_valid_cache_stamp_alone() {
        let persistence_manager = Arc::new(
            PersistenceManager::new(crate::test_utils::create_test_backend().await)
                .await
                .expect("in-memory persistence"),
        );
        let fresh_stamp = wacore::time::now_utc().timestamp_millis();
        persistence_manager
            .process_command(DeviceCommand::SetAppVersion((2, 3000, 111)))
            .await;
        let stamp_after_success = persistence_manager
            .get_device_snapshot()
            .app_version_last_fetched_ms;
        assert!(
            stamp_after_success >= fresh_stamp,
            "a successful resolve stamps the cache"
        );

        let http_client: Arc<dyn HttpClient> = Arc::new(StatusOnlyHttpClient(500));
        resolve_and_update_version(&persistence_manager, &http_client, None)
            .await
            .expect("a valid cache is used without fetching");

        let device = persistence_manager.get_device_snapshot();
        assert_eq!(
            device.app_version_last_fetched_ms, stamp_after_success,
            "the cached stamp must survive"
        );
        assert_eq!(
            (
                device.app_version_primary,
                device.app_version_secondary,
                device.app_version_tertiary
            ),
            (2, 3000, 111),
            "the cached version must survive"
        );
    }

    #[test]
    fn test_parse_sw_js_client_revision_quoted() {
        let s = r#"var x = {"client_revision": "123456"};"#;
        assert_eq!(parse_sw_js(s), Some((2, 3000, 123456)));
    }

    #[test]
    fn test_parse_sw_js_client_revision_unquoted() {
        let s = r#"client_revision:12345;"#;
        assert_eq!(parse_sw_js(s), Some((2, 3000, 12345)));
    }

    #[test]
    fn test_parse_sw_js_assets_fallback() {
        let s = "... assets-manifest-98765 ...";
        assert_eq!(parse_sw_js(s), Some((2, 3000, 0)));
    }

    #[test]
    fn test_parse_sw_js_realistic_sw_js() {
        let s = r#"__DEV__=0;/*FB_PKG_DELIM*/
self.__swData=JSON.parse(/*BTDS*/"{\"dynamic_data\":{\"dynamic_modules\":{\"cr:375\":{\"__rc\":[\"WAWebFtsLightClient\",null]},\"cr:1126\":{\"__rc\":[\"TimeSliceSham\",null]},\"cr:4122\":{\"__rc\":[null,null]},\"cr:4324\":{\"__rc\":[null,null]},\"cr:4533\":{\"__rc\":[null,null]},\"cr:4722\":{\"__rc\":[null,null]},\"cr:4941\":{\"__rc\":[null,null]},\"cr:5151\":{\"__rc\":[null,null]},\"cr:5292\":{\"__rc\":[null,null]},\"cr:5411\":{\"__rc\":[null,null]},\"cr:5664\":{\"__rc\":[null,null]},\"cr:6640\":{\"__rc\":[null,null]},\"cr:8978\":{\"__rc\":[null,null]},\"cr:9565\":{\"__rc\":[null,null]},\"cr:10197\":{\"__rc\":[null,null]},\"cr:10198\":{\"__rc\":[null,null]},\"cr:17160\":{\"__rc\":[null,null]},\"cr:17219\":{\"__rc\":[null,null]},\"cr:21223\":{\"__rc\":[null,null]},\"IntlCurrentLocale\":{\"code\":\"en_US\"},\"WAWebSwResources\":{\"wa_default_notification_icon\":\"https:\\\/\\\/static.whatsapp.net\\\/rsrc.php\\\/v4\\\/yX\\\/r\\\/JYPizEwERE4.png\"},\"SiteData\":{\"server_revision\":1026131876,\"client_revision\":1026131876,\"push_phase\":\"C3\",\"pkg_cohort\":\"BP:DEFAULT\",\"haste_session\":\"20320.BP:DEFAULT.2.0...0\",\"pr\":1,\"manifest_base_uri\":\"https:\\\/\\\/static.whatsapp.net\",\"manifest_origin\":null,\"manifest_version_prefix\":null,\"be_one_ahead\":false,\"is_rtl\":false,\"is_experimental_tier\":false,\"is_jit_warmed_up\":true,\"hsi\":\"7540800780599698108\",\"semr_host_bucket\":\"3\",\"bl_hash_version\":2,\"comet_env\":0,\"wbloks_env\":false,\"ef_page\":null,\"compose_bootloads\":false,\"spin\":4,\"__spin_r\":1026131876,\"__spin_b\":\"trunk\",\"__spin_t\":1755729499,\"vip\":\"2a03:2880:f205:c5:face:b00c:0:167\"}},\"hsdp\":{\"bxData\":{\"32186\":{\"uri\":\"https:\\\/\\\/static.whatsapp.net\\\/rsrc.php\\\/v4\\\/yR\\\/r\\\/aCneqBxOSs-.png\"},\"32187\":{\"uri\":\"https:\\\/\\\/static.whatsapp.net\\\/rsrc.php\\\/v4\\\/yT\\\/r\\\/s0hoT-Vu8xP.png\"}},\"gkxData\":{\"4112\":{\"result\":false,\"hash\":null},\"5943\":{\"result\":false,\"hash\":null},\"7685\":{\"result\":false,\"hash\":null},\"10314\":{\"result\":false,\"hash\":null},\"16915\":{\"result\":false,\"hash\":null},\"16928\":{\"result\":false,\"hash\":null},\"17038\":{\"result\":false,\"hash\":null},\"26256\":{\"result\":false,\"hash\":null},\"26258\":{\"result\":true,\"hash\":null},\"26259\":{\"result\":false,\"hash\":null}},\"justknobxData\":{\"371\":{\"r\":true},\"1050\":{\"r\":false},\"1617\":{\"r\":165},\"1618\":{\"r\":8},\"1619\":{\"r\":1},\"1620\":{\"r\":2},\"1621\":{\"r\":4},\"1622\":{\"r\":0},\"1623\":{\"r\":6},\"1624\":{\"r\":1},\"1662\":{\"r\":2},\"1663\":{\"r\":14},\"1664\":{\"r\":2},\"1854\":{\"r\":false},\"2237\":{\"r\":false},\"2337\":{\"r\":false},\"2517\":{\"r\":true},\"3717\":{\"r\":1},\"4952\":{\"r\":true}}}}}");

      if (self.trustedTypes && self.trustedTypes.createPolicy) {
        const escapeScriptURLPolicy = self.trustedTypes.createPolicy("workerPolicy", {
          createScriptURL: url => url
        });
        importScripts(escapeScriptURLPolicy.createScriptURL("https:\/\/static.whatsapp.net\/rsrc.php\/v4\/yq\/r\/odrxy-7zVX8.js"));
      } else {
         importScripts("https:\/\/static.whatsapp.net\/rsrc.php\/v4\/yq\/r\/odrxy-7zVX8.js");
      }"#;

        assert_eq!(parse_sw_js(s), Some((2, 3000, 1026131876)));
    }
}
