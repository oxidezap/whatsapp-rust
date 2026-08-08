use crate::http::{HTTP_STATUS_REDIRECTION_START, HttpClient, HttpRequest};
use crate::store::commands::DeviceCommand;
use crate::store::persistence_manager::PersistenceManager;
use anyhow::{Context as _, Result, anyhow};
use log::debug;
use std::sync::Arc;

pub use wacore::version::parse_sw_js;

const SW_URL: &str = "https://web.whatsapp.com/sw.js";

pub async fn fetch_latest_app_version(
    http_client: &Arc<dyn HttpClient>,
) -> Result<(u32, u32, u32)> {
    // `Connection: close` because this fetch runs at most once a day per
    // device: a pooled idle TLS connection would be retained for the rest of
    // the session and buys nothing back before it is purged.
    let request = HttpRequest::get(SW_URL).with_header("sec-fetch-site", "none")
    .with_header("connection", "close")
    .with_header(
        "user-agent",
        "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36"
    );
    let response = http_client
        .execute(request)
        .await
        .map_err(|e| anyhow!("HTTP request to {} failed: {}", SW_URL, e))?;

    // `HttpClient` returns a non-2xx as a response, so name the status here
    // instead of letting an error page fall through to a "no client_revision"
    // parse failure.
    if response.status_code >= HTTP_STATUS_REDIRECTION_START {
        let status = response.status_code;
        return Err(crate::http::HttpStatusError { status }
            .into_error(format!("HTTP request to {SW_URL} returned status {status}")));
    }

    let body_str = response
        .body_string()
        .map_err(|e| anyhow!("Failed to decode response body: {}", e))?;

    parse_sw_js(&body_str)
        .ok_or_else(|| anyhow!("Could not find 'client_revision' version in sw.js response"))
}

pub async fn resolve_and_update_version(
    persistence_manager: &Arc<PersistenceManager>,
    http_client: &Arc<dyn HttpClient>,
    override_version: Option<(u32, u32, u32)>,
) -> Result<()> {
    if let Some((p, s, t)) = override_version {
        debug!("Using user-provided override version: {}.{}.{}", p, s, t);
        persistence_manager
            .process_command(DeviceCommand::SetAppVersion((p, s, t)))
            .await;
        return Ok(());
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
        let (p, s, t) = fetch_latest_app_version(http_client)
            .await
            .context("Failed to fetch latest WhatsApp version")?;
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

    Ok(())
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
    }

    #[async_trait::async_trait]
    impl HttpClient for HeaderCapturingHttpClient {
        async fn execute(&self, request: HttpRequest) -> Result<HttpResponse> {
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
    /// sends it, and that a version still resolves with it set.
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
