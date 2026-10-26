use ureq::tls::{TlsConfig, TlsProvider};

use wa_store::capture::{FetchError, HttpClient, HttpResponse, RedirectResponse};

/// Use rustls with ureq's ring backend and bundled WebPKI roots.
fn tls_config() -> TlsConfig {
    TlsConfig::builder().provider(TlsProvider::Rustls).build()
}

/// The native [`HttpClient`]: two reusable `ureq::Agent`s.
///
/// Both are configured with `http_status_as_error(false)` so a non-2xx response
/// comes back as an [`HttpResponse`] with that status (not an error), keeping the
/// status policy in the calling discovery/download code. They differ only in
/// redirect handling: `agent` follows them (what discovery and download want),
/// `no_redirect` stops at the 3xx so [`HttpClient::get_redirect`] can report the
/// `Location` rather than fetching it. Redirect following is agent-level config in
/// ureq, so it takes a second agent, not a per-request flag.
#[derive(Clone)]
pub struct UreqClient {
    agent: ureq::Agent,
    no_redirect: ureq::Agent,
}

impl UreqClient {
    pub fn new() -> Self {
        let agent: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .tls_config(tls_config())
            .build()
            .into();
        let no_redirect: ureq::Agent = ureq::config::Config::builder()
            .http_status_as_error(false)
            .max_redirects(0)
            .tls_config(tls_config())
            .build()
            .into();
        Self { agent, no_redirect }
    }
}

impl Default for UreqClient {
    fn default() -> Self {
        Self::new()
    }
}

impl HttpClient for UreqClient {
    fn get(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: u64,
    ) -> Result<HttpResponse, FetchError> {
        let mut req = self.agent.get(url);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let resp = req
            .call()
            .map_err(|e| FetchError::new(format!("GET {url}: {e}")))?;
        let status = resp.status().as_u16();
        let body = resp
            .into_body()
            .into_with_config()
            .limit(max_bytes)
            .read_to_vec()
            .map_err(|e| FetchError::new(format!("read body of {url}: {e}")))?;
        Ok(HttpResponse { status, body })
    }

    fn get_redirect(
        &self,
        url: &str,
        headers: &[(&str, &str)],
        max_bytes: u64,
    ) -> Result<RedirectResponse, FetchError> {
        let mut req = self.no_redirect.get(url);
        for (name, value) in headers {
            req = req.header(*name, *value);
        }
        let resp = req
            .call()
            .map_err(|e| FetchError::new(format!("GET {url}: {e}")))?;
        let status = resp.status().as_u16();
        let location = resp
            .headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string);
        let body = resp
            .into_body()
            .into_with_config()
            .limit(max_bytes)
            .read_to_vec()
            .map_err(|e| FetchError::new(format!("read body of {url}: {e}")))?;
        Ok(RedirectResponse {
            status,
            location,
            body,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::Arc;
    use std::thread;
    use std::time::Duration;

    fn server(responses: Vec<String>) -> (String, thread::JoinHandle<Vec<String>>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        let worker = thread::spawn(move || {
            let mut requests = Vec::new();
            for response in responses {
                let (mut stream, _) = listener.accept().unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut request = [0; 4096];
                let length = stream.read(&mut request).unwrap();
                assert!(length > 0);
                requests.push(String::from_utf8_lossy(&request[..length]).into_owned());
                stream.write_all(response.as_bytes()).unwrap();
            }
            requests
        });
        (url, worker)
    }

    #[test]
    fn preserves_status_and_bounds_response_body() {
        let response =
            "HTTP/1.1 404 Not Found\r\nContent-Length: 5\r\nConnection: close\r\n\r\nhello";
        let (url, worker) = server(vec![response.into(), response.into()]);
        let client = UreqClient::new();
        let fetched = client.get(&url, &[], 6).unwrap();
        assert_eq!(fetched.status, 404);
        assert_eq!(fetched.body, b"hello");
        assert!(client.get(&url, &[], 4).is_err());
        worker.join().unwrap();
    }

    #[test]
    fn redirect_probe_does_not_follow_but_normal_get_does() {
        let redirect = "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n";
        let (url, worker) = server(vec![
            redirect.into(),
            redirect.into(),
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".into(),
        ]);
        let client = UreqClient::new();
        let probe = client.get_redirect(&url, &[], 16).unwrap();
        assert_eq!(probe.status, 302);
        assert_eq!(probe.location.as_deref(), Some("/final"));
        assert_eq!(client.get(&url, &[], 16).unwrap().body, b"ok");
        worker.join().unwrap();
    }

    #[test]
    fn redirects_do_not_forward_authorization_to_another_origin() {
        let (destination, destination_worker) = server(vec![
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok".into(),
        ]);
        let (source, source_worker) = server(vec![format!(
            "HTTP/1.1 302 Found\r\nLocation: {destination}/final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
        )]);
        UreqClient::new()
            .get(&source, &[("Authorization", "Bearer fixture")], 16)
            .unwrap();
        let original = source_worker.join().unwrap();
        let redirected = destination_worker.join().unwrap();
        assert!(
            original[0]
                .to_ascii_lowercase()
                .contains("authorization: bearer fixture")
        );
        assert!(
            !redirected[0]
                .to_ascii_lowercase()
                .contains("authorization:")
        );
    }

    #[test]
    fn rejects_untrusted_tls_certificate() {
        let certificate = rcgen::generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let key =
            rustls::pki_types::PrivatePkcs8KeyDer::from(certificate.signing_key.serialize_der());
        let config = rustls::ServerConfig::builder_with_provider(Arc::new(
            rustls::crypto::ring::default_provider(),
        ))
        .with_safe_default_protocol_versions()
        .unwrap()
        .with_no_client_auth()
        .with_single_cert(vec![certificate.cert.der().clone()], key.into())
        .unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!(
            "https://localhost:{}",
            listener.local_addr().unwrap().port()
        );
        let worker = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(5)))
                .unwrap();
            let mut connection = rustls::ServerConnection::new(Arc::new(config)).unwrap();
            assert!(connection.complete_io(&mut stream).is_err());
        });
        let error = UreqClient::new().get(&url, &[], 16).unwrap_err();
        assert!(error.to_string().contains("certificate"), "{error}");
        worker.join().unwrap();
    }
}
