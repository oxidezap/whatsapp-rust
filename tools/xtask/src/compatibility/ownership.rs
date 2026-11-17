use anyhow::{Context, Result, ensure};
use clap::ValueEnum;
use std::path::Path;
use std::process::Command;
use xtask_support::capture;

use crate::consumers::Lane;
mod results;

pub fn verify_result(root: &Path, lane: Lane, head: &str, toolchain: &str) -> Result<()> {
    results::verify(root, lane, head, toolchain)
}

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
    verify_wrapper(&head_file(root, ".github/workflows/consumers.yml")?)?;
    let source = head_file(root, ".github/workflows/consumer-hosts.yml")?;
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

fn lane_name(lane: Lane) -> Result<&'static str> {
    match lane {
        Lane::Native => Ok("native"),
        Lane::Msrv => Ok("msrv"),
        Lane::Wasm => {
            anyhow::bail!("WASM checkout consumers retain both maintained compiler qualifications")
        }
    }
}

fn verify_wrapper(source: &str) -> Result<()> {
    require_block(
        source,
        "on:",
        "on:\n  push:\n    branches: [main]\n  pull_request:\n    branches: [main]\n  workflow_call:",
    )?;
    require_block(
        source,
        "jobs:",
        "jobs:\n  qualify:\n    name: API hosts\n    uses: ./.github/workflows/consumer-hosts.yml",
    )
}

pub(super) fn verify_execution(source: &str, pin: &str, lane: Lane, toolchain: &str) -> Result<()> {
    let pin: toml::Value = toml::from_str(pin)?;
    let driver = pin["toolchain"]["channel"]
        .as_str()
        .context("execution driver toolchain pin")?;
    verify_owner(source, lane, toolchain, driver)
}

fn verify_owner(source: &str, lane: Lane, toolchain: &str, driver: &str) -> Result<()> {
    let lane_name = lane_name(lane)?;
    require_block(source, "on:", "on:\n  workflow_call:")?;
    require_block(
        source,
        "env:",
        "env:\n  EXPECTED_SHA: ${{ github.event.pull_request.head.sha || github.sha }}\n  CARGO_TERM_COLOR: always\n  CARGO_BUILD_JOBS: \"1\"\n  CARGO_INCREMENTAL: \"0\"\n  RUSTFLAGS: \"\"\n  CARGO_ENCODED_RUSTFLAGS: \"\"",
    )?;
    let job = block(source, "  consumers:")?;
    let header = job.split("    steps:").next().context("API host steps")?;
    ensure!(
        header.trim_end()
            == "  consumers:\n    name: Independent API hosts (${{ matrix.lane }})\n    runs-on: ubuntu-latest\n    strategy:\n      fail-fast: false\n      matrix:\n        lane: [native, msrv]",
        "API host job matrix or conditions changed; review delegated coverage"
    );
    ensure!(
        job.lines()
            .filter(|line| line.contains("uses: actions/checkout@"))
            .count()
            == 1,
        "API host job must contain exactly one checkout"
    );
    require_block(
        &job,
        "      - uses: actions/checkout@v7",
        "      - uses: actions/checkout@v7\n        with:\n          ref: ${{ github.event.pull_request.head.sha || github.sha }}",
    )?;
    require_block(
        &job,
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
        &job,
        &format!("      - name: {name}"),
        &format!(
            "      - name: {name}\n        if: matrix.lane == '{lane_name}'\n        env:\n          CARGO_BUILD_JOBS: \"2\"\n        run: target/debug/whatsapp-xtask ci consumers run --lane {lane_name} --toolchain {toolchain} --expected-head \"$EXPECTED_SHA\""
        ),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    const OWNER: &str = include_str!("../../../../.github/workflows/consumer-hosts.yml");

    #[test]
    fn wrapper_events_and_relative_execution_revision_are_fixed() {
        let source = include_str!("../../../../.github/workflows/consumers.yml");
        verify_wrapper(source).unwrap();
        for mutation in [
            source.replace("  push:", "  push:\n    paths: ['src/**']"),
            source.replace(
                "./.github/workflows/consumer-hosts.yml",
                "example/project/.github/workflows/consumer-hosts.yml@main",
            ),
        ] {
            assert!(verify_wrapper(&mutation).is_err());
        }
    }

    #[test]
    fn named_second_checkout_cannot_change_qualified_source() {
        let source = OWNER.replace("      - name: Build driver", "      - name: Replace source\n        uses: actions/checkout@v7\n        with:\n          ref: main\n      - name: Build driver");
        assert!(verify_owner(&source, Lane::Native, "1.100.0", "1.100.0").is_err());
    }

    #[test]
    fn supply_chain_binds_delegation_to_the_same_events_and_head() {
        let source = include_str!("../../../../.github/workflows/supply-chain.yml");
        require_block(source, "      - name: Validate contracts before release", "      - name: Validate contracts before release\n        if: inputs.release-check\n        run: target/debug/whatsapp-xtask ci compatibility --release --lane \"$LANE\" --toolchain \"$TOOLCHAIN\"").unwrap();
        require_block(source, "      - name: Validate candidate or frozen contracts", "      - name: Validate candidate or frozen contracts\n        if: ${{ !inputs.release-check }}\n        run: target/debug/whatsapp-xtask ci compatibility --lane \"$LANE\" --toolchain \"$TOOLCHAIN\" --preparing-consumers \"$PREPARING_CONSUMERS\"").unwrap();
        require_block(source, "    env:", "    env:\n      LANE: ${{ matrix.lane }}\n      TOOLCHAIN: ${{ matrix.toolchain }}\n      EXPECTED_SHA: ${{ github.event.pull_request.head.sha || github.sha }}\n      PREPARING_CONSUMERS: ${{ ((github.event_name == 'push' && github.ref == 'refs/heads/main') || (github.event_name == 'pull_request' && github.base_ref == 'main')) && (matrix.lane == 'native' || matrix.lane == 'msrv') && 'api-hosts' || 'local' }}\n      CARGO_INCREMENTAL: \"0\"\n      CARGO_PROFILE_DEV_DEBUG: \"1\"").unwrap();
        require_block(
            source,
            "    permissions:",
            "    permissions:\n      contents: read\n      actions: read",
        )
        .unwrap();
        require_block(source, "      - name: Qualify distributed artifacts outside the checkout", "      - name: Qualify distributed artifacts outside the checkout\n        run: target/debug/whatsapp-xtask ci package-consumers --lane \"$LANE\" --toolchain \"$TOOLCHAIN\"").unwrap();
        require_block(source, "      - name: Prove SDK and host breaks fail downstream compilation", "      - name: Prove SDK and host breaks fail downstream compilation\n        if: matrix.lane != 'msrv'\n        run: target/debug/whatsapp-xtask ci compatibility-controls --lane \"$LANE\" --toolchain \"$TOOLCHAIN\"").unwrap();
        require_block(source, "      - name: Verify delegated checkout consumer result", "      - name: Verify delegated checkout consumer result\n        env:\n          GH_TOKEN: ${{ github.token }}\n        run: target/debug/whatsapp-xtask ci compatibility-owner-result --lane \"$LANE\" --toolchain \"$TOOLCHAIN\" --preparing-consumers \"$PREPARING_CONSUMERS\"").unwrap();
        let controls = source
            .find("      - name: Prove SDK and host breaks fail downstream compilation")
            .unwrap();
        let verify = source
            .find("      - name: Verify delegated checkout consumer result")
            .unwrap();
        let record = source
            .find("      - name: Record exact source commit")
            .unwrap();
        assert!(controls < verify && verify < record);
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

    #[test]
    fn native_run_moved_into_a_disabled_job_is_not_an_owner() {
        let start = OWNER
            .find("      - name: Run isolated native profiles and shared negative controls")
            .unwrap();
        let end = OWNER
            .find("      - name: Run isolated MSRV profiles and shared negative controls")
            .unwrap();
        let native = &OWNER[start..end];
        let moved = format!(
            "{}\n  disabled:\n    if: false\n    runs-on: ubuntu-latest\n    steps:\n{native}",
            OWNER.replace(native, "")
        );
        assert!(verify_owner(&moved, Lane::Native, "1.100.0", "1.100.0").is_err());
    }

    #[test]
    fn release_caller_grants_the_qualification_workflow_read_permissions() {
        let source = include_str!("../../../../.github/workflows/release.yml");
        require_block(source, "  validate-compatibility:", "  validate-compatibility:\n    name: Validate release contracts and packaged consumers\n    permissions:\n      contents: read\n      actions: read\n    uses: ./.github/workflows/supply-chain.yml\n    with:\n      release-check: true").unwrap();
    }
}
