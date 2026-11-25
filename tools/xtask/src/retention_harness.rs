use anyhow::{Context, Result, ensure};
use std::path::Path;

fn aliases(net: &str, http: &str, transport: &str) -> Result<(String, String)> {
    let mut http = http.to_owned();
    let mut transport = transport.to_owned();
    if !net.contains("pub type TransportError =") {
        ensure!(
            !transport.contains("pub type TransportError"),
            "unexpected transport API"
        );
        transport.push_str("\npub type TransportError = anyhow::Error;\n");
    }
    if !net.contains("pub enum HttpError {") {
        ensure!(!http.contains("pub type HttpError"), "unexpected HTTP API");
        http.push_str("\npub type HttpError = anyhow::Error;\n");
    }
    Ok((http, transport))
}

pub fn run(root: &Path) -> Result<()> {
    let net = std::fs::read_to_string(root.join("wacore/src/net.rs"))?;
    let http_path = root.join("src/http.rs");
    let transport_path = root.join("src/transport.rs");
    let http = std::fs::read_to_string(&http_path).context("read base HTTP API")?;
    let transport = std::fs::read_to_string(&transport_path).context("read base transport API")?;
    let (adapted_http, adapted_transport) = aliases(&net, &http, &transport)?;
    if adapted_http != http {
        std::fs::write(http_path, adapted_http)?;
    }
    if adapted_transport != transport {
        std::fs::write(transport_path, adapted_transport)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_aliases_for_legacy_error_contracts() {
        let (http, transport) = aliases("", "http source", "transport source").unwrap();
        assert_eq!(http, "http source\npub type HttpError = anyhow::Error;\n");
        assert_eq!(
            transport,
            "transport source\npub type TransportError = anyhow::Error;\n"
        );
    }

    #[test]
    fn leaves_current_contracts_unchanged() {
        let net = "pub type TransportError =\npub enum HttpError {";
        let (http, transport) = aliases(net, "http source", "transport source").unwrap();
        assert_eq!(http, "http source");
        assert_eq!(transport, "transport source");
    }

    #[test]
    fn rejects_unexpected_existing_aliases() {
        assert!(aliases("", "pub type HttpError = Other;", "transport source").is_err());
    }
}
