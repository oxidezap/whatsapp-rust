use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use std::path::Path;
use std::process::Command;
use xtask_support::capture;

use crate::consumers::Lane;

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum PreparingConsumers {
    Local,
    ApiHosts,
}

pub fn api_hosts_head(root: &Path, lane: Lane, toolchain: &str) -> Result<String> {
    let expected = std::env::var("EXPECTED_SHA").context("delegation requires EXPECTED_SHA")?;
    let actual = capture(
        Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(root),
    )?;
    verify_context(
        &std::env::var("GITHUB_EVENT_NAME").context("delegation requires a CI event")?,
        &std::env::var("GITHUB_REF").context("delegation requires GITHUB_REF")?,
        std::env::var("GITHUB_BASE_REF").ok().as_deref(),
        &expected,
        std::str::from_utf8(&actual.stdout)?.trim(),
    )?;
    let source = head_file(root, ".github/workflows/consumers.yml")?;
    let pin: toml::Value = toml::from_str(&head_file(root, "rust-toolchain.toml")?)?;
    let driver = pin["toolchain"]["channel"]
        .as_str()
        .context("driver toolchain pin")?;
    verify_owner(&source, lane, toolchain, driver)?;
    Ok(expected)
}

fn head_file(root: &Path, path: &str) -> Result<String> {
    Ok(String::from_utf8(
        capture(
            Command::new("git")
                .args(["show", &format!("HEAD:{path}")])
                .current_dir(root),
        )?
        .stdout,
    )?)
}

fn verify_context(
    event: &str,
    git_ref: &str,
    base_ref: Option<&str>,
    expected: &str,
    actual: &str,
) -> Result<()> {
    ensure!(
        (event == "push" && git_ref == "refs/heads/main")
            || (event == "pull_request" && base_ref == Some("main")),
        "current consumers can be delegated only for main pushes or pull requests targeting main"
    );
    ensure!(
        super::sha(expected) && actual == expected,
        "delegated consumers must qualify the exact requested head SHA"
    );
    Ok(())
}

fn normalized(source: &str) -> String {
    source
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.trim_start().starts_with('#'))
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n")
}

// Delegation admits these fixed workflow blocks. Configuration changes require
// a coverage review before expensive qualification starts.
fn block(source: &str, marker: &str) -> Result<String> {
    let lines: Vec<_> = source.lines().collect();
    let matches: Vec<_> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| **line == marker)
        .collect();
    ensure!(
        matches.len() == 1,
        "expected one owner workflow block {marker}"
    );
    let start = matches[0].0;
    let indent = marker.len() - marker.trim_start().len();
    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find(|(_, line)| {
            !line.trim().is_empty()
                && !line.trim_start().starts_with('#')
                && line.len() - line.trim_start().len() <= indent
        })
        .map_or(lines.len(), |(index, _)| index);
    Ok(normalized(&lines[start..end].join("\n")))
}

fn require_block(source: &str, marker: &str, expected: &str) -> Result<()> {
    ensure!(
        block(source, marker)? == normalized(expected),
        "API host ownership changed at {marker}; review event, head and compiler coverage before delegating"
    );
    Ok(())
}

fn verify_owner(source: &str, lane: Lane, toolchain: &str, driver: &str) -> Result<()> {
    let lane_name = match lane {
        Lane::Native => "native",
        Lane::Msrv => "msrv",
        Lane::Wasm => {
            anyhow::bail!("WASM checkout consumers retain both maintained compiler qualifications")
        }
    };
    require_block(
        source,
        "on:",
        "on:\n  push:\n    branches: [main]\n  pull_request:\n    branches: [main]\n  workflow_call:",
    )?;
    let job = block(source, "  consumers:")?;
    let header = job.split("    steps:").next().context("API host steps")?;
    ensure!(
        header.trim_end()
            == "  consumers:\n    name: Independent API hosts (${{ matrix.lane }})\n    runs-on: ubuntu-latest\n    strategy:\n      fail-fast: false\n      matrix:\n        lane: [native, msrv]",
        "API host job matrix or conditions changed; review delegated coverage"
    );
    require_block(
        source,
        "      - uses: actions/checkout@v7",
        "      - uses: actions/checkout@v7\n        with:\n          ref: ${{ github.event.pull_request.head.sha || github.sha }}",
    )?;
    require_block(
        source,
        "      - name: Build driver",
        &format!(
            "      - name: Build driver\n        run: cargo +{driver} build --locked -p whatsapp-xtask"
        ),
    )?;
    let name = if lane == Lane::Native {
        "Run isolated native profiles and shared negative controls"
    } else {
        "Run isolated MSRV profiles and shared negative controls"
    };
    require_block(
        source,
        &format!("      - name: {name}"),
        &format!(
            "      - name: {name}\n        if: matrix.lane == '{lane_name}'\n        env:\n          CARGO_BUILD_JOBS: \"2\"\n        run: target/debug/whatsapp-xtask ci consumers run --lane {lane_name} --toolchain {toolchain}"
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const OWNER: &str = include_str!("../../../../.github/workflows/consumers.yml");

    #[test]
    fn supply_chain_binds_delegation_to_the_same_events_and_head() {
        let source = include_str!("../../../../.github/workflows/supply-chain.yml");
        require_block(source, "      - name: Validate contracts before release", "      - name: Validate contracts before release\n        if: inputs.release-check\n        run: target/debug/whatsapp-xtask ci compatibility --release --lane \"$LANE\" --toolchain \"$TOOLCHAIN\"").unwrap();
        require_block(source, "      - name: Validate candidate or frozen contracts", "      - name: Validate candidate or frozen contracts\n        if: ${{ !inputs.release-check }}\n        env:\n          EXPECTED_SHA: ${{ github.event.pull_request.head.sha || github.sha }}\n          PREPARING_CONSUMERS: ${{ ((github.event_name == 'push' && github.ref == 'refs/heads/main') || (github.event_name == 'pull_request' && github.base_ref == 'main')) && (matrix.lane == 'native' || matrix.lane == 'msrv') && 'api-hosts' || 'local' }}\n        run: target/debug/whatsapp-xtask ci compatibility --lane \"$LANE\" --toolchain \"$TOOLCHAIN\" --preparing-consumers \"$PREPARING_CONSUMERS\"").unwrap();
        require_block(source, "      - name: Qualify distributed artifacts outside the checkout", "      - name: Qualify distributed artifacts outside the checkout\n        run: target/debug/whatsapp-xtask ci package-consumers --lane \"$LANE\" --toolchain \"$TOOLCHAIN\"").unwrap();
        require_block(source, "      - name: Prove SDK and host breaks fail downstream compilation", "      - name: Prove SDK and host breaks fail downstream compilation\n        if: matrix.lane != 'msrv'\n        run: target/debug/whatsapp-xtask ci compatibility-controls --lane \"$LANE\" --toolchain \"$TOOLCHAIN\"").unwrap();
    }

    #[test]
    fn owner_runs_the_complete_registry_on_the_same_head_and_compiler() {
        verify_owner(OWNER, Lane::Native, "1.100.0", "1.100.0").unwrap();
        verify_owner(OWNER, Lane::Msrv, "1.95.0", "1.100.0").unwrap();
        assert!(verify_owner(OWNER, Lane::Native, "1.95.0", "1.100.0").is_err());
        assert!(verify_owner(OWNER, Lane::Msrv, "1.100.0", "1.100.0").is_err());
        assert!(verify_owner(OWNER, Lane::Wasm, "1.95.0", "1.100.0").is_err());
    }

    #[test]
    fn delegation_refuses_filtered_disabled_partial_or_different_head_owners() {
        for (before, after) in [
            ("  push:\n", "  push:\n    paths: ['src/**']\n"),
            (
                "  pull_request:\n",
                "  pull_request:\n    paths-ignore: ['tools/**']\n",
            ),
            ("    branches: [main]", "    branches: [other]"),
            ("  workflow_call:", "  workflow_dispatch:"),
            ("  consumers:\n", "  consumers:\n    if: false\n"),
            ("lane: [native, msrv]", "lane: [native]"),
            (
                "ref: ${{ github.event.pull_request.head.sha || github.sha }}",
                "ref: main",
            ),
            ("if: matrix.lane == 'native'", "if: false"),
            (
                "--lane native --toolchain 1.100.0",
                "--lane native --toolchain 1.100.0 --manifest tests/api-consumer/Cargo.toml",
            ),
            (
                "--lane native --toolchain 1.100.0",
                "--lane native --toolchain 1.100.0 --dry-run",
            ),
            (
                "run: target/debug/whatsapp-xtask ci consumers run --lane native --toolchain 1.100.0",
                "run: true",
            ),
        ] {
            assert!(OWNER.contains(before), "missing mutation anchor {before}");
            assert!(
                verify_owner(
                    &OWNER.replace(before, after),
                    Lane::Native,
                    "1.100.0",
                    "1.100.0"
                )
                .is_err(),
                "accepted owner mutation {after}"
            );
        }
    }

    #[test]
    fn only_identical_main_push_and_pr_heads_can_delegate() {
        let head = "a".repeat(40);
        verify_context("push", "refs/heads/main", None, &head, &head).unwrap();
        verify_context(
            "pull_request",
            "refs/pull/1/merge",
            Some("main"),
            &head,
            &head,
        )
        .unwrap();
        for (event, git_ref, base_ref) in [
            ("schedule", "refs/heads/main", None),
            ("workflow_dispatch", "refs/heads/main", None),
            ("push", "refs/tags/v1", None),
            ("push", "refs/heads/other", None),
            ("pull_request", "refs/pull/1/merge", Some("other")),
        ] {
            assert!(verify_context(event, git_ref, base_ref, &head, &head).is_err());
        }
        assert!(verify_context("push", "refs/heads/main", None, &head, &"b".repeat(40)).is_err());
        assert!(verify_context("push", "refs/heads/main", None, "main", "main").is_err());
    }

    #[test]
    fn msrv_owner_cannot_disable_or_narrow_its_registered_profiles() {
        for (before, after) in [
            ("if: matrix.lane == 'msrv'", "if: false"),
            (
                "--lane msrv --toolchain 1.95.0",
                "--lane msrv --toolchain 1.100.0",
            ),
            (
                "--lane msrv --toolchain 1.95.0",
                "--lane msrv --toolchain 1.95.0 --manifest tests/api-consumer/Cargo.toml",
            ),
        ] {
            assert!(OWNER.contains(before));
            assert!(
                verify_owner(
                    &OWNER.replace(before, after),
                    Lane::Msrv,
                    "1.95.0",
                    "1.100.0"
                )
                .is_err()
            );
        }
    }
}
