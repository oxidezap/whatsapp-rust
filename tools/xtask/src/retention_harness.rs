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

fn fixture(net: &str, hook: &str, source: &str) -> Result<(String, String)> {
    let mut hook = hook.to_owned();
    let mut source = source.to_owned();
    if !hook.contains("pub type HookResult") {
        ensure!(
            hook.contains("use anyhow::Result;"),
            "unexpected durability hook API"
        );
        hook.push_str("\npub type HookResult<T = ()> = Result<T>;\n");
    }
    if !net.contains("pub type TransportError =") {
        let current = "        self.send_fixture(data).await\n";
        ensure!(
            source.matches(current).count() == 1,
            "unexpected fixture transport"
        );
        source = source.replacen(
            current,
            "        self.send_fixture(data).await.map_err(anyhow::Error::from_boxed)\n",
            1,
        );
    }
    Ok((hook, source))
}

pub fn run(root: &Path) -> Result<()> {
    let net = std::fs::read_to_string(root.join("wacore/src/net.rs"))?;
    let http_path = root.join("src/http.rs");
    let transport_path = root.join("src/transport.rs");
    let http = std::fs::read_to_string(&http_path).context("read base HTTP API")?;
    let transport = std::fs::read_to_string(&transport_path).context("read base transport API")?;
    let (adapted_http, adapted_transport) = aliases(&net, &http, &transport)?;
    let hook_path = root.join("src/types/durability_hook.rs");
    let fixture_path = root.join("src/bench_support/connected_idle.rs");
    let hook = std::fs::read_to_string(&hook_path).context("read base durability hook API")?;
    let source = std::fs::read_to_string(&fixture_path).context("read connected fixture")?;
    let (adapted_hook, adapted_fixture) = fixture(&net, &hook, &source)?;
    if adapted_http != http {
        std::fs::write(http_path, adapted_http)?;
    }
    if adapted_transport != transport {
        std::fs::write(transport_path, adapted_transport)?;
    }
    if adapted_hook != hook {
        std::fs::write(hook_path, adapted_hook)?;
    }
    if adapted_fixture != source {
        std::fs::write(fixture_path, adapted_fixture)?;
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

    #[test]
    fn adapts_legacy_hook_and_transport_without_discarding_causes() {
        let (hook, source) = fixture(
            "",
            "use anyhow::Result;",
            "        self.send_fixture(data).await\n",
        )
        .unwrap();
        assert!(hook.contains("pub type HookResult<T = ()> = Result<T>;"));
        assert!(source.contains(".map_err(anyhow::Error::from_boxed)"));
    }

    #[test]
    fn leaves_current_fixture_unchanged() {
        let hook = "pub type HookResult<T> = Result<T, HookError>;";
        let source = "        self.send_fixture(data).await\n";
        assert_eq!(
            fixture("pub type TransportError =", hook, source).unwrap(),
            (hook.into(), source.into())
        );
    }

    #[test]
    fn rejects_unrecognized_fixture_or_hook() {
        assert!(fixture("", "use anyhow::Result;", "unexpected").is_err());
        assert!(fixture("pub type TransportError =", "unexpected", "").is_err());
    }
}
